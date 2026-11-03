use super::*;
use crate::sim::tests::flat_ground;
use crate::sim::PadState;
use crate::unit::Controller;

/// DR2: a unit standing in a door's trigger opens it through
/// `Sim::step` — the parameter reaches 1.0 in the clip's (frames−1)/30 s
/// and the door's body rides to the last frame — and nobody in range
/// leaves it shut. The door's parameter is in the hash; its sound latch
/// and the `DoorOpened` event (DRV) are not.
#[test]
fn a_unit_in_the_trigger_opens_the_door_and_the_door_is_hashed() {
    use crate::collision::{Aabb, Body, CollisionWorld, Shape};
    use crate::door::{Door, DoorBody, Trigger};
    use glam::{Mat4, Quat};
    let make = |unit_at: Vec3| {
        let mut s = Sim::empty(4);
        s.heightfield = Some(flat_ground());
        let shut = Vec3::new(0.0, 2.0, -3.0);
        let body = Body::new(
            Mat4::from_translation(shut),
            Shape::Box {
                half: Vec3::new(1.0, 2.0, 0.2),
            },
        );
        let mut env = Aabb::empty();
        env.add(Vec3::new(-1.0, 0.0, -3.2));
        env.add(Vec3::new(1.0, 7.0, -2.8));
        s.collision = CollisionWorld::with_movers(vec![body], Vec::new(), vec![(0, env)]);
        s.doors = vec![Door {
            triggers: vec![Trigger {
                center: Vec3::new(0.0, 0.0, -3.0),
                radius: 4.0,
            }],
            frames: 31,
            bodies: vec![DoorBody {
                body: 0,
                frames: (0..31)
                    .map(|f| (Quat::IDENTITY, shut + Vec3::Y * (3.0 * f as f32 / 30.0)))
                    .collect(),
            }],
            ..Door::default()
        }];
        s.spawn(&UnitSpec::player_in_slot(0, 0, unit_at)).unwrap();
        s
    };
    let mut near = make(Vec3::new(0.0, 0.0, 0.0));
    let mut far = make(Vec3::new(0.0, 0.0, 20.0));
    let opened = |s: &Sim| {
        s.events
            .iter()
            .filter(|e| matches!(e, Event::DoorOpened { door: 0 }))
            .count()
    };
    let mut locked = make(Vec3::ZERO);
    assert_eq!(near.hash(), locked.hash());
    locked.doors[0].locked = true;
    assert_ne!(near.hash(), locked.hash(), "lock affects future ticks");
    assert!(!locked.door_opening(&locked.collision.bodies()[0]));
    locked.step(&[PadState::default()]);
    assert_eq!(locked.doors[0].open, 0.0);
    assert_eq!(opened(&locked), 0);
    assert!(locked.events.contains(&Event::DoorLocked { door: 0 }));
    assert_eq!(locked.collision.bodies()[0].to_world.w_axis.y, 2.0);
    locked.doors[0].locked = false;
    locked.step(&[PadState::default()]);
    assert!(locked.doors[0].open > 0.0);
    assert_eq!(opened(&locked), 1);
    for i in 0..30 {
        near.step(&[PadState::default()]);
        far.step(&[PadState::default()]);
        // DRV: the open sound's event on the first occupied tick only.
        assert_eq!(opened(&near), usize::from(i == 0), "tick {i}");
        assert_eq!(opened(&far), 0);
    }
    assert_eq!(near.doors[0].open, 1.0, "opened in 30 ticks = 1.00 s");
    assert!(
        near.doors[0].latch && !far.doors[0].latch,
        "only the occupied door latched"
    );
    assert_eq!(far.doors[0].open, 0.0, "nobody in range: shut");
    let y = |s: &Sim| s.collision.bodies()[0].to_world.w_axis.y;
    assert!((y(&near) - 5.0).abs() < 1e-5, "the box rode the clip");
    assert_eq!(y(&far), 2.0);
    // Same everything but the door's parameter: a different hash.
    let mut shut = make(Vec3::new(0.0, 0.0, 0.0));
    let open = make(Vec3::new(0.0, 0.0, 0.0));
    shut.doors[0].open = 0.5;
    assert_ne!(shut.hash(), open.hash());
    // The sound latch is output-side: not hashed.
    let mut latched = make(Vec3::new(0.0, 0.0, 0.0));
    latched.doors[0].latch = true;
    assert_eq!(latched.hash(), open.hash());
    // And no door at all hashes exactly as before doors existed: the
    // field eats nothing when it is empty.
    let mut none = make(Vec3::ZERO);
    let before = none.hash();
    none.doors.clear();
    assert_ne!(none.hash(), before, "a door list is hashed");
    let mut bare = Sim::empty(4);
    bare.heightfield = Some(flat_ground());
    bare.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    assert_eq!(bare.hash(), none.hash(), "an empty list eats nothing");
}

