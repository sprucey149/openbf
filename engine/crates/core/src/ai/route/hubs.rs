//! The hub a unit routes from: waypoints, drawn points on a hub, floors and levels.

use super::*;

/// A hub waypoint: its XZ with `y` from the ground under it AT THE UNIT'S OWN
/// LEVEL (`from_y`), falling back to the ground at the hub's AUTHORED height.
///
/// Hub `y` is the editor's pick height, not a floor (RE_LOG). Phase A.1
/// resolved the ground from that authored `y` alone, which is what put
/// waypoints on the wrong storey or nowhere at all: BF2 Polis Massa authors
/// all 189 of its hubs at y 0 while its floors are 13–33 m up, so
/// `ground_under` started its probe under every floor in the map, returned the
/// [`crate::collision::NO_GROUND`] sentinel, and the follower walked units at
/// a waypoint 20 m below their feet. Asking from the unit's own `y` asks the
/// question the follower means: what is under that XZ, on MY level (PLAN
/// "PHASE A.2 DESIGN" decision 3).
///
/// DEVIATION from that design, measured: the unit's level ALONE is not enough,
/// because `ground_under` calls a terrain sheet more than
/// [`crate::collision::TERRAIN_BURY`] above the feet "buried" and returns the
/// sentinel for it. That rule is right for a soldier standing under an uncut
/// heightfield and wrong for a hub UPHILL of one: on TAT1 the same waypoint
/// [-464, 17, 51] resolved to `floor 16.9` for a unit at y 18.0 and to
/// NO_GROUND for one at y 11.6, and the literal rule rejected 3,894 legs in
/// 3,000 ticks (1,682 of them with no alternative left), which stopped the
/// 6-a-side replay round fighting at all. So the authored height is the
/// FALLBACK, and a leg is only rejected when neither height finds a floor —
/// the intersection of A.1's rule and this one, which is still exactly what
/// rejects POL1's shafts (their authored probe is the one that found nothing
/// in the first place).
///
/// Card BNDS: since the hubs' floors are RESOLVED (`Sim::nav_floors`, walked
/// from the posts along the arcs), a resolved hub's waypoint is at that floor
/// whatever level the unit asks from — the hub is where the graph's own
/// walkway puts it, not on whatever lies under it at the unit's height (a
/// deck under a ramp, a platform's hidden core).
pub(super) fn hub_waypoint(sim: &Sim, g: &NavGraph, hub: u16, from_y: f32) -> Vec3 {
    if let (Some(h), Some(f)) = (g.hub(hub), sim.nav_floor(hub)) {
        return Vec3::new(h.pos.x, f, h.pos.z);
    }
    match g.hub(hub) {
        Some(h) => {
            let at = |y: f32| sim.ground_under(Vec3::new(h.pos.x, y, h.pos.z), false);
            let mut y = at(from_y);
            if y <= crate::collision::NO_GROUND && h.pos.y > from_y {
                y = at(h.pos.y);
            }
            Vec3::new(h.pos.x, y, h.pos.z)
        }
        None => Vec3::ZERO,
    }
}

/// How many points of a hub [`hub_point`] draws while the draw lies in a
/// barrier: the PS2's own count (`FUN_002ed3d8`).
pub const HUB_POINT_DRAWS: u32 = 100;

/// ...and how many of them it walks the floor to before it takes the centre.
/// PROVISIONAL (ours): the PS2 has no floor test.
pub const HUB_POINT_TRIES: u32 = 8;

/// A drawn point's floor is within this of the hub's resolved floor (m):
/// half [`HUB_LEVEL_TOL`], so a unit standing on it is at the hub
/// (`hub_reached`). PROVISIONAL (ours).
pub const HUB_POINT_LEVEL: f32 = 0.5 * HUB_LEVEL_TOL;

/// ...and the floor goes on, with no wall, this far past the point (m): a
/// point on a rim or against a wall is not one to send a soldier at.
/// PROVISIONAL (ours).
pub const HUB_POINT_MARGIN: f32 = 1.0;

