use crate::projectile::{Bolt, Hit, Target};
use crate::sim::tests::flat_ground;
use crate::sim::{DeathCause, Event, PadState, Sim};
use crate::unit::{UnitHandle, UnitSpec};
use crate::vehicle::{SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use crate::weapon::{HealthType, WeaponClass};
use glam::Vec3;

/// The terrain meets a sphere only while it is within r of the centre's
/// height (re-hover §13.5; card HCON's case): a sphere buried under the
/// sheet — POL1's hangar AAT, 8 m under the asteroid — touches nothing,
/// one straddling it is pushed out.
#[test]
fn the_terrain_meets_a_sphere_only_within_its_radius() {
    use super::WorldContacts;
    use crate::collision::CollisionWorld;
    use crate::heightfield::Heightfield;
    use crate::vehicle::hover::ContactSource;
    let hf = Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![20.0; 4],
    };
    let collision = CollisionWorld::new(Vec::new(), Vec::new());
    let w = WorldContacts {
        heightfield: Some(&hf),
        collision: &collision,
    };
    let depth = |y: f32| {
        let mut out = Vec::new();
        w.sphere_contacts(Vec3::new(0.0, y, 0.0), 3.5, &mut out);
        out.iter().map(|c| c.depth).collect::<Vec<_>>()
    };
    assert_eq!(depth(24.0), Vec::<f32>::new(), "above, clear");
    assert_eq!(depth(21.0), vec![2.5]);
    assert_eq!(depth(18.0), vec![5.5], "centre under, within r");
    assert_eq!(depth(16.0), Vec::<f32>::new(), "buried deeper than r");
    assert_eq!(depth(13.0), Vec::<f32>::new(), "the hangar floor case");
}

/// An ordinary hover from a pad of `home_team`: pilot + 3 passengers.
fn hover4(home_team: Option<u8>) -> VehicleSpec {
    VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team,
        neutral_when_empty: true,
        spawn_pad: Some(3),
        pos: Vec3::new(10.0, 1.0, 0.0),
        yaw: 0.5,
        max_health: 900.0,
        health_type: HealthType::Vehicle,
        seats: vec![
            SeatKind::Pilot,
            SeatKind::Passenger(0),
            SeatKind::Passenger(1),
            SeatKind::Passenger(2),
        ],
    }
}

#[test]
fn a_dead_slot_is_reused_with_a_new_generation() {
    let mut s = Sim::empty(1);
    let a = s.spawn_vehicle(&hover4(Some(0)));
    let b = s.spawn_vehicle(&hover4(Some(1)));
    assert_eq!((a.index, a.gen, b.index, b.gen), (0, 1, 1, 1));
    s.remove_vehicle(a);
    assert!(s.vehicle(a).is_none());
    let c = s.spawn_vehicle(&hover4(None));
    assert_eq!((c.index, c.gen), (0, 2), "slot 0 reused");
    assert!(s.vehicle(a).is_none(), "the stale handle stays dead");
    assert_eq!(s.vehicle(c).unwrap().home_team, None);
    assert_eq!(s.alive_vehicles().count(), 2);
    assert_eq!(s.vehicles.len(), 2);
}

#[test]
fn occupancy_is_written_on_both_sides() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let p = s.spawn(&UnitSpec::player(1, Vec3::ZERO)).unwrap();
    let q = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(0)));
    assert!(!s.seat_unit(p, v, 9), "no such seat");
    assert!(s.seat_unit(p, v, 0));
    assert!(!s.seat_unit(q, v, 0), "taken");
    assert!(!s.seat_unit(p, v, 1), "already mounted");
    assert_eq!(s.seat_of(p), Some((v, SeatKind::Pilot)));
    assert!(s.seat_unit(q, v, 2));
    s.unseat_unit(p);
    assert_eq!(s.unit(p).unwrap().mount, None);
    assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, None);
    s.remove_vehicle(v);
    assert_eq!(s.unit(q).unwrap().mount, None, "riders are released");
}

#[test]
fn a_unit_killed_in_a_seat_leaves_it() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let q = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(0)));
    assert!(s.seat_unit(q, v, 1));
    s.kill_environment(q, crate::sim::DeathCause::Fall);
    assert_eq!(s.vehicle(v).unwrap().seats[1].occupant, None);
    assert!(s.vehicle(v).unwrap().is_empty());
    assert_eq!(
        s.vehicle_team(v),
        None,
        "the last rider's death leaves it neutral"
    );
}

/// re-vehicles §7.1, row by row, on an ordinary class.
#[test]
fn the_live_team_is_the_last_entrants_and_empty_is_neutral() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let a = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let b = s.spawn(&UnitSpec::trooper(1, Vec3::Z)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(0)));
    // A pad gives an EMPTY ordinary vehicle no team, but keeps its own.
    assert_eq!(s.vehicle_team(v), None);
    assert_eq!(s.vehicle(v).unwrap().home_team, Some(0));
    // ANY seat sets the team — a passenger here, not the pilot.
    assert!(s.seat_unit(b, v, 2));
    assert_eq!(s.vehicle_team(v), Some(1));
    // The newest entrant's team wins, in whatever seat.
    assert!(s.seat_unit(a, v, 0));
    assert_eq!(s.vehicle_team(v), Some(0));
    // A rider leaving while another stays changes nothing.
    s.unseat_unit(a);
    assert_eq!(s.vehicle_team(v), Some(0), "still the last entrant's");
    // The last one out: neutral again; the home team is untouched.
    s.unseat_unit(b);
    assert_eq!(s.vehicle_team(v), None);
    assert_eq!(s.vehicle(v).unwrap().home_team, Some(0));
}

#[test]
fn a_command_vehicle_takes_the_pads_team_and_keeps_its_last() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let b = s.spawn(&UnitSpec::trooper(1, Vec3::Z)).unwrap();
    let spec = VehicleSpec {
        neutral_when_empty: false,
        ..hover4(Some(0))
    };
    let v = s.spawn_vehicle(&spec);
    assert_eq!(s.vehicle_team(v), Some(0), "the pad's team, empty");
    assert!(s.seat_unit(b, v, 1));
    assert_eq!(s.vehicle_team(v), Some(1));
    s.unseat_unit(b);
    assert_eq!(s.vehicle_team(v), Some(1), "never neutral");
    assert_eq!(s.vehicle(v).unwrap().home_team, Some(0));
}

