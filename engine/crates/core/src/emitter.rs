//! The emitter query law (`analysis/re-ordnance-kinds.md` §8): one instant
//! cone-and-chain hit per weapon shot, no projectile. The sim gathers the
//! bodies and hands the law a line-of-sight test; this module decides who a
//! shot touches, in what order and for how much.
//!
//! BF1 (the clone pilot's arc caster) and BF2 (the dark trooper's and the
//! Jawa's arc casters, the officer's incinerator, the hero flamethrowers)
//! differ in the cone test, the gather mask, the team filter and the hit-all
//! mode; [`EmitterOrdnance::bf2`] selects the rules.
//!
//! Deviations from the PS2 body, both reported in the card:
//! - the chain origin always moves to the struck body (the PS2 moves it only
//!   once a `LightningEffect` instance exists: after hit 1, then with p 0.7);
//! - a candidate is measured at its body point (the PS2's "body + 1 m"),
//!   not at the model origin under the feet;
//! - the `FirstRadius` ball is a 3-D sphere at the muzzle on the first link;
//!   the PS2's exact shape is not transcribed.

use crate::fmath::sin_cos;
use crate::weapon::EmitterOrdnance;
use glam::Vec3;

/// BF1's gather cap: at most this many bodies are taken from the box
/// (B1:198922-198940). BF2 has no transcribed cap of its own.
pub const BF1_GATHER_MAX: usize = 32;

/// BF2 widens the cone's half-angle by [`BF2_WIDEN`] radians for a body this
/// close to the chain origin (the RE's "widened to cos 0.1 within 2.5 m").
pub const BF2_WIDEN_RANGE: f32 = 2.5;
pub const BF2_WIDEN: f32 = 0.1;

/// One body the shot may touch, in the law's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// The body point the cone, the distances and the line of sight use: a
    /// soldier's chest (feet + 1 m) or a vehicle's hull point.
    pub pos: Vec3,
    /// The body's team; `None` = neutral (never an ally).
    pub team: Option<u8>,
    /// A vehicle hull: BF1's gather mask has none, BF2's does.
    pub vehicle: bool,
}

/// One body the shot touched: `index` into the caller's candidate slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub index: usize,
    /// The damage this link deals.
    pub damage: f32,
    /// The chain origin this link was tested from (the muzzle for the first
    /// link, the previous body afterwards): the way the link travelled.
    pub from: Vec3,
}

/// A shot's whole result.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    /// The root damage `D` the shot started from.
    pub damage: f32,
    /// The bodies it touched, in chain order (hit-all: nearest first).
    pub hits: Vec<Hit>,
}

