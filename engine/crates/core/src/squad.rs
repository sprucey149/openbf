//! GBOARD pending call goals yield when a player order is installed, not on pool expiry.
//! BF1 release orders also shorten active ground-pilot pickup windows (WPOKE).
//! Following a mounted leader releases autonomous ground-claim maintenance (CWAIT).
//! Squad orders (card SQACK): BF1's four D-pad orders on bots, as the PS2 runs
//! them (`analysis/re-squad-orders.md`; PLAN plays BF1's orders on every map).
//!
//! The press marks every friendly AI soldier in reach ([`Pending`]); each
//! takes the order at its own next think, answers ([`crate::sim::Event::OrderTaken`])
//! and then does what the order says. Every writer runs inside `Sim::step`'s
//! fixed order (phase 0b and the AI write-back), so an order is replay-exact.
//!
//! | order | reaches (3-D) | a unit that takes it |
//! |---|---|---|
//! | MOVE OUT = AT EASE | [`RELEASE_REACH`] 45 m; active BF1 ground pilots 135 m | answers; a follower / holder ordered by this player is released |
//! | HOLD POSITION | [`ORDER_REACH`] 30 m | answers; stands where it is and fights from there |
//! | FOLLOW ME | [`ORDER_REACH`] 30 m | answers; the first [`FOLLOW_CAP`] to think follow the player |
//! | EVERYONE OUT (ground pilot Left) | own vehicle's AI seats | leaves; suppresses own seat search for 20 s |
//! | PILE IN (ground pilot Right) | [`ORDER_REACH`] 30 m | answers; tries to board the issued vehicle over its retained field order |
//!
//! A follower walks to where the leader stood, then follows him; nearer than
//! [`FOLLOW_NEAR`] it takes a point within +-[`STAND_HALF`] of him in his
//! frame and stands until he is past [`FOLLOW_FAR`]. A follower or a holder is
//! released after [`LEAVE_TICKS`] of thinks with the leader beyond
//! [`LEAVE_DIST`] (a count that is never refilled), by MOVE OUT / AT EASE, and
//! by the leader's death. The numbers are the RE doc's §3-4.
//!
//! PROVISIONAL (the port's, not the PS2's): which two units follow is the
//! port's think order (unit index among those thinking on a tick) and its
//! think period is 10 ticks, not the PS2's 15; the release count is in ticks
//! so it is 10 s either way; a holder does not look around; the approach also
//! ends [`APPROACH_BODY`] from the leader himself; beyond [`ROUTE_DIST`] a
//! follower routes over the hub graph. Other seated orders beyond a ground
//! pilot's Left/Right remain unbuilt, as do the AI's own on-foot orders' effect - card UTALK
//! speaks them (a
//! trooper's "follow me" / "move out" order nobody on the PS2 either; a new
//! pilot's need-gunner call, `Sim::need_gunner_call`, temporarily boards
//! recipients over their retained Hold/Follow order, WGORD).

use crate::ai::route::{self, Refresh};
use crate::unit::UnitHandle;
use glam::Vec3;

/// 3-D reach of HOLD POSITION and FOLLOW ME from the issuer (RE §1, §3).
pub const ORDER_REACH: f32 = 30.0;
/// 3-D reach of MOVE OUT and AT EASE.
pub const RELEASE_REACH: f32 = 45.0;
/// Followers one leader can have: the first two receivers to think (RE §4.3).
pub const FOLLOW_CAP: usize = 2;
/// A follower nearer its leader than this takes its standing point (a + 5 m,
/// a = 0: RE §4.3 state 3 -> 4).
pub const FOLLOW_NEAR: f32 = 5.0;
/// A standing follower whose leader is farther than this follows again
/// (b + 6 m, b = 3).
pub const FOLLOW_FAR: f32 = 9.0;
/// Half the side of the square about the leader, in his frame, a follower's
/// standing point is drawn in (b = 3.0).
pub const STAND_HALF: f32 = 3.0;
/// The standing point is at least this far from the leader.
pub const STAND_MIN: f32 = 1.0;
/// A follower has reached where its leader stood (state 2 -> 3), and a
/// commanded unit this close to its point stands (`ai::think`).
pub const STAND_ARRIVE: f32 = 0.45;
/// ...or it is this close to the leader himself: two soldiers do not overlap,
/// so a still leader's own spot cannot be reached. PROVISIONAL (the port's).
pub const APPROACH_BODY: f32 = 1.5;
/// Beyond this a follower routes to its leader's hub instead of walking
/// straight at him. PROVISIONAL (the port's).
pub const ROUTE_DIST: f32 = 10.0;
/// A follower or a holder whose leader is farther than this counts toward its
/// release (RE §4.2-4.3).
pub const LEAVE_DIST: f32 = 20.0;
/// Ticks of thinks beyond [`LEAVE_DIST`] that release it: the PS2's 20 thinks
/// of 0.5 s = 10 s, summed over the order and never refilled.
pub const LEAVE_TICKS: u16 = 300;

/// A field or driver D-pad order. The discriminant is `Event::Command`'s `kind`
/// byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Follow = 0,
    Hold = 1,
    AtEase = 2,
    MoveOut = 3,
    PileIn = 4,
    GetOut = 5,
}

impl Order {
    /// How far from the issuer the order reaches, 3-D.
    pub fn reach(self) -> f32 {
        match self {
            Order::Follow | Order::Hold | Order::PileIn | Order::GetOut => ORDER_REACH,
            Order::AtEase | Order::MoveOut => RELEASE_REACH,
        }
    }
}

/// The human responsible for an active PILE IN goal. The exact soldier
/// prevents the order surviving the issuing player's death or slot reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardIssuer {
    pub slot: u8,
    pub leader: UnitHandle,
}

impl BoardIssuer {
    pub(crate) fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        eat(b"PILEIN");
        eat(&[self.slot]);
        eat(&self.leader.index.to_le_bytes());
        eat(&self.leader.gen.to_le_bytes());
    }
}

/// An order a unit has been given and has not thought about yet: taken (or
/// lost) at its next think. HASHED only when `Some`, like [`Command`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pending {
    pub order: Order,
    pub issuer: u8,
    /// The issuer's soldier at the press.
    pub leader: UnitHandle,
    /// Mounted release delivery is bound to the vehicle generation and tenure.
    pub pickup: Option<(crate::vehicle::VehicleHandle, u64)>,
    /// The driver order's vehicle at the press, including its generation.
    pub vehicle: Option<crate::vehicle::VehicleHandle>,
}

