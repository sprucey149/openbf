//! `[ours]` Keeps the display awake while a round is played (playtest A1).
//!
//! Pads do not count as seat input, so a pad-only round lets the desktop's
//! idle timer dim and sleep the screen. While a round runs unpaused the
//! window asks the platform to stay awake: the desktop session bus on Wayland,
//! a periodic screen-saver reset on X11, and `SetThreadExecutionState` on
//! Windows. Menus, loading, the pause menu and exit release it.
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
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

    pub(super) enum Imp {
        Wayland(wl::Inhibit),
        X11(Box<x11::Reset>),
    }

    impl Imp {
        pub(super) fn new(window: &winit::window::Window) -> Option<Imp> {
            match window.display_handle().ok()?.as_raw() {
                RawDisplayHandle::Wayland(_) => wl::Inhibit::new().map(Imp::Wayland),
                RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_) => {
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
        use std::sync::mpsc::{self, Receiver, Sender};
        use std::time::Duration;
        use zbus::blocking::{connection, Connection, Proxy};

        const SERVICE: &str = "org.freedesktop.ScreenSaver";
        const PATH: &str = "/org/freedesktop/ScreenSaver";

        pub struct Inhibit(Sender<bool>);

        impl Inhibit {
            pub fn new() -> Option<Self> {
                let (tx, rx) = mpsc::channel();
                // Desktop service startup and calls can block. Own the session
                // connection on a worker, away from winit's event loop.
                std::thread::Builder::new()
                    .name("openbf-idle".into())
                    .spawn(move || run(rx))
                    .ok()?;
                Some(Self(tx))
            }

            pub fn set(&self, hold: bool) {
                let _ = self.0.send(hold);
            }
        }

        struct Hold {
            connection: Connection,
            cookie: u32,
        }

        impl Hold {
            fn acquire() -> zbus::Result<Self> {
                let connection = connection::Builder::session()?
                    .method_timeout(Duration::from_secs(1))
                    .build()?;
                let cookie =
                    Self::proxy(&connection)?.call("Inhibit", &("openbf", "Playing a battle"))?;
                Ok(Self { connection, cookie })
            }

            fn proxy(connection: &Connection) -> zbus::Result<Proxy<'_>> {
                Proxy::new(connection, SERVICE, PATH, SERVICE)
            }

            fn release(self) -> zbus::Result<()> {
                let result: zbus::Result<()> = Self::proxy(&self.connection)
                    .and_then(|proxy| proxy.call("UnInhibit", &(self.cookie,)));
                // The service also releases this sender's cookie on disconnect.
                result.and(self.connection.close())
            }
        }

        fn run(changes: Receiver<bool>) {
            let mut held = None;
            let mut reported = false;
            while let Ok(hold) = changes.recv() {
                match (changes.try_iter().last().unwrap_or(hold), &held) {
                    (true, None) => match Hold::acquire() {
                        Ok(token) => held = Some(token),
                        Err(error) if !reported => {
                            eprintln!(
                                "idle inhibit: Wayland desktop service unavailable ({error})"
                            );
                            reported = true;
                        }
                        Err(_) => {}
                    },
                    (false, Some(_)) => {
                        let _ = held.take().unwrap().release();
                    }
                    _ => {}
                }
            }
            if let Some(held) = held {
                let _ = held.release();
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            #[ignore = "requires a desktop session with the idle inhibition service"]
            fn wayland_idle_service_acquires_and_releases() {
                Hold::acquire().unwrap().release().unwrap();
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
