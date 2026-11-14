//! The host session core: the lobby, the per-connection links, timeouts and
//! the packets each update sends. `Hello` checks, slots, kicks and leaves
//! live in `host/admission.rs`; round flow, the tick clock, bundles and hash
//! checks in `host/round.rs`.
//!
//! Rules:
//! - Clock-injected: every entry point takes or reuses the `now` of the last
//!   [`HostCore::handle_datagram`] / [`HostCore::update`]; nothing reads a
//!   clock. Peers and players live in ordered maps.
//! - Slot 0 is the host's own player. Its pads go through the same input
//!   queue as everyone's ([`HostCore::submit_input`]).
//! - A new connection id is accepted only from a packet that carries
//!   reliable unit 0 (a `Hello`), and only while fewer than 64 handshakes
//!   are open. Packets from a known id at another address are dropped.
//! - `Hello` is checked in order: protocol, build (exact), ban, name, load
//!   barrier (`Busy`), then a slot: the same session token reclaims its old
//!   slot (connected or reserved), otherwise the lowest free slot below
//!   `max_players` (`Full` when none). Mid-round joins are accepted.
//! - A player who leaves or times out keeps slot and name for
//!   [`crate::RESERVE_FOR`] for the same token. A kick frees the slot.
//! - Silence: "trouble" after [`crate::TROUBLE_AFTER`], dropped after
//!   [`crate::DROP_AFTER`] ([`crate::DROP_AFTER_LOADING`] during the load
//!   barrier). Every peer hears from the host at least every
//!   [`crate::HEARTBEAT`].
//! - Lobby state is opaque, versioned and sent whole, at most every 100 ms.
//! - A peer on another protocol version gets a header-only answer (at most
//!   once a second per address) so it can say "version mismatch".
//! - Discovery: one `Found` per datagram however many `Find`s it carries,
//!   at most every [`FOUND_EVERY`] per address (a browser asks once a
//!   second), so the host is no reply amplifier.

mod admission;
use admission::Hello;
mod round;

use crate::bundle::TickBundle;
use crate::conn::{Conn, ConnStats};
use crate::msg::{Msg, RejectReason, RosterEntry};
use crate::packet::{
    bare_packet, parse_chunks, parse_header, raw_conn_id, Chunk, DropCounts, DropReason,
};
use crate::scheduler::SlotQueue;
use crate::{Micros, DROP_AFTER, DROP_AFTER_LOADING, HEARTBEAT, TROUBLE_AFTER};
use core::sim::PadState;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::{IpAddr, SocketAddr};

pub use round::RoundPhase;

/// The default lobby maximum (humans, host included).
pub const DEFAULT_MAX_PLAYERS: u8 = 16;
/// The most humans a host takes: their pads alone always fit one bundle
/// (`bundle` rules), whatever they hold.
pub const MAX_PLAYERS: u8 = {
    let n = (crate::packet::BUNDLE_BUDGET - 2) / crate::bundle::MAX_PAD_BYTES;
    if n < crate::MAX_SLOTS {
        n as u8
    } else {
        crate::MAX_SLOTS as u8
    }
};
/// [`HostConfig::lag_comp_cap`]'s default: the sim's own cap.
pub const DEFAULT_LAG_COMP_CAP: u8 = core::sim::lag_comp::VIEW_LAG_CAP;
const MAX_HANDSHAKES: usize = 64;
const LOBBY_INTERVAL: Micros = Micros::from_millis(100);
const CLOSE_LINGER: Micros = Micros::from_secs(2);
/// The shortest gap between two discovery answers to one address.
pub const FOUND_EVERY: Micros = Micros::from_millis(250);
/// The largest discovery info blob.
pub const MAX_INFO: usize = 1000;

/// What the host is and who plays on it.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// The sim build identity; a client's must match exactly.
    pub build: String,
    /// The host player's name (slot 0).
    pub name: String,
    pub profile: Vec<u8>,
    /// Humans allowed, host included (1..=[`MAX_PLAYERS`]).
    pub max_players: u8,
    /// The input queue depth the scheduler aims for.
    pub queue_target: usize,
    /// Lag compensation (`LAN_MULTIPLAYER.md` §6): the most ticks a closed
    /// pad's `view_lag` may say its player was behind (8, about 250 ms); 0
    /// turns lag compensation off.
    pub lag_comp_cap: u8,
    /// Slow machines (`LAN_MULTIPLAYER.md` §8b): a guest playing the round
    /// whose reported tick falls more than this many ticks behind live is
    /// dropped with [`FELL_BEHIND_TEXT`] ([`DEFAULT_FELL_BEHIND_TICKS`], 10 s);
    /// 0 turns the drop off. A guest catching up (a join, a reconnect, a
    /// resync) is exempt until it reports `CaughtUp`.
    pub fell_behind_ticks: u32,
}

