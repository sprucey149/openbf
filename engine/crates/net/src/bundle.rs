//! Tick bundles, session commands and the compact pad codec.
//!
//! A [`TickBundle`] is everything every peer needs to step tick `T`: the
//! session commands applied, in order, before the step, then the pad of every
//! human slot active after them. Bundles and inputs travel delta-coded.
//!
//! Rules:
//! - Each pad is followed by the newest input sequence of its slot the host
//!   consumed up to this tick (`scheduler`; 0 before any): a peer drops its
//!   own inputs through it from the ones it still previews and predicts.
//! - A pad is coded against a base pad (the same slot's pad in the previous
//!   bundle, the previous input, or the default pad). A mask names the fields
//!   that differ; only those follow. Buttons are XOR-coded, sticks and mouse
//!   deltas are zigzag varints of the value. The host-stamped `view_lag`
//!   (lag compensation) rides in the extension byte like the fire lag.
//! - Slots are below [`MAX_SLOTS`] and strictly ascending inside a bundle; a
//!   repeated or out-of-range slot, an unknown tag or a length past its limit
//!   fails the decode.
//! - A bundle always fits one datagram ([`crate::packet::BUNDLE_BUDGET`]):
//!   a pad takes at most [`MAX_PAD_BYTES`], the host caps the humans so the
//!   pads alone fit, and it admits a tick's commands (each at most
//!   [`MAX_COMMAND_BYTES`]) only while they fit beside that tick's pads
//!   coded against either base a stream may use (the previous tick's pads or
//!   none); the rest wait for the following ticks.

use crate::wire::{Reader, WireError, WireResult, Writer};
use core::sim::{PadState, ScreenPick};

/// Slot tables hold this many players (the protocol cap).
pub const MAX_SLOTS: usize = 32;
/// The largest opaque payload one session command may carry.
pub const MAX_COMMAND_BYTES: usize = 600;
/// The most commands one bundle carries.
pub const MAX_COMMANDS: usize = 64;
/// The most bytes one pad takes in a bundle: slot, mask, four sticks
/// (zigzag i16, 3 each), buttons (5), mouse deltas (zigzag i32, 5 each),
/// the extension byte, fire lag, a walker pick (1 + 5 + 5), view lag and
/// the consumed sequence (5).
pub const MAX_PAD_BYTES: usize = 1 + 1 + 4 * 3 + 5 + 2 * 5 + 1 + 1 + 11 + 1 + 5;
/// The team value meaning "the sim assigns it".
pub const TEAM_AUTO: u8 = 255;

/// Something outside the pads that changes the sim, applied before a tick's
/// step on every peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    /// A human takes `slot` (a bot steps aside). `team` 255 is auto.
    AddHuman {
        slot: u8,
        team: u8,
        profile: Vec<u8>,
    },
    /// The human leaves `slot`; a bot takes over where they stood.
    RemoveHuman { slot: u8 },
    /// An opaque game command (cheats, host-only settings, ...).
    Game(Vec<u8>),
    /// The round ends after this bundle.
    EndRound,
}

impl SessionCommand {
    /// Encoded size, for packing commands into bundles.
    pub fn wire_size(&self) -> usize {
        match self {
            SessionCommand::AddHuman { profile, .. } => 3 + 3 + profile.len(),
            SessionCommand::RemoveHuman { .. } => 2,
            SessionCommand::Game(b) => 1 + 3 + b.len(),
            SessionCommand::EndRound => 1,
        }
    }

    fn write(&self, w: &mut Writer) {
        match self {
            SessionCommand::AddHuman {
                slot,
                team,
                profile,
            } => {
                w.u8(1);
                w.u8(*slot);
                w.u8(*team);
                w.bytes(profile);
            }
            SessionCommand::RemoveHuman { slot } => {
                w.u8(2);
                w.u8(*slot);
            }
            SessionCommand::Game(b) => {
                w.u8(3);
                w.bytes(b);
            }
            SessionCommand::EndRound => w.u8(4),
        }
    }

