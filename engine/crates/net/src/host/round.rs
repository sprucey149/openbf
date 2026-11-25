//! The host's round flow: the load barrier, the 30 Hz tick clock, closing
//! bundles, the round log and catch-up streams, and hash checks.
//!
//! Rules:
//! - `load_round` takes the host game's seat list with the setup: the
//!   seated slots that are connected (and the host's own) are the round's
//!   humans from tick 0, and each seated guest gets `LoadRound` (roster = the
//!   connected players, host included). A seated slot with no connection
//!   (gone before the round loaded) gets a `RemoveHuman` in tick 0's bundle,
//!   so every peer hands it to a bot. A connected guest the seat list missed
//!   (it joined while the round was being set up) waits out the load and
//!   joins as a mid-round joiner when the round starts. Each seated guest
//!   answers `Loaded`; the host game sees `PeerLoaded` and decides what a
//!   digest mismatch means. `start_round` (or [`crate::BARRIER_TIMEOUT`])
//!   sends `Start` and starts the clock: tick `T` closes at
//!   `start + (T + 1) / 30 s`.
//! - The start is one moment on every peer: `start` is the lead after
//!   `start_round`, the slowest guest given `Start`'s half round trip plus
//!   [`crate::START_MARGIN`] (at most [`crate::START_LEAD_MAX`]; none
//!   without a guest). `Start` and `RoundStarted` carry it; each guest
//!   takes off its own half round trip, so every loading screen ends when
//!   tick 0 begins.
//! - Closing tick `T`: each active slot's queue gives its pad, then pending
//!   session commands go in, in order, while the bundle stays within
//!   [`crate::packet::BUNDLE_BUDGET`] (each pad counted against both bases
//!   a stream may code it with, an added human at the largest pad); the
//!   rest wait a tick. Their slot changes apply (an added slot closes its
//!   fresh queue). Every closed bundle is logged and handed to the host game
//!   at once.
//! - `AddHuman` makes a slot active from the bundle that carries it;
//!   `RemoveHuman` drops it from that bundle on. A leave cancels an
//!   `AddHuman` not yet closed instead of sending both.
//! - A mid-round joiner, a reconnect and a `Resync` all get a new stream:
//!   `LoadRound` (joining), the log so far as zstd chunks of
//!   [`crate::msg::LOG_CHUNK_TICKS`], then live bundles from the log's end.
//!   Full chunks are compressed once and cached.
//! - Hash reports compare a guest's hash with the host game's for the same
//!   tick, whichever arrives first; a mismatch emits `Desync` to the host
//!   game, which decides whose game parted. Only [`HostCore::tell_desync`]
//!   tells the guest (with the host's hash, build and target). A report of
//!   another round is stale and dropped. Old reports are trimmed so the maps
//!   stay bounded. The guest's `DesyncHashes` answer is handed to the host
//!   game as it comes.
//! - A check tick on which every guest live in the round (connected, in its
//!   bundles, not catching up; at least one) reported the host game's own
//!   hash raises `HashesAgreed` once, newest first: everyone holds that
//!   tick's game, so the host game may replay from it (issue #168). A tick
//!   with any differing hash never does.
//! - `load_round` drops the old round's bundles and hash events the host
//!   game has not read yet, so none is stepped or compared in the new one.
//! - Fell behind (`LAN_MULTIPLAYER.md` §8b): a connected guest in the round
//!   that is not catching up is known to have stepped the newest tick it
//!   reported a hash for (or the tick it became live). When the clock is
//!   more than [`HostConfig::fell_behind_ticks`] plus one hash interval
//!   past it, the guest is kicked with [`FELL_BEHIND_TEXT`] and a bot takes
//!   its place. A link "having trouble" is left to the silence timeout; when
//!   it is heard again it gets half the limit to report.
//!
//! [`HostConfig::fell_behind_ticks`]: super::HostConfig::fell_behind_ticks
//! [`FELL_BEHIND_TEXT`]: super::FELL_BEHIND_TEXT

