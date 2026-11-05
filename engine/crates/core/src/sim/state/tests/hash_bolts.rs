//! HASH: independently perturb every physics bolt scalar and hit payload.
//! Presentation provenance is deliberately excluded, with a null control below.
use super::*;
use crate::projectile::{Hit, Target};
use crate::vehicle::VehicleHandle;

fn bolt() -> Bolt {
    Bolt {
        visual_shot: None,
        pos: Vec3::new(1.0, 2.0, 3.0),
        vel: Vec3::new(4.0, 5.0, 6.0),
        remaining: 0.5,
        age: 0.25,
        owner: UnitHandle { index: 7, gen: 8 },
        team: 0,
        weapon_class: 0,
        hit: Hit::None,
        end: Vec3::new(9.0, 10.0, 11.0),
        shooter_is_player: false,
        hulls: crate::collision::HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    }
}

#[test]
fn every_bolt_field_and_hit_payload_changes_sim_hash() {
    let mut s = Sim::empty(42);
    let empty = s.hash();
    let original = bolt();
    s.bolts.push(original.clone());
    let base = s.hash();
    assert_ne!(empty, base);
    s.bolts[0].visual_shot = Some(crate::projectile::VisualShot { tick: 12, event: 3 });
    assert_eq!(
        s.hash(),
        base,
        "presentation provenance is not physics state"
    );
    s.bolts[0] = original.clone();
    macro_rules! changes {
        ($($field:ident $(.$axis:ident)? = $value:expr),+ $(,)?) => {
            $(
                s.bolts[0] = original.clone();
                s.bolts[0].$field $(.$axis)? = $value;
                assert_ne!(s.hash(), base, stringify!($field $(.$axis)?));
            )+
        };
    }
    changes! {
        pos.x = 1.25, pos.y = 2.25, pos.z = 3.25,
        vel.x = 4.25, vel.y = 5.25, vel.z = 6.25,
        remaining = 0.75, age = 0.5,
        end.x = 9.25, end.y = 10.25, end.z = 11.25,
        owner.index = 17, owner.gen = 18,
        team = 1, weapon_class = 1, shooter_is_player = true,
    }
    let mut hashes = vec![base];
    for hit in [
        Hit::Terrain,
        Hit::World,
        Hit::Water,
        Hit::Target(Target::Unit(UnitHandle { index: 7, gen: 8 })),
        Hit::Target(Target::Unit(UnitHandle { index: 17, gen: 8 })),
        Hit::Target(Target::Unit(UnitHandle { index: 7, gen: 18 })),
        Hit::Target(Target::Vehicle(VehicleHandle { index: 7, gen: 8 })),
        Hit::Target(Target::Vehicle(VehicleHandle { index: 17, gen: 8 })),
        Hit::Target(Target::Vehicle(VehicleHandle { index: 7, gen: 18 })),
        Hit::Target(Target::Station(7)),
        Hit::Target(Target::Station(17)),
    ] {
        s.bolts[0] = original.clone();
        s.bolts[0].hit = hit;
        let hash = s.hash();
        assert!(!hashes.contains(&hash), "hit alias: {hit:?}");
        hashes.push(hash);
    }
    s.bolts[0] = original.clone();
    assert_eq!(base, s.hash(), "null control");
    s.bolts.push(original);
    assert_ne!(base, s.hash(), "bolt count");
    s.bolts.clear();
    assert_eq!(empty, s.hash(), "empty control");
}

#[test]
fn predictive_hit_changes_damage_and_hash_before_impact() {
    let (mut a, owner, victim) = crate::sim::tests::duel();
    let (mut b, _, _) = crate::sim::tests::duel();
    let mut shot = bolt();
    shot.owner = owner;
    shot.remaining = crate::TICK_DT * 0.5;
    shot.hit = Hit::Target(Target::Unit(victim));
    a.bolts.push(shot.clone());
    shot.hit = Hit::None;
    b.bolts.push(shot);
    assert_ne!(a.hash(), b.hash());
    a.step(&[PadState::default()]);
    b.step(&[PadState::default()]);
    assert!(a.unit(victim).unwrap().health.cur < b.unit(victim).unwrap().health.cur);
}

#[test]
fn live_bolt_replays_match_every_tick() {
    fn run() -> Vec<u64> {
        let (mut s, owner, victim) = crate::sim::tests::duel();
        let mut shot = bolt();
        shot.owner = owner;
        shot.hit = Hit::Target(Target::Unit(victim));
        s.bolts.push(shot);
        let mut hashes = vec![s.hash()];
        for _ in 0..600 {
            s.step(&[PadState::default()]);
            hashes.push(s.hash());
        }
        hashes
    }
    assert_eq!(run(), run(), "601 state hashes including flight and impact");
}

#[test]
fn assist_duration_and_slot_are_hashed_with_an_inactive_null_control() {
    let mut s = Sim::empty(42);
    s.assist = vec![AssistState::default(); 2];
    let cold = s.hash();
    s.assist[0].magnet_timer = 0.25;
    let warm = s.hash();
    assert_ne!(cold, warm);
    s.assist[0].magnet_timer = 0.5;
    assert_ne!(warm, s.hash(), "remaining duration");
    s.assist[0].magnet_timer = 0.0;
    s.assist[1].magnet_timer = 0.25;
    assert_ne!(warm, s.hash(), "controller slot");
    s.assist[1].magnet_timer = 0.0;
    assert_eq!(cold, s.hash(), "inactive null control");
}

/// Desired heading is independent of launch/flight direction and changes the
/// next no-target update. Hash it even when all other bolt fields match.
#[test]
fn missile_desired_heading_is_hashed_and_changes_continuation() {
    let mut s = Sim::empty(42);
    let c = crate::missile::MissileClass::default();
    let mut original = bolt();
    original.missile = Some(crate::missile::Missile::launch(
        &c,
        Vec3::Z,
        40.0,
        None,
        &mut s.lcg,
    ));
    s.bolts.push(original.clone());
    let base = s.hash();
    for desired in [
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(0.0, 1.0, 1.0),
        Vec3::new(0.0, 0.0, 0.5),
        Vec3::new(-0.0, 0.0, 1.0),
    ] {
        s.bolts[0].missile.as_mut().unwrap().desired = desired;
        assert_ne!(s.hash(), base, "desired {desired:?}, including signed zero");
    }
    s.bolts[0] = original.clone();
    assert_eq!(s.hash(), base, "restored heading is a null control");
    s.bolts[0].missile.as_mut().unwrap().desired = Vec3::X;
    let mut rng = s.lcg.clone();
    s.bolts[0].guide(&c, None, crate::TICK_DT, &mut s.lcg);
    original.guide(&c, None, crate::TICK_DT, &mut rng);
    assert_ne!(
        s.bolts[0].vel, original.vel,
        "different future after target loss"
    );
    assert_eq!(s.bolts[0].missile.unwrap().launch, Vec3::Z);
}
