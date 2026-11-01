use super::*;
use crate::collision::CollisionWorld;
use crate::sim::tests::{ai, fire, flat_ground};
use crate::sim::{Cheats, InputLog};
use crate::soldier::GroundRules;
use crate::unit::UnitHandle;
use crate::unit::UnitSpec;
use crate::weapon::WeaponClass;

mod jet;
mod ramps;
mod scratch;

/// The combat calls `unit` asked over `ticks` steps: (tick, call).
fn calls_of(s: &mut Sim, ticks: u32) -> Vec<(u64, UnitHandle, Call)> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        let tick = s.tick;
        s.step(&[PadState::default()]);
        for e in &s.events {
            if let Event::Call { unit, call } = *e {
                out.push((tick, unit, call));
            }
        }
    }
    out
}

/// Card UCALL, BF1's "grenade!" (re-unit-chatter.md 2.3): a live
/// grenade warns the units within 7.5 m after its reaction delay and
/// then once a second, and only the FIRST AI soldier among them says
/// the line - never the human beside it, never a bot outside the
/// radius. A BF2-rule sim asks the same (its Grenade line, card UVO2).
#[test]
fn a_live_grenade_makes_one_flee_call_a_second_from_the_first_bot() {
    let run = |rules: GroundRules| {
        let mut s = Sim::empty(9);
        s.heightfield = Some(flat_ground());
        s.ground_rules = rules;
        let human = s
            .spawn(&UnitSpec::player(0, Vec3::new(0.5, 0.0, 0.0)))
            .unwrap();
        let first = s.spawn(&ai(0, Vec3::new(3.0, 0.0, 0.0), 0.0)).unwrap();
        let second = s.spawn(&ai(0, Vec3::new(2.0, 0.0, 1.0), 0.0)).unwrap();
        let outside = s.spawn(&ai(0, Vec3::new(9.0, 0.0, 0.0), 0.0)).unwrap();
        s.add_hazard(Vec3::ZERO, 150);
        let calls = calls_of(&mut s, 150);
        let gone = s.unit(first).map(|u| u.soldier.pos.length());
        (calls, [human, first, second, outside], gone)
    };
    let (calls, [human, first, second, outside], gone) = run(GroundRules::Bf1);
    let flee: Vec<_> = calls.iter().filter(|c| c.2 == Call::Flee).collect();
    assert!(flee.len() >= 3, "a call a second for 5 s: {flee:?}");
    // The lowest slot speaks while it is inside; once it has run clear
    // (it does: card HOLD) the next bot inside is the first.
    assert_eq!(flee[0].1, first, "{flee:?}");
    assert!(
        flee.iter().all(|c| c.1 == first || c.1 == second),
        "{flee:?}"
    );
    assert!(flee.iter().all(|c| c.1 != human), "a human never: {flee:?}");
    assert!(
        flee.iter().all(|c| c.1 != outside),
        "outside the radius: {flee:?}"
    );
    if flee.iter().any(|c| c.1 == second) {
        assert!(
            gone.is_some_and(|d| d > crate::sim::HAZARD_RADIUS),
            "{gone:?}"
        );
    }
    let t0 = flee[0].0;
    assert!((18..=39).contains(&t0), "after 0.6-1.3 s: tick {t0}");
    for w in flee.windows(2) {
        assert_eq!(
            w[1].0 - w[0].0,
            u64::from(TICK_HZ),
            "once a second: {flee:?}"
        );
    }
    // BF2: the same rule asks the Grenade line (card UVO2).
    let (calls, [_, first, ..], _) = run(GroundRules::Bf2);
    let flee: Vec<_> = calls.iter().filter(|c| c.2 == Call::Flee).collect();
    assert!(!flee.is_empty() && flee[0].1 == first, "BF2: {calls:?}");
}

/// Card UCALL, BF1's "there he is" (re-unit-chatter.md 2.2): a bot whose
/// tracker takes the enemy in front of it asks `Acquired` at that think,
/// and not again while it keeps the target. A BF2-rule sim asks the take
/// alone (card UVO2).
#[test]
fn a_bot_that_takes_a_target_asks_its_acquired_call() {
    let run = |rules: GroundRules| {
        let mut s = Sim::empty(3);
        s.heightfield = Some(flat_ground());
        s.ground_rules = rules;
        let bot = s.spawn(&ai(0, Vec3::ZERO, 0.0)).unwrap();
        // Straight ahead (yaw 0 faces -Z), in plain sight.
        let enemy = s
            .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, -15.0)))
            .unwrap();
        let calls = calls_of(&mut s, 60);
        let target = s.unit(bot).and_then(|u| u.ai.target);
        (
            calls.into_iter().filter(|c| c.1 == bot).collect::<Vec<_>>(),
            target,
            enemy,
        )
    };
    let (calls, target, enemy) = run(GroundRules::Bf1);
    assert_eq!(target, Some(enemy.into()), "it took the enemy");
    let acquired: Vec<_> = calls.iter().filter(|c| c.2 == Call::Acquired).collect();
    assert_eq!(acquired.len(), 1, "one take, one call: {calls:?}");
    // BF2: the take alone (its AcquiredTarget line, card UVO2); no
    // other call of BF1's.
    let (calls, ..) = run(GroundRules::Bf2);
    assert!(
        calls.len() == 1 && calls[0].2 == Call::Acquired,
        "BF2: {calls:?}"
    );
}

/// Card CLSE2 (`analysis/re-soldier-ai.md` 18.1): a BF2 bot's aim-error
/// block thinks every 5th tick, phased by its index, and a human's every
/// tick - both pay the same decay a second.
#[test]
fn a_bots_aim_block_thinks_every_fifth_tick_and_a_humans_every_tick() {
    use crate::unit::AimBlock;
    let mut s = Sim::empty(5);
    s.heightfield = Some(flat_ground());
    let p = s
        .spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, 0.0)))
        .unwrap();
    let b = s.spawn(&ai(0, Vec3::new(4.0, 0.0, 0.0), 0.0)).unwrap();
    let c = s.spawn(&ai(0, Vec3::new(8.0, 0.0, 0.0), 0.0)).unwrap();
    let mut thinks = [0u32; 3];
    let mut ticks_of = [Vec::new(), Vec::new(), Vec::new()];
    for u in [p, b, c] {
        s.units[u.index as usize].aim.err = 2.0;
    }
    for _ in 0..30 {
        let before: Vec<AimBlock> = [p, b, c].iter().map(|h| s.unit(*h).unwrap().aim).collect();
        let tick = s.tick;
        s.step(&[PadState::default()]);
        for (k, h) in [p, b, c].iter().enumerate() {
            if s.unit(*h).unwrap().aim.dir != before[k].dir {
                thinks[k] += 1;
                ticks_of[k].push(tick);
            }
        }
    }
    assert_eq!(thinks, [30, 6, 6], "30 ticks: the human 30 thinks, a bot 6");
    assert!(ticks_of[1]
        .iter()
        .all(|t| (t + u64::from(b.index)) % 5 == 0));
    assert_ne!(ticks_of[1][0], ticks_of[2][0], "phased by the unit's index");
    let decay = s.difficulty.skill().err_decay;
    for u in [p, b, c] {
        let err = s.unit(u).unwrap().aim.err;
        assert!(
            (err - (2.0 - decay)).abs() < 1e-3,
            "a second's decay: {err}"
        );
    }
}

/// Flat ground at y 0 with ONE box ahead whose top is `top` metres up:
/// the kerb / wall the A.4 step tests walk at. It spans z -5..-1, so a
/// unit at the origin walking -Z reaches it after about a metre.
fn step_sim(top: f32) -> Sim {
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    s.collision = CollisionWorld::new(
        vec![crate::collision::Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, top - 2.0, -3.0)),
            crate::collision::Shape::Box {
                half: Vec3::new(10.0, 2.0, 2.0),
            },
        )],
        Vec::new(),
    );
    s
}