/// The VENT audit's two cases: a unit taken off the field by the
/// no-overspawn shed or a vacated player slot leaves its seat, so the
/// vehicle empties and the reused unit slot can take the seat again.
#[test]
fn despawns_leave_the_seat() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let q = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(0)));
    assert!(s.seat_unit(q, v, 1));
    s.shed_ai(0, 1);
    assert!(s.unit(q).is_none());
    assert_eq!(s.vehicle(v).unwrap().seats[1].occupant, None);
    assert!(s.vehicle(v).unwrap().is_empty());
    assert_eq!(s.vehicle_team(v), None);
    let r = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    assert_eq!((r.index, r.gen), (q.index, q.gen + 1), "the slot is reused");
    assert!(s.seat_unit(r, v, 1), "the reused unit takes the seat");

    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(0)));
    assert!(s.seat_unit(p, v, 0));
    assert!(s.vacate_player_slot(0).is_some());
    assert!(s.unit(p).is_none());
    assert_eq!(s.vehicle(v).unwrap().seats[0].occupant, None);
    assert!(s.vehicle(v).unwrap().is_empty());
    assert_eq!(s.vehicle_team(v), None);
}

/// The BF1 speeder bike's hull: MaxHealth 600, one seat.
fn speederbike() -> VehicleSpec {
    VehicleSpec {
        max_health: 600.0,
        seats: vec![SeatKind::Pilot],
        ..hover4(Some(1))
    }
}

/// The BF1 AT-ST's hull: MaxHealth 6000, pilot + one gunner (a walker).
fn atst() -> VehicleSpec {
    VehicleSpec {
        kind: VehicleKind::Walker,
        max_health: 6000.0,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        ..hover4(Some(1))
    }
}

/// A bolt of weapon class `wc` fired by `owner` that the spawn ray sent
/// into vehicle `v`, landing at age 0.
fn bolt_into(s: &Sim, owner: UnitHandle, wc: usize, v: VehicleHandle) -> Bolt {
    Bolt {
        visual_shot: None,
        pos: Vec3::ZERO,
        vel: Vec3::X,
        remaining: 0.0,
        age: 0.0,
        owner,
        team: s.units[owner.index as usize].team,
        weapon_class: wc,
        hit: Hit::Target(Target::Vehicle(v)),
        end: Vec3::new(10.0, 1.0, 0.0),
        shooter_is_player: false,
        hulls: crate::collision::HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    }
}

/// A bolt takes the ordnance's `VehicleScale` off the ONE hull pool:
/// the BF2 rifle's 80 × 0.1 = 8 a hit, 75 hits for the bike's 600; BF1's
/// rifle (VehicleScale 0) never scratches it. The 75th hit destroys it in
/// the same call — the event, the rider dead with a ticket, no wreck.
#[test]
fn a_speederbike_dies_to_its_75th_bf2_rifle_bolt() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let bf2 = s.add_weapon_class(WeaponClass::bf2_rifle());
    let shooter = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let rider = s.spawn(&UnitSpec::trooper(1, Vec3::Z)).unwrap();
    let v = s.spawn_vehicle(&speederbike());
    assert!(s.seat_unit(rider, v, 0));
    let tickets = s.battle.teams[1].tickets();

    s.resolve_bolt(&bolt_into(&s, shooter, 0, v));
    assert_eq!(s.vehicle(v).unwrap().health.cur, 600.0, "BF1 rifle: 0");
    assert!(s.events.contains(&Event::BoltExpired {
        pos: Vec3::new(10.0, 1.0, 0.0),
        struck: true
    }));
    // FXP3: a hull is rigid to the impact effect, which rides it.
    assert!(s.events.iter().any(|e| matches!(
        e,
        Event::Impact {
            surface: crate::impact::Surface::Vehicle,
            vehicle: Some(h),
            ..
        } if *h == v
    )));

    for n in 1..=75 {
        s.events.clear();
        s.resolve_bolt(&bolt_into(&s, shooter, bf2, v));
        let hit = s.events.iter().find_map(|e| match e {
            Event::VehicleHit { damage, .. } => Some(*damage),
            _ => None,
        });
        assert!(hit.is_some_and(|d| (d - 8.0).abs() < 1e-4), "{hit:?}");
        if n < 75 {
            assert!(s.vehicle(v).is_some(), "alive after {n}");
        }
    }
    assert!(s.vehicle(v).is_none(), "gone: no wreck");
    let destroyed = s.events.iter().find_map(|e| match e {
        Event::VehicleDestroyed {
            vehicle, killer, ..
        } => Some((*vehicle, *killer)),
        _ => None,
    });
    assert_eq!(destroyed, Some((v, Some(shooter))));
    assert!(s.unit(rider).is_none(), "the rider died with it");
    assert!(s.events.contains(&Event::Kill {
        shooter,
        victim: rider
    }));
    assert_eq!(s.battle.teams[1].tickets(), tickets - 1);

    // A bolt already in flight at a dead hull: no impact at all.
    s.events.clear();
    s.resolve_bolt(&bolt_into(&s, shooter, bf2, v));
    assert!(s.events.is_empty(), "{:?}", s.events);
}