use super::{HostCore, NetEvent, Phase, Player};
use crate::bundle::{
    compress_log, pad_wire_len, SessionCommand, TickBundle, MAX_COMMANDS, MAX_COMMAND_BYTES,
    MAX_PAD_BYTES,
};
use crate::msg::{Msg, RosterEntry, LOG_CHUNK_TICKS};
use crate::packet::BUNDLE_BUDGET;
use crate::{Micros, BARRIER_TIMEOUT, START_LEAD_MAX, START_MARGIN};
use core::sim::PadState;
use std::collections::{BTreeMap, VecDeque};

const HOST_HASHES_KEPT: usize = 2000;
/// Ticks between a guest's hash reports (the game reports every 30).
const REPORT_SLACK: u32 = 30;
const PEER_HASHES_KEPT: usize = 8192;
/// Check ticks whose agreement is still open (a few seconds of reports).
const AGREEMENT_KEPT: usize = 64;

/// Where the host is in a round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoundPhase {
    #[default]
    Idle,
    Loading {
        deadline: Micros,
    },
    Running {
        start: Micros,
    },
    /// `EndRound` closed; no more ticks. The log stays until the next round.
    Ended,
}

#[derive(Default)]
pub(crate) struct RoundState {
    pub phase: RoundPhase,
    pub id: u32,
    setup: Vec<u8>,
    roster: Vec<RosterEntry>,
    pub log: Vec<TickBundle>,
    log_cache: Vec<Vec<u8>>,
    pending: VecDeque<SessionCommand>,
    next_stream: u32,
    /// Connected guests this round's seats missed: they join once it starts.
    pub late: Vec<u8>,
    /// The unseated slots of machines seated in part: each gets a
    /// `PeerCaughtUp` once its machine is live in the running round, so the
    /// game adds it as it adds a late joiner (module rules).
    pub late_locals: Vec<u8>,
    /// Those of `late_locals` whose pad came back: released as
    /// `LocalBack` (a return), not `PeerCaughtUp` (a late joiner).
    pub returned: Vec<u8>,
    all_loaded_sent: bool,
    host_hashes: BTreeMap<u32, u64>,
    peer_hashes: BTreeMap<(u32, u8), u64>,
    /// Per check tick still open: the guests whose hash equalled the host's,
    /// and whether any differed (module rules, `HashesAgreed`).
    agreement: BTreeMap<u32, (Vec<u8>, bool)>,
    /// The newest check tick `HashesAgreed` was emitted for.
    agreed_through: Option<u32>,
}

impl RoundState {
    pub fn accepts_inputs(&self) -> bool {
        matches!(self.phase, RoundPhase::Running { .. })
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.phase, RoundPhase::Loading { .. })
    }

    pub fn is_running(&self) -> bool {
        matches!(self.phase, RoundPhase::Running { .. })
    }

    /// Whether a peer on a stream of `round_id` gets bundles.
    pub fn streams_to(&self, round_id: u32) -> bool {
        round_id == self.id && matches!(self.phase, RoundPhase::Running { .. } | RoundPhase::Ended)
    }

    pub fn next_tick(&self) -> u32 {
        self.log.len() as u32
    }

    /// Schedules a human's removal (or cancels its pending `AddHuman`).
    pub fn remove_human(&mut self, slot: u8, active: bool) {
        if !matches!(
            self.phase,
            RoundPhase::Loading { .. } | RoundPhase::Running { .. }
        ) {
            return;
        }
        let before = self.pending.len();
        self.pending
            .retain(|c| !matches!(c, SessionCommand::AddHuman { slot: s, .. } if *s == slot));
        let cancelled = self.pending.len() != before;
        let queued = self
            .pending
            .iter()
            .any(|c| matches!(c, SessionCommand::RemoveHuman { slot: s } if *s == slot));
        if active && !cancelled && !queued {
            self.pending.push_back(SessionCommand::RemoveHuman { slot });
        }
    }

    /// Cancels `slot`'s `RemoveHuman` not yet closed (its pad came back in
    /// time); returns whether there was one.
    pub fn cancel_remove(&mut self, slot: u8) -> bool {
        let before = self.pending.len();
        self.pending
            .retain(|c| !matches!(c, SessionCommand::RemoveHuman { slot: s } if *s == slot));
        self.pending.len() != before
    }

    fn chunk(&mut self, i: usize) -> Vec<u8> {
        let first = i * LOG_CHUNK_TICKS as usize;
        let end = (first + LOG_CHUNK_TICKS as usize).min(self.log.len());
        let full = end - first == LOG_CHUNK_TICKS as usize;
        if full {
            while self.log_cache.len() <= i {
                let k = self.log_cache.len();
                let a = k * LOG_CHUNK_TICKS as usize;
                self.log_cache
                    .push(compress_log(&self.log[a..a + LOG_CHUNK_TICKS as usize]));
            }
            return self.log_cache[i].clone();
        }
        compress_log(&self.log[first..end])
    }
}