#[test]
fn overlapping_units_are_pushed_apart_deterministically() {
    let mut s = Sim::empty(1);
    s.heightfield = Some(flat_ground());
    let a = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let b = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.3, 0.0, 0.0)))
        .unwrap();
    let far = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(5.0, 0.0, 0.0)))
        .unwrap();
    s.separate_units();
    let (pa, pb) = (
        s.units[a.index as usize].soldier.pos,
        s.units[b.index as usize].soldier.pos,
    );
    let min_d = 2.0 * crate::unit::SOLDIER_RADIUS;
    assert!((pb.x - pa.x - min_d).abs() < 1e-5, "{pa} {pb}");
    assert!(((pa.x + pb.x) * 0.5 - 0.15).abs() < 1e-5, "midpoint kept");
    assert_eq!(
        s.units[far.index as usize].soldier.pos,
        Vec3::new(5.0, 0.0, 0.0)
    );
    // Coincident pair: separates along +X, still deterministic.
    let mut s2 = Sim::empty(1);
    s2.heightfield = Some(flat_ground());
    let c = s2.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let d = s2.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
    s2.separate_units();
    assert!(s2.units[d.index as usize].soldier.pos.x > s2.units[c.index as usize].soldier.pos.x);
}

/// A step LOWER than `STEP_HEIGHT` is climbed, not walled off (A.4): the
/// wall pass asks the stepped capsule, so the riser never touches it, the
/// unit walks its feet in and the ground query stands it on top. This is
/// the shape BF2 POL1's ramp pair was stuck on — `steer fwd`, `push
/// 0.301`, 33,840 ticks, one 0.3 m push in and one 0.3 m push back out
/// every tick.
#[test]
fn a_kerb_below_step_height_is_climbed_not_a_wall() {
    let mut s = step_sim(0.4);
    let h = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let fwd = PadState {
        ly: 32767,
        ..PadState::default()
    };
    let mut max_push = 0.0f32;
    let mut stood_on_it = false;
    for _ in 0..60 {
        s.step(&[fwd]);
        let u = s.unit(h).unwrap();
        max_push = max_push.max(u.wall_push.length());
        // Over the box (z -5..-1) and standing on its top.
        if u.soldier.pos.z < -1.0 && u.soldier.pos.z > -5.0 {
            stood_on_it |= (u.soldier.pos.y - 0.4).abs() < 1e-3;
        }
    }
    assert!(stood_on_it, "never stood on the 0.4 m step");
    assert_eq!(max_push, 0.0, "and was never pushed out of it");
    let u = s.unit(h).unwrap();
    assert!(
        u.soldier.pos.z < -5.0,
        "walked clean over it: {:?}",
        u.soldier.pos
    );
}

/// ...and a step TALLER than `STEP_HEIGHT` is still a wall: the stepped
/// capsule hits it, the pass pushes the unit out and records the push.
#[test]
fn a_wall_above_step_height_still_pushes() {
    let mut s = step_sim(1.0);
    let h = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    let fwd = PadState {
        ly: 32767,
        ..PadState::default()
    };
    let mut max_push = 0.0f32;
    for _ in 0..60 {
        s.step(&[fwd]);
        max_push = max_push.max(s.unit(h).unwrap().wall_push.length());
    }
    let u = s.unit(h).unwrap();
    assert!(max_push > 0.0, "pushed out of the 1.0 m wall");
    assert!(
        u.soldier.pos.y.abs() < 1e-3,
        "still on the floor, not on top: {:?}",
        u.soldier.pos
    );
    assert!(
        u.soldier.pos.z > -1.6,
        "stopped in front of it: {:?}",
        u.soldier.pos
    );
}

/// Drive a fresh player at `step_sim(top)` from (0, 0, `z0`), facing −Z:
/// stick forward every tick (+ SPRINT when `sprint`), JUMP pressed at tick
/// `jump` when given. Returns every tick's (feet, velocity).
fn drive(top: f32, z0: f32, sprint: bool, jump: Option<u64>, ticks: u64) -> Vec<(Vec3, Vec3)> {
    drive_by(GroundRules::Bf2, top, z0, sprint, jump, ticks)
}

fn drive_by(
    rules: GroundRules,
    top: f32,
    z0: f32,
    sprint: bool,
    jump: Option<u64>,
    ticks: u64,
) -> Vec<(Vec3, Vec3)> {
    let mut s = step_sim(top);
    s.ground_rules = rules;
    let h = s
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(0.0, 0.0, z0)))
        .unwrap();
    let mut out = Vec::new();
    for t in 0..ticks {
        let mut b = if sprint { buttons::SPRINT } else { 0 };
        if jump == Some(t) {
            b |= buttons::JUMP;
        }
        s.step(&[PadState {
            ly: 32767,
            buttons: b,
            ..PadState::default()
        }]);
        let u = s.unit(h).unwrap();
        out.push((u.soldier.pos, u.soldier.vel));
    }
    out
}

/// Card JWAL (`analysis/re-soldier-walls.md` §3.2): a box 0.75 m tall is
/// above the step, so a walk and a sprint stop in front of it. The bare
/// wall pass let both walk INTO it: the stepped capsule's rim contact
/// tilted past the floor limit and was skipped.
#[test]
fn a_walk_or_a_sprint_stops_at_a_ledge_above_the_step() {
    for sprint in [false, true] {
        let run = drive(0.75, 0.0, sprint, None, 60);
        let (end, _) = run[run.len() - 1];
        assert!(
            end.z > -1.0 && end.y.abs() < 1e-3,
            "sprint {sprint}: stopped in front, on the floor: {end:?}"
        );
    }
}

/// ...and a ledge taller than a jump can reach (2.25 m: the apex, 1.86 m
/// in BF2 and 1.91 m in BF1, + the air's step) stops every jump, in both
/// games, from every take-off point: the feet never get past the face
/// (the bare pass let a jump in, §2.2; the ground's whole step in the air
/// let the PS2's apex in, card JARC).
#[test]
fn no_jump_passes_a_ledge_taller_than_its_reach() {
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        for sprint in [false, true] {
            for jump in 0..40 {
                let run = drive_by(rules, 2.25, 3.0, sprint, Some(jump), 90);
                for (t, (p, _)) in run.iter().enumerate() {
                    assert!(
                        p.z > -1.0,
                        "{rules:?} sprint {sprint} jump {jump}: tick {t} past the face at {p:?}"
                    );
                }
            }
        }
    }
}

/// A jump pressed against a wall goes straight up: the wall took the run's
/// into-wall velocity off (the PS2's own response, re-soldier-walls §5.1),
/// and it takes the air's steering off again every tick he is under its top.
#[test]
fn a_jump_pressed_against_a_wall_goes_straight_up() {
    let run = drive(3.0, 0.0, true, Some(20), 50);
    let (before, _) = run[19];
    for (p, v) in &run[21..] {
        let vxz = (v.x * v.x + v.z * v.z).sqrt();
        assert!(vxz < 0.3, "|v_xz| {vxz} after the pressed jump");
        assert!((p.z - before.z).abs() < 0.05, "stayed at the wall: {p:?}");
    }
}

/// A box within a jump's reach (1.2 m) is still jumped ONTO: the feet
/// clear its top before the capsule meets its face, and the ground
/// query stands him on it.
#[test]
fn a_jump_lands_on_a_box_within_its_reach() {
    let run = drive(1.2, 5.0, false, Some(4), 60);
    let on_top = run
        .iter()
        .any(|(p, _)| p.z < -1.0 && p.z > -5.0 && (p.y - 1.2).abs() < 1e-3);
    assert!(on_top, "never stood on the 1.2 m box");
}

