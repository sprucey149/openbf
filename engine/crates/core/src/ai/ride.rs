//! A bot and a vehicle (card VAI): the boarding claim a bot on foot makes
//! and walks to ([`claim`], [`board_goal`]), the pad a seated driver makes
//! ([`drive`]) and when it gets out. `think` hands a seated bot here; step 1c
//! (`Sim::ai_use_pass`) carries out the enter / exit it asks for.
//! GBOARD/WGORD deliver BF1 call targets at the next scheduled think,
//! retaining Hold/Follow behind explicit boarding and ordinary seat arbitration.
//! WGMUL bounds concurrent explicit BF1 claims by seats minus player-held seats,
//! counting fresh same-team claims in since/index order after think write-backs.
//! WGPAS admits unarmed secondary seats only for those explicit BF1 recipients;
//! claim upkeep, maintained pickup and physical entry share that provenance.
//! PILEIN shares that machinery for human ground-pilot orders in both rulesets,
//! with separate issuer provenance and no autonomous route or AI-call chance gate.
//! WGCAL publishes the initial BF1 ground call at PickupWait.started, once,
//! after all think write-backs; wait clocks and driver precedence stay here.
//! WGOUT consumes one first routed departure attempt per pickup tenure,
//! independently of wait-start success, after all AI write-backs.
//! WGNXT independently consumes subsequent witnessed command-walker post departures.
//! STCHG preserves a seat gun's charge deadline through this dispatch.
//!
//! The PS2's side, as far as it is read (`analysis/re-vehicles.md` §5, RE_LOG
//! "Arrival and locate" / "The six type bits"):
//! - the AI enters through the soldier's own Enter slot, instantly, after a
//!   queued request (B1 FUN_00244848 → FUN_0023c418 → FUN_0023d418) — so a
//!   bot takes the seat CanEnter gives it (`Sim::enter_seat`), and the
//!   request is served in step 1c, after every think;
//! - a driver routes on the planning graph as its vehicle's `AISizeType`
//!   (`route::mover`, `SeatSetup::nav_type`) and arrives at a node within
//!   ForwardSpeed × 0.5 of it ([`HOVER_ARRIVE_FACTOR`]);
//! - a WALKER's driver is `walker_drive`'s (card WAI, re-walker-ai.md) and a
//!   HOVER's `hover_drive`'s (card VDROP, re-hover-ai.md): the sticks the
//!   PS2's AI writes, handed to the vehicle raw;
//! - an AI driver's turn rate is the class's TurnRate blend with no full-stick
//!   timer (re-hover §5.1 I7, `hover::yaw_target`), and it never boosts: the
//!   BF2 AI boost branch (I5: E+0x324 and a controller state 3) is undecoded.
//!
//! OURS, PROVISIONAL (PLAN §AI — boarding decisions are openbf's AI): which
//! vehicle a bot goes for and when ([`BOARD_SEEK`], [`BOARD_TRIP`],
//! [`BOARD_GIVEUP`], [`BOARD_BLOCK`]); how it gets there (card WAI2: over the
//! graph to the vehicle's hub, `route::retarget_board`, then straight) and
//! when a claim is lost to another bot ([`board_goal`]); the entry reach
//! (the player's, 5 m past the vehicle's radius, plus [`BOARD_SLACK`],
//! measured from the bot's feet in XZ; a building turret's is the PS2's
//! board agent, card TBRD, [`in_reach`]); the dismount at the post, with no
//! objective, when stuck (`DRIVE_STUCK_BAIL`) or routeless, or in a class
//! the port cannot drive; and (card WAI) where a walker goes when its
//! type's graph does not reach its post's hub — the hub it can reach
//! nearest the post, where its driver gets out and walks
//! (`route::post_hub`, [`RIDE_GAIN`]). A hover keeps "no route, no ride"
//! (`HOVER_NEAREST` is off: measured on card VDROP, the rule on gives 43 %
//! more rides, and on BF2 POL1 22 riders in 8 rounds get out over no floor).
//! Card WAI2, measured on the PS2 (re-walker-ai §13): a command walker (the
//! AT-AT, the AT-TE) has a bot at its pilot seat from the first seconds to
//! the end ([`command`]); a tauntaun is ridden on past the hub its graph
//! ends at (`walker_drive::drive`).
//! VTAI: bots may claim an armed gunner seat beside a teammate pilot;
//! CanEnter still selects the seat. Both occupants fire (`gunner`), and a
//! gunner leaves an unpiloted vehicle. WAIT4/HWAIT/CWAIT share the pickup
//! hold (`analysis/re-driver-wait.md` §§2,3,5,7): four seconds from seating
//! (BF1) or first think (BF2), extended by fresh same-team AI claims or BF2's
//! approaching human (15 seconds, before full-seat completion). Completion
//! lasts this pilot tenure; BF1 release orders shorten the initial window
//! by setting expiry to two seconds before the release (WPOKE), below claims.
//! Route/progress work runs only after the hold.
//! VBAIL Part 1 (re-walker-ai §8, re-hover-ai §2 H2): under 20% health,
//! a pilot whose seeded 0..=100 draw has its low two bits clear orders all
//! AI occupants out. The card's per-boarding draw lifetime is PROVISIONAL
//! against the PS2 controller-construction draw. The blended port uses
//! this BF2 rule for both source games; `VEHICLE_BAIL` controls it alone.
//! Card VMESH, ours: a vehicle its driver got out of with the stuck window
//! spent (`DRIVE_STUCK_BAIL`) is marked where it stood
//! (`Vehicle::stuck_at`, in step 1c) and no bot claims it until it has
//! moved [`STUCK_MOVED`]: a walker wedged on BF1 Endor's cp2 boulder was
//! boarded by pair after pair, each put out ~17 s later (main 662e152: 5
//! such loops in 288 rounds, 0 with the mark).

use super::gunner::Look;
use super::route::{hub_has_floor, post_hub};
use super::state::{AiState, NavTick, RideState, SteerPick};
use super::{
    BOARD_BLOCK, BOARD_FRESH, BOARD_GIVEUP, BOARD_PAD_BF1, BOARD_PAD_BF2, BOARD_SEEK, BOARD_SLACK,
    BOARD_TRIP, GUNNER_WAIT, OTHER_FLOOR, RIDE_GAIN,
};
use crate::nav;
use crate::sim::{PadState, Sim, ENTER_REACH};
use crate::soldier::GroundRules;
use crate::unit::{Unit, UnitHandle};
use crate::vehicle::{Mount, SeatKind, VehicleHandle, VehicleKind, STUCK_MOVED};
use glam::Vec3;

