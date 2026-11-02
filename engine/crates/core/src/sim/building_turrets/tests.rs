use super::*;
use crate::collision::{Body, Layers, Shape};
use crate::sim::{buttons, Event, PadState};
use crate::unit::{UnitHandle, UnitSpec};
use crate::vehicle::{SeatKind, SeatSetup, VehicleKind};
use crate::weapon::{Repair, WeaponClass};

const INSTANCE: u32 = 7;

fn cutter(bf2: bool) -> WeaponClass {
    let mut health = [0.0; 8];
    health[3] = if bf2 { 200.0 } else { 60.0 };
    health[4] = if bf2 { 100.0 } else { 60.0 };
    health[5] = if bf2 { 50.0 } else { 20.0 };
    health[6] = if bf2 { 50.0 } else { 30.0 };
    WeaponClass {
        name: "fusioncutter".into(),
        repair: Some(Repair { health }),
        target_team: 6,
        lock_on_range: if bf2 { 4.0 } else { 3.0 },
        lock_on_angle: if bf2 { 90.0 } else { 30.0 } * crate::weapon::DEG,
        shot_delay: [0.5; 2],
        heat_per_shot: 0.0,
        ..WeaponClass::default()
    }
}

/// A 300 HP turret (BF1's gun turret) on instance 7: a tall box standing,
/// a flat slab when fallen; an engineer 2 m in front of it, facing it.
fn world(bf2: bool, fall: TurretFall) -> (Sim, UnitHandle) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(super::super::tests::flat_ground());
    s.hover_params = vec![None];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 15.0,
        ai_size: 1,
        guns: vec![None],
    })];
    let mut b = Body::new(
        Mat4::from_translation(Vec3::new(0.0, 1.5, 0.0)),
        Shape::Box {
            half: Vec3::new(0.6, 1.5, 0.6),
        },
    );
    b.instance = Some(INSTANCE);
    s.collision.push_body(b);
    s.collision.rebuild_broadphase();
    s.weapon_classes.push(cutter(bf2));
    let slab = HullPart::new(
        Mat4::from_translation(Vec3::new(0.0, 0.1, 0.0)),
        Shape::Box {
            half: Vec3::new(1.0, 0.1, 1.0),
        },
        Layers::ALL,
    );
    let parts = if fall == TurretFall::Hidden {
        Vec::new()
    } else {
        vec![slab]
    };
    s.add_building_turret(
        VehicleSpec {
            kind: VehicleKind::Turret,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 300.0,
            health_type: HealthType::Building,
            seats: vec![SeatKind::Gunner(0)],
        },
        Quat::IDENTITY,
        Some((Vec3::new(0.0, 1.0, 0.0), 1.0)),
        INSTANCE,
        (fall, parts),
    );
    let mut spec = UnitSpec::player(0, Vec3::new(0.0, 0.0, 2.0));
    spec.weapon_class = s.weapon_classes.len() - 1;
    let me = s.spawn(&spec).unwrap();
    (s, me)
}

fn fire() -> PadState {
    PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }
}

/// The instance's live bodies: (count, top of the highest).
fn bodies(s: &Sim) -> (usize, f32) {
    let live: Vec<_> = s
        .collision
        .bodies()
        .iter()
        .filter(|b| b.instance == Some(INSTANCE) && b.layers != Layers::NONE)
        .collect();
    let top = live.iter().map(|b| b.aabb.max.y).fold(0.0, f32::max);
    (live.len(), top)
}

fn turret(s: &Sim) -> Option<VehicleHandle> {
    s.alive_vehicles()
        .find(|(_, v)| v.kind == VehicleKind::Turret)
        .map(|(h, _)| h)
}