/// Card HUBP: the point of hub `hub` a soldier on foot walks at — the PS2's
/// waypoint (`analysis/re-soldier-ai.md` 2; B2 `FUN_002ea128`, BF1
/// `FUN_002657c8`): the hub's centre + normalize(three draws in [-1, 1)) x
/// max(0, radius - `inset`), a point of the hub's SPHERE and so, the walk
/// being XZ, of its disc (dense toward the rim). `inset` is the PS2's 4 m
/// for a droideka, else 0; the port has no droideka mover yet, so every
/// caller passes 0. Drawn per unit per leg from the unit's own stream at
/// this tick, and drawn again while it lies in a barrier that stops a
/// soldier (the PS2, up to [`HUB_POINT_DRAWS`] times).
///
/// OURS, PROVISIONAL: the point is one the hub's own floor leads to — the
/// floor walked from the centre ([`walk_floor`]) arrives within
/// [`HUB_POINT_LEVEL`] of the hub's resolved floor with no wall on the way,
/// and both hold [`HUB_POINT_MARGIN`] past it (a disc that reaches over a
/// ledge, through a wall or onto the storey below) — and one the floor
/// walked straight from the unit's feet (`pos`) arrives at too. After
/// [`HUB_POINT_TRIES`] such walks, and for a hub with no resolved floor, the
/// centre as before ([`hub_waypoint`]).
///
/// Measured and NOT kept (8 seeds x 32 maps each, card HUBP's report): a
/// wall sweep and a barrier test along the unit's line, the leg's line from
/// the unit instead of from the arc's end, and a walk by the hub's centre
/// when the next leg is the arc's own — each sent more legs at the centres
/// (a team mate within 1.5 m 7.2 % -> 8.6-9.3 % over the maps) and cured
/// nothing they were written for.
pub(super) fn hub_point(
    sim: &Sim,
    g: &NavGraph,
    me: UnitHandle,
    hub: u16,
    pos: Vec3,
    inset: f32,
) -> Vec3 {
    let centre = hub_waypoint(sim, g, hub, pos.y);
    let (Some(h), Some(floor)) = (g.hub(hub), sim.nav_floor(hub)) else {
        return centre;
    };
    let r = (h.radius - inset).max(0.0);
    if r <= 0.0 {
        return centre;
    }
    let c = Vec2::new(centre.x, centre.z);
    let cap = crate::collision::Capsule {
        base: centre,
        height: crate::unit::capsule_height(crate::soldier::Stance::Stand),
        radius: crate::unit::SOLDIER_RADIUS,
    }
    .stepped();
    let mut walks = 0;
    for k in 0..HUB_POINT_DRAWS {
        let draw =
            |j: u32| 2.0 * unit_unit(me.index, sim.tick, me.gen, SALT_HUB_POINT + 3 * k + j) - 1.0;
        let d = Vec3::new(draw(0), draw(1), draw(2)).normalize_or_zero() * r;
        let p = c + Vec2::new(d.x, d.z);
        if g.barriers
            .iter()
            .any(|b| b.blocks(nav::SOLDIER) && b.contains(p))
        {
            continue;
        }
        let len = (p - c).length();
        if len < 1e-3 {
            return centre;
        }
        // The floor walked to the point, under it, and on past it by the
        // margin.
        let dir = (p - c) / len;
        let level = |y: &f32| (y - floor).abs() <= HUB_POINT_LEVEL;
        let stands = walk_floor(sim, centre, p).filter(level).filter(|&y| {
            // (One ray each, not the foot's ring: a foot holds an edge.)
            let under = |q: Vec2| sim.ground_point_under(Vec3::new(q.x, y, q.y), false);
            (under(p) - y).abs() <= crate::collision::STEP_HEIGHT
                && level(&under(p + dir * HUB_POINT_MARGIN))
                // ...and the floor walked from the unit's feet reaches it: a
                // straight line at a point off the centre can leave the
                // walkway the arc lies on.
                && walk_floor(sim, pos, p).is_some_and(|f| (f - y).abs() <= HUB_POINT_LEVEL)
        });
        if let Some(y) = stands {
            let dir = Vec3::new(dir.x, 0.0, dir.y);
            if sim
                .collision
                .wall_sweep(&cap, dir, len + HUB_POINT_MARGIN)
                .is_none()
            {
                return Vec3::new(p.x, y, p.y);
            }
        }
        walks += 1;
        if walks >= HUB_POINT_TRIES {
            break;
        }
    }
    centre
}

/// Card HUBP: a stuck unit's re-planned leg walks the arc's own line again,
/// hub centre to hub centre — the walkway — not at a second drawn point.
/// OURS, PROVISIONAL (the PS2's cure is a random walk).
pub(in super::super) fn centre_leg(sim: &Sim, g: &NavGraph, r: &mut Route, pos: Vec3) {
    let Some(arc) = r.via_arc.and_then(|a| g.arcs.get(usize::from(a))) else {
        return;
    };
    if let Some(a) = g.hub(arc.other(r.next_hub)) {
        r.from = Vec2::new(a.pos.x, a.pos.z);
    }
    r.waypoint = hub_waypoint(sim, g, r.next_hub, pos.y);
}

