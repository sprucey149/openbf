//! SHASH version-1 observational trace. Called explicitly AFTER `Sim::step`;
//! the normal tick path has no tracing branch or storage. This is a selected
//! state oracle, not a save state or a hash of all future-affecting state.
//!
//! Five groups (units, rng, score, posts, timers), each independent FNV-1a
//! over fixed-width little-endian fields; tuple order and widths are in the
//! header. Units include dead slots, ordered by (slot, generation); posts
//! use registration order. Full rows show the exact quantized integers fed
//! to the hashes. Non-finite/out-of-range floats are errors, never aliases.

use super::Sim;
use crate::fmath::FloatExt;
use std::io::{self, Write};

/// Version-1 quantization contract, in world metres, health points, capture
/// seconds, and timer seconds respectively. Round nearest, ties away from 0.
pub const POSITION_QUANTUM: f32 = 1.0 / 16.0;
pub const HEALTH_QUANTUM: f32 = 1.0 / 4.0;
pub const CAPTURE_QUANTUM: f32 = 1.0 / 256.0;
pub const TIMER_QUANTUM: f32 = 1.0 / 1024.0;
/// Full traces fail before exceeding this size; they are never silently cut.
pub const FULL_BYTE_LIMIT: usize = 64 * 1024 * 1024;

const OFFSET: u64 = 0xcbf29ce484222325;
const PRIME: u64 = 0x100000001b3;
/// The five group names, in row order.
pub const GROUPS: [&str; 5] = ["units", "rng", "score", "posts", "timers"];

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn quantize(value: f32, quantum: f32) -> io::Result<i32> {
    let q = libm::round(f64::from(value) / f64::from(quantum));
    if !q.is_finite() || q < f64::from(i32::MIN) || q > f64::from(i32::MAX) {
        return Err(invalid(
            "trace float is non-finite or outside quantized i32 range",
        ));
    }
    Ok(q as i32)
}

struct Group {
    hash: u64,
    raw: Option<String>,
    /// Floats feed their exact bits instead of their quantized value
    /// ([`exact_group_hashes`]).
    exact: bool,
}

impl Group {
    fn new(full: bool, exact: bool) -> Self {
        Self {
            hash: OFFSET,
            raw: full.then(String::new),
            exact,
        }
    }
    fn field(&mut self, bytes: &[u8], value: impl std::fmt::Display) {
        for b in bytes {
            self.hash = (self.hash ^ u64::from(*b)).wrapping_mul(PRIME);
        }
        if let Some(raw) = &mut self.raw {
            use std::fmt::Write;
            write!(raw, " {value}").unwrap();
        }
    }
    fn u8(&mut self, v: u8) {
        self.field(&[v], v);
    }
    fn u32(&mut self, v: u32) {
        self.field(&v.to_le_bytes(), v);
    }
    fn u64(&mut self, v: u64) {
        self.field(&v.to_le_bytes(), v);
    }
    fn i32(&mut self, v: i32) {
        self.field(&v.to_le_bytes(), v);
    }
    fn float(&mut self, v: f32, q: f32) -> io::Result<()> {
        if self.exact {
            self.u32(v.hash_bits());
        } else {
            self.i32(quantize(v, q)?);
        }
        Ok(())
    }
    fn count(&mut self, n: usize) -> io::Result<()> {
        self.u32(u32::try_from(n).map_err(|_| invalid("trace count exceeds u32"))?);
        Ok(())
    }
}

fn groups(sim: &Sim, full: bool, exact: bool) -> io::Result<[Group; 5]> {
    let [mut units, mut rng, mut score, mut posts, mut timers] =
        std::array::from_fn(|_| Group::new(full, exact));
    units.count(sim.units.len())?;
    for (id, u) in sim.units.iter().enumerate() {
        units.count(id)?;
        units.u32(u.gen);
        for v in u.soldier.pos.to_array() {
            units.float(v, POSITION_QUANTUM)?;
        }
        units.float(u.health.cur, HEALTH_QUANTUM)?;
        units.u8(u.team);
        units.u8(u8::from(u.alive));
    }
    rng.u32(sim.lcg.state());
    for team in 0..2 {
        score.u32(sim.battle.teams[team].tickets());
        let members = sim.units.iter().filter(|u| usize::from(u.team) == team);
        let (kills, deaths) = members.fold((0u64, 0u64), |(k, d), u| {
            (k + u64::from(u.kills), d + u64::from(u.deaths))
        });
        score.u64(kills);
        score.u64(deaths);
    }
    posts.count(sim.cps.len())?;
    for (id, cp) in sim.cps.iter().enumerate() {
        posts.count(id)?;
        posts.u8(cp.owner.unwrap_or(255));
        posts.u8(cp.capture_team.unwrap_or(255));
        posts.float(cp.capture, CAPTURE_QUANTUM)?;
        posts.float(cp.neutral, CAPTURE_QUANTUM)?;
    }
    // Round elapsed time is exact ticks; no f32 clock accumulated by the port.
    timers.u64(sim.tick);
    timers.u32(sim.time_limit.unwrap_or(u32::MAX));
    timers.u8(sim.outcome.unwrap_or(255));
    let (team, seconds) = sim.domination.unwrap_or((255, 0.0));
    timers.u8(team);
    timers.float(seconds, TIMER_QUANTUM)?;
    for wave in &sim.waves {
        timers.float(wave.timer, TIMER_QUANTUM)?;
    }
    timers.count(sim.units.len())?;
    for (id, u) in sim.units.iter().enumerate() {
        timers.count(id)?;
        timers.u32(u.gen);
        timers.u64(u.respawn_at.unwrap_or(u64::MAX));
    }
    Ok([units, rng, score, posts, timers])
}

