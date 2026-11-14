//! Desync reports (issue #168, LAN §4.7): when the host's hash check finds
//! a guest's `Sim::hash` different from its own, both machines write a file
//! that says where the two simulations parted, and carries what is needed
//! to re-simulate the round and see which side a build agrees with
//! (`game --desync-report`).
//!
//! Rules:
//! - While a [`super::Peer`] plays a lockstep round it keeps a [`HashRing`]:
//!   for each stepped tick its `Sim::hash` and the five state groups
//!   ([`core::sim::trace::exact_group_hashes`]: SHASH v1's units, rng,
//!   score, posts and timers over exact bits), for the last [`RING_TICKS`]
//!   ticks. A joiner catching up fills it only for the log's last
//!   [`RING_TICKS`] ticks.
//! - It also keeps a [`BundleLog`]: every bundle it stepped this round from
//!   tick 0, sealed into zstd chunks (the `net` log encoding) as they fill.
//!   The report carries all of it (a superset of the last
//!   [`REPORT_BUNDLES`]), so the round can be re-simulated from its setup.
//! - A report holds the mismatch tick, both hashes, both sides' builds and
//!   OS/CPU, each side's rows for the [`REPORT_ROWS`] ticks up to and
//!   including the mismatch (the host's file holds both sides' when the
//!   guest's arrived), the static digest by group, the round setup and the
//!   bundles.
//! - The host waits up to [`GUEST_ROWS_WAIT`] stepped ticks for the guest's
//!   rows (`Msg::DesyncHashes`), then writes without them. One report per
//!   guest until it has caught up again: the hash checks that follow the
//!   first mismatch are the same divergence.
//! - Files: `<dir>/desync-<session>-s<slot>-t<tick>-<host|guest>.json`, the
//!   session being `<map>-<seed>-r<round id>` (the same on both machines).
//!   The folder defaults to the black box's ([`default_dir`]).
//! - Rows are text, `tick hash units rng score posts timers` in hex, like a
//!   trace row; [`first_difference`] compares two sides' rows.

use super::RoundSetup;
use core::sim::Sim;
use net::TickBundle;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Ticks of hash rows a peer keeps (about 17 s).
pub const RING_TICKS: usize = 512;
/// Rows a report carries per side: the mismatch tick and the ticks before.
pub const REPORT_ROWS: u32 = 60;
/// Bundles a report must cover (it carries the whole round log).
pub const REPORT_BUNDLES: u32 = 300;
/// Stepped ticks the host waits for a guest's rows (5 s).
pub const GUEST_ROWS_WAIT: u32 = 150;
/// A row's values after its tick.
pub const COLUMNS: [&str; 6] = ["hash", "units", "rng", "score", "posts", "timers"];
/// The file format tag.
pub const FORMAT: &str = "openbf-desync-report 1";

/// One tick's row: `Sim::hash`, then the five groups.
pub type Row = [u64; 6];

/// The row of `sim` as it stands.
pub fn row_of(sim: &Sim) -> Row {
    let g = core::sim::trace::exact_group_hashes(sim);
    [sim.hash(), g[0], g[1], g[2], g[3], g[4]]
}

/// The last [`RING_TICKS`] ticks' rows, contiguous.
#[derive(Clone, Debug, Default)]
pub struct HashRing {
    rows: VecDeque<(u32, Row)>,
}

