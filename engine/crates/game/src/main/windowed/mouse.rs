//! The window's native cursor and pointer lock. Keyboard + mouse is [ours].

use super::{App, Screen};
use winit::event::WindowEvent;
use winit::window::{Cursor, CursorIcon, CustomCursor, CustomCursorSource};

/// The menu cursor: an SVG in a 32-unit square, rendered at the display's size at startup
/// (OS cursors take bitmaps; rendering the vector at the exact size keeps it crisp).
const CURSOR_SVG: &str = include_str!("../../../assets/menu_cursor/menu_cursor.svg");
/// The click point in the SVG's 32-unit space.
const HOTSPOT: (f32, f32) = (3.0, 3.0);

/// The cursor's pixel size for display scale `scale`: 32 at 1x, never below 32.
fn cursor_px(scale: f64) -> u32 {
    (32.0 * scale.clamp(1.0, 8.0)).round() as u32
}

/// The cursor rendered at `px` x `px`: straight (not premultiplied) RGBA.
fn render_cursor(px: u32) -> anyhow::Result<Vec<u8>> {
    let tree = resvg::usvg::Tree::from_str(CURSOR_SVG, &resvg::usvg::Options::default())?;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(px, px).ok_or_else(|| anyhow::anyhow!("cursor size {px}"))?;
    let k = px as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(k, k),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.take_demultiplied())
}

fn cursor_source(px: u32) -> anyhow::Result<CustomCursorSource> {
    let side = u16::try_from(px)?;
    let k = px as f32 / 32.0;
    let hot = |h: f32| ((h * k).round() as u16).min(side - 1);
    Ok(CustomCursor::from_rgba(
        render_cursor(px)?,
        side,
        side,
        hot(HOTSPOT.0),
        hot(HOTSPOT.1),
    )?)
}

pub(super) fn menu_cursor(event_loop: &winit::event_loop::ActiveEventLoop, scale: f64) -> Cursor {
    match cursor_source(cursor_px(scale)) {
        Ok(source) => event_loop.create_custom_cursor(source).into(),
        Err(e) => {
            eprintln!("mouse: custom cursor unavailable ({e}); using the system arrow");
            CursorIcon::Default.into()
        }
    }
}

impl App {
    pub(super) fn in_battle(&self) -> bool {
        self.walk.is_some() && matches!(self.screen, Screen::Battle)
    }

    pub(super) fn can_grab(&self) -> bool {
        self.focused
            && self.in_battle()
            && !self.paused
            && self.walk.as_ref().is_some_and(|(walk, _)| {
                walk.sim.outcome.is_none()
                    && walk.sim.unit(walk.player).is_some_and(|u| {
                        let slot = u.spec.player_slot.map_or(0, usize::from);
                        u.alive
                            && walk.sim.spawn_stage.get(slot) != Some(&core::sim::SpawnStage::Alive)
                    })
            })
    }

    /// The focused player of a round in play is behind the spawn screens:
    /// dead (the death camera, the spawn pages), not yet on the field, or on
    /// a post's class screen (issue #28). The pause menu does not count.
    fn on_spawn_screen(&self) -> bool {
        self.focused
            && self.in_battle()
            && self.walk.as_ref().is_some_and(|(walk, _)| {
                walk.sim.outcome.is_none()
                    && !walk.sim.unit(walk.player).is_some_and(|u| {
                        let slot = u.spec.player_slot.map_or(0, usize::from);
                        u.alive
                            && walk.sim.spawn_stage.get(slot) != Some(&core::sim::SpawnStage::Alive)
                    })
            })
    }

