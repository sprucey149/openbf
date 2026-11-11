//! Card UNSN (D23, OURS): a bot the stall watch gives up is moved on, not
//! killed, when no human can see it happen. The recycle of card RCYC
//! ([`super::stall`]) costs the bot's team a ticket for a defect of ours;
//! instead the bot is put down at a hub of its own route past the link it
//! failed on - the far end of its leg first, then the hubs after it toward
//! its goal - within [`REACH`], on a floor, clear of the map's bodies and of
//! every soldier, on the level and outside the death regions. Only when no
//! human player has a line of sight to the bot or to the spot
//! ([`seen_by_human`]); with none such, the recycle stays.
//!
//! Every move is still a defect: the WHY code [`super::why::Why::Relocated`],
//! the census's failed-walk column and an [`crate::sim::Event::Relocated`]
//! with both positions (the sweep's `relocate` list).
//!
//! Never hashed on its own: the move is the unit's position and the AI
//! state's reset, which are.

use super::state::{AiState, MoverState};
use crate::nav::{self, NavGraph};
use crate::sim::Sim;
use crate::unit::{Controller, UnitHandle};
use glam::Vec3;

/// The farthest a bot is moved (m, 3-D). PROVISIONAL (ours): "near by" -
/// the far end of most failing links is inside it (BF1 Bespin 1's arc 150);
/// a longer one keeps the recycle.
pub const REACH: f32 = 60.0;
/// Hubs past the failing link that are tried, the leg's own first.
pub const HOPS: usize = 3;
/// A spot this near the bot (XZ, m) is no move at all.
pub const MIN_MOVE: f32 = 3.0;
/// No soldier's feet within this of the spot (XZ, m)...
pub const CLEAR_XZ: f32 = 1.5;
/// ...and this in height (m).
pub const CLEAR_DY: f32 = 2.5;
/// The spot's floor within this of the hub's own (m).
pub const FLOOR_DY: f32 = 1.0;
/// Rings round the hub's centre tried after it (m), eight points each.
pub const RINGS: [f32; 2] = [1.5, 3.0];
/// Anything a human has a clear line to within this (m) counts as seen,
/// whichever way he looks: he hears it, or turns. PROVISIONAL (ours).
pub const NEAR_SEEN: f32 = 15.0;
/// Beyond [`NEAR_SEEN`], seen inside this cone of his look (cos 65 deg, wider
/// than the screen's half-width so the third-person view's edges are in).
pub const COS_VIEW: f32 = 0.42;
/// The third-person camera stands about this far behind the eye (m)...
pub const CAM_BACK: f32 = 3.0;
/// ...and this far over it (m). Both eyes are asked.
pub const CAM_UP: f32 = 0.5;
/// The heights over the feet a body is looked for at (m): shins, chest, head.
const BODY: [f32; 3] = [0.3, 1.0, 1.7];

/// The spot unit `me` (on foot, stalled) is moved to, or `None`: no route,
/// no hub past the failing link that is near, standable and free, or a human
/// sees the bot or the spot.
pub(super) fn spot(sim: &Sim, me: UnitHandle, st: &AiState) -> Option<Vec3> {
    let u = sim.unit(me).filter(|u| u.mount.is_none())?;
    let g = sim.nav.as_ref()?;
    let r = st.route()?;
    let pos = u.soldier.pos;
    if seen_by_human(sim, pos) {
        return None;
    }
    let m = super::route::mover(sim, u);
    let mut hub = r.next_hub;
    let mut prev = r.via_arc;
    for _ in 0..HOPS {
        if let Some(p) = hub_spot(sim, g, hub, pos) {
            return Some(p);
        }
        // The hub after it toward the goal: the table's own way, else the
        // fewest arcs.
        let (arc, next) = g
            .next_hop(hub, r.dest_hub, m, prev, None, 0)
            .or_else(|| g.route_bfs(hub, r.dest_hub, m, &|_| true))?;
        prev = Some(arc);
        hub = next;
    }
    None
}

