//! Datagram layout: the fixed header and the chunks after it.
//!
//! ```text
//! "OBF1" | protocol u16 | connection id u32 | seq u16 | ack u16 | ack bits u32
//! then chunks: kind u8 | body length varint | body
//! ```
//!
//! Rules:
//! - A datagram is at most [`MAX_DATAGRAM`] bytes (fits a 1280-byte tunnel
//!   MTU). Writers budget against it; readers drop anything longer.
//! - The header layout never changes between protocol versions, so a peer
//!   can always tell a version mismatch from noise.
//! - Connection id 0 is discovery (`Find`/`Found`); every other id is a
//!   connection the client chose at random.
//! - A datagram whose framing does not parse, or that holds an unknown chunk
//!   kind, is dropped whole and counted. Chunk bodies whose meaning depends on
//!   state (inputs, bundles) are decoded later by the session core, which
//!   drops just that chunk if it fails.

use crate::bundle::{read_chain, write_chain, TickBundle};
use crate::wire::{Reader, WireError, WireResult, Writer};
use core::sim::PadState;

pub const MAGIC: [u8; 4] = *b"OBF1";
/// The protocol version. Bump on any wire change. 2: inputs carry their
/// view tick and bundle pads their `view_lag` (lag compensation). 3: the
/// host status's slack. 4: bundle pads name the input they consumed. 5: desync
/// reports (`Desync` carries the host's hash, build and target; a guest
/// answers with `DesyncHashes`).
pub const PROTOCOL: u16 = 5;
/// The largest datagram either side sends or accepts.
pub const MAX_DATAGRAM: usize = 1200;
pub const HEADER_LEN: usize = 18;
/// The most bytes one tick bundle takes in a live bundles chunk: what is
/// left of a datagram after the header, the ack delay (4), the largest host
/// status (24), the chunk framing and the bundles head (12). The host keeps
/// every bundle within it (`bundle` rules), so the first bundle of every
/// live packet fits.
pub const BUNDLE_BUDGET: usize = MAX_DATAGRAM - HEADER_LEN - 4 - 24 - CHUNK_OVERHEAD - 12;
/// The default UDP port for hosting and discovery.
pub const DEFAULT_PORT: u16 = 37450;

const K_ACK_DELAY: u8 = 1;
const K_HOST_STATUS: u8 = 2;
const K_CLIENT_STATUS: u8 = 3;
const K_INPUTS: u8 = 4;
const K_BUNDLES: u8 = 5;
const K_RELIABLE: u8 = 6;
const K_FIND: u8 = 7;
const K_FOUND: u8 = 8;

/// The fixed packet header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub protocol: u16,
    pub conn: u32,
    pub seq: u16,
    pub ack: u16,
    pub ack_bits: u32,
}

impl Header {
    pub fn write(&self, w: &mut Writer) {
        w.raw(&MAGIC);
        w.u16(self.protocol);
        w.u32(self.conn);
        w.u16(self.seq);
        w.u16(self.ack);
        w.u32(self.ack_bits);
    }
}

/// Why a datagram was dropped before any session saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropReason {
    TooLong,
    BadMagic,
    /// A peer on another protocol version (its version given).
    Protocol(u16),
    Malformed,
}

/// Reads and checks the header; returns it and the chunk bytes.
pub fn parse_header(bytes: &[u8]) -> Result<(Header, &[u8]), DropReason> {
    if bytes.len() > MAX_DATAGRAM {
        return Err(DropReason::TooLong);
    }
    if bytes.len() < HEADER_LEN {
        return Err(if bytes.starts_with(&MAGIC[..bytes.len().min(4)]) {
            DropReason::Malformed
        } else {
            DropReason::BadMagic
        });
    }
    if bytes[..4] != MAGIC {
        return Err(DropReason::BadMagic);
    }
    let mut r = Reader::new(&bytes[4..HEADER_LEN]);
    let h = (|| -> WireResult<Header> {
        Ok(Header {
            protocol: r.u16()?,
            conn: r.u32()?,
            seq: r.u16()?,
            ack: r.u16()?,
            ack_bits: r.u32()?,
        })
    })()
    .map_err(|_| DropReason::Malformed)?;
    if h.protocol != PROTOCOL {
        return Err(DropReason::Protocol(h.protocol));
    }
    Ok((h, &bytes[HEADER_LEN..]))
}

