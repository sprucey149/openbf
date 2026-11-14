//! The windowed game's frame profile (cards ISPD, HITCH): where each battle
//! frame's time went - the sim's ticks (how many ran, their AI part and their
//! per-tick hook), the per-frame updates, building the draw lists and the
//! HUD, `render_frame` and its wait for the window's image, the GPU's ms
//! (with `OPENBF_GPU_TIMING=1`, `Renderer::gpu_sections`, a frame or two old)
//! and every GPU object the renderer made in the frame (`render::made`).
//!
//! Two outputs, each on its own switch:
//! - the RECORD (card HITCH; dev mode, on by default like the black box -
//!   `OPENBF_DEV=0` or `OPENBF_FRAME_RECORD=0` turns it off): the battle's
//!   frame times as a histogram, every frame over [`HITCH_MS`] with its
//!   breakdown, its worst bot and its rays ([`TickCosts`]; the newest [`SLOW_KEPT`]) and the [`WORST_KEPT`] longest
//!   past the first [`SETTLE_S`] seconds. The black box writes it into its
//!   snapshot ([`Hitches`], F9 / quit / panic) and the game prints its
//!   [`Hitches::summary`] line when the battle ends. Warm, it allocates only
//!   for a slow frame's row.
//! - the CSV (card ISPD, `OPENBF_FRAME_PROFILE=<csv>`): one row per frame.
//!   `OPENBF_NO_VSYNC=1` beside it lets a frame run as fast as it can.
//!
//! A frame's time is from its start to the next frame's start: the work that
//! made a frame late is its own, so a row holds the frame's phases and
//! `other` (the time outside them: the event loop, the OS).
//!
//! CSV columns: `frame,dt_ms,` then one per lap in the order the first frame
//! called [`FrameProfile::lap`] (a lap a later frame skipped is empty), then
//! `acquire_ms,draw_calls,gpu_ms,gpu,ticks,ai_ms,hook_ms,made,cam_x,cam_y,cam_z,me_x,me_y,me_z` -
//! `gpu` is `section=ms` pairs joined by `;`, `made` the count of GPU objects
//! made, `cam_*` the drawn camera's place and `me_*` the drawn player's feet
//! (card RINT: a frame whose camera stood while the player moved; empty
//! outside a battle).
//! Wall clock: it measures the machine, nothing here reaches the sim.

use core::fmath::FloatExt;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A frame longer than this is a hitch (ms): at 60 Hz two frames shown as
/// one and a half.
pub const HITCH_MS: f32 = 25.0;
/// The first seconds of a battle are not counted as hitches (the first
/// frames' uploads and the spawn).
pub const SETTLE_S: f32 = 5.0;
/// Slow frames kept with their breakdown, newest.
pub const SLOW_KEPT: usize = 256;
/// The longest frames past [`SETTLE_S`] kept.
pub const WORST_KEPT: usize = 5;
/// Histogram bins: 0.1 ms each up to this many ms; longer frames go in the
/// last bin (the worst is kept apart).
const HIST_MS: usize = 200;
const BINS_PER_MS: usize = 10;

/// One GPU object made in a frame (`render::made::Made`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MadeRow {
    pub kind: String,
    pub label: String,
    pub bytes: u64,
    /// Wall-clock ms its making took.
    #[serde(default)]
    pub ms: f32,
}

/// What a frame's sim ticks cost: the bot that took longest and every ray the
/// ticks cast (`core::sim::TickCost`). Rays are counted, not timed, so two logs
/// can be compared: a slow tick with few triangles spent its time elsewhere.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TickCosts {
    /// The bot's class, and its index among the units.
    pub worst_unit: String,
    pub worst_unit_index: u32,
    pub worst_unit_ms: f32,
    /// World ray queries that bot's think made.
    pub worst_unit_rays: u32,
    pub ray_queries: u32,
    pub mesh_casts: u32,
    pub triangles: u32,
    /// The dearest cast: its triangles, the map instance it was against, where
    /// it started and how far it reached.
    pub worst_cast_triangles: u32,
    pub worst_cast_instance: Option<u32>,
    pub worst_cast_from: [f32; 3],
    pub worst_cast_range: f32,
}

