//! Choosing a new goal: a post, a vehicle to board, a hunt, and the phase A rule.

use super::*;

/// (Re)plan the route toward the COMMANDER'S objective (Phase B) — or, for a
/// unit the commander has not reached yet, by Phase A's own rule.
///
/// The four cases:
///
/// - `Attack(cp)`: route to that post, exactly the way Phase A routed to the
///   nearest one. Unroutable from here (the unit moved between plans, or its
///   storey changed) leaves the ASSIGNMENT alone and drops only the route:
///   the commander sees the same unreachability next period and re-assigns.
///   [`NavTick::unroutable`] counts it.
/// - `Defend(cp)`: the same route, with the unit's own spot selected from
///   assignment time as the last leg's waypoint
///   ([`crate::commander::TeamPlan::spot`], card DEFS) — an authored
///   Cover/Snipe hint within 25 m of the post no other defender has, else a
///   point of its region as an attacker's. D13: a hint's
///   authored `PrimaryStance` becomes `AiState::want_stance`, and a Snipe hint
///   is only offered to a unit whose weapon has a scope.
/// - `Hunt`: the nearest enemy's hub, which is Phase A's fallback promoted to
///   an objective the commander hands out.
/// - `None` (a unit that spawned since the last plan): [`retarget_phase_a`],
///   unchanged, until the next plan reaches it.
pub(in super::super) fn retarget(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    nearest_enemy: Option<Vec3>,
    nt: &mut NavTick,
) {
    let Some(u) = sim.unit(me) else { return };
    // Card WAI2: a bot on foot with a boarding claim routes to its vehicle.
    if u.mount.is_none() {
        if let Some(at) = super::super::ride::board_point(sim, st) {
            retarget_board(sim, me, st, at, nt);
            return;
        }
    }
    if sim.space_assault.is_some() && u.mount.is_none() && u.team < 2 {
        retarget_deathmatch(sim, me, st, nt);
        return;
    }
    if u.team >= 2 || sim.deathmatch.is_some() {
        // PROVISIONAL, replace: trace the PS2 native Deathmatch and Conquest
        // goal orders. The main-army commander owns neither team's members.
        st.want_stance = 0;
        retarget_hunt(sim, me, st, nearest_enemy, nt);
        return;
    }
    let plan = &sim.commander[usize::from(u.team)];
    // [ours] A hunting unit goes where the fight is (`proxy::front`).
    let objective = super::super::proxy::front(sim, u).or(plan.of(me.index));
    match objective {
        // Card PARR: a last leg at a post walks to a point of its capture
        // region, not to its centre — but for a defender with a spot.
        Some(crate::commander::Objective::Attack(cp)) => {
            retarget_to_post(sim, me, st, cp, false, nt);
            aim_last_leg(sim, me, st, nt);
        }
        Some(crate::commander::Objective::Defend(cp)) => {
            if !retarget_to_post(sim, me, st, cp, true, nt) {
                aim_last_leg(sim, me, st, nt);
            }
        }
        Some(crate::commander::Objective::Hunt) => {
            st.want_stance = 0;
            retarget_hunt(sim, me, st, nearest_enemy, nt)
        }
        None => {
            retarget_phase_a(sim, me, st, nearest_enemy, nt);
            aim_last_leg(sim, me, st, nt);
            // D13: a unit the commander has not reached yet AND that came out
            // of `retarget_phase_a` with nowhere to walk is IDLE — it takes
            // the hint rule from its own position instead of from a post, so
            // a bot standing in an authored crouch spot crouches in it.
            st.want_stance = if st.route.is_none() {
                let scoped = sim.weapon_classes[u.weapon_class].ai_distance_table();
                let mask = crate::commander::hint_stance(sim, u.soldier.pos, scoped);
                stance_hold(st.want_stance, mask, me, sim.tick)
            } else {
                0
            };
        }
    }
}

const DM_SALT: u32 = 0xD3A7;
const DM_HUB_INSET: f32 = 10.0;
const DM_ENEMY_SPREAD: f32 = 10.0;