/// One parsed chunk. Stream bodies stay raw until the core has their base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chunk<'a> {
    /// Time between receiving the packet named by `ack` and sending this
    /// one, in 100 µs units.
    AckDelay(u16),
    HostStatus {
        input_through: u32,
        depth: u8,
        starved: u32,
        /// The slot's published input slack (µs, `scheduler`).
        slack_us: Option<i64>,
    },
    ClientStatus {
        stream: u32,
        next_needed: u32,
    },
    Inputs(&'a [u8]),
    Bundles(&'a [u8]),
    Reliable {
        unit: u32,
        last: bool,
        data: &'a [u8],
    },
    Find {
        nonce: u64,
    },
    Found {
        nonce: u64,
        info: &'a [u8],
    },
}

/// Splits the chunk bytes into chunks; any framing error drops them all.
pub fn parse_chunks(mut body: &[u8]) -> WireResult<Vec<Chunk<'_>>> {
    let mut out = Vec::new();
    while !body.is_empty() {
        let mut r = Reader::new(body);
        let kind = r.u8()?;
        let len = r.var()?;
        if len > r.remaining() as u64 {
            return Err(WireError::Short);
        }
        let data = r.raw(len as usize)?;
        body = &body[body.len() - r.remaining()..];
        let mut c = Reader::new(data);
        let chunk = match kind {
            K_ACK_DELAY => Chunk::AckDelay(c.u16()?),
            K_HOST_STATUS => Chunk::HostStatus {
                input_through: c.var32()?,
                depth: c.u8()?,
                starved: c.var32()?,
                slack_us: match c.u8()? {
                    0 => None,
                    1 => Some(c.ivar()?.clamp(-10_000_000, 10_000_000)),
                    _ => return Err(WireError::BadValue),
                },
            },
            K_CLIENT_STATUS => Chunk::ClientStatus {
                stream: c.var32()?,
                next_needed: c.var32()?,
            },
            K_INPUTS => {
                out.push(Chunk::Inputs(data));
                continue;
            }
            K_BUNDLES => {
                out.push(Chunk::Bundles(data));
                continue;
            }
            K_RELIABLE => {
                let unit = c.var32()?;
                let last = match c.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(WireError::BadValue),
                };
                let n = c.remaining();
                Chunk::Reliable {
                    unit,
                    last,
                    data: c.raw(n)?,
                }
            }
            K_FIND => Chunk::Find { nonce: c.u64()? },
            K_FOUND => {
                let nonce = c.u64()?;
                let n = c.remaining();
                Chunk::Found {
                    nonce,
                    info: c.raw(n)?,
                }
            }
            _ => return Err(WireError::BadValue),
        };
        c.finish()?;
        out.push(chunk);
    }
    Ok(out)
}

/// Builds one datagram within [`MAX_DATAGRAM`].
pub struct PacketWriter {
    w: Writer,
}

/// Framing bytes a chunk costs beyond its body (kind + a 2-byte length).
pub const CHUNK_OVERHEAD: usize = 3;

impl PacketWriter {
    pub fn new(h: &Header) -> Self {
        let mut w = Writer::with_capacity(MAX_DATAGRAM);
        h.write(&mut w);
        Self { w }
    }

    /// Bytes still free.
    pub fn room(&self) -> usize {
        MAX_DATAGRAM.saturating_sub(self.w.len())
    }

    pub fn len(&self) -> usize {
        self.w.len()
    }

    pub fn is_empty(&self) -> bool {
        self.w.len() <= HEADER_LEN
    }

    fn chunk(&mut self, kind: u8, body: &[u8]) {
        self.w.u8(kind);
        self.w.var(body.len() as u64);
        self.w.raw(body);
        debug_assert!(self.w.len() <= MAX_DATAGRAM);
    }