impl Pending {
    pub fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        eat(&[2, self.order as u8, self.issuer]);
        eat(&self.leader.index.to_le_bytes());
        eat(&self.leader.gen.to_le_bytes());
        if let Some((v, seated)) = self.pickup {
            eat(&[3]);
            eat(&v.index.to_le_bytes());
            eat(&v.gen.to_le_bytes());
            eat(&seated.to_le_bytes());
        }
        if let Some(v) = self.vehicle {
            eat(b"PILEIN");
            eat(&v.index.to_le_bytes());
            eat(&v.gen.to_le_bytes());
        }
    }
}

/// What a player's order has a bot doing.
///
/// `Follow` carries the LEADER's handle, `Hold` the spot the bot stands on —
/// its OWN position when it took the order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommandKind {
    Follow(UnitHandle),
    Hold(Vec3),
}

impl CommandKind {
    /// The 13 bytes that go into the hash: a tag and the payload, the
    /// [`crate::commander::Objective::bytes`] pattern. Fixed width, so a
    /// `Follow` and a `Hold` cannot hash alike by having different lengths.
    pub fn bytes(self) -> [u8; 13] {
        let mut out = [0u8; 13];
        match self {
            CommandKind::Follow(h) => {
                out[0] = 1;
                out[1..5].copy_from_slice(&h.index.to_le_bytes());
                out[5..9].copy_from_slice(&h.gen.to_le_bytes());
            }
            CommandKind::Hold(p) => {
                out[0] = 2;
                out[1..5].copy_from_slice(&p.x.to_bits().to_le_bytes());
                out[5..9].copy_from_slice(&p.y.to_bits().to_le_bytes());
                out[9..13].copy_from_slice(&p.z.to_bits().to_le_bytes());
            }
        }
        out
    }

    /// The diagnostics name (`ROUND_VERBOSE`, the HUD feed).
    pub fn label(self) -> String {
        match self {
            CommandKind::Follow(h) => format!("follow #{}", h.index),
            CommandKind::Hold(p) => format!("hold ({:.0},{:.0})", p.x, p.z),
        }
    }
}

/// One order on one bot: what, from which player SLOT, and when.
///
/// `issuer` is the slot and not the issuer's `UnitHandle` because the things
/// that read it — "release what I ordered", "my death clears my orders", "how
/// far is my leader" — are per PLAYER, and a player's handle changes every
/// respawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command {
    pub kind: CommandKind,
    pub issuer: u8,
    pub issued_tick: u64,
}

impl Command {
    /// The bytes [`crate::unit::Unit::hash_into`] eats for a unit that HAS an
    /// order. A unit with NONE eats nothing at all — not even a 0 byte — so the
    /// hash of a command-free sim is byte-for-byte what it was before Phase D
    /// (brief addendum 3, and the reason the replay gate values hold). The
    /// leading 1 tag is what keeps the stream unambiguous.
    pub fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        eat(&[1]);
        eat(&self.kind.bytes());
        eat(&[self.issuer]);
        eat(&self.issued_tick.to_le_bytes());
    }
}

/// The pad bit each order answers, in the PRIORITY order phase 0b resolves
/// them in when several rise on one tick — at most ONE order per slot per
/// tick. Fixed and documented rather than derived from the bit order, because
/// a pad that reports two bits on the same tick has to mean something definite.
const EDGE_ORDER: [(u32, Order); 4] = [
    (crate::sim::buttons::SQUAD_MOVE_OUT, Order::MoveOut),
    (crate::sim::buttons::SQUAD_AT_EASE, Order::AtEase),
    (crate::sim::buttons::SQUAD_FOLLOW, Order::Follow),
    (crate::sim::buttons::SQUAD_HOLD, Order::Hold),
];

impl crate::sim::Sim {
    /// Human ground pilot only. Capacity affects entry, never the press feedback.
    fn ground_pilot_vehicle(&self, slot: u8) -> Option<crate::vehicle::VehicleHandle> {
        use crate::vehicle::{SeatKind, VehicleKind};
        let h = self.players.get(usize::from(slot)).copied().flatten()?;
        let u = self.unit(h)?;
        let m = u.mount?;
        let v = self.vehicle(m.vehicle)?;
        (u.controller == (crate::unit::Controller::Player { slot })
            && matches!(v.kind, VehicleKind::Hover | VehicleKind::Walker)
            && v.seats
                .get(usize::from(m.seat))
                .is_some_and(|s| s.kind == SeatKind::Pilot && s.occupant == Some(h)))
        .then_some(m.vehicle)
    }

    /// `Sim::step` phase 0b: take at most one order EDGE per player slot, in slot order, then let every unit whose
    /// think is due this tick take the order it was given.
    ///
    /// A slot with no pad idles (the P9 rule — a missing pad is not a held
    /// button), and a dead player issues nothing: its soldier does not step, so
    /// its `prev_buttons` is stale, and a bit held across a respawn issues once
    /// on the first tick after it (documented, not fixed — the respawned
    /// soldier starts at `prev_buttons` 0). A ground pilot's Right issues
    /// PILE IN and Left EVERYONE OUT (RE §4.4).
    pub(crate) fn squad_pass(&mut self, pads: &[crate::sim::PadState]) {
        for slot in 0..self.players.len() {
            let Some(h) = self.players[slot] else {
                continue;
            };
            let Some(u) = self.unit(h) else { continue };
            let Some(pad) = pads.get(slot) else { continue };
            let edge = pad.buttons & !u.soldier.prev_buttons();
            let order = if u.mount.is_some() {
                if self.ground_pilot_vehicle(slot as u8).is_none() {
                    continue;
                }
                if edge & crate::sim::buttons::SQUAD_AT_EASE != 0 {
                    Order::GetOut
                } else if edge & crate::sim::buttons::SQUAD_FOLLOW != 0 {
                    Order::PileIn
                } else {
                    continue;
                }
            } else {
                let Some(&(_, order)) = EDGE_ORDER.iter().find(|(bit, _)| edge & bit != 0) else {
                    continue;
                };
                order
            };
            let slot = slot as u8;
            let units = self.give_order(slot, order);
            self.nav_stats.commands += 1;
            self.events.push(crate::sim::Event::Command {
                slot,
                kind: order as u8,
                units,
            });
        }
        for i in 0..self.units.len() {
            let Some(p) = self.units[i].order else {
                continue;
            };
            let u = &self.units[i];
            if u.alive && self.tick < u.ai.next_think {
                continue;
            }
            self.units[i].order = None;
            if self.units[i].alive {
                self.take_order(i, p);
            }
        }
    }

