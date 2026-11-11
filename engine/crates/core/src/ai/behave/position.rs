//! Fight positions, hit reactions and escape probes.

use crate::ai::draw::{unit_unit, SALT_CIRCLE, SALT_GRENADE, SALT_HOLD, SALT_REACT, SALT_STYLE};
use crate::ai::state::{AiState, FightState, HoldState, LostState, NavTick, SteerPick};
use crate::ai::{
    CIRCLE_PAUSE, CIRCLE_STOP, FIGHT_SPACING, FIGHT_STEP, HOLD_CLOSE, HOLD_COMMIT,
    HOLD_MOVE_PERIOD, HOLD_SPACING, HOLD_SPOT_RADIUS, HOLD_STRAFE, HULL_PUSH_MIN, LOST_JUMP_AGAIN,
    LOST_JUMP_CLEAR, PROBE_DIST, REACT_COOLDOWN, REACT_STEP, RING_BACK, RING_SNAP, ROLL_LANE,
    ROLL_LEAD, ROLL_PAST, ROLL_REACH, STAND_CROUCH, STAND_SHARE, STUCK_PROGRESS, STUCK_WINDOW,
};
use crate::fmath;
use crate::nav;
use crate::sim::Sim;
use crate::unit::{Unit, UnitHandle};
use glam::{Vec2, Vec3};

use super::Tick;

/// Card JITR: is `p` free to hold — not within [`HOLD_SPACING`] of the point
/// another bot holds, nor of a player? OURS; the full reservation is card
/// BOTD's.
pub(super) fn spot_free(sim: &Sim, me: UnitHandle, p: Vec3) -> bool {
    spot_clear_of(sim, me, p, HOLD_SPACING, None)
}

/// [`spot_free`] at a spacing of `r` (card FGHT: [`FIGHT_SPACING`]). With
/// `team` (a stander's own) a teammate on foot with no position counts where
/// it is, when its index is the lower: of two bots that take their positions
/// on one tick the first keeps its place.
fn spot_clear_of(sim: &Sim, me: UnitHandle, p: Vec3, r: f32, team: Option<u8>) -> bool {
    sim.alive_units().all(|(h, o)| {
        let at = match o.ai.hold.filter(|x| sim.tick < x.until) {
            _ if h == me || o.mount.is_some() => return true,
            Some(x) => x.goal,
            None if o.is_player => o.soldier.pos,
            None if team == Some(o.team) && h.index < me.index => o.soldier.pos,
            None => return true,
        };
        nav::xz_dist(at, p) >= r
    })
}