    fn read(r: &mut Reader) -> WireResult<Self> {
        Ok(match r.u8()? {
            1 => SessionCommand::AddHuman {
                slot: read_slot(r)?,
                team: r.u8()?,
                profile: r.bytes(MAX_COMMAND_BYTES)?.to_vec(),
            },
            2 => SessionCommand::RemoveHuman {
                slot: read_slot(r)?,
            },
            3 => SessionCommand::Game(r.bytes(MAX_COMMAND_BYTES)?.to_vec()),
            4 => SessionCommand::EndRound,
            _ => return Err(WireError::BadValue),
        })
    }
}

fn read_slot(r: &mut Reader) -> WireResult<u8> {
    let s = r.u8()?;
    if usize::from(s) >= MAX_SLOTS {
        return Err(WireError::BadValue);
    }
    Ok(s)
}

/// Everything step `tick` needs: commands first, then each active human
/// slot's pad (ascending by slot).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickBundle {
    pub tick: u32,
    pub pads: Vec<(u8, PadState)>,
    pub commands: Vec<SessionCommand>,
    /// Each pad's slot's newest input sequence consumed through this tick,
    /// in the order of `pads` (missing entries read as 0).
    pub seqs: Vec<u32>,
}

impl TickBundle {
    /// The pad of `slot`, if that slot is a human this tick.
    pub fn pad(&self, slot: u8) -> Option<&PadState> {
        self.pads
            .binary_search_by_key(&slot, |p| p.0)
            .ok()
            .map(|i| &self.pads[i].1)
    }

    /// The newest input sequence of `slot` the host consumed through this
    /// tick, if that slot is a human this tick.
    pub fn consumed(&self, slot: u8) -> Option<u32> {
        let i = self.pads.binary_search_by_key(&slot, |p| p.0).ok()?;
        Some(self.seqs.get(i).copied().unwrap_or(0))
    }

    /// Writes the bundle (tick implicit) with each pad coded against the same
    /// slot's pad in `base`.
    pub fn write(&self, w: &mut Writer, base: &[(u8, PadState)]) {
        w.u8(self.commands.len() as u8);
        for c in &self.commands {
            c.write(w);
        }
        w.u8(self.pads.len() as u8);
        for (i, (slot, pad)) in self.pads.iter().enumerate() {
            w.u8(*slot);
            let b = base
                .binary_search_by_key(slot, |p| p.0)
                .map(|i| base[i].1)
                .unwrap_or_default();
            write_pad(w, pad, &b);
            w.var(self.seqs.get(i).copied().unwrap_or(0).into());
        }
    }

    /// Reads a bundle written by [`TickBundle::write`] against `base`.
    pub fn read(r: &mut Reader, tick: u32, base: &[(u8, PadState)]) -> WireResult<Self> {
        let nc = usize::from(r.u8()?);
        if nc > MAX_COMMANDS {
            return Err(WireError::TooLong);
        }
        let mut commands = Vec::with_capacity(nc);
        for _ in 0..nc {
            commands.push(SessionCommand::read(r)?);
        }
        let np = usize::from(r.u8()?);
        if np > MAX_SLOTS {
            return Err(WireError::TooLong);
        }
        let mut pads: Vec<(u8, PadState)> = Vec::with_capacity(np);
        let mut seqs = Vec::with_capacity(np);
        for _ in 0..np {
            let slot = read_slot(r)?;
            if pads.last().is_some_and(|p| p.0 >= slot) {
                return Err(WireError::BadValue);
            }
            let b = base
                .binary_search_by_key(&slot, |p| p.0)
                .map(|i| base[i].1)
                .unwrap_or_default();
            pads.push((slot, read_pad(r, &b)?));
            seqs.push(r.var32()?);
        }
        Ok(TickBundle {
            tick,
            pads,
            commands,
            seqs,
        })
    }
}

const M_LX: u8 = 1;
const M_LY: u8 = 2;
const M_RX: u8 = 4;
const M_RY: u8 = 8;
const M_BUTTONS: u8 = 16;
const M_MX: u8 = 32;
const M_MY: u8 = 64;
const M_EXT: u8 = 128;
const E_LAG: u8 = 1;
const E_PICK: u8 = 2;
/// The host-stamped `view_lag` (lag compensation).
const E_VIEW: u8 = 4;