/// [`HostConfig::fell_behind_ticks`]' default: 10 s of ticks.
pub const DEFAULT_FELL_BEHIND_TICKS: u32 = 300;
/// The reject detail a guest dropped for falling behind gets.
pub const FELL_BEHIND_TEXT: &str = "Your computer could not keep up with this battle";

impl HostConfig {
    pub fn new(build: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            build: build.into(),
            name: name.into(),
            profile: Vec::new(),
            max_players: DEFAULT_MAX_PLAYERS,
            queue_target: 1,
            lag_comp_cap: DEFAULT_LAG_COMP_CAP,
            fell_behind_ticks: DEFAULT_FELL_BEHIND_TICKS,
        }
    }
}

/// Why a player left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LeaveReason {
    Left,
    Timeout,
    Kicked,
    /// The same token connected again; the old link was replaced.
    Replaced,
    /// Dropped for falling behind live ([`HostConfig::fell_behind_ticks`]).
    FellBehind,
}

/// What the host game hears from its network core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetEvent {
    PlayerJoined {
        slot: u8,
        name: String,
        content: Vec<u8>,
        profile: Vec<u8>,
        reconnect: bool,
    },
    PlayerLeft {
        slot: u8,
        reason: LeaveReason,
    },
    /// A refused `Hello` (for the log).
    Refused {
        addr: SocketAddr,
        reason: RejectReason,
    },
    Trouble {
        slot: u8,
        on: bool,
    },
    PeerLoaded {
        slot: u8,
        round_id: u32,
        static_digest: u64,
        hash0: u64,
    },
    /// Every connected roster player reported `Loaded`.
    AllLoaded {
        round_id: u32,
    },
    RoundStarted {
        round_id: u32,
    },
    /// A closed tick; the host game steps it at once.
    Bundle(TickBundle),
    PeerCaughtUp {
        slot: u8,
        round_id: u32,
        tick: u32,
    },
    Desync {
        slot: u8,
        tick: u32,
        host_hash: u64,
        peer_hash: u64,
    },
    /// A guest's per-tick hash rows around its desync at `tick`
    /// ([`crate::msg::Msg::DesyncHashes`]), for the host's report.
    DesyncHashes {
        slot: u8,
        tick: u32,
        build: String,
        target: String,
        first_tick: u32,
        width: u8,
        values: Vec<u64>,
    },
    PeerReady {
        slot: u8,
        round_id: u32,
    },
    Request {
        slot: u8,
        data: Vec<u8>,
    },
    Chat {
        slot: u8,
        text: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Handshake,
    Joined(u8),
    Closing(Micros),
}

struct Peer {
    conn: Conn,
    phase: Phase,
    /// The catch-up stream this peer is on (0: none).
    stream: u32,
    stream_round: u32,
    stream_start: u32,
    /// The first live tick the peer does not hold yet.
    next_needed: u32,
    trouble: bool,
    /// The log length when this peer's stream last sent (a packet goes out
    /// for new ticks, otherwise only for an ack, a resend or a heartbeat).
    /// Reset with every new stream.
    sent_through: u32,
}

pub(crate) struct Player {
    name: String,
    token: u64,
    profile: Vec<u8>,
    conn: Option<u32>,
    queue: SlotQueue,
    reserved_until: Option<Micros>,
    /// A human in the current round's bundles.
    active: bool,
    loaded: bool,
    /// The newest tick this player is known to have stepped: its hash
    /// reports, or the tick it became live (the fell-behind check).
    seen_tick: u32,
    /// Stepping a catch-up stream (a join, a reconnect, a resync) until
    /// its `CaughtUp`.
    catching_up: bool,
}

/// One connected player's link figures.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PeerStats {
    pub slot: u8,
    pub name: String,
    pub rtt_ms: f64,
    pub rttvar_ms: f64,
    pub silence_ms: f64,
    pub trouble: bool,
    pub queue_depth: u8,
    pub starved: u32,
    /// Slips merged away and the published slack (µs).
    pub slips: u32,
    pub slack_us: Option<i64>,
    pub link: ConnStats,
}