impl HashRing {
    /// Adds `tick`'s row; a tick that does not follow the last starts over.
    pub fn push(&mut self, tick: u32, row: Row) {
        if self.rows.back().is_some_and(|(t, _)| t + 1 != tick) {
            self.rows.clear();
        }
        if self.rows.len() >= RING_TICKS {
            self.rows.pop_front();
        }
        self.rows.push_back((tick, row));
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// `tick`'s row, if held.
    pub fn get(&self, tick: u32) -> Option<Row> {
        let first = self.rows.front()?.0;
        let i = tick.checked_sub(first)? as usize;
        self.rows.get(i).map(|r| r.1)
    }

    /// The rows of the [`REPORT_ROWS`] ticks up to and including `tick`
    /// that it holds, oldest first.
    pub fn before(&self, tick: u32) -> Vec<(u32, Row)> {
        let from = tick.saturating_sub(REPORT_ROWS - 1);
        self.rows
            .iter()
            .filter(|(t, _)| (from..=tick).contains(t))
            .copied()
            .collect()
    }
}

/// Every bundle a peer stepped this round, from tick 0.
#[derive(Clone, Debug, Default)]
pub struct BundleLog {
    sealed: Vec<Vec<u8>>,
    tail: Vec<TickBundle>,
    /// The ticks were not contiguous from 0 (a report then says so).
    pub broken: bool,
}

impl BundleLog {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Bundles held.
    pub fn len(&self) -> u32 {
        self.sealed.len() as u32 * net::msg::LOG_CHUNK_TICKS + self.tail.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, b: &TickBundle) {
        if b.tick != self.len() {
            self.broken = true;
        }
        self.tail.push(b.clone());
        if self.tail.len() as u32 >= net::msg::LOG_CHUNK_TICKS {
            self.sealed.push(net::bundle::compress_log(&self.tail));
            self.tail.clear();
        }
    }

    /// The log as `(first tick, count, zstd)` chunks.
    pub fn chunks(&self) -> Vec<BundleChunk> {
        let n = net::msg::LOG_CHUNK_TICKS;
        let mut out: Vec<BundleChunk> = self
            .sealed
            .iter()
            .enumerate()
            .map(|(i, c)| BundleChunk {
                first_tick: i as u32 * n,
                count: n,
                zstd: hex(c),
            })
            .collect();
        if !self.tail.is_empty() {
            out.push(BundleChunk {
                first_tick: self.sealed.len() as u32 * n,
                count: self.tail.len() as u32,
                zstd: hex(&net::bundle::compress_log(&self.tail)),
            });
        }
        out
    }
}

/// A run of bundles in the `net` log encoding, zstd, as hex.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BundleChunk {
    pub first_tick: u32,
    pub count: u32,
    pub zstd: String,
}

impl BundleChunk {
    pub fn decode(&self) -> anyhow::Result<Vec<TickBundle>> {
        let bytes = unhex(&self.zstd)?;
        net::bundle::decompress_log(&bytes, self.first_tick, self.count as usize)
            .map_err(|e| anyhow::anyhow!("bundle chunk at {}: {e}", self.first_tick))
    }
}

/// One machine's side of a desync.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Side {
    pub build: String,
    /// OS and CPU.
    pub target: String,
    /// Its `Sim::hash` at the mismatch tick, hex (empty when unknown).
    pub hash: String,
    /// `tick hash units rng score posts timers`, hex, oldest first (empty
    /// when this file does not have them).
    pub rows: Vec<String>,
}

/// A desync report file (module rules).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DesyncReport {
    pub format: String,
    /// Who wrote it: `host` or `guest`.
    pub side: String,
    pub session: String,
    pub round_id: u32,
    /// The guest whose hash differed.
    pub slot: u8,
    /// The writer's own slot (0 on the host).
    pub local_slot: u8,
    /// The hash check's tick.
    pub tick: u32,
    pub columns: Vec<String>,
    pub host: Side,
    pub guest: Side,
    /// The writer's static gameplay digest, and by group.
    pub static_digest: String,
    pub static_groups: Vec<(String, String)>,
    /// `RoundSetup::encode`, hex, and its map, mode and seed in words.
    pub setup: String,
    pub setup_summary: String,
    /// Every bundle the writer stepped this round, from tick 0.
    pub bundles: Vec<BundleChunk>,
    /// The bundle log was not contiguous from tick 0 (no re-simulation).
    pub bundles_broken: bool,
}

impl DesyncReport {
    /// The round's setup.
    pub fn round_setup(&self) -> anyhow::Result<RoundSetup> {
        RoundSetup::decode(&unhex(&self.setup)?)
    }