/// Card AIRC: from rest against a 0.9 m box (over the step, so a walk
/// stops at it), a jump with the stick held forward lands ON it in both
/// games: once the feet are over its top the stick carries him in (BF2 at
/// 2.1 m/s^2, BF1 at 1.3 m/s). With no stick he comes down where he stood.
#[test]
fn a_standing_jump_with_the_stick_held_gets_onto_a_box() {
    use crate::soldier::GroundRules;
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        for stick in [32767i16, 0] {
            let mut s = step_sim(0.9);
            s.ground_rules = rules;
            let h = s
                .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
                .unwrap();
            let fwd = PadState {
                ly: 32767,
                ..PadState::default()
            };
            // Walk to the face (z -1 less his radius) and come to rest.
            for t in 0..60 {
                s.step(&[if t < 40 { fwd } else { PadState::default() }]);
            }
            let at = &s.unit(h).unwrap().soldier;
            assert!((at.pos.z + 0.5).abs() < 0.01 && at.vel.length() < 1e-3);
            let mut on_top = false;
            for t in 0..40 {
                s.step(&[PadState {
                    ly: stick,
                    buttons: if t == 0 { buttons::JUMP } else { 0 },
                    ..PadState::default()
                }]);
                let so = &s.unit(h).unwrap().soldier;
                on_top |= so.grounded && (so.pos.y - 0.9).abs() < 1e-3 && so.pos.z < -1.0;
            }
            assert_eq!(on_top, stick != 0, "{rules:?} stick {stick}");
        }
    }
}

/// A 100 m mesh floor at y = 0 with a wall slab across −Z at
/// z ∈ (−3.5, −2.5).
fn wall_world() -> crate::collision::CollisionWorld {
    use crate::collision::{Body, CollisionWorld, Shape};
    let floor = crate::collision::trimesh(
        &[
            [-50.0, 0.0, -50.0],
            [50.0, 0.0, -50.0],
            [50.0, 0.0, 50.0],
            [-50.0, 0.0, 50.0],
        ],
        &[0, 1, 2, 0, 2, 3],
    )
    .expect("floor");
    CollisionWorld::new(
        vec![
            Body::new(glam::Mat4::IDENTITY, Shape::Mesh(floor)),
            // A wall across the player's path (they start facing −Z).
            Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -3.0)),
                Shape::Box {
                    half: Vec3::new(20.0, 4.0, 0.5),
                },
            ),
        ],
        Vec::new(),
    )
}

/// The wall pass applies to EVERY alive unit, not just the player (Phase A
/// design decision 5 — `walls_for_ai` is retired now that the AI follows
/// the authored corridors). A non-player unit walking at the slab stops at
/// its face, one capsule radius out.
#[test]
fn ai_units_are_pushed_out_of_walls() {
    fn run(with_wall: bool) -> f32 {
        let mut sim = Sim::empty(3);
        sim.heightfield = Some(flat_ground());
        if with_wall {
            sim.collision = wall_world();
        }
        let h = sim
            .spawn(&UnitSpec {
                controller: Controller::Pacer {
                    vel: Vec3::new(0.0, 0.0, -2.0),
                    period: 0,
                },
                ..UnitSpec::trooper(1, Vec3::ZERO)
            })
            .expect("spawned");
        for _ in 0..60 {
            sim.step(&[PadState::default()]);
        }
        sim.units[h.index as usize].soldier.pos.z
    }
    // 60 ticks at 2 m/s = 4 m with nothing in the way ...
    let free = run(false);
    assert!(free < -3.9, "walked 4 m: {free}");
    // ... and stopped at the slab's face with the wall there.
    let stopped = run(true);
    assert!(
        (stopped - -2.0).abs() < 0.1,
        "pushed out of the wall: {stopped}"
    );
}

/// Card RKSLF (OURS): a bot whose launcher's salvo is latched in the
/// cycle (pressed, InitialSalvoDelay still running, FIRE up since)
/// fires no rocket into the wall 2.5 m before its muzzle: the salvo is
/// dropped where it would leave and the bot is unhurt. 22.5 m from the
/// same wall the rocket leaves - unless a team-mate stands by the spot
/// it lands on (an enemy there does not hold it). A charge launcher
/// (MaxPressedTime, TriggerSingle: the shot leaves when the trigger comes
/// up) whose bot lets go facing the near wall keeps its charge held.
#[test]
fn a_bot_fires_no_rocket_into_the_wall_before_it() {
    use crate::weapon::{ExplosionClass, FireState};
    // (shots, the shooter's health lost, blast holds)
    let run = |z: f32, by_wall: Option<u8>, charge: bool| {
        let mut sim = Sim::empty(3);
        sim.heightfield = Some(flat_ground());
        sim.collision = wall_world();
        let mut w = sim.weapon_classes[0].clone();
        w.ordnance.velocity = 100.0;
        w.ordnance.life_span = 3.0;
        w.ordnance.explosion = Some(ExplosionClass {
            damage: 400.0,
            damage_inner: 2.0,
            damage_outer: 4.0,
            ..ExplosionClass::default()
        });
        if charge {
            w.max_pressed_time = 0.75;
            w.trigger_single = true;
        } else {
            w.initial_salvo_delay = 0.25;
        }
        sim.weapon_classes.push(w);
        let wc = sim.weapon_classes.len() - 1;
        let h = sim.spawn(&ai(0, Vec3::new(0.0, 0.0, z), 0.0)).unwrap();
        if let Some(team) = by_wall {
            // An idle trooper on this side of the slab, 2.3 m from
            // where the rocket strikes it.
            sim.spawn(&UnitSpec::trooper(team, Vec3::new(2.0, 0.0, -1.5)))
                .unwrap();
        }
        {
            let u = &mut sim.units[h.index as usize];
            u.weapon_class = wc;
            u.weapon.active = true;
            u.weapon.clip = 10;
            if charge {
                // Charging for 5 ticks; the bot's pad lets go now.
                u.weapon.fire_cmd = 5;
            } else {
                // The press of a tick ago: the cycle holds one salvo,
                // due 0.2 s from now.
                u.weapon.state = FireState::Firing;
                u.weapon.salvo_counter = 1;
                u.weapon.salvo_timer = 0.2;
            }
        }
        let hp0 = sim.units[h.index as usize].health.cur;
        let mut shots = 0;
        for _ in 0..12 {
            sim.step(&[PadState::default()]);
            shots += sim
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == h))
                .count();
        }
        let hp = sim.unit(h).map_or(0.0, |u| u.health.cur);
        (shots, hp0 - hp, sim.nav_stats.blast_holds)
    };
    for charge in [false, true] {
        let (shots, lost, holds) = run(0.0, None, charge);
        assert_eq!(
            (shots, lost),
            (0, 0.0),
            "no rocket into the wall ({charge})"
        );
        assert!(holds >= 1);
        assert_eq!(
            run(20.0, None, charge),
            (1, 0.0, 0),
            "the far wall: it leaves"
        );
        let (shots, _, holds) = run(20.0, Some(0), charge);
        assert_eq!(shots, 0, "a team-mate by the far wall ({charge})");
        assert!(holds >= 1);
        assert_eq!(run(20.0, Some(1), charge).0, 1, "an enemy by the far wall");
    }
}

