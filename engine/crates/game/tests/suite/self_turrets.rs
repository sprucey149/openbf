//! Issue #91: real Hoth self seats in its authored Instant Action layer.
use super::turret_damage::{spawn, world_mode};
use core::sim::{Event, PadState};
use core::unit::Controller;
use glam::Vec3;

#[test]
fn hoth_ia_self_seats_fire_and_track_without_riders() {
    let mut w = world_mode("bf2_hot1", ["all", "imp"], "ctf").expect("baked PS2 Hoth required");
    let selves: Vec<_> = w
        .sim
        .alive_vehicles()
        .filter(|(_, v)| {
            matches!(
                w.classes[v.class as usize].as_str(),
                "tur_bldg_chaingun_tripod" | "tur_bldg_chaingun_roof"
            )
        })
        .map(|(h, _)| h)
        .collect();
    assert_eq!(selves.len(), 14, "13 tripods and one roof gun, PS2 census");
    let h = selves
        .iter()
        .copied()
        .find(|h| {
            let v = w.sim.vehicle(*h).unwrap();
            w.classes[v.class as usize].eq_ignore_ascii_case("tur_bldg_chaingun_tripod")
        })
        .unwrap();
    let v = w.sim.vehicle(h).unwrap().clone();
    let g = w.sim.seat_setup[v.class as usize].as_ref().unwrap().guns[0]
        .as_ref()
        .unwrap();
    let wc = &w.sim.weapon_classes[g.weapons[0].class];
    assert_eq!(wc.ai_range[2], 80.0);
    assert_eq!(wc.shot_delay, [0.05; 2]);
    let gun_class = g.weapons[0].class;
    let from = v.pos + v.rot * g.fire_point;
    // Preserve map bodies and turret transform, hold a target on a clear
    // sightline. This is a gun/LOS fixture, not a bot navigation playthrough.
    let (right, forward) = (v.rot * Vec3::X, v.rot * Vec3::Z);
    let at = (0..16)
        .find_map(|k| {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            let p = from + (forward * a.cos() + right * a.sin()) * 60.0;
            core::perception::los_seat(
                &w.sim,
                from,
                p,
                core::collision::HullFilter::except(Some(h)),
                [v.instance, None],
            )
            .then_some(p)
        })
        .expect("clear 60 m sightline");
    let enemy = spawn(
        &mut w,
        "all_inf_rifleman",
        1 - v.home_team.unwrap(),
        at - Vec3::Y * 0.9,
        Controller::Player { slot: 0 },
    );
    w.sim.cheats.invincible = true;
    let mut fired = 0;
    for _ in 0..600 {
        let u = w.sim.unit_mut(enemy).unwrap();
        u.soldier.pos = at - Vec3::Y * 0.9;
        u.soldier.vel = Vec3::ZERO;
        w.sim.step(&[PadState::default()]);
        fired += w.sim.events.iter().filter(|e| matches!(e, Event::Shot { unit, weapon_class, .. } if unit.index == u32::MAX && unit.gen == h.index && *weapon_class as usize == gun_class)).count();
    }
    assert!(fired > 4, "self tripod emitted {fired} shots");
    assert!(w
        .sim
        .vehicle(h)
        .unwrap()
        .seats
        .iter()
        .all(|s| s.occupant.is_none()));
    // An autonomous building has no soldier to receive kill credit. Feed
    // an actual lethal hit through the HUD/statistics consumer as well.
    let mut feed =
        game::event_feed::EventFeed::new(game::event_feed::Texts::builtin(), ["all", "imp"], &[]);
    w.sim.cheats.invincible = false;
    w.sim.unit_mut(enemy).unwrap().health.cur = 1.0;
    let mut died = false;
    for _ in 0..300 {
        let u = w.sim.unit_mut(enemy).unwrap();
        u.soldier.pos = at - Vec3::Y * 0.9;
        u.soldier.vel = Vec3::ZERO;
        w.sim.step(&[PadState::default()]);
        feed.tick(&w.sim, enemy);
        assert!(!w
            .sim
            .events
            .iter()
            .any(|e| matches!(e, Event::Kill { shooter, .. } if shooter.index == u32::MAX)));
        if w.sim
            .events
            .iter()
            .any(|e| matches!(e, Event::EnvDeath { unit, .. } if *unit == enemy))
        {
            died = true;
            break;
        }
    }
    assert!(
        died,
        "placed gun must actually kill, without a fabricated soldier credit"
    );
    assert_eq!(w.sim.units[enemy.index as usize].deaths, 1);
}

#[test]
fn placed_self_turret_classes_use_their_ps2_guns() {
    let mut db = game::classdb::ClassDb::default();
    db.load(&game::assets::baked_root().join("classes/bf2_tur.ron"))
        .expect("PS2 TUR class bake required");
    for name in [
        "tur_bldg_chaingun_tripod",
        "tur_bldg_chaingun_roof",
        "tur_bldg_recoilless_fel_auto",
        "tur_bldg_recoilless_myg_auto",
        "tur_bldg_recoilless_nab_auto",
    ] {
        let seats = game::vehicle_class::seats(&db, name);
        let self_seats: Vec<_> = seats
            .iter()
            .filter(|s| s.pilot_type == game::vehicle_class::PilotType::SelfPilot)
            .collect();
        assert_eq!(self_seats.len(), 1, "{name}");
        assert_eq!(self_seats[0].weapons.len(), 1, "{name}");
        let wc = game::roster::weapon_from_db(&db, &self_seats[0].weapons[0].class).unwrap();
        assert_eq!(
            wc.max_pressed_time, 0.0,
            "{name}: no charged self weapon on these classes"
        );
        assert!(!wc.trigger_single, "{name}: cannon burst trigger");
        assert!(wc.ai_range[2] > 0.0 && wc.shot_delay[0] > 0.0, "{name}");
    }
}
