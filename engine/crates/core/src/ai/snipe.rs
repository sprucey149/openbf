//! Card SNPR: a SNIPER WITH A PURPOSE (OURS, a named deviation the maintainer approved:
//! INBOX 2026-10-12 16:20, ask 2; `analysis/bot-design.md` 4.3's Overwatch
//! role pulled forward, built so card TEAM's board can call [`pick`]). A bot
//! that holds a scoped rifle (the PS2's AI sniper table's class,
//! [`crate::weapon::WeaponClass::ai_distance_table`]) and is free picks the
//! spot with the longest clear lane onto enemy ground ([`pick`]: every Snipe
//! hint its route reaches within [`SNIPE_REACH`], else the high hubs), walks
//! there over the graph, and holds it while an enemy has been seen from it
//! in the last [`SNIPE_QUIET`]; then it picks again, the spot just left
//! excluded ([`upkeep`]). One sniper a spot ([`SNIPE_SPACING`]; two that
//! pick on one tick pick blind of each other, and the higher index gives
//! way). Its pistol is the arsenal's (`super::arsenal::sidearm`): drawn
//! inside the rifle's MinRange or for a target within 25 m and coming on,
//! kept to the pistol's own reach.
//!
//! Standing still with the rifle in hand it sees out to its tier's share of
//! the rifle's `MaxRange` (`perception::scope_reach`, OURS, 2026-10-29), its
//! lanes reach as far, and on its spot it fights from there
//! (`behave::position`); at Easy (share 0) none of the three.
//!
//! The PS2's sniper stands 60-120 s at one hint whatever it sees (BF1 60-100
//! s; `analysis/re-soldier-ai.md` 20, card CLSE4: 99 % of its rifle time in
//! the stand, 23 shots in 3,051 unit-seconds) - the statue the maintainer does not
//! want (bot design D13). Every constant here is OURS and PROVISIONAL.

use super::state::{AiState, HoldState, SnipeState};
use super::{HOLD_ENDGAME, HOLD_MOVE_PERIOD, HOLD_SPACING};
use crate::commander::HINT_SNIPE;
use crate::nav;
use crate::perception;
use crate::sim::Sim;
use crate::soldier::Stance;
use crate::unit::{Unit, UnitHandle};
use crate::TICK_HZ;
use glam::Vec3;

/// How far a sniper looks for a spot, m (straight; the graph must reach it).
/// A sniper lives a median 37 s on foot: of the reaches swept (8 seeds x 32
/// maps, card SNPR) 60 m kept it at its spot 9.1 % of its foot time, 120 m
/// 7.0 % and 40 m 8.7 %.
pub const SNIPE_REACH: f32 = 60.0;
/// A lane quiet this long - no enemy seen from the spot - and the sniper
/// moves on, ticks (the maintainer: 20 s).
pub const SNIPE_QUIET: u64 = 20 * TICK_HZ as u64;
/// Two snipers' spots are at least this far apart, m.
pub const SNIPE_SPACING: f32 = 8.0;
/// At most this many spots (the nearest) are scored in one pick.
const CANDIDATES: usize = 12;
/// A high hub's floor stands this far above the mean hub floor, m.
const HIGH_ABOVE: f32 = 4.0;
/// A lane's nearest counted point, m (nearer is the pistol's, not a lane).
const LANE_MIN: f32 = 15.0;
/// No pick again for this long after none was found or a walk to one failed.
const RETRY: u64 = 10 * TICK_HZ as u64;
/// The spot is reached within this, m (xz), and this height.
const ARRIVE: f32 = 1.5;
const ARRIVE_DY: f32 = 2.0;
/// The spot left last is not taken again within this, m.
const LEFT_RADIUS: f32 = 3.0;

/// Does `u` hold a scoped rifle (the AI sniper table's class) in any slot?
pub fn is_sniper(sim: &Sim, u: &Unit) -> bool {
    let scoped = |c: usize| sim.weapon_classes[c].ai_distance_table();
    scoped(u.weapon_class)
        || u.held
            .iter()
            .any(|w| scoped(w.class) && !sim.weapon_locked(u, w.class))
}

/// Is the sniper on its spot, `hold` (the live hold) the spot's?
pub(super) fn holds(st: &AiState, hold: Option<HoldState>) -> bool {
    match (st.snipe.cur, hold) {
        (Some(c), Some(h)) => c.arrived && nav::xz_dist(h.spot, c.spot) <= ARRIVE,
        _ => false,
    }
}

