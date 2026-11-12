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
fn existing_layouts_pick_up_both_starfighter_maneuver_keys() {
    let mut saved = Bindings::pc_default();
    saved
        .table
        .retain(|(_, bind)| bind.bits & (buttons::FLYER_TRICK | buttons::FLYER_REVERSE) == 0);
    assert!(saved.remap(0, Trigger::Key(KeyCode::AltLeft)));
    let upgraded = saved.with_new_actions();
    assert!(upgraded.is_valid());
    assert_eq!(upgraded.table[0].0, Trigger::Key(KeyCode::AltLeft));
    for button in [buttons::FLYER_TRICK, buttons::FLYER_REVERSE] {
        assert!(upgraded.table.iter().any(|(_, bind)| bind.bits == button));
    }
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

#[test]
fn legacy_viewpoint_migration_keeps_a_remap_onto_v_when_loading_a_profile() {
    let mut settings = crate::profile::Settings::default();
    settings
        .keyboard_bindings
        .table
        .retain(|(_, bind)| bind.bits != buttons::VIEW_TOGGLE);
    assert!(settings
        .keyboard_bindings
        .remap(0, Trigger::Key(KeyCode::KeyV)));
    let previous = settings.keyboard_bindings.table.clone();
    let loaded: crate::profile::Settings =
        ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        &loaded.keyboard_bindings.table[..previous.len()],
        previous.as_slice()
    );
    assert!(loaded.keyboard_bindings.is_valid());
    let (view_key, _) = loaded
        .keyboard_bindings
        .table
        .iter()
        .find(|(_, bind)| bind.bits == buttons::VIEW_TOGGLE)
        .unwrap();
    assert_ne!(*view_key, Trigger::Key(KeyCode::KeyV));
    let mut input = Kbm::default();
    input.follow(&loaded);
    input.press(Trigger::Key(KeyCode::KeyV), true);
    let moving = input.pad(PadState::default());
    assert_eq!(moving.ly, i16::MAX);
    assert!(!moving.pressed(buttons::VIEW_TOGGLE));
    input.press(*view_key, true);
    assert!(input.pad(PadState::default()).pressed(buttons::VIEW_TOGGLE));
}

/// A and D are value steps in every screen's bits, and nothing else's edge.
#[test]
fn a_and_d_are_the_menu_value_steps() {
    let bits = |code| menu_bits(menu_key(code), false);
    assert_eq!(bits(KeyCode::KeyA), MENU_ADJUST_LEFT);
    assert_eq!(bits(KeyCode::KeyD), MENU_ADJUST_RIGHT);
    // The arrows stay the arms the Morse matcher reads.
    assert_eq!(
        bits(KeyCode::ArrowLeft) & (MENU_ADJUST_LEFT | MENU_ADJUST_RIGHT),
        0
    );
}

/// A held D-pad or arrow does not latch a half-tilted stick into a value step,
/// while a stick that is already stepping keeps its latch down to the release point.
#[test]
fn a_held_morse_key_never_latches_the_stick() {
    let half_left = -(input::STICK_ARM_ON + input::STICK_ARM_OFF) / 2.0;
    let arm = |prev| input::stick_arms(half_left, 0.0, stick_latch(prev));
    assert_eq!(arm(4), 0, "the D-pad's Left alone");
    assert_eq!(arm(4 | MENU_ADJUST_LEFT), 4, "the stick's own Left");
    assert_eq!(input::stick_arms(0.0, half_left, stick_latch(2)), 2);
}

/// A spawn-screen pick waits like a tap: kept through a frame that steps
/// no tick, taken by the first tick that does, and dropped while paused.
#[test]
fn a_screen_pick_waits_for_a_tick_like_a_tap() {
    use core::sim::ScreenPick;
    let mut k = Kbm::default();
    k.set_pick(ScreenPick::Side(2));
    let pad = k.pad(PadState::default());
    assert_eq!(pad.pick, Some(ScreenPick::Side(2)));
    k.consumed(&pad, 0);
    let pad = k.pad(PadState::default());
    assert_eq!(pad.pick, Some(ScreenPick::Side(2)), "no tick: kept");
    k.consumed(&pad, 1);
    assert_eq!(k.pad(PadState::default()).pick, None, "taken");
    k.set_pick(ScreenPick::Post(3));
    k.drop_taps();
    assert_eq!(k.pad(PadState::default()).pick, None, "paused: dropped");
}

fn flying() -> Kbm {
    let mut k = Kbm::default();
    k.click_in();
    let _ = k.sync_grab_with(true, |_| Ok::<(), ()>(()));
    assert!(k.grabbed());
    k
}

fn flown_tick(k: &mut Kbm, dx: f64, dy: f64) -> PadState {
    k.motion(dx, dy);
    let pad = k.flyer_pad(PadState::default());
    k.consumed(&pad, 1);
    pad
}

#[test]
fn a_flyer_keeps_steering_after_a_held_mouse_move_stops() {
    let mut k = flying();
    let mut pad = PadState::default();
    for _ in 0..6 {
        pad = flown_tick(&mut k, 34.0, 0.0);
    }
    let moving = PadState::axis(pad.rx);
    assert!((moving - 0.498).abs() < 0.001, "{moving}");
    assert_eq!((pad.mx, pad.my), (0, 0), "no look delta in flight");
    for _ in 0..300 {
        pad = flown_tick(&mut k, 0.0, 0.0);
    }
    assert_eq!(PadState::axis(pad.rx), moving, "still steering 10 s later");
    assert_eq!(pad.ry, 0);
}