    /// Every bundle, in tick order.
    pub fn all_bundles(&self) -> anyhow::Result<Vec<TickBundle>> {
        let mut out = Vec::new();
        for c in &self.bundles {
            out.extend(c.decode()?);
        }
        Ok(out)
    }

    /// The file name (module rules).
    pub fn file_name(&self) -> String {
        format!(
            "desync-{}-s{}-t{}-{}.json",
            self.session, self.slot, self.tick, self.side
        )
    }

    /// Writes it into `dir` (created); returns the path.
    pub fn write(&self, dir: &Path) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(self.file_name());
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(path)
    }

    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let r: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        anyhow::ensure!(r.format == FORMAT, "not a desync report: {}", r.format);
        Ok(r)
    }
}

/// The session name both machines give a round.
pub fn session_name(setup: &RoundSetup, round_id: u32) -> String {
    let map: String = setup
        .map
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{map}-{:08x}-r{round_id}", setup.seed)
}

/// Where reports go by default: the black box's folder, `desync/`.
pub fn default_dir() -> PathBuf {
    crate::blackbox::default_dir().join("desync")
}

pub fn format_row(tick: u32, row: &Row) -> String {
    let mut s = tick.to_string();
    for v in row {
        s.push_str(&format!(" {v:016x}"));
    }
    s
}

pub fn parse_row(line: &str) -> anyhow::Result<(u32, Row)> {
    let mut it = line.split_whitespace();
    let tick = it.next().unwrap_or_default().parse()?;
    let mut row = [0u64; 6];
    for v in &mut row {
        *v = u64::from_str_radix(it.next().unwrap_or_default(), 16)?;
    }
    anyhow::ensure!(it.next().is_none(), "row has extra fields: {line}");
    Ok((tick, row))
}

pub fn parse_rows(lines: &[String]) -> anyhow::Result<Vec<(u32, Row)>> {
    lines.iter().map(|l| parse_row(l)).collect()
}

/// Where two sides' rows first differ: the tick and the columns
/// ([`COLUMNS`]) that differ there, over the ticks both hold.
pub fn first_difference(a: &[(u32, Row)], b: &[(u32, Row)]) -> Option<(u32, Vec<&'static str>)> {
    let mut common: Vec<(u32, Row, Row)> = a
        .iter()
        .filter_map(|(t, ra)| b.iter().find(|(u, _)| u == t).map(|(_, rb)| (*t, *ra, *rb)))
        .collect();
    common.sort_by_key(|c| c.0);
    common.into_iter().find_map(|(t, ra, rb)| {
        let cols: Vec<&'static str> = COLUMNS
            .iter()
            .zip(ra.iter().zip(rb.iter()))
            .filter(|(_, (x, y))| x != y)
            .map(|(name, _)| *name)
            .collect();
        (!cols.is_empty()).then_some((t, cols))
    })
}

/// How a re-simulation compared with one side's rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Agreement {
    /// Rows compared.
    pub compared: u32,
    /// The first tick it differs from that side, and the columns.
    pub differs: Option<(u32, Vec<&'static str>)>,
}