    /// Issue #128: whether this frame hands mouse look back after a spawn.
    /// The spawn screens free the pointer for their own clicks; once the
    /// player is on the field again the grab returns without the click that
    /// would otherwise take it (`Kbm::click_in`), so no click is eaten and
    /// none fires: the spawn's own press came while the pointer was free.
    /// Only a mouse player's spawn does it: mouse look had the pointer as he
    /// went behind the screens, or the pointer accepted the spawn
    /// ([`App::note_pointer_spawn`]). A controller player's spawn leaves the
    /// pointer free, so a stray mouse move never takes his aim
    /// (`buttons::MOUSE_AIM` turns the aim assist off).
    /// Focus loss or leaving the round forgets it; a pause keeps it.
    fn regrab_after_spawn(&mut self, allowed: bool) -> bool {
        if self.on_spawn_screen() {
            if !self.behind_spawn_screens {
                // This sync runs before the grab is let go (`App::sync_grab`).
                self.behind_spawn_screens = true;
                self.spawn_regrab |= self.kbm.grabbed() || self.kbm.wants_grab();
            }
            return false;
        }
        self.behind_spawn_screens = false;
        if !self.focused || !self.in_battle() {
            self.spawn_regrab = false;
            return false;
        }
        allowed && std::mem::take(&mut self.spawn_regrab)
    }

    /// Issue #128: the pointer accepted a spawn screen, so the spawn hands
    /// mouse look back ([`App::regrab_after_spawn`]).
    pub(super) fn note_pointer_spawn(&mut self) {
        if self.on_spawn_screen() {
            self.spawn_regrab = true;
        }
    }

    pub(super) fn release_mouse(&mut self) {
        if let Some(w) = &self.window {
            self.kbm.release(w);
        }
        if let Some(o) = &mut self.overlay {
            o.set_cursor_visible(true);
        }
    }

    pub(super) fn mouse_window_event(&mut self, event: &WindowEvent) {
        self.kbm
            .window_event(event, self.in_battle(), self.can_grab());
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.shell_pointer
                    .moved_to([position.x as f32, position.y as f32]);
            }
            WindowEvent::CursorLeft { .. } => self.shell_pointer.clear(),
            WindowEvent::MouseInput {
                button: winit::event::MouseButton::Left,
                state: winit::event::ElementState::Pressed,
                ..
            } if matches!(self.screen, Screen::Shell | Screen::Lobby)
                || (self.in_battle() && !self.can_grab()) =>
            {
                self.shell_pointer.click();
            }
            WindowEvent::MouseWheel { delta, .. }
                if matches!(self.screen, Screen::Shell | Screen::Lobby)
                    || (self.in_battle() && self.paused) =>
            {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        ui::scroll::wheel_lines(*y, 0.0, ui::scroll::PX_PER_LINE)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        ui::scroll::wheel_lines(0.0, p.y as f32, ui::scroll::PX_PER_LINE)
                    }
                };
                self.shell_pointer.wheel(dy);
            }
            WindowEvent::Focused(focused) => {
                self.focused = *focused;
                if !focused {
                    self.kbm.release_keys();
                    self.release_mouse();
                    self.shell_pointer.clear();
                }
                self.sync_grab();
            }
            WindowEvent::CloseRequested => self.release_mouse(),
            _ => {}
        }
    }

    pub(super) fn sync_grab(&mut self) {
        let allowed = self.can_grab();
        if self.regrab_after_spawn(allowed) {
            self.kbm.click_in();
        }
        if let Some(w) = &self.window {
            self.kbm.sync_grab(w, allowed);
        }
        if let Some(o) = &mut self.overlay {
            o.set_cursor_visible(cursor_shown(allowed, self.kbm.grabbed()));
        }
    }
}

/// The menu cursor shows outside active gameplay only: a controller player who never
/// clicks to grab the mouse still plays without a pointer over the battle. [ours]
fn cursor_shown(active_gameplay: bool, grabbed: bool) -> bool {
    !active_gameplay && !grabbed
}

impl Drop for App {
    fn drop(&mut self) {
        self.finish_loading();
        self.release_mouse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let setup = super::super::tests::setup(None);
        App::new(
            Default::default(),
            Default::default(),
            setup,
            None,
            None,
            None,
            Default::default(),
        )
    }