/// The point a sniper's walk aims at this think: its spot, held or not.
pub(super) fn walk(st: &AiState) -> Option<Vec3> {
    st.snipe.cur.map(|c| c.spot)
}

/// Once a think, before the route is planned: drop the spot of a bot that
/// is busy (a squad order, off the level, a boarding claim, the endgame),
/// keep a held one's lane clock, move on from a quiet one, pick a new one.
/// `seen`: an enemy is seen this think.
pub(super) fn upkeep(sim: &Sim, me: UnitHandle, u: &Unit, st: &mut AiState, seen: bool) {
    if !is_sniper(sim, u) {
        return;
    }
    let busy = u.mount.is_some()
        || st.lost.is_some()
        || st.cmd.is_some()
        || u.command.is_some()
        || st.ride.is_some_and(|r| r.board.is_some())
        || sim.battle.teams.iter().any(|t| t.tickets() <= HOLD_ENDGAME);
    if busy {
        st.snipe.cur = None;
        return;
    }
    // Snipers that think on one tick pick blind of each other (each think
    // reads the tick's old state): the one that has not arrived, else the
    // higher index, gives a shared spot up and picks again.
    if let Some(c) = st.snipe.cur {
        let taken = sim.alive_units().any(|(h, o)| {
            h != me
                && o.ai.snipe.cur.is_some_and(|x| {
                    nav::xz_dist(x.spot, c.spot) < SNIPE_SPACING
                        && (x.arrived && !c.arrived || x.arrived == c.arrived && h.index < me.index)
                })
        });
        if taken {
            st.snipe.cur = None;
        }
    }
    if let Some(mut c) = st.snipe.cur.filter(|c| c.arrived) {
        if seen {
            c.lane_at = sim.tick;
        }
        let live = st
            .hold
            .filter(|h| sim.tick < h.until && nav::xz_dist(h.spot, c.spot) <= ARRIVE);
        match live {
            // Held: the hold lasts while the lane is busy, and its own quiet
            // clock (`HOLD_QUIET`) is the lane's.
            Some(mut h) if sim.tick < c.lane_at + SNIPE_QUIET => {
                h.until = h.until.max(c.lane_at + SNIPE_QUIET);
                h.last_contact = sim.tick;
                st.hold = Some(h);
                st.snipe.cur = Some(c);
            }
            // Quiet for SNIPE_QUIET, or moved off it (a close fight): the
            // next spot, this one excluded.
            _ => {
                if let Some(mut h) = live {
                    h.until = sim.tick;
                    st.hold = Some(h);
                }
                st.snipe.left = Some(c.spot);
                st.snipe.cur = None;
                st.snipe.next = sim.tick;
            }
        }
    }
    if st.snipe.cur.is_none() && sim.tick >= st.snipe.next {
        st.snipe.cur = pick(sim, me, u, st.snipe.left);
        if st.snipe.cur.is_none() {
            st.snipe.next = sim.tick + RETRY;
        }
    }
    // Reached (walked there, or stood on it when picked): the hold. Under or
    // over it on the last leg, it cannot get on it: given up as a failed
    // walk. The mover never reports a spot away from the bot's post as
    // arrived, so a sniper under a perch stood there with nothing to walk
    // until the stall watch recycled it (land SNPR 2026-10-16: BF1 Kashyyyk
    // 2's hint 2.5 m over the ground at (94.4, 17.3, -144.0), 120 s).
    if let Some(c) = st.snipe.cur.filter(|c| !c.arrived) {
        let pos = u.soldier.pos;
        if nav::xz_dist(pos, c.spot) <= ARRIVE {
            if (pos.y - c.spot.y).abs() <= ARRIVE_DY {
                arrive(sim, me, st);
            } else if st.route.is_none_or(|r| r.is_last()) {
                walk_failed(sim, st);
            }
        }
    }
}