/// Resolve one emitter shot.
///
/// `damage` is the class's charge-blended damage (the root `D`);
/// `floor_damage` is its base `Damage` key, the hit-all mode's per-body
/// floor. `los(from, to)` must be true when `to` is visible from `from`
/// (the sim passes [`crate::perception::los`]).
///
/// The query is selected by the CLASS, not by the damage: the PS2's shot
/// runs the cone whenever `ConeLength > 0` (`re-officer-abilities.md`'s
/// reading of `FUN_00315f78`), so a zero-damage class (the Force push /
/// pull family: `Damage` 0 with `Push` / `ConeLength` / `MaxTargets`
/// authored) still gathers and links its bodies. Its links deal 0 and the
/// sim turns them into pure shoves.
#[allow(clippy::too_many_arguments)]
pub fn query(
    e: &EmitterOrdnance,
    damage: f32,
    floor_damage: f32,
    origin: Vec3,
    aim: Vec3,
    team: u8,
    targets: &[Candidate],
    los: impl Fn(Vec3, Vec3) -> bool,
) -> Shot {
    let mut shot = Shot {
        damage,
        hits: Vec::new(),
    };
    let aim = aim.normalize_or_zero();
    if !e.is_cone() || aim == Vec3::ZERO {
        return shot;
    }
    // The gather: BF1's mask has soldiers, riders and shields only; BF2 adds
    // vehicles and filters by team.
    let mut gathered: Vec<usize> = targets
        .iter()
        .enumerate()
        .filter(|(_, c)| e.bf2 || !c.vehicle)
        .filter(|(_, c)| !e.bf2 || team_ok(e, team, c.team))
        .map(|(i, _)| i)
        .collect();
    if !e.bf2 {
        gathered.truncate(BF1_GATHER_MAX);
    }

    if e.hit_all {
        hit_all(
            e,
            damage,
            floor_damage,
            origin,
            aim,
            &gathered,
            targets,
            &mut shot,
            &los,
        );
        return shot;
    }

    let mut d = damage;
    let mut from = origin;
    let mut first = true;
    while (shot.hits.len() as u32) < e.max_targets {
        let limit = if first {
            e.cone_length
        } else {
            e.max_jump_distance
        };
        let mut best: Option<(usize, f32)> = None;
        for &i in &gathered {
            if shot.hits.iter().any(|h| h.index == i) {
                continue;
            }
            let c = targets[i].pos;
            let rel = c - from;
            let axial = rel.dot(aim);
            if axial <= 0.0 || axial > limit {
                continue;
            }
            let dist = rel.length();
            let radial = (rel - aim * axial).length();
            // FirstRadius: a ball at the muzzle the first link accepts
            // besides the cone.
            let near = first && dist <= e.first_radius;
            if !near && !in_cone(e, radial, axial, dist) {
                continue;
            }
            if !los(from, c) {
                continue;
            }
            // Nearest first: BF1 along the aim, BF2 by 3-D distance.
            let key = if e.bf2 { dist } else { axial };
            if best.is_none_or(|(_, k)| key < k) {
                best = Some((i, key));
            }
        }
        let Some((i, _)) = best else { break };
        shot.hits.push(Hit {
            index: i,
            damage: d,
            from,
        });
        d -= e.jump_deduction;
        from = targets[i].pos;
        first = false;
        if d < e.damage_threshold {
            break;
        }
    }
    shot
}

/// BF2's hit-all mode: every body in the cone takes `max(Damage, D/N)`; no
/// chain range, no jump deduction.
#[allow(clippy::too_many_arguments)]
fn hit_all(
    e: &EmitterOrdnance,
    damage: f32,
    floor_damage: f32,
    origin: Vec3,
    aim: Vec3,
    gathered: &[usize],
    targets: &[Candidate],
    shot: &mut Shot,
    los: &impl Fn(Vec3, Vec3) -> bool,
) {
    let mut valid: Vec<(usize, f32)> = Vec::new();
    for &i in gathered {
        let c = targets[i].pos;
        let rel = c - origin;
        let axial = rel.dot(aim);
        if axial <= 0.0 || axial > e.cone_length {
            continue;
        }
        let dist = rel.length();
        let radial = (rel - aim * axial).length();
        if !in_cone(e, radial, axial, dist) {
            continue;
        }
        if !los(origin, c) {
            continue;
        }
        valid.push((i, dist));
    }
    valid.sort_by(|a, b| a.1.total_cmp(&b.1));
    valid.truncate(e.max_targets as usize);
    let n = valid.len() as f32;
    if n <= 0.0 {
        return;
    }
    let each = floor_damage.max(damage / n);
    shot.hits.extend(valid.into_iter().map(|(index, _)| Hit {
        index,
        damage: each,
        from: origin,
    }));
}

/// The cone test at the body point. BF1: `radial / axial < sin(half)`;
/// BF2: a true cone, `radial / axial <= tan(half)`, widened near the origin.
fn in_cone(e: &EmitterOrdnance, radial: f32, axial: f32, dist: f32) -> bool {
    let half = 0.5 * e.cone_angle
        + if e.bf2 && dist <= BF2_WIDEN_RANGE {
            BF2_WIDEN
        } else {
            0.0
        };
    let (s, c) = sin_cos(half);
    if e.bf2 {
        // `radial <= axial * tan(half)`, multiplied through by cos so the
        // shared `sin_cos` value is all the trig the test needs.
        radial * c <= axial * s
    } else {
        radial < axial * s
    }
}