    #[test]
    fn only_focused_live_gameplay_allows_capture() {
        let mut a = app();
        assert!(!a.can_grab(), "shell");
        a.screen = Screen::Loading;
        assert!(!a.can_grab(), "loading");
        a.screen = Screen::Battle;
        assert!(!a.can_grab(), "viewer without player");
        a.walk = Some((
            game::walk::WalkSim::new(42, glam::Vec3::ZERO, None),
            render::MeshHandle::default(),
        ));
        assert!(a.can_grab(), "live battle");
        a.screen = Screen::Shell;
        assert!(!a.can_grab(), "shell with a retained battle");
        a.screen = Screen::Loading;
        assert!(!a.can_grab(), "loading with a retained battle");
        a.screen = Screen::Battle;
        a.walk.as_mut().unwrap().0.sim.outcome = Some(0);
        assert!(!a.can_grab(), "round outcome");
        a.walk.as_mut().unwrap().0.sim.outcome = None;
        a.paused = true;
        assert!(!a.can_grab(), "pause");
        a.paused = false;
        a.mouse_window_event(&WindowEvent::Focused(false));
        assert!(!a.can_grab(), "focus loss, including later redraws");
        a.mouse_window_event(&WindowEvent::Focused(true));
        assert!(a.can_grab());
        let walk = &mut a.walk.as_mut().unwrap().0;
        walk.sim.spawn_stage[0] = core::sim::SpawnStage::Alive;
        assert!(!a.can_grab(), "live class selection");
        let walk = &mut a.walk.as_mut().unwrap().0;
        walk.sim.spawn_stage[0] = core::sim::SpawnStage::Accepted;
        walk.sim.units[walk.player.index as usize].alive = false;
        assert!(!a.can_grab(), "death and spawn screens");
    }

    /// The battle the spawn tests play: the walk's sim on the screens.
    fn battle() -> App {
        let mut a = app();
        a.screen = Screen::Battle;
        a.walk = Some((
            game::walk::WalkSim::new(42, glam::Vec3::ZERO, None),
            render::MeshHandle::default(),
        ));
        a
    }

    /// The window's grab call, as `App::sync_grab` makes it.
    fn window_grab(a: &mut App) {
        let allowed = a.can_grab();
        a.kbm.sync_grab_with(allowed, |_| Ok::<_, ()>(())).unwrap();
    }

    fn frame(a: &mut App) {
        a.sync_grab();
        window_grab(a);
    }

    fn set(a: &mut App, alive: bool, stage: core::sim::SpawnStage) {
        let walk = &mut a.walk.as_mut().unwrap().0;
        walk.sim.units[walk.player.index as usize].alive = alive;
        walk.sim.spawn_stage[0] = stage;
    }

    /// Issue #128: a mouse player's spawn hands mouse look straight back.
    /// Behind the spawn screens the pointer is free; on the field again the
    /// grab comes back with no click, so mouse motion turns the view at once,
    /// and the release of the click that spawned fires nothing. A focus loss
    /// on the way still waits for the player's click.
    #[test]
    fn a_spawn_hands_mouse_look_back_without_a_click() {
        use core::sim::{PadState, SpawnStage};
        let mut a = battle();
        // He plays with the mouse: a click took the pointer.
        set(&mut a, true, SpawnStage::Accepted);
        a.kbm.click_in();
        frame(&mut a);
        assert!(a.kbm.grabbed());
        for life in 0..3 {
            // Dead behind the class screen: the pointer is the menus'.
            set(&mut a, false, SpawnStage::Class);
            frame(&mut a);
            assert!(!a.kbm.grabbed(), "life {life}: free on the spawn screen");
            a.mouse_window_event(&WindowEvent::MouseInput {
                device_id: winit::event::DeviceId::dummy(),
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
            });
            // The accept spawns him.
            set(&mut a, true, SpawnStage::Accepted);
            frame(&mut a);
            assert!(
                a.kbm.grabbed(),
                "life {life}: mouse look back with no click"
            );
            a.mouse_window_event(&WindowEvent::MouseInput {
                device_id: winit::event::DeviceId::dummy(),
                state: winit::event::ElementState::Released,
                button: winit::event::MouseButton::Left,
            });
            assert_eq!(
                a.kbm.pad(PadState::default()).buttons & !core::sim::buttons::MOUSE_AIM,
                0,
                "life {life}: the spawn click's release fires nothing"
            );
            a.kbm.motion(40.0, 0.0);
            let pad = a.kbm.pad(PadState::default());
            assert_ne!(pad.mx, 0, "life {life}: the mouse turns the view");
            a.kbm.consumed(&pad, 1);
        }
        // Away from the window while he spawns: back on the field, the
        // player's click takes the pointer, as at any return to the window.
        set(&mut a, false, SpawnStage::Map);
        frame(&mut a);
        a.mouse_window_event(&WindowEvent::Focused(false));
        window_grab(&mut a);
        set(&mut a, true, SpawnStage::Accepted);
        frame(&mut a);
        a.mouse_window_event(&WindowEvent::Focused(true));
        frame(&mut a);
        assert!(!a.kbm.grabbed(), "focus came back on the field");
        // Pausing alive keeps the click-to-resume rule.
        a.kbm.click_in();
        frame(&mut a);
        assert!(a.kbm.grabbed());
        a.paused = true;
        frame(&mut a);
        a.paused = false;
        frame(&mut a);
        assert!(!a.kbm.grabbed(), "a pause is not a spawn");
    }