/// Card FGHT: the position a bot FIGHTS from, a seen target `e` at `range`
/// inside its weapon's reach (`analysis/re-soldier-ai.md` 4.4, 4.5). Each
/// bot has a fixed style, as on the PS2 (its controller's own number): it
/// STANDS to shoot — a third of the standers crouched — while the target is
/// inside the middle of the weapon's band, or it CIRCLES: it walks to a
/// point on a ring about the target (a random bearing, the radius drawn
/// round the weapon's optimal range), pauses 1.9 s there, or by
/// [`CIRCLE_STOP`] a think on the way, and takes the next. A scoped weapon stands wherever its
/// target is in range (card FGHTA; [`stands_at`]). The PS2's
/// zig-zag and flank styles need 12.5 and 17.5 m of open ground and are 2-5 %
/// of its attack time and are not built. The Tactician first tries a table
/// position (card TPOS); these styles are the fallback when none passes.
/// The position is a [`HoldState`], kept [`HOLD_CLOSE`] past the last tick
/// the target was in reach.
///
/// OURS (the maintainer, 2026-10-05): a stander does not stop beside another fighter
/// ([`stand_spot`]), and a ring is no wider than the bot's own distance
/// ([`ring_point`]).
pub(super) fn fight_position(
    sim: &Sim,
    tk: &Tick,
    st: &mut AiState,
    e: &Unit,
    range: f32,
    thinking: bool,
    nt: &mut NavTick,
) {
    let Tick { me, u, wc, .. } = *tk;
    let pos = u.soldier.pos;
    let (stander, crouches, anywhere) = style(tk);
    // Card HVYW: against a vehicle every bot stands where it is and shoots
    // (the PS2's stand mode and its band's stop, 4.4 / 4.5; its flank and
    // behind modes are not built) - no ring 75 m round a tank.
    let stands = e.mount.is_some() || (stander && (anywhere || stands_at(wc, range)));
    let live = st.hold.filter(|h| sim.tick < h.until);
    let mut h = live.unwrap_or(HoldState {
        spot: pos,
        goal: pos,
        strafe_side: if unit_unit(me.index, sim.tick, me.gen, SALT_HOLD ^ 4) < 0.5 {
            -1
        } else {
            1
        },
        shuffle_at: sim.tick + HOLD_MOVE_PERIOD,
        last_contact: sim.tick,
        ..HoldState::default()
    });
    if live.is_none() {
        if st.fight.mode == FightState::TAKE {
            nt.tpos_ev |= super::tactic::EV_WALK_DROPPED;
        }
        super::tactic::release(st, sim.tick);
        st.fight.mode = 0;
    }
    h.until = h.until.max(sim.tick + HOLD_CLOSE);
    // Card TPOS: the Tactician's position, when the table has one for this
    // fight; else the bot's own style below. Not for a sniper on its spot
    // with a scope's sight (not Easy): it fights from there (OURS,
    // 2026-10-29: with a scope's band it walked off its perch to a table
    // position nearer the target).
    let perched =
        super::snipe::holds(st, live) && crate::perception::scope_share(u, &tk.tier) > 0.0;
    if !perched
        && super::tactic::fight(
            sim,
            tk,
            st,
            &super::tactic::Foe::seen(e),
            &mut h,
            thinking,
            nt,
        )
    {
        // (In the open, a croucher by style crouches.)
        if !st.tpos.covered && crouches && h.stance_mask == 0 {
            h.stance_mask = crate::commander::MASK_CROUCH;
        }
        st.hold = Some(h);
        return;
    }
    let ring = |h: &mut HoldState, st: &mut AiState, nt: &mut NavTick| {
        // Card TEAM: with no ring point and the target out of reach, walk
        // at the target. Pausing here left BF1 Kashyyyk's last four soldiers
        // 57 m apart in water that refused every ring point (Hard, seed 2).
        let point = ring_point(sim, tk, e.soldier.pos).or_else(|| {
            (nav::xz_dist(pos, e.soldier.pos) >= wc.ai_range[2]).then_some(e.soldier.pos)
        });
        match point {
            Some(p) => {
                h.spot = p;
                h.goal = p;
                h.commit_until = sim.tick + HOLD_COMMIT;
                h.blocked = 0;
                st.fight.mode = FightState::WALK;
                // Over the graph to the point's hub, straight at it from there.
                super::route::retarget_board(sim, me, st, p, nt);
            }
            None => {
                h.settle(pos);
                st.fight.mode = FightState::PAUSE;
                st.fight.pause_until = sim.tick + CIRCLE_PAUSE;
            }
        }
    };
    match (stands, st.fight.mode) {
        // Card REACT: on its way to the position a hit sent it to (its
        // arrival is the mover's answer, in `think`, as a ring point's).
        (_, FightState::MOVE) => {}
        (true, FightState::STAND) => {}
        (true, mode) => {
            // A position it holds already (a hub's, card HOLD) is the one
            // it stands on; else, and after a circle, a spot of its own.
            if live.is_none() || mode != 0 {
                let p = stand_spot(sim, me, u);
                h.spot = p;
                h.goal = p;
                h.commit_until = sim.tick + HOLD_COMMIT;
                h.blocked = 0;
            }
            if h.stance_mask == 0 && crouches {
                h.stance_mask = crate::commander::MASK_CROUCH;
            }
            st.fight.mode = FightState::STAND;
        }
        // On its way (its arrival is the mover's answer, in `think`).
        (false, FightState::WALK) => {
            if thinking && unit_unit(me.index, sim.tick, me.gen, SALT_CIRCLE) < CIRCLE_STOP {
                h.settle(pos);
                st.fight.mode = FightState::PAUSE;
                st.fight.pause_until = sim.tick + CIRCLE_PAUSE;
            }
        }
        (false, FightState::PAUSE) => {
            if thinking && sim.tick >= st.fight.pause_until {
                ring(&mut h, st, nt);
            }
        }
        (false, _) => {
            h.stance_mask = 0;
            ring(&mut h, st, nt);
        }
    }
    st.hold = Some(h);
}