pub(super) fn retarget_deathmatch(sim: &Sim, me: UnitHandle, st: &mut AiState, nt: &mut NavTick) {
    let Some(u) = sim.unit(me) else { return };
    let pos = u.soldier.pos;
    let Some(g) = sim.nav.as_ref() else {
        st.route = None;
        st.wander = None;
        return;
    };
    let Some(cur) = start_hub(sim, g, me) else {
        st.route = None;
        st.wander = None;
        return;
    };
    st.target_cp = None;
    st.want_stance = 0;
    let walking = st.wander.zip(st.route).filter(|((at, hub), r)| {
        let d = nav::xz_dist(pos, *at);
        let there = d <= ARRIVE_MIN_RADIUS || (r.stuck_run > 0 && d <= g.radius(*hub));
        r.dest_hub == *hub && !(r.is_last() && there)
    });
    if walking.is_none() {
        let (at, hub) = deathmatch_goal(sim, g, me, u, cur);
        st.wander = Some((at, hub));
        st.route = plan_leg(sim, st, me, cur, hub, None, nt);
    }
    if let (Some((at, _)), Some(r)) = (st.wander, st.route.as_mut()) {
        if r.is_last() {
            r.waypoint = at;
        }
    }
}

fn deathmatch_goal(
    sim: &Sim,
    g: &NavGraph,
    me: UnitHandle,
    u: &crate::unit::Unit,
    cur: u16,
) -> (Vec3, u16) {
    let pos = u.soldier.pos;
    let m = mover(sim, u);
    let tick = sim.tick;
    let roll = |salt: u32| crate::ai::flyer_drive::draw(tick, me, DM_SALT ^ salt);
    if roll(1) < 0.5 {
        if let Some(goal) = deathmatch_enemy_goal(sim, g, me, u, cur) {
            return goal;
        }
    }
    let hubs = g.hubs.len();
    let hub = (0..hubs * 2)
        .map(|k| ((roll(0x100 + k as u32) * hubs as f32) as u16).min(hubs.saturating_sub(1) as u16))
        .find(|&h| h != cur && g.reachable_by(cur, h, m))
        .unwrap_or(cur);
    (hub_point(sim, g, me, hub, pos, DM_HUB_INSET), hub)
}

fn deathmatch_enemy_goal(
    sim: &Sim,
    g: &NavGraph,
    me: UnitHandle,
    u: &crate::unit::Unit,
    cur: u16,
) -> Option<(Vec3, u16)> {
    let m = mover(sim, u);
    let tick = sim.tick;
    let roll = |salt: u32| crate::ai::flyer_drive::draw(tick, me, DM_SALT ^ salt);
    let foes: Vec<&crate::unit::Unit> = sim
        .units
        .iter()
        .filter(|o| o.alive && sim.hostile(u.team, o.team))
        .collect();
    let foe = foes.get((roll(2) * foes.len() as f32) as usize)?;
    let at = match foe.mount.and_then(|f| sim.vehicle(f.vehicle)) {
        Some(v) => {
            if !crate::ai::arsenal::against_ride(sim, u, foe)?.1 {
                return None;
            }
            v.pos
        }
        None => foe.soldier.pos,
    };
    let spread = |salt: u32| (roll(salt) - 0.5) * 2.0 * DM_ENEMY_SPREAD;
    let at = at + Vec3::new(spread(3), spread(4), spread(5));
    let hub = g.unit_hub_on(at, m, &|h| hub_on_level(sim, g, h, at.y))?;
    let hub = level_hub(sim, g, hub, at);
    if hub != cur && !g.reachable_by(cur, hub, m) {
        return None;
    }
    let inside = g
        .hub(hub)
        .is_some_and(|h| nav::xz_dist(at, h.pos) <= h.radius);
    let point = if inside {
        at
    } else {
        hub_point(sim, g, me, hub, u.soldier.pos, DM_HUB_INSET)
    };
    Some((point, hub))
}