/// Card MOUTH2: a door is opening while a living unit stands in one of
/// its triggers and its clip is not at its end — and only its own leaves
/// answer.
#[test]
fn a_door_is_opening_while_a_unit_is_in_its_trigger_and_its_clip_runs() {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::door::{Door, DoorBody, Trigger};
    use glam::{Mat4, Quat};
    let make = |unit_at: Vec3| {
        let mut s = Sim::empty(4);
        s.heightfield = Some(flat_ground());
        let slab = |x: f32| {
            Body::new(
                Mat4::from_translation(Vec3::new(x, 2.0, -3.0)),
                Shape::Box {
                    half: Vec3::new(1.0, 2.0, 0.2),
                },
            )
        };
        s.collision = CollisionWorld::new(vec![slab(0.0), slab(10.0)], Vec::new());
        s.doors = vec![Door {
            triggers: vec![Trigger {
                center: Vec3::new(0.0, 0.0, -3.0),
                radius: 4.0,
            }],
            frames: 31,
            bodies: vec![DoorBody {
                body: 0,
                frames: vec![(Quat::IDENTITY, Vec3::new(0.0, 2.0, -3.0)); 31],
            }],
            ..Door::default()
        }];
        s.spawn(&UnitSpec::player_in_slot(0, 0, unit_at)).unwrap();
        s
    };
    let leaf = |s: &Sim| s.door_opening(&s.collision.bodies()[0]);
    let mut s = make(Vec3::new(0.0, 0.0, 0.5));
    assert!(leaf(&s), "shut, and a unit stands in the trigger");
    assert!(
        !s.door_opening(&s.collision.bodies()[1]),
        "a body that is no leaf of it"
    );
    s.doors[0].open = 0.97;
    assert!(leaf(&s), "still sliding");
    s.doors[0].open = 1.0;
    assert!(!leaf(&s), "the clip is at its end: what is left is a wall");
    s.doors[0].open = 0.5;
    s.units[0].alive = false;
    assert!(!leaf(&s), "a dead unit holds no door");
    assert!(
        !leaf(&make(Vec3::new(0.0, 0.0, 1.5))),
        "nobody within the trigger's 4 m: a wall"
    );

    // Card DOORS: the line through a point, against the leaf's box (x
    // -1 .. 1, z -3.2 .. -2.8) - at any height, and only while the door
    // is opening; the slab at x 10 is no door's.
    let mut s = make(Vec3::new(0.0, 0.0, 0.5));
    let over = |s: &Sim, x: f32, y: f32, z: f32| s.opening_leaf_over(Vec3::new(x, y, z));
    assert!(over(&s, 0.9, 0.0, -3.1) && over(&s, -0.9, 9.0, -2.9));
    assert!(!over(&s, 1.1, 1.0, -3.0), "beside the leaf");
    assert!(!over(&s, 0.0, 1.0, -2.7), "in front of it");
    assert!(!over(&s, 10.0, 1.0, -3.0), "a body that is no leaf");
    s.doors[0].open = 1.0;
    assert!(!over(&s, 0.0, 1.0, -3.0), "the clip is at its end");
    s.doors[0].open = 0.5;
    s.units[0].alive = false;
    assert!(!over(&s, 0.0, 1.0, -3.0), "a dead unit holds no door");
    assert!(
        !over(&make(Vec3::new(0.0, 0.0, 1.5)), 0.0, 1.0, -3.0),
        "nobody in the trigger"
    );
}

