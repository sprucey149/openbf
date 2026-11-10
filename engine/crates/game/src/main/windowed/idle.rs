//! `[ours]` Keeps the display awake while a round is played (playtest A1).
//!
//! Pads do not count as seat input, so a pad-only round lets the desktop's
//! idle timer dim and sleep the screen. While a round runs unpaused the
//! window asks the platform to stay awake: the Wayland idle-inhibit protocol
//! on the window's surface, a periodic screen-saver reset on X11, and
//! `SetThreadExecutionState` on Windows. Menus, loading, the pause menu and
//! exit release it. Every backend fails silently: no inhibit, no error.
//! macOS is a no-op for now (follow-up: an IOPM assertion).

/// Whether the display should be kept awake: only in a round being played.
pub(super) fn wants_idle_inhibit(in_battle: bool, paused: bool) -> bool {
    in_battle && !paused
}

pub(super) struct IdleInhibit {
    held: bool,
    imp: Option<platform::Imp>,
}

impl IdleInhibit {
    /// The window's platform backend, or a no-op when none is available.
    pub(super) fn new(window: &winit::window::Window) -> IdleInhibit {
        let imp = platform::Imp::new(window);
        if imp.is_none() {
            eprintln!("idle inhibit: unavailable here; the display may sleep during a pad round");
        }
        IdleInhibit { held: false, imp }
    }

    #[cfg(test)]
    pub(super) fn none() -> IdleInhibit {
        IdleInhibit {
            held: false,
            imp: None,
        }
    }

    #[cfg(test)]
    pub(super) fn held(&self) -> bool {
        self.held
    }

    /// Hold or release; called every frame from the event-loop thread.
    pub(super) fn set(&mut self, hold: bool) {
        let changed = hold != self.held;
        self.held = hold;
        if let Some(imp) = self.imp.as_mut() {
            if changed {
                imp.set(hold);
            } else if hold {
                imp.tick();
            }
        }
    }
}

impl Drop for IdleInhibit {
    fn drop(&mut self) {
        // Unwinding, the event loop (and the display connection the
        // backend borrows) may be gone: the compositor or server drops the
        // inhibit with the connection.
        if std::thread::panicking() {
            return;
        }
        self.set(false);
    }
}

#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "redox"
    ))
))]
mod platform {
    use winit::raw_window_handle::{
        HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    pub(super) enum Imp {
        Wayland(Box<wl::Inhibit>),
        X11(Box<x11::Reset>),
    }

    impl Imp {
        pub(super) fn new(window: &winit::window::Window) -> Option<Imp> {
            let display = window.display_handle().ok()?.as_raw();
            let surface = window.window_handle().ok()?.as_raw();
            match (display, surface) {
                (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => {
                    // SAFETY: winit owns the display and the surface and keeps
                    // both alive while the window exists; the app drops this
                    // inhibitor in `exiting()`, before the window goes.
                    unsafe { wl::Inhibit::new(d.display, w.surface) }
                        .map(|i| Imp::Wayland(Box::new(i)))
                }
                (RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_), _) => {
                    x11::Reset::new().map(|r| Imp::X11(Box::new(r)))
                }
                _ => None,
            }
        }

        pub(super) fn set(&mut self, hold: bool) {
            match self {
                Imp::Wayland(i) => i.set(hold),
                Imp::X11(r) => r.set(hold),
            }
        }

        /// While held: backends that must repeat themselves do it here.
        pub(super) fn tick(&mut self) {
            if let Imp::X11(r) = self {
                r.tick();
            }
        }
    }

    mod wl {
        use std::ffi::c_void;
        use std::ptr::NonNull;
        use wayland_client::backend::{Backend, ObjectId};
        use wayland_client::globals::{registry_queue_init, GlobalListContents};
        use wayland_client::protocol::{wl_registry::WlRegistry, wl_surface::WlSurface};
        use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
        use wayland_protocols::wp::idle_inhibit::zv1::client::{
            zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1,
            zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
        };

        pub struct State;

        impl Dispatch<WlRegistry, GlobalListContents> for State {
            fn event(
                _: &mut State,
                _: &WlRegistry,
                _: <WlRegistry as Proxy>::Event,
                _: &GlobalListContents,
                _: &Connection,
                _: &QueueHandle<State>,
            ) {
            }
        }
        delegate_noop!(State: ZwpIdleInhibitManagerV1);
        delegate_noop!(State: ZwpIdleInhibitorV1);

        /// A private queue on winit's own connection: the manager global, the
        /// window's surface and, while held, the inhibitor on it.
        pub struct Inhibit {
            conn: Connection,
            queue: EventQueue<State>,
            manager: ZwpIdleInhibitManagerV1,
            surface: WlSurface,
            inhibitor: Option<ZwpIdleInhibitorV1>,
        }

        impl Inhibit {
            /// `None` when the compositor has no idle-inhibit global.
            ///
            /// # Safety
            /// `display` and `surface` are winit's live `wl_display` and
            /// `wl_surface`, and stay alive for as long as the result.
            pub unsafe fn new(display: NonNull<c_void>, surface: NonNull<c_void>) -> Option<Self> {
                let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
                let conn = Connection::from_backend(backend);
                let (globals, queue) = registry_queue_init::<State>(&conn).ok()?;
                let manager = globals
                    .bind::<ZwpIdleInhibitManagerV1, State, ()>(&queue.handle(), 1..=1, ())
                    .ok()?;
                let id =
                    unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr().cast()) }
                        .ok()?;
                let surface = WlSurface::from_id(&conn, id).ok()?;
                Some(Inhibit {
                    conn,
                    queue,
                    manager,
                    surface,
                    inhibitor: None,
                })
            }

            pub fn set(&mut self, hold: bool) {
                // Registry events land on this queue; drain them so they do not pile up.
                let _ = self.queue.dispatch_pending(&mut State);
                if hold {
                    if self.inhibitor.is_none() {
                        let qh = self.queue.handle();
                        self.inhibitor =
                            Some(self.manager.create_inhibitor(&self.surface, &qh, ()));
                    }
                } else if let Some(i) = self.inhibitor.take() {
                    i.destroy();
                }
                let _ = self.conn.flush();
            }
        }