/// Route to command post `cp` because the commander said so. `defend` swaps
/// the last leg's aim point for the defence spot from objective assignment.
/// Intermediate arc legs are kept. Answers whether the unit has such a spot.
pub(super) fn retarget_to_post(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    cp: u32,
    defend: bool,
    nt: &mut NavTick,
) -> bool {
    let Some(u) = sim.unit(me) else { return false };
    let pos = u.soldier.pos;
    st.target_cp = Some(cp);
    let (Some(g), Some(post)) = (sim.nav.as_ref(), sim.cps.get(cp as usize)) else {
        // No graph: walk straight at the post, as before Phase A, and with no
        // graph there are no hints to take a stance from either (D13).
        st.route = None;
        st.want_stance = 0;
        return false;
    };
    // A defender's spot is the commander's (card DEFS: one defender a spot,
    // `commander::pick_spot`), read every think: the unit reaches the post
    // long after the leg that took it there was planned.
    //
    // DEVIATION, measured: the design takes the hint "once within its capture
    // volume". Gating it there makes the waypoint FLIP — inside the volume the
    // aim point is the hint, one step outside it is the post centre, and a hint
    // that sits outside the volume drags the unit back and forth across the
    // boundary forever (BF1 END1 unit 28: 45,587 ticks at full speed 1.5 m from
    // its own post; BF1 KAM1 and KAS2 the same shape). The spot is the
    // destination from the moment the objective is Defend, so there is one
    // aim point per objective and nothing to oscillate between.
    //
    // D13: the spot carries the hint's authored stance mask, and a Snipe hint
    // is only offered to a unit whose weapon passes the AI distance table. A
    // defender with no hint left, and a unit walking to a post it is
    // ATTACKING, has no spot and no stance wish.
    let found = defend
        .then(|| sim.commander[usize::from(u.team).min(1)].spot(me.index))
        .flatten();
    // The hint answers a MASK of allowed stances (`PrimaryStance` is a
    // bitmask); which of them this unit takes is drawn once and held
    // ([`stance_hold`]).
    st.want_stance = stance_hold(st.want_stance, found.map_or(0, |(_, m)| m), me, sim.tick);
    let spot = found.map(|(p, _)| p);
    // The leg we are already on is KEPT (Phase A's rule: re-planning every
    // think would also re-roll the chooser every ten ticks) — unless the
    // defence spot has to be written into it.
    // A driver's destination is its vehicle's ([`post_hub`], card WAI).
    let m = mover(sim, u);
    let cur = start_hub(sim, g, me);
    use crate::vehicle::VehicleKind::{Hover, Walker};
    let rides = u
        .mount
        .and_then(|m| sim.vehicle(m.vehicle))
        .is_some_and(|v| v.kind == Walker || (v.kind == Hover && super::super::HOVER_NEAREST));
    let want = if rides {
        cur.and_then(|c| post_hub(g, m.ty, c, post, true))
    } else {
        post.nav_hub
    };
    if let Some(r) = st.route {
        if want == Some(r.dest_hub) {
            match spot.filter(|_| r.is_last() && post.nav_hub == Some(r.dest_hub)) {
                Some(p) if r.waypoint != p => {
                    let mut r = r;
                    r.waypoint = p;
                    r.ref_dist = nav::xz_dist(pos, sub_point(&r, pos));
                    st.route = Some(r);
                }
                _ => {}
            }
            return spot.is_some();
        }
    }
    let Some(dest) = want else {
        st.route = None;
        nt.unroutable = 1;
        return false;
    };
    match cur.filter(|&c| g.reachable_by(c, dest, m)) {
        Some(c) => {
            // Card FLNK: the hub an attack on foot is planned from (a flanker
            // re-planned on its way keeps its start).
            if defend || u.mount.is_some() {
                st.flank = super::super::FlankState::default();
            } else if st.flank.to != dest {
                st.flank = super::super::FlankState {
                    from: c,
                    to: dest,
                    ..Default::default()
                };
            } else if st.flank.route.is_none() {
                st.flank.from = c;
            }
            st.route = plan_leg(sim, st, me, c, dest, None, nt);
            if let (Some(p), Some(mut r)) = (spot, st.route) {
                if r.is_last() && post.nav_hub == Some(r.dest_hub) {
                    r.waypoint = p;
                    r.ref_dist = nav::xz_dist(pos, sub_point(&r, pos));
                    st.route = Some(r);
                }
            }
        }
        None => {
            // Report it and leave the assignment for the commander to drop.
            st.route = None;
            nt.unroutable = 1;
        }
    }
    spot.is_some()
}