impl TickCosts {
    /// Add one tick. `name` names a unit, and runs only for the frame's new
    /// worst bot past [`NAMED_THINK_MS`], so a quiet frame allocates nothing.
    pub fn add(&mut self, c: &core::sim::TickCost, name: impl FnOnce(u32) -> String) {
        if let Some(w) = c.worst_think.filter(|w| w.ms > self.worst_unit_ms) {
            self.worst_unit_index = w.unit;
            self.worst_unit_ms = w.ms;
            self.worst_unit_rays = w.ray_queries;
            self.worst_unit.clear();
            if w.ms >= NAMED_THINK_MS {
                self.worst_unit = name(w.unit);
            }
        }
        self.ray_queries += c.rays.queries;
        self.mesh_casts += c.rays.mesh_casts;
        self.triangles += c.rays.triangles;
        if let Some(w) = c
            .rays
            .worst
            .filter(|w| w.triangles > self.worst_cast_triangles)
        {
            self.worst_cast_triangles = w.triangles;
            self.worst_cast_instance = w.instance;
            self.worst_cast_from = w.origin;
            self.worst_cast_range = w.range;
        }
    }

    /// `worst unit rep_inf_trooper #12 3.1 ms (40 rays), rays 900 (200 mesh,
    /// 8000 tris), dearest cast 400 tris on instance 31 from (x, y, z) over 50 m`.
    pub fn line(&self) -> String {
        let mut parts = Vec::new();
        if self.worst_unit_ms > 0.0 {
            let who = match self.worst_unit.as_str() {
                "" => format!("#{}", self.worst_unit_index),
                name => format!("{name} #{}", self.worst_unit_index),
            };
            parts.push(format!(
                "worst unit {who} {:.1} ms ({} rays)",
                self.worst_unit_ms, self.worst_unit_rays
            ));
        }
        if self.ray_queries > 0 || self.mesh_casts > 0 {
            parts.push(format!(
                "rays {} ({} mesh, {} tris)",
                self.ray_queries, self.mesh_casts, self.triangles
            ));
        }
        if self.worst_cast_triangles > 0 {
            let on = self
                .worst_cast_instance
                .map_or("no instance".to_owned(), |i| format!("instance {i}"));
            let [x, y, z] = self.worst_cast_from;
            parts.push(format!(
                "dearest cast {} tris on {on} from ({x:.0}, {y:.0}, {z:.0}) over {:.0} m",
                self.worst_cast_triangles, self.worst_cast_range
            ));
        }
        parts.join(", ")
    }
}

/// A bot's think shorter than this is not worth its name in the log (ms).
pub const NAMED_THINK_MS: f32 = 1.0;

/// The class of unit `index` (`rep_inf_ep3_rifleman`), for [`TickCosts::add`].
pub fn unit_name(walk: &crate::walk::WalkSim, index: u32) -> String {
    let Some(u) = walk.sim.units.get(index as usize) else {
        return String::new();
    };
    walk.teams
        .get(usize::from(u.team))
        .and_then(|t| t.classes.get(usize::from(u.spec.class)))
        .map_or_else(String::new, |c| c.name.clone())
}

/// One frame's breakdown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Seconds since the battle's first frame, at this frame's start.
    pub t: f32,
    /// The sim tick when the frame ended.
    pub tick: u64,
    /// This frame's start to the next frame's start, ms.
    pub ms: f32,
    /// The frame's phases in their order, ms; `other` = `ms` less their sum.
    pub laps: Vec<(String, f32)>,
    /// Sim ticks run in the frame, the AI's ms of them
    /// (`Sim::ai_wall_ms`) and the per-tick hook's (sounds, effects, black
    /// box) - both inside the `sim` lap.
    pub ticks: u32,
    pub ai_ms: f32,
    pub hook_ms: f32,
    /// `render_frame`'s wait for the window's image, ms (inside `render`).
    pub acquire_ms: f32,
    /// The GPU's ms (0 without `OPENBF_GPU_TIMING=1`; a frame or two old).
    pub gpu_ms: f32,
    /// The GPU objects made in the frame.
    pub made: Vec<MadeRow>,
    /// Of `ms`, the main thread's time ON a CPU and its time runnable but
    /// WAITING for one (Linux `/proc/thread-self/schedstat`; 0 elsewhere):
    /// a slow frame with a long wait was the machine's other work, one with
    /// neither was asleep (the image, the GPU, the disk).
    #[serde(default)]
    pub cpu_ms: f32,
    #[serde(default)]
    pub queued_ms: f32,
    /// Files read or opened in the frame (`pak::reads`, any thread) and
    /// their bytes.
    #[serde(default)]
    pub files: u32,
    #[serde(default)]
    pub file_bytes: u64,
    /// What the frame's sim ticks cost.
    #[serde(default)]
    pub cost: TickCosts,
}