/// RKPRF: exercise the first press, independently of the latched salvo and
/// charge-release cases above. The retained, seen target avoids a think refresh;
/// a ready tap gate/burst makes the real AI produce FIRE on this very tick.
fn immediate_rocket_press(rules: GroundRules, human: bool, wall: bool) {
    use crate::weapon::{ExplosionClass, FireState};
    let mut sim = Sim::empty(3);
    sim.ground_rules = rules;
    sim.heightfield = Some(flat_ground());
    sim.tick = 100;
    let mut w = sim.weapon_classes[0].clone();
    w.initial_salvo_delay = 0.0;
    w.max_pressed_time = 0.0;
    w.trigger_single = true;
    w.ordnance.velocity = 100.0;
    w.ordnance.life_span = 3.0;
    w.ordnance.explosion = Some(ExplosionClass {
        damage: 400.0,
        damage_inner: 2.0,
        damage_outer: 4.0,
        ..ExplosionClass::default()
    });
    sim.weapon_classes.push(w);
    let wc = sim.weapon_classes.len() - 1;
    let spec = if human {
        UnitSpec::player(0, Vec3::ZERO)
    } else {
        ai(0, Vec3::ZERO, 0.0)
    };
    let shooter = sim.spawn(&spec).unwrap();
    let target = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -25.0)))
        .unwrap();
    let target_pos = sim.unit(target).unwrap().soldier.pos;
    {
        let u = &mut sim.units[shooter.index as usize];
        u.weapon_class = wc;
        u.weapon.active = true;
        u.weapon.clip = 10;
        assert!(u.held.is_empty(), "the primary weapon path");
        assert_eq!(u.weapon.state, FireState::Idle);
        assert_eq!((u.weapon.fire_cmd, u.weapon.salvo_counter), (0, 0));
        u.ai.next_think = 1000;
        u.ai.target = Some(target.into());
        u.ai.tap_gate = false;
        u.ai.tap_byte = -2;
        u.ai.burst = 10;
        u.ai.err = 0.0;
        u.ai.aim_dir = Vec3::NEG_Z;
        u.ai.contacts[0] = Some(crate::perception::Contact {
            handle: target.into(),
            first_seen: 0,
            last_seen: 100,
            last_pos: target_pos,
            seen_now: true,
            heard: false,
            told: false,
        });
    }
    if wall {
        sim.collision = wall_world();
    }
    if !human {
        // Inspect a COPY: Sim::step must generate and use its own AI pad.
        let mut brain = sim.units[shooter.index as usize].ai;
        let (pad, _) = crate::ai::think(&sim, shooter, &mut brain);
        assert!(
            pad.pressed(buttons::FIRE),
            "fresh AI press: {rules:?}, wall={wall}"
        );
    }
    let hp = sim.unit(shooter).unwrap().health.cur;
    let spare = sim.unit(shooter).unwrap().weapon.spare;
    sim.step(&[if human { fire() } else { PadState::default() }]);
    let shots: Vec<_> = sim
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Shot { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect();
    let held = !human && wall;
    assert_eq!(
        shots,
        if held { vec![] } else { vec![shooter] },
        "press-tick shot ownership: {rules:?}, human={human}, wall={wall}"
    );
    assert_eq!(
        sim.nav_stats.blast_holds,
        u64::from(held),
        "press-tick holds: {rules:?}, human={human}, wall={wall}"
    );
    // The exempt human can die to this point-blank shot; its slot still
    // carries the weapon-cycle result, while unit() excludes dead soldiers.
    let u = &sim.units[shooter.index as usize];
    assert_eq!(u.weapon.clip, if held { 10 } else { 9 });
    assert_eq!(u.weapon.spare, spare);
    if !human || !wall {
        assert_eq!(u.health.cur, hp, "the held/open-air shooter is unhurt");
    }
}

#[test]
fn an_ai_fresh_rocket_press_holds_at_a_wall_but_fires_in_open_air() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for wall in [false, true] {
            immediate_rocket_press(rules, false, wall);
        }
    }
}

#[test]
fn a_human_fresh_rocket_press_is_exempt_from_blast_holds() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        for wall in [false, true] {
            immediate_rocket_press(rules, true, wall);
        }
    }
}

/// Determinism holds with a collision world, and the world CHANGES the
/// replay (the player walks into a wall instead of through it).
#[test]
fn same_log_same_hash_with_a_collision_world() {
    fn run(log: &InputLog, with_world: bool) -> u64 {
        // The player is spawned AFTER the ground exists: `Sim::new`
        // spawns onto an empty sim, and since TERRAIN_BURY a unit that
        // lands on NO_GROUND stays there (300 m under an uncut
        // heightfield is underground, not a bank to climb).
        let mut sim = Sim::empty(log.seed);
        sim.heightfield = Some(flat_ground());
        if with_world {
            sim.collision = wall_world();
        }
        sim.spawn(&UnitSpec::player(0, Vec3::ZERO));
        for pads in log.frames() {
            sim.step(pads);
        }
        sim.hash()
    }
    // Walk straight into the wall (the player faces −Z), firing now and
    // then so the weapon/RNG streams are exercised too.
    let log = InputLog {
        seed: 0xC0FF_EE00,
        slots: 1,
        ticks: (0..300)
            .map(|i| PadState {
                ly: 32767,
                buttons: if i % 7 < 3 { buttons::FIRE } else { 0 },
                ..Default::default()
            })
            .collect(),
    };
    let a = run(&log, true);
    assert_eq!(a, run(&log, true), "same log, same hash");
    assert_ne!(a, run(&log, false), "the wall changes the replay");
}

/// D9: the skill's SPREAD multiplier is written onto an AI unit's weapon
/// every fire tick and left at 1.0 for a player, so an Easy bot's spread
/// grows 1.63x faster per shot than a human's.
#[test]
fn an_ai_units_weapon_carries_the_skills_spread_multiplier() {
    use crate::skill::Difficulty;
    let mut s = Sim::empty(5);
    s.difficulty = Difficulty::Easy;
    s.heightfield = Some(flat_ground());
    s.cheats.infinite_ammo = true;
    let bot = s.spawn(&ai(0, Vec3::ZERO, 0.0)).unwrap();
    let player = s
        .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, -20.0)))
        .unwrap();
    s.step(&[fire()]);
    let want = Difficulty::Easy
        .skill_at(s.unit(bot).unwrap().skill)
        .spread_mult;
    assert!((want - 1.632).abs() < 1e-3, "Easy spread mult {want}");
    assert_eq!(s.unit(bot).unwrap().weapon.spread_mult, want);
    assert_eq!(s.unit(player).unwrap().weapon.spread_mult, 1.0);
    // ... and that multiplier is what scales the per-shot spread growth
    // (weapon.rs `spread_per_shot`): same class, same shot, 1.63x apart.
    let wc = &s.weapon_classes[0];
    let grow = |mult: f32| {
        let mut w = crate::weapon::WeaponState::new(wc, false, 0);
        w.spread_mult = mult;
        // From the PS2's floor, clear of the port's tight hold (card
        // SPRD), so the shot's growth is not clamped away.
        w.spread = 0.0;
        let mut lcg = crate::lcg::GameLcg::new(1);
        let mut shots = Vec::new();
        w.update(
            crate::TICK_DT,
            wc,
            crate::weapon::FireInput {
                held: true,
                blocked: false,
                infinite_ammo: true,
                no_reload: true,
                ai_dispersion: None,
            },
            &mut lcg,
            &mut shots,
        );
        assert_eq!(shots.len(), 1);
        w.spread
    };
    let (a, b) = (grow(1.0), grow(want));
    // `update` also pays the per-tick recovery, which is the same on both
    // sides; add it back to compare the GROWTH the multiplier scales.
    let rec = crate::TICK_DT * wc.spread_recover_rate;
    assert!(
        ((b + rec) / (a + rec) - want).abs() < 1e-3,
        "spread growth {a} vs {b} (recovery {rec})"
    );
}

/// Phase C decision 2's fairness rule, at the shot: an AI unit only ever
/// fires at a contact its own perception holds. A wall body between two
/// bots means NO shot at all (the memory never gets a record); take the
/// wall away and the shots come with `unperceived_shots` still 0.
#[test]
fn an_ai_never_fires_at_an_unperceived_target() {
    let run = |wall: bool| {
        let mut s = Sim::empty(31);
        s.heightfield = Some(flat_ground());
        if wall {
            // The wall_world slab sits at z = -3; ours has to stand
            // between two bots 25 m apart.
            s.collision.push_body(crate::collision::Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -12.5)),
                crate::collision::Shape::Box {
                    half: Vec3::new(20.0, 5.0, 0.5),
                },
            ));
        }
        s.spawn(&ai(0, Vec3::ZERO, 0.0)).unwrap();
        s.spawn(&ai(1, Vec3::new(0.0, 0.0, -25.0), std::f32::consts::PI))
            .unwrap();
        let mut shots = 0usize;
        for _ in 0..300 {
            s.step(&[PadState::default()]);
            shots += s
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { .. }))
                .count();
        }
        (shots, s.nav_stats.ai_shots, s.nav_stats.unperceived_shots)
    };
    let (shots, ai_shots, unperceived) = run(true);
    assert_eq!((shots, ai_shots), (0, 0), "blind bots do not shoot");
    assert_eq!(unperceived, 0);
    let (shots, ai_shots, unperceived) = run(false);
    assert!(shots > 0, "with sight they fight");
    assert_eq!(ai_shots as usize, shots, "every shot counted");
    assert_eq!(unperceived, 0, "and every one at a held contact");
}