/// Card RIDK (re-vehicle-death 10): a destroyed vehicle's riders are
/// KILLED by the killing hit's attacker - one kill each, the passengers
/// before the driver, a ticket each, past the `invincible` cheat - and a
/// destroyer who died first is still credited.
#[test]
fn a_destroyers_riders_are_his_kills() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    s.cheats.invincible = true;
    let mut rocket = WeaponClass::bf2_rifle();
    rocket.ordnance.max_damage = 5000.0;
    rocket.ordnance.falloff = None;
    rocket.ordnance.scales = [1.0; 7];
    let wc = s.add_weapon_class(rocket);
    let shooter = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let driver = s.spawn(&UnitSpec::player(1, Vec3::Z)).unwrap();
    let rider = s.spawn(&UnitSpec::trooper(1, Vec3::Y)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(1)));
    assert!(s.seat_unit(driver, v, 0));
    assert!(s.seat_unit(rider, v, 2));
    let tickets = s.battle.teams[1].tickets();
    s.events.clear();
    s.resolve_bolt(&bolt_into(&s, shooter, wc, v));
    assert!(s.vehicle(v).is_none());
    let deaths: Vec<&Event> = s
        .events
        .iter()
        .filter(|e| matches!(e, Event::Kill { .. } | Event::EnvDeath { .. }))
        .collect();
    assert_eq!(
        deaths,
        [
            &Event::Kill {
                shooter,
                victim: rider
            },
            &Event::Kill {
                shooter,
                victim: driver
            },
        ],
        "the passenger, then the driver; the invincible player too"
    );
    assert_eq!(s.unit(shooter).unwrap().kills, 2);
    assert_eq!(s.battle.teams[1].tickets(), tickets - 2);
    assert_eq!(s.nav_stats.unit_kills, 2);

    // A friend's rocket: team kills. A destroyer already dead when his
    // rocket lands still scores them.
    let v = s.spawn_vehicle(&hover4(Some(0)));
    let a = s.spawn(&UnitSpec::trooper(0, Vec3::Z * 3.0)).unwrap();
    let b = s.spawn(&UnitSpec::trooper(0, Vec3::Y * 3.0)).unwrap();
    assert!(s.seat_unit(a, v, 0));
    assert!(s.seat_unit(b, v, 1));
    let friend = s.spawn(&UnitSpec::trooper(0, Vec3::NEG_X)).unwrap();
    let bolt = bolt_into(&s, friend, wc, v);
    s.units[friend.index as usize].alive = false;
    s.resolve_bolt(&bolt);
    assert!(s.vehicle(v).is_none());
    assert_eq!(s.units[friend.index as usize].kills, 2);
    assert_eq!(s.nav_stats.team_kills, 2);
}

/// Card RIDK: the same hover destroyed by a hull impact (no attacker):
/// its riders die with no killer - a world death each, the invincible
/// player too.
#[test]
fn a_crashed_vehicles_riders_die_with_no_killer() {
    let mut s = Sim::empty(1);
    s.cheats.invincible = true;
    let driver = s.spawn(&UnitSpec::player(1, Vec3::Z)).unwrap();
    let rider = s.spawn(&UnitSpec::trooper(1, Vec3::Y)).unwrap();
    let v = s.spawn_vehicle(&hover4(Some(1)));
    assert!(s.seat_unit(driver, v, 0));
    assert!(s.seat_unit(rider, v, 1));
    let tickets = s.battle.teams[1].tickets();
    s.events.clear();
    s.impact_vehicle(v, -60.0, 0.0, 0.0, [5.0, 0.0, 0.0], [1.0, 1.0, 1.0], 1.0);
    assert!(s.vehicle(v).is_none(), "55^2 > 900");
    assert!(s.unit(driver).is_none() && s.unit(rider).is_none());
    let deaths: Vec<&Event> = s
        .events
        .iter()
        .filter(|e| matches!(e, Event::Kill { .. } | Event::EnvDeath { .. }))
        .collect();
    let env = |unit| Event::EnvDeath {
        unit,
        cause: DeathCause::VehicleDestroyed,
    };
    assert_eq!(deaths, [&env(rider), &env(driver)]);
    assert_eq!(s.battle.teams[1].tickets(), tickets - 2);
}

/// The AT-ST's 6000 through a heavy ordnance at VehicleScale 1 (500 a
/// hit → 12), friendly fire on its live team, and a neutral hull that
/// has no friends; the tauntaun's pool reads AnimalScale.
#[test]
fn an_atst_takes_team_and_type_scales() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    s.friendly_fire_scale = 0.5;
    let mut heavy = WeaponClass::bf2_rifle();
    heavy.ordnance.max_damage = 500.0;
    heavy.ordnance.falloff = None;
    heavy.ordnance.scales = [1.0, 0.2, 1.0, 1.0, 1.0, 1.0, 1.0];
    let wc = s.add_weapon_class(heavy);
    let red = s.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let blue = s.spawn(&UnitSpec::trooper(1, Vec3::Z)).unwrap();
    let v = s.spawn_vehicle(&atst());
    // Empty and neutral: team 1's bolt is not friendly.
    s.resolve_bolt(&bolt_into(&s, blue, wc, v));
    assert_eq!(s.vehicle(v).unwrap().health.cur, 5500.0);
    // Team 1 boards: now team 1's bolts are friendly (× 0.5).
    let pilot = s.spawn(&UnitSpec::trooper(1, Vec3::Y)).unwrap();
    assert!(s.seat_unit(pilot, v, 0));
    s.resolve_bolt(&bolt_into(&s, blue, wc, v));
    assert_eq!(s.vehicle(v).unwrap().health.cur, 5250.0);
    let mut hits = 0;
    while s.vehicle(v).is_some() {
        s.resolve_bolt(&bolt_into(&s, red, wc, v));
        hits += 1;
    }
    assert_eq!(hits, 11, "5250 / 500, rounded up");
    assert!(s.unit(pilot).is_none());

    let taun = s.spawn_vehicle(&VehicleSpec {
        health_type: HealthType::Animal,
        ..speederbike()
    });
    s.resolve_bolt(&bolt_into(&s, red, wc, taun));
    assert_eq!(s.vehicle(taun).unwrap().health.cur, 500.0, "500 × 0.2");
}