impl HostCore {
    /// The round's phase.
    pub fn round_phase(&self) -> RoundPhase {
        self.round.phase
    }

    /// The current round id (0 before the first round).
    pub fn round_id(&self) -> u32 {
        self.round.id
    }

    /// The next tick to close (= ticks closed this round).
    pub fn next_tick(&self) -> u32 {
        self.round.next_tick()
    }

    /// Starts a round with `setup`, whose seats are the slots in `humans`
    /// (module rules). Returns the round id.
    pub fn load_round(&mut self, setup: Vec<u8>, humans: &[u8]) -> u32 {
        let r = &mut self.round;
        r.id = r.id.wrapping_add(1).max(1);
        r.setup = setup;
        r.log.clear();
        r.log_cache.clear();
        r.pending.clear();
        r.host_hashes.clear();
        r.peer_hashes.clear();
        r.agreement.clear();
        r.agreed_through = None;
        r.all_loaded_sent = false;
        self.events.retain(|e| {
            !matches!(
                e,
                NetEvent::Bundle(_)
                    | NetEvent::Desync { .. }
                    | NetEvent::DesyncHashes { .. }
                    | NetEvent::HashesAgreed { .. }
            )
        });
        r.phase = RoundPhase::Loading {
            deadline: self.now + BARRIER_TIMEOUT,
        };
        self.round.roster = self.roster();
        let mut seats: Vec<u8> = humans
            .iter()
            .copied()
            .filter(|s| usize::from(*s) < crate::MAX_SLOTS)
            .collect();
        seats.sort_unstable();
        seats.dedup();
        for (s, p) in self.players.iter_mut() {
            p.active =
                seats.binary_search(s).is_ok() && (p.host_local || p.conn.is_some()) && !p.away;
            p.loaded = p.host_local;
            p.queue.clear();
        }
        self.round.late.clear();
        self.round.late_locals.clear();
        self.round.returned.clear();
        for s in seats {
            let here = self.players.get(&s).is_some_and(|p| p.active);
            // A host local whose pad is away waits like a guest machine's
            // unseated local (`late_locals`) for its return.
            if self.players.get(&s).is_some_and(|p| p.host_local && p.away) {
                self.round.late_locals.push(s);
            }
            if !here {
                self.round
                    .pending
                    .push_back(SessionCommand::RemoveHuman { slot: s });
            }
        }
        let mut ids = Vec::new();
        for (id, peer) in &self.peers {
            if let Phase::Joined(s) = peer.phase {
                let active = |x: &u8| self.players.get(x).is_some_and(|p| p.active);
                let mut all = std::iter::once(&s).chain(&peer.extra);
                if all.clone().any(active) {
                    ids.push(*id);
                    // Seated in part: the rest join once the round runs.
                    self.round
                        .late_locals
                        .extend(all.by_ref().filter(|x| !active(x)).copied());
                } else {
                    self.round.late.push(s);
                }
            }
        }
        for id in ids {
            self.start_stream(id, false);
        }
        self.check_all_loaded();
        self.round.id
    }