impl FrameRow {
    /// The phase that took longest, with its ms (`other` included).
    pub fn top(&self) -> (&str, f32) {
        self.laps
            .iter()
            .map(|(n, ms)| (n.as_str(), *ms))
            .fold(("", 0.0), |a, b| if b.1 > a.1 { b } else { a })
    }

    /// One line: when, how long, the phases, the ticks and what was made.
    pub fn line(&self) -> String {
        let laps: Vec<String> = self
            .laps
            .iter()
            .filter(|l| l.1 >= 0.05)
            .map(|(n, ms)| format!("{n} {ms:.1}"))
            .collect();
        let mut s = format!(
            "t {:7.2} s tick {:6}  {:6.1} ms  [{}]  ticks {} (ai {:.1}, hook {:.1})  acquire {:.1}  on cpu {:.1} queued {:.1}",
            self.t,
            self.tick,
            self.ms,
            laps.join(", "),
            self.ticks,
            self.ai_ms,
            self.hook_ms,
            self.acquire_ms,
            self.cpu_ms,
            self.queued_ms
        );
        if self.gpu_ms > 0.0 {
            s += &format!("  gpu {:.1}", self.gpu_ms);
        }
        if self.files > 0 {
            s += &format!("  files {} ({} kB)", self.files, self.file_bytes / 1024);
        }
        let cost = self.cost.line();
        if !cost.is_empty() {
            s += &format!("  {cost}");
        }
        if !self.made.is_empty() {
            let bytes: u64 = self.made.iter().map(|m| m.bytes).sum();
            let ms: f32 = self.made.iter().map(|m| m.ms).sum();
            let mut names: Vec<String> = Vec::new();
            for m in &self.made {
                let n = format!("{} {}", m.kind, m.label);
                if !names.contains(&n) && names.len() < 4 {
                    names.push(n);
                }
            }
            s += &format!(
                "  made {} ({:.1} MB, {ms:.1} ms: {}{})",
                self.made.len(),
                bytes as f64 / (1024.0 * 1024.0),
                names.join(", "),
                if self.made.len() > names.len() {
                    ", ..."
                } else {
                    ""
                }
            );
        }
        s
    }
}

/// GPU objects of one kind and label made past the settle time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MadeCount {
    pub kind: String,
    pub label: String,
    pub count: u32,
    pub bytes: u64,
    /// Their ms together.
    #[serde(default)]
    pub ms: f32,
}

/// The battle's frame-time record as the black box writes it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hitches {
    pub limit_ms: f32,
    pub settle_s: f32,
    /// Frames recorded and the seconds they span.
    pub frames: u64,
    pub seconds: f32,
    pub p50_ms: f32,
    pub p99_ms: f32,
    pub worst_ms: f32,
    /// Frames over `limit_ms` past `settle_s`, and per minute of that time.
    pub over: u32,
    pub per_min: f32,
    /// Frames past `settle_s` that made GPU objects.
    pub made_frames: u32,
    /// Files read or opened past `settle_s`, and in how many frames.
    #[serde(default)]
    pub files: u32,
    #[serde(default)]
    pub file_frames: u32,
    /// Every GPU object made past `settle_s`, by kind and label: how many
    /// and their bytes (a first-use upload shows here slow or not).
    #[serde(default)]
    pub made: Vec<MadeCount>,
    /// The newest frames over `limit_ms` (the first seconds' too), oldest
    /// first.
    pub slow: Vec<FrameRow>,
    /// The longest frames past `settle_s`, longest first.
    pub worst: Vec<FrameRow>,
}