/// A hull contact (§3.2 item 10) through the bike's CollisionScale
/// "1.0 -0.5 0" with a 5 m/s threshold, raw: no HealthType scale, no
/// attacker; the other body's factor multiplies it.
#[test]
fn collision_damage_is_raw_and_can_kill() {
    let mut s = Sim::empty(1);
    let v = s.spawn_vehicle(&speederbike());
    let (t, sc) = ([5.0, 0.0, 0.0], [1.0, -0.5, 0.0]);
    assert_eq!(s.impact_vehicle(v, -4.0, 0.0, 0.0, t, sc, 1.0), 0.0);
    assert!(s.events.is_empty(), "under the threshold: no event");
    // Side-on (n·up = 0) at 15 m/s: (−15 + 5)² × 1 = 100.
    assert_eq!(s.impact_vehicle(v, -15.0, 0.0, 0.0, t, sc, 1.0), 100.0);
    assert!(s.events.contains(&Event::VehicleHit {
        shooter: None,
        vehicle: v,
        damage: 100.0,
        dir: None,
    }));
    // Belly-first (n·up = 1) halves the scale; a factor 2 other body.
    assert_eq!(s.impact_vehicle(v, -15.0, 1.0, 0.0, t, sc, 2.0), 100.0);
    assert_eq!(s.vehicle(v).unwrap().health.cur, 400.0);
    s.impact_vehicle(v, -30.0, 0.0, 0.0, t, sc, 1.0);
    assert!(s.vehicle(v).is_none(), "625 > 400");
    assert!(s
        .events
        .iter()
        .any(|e| matches!(e, Event::VehicleDestroyed { killer: None, .. })));
}

/// The acceptance pair: an EMPTY vehicle list costs the hash nothing (the
/// pinned `--sim-hash 100` literal in state.rs holds that for the whole
/// sim), and a non-empty one is appended — every field reaches it.
#[test]
fn vehicles_are_appended_to_the_hash() {
    let mut a = Sim::empty(4);
    a.heightfield = Some(flat_ground());
    let p = a.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    a.step(&[PadState::default()]);
    let bare = a.hash();
    let v = a.spawn_vehicle(&hover4(Some(0)));
    let one = a.hash();
    assert_ne!(bare, one, "a vehicle is state");
    a.remove_vehicle(v);
    assert_ne!(a.hash(), bare, "a dead slot is still a slot");

    let base = {
        let mut s = Sim::empty(4);
        s.spawn_vehicle(&hover4(Some(0)));
        s
    };
    let h0 = base.hash();
    type Poke = fn(&mut crate::vehicle::Vehicle);
    let pokes: [(&str, Poke); 12] = [
        ("pos", |v| v.pos.y += 0.001),
        ("vel", |v| v.vel.x = 1.0),
        ("rot", |v| v.rot *= crate::vehicle::hover::quat_x(0.1)),
        ("input", |v| v.input.throttle = 1.0),
        ("hover", |v| v.hover.g = 0.5),
        ("health", |v| v.health.cur -= 1.0),
        ("team", |v| v.team = Some(1)),
        ("home_team", |v| v.home_team = None),
        ("neutral_when_empty", |v| v.neutral_when_empty = false),
        ("health_type", |v| v.health.health_type = HealthType::Animal),
        ("class", |v| v.class = 7),
        ("pad", |v| v.spawn_pad = None),
    ];
    for (name, poke) in pokes {
        let mut s = Sim::empty(4);
        s.spawn_vehicle(&hover4(Some(0)));
        poke(&mut s.vehicles[0]);
        assert_ne!(s.hash(), h0, "{name} is hashed");
    }

    // Occupancy reaches the hash on both sides, and seat order matters.
    let mut m = Sim::empty(4);
    m.heightfield = Some(flat_ground());
    let u = m.spawn(&UnitSpec::trooper(0, Vec3::X)).unwrap();
    let v = m.spawn_vehicle(&hover4(Some(0)));
    let empty = m.hash();
    assert!(m.seat_unit(u, v, 1));
    let seated1 = m.hash();
    m.unseat_unit(u);
    assert_eq!(m.hash(), empty, "unseating restores the state");
    assert!(m.seat_unit(u, v, 2));
    assert_ne!(m.hash(), seated1);
    assert_ne!(m.hash(), empty);
    let _ = p;
}

// ---- The spawn pads (re-vehicles §7, card VPAD) ----

use crate::cp::CpDef;
use crate::region::Region;
use crate::vehicle_pad::{PadClass, PadDef, DECAY_TIME, SPAWN_TIME};

const TICKS_PER_S: u32 = crate::TICK_HZ;

/// A pad at the origin: class 7 (ordinary hover) for team 0, class 8 for
/// team 1, the §7 defaults, no post.
fn pad(team: Option<u8>, cp: Option<usize>) -> PadDef {
    let class = |c: u32| {
        Some(PadClass {
            spec: VehicleSpec {
                class: c,
                spawn_pad: None,
                ..hover4(None)
            },
            radius: 3.0,
            landed_height: None,
            vis_box: None,
        })
    };
    PadDef {
        name: "pad".into(),
        pos: Vec3::ZERO,
        yaw: 1.0,
        cp,
        team,
        spawn_time: SPAWN_TIME,
        decay_time: DECAY_TIME,
        spawn_count: 1,
        classes: [class(7), class(8)],
    }
}

/// A post at the origin with a 30 m ControlRegion (or none), owned by
/// `owner`. Nobody stands in it, so its owner never changes by itself.
fn post(s: &mut Sim, owner: Option<u8>, control: bool) -> usize {
    let (capture_time, neutralize_time) = CpDef::class_times(false);
    s.add_cp(
        CpDef {
            name: "cp".into(),
            pos: Vec3::ZERO,
            capture: Region::sphere(Vec3::ZERO, 8.0),
            control: control.then(|| Region::sphere(Vec3::ZERO, 30.0)),
            value_bleed: 0,
            value_atk: [0, 0],
            value_def: [0, 0],
            spawn_nodes: vec![(Vec3::new(0.0, 0.0, 3.0), 0.0)],
            capture_time,
            neutralize_time,
        },
        owner,
    )
}

fn pad_vehicle(s: &Sim, pi: usize) -> Option<VehicleHandle> {
    s.vehicle_pads[pi].vehicles.first().map(|v| v.handle)
}

/// Step until pad `pi` has a vehicle; the ticks taken.
fn ticks_to_spawn(s: &mut Sim, pi: usize, limit: u32) -> Option<u32> {
    (1..=limit).find(|_| {
        s.step(&[]);
        pad_vehicle(s, pi).is_some()
    })
}