/// Infinite stamina keeps a sprinting player's energy pinned at the bar.
#[test]
fn infinite_stamina_cheat_pins_energy() {
    let mut s = Sim::new(3);
    s.cheats.infinite_stamina = true;
    let sprint = PadState {
        ly: 32767,
        buttons: buttons::SPRINT,
        ..Default::default()
    };
    for _ in 0..300 {
        s.step(&[sprint]);
    }
    let p = s.unit(s.players[0].unwrap()).unwrap();
    assert_eq!(p.soldier.energy, p.consts.energy_bar);
    assert_eq!(p.soldier.stance, crate::soldier::Stance::Sprint);
    let mut c = Cheats::default();
    c.toggle("stamina");
    assert!(c.infinite_stamina && c.entries()[3].2);
}

/// A human at the origin on flat ground, facing -Z, the spread off, with
/// a wall whose near face is `wall` metres ahead (`thick` deep).
fn aim_sim(rules: GroundRules, pitch: f32, wall: Option<(f32, f32)>) -> Sim {
    let mut s = Sim::empty(4);
    s.ground_rules = rules;
    s.heightfield = Some(flat_ground());
    let w = &mut s.weapon_classes[0];
    w.pitch_spread = 0.0;
    w.yaw_spread = 0.0;
    w.spread_per_shot = 0.0;
    w.posture_spread = [[0.0; 3]; 3];
    if let Some((z, thick)) = wall {
        s.collision = CollisionWorld::new(
            vec![crate::collision::Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -z - thick * 0.5)),
                crate::collision::Shape::Box {
                    half: Vec3::new(50.0, 50.0, thick * 0.5),
                },
            )],
            Vec::new(),
        );
    }
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    s.units[p.index as usize].soldier.pitch = pitch;
    s
}

/// One shot: (the aim ray, H, the bolt's origin and direction).
fn one_shot(s: &mut Sim) -> ((Vec3, Vec3), Vec3, Vec3, Vec3) {
    let ray = s.units[0].aim_ray(s.ground_rules, 1);
    let h = s.aim_hit(0);
    s.step(&[fire()]);
    let (o, d) = s
        .events
        .iter()
        .find_map(|e| match e {
            Event::Shot { origin, dir, .. } => Some((*origin, *dir)),
            _ => None,
        })
        .expect("the player fired");
    (ray, h, o, d)
}

/// Card AIMA, the PS2's numbers (`re-aim-assist.md` §10.2-10.3, [V-RAM]):
/// a soldier spawns at pitch -0.07; standing, the aim origin A is 2.3543 m
/// above the feet and the ideal camera looks 10 degrees below the aim -
/// BF2's on every map (ours since the playtest of 2026-10-28; the PS2's BF1
/// has 2.1043 m and 5 degrees, `unit::ps2_camera_section`) - and the fire
/// point F is 1.5828 m up, 0.060 m to the right and 0.2424 m ahead.
#[test]
fn the_aim_origin_camera_and_fire_point_are_the_ps2s() {
    for (rules, a_y, cam_deg) in [
        (GroundRules::Bf1, 2.3543, -14.011),
        (GroundRules::Bf2, 2.3543, -14.011),
    ] {
        let s = aim_sim(rules, crate::unit::SPAWN_PITCH, None);
        let u = &s.units[0];
        assert_eq!(u.soldier.pitch, -0.07, "a human's spawn pitch");
        let (a, d) = u.aim_ray(rules, 1);
        let cam = u.ideal_camera(rules, 1);
        let pitch = cam.forward.y.asin().to_degrees();
        let f = u.fire_point();
        println!(
            "{rules:?}: A {:.4} m up, camera {pitch:.3} deg, F up {:.4} right {:.4} ahead {:.4}",
            a.y, f.y, f.x, -f.z
        );
        assert!((a.y - a_y).abs() < 0.002, "A {} != {a_y}", a.y);
        assert!(
            (pitch - cam_deg).abs() < 0.01,
            "camera {pitch} != {cam_deg}"
        );
        assert!((f.y - 1.5828).abs() < 0.002, "F up {}", f.y);
        assert!((f.x - 0.060).abs() < 0.002, "F right {}", f.x);
        assert!((-f.z - 0.2424).abs() < 0.002, "F ahead {}", -f.z);
        // The ray leaves the camera's own position along the aim.
        assert!((a - cam.position).cross(d).length() < 1e-4);
    }
    // Split screen: 0.4 of the tilt (4 deg).
    let s = aim_sim(GroundRules::Bf1, 0.0, None);
    let cam = s.units[0].ideal_camera(GroundRules::Bf1, 2);
    assert!((cam.forward.y.asin().to_degrees() + 4.0).abs() < 0.01);
}

/// Card AIMA, the wall test (on a BF1 map, which plays BF2's camera table
/// since 2026-10-28; the spread off). Within the
/// ray's reach the bolt goes to the reticle's point; past it the bolt is
/// aimed at the ray's far end; a wall nearer than the ray's start is not
/// seen (the PS2's blind spot) and the bolt lands under the reticle.
#[test]
fn a_humans_bolt_goes_where_the_reticles_ray_hits() {
    let rules = GroundRules::Bf1;
    let pitch = crate::unit::SPAWN_PITCH;
    // Where a line from `o` along `d` meets the plane z = -wall.
    let at = |o: Vec3, d: Vec3, wall: f32| o + d * ((-wall - o.z) / d.z);

    // The wall 20 m out. On flat ground the default pitch's ray meets
    // the floor first, so the ray is raised to meet the wall.
    let mut s = aim_sim(rules, 0.05, Some((20.0, 1.0)));
    let ((a, d), h, f, dir) = one_shot(&mut s);
    let miss = (at(f, dir, 20.0) - at(a, d, 20.0)).length();
    println!("wall 20 m: H {h:?}, the bolt passes {miss:.4} m from the ray's point");
    assert!((h.z + 20.0).abs() < 1e-3, "H is on the wall: {h:?}");
    assert!(miss < 0.05);

    // 80 m out: H is the ray's end, 65.2 m from A (the 3.2 m track and the
    // AIM_RAY_START and AIM_RAY_LENGTH past it).
    let mut s = aim_sim(rules, 0.05, Some((80.0, 1.0)));
    let ((a, d), h, f, dir) = one_shot(&mut s);
    let over = at(f, dir, 80.0).y - at(a, d, 80.0).y;
    println!(
        "wall 80 m: |H - A| {:.3}, the bolt {over:.4} m over the ray's point",
        (h - a).length()
    );
    assert!(((h - a).length() - 65.2).abs() < 1e-3);
    assert!((0.0..0.2).contains(&over), "{over}");

    // 4.8 m from the feet (0.2 m thick): before the ray's 5.2 m start.
    let mut s = aim_sim(rules, 0.05, Some((4.8, 0.2)));
    let ((a, d), h, f, dir) = one_shot(&mut s);
    let under = at(a, d, 4.8).y - at(f, dir, 4.8).y;
    println!(
        "wall 4.8 m: |H - A| {:.3}, the bolt {under:.4} m under the ray's point",
        (h - a).length()
    );
    assert!(
        ((h - a).length() - 65.2).abs() < 1e-3,
        "the wall is not seen"
    );
    assert!((0.65..0.75).contains(&under), "{under}");

    // The default pitch: the bolt goes to the floor under the reticle,
    // not into the floor near the feet.
    let mut s = aim_sim(rules, pitch, None);
    let ((a, d), h, f, dir) = one_shot(&mut s);
    let land = f + dir * (-f.y / dir.y);
    let ray_floor = a + d * (-a.y / d.y);
    println!(
        "default pitch: H {h:?}, the bolt lands {:.2} m ahead, the ray {:.2} m",
        -land.z, -ray_floor.z
    );
    assert!(-land.z > 5.0);
    assert!(
        (land - ray_floor).length() < 0.6,
        "on the floor under the reticle"
    );

    // The steepest aim: 1.53 m ahead of the feet (BF1), 1.54 m (BF2);
    // never on them.
    for (rules, want) in [(GroundRules::Bf1, 1.53), (GroundRules::Bf2, 1.54)] {
        let mut s = aim_sim(rules, -0.8, None);
        let (_, _, f, dir) = one_shot(&mut s);
        let land = f + dir * (-f.y / dir.y);
        println!(
            "{rules:?} pitch -0.8: the bolt lands {:.3} m ahead",
            -land.z
        );
        assert!((-land.z - want).abs() < 0.02, "{}", -land.z);
    }
}