    /// The alive AI soldiers on foot of player `slot`'s team within `reach`
    /// of him, 3-D, in index order — whatever order they already carry.
    fn in_reach(&self, slot: u8, reach: f32) -> Vec<usize> {
        let Some(issuer) = self
            .players
            .get(usize::from(slot))
            .copied()
            .flatten()
            .and_then(|h| self.unit(h))
        else {
            return Vec::new();
        };
        let (from, team) = (issuer.soldier.pos, issuer.team);
        let ai = crate::unit::Controller::Ai;
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                u.alive
                    && u.team == team
                    && u.controller == ai
                    && u.mount.is_none()
                    && (u.soldier.pos - from).length() <= reach
            })
            .collect()
    }

    /// WPOKE's narrow mounted receiver; ordinary in_reach remains on foot.
    /// Both games (BF1's rule on BF2 too, a named deviation, issue #7).
    fn pickup_receiver(&self, i: usize) -> Option<(crate::vehicle::VehicleHandle, u64)> {
        let u = &self.units[i];
        if !u.alive || u.controller != crate::unit::Controller::Ai {
            return None;
        }
        let m = u.mount?;
        let v = self.vehicle(m.vehicle)?;
        let w = u.ai.ride?.pickup?;
        (m.seat == 0
            && v.seats.first()?.occupant == Some(self.handle(i))
            && matches!(
                v.kind,
                crate::vehicle::VehicleKind::Hover | crate::vehicle::VehicleKind::Walker
            )
            && w.vehicle == m.vehicle
            && !w.complete)
            .then_some((m.vehicle, w.seated))
    }

    /// The press: every unit in the order's reach has it pending until its
    /// next think. Answers how many.
    pub fn give_order(&mut self, slot: u8, order: Order) -> u8 {
        let Some(leader) = self.players.get(usize::from(slot)).copied().flatten() else {
            return 0;
        };
        let vehicle = if matches!(order, Order::PileIn | Order::GetOut) {
            let Some(v) = self.ground_pilot_vehicle(slot) else {
                return 0;
            };
            Some(v)
        } else {
            None
        };
        let mut near = if order == Order::GetOut {
            // Bind to these exact AI occupants; no distance or free-gun test.
            self.vehicle(vehicle.unwrap())
                .unwrap()
                .seats
                .iter()
                .filter_map(|seat| seat.occupant)
                .filter(|&h| {
                    self.unit(h)
                        .is_some_and(|u| u.controller == crate::unit::Controller::Ai)
                })
                .map(|h| h.index as usize)
                .collect()
        } else {
            self.in_reach(slot, order.reach())
        };
        if matches!(order, Order::MoveOut | Order::AtEase) {
            if let Some(issuer) = self.unit(leader).filter(|u| {
                u.mount.is_none() && u.controller == (crate::unit::Controller::Player { slot })
            }) {
                for (i, u) in self.units.iter().enumerate() {
                    if u.team == issuer.team
                        && self.pickup_receiver(i).is_some()
                        && u.soldier.pos.distance(issuer.soldier.pos) <= 3.0 * RELEASE_REACH
                    {
                        near.push(i);
                    }
                }
            }
        }
        for &i in &near {
            self.units[i].order = Some(Pending {
                order,
                issuer: slot,
                leader,
                pickup: self.pickup_receiver(i),
                vehicle,
            });
        }
        near.len().min(255) as u8
    }

    /// Unit `i` thinks about the order it was given (RE §3-4): it answers,
    /// then the order's own rule. True when the unit now follows or holds by
    /// this order.
    pub(crate) fn take_order(&mut self, i: usize, p: Pending) -> bool {
        if p.order == Order::GetOut {
            let u = &self.units[i];
            // Unseating cancels pending delivery for both issuer and receiver,
            // including exit/reentry on the same tick. Handles bind generations.
            if u.controller != crate::unit::Controller::Ai
                || self.players.get(usize::from(p.issuer)).copied().flatten() != Some(p.leader)
                || self.ground_pilot_vehicle(p.issuer) != p.vehicle
                || !u.mount.is_some_and(|m| {
                    Some(m.vehicle) == p.vehicle
                        && self.vehicle(m.vehicle).is_some_and(|v| {
                            v.seats
                                .get(usize::from(m.seat))
                                .is_some_and(|s| s.occupant == Some(self.handle(i)))
                        })
                })
            {
                return false;
            }
        }
        if p.order == Order::PileIn {
            let u = &self.units[i];
            if !u.alive
                || u.controller != crate::unit::Controller::Ai
                || u.mount.is_some()
                || self.players.get(usize::from(p.issuer)).copied().flatten() != Some(p.leader)
                || !self.unit(p.leader).is_some_and(|l| {
                    l.team == u.team
                        && l.controller == (crate::unit::Controller::Player { slot: p.issuer })
                })
            {
                return false;
            }
        }
        if let Some(target) = p.pickup.filter(|_| p.order != Order::GetOut) {
            let valid_issuer = self.players.get(usize::from(p.issuer)).copied().flatten()
                == Some(p.leader)
                && self.unit(p.leader).is_some_and(|u| {
                    u.mount.is_none()
                        && u.team == self.units[i].team
                        && u.controller == (crate::unit::Controller::Player { slot: p.issuer })
                });
            if !valid_issuer || self.pickup_receiver(i) != Some(target) {
                return false;
            }
        }
        let has = self.units[i].command;
        // Even a redundant Follow supersedes a previous explicit board goal.
        if p.order != Order::PileIn
            && self.units[i]
                .ai
                .ride
                .is_some_and(|r| r.player_board.is_some())
        {
            self.units[i].ai.cmd_clear();
        }
        let follows = matches!(has.map(|c| c.kind), Some(CommandKind::Follow(_)));
        // The one veto: a follower refuses another FOLLOW ME, with no answer.
        if p.order == Order::Follow && follows {
            return false;
        }
        let (did, carries) = self.obey(i, p, has);
        let unit = self.handle(i);
        self.events.push(crate::sim::Event::OrderTaken {
            unit,
            slot: p.issuer,
            did,
        });
        carries
    }

    /// The order's own rule on unit `i`, which carried `has`: (it changed
    /// what the unit does, the unit now follows or holds by it).
    fn obey(&mut self, i: usize, p: Pending, has: Option<Command>) -> (bool, bool) {
        match p.order {
            Order::GetOut => {
                // Replace this player's old field/boarding business before the
                // same tick's follower shortcut and AI write-back can revive it.
                if has.is_some_and(|c| c.issuer == p.issuer) {
                    self.units[i].command = None;
                }
                let ai = &mut self.units[i].ai;
                ai.pending_board = None;
                if has.is_none_or(|c| c.issuer == p.issuer) {
                    ai.cmd_clear();
                } else {
                    // Leaving ends the vehicle route, not another player's
                    // retained field order or its accumulated release clock.
                    route::clear(ai);
                }
                let r = ai.ride.get_or_insert_default();
                r.board = None;
                r.use_req = false;
                r.own_search_until = self.tick + 600;
                let me = self.handle(i);
                self.exit_vehicle(me);
                self.nav_stats.ai_dismounts += 1;
                (true, false)
            }
            Order::PileIn => {
                let Some(vh) = p.vehicle else {
                    return (false, false);
                };
                // Replace older boarding business, retaining the field order and
                // its scratch behind this explicit goal. No post/AI-call gates.
                self.units[i].ai.pending_board = None;
                let me = self.handle(i);
                let mut r = self.units[i].ai.ride.unwrap_or_default();
                r.board = Some(vh);
                r.called_board = Some(me);
                r.player_board = Some(BoardIssuer {
                    slot: p.issuer,
                    leader: p.leader,
                });
                r.ordered_call = false;
                r.since = self.tick;
                r.maintenance = None;
                r.use_req = false;
                r.call_blocked = false;
                let admitted = crate::ai::claimed_seat(self, me, vh, r).is_some();
                if !admitted {
                    r.board = None;
                    r.called_board = None;
                    r.player_board = None;
                }
                self.units[i].ai.ride = Some(r);
                route::clear(&mut self.units[i].ai);
                (admitted, false)
            }
            Order::MoveOut | Order::AtEase => {
                let released = has.is_some_and(|c| c.issuer == p.issuer);
                if released {
                    self.units[i].ai.pending_board = None;
                    self.units[i].command = None;
                    self.units[i].ai.cmd_clear();
                }
                let poked = p.pickup.is_some();
                if poked {
                    let w = self.units[i]
                        .ai
                        .ride
                        .as_mut()
                        .unwrap()
                        .pickup
                        .as_mut()
                        .unwrap();
                    // Store the release time rather than subtracting on an unsigned clock.
                    w.released = Some(w.released.map_or(self.tick, |t| t.min(self.tick)));
                }
                (released || poked, false)
            }
            Order::Hold => {
                let at = self.units[i].soldier.pos;
                self.put_command(i, CommandKind::Hold(at), p.issuer);
                (true, true)
            }
            Order::Follow => {
                let kind = CommandKind::Follow(p.leader);
                let followers = self
                    .units
                    .iter()
                    .filter(|u| u.alive && u.command.is_some_and(|c| c.kind == kind))
                    .count();
                if self.unit(p.leader).is_none() || followers >= FOLLOW_CAP {
                    return (false, false);
                }
                self.put_command(i, kind, p.issuer);
                (true, true)
            }
        }
    }

    fn put_command(&mut self, i: usize, kind: CommandKind, issuer: u8) {
        self.units[i].order = None;
        self.units[i].ai.pending_board = None;
        self.units[i].command = Some(Command {
            kind,
            issuer,
            issued_tick: self.tick,
        });
        // It leaves the plan NOW, not at the next one (brief D3/D5), and
        // the follower state starts fresh.
        let team = usize::from(self.units[i].team).min(1);
        if let Some(a) = self.commander[team].assignment.get_mut(i) {
            *a = None;
        }
        self.units[i].ai.cmd_clear();
    }

    /// An order put on the bots around player `slot` AT ONCE, without the wait
    /// for each unit's think (tests, dev tools): every unit in [`ORDER_REACH`]
    /// in index order, under the order's own rule — so a `Follow` stops at
    /// [`FOLLOW_CAP`]. `Hold`'s spot is filled in per bot. Answers how many
    /// carry the order afterwards.
    pub fn issue_command(&mut self, slot: u8, kind: CommandKind) -> u8 {
        let Some(leader) = self.players.get(usize::from(slot)).copied().flatten() else {
            return 0;
        };
        let mut n = 0u8;
        for i in self.in_reach(slot, ORDER_REACH) {
            let order = match kind {
                CommandKind::Follow(_) => Order::Follow,
                CommandKind::Hold(_) => Order::Hold,
            };
            let p = Pending {
                order,
                issuer: slot,
                leader,
                pickup: None,
                vehicle: None,
            };
            n += u8::from(self.take_order(i, p));
        }
        n
    }

    /// Every order player `slot` gave is off — at any distance, in any state
    /// (his death). The bots go back to the commander's pool and take an
    /// objective at the next plan (≤ `PLAN_PERIOD` ticks); until then
    /// `retarget_phase_a` steers them, exactly as it does a unit that spawned
    /// between plans.
    pub fn clear_all(&mut self, slot: u8) -> u8 {
        let mut n = 0u8;
        for i in 0..self.units.len() {
            if self.units[i].order.is_some_and(|p| p.issuer == slot) {
                self.units[i].order = None;
            }
            if self.units[i]
                .ai
                .ride
                .is_some_and(|r| r.player_board.is_some_and(|issuer| issuer.slot == slot))
            {
                self.units[i].ai.cmd_clear();
            }
            if self.units[i].command.is_some_and(|c| c.issuer == slot) {
                self.units[i].ai.pending_board = None;
                self.units[i].command = None;
                self.units[i].ai.cmd_clear();
                n = n.saturating_add(1);
            }
        }
        n
    }
}