#[test]
fn a_pad_spawns_at_once_then_respawns_after_spawn_time() {
    let mut s = Sim::empty(1);
    let pi = s.add_vehicle_pad(pad(Some(1), None));
    assert_eq!(ticks_to_spawn(&mut s, pi, 1), Some(1), "armed at 0");
    let h = pad_vehicle(&s, pi).unwrap();
    let v = s.vehicle(h).unwrap();
    assert_eq!((v.class, v.spawn_pad), (8, Some(0)));
    assert_eq!(v.rot, crate::vehicle::hover::quat_y(-1.0), "the pad yaw");
    assert_eq!(v.team, None, "an empty ordinary vehicle is neutral");
    assert_eq!(v.home_team, Some(1));
    // One vehicle per pad: nothing more while it lives.
    for _ in 0..(20 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.alive_vehicles().count(), 1);
    // Gone → the next one comes SpawnTime later.
    s.remove_vehicle(h);
    let n = ticks_to_spawn(&mut s, pi, 20 * TICKS_PER_S).unwrap();
    assert!(n.abs_diff(10 * TICKS_PER_S) <= 1, "respawn after {n} ticks");
}

/// Card VREST (re-vehicle-rest §1-§3): a flyer is created at the first
/// surface within 1 m of its pad — else at the pad's own point — plus
/// its class's LandedHeight, and stays there; a hover's pad still finds
/// the ground at any depth.
#[test]
fn a_flyer_spawns_its_landed_height_over_the_pads_ray() {
    let spawn_y = |pad_y: f32, landed: Option<f32>| {
        let mut s = Sim::empty(1);
        s.heightfield = Some(flat_ground());
        let mut def = pad(Some(0), None);
        def.pos.y = pad_y;
        for c in def.classes.iter_mut().flatten() {
            c.landed_height = landed;
            if landed.is_some() {
                c.spec.kind = VehicleKind::Flyer;
            }
        }
        let pi = s.add_vehicle_pad(def);
        s.step(&[]);
        let h = pad_vehicle(&s, pi).unwrap();
        let first = s.vehicle(h).unwrap().pos.y;
        for _ in 0..(5 * TICKS_PER_S) {
            s.step(&[]);
        }
        assert_eq!(s.vehicle(h).unwrap().pos.y, first, "a landed flyer moved");
        first
    };
    // The ground 0.4 m under the pad, and 0.4 m over it: the hit.
    assert_eq!(spawn_y(0.4, Some(1.75)), 1.75);
    assert_eq!(spawn_y(-0.4, Some(1.75)), 1.75);
    // A negative height (the TIE bomber's model starts over its origin) must
    // not stand the origin under the deck: every flyer ray starts at the origin.
    assert_eq!(spawn_y(0.4, Some(-0.25)), crate::vehicle::flyer::REST_LIFT);
    // The ground over 1 m away either way: the pad's own point.
    assert_eq!(spawn_y(3.0, Some(1.75)), 4.75);
    assert_eq!(spawn_y(-1.5, Some(1.75)), 0.25);
}

/// BF2's `SpawnCount`: the list fills one countdown at a time.
#[test]
fn spawn_count_two_keeps_two() {
    let mut s = Sim::empty(1);
    let pi = s.add_vehicle_pad(PadDef {
        spawn_count: 2,
        ..pad(Some(0), None)
    });
    s.step(&[]);
    assert_eq!(s.vehicle_pads[pi].vehicles.len(), 1);
    for _ in 0..(10 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.vehicle_pads[pi].vehicles.len(), 2);
    for _ in 0..(30 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.alive_vehicles().count(), 2, "capped at SpawnCount");
}