/// At 0 the turret falls to its look's bodies and is no vehicle; one shot
/// short of full it is still down; the last shot builds it, standing bodies
/// and all, neutral and full. Its health while down is the hash's.
#[test]
fn a_fallen_turret_takes_its_fallen_bodies_until_it_is_built_at_full_health() {
    for fall in [TurretFall::Pad, TurretFall::Wreck, TurretFall::Hidden] {
        let (mut s, _) = world(false, fall);
        assert_eq!(bodies(&s), (1, 3.0), "standing");
        let h = turret(&s).unwrap();
        s.destroy_vehicle(h, None);
        assert!(turret(&s).is_none());
        assert!(s.building_down(INSTANCE));
        assert_eq!(s.collision.turret(INSTANCE), None);
        let fallen = bodies(&s);
        match fall {
            TurretFall::Hidden => assert_eq!(fallen.0, 0, "{fall:?}"),
            _ => assert_eq!(fallen, (1, 0.2), "{fall:?}"),
        }
        let a = s.hash();
        assert_eq!(s.repair_building(0, 299.0), 299.0);
        assert_ne!(a, s.hash(), "the down health is hashed");
        assert!(
            s.building_down(INSTANCE),
            "{fall:?}: 299 of 300 is still down"
        );
        assert_eq!(bodies(&s), fallen);
        assert_eq!(s.repair_building(0, 50.0), 1.0);
        assert!(!s.building_down(INSTANCE));
        let h = turret(&s).unwrap();
        let v = s.vehicle(h).unwrap();
        assert_eq!((v.health.cur, v.team), (300.0, None));
        assert_eq!(s.collision.turret(INSTANCE), Some(h));
        assert_eq!(bodies(&s), (1, 3.0), "{fall:?}: standing again");
    }
}

/// Only a cutter builds a fallen turret (`[ours]`, the maintainer 2026-10-28; the
/// PS2 regenerates MaxHealth / `RespawnTime` a second): with nobody near it
/// is still down, at health 0, 300 s after it fell.
#[test]
fn a_fallen_turret_never_comes_back_by_itself() {
    let (mut s, me) = world(false, TurretFall::Pad);
    s.unit_mut(me).unwrap().soldier.pos.z = 30.0;
    let h = turret(&s).unwrap();
    s.destroy_vehicle(h, None);
    for _ in 0..300 * 30 {
        s.step(&[PadState::default()]);
    }
    assert!(s.building_down(INSTANCE));
    assert_eq!(s.building_turrets[0].down, Some(0.0));
    assert!(turret(&s).is_none());
}

/// The cutter: a damaged standing turret takes BuildingHealth a shot (BF1
/// 60, BF2 100), a fallen pad BuildingBuild (30 / 50) and a fallen wreck
/// BuildingRebuild (20 / 50), one shot per 0.5 s (15 ticks); held to the
/// end it builds the fallen one.
#[test]
fn the_cutter_heals_a_turret_and_builds_a_fallen_one_at_the_disc_rates() {
    for bf2 in [false, true] {
        for fall in [TurretFall::Pad, TurretFall::Wreck] {
            let (mut s, _) = world(bf2, fall);
            let h = turret(&s).unwrap();
            s.vehicle_mut(h).unwrap().health.cur = 10.0;
            s.step(&[fire()]);
            let per = if bf2 { 100.0 } else { 60.0 };
            assert_eq!(s.vehicle(h).unwrap().health.cur, 10.0 + per);
            assert!(s.events.iter().any(|e| matches!(e,
                Event::Repair { target: crate::projectile::Target::Vehicle(v), .. } if *v == h)));
            for _ in 0..20 {
                s.step(&[PadState::default()]);
            }
            s.destroy_vehicle(h, None);
            let per = match (bf2, fall) {
                (false, TurretFall::Pad) => 30.0,
                (false, _) => 20.0,
                (true, _) => 50.0,
            };
            let mut shots = Vec::new();
            let mut before = 0.0;
            for tick in 0..2000 {
                s.step(&[fire()]);
                let Some(now) = s.building_turrets[0].down else {
                    shots.push(tick);
                    break;
                };
                if now != before {
                    assert_eq!(now - before, per, "bf2 {bf2} {fall:?}");
                    before = now;
                    shots.push(tick);
                }
            }
            assert!(!s.building_down(INSTANCE), "bf2 {bf2} {fall:?}: built");
            assert_eq!(shots.len(), (300.0_f32 / per).ceil() as usize);
            assert!(shots.windows(2).all(|w| w[1] - w[0] == 15), "{shots:?}");
            assert_eq!(s.vehicle(turret(&s).unwrap()).unwrap().health.cur, 300.0);
        }
    }
}

