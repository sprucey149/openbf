use super::*;
use crate::walk::WalkSim;
use core::sim::InputLog;
use glam::Vec3;

#[test]
fn saved_wheel_movement_is_repaired_before_sampling() {
    let mut settings = crate::profile::Settings::default();
    let table = &mut settings.keyboard_bindings.table;
    let wheel = table
        .iter()
        .position(|(t, _)| *t == Trigger::WheelDown)
        .unwrap();
    let old = table[0].0;
    table[0].0 = Trigger::WheelDown;
    table[wheel].0 = old;
    let saved = ron::to_string(&settings).unwrap();
    let loaded: crate::profile::Settings = ron::from_str(&saved).unwrap();
    assert_eq!(loaded.keyboard_bindings, Bindings::default());
    let mut k = Kbm::default();
    k.follow(&loaded);
    k.wheel(-1.0);
    assert_eq!(k.pad(PadState::default()).ly, 0);
    k.press(Trigger::Key(KeyCode::KeyW), true);
    assert_eq!(k.pad(PadState::default()).ly, i16::MAX);
}

#[test]
fn invalid_saved_keyboard_actions_and_conflicts_restore_defaults() {
    let valid = Bindings::default();
    let mut duplicate = valid.clone();
    duplicate.table[1].0 = duplicate.table[0].0;
    let mut unknown_action = valid.clone();
    unknown_action.table[0].1.bits |= buttons::RESPAWN;
    let mut missing = valid.clone();
    missing.table.pop();
    let mut repeated_action = valid.clone();
    repeated_action.table[1].1 = repeated_action.table[0].1;
    let mut invalid_walk = valid.clone();
    invalid_walk.table[0].1.walk = (0, 127);
    let mut reserved = valid.clone();
    reserved.table[0].0 = Trigger::Key(KeyCode::Escape);
    for bindings in [
        duplicate,
        unknown_action,
        missing,
        repeated_action,
        invalid_walk,
        reserved,
    ] {
        let settings = crate::profile::Settings {
            keyboard_bindings: bindings,
            ..Default::default()
        };
        let loaded: crate::profile::Settings =
            ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
        assert_eq!(loaded.keyboard_bindings, valid);
    }
}

#[test]
fn escape_stays_available_for_menu_recovery() {
    let mut bindings = Bindings::default();
    let before = bindings.clone();
    assert!(!bindings.remap(0, Trigger::Key(KeyCode::Escape)));
    assert_eq!(bindings, before);
}

#[test]
fn a_valid_reordered_saved_keyboard_layout_drives_the_remapped_action() {
    let mut settings = crate::profile::Settings::default();
    assert!(settings
        .keyboard_bindings
        .remap(0, Trigger::Key(KeyCode::KeyP)));
    settings.keyboard_bindings.table.reverse();
    let loaded: crate::profile::Settings =
        ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
    assert_eq!(loaded, settings);
    let mut k = Kbm::default();
    k.follow(&loaded);
    k.press(Trigger::Key(KeyCode::KeyW), true);
    assert_eq!(k.pad(PadState::default()).ly, 0);
    k.press(Trigger::Key(KeyCode::KeyP), true);
    assert_eq!(k.pad(PadState::default()).ly, i16::MAX);
    k.press(Trigger::Key(KeyCode::KeyP), false);
    assert_eq!(k.pad(PadState::default()).ly, 0);
}

#[test]
fn remapping_swaps_conflicts_and_releases_old_held_keys() {
    let mut k = Kbm::default();
    let mut settings = crate::profile::Settings::default();
    k.press(Trigger::Key(KeyCode::KeyW), true);
    assert!(k.pad(PadState::default()).ly > 0);
    settings
        .keyboard_bindings
        .remap(0, Trigger::Key(KeyCode::KeyS));
    k.follow(&settings);
    assert_eq!(k.pad(PadState::default()).ly, 0);
    k.press(Trigger::Key(KeyCode::KeyS), true);
    assert!(k.pad(PadState::default()).ly > 0);
    k.press(Trigger::Key(KeyCode::KeyS), false);
    k.press(Trigger::Key(KeyCode::KeyW), true);
    assert!(k.pad(PadState::default()).ly < 0);
}

#[test]
fn wheel_cannot_be_assigned_to_held_movement_directly_or_by_swap() {
    let mut bindings = Bindings::default();
    let original = bindings.clone();
    let wheel = bindings
        .table
        .iter()
        .position(|(t, _)| *t == Trigger::WheelDown)
        .unwrap();
    assert!(!bindings.remap(0, Trigger::WheelDown));
    assert!(!bindings.remap(wheel, Trigger::Key(KeyCode::KeyW)));
    assert_eq!(bindings, original);
}

fn grabbed() -> Kbm {
    Kbm {
        grabbed: true,
        ..Kbm::default()
    }
}