/// The host's net meter.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct HostStats {
    pub tick: u32,
    pub local_depth: u8,
    pub local_starved: u32,
    /// The host player's published slack (µs, `scheduler`).
    pub local_slack_us: Option<i64>,
    pub peers: Vec<PeerStats>,
    pub drops: DropCounts,
    /// Live packets whose next bundle did not fit (the `bundle` rules keep
    /// this 0; anything else is a host bug that stalls that guest).
    pub oversize_bundles: u64,
}

/// The host's protocol state machine.
pub struct HostCore {
    config: HostConfig,
    now: Micros,
    peers: BTreeMap<u32, Peer>,
    players: BTreeMap<u8, Player>,
    banned_tokens: BTreeSet<u64>,
    banned_ips: BTreeSet<IpAddr>,
    events: VecDeque<NetEvent>,
    outbox: Vec<(SocketAddr, Vec<u8>)>,
    version_replies: BTreeMap<SocketAddr, Micros>,
    found_replies: BTreeMap<SocketAddr, Micros>,
    lobby: Vec<u8>,
    lobby_version: u32,
    lobby_dirty: bool,
    lobby_sent: Micros,
    info: Vec<u8>,
    pub(crate) round: round::RoundState,
    pub drops: DropCounts,
    oversize_bundles: u64,
}

impl HostCore {
    pub fn new(mut config: HostConfig) -> Self {
        config.max_players = config.max_players.clamp(1, MAX_PLAYERS);
        let mut players = BTreeMap::new();
        players.insert(
            0,
            Player {
                name: config.name.clone(),
                token: 0,
                profile: config.profile.clone(),
                conn: None,
                queue: SlotQueue::new(config.queue_target),
                reserved_until: None,
                active: false,
                loaded: true,
                seen_tick: 0,
                catching_up: false,
            },
        );
        Self {
            config,
            now: Micros::ZERO,
            peers: BTreeMap::new(),
            players,
            banned_tokens: BTreeSet::new(),
            banned_ips: BTreeSet::new(),
            events: VecDeque::new(),
            outbox: Vec::new(),
            version_replies: BTreeMap::new(),
            found_replies: BTreeMap::new(),
            lobby: Vec::new(),
            lobby_version: 0,
            lobby_dirty: false,
            lobby_sent: Micros::ZERO,
            info: Vec::new(),
            round: round::RoundState::default(),
            drops: DropCounts::default(),
            oversize_bundles: 0,
        }
    }

    pub fn config(&self) -> &HostConfig {
        &self.config
    }

    /// The next event for the host game.
    pub fn poll_event(&mut self) -> Option<NetEvent> {
        self.events.pop_front()
    }