/// A unit on the aim ray is what the bolt is aimed at; a bot's shot is
/// untouched (the chest, along its aim).
#[test]
fn the_aim_ray_sees_units_and_bots_keep_their_muzzle() {
    let mut s = aim_sim(GroundRules::Bf2, -0.05, None);
    let (a, d) = s.units[0].aim_ray(GroundRules::Bf2, 1);
    // A trooper standing where the ray is 1 m above the floor.
    let on_ray = a + d * ((1.0 - a.y) / d.y);
    s.spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, on_ray.z)))
        .unwrap();
    let h = s.aim_hit(0);
    println!("the ray is at 1 m {:.2} m out; H {h:?}", -on_ray.z);
    assert!(
        (h.z - (on_ray.z + 0.5)).abs() < 0.05,
        "the capsule's near side: {h:?}"
    );
    let (_, _, f, dir) = one_shot(&mut s);
    assert!(
        (f + dir * (h - f).length() - h).length() < 1e-3,
        "the bolt flies at H"
    );
    let b = s.spawn(&ai(1, Vec3::new(40.0, 0.0, 0.0), 0.0)).unwrap();
    let u = s.unit(b).unwrap();
    assert_eq!(u.soldier.pitch, 0.0, "a bot's spawn pitch is FPNT's");
    assert_eq!(u.muzzle().y, 1.425);
}

/// Card ZOOM's fixtures: a rifle (2.5 / 2.5), a sniper rifle (2 / 8,
/// `ZoomFirstPerson`) in channel 0's second slot and a grenade (no zoom
/// key) in channel 1, on a human.
fn zoom_sim(rules: GroundRules) -> Sim {
    use crate::unit::SlotSpec;
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    s.ground_rules = rules;
    let mut sniper = WeaponClass::bf1_rifle();
    sniper.name = "sniper".into();
    (sniper.zoom_min, sniper.zoom_max) = (2.0, 8.0);
    sniper.zoom_turn_div = [2.0, 8.0];
    sniper.zoom_first_person = true;
    let sniper = s.add_weapon_class(sniper);
    let mut grenade = WeaponClass::bf1_rifle();
    grenade.name = "grenade".into();
    (grenade.zoom_min, grenade.zoom_max) = (1.0, 1.0);
    grenade.zoom_turn_div = [1.0, 1.0];
    let grenade = s.add_weapon_class(grenade);
    let slot = |slot, channel, weapon_class| SlotSpec {
        slot,
        channel,
        weapon_class,
        clips: 4,
    };
    s.spawn(&UnitSpec {
        slots: std::sync::Arc::new([slot(1, 0, sniper), slot(2, 1, grenade)]),
        ..UnitSpec::player(0, Vec3::ZERO)
    })
    .unwrap();
    s
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

/// A press of `bit` (one tick down, one up); the zoom value after it.
fn tap(s: &mut Sim, bit: u32) -> Option<f32> {
    s.step(&[pad(bit)]);
    s.step(&[pad(0)]);
    s.units[0].zoom.map(|z| z.value)
}

/// Card ZOOM acceptance 1 (`analysis/re-zoom.md` §3, §9): R3 is a press
/// toggle - the rifle on, off; the sniper rifle 2, 8, off; a held button
/// is one press; a grenade in hand and a bot never.
#[test]
fn the_zoom_press_steps_the_weapon_in_hand() {
    let mut s = zoom_sim(GroundRules::Bf1);
    let seq: Vec<_> = (0..4).map(|_| tap(&mut s, buttons::ZOOM)).collect();
    println!("rifle: {seq:?}");
    assert_eq!(seq, [Some(2.5), None, Some(2.5), None]);
    // Held 1.5 s: one toggle on the press, nothing at the release.
    for _ in 0..45 {
        s.step(&[pad(buttons::ZOOM)]);
        assert_eq!(s.units[0].zoom.map(|z| z.step), Some(0));
    }
    s.step(&[pad(0)]);
    assert!(s.units[0].zoom.is_some());
    tap(&mut s, buttons::ZOOM);
    // The sniper rifle: 2, 8, off, 2 with the divisors 2, 8.
    tap(&mut s, buttons::SWITCH_WEAPON);
    assert_eq!(s.weapon_classes[s.units[0].weapon_class].name, "sniper");
    let seq: Vec<_> = (0..4)
        .map(|_| {
            tap(&mut s, buttons::ZOOM);
            s.units[0].zoom.map(|z| (z.value, z.divisor, z.scope))
        })
        .collect();
    println!("sniper: {seq:?}");
    assert_eq!(
        seq,
        [
            Some((2.0, 2.0, true)),
            Some((8.0, 8.0, true)),
            None,
            Some((2.0, 2.0, true))
        ]
    );
    // A weapon with no zoom key in the hands: nothing.
    let mut s = zoom_sim(GroundRules::Bf1);
    s.units[0].weapon_class = 2;
    assert_eq!(tap(&mut s, buttons::ZOOM), None);
    // A bot with the ZOOM bit forced: nothing.
    let mut s = zoom_sim(GroundRules::Bf1);
    s.units[0].controller = Controller::Ai;
    s.tick_zoom(0, &pad(buttons::ZOOM));
    assert_eq!(s.units[0].zoom, None);
    s.units[0].controller = Controller::Player { slot: 0 };
    s.tick_zoom(0, &pad(buttons::ZOOM));
    assert!(s.units[0].zoom.is_some(), "the same press on a human");
}

/// Card ZOOM acceptance 2 (§4, §9): what ends the zoom and what keeps it.
#[test]
fn the_zoom_ends_on_its_cancels_and_no_others() {
    let fwd = |buttons| PadState {
        ly: 32767,
        buttons,
        ..Default::default()
    };
    // (name, rules, the pad held after zooming, ticks, still zoomed?)
    let cases: [(&str, GroundRules, PadState, u32, bool); 9] = [
        ("reload", GroundRules::Bf1, pad(buttons::RELOAD), 3, false),
        (
            "primary switch",
            GroundRules::Bf1,
            pad(buttons::SWITCH_WEAPON),
            1,
            false,
        ),
        (
            "secondary throw",
            GroundRules::Bf1,
            pad(buttons::SECONDARY_FIRE),
            3,
            false,
        ),
        (
            "roll",
            GroundRules::Bf1,
            fwd(buttons::ROLL_STANCE),
            3,
            false,
        ),
        ("sprint", GroundRules::Bf2, fwd(buttons::SPRINT), 60, false),
        ("walk", GroundRules::Bf2, fwd(0), 60, true),
        ("jump", GroundRules::Bf1, pad(buttons::JUMP), 20, true),
        (
            "crouch",
            GroundRules::Bf1,
            pad(buttons::ROLL_STANCE),
            20,
            true,
        ),
        ("fire", GroundRules::Bf1, pad(buttons::FIRE), 20, true),
    ];
    for (name, rules, held, ticks, kept) in cases {
        let mut s = zoom_sim(rules);
        // A clip some rounds short (a full one does not reload), then
        // past the stance cycle's half second.
        for _ in 0..6 {
            s.step(&[pad(buttons::FIRE)]);
        }
        for _ in 0..20 {
            s.step(&[pad(0)]);
        }
        // The roll and the sprint need a soldier already moving.
        let moving = held.ly != 0;
        for _ in 0..if moving { 30 } else { 0 } {
            s.step(&[fwd(0)]);
        }
        s.step(&[if moving {
            fwd(buttons::ZOOM)
        } else {
            pad(buttons::ZOOM)
        }]);
        assert!(s.units[0].zoom.is_some(), "{name}: zoomed");
        let stance0 = s.units[0].soldier.stance;
        let mut ended = None;
        let mut stances = vec![];
        for t in 1..=ticks {
            s.step(&[held]);
            let st = s.units[0].soldier.stance;
            if !stances.contains(&st) {
                stances.push(st);
            }
            if ended.is_none() && s.units[0].zoom.is_none() {
                ended = Some(t);
            }
        }
        println!("{name}: from {stance0:?} through {stances:?}, zoom ended on tick {ended:?}");
        assert_eq!(ended.is_none(), kept, "{name}");
        match name {
            "roll" => assert!(stances.contains(&Stance::Roll)),
            "sprint" => assert!(stances.contains(&Stance::Sprint)),
            "jump" => assert!(stances.contains(&Stance::Jump)),
            "crouch" => assert!(stances.contains(&Stance::Crouch)),
            _ => {}
        }
        // Nothing brings it back: the next press starts at ZoomMin.
        if !kept {
            s.step(&[pad(0)]);
            assert_eq!(s.units[0].zoom, None, "{name}: stays off");
        }
    }
    // Death.
    let mut s = zoom_sim(GroundRules::Bf1);
    tap(&mut s, buttons::ZOOM);
    let h = s.handle(0);
    s.kill_environment(h, DeathCause::Fall);
    s.step(&[pad(0)]);
    assert_eq!(s.units[0].zoom, None, "death");
}

/// A blast that throws the zoomed player ends the zoom (re-zoom §4: the
/// thrown state clears it), and nothing zooms or fires again until he is
/// up and out of the get-up's lock (playtest 2026-10-29, invincible: the
/// scope stayed up while his body flew).
#[test]
fn a_throw_ends_the_zoom_and_holds_it_and_the_trigger_off_until_he_is_up() {
    let thermal = crate::weapon::ExplosionClass {
        damage: 320.0,
        damage_inner: 4.5,
        damage_outer: 6.5,
        push: 10.0,
        push_outer: 4.5,
        ..Default::default()
    };
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut s = zoom_sim(rules);
        s.cheats.invincible = true;
        tap(&mut s, buttons::ZOOM);
        assert!(s.units[0].zoom.is_some(), "{rules:?}: zoomed");
        let feet = s.units[0].soldier.pos;
        s.explode(&thermal, feet + Vec3::new(2.0, 0.12, 0.0), None, Some(1));
        assert_eq!(s.units[0].soldier.stance, Stance::Thrown, "{rules:?}");
        let mut down = 0;
        let mut shots = 0;
        for t in 0..300 {
            if !s.units[0].soldier.no_fire() {
                break;
            }
            down += 1;
            // Mash zoom and fire the whole time.
            let b = if t % 2 == 0 {
                buttons::ZOOM | buttons::FIRE
            } else {
                buttons::FIRE
            };
            s.step(&[pad(b)]);
            assert_eq!(
                s.units[0].zoom, None,
                "{rules:?} tick {t}: no zoom while down"
            );
            // A shot on the tick the lock runs out is his first free one.
            if s.units[0].soldier.no_fire() {
                shots += s
                    .events
                    .iter()
                    .filter(|e| matches!(e, Event::Shot { .. }))
                    .count();
            }
        }
        println!("{rules:?}: down {down} ticks, {shots} shots");
        assert!(down > 30 && down < 300, "{rules:?}: thrown, then up");
        assert_eq!(shots, 0, "{rules:?}: no shot while down");
        // Up again: the zoom and the trigger work.
        s.step(&[pad(0)]);
        assert!(
            tap(&mut s, buttons::ZOOM).is_some(),
            "{rules:?}: zooms again"
        );
    }
}