/// Is there any floor under this hub's XZ that [`hub_waypoint`] can find from
/// `from_y`?
///
/// `false` means the NO_GROUND sentinel — nothing under the unit's feet there
/// and nothing under the authored height either — and the leg to that hub is
/// REJECTED from here. Note what this can and cannot see: `ground_under` looks
/// DOWN, so it catches a hub whose only floor is ABOVE the unit (a walkway
/// over an open shaft) and NOT one whose floor is far below (a unit can fall
/// to that, and on the discs often should). The design's rule is the sentinel,
/// and the sentinel is what this tests.
pub(crate) fn hub_has_floor(sim: &Sim, g: &NavGraph, hub: u16, from_y: f32) -> bool {
    hub_waypoint(sim, g, hub, from_y).y > crate::collision::NO_GROUND
}

/// Is hub `hub` on the level of a unit whose feet are at `y` — its resolved
/// floor within [`HUB_LEVEL_TOL`] of them? The test for the hub a unit ROUTES
/// FROM and for ARRIVAL (card BNDS): a unit under a walkway's hub is not at
/// that hub. An unresolved hub falls back to [`hub_has_floor`].
pub(crate) fn hub_on_level(sim: &Sim, g: &NavGraph, hub: u16, y: f32) -> bool {
    match sim.nav_floor(hub) {
        Some(f) => (f - y).abs() <= HUB_LEVEL_TOL,
        None => hub_has_floor(sim, g, hub, y),
    }
}

/// The hub a unit ROUTES FROM: [`NavGraph::unit_hub_on`] - the port's own
/// XZ reading of where a unit stands (a named deviation, card LOC3; a POST
/// is resolved by the PS2's 3-D [`NavGraph::locate`]) - with the same-level
/// preference every caller here uses. Phase B's commander needs the same
/// answer `retarget` uses, so it lives in one place.
///
/// Every objective rule starts here, [`retarget_phase_a`] too (card DAGS).
///
/// Card STLVL: a soldier on foot whose answer is a hub on ANOTHER STOREY —
/// the start is located in XZ, so a unit under a rooftop walkway or over a
/// cave locates at the hub above or below it — routes from the nearest hub
/// on its own
/// ([`level_hub`]).
pub(crate) fn start_hub(sim: &Sim, g: &NavGraph, me: UnitHandle) -> Option<u16> {
    let u = sim.unit(me)?;
    let pos = u.soldier.pos;
    let h = g.unit_hub_on(pos, mover(sim, u), &|h| hub_on_level(sim, g, h, pos.y))?;
    Some(if u.mount.is_none() {
        level_hub(sim, g, h, pos)
    } else {
        h
    })
}

/// Card SPNC's census of the spawn nodes (`game/examples/spawn_node_census.rs`):
/// the first leg unit `me`, standing where it is, plans toward hub `dest` -
/// from [`start_hub`] (`[1]`, today's rule) and from the bare XZ locate the
/// first leg after a spawn took before card DAGS (`[0]`). Each is the start
/// hub and the leg [`plan_leg`] answers from it; `None` off the graph.
/// Reads the sim only.
pub fn first_leg(sim: &Sim, me: UnitHandle, dest: u16) -> [Option<(u16, Route)>; 2] {
    let (Some(g), Some(u)) = (sim.nav.as_ref(), sim.unit(me)) else {
        return [None, None];
    };
    let pos = u.soldier.pos;
    let bare = g.unit_hub_on(pos, mover(sim, u), &|h| hub_on_level(sim, g, h, pos.y));
    let st = AiState::default();
    let mut nt = NavTick::default();
    [bare, start_hub(sim, g, me)]
        .map(|cur| cur.and_then(|c| plan_leg(sim, &st, me, c, dest, None, &mut nt).map(|r| (c, r))))
}

/// [`walk_floor`] for the census (card SPNC).
pub fn floor_walked(sim: &Sim, from: Vec3, to: Vec2) -> Option<f32> {
    walk_floor(sim, from, to)
}