/// A magnet held by slot 0 and the same magnet held by slot 1 are
/// different states: both slot tables are length-prefixed in the hash.
#[test]
fn magnet_slot_is_part_of_the_hash() {
    let mut a = Sim::empty(4);
    let mut b = Sim::empty(4);
    for s in [&mut a, &mut b] {
        s.heightfield = Some(flat_ground());
        s.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        s.spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
            .unwrap();
    }
    let t = a
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -30.0)))
        .unwrap();
    let tb = b
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -30.0)))
        .unwrap();
    assert_eq!(t, tb);
    assert_eq!(a.hash(), b.hash());
    let t = crate::sim::AimTarget::Unit(t);
    a.magnet[0] = Some(t);
    b.magnet[1] = Some(t);
    assert_ne!(a.hash(), b.hash(), "which slot holds the magnet is hashed");
    a.magnet[0] = None;
    a.magnet[1] = Some(t);
    assert_eq!(a.hash(), b.hash());
}

/// Phase D D1: the squad order is per-unit hashed state — and a unit with
/// NO order costs the hash nothing, which is the property the whole phase
/// leans on (brief addendum 3).
#[test]
fn a_squad_command_is_part_of_the_hash_and_no_command_costs_nothing() {
    use crate::squad::{Command, CommandKind};
    // The pinned literal below: `Sim::new(42)` stepped 100 idle ticks, which is
    // exactly what `game --sim-hash 100` runs. Measured at HEAD before this
    // commit (`phase-d` 1aa9651, and the same value at 96814de) and pinned
    // HERE so that adding a field to `Unit` can never move it unnoticed:
    // `Unit::hash_into` eats nothing at all for a unit with no command, so
    // a command-free sim hashes byte-for-byte as it did before Phase D.
    // If this fails, the hash FORMAT moved and every replay gate value with
    // it — that is a lead decision (PLAN B1), not a test to update.
    // Card STHOP (2026-09-30) moved it on purpose, 17e1b946ff0e821c ->
    // ba980b4058ec387f: `Soldier::hash_into` now eats the ground normal
    // and the airtime, state the next tick's motion depends on.
    // Card CONT (2026-10-02) moved it on purpose, ba980b4058ec387f ->
    // 4bf3989459aaec77: `TeamPlan::hash_into` now eats each slot's
    // generation and at-post clock and the posts' owners at the last
    // plan, the state a kept target's next draw depends on.
    // Card JITR (2026-10-02) moved it on purpose, 4bf3989459aaec77 ->
    // dc6414f430a49ed7: `AiState::hash_into` now eats the steering's
    // remembered slide side and the tick that memory ends.
    // Card AIMA (2026-10-02) moved it on purpose, dc6414f430a49ed7 ->
    // f5046a190fcf7eda: the state, not the format - a human spawns at
    // the PS2's pitch -0.07 (with a pitch of 0 the old value returns).
    // Card JGATE (2026-10-05) moved it on purpose, f5046a190fcf7eda ->
    // e20eb63fc9d796e6: `Soldier::hash_into` now eats the stand timer
    // (`stand_age`), which gates BF1's jump and its fall.
    // Card SPRD (2026-10-06) moved it on purpose, e20eb63fc9d796e6 ->
    // 76799f150370b936: the state, not the format - a weapon's spread
    // rests `tight_hold` under the PS2's floor (with both
    // `TIGHT_HOLD_SECS` at 0 the old value returns).
    // Card OBST (2026-10-06) moved it on purpose, 76799f150370b936 ->
    // 655566562afc1358: `AiState::hash_into` no longer eats the slide
    // memory card JITR added (`slide_last`, `slide_keep`: deleted, the
    // side is a commitment in `MoverState::round` now).
    // Card CLSE2 (2026-10-12) moved it on purpose, 655566562afc1358 ->
    // 9f5f669ed7142854: the format - a unit's hash eats its `AimBlock` (12
    // floats: BF2's aim error is the target's own block).
    // Card DEFS (2026-10-12) moved it on purpose, 9f5f669ed7142854 ->
    // 301ed5aeda7aae76: the format - a team's plan hashes its defenders'
    // spots and each post's last threat (`TeamPlan::spots`, `threat`).
    // Card SKIRT (2026-10-15) moved it on purpose, 301ed5aeda7aae76 ->
    // 5affffe147952dc0: the format - a soldier's hash eats what is left of
    // the floor overlap and whether last tick kept the flatter normal.
    // Card KNOCK (2026-10-21) moved it on purpose, 5affffe147952dc0 ->
    // f9f37400e0f3171e: the format - a soldier's hash eats its landing
    // draw and the get-up's fire lock (`knock_draw`, `fire_lock`).
    // Card DTEAM (2026-10-28) moved it on purpose, 06da5e043dd4c755 ->
    // 26e385b8a5a49460: the format - the scene is one human against none, so
    // the hash eats the per-team bot rows (`Sim::team_rows`).
    // 2026-10-29 moved it on purpose, 26e385b8a5a49460 -> 5573f2b603ed95f4:
    // every human's spawn tick is kept (`focus_born`, the untargeted second).
    let mut sim = Sim::new(42);
    for _ in 0..100 {
        sim.step(&[PadState::default()]);
    }
    assert_eq!(
        format!("{:016x}", sim.hash()),
        "5573f2b603ed95f4",
        "the command-free hash format moved"
    );

    let mut a = Sim::empty(4);
    let mut b = Sim::empty(4);
    for s in [&mut a, &mut b] {
        s.heightfield = Some(flat_ground());
        s.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        s.spawn(&UnitSpec::trooper(0, Vec3::new(5.0, 0.0, 0.0)))
            .unwrap();
    }
    assert_eq!(a.hash(), b.hash());
    let leader = a.players[0].expect("player");
    let cmd = |kind, issuer, issued_tick| {
        Some(Command {
            kind,
            issuer,
            issued_tick,
        })
    };
    a.units[1].command = cmd(CommandKind::Follow(leader), 0, 0);
    assert_ne!(a.hash(), b.hash(), "having an order is hashed");
    // ... and every field of it: the kind, the payload, the slot, the tick.
    b.units[1].command = cmd(CommandKind::Hold(Vec3::ZERO), 0, 0);
    assert_ne!(a.hash(), b.hash(), "the kind is hashed");
    b.units[1].command = cmd(CommandKind::Follow(leader), 1, 0);
    assert_ne!(a.hash(), b.hash(), "the issuer slot is hashed");
    b.units[1].command = cmd(CommandKind::Follow(leader), 0, 7);
    assert_ne!(a.hash(), b.hash(), "the issue tick is hashed");
    b.units[1].command = cmd(CommandKind::Follow(leader), 0, 0);
    assert_eq!(a.hash(), b.hash());
    // WHICH unit carries the order is hashed too (the order is not a
    // sim-wide fact): move it to the player and the sims differ.
    b.units[1].command = None;
    b.units[0].command = cmd(CommandKind::Follow(leader), 0, 0);
    assert_ne!(a.hash(), b.hash(), "whose order it is, is hashed");
}