#[test]
fn every_bit_the_pad_presses_has_a_key_or_button() {
    let pad = input::Mapping::ps2_default()
        .table
        .iter()
        .fold(0, |a, (_, b)| a | b);
    let pad = pad
        | buttons::SPAWN_POST_UP
        | buttons::SPAWN_POST_DOWN
        | buttons::SPAWN_POST_LEFT
        | buttons::SPAWN_POST_RIGHT;
    let kbm = Bindings::pc_default()
        .table
        .iter()
        .fold(0, |a, (_, b)| a | b.bits);
    assert_eq!(pad & !kbm, 0, "unbound: {:#x}", pad & !kbm);
}

#[test]
fn a_saved_layout_from_before_the_viewpoint_key_keeps_its_remaps() {
    let mut saved = Bindings::pc_default();
    saved.table.retain(|(_, b)| b.bits != buttons::VIEW_TOGGLE);
    assert!(saved.remap(0, Trigger::Key(KeyCode::KeyI)));
    assert!(!saved.is_valid());
    let upgraded = saved.clone().with_new_actions();
    assert!(upgraded.is_valid());
    assert_eq!(upgraded.table[0].0, Trigger::Key(KeyCode::KeyI));
    assert!(upgraded.table.contains(&(
        Trigger::Key(KeyCode::KeyV),
        Bind::bits(buttons::VIEW_TOGGLE)
    )));
}

#[test]
fn a_controller_alone_passes_through_untouched() {
    let mut k = Kbm::default();
    let pad = PadState {
        lx: -1200,
        ly: 30000,
        rx: 500,
        ry: -9000,
        buttons: buttons::FIRE | buttons::ZOOM,
        ..PadState::default()
    };
    assert_eq!(k.pad(pad), pad);
    assert_eq!(k.pad(PadState::default()), PadState::default());
}

#[test]
fn wasd_walks_at_full_stick_and_a_diagonal_is_unit_length() {
    let mut k = Kbm::default();
    k.press(Trigger::Key(KeyCode::KeyW), true);
    let p = k.pad(PadState::default());
    assert_eq!((p.lx, p.ly), (0, i16::MAX));
    assert!(p.pressed(buttons::SPAWN_POST_UP));
    k.press(Trigger::Key(KeyCode::KeyA), true);
    let p = k.pad(PadState::default());
    let (x, y) = (PadState::axis(p.lx), PadState::axis(p.ly));
    assert!((x * x + y * y - 1.0).abs() < 1e-3 && x < 0.0 && y > 0.0);
    k.press(Trigger::Key(KeyCode::KeyD), true);
    let p = k.pad(PadState::default());
    assert_eq!(p.lx, 0, "left and right cancel");
    k.press(Trigger::Key(KeyCode::KeyA), false);
    k.press(Trigger::Key(KeyCode::KeyD), false);
    let stick = PadState {
        lx: -20000,
        ly: -5000,
        ..PadState::default()
    };
    let p = k.pad(stick);
    assert_eq!((p.lx, p.ly), (-20000, i16::MAX), "the stronger push wins");
}

#[test]
fn a_tap_shorter_than_a_tick_reaches_the_next_tick() {
    let mut k = Kbm::default();
    k.tap(Trigger::Key(KeyCode::KeyE));
    k.wheel(-1.0);
    let p = k.pad(PadState::default());
    assert!(p.pressed(buttons::ACTION) && p.pressed(buttons::SWITCH_WEAPON));
    k.consumed(&p, 0);
    assert!(k.pad(PadState::default()).pressed(buttons::ACTION));
    k.consumed(&p, 1);
    assert_eq!(k.pad(PadState::default()).buttons, 0);
    k.press(Trigger::Mouse(MouseButton::Left), true);
    k.consumed(&p, 2);
    assert!(k.pad(PadState::default()).pressed(buttons::FIRE), "held");
}

#[test]
fn motion_counts_only_while_grabbed_and_follows_sensitivity_and_invert() {
    let mut k = Kbm::default();
    k.motion(100.0, 100.0);
    assert!(!k.pad(PadState::default()).has_look());
    let mut k = grabbed();
    k.sensitivity = 2.0;
    k.motion(100.0, -50.0);
    let p = k.pad(PadState::default());
    let (yaw, pitch) = p.look();
    assert!((f64::from(yaw) - 100.0 * 2.0 * RAD_PER_COUNT).abs() < 1e-4);
    assert!(pitch > 0.0, "the mouse pushed away looks up");
    assert!(p.mouse_aim());
    k.consumed(&p, 1);
    assert!(!k.pad(PadState::default()).has_look());
    k.invert_y = true;
    k.motion(0.0, -50.0);
    assert!(k.pad(PadState::default()).look().1 < 0.0);
}

#[test]
fn a_deflected_right_stick_takes_the_aim_back_from_the_mouse() {
    let mut k = grabbed();
    k.motion(3.0, 0.0);
    assert!(k.pad(PadState::default()).mouse_aim());
    let stick = PadState {
        rx: 8000,
        ..PadState::default()
    };
    let p = k.pad(stick);
    assert!(!p.mouse_aim());
    k.consumed(&p, 1);
    assert!(!k.pad(PadState::default()).mouse_aim());
}