#[test]
fn a_pad_waits_for_its_last_vehicle_to_leave_the_class_box() {
    let mut s = Sim::empty(1);
    let mut def = PadDef {
        spawn_count: 2,
        ..pad(Some(0), None)
    };
    def.classes[0].as_mut().unwrap().vis_box = Some((Vec3::splat(-4.0), Vec3::splat(4.0)));
    let pi = s.add_vehicle_pad(def);
    s.step(&[]);
    let first = pad_vehicle(&s, pi).unwrap();
    s.vehicle_mut(first).unwrap().aim_sphere = Some((Vec3::ZERO, 2.0));
    for _ in 0..(30 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.vehicle_pads[pi].vehicles.len(), 1, "the box is taken");
    s.vehicle_mut(first).unwrap().pos = Vec3::new(5.0, 0.0, 0.0);
    for _ in 0..(30 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(
        s.vehicle_pads[pi].vehicles.len(),
        1,
        "its sphere still reaches"
    );
    s.vehicle_mut(first).unwrap().pos = Vec3::new(12.0, 0.0, 0.0);
    for _ in 0..(10 * TICKS_PER_S + 1) {
        s.step(&[]);
    }
    assert_eq!(s.vehicle_pads[pi].vehicles.len(), 2, "clear: one more");
}

#[test]
fn a_pad_with_no_class_for_its_team_spawns_nothing() {
    let mut s = Sim::empty(1);
    let mut d = pad(Some(0), None);
    d.classes[0] = None;
    s.add_vehicle_pad(d);
    s.add_vehicle_pad(pad(None, None));
    for _ in 0..(30 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.alive_vehicles().count(), 0);
}

/// The pad takes its post's owner each tick; a change reloads the
/// countdown, and the next vehicle is the new team's class.
#[test]
fn a_pad_follows_its_post_and_a_change_resets_the_countdown() {
    let mut s = Sim::empty(1);
    let ci = post(&mut s, None, true);
    let pi = s.add_vehicle_pad(pad(Some(0), Some(ci)));
    for _ in 0..(30 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.vehicle_pads[pi].team, None, "a neutral post");
    assert_eq!(s.alive_vehicles().count(), 0);
    s.cps[ci].owner = Some(1);
    let n = ticks_to_spawn(&mut s, pi, 20 * TICKS_PER_S).unwrap();
    assert!(n.abs_diff(10 * TICKS_PER_S) <= 1, "{n} ticks");
    let v = s.vehicle(pad_vehicle(&s, pi).unwrap()).unwrap();
    assert_eq!((v.class, v.home_team), (8, Some(1)));
}

/// Ticks until pad `pi`'s first vehicle is gone, from now.
fn ticks_to_decay(s: &mut Sim, pi: usize, limit: u32) -> Option<u32> {
    let h = pad_vehicle(s, pi).unwrap();
    (1..=limit).find(|_| {
        s.step(&[]);
        s.vehicle(h).is_none()
    })
}

/// Outside the ControlRegion (here: a post with none): 21 idle seconds,
/// then MaxHealth / DecayTime a second — gone 10 hits later.
#[test]
fn an_idle_vehicle_outside_its_post_decays_after_twenty_seconds() {
    let mut s = Sim::empty(1);
    let ci = post(&mut s, Some(0), false);
    let pi = s.add_vehicle_pad(pad(Some(0), Some(ci)));
    s.step(&[]);
    let h = pad_vehicle(&s, pi).unwrap();
    // The idle count grows once a second from the first tick (the
    // 1 s sub-timer's f32 residue makes some periods 29 ticks, as the
    // PS2's own subtraction does): the 21st (> 20) hits, ~20 s on.
    let first_hit = (1..=(25 * TICKS_PER_S))
        .find(|_| {
            s.step(&[]);
            s.vehicle(h).unwrap().health.cur < 900.0
        })
        .unwrap();
    assert!(
        first_hit.abs_diff(20 * TICKS_PER_S) <= 10,
        "first hit after {first_hit} ticks"
    );
    assert_eq!(s.vehicle(h).unwrap().health.cur, 810.0, "900 / 10");
    let n = ticks_to_decay(&mut s, pi, 20 * TICKS_PER_S).unwrap();
    assert!(n.abs_diff(9 * TICKS_PER_S) <= 5, "{n}");
    // … and the pad makes another SpawnTime later.
    let n = ticks_to_spawn(&mut s, pi, 20 * TICKS_PER_S).unwrap();
    assert!(n.abs_diff(10 * TICKS_PER_S) <= 1, "{n}");
}

/// Inside the ControlRegion, same class: it never decays. Lose the post
/// (the new owner's class differs) and it decays after 10 s instead.
#[test]
fn a_class_change_decays_after_ten_seconds() {
    let mut s = Sim::empty(1);
    let ci = post(&mut s, Some(0), true);
    let pi = s.add_vehicle_pad(pad(Some(0), Some(ci)));
    s.step(&[]);
    let h = pad_vehicle(&s, pi).unwrap();
    for _ in 0..(60 * TICKS_PER_S) {
        s.step(&[]);
    }
    assert_eq!(s.vehicle(h).unwrap().health.cur, 900.0, "at home");
    s.cps[ci].owner = Some(1);
    let n = ticks_to_decay(&mut s, pi, 40 * TICKS_PER_S).unwrap();
    // 11 idle seconds, then 10 hits a second apart.
    assert!(n.abs_diff(20 * TICKS_PER_S) <= 31, "{n}");
    assert_eq!(s.alive_vehicles().count(), 0, "no respawn yet");
}

#[test]
fn a_player_nearby_or_a_rider_or_a_command_class_stops_the_decay() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let ci = post(&mut s, Some(0), false);
    // Pad 0: a player 12 m off (< 0.5 + 3 + 10). Pad 1, 100 m away: a
    // rider. Pad 2: a command class.
    let p0 = s.add_vehicle_pad(pad(Some(0), Some(ci)));
    let p1 = s.add_vehicle_pad(PadDef {
        pos: Vec3::new(100.0, 0.0, 0.0),
        ..pad(Some(0), Some(ci))
    });
    let mut cmd = pad(Some(0), Some(ci));
    cmd.pos = Vec3::new(-100.0, 0.0, 0.0);
    for c in cmd.classes.iter_mut().flatten() {
        c.spec.neutral_when_empty = false;
    }
    let p2 = s.add_vehicle_pad(cmd);
    s.spawn(&UnitSpec::player(0, Vec3::new(12.0, 0.0, 0.0)))
        .unwrap();
    let rider = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(100.0, 0.0, 50.0)))
        .unwrap();
    s.step(&[PadState::default()]);
    let v1 = pad_vehicle(&s, p1).unwrap();
    assert!(s.seat_unit(rider, v1, 1));
    let pads = [PadState::default()];
    for _ in 0..(60 * TICKS_PER_S) {
        s.step(&pads);
    }
    for pi in [p0, p1, p2] {
        let v = s.vehicle(pad_vehicle(&s, pi).unwrap()).unwrap();
        assert_eq!(v.health.cur, 900.0, "pad {pi}");
    }
    assert_eq!(s.vehicle_pads[p0].vehicles[0].idle, 0.0);
}

/// Pads cost the hash nothing until one exists, then their state counts.
#[test]
fn pads_are_hashed_only_when_present() {
    let a = Sim::empty(1);
    let mut b = Sim::empty(1);
    assert_eq!(a.hash(), b.hash());
    b.add_vehicle_pad(pad(None, None));
    assert_ne!(a.hash(), b.hash());
}

// ---- hulls (card VHULL) ---------------------------------------------

/// A 4 x 2 x 6 m box hull standing on the ground (its centre 1 m up),
/// in both subsets — class 0's only part.
fn boxed(s: &mut Sim) {
    use crate::collision::{HullPart, Layers, Shape};
    s.hull_parts = vec![Some(
        vec![HullPart::new(
            glam::Mat4::from_translation(Vec3::Y),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 3.0),
            },
            Layers::ALL,
        )]
        .into(),
    )];
}

/// A player at the origin facing −Z and a parked box-hulled hover 6 m
/// ahead (its near face at z = −3), or no hull when `solid` is false.
fn parked(solid: bool) -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    if solid {
        boxed(&mut s);
    }
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let v = s.spawn_vehicle(&VehicleSpec {
        pos: Vec3::new(0.0, 0.0, -6.0),
        yaw: 0.0,
        ..hover4(None)
    });
    (s, p, v)
}

/// Walk the player forward for `ticks`; its last feet.
fn walk(s: &mut Sim, p: UnitHandle, ticks: u32) -> Vec3 {
    let fwd = PadState {
        ly: 32767,
        ..PadState::default()
    };
    for _ in 0..ticks {
        s.step(&[fwd]);
    }
    s.unit(p).unwrap().soldier.pos
}

