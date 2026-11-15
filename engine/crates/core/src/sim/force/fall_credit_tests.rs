//! Issue #173, the maintainer's rule (`sim/fall_credit.rs`): a death by a fall
//! after an attacker's choke, push, pull, shove or blast is that attacker's
//! kill, under the damage path's friendly-fire rule.
use super::*;
use crate::sim::{DeathCause, ForcedMove};
use crate::soldier::Stance;
use crate::weapon::ExplosionClass;

/// What the death of `victim` was credited to, scanning this tick's events:
/// `Some(Some(k))` a kill by `k`, `Some(None)` the world's death.
fn death_of(sim: &Sim, victim: UnitHandle) -> Option<Option<UnitHandle>> {
    sim.events.iter().find_map(|e| match *e {
        Event::Kill { shooter, victim: v } if v == victim => Some(Some(shooter)),
        Event::EnvDeath { unit, .. } if unit == victim => Some(None),
        _ => None,
    })
}

/// Step until `victim` dies (or `limit` ticks pass) and return the credit.
fn run_to_death(sim: &mut Sim, victim: UnitHandle, limit: usize) -> Option<Option<UnitHandle>> {
    let pads = vec![PadState::default(); sim.units.len()];
    for _ in 0..limit {
        sim.step(&pads);
        if let Some(credit) = death_of(sim, victim) {
            return Some(credit);
        }
    }
    None
}

/// Lift a soldier `height` metres straight up, falling (a stand-in for a
/// ledge: the test ground is flat).
fn lift(sim: &mut Sim, h: UnitHandle, height: f32) {
    let s = &mut sim.unit_mut(h).unwrap().soldier;
    s.pos.y += height;
    s.grounded = false;
    if !s.stance.knocked() {
        s.stance = Stance::Fall;
        s.time_in_stance = 0.0;
    }
}

fn choke_scene() -> (Sim, UnitHandle) {
    let (mut sim, owner) = scene(emitter());
    let power = sim.weapon_classes[0].force.as_mut().unwrap();
    power.kind = ForceKind::Choke {
        angle: 10.0 * crate::weapon::DEG,
        damage_per_second: 150.0,
        height: 4.0,
        falloff: 1.0,
        drain_rate: 15.0,
    };
    power.cone_cos = crate::fmath::sin_cos(10.0 * crate::weapon::DEG).1;
    power.max_targets = 1;
    (sim, owner)
}

/// A soldier caught in the air high up is held there; when the hold lets go
/// he falls, and the lethal landing is the caster's kill.
#[test]
fn a_fall_after_a_choke_held_in_the_air_is_the_casters_kill() {
    let (mut sim, owner) = choke_scene();
    let target = victim(&mut sim, 8.0, 0.0);
    {
        let s = &mut sim.unit_mut(target).unwrap().soldier;
        s.stance = Stance::Fall;
        s.grounded = false;
        s.pos.y += 1.2;
    }
    sim.force_input(owner.index as usize, None, true, false);
    assert!(sim.unit(target).unwrap().choked.unwrap().hold.is_some());
    // Raise the held point: the hold keeps him where it caught him.
    {
        let u = sim.unit_mut(target).unwrap();
        u.soldier.pos.y += 80.0;
        u.choked.as_mut().unwrap().hold = Some(u.soldier.pos);
    }
    assert_eq!(run_to_death(&mut sim, target, 900), Some(Some(owner)));
    assert_eq!(sim.unit(owner).unwrap().kills, 1);
}

/// A choked jet trooper takes no landing damage (his class's
/// `CollisionScale` is 0), so his fall kills only over a pit: the kill floor
/// under a high hold is the caster's kill too.
#[test]
fn a_choked_jet_trooper_dropped_past_the_kill_floor_is_the_casters_kill() {
    let (mut sim, owner) = choke_scene();
    let target = victim(&mut sim, 8.0, 0.0);
    {
        let u = sim.unit_mut(target).unwrap();
        u.consts.collision_damage_immune = true;
        u.soldier.stance = Stance::Hover;
        u.soldier.grounded = false;
        u.soldier.pos.y += 1.2;
        u.soldier.fuel = 1.0;
    }
    sim.force_input(owner.index as usize, None, true, false);
    assert_eq!(
        sim.unit(target).unwrap().soldier.fuel,
        0.0,
        "the jet is cut"
    );
    // Over a pit: the floor is far below and the kill floor between.
    sim.heightfield = None;
    sim.kill_floor = Some(-20.0);
    assert_eq!(run_to_death(&mut sim, target, 900), Some(Some(owner)));
}