impl Hitches {
    /// The one-line summary: frames, p50 / p99 / worst ms, hitches per
    /// minute, and what the worst five were spent on.
    pub fn summary(&self) -> String {
        let worst: Vec<String> = self
            .worst
            .iter()
            .map(|w| {
                let (top, ms) = w.top();
                format!("{:.0} ms {top} {ms:.0} queued {:.0}", w.ms, w.queued_ms)
            })
            .collect();
        format!(
            "frames {} in {:.0} s: p50 {:.1} ms, p99 {:.1}, worst {:.1}; {} over {:.0} ms past {:.0} s ({:.2}/min); {} frames made GPU objects, {} read {} files; worst: {}",
            self.frames,
            self.seconds,
            self.p50_ms,
            self.p99_ms,
            self.worst_ms,
            self.over,
            self.limit_ms,
            self.settle_s,
            self.per_min,
            self.made_frames,
            self.file_frames,
            self.files,
            if worst.is_empty() {
                "-".to_owned()
            } else {
                worst.join("; ")
            }
        )
    }
}

/// The record a battle builds, shared with the black box (its snapshot reads
/// it, the panic hook's too).
#[derive(Debug, Clone)]
pub struct Record {
    hist: Vec<u32>,
    frames: u64,
    seconds: f64,
    /// Seconds recorded past the settle time.
    settled: f64,
    over: u32,
    made_frames: u32,
    made: Vec<MadeCount>,
    files: u32,
    file_frames: u32,
    worst_ms: f32,
    slow: VecDeque<FrameRow>,
    worst: Vec<FrameRow>,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            hist: vec![0; HIST_MS * BINS_PER_MS + 1],
            frames: 0,
            seconds: 0.0,
            settled: 0.0,
            over: 0,
            made_frames: 0,
            made: Vec::new(),
            files: 0,
            file_frames: 0,
            worst_ms: 0.0,
            slow: VecDeque::new(),
            worst: Vec::new(),
        }
    }
}

impl Record {
    /// Count one finished frame (its `ms` known).
    pub fn push(&mut self, row: FrameRow) {
        self.frames += 1;
        self.seconds += f64::from(row.ms) / 1000.0;
        let bin = ((row.ms.fmax(0.0) * BINS_PER_MS as f32) as usize).min(self.hist.len() - 1);
        self.hist[bin] += 1;
        let settled = row.t >= SETTLE_S;
        if settled {
            self.settled += f64::from(row.ms) / 1000.0;
            self.worst_ms = self.worst_ms.fmax(row.ms);
            self.made_frames += u32::from(!row.made.is_empty());
            self.files += row.files;
            self.file_frames += u32::from(row.files > 0);
            for m in &row.made {
                match self
                    .made
                    .iter_mut()
                    .find(|c| c.kind == m.kind && c.label == m.label)
                {
                    Some(c) => {
                        c.count += 1;
                        c.bytes += m.bytes;
                        c.ms += m.ms;
                    }
                    None => self.made.push(MadeCount {
                        kind: m.kind.clone(),
                        label: m.label.clone(),
                        count: 1,
                        bytes: m.bytes,
                        ms: m.ms,
                    }),
                }
            }
        }
        let slow = row.ms > HITCH_MS;
        if settled && slow {
            self.over += 1;
        }
        let worst = settled
            && (self.worst.len() < WORST_KEPT || self.worst.last().is_some_and(|w| row.ms > w.ms));
        match (slow, worst) {
            (false, false) => {}
            (true, false) => self.keep_slow(row),
            (false, true) => self.keep_worst(row),
            (true, true) => {
                self.keep_worst(row.clone());
                self.keep_slow(row);
            }
        }
    }

    fn keep_slow(&mut self, row: FrameRow) {
        if self.slow.len() == SLOW_KEPT {
            self.slow.pop_front();
        }
        self.slow.push_back(row);
    }

    fn keep_worst(&mut self, row: FrameRow) {
        let at = self.worst.partition_point(|w| w.ms >= row.ms);
        self.worst.insert(at, row);
        self.worst.truncate(WORST_KEPT);
    }