    /// The host player's input for its next input tick, with the confirmed
    /// tick the host was viewing (its sim's `tick`: the bundles it has
    /// stepped), stamped into `view_lag` at the close like a guest's.
    pub fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        let running = self.round.accepts_inputs();
        if let Some(p) = self.players.get_mut(&0) {
            let queue = running && p.active;
            p.queue.push(pad, view_tick, self.now, queue);
        }
    }

    /// Sets the discovery reply blob (name, map, players, ...), truncated to
    /// [`MAX_INFO`] bytes.
    pub fn set_info(&mut self, info: Vec<u8>) {
        let mut info = info;
        info.truncate(MAX_INFO);
        self.info = info;
    }

    /// Replaces the lobby state; guests get it within 100 ms.
    pub fn set_lobby(&mut self, state: Vec<u8>) {
        self.lobby = state;
        self.lobby_version = self.lobby_version.wrapping_add(1);
        self.lobby_dirty = true;
    }

    /// Connected players by slot (host included), with names.
    pub fn players(&self) -> Vec<(u8, String)> {
        self.players
            .iter()
            .filter(|(s, p)| **s == 0 || p.conn.is_some())
            .map(|(s, p)| (*s, p.name.clone()))
            .collect()
    }

    /// Sends an opaque message to one player, or to every guest.
    pub fn send_notice(&mut self, slot: Option<u8>, data: Vec<u8>) {
        let m = Msg::Notice(data).encode();
        for (s, id) in self.guest_conns() {
            if slot.is_none_or(|x| x == s) {
                if let Some(peer) = self.peers.get_mut(&id) {
                    peer.conn.send_reliable(&m);
                }
            }
        }
    }

    /// The host player's chat line, to every guest.
    pub fn chat(&mut self, text: &str) {
        self.relay_chat(0, text);
    }

    fn relay_chat(&mut self, from: u8, text: &str) {
        let m = Msg::Chat {
            slot: from,
            text: text.to_owned(),
        }
        .encode();
        for (s, id) in self.guest_conns() {
            if s != from {
                if let Some(peer) = self.peers.get_mut(&id) {
                    peer.conn.send_reliable(&m);
                }
            }
        }
    }

    /// Tells every guest the host is leaving; send one more update after it.
    pub fn close(&mut self) {
        let m = Msg::Leave.encode();
        for peer in self.peers.values_mut() {
            peer.conn.send_reliable(&m);
        }
    }

    fn guest_conns(&self) -> Vec<(u8, u32)> {
        self.peers
            .iter()
            .filter_map(|(id, p)| match p.phase {
                Phase::Joined(s) => Some((s, *id)),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn send_to_slot(&mut self, slot: u8, msg: &Msg) {
        let id = self.players.get(&slot).and_then(|p| p.conn);
        if let Some(peer) = id.and_then(|id| self.peers.get_mut(&id)) {
            peer.conn.send_reliable(&msg.encode());
        }
    }

    /// Takes one datagram from the network.
    pub fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]) {
        self.now = now;
        let (h, body) = match parse_header(bytes) {
            Ok(x) => x,
            Err(d) => {
                self.drops.count(d);
                if let (DropReason::Protocol(_), Some(id)) = (d, raw_conn_id(bytes)) {
                    self.answer_version(from, id);
                }
                return;
            }
        };
        let Ok(chunks) = parse_chunks(body) else {
            self.drops.malformed += 1;
            return;
        };
        if h.conn == 0 {
            let nonce = chunks.iter().find_map(|c| match c {
                Chunk::Find { nonce } => Some(*nonce),
                _ => None,
            });
            let due = self
                .found_replies
                .get(&from)
                .is_none_or(|t| now.0 >= t.0 + FOUND_EVERY.0);
            if let (Some(nonce), true) = (nonce, due) {
                self.found_replies.insert(from, now);
                let mut pw = bare_packet(0);
                pw.found(nonce, &self.info);
                self.outbox.push((from, pw.finish()));
            }
            return;
        }
        if !self.peers.contains_key(&h.conn) {
            let opens = chunks
                .iter()
                .any(|c| matches!(c, Chunk::Reliable { unit: 0, .. }));
            let handshakes = self
                .peers
                .values()
                .filter(|p| p.phase == Phase::Handshake)
                .count();
            if !opens || handshakes >= MAX_HANDSHAKES {
                self.drops.unknown_conn += 1;
                return;
            }
            self.peers.insert(
                h.conn,
                Peer {
                    conn: Conn::new(h.conn, from, now),
                    phase: Phase::Handshake,
                    stream: 0,
                    stream_round: 0,
                    stream_start: 0,
                    next_needed: 0,
                    trouble: false,
                    sent_through: 0,
                },
            );
        }
        let peer = self.peers.get_mut(&h.conn).expect("peer present");
        if peer.conn.addr != from {
            self.drops.wrong_addr += 1;
            return;
        }
        let Some(rest) = peer.conn.receive(now, &h, chunks, bytes.len()) else {
            return;
        };
        let phase = peer.phase;
        for c in rest {
            match c {
                Chunk::ClientStatus {
                    stream,
                    next_needed,
                } => {
                    if let Some(peer) = self.peers.get_mut(&h.conn) {
                        if stream == peer.stream && next_needed > peer.next_needed {
                            peer.next_needed = next_needed;
                        }
                    }
                }
                Chunk::Inputs(body) => {
                    if let Phase::Joined(slot) = phase {
                        self.take_inputs(slot, body);
                    }
                }
                _ => self.drops.bad_chunk += 1,
            }
        }
        self.drain_messages(h.conn);
    }

    fn answer_version(&mut self, from: SocketAddr, conn: u32) {
        let due = self
            .version_replies
            .get(&from)
            .is_none_or(|t| self.now.0 >= t.0 + 1_000_000);
        if due && conn != 0 {
            self.version_replies.insert(from, self.now);
            self.outbox.push((from, bare_packet(conn).finish()));
        }
    }

    fn take_inputs(&mut self, slot: u8, body: &[u8]) {
        let accepts = self.round.accepts_inputs();
        let now = self.now;
        let Some(p) = self.players.get_mut(&slot) else {
            return;
        };
        let q = &mut p.queue;
        match crate::packet::decode_inputs(body, |s| q.base(s)) {
            Ok(Some((base, pads))) => q.receive(base, &pads, accepts && p.active, now),
            Ok(None) => self.drops.bad_chunk += 1,
            Err(_) => self.drops.bad_chunk += 1,
        }
    }

    fn drain_messages(&mut self, id: u32) {
        loop {
            let Some(peer) = self.peers.get_mut(&id) else {
                return;
            };
            if peer.conn.broken {
                self.drop_peer(id, LeaveReason::Left);
                return;
            }
            let Some(bytes) = peer.conn.take_message() else {
                return;
            };
            let phase = peer.phase;
            let Ok(msg) = Msg::decode(&bytes) else {
                self.drops.bad_message += 1;
                continue;
            };
            match (phase, msg) {
                (
                    Phase::Handshake,
                    Msg::Hello {
                        protocol,
                        build,
                        name,
                        token,
                        content,
                        profile,
                    },
                ) => {
                    let hello = Hello {
                        protocol,
                        build,
                        name,
                        token,
                        content,
                        profile,
                    };
                    self.admit(id, hello);
                }
                (Phase::Joined(_), Msg::Leave) => {
                    self.drop_peer(id, LeaveReason::Left);
                    return;
                }
                (Phase::Joined(slot), Msg::Chat { text, .. }) => {
                    self.events.push_back(NetEvent::Chat {
                        slot,
                        text: text.clone(),
                    });
                    self.relay_chat(slot, &text);
                }
                (Phase::Joined(slot), Msg::Request(data)) => {
                    self.events.push_back(NetEvent::Request { slot, data });
                }
                (Phase::Joined(slot), Msg::Ready { round_id }) => {
                    self.events
                        .push_back(NetEvent::PeerReady { slot, round_id });
                }
                (Phase::Joined(slot), m) => {
                    if !self.round_message(slot, m) {
                        self.drops.bad_message += 1;
                    }
                }
                _ => self.drops.bad_message += 1,
            }
        }
    }

    /// Advances timers and the tick clock; returns the datagrams to send.
    pub fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        self.now = now;
        self.round_update();
        self.expire(now);
        let lobby = if self.lobby_dirty && now.0 >= self.lobby_sent.0 + LOBBY_INTERVAL.0 {
            self.lobby_dirty = false;
            self.lobby_sent = now;
            Some(
                Msg::Lobby {
                    version: self.lobby_version,
                    state: self.lobby.clone(),
                }
                .encode(),
            )
        } else {
            None
        };
        let mut out = std::mem::take(&mut self.outbox);
        let ids: Vec<u32> = self.peers.keys().copied().collect();
        for id in ids {
            if let (Some(l), Some(peer)) = (&lobby, self.peers.get_mut(&id)) {
                if matches!(peer.phase, Phase::Joined(_)) {
                    peer.conn.send_reliable(l);
                }
            }
            self.build_for(id, now, &mut out);
        }
        out
    }

    fn expire(&mut self, now: Micros) {
        let limit = if self.round.is_loading() {
            DROP_AFTER_LOADING
        } else {
            DROP_AFTER
        };
        let ids: Vec<u32> = self.peers.keys().copied().collect();
        for id in ids {
            let peer = self.peers.get_mut(&id).expect("peer");
            let silence = peer.conn.silence(now);
            match peer.phase {
                Phase::Closing(until) => {
                    if now >= until || peer.conn.reliable_idle() {
                        self.peers.remove(&id);
                    }
                    continue;
                }
                Phase::Handshake if silence >= DROP_AFTER => {
                    self.peers.remove(&id);
                    continue;
                }
                Phase::Handshake => continue,
                Phase::Joined(slot) => {
                    if silence >= limit {
                        self.drop_peer(id, LeaveReason::Timeout);
                        continue;
                    }
                    let trouble = silence >= TROUBLE_AFTER;
                    if trouble != peer.trouble {
                        peer.trouble = trouble;
                        self.events
                            .push_back(NetEvent::Trouble { slot, on: trouble });
                        if !trouble {
                            // Heard again: half the fell-behind limit to report.
                            let grace = self
                                .round
                                .next_tick()
                                .saturating_sub(self.config.fell_behind_ticks / 2);
                            if let Some(p) = self.players.get_mut(&slot) {
                                p.seen_tick = p.seen_tick.max(grace);
                            }
                        }
                    }
                }
            }
        }
        self.players.retain(|s, p| {
            *s == 0 || p.conn.is_some() || p.reserved_until.is_some_and(|t| now < t)
        });
        self.version_replies.retain(|_, t| now.0 < t.0 + 10_000_000);
        self.found_replies
            .retain(|_, t| now.0 < t.0 + FOUND_EVERY.0);
    }

    fn build_for(&mut self, id: u32, now: Micros, out: &mut Vec<(SocketAddr, Vec<u8>)>) {
        let log_len = self.round.log.len() as u32;
        let streaming = self.round.streams_to(self.peers[&id].stream_round);
        let peer = self.peers.get_mut(&id).expect("peer");
        let new_ticks = streaming && log_len > peer.sent_through && peer.stream != 0;
        if !new_ticks && !peer.conn.wants_send(now, HEARTBEAT) {
            return;
        }
        let status = match peer.phase {
            Phase::Joined(slot) => self.players.get(&slot).map(|p| {
                (
                    p.queue.recv_through(),
                    p.queue.depth,
                    p.queue.starved,
                    p.queue.slack_us(),
                )
            }),
            _ => None,
        };
        let log = &self.round.log;
        let (stream, start, next) = (peer.stream, peer.stream_start, peer.next_needed);
        let mut sent_through = peer.sent_through;
        let mut oversize = false;
        let packets = peer.conn.build(now, |pw| {
            if let Some((through, depth, starved, slack)) = status {
                pw.host_status(through, depth, starved, slack);
            }
            if streaming && stream != 0 && next < log_len {
                let base = (next > start).then(|| log[next as usize - 1].pads.as_slice());
                let budget = pw.room().saturating_sub(crate::packet::CHUNK_OVERHEAD);
                let (body, n) = crate::packet::encode_bundles(
                    stream,
                    next,
                    base,
                    &log[next as usize..],
                    budget,
                );
                if n > 0 {
                    pw.bundles(&body);
                } else {
                    oversize = true;
                }
            }
        });
        if streaming {
            sent_through = log_len;
        }
        peer.sent_through = sent_through;
        let addr = peer.conn.addr;
        self.oversize_bundles += u64::from(oversize);
        out.extend(packets.into_iter().map(|p| (addr, p)));
    }

    /// The net meter.
    pub fn stats(&self) -> HostStats {
        let local = &self.players[&0].queue;
        HostStats {
            tick: self.round.next_tick(),
            local_depth: local.depth,
            local_starved: local.starved,
            local_slack_us: local.slack_us(),
            peers: self
                .peers
                .values()
                .filter_map(|peer| {
                    let Phase::Joined(slot) = peer.phase else {
                        return None;
                    };
                    let p = self.players.get(&slot)?;
                    Some(PeerStats {
                        slot,
                        name: p.name.clone(),
                        rtt_ms: peer.conn.rtt.srtt as f64 / 1e3,
                        rttvar_ms: peer.conn.rtt.rttvar as f64 / 1e3,
                        silence_ms: peer.conn.silence(self.now).as_millis_f64(),
                        trouble: peer.trouble,
                        queue_depth: p.queue.depth,
                        starved: p.queue.starved,
                        slips: p.queue.slips,
                        slack_us: p.queue.slack_us(),
                        link: peer.conn.stats,
                    })
                })
                .collect(),
            drops: self.drops,
            oversize_bundles: self.oversize_bundles,
        }
    }

    /// The queue depth (before the pop) at the last close for `slot`.
    pub fn queue_depth(&self, slot: u8) -> Option<u8> {
        self.players.get(&slot).map(|p| p.queue.depth)
    }

    /// `slot`'s pacing report (depth, starvations), as a client gets it; the
    /// host game feeds slot 0's to its own [`crate::InputPacer`].
    pub fn pacing(&self, slot: u8) -> Option<(u8, u32)> {
        self.players
            .get(&slot)
            .map(|p| (p.queue.depth, p.queue.starved))
    }

    /// `slot`'s published input slack in microseconds (`scheduler`), as a
    /// client gets it.
    pub fn slack_us(&self, slot: u8) -> Option<i64> {
        self.players.get(&slot).and_then(|p| p.queue.slack_us())
    }

    /// Whether `slot` is a human in the current round's bundles.
    pub fn is_active(&self, slot: u8) -> bool {
        self.players.get(&slot).is_some_and(|p| p.active)
    }

    pub(crate) fn roster(&self) -> Vec<RosterEntry> {
        self.players
            .iter()
            .filter(|(s, p)| **s == 0 || p.conn.is_some())
            .map(|(s, p)| RosterEntry {
                slot: *s,
                name: p.name.clone(),
                profile: p.profile.clone(),
            })
            .collect()
    }
}