#[test]
fn the_difficulty_and_every_bots_skill_are_part_of_the_hash() {
    // Phase C D2: `Sim::difficulty` is SIM STATE (one byte after `tick`)
    // and `Unit.skill` is per unit. Two sims that differ only in the tier
    // must not hash alike, and the skill must reach the unit at spawn.
    let build = |d: crate::skill::Difficulty| {
        let mut s = Sim::empty(4);
        s.difficulty = d;
        s.heightfield = Some(flat_ground());
        let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        let mut ai = UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -30.0));
        ai.controller = Controller::Ai;
        let a = s.spawn(&ai).unwrap();
        (s, p, a)
    };
    let (norm, p, a) = build(crate::skill::Difficulty::Normal);
    let (elite, _, _) = build(crate::skill::Difficulty::Elite);
    let (low, _, _) = build(crate::skill::Difficulty::Easy);
    assert_ne!(norm.hash(), elite.hash(), "the tier byte is hashed");
    assert_ne!(norm.hash(), low.hash(), "the tier byte is hashed");
    // Every bot receives the tier's skill; a player keeps 0.
    assert_eq!(norm.unit(a).unwrap().skill, 17);
    assert_eq!(elite.unit(a).unwrap().skill, 20);
    assert_eq!(low.unit(a).unwrap().skill, 14);
    assert_eq!(norm.unit(p).unwrap().skill, 0, "a player is not a bot");
    // The spec carries it too, so a rebuild starts from the same place.
    assert_eq!(norm.unit(a).unwrap().spec.skill, 17);
    // `Unit.skill` alone moves the hash (same tier byte, hand-edited).
    let mut hacked = build(crate::skill::Difficulty::Normal).0;
    hacked.units[a.index as usize].skill = 3;
    assert_ne!(hacked.hash(), norm.hash(), "Unit.skill is hashed");
}