/// The hub a soldier at `pos` routes from (or is hunted at), given the
/// graph's own XZ answer `h`: `h` when it is on the soldier's storey — its
/// resolved floor within [`LEVEL_HUB_NEAR`] of the feet, or reached within
/// [`LEVEL_HUB_REACH`] by the floor walked from the feet (a wide hub on a
/// slope, a ramp) — or unresolved; else the nearest of the
/// [`LEVEL_HUB_TRIES`] nearest resolved hubs ([`crate::sim::Sim::level_at`]'s
/// order: XZ to the disc plus the height between, weighted) that the floor
/// walked from the feet does reach; `h` when there is none. The tolerances
/// are under [`HUB_LEVEL_TOL`]: BF1 Geonosis's cp13 stands on a platform
/// 3.6 m over the terrain with a hub on its ramp between the two, and the
/// last clone under that platform was hunted from on top of it. OURS,
/// PROVISIONAL, part of the unit-locate deviation (card LOC3): the PS2
/// locates a unit in 3-D like a goal (BF1 a hub sphere, both games 3-D
/// corridors; RE_LOG "Arrival and locate" and its audit of 2026-10-03) and
/// has no level rule.
pub(crate) fn level_hub(sim: &Sim, g: &NavGraph, h: u16, pos: Vec3) -> u16 {
    let Some(f) = sim.nav_floor(h) else {
        return h;
    };
    if (f - pos.y).abs() <= LEVEL_HUB_NEAR || sim.floor_reaches_hub(pos, h, LEVEL_HUB_REACH) {
        return untrapped(sim, g, h, pos);
    }
    let near = sim.nearest_floor_hubs::<LEVEL_HUB_TRIES>(g, pos, |i| {
        sim.nav_floor(i as u16).filter(|_| i as u16 != h)
    });
    let h = near
        .iter()
        .filter(|c| c.1 != u16::MAX)
        .find(|c| sim.floor_reaches_hub(pos, c.1, LEVEL_HUB_REACH))
        .map_or(h, |c| c.1);
    untrapped(sim, g, h, pos)
}

/// Card TGAP: [`level_hub`]'s answer `h`, unless a soldier would be trapped
/// there ([`crate::sim::Sim::hub_trapped`]) and he does not stand in its
/// disc: then the nearest of the [`LEVEL_HUB_TRIES`] nearest hubs he is not
/// trapped in that the floor walked from his feet reaches, `h` when none
/// does. BF1 Tatooine's spawn node (-377, 83) is 17 m off the walled
/// footpath and was started on it (hub 85) toward every post; a soldier in
/// the disc of a trapped hub (a deck no walkable arc joins) starts there.
pub(super) fn untrapped(sim: &Sim, g: &NavGraph, h: u16, pos: Vec3) -> u16 {
    let p = Vec2::new(pos.x, pos.z);
    let inside = g
        .hub(h)
        .is_some_and(|hub| (hub.xz() - p).length() <= hub.radius);
    if inside || !sim.hub_trapped(h) {
        return h;
    }
    let near = sim.nearest_floor_hubs::<LEVEL_HUB_TRIES>(g, pos, |i| {
        sim.nav_floor(i as u16)
            .filter(|_| !sim.hub_trapped(i as u16))
    });
    near.iter()
        .filter(|c| c.1 != u16::MAX)
        .find(|c| sim.floor_reaches_hub(pos, c.1, LEVEL_HUB_REACH))
        .map_or(h, |c| c.1)
}

/// The hub a mover of type `ty` standing at hub `cur` routes to for `post`:
/// the post's own when its type can get there. A walker or (card VDROP) a
/// hover that cannot (`nearest`, card WAI) takes the hub it can reach that
/// is nearest the post, and its driver gets out there — on both Hoths no
/// post's hub is on a vehicle type's graph (BF2 HOT1: the nearest are
/// 29-86 m off), on BF2 GEO1 four of seven are not, and the PS2's bots ride
/// there all the same (re-walker-ai §10). OURS, PROVISIONAL: where a PS2
/// vehicle's move order comes from is not transcribed.
pub(crate) fn post_hub(
    g: &NavGraph,
    ty: usize,
    cur: u16,
    post: &crate::cp::CommandPost,
    nearest: bool,
) -> Option<u16> {
    let own = post.nav_hub?;
    if g.reachable(cur, own, ty) {
        return Some(own);
    }
    if !nearest {
        return None;
    }
    let mut best: Option<(f32, u16)> = None;
    for (i, h) in g.hubs.iter().enumerate() {
        let d = nav::xz_dist(h.pos, post.def.pos);
        if best.is_none_or(|(b, _)| d < b) && g.reachable(cur, i as u16, ty) {
            best = Some((d, i as u16));
        }
    }
    best.map(|(_, i)| i)
}
