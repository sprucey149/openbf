//! The client session core: the handshake, the input stream up, the bundle
//! stream down and the round messages.
//!
//! Rules:
//! - Clock-injected like the host core: nothing reads a clock.
//! - Inputs: [`ClientCore::submit_input`] numbers each input tick from 1.
//!   Each carries the confirmed tick its player was viewing (the client
//!   sim's `tick`, the bundles it has stepped), for the host's lag
//!   compensation stamp.
//!   Every packet carries every input the host has not acknowledged (as many
//!   as fit), delta-coded from the last acknowledged one. There is no resend
//!   timer; the next packet covers a loss.
//! - Bundles: the host streams from the first tick this client lacks
//!   (`ClientStatus.next_needed`), delta-coded from the tick before when the
//!   client holds it. Bundles of an older stream are ignored.
//! - Delivery: [`ClientEvent::Bundle`]s come out in strict tick order from
//!   tick 0 of the round: the catch-up log first (ticks below `log_len`),
//!   then live bundles. A round-start client gets none before `Start`.
//! - Arrival jitter (RFC 3550 style, gain 1/16) of new live bundles feeds the
//!   game's [`crate::Playback`] target.
//! - A packet from the host address carrying another protocol version means
//!   "version mismatch": the session ends with `Rejected(Version)`.
//! - Silence: trouble after 1.5 s, disconnected after 12 s (60 s while a
//!   round loads, between `LoadRound` and [`ClientCore::loaded`]).

use crate::bundle::{decompress_log, TickBundle};
use crate::conn::{Conn, ConnStats};
use crate::msg::{Msg, RejectReason, RosterEntry};
use crate::packet::{
    decode_bundles_head, decode_bundles_rest, encode_inputs, parse_chunks, parse_header,
    raw_conn_id, Chunk, DropCounts, DropReason, CHUNK_OVERHEAD, PROTOCOL,
};
use crate::{Micros, DROP_AFTER, DROP_AFTER_LOADING, HEARTBEAT, TROUBLE_AFTER};
use core::sim::PadState;
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

/// Live bundles kept as delta bases behind the newest held one.
const BASES_KEPT: u32 = 256;
const LEAVE_LINGER: Micros = Micros::from_secs(1);

/// Who this client is.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub build: String,
    pub name: String,
    /// Random per player session; reconnecting with it reclaims the slot.
    pub token: u64,
    pub content: Vec<u8>,
    pub profile: Vec<u8>,
    /// Random, nonzero, new for every connection attempt.
    pub conn_id: u32,
}

impl ClientConfig {
    pub fn new(build: impl Into<String>, name: impl Into<String>, token: u64) -> Self {
        Self {
            build: build.into(),
            name: name.into(),
            token,
            content: Vec::new(),
            profile: Vec::new(),
            conn_id: crate::fresh_conn_id(),
        }
    }
}

/// Why the session ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum DisconnectReason {
    HostLeft,
    Timeout,
    /// This client called [`ClientCore::leave`].
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ClientState {
    Connecting,
    Connected { slot: u8 },
    Rejected(RejectReason),
    Disconnected(DisconnectReason),
}

/// What the client game hears from its network core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    Welcome {
        slot: u8,
        max_players: u8,
    },
    Rejected {
        reason: RejectReason,
        detail: String,
    },
    Lobby {
        version: u32,
        state: Vec<u8>,
    },
    /// Build this round. `joining`: no `Start` follows; ticks `0..log_len`
    /// arrive as bundles to fast-forward through, then live ones.
    LoadRound {
        round_id: u32,
        joining: bool,
        log_len: u32,
        setup: Vec<u8>,
        roster: Vec<RosterEntry>,
    },
    Start {
        round_id: u32,
    },
    Bundle(TickBundle),
    /// The host's hash differed from ours at `tick`; `host_build` and
    /// `host_target` are for the desync report.
    Desync {
        tick: u32,
        host_hash: u64,
        host_build: String,
        host_target: String,
    },
    Notice(Vec<u8>),
    Chat {
        slot: u8,
        text: String,
    },
    Trouble(bool),
    Disconnected(DisconnectReason),
}