/// A soldier walking into a parked vehicle stops at its hull, one
/// capsule radius out; with no hull he walks through it, as before.
#[test]
fn a_soldier_stops_at_a_parked_hull() {
    let (mut s, p, v) = parked(true);
    assert_eq!(s.collision.hull(v).len(), 1, "spawned with its hull");
    let end = walk(&mut s, p, 90);
    assert!(end.z > -3.0 && end.z < -2.0, "stopped at the face: {end}");
    assert!(end.y.abs() < 1e-3, "on the ground: {end}");
    let (mut s, p, _) = parked(false);
    let end = walk(&mut s, p, 90);
    assert!(end.z < -9.0, "no hull, no wall: {end}");
}

/// The spawn ray meets the hull's ordnance subset: a bolt fired at the
/// vehicle is a [`Target::Vehicle`] hit and takes the BF2 rifle's
/// 80 × VehicleScale 0.1 off the pool (re-vehicles §6).
#[test]
fn a_bolt_at_a_parked_hull_damages_it() {
    let (mut s, p, v) = parked(true);
    let bf2 = s.add_weapon_class(WeaponClass::bf2_rifle());
    let fire = PadState {
        buttons: crate::sim::buttons::FIRE,
        ..PadState::default()
    };
    s.step(&[fire]);
    // The hull is met along the flight (issue #41; 3 m at 2.67 m a
    // tick: the second tick), at its near face.
    let mut at = None;
    for _ in 0..4 {
        at = at.or(s.events.iter().find_map(|e| match e {
            Event::Impact {
                pos,
                vehicle: Some(h),
                ..
            } if *h == v => Some(*pos),
            _ => None,
        }));
        s.step(&[PadState::default()]);
    }
    let at = at.expect("an impact on the hull");
    assert!(at.z > -3.01 && at.z < -2.99, "at the near face: {at}");
    // The BF2 rifle's bolt along the same line, landed.
    let shooter = crate::projectile::Shooter {
        owner: p,
        team: 0,
        weapon_class: bf2,
        is_player: true,
    };
    let b = Bolt::spawn(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::NEG_Z,
        80.0,
        2.0,
        shooter,
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        crate::collision::HullFilter::All,
        std::iter::empty(),
    );
    assert_eq!(b.hit, Hit::Target(Target::Vehicle(v)));
    s.events.clear();
    s.resolve_bolt(&b);
    assert!(s.events.contains(&Event::VehicleHit {
        shooter: Some(p),
        vehicle: v,
        damage: 8.0,
        dir: Some(Vec3::NEG_Z),
    }));
    assert_eq!(s.vehicle(v).unwrap().health.cur, 892.0);
}

/// Issue #41: a bolt into a hull part its class authors a HitLocation
/// for (the x4 critical prims) does that much more: a human's always, a
/// bot's only on Hard and Elite (the PS2's level 3 gate), as a headshot.
#[test]
fn a_critical_prim_takes_its_hit_location() {
    use crate::collision::{HullPart, Layers, Shape};
    use crate::skill::Difficulty;
    let damage = |mult: f32, human: bool, d: Difficulty| {
        let (mut s, p, v) = parked(true);
        s.difficulty = d;
        // Card DTEAM: the human is on team 0; one counted on team 1
        // evens them, so team 0's bots keep the tier's row.
        s.extra_humans = [0, 1];
        s.hull_parts = vec![Some(
            vec![HullPart::new(
                glam::Mat4::from_translation(Vec3::Y),
                Shape::Box {
                    half: Vec3::new(2.0, 1.0, 3.0),
                },
                Layers::ALL,
            )
            .with_hit_mult(mult)]
            .into(),
        )];
        s.sync_hull(v.index as usize);
        let bf2 = s.add_weapon_class(WeaponClass::bf2_rifle());
        let b = Bolt::fire(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::NEG_Z,
            80.0,
            2.0,
            crate::projectile::Shooter {
                owner: p,
                team: 0,
                weapon_class: bf2,
                is_player: human,
            },
            s.heightfield.as_ref(),
            s.water_plane(),
            &s.collision,
            crate::collision::HullFilter::All,
        );
        s.bolts.push(b);
        let before = s.vehicle(v).unwrap().health.cur;
        let mut critical = false;
        for _ in 0..4 {
            s.step(&[PadState::default()]);
            critical |= s.events.iter().any(|e| {
                matches!(e, Event::CriticalHit { shooter, victim, weapon_class }
                    if *shooter == p
                        && *victim == Target::Vehicle(v)
                        && *weapon_class == bf2)
            });
        }
        (before - s.vehicle(v).unwrap().health.cur, critical)
    };
    // The critical-hit event (hitmark): the x4 hits that took health.
    assert!(damage(4.0, true, Difficulty::Normal).1, "a human's x4");
    assert!(!damage(1.0, true, Difficulty::Normal).1, "the hull: none");
    assert!(!damage(4.0, false, Difficulty::Normal).1, "a bot's x1");
    assert!(damage(4.0, false, Difficulty::Hard).1, "a bot's x4 on Hard");
    assert!(!damage(0.0, true, Difficulty::Normal).1, "BF1's x0");
    let damage = |mult, human, d| damage(mult, human, d).0;
    let plain = damage(1.0, true, Difficulty::Normal);
    assert_eq!(plain, 8.0, "the BF2 rifle's 80 x VehicleScale 0.1");
    assert_eq!(damage(4.0, true, Difficulty::Normal), 32.0);
    assert_eq!(damage(4.0, false, Difficulty::Normal), 8.0, "a bot: no x4");
    assert_eq!(damage(4.0, false, Difficulty::Easy), 8.0, "nor on Easy");
    assert_eq!(damage(4.0, false, Difficulty::Hard), 32.0);
    assert_eq!(damage(0.0, true, Difficulty::Normal), 0.0, "BF1's x0 prims");
}