/// SQVEH: the vehicle a follower's living leader currently occupies (RE §4.3).
/// Kept separate from autonomous seat search: following an order does not
/// consult the bot's own-search cooldown. Only unmounted followers may board.
pub(crate) fn follow_vehicle(
    sim: &crate::sim::Sim,
    u: &crate::unit::Unit,
) -> Option<crate::vehicle::VehicleHandle> {
    if u.mount.is_some()
        || u.order
            .is_some_and(|p| matches!(p.order, Order::PileIn | Order::GetOut))
    {
        return None;
    }
    let CommandKind::Follow(leader) = u.command?.kind else {
        return None;
    };
    Some(sim.unit(leader)?.mount?.vehicle)
}

/// [`crate::ai::CmdState::roll`] of a follower: where it is in the PS2's
/// follow agent (RE §4.3 states 2, 3, 4). `u64::MAX` is an order not thought
/// about yet.
const APPROACH: u64 = 2;
const FOLLOW: u64 = 3;
const STAND: u64 = 4;

/// `ai::think`'s COMMAND BRANCH: where a Follow or Hold bot walks this think,
/// and whether its order has ended.
///
/// It takes `&Sim` like every other think path — nothing here mutates the sim.
/// A unit whose order is over cannot clear it from inside `think`, so the fact
/// is reported through [`crate::ai::NavTick::released`] and `Sim::step`'s AI
/// write-back does the clearing.
///
/// A commanded unit is NOT the commander's: it has left the plan and it takes
/// no post, so `target_cp` goes. The fighting is not touched: the aim and the
/// bursts run unchanged on a unit whose walking goal is its leader or its
/// spot.
pub fn steer(
    sim: &crate::sim::Sim,
    me: UnitHandle,
    st: &mut crate::ai::AiState,
    cmd: &Command,
    nt: &mut crate::ai::NavTick,
) {
    let Some(u) = sim.unit(me) else { return };
    let pos = u.soldier.pos;
    st.target_cp = None;
    let mut c = st.cmd.unwrap_or(crate::ai::CmdState {
        goal: pos,
        far_ticks: 0,
        roll: u64::MAX,
    });
    // The leader: a follower's is the soldier it was told to follow, a
    // holder's the soldier of the player who ordered it.
    let leader = match cmd.kind {
        CommandKind::Follow(l) => Some(l),
        _ => sim.players.get(usize::from(cmd.issuer)).copied().flatten(),
    };
    let Some(l) = leader.and_then(|h| sim.unit(h)) else {
        // The leader is dead or gone: the order ends this think.
        nt.released = 1;
        st.cmd = Some(c);
        return;
    };
    let lp = l.soldier.pos;
    let d = (pos - lp).length();
    // WGORD: retain the ordinary release clock while the explicit board goal
    // runs. Do not advance Follow's approach/stand scratch or its route.
    if st
        .ride
        .is_some_and(|r| r.ordered_call || r.player_board.is_some())
        || st.pending_board.is_some()
    {
        if d > LEAVE_DIST {
            c.far_ticks = c.far_ticks.saturating_add(crate::ai::THINK_PERIOD as u16);
        }
        if c.far_ticks >= LEAVE_TICKS {
            nt.released = 1;
        }
        st.cmd = Some(c);
        return;
    }
    match cmd.kind {
        CommandKind::Follow(_) => {
            // SQVEH, RE §4.3: walk to the leader's vehicle, rather than
            // taking a formation point beside it. Step 1c takes a free seat
            // through CanEnter, including an unarmed passenger seat.
            if let Some(vh) = follow_vehicle(sim, u) {
                // Abandon an older autonomous boarding request, preserving
                // its search cooldown. This order walks to the leader alone.
                if let Some(r) = st.ride.as_mut() {
                    r.board = None;
                    r.called_board = None;
                    r.player_board = None;
                    r.maintenance = None;
                    r.use_req = false;
                }
                if sim.enter_seat(me, vh).is_some() {
                    c.roll = FOLLOW;
                    c.goal = lp;
                }
                // [ours] A full vehicle keeps Follow; continue on foot below.
            }
            if c.roll == u64::MAX {
                c.roll = APPROACH;
                c.goal = lp;
            }
            if c.roll == APPROACH
                && (crate::nav::xz_dist(pos, c.goal) <= STAND_ARRIVE || d <= APPROACH_BODY)
            {
                c.roll = FOLLOW;
            }
            if c.roll == STAND && d > FOLLOW_FAR {
                c.roll = FOLLOW;
            }
            if c.roll == FOLLOW {
                if d < FOLLOW_NEAR && l.mount.is_none() {
                    // The standing point is drawn in the LEADER's frame, once.
                    c.roll = STAND;
                    c.goal = lp + stand_offset(me.index, me.gen, sim.tick, l.soldier.yaw);
                } else {
                    c.goal = lp;
                }
            }
            if crate::nav::xz_dist(pos, c.goal) > ROUTE_DIST {
                // `ai::route::retarget_hunt`'s shape with the goal in place
                // of the nearest enemy — route to its hub and keep the leg
                // while that hub is unchanged. With no graph, no hub or
                // nothing joining the two the unit walks straight at it.
                let m = crate::ai::mover(sim, u);
                let g = sim.nav.as_ref();
                let dest = g.and_then(|g| {
                    g.unit_hub_on(c.goal, m, &|h| crate::ai::hub_on_level(sim, g, h, c.goal.y))
                });
                let cur = g.and_then(|g| crate::ai::start_hub(sim, g, me));
                match (g, cur, dest) {
                    (Some(g), Some(cu), Some(de)) if g.reachable(cu, de, m.ty) => {
                        route::keep_or_plan(sim, st, me, cu, de, nt);
                    }
                    _ => route::clear(st),
                }
                // The last leg ends at the goal itself, not at its hub.
                route::set_last_waypoint(st, c.goal, Refresh::None);
            } else {
                route::clear(st);
            }
        }
        CommandKind::Hold(spot) => {
            // The PS2's hold agent never moves the unit.
            route::clear(st);
            c.goal = spot;
        }
    }
    // The release, counted in THINKS and never refilled.
    if d > LEAVE_DIST {
        c.far_ticks = c
            .far_ticks
            .saturating_add(crate::ai::THINK_PERIOD.try_into().unwrap_or(u16::MAX));
    }
    if c.far_ticks >= LEAVE_TICKS {
        nt.released = 1;
    }
    st.cmd = Some(c);
}