/// Does a bot whose style is to stand STAND with its target at `range`, or
/// take a ring point as a circler does? Inside the middle of the weapon's
/// band (the PS2's "2 becomes 3 outside the band", `analysis/re-soldier-ai.md`
/// 4.4) — and, card FGHTA, a scoped weapon anywhere between its `MinRange`
/// and `MaxRange`: the PS2 gives a class of UnitType scout with a rifle in
/// range mode 2 before that draw is made (`FUN_002abc38`, B2:231230-231234).
/// OURS, two named deviations: the PS2 keys on the soldier's CLASS, the port
/// on the weapon in hand ([`crate::weapon::WeaponClass::ai_distance_table`]);
/// and out of range the PS2's scout does not attack, the port's circles.
fn stands_at(wc: &crate::weapon::WeaponClass, range: f32) -> bool {
    let [min, opt, max] = wc.ai_range;
    (wc.ai_distance_table() && range > min && range < max)
        || (range > (min + opt) * 0.5 && range < (max + opt) * 0.5)
}

/// Card FGHT: a bot's fixed fighting style — does it STAND to shoot (else it
/// circles), and does it crouch when it stands? One draw per unit and life,
/// as the PS2's is its controller's own number (`analysis/re-soldier-ai.md`
/// 4.4): [`STAND_SHARE`] of the bots stand, a scoped weapon always, and
/// [`STAND_CROUCH`] of the standers crouch. The third answer is a test
/// scene's only: its standers stand at any range, not just inside the band.
fn style(tk: &Tick) -> (bool, bool, bool) {
    #[cfg(test)]
    if let Some((stands, anywhere)) = tests::STYLE.with(std::cell::Cell::get) {
        return (stands, false, anywhere);
    }
    let Tick { me, u, wc, .. } = *tk;
    let (stander, crouches) = style_of(me, wc.ai_distance_table());
    // Card DPRX: a proxy that strafes fights on the move (a scope stands).
    if u.proxy_human.is_some_and(|p| p.circles) && !wc.ai_distance_table() {
        return (false, false, false);
    }
    (stander, crouches, false)
}

/// [`style`]'s draw: (stands, crouches) of unit `me`; every tier the same.
fn style_of(me: UnitHandle, scoped: bool) -> (bool, bool) {
    (
        scoped || unit_unit(me.index, 0, me.gen, SALT_STYLE) < STAND_SHARE,
        unit_unit(me.index, 0, me.gen, SALT_STYLE ^ 1) < STAND_CROUCH,
    )
}

/// Card FGHT: the ring point a circling bot is walking to, while its fight's
/// position is live.
pub(super) fn ring_walk(sim: &Sim, st: &AiState) -> Option<Vec3> {
    st.hold
        .filter(|h| st.fight.walks() && sim.tick < h.until)
        .map(|h| h.goal)
}