        impl Drop for Inhibit {
            fn drop(&mut self) {
                if std::thread::panicking() {
                    return;
                }
                self.set(false);
                self.manager.destroy();
                let _ = self.conn.flush();
            }
        }
    }

    mod x11 {
        use std::time::{Duration, Instant};
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{ConnectionExt, ScreenSaver};
        use x11rb::rust_connection::RustConnection;

        /// Resetting the server's screen saver also resets the idle time the
        /// desktop's dimming reads, so a held reset repeats well inside the
        /// shortest dim delay a desktop offers (one minute).
        const EVERY: Duration = Duration::from_secs(30);

        pub struct Reset {
            conn: RustConnection,
            last: Option<Instant>,
        }

        impl Reset {
            pub fn new() -> Option<Self> {
                let (conn, _) = RustConnection::connect(None).ok()?;
                Some(Reset { conn, last: None })
            }

            pub fn set(&mut self, hold: bool) {
                self.last = None;
                if hold {
                    self.tick();
                }
            }

            pub fn tick(&mut self) {
                if self.last.is_some_and(|t| t.elapsed() < EVERY) {
                    return;
                }
                self.last = Some(Instant::now());
                if self.conn.force_screen_saver(ScreenSaver::RESET).is_ok() {
                    let _ = self.conn.flush();
                }
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
    const ES_CONTINUOUS: u32 = 0x8000_0000;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }

    pub(super) struct Imp;

    impl Imp {
        pub(super) fn new(_: &winit::window::Window) -> Option<Imp> {
            Some(Imp)
        }

        /// Per thread: always called from the event-loop thread.
        pub(super) fn set(&mut self, hold: bool) {
            let flags = if hold {
                ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
            } else {
                ES_CONTINUOUS
            };
            // SAFETY: a plain kernel32 call with flag bits only.
            unsafe {
                SetThreadExecutionState(flags);
            }
        }

        pub(super) fn tick(&mut self) {}
    }
}

#[cfg(not(any(
    windows,
    all(
        unix,
        not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "android",
            target_os = "redox"
        ))
    )
)))]
mod platform {
    /// macOS and the rest: no backend yet (follow-up: an IOPM assertion).
    pub(super) enum Imp {}

    impl Imp {
        pub(super) fn new(_: &winit::window::Window) -> Option<Imp> {
            None
        }

        pub(super) fn set(&mut self, _: bool) {
            match *self {}
        }

        pub(super) fn tick(&mut self) {
            match *self {}
        }
    }
}
