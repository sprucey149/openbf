//! One connection's link state: packet sequence and acks, RTT, and the
//! reliable ordered fragmented channel.
//!
//! Rules:
//! - Each datagram gets the next 16-bit sequence. The receiver acks the
//!   newest sequence it has seen plus a 32-bit field for the 32 before it.
//!   A duplicate or one more than 32 behind the newest is dropped whole.
//!   The header's ack fields count only when the packet carries an ack-delay
//!   chunk; a peer that has received nothing yet sends none.
//! - RTT: each newly acked newest packet gives a sample (minus the peer's
//!   reported ack delay); EWMA mean gain 1/8, deviation gain 1/4.
//! - Reliable messages split into units of at most [`FRAGMENT`] bytes, each
//!   with a 32-bit id; the receiver delivers units in id order and joins them
//!   into messages at a unit marked last. A unit is resent when unacked after
//!   RTT + 4·RTTVAR (at least [`MIN_RTO`]), doubled for each resend of that
//!   unit up to [`MAX_BACKOFF`] (a dead or swamped link is not flooded).
//! - At most [`MAX_IN_FLIGHT`] units from the oldest unacked one are in
//!   flight (about 38 KB): a big transfer (a mid-round join's log) is paced
//!   by its acks instead of queueing a quarter megabyte at the sender's
//!   uplink in front of everyone's live bundles. The receiver buffers up to
//!   [`RECV_WINDOW`] ahead, which the flight can never exceed.
//! - Packet-level acks are the only acks: a unit is delivered once a packet
//!   that carried it is acked.

use crate::packet::{seq_newer, Chunk, Header, PacketWriter, CHUNK_OVERHEAD, PROTOCOL};
use crate::Micros;
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

/// Largest reliable unit payload.
pub const FRAGMENT: usize = 1024;
/// Reliable units in flight from the oldest unacked one (module rules).
pub const MAX_IN_FLIGHT: usize = 32;
const _: () = assert!(MAX_IN_FLIGHT as u32 <= RECV_WINDOW);
pub const RECV_WINDOW: u32 = 1024;
/// Shortest resend timeout.
pub const MIN_RTO: Micros = Micros(50_000);
/// The longest a resend backoff grows (unless the RTO itself is longer).
pub const MAX_BACKOFF: Micros = Micros::from_secs(1);
/// The largest reliable message (a reassembled one past it breaks the link).
pub const MAX_MESSAGE: usize = 4 << 20;
/// Extra reliable-only datagrams one update may send per connection.
pub const MAX_EXTRA_PACKETS: usize = 8;
const SENT_RING: usize = 1024;

/// RTT estimator (microseconds).
#[derive(Clone, Copy, Debug)]
pub struct Rtt {
    pub srtt: i64,
    pub rttvar: i64,
    pub samples: u32,
}

impl Default for Rtt {
    fn default() -> Self {
        Self {
            srtt: 100_000,
            rttvar: 50_000,
            samples: 0,
        }
    }
}

impl Rtt {
    pub fn sample(&mut self, us: i64) {
        let us = us.max(0);
        if self.samples == 0 {
            self.srtt = us;
            self.rttvar = us / 2;
        } else {
            let err = us - self.srtt;
            self.srtt += err / 8;
            self.rttvar += (err.abs() - self.rttvar) / 4;
        }
        self.samples = self.samples.saturating_add(1);
    }

    /// The resend timeout.
    pub fn rto(&self) -> Micros {
        Micros(((self.srtt + 4 * self.rttvar).max(0) as u64).max(MIN_RTO.0))
    }
}

/// Link counters for the net meter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ConnStats {
    pub packets_sent: u64,
    pub packets_recv: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    /// Packets that fell out of the ack window unacknowledged.
    pub packets_lost: u64,
    pub duplicates: u64,
    pub resends: u64,
}

struct Unit {
    id: u32,
    last: bool,
    data: Vec<u8>,
    sent_at: Option<Micros>,
    /// Times sent (the resend backoff).
    sends: u32,
    acked: bool,
}

