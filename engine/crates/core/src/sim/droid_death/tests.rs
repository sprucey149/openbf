//! Card DROIDDEATH's referees (`analysis/re-droid-death.md`): the death
//! roll's 6 in 10 with no damage threshold, the push roll's mark, the burst
//! (event, no body, the drop), and a clone that never bursts.
use super::*;
use crate::projectile::{Bolt, Hit, Target};
use crate::sim::tests::flat_ground;
use crate::sim::{DropTable, ItemClass};
use crate::soldier::GroundRules;
use crate::unit::{UnitClass, UnitHandle, UnitSpec};
use crate::weapon::ExplosionClass;
use glam::Vec3;

/// The CIS side (team 1) fields one class: BF2's `cis_inf_default` chain
/// (ChunkFrequency 0.6, `cis_inf_droid_exp`: an Effect and nothing else).
const CIS: u8 = 1;

fn droid_exp() -> ExplosionClass {
    let mut e = ExplosionClass::default();
    e.look.effects = vec!["cis_sfx_droid_explosion".into()];
    e
}

fn class(chunk: Option<ChunkDeath>) -> UnitClass {
    UnitClass {
        consts: crate::soldier::SoldierConstants::default(),
        max_health: 300.0,
        health_type: crate::weapon::HealthType::Droid,
        weapon_class: 0,
        total_clips: 6,
        weapon_slot: 0,
        slots: std::sync::Arc::new([]),
        jet: false,
        capture_posts: true,
        drops: DropTable::from_pairs(&[(0, 1.0)]),
        chunk,
        min: 0,
        max: None,
        unlock_points: 0,
        droideka: None,
    }
}

fn droid_class() -> UnitClass {
    class(Some(ChunkDeath {
        frequency: 0.6,
        explosion: Some(droid_exp()),
    }))
}

/// A BF2 sim: team 0 clones (no chunk death), team 1 droids; every death
/// drops the one item class.
fn sim(seed: u32) -> Sim {
    let mut s = Sim::empty(seed);
    s.heightfield = Some(flat_ground());
    s.ground_rules = GroundRules::Bf2;
    s.add_item_class(ItemClass::basic("com_item_powerup_ammo", 0.0, 1));
    s.classes[0] = vec![class(None)];
    s.classes[usize::from(CIS)] = vec![droid_class()];
    s
}

fn spawn(s: &mut Sim, team: u8, pos: Vec3) -> UnitHandle {
    let spec = UnitSpec {
        drops: DropTable::from_pairs(&[(0, 1.0)]),
        ..UnitSpec::trooper(team, pos)
    };
    s.spawn(&spec).unwrap()
}

/// `h` alive again, a fresh life of the same spec (no mark).
fn reset(s: &mut Sim, h: UnitHandle) {
    let i = h.index as usize;
    let spec = s.units[i].spec.clone();
    s.units[i] = spec.build(h.gen, &s.weapon_classes);
}

/// The LCG's next raw step, without moving it.
fn peek(s: &Sim) -> u32 {
    s.lcg.clone().step()
}

fn bursts(s: &Sim) -> usize {
    s.events
        .iter()
        .filter(|e| matches!(e, Event::DroidBurst { .. }))
        .count()
}

/// Kill `h` with `damage` past its health (`overkill`), then run its death
/// block; whether it burst.
fn kill_and_block(s: &mut Sim, h: UnitHandle, overkill: f32) -> bool {
    let i = h.index as usize;
    reset(s, h);
    s.units[i].health.cur = -overkill;
    s.unit_killed(h, None);
    s.events.clear();
    s.death_block(i);
    let burst = bursts(s) == 1;
    assert_eq!(burst, s.units[i].burst);
    burst
}

/// §2: the death block bursts a droid when its draw is <= 0.6 - 600 +- 46
/// of 1,000 (3 sigma) - and the overkill does not move the rate: a kill by
/// 1 hit point and one by 500 (a headshot, a rocket) burst alike.
#[test]
fn a_dead_droid_bursts_six_times_in_ten_whatever_the_overkill() {
    let mut s = sim(7);
    let h = spawn(&mut s, CIS, Vec3::ZERO);
    for overkill in [1.0, 500.0] {
        let n = (0..1000)
            .filter(|_| kill_and_block(&mut s, h, overkill))
            .count();
        assert!((554..=646).contains(&n), "overkill {overkill}: {n} of 1000");
    }
}

/// A clone class has no chunk geometry: it never bursts, and its death
/// draws nothing from the LCG (the port's `[ours]`, §4).
#[test]
fn a_clone_never_bursts_and_draws_nothing() {
    let mut s = sim(7);
    let h = spawn(&mut s, 0, Vec3::ZERO);
    for _ in 0..200 {
        // The drop's own roll is the only draw.
        let mut want = s.lcg.clone();
        want.unit();
        assert!(!kill_and_block(&mut s, h, 500.0));
        assert_eq!(peek(&s), want.step());
        assert_eq!(s.units[h.index as usize].soldier.stance, Stance::Dead);
        assert!(s.units[h.index as usize].body, "a corpse");
    }
}