/// A Force Push and a Force Pull both credit the fall that follows them.
#[test]
fn a_fall_after_a_push_or_a_pull_is_the_casters_kill() {
    for strength in [30.0, -30.0] {
        let (mut sim, owner) = scene(emitter());
        sim.weapon_classes[0].force.as_mut().unwrap().push = strength;
        let target = victim(&mut sim, 3.0, 0.0);
        sim.fire_force(owner.index as usize, 0, 1.0);
        assert!(sim.unit(target).unwrap().soldier.stance.knocked());
        assert_eq!(
            sim.unit(target).unwrap().forced_by.map(|f| f.by),
            Some(owner),
            "strength {strength}"
        );
        lift(&mut sim, target, 80.0);
        assert_eq!(
            run_to_death(&mut sim, target, 900),
            Some(Some(owner)),
            "strength {strength}"
        );
    }
}

fn throwing_blast() -> ExplosionClass {
    ExplosionClass {
        push: 13.0,
        push_outer: 7.0,
        ..Default::default()
    }
}

/// A grenade's or rocket's throw credits the fall that follows it to the
/// thrower; his own throw stays the world's death.
#[test]
fn a_fall_after_a_blast_is_the_throwers_kill_but_not_after_his_own() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    let at = sim.unit(target).unwrap().soldier.pos - Vec3::new(1.0, 0.0, 0.0);
    sim.explode(&throwing_blast(), at, Some(owner), Some(0));
    assert!(sim.unit(target).unwrap().soldier.stance.knocked());
    lift(&mut sim, target, 80.0);
    assert_eq!(run_to_death(&mut sim, target, 900), Some(Some(owner)));

    // His own blast: no credit, as before.
    let (mut sim, _) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    let at = sim.unit(target).unwrap().soldier.pos - Vec3::new(1.0, 0.0, 0.0);
    sim.explode(&throwing_blast(), at, Some(target), Some(1));
    assert!(sim.unit(target).unwrap().soldier.stance.knocked());
    assert!(sim.unit(target).unwrap().forced_by.is_none());
    lift(&mut sim, target, 80.0);
    assert_eq!(run_to_death(&mut sim, target, 900), Some(None));
}

/// Back on his feet, the push no longer counts: a later fall of his own is
/// the world's death.
#[test]
fn a_soldier_back_on_his_feet_falls_on_his_own_account() {
    let (mut sim, owner) = scene(emitter());
    sim.weapon_classes[0].force.as_mut().unwrap().push = 30.0;
    let target = victim(&mut sim, 3.0, 0.0);
    sim.fire_force(owner.index as usize, 0, 1.0);
    assert!(sim.unit(target).unwrap().forced_by.is_some());
    let pads = vec![PadState::default(); sim.units.len()];
    for _ in 0..(6 * crate::TICK_HZ as usize) {
        sim.step(&pads);
    }
    let u = sim.unit(target).unwrap();
    assert!(u.alive && !u.soldier.stance.knocked(), "up again");
    assert!(u.forced_by.is_none(), "the push stopped counting");
    lift(&mut sim, target, 80.0);
    assert_eq!(run_to_death(&mut sim, target, 900), Some(None));
}

/// The window ends after `FORCED_MAX` even if he never stood up.
#[test]
fn a_forced_move_counts_for_at_most_its_window() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    sim.tick = 10_000;
    let max = (crate::sim::fall_credit::FORCED_MAX * crate::TICK_HZ as f32) as u64;
    sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick - max - 1));
    sim.kill_environment(target, DeathCause::Fall);
    assert_eq!(death_of(&sim, target), Some(None));

    let target = victim(&mut sim, 4.0, 0.0);
    sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick - max));
    sim.kill_environment(target, DeathCause::Fall);
    assert_eq!(death_of(&sim, target), Some(Some(owner)));
}

/// Only the falls are credited: the kill floor, a death region and a landing
/// under the map are; a pause-menu respawn is not.
#[test]
fn only_the_fall_deaths_are_credited() {
    for (cause, credited) in [
        (DeathCause::Fall, true),
        (DeathCause::KillFloor, true),
        (DeathCause::DeathRegion, true),
        (DeathCause::UnderMap, true),
        (DeathCause::Suicide, false),
        (DeathCause::Stalled, false),
    ] {
        let (mut sim, owner) = scene(emitter());
        let target = victim(&mut sim, 3.0, 0.0);
        sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick));
        sim.kill_environment(target, cause);
        let want = if credited {
            Some(Some(owner))
        } else {
            Some(None)
        };
        assert_eq!(death_of(&sim, target), want, "{cause:?}");
        assert!(
            sim.units[target.index as usize].forced_by.is_none(),
            "{cause:?}"
        );
    }
}

