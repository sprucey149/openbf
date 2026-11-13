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
    BOARD_TRIP, GUNNER_WAIT, OTHER_FLOOR, RIDE_GAIN, VEHICLE_NOTIFY_RADIUS,
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
    let mut r = st.ride.unwrap_or_default();
    let endgame = r.command_dismounted && super::walker_drive::command_endgame(sim, u.team);
    if r.command_dismounted && !endgame {
        r.command_dismounted = false;
        st.ride = Some(r);
    }
    if r.board.is_some()
        || u.droideka.is_some()
        || sim.tick < r.block_until
        || sim.tick < r.own_search_until
        || sim.vehicles.is_empty()
    {
        return;
    }
    let cp = st.target_cp.and_then(|i| sim.cps.get(i as usize));
    let space = sim.round_rules().space_deathmatch();
    if cp.is_none() && !space {
        return;
    }
    let pos = u.soldier.pos;
    // (A command walker is boarded whatever the trip: see [`command`].)
    let near_post = cp.is_some_and(|cp| nav::xz_dist(pos, cp.def.pos) <= BOARD_TRIP);
    let taken = |vh: VehicleHandle| {
        sim.units.iter().enumerate().any(|(i, o)| {
            i != me.index as usize && o.alive && o.ai.ride.is_some_and(|r| r.board == Some(vh))
        })
    };
    let mut best: Option<(f32, VehicleHandle)> = None;
    for (vh, v) in sim.alive_vehicles() {
        // OURS: an endgame escape must not reopen the command seat for the
        // same nearby crew every BOARD_BLOCK. Explicit calls/orders bypass
        // autonomous search. measure: PS2 last-crew reboarding decisions.
        if r.command_dismounted && endgame && command(sim, vh) {
            continue;
        }
        // Card TURR: a turret is offered with a purpose (`man::offer_pass`).
        if v.kind == VehicleKind::Turret {
            continue;
        }
        if !v.health.alive() {
            continue;
        }
        let space_flyer = space && v.kind == VehicleKind::Flyer;
        let d = if space_flyer {
            (v.pos - pos).length()
        } else {
            nav::xz_dist(pos, v.pos)
        };
        let too_far = if space_flyer {
            d > VEHICLE_NOTIFY_RADIUS
        } else {
            d > BOARD_SEEK || (near_post && !command(sim, vh))
        };
        if too_far || best.is_some_and(|(b, _)| d >= b) {
            continue;
        }
        // Card VMESH: not one a driver gave up as stuck that has not moved since.
        if v.stuck_at
            .is_some_and(|p| nav::xz_dist(p, v.pos) < STUCK_MOVED)
        {
            continue;
        }
        let routed = space_flyer || cp.is_some_and(|cp| routes_to(sim, vh, cp));
        if routed && claim_seat(sim, me, vh).is_some() && !taken(vh) {
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
    } else if sim.round_rules().space_deathmatch()
        && sim
            .vehicle(vh)
            .is_some_and(|v| v.kind == VehicleKind::Flyer)
    {
        true
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
/// (the recovery ladder): the claim is given up and the vehicle blocked for
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
    st.route().is_some_and(|r| {
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
mod tests;