/// The sniper stands on its spot: a hold there for [`SNIPE_QUIET`], the
/// hint's stance with it. A live hold of another kind (a fight on the way)
/// is left alone; the arrival waits for it. Not from under or over it (the
/// mover's arrival is XZ only): [`upkeep`] gives such a spot up.
pub(super) fn arrive(sim: &Sim, me: UnitHandle, st: &mut AiState) {
    let Some(mut c) = st.snipe.cur.filter(|c| !c.arrived) else {
        return;
    };
    if st.hold.is_some_and(|h| sim.tick < h.until)
        || sim
            .unit(me)
            .is_some_and(|u| (u.soldier.pos.y - c.spot.y).abs() > ARRIVE_DY)
    {
        return;
    }
    c.arrived = true;
    c.lane_at = sim.tick;
    st.snipe.cur = Some(c);
    st.hold = Some(HoldState {
        spot: c.spot,
        goal: c.spot,
        until: sim.tick + SNIPE_QUIET,
        skip: 0,
        shuffle_at: sim.tick + HOLD_MOVE_PERIOD,
        strafe_side: if me.index.is_multiple_of(2) { 1 } else { -1 },
        stance_mask: c.mask,
        last_contact: sim.tick,
        // It watches down its lane until an enemy it can see is nearer
        // (the hold's upkeep, `behave::brain`).
        look: c.look,
        ..HoldState::default()
    });
}

/// The walk to the spot failed (the mover's progress window): it is given
/// up as the one left, and the next pick waits [`RETRY`].
pub(super) fn walk_failed(sim: &Sim, st: &mut AiState) {
    if let Some(c) = st.snipe.cur.filter(|c| !c.arrived) {
        st.snipe.left = Some(c.spot);
        st.snipe.cur = None;
        st.snipe.next = sim.tick + RETRY;
    }
}

/// Card SNPR: the best free spot for sniper `me` - the Snipe hints within
/// [`SNIPE_REACH`] that its route reaches on their own level, the
/// [`CANDIDATES`] nearest, scored by their lane ([`lane`]); when none of them
/// has one, the high hubs (a floor [`HIGH_ABOVE`] over the mean) the same
/// way. Not the spot it left last, not within [`SNIPE_SPACING`] of another
/// sniper's or [`HOLD_SPACING`] of a holder's. `None`: nothing with a lane.
pub fn pick(sim: &Sim, me: UnitHandle, u: &Unit, left: Option<Vec3>) -> Option<SnipeState> {
    let g = sim.nav.as_ref()?;
    let pos = u.soldier.pos;
    let m = super::route::mover(sim, u);
    let from = super::route::start_hub(sim, g, me);
    let reach = |p: Vec3| {
        let d = g
            .unit_hub_on(p, m, &|h| super::route::hub_on_level(sim, g, h, p.y))
            .map(|h| super::route::level_hub(sim, g, h, p));
        match (from, d) {
            (Some(c), Some(d)) => g.reachable(c, d, m.ty),
            _ => false,
        }
    };
    let free = |p: Vec3| {
        left.is_none_or(|l| nav::xz_dist(l, p) > LEFT_RADIUS)
            && sim.alive_units().all(|(h, o)| {
                h == me
                    || o.ai
                        .snipe
                        .cur
                        .is_none_or(|c| nav::xz_dist(c.spot, p) >= SNIPE_SPACING)
                        && o.ai
                            .hold
                            .filter(|x| sim.tick < x.until)
                            .is_none_or(|x| nav::xz_dist(x.goal, p) >= HOLD_SPACING)
                        && !(o.is_player && nav::xz_dist(o.soldier.pos, p) < HOLD_SPACING)
            })
    };
    let best = |cands: &mut Vec<(f32, Vec3, u8)>| -> Option<SnipeState> {
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut best: Option<(f32, SnipeState)> = None;
        for &(_, p, mask) in cands.iter().filter(|c| free(c.1)).take(CANDIDATES) {
            if !reach(p) {
                continue;
            }
            let (score, len, far) = lane_far(sim, u, p);
            if score > 0.0 && best.is_none_or(|(b, _)| score > b) {
                best = Some((
                    score,
                    SnipeState {
                        spot: p,
                        mask,
                        lane: len,
                        // Ground rounds only: a space round's sniper (a
                        // hangar fight) holds as it did.
                        look: if !sim.space_round() {
                            far.unwrap_or(Vec3::ZERO)
                        } else {
                            Vec3::ZERO
                        },
                        ..SnipeState::default()
                    },
                ));
            }
        }
        best.map(|b| b.1)
    };
    let mut hints: Vec<(f32, Vec3, u8)> = g
        .hints
        .iter()
        .filter(|h| h.kind == HINT_SNIPE)
        .map(|h| ((h.pos - pos).length(), h.pos, h.stance_mask))
        .filter(|c| c.0 <= SNIPE_REACH)
        .collect();
    if let Some(s) = best(&mut hints) {
        return Some(s);
    }
    let floors: Vec<(u16, f32)> = (0..g.hubs.len() as u16)
        .filter_map(|h| sim.nav_floor(h).map(|y| (h, y)))
        .collect();
    if floors.is_empty() {
        return None;
    }
    let mean = floors.iter().map(|f| f.1).sum::<f32>() / floors.len() as f32;
    let mut high: Vec<(f32, Vec3, u8)> = floors
        .iter()
        .filter(|f| f.1 >= mean + HIGH_ABOVE)
        .map(|&(h, y)| {
            let c = g.hubs[usize::from(h)].pos;
            let p = Vec3::new(c.x, y, c.z);
            ((p - pos).length(), p, 0)
        })
        .filter(|c| c.0 <= SNIPE_REACH)
        .collect();
    best(&mut high)
}