    /// The ms under which a share `q` of the frames fall (a bin's top).
    fn quantile(&self, q: f64) -> f32 {
        let want = (self.frames as f64 * q).ceil().fmax(1.0) as u64;
        let mut seen = 0u64;
        for (i, n) in self.hist.iter().enumerate() {
            seen += u64::from(*n);
            if seen >= want {
                return (i + 1) as f32 / BINS_PER_MS as f32;
            }
        }
        0.0
    }

    pub fn hitches(&self) -> Hitches {
        let r2 = |v: f32| (v * 100.0).round() / 100.0;
        Hitches {
            limit_ms: HITCH_MS,
            settle_s: SETTLE_S,
            frames: self.frames,
            seconds: r2(self.seconds as f32),
            p50_ms: r2(self.quantile(0.5)),
            p99_ms: r2(self.quantile(0.99)),
            worst_ms: r2(self.worst_ms),
            over: self.over,
            per_min: match self.settled > 0.0 {
                true => r2((f64::from(self.over) * 60.0 / self.settled) as f32),
                false => 0.0,
            },
            made_frames: self.made_frames,
            made: self.made.clone(),
            files: self.files,
            file_frames: self.file_frames,
            slow: self.slow.iter().cloned().collect(),
            worst: self.worst.clone(),
        }
    }
}

/// The CSV's writer (`OPENBF_FRAME_PROFILE`).
struct Csv {
    out: std::io::BufWriter<std::fs::File>,
    /// The lap columns, from the first row.
    columns: Vec<&'static str>,
    frame: u64,
}

