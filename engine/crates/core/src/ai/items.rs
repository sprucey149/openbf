//! BF2 bots' pickup detour: the scan, need checks, the walk past the item and
//! back, and deterministic state. Follows `analysis/re-powerups.md` §6 (PS2) and
//! §12 (the port's own gates, card B2ITEM).

use super::state::AiState;
use crate::fmath::FloatExt;
use crate::perception;
use crate::sim::Sim;
use crate::soldier::GroundRules;
use crate::unit::Unit;
use glam::Vec3;

/// The scan every 2.0 s (`analysis/re-powerups.md` §6, BF2 `FUN_002bd628`).
const PERIOD: u64 = 2 * crate::TICK_HZ as u64;
/// 4.0 s more after a hit (§6).
const FOUND_DELAY: u64 = 4 * crate::TICK_HZ as u64;
/// Items within 6 m (§6, `FUN_002be050(6.0)`).
const RADIUS: f32 = 6.0;
/// The PS2's 7 m level-of-detail step (§6); gating the detour by it, with LOS
/// beyond, is OURS (§12). PROVISIONAL.
const HUMAN_NEAR: f32 = 7.0;
/// The PS2's 20 m level-of-detail step (§6); the hard cut is OURS (§12).
/// PROVISIONAL.
const HUMAN_FAR: f32 = 20.0;
/// OURS: arrival distance at a detour point. PROVISIONAL (not in re-powerups.md).
const WAYPOINT_REACH: f32 = 0.3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemSeek {
    pub next_scan: u64,
    pub detour: Option<ItemDetour>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDetour {
    pub origin: Vec3,
    pub item: Vec3,
    pub past: Vec3,
    pub class: u8,
    pub returning: bool,
    pub until: u64,
}

impl ItemSeek {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if *self == Self::default() {
            return;
        }
        eat(b"ITEM");
        eat(&self.next_scan.to_le_bytes());
        eat(&[u8::from(self.detour.is_some())]);
        if let Some(d) = self.detour {
            eat(&[d.class, u8::from(d.returning)]);
            eat(&d.until.to_le_bytes());
            for p in [d.origin, d.item, d.past] {
                for f in p.to_array() {
                    eat(&f.hash_bits().to_le_bytes());
                }
            }
        }
    }
}

pub(super) fn goal(sim: &Sim, u: &Unit, st: &mut AiState) -> Option<Vec3> {
    if sim.ground_rules != GroundRules::Bf2 {
        return None;
    }
    if u.mount.is_some()
        || u.droideka.is_some()
        || st.lost.is_some_and(|l| l.confirmed)
        || st.ride.is_some()
        || u.command.is_some()
    {
        st.items.detour = None;
        return None;
    }
    let pos = u.soldier.pos;
    if let Some(mut d) = st.items.detour {
        if sim.tick >= d.until {
            st.items.detour = None;
            return None;
        }
        if !d.returning {
            let remains = sim.pickups.iter().any(|p| {
                p.class == d.class
                    && p.pos.distance_squared(d.item) < 0.25
                    && sim
                        .item_classes
                        .get(usize::from(p.class))
                        .is_some_and(|c| sim.item_needed(u, c))
            });
            if !remains || crate::nav::xz_dist(pos, d.past) <= WAYPOINT_REACH {
                d.returning = true;
            }
        }
        if d.returning && crate::nav::xz_dist(pos, d.origin) <= WAYPOINT_REACH {
            st.items.detour = None;
            return None;
        }
        st.items.detour = Some(d);
        return Some(if d.returning { d.origin } else { d.past });
    }
    if sim.tick < st.items.next_scan {
        return None;
    }
    let nearby_human = sim
        .players
        .iter()
        .flatten()
        .filter_map(|h| sim.unit(*h))
        .any(|p| {
            let d2 = p.soldier.pos.distance_squared(pos);
            d2 <= HUMAN_FAR * HUMAN_FAR
                && (d2 <= HUMAN_NEAR * HUMAN_NEAR || perception::los(sim, u.muzzle(), p.muzzle()))
        });
    if !nearby_human {
        return None;
    }
    st.items.next_scan = sim.tick + PERIOD;
    let (item, past) = sim
        .pickups
        .iter()
        .filter(|p| p.pos.distance_squared(pos) <= RADIUS * RADIUS)
        .take(6)
        .find_map(|p| {
            let c = sim.item_classes.get(usize::from(p.class))?;
            if !sim.item_needed(u, c)
                || !perception::los(sim, u.muzzle(), p.pos + Vec3::Y * 0.3)
                || !sim.floor_line(pos, p.pos)
            {
                return None;
            }
            let dir = Vec3::new(p.pos.x - pos.x, 0.0, p.pos.z - pos.z).normalize_or_zero();
            let past = p.pos + dir;
            sim.floor_line(pos, past).then_some((p, past))
        })?;
    st.items.next_scan += FOUND_DELAY;
    st.items.detour = Some(ItemDetour {
        origin: pos,
        item: item.pos,
        past,
        class: item.class,
        returning: false,
        until: sim.tick + PERIOD + FOUND_DELAY,
    });
    Some(past)
}

#[cfg(test)]
mod tests;