    /// Issue #128 (the maintainer plays on a controller): a controller
    /// player's spawn, his first or any later one, leaves the pointer free,
    /// so a stray mouse move neither turns his view nor marks his pad
    /// mouse-aimed (which would switch off his aim assist).
    #[test]
    fn a_controller_players_spawn_leaves_the_pointer_free() {
        use core::sim::{buttons::MOUSE_AIM, PadState, SpawnStage};
        let mut a = battle();
        set(&mut a, false, SpawnStage::Side);
        frame(&mut a);
        for life in 0..3 {
            for stage in [SpawnStage::Map, SpawnStage::Class] {
                set(&mut a, false, stage);
                frame(&mut a);
            }
            // The pad's accept spawns him.
            set(&mut a, true, SpawnStage::Accepted);
            frame(&mut a);
            frame(&mut a);
            assert!(!a.kbm.grabbed(), "life {life}: the pointer stays free");
            a.kbm.motion(40.0, 0.0);
            let pad = a.kbm.pad(PadState::default());
            assert_eq!((pad.mx, pad.buttons & MOUSE_AIM), (0, 0), "life {life}");
            a.kbm.consumed(&pad, 1);
            set(&mut a, false, SpawnStage::Class);
            frame(&mut a);
        }
    }

    /// Issue #128: a pointer accept on the spawn screens marks a mouse
    /// player, so even his first spawn of the round, with no click ever
    /// taken by mouse look, hands mouse look over.
    #[test]
    fn a_pointer_accepted_first_spawn_takes_the_pointer() {
        use core::sim::SpawnStage;
        let mut a = battle();
        set(&mut a, false, SpawnStage::Side);
        frame(&mut a);
        // The side screen's first team (selected) under the pointer, clicked
        // once the screen has settled: an accept.
        let size = [640.0, 480.0];
        let row = ui::spawn_draw::window_point(
            [
                (ui::spawn_draw::SIDE_STACK_X[0] + ui::spawn_draw::SIDE_STACK_X[1]) * 0.5,
                ui::spawn_draw::SIDE_STACK_TOP + ui::spawn_draw::ROW_PITCH + 10.0,
            ],
            size,
        );
        a.shell_pointer.moved_to(row);
        a.step_spawn_pointer_at(1.0, None);
        a.shell_pointer.click();
        a.step_spawn_pointer_at(1.0, None);
        assert_ne!(
            a.kbm.pad(core::sim::PadState::default()).buttons & core::sim::buttons::SPAWN_ACCEPT,
            0,
            "the click accepts"
        );
        for stage in [SpawnStage::Map, SpawnStage::Class] {
            set(&mut a, false, stage);
            frame(&mut a);
        }
        set(&mut a, true, SpawnStage::Accepted);
        frame(&mut a);
        assert!(a.kbm.grabbed(), "mouse look back with no click");
    }