/// The seat `me` can still take in `vh`, or `None`: the vehicle is alive, the
/// port can drive its class (a hover, or since card WAI a walker), and CanEnter gives this bot either the
/// pilot seat or — its pilot seat taken, by a teammate since CanEnter checks
/// the team — a gunner seat with a gun (card VTAI). Card TURR: a building
/// turret (no pilot seat) when CanEnter gives a seat with a gun.
pub(crate) fn claim_seat(sim: &Sim, me: UnitHandle, vh: VehicleHandle) -> Option<u8> {
    claim_seat_policy(sim, me, vh, SeatAdmission::Autonomous)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SeatAdmission {
    Autonomous,
    AiCall,
    PlayerOrder,
}

/// Admission for an existing claim; autonomous search always uses `claim_seat`.
pub(crate) fn claimed_seat(
    sim: &Sim,
    me: UnitHandle,
    vh: VehicleHandle,
    r: RideState,
) -> Option<u8> {
    let source = if r.board != Some(vh) || r.called_board != Some(me) {
        SeatAdmission::Autonomous
    } else if r.player_board.is_some() {
        SeatAdmission::PlayerOrder
    } else {
        SeatAdmission::AiCall
    };
    claim_seat_policy(sim, me, vh, source)
}

fn claim_seat_policy(
    sim: &Sim,
    me: UnitHandle,
    vh: VehicleHandle,
    source: SeatAdmission,
) -> Option<u8> {
    let v = sim.vehicle(vh)?;
    let class = v.class as usize;
    let armed = |k: u8| {
        sim.seat_setup
            .get(class)
            .and_then(|s| s.as_ref())
            .and_then(|s| s.guns.get(usize::from(k)))
            .is_some_and(Option::is_some)
    };
    if v.kind == VehicleKind::Turret {
        return sim.enter_seat(me, vh).filter(|&k| armed(k));
    }
    let hover = sim.hover_params.get(class).is_some_and(Option::is_some);
    let walker = sim.walker_params.get(class).is_some_and(Option::is_some);
    if !hover && !walker && !super::flyer_drive::can_fly(sim, v) {
        return None;
    }
    // A flyer is boarded on the ground. The reach is in XZ, so without this a
    // bot under a passing fighter would climb into it. (OURS for BF1; BF2's
    // CanEnter refuses state 2, `Sim::enter_seat`.)
    if v.kind == VehicleKind::Flyer && v.flyer.phase != crate::vehicle::flyer::FlightPhase::Landed {
        return None;
    }
    if v.seats.first().map(|s| s.kind) != Some(SeatKind::Pilot) {
        return None;
    }
    let k = sim.enter_seat(me, vh)?;
    let explicit = (source == SeatAdmission::PlayerOrder
        || (source == SeatAdmission::AiCall && sim.ground_rules == GroundRules::Bf1))
        && matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
        && sim
            .unit(me)
            .is_some_and(|u| u.controller == crate::unit::Controller::Ai && u.mount.is_none());
    (k == 0 || armed(k) || explicit).then_some(k)
}

/// Is `u` near enough to board `vh`? A vehicle: within the vehicle's radius
/// plus the player's 5 m (`ENTER_REACH`) and [`BOARD_SLACK`], from the bot's
/// feet in XZ. Card TBRD, a building turret: the PS2's board agent - the
/// 3-D distance between the two bodies' spheres under [`BOARD_PAD_BF2`]
/// (B2 `FUN_002c42d8`) / [`BOARD_PAD_BF1`] (B1 `FUN_00244848`), so a bot
/// walks up to the turret and a turret above or below it is not boarded.
pub(crate) fn in_reach(sim: &Sim, u: &Unit, vh: VehicleHandle) -> bool {
    reach_test(sim, u, vh).is_some_and(|(d2, reach)| d2 <= reach * reach)
}

/// Card TBRD: `u` stands inside a building turret's reach in XZ, out of it
/// in 3-D and over [`OTHER_FLOOR`] above or below it - on another floor.
/// OURS: its claim is given up at once (the PS2's board agent waits on, BF1
/// with no timer).
fn on_another_floor(sim: &Sim, u: &Unit, vh: VehicleHandle) -> bool {
    let Some(v) = sim.vehicle(vh).filter(|v| v.kind == VehicleKind::Turret) else {
        return false;
    };
    let c = u.capsule().center();
    reach_test(sim, u, vh).is_some_and(|(d2, reach)| {
        d2 > reach * reach && nav::xz_dist(c, v.pos) <= reach && (c.y - v.pos.y).abs() > OTHER_FLOOR
    })
}

/// [`in_reach`]'s squared distance and reach.
fn reach_test(sim: &Sim, u: &Unit, vh: VehicleHandle) -> Option<(f32, f32)> {
    let v = sim.vehicle(vh)?;
    let radius = sim
        .seat_setup
        .get(v.class as usize)
        .and_then(|s| s.as_ref())
        .map_or(0.0, |s| s.radius);
    if v.kind != VehicleKind::Turret {
        let d = nav::xz_dist(u.soldier.pos, v.pos);
        return Some((d * d, radius + ENTER_REACH + BOARD_SLACK));
    }
    let pad = if sim.ground_rules == GroundRules::Bf2 {
        BOARD_PAD_BF2
    } else {
        BOARD_PAD_BF1
    };
    let cap = u.capsule();
    let reach = radius + cap.radius.max(cap.height * 0.5) + pad;
    Some((cap.center().distance_squared(v.pos), reach))
}

/// On the think period, a bot on foot with no claim: the nearest vehicle it
/// could drive within [`BOARD_SEEK`], when its post is farther than
/// [`BOARD_TRIP`] (or the vehicle is a [`command`] walker) and no other bot
/// has claimed that vehicle.
pub(super) fn claim(sim: &Sim, me: UnitHandle, u: &Unit, st: &mut AiState) {
    let r = st.ride.unwrap_or_default();
    if r.board.is_some()
        || u.droideka.is_some()
        || sim.tick < r.block_until
        || sim.tick < r.own_search_until
        || sim.vehicles.is_empty()
    {
        return;
    }
    let Some(cp) = st.target_cp.and_then(|i| sim.cps.get(i as usize)) else {
        return;
    };
    let pos = u.soldier.pos;
    // (A command walker is boarded whatever the trip: see [`command`].)
    let near_post = nav::xz_dist(pos, cp.def.pos) <= BOARD_TRIP;
    let taken = |vh: VehicleHandle| {
        sim.units.iter().enumerate().any(|(i, o)| {
            i != me.index as usize && o.alive && o.ai.ride.is_some_and(|r| r.board == Some(vh))
        })
    };
    let mut best: Option<(f32, VehicleHandle)> = None;
    for (vh, v) in sim.alive_vehicles() {
        // Card TURR: a turret is offered with a purpose (`man::offer_pass`).
        if v.kind == VehicleKind::Turret {
            continue;
        }
        let d = nav::xz_dist(pos, v.pos);
        if d > BOARD_SEEK || best.is_some_and(|(b, _)| d >= b) || (near_post && !command(sim, vh)) {
            continue;
        }
        // Card VMESH: not one a driver gave up as stuck that has not moved since.
        if v.stuck_at
            .is_some_and(|p| nav::xz_dist(p, v.pos) < STUCK_MOVED)
        {
            continue;
        }
        if claim_seat(sim, me, vh).is_some() && !taken(vh) && routes_to(sim, vh, cp) {
            best = Some((d, vh));
        }
    }
    if let Some((_, vh)) = best {
        st.ride = Some(RideState {
            board: Some(vh),
            called_board: None,
            player_board: None,
            ordered_call: false,
            since: sim.tick,
            maintenance: None,
            ..r
        });
    }
}

/// Consume a call once, at the receiver's scheduled think. Only the exact
/// live receiver may take it. A newer player order or an existing claim wins;
/// repeated delivery cannot restart arbitration or the give-up clock.
/// Unlike autonomous search, this explicit target needs neither a long trip
/// nor an expired search cooldown (`re-driver-wait.md` §§3/4/7). WGORD keeps
/// earlier Hold/Follow installed: their explicit vehicle needs no autonomous post.
pub(super) fn deliver_call(sim: &Sim, me: UnitHandle, u: &Unit, st: &mut AiState) {
    let Some((owner, vh)) = st.pending_board.take() else {
        return;
    };
    if owner != me
        || !u.alive
        || u.controller != crate::unit::Controller::Ai
        || u.mount.is_some()
        || sim.ground_rules != GroundRules::Bf1
        || st
            .ride
            .is_some_and(|r| r.board.is_some() || (r.call_blocked && sim.tick < r.block_until))
        || u.droideka.is_some()
    {
        return;
    }
    let ordered = u.command.is_some();
    let Some(v) = sim.vehicle(vh) else { return };
    if !matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
        || v.stuck_at
            .is_some_and(|p| nav::xz_dist(p, v.pos) < STUCK_MOVED)
        || claim_seat_policy(sim, me, vh, SeatAdmission::AiCall).is_none()
        || (!ordered
            && !st
                .target_cp
                .and_then(|i| sim.cps.get(i as usize))
                .is_some_and(|cp| routes_to(sim, vh, cp)))
    {
        return;
    }
    st.ride = Some(RideState {
        board: Some(vh),
        since: sim.tick,
        ordered_call: ordered,
        called_board: Some(me),
        call_blocked: false,
        maintenance: None,
        ..st.ride.unwrap_or_default()
    });
}

/// Is `vh` a command walker (card CWSP: a walker whose class authors spawn
/// points — the AT-AT, the AT-TE)? Card WAI2: the PS2 has a bot at its pilot
/// seat from the first seconds to the end of the round, wherever it stands
/// (re-walker-ai §13: every AT-AT and AT-TE, both games, 240 s runs), so its
/// claim asks for no trip and no gain, and its pilot does not get out at the
/// end of its route (`walker_drive::drive`).
pub(super) fn command(sim: &Sim, vh: VehicleHandle) -> bool {
    sim.vehicle(vh)
        .and_then(|v| sim.walker_params.get(v.class as usize))
        .and_then(|p| p.as_ref())
        .is_some_and(|p| !p.spawn_points.is_empty())
}

/// Can `vh`, routed as its class's `AISizeType`, get from where it stands
/// to `cp`'s hub on the planning graph — or, where its type's graph does not
/// reach that hub, a walker's at least [`RIDE_GAIN`] nearer the post
/// (`route::post_hub`, card WAI)? Always, with no graph (the driver then drives straight at the
/// post).
fn routes_to(sim: &Sim, vh: VehicleHandle, cp: &crate::cp::CommandPost) -> bool {
    let Some(g) = sim.nav.as_ref() else {
        return true;
    };
    let Some(v) = sim.vehicle(vh) else {
        return false;
    };
    // FLYER has no planning-graph table: flyers do not route (nav.rs).
    if command(sim, vh) || v.kind == VehicleKind::Flyer {
        return true;
    }
    let ty = sim
        .seat_setup
        .get(v.class as usize)
        .and_then(|s| s.as_ref())
        .map_or(nav::SOLDIER, |s| s.nav_type());
    // The hub the driver's planner will start from (`route::start_hub`'s
    // same-level rule, asked at the vehicle).
    let from = g.unit_hub_on(v.pos, nav::Mover::new(ty, false), &|h| {
        hub_has_floor(sim, g, h, v.pos.y)
    });
    let nearest = v.kind == crate::vehicle::VehicleKind::Walker || super::HOVER_NEAREST;
    match from.and_then(|f| post_hub(g, ty, f, cp, nearest)) {
        Some(to) if cp.nav_hub == Some(to) => true,
        Some(to) => g.hub(to).is_some_and(|h| {
            nav::xz_dist(v.pos, cp.def.pos) - nav::xz_dist(h.pos, cp.def.pos) >= RIDE_GAIN
        }),
        None => false,
    }
}

/// Explicit ground goals share the source claim budget, independent of
/// empty physical seats (re-driver-wait §§3/4/7). Ordinary goals retain their
/// one-winner policy. Existing since/index priority also resolves simultaneous
/// scratch thinks; the post-write-back pass below sees all of them.
fn claim_preempted(sim: &Sim, me: UnitHandle, u: &Unit, r: RideState, vh: VehicleHandle) -> bool {
    let explicit = (r.player_board.is_some() || sim.ground_rules == GroundRules::Bf1)
        && r.called_board == Some(me)
        && sim
            .vehicle(vh)
            .is_some_and(|v| matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker));
    if !explicit {
        // A human's PILE IN outranks an ordinary claim on the same vehicle.
        return sim.units.iter().enumerate().any(|(i, o)| {
            o.alive
                && o.mount.is_none()
                && i != me.index as usize
                && o.ai.ride.is_some_and(|x| {
                    x.board == Some(vh)
                        && (x.player_board.is_some() || (x.since, i) < (r.since, me.index as usize))
                })
        });
    }
    let Some(v) = sim.vehicle(vh) else {
        return true;
    };
    let players = v
        .seats
        .iter()
        .filter(|s| {
            s.occupant
                .and_then(|h| sim.unit(h))
                .is_some_and(|o| matches!(o.controller, crate::unit::Controller::Player { .. }))
        })
        .count();
    let slots = v.seats.len().saturating_sub(players);
    let preceding = sim
        .alive_units()
        .filter(|(h, o)| {
            *h != me
                && o.team == u.team
                && o.controller == crate::unit::Controller::Ai
                && o.mount.is_none()
                && o.ai.ride.is_some_and(|x| {
                    // A human's PILE IN is preceded only by older PILE IN claims;
                    // it precedes every other explicit claim.
                    let older = (x.since, h.index) < (r.since, me.index);
                    x.board == Some(vh)
                        && if r.player_board.is_some() {
                            x.player_board.is_some() && older
                        } else {
                            x.player_board.is_some() || older
                        }
                        && x.called_board.is_none_or(|owner| owner == *h)
                        && sim.tick < x.since + BOARD_GIVEUP
                        && x.maintenance.map_or(sim.tick < x.since + BOARD_FRESH, |c| {
                            c.owner == *h && c.tick <= sim.tick && sim.tick < c.tick + BOARD_FRESH
                        })
                })
        })
        .count();
    preceding >= slots
}

