#[path = "../examples/support/walker_battle.rs"]
mod walker_battle;
use core::sim::{buttons, Event, PadState};
use core::vehicle::{gun, VehicleHandle};
use game::walk::WalkSim;

fn fixture() -> (WalkSim, VehicleHandle) {
    let (mut w, _) = walker_battle::battle(&game::assets::baked_root()).unwrap();
    let class = w
        .vehicle_classes
        .iter()
        .position(|c| c == "rep_walk_atte")
        .unwrap();
    let (i, v) = w
        .sim
        .vehicles
        .iter()
        .enumerate()
        .find(|(_, v)| v.alive && v.class as usize == class)
        .unwrap();
    let h = VehicleHandle {
        index: i as u32,
        gen: v.gen,
    };
    assert!(w.sim.seat_unit(w.player, h, 0));
    (w, h)
}

fn mouse(yaw: f32) -> PadState {
    PadState {
        mx: PadState::quantize_look(yaw as f64),
        buttons: buttons::MOUSE_AIM,
        ..Default::default()
    }
}

#[test]
#[ignore = "requires local BF2 Geonosis assets"]
fn atte_mouse_view_crosses_both_windows_and_the_body_follows_at_its_original_rate() {
    for delta in [-2.0f32, 2.0] {
        let (mut w, h) = fixture();
        let old = w.sim.vehicle(h).unwrap().rot;
        let class = w.sim.vehicle(h).unwrap().class as usize;
        let params = w.sim.walker_params[class].clone().unwrap();
        let pad = mouse(delta);
        let want = old * gun::dir(-pad.look().0, 0.0);
        let preview = w.sim.look_preview(w.player, &pad).unwrap();
        if let core::sim::LookPreview::Seat { look, .. } = preview {
            assert!(look.0.abs() > 1.9);
        } else {
            panic!("seat preview");
        }
        w.step_tick(&[pad]);
        let v = w.sim.vehicle(h).unwrap();
        let view = w.sim.seat_look(w.player).unwrap().view;
        assert!(view.abs_diff_eq(want, 0.002), "{view:?} {want:?}");
        assert!(
            v.rot.angle_between(old) <= 2.0 * params.stopped_turn_speed * core::TICK_DT + 0.001
        );
        assert!(v.walker.aim_yaw.abs() > params.yaw_limits[1].abs() + 1.0);
        let first = v.walker.aim_yaw.abs();
        for _ in 0..180 {
            w.step_tick(&[PadState::default()]);
        }
        let v = w.sim.vehicle(h).unwrap();
        assert!(v.rot.angle_between(old) > 0.1);
        assert!(v.walker.aim_yaw.abs() < first);
        assert!(w
            .sim
            .seat_look(w.player)
            .unwrap()
            .view
            .abs_diff_eq(want, 0.01));
    }
}

#[test]
#[ignore = "requires local BF2 Geonosis assets"]
fn atte_cycles_both_ai_held_guns_and_each_seat_fires_its_authored_weapon() {
    let (mut w, h) = fixture();
    let class = w.sim.vehicle(h).unwrap().class as usize;
    let pos = w.sim.vehicle(h).unwrap().pos;
    for seat in [1, 2] {
        let bot = w
            .sim
            .spawn(&core::unit::UnitSpec {
                controller: core::unit::Controller::Ai,
                ..core::unit::UnitSpec::trooper(0, pos)
            })
            .unwrap();
        assert!(w.sim.seat_unit(bot, h, seat));
    }
    let setup = w.sim.seat_setup[class].as_ref().unwrap();
    assert!(setup.guns[1].as_ref().unwrap().turn.unwrap().yaw_wrap);
    assert!(setup.guns[1].as_ref().unwrap().framed);
    let rear = setup.guns[2].as_ref().unwrap();
    assert!(!rear.framed);
    let turn = rear.turn.unwrap();
    assert!(!turn.yaw_wrap && turn.yaw_positive);
    assert!((turn.rest().0 - std::f32::consts::PI).abs() < 1e-5);
    assert_eq!(turn.yaw_speed, 1.0);
    assert_eq!(turn.yaw_filter, 3.75);
    assert!(setup.guns[0].as_ref().unwrap().aimer.yaw[1] > 0.0);
    for seat in [0u8, 1, 2] {
        if seat != 0 {
            w.step_tick(&[PadState {
                buttons: buttons::SWITCH_WEAPON,
                ..Default::default()
            }]);
            w.step_tick(&[PadState::default()]);
        }
        assert_eq!(w.sim.unit(w.player).unwrap().mount.unwrap().seat, seat);
        let expected = w.sim.seat_setup[class].as_ref().unwrap().guns[seat as usize]
            .as_ref()
            .unwrap()
            .weapons[0]
            .class;
        let mut fired = false;
        for _ in 0..60 {
            w.step_tick(&[PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            }]);
            fired |= w.sim.events.iter().any(|e| matches!(e,Event::Shot {unit,weapon_class,..} if *unit==w.player && *weapon_class as usize==expected));
        }
        assert!(
            fired,
            "seat {seat}, expected {}",
            w.sim.weapon_classes[expected].name
        );
        if seat == 2 {
            let style = game::walker_shots::style(&w, expected).unwrap();
            assert_eq!(
                w.sim.weapon_classes[expected].secondary.as_ref().map(|s| s.period),
                Some(3)
            );
            assert_eq!(
                style.fire_sound.as_deref(),
                Some("rep_weap_walk_atte_chaingun_fire")
            );
            let left = game::walker_shots::frame(&w, w.player, expected, 0)
                .unwrap()
                .w_axis
                .truncate();
            let right = game::walker_shots::frame(&w, w.player, expected, 1)
                .unwrap()
                .w_axis
                .truncate();
            assert!(left.distance(right) > 0.1);
            assert!(!game::walker_shots::voices(&w).is_empty());
            let look = w.sim.seat_look(w.player).unwrap();

            assert!(
                look.barrel.dot(look.rot * -glam::Vec3::Z) > 0.9,
                "rear barrel {:?}",
                look.barrel
            );
        }
        let v = w.sim.vehicle(h).unwrap();
        assert_eq!(v.seats.iter().filter(|s| s.occupant.is_some()).count(), 3);
        let camera = w.camera();
        assert!(camera.position.is_finite());
        assert!(camera.position.distance(v.pos) > 1.0);
        if seat == 1 {
            assert!(camera.position.y > v.pos.y + 7.0);
            assert!(w.reticle_y().is_finite());
        }
        assert!(game::hud_feed::armed_seat(&w));
        assert!(game::hud_feed::weapon_rows(&w)[0].is_some());
    }
    let before = w.sim.seat_look(w.player).unwrap().look;
    w.step_tick(&[mouse(4.0)]);
    let after = w.sim.seat_look(w.player).unwrap().look;
    assert_ne!(before, after);
    assert!((after.0 - 120.0f32.to_radians()).abs() < 1e-5);
    w.step_tick(&[mouse(-8.0)]);
    let after = w.sim.seat_look(w.player).unwrap().look;
    assert!((after.0 - 240.0f32.to_radians()).abs() < 1e-5);
}

