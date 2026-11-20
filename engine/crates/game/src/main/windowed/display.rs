//! Issue #172: the display mode. The profile's Windowed / Fullscreen /
//! Exclusive choice is applied when the window opens, from the Graphics row,
//! from the toggle key, and after a profile switch. A new profile and a
//! first launch open Borderless fullscreen; the toggle leaves fullscreen for
//! a maximized window.
//!
//! One toggle per OS, everywhere (shell, loading, spawn screens, battle; the
//! maintainer, 2026-11-14): F11 on Windows and Linux, Control+Command+F on
//! macOS (its system shortcut; F11 is Show Desktop there). Alt+Enter is not
//! a toggle: Left Alt is the flyer trick. On macOS Command is the system's
//! modifier, so the battle never sees a key pressed with it, and a Control
//! press waits [`game::kbm::MAC_CHORD_WAIT`] before it rolls in case Command
//! follows (`Kbm`'s macOS chord rule).
//!
//! A change the system makes is read back after a resize: in practice only
//! the macOS green button and a Wayland compositor's own fullscreen
//! shortcut, since winit's X11 and Windows backends report the window's
//! last request rather than its state.

use super::App;
use ui::menu::WindowMode;
use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::Fullscreen;

/// OURS: how long a requested mode may take to show in the window's own
/// state before a different report is taken as the system's choice
/// (compositors answer a fullscreen request asynchronously; macOS animates
/// into its own Space). measure: the time to the first matching Resized on
/// each OS.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Default)]
pub(super) struct Display {
    /// The modifier keys held (Control+Command+F on macOS).
    mods: Mods,
    /// The last requested mode, until the window reports it (or [`SETTLE`]).
    pending: Option<(WindowMode, std::time::Instant)>,
    /// `OPENBF_WINDOW_SIZE` keeps the window as it opened until a mode is
    /// chosen in this session; the saved mode stays as saved.
    pub(super) forced: bool,
    /// This display can take an exclusive video mode (not Wayland).
    pub(super) exclusive: bool,
    /// A pause menu's choice, applied at the next frame (the battle holds
    /// the App while its actions run).
    pub(super) requested: Option<WindowMode>,
    /// A mode chosen before a profile is picked (F11 on the title screen):
    /// that profile has nowhere to save, so the choice is handed to the
    /// profile picked next ([`hold_window_mode`]).
    pub(super) held: Option<WindowMode>,
}

impl Display {
    /// The mode the window is meant to be in: a forced size is a window.
    pub(super) fn shown(&self, setting: WindowMode) -> WindowMode {
        if self.forced {
            WindowMode::Windowed
        } else {
            setting
        }
    }

    /// The mode the window is in, or is on its way to: a request not yet
    /// shown (a Wayland compositor answers with a later configure) counts as
    /// done, so a second toggle before it lands is still sent.
    fn current(&self, reported: WindowMode, now: std::time::Instant) -> WindowMode {
        match self.pending {
            Some((want, at)) if now.saturating_duration_since(at) < SETTLE => want,
            _ => reported,
        }
    }

    /// The mode the window reports after a resize, against the saved one:
    /// the system's change to keep, if it made one.
    fn readback(
        &mut self,
        setting: WindowMode,
        reported: WindowMode,
        now: std::time::Instant,
    ) -> Option<WindowMode> {
        if let Some((want, at)) = self.pending {
            if want.is_fullscreen() == reported.is_fullscreen() {
                self.pending = None;
                return None;
            }
            if now.saturating_duration_since(at) < SETTLE {
                return None;
            }
            self.pending = None;
        }
        // Exclusive shown as a borderless fallback is still the choice.
        (setting.is_fullscreen() != reported.is_fullscreen()).then_some(reported)
    }
}

/// The modifier keys the toggle reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Mods {
    pub ctrl: bool,
    /// Command on macOS, the Windows key elsewhere.
    pub logo: bool,
}

/// This build's OS uses Control+Command+F for fullscreen.
pub(super) const MAC: bool = game::kbm::MAC;

/// The toggle key, one per OS and live everywhere, battle included: F11 on
/// Windows and Linux; on macOS (`mac`) Control+Command+F, and F11 is not a
/// toggle there (the system's Show Desktop). Enter with Alt is never one.
pub(super) fn is_toggle_key(code: KeyCode, mods: Mods, mac: bool) -> bool {
    if mac {
        mods.ctrl && mods.logo && code == KeyCode::KeyF
    } else {
        code == KeyCode::F11
    }
}

/// The window requests for showing `mode`: the fullscreen request, and
/// whether the window is then maximized. Leaving fullscreen maximizes, so the
/// toggle and the Display row never land on the 1280x720 restore size.
pub(super) fn window_requests(
    mode: WindowMode,
    monitor: Option<MonitorHandle>,
    exclusive: bool,
) -> (Option<Fullscreen>, bool) {
    (fullscreen_for(mode, monitor, exclusive), maximizes(mode))
}