/// The client's net meter.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct ClientStats {
    pub rtt_ms: f64,
    pub rttvar_ms: f64,
    pub queue_depth: u8,
    pub starved: u32,
    /// [`ClientCore::slack_us`].
    pub slack_us: Option<i64>,
    /// The newest input sequence submitted.
    pub inputs_sent: u32,
    pub jitter_ticks: f64,
    pub unacked_inputs: usize,
    pub link: ConnStats,
    pub drops: DropCounts,
}

/// The client's protocol state machine.
pub struct ClientCore {
    config: ClientConfig,
    host: SocketAddr,
    now: Micros,
    conn: Conn,
    state: ClientState,
    events: VecDeque<ClientEvent>,
    /// Unacknowledged inputs: sequence, pad, view tick.
    inputs: VecDeque<(u32, PadState, u32)>,
    next_input: u32,
    input_acked: u32,
    acked_pad: PadState,
    inputs_dirty: bool,
    depth: u8,
    starved: u32,
    slack_us: Option<i64>,
    round_id: u32,
    stream: u32,
    log_len: u32,
    awaiting_start: bool,
    loading: bool,
    log: BTreeMap<u32, TickBundle>,
    live: BTreeMap<u32, TickBundle>,
    bases: BTreeMap<u32, Vec<(u8, PadState)>>,
    next_needed: u32,
    next_deliver: u32,
    status_dirty: bool,
    last_offset: Option<i64>,
    jitter_us: f64,
    trouble: bool,
    leave_until: Option<Micros>,
    pub drops: DropCounts,
}

impl ClientCore {
    /// Starts connecting to `host`; the `Hello` goes out on the first update.
    pub fn new(config: ClientConfig, host: SocketAddr, now: Micros) -> Self {
        let mut conn = Conn::new(config.conn_id, host, now);
        conn.send_reliable(
            &Msg::Hello {
                protocol: PROTOCOL,
                build: config.build.clone(),
                name: config.name.clone(),
                token: config.token,
                content: config.content.clone(),
                profile: config.profile.clone(),
            }
            .encode(),
        );
        Self {
            config,
            host,
            now,
            conn,
            state: ClientState::Connecting,
            events: VecDeque::new(),
            inputs: VecDeque::new(),
            next_input: 1,
            input_acked: 0,
            acked_pad: PadState::default(),
            inputs_dirty: false,
            depth: 0,
            starved: 0,
            slack_us: None,
            round_id: 0,
            stream: 0,
            log_len: 0,
            awaiting_start: false,
            loading: false,
            log: BTreeMap::new(),
            live: BTreeMap::new(),
            bases: BTreeMap::new(),
            next_needed: 0,
            next_deliver: 0,
            status_dirty: false,
            last_offset: None,
            jitter_us: 0.0,
            trouble: false,
            leave_until: None,
            drops: DropCounts::default(),
        }
    }

    pub fn state(&self) -> ClientState {
        self.state
    }

    pub fn slot(&self) -> Option<u8> {
        match self.state {
            ClientState::Connected { slot } => Some(slot),
            _ => None,
        }
    }

    pub fn host_addr(&self) -> SocketAddr {
        self.host
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }

    /// The host's last report on this slot's input queue: depth before the
    /// last close's pop, and the starvation count (for [`crate::InputPacer`]).
    pub fn pacing(&self) -> (u8, u32) {
        (self.depth, self.starved)
    }

    /// The host's last published slack for this client's inputs (µs,
    /// `scheduler`): how early the latest of them arrived.
    pub fn slack_us(&self) -> Option<i64> {
        self.slack_us
    }

    /// Live-bundle arrival jitter in ticks.
    pub fn jitter_ticks(&self) -> f64 {
        self.jitter_us * crate::TICK_HZ / 1e6
    }

    fn connected(&self) -> bool {
        matches!(self.state, ClientState::Connected { .. })
    }

    fn send(&mut self, m: Msg) {
        if self.connected() {
            self.conn.send_reliable(&m.encode());
        }
    }

    /// Sends one input tick's pad, made while the player was viewing the
    /// confirmed sim at `view_tick` (its `Sim::tick`: how many bundles it
    /// had stepped). Returns its sequence number.
    pub fn submit_input(&mut self, pad: PadState, view_tick: u32) -> Option<u32> {
        if !self.connected() {
            return None;
        }
        let seq = self.next_input;
        self.next_input += 1;
        self.inputs.push_back((seq, pad, view_tick));
        self.inputs_dirty = true;
        Some(seq)
    }