#[test]
#[ignore = "requires local BF2 Geonosis assets"]
fn main_gunner_camera_tracks_the_mount_without_inheriting_its_roll() {
    let (mut w, h) = fixture();
    w.sim.unseat_unit(w.player);
    assert!(w.sim.seat_unit(w.player, h, 1));
    let class = w.sim.vehicle(h).unwrap().class as usize;
    let rig = w.walker_rigs[class].as_ref().unwrap().clone();
    let scale = w.sim.walker_params[class].as_ref().unwrap().root_scale;
    let first = w.camera();
    let mut relative = None;
    let mut moved = false;
    let base = rig
        .node_now(w.sim.vehicle(h).unwrap(), scale, "aimer_maincannon_y")
        .unwrap();
    for i in 0..30 {
        let v = w.sim.vehicle_mut(h).unwrap();
        v.walker.pitch = (i as f32 * 0.13).sin() * 0.12;
        v.walker.roll = ((i as f32 * 0.17).cos(), (i as f32 * 0.17).sin());
        v.walker.clip_time = i as f32 / 30.0;
        let node = rig.node_now(v, scale, "aimer_maincannon_y").unwrap();
        moved |= !node.abs_diff_eq(base, 0.001);
        let mount_world = v.pos + v.rot * node.w_axis.truncate();
        let cam = w.camera();
        let offset = cam.position - mount_world;
        if let Some(previous) = relative {
            assert!(offset.abs_diff_eq(previous, 0.0001));
        }
        relative = Some(offset);
        assert!(cam.forward().abs_diff_eq(first.forward(), 0.0001));
        assert!((cam.roll - first.roll).abs() < 0.0001);
    }
    assert!(moved);
    w.step_tick(&[mouse(0.4)]);
    assert!(w.camera().forward().distance(first.forward()) > 0.1);
}

#[test]
#[ignore = "requires local BF2 Geonosis assets"]
fn ai_driven_atte_gunner_view_suppresses_tick_to_tick_yaw_oscillation() {
    let (mut w, h) = fixture();
    w.sim.unseat_unit(w.player);
    let pos = w.sim.vehicle(h).unwrap().pos;
    let bot = w
        .sim
        .spawn(&core::unit::UnitSpec {
            controller: core::unit::Controller::Ai,
            ..core::unit::UnitSpec::trooper(0, pos)
        })
        .unwrap();
    assert!(w.sim.seat_unit(bot, h, 0));
    assert!(w.sim.seat_unit(w.player, h, 2));
    w.blend.on = true;
    let (mut raw_sum, mut drawn_sum, mut prev_raw, mut prev_drawn) = (0.0f32, 0.0f32, None, None);
    for i in 0..600 {
        let mut blend = std::mem::take(&mut w.blend);
        blend.before_tick(&w);
        w.blend = blend;
        w.step_tick(&[PadState::default()]);
        let hash = w.sim.hash();
        let raw = w.sim.vehicle(h).unwrap().rot;
        let mut blend = std::mem::take(&mut w.blend);
        assert!(blend.apply(&mut w, 1.0));
        let drawn = w.sim.vehicle(h).unwrap().rot;
        if i >= 450 {
            if let (Some(a), Some(b)) = (prev_raw, prev_drawn) {
                raw_sum += raw.angle_between(a);
                drawn_sum += drawn.angle_between(b);
            }
        }
        prev_raw = Some(raw);
        prev_drawn = Some(drawn);
        blend.restore(&mut w);
        w.blend = blend;
        assert_eq!(w.sim.hash(), hash);
    }
    assert!(raw_sum > 0.5, "raw oscillation {raw_sum}");
    assert!(
        drawn_sum < raw_sum * 0.15,
        "drawn {drawn_sum}, raw {raw_sum}"
    );
}
