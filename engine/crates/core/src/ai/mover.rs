//! The MOVER of a bot on foot: [`run`] turns the tick's one [`MoveRequest`]
//! into the move sticks — the commitment to a destination and the ARRIVED
//! latch (card MOVR: at the post, on a held position, on a point walked
//! straight at), the stand in hold range, the walk block (the side-step
//! offset, the steering's heading picked every third tick and held, the
//! short step, the last step's scale, a lost bot's jump), the routeless
//! dodge's strafe, the D13 stance controller and the dodge as a roll — and
//! then calls the navigator's stuck window ([`progress`]) and keeps a lost
//! bot's return window and the last resort's watch ([`super::stall`]). It
//! answers the walk's outcome ([`Walk`]). Card
//! SPL11, `analysis/bot-design.md` 3 and 4.7: this is the ONLY writer of a
//! foot soldier's `pad.lx` / `pad.ly` and of ROLL_STANCE and JUMP. The
//! heading itself is the avoidance's ([`super::avoid::pick`], card OBST). The
//! arrival, deadband, blocked-walk, short step and hold range tests live here.

use super::avoid;
use super::brain::behaviours::recover::lost_jump;
use super::brain::intent::Acts;
use super::brain::Tick;
use super::route::{progress, sidestep_tick, sub_point};
use super::state::{AiState, HoldState, NavTick, SteerPick, Walk, WalkAsk};
use super::steer::floor_ahead;
use super::{
    ARRIVED_SLACK, CORNER_ARRIVE, DEST_DEADBAND, HEADING_PERIOD, HOLD_BLOCKED, HOLD_ENDGAME,
    HOLD_WALK_DIST, HOLD_WALK_STICK, LEDGE_DROP, LOST_DOWN, SIDESTEP_OFFSET, STUCK_PROGRESS,
    STUCK_WINDOW,
};
use crate::cp::CommandPost;
use crate::fmath;
use crate::nav;
use crate::sim::{buttons, PadState, Sim};
use crate::soldier::Stance;
use glam::Vec3;

/// What the running behaviour asks of the mover this tick — exactly one a
/// tick (`brain::think`): today's priority chain as one value.
#[derive(Clone, Copy)]
pub(super) struct MoveRequest<'a> {
    /// Which link of the chain walks the bot ([`WalkAsk::RETURN`] ..
    /// [`WalkAsk::MEND`]), as recorded by the black box.
    pub kind: u8,
    /// The point to walk at this tick (the destination, or the navigator's
    /// point on the way to it); `None` = nowhere to be.
    pub goal: Option<Vec3>,
    /// Card MOVR: the DESTINATION and its arrive radius, when the walk ends
    /// somewhere (a held position, the last leg's point, a post with no
    /// graph). The bot stands once it is there, until this moves by more
    /// than [`DEST_DEADBAND`] or `kind` changes. `None`: a walk with no end
    /// of its own (a leg to a hub, the return to the level, a run from a
    /// grenade, the walk to a vehicle).
    pub arrive: Option<(Vec3, f32)>,
    /// The post the bot is sent to, for the arrival test on the last leg.
    pub post: Option<&'a CommandPost>,
    /// The bot defends that post: it arrives at its defence spot.
    pub defending: bool,
    /// A seen enemy is inside [`super::HOLD_RANGE`]: stand and fight.
    pub in_range: bool,
    /// A lost bot's return point (`goal` is this point then).
    pub ret: Option<Vec3>,
    /// The position held, as the tick's steps left it.
    pub hold: Option<HoldState>,
    /// The bot runs from a grenade.
    pub flee: bool,
    /// The bot has a boarding claim.
    pub boarding: bool,
    /// The bot is in reach of its repair target and ready to use its cutter.
    pub serving: bool,
    /// Excused from the last resort's watch (card RCYC).
    pub excused: bool,
    pub acts: Acts,
}

pub(super) enum MoverMode {
    Serve,
    SupplyCollect((i16, i16)),
    Droideka { stop: bool },
}

pub(super) fn finish(pad: &mut PadState, mode: MoverMode) {
    pad.lx = 0;
    match mode {
        MoverMode::Serve => pad.ly = 0,
        MoverMode::SupplyCollect((lx, ly)) => {
            pad.lx = lx;
            pad.ly = ly;
        }
        MoverMode::Droideka { stop } => {
            pad.buttons &=
                !(buttons::ROLL_STANCE | buttons::JUMP | buttons::SPRINT | buttons::ACTION);
            if stop {
                pad.ly = 0;
            }
        }
    }
}

/// What [`run`] answers: the walk's outcome, and whether the bot stands AT
/// ITS POST (the behaviour's dwell counter reads it).
#[derive(Clone, Copy)]
pub(super) struct Moved {
    pub walk: Walk,
    pub at_post: bool,
}

/// Is this arc one a soldier jumps at the start of? `Arc::JET_JUMP` is `ai::jet`'s.
fn arc_jumps(sim: &Sim, arc: u16) -> bool {
    sim.nav
        .as_ref()
        .and_then(|g| g.arcs.get(usize::from(arc)))
        .is_some_and(|a| a.flags & crate::nav::Arc::JUMP != 0)
}