/// Card TCOL: a prim on the aimer node turns with the gun. The aimer node
/// bound 2 m up (its parent the turret node, 0.3 m up), a 1 m gun box 1 m
/// ahead of it: with the look and aim turned 90 deg (+z to +x) and pitched
/// up 30 deg, the box stands 1 m along the aim from the aimer's origin.
/// Down, it no longer moves.
#[test]
fn a_prim_on_the_aimer_node_turns_with_the_gun() {
    let (mut s, _) = world(true, TurretFall::Wreck);
    let mut gun = Body::new(
        Mat4::from_translation(Vec3::new(0.0, 2.0, 1.0)),
        Shape::Box {
            half: Vec3::splat(0.5),
        },
    );
    gun.instance = Some(INSTANCE);
    s.collision.push_body(gun);
    s.collision.rebuild_broadphase();
    let body = s.collision.bodies().len() - 1;
    let at = |y: f32| Mat4::from_translation(Vec3::new(0.0, y, 0.0));
    s.set_turret_posed(
        INSTANCE,
        PosedParts {
            rig: Rig {
                binds: vec![Mat4::IDENTITY, at(0.3), at(2.0)],
                parents: vec![None, Some(0), Some(1)],
                turret: Some(1),
                pitch_factor: 0.0,
                aimer: Some(2),
                barrels: Vec::new(),
            },
            seat: 0,
            bodies: vec![PosedBody {
                body,
                node: 2,
                rest: Mat4::from_translation(Vec3::new(0.0, 2.0, 1.0)),
            }],
            at: None,
        },
    );
    let h = s.building_turrets[0].handle;
    let q = std::f32::consts::FRAC_PI_2;
    let p = 30.0f32.to_radians();
    s.vehicle_mut(h).unwrap().guns = vec![Some(crate::vehicle::gun::GunState {
        look: (q, 0.0),
        aim: (q, p),
        rate: (0.0, 0.0),
        rel: (0.0, 0.0),
        weapons: Vec::new(),
        zoom: None,
    })];
    s.pose_building_turrets();
    let c = s.collision.bodies()[body].to_world.w_axis.truncate();
    let want = Vec3::new(0.0, 2.0, 0.0) + crate::vehicle::gun::dir(q, p);
    assert!(c.abs_diff_eq(want, 1e-4), "{c} vs {want}");
    // A ray down the old line (along +z at 2 m) now misses; one at the
    // turned box meets it.
    let hit = |s: &Sim, from: Vec3, to: Vec3| {
        s.collision
            .ray_with(
                from,
                (to - from).normalize(),
                20.0,
                Layers::ORDNANCE,
                crate::collision::HullFilter::All,
            )
            .is_some_and(|r| r.instance == Some(INSTANCE) && from.distance(to) > r.t)
    };
    assert!(hit(&s, Vec3::new(10.0, want.y, 0.0), want));
    s.building_turrets[0].down = Some(0.0);
    s.vehicle_mut(h).unwrap().guns[0].as_mut().unwrap().aim = (0.0, 0.0);
    s.pose_building_turrets();
    assert!(s.collision.bodies()[body]
        .to_world
        .w_axis
        .truncate()
        .abs_diff_eq(want, 1e-4));
}

#[test]
fn initial_unbuilt_state_has_no_death_and_keeps_handles_and_collision_separate() {
    let (mut sim, _) = world(true, TurretFall::Pad);
    let old = sim.building_turrets[0].handle;
    let parts = sim.building_turrets[0].standing_parts.len();
    sim.events.clear();
    sim.start_building_unbuilt(INSTANCE);
    assert!(
        sim.events.is_empty(),
        "construction is not a death/explosion"
    );
    assert_eq!(sim.building_turrets[0].down, Some(0.0));
    assert!(sim.vehicle(old).is_none());
    assert_eq!(sim.building_turrets[0].repair_kind(), HealthType::Building3);
    let other = sim.spawn_vehicle(&sim.building_turrets[0].spec.clone());
    assert_eq!(other.index, old.index, "exercise actual pool-slot reuse");
    assert_ne!(other.gen, old.gen);
    assert!(
        sim.vehicle(old).is_none(),
        "down handle cannot alias the new occupant"
    );
    assert_eq!(sim.repair_building(0, 299.0), 299.0);
    assert_eq!(sim.building_turrets[0].down, Some(299.0));
    sim.repair_building(0, 1.0);
    let built = sim.building_turrets[0].handle;
    assert_ne!(built, other);
    assert!(sim.vehicle(built).unwrap().alive);
    assert!(sim.vehicle(other).unwrap().alive);
    assert_eq!(sim.building_turrets[0].standing_parts.len(), parts);
    assert_eq!(sim.collision.turret(INSTANCE), Some(built));
}