/// Card WAI2: a bot with a boarding claim walks the graph to the hub its
/// vehicle stands at, and from there (`think`: on the last leg) straight at
/// it — a claimer that walked straight from where it claimed slid along a
/// wall 17 m short of BF2 Hoth's snow AT-ST until its claim ran out. The
/// post is kept (`ride::board_goal` asks whether the vehicle still routes to
/// it). No graph, no hub under the vehicle or no way to it: no route, and
/// the straight walk as before. OURS, PROVISIONAL (boarding is the port's).
pub(in super::super) fn retarget_board(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    at: Vec3,
    nt: &mut NavTick,
) {
    let (Some(g), Some(u)) = (sim.nav.as_ref(), sim.unit(me)) else {
        return;
    };
    let m = mover(sim, u);
    let dest = unit_goal_hub(sim, g, m, at);
    match (start_hub(sim, g, me), dest) {
        (Some(c), Some(d)) if g.reachable_by(c, d, m) => {
            if st.route.is_none_or(|r| r.dest_hub != d) {
                st.route = plan_leg(sim, st, me, c, d, None, nt);
            }
        }
        _ => st.route = None,
    }
}

/// A hunter's LAST leg walks at the enemy itself, not at the enemy's hub
/// ([`retarget_hunt`]): `false` restores the pre-STK2 rule, where a hunter
/// that reached the hub stood on it for the rest of the round with its quarry
/// out of sight — BF1 YAV1's last team-1 unit on the aqueduct deck, 36 m from
/// and 10 m above the last team-0 unit (queue STK2, NO OUTCOME at 60,000
/// ticks). PROVISIONAL: a port rule under the commander's Hunt, which is
/// itself the port's design (D6), not a PS2 behaviour in analysis/.
pub const HUNT_AT_ENEMY: bool = true;

/// HUNT: the nearest enemy's hub, resolved at the ENEMY'S level; on the last
/// leg, the enemy's own position ([`HUNT_AT_ENEMY`]).
///
/// DEVIATION (and a safety net): a Hunt that resolves to NOTHING — no enemy
/// alive, no hub under it, or no way to it — falls back to
/// [`retarget_phase_a`] rather than leaving the unit with no objective and no
/// route at all. Standing still forever is never the right answer to "there is
/// nobody to hunt", and Phase A's straight walk at the nearest post is what
/// the unit did before the commander existed.
pub(super) fn retarget_hunt(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    nearest_enemy: Option<Vec3>,
    nt: &mut NavTick,
) {
    let Some(u) = sim.unit(me) else { return };
    let dest = match (sim.nav.as_ref(), nearest_enemy) {
        (Some(g), Some(e)) => {
            let m = mover(sim, u);
            g.unit_hub_on(e, m, &|h| hub_on_level(sim, g, h, e.y))
                .map(|h| level_hub(sim, g, h, e))
        }
        _ => None,
    };
    let g = sim.nav.as_ref();
    let cur = g.and_then(|g| start_hub(sim, g, me));
    match (g, cur, dest) {
        (Some(g), Some(c), Some(d)) if g.reachable_by(c, d, mover(sim, u)) => {
            st.target_cp = None;
            if st.route.is_none_or(|r| r.dest_hub != d) {
                st.route = plan_leg(sim, st, me, c, d, None, nt);
            }
            // (An armed walker stops at the end of its graph: card WAI2,
            // `ride::at_drop`.)
            let walker = u
                .mount
                .and_then(|m| sim.vehicle(m.vehicle))
                .is_some_and(|v| v.kind == crate::vehicle::VehicleKind::Walker)
                && sim.seat_look(me).is_some();
            if HUNT_AT_ENEMY && !walker {
                if let (Some(r), Some(e)) = (st.route.as_mut(), nearest_enemy) {
                    if r.is_last() {
                        r.waypoint = e;
                    }
                }
            }
        }
        _ => {
            nt.unroutable = 1;
            retarget_phase_a(sim, me, st, nearest_enemy, nt);
        }
    }
}