#[test]
fn a_short_flick_does_not_hold() {
    let mut k = flying();
    let first = flown_tick(&mut k, 34.0, 0.0);
    assert!(first.rx > 0);
    let echo = flown_tick(&mut k, 0.0, 0.0);
    assert_eq!(echo.rx, first.rx, "a quiet read repeats the last move once");
    assert_eq!(flown_tick(&mut k, 0.0, 0.0).rx, 0);
    assert_eq!(flown_tick(&mut k, 0.0, 0.0).rx, 0);
}

#[test]
fn moving_back_cancels_the_held_steering() {
    let mut k = flying();
    for _ in 0..6 {
        flown_tick(&mut k, 34.0, 0.0);
    }
    flown_tick(&mut k, 0.0, 0.0);
    let eased = flown_tick(&mut k, -4.0, 0.0);
    let held = PadState::axis(flown_tick(&mut k, 0.0, 0.0).rx);
    assert!(held > 0.0 && held < 0.498, "{held} after {}", eased.rx);
    for _ in 0..3 {
        flown_tick(&mut k, -4.0, 0.0);
    }
    flown_tick(&mut k, 0.0, 0.0);
    assert_eq!(flown_tick(&mut k, 0.0, 0.0).rx, 0, "back at rest");
    for _ in 0..6 {
        flown_tick(&mut k, -4.0, 0.0);
    }
    let reversed = flown_tick(&mut k, 0.0, 0.0).rx;
    assert!(reversed < 0, "going on past rest steers the other way");
    assert_eq!(flown_tick(&mut k, 0.0, 0.0).rx, reversed);
}

#[test]
fn steering_tops_out_and_follows_sensitivity_and_invert() {
    let mut k = flying();
    let mut pad = PadState::default();
    for _ in 0..6 {
        pad = flown_tick(&mut k, 200.0, -20.0);
    }
    assert_eq!(PadState::axis(pad.rx), 1.0);
    assert!(pad.ry > 0, "the mouse pushed away raises the stick");
    let mut k = flying();
    k.invert_y = true;
    assert!(flown_tick(&mut k, 0.0, -20.0).ry < 0);
    let mut k = flying();
    k.sensitivity = 2.0;
    let pad = flown_tick(&mut k, 34.0, 0.0);
    assert!((PadState::axis(pad.rx) - 0.996).abs() < 0.002);
}

#[test]
fn leaving_the_seat_or_losing_the_pointer_or_a_firm_stick_lets_go() {
    let mut k = flying();
    for _ in 0..6 {
        flown_tick(&mut k, 34.0, 0.0);
    }
    assert_eq!(k.pad(PadState::default()).rx, 0);
    assert_eq!(
        flown_tick(&mut k, 0.0, 0.0).rx,
        0,
        "on foot resets the hold"
    );
    for _ in 0..6 {
        flown_tick(&mut k, 34.0, 0.0);
    }
    let stick = PadState {
        rx: PadState::quantize(-0.5),
        ..PadState::default()
    };
    let pad = k.flyer_pad(stick);
    k.consumed(&pad, 1);
    assert_eq!(pad.rx, stick.rx);
    k.motion(0.0, 0.0);
    flown_tick(&mut k, 0.0, 0.0);
    assert_eq!(flown_tick(&mut k, 0.0, 0.0).rx, 0, "the stick cancels it");
    for _ in 0..6 {
        flown_tick(&mut k, 34.0, 0.0);
    }
    k.clear_grab();
    assert_eq!(flown_tick(&mut k, 0.0, 0.0).rx, 0, "pointer released");
}

#[test]
fn a_frame_without_a_tick_does_not_spend_the_mouse_move() {
    let mut k = flying();
    k.motion(34.0, 0.0);
    let preview = k.flyer_pad(PadState::default());
    k.consumed(&preview, 0);
    k.motion(34.0, 0.0);
    let pad = k.flyer_pad(PadState::default());
    assert_eq!(pad.rx, PadState::quantize((68.0 * 30.0 / 2048.0) as f32));
}

/// Triangle / Y backs out of the ground pause menu on its own fresh press: one
/// already held when the menu opens is no Back, and space keeps it inert.
#[test]
fn north_backs_the_pause_menu_only_on_a_fresh_press() {
    // The Start frame: not yet paused, Triangle (ACTION) held.
    let open = MENU_NORTH | 64;
    assert_eq!(menu_edges(open, 0, 0, false), 64);
    // Paused next frame, Triangle still held: no Back.
    assert_eq!(menu_edges(MENU_NORTH, open, 0, true), 0);
    // Released, then pressed again: Back.
    assert_eq!(menu_edges(0, MENU_NORTH, 0, true), 0);
    assert_eq!(menu_edges(MENU_NORTH, 0, 0, true), 32);
    // Space battles: Triangle does nothing in the pause menu.
    assert_eq!(menu_edges(MENU_NORTH, 0, 0, false), 0);
    // Other edges and taps pass as before.
    assert_eq!(menu_edges(16 | 32, 32, 2, true), 16 | 2);
}
