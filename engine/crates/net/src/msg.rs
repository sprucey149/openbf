//! Messages carried by the reliable ordered channel (`reliable.rs`).
//!
//! Rules:
//! - One tag byte, then the fields. Unknown tags, out-of-range enums and
//!   lengths over each field's limit fail the decode; the receiver counts and
//!   drops the message.
//! - Game data stays opaque here (`setup`, `content`, `profile`, lobby state,
//!   requests, host messages): the `net` crate never interprets it.
//! - `stream` numbers a catch-up stream per peer: every `LoadRound` the host
//!   sends a peer starts a new one, and bundles and log chunks of an older
//!   stream are ignored.

use crate::wire::{Reader, WireError, WireResult, Writer};

/// The longest player name, in characters.
pub const MAX_NAME_CHARS: usize = 24;
pub const MAX_BUILD: usize = 128;
pub const MAX_CONTENT: usize = 64 << 10;
pub const MAX_PROFILE: usize = 4 << 10;
pub const MAX_LOBBY: usize = 64 << 10;
pub const MAX_SETUP: usize = 1 << 20;
pub const MAX_LOG_CHUNK: usize = 1 << 20;
pub const MAX_REQUEST: usize = 4 << 10;
pub const MAX_NOTICE: usize = 64 << 10;
pub const MAX_CHAT: usize = 512;
pub const MAX_DETAIL: usize = 256;
/// The longest OS/CPU string a desync message carries.
pub const MAX_TARGET: usize = 64;
/// The most per-tick rows a [`Msg::DesyncHashes`] carries.
pub const MAX_DESYNC_ROWS: usize = 128;
/// The most values in one row of a [`Msg::DesyncHashes`].
pub const MAX_DESYNC_WIDTH: u8 = 8;
/// Ticks covered by one compressed log chunk.
pub const LOG_CHUNK_TICKS: u32 = 2048;

/// Why the host refused (or ended) a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum RejectReason {
    /// Protocol or build differ; the detail says which.
    Version,
    Full,
    /// A round is loading; try again shortly.
    Busy,
    /// Kicked and banned for this session.
    Banned,
    BadName,
    /// Kicked by the host (not banned).
    Kicked,
}

impl RejectReason {
    fn tag(self) -> u8 {
        match self {
            RejectReason::Version => 0,
            RejectReason::Full => 1,
            RejectReason::Busy => 2,
            RejectReason::Banned => 3,
            RejectReason::BadName => 4,
            RejectReason::Kicked => 5,
        }
    }

    fn from_tag(t: u8) -> WireResult<Self> {
        Ok(match t {
            0 => RejectReason::Version,
            1 => RejectReason::Full,
            2 => RejectReason::Busy,
            3 => RejectReason::Banned,
            4 => RejectReason::BadName,
            5 => RejectReason::Kicked,
            _ => return Err(WireError::BadValue),
        })
    }
}

/// A human in a round's roster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterEntry {
    pub slot: u8,
    pub name: String,
    pub profile: Vec<u8>,
}