/// Thinks read one snapshot. Referee explicit claims after every write-back,
/// before any physical entry, including same-tick contenders in either order.
/// No clocks are refreshed here; ordinary claims are never changed by this pass.
pub(crate) fn called_claim_pass(sim: &mut Sim) {
    for i in 0..sim.units.len() {
        let me = sim.handle(i);
        let u = &sim.units[i];
        let Some(r) = u.ai.ride.filter(|r| {
            r.called_board == Some(me)
                && (r.player_board.is_some() || sim.ground_rules == GroundRules::Bf1)
        }) else {
            continue;
        };
        let Some(vh) = r.board else { continue };
        if !u.alive
            || u.controller != crate::unit::Controller::Ai
            || u.mount.is_some()
            || !sim
                .vehicle(vh)
                .is_some_and(|v| matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker))
            || !claim_preempted(sim, me, u, r, vh)
        {
            continue;
        }
        let r = sim.units[i].ai.ride.as_mut().unwrap();
        r.board = None;
        r.called_board = None;
        r.player_board = None;
        r.ordered_call = false;
        r.maintenance = None;
        r.use_req = false;
        r.block_until = sim.tick;
    }
}

/// Every tick, a bot on foot: where its claim sends it, or `None`. A claim
/// that can no longer be boarded, no longer routes to the bot's post, or has
/// run [`BOARD_GIVEUP`], is dropped (and blocked for [`BOARD_BLOCK`]); one
/// within reach asks step 1c to enter. Card WAI2 — a claim is LOST, not
/// given up, when an older bot has it. CWAIT replaces the old eight-second
/// abandonment with a three-second maintenance lifetime for ground claims.
/// `since` remains the original arbitration and thirty-second give-up clock.
pub(super) fn board_goal(
    sim: &Sim,
    me: UnitHandle,
    u: &Unit,
    st: &mut AiState,
    nt: &mut NavTick,
) -> Option<Vec3> {
    let mut r = st.ride?;
    let vh = r.board?;
    let at = sim.vehicle(vh).map(|v| v.pos);
    let reach = in_reach(sim, u, vh);
    let older = claim_preempted(sim, me, u, r, vh);
    // The post can change under a claim (the commander re-plans), so the
    // vehicle must still route to the CURRENT one. Card TURR: a turret is
    // claimed for itself, not for a post - but not in the endgame.
    let turret = super::man::is_turret(sim, vh);
    // Preserve the turret's existing claim policy; CWAIT is ground pickup only.
    let left = turret
        && !reach
        && sim.tick >= r.since + GUNNER_WAIT
        && sim
            .vehicle(vh)
            .and_then(|v| v.seats.first())
            .is_some_and(|s| s.occupant.is_some());
    let routes = if let Some(issuer) = r.player_board {
        sim.players.get(usize::from(issuer.slot)).copied().flatten() == Some(issuer.leader)
            && sim.unit(issuer.leader).is_some_and(|l| l.team == u.team)
    } else if r.ordered_call {
        u.command.is_some()
    } else if turret {
        !super::man::endgame(sim, u.team)
    } else {
        st.target_cp
            .and_then(|i| sim.cps.get(i as usize))
            .is_some_and(|cp| routes_to(sim, vh, cp))
    };
    let late = sim.tick >= r.since + BOARD_GIVEUP || on_another_floor(sim, u, vh);
    let stale = !turret
        && (r
            .maintenance
            .is_some_and(|c| c.owner != me || c.tick > sim.tick)
            || sim.tick >= r.maintenance.map_or(r.since, |c| c.tick) + BOARD_FRESH);
    if older || left || stale || late || !routes || claimed_seat(sim, me, vh, r).is_none() {
        nt.claim = if older || left || stale {
            NavTick::CLAIM_LOST
        } else if late {
            NavTick::CLAIM_GIVEUP
        } else {
            0
        };
        r.call_blocked |= r.called_board.is_some() && !older;
        r.called_board = None;
        r.player_board = None;
        r.board = None;
        r.ordered_call = false;
        r.maintenance = None;
        r.use_req = false;
        r.block_until = if older {
            sim.tick
        } else {
            sim.tick + BOARD_BLOCK
        };
        st.ride = Some(r);
        if turret && late {
            super::man::unreached(sim, st, vh, nt);
        }
        return None;
    }
    let at = at?;
    r.use_req = reach;
    st.ride = Some(r);
    Some(at)
}

/// Card MOVR: the walk to the claimed vehicle has FAILED twice
/// (`behave::recover`): the claim is given up and the vehicle blocked for
/// [`BOARD_BLOCK`], as when its clock runs out.
pub(super) fn give_up(sim: &Sim, st: &mut AiState, nt: &mut NavTick) {
    if let Some(mut r) = st.ride {
        let vh = r.board.take();
        r.call_blocked |= r.called_board.is_some();
        r.called_board = None;
        r.player_board = None;
        r.ordered_call = false;
        r.maintenance = None;
        r.use_req = false;
        r.block_until = sim.tick + BOARD_BLOCK;
        st.ride = Some(r);
        nt.claim = NavTick::CLAIM_GIVEUP;
        // Card TURR: a turret the walk cannot reach is closed to the bot.
        if let Some(vh) = vh.filter(|&vh| super::man::is_turret(sim, vh)) {
            super::man::unreached(sim, st, vh, nt);
        }
    }
}

/// Where the vehicle this bot has claimed stands (`route::retarget_board`).
pub(super) fn board_point(sim: &Sim, st: &AiState) -> Option<Vec3> {
    Some(sim.vehicle(st.ride?.board?)?.pos)
}