/// A free spot at hub `h` for a bot at `pos`: the hub's centre on its floor,
/// else the first of [`RINGS`] round it (fixed order).
fn hub_spot(sim: &Sim, g: &NavGraph, h: u16, pos: Vec3) -> Option<Vec3> {
    let hub = g.hub(h)?;
    let floor = sim
        .nav_floor(h)
        .unwrap_or_else(|| sim.ground_under(hub.pos + Vec3::Y * 2.0, false));
    if floor <= crate::collision::NO_GROUND {
        return None;
    }
    let centre = Vec3::new(hub.pos.x, floor, hub.pos.z);
    if (centre - pos).length() > REACH || nav::xz_dist(centre, pos) < MIN_MOVE {
        return None;
    }
    let ring = RINGS.iter().filter(|&&d| d <= hub.radius.max(RINGS[0]));
    std::iter::once(centre)
        .chain(ring.flat_map(|&d| {
            (0..8).map(move |k| {
                let (s, c) = crate::fmath::sin_cos(k as f32 * std::f32::consts::FRAC_PI_4);
                centre + Vec3::new(s * d, 0.0, c * d)
            })
        }))
        .find_map(|p| standable(sim, p, floor))
}

/// `p` (XZ) put on its floor, when that floor is the hub's (within
/// [`FLOOR_DY`] of `floor`), on the level, out of the death regions, clear of
/// every body and soldier, and unseen.
fn standable(sim: &Sim, p: Vec3, floor: f32) -> Option<Vec3> {
    let y = sim.ground_under(Vec3::new(p.x, floor + 1.5, p.z), false);
    let at = Vec3::new(p.x, y, p.z);
    let free = |a: Vec3| {
        !sim.alive_units().any(|(_, o)| {
            nav::xz_dist(o.soldier.pos, a) < CLEAR_XZ && (o.soldier.pos.y - a.y).abs() < CLEAR_DY
        })
    };
    ((y - floor).abs() <= FLOOR_DY
        && !sim.level_at(at).is_off()
        && !sim.death_regions.iter().any(|r| r.contains(at + Vec3::Y))
        && sim.soldier_clear(at)
        && free(at)
        && !seen_by_human(sim, at))
    .then_some(at)
}

/// Does any living human player have a line of sight ([`crate::perception::los`],
/// the bolts' ray) to a soldier standing at `feet` - from his eye or from
/// behind it, where his camera is - within [`NEAR_SEEN`] whichever way he
/// looks, beyond it inside his view ([`COS_VIEW`])?
pub fn seen_by_human(sim: &Sim, feet: Vec3) -> bool {
    sim.alive_units()
        .filter(|(_, u)| matches!(u.controller, Controller::Player { .. }))
        .any(|(_, u)| {
            let eye = u.muzzle();
            let (sy, cy) = crate::fmath::sin_cos(u.soldier.yaw);
            let (sp, cp) = crate::fmath::sin_cos(u.soldier.pitch);
            let look = Vec3::new(-sy * cp, sp, -cy * cp);
            let cam = eye - Vec3::new(-sy, 0.0, -cy) * CAM_BACK + Vec3::Y * CAM_UP;
            BODY.iter().any(|&up| {
                let t = feet + Vec3::Y * up;
                [eye, cam].iter().any(|&from| {
                    let d = t - from;
                    let len = d.length();
                    (len <= NEAR_SEEN || d.dot(look) >= COS_VIEW * len)
                        && crate::perception::los(sim, from, t)
                })
            })
        })
}

/// The bot's walk starts again where it now stands: no leg, no watch, no
/// lost spell, a fresh mover (the post it gave up is kept) and a think at
/// the next tick. Its failed-link memory (card ARCM) is kept, so the link
/// stays closed to it.
pub(super) fn reset(st: &mut AiState, tick: u64) {
    super::route::clear(st);
    st.stall = None;
    st.lost = None;
    st.mv = MoverState {
        give_up: st.mv.give_up,
        ..MoverState::default()
    };
    st.slide = 0;
    st.unreachable_dwell = 0;
    st.next_think = tick + 1;
}

#[cfg(test)]
mod tests;
