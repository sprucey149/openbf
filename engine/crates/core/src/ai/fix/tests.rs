//! Issue #35: a fixer's aid to vehicles and droids.
use super::*;
use crate::perception::{Contact, Foe};
use crate::unit::{Controller, UnitSpec};
use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
use crate::weapon::{HealthType, Repair, WeaponClass};

fn cutter() -> WeaponClass {
    let mut health = [0.0; 8];
    (health[3], health[4], health[5], health[6]) = (200.0, 100.0, 50.0, 50.0);
    WeaponClass {
        name: "fusioncutter".into(),
        repair: Some(Repair { health }),
        target_team: 6,
        lock_on_range: 4.0,
        lock_on_angle: 90.0 * crate::weapon::DEG,
        shot_delay: [0.5; 2],
        heat_per_shot: 0.06,
        ..WeaponClass::default()
    }
}

/// A damaged friendly hover at the origin and `bots` cutter carriers on foot
/// `d` m from it.
fn world(bots: usize, d: f32) -> (Sim, Vec<UnitHandle>, VehicleHandle) {
    let mut sim = Sim::empty(7);
    sim.heightfield = Some(crate::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    sim.weapon_classes.push(cutter());
    sim.seat_setup = vec![Some(SeatSetup {
        radius: 1.5,
        body_active: vec![],
        boardable: vec![true; 2],
        exit_drop: 10.0,
        ai_size: 1,
        guns: Vec::new(),
    })];
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.aim_sphere = Some((Vec3::Y, 1.0));
        v.health.cur = 400.0;
    }
    let bots = (0..bots)
        .map(|k| {
            let a = k as f32 * 0.7;
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                weapon_class: 1,
                ..UnitSpec::trooper(0, Vec3::new(a.cos() * d, 0.0, a.sin() * d))
            })
            .unwrap()
        })
        .collect();
    (sim, bots, vh)
}

/// The claim one think's upkeep makes for `b` (on a copy of its state).
fn pick(sim: &Sim, b: UnitHandle, contact: bool) -> Option<FixTarget> {
    let u = sim.unit(b).unwrap();
    let mut st = u.ai;
    st.fix = FixState::default();
    if contact {
        st.contacts[0] = Some(Contact {
            handle: Foe::Unit(b),
            first_seen: 0,
            last_seen: 0,
            last_pos: Vec3::new(40.0, 0.0, 0.0),
            seen_now: true,
            heard: false,
            told: false,
        });
    }
    upkeep(sim, b, u, &mut st);
    st.fix.cur.map(|c| c.target)
}

/// An idle engineer near a damaged friendly vehicle takes it and heals it
/// with its cutter.
#[test]
fn an_idle_engineer_repairs_a_nearby_friendly_vehicle() {
    let (mut sim, bots, vh) = world(1, 8.0);
    assert_eq!(pick(&sim, bots[0], false), Some(FixTarget::Vehicle(vh)));
    for _ in 0..20 * TICK_HZ {
        sim.step(&[]);
        if sim.vehicle(vh).unwrap().health.cur >= 1000.0 {
            break;
        }
    }
    let v = sim.vehicle(vh).unwrap();
    assert_eq!(v.health.cur, 1000.0, "healed to full");
    // Whole: the claim ends.
    for _ in 0..TICK_HZ {
        sim.step(&[]);
    }
    assert!(sim.unit(bots[0]).unwrap().ai.fix.cur.is_none());
}