#[test]
fn a_frames_motion_reaches_one_tick_and_the_record_replays() {
    let mut k = grabbed();
    let mut walk = WalkSim::new(5, Vec3::ZERO, None);
    let mut replay = WalkSim::new(5, Vec3::ZERO, None);
    let start = walk.sim.units[walk.player.index as usize].soldier.yaw;
    let mut log = InputLog::default();
    let mut moved = 0.0f64;
    let dt = 1.0 / 144.0;
    for f in 0..432 {
        let dx = f64::from((f * 37) % 23) - 6.0;
        k.motion(dx, 0.0);
        moved += dx * RAD_PER_COUNT;
        if f % 50 < 20 {
            k.press(Trigger::Key(KeyCode::KeyW), true);
        } else {
            k.press(Trigger::Key(KeyCode::KeyW), false);
        }
        let pad = k.pad(PadState::default());
        let mut first = Some(pad);
        let ticks = walk.advance_with(dt, &[pad], |_| {
            let p = first.take().unwrap_or(PadState {
                mx: 0,
                my: 0,
                ..pad
            });
            log.ticks.push(p);
        });
        k.consumed(&pad, ticks);
    }
    let yaw = walk.sim.units[walk.player.index as usize].soldier.yaw;
    let left = k.pad(PadState::default()).look().0;
    let drawn = f64::from(start - yaw) + f64::from(left);
    assert!(
        (drawn - moved).abs() < 1e-3,
        "turned {drawn}, moved {moved}"
    );
    for pads in log.frames() {
        replay.step_tick(pads);
    }
    assert_eq!(replay.sim.hash(), walk.sim.hash());
}

#[test]
fn a_steady_mouse_turns_the_blended_view_evenly_through_the_ticks() {
    let mut k = grabbed();
    let mut walk = WalkSim::new(5, Vec3::ZERO, None);
    walk.blend.on = true;
    for _ in 0..60 {
        walk.step_tick(&[PadState::default()]);
    }
    k.press(Trigger::Key(KeyCode::KeyW), true);
    let step = 10.0 * RAD_PER_COUNT as f32;
    let mut last: Option<f32> = None;
    let mut ticked = 0;
    for _ in 0..60 {
        k.motion(10.0, 0.0);
        let pad = k.pad(PadState::default());
        let ticks = walk.advance_with(1.0 / 144.0, &[pad], |_| {});
        k.consumed(&pad, ticks);
        ticked += ticks;
        crate::render_blend::blend_in_looking(&mut walk, &k.look());
        let yaw = walk.camera().yaw;
        walk.blend_out();
        if let Some(l) = last {
            assert!(
                (l - yaw - step).abs() < 1e-4,
                "turned {} after {ticks} ticks",
                l - yaw
            );
        }
        last = Some(yaw);
    }
    assert!(ticked >= 10);
}

#[test]
fn grab_tries_locked_then_confined_and_only_marks_success() {
    for locked_works in [true, false] {
        let mut k = Kbm::default();
        k.click_in();
        let mut tried = Vec::new();
        k.sync_grab_with(true, |mode| {
            tried.push(mode);
            if mode == CursorGrabMode::Locked && !locked_works {
                Err(())
            } else {
                Ok(())
            }
        })
        .unwrap();
        let expected = if locked_works {
            vec![CursorGrabMode::Locked]
        } else {
            vec![CursorGrabMode::Locked, CursorGrabMode::Confined]
        };
        assert_eq!(tried, expected);
        assert!(k.grabbed());
    }
}

#[test]
fn failed_grabs_stay_visible_and_do_not_accept_look_or_retry_without_click() {
    let mut k = Kbm::default();
    k.click_in();
    let mut tried = Vec::new();
    assert!(k
        .sync_grab_with(true, |mode| {
            tried.push(mode);
            Err(())
        })
        .is_err());
    assert_eq!(tried, [CursorGrabMode::Locked, CursorGrabMode::Confined]);
    assert!(!k.grabbed());
    k.motion(100.0, 100.0);
    assert!(!k.look().has_look());
    k.sync_grab_with(true, |_| -> Result<(), ()> {
        panic!("retried without a click")
    })
    .unwrap();
    k.click_in();
    k.sync_grab_with(true, |_| Ok::<_, ()>(())).unwrap();
    assert!(k.grabbed());
}

#[test]
fn pause_menus_and_focus_loss_release_motion_and_require_a_new_click() {
    let mut k = Kbm::default();
    k.click_in();
    k.sync_grab_with(true, |_| Ok::<_, ()>(())).unwrap();
    k.motion(100.0, 100.0);
    let mut tried = Vec::new();
    k.sync_grab_with(false, |mode| {
        tried.push(mode);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(tried, [CursorGrabMode::None]);
    assert!(!k.grabbed());
    assert!(!k.look().has_look());
    k.sync_grab_with(true, |_| -> Result<(), ()> {
        panic!("regrabbed without click")
    })
    .unwrap();
    k.click_in();
    k.sync_grab_with(false, |_| -> Result<(), ()> { panic!("grabbed in a menu") })
        .unwrap();
    k.sync_grab_with(true, |_| -> Result<(), ()> { panic!("kept a menu click") })
        .unwrap();
}