/// The bytes `slot`'s `pad` and consumed `seq` take in a bundle, coded
/// against `base`.
pub fn pad_wire_len(pad: &PadState, seq: u32, base: &PadState) -> usize {
    let mut w = Writer::new();
    w.u8(0);
    write_pad(&mut w, pad, base);
    w.var(seq.into());
    w.len()
}

/// Writes `pad` coded against `base`.
pub fn write_pad(w: &mut Writer, pad: &PadState, base: &PadState) {
    let mut mask = 0u8;
    let mut ext = 0u8;
    mask |= if pad.lx != base.lx { M_LX } else { 0 };
    mask |= if pad.ly != base.ly { M_LY } else { 0 };
    mask |= if pad.rx != base.rx { M_RX } else { 0 };
    mask |= if pad.ry != base.ry { M_RY } else { 0 };
    mask |= if pad.buttons != base.buttons {
        M_BUTTONS
    } else {
        0
    };
    mask |= if pad.mx != base.mx { M_MX } else { 0 };
    mask |= if pad.my != base.my { M_MY } else { 0 };
    ext |= if pad.fire_lag != base.fire_lag {
        E_LAG
    } else {
        0
    };
    ext |= if pad.pick != base.pick { E_PICK } else { 0 };
    ext |= if pad.view_lag != base.view_lag {
        E_VIEW
    } else {
        0
    };
    if ext != 0 {
        mask |= M_EXT;
    }
    w.u8(mask);
    if mask & M_LX != 0 {
        w.ivar(pad.lx.into());
    }
    if mask & M_LY != 0 {
        w.ivar(pad.ly.into());
    }
    if mask & M_RX != 0 {
        w.ivar(pad.rx.into());
    }
    if mask & M_RY != 0 {
        w.ivar(pad.ry.into());
    }
    if mask & M_BUTTONS != 0 {
        w.var(u64::from(pad.buttons ^ base.buttons));
    }
    if mask & M_MX != 0 {
        w.ivar(pad.mx.into());
    }
    if mask & M_MY != 0 {
        w.ivar(pad.my.into());
    }
    if ext != 0 {
        w.u8(ext);
        if ext & E_LAG != 0 {
            w.u8(pad.fire_lag);
        }
        if ext & E_PICK != 0 {
            match pad.pick {
                None => w.u8(0),
                Some(ScreenPick::Side(s)) => {
                    w.u8(1);
                    w.u8(s);
                }
                Some(ScreenPick::Post(p)) => {
                    w.u8(2);
                    w.u16(p);
                }
                Some(ScreenPick::Walker { index, gen }) => {
                    w.u8(3);
                    w.var(index.into());
                    w.var(gen.into());
                }
            }
        }
        if ext & E_VIEW != 0 {
            w.u8(pad.view_lag);
        }
    }
}

/// Reads a pad written by [`write_pad`] against `base`.
pub fn read_pad(r: &mut Reader, base: &PadState) -> WireResult<PadState> {
    let mask = r.u8()?;
    let mut p = *base;
    if mask & M_LX != 0 {
        p.lx = r.i16()?;
    }
    if mask & M_LY != 0 {
        p.ly = r.i16()?;
    }
    if mask & M_RX != 0 {
        p.rx = r.i16()?;
    }
    if mask & M_RY != 0 {
        p.ry = r.i16()?;
    }
    if mask & M_BUTTONS != 0 {
        p.buttons = base.buttons ^ r.var32()?;
    }
    if mask & M_MX != 0 {
        p.mx = r.i32()?;
    }
    if mask & M_MY != 0 {
        p.my = r.i32()?;
    }
    if mask & M_EXT != 0 {
        let ext = r.u8()?;
        if ext & !(E_LAG | E_PICK | E_VIEW) != 0 || ext == 0 {
            return Err(WireError::BadValue);
        }
        if ext & E_LAG != 0 {
            p.fire_lag = r.u8()?;
        }
        if ext & E_PICK != 0 {
            p.pick = match r.u8()? {
                0 => None,
                1 => Some(ScreenPick::Side(r.u8()?)),
                2 => Some(ScreenPick::Post(r.u16()?)),
                3 => Some(ScreenPick::Walker {
                    index: r.var32()?,
                    gen: r.var32()?,
                }),
                _ => return Err(WireError::BadValue),
            };
        }
        if ext & E_VIEW != 0 {
            p.view_lag = r.u8()?;
        }
    }
    Ok(p)
}