/// The five groups' hashes ([`GROUPS`] order) with the SHASH v1 fields and
/// order, but every float fed as its exact `f32` bits instead of its
/// quantized value: a lockstep desync is a bit difference, and quantizing
/// would hide one smaller than a quantum (a 1 cm nudge under the 6.25 cm
/// position quantum). The LAN desync reports keep these per tick (issue
/// #168, LAN §4.7). Never fails: only counts over `u32::MAX` could, and
/// those hash as 0.
pub fn exact_group_hashes(sim: &Sim) -> [u64; 5] {
    groups(sim, false, true).map_or([0; 5], |g| g.map(|g| g.hash))
}

/// Streaming text writer. It only borrows the sim immutably. The caller owns
/// buffering; `finish` flushes and propagates delayed I/O failures.
pub struct TraceWriter<W: Write> {
    writer: W,
    full: bool,
    bytes: usize,
}

impl<W: Write> TraceWriter<W> {
    pub fn new(writer: W, map: &str, seed: u32, full: bool) -> io::Result<Self> {
        if map.contains(['\r', '\n']) {
            return Err(invalid("trace map contains a newline"));
        }
        let mut this = Self {
            writer,
            full,
            bytes: 0,
        };
        this.emit(&format!(concat!(
            "# openbf-trace 1\n# map {}\n# seed {}\n# tick_hz {}\n",
            "# quanta position={} health={} capture={} timer={}\n",
            "# encoding fnv1a64 little-endian round-nearest-ties-away none_u8=255 none_u32=4294967295 none_u64=18446744073709551615\n",
            "# units count:u32 (id:u32 gen:u32 x:i32 y:i32 z:i32 health:i32 team:u8 alive:u8)*\n",
            "# rng lcg:u32\n",
            "# score (tickets:u32 kills:u64 casualties:u64)*2 teams=0,1\n",
            "# posts count:u32 (id:u32 owner:u8 capture_team:u8 capture:i32 neutral:i32)*\n",
            "# timers elapsed_ticks:u64 limit_seconds:u32 outcome:u8 domination_team:u8 domination:i32 wave0:i32 wave1:i32 count:u32 (id:u32 gen:u32 respawn_at_tick:u64)*\n",
            "# columns t units rng score posts timers\n# full {}\n"
        ), map, seed, crate::TICK_HZ, POSITION_QUANTUM, HEALTH_QUANTUM, CAPTURE_QUANTUM, TIMER_QUANTUM, full))?;
        Ok(this)
    }

    fn emit(&mut self, text: &str) -> io::Result<()> {
        if self.full && text.len() > FULL_BYTE_LIMIT.saturating_sub(self.bytes) {
            return Err(invalid("full trace exceeds 64 MiB; request fewer ticks"));
        }
        self.writer.write_all(text.as_bytes())?;
        self.bytes += text.len();
        Ok(())
    }

    /// Emit the state after a completed tick. Tick 1 is after log frame 0.
    pub fn write_tick(&mut self, sim: &Sim) -> io::Result<()> {
        use std::fmt::Write;
        let groups = groups(sim, self.full, false)?;
        let mut row = sim.tick.to_string();
        for group in &groups {
            write!(row, " {:016x}", group.hash).unwrap();
        }
        row.push('\n');
        for (name, group) in GROUPS.iter().zip(groups) {
            if let Some(raw) = group.raw {
                writeln!(row, "# full {} {name}{raw}", sim.tick).unwrap();
            }
        }
        self.emit(&row)
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.writer.flush()?;
        Ok(self.writer)
    }
}

#[cfg(test)]
mod tests;