/// A spent ride state goes back to `None`, so a bot with no vehicle business
/// hashes as it did before card VAI.
pub(super) fn settle(sim: &Sim, st: &mut AiState) {
    // A walker driver's sticks last one tick (`walker_drive::drive`).
    if let Some(r) = st.ride.as_mut() {
        r.sticks = None;
        r.hover = None;
        r.flyer = None;
    }
    // (Card TURR: a turret closed to it, or its seat's clocks, keep it.)
    if st.ride.is_some_and(|r| {
        r.board.is_none()
            && !r.use_req
            && r.pickup.is_none()
            && r.bail_draw.is_none()
            && sim.tick >= r.block_until
            && sim.tick >= r.own_search_until
            && sim.tick >= r.closed_until
            && r.man.is_none()
    }) {
        st.ride = None;
    }
}

pub(super) fn request_exit(st: &mut AiState) {
    let r = st.ride.unwrap_or_default();
    st.ride = Some(RideState {
        use_req: true,
        board: None,
        called_board: None,
        player_board: None,
        ordered_call: false,
        maintenance: None,
        ..r
    });
}

/// VBAIL, re-walker-ai §8 / re-hover-ai §2 H2: the PILOT's draw
/// controls every AI occupant's get-out order, with a strict <20% test.
/// The blended port uses the BF2 rule on both source games (BF1's hover
/// has no health test, re-hover-ai §9). Humans never issue this order.
pub(crate) fn vehicle_bail_allowed(sim: &Sim, vh: VehicleHandle) -> bool {
    if !super::VEHICLE_BAIL {
        return false;
    }
    let Some(v) = sim.vehicle(vh) else {
        return false;
    };
    if !matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
        || v.health.cur >= v.health.max * 0.2
    {
        return false;
    }
    v.seats
        .first()
        .filter(|s| s.kind == SeatKind::Pilot)
        .and_then(|s| s.occupant)
        .and_then(|h| sim.unit(h))
        .filter(|u| u.controller == crate::unit::Controller::Ai)
        .and_then(|u| u.ai.ride)
        .and_then(|r| r.bail_draw)
        .is_some_and(|draw| draw & 3 == 0)
}

/// Seated tick: pilots drive and fire; gunners aim and fire (VTAI).
/// Pilots leave at their post, with no goal, stuck, or in an unsupported
/// class; gunners leave an unpiloted vehicle. Turrets use `man::seated`.
pub(super) fn drive(
    sim: &Sim,
    me: UnitHandle,
    m: Mount,
    st: &mut AiState,
    mut nt: NavTick,
    thinking: bool,
) -> (PadState, NavTick) {
    let mut pad = PadState::default();
    st.steer = SteerPick::Idle;
    st.aim_dir = Vec3::ZERO;
    st.aim_vel = Vec3::ZERO;
    st.dispersion = None;
    let Some(v) = sim.vehicle(m.vehicle) else {
        return (pad, nt);
    };
    if v.kind == VehicleKind::Turret {
        super::man::seated(sim, me, m, st, &mut pad, &mut nt, thinking);
        return (pad, nt);
    }
    if v.seats.get(usize::from(m.seat)).map(|s| s.kind) != Some(SeatKind::Pilot) {
        let piloted = v
            .seats
            .first()
            .is_some_and(|s| s.kind == SeatKind::Pilot && s.occupant.is_some());
        if piloted {
            super::gunner::aim(sim, me, st, &mut pad, Look::Seat, None);
        } else {
            request_exit(st);
        }
        return (pad, nt);
    }
    if thinking && vehicle_bail_allowed(sim, m.vehicle) {
        request_exit(st);
        return (pad, nt);
    }
    if pickup_wait(sim, m, st, thinking) {
        // Return before either driver's route/follower progress clocks run.
        let r = st.ride.get_or_insert_default();
        if v.kind == VehicleKind::Walker {
            r.sticks = Some([0.0; 2]);
        } else {
            r.hover = Some(super::state::HoverOut {
                sticks: [0.0; 3],
                ai: v.hover.ai,
            });
        }
        super::gunner::aim(sim, me, st, &mut pad, Look::Steering, None);
        return (pad, nt);
    }
    // Card WAI: a walker's pilot steers by the PS2's own stick law.
    if let Some(wp) = sim
        .walker_params
        .get(v.class as usize)
        .and_then(|p| p.as_ref())
    {
        return super::walker_drive::drive(sim, me, m, wp, st, nt);
    }
    if super::flyer_drive::can_fly(sim, v) {
        return super::flyer_drive::drive(sim, me, m, st, nt);
    }
    let Some(hp) = sim
        .hover_params
        .get(v.class as usize)
        .and_then(|p| p.as_ref())
    else {
        request_exit(st);
        return (pad, nt);
    };
    super::hover_drive::drive(sim, me, m, hp, st, nt)
}

/// A ground driver's route branch has accepted a driving goal after pickup.
/// Turning toward that goal is departure too; speed and wait expiry alone
/// cannot identify it. Failed rolls consume this tenure's attempt as well.
pub(super) fn set_out(sim: &Sim, m: Mount, st: &mut AiState) {
    if sim.ground_rules == GroundRules::Bf1
        && sim
            .vehicle(m.vehicle)
            .is_some_and(|v| matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker))
    {
        // WGSEAT: the source is independent of vehicle armament/capacity
        // (re-driver-wait.md §§4/7). A failed publication roll is consumed too.
        if let Some(w) = st.ride.as_mut().and_then(|r| r.pickup.as_mut()) {
            if w.vehicle == m.vehicle && w.complete {
                w.departed = true;
            }
        }
    }
}

/// Initial window, BF2 human precedence, then maintained AI claims (CWAIT).
fn pickup_wait(sim: &Sim, m: Mount, st: &mut AiState, thinking: bool) -> bool {
    let Some(w) = st.ride.as_mut().and_then(|r| r.pickup.as_mut()) else {
        return false;
    };
    if w.vehicle != m.vehicle {
        // A stale handle cannot lend a different vehicle its boarding clock.
        st.ride.as_mut().unwrap().pickup = None;
        return false;
    }
    if w.complete {
        return false;
    }
    let full = sim
        .vehicle(m.vehicle)
        .is_some_and(|v| v.seats.iter().all(|s| s.occupant.is_some()));
    if thinking && w.started.is_none() {
        w.started = Some(sim.tick);
    }
    let start = if sim.ground_rules == GroundRules::Bf1 {
        Some(w.seated)
    } else {
        w.started
    };
    // Signed widening models now-minus-two even during the first two seconds.
    // MOVE OUT / AT EASE release on both games (BF1's rule, issue #7).
    let released = w
        .released
        .is_some_and(|t| i128::from(sim.tick) > i128::from(t) - 2 * i128::from(crate::TICK_HZ));
    // BF2 tests humans before occupancy (re-driver-wait.md §5). Reuse the
    // first-think clock: neither a later arrival nor a full seat restarts it.
    // A release ends the human hold.
    if sim.ground_rules == GroundRules::Bf2
        && !released
        && w.started
            .is_none_or(|t| sim.tick < t + 15 * u64::from(crate::TICK_HZ))
        && human_approaching(sim, m)
    {
        return true;
    }
    if !full && maintained_boarder(sim, m) {
        return true;
    }
    if full || released || start.is_some_and(|t| sim.tick > t + 4 * u64::from(crate::TICK_HZ)) {
        w.complete = true;
        return false;
    }
    true
}

/// Resolve current player slots on every update, including respawn generations.
/// BF2's member test uses 3-D range and the human's forward aim row (§5).
fn human_approaching(sim: &Sim, m: Mount) -> bool {
    let Some(v) = sim.vehicle(m.vehicle) else {
        return false;
    };
    let Some(driver) = v.seats[usize::from(m.seat)]
        .occupant
        .and_then(|h| sim.unit(h))
    else {
        return false;
    };
    sim.players.iter().enumerate().any(|(slot, handle)| {
        let Some(u) = handle.and_then(|h| sim.unit(h)) else {
            return false;
        };
        let to = v.pos - u.soldier.pos;
        let distance_sq = to.length_squared();
        u.controller == (crate::unit::Controller::Player { slot: slot as u8 })
            && u.team == driver.team
            && u.mount.is_none()
            && distance_sq <= 150.0 * 150.0
            && (distance_sq < 12.0 * 12.0
                || u.aim_frame().0.dot(to.normalize_or_zero()) > 0.8660254)
    })
}

/// Has this walker come within `reach` of the hub its route ends at, where
/// that is not its post's own (`route::post_hub`, card WAI: the hub nearest
/// the post its type can reach; card WAI2: a hunter's — the enemy's hub)? It
/// stops short of the node, so its hull is left off it. (Card WAI2: a last
/// leg planned far from that hub — the walker re-planned between two hubs —
/// is walked to it; it used to count as arrived where it stood.)
pub(super) fn at_drop(
    st: &AiState,
    post: Option<&crate::cp::CommandPost>,
    pos: Vec3,
    reach: f32,
) -> bool {
    st.route.is_some_and(|r| {
        post.is_none_or(|cp| cp.nav_hub != Some(r.dest_hub))
            && r.next_hub == r.dest_hub
            && nav::xz_dist(pos, r.waypoint) <= reach
    })
}