/// A player's blast that throws a bot of his own team to its death is a team
/// kill: friendly fire would have hurt that bot.
#[test]
fn a_friends_throw_to_a_fall_is_a_team_kill() {
    let (mut sim, owner) = scene(emitter());
    let y = sim.units[0].muzzle().y - 1.0;
    let friend = sim
        .spawn(&crate::unit::UnitSpec::trooper(0, Vec3::new(0.0, y, -3.0)))
        .unwrap();
    let at = sim.unit(friend).unwrap().soldier.pos - Vec3::new(1.0, 0.0, 0.0);
    sim.explode(&throwing_blast(), at, Some(owner), Some(0));
    assert!(sim.unit(friend).unwrap().soldier.stance.knocked());
    lift(&mut sim, friend, 80.0);
    let team_kills = sim.nav_stats.team_kills;
    assert_eq!(run_to_death(&mut sim, friend, 900), Some(Some(owner)));
    assert_eq!(sim.nav_stats.team_kills, team_kills + 1);
}

/// The grace counts from the first tick he is steady again, not from the
/// throw: on that tick the credit holds, and it lapses `FORCED_GRACE` later.
#[test]
fn the_grace_counts_from_his_first_steady_tick() {
    let (mut sim, owner) = scene(emitter());
    sim.weapon_classes[0].force.as_mut().unwrap().push = 30.0;
    let target = victim(&mut sim, 3.0, 0.0);
    sim.fire_force(owner.index as usize, 0, 1.0);
    assert!(sim.unit(target).unwrap().soldier.stance.knocked());
    let grace = (crate::sim::FORCED_GRACE * crate::TICK_HZ as f32) as u64;
    let pads = vec![PadState::default(); sim.units.len()];
    let mut steady_at = None;
    for _ in 0..(8 * crate::TICK_HZ as usize) {
        sim.step(&pads);
        let u = sim.unit(target).unwrap();
        let s = &u.soldier;
        if s.grounded && !s.stance.knocked() && !s.stance.airborne() {
            steady_at = Some(sim.tick);
            break;
        }
        assert!(
            u.forced_by.is_some(),
            "held while knocked, tick {}",
            sim.tick
        );
    }
    let steady_at = steady_at.expect("he got up");
    assert!(
        steady_at > grace,
        "the knock outlasts the grace, so a grace from the throw would be over"
    );
    assert_eq!(
        sim.unit(target).unwrap().forced_by.map(|f| f.by),
        Some(owner),
        "still counts on his first tick on his feet"
    );
    while sim.tick < steady_at + grace - 1 {
        sim.step(&pads);
        assert!(
            sim.unit(target).unwrap().forced_by.is_some(),
            "tick {}",
            sim.tick
        );
    }
    sim.step(&pads);
    assert!(
        sim.unit(target).unwrap().forced_by.is_none(),
        "lapsed after the grace"
    );
}

/// A bot's grenade cannot hurt a human of its own team, so its throw of him
/// into a fall stays the world's death, not a team kill.
#[test]
fn a_bot_allys_throw_of_a_human_is_not_credited() {
    let (mut sim, player) = scene(emitter());
    let y = sim.units[0].muzzle().y - 1.0;
    let bot = sim
        .spawn(&crate::unit::UnitSpec::trooper(0, Vec3::new(0.0, y, -6.0)))
        .unwrap();
    let health = sim.unit(player).unwrap().health.cur;
    let at = sim.unit(player).unwrap().soldier.pos - Vec3::new(1.0, 0.0, 0.0);
    sim.explode(&throwing_blast(), at, Some(bot), Some(0));
    let u = sim.unit(player).unwrap();
    assert!(u.soldier.stance.knocked(), "the push still lands");
    assert_eq!(u.health.cur, health, "the bot's blast spares him");
    assert!(u.forced_by.is_none());
    lift(&mut sim, player, 80.0);
    let team_kills = sim.nav_stats.team_kills;
    assert_eq!(run_to_death(&mut sim, player, 900), Some(None));
    assert_eq!(sim.nav_stats.team_kills, team_kills);
}

/// With friendly fire off a friend's throw is never credited.
#[test]
fn friendly_fire_off_credits_no_friend() {
    let (mut sim, owner) = scene(emitter());
    sim.friendly_fire_scale = 0.0;
    let y = sim.units[0].muzzle().y - 1.0;
    let friend = sim
        .spawn(&crate::unit::UnitSpec::trooper(0, Vec3::new(0.0, y, -3.0)))
        .unwrap();
    let at = sim.unit(friend).unwrap().soldier.pos - Vec3::new(1.0, 0.0, 0.0);
    sim.explode(&throwing_blast(), at, Some(owner), Some(0));
    assert!(sim.unit(friend).unwrap().soldier.stance.knocked());
    assert!(sim.unit(friend).unwrap().forced_by.is_none());
}

/// A move that leaves him standing (no knock, on the ground, not held)
/// records nothing.
#[test]
fn a_move_that_leaves_him_standing_records_nothing() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    let s = &sim.unit(target).unwrap().soldier;
    assert!(s.grounded && !s.stance.knocked());
    sim.note_forced_move(target, Some(owner));
    assert!(sim.unit(target).unwrap().forced_by.is_none());
}