    pub fn ack_delay(&mut self, units: u16) {
        self.chunk(K_ACK_DELAY, &units.to_le_bytes());
    }

    pub fn host_status(
        &mut self,
        input_through: u32,
        depth: u8,
        starved: u32,
        slack_us: Option<i64>,
    ) {
        let mut b = Writer::new();
        b.var(input_through.into());
        b.u8(depth);
        b.var(starved.into());
        match slack_us {
            None => b.u8(0),
            Some(v) => {
                b.u8(1);
                b.ivar(v);
            }
        }
        self.chunk(K_HOST_STATUS, &b.buf);
    }

    pub fn client_status(&mut self, stream: u32, next_needed: u32) {
        let mut b = Writer::new();
        b.var(stream.into());
        b.var(next_needed.into());
        self.chunk(K_CLIENT_STATUS, &b.buf);
    }

    pub fn inputs(&mut self, body: &[u8]) {
        self.chunk(K_INPUTS, body);
    }

    pub fn bundles(&mut self, body: &[u8]) {
        self.chunk(K_BUNDLES, body);
    }

    pub fn reliable(&mut self, unit: u32, last: bool, data: &[u8]) {
        let mut b = Writer::with_capacity(data.len() + 6);
        b.var(unit.into());
        b.u8(u8::from(last));
        b.raw(data);
        self.chunk(K_RELIABLE, &b.buf);
    }

    pub fn find(&mut self, nonce: u64) {
        self.chunk(K_FIND, &nonce.to_le_bytes());
    }

    pub fn found(&mut self, nonce: u64, info: &[u8]) {
        let mut b = Writer::with_capacity(info.len() + 8);
        b.u64(nonce);
        b.raw(info);
        self.chunk(K_FOUND, &b.buf);
    }

    pub fn finish(self) -> Vec<u8> {
        self.w.buf
    }
}

/// Encodes as many of `inputs` (sequences `base_seq+1..`) as fit in
/// `budget` body bytes, each pad coded against the one before (the first
/// against `base`), each followed by its view tick (the confirmed tick its
/// player was viewing: the first absolute, the rest as a signed delta from
/// the one before). Returns the body and how many went in.
pub fn encode_inputs(
    base_seq: u32,
    base: &PadState,
    inputs: &[ViewedPad],
    budget: usize,
) -> (Vec<u8>, usize) {
    let mut w = Writer::new();
    w.var(base_seq.into());
    let count_at = w.len();
    w.u8(0);
    let mut prev = *base;
    let mut prev_view = None;
    let mut n = 0;
    for (p, view) in inputs.iter().take(255) {
        let mark = w.len();
        crate::bundle::write_pad(&mut w, p, &prev);
        match prev_view {
            None => w.var((*view).into()),
            Some(v) => w.ivar(i64::from(*view) - i64::from(v)),
        }
        if w.len() > budget {
            w.buf.truncate(mark);
            break;
        }
        prev = *p;
        prev_view = Some(*view);
        n += 1;
    }
    w.buf[count_at] = n as u8;
    (w.buf, n)
}

/// One input on the wire: the pad and the confirmed tick its player was
/// viewing.
pub type ViewedPad = (PadState, u32);

/// Decodes an inputs body: the base sequence and the pads after it, each
/// with its view tick. `base` looks up the pad of a sequence the receiver
/// holds (`None`: cannot decode).
pub fn decode_inputs(
    body: &[u8],
    base: impl FnOnce(u32) -> Option<PadState>,
) -> WireResult<Option<(u32, Vec<ViewedPad>)>> {
    let mut r = Reader::new(body);
    let base_seq = r.var32()?;
    let n = usize::from(r.u8()?);
    let Some(mut prev) = base(base_seq) else {
        return Ok(None);
    };
    let mut out = Vec::with_capacity(n);
    let mut view: Option<u32> = None;
    for _ in 0..n {
        prev = crate::bundle::read_pad(&mut r, &prev)?;
        let v = match view {
            None => r.var32()?,
            Some(v) => u32::try_from(i64::from(v) + r.ivar()?).map_err(|_| WireError::BadValue)?,
        };
        view = Some(v);
        out.push((prev, v));
    }
    r.finish()?;
    Ok(Some((base_seq, out)))
}