/// Writes a run of bundles, each coded against the one before it (the first
/// against `base`).
pub fn write_chain(w: &mut Writer, bundles: &[TickBundle], base: &[(u8, PadState)]) {
    let mut prev = base;
    for b in bundles {
        b.write(w, prev);
        prev = &b.pads;
    }
}

/// Reads `count` bundles from `first_tick` written by [`write_chain`].
pub fn read_chain(
    r: &mut Reader,
    first_tick: u32,
    count: usize,
    base: &[(u8, PadState)],
) -> WireResult<Vec<TickBundle>> {
    let mut out: Vec<TickBundle> = Vec::with_capacity(count.min(4096));
    for i in 0..count {
        let tick = first_tick
            .checked_add(i as u32)
            .ok_or(WireError::BadValue)?;
        let b = {
            let prev = out.last().map(|b| b.pads.as_slice()).unwrap_or(base);
            TickBundle::read(r, tick, prev)?
        };
        out.push(b);
    }
    Ok(out)
}

/// Compresses a closed-bundle log segment (ticks `first..first+len`) for the
/// catch-up stream.
pub fn compress_log(bundles: &[TickBundle]) -> Vec<u8> {
    let mut w = Writer::new();
    write_chain(&mut w, bundles, &[]);
    zstd::bulk::compress(&w.buf, 3).unwrap_or_default()
}

/// The inverse of [`compress_log`]; `count` bundles from `first_tick`.
pub fn decompress_log(data: &[u8], first_tick: u32, count: usize) -> WireResult<Vec<TickBundle>> {
    // A bundle is at most a few kilobytes; bound the inflated size.
    let cap = count.saturating_mul(4096).clamp(64, 64 << 20);
    let raw = zstd::bulk::decompress(data, cap).map_err(|_| WireError::BadValue)?;
    let mut r = Reader::new(&raw);
    let out = read_chain(&mut r, first_tick, count, &[])?;
    r.finish()?;
    Ok(out)
}

/// Merges two consecutive pads `a` then `b` into one, or `None` when that
/// would change what the sim sees.
///
/// `prior` is the pad stepped before `a`; `next` the one after `b`, or
/// `None` when it is not known yet (the merge must then hold for any next
/// pad). Sticks and held levels take `b`'s; a button that rose in `a` stays
/// down for the merged tick (its release waits a tick); mouse deltas sum; the
/// first pick and its fire lag win. The merge is refused when it would change
/// the number of press edges of any button (a release and re-press, two taps)
/// or when both pads carry a pick.
pub fn merge_pads(
    prior: &PadState,
    a: &PadState,
    b: &PadState,
    next: Option<&PadState>,
) -> Option<PadState> {
    if a.pick.is_some() && b.pick.is_some() {
        return None;
    }
    let (p, x, y) = (prior.buttons, a.buttons, b.buttons);
    let m = y | (x & !p);
    let keeps_edges = |n: u32| {
        (0..32).all(|bit| {
            let k = 1u32 << bit;
            let rise = |f: u32, t: u32| u8::from(t & k != 0 && f & k == 0);
            rise(p, x) + rise(x, y) + rise(y, n) == rise(p, m) + rise(m, n)
        })
    };
    let ok = match next {
        Some(n) => keeps_edges(n.buttons),
        None => keeps_edges(0) && keeps_edges(u32::MAX),
    };
    if !ok {
        return None;
    }
    let mut out = *b;
    out.buttons = m;
    out.mx = a.mx.saturating_add(b.mx);
    out.my = a.my.saturating_add(b.my);
    out.pick = a.pick.or(b.pick);
    out.fire_lag = if a.fire_lag != 0 || a.pick.is_some() {
        a.fire_lag
    } else {
        b.fire_lag
    };
    Some(out)
}

/// The pad repeated for a slot whose input did not arrive: levels and
/// sticks held, once-only fields cleared.
pub fn repeat_pad(last: &PadState) -> PadState {
    let mut p = *last;
    p.clear_once();
    p
}