/// Card ZOOM acceptance 3 (§6): zoomed, the yaw and pitch rates are the
/// plain ones over the weapon's turn divisor (PS2: 1 / 2.497 to 1 / 2.499
/// for 2.5 on every update), and the spread's posture takes the flag.
#[test]
fn the_zoomed_turn_rates_are_divided_by_the_divisor() {
    let turn = PadState {
        rx: 32767,
        ry: 16000,
        ..Default::default()
    };
    // (yaw, pitch) turned in one tick, per zoom step reached by `taps`.
    let rate = |taps: u32, sniper: bool| {
        let mut s = zoom_sim(GroundRules::Bf1);
        s.assist_intensity = 0.0;
        s.units[0].soldier.pitch = 0.0;
        if sniper {
            tap(&mut s, buttons::SWITCH_WEAPON);
        }
        for _ in 0..taps {
            tap(&mut s, buttons::ZOOM);
        }
        let before = s.units[0].soldier.clone();
        s.step(&[turn]);
        let after = &s.units[0].soldier;
        (after.yaw - before.yaw, after.pitch - before.pitch)
    };
    let plain = rate(0, false);
    assert!(plain.0 != 0.0 && plain.1 != 0.0);
    for (taps, sniper, div) in [(1, false, 2.5), (1, true, 2.0), (2, true, 8.0)] {
        let z = rate(taps, sniper);
        let (ry, rp) = (plain.0 / z.0, plain.1 / z.1);
        println!("divisor {div}: yaw plain / zoomed {ry:.6}, pitch {rp:.6}");
        assert!((ry - div).abs() < 1e-4 * div, "yaw {ry}");
        assert!((rp - div).abs() < 1e-4 * div, "pitch {rp}");
        assert!((z.0 - plain.0 / div).abs() < 1e-5);
        assert!((z.1 - plain.1 / div).abs() < 1e-5);
    }
}

/// Card ZOOM acceptance 4, the sim's half (§5): the zoomed camera is the
/// stance's ZOOM section on the first zoomed tick - the PS2's eye (right
/// 0.400, up 2.216, back 2.769) 3.50 deg under the aim at pitch -4.01
/// deg, both games - and the marksman's scope eye is (0.060, 1.604,
/// 0.057) with no tilt (BF2: 0.6 m back, no sideways terms).
#[test]
fn the_zoomed_camera_is_the_ps2s() {
    let rel = |s: &Sim| {
        let u = &s.units[0];
        let cam = u.ideal_camera(s.ground_rules, 1);
        let (sy, cy) = crate::fmath::sin_cos(u.soldier.yaw);
        let (ahead, right) = (Vec3::new(-sy, 0.0, -cy), Vec3::new(cy, 0.0, -sy));
        let p = cam.position - u.soldier.pos;
        let pitch = cam.forward.y.atan2(cam.forward.with_y(0.0).length());
        (
            Vec3::new(p.dot(right), p.y, -p.dot(ahead)),
            (u.soldier.pitch - pitch).to_degrees(),
        )
    };
    let near = |a: Vec3, b: Vec3| (a - b).abs().max_element() < 0.0015;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut s = zoom_sim(rules);
        s.units[0].soldier.yaw = 0.7;
        s.units[0].soldier.pitch = f32::to_radians(-4.01);
        let plain = rel(&s);
        s.step(&[pad(buttons::ZOOM)]);
        let (eye, tilt) = rel(&s);
        println!("{rules:?}: unzoomed {plain:?}, first zoomed tick eye {eye}, tilt {tilt:.3}");
        // The unzoomed camera is BF2's on both (ours; the PS2's BF1 stands
        // at (0, 2.348, 3.457), 5 deg under the aim).
        let want = (Vec3::new(0.0, 2.575, 3.105), 10.0);
        assert!(near(plain.0, want.0) && (plain.1 - want.1).abs() < 0.01);
        assert!(near(eye, Vec3::new(0.400, 2.216, 2.769)), "{eye}");
        assert!((tilt - 3.5).abs() < 0.01);
        // The aim ray leaves abreast of the eye point, 2.8 m ahead of
        // the camera, and the reticle's ray is still the aim.
        let (a, d) = s.units[0].aim_ray(rules, 1);
        assert_eq!(d, s.units[0].aim_frame().0);
        assert!(a.distance(s.units[0].ideal_camera(rules, 1).position) < 2.9);
        // The scope.
        s.step(&[pad(0)]);
        s.step(&[pad(buttons::ZOOM)]);
        tap(&mut s, buttons::SWITCH_WEAPON);
        s.step(&[pad(buttons::ZOOM)]);
        assert!(s.units[0].scoped());
        let (eye, tilt) = rel(&s);
        println!("{rules:?}: scope eye {eye}, tilt {tilt:.3}");
        let want = match rules {
            GroundRules::Bf1 => Vec3::new(0.060, 1.604, 0.057),
            GroundRules::Bf2 => Vec3::new(-0.060, 1.742, 0.599),
        };
        assert!(near(eye, want), "{eye}");
        assert!(tilt.abs() < 1e-3);
        // The scoped shot leaves the scope point along the aim itself.
        let u = &s.units[0];
        let (o, (fwd, ..)) = s.human_shot(0);
        assert_eq!((o, fwd), (u.scope_point(rules), u.aim_frame().0));
        assert!((s.aim_hit(0) - (o + fwd * SCOPE_AIM_LENGTH)).length() < 1e-3);
    }
}