/// BF2's team filter: a body of the shooter's team needs `AffectFriends`,
/// anything else `AffectEnemies` (neutral vehicle included).
fn team_ok(e: &EmitterOrdnance, team: u8, target: Option<u8>) -> bool {
    if target == Some(team) {
        e.affect_friends
    } else {
        e.affect_enemies
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::weapon::DEG;

    pub(crate) fn bf1() -> EmitterOrdnance {
        EmitterOrdnance {
            cone_length: 30.0,
            cone_angle: 50.0 * DEG,
            max_targets: 4,
            jump_deduction: 75.0,
            damage_threshold: 75.0,
            max_jump_distance: 20.0,
            first_radius: 0.6,
            push: 0.0,
            lightning: Some("arccaster_lightning".into()),
            bolts: 1,
            affect_friends: true,
            affect_enemies: true,
            hit_all: false,
            radius: 0.0,
            bf2: false,
            effect: None,
            burn: Default::default(),
        }
    }

    pub(crate) fn bf2() -> EmitterOrdnance {
        EmitterOrdnance {
            cone_length: 30.0,
            cone_angle: 50.0 * DEG,
            max_targets: 5,
            jump_deduction: 5.0,
            damage_threshold: 300.0,
            max_jump_distance: 20.0,
            first_radius: 1.2,
            push: 3.0,
            lightning: Some("com_sfx_lightning".into()),
            bolts: 1,
            affect_friends: true,
            affect_enemies: true,
            hit_all: false,
            radius: 0.0,
            bf2: true,
            effect: None,
            burn: Default::default(),
        }
    }

    /// `com_weap_inf_flamethrower_ord`: Damage 5, MaxDamage 15, L 10, 45°.
    pub(crate) fn flame() -> EmitterOrdnance {
        EmitterOrdnance {
            cone_length: 10.0,
            cone_angle: 45.0 * DEG,
            max_targets: 10,
            jump_deduction: 0.0,
            damage_threshold: 0.0,
            max_jump_distance: 0.0,
            first_radius: 2.0,
            push: 0.0,
            lightning: Some("com_sfx_flamethrower".into()),
            bolts: 1,
            affect_friends: true,
            affect_enemies: true,
            hit_all: true,
            radius: 0.0,
            bf2: true,
            effect: None,
            burn: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{bf1, bf2, flame};
    use super::*;

    fn origin() -> Vec3 {
        Vec3::new(0.0, 1.46, 0.0)
    }

    fn body(x: f32, team: u8) -> Candidate {
        Candidate {
            pos: Vec3::new(x, 1.0, 0.0),
            team: Some(team),
            vehicle: false,
        }
    }

    fn clear(_: Vec3, _: Vec3) -> bool {
        true
    }

    fn indices(shot: &Shot) -> Vec<usize> {
        shot.hits.iter().map(|h| h.index).collect()
    }

    fn damages(shot: &Shot) -> Vec<f32> {
        shot.hits.iter().map(|h| h.damage).collect()
    }

    #[test]
    fn bf1_chain_length_follows_the_charge() {
        let e = bf1();
        let targets = [body(6.0, 2), body(9.0, 2), body(12.0, 2)];
        let count = |charge: f32| {
            query(
                &e,
                100.0 + 150.0 * charge,
                100.0,
                origin(),
                Vec3::X,
                1,
                &targets,
                clear,
            )
            .hits
            .len()
        };
        assert_eq!(count(0.0), 1);
        assert_eq!(count(0.3), 1, "below charge 1/3");
        assert_eq!(count(0.4), 2, "from 1/3");
        assert_eq!(count(0.8), 2, "below charge 5/6");
        assert_eq!(count(0.9), 3, "above 5/6");
        assert_eq!(count(1.0), 3);
    }

    #[test]
    fn bf1_full_charge_deals_250_175_100_down_a_line() {
        let e = bf1();
        let targets = [body(6.0, 2), body(9.0, 2), body(12.0, 2)];
        let shot = query(&e, 250.0, 100.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(indices(&shot), vec![0, 1, 2]);
        assert_eq!(damages(&shot), vec![250.0, 175.0, 100.0]);
    }

    #[test]
    fn a_blocked_link_is_skipped_and_the_chain_goes_on() {
        let e = bf1();
        // The middle body (9 m) is behind a wall; the third at (15, 3.5) is
        // in the cone from the first body and clear of it.
        let targets = [
            body(6.0, 2),
            body(9.0, 2),
            Candidate {
                pos: Vec3::new(15.0, 1.0, 3.5),
                team: Some(2),
                vehicle: false,
            },
        ];
        let los = |_: Vec3, to: Vec3| !(8.0 < to.x && to.x < 10.0);
        let shot = query(&e, 250.0, 100.0, origin(), Vec3::X, 1, &targets, los);
        assert_eq!(indices(&shot), vec![0, 2]);
        assert_eq!(damages(&shot), vec![250.0, 175.0]);
    }

    #[test]
    fn bf1_has_no_team_filter() {
        let e = bf1();
        let targets = [body(6.0, 1), body(9.0, 1)];
        let shot = query(&e, 250.0, 100.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(indices(&shot), vec![0, 1]);
    }

    #[test]
    fn bf2_hit_all_splits_the_shot_between_the_bodies() {
        let e = flame();
        let targets = [body(3.0, 2), body(4.0, 2)];
        let shot = query(&e, 15.0, 5.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(indices(&shot), vec![0, 1]);
        assert_eq!(damages(&shot), vec![7.5, 7.5]);
    }

    #[test]
    fn bf2_hit_all_never_goes_under_its_damage_key() {
        let e = flame();
        let targets = [body(3.0, 2), body(4.0, 2), body(5.0, 2)];
        let shot = query(&e, 15.0, 5.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(damages(&shot), vec![5.0, 5.0, 5.0]);
    }

    #[test]
    fn bf2_affect_enemies_false_skips_enemies() {
        let mut e = bf2();
        e.affect_enemies = false;
        // The enemy is nearest; the ally behind it is the only body taken.
        let targets = [body(6.0, 2), body(9.0, 1)];
        let shot = query(&e, 300.0, 300.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(indices(&shot), vec![1]);
    }

    #[test]
    fn bf2_affect_friends_false_skips_allies() {
        let mut e = bf2();
        e.affect_friends = false;
        let targets = [body(6.0, 1), body(9.0, 2)];
        let shot = query(&e, 300.0, 300.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(indices(&shot), vec![1]);
    }

    #[test]
    fn a_vehicle_is_a_candidate_on_bf2_only() {
        let targets = [Candidate {
            pos: Vec3::new(6.0, 1.0, 0.0),
            team: Some(2),
            vehicle: true,
        }];
        assert!(
            query(&bf1(), 250.0, 100.0, origin(), Vec3::X, 1, &targets, clear)
                .hits
                .is_empty()
        );
        let shot = query(&bf2(), 300.0, 300.0, origin(), Vec3::X, 1, &targets, clear);
        assert_eq!(indices(&shot), vec![0]);
        assert_eq!(damages(&shot), vec![300.0]);
    }

    #[test]
    fn a_radius_class_does_not_query() {
        let mut e = bf2();
        e.cone_length = 0.0;
        let targets = [body(3.0, 2)];
        assert!(
            query(&e, 300.0, 300.0, origin(), Vec3::X, 1, &targets, clear)
                .hits
                .is_empty()
        );
    }
}
