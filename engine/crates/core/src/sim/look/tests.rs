use crate::sim::tests::flat_ground;
use crate::sim::{buttons, Event, InputLog, LookPreview, PadState, Sim, LOOK_UNIT};
use crate::soldier::Stance;
use crate::unit::UnitSpec;
use glam::Vec3;

fn foot() -> Sim {
    let mut s = Sim::empty(7);
    s.heightfield = Some(flat_ground());
    s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    s.units[0].soldier.pitch = 0.0;
    s.step(&[PadState::default()]);
    s
}

fn mouse(yaw: f64, pitch: f64) -> PadState {
    PadState {
        mx: PadState::quantize_look(yaw),
        my: PadState::quantize_look(pitch),
        buttons: buttons::MOUSE_AIM,
        ..Default::default()
    }
}

fn aim(s: &Sim) -> (f32, f32) {
    (s.units[0].soldier.yaw, s.units[0].soldier.pitch)
}

#[test]
fn mouse_aim_keeps_the_player_alive_until_explicit_respawn() {
    let mut s = foot();
    let player = s.handle(0);
    let pad = mouse(0.3, -0.1);
    s.step(&[pad]);
    assert!(s.unit(player).is_some());
    s.step(&[PadState {
        buttons: pad.buttons | buttons::RESPAWN,
        ..pad
    }]);
    assert!(s.unit(player).is_none());
}

#[test]
fn mouse_look_turns_by_its_angle_past_the_turn_rate() {
    let mut s = foot();
    let cap = s.units[0].consts.max_turn_speed * crate::TICK_DT;
    let pad = mouse(1.0, 0.25);
    let (yaw, pitch) = aim(&s);
    s.step(&[pad]);
    let (turned, raised) = (yaw - aim(&s).0, aim(&s).1 - pitch);
    assert_eq!(turned, pad.mx as f32 * LOOK_UNIT);
    assert_eq!(raised, pad.my as f32 * LOOK_UNIT);
    assert!(turned > 3.0 * cap, "{turned} against the stick's {cap}");
    s.step(&[mouse(-0.5, 0.0)]);
    assert!((aim(&s).0 - (yaw - 0.5)).abs() < 1e-5);
}

#[test]
fn mouse_pitch_holds_the_limits_and_the_prone_clamp() {
    let mut s = foot();
    let (lo, hi) = s.units[0].consts.pitch_limits;
    s.step(&[mouse(0.0, 4.0)]);
    assert_eq!(aim(&s).1, hi);
    s.step(&[mouse(0.0, -8.0)]);
    assert_eq!(aim(&s).1, lo);
    s.units[0].soldier.pitch = 0.0;
    s.units[0].soldier.stance = Stance::Prone;
    s.step(&[mouse(0.0, 2.0)]);
    assert_eq!(aim(&s).1, 0.4);
    s.step(&[mouse(0.0, -2.0)]);
    assert_eq!(aim(&s).1, -0.4);
}

#[test]
fn the_zoom_divides_mouse_look_by_its_magnification() {
    let mut s = foot();
    let zoom = PadState {
        buttons: buttons::ZOOM | buttons::MOUSE_AIM,
        ..Default::default()
    };
    s.step(&[zoom]);
    s.step(&[mouse(0.0, 0.0)]);
    let z = s.units[0].zoom.expect("zoomed").value;
    assert!(z > 1.0);
    let yaw = aim(&s).0;
    let pad = mouse(0.5, 0.0);
    s.step(&[pad]);
    let want = pad.mx as f32 * LOOK_UNIT / z;
    assert!(((yaw - aim(&s).0) - want).abs() < 1e-6, "{z}");
}