/// One tick of the mover: the move sticks, the stance, the roll and the jump
/// for `mv`, then the progress books. Card MOVR (`analysis/bot-design.md`
/// 4.7): a bot that has arrived STANDS (the latch and the deadband), the last
/// step is scaled so it does not overshoot (the PS2, `FUN_00249f80`), the
/// heading is picked every [`HEADING_PERIOD`]-th tick and held between, and
/// a progress window with no progress is a FAILED walk ([`Walk::Failed`]) —
/// what is done about it beyond the window's own re-plan and side-step is
/// the behaviour's (the recovery ladder).
pub(super) fn run(
    sim: &Sim,
    tk: &Tick,
    st: &mut AiState,
    mv: &MoveRequest,
    pad: &mut PadState,
    nt: &mut NavTick,
) -> Moved {
    let Tick { me, u, .. } = *tk;
    let pos = u.soldier.pos;
    if super::jet::run(sim, tk, st, mv, pad) {
        return Moved {
            walk: Walk::Following,
            at_post: false,
        };
    }
    let MoveRequest {
        goal,
        post,
        defending,
        ret,
        acts: Acts { roll, .. },
        ..
    } = *mv;
    let (walk_kind, holding, hold) = (mv.kind, mv.hold, mv.in_range);
    let route_leg = st.route().filter(|r| !r.is_last());

    // THE COMMITMENT. A destination within the deadband of the one the mover
    // is committed to (and asked by the same link of the chain) is that one
    // still: a request that barely moves neither restarts the walk nor ends
    // an arrival.
    let same = |p: Vec3| {
        st.mv.kind == mv.kind
            && nav::xz_dist(p, st.mv.dest) <= DEST_DEADBAND
            && (p.y - st.mv.dest.y).abs() <= SPOT_FLOOR_TOL
    };
    match mv.arrive {
        Some((p, _)) if same(p) => {}
        Some((p, _)) => {
            st.mv.dest = p;
            st.mv.kind = mv.kind;
            st.mv.arrived = false;
        }
        None => {
            st.mv.dest = Vec3::ZERO;
            st.mv.kind = 0;
            st.mv.arrived = false;
        }
    }
    let dest = mv.arrive.map(|(_, r)| (st.mv.dest, r));
    // The point walked at: the committed destination when the request walks
    // straight at its own.
    let goal = match (goal, mv.arrive, dest) {
        (Some(g), Some((p, _)), Some((d, _))) if g == p => Some(d),
        _ => goal,
    };

    // ARRIVAL, in XZ, at the destination's radius — on a last leg the PS2's
    // last-node radius as RE_LOG "Arrival and locate" has it (1.0 m for an
    // EntitySoldier; re-soldier-ai 2 reads 0.5 m for BF2 — measured with the
    // port's steering, 0.5 m costs: card MOVR's report, and again card
    // MOVR2's on card OBST's steering).
    // At a post it used to be "anywhere inside the capture volume", which
    // stops a unit dead the instant it crosses the boundary — on BF1 NAB1
    // that left all sixteen attackers and one defender frozen in a ring 21 m
    // out round cp3's building, none of them able to see or shoot any other,
    // and the round never resolved.
    // Phase B: a DEFENDER's destination is its defence spot, which may be a
    // Cover/Snipe hint outside the capture volume, so it arrives at the SPOT.
    // Card STEND: ...and on the spot's own FLOOR. A bot under it (inside the
    // shell of a post's platform, where a walker left it: BF1 GEO1 seed 42,
    // 9 v 2 at 60,000 ticks) or on a roof over it (TAT1's cp2, 5 m over the
    // floor) is in the capture volume and a metre from the centre in XZ, and
    // it has not arrived: it stood there unseen and held the post contested
    // for the rest of the round.
    // Card PARR: an attacker's destination is a point of the capture region
    // too (`route::post_point`: the PS2 walks to a random point of it and is
    // at the post inside it, re-conquest-goal section 4), so it arrives at
    // its leg's point, inside the region. With no leg (no graph) the centre.
    let at_spot = |slack: f32| {
        dest.is_some_and(|(d, r)| {
            let in_region = match post {
                Some(cp) if walk_kind == WalkAsk::LEG => {
                    route_leg.is_none()
                        && ((defending && st.route().is_some()) || cp.def.capture.contains(pos))
                        && !off_floor(sim, pos, d.y)
                }
                _ => true,
            };
            in_region && nav::xz_dist(pos, d) <= r + slack
        })
    };
    // Card MOVR: THE LATCH. Arrived, the bot stands until the request
    // changes — or until it is pushed [`ARRIVED_SLACK`] out of the radius.
    if at_spot(0.0) {
        st.mv.arrived = true;
    } else if !(st.mv.arrived && at_spot(ARRIVED_SLACK)) {
        st.mv.arrived = false;
    }
    let arrived = st.mv.arrived;
    // AT ITS POST: arrived there — or, when another link of the chain walks
    // the bot (a holder on its post's point is at the post), inside the
    // latch's outer radius of the point.
    let at_post = match walk_kind {
        WalkAsk::LEG => arrived && post.is_some(),
        _ => post.is_some_and(|cp| {
            let p = st.route().map_or(cp.def.pos, |r| r.waypoint);
            route_leg.is_none()
                && ((defending && st.route().is_some()) || cp.def.capture.contains(pos))
                && nav::xz_dist(pos, p) <= CORNER_ARRIVE + ARRIVED_SLACK
                && !off_floor(sim, pos, p.y)
        }),
    };
    // A lost or a fleeing bot keeps walking; a bot with a position walks to
    // it and stands on it.
    let hold = hold && holding.is_none() && ret.is_none() && !mv.flee;
    let mut steer = SteerPick::Idle;
    // D13: is this tick a WALKING one? An engaged unit inside hold range and
    // a unit standing on its own defence spot are both staying put, and both
    // may keep an authored stance; anything with somewhere to be stands up.
    let must_walk = !hold && !arrived && goal.is_some();
    if must_walk {
        if let Some(gp) = goal {
            let mut to = Vec3::new(gp.x - pos.x, 0.0, gp.z - pos.z);
            // Card STFAL: the lowest floor a step may land on — a drop
            // below both the feet and the point walked at is a ledge (a
            // corner is at the feet's own y, `sub_point`).
            // Card BNDS: a lost bot does not walk further down.
            // Card RETW: ...but down to the point of its floor walk.
            let low = match ret {
                Some(_) => pos.y.min(gp.y) - LOST_DOWN,
                // OURS: a stale hold on a distant lower floor can pull a
                // soldier off an active route after contact ends.
                // measure: Citadel's archived shelf and native hold behavior (#108).
                None if walk_kind == WalkAsk::HOLD
                    && pos.y - gp.y > LEDGE_DROP
                    && nav::xz_dist(pos, gp) > super::HOLD_RANGE
                    && st.target.is_none()
                    && route_leg.is_some_and(|r| r.via_arc.is_some())
                    && u.soldier.grounded =>
                {
                    pos.y - LOST_DOWN
                }
                None => pos.y.min(gp.y) - LEDGE_DROP,
            };
            // The PS2's contact side-step (`FUN_0026aba8`) offsets the move
            // TARGET 5 m along the path perpendicular while it lasts; ours does
            // the same rather than jamming the strafe axis, so the steering
            // below still gets to probe where it is sending the unit. Card
            // STFAL: not over a drop — a side-step with no floor ahead is
            // dropped for the straight line.
            // Card JITR: nor on the way to a held position — that is not
            // the leg the side-step belongs to.
            let stepping = ret.is_none() && holding.is_none() && walk_kind != WalkAsk::PICKUP;
            if let Some(r) = st.route().filter(|r| r.sidestep.abs() > 1 && stepping) {
                let d = to.normalize_or_zero();
                if d != Vec3::ZERO {
                    let off = to
                        + Vec3::new(-d.z, 0.0, d.x)
                            * (f32::from(r.sidestep.signum()) * SIDESTEP_OFFSET);
                    if floor_ahead(sim, u, off.normalize_or_zero(), low) {
                        to = off;
                    }
                }
            }
            let d = to.normalize_or_zero();
            if d != Vec3::ZERO {
                let side = st
                    .route()
                    .map_or(1, |r| if r.sidestep < 0 { -1 } else { 1 });
                let mut slide = st.slide;
                // Card OBST: the point the walk is measured against — the
                // leg's own point (a corner, the waypoint), not the point
                // ahead on its line.
                let aim = match st.route() {
                    Some(r) if walk_kind == WalkAsk::LEG => sub_point(&r, pos),
                    _ => gp,
                };
                st.walk = WalkAsk {
                    kind: walk_kind,
                    goal: gp,
                    dir: d,
                    low,
                };
                // Card MOVR: the heading is PICKED on this bot's tick of
                // every [`HEADING_PERIOD`] and held between: a clear way is
                // walked straight at the point, a way round is kept as it
                // was picked. (A lost bot's jump asks every tick.)
                let picks = (sim.tick + u64::from(me.index)).is_multiple_of(HEADING_PERIOD)
                    || st.steer == SteerPick::Idle
                    || st.mv.heading == Vec3::ZERO
                    || ret.is_some();
                let (mut m, pick) = if picks {
                    avoid::pick(sim, u, d, low, aim, &mut slide, side, &mut st.mv.round, nt)
                } else if st.steer == SteerPick::Fwd {
                    (d, SteerPick::Fwd)
                } else {
                    (st.mv.heading, st.steer)
                };
                st.mv.heading = m;
                st.slide = slide;
                steer = pick;
                let blocked = !matches!(pick, SteerPick::Fwd | SteerPick::Round);
                if blocked {
                    nt.blocked = 1;
                }
                if st.mv.round.live() {
                    st.mv.round.ticks = st.mv.round.ticks.saturating_add(1);
                }
                // The PS2's last step (`FUN_00249f80`, re-soldier-ai 1.5): a
                // destination nearer than one step is walked at distance /
                // step of the stick, so the step does not overshoot.
                if dest.is_some_and(|(p, _)| p == gp) {
                    let step = u.consts.max_speed * crate::TICK_DT;
                    let dist = to.length();
                    if dist < step {
                        m *= dist / step;
                    }
                }
                // Card TEAM: on its way, a step behind a team-mate walking
                // ahead the same way (`team::follow_ease`) - not on the way
                // to a position, back to the level or away from a grenade.
                if holding.is_none() && ret.is_none() && !mv.flee {
                    m *= super::team::follow_ease(sim, me, m);
                }
                // Card JITR: on the way to a held position a short step is
                // WALKED, and a goal the steering cannot go straight at is
                // given up for where the bot stands.
                if let Some(mut h) = holding {
                    if to.length() <= HOLD_WALK_DIST {
                        m *= HOLD_WALK_STICK;
                    }
                    // (Card OBSF: a way ROUND a body counts too — a hold
                    // point beside a crate was circled along the crate's
                    // rim, the maintainer's Kamino scene: 17 reversals in 10 s.)
                    if pick != SteerPick::Fwd {
                        h.blocked = h.blocked.saturating_add(1);
                        if h.blocked >= HOLD_BLOCKED {
                            h.settle(pos);
                        }
                        st.hold = Some(h);
                    }
                }
                if let (Some(mut l), true) = (st.lost, ret.is_some()) {
                    if lost_jump(sim, u, d, pick, &mut l) {
                        pad.buttons |= buttons::JUMP;
                        m = d;
                    } else if u.soldier.stance == Stance::Jump {
                        // In the air the stick stays AT the wall: it steers
                        // the jump over it (card AIRC), where the slide
                        // would carry him along its face.
                        m = d;
                    }
                    st.lost = Some(l);
                }
                // A leg on an arc flagged to jump starts with a jump toward its
                // far end (BF2 `FUN_002c0220`, `FUN_002f1778`, RE_LOG "BF2 arc
                // flags"), and the stick stays on that line in the air.
                let jump_arc = st
                    .route()
                    .and_then(|r| r.via_arc)
                    .filter(|&a| arc_jumps(sim, a));
                if st.mv.jumped != jump_arc {
                    match jump_arc {
                        Some(_) if !u.soldier.grounded => {}
                        Some(_) => {
                            pad.buttons |= buttons::JUMP;
                            st.mv.jumped = jump_arc;
                            m = d;
                        }
                        None => st.mv.jumped = None,
                    }
                } else if jump_arc.is_some() && !u.soldier.grounded {
                    m = d;
                }
                // The pad is built from the chosen WORLD direction in the
                // soldier's own frame (`Soldier::step`: fwd_axis = (-sin yaw,
                // -cos yaw), right_axis = (cos yaw, -sin yaw)), so the unit
                // moves the intended way while its yaw catches up, with no
                // cone gating the walk (Phase A's `ADVANCE_CONE`).
                let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
                let fwd_axis = Vec3::new(-sy, 0.0, -cy);
                let right_axis = Vec3::new(cy, 0.0, -sy);
                pad.ly = PadState::quantize(m.dot(fwd_axis).clamp(-1.0, 1.0));
                pad.lx = PadState::quantize(m.dot(right_axis).clamp(-1.0, 1.0));
            }
        }
    }
    if steer == SteerPick::Idle {
        st.mv.heading = Vec3::ZERO;
    }
    st.steer = steer;
    // D11: the routeless dodge. It is applied AFTER the walk block and
    // outside its `!hold` guard on purpose — a unit holding its ground in a
    // firefight is exactly the one that has just been hit.
    if sim.tick < st.strafe_until {
        pad.lx = PadState::quantize(f32::from(st.strafe_side));
    } else {
        st.strafe_side = 0;
    }

    // D13: THE STANCE CONTROLLER. The levers are the contextual ROLL_STANCE
    // button (`sim::buttons`) and JUMP. With the stick down the soldier's
    // own tap rule (`Soldier::stance_tap_target`, `[ours]` 2026-10-28) is:
    // a stand crouches; a crouch goes prone on the second tap of a double
    // tap (within `PRONE_DOUBLE_TAP` of the first) and stands up after it; a
    // prone stands up. JUMP stands a crouch or a prone up. So: crouch = one
    // tap, prone = two taps a couple of ticks apart, up = one tap or JUMP.
    //
    // A tap is ONE press on ONE tick, spent only where the soldier will take
    // it (`Soldier::stance_tap_after`: no lock, the button released last
    // tick, the tap's wait). MEASURED — pressing through a prone
    // transition's lock instead (`prone_down_lock`, 40 ticks) leaves the
    // button HELD when the lock clears, so no edge ever arrives.
    //
    // A unit that has to WALK stands up FIRST, and gives up the tick's
    // movement to do it: with the stick down a crouched unit that presses
    // ROLLS instead of standing, and a PRONE one is refused the roll. A
    // crouched one stands with JUMP: a tap inside the double-tap window
    // would put it PRONE (the overshoot: 571 prone entries over 4 maps, none
    // wanted, under the old Stand -> Crouch -> Prone cycle). No press ever
    // lands a unit prone unless it wants prone.
    //
    // Card HOLD: a bot on its position that steps to another spot (a shuffle,
    // a strafe) MOVES IN ITS STANCE — crouched or prone, at that stance's
    // speed — instead of standing up for every step and going down again
    // when it gets there.
    let want = crate::commander::stance_of_code(st.want_stance);
    let stance = u.soldier.stance;
    let in_stance = holding.is_some() && want == Some(stance);
    let aim = if must_walk {
        (!in_stance && matches!(stance, Stance::Crouch | Stance::Prone)).then_some(Stance::Stand)
    } else {
        // Standing still is the soldier's own precondition, so it is ours.
        want.filter(|&w| pad.lx == 0 && pad.ly == 0 && w != stance)
    };
    if let Some(aim) = aim {
        if must_walk {
            pad.lx = 0;
            pad.ly = 0;
        }
        let dt = crate::TICK_DT;
        match u.soldier.stance_tap_after(dt) {
            Some(next) if next != Stance::Prone || aim == Stance::Prone => {
                pad.buttons |= buttons::ROLL_STANCE;
            }
            _ if aim != Stance::Prone && u.soldier.jump_stands_after(dt) => {
                pad.buttons |= buttons::JUMP;
            }
            _ => {}
        }
    }

    if mv.acts.melee_jump && !mv.flee && ret.is_none() {
        pad.buttons |= buttons::JUMP;
    }
    if mv.acts.throw_stand && !mv.flee && ret.is_none() {
        pad.lx = 0;
        pad.ly = 0;
    }

    // Cards HOLD, REACT: a hit bot's roll — the contextual button with the
    // stick hard over (`soldier.rs`: moving = roll), this one tick.
    if roll != 0 && u.soldier.grounded {
        pad.lx = PadState::quantize(f32::from(roll));
        pad.ly = 0;
        pad.buttons |= buttons::ROLL_STANCE;
    }

    // The side-step countdown, then the progress window.
    // A unit standing at a ledge (card STFAL) is still trying to walk: its
    // window runs and it re-plans. Measured (AUDIT of STFAL, 2026-10-01),
    // the re-plan hands back the same leg and the unit stays: what ends a
    // long stand is the behaviour's give-up (card MOVR, the recovery ladder),
    // then the watch below.
    let walking = (pad.lx != 0 || pad.ly != 0 || steer == SteerPick::Ledge) && !mv.boarding;
    // Card BSTK: a bot flying its remote droid stands still on purpose (the
    // droid takes its pad, `Sim::droid_pads`): no window runs, so its stand
    // is not a failed walk, and the last resort's watch waits. The same for
    // an engineer cutting its claimed turret back up (card TBLD).
    let piloting = sim.controlled_droid(me).is_some() || mv.serving;
    let mut failed = false;
    let mut lost_failed = false;
    match (ret, st.lost) {
        _ if piloting => {}
        // A bot on its hold position, or running from a grenade, is not
        // walking its leg: the leg's progress window waits.
        // Card JITR: ...but a side-step it brought along still runs down.
        _ if ret.is_none() && (holding.is_some() || mv.flee) => {
            sidestep_tick(st);
        }
        // The return walk's own progress window: no nearer its hub after a
        // window, the next hub.
        (Some(rp), Some(mut l)) => {
            l.window += 1;
            if l.window >= STUCK_WINDOW {
                l.window = 0;
                let d = nav::xz_dist(pos, rp);
                if l.ref_dist > 0.0 && l.ref_dist - d < STUCK_PROGRESS {
                    // Card RETW: a floor walk that got nowhere is looked
                    // for again from here (`brain::think`), and given up
                    // for the hubs after `RETURN_PLANS`.
                    if l.path_at < l.path.len {
                        l.path_at = l.path.len;
                    } else {
                        l.tries = l.tries.wrapping_add(1);
                    }
                    l.ref_dist = 0.0;
                    lost_failed = true;
                    nt.failed(super::why::Why::ReturnFailed);
                } else {
                    l.ref_dist = d;
                }
            }
            st.lost = Some(l);
        }
        _ => {
            // Card MOVR: a window with no progress is a FAILED walk, and a
            // window with progress ends the recovery.
            let was = st.route().map_or(0, |r| r.stuck_run);
            // Card ARCM: back and forth at the window's own waypoint closes
            // the window now, as a failed one - not while the bot goes round
            // a body on its committed side under the allowance (card OBST:
            // along a long wall the way to the waypoint opens and closes).
            let rd = st.mv.round;
            let rounding = rd.live() && !rd.rim && rd.ticks < avoid::ROUND_MAX;
            let osc = match st.route() {
                Some(r) if walking && !mv.excused && !rounding && r.via_arc.is_some() => {
                    st.arcm.osc.step(sub_point(&r, pos), pos, sim.tick)
                }
                _ => {
                    st.arcm.osc = super::arcm::Osc::default();
                    false
                }
            };
            progress(sim, me, st, pos, walking, osc, nt);
            let now = st.route().map_or(0, |r| r.stuck_run);
            if now > was {
                failed = true;
            } else if now == 0 && st.mv.round.ticks < avoid::ROUND_MAX {
                // (Card OBST: not while the allowance of going round is
                // spent — a bot circling what it cannot leave gets nearer
                // its waypoint now and then, and its failures still count.)
                st.mv.recover = 0;
                if !st.mv.round.live() {
                    st.mv.round.ticks = 0;
                }
            }
        }
    }
    // Card RCYC: the last resort. A bot with somewhere to go that makes no
    // net progress is given up — never one that is where it wants to be, on
    // a held position, fighting an enemy it sees, running from a grenade or
    // under a squad order.
    let arrived_at_post = at_post && holding.is_none() && ret.is_none() && !mv.flee;
    // Card RLOOP: a pinned spell's walk that got nowhere is the end of it.
    let pin_failed = lost_failed && st.lost.is_some_and(|l| l.pinned);
    let pinned = super::stall::watch(
        st,
        pos,
        arrived_at_post || goal.is_none(),
        must_walk && !mv.excused && !piloting,
        sim.battle
            .teams
            .get(usize::from(u.team))
            .is_some_and(|team| team.tickets() <= HOLD_ENDGAME),
        if lost_failed { 2 } else { u8::from(failed) },
        nt,
    );
    if pin_failed {
        nt.stalled = 1;
    } else if pinned {
        super::stall::send_off(sim, st, pos, nt);
    }
    let walk = match () {
        _ if failed => Walk::Failed,
        _ if arrived && goal.is_some() => Walk::Arrived,
        _ if must_walk => Walk::Following,
        _ => Walk::Idle,
    };
    Moved { walk, at_post }
}