    /// The round is built: its static gameplay digest and tick-0 hash.
    pub fn loaded(&mut self, round_id: u32, static_digest: u64, hash0: u64) {
        self.loading = false;
        self.send(Msg::Loaded {
            round_id,
            static_digest,
            hash0,
        });
    }

    /// A joiner is within reach of live play.
    pub fn caught_up(&mut self, tick: u32) {
        self.loading = false;
        let round_id = self.round_id;
        self.send(Msg::CaughtUp { round_id, tick });
    }

    /// This peer's `Sim::hash` after `tick` (every 30 ticks).
    pub fn report_hash(&mut self, tick: u32, hash: u64) {
        self.send(Msg::HashReport { tick, hash });
    }

    /// Our per-tick hash rows around the desync at `tick` (LAN §4.7), for
    /// the host's report: `width` values a row, rows from `first_tick`.
    /// Cut to whole rows and at most [`crate::msg::MAX_DESYNC_ROWS`] (the
    /// newest kept); a width outside 1..=8 sends nothing.
    pub fn desync_hashes(&mut self, tick: u32, first_tick: u32, width: u8, values: Vec<u64>) {
        if width == 0 || width > crate::msg::MAX_DESYNC_WIDTH {
            return;
        }
        let w = usize::from(width);
        let rows = values.len() / w;
        let skip = rows.saturating_sub(crate::msg::MAX_DESYNC_ROWS);
        let values = values[skip * w..rows * w].to_vec();
        let build = self.config.build.clone();
        self.send(Msg::DesyncHashes {
            tick,
            build,
            target: crate::local_target(),
            first_tick: first_tick + skip as u32,
            width,
            values,
        });
    }

    /// Asks for the round again (after a desync): a new `LoadRound` and log.
    pub fn request_resync(&mut self) {
        self.stream = 0;
        self.log.clear();
        self.live.clear();
        self.bases.clear();
        self.send(Msg::Resync);
    }

    pub fn ready(&mut self, round_id: u32) {
        self.send(Msg::Ready { round_id });
    }

    /// An opaque request for the host game.
    pub fn request(&mut self, data: Vec<u8>) {
        self.send(Msg::Request(data));
    }

    pub fn chat(&mut self, text: &str) {
        let slot = self.slot().unwrap_or(0);
        self.send(Msg::Chat {
            slot,
            text: text.to_owned(),
        });
    }

    /// Leaves the session; a few more updates deliver the `Leave`.
    pub fn leave(&mut self) {
        self.send(Msg::Leave);
        self.state = ClientState::Disconnected(DisconnectReason::Left);
        self.leave_until = Some(self.now + LEAVE_LINGER);
    }

    fn end(&mut self, st: ClientState, ev: ClientEvent) {
        if matches!(
            self.state,
            ClientState::Connecting | ClientState::Connected { .. }
        ) {
            self.state = st;
            self.events.push_back(ev);
        }
    }