#[test]
fn the_preview_is_what_the_tick_does() {
    let mut s = foot();
    let h = s.handle(0);
    for pad in [mouse(0.3, -0.1), mouse(-2.0, 3.0), mouse(0.0, 0.0)] {
        let preview = s.look_preview(h, &pad);
        s.step(&[pad]);
        match preview {
            Some(LookPreview::Foot { unit, yaw, pitch }) => {
                assert_eq!(unit, 0);
                assert_eq!((yaw, pitch), aim(&s));
            }
            None => assert!(!pad.has_look()),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_log_without_mouse_fields_loads_and_writes_none() {
    let old = "(seed: 7, ticks: [(lx: 0, ly: 20000, rx: 3000, ry: -200, buttons: 1), \
               (lx: 5, ly: 0, rx: 0, ry: 0, buttons: 0)])";
    let log: InputLog = ron::from_str(old).unwrap();
    assert_eq!(log.tick_count(), 2);
    assert!(log.ticks.iter().all(|p| !p.has_look()));
    assert_eq!((log.ticks[0].rx, log.ticks[0].buttons), (3000, 1));
    assert!(log.ticks.iter().all(|p| p.fire_lag == 0));
    let text = ron::to_string(&log).unwrap();
    assert!(!text.contains("mx") && !text.contains("my"), "{text}");
    assert!(!text.contains("fire_lag"), "{text}");
}

#[test]
fn a_mouse_log_replays_to_the_same_sim() {
    let log = InputLog {
        seed: 11,
        slots: 1,
        ticks: (0..240)
            .map(|i| PadState {
                ly: if i % 50 < 30 { 20000 } else { 0 },
                mx: ((i * 911) % 4000) - 2000,
                my: ((i * 433) % 1200) - 600,
                buttons: buttons::MOUSE_AIM | if i % 9 < 2 { buttons::FIRE } else { 0 },
                ..Default::default()
            })
            .collect(),
    };
    let text = ron::to_string(&log).unwrap();
    let back: InputLog = ron::from_str(&text).unwrap();
    assert_eq!(back.ticks, log.ticks);
    let run = |log: &InputLog| {
        let mut s = foot();
        for pads in log.frames() {
            s.step(pads);
        }
        (s.hash(), aim(&s))
    };
    let (a, b) = (run(&log), run(&back));
    assert_eq!(a, b);
    let still = InputLog {
        ticks: log
            .ticks
            .iter()
            .map(|p| PadState { mx: 0, my: 0, ..*p })
            .collect(),
        ..log.clone()
    };
    assert_ne!(run(&still).1, a.1, "the mouse turned the soldier");
}

#[test]
fn walker_mouse_yaw_crosses_the_turn_window_while_pitch_remains_limited() {
    use crate::vehicle::walker::{WalkerParams, WalkerState};
    let p = WalkerParams {
        yaw_limits: [-0.2, 0.2],
        pitch_limits: [-0.4, 0.3],
        ..Default::default()
    };
    let w = WalkerState::default();
    assert_eq!(super::walker_aim(&w, &p, (-2.4, 2.0)), (2.4, 0.3));
    assert_eq!(super::walker_aim(&w, &p, (2.4, -2.0)), (-2.4, -0.4));
}

#[test]
fn the_hull_catches_an_outside_view_at_the_authored_rate_without_turning_the_view() {
    use crate::vehicle::{
        walker::{self, flags, WalkerParams},
        SeatKind, VehicleKind, VehicleSpec,
    };
    use glam::Vec3;
    for yaw in [-1.5, 1.5] {
        let p = WalkerParams {
            flags: flags::AIM_WINDOW | flags::CAMERA_TURN,
            yaw_limits: [-0.2, 0.2],
            stopped_turn_speed: 0.1,
            ..Default::default()
        };
        let mut v = VehicleSpec {
            kind: VehicleKind::Walker,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 1.0,
            health_type: crate::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        }
        .build(1);
        v.input.human = true;
        v.walker.aim_yaw = yaw;
        let before = v.rot * crate::vehicle::gun::dir(yaw, 0.0);
        let rot = v.rot;
        walker::input(&mut v, &p, crate::TICK_DT);
        assert!(
            (rot.angle_between(v.rot) - 2.0 * p.stopped_turn_speed * crate::TICK_DT).abs() < 1e-4
        );
        assert!(v.walker.aim_yaw.abs() > 0.2);
        let after = v.rot * crate::vehicle::gun::dir(v.walker.aim_yaw, 0.0);
        assert!(before.abs_diff_eq(after, 1e-5));
    }
}
fn stick(rx: f32, ry: f32, b: u32) -> PadState {
    PadState {
        rx: PadState::quantize(rx),
        ry: PadState::quantize(ry),
        buttons: b,
        ..Default::default()
    }
}

/// A controller gets the mouse's preview (2026-10-29): the drawn aim is the
/// one the next tick turns to under the held stick - standing, zoomed,
/// crouched and with an enemy in the assist's magnet (its slowdown, and
/// the pull once FIRE is held) - so the shot leaves where the reticle was
/// drawn.
#[test]
fn the_stick_preview_is_what_the_tick_does() {
    let mut s = foot();
    s.spawn(&UnitSpec::trooper(1, Vec3::new(0.5, 0.0, -25.0)))
        .unwrap();
    let h = s.handle(0);
    let zoom = stick(0.0, 0.0, buttons::ZOOM);
    let crouch = stick(0.0, 0.0, buttons::ROLL_STANCE);
    let mut checked = 0;
    for pad in [
        stick(0.3, -0.2, 0),
        stick(-0.5, 0.1, 0),
        zoom,
        stick(0.4, 0.0, 0),
        stick(-0.2, 0.3, buttons::FIRE),
        zoom,
        crouch,
        stick(0.6, -0.4, 0),
        stick(0.0, 0.0, 0),
    ] {
        let preview = s.look_preview(h, &pad);
        let edge = pad.buttons & (buttons::ZOOM | buttons::ROLL_STANCE) != 0;
        s.step(&[pad]);
        if edge {
            continue;
        }
        match preview {
            Some(LookPreview::Foot { unit, yaw, pitch }) => {
                assert_eq!(unit, 0);
                assert_eq!((yaw, pitch), aim(&s), "{pad:?}");
                checked += 1;
            }
            other => panic!("{pad:?}: {other:?}"),
        }
    }
    assert_eq!(checked, 6);
    assert!(s.magnet[0].is_some(), "the assist had a magnet");
}

fn shot_dir(s: &Sim) -> Option<Vec3> {
    s.events.iter().find_map(|e| match e {
        Event::Shot { dir, .. } => Some(*dir),
        _ => None,
    })
}

/// [ours] 2026-10-29: a FIRE press with a fire lag leaves that share of the
/// tick's stick turn early (where the window drew the aim), and the turn
/// still ends where it would have.
#[test]
fn a_lagged_press_fires_part_way_into_the_turn() {
    let shot = |lag: u8| {
        let mut s = foot();
        for _ in 0..40 {
            s.step(&[PadState::default()]);
        }
        let from = aim(&s);
        s.step(&[PadState {
            fire_lag: lag,
            ..stick(0.6, 0.3, buttons::FIRE)
        }]);
        let d = shot_dir(&s).expect("fired");
        (from, aim(&s), (-d.x).atan2(-d.z), d.y.asin())
    };
    let (from, to, yaw0, pitch0) = shot(0);
    let (f, t, yaw1, pitch1) = shot(255);
    assert_eq!((f, t), (from, to));
    // Lag 0 fires at the turned aim, lag 255 at the aim before it.
    assert!(
        (yaw0 - yaw1 - (to.0 - from.0)).abs() < 1e-3,
        "{yaw0} {yaw1}"
    );
    assert!((pitch0 - pitch1 - (to.1 - from.1)).abs() < 1e-3);
    assert!(
        (to.0 - from.0).abs() > 0.03,
        "the stick turned: {from:?} {to:?}"
    );
    for lag in [64, 128, 191] {
        let (f, t, yaw, pitch) = shot(lag);
        assert_eq!((f, t), (from, to), "the turn ends where it would");
        let k = 1.0 - f32::from(lag) / 255.0;
        assert!(
            (yaw - (yaw1 + (yaw0 - yaw1) * k)).abs() < 1e-4,
            "{lag}: {yaw}"
        );
        assert!((pitch - (pitch1 + (pitch0 - pitch1) * k)).abs() < 1e-4);
    }
}

/// A log with fire lags replays to the same sim, through text; the lags
/// change it (the shots left earlier in their turns).
#[test]
fn a_log_with_fire_lags_replays_to_the_same_sim() {
    let log = InputLog {
        seed: 11,
        slots: 1,
        ticks: (0..240)
            .map(|i| {
                let press = i % 9 == 0;
                PadState {
                    fire_lag: if press { ((i * 37) % 255) as u8 } else { 0 },
                    ..stick(
                        if i % 60 < 30 { 0.5 } else { -0.4 },
                        0.1,
                        if i % 9 < 2 { buttons::FIRE } else { 0 },
                    )
                }
            })
            .collect(),
    };
    let text = ron::to_string(&log).unwrap();
    assert!(text.contains("fire_lag"));
    let back: InputLog = ron::from_str(&text).unwrap();
    assert_eq!(back.ticks, log.ticks);
    let run = |log: &InputLog| {
        let mut s = foot();
        for pads in log.frames() {
            s.step(pads);
        }
        s.hash()
    };
    assert_eq!(run(&log), run(&back));
    let plain = InputLog {
        ticks: log
            .ticks
            .iter()
            .map(|p| PadState { fire_lag: 0, ..*p })
            .collect(),
        ..log.clone()
    };
    assert_ne!(run(&plain), run(&log), "the lags moved the shots");
}

/// Issue #168 (LAN §6): a lockstep peer's camera shows the aim its pending
/// inputs will leave. The preview folded over several pads - mouse looks,
/// held sticks and a mix, past the pitch limit - is the aim stepping those
/// pads leaves, and folding writes nothing.
#[test]
fn the_folded_preview_is_what_stepping_the_pads_does() {
    let runs: [&[PadState]; 4] = [
        &[mouse(0.3, -0.1), mouse(-2.0, 3.0), mouse(0.05, 0.02)],
        &[stick(0.3, -0.2, 0), stick(-0.5, 0.1, 0), stick(0.9, 0.9, 0)],
        &[mouse(0.2, 0.1), stick(0.4, -0.3, 0), mouse(-0.1, 0.4)],
        &[mouse(0.0, 2.5), mouse(0.0, 2.5), stick(0.0, -1.0, 0)],
    ];
    for pads in runs {
        let mut s = foot();
        let h = s.handle(0);
        let before = s.hash();
        let preview = s.look_preview_over(h, pads);
        assert_eq!(s.hash(), before, "folding wrote into the sim");
        for pad in pads {
            s.step(&[*pad]);
        }
        match preview {
            Some(LookPreview::Foot { unit, yaw, pitch }) => {
                assert_eq!(unit, 0);
                assert_eq!((yaw, pitch), aim(&s), "{pads:?}");
            }
            other => panic!("{pads:?}: {other:?}"),
        }
    }
    // One pad folds to the plain preview; none folds to nothing.
    let s = foot();
    let h = s.handle(0);
    let pad = mouse(0.4, 0.2);
    assert_eq!(s.look_preview_over(h, &[pad]), s.look_preview(h, &pad));
    assert_eq!(s.look_preview_over(h, &[]), None);
}