/// Re-simulates the report's round on `walk` (built from
/// [`DesyncReport::round_setup`] for its `local_slot`, with `feed` the
/// round's event feed, as a peer builds it) through the last row tick, and
/// compares this build's rows with each side's. Returns the ticks stepped
/// and the host's and the guest's [`Agreement`].
pub fn resimulate(
    report: &DesyncReport,
    host_rows: &[(u32, Row)],
    guest_rows: &[(u32, Row)],
    walk: &mut crate::walk::WalkSim,
    mut feed: Option<&mut crate::event_feed::EventFeed>,
) -> anyhow::Result<(u32, Agreement, Agreement)> {
    anyhow::ensure!(
        !report.bundles_broken,
        "the report's bundle log is not contiguous from tick 0"
    );
    let last = host_rows
        .iter()
        .chain(guest_rows)
        .map(|r| r.0)
        .max()
        .ok_or_else(|| anyhow::anyhow!("no rows to compare"))?;
    let bundles = report.all_bundles()?;
    anyhow::ensure!(
        bundles.len() as u64 > u64::from(last),
        "the log ends at tick {}, the rows at {last}",
        bundles.len()
    );
    let (mut host, mut guest) = (Agreement::default(), Agreement::default());
    let compare = |a: &mut Agreement, rows: &[(u32, Row)], tick: u32, mine: &Row| {
        if let Some((_, theirs)) = rows.iter().find(|r| r.0 == tick) {
            a.compared += 1;
            if a.differs.is_none() {
                a.differs = first_difference(&[(tick, *mine)], &[(tick, *theirs)]);
            }
        }
    };
    let mut stepped = 0;
    for (i, b) in bundles.iter().take(last as usize + 1).enumerate() {
        anyhow::ensure!(b.tick == i as u32, "bundle {i} says tick {}", b.tick);
        super::step(walk, b, feed.as_deref_mut(), |_, _| {});
        stepped += 1;
        let mine = row_of(&walk.sim);
        compare(&mut host, host_rows, b.tick, &mine);
        compare(&mut guest, guest_rows, b.tick, &mine);
    }
    Ok((stepped, host, guest))
}

/// Lowercase hex.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

pub fn unhex(s: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(s.len().is_multiple_of(2), "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| {
            s.get(i..i + 2)
                .and_then(|p| u8::from_str_radix(p, 16).ok())
                .ok_or_else(|| anyhow::anyhow!("bad hex at {i}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desync_rows_ring_and_codecs() {
        let mut ring = HashRing::default();
        for t in 0..600u32 {
            ring.push(t, [u64::from(t); 6]);
        }
        assert_eq!(ring.len(), RING_TICKS);
        let rows = ring.before(550);
        assert_eq!(rows.len(), REPORT_ROWS as usize);
        assert_eq!((rows[0].0, rows[59].0), (491, 550));
        assert_eq!(ring.get(599), Some([599; 6]));
        assert_eq!(ring.get(10), None);
        ring.push(700, [1; 6]);
        assert_eq!(ring.len(), 1, "a gap starts over");
        let line = format_row(42, &[1, 2, 3, u64::MAX, 0, 9]);
        assert_eq!(parse_row(&line).unwrap(), (42, [1, 2, 3, u64::MAX, 0, 9]));
        assert!(parse_row("1 2").is_err());
        assert_eq!(unhex(&hex(&[0, 255, 16])).unwrap(), vec![0, 255, 16]);
        assert!(unhex("abc").is_err());
        let a = vec![(1, [1, 1, 1, 1, 1, 1]), (2, [2, 2, 2, 2, 2, 2])];
        let mut b = a.clone();
        assert_eq!(first_difference(&a, &b), None);
        b[1].1[0] = 7;
        b[1].1[2] = 7;
        assert_eq!(first_difference(&a, &b), Some((2, vec!["hash", "rng"])));
    }

    #[test]
    fn desync_bundle_log_seals_and_decodes() {
        let mut log = BundleLog::default();
        let n = net::msg::LOG_CHUNK_TICKS + 5;
        for tick in 0..n {
            log.push(&TickBundle {
                tick,
                pads: vec![(
                    0,
                    core::sim::PadState {
                        lx: tick as i16,
                        ..Default::default()
                    },
                )],
                commands: Vec::new(),
                seqs: Vec::new(),
            });
        }
        assert_eq!(log.len(), n);
        assert!(!log.broken);
        let chunks = log.chunks();
        assert_eq!(chunks.len(), 2);
        let back: Vec<TickBundle> = chunks.iter().flat_map(|c| c.decode().unwrap()).collect();
        assert_eq!(back.len() as u32, n);
        assert!(back
            .iter()
            .enumerate()
            .all(|(i, b)| b.tick == i as u32 && b.pads[0].1.lx == i as i16));
    }
}