/// A destroyed vehicle leaves no wreck (§6) and no hull: the next walk
/// and the next ray pass where it stood.
#[test]
fn a_destroyed_hull_no_longer_blocks() {
    let (mut s, p, v) = parked(true);
    s.destroy_vehicle(v, None);
    assert!(s.collision.hull(v).is_empty());
    assert!(s.collision.is_empty());
    let eye = Vec3::new(0.0, 1.0, 0.0);
    assert!(s
        .collision
        .ray(eye, Vec3::NEG_Z, 20.0, crate::collision::Layers::ORDNANCE)
        .is_none());
    let end = walk(&mut s, p, 90);
    assert!(end.z < -9.0, "walked through the empty pad: {end}");
}

/// The hull follows its vehicle: moved by a caller between ticks, the
/// step's start puts it there; a new vehicle in the slot takes the slot's
/// hull under its own handle.
#[test]
fn the_hull_follows_its_vehicle() {
    use crate::collision::Layers;
    let (mut s, _, v) = parked(true);
    s.vehicle_mut(v).unwrap().pos = Vec3::new(30.0, 0.0, -6.0);
    s.step(&[PadState::default()]);
    let eye = Vec3::new(30.0, 1.0, 0.0);
    let hit = s
        .collision
        .ray_with(
            eye,
            Vec3::NEG_Z,
            20.0,
            Layers::ORDNANCE,
            crate::collision::HullFilter::All,
        )
        .expect("moved with it");
    assert_eq!(hit.owner, Some(v));
    assert!(s
        .collision
        .ray(Vec3::Y, Vec3::NEG_Z, 20.0, Layers::ORDNANCE)
        .is_none());
    s.remove_vehicle(v);
    let w = s.spawn_vehicle(&hover4(None));
    assert_eq!(w.index, v.index);
    assert!(s.collision.hull(v).is_empty(), "the old handle has none");
    assert_eq!(s.collision.hull(w).len(), 1);
}

/// A rider sees and shoots out of its own vehicle: the rays from inside
/// skip its hull (`HullFilter::except`), and a bot's line of sight to a
/// rider skips the rider's (`perception::past_mounts`); every other ray
/// meets it.
#[test]
fn a_rider_looks_out_through_its_own_hull() {
    use crate::collision::{HullFilter, Layers};
    let (mut s, p, v) = parked(true);
    let rider = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -6.0)))
        .unwrap();
    assert!(s.seat_unit(rider, v, 0));
    let inside = Vec3::new(0.0, 1.0, -6.0);
    let out = |f| {
        s.collision
            .ray_with(inside, Vec3::Z, 10.0, Layers::ORDNANCE, f)
            .map(|h| h.owner)
    };
    assert_eq!(out(HullFilter::All), Some(Some(v)), "the hull from inside");
    assert_eq!(out(HullFilter::except(Some(v))), None, "its own: skipped");
    let eye = Vec3::new(0.0, 1.6, 0.0);
    assert!(!crate::perception::los(&s, eye, inside));
    assert!(crate::perception::los_past(
        &s,
        eye,
        inside,
        crate::perception::past_mounts(&s, p, rider)
    ));
}

/// Compare stage 3a against its pre-traffic locomotion consumer, with a real
/// vehicle hull ahead. Both player ground kinds must keep that response.
#[test]
fn player_ground_traffic_keeps_the_legacy_locomotion_response() {
    use crate::collision::{HullPart, Layers, Shape};
    use crate::vehicle::{hover, walker};
    use glam::Mat4;
    for kind in [VehicleKind::Walker, VehicleKind::Hover] {
        let mut s = Sim::empty(1);
        s.heightfield = Some(flat_ground());
        let part = HullPart::new(
            Mat4::IDENTITY,
            Shape::Cylinder {
                radius: 1.0,
                half_height: 10.0,
            },
            Layers::SOLDIER,
        );
        s.hull_parts = vec![Some(vec![part.clone()].into()), Some(vec![part].into())];
        let mut spec = hover4(None);
        spec.kind = kind;
        spec.pos = Vec3::new(0.0, 1.0, 0.0);
        spec.yaw = 0.0;
        let wp = walker::tests::atst();
        let hp = hover::tests::skiff();
        if kind == VehicleKind::Walker {
            s.walker_params = vec![Some(wp.clone())];
        } else {
            s.hover_params = vec![Some(hp.clone())];
        }
        let vehicle = s.spawn_vehicle(&spec);
        spec.kind = VehicleKind::Walker;
        spec.pos.z = 4.0;
        spec.class = 1;
        s.spawn_vehicle(&spec);
        let pilot = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        assert!(s.seat_unit(pilot, vehicle, 0));
        let v = s.vehicle_mut(vehicle).unwrap();
        v.vel = Vec3::Z * 10.0;
        v.input.throttle = 1.0;
        v.input.human = true;
        let mut contacts = 0;
        for _ in 0..180 {
            let from = s.vehicle(vehicle).unwrap().pos;
            let mut expected = s.vehicle(vehicle).unwrap().clone();
            let world = super::WorldContacts {
                heightfield: s.heightfield.as_ref(),
                collision: &s.collision,
            };
            if kind == VehicleKind::Walker {
                walker::step(&mut expected, &wp, &world, crate::TICK_DT);
            } else {
                hover::step(&mut expected, &hp, &world, crate::TICK_DT);
            }
            if s.collision
                .ground_hull_cast(
                    vehicle,
                    VehicleHandle { index: 1, gen: 1 },
                    expected.pos - from,
                )
                .is_some()
            {
                contacts += 1;
            }
            if kind == VehicleKind::Walker {
                s.tick_walkers(crate::TICK_DT);
            } else {
                s.tick_hovers(crate::TICK_DT);
            }
            s.tick += 1;
            let actual = s.vehicle(vehicle).unwrap();
            assert_eq!(actual.pos, expected.pos, "{kind:?} displacement");
            assert_eq!(actual.vel, expected.vel, "{kind:?} velocity");
            assert_eq!(actual.yield_until, 0);
        }
        assert!(
            contacts > 0,
            "fixture must reach another hull: {kind:?}, end={:?}",
            s.vehicle(vehicle).unwrap().pos
        );
        eprintln!("player {kind:?}: {contacts} witnessed hull contacts, legacy position/velocity equal for 180 ticks");
    }
}