impl Unit {
    /// Whether this unit goes out now: never sent, or unacked past its
    /// backed-off resend timeout.
    fn due(&self, now: Micros, rto: Micros) -> bool {
        if self.acked {
            return false;
        }
        let Some(t) = self.sent_at else {
            return true;
        };
        let doubling = 1u64 << self.sends.saturating_sub(1).min(16);
        let wait = rto.0.saturating_mul(doubling).min(rto.0.max(MAX_BACKOFF.0));
        now.0 >= t.0 + wait
    }
}

struct Sent {
    seq: u16,
    at: Micros,
    units: Vec<u32>,
    acked: bool,
}

/// A connection to one peer.
pub struct Conn {
    pub id: u32,
    pub addr: SocketAddr,
    next_seq: u16,
    recv_any: bool,
    recv_latest: u16,
    recv_bits: u32,
    recv_latest_at: Micros,
    sent: Vec<Option<Sent>>,
    acked_latest: Option<u16>,
    pub rtt: Rtt,
    units: VecDeque<Unit>,
    next_unit: u32,
    recv_next: u32,
    recv_buf: BTreeMap<u32, (bool, Vec<u8>)>,
    partial: Vec<u8>,
    messages: VecDeque<Vec<u8>>,
    /// The peer sent a message past [`MAX_MESSAGE`].
    pub broken: bool,
    pub last_recv: Micros,
    pub last_send: Micros,
    /// A received packet carried reliable data and wants an ack soon.
    pub ack_owed: bool,
    pub stats: ConnStats,
}

impl Conn {
    pub fn new(id: u32, addr: SocketAddr, now: Micros) -> Self {
        let mut sent = Vec::with_capacity(SENT_RING);
        sent.resize_with(SENT_RING, || None);
        Self {
            id,
            addr,
            next_seq: 0,
            recv_any: false,
            recv_latest: 0,
            recv_bits: 0,
            recv_latest_at: now,
            sent,
            acked_latest: None,
            rtt: Rtt::default(),
            units: VecDeque::new(),
            next_unit: 0,
            recv_next: 0,
            recv_buf: BTreeMap::new(),
            partial: Vec::new(),
            messages: VecDeque::new(),
            broken: false,
            last_recv: now,
            last_send: Micros(0),
            ack_owed: false,
            stats: ConnStats::default(),
        }
    }

    /// Queues a reliable message.
    pub fn send_reliable(&mut self, msg: &[u8]) {
        let mut parts = msg.chunks(FRAGMENT).peekable();
        if parts.peek().is_none() {
            self.push_unit(Vec::new(), true);
            return;
        }
        while let Some(p) = parts.next() {
            let last = parts.peek().is_none();
            self.push_unit(p.to_vec(), last);
        }
    }

    fn push_unit(&mut self, data: Vec<u8>, last: bool) {
        self.units.push_back(Unit {
            id: self.next_unit,
            last,
            data,
            sent_at: None,
            sends: 0,
            acked: false,
        });
        self.next_unit = self.next_unit.wrapping_add(1);
    }

    /// Reliable messages received, in order.
    pub fn take_message(&mut self) -> Option<Vec<u8>> {
        self.messages.pop_front()
    }

    /// Whether every queued reliable unit has been acknowledged.
    pub fn reliable_idle(&self) -> bool {
        self.units.is_empty()
    }

    /// Reliable units not yet acknowledged.
    pub fn reliable_pending(&self) -> usize {
        self.units.len()
    }

