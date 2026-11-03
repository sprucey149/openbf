//! Card HYST: a bot thinks at once when something happens to it, not at its
//! next scheduled think (`analysis/bot-design.md` 9 D21: actions answer
//! events as they come; a bot hit just after a think stood up to a think
//! period without answering). Three events wake it: a hit, an enemy bolt
//! passing within [`NEAR_MISS`] of its body, and an enemy it does not know
//! of coming into its sight within [`SIGHT`]. The think period still paces
//! everything else (the travel decisions); the tier's first-shot delay
//! (`skill::Tier::reaction_ticks`) stays the reaction handicap, since a woken
//! think only sees sooner. A bot is woken at most every [`GAP`] ticks.
//!
//! OURS (D21, `analysis/bot-design.md` 9): the PS2's BF2 bot thinks at the
//! next step on a threat entry for a HUMAN unit only (re-ai-think-rate 4);
//! BF1 has no such rule.
//!
//! Deterministic: every test reads the sim as it stood after the last step.

use super::state::AiState;
use crate::perception;
use crate::sim::Sim;
use crate::unit::{Unit, UnitHandle};

/// The events, one bit each (`AiState::wake`, `NavTick::wake`).
pub const HIT: u8 = 1 << 0;
pub const NEAR: u8 = 1 << 1;
pub const SEEN: u8 = 1 << 2;

/// A bolt this near the bot's body axis is a near miss (m).
pub const NEAR_MISS: f32 = 2.0;
/// An enemy this near is looked for every tick (m): beyond it, the think's
/// own scan (a ray per enemy in view each tick would cost ten times
/// perception's share).
pub const SIGHT: f32 = 20.0;
/// At most one early think this often (ticks): 0.1 s, the decision time a
/// study of Quake bots judged most human (`modern.md` B3).
pub const GAP: u64 = 3;

/// What has happened to bot `me` since its last think that wakes it.
pub(super) fn events(sim: &Sim, me: UnitHandle, u: &Unit, st: &AiState) -> u8 {
    let mut bits = 0;
    if u.last_hit.is_some_and(|(_, t)| t >= st.last_think) {
        bits |= HIT;
    }
    if u.mount.is_some() {
        return bits;
    }
    if near_miss(sim, me, u) {
        bits |= NEAR;
    }
    if new_in_sight(sim, me, u, st) {
        bits |= SEEN;
    }
    bits
}

/// An enemy bolt's last step passed within [`NEAR_MISS`] of `u`'s body (the
/// segment from its feet to its eye).
fn near_miss(sim: &Sim, me: UnitHandle, u: &Unit) -> bool {
    let (feet, eye) = (u.soldier.pos, u.muzzle());
    sim.bolts.iter().any(|b| {
        if !sim.hostile(u.team, b.team) || b.owner == me {
            return false;
        }
        let from = b.pos - b.vel * crate::TICK_DT;
        seg_seg_dist(from, b.pos, feet, eye) < NEAR_MISS
    })
}

/// An enemy within [`SIGHT`], in `u`'s view and in the line of its eye, that
/// it holds no record of (perception's own tests, `perception::scan`).
fn new_in_sight(sim: &Sim, me: UnitHandle, u: &Unit, st: &AiState) -> bool {
    if st.contacts.iter().all(|c| c.is_some()) {
        return false;
    }
    let tier = sim.unit_tier(u);
    let eye = u.muzzle();
    let view = u.aim_frame().0;
    let mut near = crate::ai::team::NearList::new();
    sim.units_within(eye, SIGHT, &mut near);
    near.iter().any(|&i| {
        let (h, o) = (sim.handle(i as usize), &sim.units[i as usize]);
        if !sim.hostile(u.team, o.team) || !o.alive || perception::contact_of(st, h).is_some() {
            return false;
        }
        let p = o.capsule().center();
        let off = p - eye;
        let d2 = off.dot(off);
        let vd = perception::view_distance(o.soldier.stance, tier.view_mult).min(SIGHT);
        d2 < vd * vd
            && perception::in_cone(view, off, d2, sim.omni_radius)
            && perception::los_seat(
                sim,
                eye,
                p,
                perception::past_mounts(sim, me, h),
                perception::past_buildings(sim, me, h),
            )
    })
}

/// The least distance between segments `p0 p1` and `q0 q1`.
fn seg_seg_dist(p0: glam::Vec3, p1: glam::Vec3, q0: glam::Vec3, q1: glam::Vec3) -> f32 {
    let (d1, d2, r) = (p1 - p0, q1 - q0, p0 - q0);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t) = if a <= 1e-9 && e <= 1e-9 {
        (0.0, 0.0)
    } else if a <= 1e-9 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-9 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let den = a * e - b * b;
            let mut s = if den > 1e-9 {
                ((b * f - c * e) / den).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p0 + d1 * s).distance(q0 + d2 * t)
}

#[cfg(test)]
mod tests;