    /// Ends the load barrier: `Start` goes out and the tick clock runs.
    pub fn start_round(&mut self) {
        if !self.round.is_loading() {
            return;
        }
        let id = self.round.id;
        let lead = self.start_lead();
        self.round.phase = RoundPhase::Running {
            start: self.now + lead,
        };
        for p in self.players.values_mut() {
            p.seen_tick = 0;
            // A guest still loading at a barrier timeout starts late.
            p.catching_up = !p.host_local && !p.loaded;
        }
        let lead_us = u32::try_from(lead.0).unwrap_or(u32::MAX);
        let start = Msg::Start {
            round_id: id,
            lead_us,
        }
        .encode();
        for peer in self.peers.values_mut() {
            if peer.stream_round == id && matches!(peer.phase, Phase::Joined(_)) {
                peer.conn.send_reliable(&start);
            }
        }
        self.events.push_back(NetEvent::RoundStarted {
            round_id: id,
            lead_us,
        });
        // Connected but not seated: they join the running round.
        for slot in std::mem::take(&mut self.round.late) {
            let peer = self.players.get(&slot).and_then(|p| p.conn);
            if let Some(pid) = peer.filter(|pid| self.peers.contains_key(pid)) {
                self.start_stream(pid, true);
            }
        }
        self.release_late_locals();
    }

    /// `PeerCaughtUp` for each of [`RoundState::late_locals`] whose machine
    /// is live in the running round (loaded, not catching up); a slot that
    /// left or got seated meanwhile is dropped from the list.
    pub(crate) fn release_late_locals(&mut self) {
        if !self.round.is_running() {
            return;
        }
        let (round_id, tick) = (self.round.id, self.round.next_tick());
        let mut waiting = Vec::new();
        for slot in std::mem::take(&mut self.round.late_locals) {
            let Some(p) = self.players.get(&slot) else {
                continue;
            };
            let here = p.host_local || p.conn.is_some_and(|id| self.peers.contains_key(&id));
            if p.active || !here {
                continue;
            }
            // Away: it waits for its pad (`LocalBack` releases it).
            if p.away || !p.loaded || p.catching_up {
                waiting.push(slot);
                continue;
            }
            if self.round.returned.contains(&slot) {
                self.events.push_back(NetEvent::LocalBack {
                    slot,
                    round_id,
                    tick,
                });
            } else {
                self.events.push_back(NetEvent::PeerCaughtUp {
                    slot,
                    round_id,
                    tick,
                });
            }
        }
        self.round.returned.retain(|s| waiting.contains(s));
        self.round.late_locals = waiting;
    }

    /// The start's lead (module rules): the slowest guest `Start` goes to.
    fn start_lead(&self) -> Micros {
        let id = self.round.id;
        let half = self
            .peers
            .values()
            .filter(|p| p.stream_round == id && matches!(p.phase, Phase::Joined(_)))
            .map(|p| p.conn.rtt.srtt.max(0) as u64 / 2)
            .max();
        half.map_or(Micros(0), |h| {
            Micros((h + START_MARGIN.0).min(START_LEAD_MAX.0))
        })
    }

    /// Schedules `EndRound` in the next bundle.
    pub fn end_round(&mut self) {
        if self.round.is_running() {
            self.round.pending.push_back(SessionCommand::EndRound);
        }
    }

    /// The profile a later `AddHuman` of `slot` carries: the game sets
    /// each slot's latest (a guest's `Ready` profile, a host local's seat
    /// profile as the last round left it) when it loads a round, so a
    /// player seated after the round started (a late local, a pad back)
    /// plays on it rather than on the one its `Hello` brought. Returns
    /// whether `slot` is a player.
    pub fn set_profile(&mut self, slot: u8, profile: Vec<u8>) -> bool {
        match self.players.get_mut(&slot) {
            Some(p) => {
                p.profile = profile;
                true
            }
            None => false,
        }
    }

    /// Schedules `AddHuman` for a connected guest not yet in the round
    /// (after its `PeerCaughtUp` or `LocalBack`). Returns whether it was
    /// scheduled; refused for a slot whose pad is away.
    pub fn add_human(&mut self, slot: u8, team: u8) -> bool {
        if !self.round.is_running() {
            return false;
        }
        let Some(p) = self.players.get(&slot) else {
            return false;
        };
        let pending = self
            .round
            .pending
            .iter()
            .any(|c| matches!(c, SessionCommand::AddHuman { slot: s, .. } if *s == slot));
        // A guest's slot plays only while its machine is on this round's
        // stream (a benched machine keeps its link, not its bundles).
        let streaming = p
            .conn
            .and_then(|id| self.peers.get(&id))
            .is_some_and(|pr| pr.stream_round == self.round.id);
        if p.active || pending || p.away || (!p.host_local && !streaming) {
            return false;
        }
        let profile = p.profile.clone();
        self.round.pending.push_back(SessionCommand::AddHuman {
            slot,
            team,
            profile,
        });
        true
    }