/// A reliable-channel message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    Hello {
        protocol: u16,
        build: String,
        name: String,
        token: u64,
        content: Vec<u8>,
        profile: Vec<u8>,
    },
    Welcome {
        slot: u8,
        max_players: u8,
    },
    Reject {
        reason: RejectReason,
        detail: String,
    },
    Lobby {
        version: u32,
        state: Vec<u8>,
    },
    /// Build the round. `log_len` closed bundles (ticks `0..log_len`) follow
    /// as `LogChunk`s; live bundles of `stream` start at `log_len`.
    LoadRound {
        round_id: u32,
        stream: u32,
        joining: bool,
        log_len: u32,
        setup: Vec<u8>,
        roster: Vec<RosterEntry>,
    },
    LogChunk {
        stream: u32,
        first_tick: u32,
        count: u32,
        data: Vec<u8>,
    },
    Loaded {
        round_id: u32,
        static_digest: u64,
        hash0: u64,
    },
    Start {
        round_id: u32,
    },
    HashReport {
        tick: u32,
        hash: u64,
    },
    /// The host's hash differed from this guest's at `tick` (LAN §4.7);
    /// `build` and `target` (OS and CPU) are the host's, for the report.
    Desync {
        tick: u32,
        host_hash: u64,
        build: String,
        target: String,
    },
    Leave,
    Ready {
        round_id: u32,
    },
    Request(Vec<u8>),
    Notice(Vec<u8>),
    Chat {
        slot: u8,
        text: String,
    },
    CaughtUp {
        round_id: u32,
        tick: u32,
    },
    Resync,
    /// A guest told of a desync at `tick` answers with its per-tick hash
    /// rows for the host's report: `width` values a row (its whole-state
    /// hash, then its state groups), rows for ticks `first_tick..`, at most
    /// [`MAX_DESYNC_ROWS`]. `build` and `target` are the guest's.
    DesyncHashes {
        tick: u32,
        build: String,
        target: String,
        first_tick: u32,
        width: u8,
        values: Vec<u64>,
    },
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Msg::Hello {
                protocol,
                build,
                name,
                token,
                content,
                profile,
            } => {
                w.u8(1);
                w.u16(*protocol);
                w.str(build);
                w.str(name);
                w.u64(*token);
                w.bytes(content);
                w.bytes(profile);
            }
            Msg::Welcome { slot, max_players } => {
                w.u8(2);
                w.u8(*slot);
                w.u8(*max_players);
            }
            Msg::Reject { reason, detail } => {
                w.u8(3);
                w.u8(reason.tag());
                w.str(detail);
            }
            Msg::Lobby { version, state } => {
                w.u8(4);
                w.var((*version).into());
                w.bytes(state);
            }
            Msg::LoadRound {
                round_id,
                stream,
                joining,
                log_len,
                setup,
                roster,
            } => {
                w.u8(5);
                w.var((*round_id).into());
                w.var((*stream).into());
                w.u8(u8::from(*joining));
                w.var((*log_len).into());
                w.bytes(setup);
                w.u8(roster.len() as u8);
                for e in roster {
                    w.u8(e.slot);
                    w.str(&e.name);
                    w.bytes(&e.profile);
                }
            }
            Msg::LogChunk {
                stream,
                first_tick,
                count,
                data,
            } => {
                w.u8(6);
                w.var((*stream).into());
                w.var((*first_tick).into());
                w.var((*count).into());
                w.bytes(data);
            }
            Msg::Loaded {
                round_id,
                static_digest,
                hash0,
            } => {
                w.u8(7);
                w.var((*round_id).into());
                w.u64(*static_digest);
                w.u64(*hash0);
            }
            Msg::Start { round_id } => {
                w.u8(8);
                w.var((*round_id).into());
            }
            Msg::HashReport { tick, hash } => {
                w.u8(9);
                w.var((*tick).into());
                w.u64(*hash);
            }
            Msg::Desync {
                tick,
                host_hash,
                build,
                target,
            } => {
                w.u8(10);
                w.var((*tick).into());
                w.u64(*host_hash);
                w.str(build);
                w.str(target);
            }
            Msg::Leave => w.u8(11),
            Msg::Ready { round_id } => {
                w.u8(12);
                w.var((*round_id).into());
            }
            Msg::Request(b) => {
                w.u8(13);
                w.bytes(b);
            }
            Msg::Notice(b) => {
                w.u8(14);
                w.bytes(b);
            }
            Msg::Chat { slot, text } => {
                w.u8(15);
                w.u8(*slot);
                w.str(text);
            }
            Msg::CaughtUp { round_id, tick } => {
                w.u8(16);
                w.var((*round_id).into());
                w.var((*tick).into());
            }
            Msg::Resync => w.u8(17),
            Msg::DesyncHashes {
                tick,
                build,
                target,
                first_tick,
                width,
                values,
            } => {
                w.u8(18);
                w.var((*tick).into());
                w.str(build);
                w.str(target);
                w.var((*first_tick).into());
                w.u8(*width);
                let rows = values.len() / usize::from((*width).max(1));
                w.var(rows as u64);
                for v in values {
                    w.u64(*v);
                }
            }
        }
        w.buf
    }

    pub fn decode(bytes: &[u8]) -> WireResult<Msg> {
        let mut r = Reader::new(bytes);
        let m = match r.u8()? {
            1 => Msg::Hello {
                protocol: r.u16()?,
                build: r.str(MAX_BUILD)?,
                name: r.str(MAX_NAME_CHARS * 4)?,
                token: r.u64()?,
                content: r.bytes(MAX_CONTENT)?.to_vec(),
                profile: r.bytes(MAX_PROFILE)?.to_vec(),
            },
            2 => Msg::Welcome {
                slot: r.u8()?,
                max_players: r.u8()?,
            },
            3 => Msg::Reject {
                reason: RejectReason::from_tag(r.u8()?)?,
                detail: r.str(MAX_DETAIL)?,
            },
            4 => Msg::Lobby {
                version: r.var32()?,
                state: r.bytes(MAX_LOBBY)?.to_vec(),
            },
            5 => {
                let round_id = r.var32()?;
                let stream = r.var32()?;
                let joining = match r.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(WireError::BadValue),
                };
                let log_len = r.var32()?;
                let setup = r.bytes(MAX_SETUP)?.to_vec();
                let n = usize::from(r.u8()?);
                if n > crate::bundle::MAX_SLOTS {
                    return Err(WireError::TooLong);
                }
                let mut roster = Vec::with_capacity(n);
                for _ in 0..n {
                    roster.push(RosterEntry {
                        slot: r.u8()?,
                        name: r.str(MAX_NAME_CHARS * 4)?,
                        profile: r.bytes(MAX_PROFILE)?.to_vec(),
                    });
                }
                Msg::LoadRound {
                    round_id,
                    stream,
                    joining,
                    log_len,
                    setup,
                    roster,
                }
            }
            6 => Msg::LogChunk {
                stream: r.var32()?,
                first_tick: r.var32()?,
                count: r.var32()?,
                data: r.bytes(MAX_LOG_CHUNK)?.to_vec(),
            },
            7 => Msg::Loaded {
                round_id: r.var32()?,
                static_digest: r.u64()?,
                hash0: r.u64()?,
            },
            8 => Msg::Start {
                round_id: r.var32()?,
            },
            9 => Msg::HashReport {
                tick: r.var32()?,
                hash: r.u64()?,
            },
            10 => Msg::Desync {
                tick: r.var32()?,
                host_hash: r.u64()?,
                build: r.str(MAX_BUILD)?,
                target: r.str(MAX_TARGET)?,
            },
            11 => Msg::Leave,
            12 => Msg::Ready {
                round_id: r.var32()?,
            },
            13 => Msg::Request(r.bytes(MAX_REQUEST)?.to_vec()),
            14 => Msg::Notice(r.bytes(MAX_NOTICE)?.to_vec()),
            15 => Msg::Chat {
                slot: r.u8()?,
                text: r.str(MAX_CHAT)?,
            },
            16 => Msg::CaughtUp {
                round_id: r.var32()?,
                tick: r.var32()?,
            },
            17 => Msg::Resync,
            18 => {
                let tick = r.var32()?;
                let build = r.str(MAX_BUILD)?;
                let target = r.str(MAX_TARGET)?;
                let first_tick = r.var32()?;
                let width = r.u8()?;
                if width == 0 || width > MAX_DESYNC_WIDTH {
                    return Err(WireError::BadValue);
                }
                let rows = r.var()?;
                if rows > MAX_DESYNC_ROWS as u64 {
                    return Err(WireError::TooLong);
                }
                let n = rows as usize * usize::from(width);
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(r.u64()?);
                }
                Msg::DesyncHashes {
                    tick,
                    build,
                    target,
                    first_tick,
                    width,
                    values,
                }
            }
            _ => return Err(WireError::BadValue),
        };
        r.finish()?;
        Ok(m)
    }
}

/// `name` made unique against the names `taken` reports (case-insensitive,
/// LAN §5b): the name itself when free, else `name_1`, `name_2`, ... in
/// order, the base trimmed so the result stays within [`MAX_NAME_CHARS`].
pub fn unique_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = name.trim();
    if !taken(base) {
        return base.to_string();
    }
    for n in 1u32.. {
        let suffix = format!("_{n}");
        let room = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
        let stem: String = base.chars().take(room).collect();
        let candidate = format!("{}{suffix}", stem.trim_end());
        if !taken(&candidate) {
            return candidate;
        }
    }
    unreachable!("a free suffix always exists")
}

/// Whether `name` is a valid player name: 1-24 characters, none of them
/// control characters, not all whitespace.
pub fn valid_name(name: &str) -> bool {
    let n = name.chars().count();
    (1..=MAX_NAME_CHARS).contains(&n)
        && name.chars().all(|c| !c.is_control())
        && name.chars().any(|c| !c.is_whitespace())
}