/// The lane from spot `p` for sniper `u`: every point of ENEMY GROUND - a
/// post its side does not hold, and a hub whose nearest post is such a one -
/// between [`LANE_MIN`] and the sniper's sight of a standing soldier
/// (`perception::view_distance` at its tier, or its scope's at the tier,
/// `perception::scope_reach`, if farther) that a ray from the spot's eye
/// reaches, each weighted by its distance over that sight. Answers the score
/// and the lane's length (its farthest point in sight).
///
/// Card MAIS: and no farther than twice the mission's
/// `AISnipeSuitabilityDist` - the far edge of the PS2's own suitability
/// query, a sphere of that radius centred that far out along the hint
/// (`FUN_002d8398`, whose answer the PS2 never uses: re-soldier-ai 20). Its
/// default (70 m) gives a lane to 140 m, past a scope's 128 m; the indoor
/// maps' 30 m keeps a lane to 60 m.
pub fn lane(sim: &Sim, u: &Unit, p: Vec3) -> (f32, f32) {
    let (score, len, _) = lane_far(sim, u, p);
    (score, len)
}

/// [`lane`] with the farthest point in sight itself (a metre up), `None`
/// for no lane: where the sniper looks from the spot with nothing in sight
/// (`SnipeState::look`).
pub fn lane_far(sim: &Sim, u: &Unit, p: Vec3) -> (f32, f32, Option<Vec3>) {
    let Some(g) = sim.nav.as_ref() else {
        return (0.0, 0.0, None);
    };
    let tier = sim.unit_tier(u);
    let share = perception::scope_share(u, &tier);
    let scope = std::iter::once(u.weapon_class)
        .chain(u.held.iter().map(|w| w.class))
        .filter(|&c| !sim.weapon_locked(u, c))
        .filter_map(|c| perception::scope_reach(sim, &sim.weapon_classes[c], share))
        .fold(0.0f32, f32::max);
    let sight = perception::view_distance(Stance::Stand, tier.view_mult)
        .max(scope)
        .min(2.0 * sim.mission_ai.snipe_suitability);
    let eye = p + Vec3::Y * crate::unit::muzzle_height(Stance::Stand);
    let enemy_post = |i: usize| sim.cps[i].owner != Some(u.team);
    let mut pts: Vec<Vec3> = Vec::new();
    for (i, cp) in sim.cps.iter().enumerate() {
        if enemy_post(i) {
            pts.push(cp.def.pos);
        }
    }
    if !sim.cps.is_empty() {
        for (h, hub) in g.hubs.iter().enumerate() {
            let near = (0..sim.cps.len()).min_by(|&a, &b| {
                (sim.cps[a].def.pos - hub.pos)
                    .length_squared()
                    .total_cmp(&(sim.cps[b].def.pos - hub.pos).length_squared())
            });
            if near.is_some_and(enemy_post) {
                let y = sim.nav_floor(h as u16).unwrap_or(hub.pos.y);
                pts.push(Vec3::new(hub.pos.x, y, hub.pos.z));
            }
        }
    }
    let (mut score, mut len, mut far) = (0.0f32, 0.0f32, None);
    for q in pts {
        let d = (q - p).length();
        if !(LANE_MIN..=sight).contains(&d) {
            continue;
        }
        let to = q + Vec3::Y;
        // A team's snipers weigh the same spots against the same ground: a
        // map's opening tick, when they all pick at once, spent most of its
        // time here.
        if super::think_memo::lane(eye, to, || {
            perception::los_terrain(sim, eye, to) && perception::los_world(sim, eye, to)
        }) {
            score += d / sight;
            if d > len {
                len = d;
                far = Some(to);
            }
        }
    }
    (score, len, far)
}

#[cfg(test)]
mod tests;