    pub fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]) {
        self.now = now;
        if from != self.host {
            self.drops.wrong_addr += 1;
            return;
        }
        let (h, body) = match parse_header(bytes) {
            Ok(x) => x,
            Err(d) => {
                self.drops.count(d);
                if let (DropReason::Protocol(v), Some(id)) = (d, raw_conn_id(bytes)) {
                    if id == self.config.conn_id {
                        let detail = format!("protocol {PROTOCOL}, host {v}");
                        self.end(
                            ClientState::Rejected(RejectReason::Version),
                            ClientEvent::Rejected {
                                reason: RejectReason::Version,
                                detail,
                            },
                        );
                    }
                }
                return;
            }
        };
        if h.conn != self.config.conn_id {
            self.drops.unknown_conn += 1;
            return;
        }
        let Ok(chunks) = parse_chunks(body) else {
            self.drops.malformed += 1;
            return;
        };
        let Some(rest) = self.conn.receive(now, &h, chunks, bytes.len()) else {
            return;
        };
        for c in rest {
            match c {
                Chunk::HostStatus {
                    input_through,
                    depth,
                    starved,
                    slack_us,
                } => {
                    self.take_status(input_through, depth, starved);
                    if slack_us.is_some() {
                        self.slack_us = slack_us;
                    }
                }
                Chunk::Bundles(body) => {
                    if self.take_bundles(now, body).is_err() {
                        self.drops.bad_chunk += 1;
                    }
                }
                _ => self.drops.bad_chunk += 1,
            }
        }
        while let Some(m) = self.conn.take_message() {
            match Msg::decode(&m) {
                Ok(msg) => self.take_message(msg),
                Err(_) => self.drops.bad_message += 1,
            }
        }
        self.deliver();
    }

    fn take_status(&mut self, through: u32, depth: u8, starved: u32) {
        if through > self.input_acked && through < self.next_input {
            while let Some(&(seq, pad, _)) = self.inputs.front() {
                if seq > through {
                    break;
                }
                self.acked_pad = pad;
                self.inputs.pop_front();
            }
            self.input_acked = through;
        }
        self.depth = depth;
        self.starved = starved;
    }

    fn take_bundles(&mut self, now: Micros, body: &[u8]) -> Result<(), ()> {
        let (head, r) = decode_bundles_head(body).map_err(|_| ())?;
        if head.stream != self.stream || self.stream == 0 {
            return Ok(());
        }
        if head.first_tick < self.log_len {
            return Err(());
        }
        let empty = Vec::new();
        let base = if head.has_base {
            let prev = head.first_tick.checked_sub(1);
            let Some(b) = prev.and_then(|t| self.bases.get(&t)) else {
                return Err(());
            };
            b
        } else {
            &empty
        };
        let bundles = decode_bundles_rest(r, head.first_tick, base).map_err(|_| ())?;
        for b in bundles {
            if b.tick < self.next_needed || self.live.contains_key(&b.tick) {
                continue;
            }
            let offset = now.0 as i64 - (u64::from(b.tick) * 1_000_000 / 30) as i64;
            if let Some(prev) = self.last_offset {
                let d = (offset - prev).abs() as f64;
                self.jitter_us += (d - self.jitter_us) / 16.0;
            }
            self.last_offset = Some(offset);
            self.bases.insert(b.tick, b.pads.clone());
            self.live.insert(b.tick, b);
        }
        let before = self.next_needed;
        while self.bases.contains_key(&self.next_needed) {
            self.next_needed += 1;
        }
        if self.next_needed != before {
            self.status_dirty = true;
        }
        let keep_from = self.next_needed.saturating_sub(BASES_KEPT);
        while self
            .bases
            .first_key_value()
            .is_some_and(|(t, _)| *t < keep_from)
        {
            self.bases.pop_first();
        }
        Ok(())
    }

    fn take_message(&mut self, msg: Msg) {
        match msg {
            Msg::Welcome { slot, max_players } => {
                if self.state == ClientState::Connecting {
                    self.state = ClientState::Connected { slot };
                    self.events
                        .push_back(ClientEvent::Welcome { slot, max_players });
                }
            }
            Msg::Reject { reason, detail } => self.end(
                ClientState::Rejected(reason),
                ClientEvent::Rejected { reason, detail },
            ),
            Msg::Lobby { version, state } => {
                self.events.push_back(ClientEvent::Lobby { version, state })
            }
            Msg::LoadRound {
                round_id,
                stream,
                joining,
                log_len,
                setup,
                roster,
            } => {
                self.round_id = round_id;
                self.stream = stream;
                self.log_len = log_len;
                self.next_needed = log_len;
                self.next_deliver = 0;
                self.awaiting_start = !joining;
                self.loading = true;
                self.log.clear();
                self.live.clear();
                self.bases.clear();
                self.last_offset = None;
                self.status_dirty = true;
                self.events.push_back(ClientEvent::LoadRound {
                    round_id,
                    joining,
                    log_len,
                    setup,
                    roster,
                });
            }
            Msg::LogChunk {
                stream,
                first_tick,
                count,
                data,
            } => {
                let fits = first_tick
                    .checked_add(count)
                    .is_some_and(|end| end <= self.log_len);
                if stream != self.stream || !fits {
                    self.drops.bad_message += 1;
                    return;
                }
                match decompress_log(&data, first_tick, count as usize) {
                    Ok(bundles) => {
                        for b in bundles {
                            self.log.insert(b.tick, b);
                        }
                    }
                    Err(_) => self.drops.bad_message += 1,
                }
            }
            Msg::Start { round_id } => {
                if round_id == self.round_id && self.awaiting_start {
                    self.awaiting_start = false;
                    self.events.push_back(ClientEvent::Start { round_id });
                }
            }
            Msg::Desync {
                tick,
                host_hash,
                build,
                target,
            } => self.events.push_back(ClientEvent::Desync {
                tick,
                host_hash,
                host_build: build,
                host_target: target,
            }),
            Msg::Leave => self.end(
                ClientState::Disconnected(DisconnectReason::HostLeft),
                ClientEvent::Disconnected(DisconnectReason::HostLeft),
            ),
            Msg::Notice(b) => self.events.push_back(ClientEvent::Notice(b)),
            Msg::Chat { slot, text } => self.events.push_back(ClientEvent::Chat { slot, text }),
            _ => self.drops.bad_message += 1,
        }
    }

    fn deliver(&mut self) {
        if self.awaiting_start || self.stream == 0 {
            return;
        }
        loop {
            let t = self.next_deliver;
            let b = if t < self.log_len {
                self.log.remove(&t)
            } else {
                self.live.remove(&t)
            };
            let Some(b) = b else {
                return;
            };
            self.events.push_back(ClientEvent::Bundle(b));
            self.next_deliver += 1;
        }
    }

    /// Advances timers; returns the datagrams to send.
    pub fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        self.now = now;
        match self.state {
            ClientState::Connecting | ClientState::Connected { .. } => {
                let silence = self.conn.silence(now);
                let limit = if self.loading {
                    DROP_AFTER_LOADING
                } else {
                    DROP_AFTER
                };
                if silence >= limit {
                    self.end(
                        ClientState::Disconnected(DisconnectReason::Timeout),
                        ClientEvent::Disconnected(DisconnectReason::Timeout),
                    );
                    return Vec::new();
                }
                let trouble = silence >= TROUBLE_AFTER;
                if trouble != self.trouble {
                    self.trouble = trouble;
                    self.events.push_back(ClientEvent::Trouble(trouble));
                }
            }
            ClientState::Disconnected(DisconnectReason::Left) => {
                let lingering = self.leave_until.is_some_and(|t| now < t);
                if !lingering || self.conn.reliable_idle() {
                    return Vec::new();
                }
            }
            _ => return Vec::new(),
        }
        if !(self.inputs_dirty || self.status_dirty || self.conn.wants_send(now, HEARTBEAT)) {
            return Vec::new();
        }
        let status = (self.stream != 0).then_some((self.stream, self.next_needed));
        let pads: Vec<(PadState, u32)> = self.inputs.iter().map(|(_, p, v)| (*p, *v)).collect();
        let (acked, acked_pad) = (self.input_acked, self.acked_pad);
        let packets = self.conn.build(now, |pw| {
            if let Some((stream, next)) = status {
                pw.client_status(stream, next);
            }
            if !pads.is_empty() {
                let budget = pw.room().saturating_sub(CHUNK_OVERHEAD + 128);
                let (body, n) = encode_inputs(acked, &acked_pad, &pads, budget);
                if n > 0 {
                    pw.inputs(&body);
                }
            }
        });
        self.inputs_dirty = false;
        self.status_dirty = false;
        packets.into_iter().map(|p| (self.host, p)).collect()
    }

    pub fn stats(&self) -> ClientStats {
        ClientStats {
            rtt_ms: self.conn.rtt.srtt as f64 / 1e3,
            rttvar_ms: self.conn.rtt.rttvar as f64 / 1e3,
            queue_depth: self.depth,
            starved: self.starved,
            slack_us: self.slack_us,
            inputs_sent: self.next_input.saturating_sub(1),
            jitter_ticks: self.jitter_ticks(),
            unacked_inputs: self.inputs.len(),
            link: self.conn.stats,
            drops: self.drops,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile bundles chunk claiming a base before tick 0 is a bad chunk,
    /// not an underflow.
    #[test]
    fn client_rejects_a_base_before_tick_zero() {
        let host: SocketAddr = "10.0.0.1:1".parse().unwrap();
        let mut c = ClientCore::new(ClientConfig::new("b", "Guest", 1), host, Micros::ZERO);
        c.stream = 3;
        c.log_len = 0;
        let bundle = TickBundle {
            tick: 0,
            pads: vec![(0, PadState::default())],
            commands: Vec::new(),
            seqs: vec![0],
        };
        let (body, n) = crate::packet::encode_bundles(3, 0, Some(&[]), &[bundle], 1000);
        assert_eq!(n, 1);
        assert_eq!(c.take_bundles(Micros(10), &body), Err(()));
        assert!(c.live.is_empty());
    }
}