/// Whether leaving for `mode` also maximizes the window: the toggle and the
/// Display row take fullscreen back to a maximized window, never to the
/// 1280x720 size it restores to (the maintainer, 2026-11-14).
pub(super) fn maximizes(mode: WindowMode) -> bool {
    mode == WindowMode::Windowed
}

/// Exclusive needs a display that sets video modes: Wayland has none.
pub(super) fn exclusive_supported(
    display: &impl winit::raw_window_handle::HasDisplayHandle,
) -> bool {
    !matches!(
        display.display_handle().map(|h| h.as_raw()),
        Ok(winit::raw_window_handle::RawDisplayHandle::Wayland(_))
    )
}

/// The video mode Exclusive takes, from (size, refresh in mHz, bit depth):
/// the monitor's native size at its highest refresh rate, else the largest.
pub(super) fn best_video_mode(
    native: (u32, u32),
    modes: &[((u32, u32), u32, u16)],
) -> Option<usize> {
    let key = |&(size, refresh, depth): &((u32, u32), u32, u16)| {
        (
            size == native,
            u64::from(size.0) * u64::from(size.1),
            refresh,
            depth,
        )
    };
    (0..modes.len()).max_by_key(|&i| key(&modes[i]))
}

/// winit's request for `mode`: Fullscreen covers the window's monitor;
/// Exclusive falls back to it without a monitor, a video mode or support.
pub(super) fn fullscreen_for(
    mode: WindowMode,
    monitor: Option<MonitorHandle>,
    exclusive: bool,
) -> Option<Fullscreen> {
    match mode {
        WindowMode::Windowed => None,
        WindowMode::Borderless => Some(Fullscreen::Borderless(None)),
        WindowMode::Exclusive => {
            let video = monitor.filter(|_| exclusive).and_then(|m| {
                let native = m.size();
                let modes: Vec<_> = m.video_modes().collect();
                let keys: Vec<_> = modes
                    .iter()
                    .map(|v| {
                        let s = v.size();
                        (
                            (s.width, s.height),
                            v.refresh_rate_millihertz(),
                            v.bit_depth(),
                        )
                    })
                    .collect();
                let i = best_video_mode((native.width, native.height), &keys)?;
                modes.into_iter().nth(i)
            });
            Some(video.map_or(Fullscreen::Borderless(None), Fullscreen::Exclusive))
        }
    }
}

/// The mode a window is in, by its own report.
pub(super) fn reported(fullscreen: Option<&Fullscreen>) -> WindowMode {
    match fullscreen {
        None => WindowMode::Windowed,
        Some(Fullscreen::Borderless(_)) => WindowMode::Borderless,
        Some(Fullscreen::Exclusive(_)) => WindowMode::Exclusive,
    }
}

impl App {
    /// The mode the window is meant to be in: a forced size is a window.
    pub(super) fn shown_window_mode(&self) -> WindowMode {
        self.display.shown(self.profile.settings.window_mode)
    }

