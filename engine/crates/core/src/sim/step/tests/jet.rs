//! Card JETP step 8: the jets in the sim - the cheat, the hash and a
//! replay, the wall pass and the ceiling input.
use super::*;
use crate::soldier::{SoldierConstants, Stance};

/// BF2 `rep_inf_ep3_jettrooper` (re-jetpack.md §2.2), the disc's numbers;
/// the game flies every jet trooper on `game::roster::JET_TROOPER_FUEL_*`.
fn jet_trooper() -> SoldierConstants {
    SoldierConstants {
        max_speed: 7.0,
        max_strafe_speed: 5.25,
        jet_jump: 5.0,
        jet_push: 8.0,
        jet_acceleration: 30.0,
        jet_hover: true,
        jet_fuel_cost: 0.16,
        jet_fuel_initial: 0.25,
        jet_fuel_recharge: 0.12,
        jet_fuel_min_border: 0.24,
        jet_control: [1.5, 1.25, 1.25],
        ..SoldierConstants::default()
    }
}

fn jet_sim(world: Option<CollisionWorld>) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    if let Some(w) = world {
        s.collision = w;
    }
    let spec = UnitSpec {
        consts: jet_trooper(),
        ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
    };
    let h = s.spawn(&spec).unwrap();
    (s, h)
}

/// Tap, release, tap: into the hover on the third tick, then `ly` forward.
fn pads(ticks: usize, ly: i16) -> Vec<PadState> {
    (0..ticks)
        .map(|t| PadState {
            ly: if t < 3 { 0 } else { ly },
            buttons: if t == 0 || t == 2 { buttons::JUMP } else { 0 },
            ..Default::default()
        })
        .collect()
}

#[test]
fn a_player_spawns_with_half_a_tank_and_hovers_on_the_second_tap() {
    let (mut s, h) = jet_sim(None);
    assert_eq!(s.unit(h).unwrap().soldier.fuel, 0.5);
    for p in pads(3, 0) {
        s.step(&[p]);
    }
    let u = s.unit(h).unwrap();
    assert_eq!(u.soldier.stance, Stance::Hover);
    // 0.5 + three updates' recharge (the start's own runs before the
    // switch) - 0.25.
    assert!((u.soldier.fuel - 0.262).abs() < 1e-4, "{}", u.soldier.fuel);
}

/// The cheat keeps a jet class's tank full; without it the spawn's half
/// tank lasts 1.6 s on the disc's numbers (§6).
#[test]
fn the_stamina_cheat_keeps_the_jet_tank_full() {
    for cheat in [false, true] {
        let (mut s, h) = jet_sim(None);
        s.cheats.infinite_stamina = cheat;
        let mut hovered = 0;
        for p in pads(300, 32767) {
            s.step(&[p]);
            if s.unit(h).unwrap().soldier.stance == Stance::Hover {
                hovered += 1;
            }
        }
        let u = s.unit(h).unwrap();
        if cheat {
            assert_eq!(hovered, 298, "the whole run");
            assert_eq!(u.soldier.fuel, 1.0);
        } else {
            assert!((45..=52).contains(&hovered), "{hovered} updates");
        }
    }
    // The cheat gives no jet to a class without one.
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    s.cheats.infinite_stamina = true;
    let h = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    for p in pads(10, 0) {
        s.step(&[p]);
    }
    let u = s.unit(h).unwrap();
    assert_eq!((u.soldier.fuel, u.soldier.stance), (0.0, Stance::Jump));
    let mut c = Cheats::default();
    c.toggle("stamina");
    assert!(c.infinite_stamina && c.entries().iter().all(|e| e.0 != "jet"));
}

/// A replay of a jetting player is the same hash; jetting moves the hash.
#[test]
fn a_jet_replays_to_the_same_hash() {
    let run = |taps: bool| {
        let (mut s, _) = jet_sim(Some(wall_world()));
        for mut p in pads(240, 32767) {
            if !taps {
                p.buttons = 0;
            }
            s.step(&[p]);
        }
        s.hash()
    };
    let a = run(true);
    assert_eq!(a, run(true));
    assert_ne!(a, run(false));
}

/// A hover into a wall is pushed out by the wall pass (the air's step):
/// the wall stands 4 m tall from z -1 to -5 and he never passes its face.
#[test]
fn a_hover_into_a_wall_does_not_pass_it() {
    let mut s = step_sim(4.0);
    let spec = UnitSpec {
        consts: jet_trooper(),
        ..UnitSpec::player_in_slot(0, 0, Vec3::new(0.0, 0.0, 2.0))
    };
    let h = s.spawn(&spec).unwrap();
    s.cheats.infinite_stamina = true;
    let mut min_z = f32::MAX;
    for p in pads(120, 32767) {
        s.step(&[p]);
        let u = s.unit(h).unwrap();
        if u.soldier.pos.y < 3.5 {
            min_z = min_z.min(u.soldier.pos.z);
        }
    }
    assert!(min_z > -1.0, "the hover reached z {min_z}");
}

/// The sim hands its mission ceiling to the soldier: under a 12 m ceiling
/// a hover climbing at full aim stays under it; without one it climbs on.
#[test]
fn the_sims_ceiling_holds_a_climbing_hover_down() {
    let top = |ceiling: f32| {
        let (mut s, h) = jet_sim(None);
        s.fly_ceiling = ceiling;
        s.cheats.infinite_stamina = true;
        let mut top = 0.0f32;
        for (t, mut p) in pads(300, 32767).into_iter().enumerate() {
            // Aim up from the fourth tick.
            if t >= 3 {
                p.ry = 32767;
            }
            s.step(&[p]);
            top = top.max(s.unit(h).unwrap().soldier.pos.y);
        }
        top
    };
    let (capped, free) = (top(12.0), top(crate::soldier::DEFAULT_FLY_CEILING));
    assert!(capped > 6.0 && capped < 12.0, "capped top {capped}");
    assert!(free > 40.0, "free top {free}");
}