/// Refresh only after valid boarding work wins the foot priority chain on a
/// scheduled think. Turret offers and follower entry keep their existing policy.
pub(super) fn maintain(sim: &Sim, me: UnitHandle, st: &mut AiState) {
    if let Some(r) = st.ride.as_mut().filter(|r| {
        r.board.is_some_and(|vh| {
            sim.vehicle(vh).is_some_and(|v| {
                matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
                    || super::flyer_drive::can_fly(sim, v)
            })
        })
    }) {
        r.maintenance = Some(super::state::BoardMaintenance {
            owner: me,
            tick: sim.tick,
        });
    }
}

/// Resolve both entity generations and current eligibility every tick. A copied
/// record cannot survive unit reuse; `board` carries the vehicle generation.
fn maintained_boarder(sim: &Sim, m: Mount) -> bool {
    let Some(driver) = sim
        .vehicle(m.vehicle)
        .and_then(|v| v.seats.get(usize::from(m.seat)))
        .and_then(|s| s.occupant)
        .and_then(|h| sim.unit(h))
    else {
        return false;
    };
    sim.alive_units().any(|(h, u)| {
        u.controller == crate::unit::Controller::Ai
            && u.team == driver.team
            && u.mount.is_none()
            && u.ai.ride.is_some_and(|r| {
                r.board == Some(m.vehicle)
                    && sim.tick < r.since + BOARD_GIVEUP
                    && r.maintenance.is_some_and(|c| {
                        c.owner == h && c.tick <= sim.tick && sim.tick < c.tick + BOARD_FRESH
                    })
            })
            && u.ai
                .ride
                .is_some_and(|r| claimed_seat(sim, h, m.vehicle, r).is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cp::CpDef;
    use crate::heightfield::Heightfield;
    use crate::region::Region;
    use crate::unit::{Controller, UnitSpec};
    use crate::vehicle::hover::tests::skiff;
    use crate::vehicle::{SeatSetup, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;

    /// Flat ground, an enemy post 300 m down −Z, a bot at the origin and a
    /// skiff-like hover (pilot + one gunner) 12 m away, facing −Z.
    fn world() -> (Sim, UnitHandle, VehicleHandle) {
        let mut s = Sim::empty(3);
        s.heightfield = Some(Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        });
        s.hover_params = vec![Some(skiff())];
        s.seat_setup = vec![Some(SeatSetup {
            radius: 5.0,
            body_active: vec![],
            boardable: vec![true; 2],
            exit_drop: 10.0,
            ai_size: 8,
            guns: Vec::new(),
        })];
        let post = Vec3::new(0.0, 0.0, -300.0);
        s.add_cp(
            CpDef {
                name: "far".into(),
                pos: post,
                capture: Region::sphere(post, 8.0),
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
        let b = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let v = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(12.0, 2.0, 0.0),
            yaw: std::f32::consts::PI,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        });
        (s, b, v)
    }

    fn bail_world(
        kind: VehicleKind,
        draw: u8,
        health: f32,
        gunner_first: bool,
    ) -> (Sim, UnitHandle, UnitHandle, VehicleHandle) {
        let (mut s, first, v) = world();
        let second = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let (pilot, gunner) = if gunner_first {
            (second, first)
        } else {
            (first, second)
        };
        s.vehicle_mut(v).unwrap().kind = kind;
        if kind == VehicleKind::Walker {
            s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
        }
        assert!(s.enter_vehicle(pilot, v, 0));
        assert!(s.enter_vehicle(gunner, v, 1));
        s.unit_mut(pilot)
            .unwrap()
            .ai
            .ride
            .as_mut()
            .unwrap()
            .bail_draw = Some(draw);
        // A gunner's passing draw must not override a failing pilot draw.
        s.unit_mut(gunner)
            .unwrap()
            .ai
            .ride
            .as_mut()
            .unwrap()
            .bail_draw = Some(0);
        for h in [pilot, gunner] {
            let ai = &mut s.unit_mut(h).unwrap().ai;
            ai.target_cp = Some(0);
            ai.next_think = 0;
        }
        s.vehicle_mut(v).unwrap().health.cur = health;
        (s, pilot, gunner, v)
    }

    #[test]
    fn vbail_threshold_draw_and_all_ai_seats_on_next_think() {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for gunner_first in [false, true] {
                for (draw, health, leaves) in [
                    (0, 190.0, true),
                    (100, 190.0, true),
                    (83, 190.0, false),
                    // Low two bits, not one: 2 and 6 fail, 4 passes.
                    (2, 190.0, false),
                    (6, 190.0, false),
                    (4, 190.0, true),
                    (0, 200.0, false),
                    (0, 210.0, false),
                ] {
                    let (mut s, p, g, v) = bail_world(kind, draw, health, gunner_first);
                    s.step(&[]);
                    assert_eq!(
                        s.unit(p).unwrap().mount.is_none(),
                        leaves,
                        "pilot {kind:?} draw={draw} health={health}"
                    );
                    assert_eq!(
                        s.unit(g).unwrap().mount.is_none(),
                        leaves,
                        "gunner {kind:?} draw={draw} health={health}"
                    );
                    assert_eq!(s.nav_stats.ai_bail_drivers, u64::from(leaves));
                    assert_eq!(s.nav_stats.ai_bail_occupants, 2 * u64::from(leaves));
                    assert_eq!(s.nav_stats.ai_bail_exposed, u64::from(health < 200.0));
                    if leaves {
                        for h in [p, g] {
                            let u = s.unit(h).unwrap();
                            assert!(u.ai.ride.unwrap().bail_draw.is_none());
                            assert_eq!(u.ai.ride.unwrap().block_until, crate::ai::BOARD_BLOCK);
                            assert!(u.soldier.pos.y.abs() < 0.1, "flat-ground exit");
                        }
                        assert!(s.vehicle(v).unwrap().is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn vbail_waits_for_pilot_think_and_does_not_evict_humans() {
        let (mut s, p, g, v) = bail_world(VehicleKind::Hover, 0, 190.0, true);
        s.unit_mut(p).unwrap().ai.next_think = 10;
        s.step(&[]);
        assert!(s.unit(p).unwrap().mount.is_some());
        assert!(s.unit(g).unwrap().mount.is_some());
        assert_eq!(s.nav_stats.ai_bail_exposed, 1);
        s.unit_mut(g).unwrap().controller = Controller::Player { slot: 0 };
        s.unit_mut(g).unwrap().is_player = true;
        s.unit_mut(p).unwrap().ai.next_think = s.tick;
        s.step(&[PadState::default()]);
        assert!(s.unit(p).unwrap().mount.is_none());
        assert_eq!(s.unit(g).unwrap().mount.unwrap().vehicle, v);
        assert_eq!(s.nav_stats.ai_bail_exposed, 1);
        assert_eq!(s.nav_stats.ai_bail_occupants, 1);
        // Human pilot does not authorize the gunner's passing draw.
        let (mut s, p, g, v) = bail_world(VehicleKind::Hover, 0, 190.0, false);
        s.unit_mut(p).unwrap().controller = Controller::Player { slot: 0 };
        s.unit_mut(p).unwrap().is_player = true;
        assert!(!vehicle_bail_allowed(&s, v));
        s.step(&[PadState::default()]);
        assert!(s.unit(g).unwrap().mount.is_some());
        assert_eq!(s.nav_stats.ai_bail_drivers, 0);
    }

    #[test]
    fn vbail_draw_uses_entropy_once_per_successful_boarding_and_is_hashed() {
        let (mut s, p, v) = world();
        let mut rng = s.rng.clone();
        let expected = (rng.next_rand() % 101) as u8;
        assert!(s.enter_vehicle(p, v, 0));
        assert_eq!(s.rng.state(), rng.state());
        assert_eq!(
            s.unit(p).unwrap().ai.ride.unwrap().bail_draw,
            Some(expected)
        );
        assert!(!s.enter_vehicle(p, v, 1));
        assert_eq!(s.rng.state(), rng.state());
        let hash = s.hash();
        s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some((expected + 1) % 101);
        assert_ne!(s.hash(), hash);
        s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some(expected);
        s.unit_mut(p).unwrap().ai.ride.as_mut().unwrap().bail_seen = true;
        assert_ne!(s.hash(), hash);
        let draw = s.unit(p).unwrap().ai.ride.unwrap().bail_draw;
        let mut st = s.unit(p).unwrap().ai;
        settle(&s, &mut st);
        assert_eq!(st.ride.unwrap().bail_draw, draw);
        s.exit_vehicle(p);
        assert!(s.unit(p).unwrap().ai.ride.unwrap().bail_draw.is_none());
        assert!(s.enter_vehicle(p, v, 0));
        let expected = (rng.next_rand() % 101) as u8;
        assert_eq!(s.rng.state(), rng.state());
        assert_eq!(
            s.unit(p).unwrap().ai.ride.unwrap().bail_draw,
            Some(expected)
        );
    }

    #[test]
    fn vbail_autonomous_boarding_keeps_draw_and_non_ground_entries_consume_none() {
        let (mut s, p, v) = world();
        s.unit_mut(p).unwrap().soldier.pos = s.vehicle(v).unwrap().pos;
        s.unit_mut(p).unwrap().ai.ride = Some(RideState {
            board: Some(v),
            use_req: true,
            ..RideState::default()
        });
        s.ai_use_pass();
        assert!(s.unit(p).unwrap().ai.ride.unwrap().bail_draw.is_some());
        for kind in [VehicleKind::Flyer, VehicleKind::Turret] {
            let (mut s, p, v) = world();
            s.vehicle_mut(v).unwrap().kind = kind;
            let rng = s.rng.state();
            assert!(s.enter_vehicle(p, v, 0));
            assert_eq!(s.rng.state(), rng);
            assert!(s.unit(p).unwrap().ai.ride.is_none());
        }
        let (mut s, p, v) = world();
        s.unit_mut(p).unwrap().controller = Controller::Player { slot: 0 };
        let rng = s.rng.state();
        assert!(s.enter_vehicle(p, v, 0));
        assert_eq!(s.rng.state(), rng);
    }

    #[test]
    fn vbail_seeded_draw_population_and_exact_passing_values() {
        assert_eq!((0..=100).filter(|n| n & 3 == 0).count(), 26);
        let mut total = 0;
        for seed in 1..=8 {
            let (mut s, p, v) = world();
            s.rng = crate::rng::SimRng::new(seed);
            let mut passing = 0;
            for _ in 0..101 {
                assert!(s.enter_vehicle(p, v, 0));
                passing +=
                    usize::from(s.unit(p).unwrap().ai.ride.unwrap().bail_draw.unwrap() & 3 == 0);
                s.exit_vehicle(p);
            }
            total += passing;
            eprintln!("VBAIL synthetic seed {seed}: {passing}/101 passing draws");
        }
        let share = total as f32 / 808.0;
        eprintln!(
            "VBAIL synthetic aggregate {total}/808 = {:.2}% (not a map sweep)",
            100.0 * share
        );
        assert!((0.15..=0.37).contains(&share));
    }

    /// The whole card on one plane: the bot claims the hover, walks to it,
    /// takes the pilot seat, drives it to the post and gets out there.
    #[test]
    fn a_bot_boards_drives_to_its_post_and_gets_out() {
        let (mut s, b, v) = world();
        let mut boarded_at = None;
        let mut out_at = None;
        for _ in 0..(90 * crate::TICK_HZ) {
            s.step(&[]);
            let seated = s.unit(b).unwrap().mount.is_some();
            if seated && boarded_at.is_none() {
                boarded_at = Some(s.tick);
            }
            if !seated && boarded_at.is_some() {
                out_at = Some(s.tick);
                break;
            }
        }
        let boarded = boarded_at.expect("the bot never boarded");
        assert!(
            boarded < 10 * u64::from(crate::TICK_HZ),
            "boarded at {boarded}"
        );
        let out = out_at.expect("the bot never got out");
        let u = s.unit(b).unwrap();
        let post = Vec3::new(0.0, 0.0, -300.0);
        let arrive = skiff().forward_speed * crate::ai::HOVER_ARRIVE_FACTOR;
        let d = nav::xz_dist(s.vehicle(v).unwrap().pos, post);
        assert!(
            d <= arrive + 1.0,
            "got out {d} m from the post (tick {out})"
        );
        assert!(nav::xz_dist(u.soldier.pos, post) < arrive + 12.0);
        assert_eq!(s.nav_stats.ai_boards, 1);
        assert_eq!(s.nav_stats.ai_dismounts, 1);
        assert!(s.nav_stats.ai_drive_ticks > 0);
        // It does not hop straight back in.
        for _ in 0..crate::TICK_HZ * 5 {
            s.step(&[]);
        }
        assert!(s.unit(b).unwrap().mount.is_none());
    }

    /// Card VMESH: a driver getting out with its stuck window spent marks
    /// its vehicle where it stands, an ordinary dismount does not; no bot claims a marked
    /// vehicle until it has moved [`STUCK_MOVED`] (a walker wedged on BF1
    /// Endor's cp2 boulder was boarded by pair after pair).
    #[test]
    fn a_vehicle_given_up_as_stuck_is_not_claimed_until_it_moves() {
        let (mut s, b, v) = world();
        let out = |s: &mut Sim, stuck: bool| {
            assert!(s.enter_vehicle(b, v, 0));
            let u = &mut s.units[b.index as usize];
            let r = u.ai.ride.unwrap_or_default();
            u.ai.ride = Some(RideState { use_req: true, ..r });
            // The driver's stuck window, as its vehicle's drive left it.
            u.ai.route = Some(crate::ai::Route {
                stuck_run: if stuck {
                    crate::ai::DRIVE_STUCK_BAIL
                } else {
                    0
                },
                ..Default::default()
            });
            s.ai_use_pass();
            assert!(s.unit(b).unwrap().mount.is_none());
            s.units[b.index as usize].ai.ride = None;
        };
        out(&mut s, false);
        assert_eq!(s.vehicle(v).unwrap().stuck_at, None, "a plain dismount");
        out(&mut s, true);
        let at = s.vehicle(v).unwrap().pos;
        assert_eq!(
            s.vehicle(v).unwrap().stuck_at,
            Some(at),
            "given up as stuck"
        );
        // The claim is made in a think: a fresh bot 12 m off makes it at
        // once (the moved case below), and never at the marked vehicle.
        for _ in 0..(3 * crate::TICK_HZ) {
            s.step(&[]);
            let u = s.unit(b).unwrap();
            assert!(
                u.mount.is_none() && u.ai.ride.and_then(|r| r.board).is_none(),
                "claimed a stuck vehicle"
            );
        }
        // Moved off the spot, it is a vehicle again.
        let pos = s.vehicle(v).unwrap().pos + Vec3::new(0.0, 0.0, 1.5 * STUCK_MOVED);
        s.vehicles[v.index as usize].pos = pos;
        s.sync_hull(v.index as usize);
        s.units[b.index as usize].soldier.pos = Vec3::ZERO;
        let boarded = (0..(10 * crate::TICK_HZ)).any(|_| {
            s.step(&[]);
            s.unit(b).unwrap().mount.is_some()
        });
        assert!(boarded, "never boarded once it had moved");
    }

    /// A driver routes as its vehicle's AISizeType (MEDIUM → table 3) and
    /// arrives at ForwardSpeed × 0.5.
    #[test]
    fn a_seated_bot_routes_as_its_vehicle() {
        let (mut s, b, v) = world();
        assert_eq!(crate::ai::mover(&s, s.unit(b).unwrap()).ty, nav::SOLDIER);
        assert!(s.enter_vehicle(b, v, 0));
        assert_eq!(crate::ai::mover(&s, s.unit(b).unwrap()).ty, 3);
    }

    /// Two bots, one vehicle: one claim, one driver.
    #[test]
    fn one_vehicle_takes_one_driver() {
        let (mut s, b, v) = world();
        let c = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(2.0, 0.0, 0.0))
            })
            .unwrap();
        for _ in 0..(10 * crate::TICK_HZ) {
            s.step(&[]);
        }
        let seated: Vec<bool> = [b, c]
            .iter()
            .map(|&h| s.unit(h).unwrap().mount.is_some())
            .collect();
        assert_eq!(seated.iter().filter(|&&x| x).count(), 1, "{seated:?}");
        assert!(s.vehicle(v).unwrap().seats[1].occupant.is_none());
    }

    /// Card VTAI: two bots and a hover with an armed gunner seat — one
    /// drives, and the other claims the gunner seat and boards it while the
    /// pilot holds its throttle.
    /// [`world`]'s gunner seat gets a gun.
    fn arm_gunner_seat(s: &mut Sim) {
        s.seat_setup[0].as_mut().unwrap().guns = vec![
            None,
            Some(crate::vehicle::gun::GunSetup {
                weapons: vec![crate::vehicle::gun::GunWeapon {
                    class: 0,
                    infinite: true,
                    reserve_clips: 0,
                }],
                fire_point: Vec3::new(0.0, 0.9, 4.2),
                pivot: Vec3::new(0.0, 0.8, 3.3),
                eye: Vec3::new(0.0, 1.0, 2.0),
                aimer: crate::vehicle::gun::Aimer::default(),
                pilot: false,
                turn: None,
                framed: false,
                aimer_bind: glam::Quat::IDENTITY,
                gunner: None,
            }),
        ];
    }

    #[test]
    fn a_second_bot_takes_the_gunner_seat() {
        let (mut s, b, v) = world();
        arm_gunner_seat(&mut s);
        let c = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(-8.0, 0.0, 0.0))
            })
            .unwrap();
        let mut both = false;
        for _ in 0..(20 * crate::TICK_HZ) {
            s.step(&[]);
            let seated = [b, c].map(|h| s.unit(h).unwrap().mount.is_some());
            if seated == [true, true] {
                both = true;
                break;
            }
        }
        assert!(both, "both bots aboard");
        assert_eq!(s.nav_stats.ai_gunners, 1);
        let seats = &s.vehicle(v).unwrap().seats;
        assert!(seats.iter().all(|x| x.occupant.is_some()));
        // The gunner stays aboard while the pilot drives.
        for _ in 0..(5 * crate::TICK_HZ) {
            s.step(&[]);
        }
        assert!(s.vehicle(v).unwrap().seats[1].occupant.is_some());
    }

    /// Card UTALK, BF1's need-gunner call (re-squad-orders.md 1, 5): an AI
    /// ground pilot calls independently of the vehicle's gun seats
    /// with chance 0.3, and every friendly AI soldier on foot within 30 m
    /// (3-D) answers "yes" - not one at 35 m, not an enemy, not a human, not
    /// the caller. BF2 remains silent; boarding admission is independent.
    #[test]
    fn a_seated_ai_calls_for_a_gunner_and_the_ai_near_it_answer() {
        use crate::sim::{Call, Event};
        let (mut s, b, v) = world();
        s.ground_rules = crate::soldier::GroundRules::Bf1;
        let ai = |team: u8, at: Vec3| UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, at)
        };
        let near = s.spawn(&ai(0, Vec3::new(12.0, 20.0, 20.0))).unwrap();
        let far = s.spawn(&ai(0, Vec3::new(12.0, 0.0, 35.0))).unwrap();
        let enemy = s.spawn(&ai(1, Vec3::new(12.0, 0.0, 5.0))).unwrap();
        let human = s
            .spawn(&UnitSpec::player(0, Vec3::new(12.0, 0.0, -5.0)))
            .unwrap();
        assert!(s.enter_vehicle(b, v, 0));
        // Over 1000 ticks of the call alone: (calls, answers by unit).
        let tally = |s: &mut Sim| {
            let (mut calls, mut yes) = (0, Vec::new());
            for t in 0..1000 {
                s.tick = t;
                s.events.clear();
                s.need_gunner_call(b, v);
                for e in &s.events {
                    match *e {
                        Event::Call {
                            unit,
                            call: Call::NeedGunner,
                        } => {
                            assert_eq!(unit, b);
                            calls += 1;
                        }
                        Event::Call {
                            unit,
                            call: Call::Yes,
                        } => yes.push(unit),
                        _ => {}
                    }
                }
            }
            (calls, yes)
        };
        let (calls, yes) = tally(&mut s);
        assert!((250..=350).contains(&calls), "unarmed ground call: {calls}");
        assert_eq!(yes.len(), calls);
        assert!(yes.iter().all(|&u| u == near));
        arm_gunner_seat(&mut s);
        let (calls, yes) = tally(&mut s);
        assert!((250..=350).contains(&calls), "chance 0.3: {calls} of 1000");
        assert_eq!(yes.len(), calls, "one answer per call");
        assert!(
            yes.iter().all(|&u| u == near),
            "only the one in reach: {:?}",
            [far, enemy, human]
        );
        s.ground_rules = crate::soldier::GroundRules::Bf2;
        assert_eq!(tally(&mut s).0, 0, "BF2");
        s.ground_rules = crate::soldier::GroundRules::Bf1;
        assert!(s.enter_vehicle(near, v, 1));
        assert_eq!(tally(&mut s).0, calls, "full ground vehicle still calls");
    }
    fn pickup_probe(rules: GroundRules, full: bool) {
        let (mut s, b, v) = world();
        s.ground_rules = rules;
        let mut boarded = None;
        for _ in 0..300 {
            s.step(&[]);
            if s.unit(b).unwrap().mount.is_some() {
                boarded = Some(s.tick);
                break;
            }
        }
        let boarded = boarded.expect("natural boarding failed");
        if full {
            let rider = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
            assert!(s.enter_vehicle(rider, v, 1));
        }
        assert_eq!(s.vehicle(v).unwrap().seats[1].occupant.is_some(), full);
        let start = s.vehicle(v).unwrap().pos;
        let mut max_throttle: f32 = 0.0;
        for _ in 0..60 {
            s.step(&[]);
            assert!(s.unit(b).unwrap().mount.is_some(), "pilot left");
            if let Some(h) = s.unit(b).unwrap().ai.ride.and_then(|r| r.hover) {
                max_throttle = max_throttle.max(h.sticks[0].abs()).max(h.sticks[1].abs());
            }
        }
        let delta = nav::xz_dist(start, s.vehicle(v).unwrap().pos);
        eprintln!("WAIT {rules:?} full={full} boarded={boarded} tick={} delta={delta:.6} max_stick={max_throttle:.6}",s.tick);
        if full {
            assert!(
                delta > 1.0 && max_throttle > 0.0,
                "full vehicle must depart"
            );
        } else {
            assert_eq!(
                max_throttle, 0.0,
                "new driver must wait while empty seat and within four seconds"
            );
            assert!(delta < 0.1, "waiting vehicle must stay put");
        }
        if !full {
            let w = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
            assert_eq!(
                w.seated + 1,
                boarded,
                "successful seat clock survives ai_use_pass"
            );
            let expected = if rules == crate::soldier::GroundRules::Bf1 {
                w.seated
            } else {
                w.started.unwrap()
            } + 120;
            let mut departure = None;
            for _ in 0..100 {
                s.step(&[]);
                let r = s.unit(b).unwrap().ai.ride.unwrap();
                let stick = r.sticks.map_or_else(
                    || {
                        r.hover
                            .map_or(0.0, |h| h.sticks[0].abs().max(h.sticks[1].abs()))
                    },
                    |x| x[0].abs().max(x[1].abs()),
                );
                if stick > 0.0 {
                    departure = Some(s.tick);
                    break;
                }
                assert!(crate::nav::xz_dist(start, s.vehicle(v).unwrap().pos) < 0.1);
                assert!(s.vehicle(v).unwrap().stuck_at.is_none());
            }
            let departure = departure.expect("driver must depart after initial window");
            assert!(
                (expected..=expected + crate::ai::THINK_PERIOD + 2).contains(&departure),
                "departure {departure}, deadline {expected}"
            );
            assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().complete);
            eprintln!("PICKUP {rules:?} boarded={boarded} start={:?} deadline={expected} complete/depart={departure} pos={:?}", w.started, s.vehicle(v).unwrap().pos);
        }
    }
    #[test]
    fn pickup_bf1_empty() {
        pickup_probe(GroundRules::Bf1, false);
    }
    #[test]
    fn pickup_bf2_empty() {
        pickup_probe(GroundRules::Bf2, false);
    }
    #[test]
    fn pickup_bf1_full_control() {
        pickup_probe(GroundRules::Bf1, true);
    }
    #[test]
    fn pickup_bf2_full_control() {
        pickup_probe(GroundRules::Bf2, true);
    }
    fn pickup_world(kind: VehicleKind) -> (Sim, UnitHandle, VehicleHandle) {
        let (mut s, b, v) = world();
        if kind == VehicleKind::Walker {
            s.vehicle_mut(v).unwrap().kind = kind;
            s.walker_params = vec![Some(crate::vehicle::walker::tests::atst())];
        }
        (s, b, v)
    }

    fn locomotion(s: &Sim, b: UnitHandle) -> f32 {
        let r = s.unit(b).unwrap().ai.ride.unwrap();
        r.sticks.map_or_else(
            || {
                r.hover
                    .map_or(0.0, |h| h.sticks[0].abs().max(h.sticks[1].abs()))
            },
            |x| x[0].abs().max(x[1].abs()),
        )
    }

    #[test]
    fn pickup_lifecycle_success_refusal_handover_switch_and_handle_reuse() {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            let (mut s, b, v) = pickup_world(kind);
            assert!(!s.enter_vehicle(b, v, 9));
            assert!(s.unit(b).unwrap().ai.ride.is_none());
            s.tick = 500; // Distinguish a new tenure from an expired zero clock.
            assert!(s.enter_vehicle(b, v, 0));
            let w = s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap();
            assert_eq!(
                (w.vehicle, w.seated, w.started, w.complete),
                (v, 500, None, false)
            );
            assert!(!s.enter_vehicle(b, v, 1));
            assert_eq!(s.unit(b).unwrap().ai.ride.unwrap().pickup, Some(w));
            s.step(&[]);
            assert_eq!(locomotion(&s, b), 0.0);
            let old = s.unit(b).unwrap().ai.ride.unwrap().pickup;
            s.exit_vehicle(b);
            assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
            assert!(s.enter_vehicle(b, v, 0));
            assert_ne!(s.unit(b).unwrap().ai.ride.unwrap().pickup, old);
            s.step(&[]);
            assert_eq!(locomotion(&s, b), 0.0);
            // A successful seat switch uses unseat/seat, just like switch_pass.
            s.unseat_unit(b);
            assert!(s.seat_unit(b, v, 1));
            assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
            s.unseat_unit(b);
            assert!(s.seat_unit(b, v, 0));
            assert_eq!(
                s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().seated,
                s.tick
            );
            // A human takeover ends the AI tenure; the next AI pilot starts fresh.
            let human = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
            assert!(s.enter_vehicle(human, v, 0));
            assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
            assert!(s.unit(human).unwrap().ai.ride.is_none());
            s.exit_vehicle(human);
            assert!(s.enter_vehicle(b, v, 0));
            s.remove_vehicle(v);
            assert!(s.unit(b).unwrap().ai.ride.unwrap().pickup.is_none());
            let replacement = s.spawn_vehicle(&VehicleSpec {
                kind,
                class: 0,
                home_team: None,
                neutral_when_empty: true,
                spawn_pad: None,
                pos: Vec3::ZERO,
                yaw: 0.0,
                max_health: 1000.0,
                health_type: HealthType::Vehicle,
                seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
            });
            assert_eq!(v.index, replacement.index);
            assert_ne!(v.gen, replacement.gen);
            assert!(s.enter_vehicle(b, replacement, 0));
            s.step(&[]);
            assert_eq!(
                s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().vehicle,
                replacement
            );
            assert_eq!(locomotion(&s, b), 0.0);
        }
    }

    #[test]
    fn pickup_wait_preserves_progress_rng_events_and_bailout_priority() {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            let (mut s, b, v) = pickup_world(kind);
            assert!(s.enter_vehicle(b, v, 0));
            let mut st = s.unit(b).unwrap().ai;
            st.route = Some(super::super::state::Route {
                waypoint: Vec3::new(0.0, 0.0, -300.0),
                ref_dist: 300.0,
                stuck_ticks: 59,
                stuck_run: 100,
                sidestep: 7,
                ..Default::default()
            });
            let route = st.route;
            let follower = s.vehicle(v).unwrap().hover.ai;
            let rng = s.rng.state();
            let events = s.events.len();
            let m = s.unit(b).unwrap().mount.unwrap();
            for tick in 0..120 {
                s.tick = tick;
                let (_, nt) = drive(&s, b, m, &mut st, NavTick::default(), tick % 10 == 0);
                assert_eq!(st.route, route, "wait cannot spend route progress");
                assert_eq!(nt.stuck_run, 0);
                assert!(!st.ride.unwrap().use_req);
                if let Some(h) = st.ride.unwrap().hover {
                    assert_eq!(h.ai, follower);
                }
            }
            assert_eq!(s.rng.state(), rng);
            assert_eq!(s.events.len(), events, "wait emits no call/reply");
            s.unit_mut(b).unwrap().ai = st;
            s.unit_mut(b).unwrap().ai.ride.as_mut().unwrap().bail_draw = Some(0);
            s.unit_mut(b).unwrap().ai.next_think = 0;
            s.vehicle_mut(v).unwrap().health.cur = 100.0;
            s.step(&[]);
            assert!(s.unit(b).unwrap().mount.is_none(), "bailout preempts wait");
        }
    }

    #[test]
    fn pickup_reachable_different_clocks_hash_and_continue_differently() {
        // Both scenes stay still: only the time of successful entry differs.
        // A player controller holds the late entrant on foot until entry.
        for rules in [GroundRules::Bf1, GroundRules::Bf2] {
            let scene = |late: bool| {
                let (mut s, b, v) = world();
                s.ground_rules = rules;
                if late {
                    s.unit_mut(b).unwrap().controller = Controller::Player { slot: 0 };
                } else {
                    assert!(s.enter_vehicle(b, v, 0));
                }
                for _ in 0..30 {
                    s.step(&[]);
                }
                if late {
                    s.unit_mut(b).unwrap().controller = Controller::Ai;
                    assert!(s.enter_vehicle(b, v, 0));
                }
                for _ in 0..10 {
                    s.step(&[]);
                }
                (
                    s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap(),
                    s.vehicle(v).unwrap().pos,
                )
            };
            let (early, ep) = scene(false);
            let (late, lp) = scene(true);
            assert_eq!(ep, lp, "identical stationary geometry");
            assert_ne!(early.seated, late.seated);
            // Put each reachable clock into the same stationary continuation.
            // Every other sim field, including RNG, geometry and AI, is equal.
            let continuation = |wait| {
                let (mut s, b, v) = world();
                s.ground_rules = rules;
                assert!(s.enter_vehicle(b, v, 0));
                for _ in 0..40 {
                    s.step(&[]);
                }
                s.tick = 125;
                s.unit_mut(b).unwrap().ai.ride.as_mut().unwrap().pickup = Some(wait);
                (s, b, v)
            };
            let (mut a, ab, av) = continuation(early);
            let (mut c, cb, cv) = continuation(late);
            assert_eq!(a.vehicle(av).unwrap().pos, c.vehicle(cv).unwrap().pos);
            assert_ne!(
                a.hash(),
                c.hash(),
                "different future departure must be hashed"
            );
            a.step(&[]);
            c.step(&[]);
            assert!(
                a.unit(ab)
                    .unwrap()
                    .ai
                    .ride
                    .unwrap()
                    .pickup
                    .unwrap()
                    .complete
            );
            assert!(
                !c.unit(cb)
                    .unwrap()
                    .ai
                    .ride
                    .unwrap()
                    .pickup
                    .unwrap()
                    .complete
            );
            assert!(locomotion(&a, ab) > 0.0);
            assert_eq!(locomotion(&c, cb), 0.0);
        }
    }
    #[test]
    fn pickup_clock_boundary_uses_source_rule_and_first_think() {
        for kind in [VehicleKind::Hover, VehicleKind::Walker] {
            for rules in [GroundRules::Bf1, GroundRules::Bf2] {
                let (mut s, b, v) = pickup_world(kind);
                s.ground_rules = rules;
                let mut expected_rng = s.rng.clone();
                expected_rng.next_rand(); // unchanged VBAIL boarding draw
                assert!(s.enter_vehicle(b, v, 0));
                assert_eq!(s.rng.state(), expected_rng.state());
                assert!(!s.enter_vehicle(b, v, 0));
                assert_eq!(s.rng.state(), expected_rng.state());
                s.unit_mut(b).unwrap().ai.next_think = 10;
                for _ in 0..10 {
                    s.step(&[]);
                }
                assert!(s
                    .unit(b)
                    .unwrap()
                    .ai
                    .ride
                    .unwrap()
                    .pickup
                    .unwrap()
                    .started
                    .is_none());
                s.step(&[]);
                assert_eq!(
                    s.unit(b).unwrap().ai.ride.unwrap().pickup.unwrap().started,
                    Some(10)
                );
                let end = if rules == GroundRules::Bf1 { 120 } else { 130 };
                while s.tick <= end {
                    s.step(&[]);
                    assert_eq!(locomotion(&s, b), 0.0);
                }
                s.step(&[]);
                assert!(locomotion(&s, b) > 0.0);
                eprintln!(
                    "BOUNDARY {kind:?} {rules:?} seated=0 first_think=10 departure={}",
                    s.tick
                );
            }
        }
        let (mut s, b, v) = world();
        s.vehicle_mut(v).unwrap().kind = VehicleKind::Turret;
        let rng = s.rng.state();
        assert!(s.enter_vehicle(b, v, 0));
        assert!(s.unit(b).unwrap().ai.ride.is_none());
        assert_eq!(rng, s.rng.state(), "turret has no wait or boarding draw");
    }
    #[path = "post_setout_tests.rs"]
    mod post_setout_tests;
    #[path = "seat_call_tests.rs"]
    mod seat_call_tests;
    #[path = "setout_call_tests.rs"]
    mod setout_call_tests;
    #[path = "wait_call_tests.rs"]
    mod wait_call_tests;

    #[path = "call_board_tests.rs"]
    mod call_board_tests;
    #[path = "claim_wait_tests.rs"]
    mod claim_wait_tests;
    mod get_out_tests;
    #[path = "human_wait_tests.rs"]
    mod human_wait_tests;
    #[path = "order_wait_tests.rs"]
    mod order_wait_tests;
    #[path = "pile_in_tests.rs"]
    mod pile_in_tests;
}