/// A bot standing at a spot's XZ is off the spot's floor when its feet are
/// more than this from it (m). PROVISIONAL (ours): under a soldier's height,
/// over a step and a kerb ([`super::LEVEL_HUB_REACH`]'s size).
pub const SPOT_FLOOR_TOL: f32 = 1.5;

/// Card STEND: is `pos` on ANOTHER FLOOR than a spot at height `spot_y` —
/// under the slab the spot is on, or on a roof over it? The spot's floor is
/// the one a soldier at the spot's height stands on at the bot's XZ; a spot
/// authored with no floor at its own height (buried, or in the air) has
/// none, and a bot at its XZ has arrived as before.
fn off_floor(sim: &Sim, pos: Vec3, spot_y: f32) -> bool {
    if (pos.y - spot_y).abs() <= SPOT_FLOOR_TOL {
        return false;
    }
    let floor = sim.ground_under(Vec3::new(pos.x, spot_y, pos.z), false);
    (floor - spot_y).abs() <= SPOT_FLOOR_TOL && (pos.y - floor).abs() > SPOT_FLOOR_TOL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::l_sim;
    use crate::ai::think;
    use crate::sim::Sim;
    use crate::unit::UnitHandle;
    use glam::Vec2;

    #[test]
    fn a_travelling_bot_uses_its_own_teams_endgame_watch() {
        use crate::ai::{StallState, RECYCLE_AFTER, RECYCLE_ROAM};

        let (mut sim, h) = l_sim();
        sim.battle.teams[1].reinforcements = 1.0;
        for (own_tickets, travelled, stalled) in [(12.0, true, 0), (1.0, true, 1), (12.0, false, 1)]
        {
            sim.battle.teams[0].reinforcements = own_tickets;
            let mut state = AiState {
                stall: Some(StallState {
                    at: if travelled {
                        Vec3::NEG_X * RECYCLE_ROAM
                    } else {
                        Vec3::ZERO
                    },
                    lo: Vec3::splat(-50.0),
                    hi: Vec3::splat(50.0),
                    ticks: RECYCLE_AFTER - 1,
                    ..StallState::default()
                }),
                ..AiState::default()
            };
            let u = sim.unit(h).unwrap();
            let tick = Tick {
                rules: crate::ai::UnitRules::for_unit(&sim, u),
                me: h,
                u,
                eye: u.ai_eye(),
                wc: &sim.weapon_classes[u.weapon_class],
                tier: sim.unit_tier(u),
                skill: sim.skill_of(u),
            };
            let move_request = MoveRequest {
                kind: WalkAsk::LEG,
                goal: Some(Vec3::NEG_Z * 40.0),
                arrive: None,
                post: None,
                defending: false,
                in_range: false,
                ret: None,
                hold: None,
                flee: false,
                boarding: false,
                serving: false,
                excused: false,
                acts: Acts::default(),
            };
            let mut pad = PadState::default();
            let mut nav = NavTick::default();
            run(&sim, &tick, &mut state, &move_request, &mut pad, &mut nav);
            assert_ne!((pad.lx, pad.ly), (0, 0), "the bot has a route to walk");
            assert_eq!(
                nav.stalled, stalled,
                "own tickets {own_tickets}, travelled {travelled}"
            );
        }
    }

    /// Card PARR's scenes: `l_sim`'s post (hub 2, a 6 m capture sphere) with
    /// a body on it and the bot 12 m short of it. `plinth`: BF1's post, a
    /// solid cylinder of radius 1.3 m on the centre. Else BF2 Kamino's cp0:
    /// the centre 3 m up on a 4 x 3 x 4 m block, the capture sphere (8 m)
    /// reaching the floor.
    fn post_scene(plinth: bool) -> (Sim, UnitHandle) {
        use crate::collision::{Body, Shape};
        let (mut sim, h) = l_sim();
        let c = Vec3::new(40.0, 0.0, -40.0);
        let shape = if plinth {
            Shape::Cylinder {
                radius: 1.3,
                half_height: 1.5,
            }
        } else {
            sim.cps[0].def.pos = c + Vec3::Y * 3.0;
            sim.cps[0].def.capture = crate::region::Region::sphere(c + Vec3::Y * 3.0, 8.0);
            Shape::Box {
                half: Vec3::new(2.0, 1.5, 2.0),
            }
        };
        sim.collision.push_body(Body::new(
            glam::Mat4::from_translation(c + Vec3::Y * 1.5),
            shape,
        ));
        sim.unit_mut(h).expect("bot").soldier.pos = Vec3::new(40.0, 0.0, -28.0);
        (sim, h)
    }

    /// Steps `post_scene` for 10 s and answers the bot's position when it
    /// has arrived and stands: the stick released, inside the capture region.
    fn stands_at_post(sim: &mut Sim, h: UnitHandle) -> Option<Vec3> {
        use crate::sim::PadState;
        for _ in 0..300 {
            sim.step(&[PadState::default()]);
        }
        let u = sim.unit(h).expect("alive");
        let (pad, _) = think(sim, h, &mut u.ai.clone());
        let pos = u.soldier.pos;
        (pad.lx == 0 && pad.ly == 0 && sim.cps[0].def.capture.contains(pos)).then_some(pos)
    }

    /// Card PARR: a bot sent to capture a post with a solid plinth on its
    /// centre arrives and stands within 10 s, beside the plinth.
    #[test]
    fn a_bot_arrives_at_a_post_with_a_solid_plinth() {
        let (mut sim, h) = post_scene(true);
        let pos = stands_at_post(&mut sim, h).expect("arrived and stands");
        assert!(
            nav::xz_dist(pos, sim.cps[0].def.pos) >= 1.3 + crate::unit::SOLDIER_RADIUS - 0.05,
            "beside the plinth: {pos}"
        );
    }

    /// Card PARR: a post whose centre is 3 m up on a block, its capture
    /// region reaching the floor: the bot arrives on the floor.
    #[test]
    fn a_bot_arrives_on_the_floor_under_a_raised_post() {
        let (mut sim, h) = post_scene(false);
        let pos = stands_at_post(&mut sim, h).expect("arrived and stands");
        assert!(pos.y.abs() < 0.05, "on the floor: {pos}");
    }

    /// Card PARR: the drawn point is in the capture region, on the floor
    /// and never inside a body, whatever the tick it is drawn at.
    #[test]
    fn a_post_point_is_never_inside_a_body() {
        for plinth in [true, false] {
            let (mut sim, h) = post_scene(plinth);
            let c = Vec3::new(40.0, 0.0, -40.0);
            let mut seen = Vec::new();
            for t in 0..200 {
                sim.tick = t;
                let u = sim.unit(h).expect("bot");
                let p = super::super::route::post_point(&sim, h, u, &sim.cps[0]);
                assert!(sim.cps[0].def.capture.contains(p), "{plinth} {t}: {p}");
                assert!(p.y.abs() < 1e-3, "{plinth} {t}: on the floor {p}");
                let d = (p - c).abs();
                let clear = if plinth {
                    nav::xz_dist(p, c) >= 1.3 + crate::unit::SOLDIER_RADIUS
                } else {
                    d.x.max(d.z) >= 2.0 + crate::unit::SOLDIER_RADIUS
                };
                assert!(clear, "{plinth} {t}: in the body {p}");
                seen.push(p);
            }
            // A draw per arrival: not one point for everybody.
            seen.dedup();
            assert!(seen.len() > 100, "{plinth}: {} points", seen.len());
        }
    }

    /// Card TEAM: two bots of one team set out in file 0.8 m apart on the
    /// L graph's two 40 m legs to the post: the one behind has fallen back
    /// to a stride (`team::FOLLOW_GAP`) after 1.5 s and keeps it while both
    /// walk, and both get there.
    #[test]
    fn a_file_of_two_walks_a_stride_apart() {
        use crate::sim::PadState;
        use crate::unit::{Controller, UnitSpec};
        let (mut sim, a) = l_sim();
        let b = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 0.8))
            })
            .expect("spawned");
        sim.unit_mut(b).expect("b").soldier.pos = Vec3::new(0.0, 0.0, 0.8);
        let (mut walked, mut close) = (0, 0);
        for t in 0..900 {
            sim.step(&[PadState::default()]);
            let (ua, ub) = (sim.unit(a).expect("a"), sim.unit(b).expect("b"));
            let both = ua.soldier.vel.length() > 1.0 && ub.soldier.vel.length() > 1.0;
            if t >= 45 && both {
                walked += 1;
                close += usize::from(nav::xz_dist(ua.soldier.pos, ub.soldier.pos) < 1.5);
            }
        }
        assert!(walked > 150, "{walked} ticks of both walking");
        assert_eq!(close, 0, "{close} of {walked} ticks within 1.5 m");
        for h in [a, b] {
            let p = sim.unit(h).expect("alive").soldier.pos;
            assert!(
                nav::xz_dist(p, sim.cps[0].def.pos) < 6.0,
                "at the post: {p}"
            );
        }
    }

    /// A position `d` from where the bot stands, held for good: the mover's
    /// request is to walk there and stand (kind HOLD).
    fn hold_at(sim: &mut Sim, h: UnitHandle, d: Vec3) -> Vec3 {
        let u = sim.unit_mut(h).expect("alive");
        let goal = u.soldier.pos + d;
        u.ai.hold = Some(HoldState {
            spot: goal,
            goal,
            until: u64::MAX,
            shuffle_at: u64::MAX,
            commit_until: u64::MAX,
            last_contact: u64::MAX,
            ..HoldState::default()
        });
        goal
    }

    /// The stick `think` would give the bot on this tick.
    fn stick(sim: &Sim, h: UnitHandle) -> (i16, i16) {
        let (pad, _) = think(sim, h, &mut sim.unit(h).expect("alive").ai.clone());
        (pad.lx, pad.ly)
    }

    /// Card MOVR (a): a bot sent to a point 6 m away arrives, and its move
    /// stick reads zero on each of the next 300 ticks — also after a shove
    /// that leaves it 0.9 m from the point, outside the arrive radius.
    #[test]
    fn a_bot_that_arrives_stands() {
        use crate::ai::{ARRIVED_SLACK, HOLD_ARRIVE};
        let (mut sim, h) = l_sim();
        let goal = hold_at(&mut sim, h, Vec3::new(6.0, 0.0, 0.0));
        let mut arrived = None;
        for t in 0..150 {
            sim.step(&[PadState::default()]);
            if sim.unit(h).expect("alive").ai.mv.arrived {
                arrived = Some(t);
                break;
            }
        }
        assert!(arrived.is_some(), "it arrives");
        let at = sim.unit(h).expect("alive").soldier.pos;
        assert!(nav::xz_dist(at, goal) <= HOLD_ARRIVE, "on the point: {at}");
        for t in 0..300 {
            if t == 100 {
                // A friend's shove: 0.9 m off the point.
                let shove = Vec3::new(0.0, 0.0, 0.9);
                assert!(shove.z > HOLD_ARRIVE && shove.z < HOLD_ARRIVE + ARRIVED_SLACK);
                sim.unit_mut(h).expect("alive").soldier.pos = goal + shove;
            }
            assert_eq!(stick(&sim, h), (0, 0), "tick {t} after the arrival");
            sim.step(&[PadState::default()]);
        }
        let u = sim.unit(h).expect("alive");
        assert!(u.ai.mv.arrived);
        assert_eq!(u.ai.behaviour, crate::ai::Behaviour::Fight);
        // Shoved right off it, it walks back.
        sim.unit_mut(h).expect("alive").soldier.pos = goal + Vec3::new(0.0, 0.0, 4.0);
        assert_ne!(stick(&sim, h), (0, 0), "4 m off its point it walks");
    }

    /// Card OBSF: a hold point the bot cannot walk straight at — here inside
    /// a crate, so the way to it is the early turn ROUND the crate for ever —
    /// is given up for where the bot stands within 2 s (`HOLD_BLOCKED` of
    /// picks that are not straight), and the bot stands there. Before, the
    /// early turn was no blocked pick: the maintainer's Kamino scene had a holder
    /// circle a crate's rim, 17 reversals in 10 s.
    #[test]
    fn a_hold_point_behind_a_crate_is_given_up_not_circled() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let (mut sim, h) = l_sim();
        sim.collision = CollisionWorld::new(
            vec![Body::new(
                glam::Mat4::from_translation(Vec3::new(2.2, 1.0, 0.1)),
                Shape::Box {
                    half: Vec3::new(0.6, 1.0, 0.6),
                },
            )],
            Vec::new(),
        );
        let goal = hold_at(&mut sim, h, Vec3::new(2.2, 0.0, 0.1));
        let mut round = 0;
        for _ in 0..60 {
            sim.step(&[PadState::default()]);
            round += u32::from(sim.unit(h).expect("alive").ai.steer == SteerPick::Round);
        }
        assert!(round > 0, "the crate was seen and walked round");
        let at = sim.unit(h).expect("alive").soldier.pos;
        for t in 0..90 {
            assert_eq!(stick(&sim, h), (0, 0), "tick {t}: it stands");
            sim.step(&[PadState::default()]);
        }
        let u = sim.unit(h).expect("alive");
        assert!(nav::xz_dist(u.soldier.pos, at) < 0.1, "and stays");
        let hold = u.ai.hold.expect("holding");
        assert!(
            nav::xz_dist(hold.goal, goal) > 0.5,
            "the point was given up"
        );
    }

    #[test]
    fn a_hold_point_below_a_shelf_does_not_send_the_bot_over_the_edge() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let (mut sim, h) = l_sim();
        sim.collision = CollisionWorld::new(
            vec![Body::new(
                glam::Mat4::from_translation(Vec3::new(-5.0, 4.0, 0.0)),
                Shape::Box {
                    half: Vec3::new(7.0, 0.5, 8.0),
                },
            )],
            Vec::new(),
        );
        sim.unit_mut(h).unwrap().soldier.pos = Vec3::Y * 4.5;
        hold_at(&mut sim, h, Vec3::new(28.0, -4.5, 0.0));

        for _ in 0..150 {
            sim.step(&[PadState::default()]);
            let feet = sim.unit(h).unwrap().soldier.pos;
            assert!(feet.y >= 4.4, "fell from the shelf at {feet}");
        }
    }

    /// Card MOVR (b): a request that alternates between two points 0.4 m
    /// apart moves nothing — the bot stands on the first, 0.5 m short of it,
    /// and the second is 0.9 m away, outside the arrive radius.
    #[test]
    fn a_request_that_barely_moves_is_ignored() {
        use crate::ai::HOLD_ARRIVE;
        let (mut sim, h) = l_sim();
        let a = hold_at(&mut sim, h, Vec3::new(0.5, 0.0, 0.0));
        let b = a + Vec3::new(0.4, 0.0, 0.0);
        let from = sim.unit(h).expect("alive").soldier.pos;
        assert!(nav::xz_dist(from, a) <= HOLD_ARRIVE && nav::xz_dist(from, b) > HOLD_ARRIVE);
        sim.step(&[PadState::default()]);
        for t in 0..120 {
            let u = sim.unit_mut(h).expect("alive");
            if let Some(hs) = u.ai.hold.as_mut() {
                hs.goal = if t % 2 == 0 { b } else { a };
            }
            assert_eq!(stick(&sim, h), (0, 0), "tick {t}");
            sim.step(&[PadState::default()]);
        }
        let at = sim.unit(h).expect("alive").soldier.pos;
        assert!(nav::xz_dist(at, from) < 0.01, "it has not moved: {at}");
        // A request that really moves (2 m) is followed.
        let u = sim.unit_mut(h).expect("alive");
        if let Some(hs) = u.ai.hold.as_mut() {
            hs.goal = a + Vec3::new(2.0, 0.0, 0.0);
        }
        assert_ne!(stick(&sim, h), (0, 0), "a new destination is walked to");
    }

    /// A pen round the origin: four walls a metre high, 2 m from the centre.
    fn pen(sim: &mut Sim) {
        use crate::collision::{Body, CollisionWorld, Shape};
        let wall = |x: f32, z: f32, hx: f32, hz: f32| {
            Body::new(
                glam::Mat4::from_translation(Vec3::new(x, 0.5, z)),
                Shape::Box {
                    half: Vec3::new(hx, 0.5, hz),
                },
            )
        };
        sim.collision = CollisionWorld::new(
            vec![
                wall(2.5, 0.0, 0.5, 3.0),
                wall(-2.5, 0.0, 0.5, 3.0),
                wall(0.0, 2.5, 3.0, 0.5),
                wall(0.0, -2.5, 3.0, 0.5),
            ],
            Vec::new(),
        );
    }

    /// Card MOVR (c): a penned bot's walk FAILS at its first progress window
    /// with no progress — card OBST: once the allowance of a bot going round
    /// is spent, `avoid::ROUND_MAX` after it met the pen's wall (it goes
    /// round the pen meanwhile) — and it is in Recover; at the
    /// [`GIVE_UP_AFTER`]-th failure it gives its post up, and the
    /// commander's next plan draws it another.
    #[test]
    fn a_blocked_walk_fails_recovers_and_gives_its_post_up() {
        use crate::ai::{Behaviour, GIVE_UP_AFTER, THINK_PERIOD};
        use crate::commander::{Objective, PLAN_PERIOD};
        use crate::cp::CpDef;
        use crate::region::Region;
        let (mut sim, h) = l_sim();
        // A second enemy post, at hub 1.
        let other = Vec3::new(0.0, 0.0, -40.0);
        sim.add_cp(
            CpDef {
                name: "cp2".into(),
                pos: other,
                capture: Region::sphere(other, 6.0),
                control: None,
                value_bleed: 100,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: vec![],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(1),
        );
        pen(&mut sim);
        let window = u64::from(STUCK_WINDOW);
        // The first window still has the run up to the pen's wall in it.
        let round = u64::from(avoid::ROUND_MAX);
        // (Going round its pen the bot gets nearer its waypoint in some
        // windows: they are not failures, and with the allowance spent they
        // do not end the recovery either.)
        let limit =
            round + 2 * (u64::from(GIVE_UP_AFTER) + 2) * window + PLAN_PERIOD + 2 * THINK_PERIOD;
        let (mut first, mut failed_at, mut changed_at) = (None, None, None);
        for t in 0..limit + 30 {
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).expect("alive: no recycle this soon");
            assert!(nav::xz_dist(u.soldier.pos, Vec3::ZERO) < 2.5, "penned");
            let now = sim.commander[0].of(h.index).and_then(Objective::cp);
            first = first.or(now);
            if failed_at.is_none() && u.ai.mv.recover > 0 {
                assert_eq!(u.ai.behaviour, Behaviour::Recover);
                failed_at = Some(t);
            }
            if changed_at.is_none() && first.is_some() && now != first {
                changed_at = Some(t);
            }
        }
        let failed_at = failed_at.expect("the walk failed");
        assert!(
            failed_at <= round + 3 * window + THINK_PERIOD,
            "at its first window with no progress: tick {failed_at}"
        );
        let changed_at = changed_at.expect("a new objective");
        assert!(
            changed_at > failed_at && changed_at <= limit,
            "after {GIVE_UP_AFTER} failures and a plan: tick {changed_at} (limit {limit})"
        );
    }

    /// Card OBSR: the progress window's allowance for a bot going round
    /// ([`avoid::ROUND_MAX`]) is for a BODY. A commitment whose last contact
    /// was a rim barrier — an edge of the level, which is not gone round —
    /// fails its window like any walk that gets no nearer, however far the
    /// bot moved along the rim meanwhile.
    #[test]
    fn a_bot_going_along_a_rim_barrier_is_not_excused_by_moving_on() {
        let window_fails = |rim: bool| {
            let (mut sim, h) = l_sim();
            sim.step(&[PadState::default()]);
            let u = sim.unit(h).expect("alive");
            let pos = u.soldier.pos;
            let mut st = u.ai;
            let mut r = st.route().expect("a route");
            // No nearer its waypoint than at the last window...
            r.ref_dist = nav::xz_dist(pos, sub_point(&r, pos));
            r.stuck_ticks = STUCK_WINDOW - 1;
            r.stuck_run = 0;
            st.install_route_for_fixture(Some(r));
            // ...3 m along what it is going round.
            st.slide = 1;
            st.mv.round = crate::ai::Round {
                meet: 10.0,
                anchor: pos + Vec3::X * 3.0,
                rim,
                ..crate::ai::Round::default()
            };
            let mut nt = NavTick::default();
            progress(&sim, h, &mut st, pos, true, false, &mut nt);
            (
                st.route().expect("a route").stuck_run > 0,
                st.mv.round.live(),
            )
        };
        assert_eq!(window_fails(false), (false, true), "round a body");
        assert_eq!(window_fails(true), (true, false), "along a rim");
    }

    /// Card MOVR (d): a post no route can be planned to (no arc for a
    /// soldier anywhere) leaves no bot without a behaviour: after two thinks
    /// it is travelling — straight at its post — and its stick is down.
    #[test]
    fn a_post_with_no_route_leaves_no_bot_without_a_behaviour() {
        use crate::ai::{Behaviour, THINK_PERIOD};
        let (mut sim, h) = l_sim();
        if let Some(g) = sim.nav.as_mut() {
            for a in g.arcs.iter_mut() {
                a.type_mask = 0;
            }
        }
        for _ in 0..=2 * THINK_PERIOD {
            sim.step(&[PadState::default()]);
        }
        let u = sim.unit(h).expect("alive");
        assert!(
            u.ai.route()
                .is_none_or(|r| r.source == crate::ai::RouteSource::Straight),
            "no route on the graph: {:?}",
            u.ai.route()
        );
        assert_eq!(u.ai.behaviour, Behaviour::Travel);
        assert_ne!(stick(&sim, h), (0, 0), "it walks at its post");
    }

    /// Card MOVR: the think's phase is the unit's index — two bots spawned
    /// on one tick do not think on the same tick after their first.
    #[test]
    fn two_bots_spawned_together_think_on_different_ticks() {
        use crate::ai::THINK_PERIOD;
        use crate::unit::{Controller, UnitSpec};
        let (mut sim, a) = l_sim();
        let b = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(3.0, 0.0, 0.0))
            })
            .expect("spawned");
        for _ in 0..3 {
            sim.step(&[PadState::default()]);
        }
        let next = |h: UnitHandle| sim.unit(h).expect("alive").ai.next_think;
        assert_ne!(next(a), next(b));
        for h in [a, b] {
            assert_eq!((next(h) + u64::from(h.index)) % THINK_PERIOD, 0);
            assert!(next(h) > sim.tick && next(h) <= sim.tick + THINK_PERIOD);
        }
    }

    /// Card MOVR: the steering PICKS its heading on every third tick and the
    /// mover holds it between — a bot sliding along a slab across its way
    /// is blocked on every tick and probes on a third of them.
    #[test]
    fn the_heading_is_picked_every_third_tick_and_held() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let (mut sim, _) = l_sim();
        sim.collision = CollisionWorld::new(
            vec![Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)),
                Shape::Box {
                    half: Vec3::new(30.0, 6.0, 0.5),
                },
            )],
            Vec::new(),
        );
        for _ in 0..240 {
            sim.step(&[PadState::default()]);
        }
        let (blocked, probed) = (sim.nav_stats.blocked_ticks, sim.nav_stats.sweep_hits);
        assert!(blocked > 90, "it slides along the slab: {blocked} ticks");
        assert!(probed > 0, "it probes");
        assert!(
            probed * 5 <= blocked * 2,
            "a probe on a third of the blocked ticks: {probed} of {blocked}"
        );
    }

    /// Card JITR: a step of under [`HOLD_WALK_DIST`] is walked, at
    /// [`HOLD_WALK_STICK`] of the stick, and it still gets there; a longer
    /// walk to the position runs until its last metres.
    #[test]
    fn a_short_step_on_a_position_is_walked() {
        // Card FGHT: a shooter that keeps its place.
        let _style = crate::ai::tests::all_stand();
        use crate::ai::{HOLD_ARRIVE, HOLD_WALK_DIST, HOLD_WALK_STICK};
        use crate::sim::PadState;
        use crate::unit::UnitSpec;
        let top = |d: f32| {
            let (mut sim, h) = l_sim();
            let e = sim
                .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -32.0)))
                .expect("enemy");
            let u = sim.unit_mut(h).expect("alive");
            let goal = u.soldier.pos + Vec3::new(d, 0.0, 0.0);
            u.ai.hold = Some(HoldState {
                spot: goal,
                goal,
                until: u64::MAX,
                shuffle_at: u64::MAX,
                commit_until: u64::MAX,
                ..HoldState::default()
            });
            let (mut top, mut arrived) = (0.0f32, None);
            for _ in 0..150 {
                sim.step(&[PadState::default()]);
                sim.unit_mut(e).expect("enemy").health.cur = 300.0;
                let u = sim.unit(h).expect("alive");
                top = top.max(Vec2::new(u.soldier.vel.x, u.soldier.vel.z).length());
                if arrived.is_none() && nav::xz_dist(u.soldier.pos, goal) <= HOLD_ARRIVE {
                    arrived = Some(sim.tick);
                }
            }
            (top, arrived, sim.unit(h).expect("alive").consts.max_speed)
        };
        // The step is sideways to the enemy it faces: the strafe cap, not
        // MaxSpeed, is the full speed here — the long walk measures it.
        let (long, _, max) = top(4.0 * HOLD_WALK_DIST);
        assert!(
            long > 0.7 * max,
            "a long one runs: {long:.2} m/s of {max:.2}"
        );
        let (short, got_there, _) = top(HOLD_WALK_DIST - 0.5);
        assert!(got_there.is_some(), "the short step arrives");
        assert!(
            short <= HOLD_WALK_STICK * long + 0.05,
            "a short step is walked: {short:.2} m/s, the run {long:.2}"
        );
    }

    /// Card FGHT (it was "holds only inside HOLD_RANGE"): an engaged unit
    /// KEEPS WALKING its route while it aims and fires at a target beyond its
    /// weapon's reach (`MaxRange` + 10 m), and fights from a position of its
    /// own inside it — here a stander's, where it is.
    #[test]
    fn an_engaged_unit_walks_on_until_its_weapons_reach() {
        use crate::ai::think;
        use crate::unit::UnitSpec;
        let _style = crate::ai::tests::all_stand();
        // An enemy 40 m away, inside the view distance (49 m at Easy and Normal).
        let scene_of = |max_range: f32, rider: bool| {
            let (mut sim, h) = l_sim();
            let e = sim
                .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -40.0)))
                .unwrap();
            if rider {
                sim.unit_mut(e).expect("enemy").mount = Some(crate::vehicle::Mount {
                    vehicle: crate::vehicle::VehicleHandle { index: 0, gen: 0 },
                    seat: 1,
                });
            }
            let wc = sim.unit(h).expect("alive").weapon_class;
            sim.weapon_classes[wc].ai_range = [0.0, max_range * 0.5, max_range];
            let mut st = AiState::default();
            let (pad, _) = think(&sim, h, &mut st);
            assert!(st.target.is_some(), "engaged");
            (pad, st)
        };
        let scene = |max_range: f32| scene_of(max_range, false);
        // A 16 m weapon: 40 m is beyond 16 + 10.
        let (pad, st) = scene(16.0);
        assert_ne!(pad.ly, 0, "still walking the route: {pad:?}");
        assert_eq!(st.steer, SteerPick::Fwd);
        assert!(st.hold.is_none(), "no position beyond the reach");
        // A 60 m weapon: in reach — it stands on a position where it is.
        let (pad, st) = scene(60.0);
        assert_eq!((pad.lx, pad.ly), (0, 0), "fights from its place: {pad:?}");
        assert_eq!(st.steer, SteerPick::Idle);
        assert_eq!(st.behaviour, crate::ai::Behaviour::Fight);
        assert_eq!(st.fight.mode, crate::ai::FightState::STAND);
        assert!(st.hold.is_some(), "on a position");
        // ...but not against a RIDER (a gunner in his seat, whom a rifle
        // cannot hurt): it fires and walks on.
        let (pad, st) = scene_of(60.0, true);
        assert_ne!(pad.ly, 0, "walks on past a rider: {pad:?}");
        assert!(st.hold.is_none(), "no position against a rider");
    }

    /// Card UTALK, the trooper's own talk (re-squad-orders.md 5): a BF1 bot
    /// says "follow me" (chance 0.3) only on a tick it set out for its post
    /// and "move out" on the tick its walk reached it; over 60 bots setting
    /// out together about 0.3 of them say it. A BF2-rule sim says neither.
    #[test]
    fn a_trooper_says_follow_as_it_sets_out_and_move_out_as_it_arrives() {
        use crate::ai::Behaviour;
        use crate::sim::{Call, Event};
        use crate::soldier::GroundRules;
        use crate::unit::{Controller, UnitSpec};
        let run = |rules: GroundRules| {
            let (mut sim, h) = post_scene(true);
            sim.ground_rules = rules;
            let mut rows = Vec::new();
            for _ in 0..300 {
                let was = sim.unit(h).expect("bot").ai.behaviour;
                sim.step(&[PadState::default()]);
                let now = sim.unit(h).expect("bot").ai.behaviour;
                for e in &sim.events {
                    if let Event::Call { unit, call } = *e {
                        if unit == h && matches!(call, Call::Follow | Call::MoveOut) {
                            rows.push((was, now, call));
                        }
                    }
                }
            }
            rows
        };
        let rows = run(GroundRules::Bf1);
        let moveouts: Vec<_> = rows.iter().filter(|r| r.2 == Call::MoveOut).collect();
        assert_eq!(moveouts.len(), 1, "one arrival: {rows:?}");
        assert_eq!(moveouts[0].0, Behaviour::Travel, "{rows:?}");
        assert!(
            matches!(moveouts[0].1, Behaviour::Capture | Behaviour::Guard),
            "{rows:?}"
        );
        assert!(
            rows.iter()
                .filter(|r| r.2 == Call::Follow)
                .all(|r| r.0 != Behaviour::Travel && r.1 == Behaviour::Travel),
            "follow only as it sets out: {rows:?}"
        );
        assert!(run(GroundRules::Bf2).is_empty(), "BF2");
        // The chance, over 60 bots setting out on their first think.
        let (mut sim, _) = l_sim();
        sim.ground_rules = GroundRules::Bf1;
        for k in 0..60u8 {
            let at = Vec3::new(f32::from(k % 10) * 1.5, 0.0, f32::from(k / 10) * 1.5);
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, at)
            })
            .expect("spawned");
        }
        let (mut set_out, mut follow) = (0, 0);
        for _ in 0..40 {
            let was: Vec<_> = sim.units.iter().map(|u| u.ai.behaviour).collect();
            sim.step(&[PadState::default()]);
            set_out += sim
                .units
                .iter()
                .zip(&was)
                .filter(|(u, w)| u.ai.behaviour == Behaviour::Travel && **w != Behaviour::Travel)
                .count();
            follow += sim
                .events
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        Event::Call {
                            call: Call::Follow,
                            ..
                        }
                    )
                })
                .count();
        }
        assert!(set_out >= 60, "every bot set out: {set_out}");
        let share = follow as f32 / set_out as f32;
        assert!((0.18..=0.42).contains(&share), "{follow} of {set_out}");
    }

    /// Card BSTK: a bot flying its remote droid stands still on purpose (the
    /// droid takes its pad). Its walk's progress window does not run, so the
    /// stand is no stuck run and nothing the sweep's `stuck_max` counts; the
    /// same bot on foot walks off.
    #[test]
    fn a_bot_flying_its_droid_is_not_stuck() {
        use crate::remote::tests::{Props, BF1_DROID};
        use crate::remote::{Droid, DroidClass, RemoteWeapon};
        use crate::weapon::WeaponClass;
        let scene = |fly: bool| {
            let (mut sim, h) = l_sim();
            if fly {
                let mut w = WeaponClass::bf1_rifle();
                w.remote = Some(Box::new(RemoteWeapon::new(
                    &Props(&[("0xc43b77dd", "1")]),
                    DroidClass::from_props(&Props(BF1_DROID), false),
                    WeaponClass::bf1_rifle(),
                )));
                let wc = sim.add_weapon_class(w);
                let r = sim.weapon_classes[wc].remote.as_deref().expect("remote");
                let d = Droid::new(
                    0,
                    h,
                    0,
                    wc,
                    r,
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::ZERO,
                    0.0,
                    false,
                );
                sim.droids.push(d);
            }
            // Under the bot's recall (10 s with no enemy near the droid).
            for _ in 0..240 {
                sim.step(&[PadState::default()]);
            }
            (sim, h)
        };
        let (sim, h) = scene(false);
        let walked = sim.unit(h).expect("alive").soldier.pos.length();
        assert!(walked > 5.0, "on foot it walks: {walked} m");
        let (sim, h) = scene(true);
        assert!(sim.controlled_droid(h).is_some(), "still flying");
        let u = sim.unit(h).expect("alive");
        assert!(u.soldier.pos.length() < 0.5, "the body stood");
        assert_eq!(u.ai.route().map_or(0, |r| r.stuck_run), 0);
        assert_eq!(sim.nav_stats.stuck_max, 0);
        assert_eq!(u.ai.stall.map_or((0, 0), |s| (s.ticks, s.fails)), (0, 0));
    }
}