pub struct FrameProfile {
    csv: Option<Csv>,
    record: Option<Arc<Mutex<Record>>>,
    /// The battle's first frame.
    started: Option<Instant>,
    /// This frame's start, and the last lap's end.
    start: Instant,
    t: Instant,
    laps: Vec<(&'static str, f64)>,
    ticks: u32,
    ai_ms: f64,
    hook_ms: f64,
    cost: TickCosts,
    /// The last frame, waiting for this one's start to know its length.
    pending: Option<(Instant, FrameRow)>,
    /// `pak::reads` at this frame's start.
    reads: (u64, u64),
    /// The main thread's (on-CPU, queued) ns at this frame's start.
    sched: (u64, u64),
    /// This frame's drawn camera and player (`FrameProfile::drawn`).
    drawn: Option<(glam::Vec3, glam::Vec3)>,
}

/// The calling thread's (on-CPU, runnable-but-waiting) ns so far, from
/// `/proc/thread-self/schedstat`; (0, 0) where there is none.
fn schedstat() -> (u64, u64) {
    let Ok(t) = std::fs::read_to_string("/proc/thread-self/schedstat") else {
        return (0, 0);
    };
    let mut it = t.split_whitespace().map(|v| v.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn locked(r: &Mutex<Record>) -> std::sync::MutexGuard<'_, Record> {
    r.lock().unwrap_or_else(|e| e.into_inner())
}

/// The record's switch: dev mode, and `OPENBF_FRAME_RECORD` not `0`.
pub fn record_on() -> bool {
    crate::dev::on() && std::env::var("OPENBF_FRAME_RECORD").map_or(true, |v| v != "0")
}

impl FrameProfile {
    /// The profile the environment asks for: the record in dev mode, the CSV
    /// `OPENBF_FRAME_PROFILE` names (a file that will not open is reported
    /// and leaves the CSV off); None when neither is on.
    pub fn from_env() -> Option<Self> {
        let csv = std::env::var_os("OPENBF_FRAME_PROFILE").and_then(|path| {
            match std::fs::File::create(&path) {
                Ok(f) => Some(Csv {
                    out: std::io::BufWriter::new(f),
                    columns: Vec::new(),
                    frame: 0,
                }),
                Err(e) => {
                    eprintln!("OPENBF_FRAME_PROFILE {}: {e}", path.to_string_lossy());
                    None
                }
            }
        });
        let record = record_on().then(|| Arc::new(Mutex::new(Record::default())));
        Self::from_outputs(csv, record)
    }

    fn from_outputs(csv: Option<Csv>, record: Option<Arc<Mutex<Record>>>) -> Option<Self> {
        if csv.is_none() && record.is_none() {
            return None;
        }
        render::made::enable(csv.is_some() || record.is_some());
        let now = Instant::now();
        Some(Self {
            csv,
            record,
            started: None,
            start: now,
            t: now,
            laps: Vec::new(),
            ticks: 0,
            ai_ms: 0.0,
            hook_ms: 0.0,
            cost: TickCosts::default(),
            pending: None,
            reads: pak::reads(),
            sched: (0, 0),
            drawn: None,
        })
    }

    /// The record, for the black box's snapshot (None with the record off).
    pub fn record(&self) -> Option<Arc<Mutex<Record>>> {
        self.record.clone()
    }

    /// A new battle: the record starts over (a new one, so a snapshot of the
    /// last battle still being written keeps its own), and what the loading
    /// made is not this battle's first frame.
    pub fn reset(&mut self) {
        if let Some(r) = self.record.as_mut() {
            *r = Arc::new(Mutex::new(Record::default()));
        }
        self.started = None;
        self.pending = None;
        let _ = render::made::take();
    }

    /// The record's summary now (None with the record off).
    pub fn hitches(&self) -> Option<Hitches> {
        self.record.as_ref().map(|r| locked(r).hitches())
    }

    /// The start of a frame's work; the last frame's length is known now.
    pub fn begin(&mut self) {
        let now = Instant::now();
        let sched = match self.record.is_some() {
            true => schedstat(),
            false => (0, 0),
        };
        if let Some((start, mut row)) = self.pending.take() {
            row.ms = ((now - start).as_secs_f64() * 1000.0) as f32;
            // On-CPU and queued time up to this frame's start (the event
            // loop's share after `end` included).
            row.cpu_ms = sched.0.saturating_sub(self.sched.0) as f32 / 1e6;
            row.queued_ms = sched.1.saturating_sub(self.sched.1) as f32 / 1e6;
            let other = row.ms - row.laps.iter().map(|l| l.1).sum::<f32>();
            row.laps.push(("other".to_owned(), other.fmax(0.0)));
            if let Some(r) = &self.record {
                locked(r).push(row);
            }
        }
        self.started.get_or_insert(now);
        self.start = now;
        self.t = now;
        self.laps.clear();
        self.ticks = 0;
        self.ai_ms = 0.0;
        self.hook_ms = 0.0;
        self.cost = TickCosts::default();
        self.reads = pak::reads();
        self.sched = sched;
    }

    /// The end of phase `name`: the time since the last lap (or `begin`).
    pub fn lap(&mut self, name: &'static str) {
        let now = Instant::now();
        self.laps
            .push((name, (now - self.t).as_secs_f64() * 1000.0));
        self.t = now;
    }

    /// The frame's drawn camera place and the drawn player's feet (the
    /// CSV's `cam_*` / `me_*`, card RINT).
    pub fn drawn(&mut self, camera: glam::Vec3, player: glam::Vec3) {
        self.drawn = Some((camera, player));
    }

    /// One sim tick ran in this frame: its AI ms, its hook's and what it cost
    /// (`name` names a bot for [`TickCosts::add`]).
    pub fn tick(
        &mut self,
        ai_ms: f32,
        hook_ms: f64,
        cost: &core::sim::TickCost,
        name: impl FnOnce(u32) -> String,
    ) {
        self.ticks += 1;
        self.ai_ms += f64::from(ai_ms);
        self.hook_ms += hook_ms;
        self.cost.add(cost, name);
    }

    /// The frame's work is done (`tick` = the sim's tick now): write the
    /// CSV row; the record's row waits for the next frame's start.
    pub fn end(&mut self, dt_ms: f64, tick: u64, r: &render::Renderer) {
        let gpu = r.gpu_sections();
        let gpu_ms: f64 = gpu.iter().map(|g| g.1).sum();
        let made = render::made::take();
        if let Some(c) = self.csv.as_mut() {
            if c.columns.is_empty() {
                c.columns = self.laps.iter().map(|l| l.0).collect();
                let _ = writeln!(
                    c.out,
                    "frame,dt_ms,{},acquire_ms,draw_calls,gpu_ms,gpu,ticks,ai_ms,hook_ms,made,cam_x,cam_y,cam_z,me_x,me_y,me_z",
                    c.columns.join(",")
                );
            }
            let laps: Vec<String> = c
                .columns
                .iter()
                .map(|col| {
                    self.laps
                        .iter()
                        .find(|l| l.0 == *col)
                        .map_or(String::new(), |l| format!("{:.3}", l.1))
                })
                .collect();
            let parts: Vec<String> = gpu.iter().map(|(l, ms)| format!("{l}={ms:.3}")).collect();
            let drawn = self.drawn.take().map_or(",,,,,".to_owned(), |(c, m)| {
                format!("{},{},{},{},{},{}", c.x, c.y, c.z, m.x, m.y, m.z)
            });
            let _ = writeln!(
                c.out,
                "{},{dt_ms:.3},{},{:.3},{},{gpu_ms:.3},{},{},{:.3},{:.3},{},{drawn}",
                c.frame,
                laps.join(","),
                r.acquire_ms(),
                r.frame_stats().draw_calls,
                parts.join(";"),
                self.ticks,
                self.ai_ms,
                self.hook_ms,
                made.len()
            );
            c.frame += 1;
        }
        if self.record.is_some() {
            let now = pak::reads();
            let since = self.started.map_or(0.0, |s| (self.start - s).as_secs_f32());
            let row = FrameRow {
                t: since,
                tick,
                ms: 0.0,
                laps: self
                    .laps
                    .iter()
                    .map(|(n, ms)| ((*n).trim_end_matches("_ms").to_owned(), *ms as f32))
                    .collect(),
                ticks: self.ticks,
                ai_ms: self.ai_ms as f32,
                hook_ms: self.hook_ms as f32,
                acquire_ms: r.acquire_ms() as f32,
                gpu_ms: gpu_ms as f32,
                made: made
                    .into_iter()
                    .map(|m| MadeRow {
                        kind: m.kind.to_owned(),
                        label: m.label,
                        bytes: m.bytes,
                        ms: m.ms,
                    })
                    .collect(),
                cpu_ms: 0.0,
                queued_ms: 0.0,
                files: (now.0 - self.reads.0) as u32,
                file_bytes: now.1 - self.reads.1,
                cost: std::mem::take(&mut self.cost),
            };
            self.pending = Some((self.start, row));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_output_collects_object_notes_and_reset_drops_loading_notes() {
        // Other game tests decode sounds and can note into the process-wide
        // collector. Run this case alone in a child, including in parallel suites.
        const CHILD: &str = "OPENBF_PROFILE_NOTE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "frame_profile::tests::either_output_collects_object_notes_and_reset_drops_loading_notes"])
                .env(CHILD, "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        // Exercise all output combinations at the constructor seam used by
        // from_env. Preserve the global recorder.
        let was_on = render::made::on();
        let saved = render::made::take();
        for (csv_on, record_on) in [(false, false), (true, false), (false, true), (true, true)] {
            render::made::enable(false);
            let file =
                std::env::temp_dir().join(format!("openbf-profile-{}.csv", std::process::id()));
            let csv = csv_on.then(|| Csv {
                out: std::io::BufWriter::new(std::fs::File::create(&file).unwrap()),
                columns: Vec::new(),
                frame: 0,
            });
            let record = record_on.then(|| Arc::new(Mutex::new(Record::default())));
            let mut profile = FrameProfile::from_outputs(csv, record);
            assert_eq!(profile.is_some(), csv_on || record_on);
            assert_eq!(render::made::on(), csv_on || record_on);
            render::made::note("buffer", Some("synthetic"), 4, 0.0);
            let made = render::made::take();
            assert_eq!(made.len(), usize::from(csv_on || record_on));
            if let Some(p) = profile.as_mut() {
                assert_eq!(p.record().is_some(), record_on);
                render::made::note("texture", Some("loading"), 4, 0.0);
                p.reset();
                assert!(render::made::take().is_empty());
            }
            drop(profile);
            let _ = std::fs::remove_file(file);
        }
        render::made::enable(true);
        for m in saved {
            render::made::note(m.kind, Some(&m.label), m.bytes, m.ms);
        }
        render::made::enable(was_on);
    }

    fn row(t: f32, ms: f32) -> FrameRow {
        FrameRow {
            t,
            ms,
            laps: vec![("render".into(), ms * 0.5), ("other".into(), ms * 0.5)],
            ..FrameRow::default()
        }
    }

    /// The log names the worst bot of the frame's ticks and adds up their rays.
    #[test]
    fn a_frames_ticks_add_up_and_the_worst_bot_is_named() {
        use core::collision::cost::{RayCost, WorstCast};
        use core::sim::{TickCost, UnitCost};
        let tick = |unit, ms, tris| TickCost {
            worst_think: Some(UnitCost {
                unit,
                ms,
                ray_queries: 40,
            }),
            rays: RayCost {
                queries: 100,
                mesh_casts: 10,
                triangles: tris,
                worst: Some(WorstCast {
                    triangles: tris,
                    instance: Some(31),
                    origin: [1.0, 2.0, 3.0],
                    range: 50.0,
                }),
            },
        };
        let mut c = TickCosts::default();
        c.add(&tick(3, 0.5, 20), |_| panic!("too short to name"));
        c.add(&tick(12, 3.0, 400), |u| format!("trooper{u}"));
        c.add(&tick(7, 2.0, 30), |_| panic!("not the worst"));
        assert_eq!(c.ray_queries, 300);
        assert_eq!(c.triangles, 450);
        assert_eq!(
            c.line(),
            "worst unit trooper12 #12 3.0 ms (40 rays), rays 300 (30 mesh, 450 tris), \
             dearest cast 400 tris on instance 31 from (1, 2, 3) over 50 m"
        );
        assert_eq!(TickCosts::default().line(), "");
    }

    /// An older snapshot without the new fields still reads.
    #[test]
    fn a_row_from_before_the_costs_still_reads() {
        let mut old = serde_json::to_value(FrameRow::default()).unwrap();
        old.as_object_mut().unwrap().remove("cost");
        let row: FrameRow = serde_json::from_value(old).unwrap();
        assert_eq!(row.cost, TickCosts::default());
    }

    #[test]
    fn the_record_counts_hitches_past_the_settle_time_only() {
        let mut r = Record::default();
        // 10 s of 60 Hz frames, with a 40 ms frame at 2 s (settling) and
        // two at 6 s and 8 s.
        let mut t = 0.0;
        for i in 0..600 {
            let ms = match i {
                120 | 360 | 480 => 40.0 + i as f32 / 100.0,
                _ => 16.7,
            };
            r.push(row(t, ms));
            t += ms / 1000.0;
        }
        let h = r.hitches();
        assert_eq!(h.frames, 600);
        assert_eq!(h.over, 2, "the one in the first 5 s is not counted");
        assert_eq!(h.slow.len(), 3, "but it is kept");
        assert_eq!(h.worst.len(), WORST_KEPT);
        assert!(
            (h.worst[0].ms - 44.8).abs() < 1e-3,
            "longest first: {}",
            h.worst[0].ms
        );
        assert!((h.worst[1].ms - 43.6).abs() < 1e-3);
        assert!((h.p50_ms - 16.7).abs() < 0.11, "p50 {}", h.p50_ms);
        assert!((h.worst_ms - 44.8).abs() < 0.01);
        // 2 hitches in the ~5 s past the settle time.
        assert!(h.per_min > 20.0 && h.per_min < 30.0, "{}", h.per_min);
        assert!(h.summary().contains("2 over 25 ms"), "{}", h.summary());
    }

    #[test]
    fn the_slow_list_keeps_the_newest() {
        let mut r = Record::default();
        for i in 0..(SLOW_KEPT + 10) {
            r.push(row(10.0 + i as f32, 30.0));
        }
        let h = r.hitches();
        assert_eq!(h.slow.len(), SLOW_KEPT);
        assert_eq!(h.slow[0].t, 20.0, "the ten oldest are gone");
        assert_eq!(h.over as usize, SLOW_KEPT + 10, "but all are counted");
    }
}