    /// Takes one playing slot out of the round without marking it away
    /// (the scheduler tests' bare removal; a lost pad is `mark_away`).
    #[cfg(test)]
    pub(crate) fn unseat(&mut self, slot: u8) {
        let active = self.players.get(&slot).is_some_and(|p| p.active);
        self.round.remove_human(slot, active);
    }

    /// Schedules an opaque game command. Refused past
    /// [`MAX_COMMAND_BYTES`] or outside a round.
    pub fn issue_command(&mut self, data: Vec<u8>) -> bool {
        if data.len() > MAX_COMMAND_BYTES || !(self.round.is_running() || self.round.is_loading()) {
            return false;
        }
        self.round.pending.push_back(SessionCommand::Game(data));
        true
    }

    /// The host game's own `Sim::hash` for `tick`.
    pub fn report_hash(&mut self, tick: u32, hash: u64) {
        let r = &mut self.round;
        r.host_hashes.insert(tick, hash);
        while r.host_hashes.len() > HOST_HASHES_KEPT {
            r.host_hashes.pop_first();
        }
        let waiting: Vec<((u32, u8), u64)> = r
            .peer_hashes
            .range((tick, 0)..=(tick, u8::MAX))
            .map(|(k, v)| (*k, *v))
            .collect();
        for ((t, slot), peer_hash) in waiting {
            self.round.peer_hashes.remove(&(t, slot));
            self.compare_hash(slot, t, hash, peer_hash);
        }
    }

    fn compare_hash(&mut self, slot: u8, tick: u32, host_hash: u64, peer_hash: u64) {
        self.note_agreement(slot, tick, host_hash == peer_hash);
        if host_hash == peer_hash {
            return;
        }
        self.events.push_back(NetEvent::Desync {
            round_id: self.round.id,
            slot,
            tick,
            host_hash,
            peer_hash,
        });
    }

    /// Guest `slot`'s hash for `tick` was compared (`equal`): once every
    /// live guest's equalled the host's, `HashesAgreed` (module rules).
    fn note_agreement(&mut self, slot: u8, tick: u32, equal: bool) {
        if self.round.agreed_through.is_some_and(|t| tick <= t) {
            return;
        }
        // A machine's hash speaks for all its slots (module rules).
        let live: Vec<u8> = self
            .live_machines()
            .into_iter()
            .map(|(s, _, _)| s)
            .collect();
        let r = &mut self.round;
        let (agreed, differed) = r.agreement.entry(tick).or_default();
        if equal {
            if !agreed.contains(&slot) {
                agreed.push(slot);
            }
        } else {
            *differed = true;
        }
        let all = !*differed && !live.is_empty() && live.iter().all(|s| agreed.contains(s));
        if all {
            r.agreed_through = Some(tick);
            r.agreement.retain(|t, _| *t > tick);
            self.events.push_back(NetEvent::HashesAgreed {
                round_id: r.id,
                tick,
            });
        }
        while r.agreement.len() > AGREEMENT_KEPT {
            r.agreement.pop_first();
        }
    }

    /// Tells guest `slot` its hash at `tick` of this round differed from
    /// the host's `host_hash` (the host game decided its game parted).
    pub fn tell_desync(&mut self, slot: u8, tick: u32, host_hash: u64) {
        let msg = Msg::Desync {
            round_id: self.round.id,
            tick,
            host_hash,
            build: self.config.build.clone(),
            target: crate::local_target(),
        };
        self.send_to_slot(slot, &msg);
    }

    pub(crate) fn check_all_loaded(&mut self) {
        if !self.round.is_loading() || self.round.all_loaded_sent {
            return;
        }
        let all = self
            .players
            .iter()
            .filter(|(_, p)| p.active && (p.host_local || p.conn.is_some()))
            .all(|(_, p)| p.loaded);
        if all {
            self.round.all_loaded_sent = true;
            self.events.push_back(NetEvent::AllLoaded {
                round_id: self.round.id,
            });
        }
    }