    /// The toggle keys ([`is_toggle_key`]), after a binding capture or a
    /// name edit has had the keyboard: true when the key was the toggle's, so
    /// menus, egui and the gameplay bindings never see it (Enter does not
    /// also confirm). Live play takes the same key.
    pub(super) fn display_hotkey(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::ModifiersChanged(m) => {
                let state = m.state();
                self.display.mods = Mods {
                    ctrl: state.control_key(),
                    logo: state.super_key(),
                };
                false
            }
            WindowEvent::Focused(false) => {
                self.display.mods = Mods::default();
                false
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return false;
                };
                if !is_toggle_key(code, self.display.mods, MAC) {
                    return false;
                }
                if !event.repeat {
                    let last = self.profile.settings.last_fullscreen;
                    self.set_window_mode(self.shown_window_mode().toggled(last));
                }
                true
            }
            _ => false,
        }
    }

    /// A mode chosen here (the Graphics row, F11): saved and applied.
    pub(super) fn set_window_mode(&mut self, mode: WindowMode) {
        self.record_window_mode(mode);
        self.display.forced = false;
        self.apply_window_mode();
    }

    /// Put `mode` in the current profile's settings and save it there.
    ///
    /// Before the first profile is picked there is no current profile (the
    /// shell's default has no catalog slot), so the choice is held for the
    /// profile picked next ([`hold_window_mode`]). After backing out to the
    /// title screen the profile picked before is still the current one and
    /// keeps the choice, like every other setting; picking another profile
    /// then applies that profile's own mode.
    fn record_window_mode(&mut self, mode: WindowMode) {
        let s = &mut self.profile.settings;
        s.window_mode = mode;
        if mode.is_fullscreen() {
            s.last_fullscreen = mode;
        }
        if self.profile.persists() {
            if let Err(e) = self.profile.save() {
                eprintln!("profile autosave failed: {e:#}");
            }
        } else {
            self.display.held = Some(mode);
        }
    }

    /// Put the window in the profile's mode (a profile switch, a choice).
    pub(super) fn apply_window_mode(&mut self) {
        let mode = self.shown_window_mode();
        let Some(w) = &self.window else {
            return;
        };
        let now = std::time::Instant::now();
        let current = self.display.current(reported(w.fullscreen().as_ref()), now);
        if self.display.forced || current == mode {
            return;
        }
        let monitor = w
            .current_monitor()
            .or_else(|| w.primary_monitor())
            .or_else(|| w.available_monitors().next());
        let (request, maximize) = window_requests(mode, monitor, self.display.exclusive);
        println!(
            "display: {} ({})",
            mode.label(),
            match &request {
                None => "window".to_string(),
                Some(Fullscreen::Borderless(_)) => "borderless".to_string(),
                Some(Fullscreen::Exclusive(v)) => format!(
                    "{}x{} at {} mHz",
                    v.size().width,
                    v.size().height,
                    v.refresh_rate_millihertz()
                ),
            }
        );
        w.set_fullscreen(request);
        if maximize {
            w.set_maximized(true);
        }
        self.display.pending = Some((mode, now));
        self.display_changed();
    }

    /// After a resize: keep a mode the system chose. A request that has
    /// now landed also reads the refresh rate and the grab again, as the
    /// mode the window settled in may differ from the one it was asked for.
    pub(super) fn display_resized(&mut self) {
        let Some(w) = &self.window else {
            return;
        };
        if self.display.forced {
            return;
        }
        let now = reported(w.fullscreen().as_ref());
        let setting = self.profile.settings.window_mode;
        let was_pending = self.display.pending.is_some();
        let Some(mode) = self
            .display
            .readback(setting, now, std::time::Instant::now())
        else {
            if was_pending && self.display.pending.is_none() {
                self.display_changed();
            }
            return;
        };
        println!("display: the system chose {}", mode.label());
        self.record_window_mode(mode);
        self.display_changed();
    }

    /// A new mode may bring a new refresh rate and drop the pointer grab:
    /// the governor reads its budget again and the next sync grabs afresh.
    fn display_changed(&mut self) {
        self.dyn_res_budget_ms = None;
        self.kbm.forget_grab();
        // Issue #182: the system may rebuild the cursor after the change.
        if let Some(o) = &mut self.overlay {
            o.display_changed();
        }
    }

    /// The window opened in `mode`: its first reports are not the system's.
    pub(super) fn display_opened(&mut self, mode: WindowMode) {
        self.display.pending = Some((mode, std::time::Instant::now()));
    }
}