/// The burst (§2): the event at the body's sphere centre with its team,
/// class and the chunks' scale; the body is out of the world at once; the
/// item drop comes as on any death; and the battle carries on.
#[test]
fn the_burst_leaves_no_body_and_drops_the_item() {
    let mut s = sim(7);
    let h = spawn(&mut s, CIS, Vec3::new(3.0, 0.0, 2.0));
    // A marked droid always bursts, so no seed hunting.
    s.units[h.index as usize].chunk_mark = true;
    s.units[h.index as usize].health.cur = -4.0;
    s.unit_killed(h, None);
    s.events.clear();
    s.step(&[]);
    let u = &s.units[h.index as usize];
    assert!(u.burst && !u.body && !u.alive && !u.drop_due);
    let ev = s.events.iter().find_map(|e| match e {
        Event::DroidBurst {
            unit,
            pos,
            team,
            class,
            scale,
        } => Some((*unit, *pos, *team, *class, *scale)),
        _ => None,
    });
    let (unit, pos, team, class, scale) = ev.expect("a DroidBurst");
    assert_eq!((unit, team, class), (h, CIS, 0));
    assert!((pos - Vec3::new(3.0, 1.0, 2.0)).length() < 1e-4, "{pos:?}");
    assert!((scale - 0.172).abs() < 1e-6, "0.1 + 0.018 x 4: {scale}");
    assert!(
        s.events
            .iter()
            .any(|e| matches!(e, Event::ItemDropped { item: 0, .. })),
        "the drop"
    );
    assert_eq!(s.pickups.len(), 1);
    // Out of the world: a later blast neither gathers nor moves it.
    let at = s.units[h.index as usize].soldier.pos;
    let e = ExplosionClass {
        damage: 320.0,
        damage_outer: 6.5,
        push: 10.0,
        push_outer: 4.5,
        ..Default::default()
    };
    s.explode(&e, at + Vec3::X, None, Some(0));
    assert_eq!(s.units[h.index as usize].soldier.vel, Vec3::ZERO);
    for _ in 0..30 {
        s.step(&[]);
    }
    assert_eq!(s.units[h.index as usize].soldier.pos, at);
}

/// [`chunk_scale`] (§2): 0.1 + 0.018 x overkill, 1.0 from -50.
#[test]
fn the_chunk_scale_follows_the_overkill() {
    assert!((chunk_scale(-4.0) - 0.172).abs() < 1e-6);
    assert!((chunk_scale(-25.0) - 0.55).abs() < 1e-6);
    assert_eq!(chunk_scale(-50.0), 1.0);
    assert_eq!(chunk_scale(-900.0), 1.0);
}

/// A rifle bolt that kills a BF2 droid (the light kill): the kill at the
/// hit, the burst or the corpse at the next tick's death block, and the
/// drop either way - the same seed gives the same answer.
#[test]
fn a_bolt_kill_is_decided_at_the_death_block_deterministically() {
    let run = |seed: u32| {
        let mut s = sim(seed);
        let shooter = spawn(&mut s, 0, Vec3::ZERO);
        let mut out = Vec::new();
        for k in 0..40 {
            let v = spawn(&mut s, CIS, Vec3::new(6.0, 0.0, k as f32));
            s.units[v.index as usize].health.cur = 1.0;
            s.resolve_bolt(&Bolt {
                visual_shot: None,
                pos: Vec3::ZERO,
                vel: Vec3::X * 40.0,
                remaining: 0.0,
                age: 0.0,
                owner: shooter,
                team: 0,
                weapon_class: 0,
                hit: Hit::Target(Target::Unit(v)),
                end: Vec3::ZERO,
                shooter_is_player: false,
                hulls: crate::collision::HullFilter::All,
                hit_mult: 1.0,
                gravity: 0.0,
                charge: 1.0,
                missile: None,
                fat: None,
            });
            assert!(!s.units[v.index as usize].alive);
            assert!(s
                .events
                .iter()
                .any(|e| matches!(e, Event::Kill { victim, .. } if *victim == v)));
            s.events.clear();
            s.step(&[]);
            let u = &s.units[v.index as usize];
            assert!(!u.drop_due, "the drop came with the death block");
            assert_eq!(u.burst, !u.body);
            out.push(u.burst);
        }
        out
    };
    let a = run(3);
    assert_eq!(a, run(3), "deterministic");
    let n = a.iter().filter(|b| **b).count();
    assert!((10..=36).contains(&n), "{n} of 40 burst");
}