/// Phase A's own objective rule, kept verbatim for a unit the commander has
/// not assigned yet — the nearest post not owned by my team, an unroutable one
/// skipped for the nearest routable one, and the hunt fallback when there is
/// no reachable post at all.
pub(super) fn retarget_phase_a(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    nearest_enemy: Option<Vec3>,
    nt: &mut NavTick,
) {
    let Some(u) = sim.unit(me) else { return };
    let mine = |c: &crate::cp::CommandPost| c.owner.is_some_and(|owner| sim.allied(u.team, owner));
    let pos = u.soldier.pos;
    let nearest = |pred: &dyn Fn(&crate::cp::CommandPost) -> bool| {
        sim.cps
            .iter()
            .enumerate()
            .filter(|(_, c)| pred(c))
            .min_by(|(_, a), (_, b)| {
                let da = (a.def.pos - pos).length_squared();
                let db = (b.def.pos - pos).length_squared();
                da.total_cmp(&db)
            })
            .map(|(i, _)| i as u32)
    };
    let Some(g) = sim.nav.as_ref() else {
        // No graph: exactly the pre-Phase-A placeholder.
        st.target_cp = nearest(&|c| !mine(c)).or_else(|| nearest(&|_| true));
        st.route = None;
        return;
    };

    // Capturable posts, nearest first; index breaks distance ties.
    let mut cands: Vec<(f32, u32)> = sim
        .cps
        .iter()
        .enumerate()
        .filter(|(_, c)| !mine(c))
        .map(|(i, c)| ((c.def.pos - pos).length_squared(), i as u32))
        .collect();
    cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

    // The first choice has not changed and we are already walking to it: keep
    // the leg we are on (re-planning every think would also re-roll the
    // chooser every ten ticks).
    if let (Some(t), Some(r)) = (st.target_cp, st.route) {
        if cands.first().is_some_and(|&(_, i)| i == t)
            && sim.cps.get(t as usize).and_then(|c| c.nav_hub) == Some(r.dest_hub)
        {
            return;
        }
    }

    // The START hub is [`start_hub`]'s, as for every other objective (card
    // DAGS): this is the rule that plans a bot's FIRST leg after a spawn, and
    // with the bare XZ locate a bot spawned on the ground over BF2 Dagobah's
    // cave was put on the cave's arc, 8 m under its feet, and walked its roof
    // into a prop for 150 s. The enemy's hub, the HUNT destination, is
    // resolved at the ENEMY's level (PLAN "PHASE A.2 DESIGN" decision 3).
    let m = mover(sim, u);
    if let Some(cur) = start_hub(sim, g, me) {
        for &(_, i) in &cands {
            let Some(dest) = sim.cps[i as usize].nav_hub else {
                continue;
            };
            if !g.reachable_by(cur, dest, m) {
                continue;
            }
            st.target_cp = Some(i);
            st.route = plan_leg(sim, st, me, cur, dest, None, nt);
            return;
        }
        // HUNT: nothing capturable we can reach, so go to the nearest enemy's
        // hub (design decision 6 — the RHN1 "camp on the citadel" fix).
        if let Some(e) = nearest_enemy {
            if let Some(dest) = g.unit_hub_on(e, m, &|h| hub_on_level(sim, g, h, e.y)) {
                if g.reachable_by(cur, dest, m) {
                    st.target_cp = None;
                    st.route = plan_leg(sim, st, me, cur, dest, None, nt);
                    return;
                }
            }
        }
    }
    // Off the graph entirely: the placeholder's own choice, walked straight.
    st.target_cp = nearest(&|c| !mine(c)).or_else(|| nearest(&|_| true));
    st.route = None;
}