#[test]
fn the_first_person_eye_and_aim_are_the_ps2s() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut s = zoom_sim(rules);
        s.units[0].soldier.yaw = 0.7;
        s.units[0].soldier.pitch = f32::to_radians(-4.01);
        s.step(&[pad(buttons::FIRST_PERSON)]);
        let u = &s.units[0];
        assert!(u.first_person_view() && u.eye_view() && !u.scope_path());
        let d = u.aim_frame().0;
        let cam = u.ideal_camera(rules, 1);
        assert!(cam.position.distance(u.shoulder_pivot() - d * 0.3) < 1e-5);
        assert_eq!(cam.forward, d);
        assert_eq!(u.aim_ray(rules, 1), (u.fire_point(), d));
        let (o, _) = s.human_shot(0);
        assert_eq!(o, u.fire_point());

        s.step(&[pad(buttons::FIRST_PERSON | buttons::ZOOM)]);
        let u = &s.units[0];
        assert!(u.zoom.is_some() && !u.scoped() && u.scope_path());
        let d = u.aim_frame().0;
        let (eye, back) = match rules {
            GroundRules::Bf1 => (u.fire_point(), 0.3),
            GroundRules::Bf2 => (u.shoulder_pivot(), 0.6),
        };
        let cam = u.ideal_camera(rules, 1);
        assert!(cam.position.distance(eye - d * back) < 1e-5);
        let (o, (fwd, ..)) = s.human_shot(0);
        assert_eq!((o, fwd), (u.scope_point(rules), d));

        s.step(&[pad(0)]);
        let u = &s.units[0];
        assert!(!u.first_person && !u.eye_view());
    }
}

#[test]
fn a_thrown_or_dead_soldier_leaves_first_person() {
    use crate::soldier::Stance;
    let mut s = zoom_sim(GroundRules::Bf2);
    s.step(&[pad(buttons::FIRST_PERSON)]);
    for stance in [Stance::Thrown, Stance::Tumble, Stance::GetUp, Stance::Dead] {
        s.units[0].soldier.stance = stance;
        assert!(s.units[0].first_person && !s.units[0].first_person_view());
    }
    s.units[0].soldier.stance = Stance::Stand;
    assert!(s.units[0].first_person_view());
    s.units[0].alive = false;
    assert!(!s.units[0].first_person_view());
}

#[test]
fn a_bot_never_takes_first_person_and_third_person_hashes_as_before() {
    let mut a = zoom_sim(GroundRules::Bf1);
    let mut b = zoom_sim(GroundRules::Bf1);
    a.step(&[pad(0)]);
    b.step(&[pad(0)]);
    assert_eq!(a.hash(), b.hash());
    b.step(&[pad(buttons::FIRST_PERSON)]);
    a.step(&[pad(0)]);
    assert_ne!(a.hash(), b.hash());
    assert!(b.units.iter().skip(1).all(|u| !u.first_person));
}

/// A round nobody zooms in hashes as it did: the zoom enters the hash
/// only while it is on.
#[test]
fn the_zoom_is_hashed_only_while_on() {
    let mut a = zoom_sim(GroundRules::Bf1);
    let b = zoom_sim(GroundRules::Bf1);
    assert_eq!(a.hash(), b.hash());
    a.units[0].zoom = Some(crate::unit::Zoom {
        step: 0,
        value: 2.5,
        divisor: 2.5,
        scope: false,
    });
    assert_ne!(a.hash(), b.hash());
}

/// RKMAR: the tick must consult release clearance for a due salvo. Distances
/// are lateral capsule-centre offsets from the wall blast, not muzzle ranges.
fn rkmar_due_salvo(rules: GroundRules) {
    use crate::weapon::{ExplosionClass, FireState};
    for (distance, team, held) in [(4.5, 0, true), (5.5, 0, false), (4.5, 1, false)] {
        let mut s = Sim::empty(3);
        s.ground_rules = rules;
        s.heightfield = Some(flat_ground());
        s.collision = wall_world();
        s.weapon_classes[0].ordnance.explosion = Some(ExplosionClass {
            damage: 400.0,
            damage_inner: 2.0,
            damage_outer: 4.0,
            ..ExplosionClass::default()
        });
        let shooter = s.spawn(&ai(0, Vec3::new(0.0, 0.0, 20.0), 0.0)).unwrap();
        let muzzle = s.unit(shooter).unwrap().muzzle();
        let other = s
            .spawn(&UnitSpec::trooper(
                team,
                Vec3::new(muzzle.x + distance, 0.0, -2.3),
            ))
            .unwrap();
        let hp = s.unit(other).unwrap().health.cur;
        let own_hp = s.unit(shooter).unwrap().health.cur;
        let u = s.unit_mut(shooter).unwrap();
        u.weapon.active = true;
        u.weapon.clip = 10;
        u.weapon.state = FireState::Firing;
        u.weapon.salvo_counter = 1;
        u.weapon.salvo_timer = 0.2;
        let spare = u.weapon.spare;
        let mut owners = Vec::new();
        for _ in 0..12 {
            s.step(&[]);
            owners.extend(s.events.iter().filter_map(|e| match e {
                Event::Shot { unit, .. } => Some(*unit),
                _ => None,
            }));
        }
        assert_eq!(
            owners,
            if held { vec![] } else { vec![shooter] },
            "{rules:?} d={distance} team={team}"
        );
        assert_eq!(s.nav_stats.blast_holds, u64::from(held));
        let u = &s.units[shooter.index as usize];
        assert_eq!(
            (u.weapon.clip, u.weapon.spare),
            (if held { 10 } else { 9 }, spare)
        );
        assert_eq!(u.health.cur, own_hp);
        assert_eq!(s.units[other.index as usize].health.cur, hp);
    }
}

#[test]
fn rkmar_bf1_due_salvo_holds_only_for_the_margin_friend() {
    rkmar_due_salvo(GroundRules::Bf1);
}

#[test]
fn rkmar_bf2_due_salvo_holds_only_for_the_margin_friend() {
    rkmar_due_salvo(GroundRules::Bf2);
}

#[test]
fn a_jump_holds_the_trigger_off_until_it_lands() {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    s.cheats.infinite_ammo = true;
    let h = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    for _ in 0..10 {
        s.step(&[PadState::default()]);
    }
    s.step(&[PadState {
        buttons: buttons::JUMP | buttons::FIRE,
        ..PadState::default()
    }]);
    let shots = s.unit(h).unwrap().weapon.shots_fired;
    let mut airborne = 0;
    for _ in 0..120 {
        s.step(&[fire()]);
        let u = s.unit(h).unwrap();
        if u.soldier.stance != Stance::Jump {
            break;
        }
        airborne += 1;
        assert_eq!(u.weapon.shots_fired, shots, "fired in the air");
    }
    assert!(airborne > 20, "{airborne} ticks in the air");
    for _ in 0..60 {
        s.step(&[fire()]);
    }
    assert!(s.unit(h).unwrap().weapon.shots_fired > shots);
}