/// Where a follower stands relative to its LEADER (RE §4.3 state 4): a point
/// of the square of +-[`STAND_HALF`] about him in his frame, at least
/// [`STAND_MIN`] from him (a nearer draw is pushed out to it: PROVISIONAL, the
/// PS2's way of keeping the metre was not read).
///
/// The draw is the per-unit hash ([`crate::ai::unit_rand`] with
/// [`crate::ai::SALT_FORMATION`]), never the sim LCG: `think` takes `&Sim` and
/// may not mutate shared state.
pub fn stand_offset(unit_index: u32, gen: u32, tick: u64, leader_yaw: f32) -> Vec3 {
    let r = crate::ai::unit_rand(unit_index, tick, gen, crate::ai::SALT_FORMATION);
    let axis = |w: u32| ((w & 0xFFFF) as f32 / 65536.0 - 0.5) * 2.0 * STAND_HALF;
    let mut v = Vec3::new(axis(r), 0.0, axis(r >> 16));
    let len = v.length();
    if len < STAND_MIN {
        v = if len > 1e-3 {
            v * (STAND_MIN / len)
        } else {
            Vec3::X * STAND_MIN
        };
    }
    crate::ai::rotate_xz(v, leader_yaw)
}

/// Both death paths and the no-overspawn despawn call this (brief D4): the
/// unit's own order goes, and if the unit was a PLAYER, every order that player
/// gave goes with it (the maintainer's rule: "the player's death clears every command
/// they gave").
///
/// A respawn rebuilds the unit from its `UnitSpec`, whose `command` is `None` by
/// construction, so nothing has to be cleared there.
///
/// It clears the ORDER and the follower scratch state, and nothing else. The
/// dead unit's `route`/`target_cp` are
/// deliberately left alone — a dead unit's route is never read and its `AiState`
/// is rebuilt from `default()` on respawn, but both fields ARE hashed, so
/// clearing them here moves the sim hash of every round that has a death in it.
/// MEASURED: doing so moved the hash column of 23 of the 32 sweep maps with
/// every other column, and the footer, byte-identical
/// (`lead_tmp/phase-d/d1b/sweep_1.txt` against `after/sweep_1.txt`) — a hash
/// move for no behaviour, which is exactly what the brief's addendum 3 forbids.
/// `ai.cmd` (D6) IS cleared, because it is state that exists only while the
/// order does and is hashed only when `Some`: no command-free round can see it.
pub fn on_unit_died(sim: &mut crate::sim::Sim, h: UnitHandle) {
    let index = h.index as usize;
    if index >= sim.units.len() {
        return;
    }
    sim.units[index].command = None;
    sim.units[index].order = None;
    sim.units[index].ai.cmd = None;
    if let Some(slot) = sim.units[index].spec.player_slot {
        sim.clear_all(slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightfield::Heightfield;
    use crate::sim::{buttons, Event, PadState, Sim};
    use crate::unit::{Controller, UnitSpec};

    fn flat() -> Heightfield {
        Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }
    }

    /// A sim with a player in slot 0 at the origin and `bots` AI teammates at
    /// the given offsets, in the order given (so the tests can pick indices).
    fn squad_sim(bots: &[Vec3]) -> Sim {
        let mut sim = Sim::empty(9);
        sim.heightfield = Some(flat());
        sim.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .expect("player");
        for p in bots {
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, *p)
            })
            .expect("bot");
        }
        sim
    }

    fn commanded(sim: &Sim) -> Vec<(usize, Option<Command>)> {
        sim.units
            .iter()
            .enumerate()
            .map(|(i, u)| (i, u.command))
            .filter(|(_, c)| c.is_some())
            .collect()
    }

    /// D4: a follower's death clears its own order; the ISSUER's death clears
    /// every order that slot gave; a respawn comes back with none.
    #[test]
    fn a_death_clears_the_right_orders() {
        let mut sim = squad_sim(&[Vec3::new(3.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0)]);
        sim.spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(5.0, 0.0, 0.0)))
            .expect("player 2");
        let (l0, l1) = (sim.players[0].expect("p0"), sim.players[1].expect("p1"));
        sim.issue_command(0, CommandKind::Follow(l0));
        sim.units[2].command = Some(Command {
            kind: CommandKind::Follow(l1),
            issuer: 1,
            issued_tick: 0,
        });
        // One follower dies: its own order goes, the other keeps its.
        let h1 = sim.handle(1);
        sim.kill_environment(h1, crate::sim::DeathCause::Fall);
        assert_eq!(sim.units[1].command, None);
        assert!(sim.units[2].command.is_some());
        // The ISSUER dies: every order slot 1 gave goes with it, and slot 0's
        // (there are none left) are untouched.
        sim.issue_command(0, CommandKind::Follow(l0));
        let alive_of_slot0: Vec<usize> = commanded(&sim)
            .iter()
            .filter(|(_, c)| c.expect("order").issuer == 0)
            .map(|(i, _)| *i)
            .collect();
        sim.kill_environment(l1, crate::sim::DeathCause::DeathRegion);
        assert!(
            commanded(&sim)
                .iter()
                .all(|(_, c)| c.expect("order").issuer == 0),
            "the dead player's orders are all off"
        );
        for i in alive_of_slot0 {
            assert!(sim.units[i].command.is_some(), "unit {i} kept slot 0's");
        }
        // A respawn brings the unit back with no order at all — it is rebuilt
        // from its `UnitSpec`, whose `command` is `None` by construction, which
        // is why `on_unit_died` does not have to clear it twice.
        let spec = sim.units[1].spec.clone();
        assert_eq!(spec.build(9, &sim.weapon_classes).command, None);
        // (The respawn PASS itself needs a mission's spawn waves, which this
        // bare `Sim::empty` fixture has none of — `sim/spawn/tests.rs`'s respawn tests own
        // that path; what Phase D adds to it is the line above.)
        assert_eq!(sim.units[1].command, None, "and it stays off while dead");
    }

    /// The tag and the payload both reach the hash, and the three kinds are
    /// distinct — including the two that carry a `u64`-worth of payload in
    /// different fields.
    #[test]
    fn a_commands_bytes_carry_the_kind_and_its_payload() {
        let h = |index, gen| UnitHandle { index, gen };
        let follow = CommandKind::Follow(h(3, 1));
        assert_ne!(follow.bytes(), CommandKind::Follow(h(4, 1)).bytes());
        assert_ne!(follow.bytes(), CommandKind::Follow(h(3, 2)).bytes());
        assert_ne!(
            CommandKind::Hold(Vec3::new(1.0, 0.0, 2.0)).bytes(),
            CommandKind::Hold(Vec3::new(1.0, 0.0, 2.5)).bytes()
        );
        // Hold's y is part of it: two anchors on top of each other are not the
        // same anchor (POL1's stacked hubs are 30 m apart in y alone).
        assert_ne!(
            CommandKind::Hold(Vec3::new(1.0, 0.0, 2.0)).bytes(),
            CommandKind::Hold(Vec3::new(1.0, 30.0, 2.0)).bytes()
        );
        // ... and no two KINDS collide, whatever they carry: the tag is first
        // and the width is fixed.
        let kinds = [CommandKind::Follow(h(0, 0)), CommandKind::Hold(Vec3::ZERO)];
        for i in 0..kinds.len() {
            for j in i + 1..kinds.len() {
                assert_ne!(kinds[i].bytes(), kinds[j].bytes(), "{i} vs {j}");
            }
        }
    }

    /// The labels the diagnostics print.
    #[test]
    fn the_labels_say_what_the_order_is() {
        let h = UnitHandle { index: 7, gen: 1 };
        assert_eq!(CommandKind::Follow(h).label(), "follow #7");
        assert_eq!(
            CommandKind::Hold(Vec3::new(-12.4, 3.0, 8.6)).label(),
            "hold (-12,9)"
        );
    }

    /// D6/D9, and the phase's own acceptance ("`--sim-hash` with a scripted
    /// command log byte-identical run to run"): a SCRIPTED order log — follow,
    /// then hold, then clear — steps to the same sim hash twice, and to a
    /// DIFFERENT one from the same log with the command bits stripped out.
    ///
    /// The second half is what makes it a test rather than a tautology: the
    /// followers really do walk somewhere else, so the equality above is the
    /// replay guarantee and not "commands do nothing".
    #[test]
    fn a_scripted_command_log_replays_to_the_same_hash() {
        let script = |commanded: bool| {
            let mut sim = squad_sim(&[
                Vec3::new(3.0, 0.0, 1.0),
                Vec3::new(-4.0, 0.0, 2.0),
                Vec3::new(6.0, 0.0, -3.0),
            ]);
            let bit = |b: u32| {
                if commanded {
                    b
                } else {
                    0
                }
            };
            for t in 0..600u64 {
                let buttons = match t {
                    10 => bit(buttons::SQUAD_FOLLOW),
                    200 => bit(buttons::SQUAD_HOLD),
                    400 => bit(buttons::SQUAD_MOVE_OUT),
                    // The leader walks the whole time, so the followers have
                    // somewhere to be and the formation slots keep moving.
                    _ => 0,
                };
                sim.step(&[PadState {
                    buttons,
                    ly: 90,
                    ..PadState::default()
                }]);
            }
            // The bots' end points (which two of the three follow is the
            // order they think in: `FOLLOW_CAP`).
            let ends: Vec<Vec3> = sim.units[1..].iter().map(|u| u.soldier.pos).collect();
            (sim.hash(), sim.nav_stats.commands, ends)
        };
        let (a, cmds, pos) = script(true);
        let (b, _, _) = script(true);
        assert_eq!(a, b, "the same order log replays to the same hash");
        assert_eq!(cmds, 3, "three orders were issued");
        let (bare, bare_cmds, bare_pos) = script(false);
        assert_eq!(bare_cmds, 0);
        assert_ne!(a, bare, "and the orders MOVED the sim");
        assert!(
            pos.iter()
                .zip(&bare_pos)
                .any(|(a, b)| (*a - *b).length() > 1.0),
            "a follower ends somewhere else: {pos:?} vs {bare_pos:?}"
        );
    }

    /// A unit with an order eats a 1 tag first, and a unit without one eats
    /// NOTHING (`Unit::hash_into`) — that pair is what keeps every replay hash
    /// where it was (brief addendum 3: the hash format does not move for a sim
    /// with no commands in it).
    #[test]
    fn the_hash_bytes_of_an_order_start_with_a_one() {
        let mut got: Vec<u8> = Vec::new();
        Command {
            kind: CommandKind::Hold(Vec3::ZERO),
            issuer: 1,
            issued_tick: 30,
        }
        .hash_into(&mut |b| got.extend_from_slice(b));
        assert_eq!(got.len(), 1 + 13 + 1 + 8);
        assert_eq!(got[0], 1, "the presence tag is 1, not 0");
        assert_eq!(got[14], 1, "the issuer slot");
        assert_eq!(got[15..], 30u64.to_le_bytes(), "the issue tick");
    }

    fn takes(sim: &Sim) -> Vec<u32> {
        sim.events
            .iter()
            .filter_map(|e| match e {
                Event::OrderTaken { unit, .. } => Some(unit.index),
                _ => None,
            })
            .collect()
    }

    fn press(buttons: u32) -> PadState {
        PadState {
            buttons,
            ..PadState::default()
        }
    }

    /// RE §3: the reach is 3-D — 30 m for HOLD and FOLLOW, 45 m for the two
    /// release orders — and has no cap; enemies and players are not in it.
    #[test]
    fn the_reach_is_3d_30_m_and_45_m_for_the_release_orders() {
        let mut sim = squad_sim(&[
            Vec3::new(29.0, 0.0, 0.0),
            Vec3::new(31.0, 0.0, 0.0),
            Vec3::new(15.0, 20.0, 0.0),
            Vec3::new(15.0, 27.0, 0.0),
            Vec3::new(44.0, 0.0, 0.0),
            Vec3::new(46.0, 0.0, 0.0),
        ]);
        // (A spawn stands on the ground: the two up in the air are put there.)
        sim.units[3].soldier.pos = Vec3::new(15.0, 20.0, 0.0);
        sim.units[4].soldier.pos = Vec3::new(15.0, 27.0, 0.0);
        for k in 1..=8 {
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, k as f32))
            })
            .expect("bot");
        }
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(1, Vec3::new(2.0, 0.0, 0.0))
        })
        .expect("enemy");
        assert_eq!(sim.give_order(0, Order::Hold), 2 + 8);
        assert_eq!(sim.give_order(0, Order::Follow), 2 + 8);
        let pending = |sim: &Sim| -> Vec<usize> {
            (0..sim.units.len())
                .filter(|&i| sim.units[i].order.is_some())
                .collect()
        };
        assert_eq!(pending(&sim), vec![1, 3, 7, 8, 9, 10, 11, 12, 13, 14]);
        assert_eq!(sim.give_order(0, Order::MoveOut), 5 + 8);
        assert_eq!(sim.give_order(0, Order::AtEase), 5 + 8);
        assert!(sim.units[6].order.is_none(), "46 m is out of every reach");
        assert!(sim.units[15].order.is_none(), "an enemy takes no order");
    }

    /// RE §3: an order is taken at the unit's own next think — not at the
    /// press — and every unit that takes it answers once.
    #[test]
    fn each_unit_takes_the_order_at_its_next_think_and_answers() {
        let mut sim = squad_sim(&[
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(-5.0, 0.0, 0.0),
        ]);
        sim.step(&[PadState::default()]);
        for (i, wait) in [(1usize, 7u64), (2, 1), (3, 4)] {
            sim.units[i].ai.next_think = sim.tick + wait;
        }
        let pressed = sim.tick;
        let mut at = [None; 4];
        for t in 0..12 {
            sim.step(&[press(if t == 0 { buttons::SQUAD_HOLD } else { 0 })]);
            for i in takes(&sim) {
                assert!(at[i as usize].is_none(), "unit {i} answered twice");
                at[i as usize] = Some(sim.tick - 1 - pressed);
            }
        }
        assert_eq!(at, [None, Some(7), Some(1), Some(4)]);
        for i in 1..4 {
            let c = sim.units[i].command.expect("holds");
            assert!(matches!(c.kind, CommandKind::Hold(_)));
            assert_eq!(sim.units[i].order, None, "the order is consumed");
        }
    }

    /// RE §4.3: FOLLOW ME is taken by the first two receivers to think; the
    /// rest answer and carry on; a follower refuses a second FOLLOW ME with
    /// no answer.
    #[test]
    fn follow_me_is_taken_by_the_first_two_to_think() {
        let ring: Vec<Vec3> = (0..5)
            .map(|k| crate::ai::rotate_xz(Vec3::X * 5.0, k as f32 * 1.2566))
            .collect();
        let mut sim = squad_sim(&ring);
        sim.step(&[PadState::default()]);
        for (i, wait) in [(1usize, 6u64), (2, 2), (3, 8), (4, 1), (5, 5)] {
            sim.units[i].ai.next_think = sim.tick + wait;
        }
        let mut answers = 0;
        for t in 0..12 {
            sim.step(&[press(if t == 0 { buttons::SQUAD_FOLLOW } else { 0 })]);
            answers += takes(&sim).len();
        }
        assert_eq!(answers, 5, "all five answer");
        let followers: Vec<usize> = commanded(&sim).iter().map(|(i, _)| *i).collect();
        assert_eq!(followers, vec![2, 4], "the first two to think");
        let mut again = Vec::new();
        for t in 0..12 {
            sim.step(&[press(if t == 0 { buttons::SQUAD_FOLLOW } else { 0 })]);
            again.extend(takes(&sim));
        }
        again.sort_unstable();
        assert_eq!(again, vec![1, 3, 5], "a follower does not answer again");
        assert_eq!(commanded(&sim).len(), FOLLOW_CAP);
    }

    /// RE §4.1: MOVE OUT and AT EASE are one rule — every receiver answers,
    /// and a follower or holder ordered by THIS player is released; a HOLD
    /// replaces a follow.
    #[test]
    fn move_out_and_at_ease_release_what_this_player_ordered() {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            let mut sim = squad_sim(&[
                Vec3::new(3.0, 0.0, 0.0),
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(40.0, 0.0, 0.0),
            ]);
            let l0 = sim.players[0].expect("p0");
            assert_eq!(sim.issue_command(0, CommandKind::Follow(l0)), 2);
            assert_eq!(sim.issue_command(0, CommandKind::Hold(Vec3::ZERO)), 2);
            assert!(matches!(
                sim.units[1].command.expect("order").kind,
                CommandKind::Hold(_)
            ));
            // Unit 2's order is another player's.
            sim.units[2].command = Some(Command {
                kind: CommandKind::Follow(l0),
                issuer: 1,
                issued_tick: 0,
            });
            let mut answers = 0;
            for t in 0..12 {
                sim.step(&[press(if t == 0 { bit } else { 0 })]);
                answers += takes(&sim).len();
            }
            assert_eq!(answers, 3, "everyone within 45 m answers");
            assert_eq!(sim.units[1].command, None, "released");
            assert_eq!(sim.units[2].command.expect("kept").issuer, 1);
        }
    }

    /// The pad EDGE issues exactly once, the priority order is fixed, and the
    /// press is an event whoever is in reach.
    #[test]
    fn the_pad_edge_issues_once_and_the_priority_is_fixed() {
        let mut sim = squad_sim(&[Vec3::new(3.0, 0.0, 0.0)]);
        let held = press(buttons::SQUAD_FOLLOW);
        let mut issued = 0;
        for _ in 0..12 {
            sim.step(&[held]);
            issued += sim
                .events
                .iter()
                .filter(|e| matches!(e, Event::Command { .. }))
                .count();
        }
        assert_eq!(issued, 1, "a held button is one edge");
        assert_eq!(sim.nav_stats.commands, 1);
        assert!(matches!(
            sim.units[1].command.expect("order").kind,
            CommandKind::Follow(_)
        ));
        // Two bits on one tick: MOVE OUT wins over HOLD.
        sim.step(&[PadState::default()]);
        sim.step(&[press(buttons::SQUAD_MOVE_OUT | buttons::SQUAD_HOLD)]);
        let kinds: Vec<u8> = sim
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Command { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, vec![3], "one order per slot per tick");
        // Nobody in reach is still an event: the player's line plays.
        sim.units[1].soldier.pos = Vec3::new(500.0, 0.0, 0.0);
        sim.step(&[PadState::default()]);
        sim.step(&[press(buttons::SQUAD_HOLD)]);
        assert!(sim.events.iter().any(|e| matches!(
            e,
            Event::Command {
                slot: 0,
                kind: 1,
                units: 0
            }
        )));
    }

    /// RE §4.3 state 4: the standing point is inside the +-3 m square of the
    /// leader's frame, at least 1 m from him, and turns with him.
    #[test]
    fn the_standing_point_is_in_the_leaders_square_and_turns_with_him() {
        let mut far = false;
        for unit in 0..64u32 {
            for tick in 0..64u64 {
                let v = stand_offset(unit, 1, tick, 0.0);
                assert_eq!(v.y, 0.0);
                assert!(v.x.abs() <= STAND_HALF + 1e-4 && v.z.abs() <= STAND_HALF + 1e-4);
                assert!(v.length() >= STAND_MIN - 1e-4, "{v}");
                far |= v.length() > STAND_HALF;
            }
        }
        assert!(far, "the corners of the square are reached");
        let a = stand_offset(3, 1, 7, 0.0);
        let b = stand_offset(3, 1, 7, 1.0);
        assert!((b - crate::ai::rotate_xz(a, 1.0)).length() < 1e-5);
    }
}