/// Card FGHT: a point of the circle's ring about `target` this bot can walk
/// to — a random bearing, the radius drawn in the PS2's band round the
/// weapon's optimal range (`FUN_002d9228`: opt - 0.2 (opt - min) to opt +
/// 0.2 (range - opt)): on the level, a floor a slope from here leads to,
/// dry, in no barrier, nobody else's. Four draws, each also tried half way;
/// `None` = it stays where it is this time. The PS2 hands the point to its
/// path finder untested; walls on the way are the mover's (a way that is
/// shut ends the walk where the bot stands).
///
/// OURS, PROVISIONAL, two rules. With a graph the point is taken in the hub
/// nearest the drawn one ([`RING_SNAP`], a drawn point of the hub's disc on
/// the hub's own floor): the walk stays on the authored graph. And the ring
/// is no wider than the bot's own distance from its target, a hub's point
/// no more than [`RING_BACK`] farther — a circling bot does not back away
/// from an enemy it has closed with (the PS2's ring does: 19-32 m for BF2's
/// rifle whatever the distance). The bearing is any, as the PS2's: a point
/// beyond the target is walked to (kept off the far side, 8 seeds x 32 maps
/// lost the gain in contact: 11.2 -> 10.2 % against the old 10.0 %).
fn ring_point(sim: &Sim, tk: &Tick, target: Vec3) -> Option<Vec3> {
    let Tick { me, u, wc, .. } = *tk;
    let pos = u.soldier.pos;
    let [min, opt, max] = wc.ai_range;
    let d = nav::xz_dist(pos, target);
    let (lo, hi) = (
        (opt - 0.2 * (opt - min)).min(d),
        (opt + 0.2 * (max - opt)).min(d),
    );
    let draw = |k: u32| unit_unit(me.index, sim.tick, me.gen, SALT_CIRCLE ^ k);
    (0..4u32).find_map(|k| {
        let (sa, ca) = fmath::sin_cos(draw(2 * k + 2) * std::f32::consts::TAU);
        let far = target + Vec3::new(ca, 0.0, sa) * (lo + (hi - lo) * draw(2 * k + 3));
        [1.0f32, 0.5].into_iter().find_map(|f| {
            let q = pos + (far - pos) * f;
            let rise = 1.0 + 0.35 * nav::xz_dist(pos, q);
            // With a graph: in the nearest hub a slope from here leads to.
            let q = match sim.nav.as_ref() {
                Some(g) => {
                    let (_, i) = (0..g.hubs.len() as u16)
                        .filter_map(|i| {
                            let hub = g.hub(i)?;
                            let floor = sim.nav_floor(i)?;
                            let d = nav::xz_dist(hub.pos, q);
                            ((floor - pos.y).abs() <= rise && d <= RING_SNAP).then_some((d, i))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?;
                    let hub = g.hub(i)?;
                    let (sb, cb) = fmath::sin_cos(draw(2 * k + 10) * std::f32::consts::TAU);
                    let r = hub.radius.min(HOLD_SPOT_RADIUS) * draw(2 * k + 11).sqrt();
                    Vec3::new(hub.pos.x + cb * r, pos.y, hub.pos.z + sb * r)
                }
                None => q,
            };
            let len = nav::xz_dist(pos, q);
            // (A hub's point may lie beyond the ring: still no backing away.)
            if len < 2.0 || nav::xz_dist(q, target) > d + RING_BACK {
                return None;
            }
            let y = sim.ground_point_under(Vec3::new(q.x, pos.y + rise, q.z), false);
            let q = Vec3::new(q.x, y, q.z);
            let ok = (y - pos.y).abs() <= rise
                && !sim.level_at(q).is_off()
                && sim.still_water_under(q).is_none_or(|w| y >= w - 0.2)
                && sim.nav.as_ref().is_none_or(|g| {
                    !g.barriers
                        .iter()
                        .any(|b| b.blocks(nav::SOLDIER) && b.contains(Vec2::new(q.x, q.z)))
                })
                && spot_free(sim, me, q);
            ok.then_some(q)
        })
    })
}

/// Card FGHT, OURS (the maintainer, 2026-10-05): where a bot STANDS to shoot — where
/// it is, unless that is within [`FIGHT_SPACING`] of another fighter's
/// position or of a player: then the first of eight drawn points within
/// [`FIGHT_STEP`] that is clear of them and can be stepped to.
fn stand_spot(sim: &Sim, me: UnitHandle, u: &Unit) -> Vec3 {
    let pos = u.soldier.pos;
    let clear = |p: Vec3| spot_clear_of(sim, me, p, FIGHT_SPACING, Some(u.team));
    if clear(pos) {
        return pos;
    }
    (0..8u32)
        .find_map(|k| {
            let a = unit_unit(me.index, sim.tick, me.gen, SALT_STYLE ^ (2 * k + 2));
            let r = unit_unit(me.index, sim.tick, me.gen, SALT_STYLE ^ (2 * k + 3));
            let (sa, ca) = fmath::sin_cos(a * std::f32::consts::TAU);
            let p = pos + Vec3::new(ca, 0.0, sa) * (FIGHT_STEP * (0.4 + 0.6 * r));
            (stand_ok(sim, u, pos, p) && clear(p)).then_some(p)
        })
        .unwrap_or(pos)
}

/// Card HOLD: can a soldier step from `from` to `p` and stand there — a floor
/// within a metre of `from`'s height, no wall on the way, no `BARR` barrier
/// crossed?
fn stand_ok(sim: &Sim, u: &Unit, from: Vec3, p: Vec3) -> bool {
    let f = sim.ground_point_under(Vec3::new(p.x, from.y + 1.0, p.z), false);
    if (f - from.y).abs() > 1.0 {
        return false;
    }
    let d = Vec3::new(p.x - from.x, 0.0, p.z - from.z);
    let len = d.length();
    if len < 1e-3 {
        return true;
    }
    let mut cap = u.capsule().stepped();
    cap.base = Vec3::new(from.x, cap.base.y - u.soldier.pos.y + from.y, from.z);
    if sim.collision.wall_sweep(&cap, d / len, len).is_some() {
        return false;
    }
    sim.nav.as_ref().is_none_or(|g| {
        g.barrier_stop(Vec2::new(from.x, from.z), Vec2::new(p.x, p.z), nav::SOLDIER) >= 1.0
    })
}

/// Card REACT (the maintainer, 2026-10-07: "if it doesn't die after being hit, it
/// just stays standing there"): what a bot on a position does about a HIT —
/// one draw per hit outside the cooldown ([`REACT_COOLDOWN`]: a burst is one
/// draw) over its tier's row ([`crate::skill::Tier::react`]): it STAYS, it
/// STRAFES ([`HOLD_STRAFE`] aside, the other way from its last step), it
/// ROLLS aside and stands where the roll ends, or it MOVES to a new
/// position — a point of the ring round the target it sees
/// ([`ring_point`], walked as a circler's; [`FightState::MOVE`]), and with
/// no target in sight, a scoped weapon or no ring point, a spot within
/// [`REACT_STEP`], in a hub of the graph as the roll's end is ([`in_hub`]).
/// A roll with no floor for it and a move with nowhere to go are a strafe; a bot already walking to a ring point rolls or walks on.
/// It answers the HIT and never looks for the shooter: the target is the
/// one it already sees. Returns the roll's side for the mover (0 = none).
///
/// OURS, a named deviation (`analysis/bot-design.md` 9): the PS2's soldier
/// ignores a damage message while it attacks (`analysis/re-soldier-ai.md`,
/// the fight row) and re-plans by its skill's `lerp(0, 0.2)` coin otherwise —
/// that coin is still the dodge of a bot walking its leg (`think`).
pub(super) fn react(
    sim: &Sim,
    tk: &Tick,
    st: &mut AiState,
    mut h: HoldState,
    enemy: Option<&Unit>,
    nt: &mut NavTick,
) -> i8 {
    let Tick {
        me, u, wc, tier, ..
    } = *tk;
    st.dodge_until = sim.tick + REACT_COOLDOWN;
    let [strafes, rolls, moves] = tier.react;
    let x = unit_unit(me.index, sim.tick, me.gen, SALT_REACT);
    if x >= strafes + rolls + moves {
        nt.levers |= crate::ai::census::LV_REACT_STAY;
        return 0;
    }
    let pos = u.soldier.pos;
    let walking = st.fight.walks();
    let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
    let right = Vec3::new(cy, 0.0, -sy);
    let side: i8 = if h.strafe_side > 0 { -1 } else { 1 };
    if x < moves && !walking {
        let ring = enemy
            .filter(|_| st.fight.mode != 0 && st.cmd.is_none() && !wc.ai_distance_table())
            .and_then(|e| ring_point(sim, tk, e.soldier.pos))
            .filter(|p| !sim.death_regions.iter().any(|r| r.contains(*p + Vec3::Y)));
        if let Some(p) = ring {
            h.spot = p;
            h.goal = p;
            h.commit_until = sim.tick + HOLD_COMMIT;
            h.blocked = 0;
            st.fight.mode = FightState::MOVE;
            super::route::retarget_board(sim, me, st, p, nt);
            st.hold = Some(h);
            nt.levers |= crate::ai::census::LV_REACT_MOVE;
            return 0;
        }
        let near = (0..8u32).find_map(|k| {
            let a = unit_unit(me.index, sim.tick, me.gen, SALT_REACT ^ (2 * k + 2));
            let r = unit_unit(me.index, sim.tick, me.gen, SALT_REACT ^ (2 * k + 3));
            let (sa, ca) = fmath::sin_cos(a * std::f32::consts::TAU);
            let p = pos + Vec3::new(ca, 0.0, sa) * (REACT_STEP * (0.5 + 0.5 * r));
            (react_ok(sim, u, p) && in_hub(sim, p) && spot_free(sim, me, p)).then_some(p)
        });
        if let Some(p) = near {
            h.spot = p;
            h.step_to(p, sim.tick, true);
            st.hold = Some(h);
            nt.levers |= crate::ai::census::LV_REACT_MOVE;
            return 0;
        }
    } else if x >= moves && x < moves + rolls {
        let lands = |s: i8| pos + right * (f32::from(s) * ROLL_REACH);
        if let Some(s) = [side, -side].into_iter().find(|s| {
            roll_clear(sim, u, *s) && react_ok(sim, u, lands(*s)) && in_hub(sim, lands(*s))
        }) {
            nt.levers |= crate::ai::census::LV_REACT_ROLL;
            if walking {
                return s;
            }
            // It sets off for where the roll ends, stands there, and rolls
            // once it is under way (`think`).
            let p = lands(s);
            h.spot = p;
            h.step_to(p, sim.tick, true);
            h.strafe_side = s;
            h.roll_at = sim.tick + ROLL_LEAD;
            st.hold = Some(h);
            return 0;
        }
    }
    let mut strafed = false;
    if !walking {
        for s in [side, -side] {
            let p = pos + right * (f32::from(s) * HOLD_STRAFE);
            if react_ok(sim, u, p) && spot_free(sim, me, p) {
                h.step_to(p, sim.tick, true);
                h.strafe_side = s;
                st.hold = Some(h);
                strafed = true;
                break;
            }
        }
    }
    nt.levers |= if strafed {
        crate::ai::census::LV_REACT_STRAFE
    } else {
        crate::ai::census::LV_REACT_STAY
    };
    0
}

/// Card REACT: may a bot on a position step, roll or move from where it
/// stands to `p` (a hit reaction or a step out of a friend's line) —
/// [`stand_ok`], `p` on the level, and
/// no metre of the way inside a mission death region?
pub(super) fn react_ok(sim: &Sim, u: &Unit, p: Vec3) -> bool {
    let from = u.soldier.pos;
    let n = nav::xz_dist(from, p).ceil().max(1.0) as u32;
    stand_ok(sim, u, from, p)
        && !sim.level_at(p).is_off()
        && (1..=n).all(|k| {
            let q = from + (p - from) * (k as f32 / n as f32);
            let q = Vec3::new(q.x, from.y, q.z);
            !sim.death_regions
                .iter()
                .any(|r| r.contains(q) || r.contains(q + Vec3::Y))
        })
}

/// Card REACT: does `p` lie in a hub of the graph, on the hub's floor (any
/// point, with no graph)? A roll or a move ends on the authored graph: on
/// Bespin Platforms 4 of 55 rolls that ended beside it were followed by a
/// fall from the walk that came next (4 seeds, traced).
fn in_hub(sim: &Sim, p: Vec3) -> bool {
    sim.nav.as_ref().is_none_or(|g| {
        (0..g.hubs.len() as u16).any(|i| {
            g.hub(i)
                .is_some_and(|hub| nav::xz_dist(hub.pos, p) <= hub.radius)
                && sim.nav_floor(i).is_some_and(|f| (f - p.y).abs() <= 1.0)
        })
    })
}

/// Card HOLD: is there [`ROLL_REACH`] of floor to roll over toward `side`
/// (+1 right, −1 left of the facing) — level floor every metre and to each
/// side of the line, no wall, no barrier?
fn roll_clear(sim: &Sim, u: &Unit, side: i8) -> bool {
    let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
    let dir = Vec3::new(cy, 0.0, -sy) * f32::from(side);
    let fwd = Vec3::new(-sy, 0.0, -cy);
    let pos = u.soldier.pos;
    // Card REACT: a roll that carries (it did not before: `ROLL_LEAD`) ends
    // a metre or two past its reach and off its line — so floor ON THE
    // LEVEL, in a lane [`ROLL_LANE`] to each side, to [`ROLL_PAST`] beyond
    // (the first sweep with rolls that travel: Bespin Platforms' falls
    // 196 -> 261 in 32 rounds).
    let n = (ROLL_REACH + ROLL_PAST).ceil() as u32;
    (1..=n).all(|k| {
        [-ROLL_LANE, 0.0, ROLL_LANE].into_iter().all(|lane| {
            let p = pos + dir * k as f32 + fwd * lane;
            (sim.ground_point_under(p + Vec3::Y, false) - pos.y).abs() <= 1.0
                && !sim.level_at(Vec3::new(p.x, pos.y, p.z)).is_off()
        })
    }) && stand_ok(sim, u, pos, pos + dir * ROLL_REACH)
}

/// Card HOLD: the point to run to from a grenade on the ground
/// ([`crate::sim::Hazard`]) — the nearest one this bot stands inside the
/// warning radius of, has noticed (the tier's chance, one coin per bot per
/// grenade) and has had the tier's reaction delay to act on: straight away
/// from it, to 3 m past its radius. The PS2 warns every AI unit within 7.5 m
/// after a difficulty-ranked delay (re-grenades.md §7.4); what its
/// controller then does is not traced, and running clear is ours.
pub(super) fn hazard_flee(
    sim: &Sim,
    me: UnitHandle,
    u: &Unit,
    tier: &crate::skill::Tier,
) -> Option<Vec3> {
    let pos = u.soldier.pos;
    let mut best: Option<(f32, Vec3)> = None;
    for z in &sim.hazards {
        let d = pos - z.pos;
        let dist = Vec2::new(d.x, d.z).length();
        if dist >= z.radius || d.y.abs() > z.radius {
            continue;
        }
        // Card GRAI (OURS): the thrower knows its own grenade — no coin, no
        // delay (the PS2 leaves the thrower out of the warning altogether).
        let own = z.owner == Some(me);
        if !own && unit_unit(me.index, z.since, me.gen, SALT_GRENADE) >= tier.grenade {
            continue;
        }
        let (lo, hi) = tier.grenade_delay;
        let wait = unit_unit(me.index, z.since, me.gen, SALT_GRENADE ^ 1);
        if !own && sim.tick < z.since + lo + ((hi - lo) as f32 * wait) as u64 {
            continue;
        }
        let away = if dist > 1e-3 {
            Vec3::new(d.x, 0.0, d.z) / dist
        } else {
            // On top of it: back the way the bot faces away from.
            let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
            Vec3::new(sy, 0.0, cy)
        };
        if best.is_none_or(|(bd, _)| dist < bd) {
            best = Some((dist, pos + away * (z.radius - dist + 3.0)));
        }
    }
    best.map(|b| b.1)
}

/// Card BNDS: should a lost bot JUMP this tick? Its way `d` is shut by a wall
/// low enough to clear (the sweep of its capsule lifted [`LOST_JUMP_CLEAR`]
/// is open) — a rim rail from outside, a kerb over the step. It jumps from
/// where it stands: the stick steers a jump in the air (card AIRC), and on
/// BF2 Kamino's 50 rim stops a standing jump with the stick held clears
/// every one a running jump does. One try per [`LOST_JUMP_AGAIN`]
/// (`LostState::jump_after`); between tries the steering slides it along.
pub(in crate::ai) fn lost_jump(
    sim: &Sim,
    u: &Unit,
    d: Vec3,
    pick: SteerPick,
    l: &mut LostState,
) -> bool {
    if !u.soldier.grounded || sim.tick < l.jump_after {
        return false;
    }
    let cap = u.capsule();
    let wall_ahead = || {
        sim.collision
            .wall_sweep(&cap.stepped(), d, PROBE_DIST)
            .is_some()
    };
    // OURS: a pinned return can report an open probe while the full capsule
    // cancels its step on a low ledge. Use that physical contact only here.
    // trace: compare a PS2 soldier's escape at the Yavin Bazaar riser.
    let hidden_ledge = pick == SteerPick::Fwd;
    if hidden_ledge {
        if !l.pinned
            || u.droideka.is_some()
            || u.soldier.stance != crate::soldier::Stance::Stand
            || u.wall_push.dot(d) >= -HULL_PUSH_MIN
            || u.soldier.vel.dot(d).abs() * crate::TICK_DT
                >= STUCK_PROGRESS / f32::from(STUCK_WINDOW)
            || wall_ahead()
        {
            return false;
        }
    } else if !wall_ahead() {
        return false; // a drop or a barrier, not a wall
    }
    let mut over = cap;
    over.base += Vec3::Y * LOST_JUMP_CLEAR;
    if (hidden_ledge && sim.collision.ceiling_sweep(&cap, &over).is_some())
        || sim
            .collision
            .wall_sweep(&over, d, PROBE_DIST + 0.5)
            .is_some()
    {
        return false; // too high to jump
    }
    l.jump_after = sim.tick + LOST_JUMP_AGAIN;
    true
}

#[cfg(test)]
mod tests;