/// Not with an enemy in sight, not past AID_REACH, not an enemy's vehicle,
/// not inside a post its team does not hold.
#[test]
fn an_engineer_does_not_aid_while_fighting_or_far_or_at_a_post() {
    let (sim, bots, vh) = world(1, 8.0);
    assert_eq!(pick(&sim, bots[0], true), None, "an enemy in sight");
    let (sim, bots, _) = world(1, AID_REACH + 2.0);
    assert_eq!(pick(&sim, bots[0], false), None, "too far");
    let (mut sim2, bots2, vh2) = world(1, 8.0);
    sim2.vehicle_mut(vh2).unwrap().team = Some(1);
    assert_eq!(pick(&sim2, bots2[0], false), None, "an enemy vehicle");
    let (mut sim3, bots3, _) = world(1, 8.0);
    let at = sim3.unit(bots3[0]).unwrap().soldier.pos;
    sim3.add_cp(
        crate::cp::CpDef {
            name: "cp".into(),
            pos: at,
            capture: crate::region::Region::sphere(at, 8.0),
            control: None,
            value_bleed: 10,
            value_atk: [10; 2],
            value_def: [10; 2],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        None,
    );
    assert_eq!(pick(&sim3, bots3[0], false), None, "fighting for a post");
    let _ = (sim, vh);
}

/// Two engineers by one damaged vehicle: one claim at a time.
#[test]
fn two_engineers_do_not_both_claim_one_vehicle() {
    let (mut sim, bots, vh) = world(2, 8.0);
    // A held claim is not picked by the other.
    sim.units[bots[0].index as usize].ai.fix.cur = Some(FixClaim {
        target: FixTarget::Vehicle(vh),
        since: 0,
        best: 400.0,
        progress_at: 0,
    });
    assert_eq!(pick(&sim, bots[1], false), None);
    sim.units[bots[0].index as usize].ai.fix.cur = None;
    let mut both = 0;
    for t in 0..10 * TICK_HZ {
        sim.step(&[]);
        let n = bots
            .iter()
            .filter(|&&b| {
                sim.unit(b)
                    .unwrap()
                    .ai
                    .fix
                    .cur
                    .is_some_and(|c| c.target == FixTarget::Vehicle(vh))
            })
            .count();
        // Two that pick on one think give way at the next.
        if t >= TICK_HZ {
            assert!(n <= 1, "tick {t}: {n} claims");
        }
        both += usize::from(n > 1);
    }
    assert!(both <= TICK_HZ as usize);
}

#[test]
fn engineer_walks_to_portable_turret_and_releases_claim_on_repair_or_expiry() {
    let (mut sim, bots, vh) = world(2, 8.0);
    sim.vehicle_mut(vh).unwrap().health.cur = 1000.0;
    let mut item = crate::autoturret::TurretClass::new(
        &crate::remote::tests::Props(&[("MaxHealth", "50"), ("Lifetime", "60")]),
        WeaponClass::default(),
    );
    item.gun.ai_range = [0.0; 3];
    let mut weapon = WeaponClass::default();
    weapon.turret = Some(Box::new(crate::weapon::Dispenser::new(
        &mut WeaponClass::default(),
        &crate::remote::tests::Props(&[]),
        true,
        item,
    )));
    let weapon = sim.add_weapon_class(weapon);
    let class = sim.weapon_classes[weapon]
        .turret
        .as_ref()
        .unwrap()
        .item
        .clone();
    let mut turret = crate::autoturret::Turret::new(
        77,
        bots[0],
        0,
        weapon,
        &class,
        Vec3::Y * crate::autoturret::CENTRE,
        Vec3::ZERO,
        0.0,
    );
    turret.land(Vec3::ZERO, Vec3::Y);
    turret.health = 10.0;
    sim.turrets.push(turret);
    assert_eq!(pick(&sim, bots[0], false), Some(FixTarget::Portable(77)));
    assert_eq!(pick(&sim, bots[0], true), None, "aid yields to combat");
    sim.units[bots[0].index as usize].ai.fix.cur = Some(FixClaim {
        target: FixTarget::Portable(77),
        since: 0,
        best: 10.0,
        progress_at: 0,
    });
    assert_eq!(pick(&sim, bots[1], false), None, "one engineer per turret");
    for _ in 0..20 * TICK_HZ {
        sim.step(&[]);
        if sim.turrets[0].health == 50.0 {
            break;
        }
    }
    assert_eq!(
        sim.turrets[0].health, 50.0,
        "engineer walked over and fired cutter"
    );
    for _ in 0..TICK_HZ {
        sim.step(&[]);
    }
    assert!(sim.unit(bots[0]).unwrap().ai.fix.cur.is_none());
    sim.turrets[0].health = 10.0;
    assert_eq!(pick(&sim, bots[0], false), Some(FixTarget::Portable(77)));
    sim.turrets.clear();
    assert_eq!(
        pick(&sim, bots[0], false),
        None,
        "expired identity is never reused"
    );
}

#[test]
fn serve_finish_replaces_seat_input() {
    let (sim, bots, vh) = world(1, 3.0);
    let me = bots[0];
    let u = sim.unit(me).unwrap();
    let mut st = u.ai;
    st.fix.cur = Some(FixClaim {
        target: FixTarget::Vehicle(vh),
        since: 0,
        best: 400.0,
        progress_at: 0,
    });
    let plan = serve_plan(&sim, u, &st).expect("cutter in reach");
    assert_eq!(plan.button, buttons::FIRE);
    let mut pad = PadState {
        lx: 1,
        ly: 2,
        rx: 3,
        ry: 4,
        buttons: buttons::ACTION,
        mx: 5,
        my: 6,
        fire_lag: 7,
        pick: Some(crate::sim::ScreenPick::Post(0)),
    };
    crate::ai::brain::finish_pad(&sim, me, &st, Some(plan), &mut pad);
    assert_eq!(
        pad,
        PadState {
            rx: plan.rx,
            ry: plan.ry,
            buttons: buttons::FIRE,
            ..PadState::default()
        }
    );
}