/// Card DTEAM (the maintainer, 2026-10-28) [ours]: a human surplus lowers the
/// surplus team's bots at their next spawn, never a living bot; the other
/// team keeps the tier's row; equal humans change nothing; the per-team
/// rows reach the hash only when a surplus moves them.
#[test]
fn a_human_surplus_lowers_its_own_teams_bots_at_their_next_spawn() {
    use crate::skill::Difficulty as D;
    let bot = |team: u8| {
        let mut ai = UnitSpec::trooper(team, Vec3::new(f32::from(team) * 5.0, 0.0, 0.0));
        ai.controller = Controller::Ai;
        ai
    };
    let make = |extra: [u8; 2]| {
        let mut s = Sim::empty(4);
        s.heightfield = Some(flat_ground());
        s.difficulty = D::Hard;
        s.extra_humans = extra;
        s
    };
    // 0v0 and 1v1: both teams the Hard row, and the hash of a sim without
    // the rule.
    let even = make([1, 1]);
    assert_eq!(
        even.hash(),
        make([0, 0]).hash(),
        "even humans: no hash change"
    );
    assert_eq!(even.difficulty_of(0), D::Hard);
    assert_eq!(even.bot_skill(0), 20);
    // 1v0 at Hard: team 0's bots play the Normal row at 18, team 1 Hard.
    let mut s = make([1, 0]);
    assert_ne!(s.hash(), make([0, 0]).hash(), "a surplus is hashed");
    assert_eq!(s.hash(), make([2, 1]).hash(), "2v1 is 1v0");
    assert_ne!(make([3, 0]).hash(), make([2, 0]).hash());
    assert_eq!(make([3, 0]).hash(), make([4, 0]).hash(), "the cap");
    let a0 = s.spawn(&bot(0)).unwrap();
    let a1 = s.spawn(&bot(1)).unwrap();
    assert_eq!(s.unit(a0).unwrap().skill, 18);
    assert_eq!(s.unit(a1).unwrap().skill, 20, "the enemy keeps the row");
    assert_eq!(
        (s.difficulty_of(0), s.difficulty_of(1)),
        (D::Normal, D::Hard)
    );
    // Hard's extra (the 1.2 m error cap) is off on the surplus team only.
    assert!(s.difficulty_of(0).skill_at(18).err_cap > 1.2);
    assert_eq!(s.difficulty_of(1).skill_at(20).err_cap, 1.2);
    // Who shoots at team 1 is team 0's row; at team 0, team 1's.
    assert_eq!(s.enemy_skill_of(1).level, 18);
    assert_eq!(s.enemy_skill_of(0), D::Hard.skill());
    // A real human joins team 1: 1v1. The living bot keeps its 18; the
    // next bot spawned on team 0 takes the full row.
    let h = s
        .spawn(&UnitSpec::player(1, Vec3::new(9.0, 0.0, 0.0)))
        .unwrap();
    assert_eq!(s.humans_per_team(), [1, 1]);
    assert_eq!(s.unit(a0).unwrap().skill, 18, "no bot changes mid-life");
    let b0 = s.spawn(&bot(0)).unwrap();
    assert_eq!(s.unit(b0).unwrap().skill, 20);
    // He changes team (to 0): 2v0 counts him with the stand-in, team 0's
    // next bot drops to 16 on the Easy row.
    s.units[h.index as usize].team = 0;
    assert_eq!(s.humans_per_team(), [2, 0]);
    let c0 = s.spawn(&bot(0)).unwrap();
    assert_eq!(s.unit(c0).unwrap().skill, 16);
    assert_eq!(s.difficulty_of(0), D::Easy);
    // An arena override still wins over the rule.
    s.team_difficulty = [Some(D::Hard), None];
    assert_eq!((s.difficulty_of(0), s.bot_skill(0)), (D::Hard, 20));
}