    /// Takes a datagram's header and chunks. Returns the chunks the session
    /// core handles (reliable ones are consumed here), or `None` for a
    /// duplicate or stale packet.
    pub fn receive<'a>(
        &mut self,
        now: Micros,
        h: &Header,
        chunks: Vec<Chunk<'a>>,
        datagram_len: usize,
    ) -> Option<Vec<Chunk<'a>>> {
        if !self.note_seq(h.seq, now) {
            self.stats.duplicates += 1;
            return None;
        }
        self.stats.packets_recv += 1;
        self.stats.bytes_recv += datagram_len as u64;
        self.last_recv = now;
        // The header's ack is meaningful only with an ack-delay chunk: a peer
        // that has received nothing yet sends neither.
        let delay = chunks.iter().find_map(|c| match c {
            Chunk::AckDelay(d) => Some(i64::from(*d) * 100),
            _ => None,
        });
        if let Some(delay) = delay {
            self.process_acks(now, h.ack, h.ack_bits, delay);
        }
        let mut rest = Vec::with_capacity(chunks.len());
        for c in chunks {
            match c {
                Chunk::Reliable { unit, last, data } => {
                    self.ack_owed = true;
                    self.take_unit(unit, last, data);
                }
                Chunk::AckDelay(_) => {}
                other => rest.push(other),
            }
        }
        Some(rest)
    }

    fn note_seq(&mut self, seq: u16, now: Micros) -> bool {
        if !self.recv_any {
            self.recv_any = true;
            self.recv_latest = seq;
            self.recv_bits = 0;
            self.recv_latest_at = now;
            return true;
        }
        if seq_newer(seq, self.recv_latest) {
            let d = u32::from(seq.wrapping_sub(self.recv_latest));
            self.recv_bits = if d > 32 {
                0
            } else {
                ((u64::from(self.recv_bits) << d) | (1u64 << (d - 1))) as u32
            };
            self.recv_latest = seq;
            self.recv_latest_at = now;
            true
        } else {
            let d = u32::from(self.recv_latest.wrapping_sub(seq));
            if d == 0 || d > 32 {
                return false;
            }
            let bit = 1u32 << (d - 1);
            if self.recv_bits & bit != 0 {
                return false;
            }
            self.recv_bits |= bit;
            true
        }
    }

    fn process_acks(&mut self, now: Micros, ack: u16, bits: u32, delay_us: i64) {
        let newest_is_new = match self.acked_latest {
            None => true,
            Some(prev) => seq_newer(ack, prev),
        };
        if newest_is_new {
            // Packets falling out of the 33-packet window unacked are lost.
            if let Some(prev) = self.acked_latest {
                let steps = ack.wrapping_sub(prev).min(64);
                for i in 0..steps {
                    let s = prev.wrapping_sub(32).wrapping_add(i);
                    if let Some(rec) = &self.sent[usize::from(s) % SENT_RING] {
                        if rec.seq == s && !rec.acked {
                            self.stats.packets_lost += 1;
                        }
                    }
                }
            }
            self.acked_latest = Some(ack);
        }
        for i in 0..=32u16 {
            let s = ack.wrapping_sub(i);
            if i > 0 && bits & (1 << (i - 1)) == 0 {
                continue;
            }
            let idx = usize::from(s) % SENT_RING;
            let Some(rec) = self.sent[idx].as_mut() else {
                continue;
            };
            if rec.seq != s || rec.acked {
                continue;
            }
            rec.acked = true;
            if i == 0 && newest_is_new {
                let sample = now.0 as i64 - rec.at.0 as i64 - delay_us;
                self.rtt.sample(sample);
            }
            let units = std::mem::take(&mut rec.units);
            for u in units {
                if let Some(front) = self.units.front() {
                    let off = u.wrapping_sub(front.id) as usize;
                    if off < self.units.len() {
                        self.units[off].acked = true;
                    }
                }
            }
        }
        while self.units.front().is_some_and(|u| u.acked) {
            self.units.pop_front();
        }
    }

    fn take_unit(&mut self, id: u32, last: bool, data: &[u8]) {
        let ahead = id.wrapping_sub(self.recv_next);
        if ahead >= RECV_WINDOW {
            return;
        }
        self.recv_buf
            .entry(id)
            .or_insert_with(|| (last, data.to_vec()));
        while let Some((last, data)) = self.recv_buf.remove(&self.recv_next) {
            self.recv_next = self.recv_next.wrapping_add(1);
            if self.partial.len() + data.len() > MAX_MESSAGE {
                self.broken = true;
                self.partial.clear();
                continue;
            }
            self.partial.extend_from_slice(&data);
            if last {
                self.messages.push_back(std::mem::take(&mut self.partial));
            }
        }
    }

    fn reliable_due(&self, now: Micros, rto: Micros) -> bool {
        self.units
            .iter()
            .take(MAX_IN_FLIGHT)
            .any(|u| u.due(now, rto))
    }

    /// Whether a datagram should go out now even with no stream data: an
    /// ack is owed, a reliable unit is due, or the heartbeat interval passed.
    pub fn wants_send(&self, now: Micros, heartbeat: Micros) -> bool {
        self.ack_owed
            || now.0 >= self.last_send.0 + heartbeat.0
            || self.reliable_due(now, self.rtt.rto())
    }

    /// Builds this update's datagrams. `fill` writes the session core's
    /// stream chunks into the first one; reliable units fill the rest of it
    /// and up to [`MAX_EXTRA_PACKETS`] more.
    pub fn build(&mut self, now: Micros, fill: impl FnOnce(&mut PacketWriter)) -> Vec<Vec<u8>> {
        let rto = self.rtt.rto();
        let mut out = Vec::new();
        let mut pw = self.start_packet(now);
        fill(&mut pw);
        let mut units = Vec::new();
        self.fill_reliable(now, rto, &mut pw, &mut units);
        out.push(self.finish_packet(now, pw, units));
        for _ in 0..MAX_EXTRA_PACKETS {
            if !self.reliable_due(now, rto) {
                break;
            }
            let mut pw = self.start_packet(now);
            let mut units = Vec::new();
            self.fill_reliable(now, rto, &mut pw, &mut units);
            if units.is_empty() {
                break;
            }
            out.push(self.finish_packet(now, pw, units));
        }
        self.ack_owed = false;
        self.last_send = now;
        out
    }

    fn start_packet(&mut self, now: Micros) -> PacketWriter {
        let h = Header {
            protocol: PROTOCOL,
            conn: self.id,
            seq: self.next_seq,
            ack: self.recv_latest,
            ack_bits: self.recv_bits,
        };
        let mut pw = PacketWriter::new(&h);
        if self.recv_any {
            let d = (now.0.saturating_sub(self.recv_latest_at.0) / 100).min(u64::from(u16::MAX));
            pw.ack_delay(d as u16);
        }
        pw
    }

    fn fill_reliable(
        &mut self,
        now: Micros,
        rto: Micros,
        pw: &mut PacketWriter,
        units: &mut Vec<u32>,
    ) {
        for u in self.units.iter_mut().take(MAX_IN_FLIGHT) {
            if !u.due(now, rto) {
                continue;
            }
            let need = u.data.len() + 6 + CHUNK_OVERHEAD;
            if pw.room() < need {
                if pw.room() < 64 {
                    break;
                }
                continue;
            }
            if u.sent_at.is_some() {
                self.stats.resends += 1;
            }
            pw.reliable(u.id, u.last, &u.data);
            u.sent_at = Some(now);
            u.sends = u.sends.saturating_add(1);
            units.push(u.id);
        }
    }

    fn finish_packet(&mut self, now: Micros, pw: PacketWriter, units: Vec<u32>) -> Vec<u8> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        let idx = usize::from(seq) % SENT_RING;
        self.sent[idx] = Some(Sent {
            seq,
            at: now,
            units,
            acked: false,
        });
        let bytes = pw.finish();
        self.stats.packets_sent += 1;
        self.stats.bytes_sent += bytes.len() as u64;
        bytes
    }

    /// Microseconds since the peer was last heard.
    pub fn silence(&self, now: Micros) -> Micros {
        Micros(now.0.saturating_sub(self.last_recv.0))
    }
}