/// Encodes as many bundles as fit in `budget` body bytes. `base` is the
/// pads of tick `first_tick - 1` when the receiver holds it.
pub fn encode_bundles(
    stream: u32,
    first_tick: u32,
    base: Option<&[(u8, PadState)]>,
    bundles: &[TickBundle],
    budget: usize,
) -> (Vec<u8>, usize) {
    let mut w = Writer::new();
    w.var(stream.into());
    w.var(first_tick.into());
    w.u8(u8::from(base.is_some()));
    let count_at = w.len();
    w.u8(0);
    let mut prev: &[(u8, PadState)] = base.unwrap_or(&[]);
    let mut n = 0;
    for b in bundles.iter().take(255) {
        let mark = w.len();
        write_chain(&mut w, std::slice::from_ref(b), prev);
        if w.len() > budget {
            w.buf.truncate(mark);
            break;
        }
        prev = &b.pads;
        n += 1;
    }
    w.buf[count_at] = n as u8;
    (w.buf, n)
}

/// The fixed part of a bundles body.
pub struct BundlesHead {
    pub stream: u32,
    pub first_tick: u32,
    pub has_base: bool,
}

/// Reads a bundles body's head; then [`decode_bundles_rest`] with the base.
pub fn decode_bundles_head(body: &[u8]) -> WireResult<(BundlesHead, Reader<'_>)> {
    let mut r = Reader::new(body);
    let stream = r.var32()?;
    let first_tick = r.var32()?;
    let has_base = match r.u8()? {
        0 => false,
        1 => true,
        _ => return Err(WireError::BadValue),
    };
    Ok((
        BundlesHead {
            stream,
            first_tick,
            has_base,
        },
        r,
    ))
}

pub fn decode_bundles_rest(
    mut r: Reader<'_>,
    first_tick: u32,
    base: &[(u8, PadState)],
) -> WireResult<Vec<TickBundle>> {
    let n = usize::from(r.u8()?);
    let out = read_chain(&mut r, first_tick, n, base)?;
    r.finish()?;
    Ok(out)
}

/// Whether sequence `a` is newer than `b` (wrapping).
pub fn seq_newer(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) < 0x8000
}

/// Datagrams and chunks a session dropped, by cause.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct DropCounts {
    pub bad_magic: u64,
    pub wrong_protocol: u64,
    pub malformed: u64,
    pub unknown_conn: u64,
    pub wrong_addr: u64,
    /// Stream chunks (inputs, bundles) that failed to decode or lacked a base.
    pub bad_chunk: u64,
    /// Reliable messages that failed to decode or came at the wrong time.
    pub bad_message: u64,
}

impl DropCounts {
    pub fn count(&mut self, d: DropReason) {
        match d {
            DropReason::BadMagic => self.bad_magic += 1,
            DropReason::Protocol(_) => self.wrong_protocol += 1,
            DropReason::TooLong | DropReason::Malformed => self.malformed += 1,
        }
    }

    pub fn total(&self) -> u64 {
        self.bad_magic
            + self.wrong_protocol
            + self.malformed
            + self.unknown_conn
            + self.wrong_addr
            + self.bad_chunk
            + self.bad_message
    }
}

/// The connection id of a datagram whose header is otherwise unusable (for
/// answering a peer on another protocol version).
pub fn raw_conn_id(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < HEADER_LEN || bytes[..4] != MAGIC {
        return None;
    }
    Some(u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]))
}

/// A header-only datagram: the version answer and the base of discovery.
pub fn bare_packet(conn: u32) -> PacketWriter {
    PacketWriter::new(&Header {
        protocol: PROTOCOL,
        conn,
        seq: 0,
        ack: 0,
        ack_bits: 0,
    })
}