    #[test]
    fn mouse_wheel_records_shell_scroll_activity() {
        let mut a = app();
        a.mouse_window_event(&WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: winit::event::MouseScrollDelta::LineDelta(0.0, -1.0),
            phase: winit::event::TouchPhase::Moved,
        });
        assert_eq!(a.shell_pointer.take_wheel(), -1.0);
    }

    #[test]
    fn spawn_and_pause_screens_record_pointer_clicks() {
        let mut a = app();
        a.screen = Screen::Battle;
        a.walk = Some((
            game::walk::WalkSim::new(42, glam::Vec3::ZERO, None),
            render::MeshHandle::default(),
        ));
        let walk = &mut a.walk.as_mut().unwrap().0;
        walk.sim.spawn_stage[0] = core::sim::SpawnStage::Side;
        walk.sim.units[walk.player.index as usize].alive = false;
        assert!(!a.can_grab());

        a.shell_pointer.moved_to([100.0, 100.0]);
        a.mouse_window_event(&WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: winit::event::ElementState::Pressed,
            button: winit::event::MouseButton::Left,
        });
        assert_eq!(a.shell_pointer.take_frame(), Some(([100.0, 100.0], true)));

        // A click on the pause menu must not wait for the spawn screen or shell.
        a.paused = true;
        a.mouse_window_event(&WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: winit::event::ElementState::Pressed,
            button: winit::event::MouseButton::Left,
        });
        a.step_spawn_pointer_at(0.016, None);
        assert_eq!(a.shell_pointer.take_frame(), Some(([100.0, 100.0], true)));
        a.mouse_window_event(&WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: winit::event::MouseScrollDelta::LineDelta(0.0, -1.0),
            phase: winit::event::TouchPhase::Moved,
        });
        assert_eq!(a.shell_pointer.take_wheel(), -1.0);
    }

    #[test]
    fn focus_loss_and_pointer_leave_clear_hover_activity() {
        let mut a = app();
        a.shell_pointer.moved_to([10.0, 10.0]);
        a.shell_pointer.click();
        a.mouse_window_event(&WindowEvent::Focused(false));
        assert_eq!(a.shell_pointer.take_frame(), None);
        a.shell_pointer.moved_to([10.0, 10.0]);
        a.mouse_window_event(&WindowEvent::CursorLeft {
            device_id: winit::event::DeviceId::dummy(),
        });
        assert_eq!(a.shell_pointer.take_frame(), None);
    }

    #[test]
    fn the_svg_cursor_renders_with_its_tip_on_the_hotspot() {
        for px in [32, 40, 48, 64, 96] {
            let rgba = render_cursor(px).unwrap();
            assert_eq!(rgba.len(), (px * px * 4) as usize);
            let k = px as f32 / 32.0;
            let (x, y) = (
                (HOTSPOT.0 * k).round() as u32,
                (HOTSPOT.1 * k).round() as u32,
            );
            let a = rgba[((y * px + x) * 4 + 3) as usize];
            assert!(
                a > 0,
                "{px} px: the click point is on the drawn tip (alpha {a})"
            );
            assert!(cursor_source(px).is_ok(), "{px} px");
        }
        // Mostly transparent: the arrow, not a filled square.
        let rgba = render_cursor(32).unwrap();
        let clear = rgba.chunks(4).filter(|p| p[3] == 0).count();
        assert!(clear > 32 * 32 / 2, "{clear} clear pixels");
    }

    #[test]
    fn the_cursor_hides_in_active_gameplay_grabbed_or_not() {
        assert!(cursor_shown(false, false), "menus");
        assert!(
            !cursor_shown(true, false),
            "a battle with no mouse grab (controller)"
        );
        assert!(!cursor_shown(true, true), "a battle with the mouse grabbed");
    }

    #[test]
    fn the_cursor_size_follows_the_display_scale() {
        assert_eq!(cursor_px(1.0), 32);
        assert_eq!(cursor_px(1.25), 40);
        assert_eq!(cursor_px(1.5), 48);
        assert_eq!(cursor_px(2.0), 64);
        assert_eq!(cursor_px(0.5), 32);
    }
}