#[test]
fn a_team_tier_reaches_that_teams_bots_and_only_then_moves_the_hash() {
    // Card ARVS: `team_difficulty` unset is the sim it was (same hash);
    // set, each team's bots take their own tier's skill at spawn.
    use crate::skill::Difficulty as D;
    let build = |td: [Option<D>; 2]| {
        let mut s = Sim::empty(4);
        s.team_difficulty = td;
        s.heightfield = Some(flat_ground());
        let mut hs = Vec::new();
        for team in [0u8, 1] {
            let mut ai = UnitSpec::trooper(team, Vec3::new(f32::from(team) * 5.0, 0.0, 0.0));
            ai.controller = Controller::Ai;
            hs.push(s.spawn(&ai).unwrap());
        }
        (s, hs)
    };
    let (plain, _) = build([None; 2]);
    let base = {
        let mut s = Sim::empty(4);
        s.heightfield = Some(flat_ground());
        for team in [0u8, 1] {
            let mut ai = UnitSpec::trooper(team, Vec3::new(f32::from(team) * 5.0, 0.0, 0.0));
            ai.controller = Controller::Ai;
            s.spawn(&ai).unwrap();
        }
        s
    };
    assert_eq!(plain.hash(), base.hash(), "no override: the old hash");
    let (split, hs) = build([Some(D::Easy), Some(D::Elite)]);
    assert_ne!(split.hash(), plain.hash(), "an override is hashed");
    assert_eq!(split.unit(hs[0]).unwrap().skill, 14, "team 0 at Easy");
    assert_eq!(split.unit(hs[1]).unwrap().skill, 20, "team 1 at Elite");
    assert_eq!(split.difficulty_of(0), D::Easy);
    assert_eq!(
        split.difficulty_of(2),
        D::Normal,
        "a third team: the round's"
    );
    assert_eq!(split.enemy_difficulty_of(0), D::Elite);
    assert_eq!(split.enemy_difficulty_of(1), D::Easy);
}

#[test]
fn a_bf1_map_has_no_view_cone_omni() {
    // RE §9.1: BF1's cone test has no 10 m omni branch, BF2's does.
    assert_eq!(crate::sim::omni_radius_for("bf1"), 0.0);
    assert_eq!(crate::sim::omni_radius_for("BF1"), 0.0);
    assert_eq!(crate::sim::omni_radius_for("bf2"), 10.0);
    // `omni_radius` is an INPUT, like `nav`: it must NOT move the hash.
    let mut a = Sim::empty(4);
    let mut b = Sim::empty(4);
    a.omni_radius = 0.0;
    b.omni_radius = 10.0;
    assert_eq!(a.hash(), b.hash(), "omni_radius is an input, not state");
    // So is the host-option gate.
    a.team_handicap_on = false;
    assert_eq!(a.hash(), b.hash(), "the host option is an input");
    assert_eq!(Sim::empty(1).difficulty, crate::skill::Difficulty::Normal);
    assert_eq!(Sim::empty(1).omni_radius, 10.0);
}

/// The wall pass's recorded push is part of the sim hash: the AI reads it
/// as its contact cue next tick, so a build that lost it would replay
/// differently and nothing else would say so (PLAN "PHASE A.2 DESIGN"
/// decision 2).
#[test]
fn wall_push_is_part_of_the_hash() {
    let mut a = Sim::empty(4);
    let mut b = Sim::empty(4);
    for s in [&mut a, &mut b] {
        s.heightfield = Some(flat_ground());
        s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    }
    assert_eq!(a.hash(), b.hash());
    a.units[0].wall_push = Vec3::new(0.25, 0.0, 0.0);
    assert_ne!(a.hash(), b.hash(), "the contact accumulator is hashed");
    b.units[0].wall_push = Vec3::new(0.25, 0.0, 0.0);
    assert_eq!(a.hash(), b.hash());
    // ... and the wall pass is what writes it: a unit walked into a wall
    // is pushed out AND records the push, with the push equal to the
    // correction its position actually took.
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    s.collision = CollisionWorld::new(
        vec![crate::collision::Body::new(
            glam::Mat4::from_translation(Vec3::new(0.2, 1.0, 0.0)),
            crate::collision::Shape::Box {
                half: Vec3::new(0.5, 5.0, 5.0),
            },
        )],
        Vec::new(),
    );
    let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let before = s.unit(h).unwrap().soldier.pos;
    s.step(&[PadState::default()]);
    let u = s.unit(h).unwrap();
    assert!(u.wall_push.length() > 0.0, "pushed out of the slab");
    assert_eq!(u.wall_push.y, 0.0, "the push is horizontal");
    let moved = u.soldier.pos - before;
    assert!(
        (Vec3::new(moved.x, 0.0, moved.z) - u.wall_push).length() < 1e-5,
        "records exactly the push it applied: {moved} vs {}",
        u.wall_push
    );
}