/// Hand a mode chosen before any profile was picked to `profile`, the one
/// just picked, and save it there.
pub(super) fn hold_window_mode(
    held: &mut Option<WindowMode>,
    profile: &mut game::profile::Profile,
) {
    let Some(mode) = held.take() else {
        return;
    };
    let s = &mut profile.settings;
    s.window_mode = mode;
    if mode.is_fullscreen() {
        s.last_fullscreen = mode;
    }
    if let Err(e) = profile.save() {
        eprintln!("profile autosave failed: {e:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// One toggle per OS: F11 on Windows and Linux; on macOS only
    /// Control+Command+F (F11 is the system's Show Desktop). Alt+Enter is a
    /// toggle nowhere (Left Alt is the flyer trick).
    #[test]
    fn each_os_has_exactly_one_fullscreen_key() {
        let none = Mods::default();
        let ctrl = Mods { ctrl: true, ..none };
        let cmd = Mods { logo: true, ..none };
        let cmd_ctrl = Mods {
            ctrl: true,
            logo: true,
        };
        // Windows and Linux: F11, whatever is held; nothing else.
        for mods in [none, ctrl, cmd, cmd_ctrl] {
            assert!(is_toggle_key(KeyCode::F11, mods, false));
            assert!(!is_toggle_key(KeyCode::KeyF, mods, false));
            assert!(!is_toggle_key(KeyCode::Enter, mods, false));
            assert!(!is_toggle_key(KeyCode::NumpadEnter, mods, false));
        }
        // macOS: Control+Command+F only.
        assert!(is_toggle_key(KeyCode::KeyF, cmd_ctrl, true));
        for mods in [none, ctrl, cmd, cmd_ctrl] {
            assert!(
                !is_toggle_key(KeyCode::F11, mods, true),
                "F11 is Show Desktop"
            );
            assert!(!is_toggle_key(KeyCode::Enter, mods, true));
            assert!(!is_toggle_key(KeyCode::KeyG, mods, true));
        }
        for partial in [none, ctrl, cmd] {
            assert!(!is_toggle_key(KeyCode::KeyF, partial, true));
        }
    }

    /// The toggle takes fullscreen to a maximized window and back to the
    /// fullscreen kind it left.
    #[test]
    fn the_toggle_leaves_fullscreen_for_a_maximized_window() {
        use WindowMode::*;
        for full in [Borderless, Exclusive] {
            let out = full.toggled(full);
            assert_eq!(out, Windowed);
            // apply_window_mode's requests: leave fullscreen, then maximize.
            let (request, maximize) = window_requests(out, None, true);
            assert!(request.is_none());
            assert!(
                maximize,
                "leaving fullscreen must maximize, not restore 1280x720"
            );
            assert_eq!(out.toggled(full), full);
            let (request, maximize) = window_requests(full, None, true);
            assert!(request.is_some());
            assert!(!maximize);
        }
    }

    #[test]
    fn fullscreen_for_falls_back_to_borderless_without_exclusive_modes() {
        assert!(fullscreen_for(WindowMode::Windowed, None, true).is_none());
        assert_eq!(
            fullscreen_for(WindowMode::Borderless, None, true),
            Some(Fullscreen::Borderless(None))
        );
        for exclusive in [false, true] {
            assert_eq!(
                fullscreen_for(WindowMode::Exclusive, None, exclusive),
                Some(Fullscreen::Borderless(None))
            );
        }
        assert_eq!(best_video_mode((1920, 1080), &[]), None);
        let modes = [
            ((2560, 1440), 60_000, 32),
            ((1920, 1080), 60_000, 32),
            ((1920, 1080), 143_981, 24),
            ((1920, 1080), 143_981, 32),
            ((1280, 720), 240_000, 32),
        ];
        assert_eq!(best_video_mode((1920, 1080), &modes), Some(3));
        // No native-size mode: the largest, at its best rate.
        assert_eq!(best_video_mode((3840, 2160), &modes), Some(0));
    }

    #[test]
    fn the_window_report_maps_to_its_mode() {
        assert_eq!(reported(None), WindowMode::Windowed);
        assert_eq!(
            reported(Some(&Fullscreen::Borderless(None))),
            WindowMode::Borderless
        );
    }

    /// A system change (the macOS green button) is kept; the window's own
    /// reports while a request settles are not taken for one.
    #[test]
    fn a_system_mode_change_is_read_back_after_requests_settle() {
        use WindowMode::*;
        let t = Instant::now();
        let mut d = Display::default();
        assert_eq!(d.readback(Windowed, Borderless, t), Some(Borderless));
        assert_eq!(d.readback(Borderless, Windowed, t), Some(Windowed));
        assert_eq!(d.readback(Borderless, Borderless, t), None);
        // Exclusive shown as borderless is still Exclusive.
        assert_eq!(d.readback(Exclusive, Borderless, t), None);
        // Opened fullscreen: the first windowed resize is the request settling.
        d.pending = Some((Borderless, t));
        assert_eq!(d.readback(Borderless, Windowed, t), None);
        assert_eq!(d.readback(Borderless, Borderless, t), None);
        assert!(d.pending.is_none());
        assert_eq!(d.readback(Borderless, Windowed, t), Some(Windowed));
        // A request the system never honours: its report wins after SETTLE.
        d.pending = Some((Borderless, t));
        let late = t + SETTLE + Duration::from_millis(1);
        assert_eq!(d.readback(Borderless, Windowed, late), Some(Windowed));
        assert!(d.pending.is_none());
    }

    /// Wayland shows a request only at the compositor's next configure: a
    /// second toggle before then compares against the request, not the
    /// window's stale report, so it is sent and the window ends as saved.
    #[test]
    fn a_fast_second_toggle_compares_against_the_pending_request() {
        use WindowMode::*;
        let t = Instant::now();
        let mut d = Display::default();
        assert_eq!(d.current(Windowed, t), Windowed);
        // F11: Fullscreen requested; the window still reports a window.
        d.pending = Some((Borderless, t));
        assert_eq!(d.current(Windowed, t), Borderless);
        // F11 again before the configure: Windowed differs from the
        // request, so it is sent (the window's report alone would drop it).
        assert_ne!(d.current(Windowed, t), Windowed);
        d.pending = Some((Windowed, t));
        // The first request's configure lands, then the second's.
        assert_eq!(d.readback(Windowed, Borderless, t), None, "still settling");
        assert_eq!(d.pending, Some((Windowed, t)));
        assert_eq!(d.readback(Windowed, Windowed, t), None);
        assert!(d.pending.is_none());
        assert_eq!(d.current(Windowed, t), Windowed);
        // A request the compositor ignored stops counting after SETTLE.
        d.pending = Some((Borderless, t));
        let late = t + SETTLE + Duration::from_millis(1);
        assert_eq!(d.current(Windowed, late), Windowed);
    }
}