    /// A round message from a joined guest; false when it does not belong.
    pub(crate) fn round_message(&mut self, slot: u8, msg: Msg) -> bool {
        match msg {
            Msg::Loaded {
                round_id,
                static_digest,
                hash0,
            } => {
                if round_id != self.round.id || round_id == 0 {
                    return false;
                }
                let live = self.round.next_tick();
                for s in self.machine_slots(slot) {
                    if let Some(p) = self.players.get_mut(&s) {
                        p.loaded = true;
                        if p.catching_up {
                            // Loaded after the start: behind from here on.
                            p.catching_up = false;
                            p.seen_tick = p.seen_tick.max(live);
                        }
                    }
                    self.events.push_back(NetEvent::PeerLoaded {
                        slot: s,
                        round_id,
                        static_digest,
                        hash0,
                    });
                }
                self.check_all_loaded();
                self.release_late_locals();
                true
            }
            Msg::HashReport {
                round_id,
                tick,
                hash,
            } => {
                if round_id != self.round.id {
                    // Stale: sent before this round loaded.
                    return true;
                }
                for s in self.machine_slots(slot) {
                    if let Some(p) = self.players.get_mut(&s) {
                        p.seen_tick = p.seen_tick.max(tick);
                    }
                }
                if let Some(h) = self.round.host_hashes.get(&tick).copied() {
                    self.compare_hash(slot, tick, h, hash);
                } else {
                    let r = &mut self.round;
                    r.peer_hashes.insert((tick, slot), hash);
                    while r.peer_hashes.len() > PEER_HASHES_KEPT {
                        r.peer_hashes.pop_first();
                    }
                }
                true
            }
            Msg::CaughtUp { round_id, tick } => {
                if round_id != self.round.id || !self.round.is_running() {
                    return true;
                }
                let slots = self.machine_slots(slot);
                // Every slot of the machine hears `PeerCaughtUp` here.
                self.round.late_locals.retain(|x| !slots.contains(x));
                for s in slots {
                    if let Some(p) = self.players.get_mut(&s) {
                        p.catching_up = false;
                        p.seen_tick = p.seen_tick.max(tick);
                    }
                    self.events.push_back(NetEvent::PeerCaughtUp {
                        slot: s,
                        round_id,
                        tick,
                    });
                }
                true
            }
            Msg::DesyncHashes {
                tick,
                build,
                target,
                first_tick,
                width,
                values,
            } => {
                self.events.push_back(NetEvent::DesyncHashes {
                    slot,
                    tick,
                    build,
                    target,
                    first_tick,
                    width,
                    values,
                });
                true
            }
            Msg::Resync => {
                for s in self.machine_slots(slot) {
                    if let Some(p) = self.players.get_mut(&s) {
                        p.catching_up = true;
                    }
                }
                let id = self.players.get(&slot).and_then(|p| p.conn);
                if let Some(id) = id {
                    if self.round.streams_to(self.round.id) {
                        self.start_stream(id, true);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Opens a new catch-up stream to a peer: `LoadRound`, then the log.
    pub(crate) fn start_stream(&mut self, id: u32, joining: bool) {
        let r = &mut self.round;
        r.next_stream = r.next_stream.wrapping_add(1).max(1);
        let stream = r.next_stream;
        let log_len = r.log.len() as u32;
        let load = Msg::LoadRound {
            round_id: r.id,
            stream,
            joining,
            log_len,
            setup: r.setup.clone(),
            roster: r.roster.clone(),
        }
        .encode();
        let chunks = log_len.div_ceil(LOG_CHUNK_TICKS) as usize;
        let mut msgs = Vec::with_capacity(chunks);
        for i in 0..chunks {
            let first_tick = i as u32 * LOG_CHUNK_TICKS;
            let count = (log_len - first_tick).min(LOG_CHUNK_TICKS);
            msgs.push(
                Msg::LogChunk {
                    stream,
                    first_tick,
                    count,
                    data: r.chunk(i),
                }
                .encode(),
            );
        }
        let round_id = r.id;
        if joining {
            for s in self.conn_slots(id) {
                if let Some(p) = self.players.get_mut(&s) {
                    p.catching_up = true;
                }
            }
        }
        let Some(peer) = self.peers.get_mut(&id) else {
            return;
        };
        peer.stream = stream;
        peer.stream_round = round_id;
        peer.stream_start = log_len;
        peer.next_needed = log_len;
        // The new stream's ticks are all unsent (a mark from a longer
        // earlier round would hold back every tick until it was passed).
        peer.sent_through = log_len;
        peer.conn.send_reliable(&load);
        for m in &msgs {
            peer.conn.send_reliable(m);
        }
    }

    /// Barrier timeout and the tick clock.
    pub(crate) fn round_update(&mut self) {
        match self.round.phase {
            RoundPhase::Loading { deadline } if self.now >= deadline => self.start_round(),
            _ => {}
        }
        while let RoundPhase::Running { start } = self.round.phase {
            let t = self.round.next_tick();
            if self.now < start.tick_time(t + 1) {
                break;
            }
            self.close_tick();
        }
        self.drop_fallen_behind();
    }

    /// The guest machines playing this round (module rules): each by its
    /// local 0's slot (its reports' key), with its link's trouble flag and
    /// the newest tick any of its seated slots is known to have stepped.
    /// A machine counts while any of its slots is seated and none of them
    /// is catching up, whichever local that is.
    fn live_machines(&self) -> Vec<(u8, bool, u32)> {
        let mut out = Vec::new();
        for (id, peer) in &self.peers {
            let Phase::Joined(s) = peer.phase else {
                continue;
            };
            let slots: Vec<&Player> = self
                .conn_slots(*id)
                .iter()
                .filter_map(|x| self.players.get(x))
                .filter(|p| p.conn == Some(*id))
                .collect();
            let seated = slots.iter().filter(|p| p.active);
            let seen = seated.clone().map(|p| p.seen_tick).max();
            if let Some(seen) = seen {
                if !slots.iter().any(|p| p.catching_up) {
                    out.push((s, peer.trouble, seen));
                }
            }
        }
        out
    }

    /// The fell-behind drop (module rules).
    fn drop_fallen_behind(&mut self) {
        let limit = self.config.fell_behind_ticks;
        if limit == 0 || !self.round.is_running() {
            return;
        }
        let now = self.round.next_tick();
        // A silent link is the timeout's business, not this check's.
        let fallen: Vec<u8> = self
            .live_machines()
            .into_iter()
            .filter(|(_, trouble, seen)| {
                !trouble && now > seen.saturating_add(limit + REPORT_SLACK)
            })
            .map(|(s, _, _)| s)
            .collect();
        for slot in fallen {
            self.kick_with(
                slot,
                false,
                super::FELL_BEHIND_TEXT.to_owned(),
                super::LeaveReason::FellBehind,
            );
        }
    }

    fn close_tick(&mut self) {
        let tick = self.round.next_tick();
        // Lag compensation: each pad stamped with how far behind its player
        // was viewing (`scheduler`). The active slots close first so the
        // commands can be fitted beside their pads.
        let cap = self.config.lag_comp_cap;
        let now = self.now;
        // One machine's locals that hold the same queued inputs close
        // together (`scheduler` rules), so they consume the same sequence.
        let mut by_conn: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
        for (s, p) in self.players.iter().filter(|(_, p)| p.active) {
            if let Some(c) = p.machine_key() {
                by_conn.entry(c).or_default().push(*s);
            }
        }
        let mut groups: Vec<Vec<u8>> = Vec::new();
        for slots in by_conn.into_values().filter(|g| g.len() > 1) {
            let mut classes: Vec<Vec<u8>> = Vec::new();
            for s in slots {
                let q = &self.players[&s].queue;
                match classes
                    .iter_mut()
                    .find(|c| self.players[&c[0]].queue.same_queued(q))
                {
                    Some(c) => c.push(s),
                    None => classes.push(vec![s]),
                }
            }
            groups.extend(classes.into_iter().filter(|c| c.len() > 1));
        }
        let mut pads: BTreeMap<u8, (PadState, u32)> = BTreeMap::new();
        for slots in groups {
            let mut qs: Vec<&mut crate::scheduler::SlotQueue> = self
                .players
                .iter_mut()
                .filter(|(s, _)| slots.contains(s))
                .map(|(_, p)| &mut p.queue)
                .collect();
            let closed = crate::scheduler::SlotQueue::close_group(&mut qs, tick, cap, now);
            for ((s, pad), q) in slots.iter().zip(closed).zip(&qs) {
                pads.insert(*s, (pad, q.consumed_seq()));
            }
        }
        for (s, p) in self.players.iter_mut().filter(|(_, p)| p.active) {
            if let std::collections::btree_map::Entry::Vacant(e) = pads.entry(*s) {
                e.insert((p.queue.close(tick, cap, now), p.queue.consumed_seq()));
            }
        }

        // Each pad's bytes against either base a stream codes it with.
        let prev: &[(u8, PadState)] = self.round.log.last().map_or(&[], |b| b.pads.as_slice());
        let mut pad_bytes: BTreeMap<u8, usize> = pads
            .iter()
            .map(|(s, (pad, seq))| {
                let base = prev
                    .binary_search_by_key(s, |p| p.0)
                    .map_or(PadState::default(), |i| prev[i].1);
                let n = pad_wire_len(pad, *seq, &PadState::default())
                    .max(pad_wire_len(pad, *seq, &base));
                (*s, n)
            })
            .collect();
        // Commands in order while they fit the bundle (`bundle` rules).
        let mut commands = Vec::new();
        let mut size = 2;
        while let Some(c) = self.round.pending.front() {
            if commands.len() >= MAX_COMMANDS {
                break;
            }
            let mut after = pad_bytes.clone();
            match c {
                SessionCommand::AddHuman { slot, .. } => {
                    after.insert(*slot, MAX_PAD_BYTES);
                }
                SessionCommand::RemoveHuman { slot } => {
                    after.remove(slot);
                }
                SessionCommand::Game(_) | SessionCommand::EndRound => {}
            }
            let n = c.wire_size();
            if size + n + after.values().sum::<usize>() > BUNDLE_BUDGET {
                break;
            }
            size += n;
            pad_bytes = after;
            commands.extend(self.round.pending.pop_front());
        }
        let mut ended = false;
        for c in &commands {
            match c {
                SessionCommand::AddHuman { slot, .. } => {
                    // A sibling local already playing (same connection,
                    // stepped this tick) to line the seated slot up with
                    // (`scheduler` rules).
                    let sib = self
                        .players
                        .get(slot)
                        .and_then(|p| p.machine_key())
                        .and_then(|c| {
                            self.players
                                .iter()
                                .find(|(s, x)| {
                                    **s != *slot
                                        && x.active
                                        && x.machine_key() == Some(c)
                                        && pads.contains_key(s)
                                })
                                .map(|(s, x)| (x.queue.clone(), pads[s].0.view_lag))
                        });
                    if let Some(p) = self.players.get_mut(slot) {
                        p.active = true;
                        p.seen_tick = p.seen_tick.max(tick);
                        let seated = sib.and_then(|(q, lag)| p.queue.seat_beside(&q, lag));
                        let pad = seated.unwrap_or_else(|| {
                            p.queue.clear();
                            p.queue.close(tick, cap, now)
                        });
                        pads.insert(*slot, (pad, p.queue.consumed_seq()));
                    }
                }
                SessionCommand::RemoveHuman { slot } => {
                    if let Some(p) = self.players.get_mut(slot) {
                        p.active = false;
                    }
                    pads.remove(slot);
                }
                SessionCommand::EndRound => ended = true,
                SessionCommand::Game(_) => {}
            }
        }
        let (pads, seqs) = pads
            .into_iter()
            .map(|(s, (pad, seq))| ((s, pad), seq))
            .unzip();
        let bundle = TickBundle {
            tick,
            pads,
            commands,
            seqs,
        };
        self.round.log.push(bundle.clone());
        self.events.push_back(NetEvent::Bundle(bundle));
        if ended {
            self.round.phase = RoundPhase::Ended;
        }
    }
}