/// A box body is real ground: a Pacer walks its top at a constant y and
/// drops to the no-ground sentinel the moment it steps off the edge.
#[test]
fn unit_stands_on_a_box_and_walks_off_its_edge() {
    use crate::collision::{Body, CollisionWorld, Shape, NO_GROUND};
    let mut s = Sim::empty(5);
    // Box spanning x, z in (−5, 5) with its top at y = 6.
    s.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0)),
            Shape::Box {
                half: Vec3::new(5.0, 1.0, 5.0),
            },
        )],
        Vec::new(),
    );
    let h = s
        .spawn(&UnitSpec {
            controller: Controller::Pacer {
                vel: Vec3::new(2.0, 0.0, 0.0),
                period: 0,
            },
            ..UnitSpec::trooper(1, Vec3::new(0.0, 6.0, 0.0))
        })
        .unwrap();
    assert_eq!(
        s.units[h.index as usize].soldier.pos.y, 6.0,
        "spawned on it"
    );
    for _ in 0..60 {
        s.step(&[PadState::default()]);
        let p = s.units[h.index as usize].soldier.pos;
        assert!((p.y - 6.0).abs() < 1e-4, "still on the box at {p}");
    }
    // 2 m/s for 60 ticks = 4 m; keep walking until it is past x = 5.
    for _ in 0..30 {
        s.step(&[PadState::default()]);
    }
    let p = s.units[h.index as usize].soldier.pos;
    assert!(p.x > 5.0, "walked off the edge: {p}");
    assert_eq!(p.y, NO_GROUND, "nothing under it any more: {p}");
}

mod hash_history;

/// HTGTW: identical handle words in different target domains must not alias.
#[test]
fn htgtw_vehicle_magnet_hash_is_distinct_from_unit_and_none() {
    use crate::sim::AimTarget;
    use crate::vehicle::VehicleHandle;
    let mut s = Sim::empty(5);
    s.magnet = vec![None];
    let none = s.hash();
    s.magnet[0] = Some(AimTarget::Unit(UnitHandle { index: 7, gen: 11 }));
    let unit = s.hash();
    s.magnet[0] = Some(AimTarget::Vehicle(VehicleHandle { index: 7, gen: 11 }));
    let vehicle = s.hash();
    assert_ne!(vehicle, none);
    assert_ne!(vehicle, unit);
    assert_ne!(unit, none);
    for (index, gen) in [(8, 11), (7, 12)] {
        s.magnet[0] = Some(AimTarget::Vehicle(VehicleHandle { index, gen }));
        assert_ne!(
            s.hash(),
            vehicle,
            "vehicle index and generation both matter"
        );
    }
    s.magnet[0] = None;
    assert_eq!(s.hash(), none, "null control restores the original hash");
}

mod hash_bolts;

/// Card DTEAM with DPRX: a human proxy counts toward its team's humans, so
/// its team's bots take the surplus row as they would for a player.
#[test]
fn a_human_proxy_counts_as_its_teams_human() {
    let mut s = Sim::empty(1);
    let h = s
        .spawn(&crate::unit::UnitSpec::trooper(1, glam::Vec3::ZERO))
        .unwrap();
    assert_eq!(s.humans_per_team(), [0, 0]);
    s.units[h.index as usize].proxy_human = Some(crate::ai::proxy::Strength::Good.profile());
    assert_eq!(s.humans_per_team(), [0, 1]);
}

#[test]
fn shared_sprint_keeps_hero_energy_nonnegative_but_soldiers_take_debt() {
    use crate::sim::buttons;
    use crate::soldier::Stance;
    for hero in [false, true] {
        let mut sim = Sim::empty(42);
        sim.heightfield = Some(flat_ground());
        let h = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        if hero {
            sim.heroes = Some(crate::sim::heroes::Heroes::new([Some(0), None]));
        }
        assert_eq!(sim.is_hero(h), hero);
        let u = sim.unit_mut(h).unwrap();
        assert!(!u.consts.saber, "gun heroes must be isolated too");
        u.soldier.stance = Stance::Sprint;
        u.soldier.energy = 0.1;
        u.soldier.vel = Vec3::NEG_Z * 13.5;
        let sprint = PadState {
            ly: 32767,
            buttons: buttons::SPRINT,
            ..Default::default()
        };
        sim.step(&[sprint]);
        let u = sim.unit(h).unwrap();
        assert!(u.soldier.energy_depleted);
        if hero {
            assert_eq!(u.soldier.energy, 0.0);
            for _ in 0..180 {
                sim.step(&[sprint]);
                assert!(sim.unit(h).unwrap().soldier.energy >= 0.0);
            }
        } else {
            assert!(u.soldier.energy < 0.0);
        }
    }
}