/// The push roll (§1): of 1,000 droids in a push blast's reach about 600
/// are marked and not pushed (600 +- 46); the others are pushed, and a
/// dead one among them is left at health -4.0. A clone beside them is
/// always pushed and never marked.
#[test]
fn the_push_rolls_the_mark_before_the_distance() {
    let mut s = sim(11);
    // The port pushes the living on BF2 maps too (KNOCK's deviation).
    let e = ExplosionClass {
        push: 10.0,
        push_outer: 4.5,
        ..Default::default()
    };
    let clone = spawn(&mut s, 0, Vec3::new(-1.5, 0.0, 0.0));
    let h = spawn(&mut s, CIS, Vec3::new(1.5, 0.0, 0.0));
    let (mut marked, mut pushed) = (0, 0);
    for k in 0..1000 {
        reset(&mut s, h);
        reset(&mut s, clone);
        let dead = k % 2 == 1;
        if dead {
            s.units[h.index as usize].health.cur = -100.0;
            s.unit_killed(h, None);
        }
        s.explode(&e, Vec3::new(0.0, 0.12, 0.0), None, Some(0));
        let u = &s.units[h.index as usize];
        // A dead one is kicked either way (the splash's death kick).
        let kick = if dead {
            (u.soldier.pos - Vec3::new(0.0, 0.12, 0.0)).normalize() * crate::soldier::DEATH_KICK
        } else {
            Vec3::ZERO
        };
        let moved = (u.soldier.vel - kick).x > 1.0;
        if u.chunk_mark {
            marked += 1;
            assert!(!moved, "a marked droid is not pushed");
        } else {
            pushed += 1;
            assert!(moved, "an unmarked droid is pushed");
            if dead {
                assert_eq!(u.health.cur, PUSHED_DEAD_HEALTH);
            }
        }
        let cl = &s.units[clone.index as usize];
        assert!(
            !cl.chunk_mark && cl.soldier.vel.x < -1.0,
            "the clone is pushed"
        );
    }
    assert_eq!(marked + pushed, 1000);
    assert!((554..=646).contains(&marked), "{marked} of 1000 marked");
}

/// A thermal detonator that kills a droid: the push roll and the death
/// roll both burst it, so 1 - 0.4 x 0.4 = 84 in 100 burst [D] - against 60
/// for a bolt.
#[test]
fn a_blast_kill_bursts_more_often_than_a_bolt_kill() {
    let mut s = sim(5);
    let e = ExplosionClass {
        damage: 320.0,
        damage_outer: 6.5,
        push: 10.0,
        push_outer: 4.5,
        ..Default::default()
    };
    let h = spawn(&mut s, CIS, Vec3::new(1.5, 0.0, 0.0));
    let mut n = 0;
    for _ in 0..1000 {
        reset(&mut s, h);
        s.units[h.index as usize].soldier.pos = Vec3::new(1.5, 0.0, 0.0);
        s.units[h.index as usize].health.cur = 1.0;
        s.explode(&e, Vec3::new(0.0, 0.12, 0.0), None, Some(0));
        assert!(!s.units[h.index as usize].alive);
        let mut t = 0;
        while s.units[h.index as usize].body
            && s.units[h.index as usize].soldier.stance != Stance::Dead
        {
            s.step(&[]);
            t += 1;
            assert!(t < 300);
        }
        n += usize::from(s.units[h.index as usize].burst);
    }
    // 840 +- 3 sigma (11.6 each).
    assert!((805..=875).contains(&n), "{n} of 1000");
}

/// A native (team 2, a Tusken beside a Clone Wars battle) carries a class
/// index into its own table, not the CIS army's: even a marked native whose
/// index lines up with a CIS droid class falls and leaves a body, while the
/// marked droid beside it still bursts. A native class that does have a
/// chunk death bursts with its own explosion.
#[test]
fn a_native_beside_the_cis_falls_and_leaves_a_body() {
    let mut s = sim(7);
    let member = UnitSpec {
        controller: crate::unit::Controller::Ai,
        drops: DropTable::from_pairs(&[(0, 1.0)]),
        ..UnitSpec::trooper(2, Vec3::ZERO)
    };
    s.add_native_team(2, vec![member]);
    let tusken = s
        .spawn_native_member(2, (Vec3::new(4.0, 0.0, 0.0), 0.0), None)
        .unwrap();
    let droid = spawn(&mut s, CIS, Vec3::new(-4.0, 0.0, 0.0));
    assert_eq!(s.units[tusken.index as usize].spec.class, 0);
    assert!(s.burst_explosion(2, 0).is_none());
    for _ in 0..50 {
        assert!(!marked_death(&mut s, tusken), "a Tusken never bursts");
        let t = &s.units[tusken.index as usize];
        assert!(t.body && t.soldier.stance == Stance::Dead, "a corpse");
        assert!(marked_death(&mut s, droid), "a marked droid bursts");
    }
    // A native class with its own chunk geometry keeps its own burst.
    s.set_native_chunk_deaths(
        2,
        vec![Some(ChunkDeath {
            frequency: 0.0,
            explosion: Some(droid_exp()),
        })],
    );
    assert!(s.burst_explosion(2, 0).is_some());
    assert!(marked_death(&mut s, tusken));
}

/// A fresh life of `h` marked by a push, killed by 500, its death block
/// run; whether it burst.
fn marked_death(s: &mut Sim, h: UnitHandle) -> bool {
    let i = h.index as usize;
    reset(s, h);
    s.units[i].chunk_mark = true;
    s.units[i].health.cur = -500.0;
    s.unit_killed(h, None);
    s.events.clear();
    s.death_block(i);
    let burst = bursts(s) == 1;
    assert_eq!(burst, s.units[i].burst);
    burst
}