/// A jet trooper flying his own jets is back in control: the grace runs and
/// the credit lapses, rather than holding for the whole window.
#[test]
fn a_jet_trooper_flying_himself_is_back_in_control() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    {
        let u = sim.unit_mut(target).unwrap();
        u.soldier.stance = Stance::Hover;
        u.soldier.grounded = false;
    }
    sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick));
    let grace = (crate::sim::FORCED_GRACE * crate::TICK_HZ as f32) as u64;
    let start = sim.tick;
    for _ in 0..grace {
        sim.expire_forced_moves();
        assert!(
            sim.unit(target).unwrap().forced_by.is_some(),
            "tick {}",
            sim.tick
        );
        sim.tick += 1;
    }
    assert_eq!(sim.tick, start + grace);
    sim.expire_forced_moves();
    assert!(sim.unit(target).unwrap().forced_by.is_none());
}

/// A credited attacker whose life is gone (his slot now holds a new
/// generation) is credited with nothing, and the record is dropped.
#[test]
fn a_stale_attacker_is_not_credited() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick));
    sim.units[owner.index as usize].gen += 1;
    sim.kill_environment(target, DeathCause::Fall);
    assert_eq!(death_of(&sim, target), Some(None));

    let target = victim(&mut sim, 4.0, 0.0);
    sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick));
    sim.expire_forced_moves();
    assert!(sim.unit(target).unwrap().forced_by.is_none());
}

/// A soldier who rides a vehicle is back in control: the record goes.
#[test]
fn boarding_a_vehicle_drops_the_record() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    {
        let u = sim.unit_mut(target).unwrap();
        u.soldier.stance = Stance::Thrown;
        u.forced_by = Some(ForcedMove::new(owner, 0));
        u.mount = Some(crate::vehicle::Mount {
            vehicle: crate::vehicle::VehicleHandle { index: 0, gen: 0 },
            seat: 0,
        });
    }
    sim.expire_forced_moves();
    assert!(sim.unit(target).unwrap().forced_by.is_none());
}

/// A credited death keeps its cause: the kill, then `ForcedDeath`.
#[test]
fn a_credited_death_keeps_its_cause() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    sim.unit_mut(target).unwrap().forced_by = Some(ForcedMove::new(owner, sim.tick));
    sim.events.clear();
    sim.kill_environment(target, DeathCause::KillFloor);
    let kill = sim.events.iter().position(
        |e| matches!(e, Event::Kill { shooter, victim } if *shooter == owner && *victim == target),
    );
    let cause = sim.events.iter().position(|e| {
        matches!(e, Event::ForcedDeath { unit, cause: DeathCause::KillFloor } if *unit == target)
    });
    assert!(
        kill.is_some() && cause.is_some() && kill < cause,
        "{:?}",
        sim.events
    );
    assert!(!sim
        .events
        .iter()
        .any(|e| matches!(e, Event::EnvDeath { unit, .. } if *unit == target)));
}

/// A saber shove that throws him credits the fall that follows.
#[test]
fn a_fall_after_a_saber_shove_is_the_attackers_kill() {
    let (mut sim, owner) = scene(emitter());
    let target = victim(&mut sim, 3.0, 0.0);
    sim.hero_knock(target, Vec3::new(0.0, 0.3, -1.0).normalize(), 30.0, owner);
    assert!(sim.unit(target).unwrap().soldier.stance.knocked());
    assert_eq!(
        sim.unit(target).unwrap().forced_by.map(|f| f.by),
        Some(owner)
    );
    lift(&mut sim, target, 80.0);
    assert_eq!(run_to_death(&mut sim, target, 900), Some(Some(owner)));
}

/// A choke's release restamps the record, so the window counts from the
/// drop, not from the catch.
#[test]
fn a_chokes_release_restamps_the_window() {
    let (mut sim, owner) = choke_scene();
    let target = victim(&mut sim, 8.0, 0.0);
    sim.force_input(owner.index as usize, None, true, false);
    assert!(sim.unit(target).unwrap().choked.is_some());
    let caught = sim.unit(target).unwrap().forced_by.expect("recorded").at;
    let pads = vec![PadState::default(); sim.units.len()];
    let mut released = None;
    for _ in 0..(3 * crate::TICK_HZ as usize) {
        let before = sim.tick;
        sim.step(&pads);
        if sim.unit(target).unwrap().choked.is_none() {
            released = Some(before);
            break;
        }
    }
    let released = released.expect("the hold let go");
    let f = sim
        .unit(target)
        .unwrap()
        .forced_by
        .expect("still counts at the drop");
    assert_eq!(f.by, owner);
    assert!(
        f.at >= released && f.at > caught,
        "restamped: {f:?} caught {caught}"
    );
}
