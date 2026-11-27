//! Automatic quality (LAN §8b.2, issue #168): when a PC cannot keep up, the
//! picture gets softer so the battle keeps running, like a map that loads a
//! blurry tile before a sharp one. On by default (Options → Graphics →
//! Automatic quality); Off keeps the player's own choice.
//!
//! Rules:
//! - The signal is the 95th percentile of the frame times over the last
//!   [`WINDOW_S`] seconds, against the frame budget (the display's period, at
//!   most 60 a second: [`crate::dyn_res::display_budget_ms`]).
//! - Over: the p95 is more than [`OVER`] × budget with a full window of
//!   samples taken since the last change. One step down, then a fresh window.
//! - Headroom: the p95 is at most [`HEADROOM`] × budget. After [`UP_AFTER_S`]
//!   seconds of headroom in a row, one step up, on trial for [`PROBE_S`]. A
//!   trial that goes over steps back down and waits [`HOLD_S`] before the next
//!   try, twice as long for each failure in a row, up to [`HOLD_MAX_S`]. The
//!   band between [`HEADROOM`] and [`OVER`] changes nothing. Together these
//!   are the hysteresis: the level never flickers between two steps.
//! - Under vsync a frame never shows its spare time (every frame rounds up to
//!   the display's period), so a step up is a trial, not a measurement.
//! - The ladder, top to bottom: the render resolution scale ([`SCALES`]: the
//!   3D world draws into a smaller target and is scaled up; the HUD and menus
//!   stay at the window's size, `render::world_scale`), then the graphics
//!   tiers through a [`TierHook`] (shadows, draw distance, LOD bias,
//!   particles, foliage; none exist yet, issues #93 and #128). Steps down go
//!   down the ladder and steps up come back the same way.
//! - CPU-bound frames step nothing: when the frame's own work without the
//!   wait for the display (the caller's `busy_ms`) is over budget too, and
//!   the frame hardly waits for the display's image (the caller's
//!   `wait_ms`, its p95 at most [`WAIT_SHARE`] × budget), a smaller picture
//!   would not help, only blur it (a host whose sim and bots fill the
//!   frame). The note says "CPU-bound" instead. A frame that is over budget
//!   on the CPU and still waits for the image has the GPU as part of its
//!   limit, and steps down as a GPU-bound one does.
//! - The first [`WARMUP_S`] seconds of a battle do not count: its first
//!   frames upload what the loading left. [`Governor::restart`] starts a new
//!   battle's warm-up and keeps the level it learned.
//! - Nothing here reaches the sim: it reads frame times and moves only the
//!   renderer, so it is safe in a LAN game (every peer may draw differently).

use core::fmath::FloatExt;
use std::collections::VecDeque;

/// The render resolution scales, largest first. PROVISIONAL, measure: where
/// the world looks too soft on a 1080p panel, and what each step saves on a
/// weak integrated GPU. 0.7 took 39% off the GPU frame on this PC's Intel GPU
/// at 1080p (BF2 Hoth, 8.87 -> 5.44 ms, `crate::dyn_res`).
pub const SCALES: [f32; 5] = [1.0, 0.85, 0.7, 0.6, 0.5];

/// The sliding window the p95 is taken over.
pub const WINDOW_S: f32 = 3.0;
/// A p95 above budget × this is over budget. OURS: 1.2 lets a 60 Hz frame
/// jitter to 20 ms (50 a second) before quality drops; a missed vsync
/// (33 ms) is far over. measure: frame jitter on a compositor under load.
pub const OVER: f32 = 1.2;
/// A p95 at or below budget × this is headroom.
pub const HEADROOM: f32 = 1.1;
/// Seconds of headroom in a row before a step up is tried.
pub const UP_AFTER_S: f32 = 10.0;
/// A step up that holds this long has held.
pub const PROBE_S: f32 = 6.0;
/// The wait after a failed step up, doubled per failure in a row.
pub const HOLD_S: f32 = 20.0;
pub const HOLD_MAX_S: f32 = 160.0;
/// Seconds at a battle's start before anything counts.
pub const WARMUP_S: f32 = 4.0;
/// A single frame counts as at most this long: an alt-tab or a disk stall
/// is one late frame, not a measure of the GPU.
const SAMPLE_CAP_MS: f32 = 250.0;
/// A wait-for-the-image p95 above budget × this means the GPU is part of
/// the frame's limit (module rules, CPU-bound). OURS, PROVISIONAL, measure:
/// set from littlemallard's headless pipelined bench, which has no vsync and
/// whose wait is for a frame in flight, not the window's acquire. There a
/// frame paced by its own work alone waits next to nothing (kas2 XL four
/// views: p95 under 0.01 ms) and one the GPU holds back waits several ms
/// (Mustafar four views: p95 8 ms). Untested under the window's Fifo: a
/// CPU-bound frame over budget that the swapchain paces to the next vblank
/// may wait in the acquire too, which this rule would read as the GPU's. A
/// windowed round with `OPENBF_FRAME_PROFILE` (acquire p95 on Mustafar and
/// kas2 XL four views) decides it.
pub const WAIT_SHARE: f32 = 0.25;
/// The CPU-bound summary's period ([`Note::CpuBound`]).
pub const SUMMARY_S: f32 = 60.0;

/// The graphics tiers' side of the ladder (issues #93 and #128). A tier
/// implementation reports how many steps below the player's chosen tier it
/// has, and applies a number of steps down (0 = the player's choice). It
/// must change only what the renderer draws, never what the sim reads.
pub trait TierHook {
    /// Steps available below the player's chosen tier.
    fn steps(&self) -> usize;
    /// Draw `down` steps below the player's chosen tier.
    fn apply(&mut self, down: usize);
}

/// No graphics tiers yet: the ladder is the render scale alone.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoTiers;

impl TierHook for NoTiers {
    fn steps(&self) -> usize {
        0
    }
    fn apply(&mut self, _down: usize) {}
}

/// What the governor asks the renderer for this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quality {
    /// The fraction of the window the world draws at.
    pub scale: f32,
    /// Graphics tier steps below the player's choice.
    pub tier_down: usize,
    /// The last window was over budget on the CPU: nothing stepped.
    pub cpu_bound: bool,
}

impl Quality {
    /// The fps counter's and the net meter's note, e.g. "res 85%" or
    /// "res 50%, tier -1".
    pub fn label(&self) -> String {
        let mut out = format!("res {:.0}%", self.scale * 100.0);
        if self.tier_down > 0 {
            out += &format!(", tier -{}", self.tier_down);
        }
        if self.cpu_bound {
            out += ", CPU-bound";
        }
        out
    }
}

/// What the governor did, for the session log ([`Governor::take_notes`]):
/// a step, a failed step up and, at most once a [`SUMMARY_S`], how long the
/// frames were CPU-bound. Nothing per window, so the log cannot flap.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    /// The level changed: the quality now, its steps down the ladder, and
    /// the window that decided it.
    Step {
        quality: Quality,
        steps: usize,
        p95: f32,
        window: Window,
    },
    /// A step up on trial at `quality` went over budget and stepped back.
    FailedUp { quality: Quality, p95: f32 },
    /// Of the last `period_s` seconds of counted frames, `seconds` were in
    /// CPU-bound windows; the window then.
    CpuBound {
        seconds: f32,
        period_s: f32,
        p95: f32,
        window: Window,
    },
}

/// The window's busy and wait p95s, when the caller gave them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Window {
    pub busy_p95: Option<f32>,
    pub wait_p95: Option<f32>,
}

fn ms(v: Option<f32>) -> String {
    v.map_or_else(|| "-".to_owned(), |v| format!("{v:.1}"))
}

impl Note {
    /// The session log's line, `map` and `views` the battle's.
    pub fn line(&self, map: &str, views: usize) -> String {
        match self {
            Note::Step {
                quality,
                steps,
                p95,
                window,
            } => format!(
                "quality: map {map} views {views} scale {:.2} tier -{} steps {steps} cpu_bound {} p95 {p95:.1} busy p95 {} wait p95 {}",
                quality.scale,
                quality.tier_down,
                quality.cpu_bound,
                ms(window.busy_p95),
                ms(window.wait_p95)
            ),
            Note::FailedUp { quality, p95 } => format!(
                "quality: map {map} views {views} step up to scale {:.2} tier -{} failed, p95 {p95:.1}",
                quality.scale, quality.tier_down
            ),
            Note::CpuBound {
                seconds,
                period_s,
                p95,
                window,
            } => format!(
                "quality: map {map} views {views} cpu_bound {seconds:.0} of {period_s:.0} s, p95 {p95:.1} busy p95 {} wait p95 {}",
                ms(window.busy_p95),
                ms(window.wait_p95)
            ),
        }
    }
}

/// The governor (module rules).
#[derive(Debug, Clone)]
pub struct Governor {
    /// Steps down the ladder: `0..SCALES.len()` are scales, beyond that
    /// tier steps at the lowest scale.
    level: usize,
    /// (seconds since the window began, frame ms, busy ms, wait ms),
    /// oldest first.
    samples: VecDeque<(f32, f32, Option<f32>, Option<f32>)>,
    /// Seconds of frames since the last change (or the restart).
    clock: f32,
    /// Seconds of frames the warm-up still ignores.
    warmup: f32,
    headroom_s: f32,
    /// Seconds since a step up, while it is on trial.
    probing: Option<f32>,
    hold: f32,
    failures: u32,
    cpu_bound: bool,
    /// The quality on trial, while a step up is.
    trial: Option<Quality>,
    /// Counted seconds since the last CPU-bound summary, and of them the
    /// CPU-bound ones.
    summary_s: f32,
    cpu_bound_s: f32,
    notes: Vec<Note>,
}

impl Default for Governor {
    fn default() -> Self {
        Self {
            level: 0,
            samples: VecDeque::new(),
            clock: 0.0,
            warmup: WARMUP_S,
            headroom_s: 0.0,
            probing: None,
            hold: 0.0,
            failures: 0,
            cpu_bound: false,
            trial: None,
            summary_s: 0.0,
            cpu_bound_s: 0.0,
            notes: Vec::new(),
        }
    }
}

impl Governor {
    /// A new battle: its warm-up and a fresh window; the level stays.
    pub fn restart(&mut self) {
        self.samples.clear();
        self.clock = 0.0;
        self.warmup = WARMUP_S;
        self.headroom_s = 0.0;
        self.probing = None;
        self.trial = None;
        self.summary_s = 0.0;
        self.cpu_bound_s = 0.0;
    }

    /// What it did since the last call (the session log's lines, [`Note`]).
    pub fn take_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    /// The level now, 0 = full quality.
    pub fn level(&self) -> usize {
        self.level
    }

    /// The quality at the current level with `tier_steps` tier steps below
    /// the player's choice.
    pub fn quality(&self, tier_steps: usize) -> Quality {
        let last = SCALES.len() - 1;
        Quality {
            scale: SCALES[self.level.min(last)],
            tier_down: self.level.saturating_sub(last).min(tier_steps),
            cpu_bound: self.cpu_bound,
        }
    }

    /// The p95 of the window's frame times in ms, or `None` before a full
    /// window.
    pub fn p95(&self) -> Option<f32> {
        if self.clock < WINDOW_S {
            return None;
        }
        p95(self.samples.iter().map(|s| s.1).collect())
    }

    /// The p95s of the window's busy and wait times, when the caller gave
    /// them.
    fn window(&self) -> Window {
        Window {
            busy_p95: p95(self.samples.iter().filter_map(|s| s.2).collect()),
            wait_p95: p95(self.samples.iter().filter_map(|s| s.3).collect()),
        }
    }

    /// One frame of `dt_ms` held to `budget_ms`, with `tier_steps` tier
    /// steps on the ladder below the scales. `busy_ms`: the frame's own work
    /// without its wait for the display, and `wait_ms` that wait (the
    /// acquire of the window's image), when known (module rules). Returns
    /// the quality to draw the next frame at.
    pub fn frame(
        &mut self,
        dt_ms: f32,
        busy_ms: Option<f32>,
        wait_ms: Option<f32>,
        budget_ms: f32,
        tier_steps: usize,
    ) -> Quality {
        let dt_ms = dt_ms.clamp(0.0, SAMPLE_CAP_MS);
        let dt = dt_ms / 1000.0;
        let floor = SCALES.len() - 1 + tier_steps;
        // Tiers that went away (a setting change) cannot be stood on.
        self.level = self.level.min(floor);
        self.hold = (self.hold - dt).fmax(0.0);
        if self.warmup > 0.0 {
            self.warmup -= dt;
            return self.quality(tier_steps);
        }
        self.clock += dt;
        let busy = busy_ms.map(|b| b.clamp(0.0, dt_ms));
        let wait = wait_ms.map(|w| w.clamp(0.0, dt_ms));
        self.samples.push_back((self.clock, dt_ms, busy, wait));
        while self
            .samples
            .front()
            .is_some_and(|s| s.0 <= self.clock - WINDOW_S)
        {
            self.samples.pop_front();
        }
        if let Some(p) = &mut self.probing {
            *p += dt;
            if *p > PROBE_S {
                self.probing = None;
                self.trial = None;
                self.failures = 0;
            }
        }
        let Some(p95) = self.p95() else {
            return self.quality(tier_steps);
        };
        // This frame's window was over budget and CPU-bound (the summary
        // counts these only, not the band after one).
        let mut cpu_bound_now = false;
        if p95 > budget_ms * OVER {
            self.headroom_s = 0.0;
            let window = self.window();
            // Over on the CPU, and the image hardly waited for: the GPU is
            // not part of the limit (module rules).
            let busy_over = window.busy_p95.is_some_and(|b| b > budget_ms * OVER);
            let gpu_waits = window.wait_p95.is_some_and(|w| w > budget_ms * WAIT_SHARE);
            self.cpu_bound = busy_over && !gpu_waits;
            cpu_bound_now = self.cpu_bound;
            if self.level < floor && !self.cpu_bound {
                self.level += 1;
                if self.probing.take().is_some() {
                    self.failures += 1;
                    self.hold = (1..self.failures).fold(HOLD_S, |h, _| (h * 2.0).fmin(HOLD_MAX_S));
                    if let Some(quality) = self.trial.take() {
                        self.notes.push(Note::FailedUp { quality, p95 });
                    }
                }
                self.step_note(tier_steps, p95, window);
                self.changed();
            }
        } else if p95 <= budget_ms * HEADROOM {
            self.cpu_bound = false;
            self.headroom_s += dt;
            if self.headroom_s >= UP_AFTER_S && self.level > 0 && self.hold == 0.0 {
                self.level -= 1;
                self.probing = Some(0.0);
                self.trial = Some(self.quality(tier_steps));
                self.step_note(tier_steps, p95, self.window());
                self.changed();
            }
        } else {
            self.headroom_s = 0.0;
        }
        // The CPU-bound summary, at most once a period.
        self.summary_s += dt;
        if cpu_bound_now {
            self.cpu_bound_s += dt;
        }
        if self.summary_s >= SUMMARY_S {
            if self.cpu_bound_s > 0.0 {
                self.notes.push(Note::CpuBound {
                    seconds: self.cpu_bound_s,
                    period_s: self.summary_s,
                    p95,
                    window: self.window(),
                });
            }
            self.summary_s = 0.0;
            self.cpu_bound_s = 0.0;
        }
        self.quality(tier_steps)
    }

    fn step_note(&mut self, tier_steps: usize, p95: f32, window: Window) {
        self.notes.push(Note::Step {
            quality: self.quality(tier_steps),
            steps: self.level,
            p95,
            window,
        });
    }

    /// A step: the next decision waits for a full window of new frames.
    fn changed(&mut self) {
        self.samples.clear();
        self.clock = 0.0;
        self.headroom_s = 0.0;
    }
}

/// The battle's name in the session log's lines: the map's directory and
/// " xl" for the XL population (`bf2_kas2 xl`), the same in the window and
/// the bench.
pub fn map_label(dir: &std::path::Path, map: &str, xl: bool) -> String {
    let name = dir
        .file_name()
        .map_or_else(|| map.to_owned(), |n| n.to_string_lossy().into_owned());
    format!("{name}{}", if xl { " xl" } else { "" })
}

/// The 95th percentile of `v`, `None` when empty.
fn p95(mut v: Vec<f32>) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    let k = ((v.len() as f32 * 0.95).ceil() as usize).clamp(1, v.len()) - 1;
    let (_, x, _) = v.select_nth_unstable_by(k, f32::total_cmp);
    Some(*x)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGET: f32 = 1000.0 / 60.0;

    /// Feed `seconds` of frames of `ms`; returns each change of level as
    /// (time, level) relative to the call's start.
    fn run(g: &mut Governor, tiers: usize, seconds: f32, ms: f32) -> Vec<(f32, usize)> {
        let mut t = 0.0;
        let mut out = Vec::new();
        let mut level = g.level();
        while t < seconds {
            g.frame(ms, None, None, BUDGET, tiers);
            t += ms / 1000.0;
            if g.level() != level {
                level = g.level();
                out.push((t, level));
            }
        }
        out
    }

    fn levels(changes: &[(f32, usize)]) -> Vec<usize> {
        changes.iter().map(|c| c.1).collect()
    }

    #[test]
    fn warmup_ignores_a_slow_start() {
        let mut g = Governor::default();
        assert!(run(&mut g, 0, WARMUP_S - 0.1, 200.0).is_empty());
        assert_eq!(g.quality(0).scale, 1.0);
    }

    /// A steady 60: never a change, over many minutes.
    #[test]
    fn frames_at_budget_never_change_anything() {
        let mut g = Governor::default();
        assert!(run(&mut g, 2, 600.0, BUDGET).is_empty());
        // Jitter inside the band too: 1 frame in 10 at 19 ms.
        let mut t = 0.0;
        for i in 0..36_000 {
            let ms = if i % 10 == 0 { 19.0 } else { BUDGET };
            g.frame(ms, None, None, BUDGET, 2);
            t += ms;
            assert_eq!(g.level(), 0, "at {t} ms");
        }
    }

    /// A GPU that cannot hold 60: one step per full window, scales first,
    /// then the tiers, then the floor holds.
    #[test]
    fn a_slow_stretch_steps_down_the_scales_then_the_tiers() {
        let mut g = Governor::default();
        run(&mut g, 2, WARMUP_S, BUDGET);
        let changes = run(&mut g, 2, 60.0, 40.0);
        assert_eq!(levels(&changes), vec![1, 2, 3, 4, 5, 6]);
        for pair in changes.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= WINDOW_S - 0.05, "{changes:?}");
        }
        assert_eq!(
            g.quality(2),
            Quality {
                scale: 0.5,
                tier_down: 2,
                cpu_bound: false,
            }
        );
        // No tiers: the floor is the lowest scale.
        let mut g = Governor::default();
        run(&mut g, 0, WARMUP_S, BUDGET);
        assert_eq!(levels(&run(&mut g, 0, 60.0, 40.0)), vec![1, 2, 3, 4]);
        assert_eq!(g.quality(0).tier_down, 0);
    }

    /// A short hitch (under 5% of the window) never steps down.
    #[test]
    fn a_short_hitch_is_not_a_slow_pc() {
        let mut g = Governor::default();
        run(&mut g, 0, WARMUP_S + 5.0, BUDGET);
        for _ in 0..5 {
            g.frame(120.0, None, None, BUDGET, 0);
        }
        assert!(run(&mut g, 0, 30.0, BUDGET).is_empty());
    }

    /// Back up one step after 10 s of headroom, the tiers before the scale;
    /// each step up waits its own 10 s.
    #[test]
    fn headroom_steps_back_up_one_step_at_a_time() {
        let mut g = Governor::default();
        run(&mut g, 1, WARMUP_S, BUDGET);
        run(&mut g, 1, 60.0, 40.0);
        assert_eq!(g.level(), 5);
        let changes = run(&mut g, 1, 120.0, 10.0);
        assert_eq!(levels(&changes), vec![4, 3, 2, 1, 0]);
        // The slow frames age out of the window first, then 10 s of headroom.
        assert!(changes[0].0 >= UP_AFTER_S, "{changes:?}");
        for pair in changes.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= UP_AFTER_S, "{changes:?}");
        }
    }

    /// The flicker case: full quality is just too slow, one step down is
    /// fine. The governor tries the step up, fails, and waits longer each
    /// time: few changes over ten minutes, never a back-and-forth at the
    /// window's rate.
    #[test]
    fn a_borderline_load_never_flickers() {
        let mut g = Governor::default();
        run(&mut g, 0, WARMUP_S, BUDGET);
        let mut t = 0.0;
        let mut changes = Vec::new();
        let mut level = g.level();
        while t < 600.0 {
            // Full scale costs 22 ms a frame, anything lower fits in 15.
            let ms = if g.level() == 0 { 22.0 } else { 15.0 };
            g.frame(ms, None, None, BUDGET, 0);
            t += ms / 1000.0;
            if g.level() != level {
                level = g.level();
                changes.push((t, level));
            }
        }
        // Down once, then failed trials whose gaps grow to the hold cap.
        assert_eq!(changes[0].1, 1);
        assert!(
            changes.len() <= 16,
            "{} changes: {changes:?}",
            changes.len()
        );
        let ups: Vec<f32> = changes.iter().filter(|c| c.1 == 0).map(|c| c.0).collect();
        for pair in ups.windows(2) {
            // The failed trial's window, then the hold (at least 20 s).
            assert!(pair[1] - pair[0] >= WINDOW_S + HOLD_S - 1.0, "{ups:?}");
        }
        let gaps: Vec<f32> = ups.windows(2).map(|p| p[1] - p[0]).collect();
        assert!(
            gaps.windows(2).all(|g| g[1] >= g[0] - 1.0),
            "the waits never shrink: {gaps:?}"
        );
        assert!(gaps.last().is_some_and(|g| *g >= HOLD_MAX_S), "{gaps:?}");
    }

    /// A trial that holds clears the failures: the next failed trial waits
    /// only the first hold.
    #[test]
    fn a_held_trial_clears_the_failures() {
        let mut g = Governor::default();
        run(&mut g, 0, WARMUP_S, BUDGET);
        while g.level() == 0 {
            g.frame(40.0, None, None, BUDGET, 0);
        }
        run(&mut g, 0, UP_AFTER_S + WINDOW_S + PROBE_S + 1.0, BUDGET);
        assert_eq!((g.level(), g.failures, g.probing), (0, 0, None));
    }

    /// Another battle keeps the level learned and starts a new warm-up.
    #[test]
    fn restart_keeps_the_level_and_warms_up_again() {
        let mut g = Governor::default();
        run(&mut g, 0, WARMUP_S, BUDGET);
        run(&mut g, 0, 8.0, 40.0);
        let level = g.level();
        assert!(level >= 2);
        g.restart();
        assert!(run(&mut g, 0, WARMUP_S - 0.1, 200.0).is_empty());
        assert_eq!(g.level(), level);
    }

    /// Tiers that went away clamp the level to the scales' floor.
    #[test]
    fn tiers_that_go_away_clamp_the_level() {
        let mut g = Governor::default();
        run(&mut g, 3, WARMUP_S, BUDGET);
        run(&mut g, 3, 60.0, 40.0);
        assert_eq!(g.level(), 7);
        g.frame(BUDGET, None, None, BUDGET, 0);
        assert_eq!(g.level(), SCALES.len() - 1);
        assert_eq!(g.quality(0).scale, *SCALES.last().unwrap());
    }

    #[test]
    fn labels_name_the_scale_and_the_tier() {
        let q = Quality {
            scale: 0.85,
            tier_down: 0,
            cpu_bound: false,
        };
        assert_eq!(q.label(), "res 85%");
        let q = Quality {
            scale: 0.5,
            tier_down: 2,
            cpu_bound: true,
        };
        assert_eq!(q.label(), "res 50%, tier -2, CPU-bound");
    }

    /// A frame whose own work is over budget (the sim and the bots, not the
    /// picture) steps nothing down and says so; a GPU-bound one, whose work
    /// fits and whose time goes to the wait, steps down as before.
    #[test]
    fn cpu_bound_frames_keep_the_picture() {
        let mut g = Governor::default();
        for _ in 0..240 {
            g.frame(BUDGET, Some(5.0), Some(BUDGET - 5.0), BUDGET, 0);
        }
        for _ in 0..1500 {
            g.frame(40.0, Some(38.0), Some(2.0), BUDGET, 0);
        }
        assert_eq!(g.level(), 0);
        assert!(g.quality(0).cpu_bound);
        assert_eq!(g.quality(0).label(), "res 100%, CPU-bound");
        // Light work that waits for the image: one step as soon as the wait
        // reaches the window's p95, and the next waits its own window.
        for _ in 0..60 {
            g.frame(40.0, Some(8.0), Some(32.0), BUDGET, 0);
        }
        assert_eq!(g.level(), 1);
        assert!(!g.quality(0).cpu_bound);
    }

    /// Over budget on the CPU and still waiting for the image (a synthetic
    /// mixed case, not a measured map: the GPU is part of the limit): a step
    /// down, not "CPU-bound".
    #[test]
    fn cpu_over_with_a_gpu_wait_steps_down() {
        let mut g = Governor::default();
        for _ in 0..240 {
            g.frame(BUDGET, Some(5.0), Some(BUDGET - 5.0), BUDGET, 0);
        }
        for _ in 0..120 {
            g.frame(33.0, Some(24.0), Some(9.0), BUDGET, 0);
        }
        assert_eq!(g.level(), 1);
        assert!(!g.quality(0).cpu_bound);
    }

    /// The session log's notes: one per step, one per failed step up, and
    /// the CPU-bound summary at most once a minute.
    #[test]
    fn notes_name_each_step_and_failed_trial_once() {
        let mut g = Governor::default();
        run(&mut g, 0, WARMUP_S, BUDGET);
        assert!(g.take_notes().is_empty());
        while g.level() == 0 {
            g.frame(40.0, Some(10.0), Some(30.0), BUDGET, 0);
        }
        let notes = g.take_notes();
        assert_eq!(notes.len(), 1, "{notes:?}");
        let Note::Step {
            quality,
            steps,
            window,
            ..
        } = &notes[0]
        else {
            panic!("{notes:?}");
        };
        assert_eq!((quality.scale, *steps), (0.85, 1));
        assert_eq!(window.busy_p95, Some(10.0));
        assert_eq!(window.wait_p95, Some(30.0));
        assert_eq!(
            notes[0].line("bf2_kas2 xl", 4),
            "quality: map bf2_kas2 xl views 4 scale 0.85 tier -0 steps 1 cpu_bound false p95 40.0 busy p95 10.0 wait p95 30.0"
        );
        // Headroom: a trial up to 1.0, which fails.
        while g.level() == 1 {
            g.frame(10.0, None, None, BUDGET, 0);
        }
        assert_eq!(g.take_notes().len(), 1);
        while g.level() == 0 {
            g.frame(40.0, None, None, BUDGET, 0);
        }
        let notes = g.take_notes();
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert_eq!(
            notes[0],
            Note::FailedUp {
                quality: Quality {
                    scale: 1.0,
                    tier_down: 0,
                    cpu_bound: false,
                },
                p95: 40.0
            }
        );
        assert_eq!(
            notes[0].line("bf2_mus1", 4),
            "quality: map bf2_mus1 views 4 step up to scale 1.00 tier -0 failed, p95 40.0"
        );
        assert!(matches!(notes[1], Note::Step { steps: 1, .. }));
        // Ten minutes CPU-bound: ten summaries, nothing else.
        let mut t = 0.0;
        let mut summaries = 0;
        while t < 600.0 {
            g.frame(40.0, Some(38.0), Some(2.0), BUDGET, 0);
            t += 0.04;
            for n in g.take_notes() {
                assert!(matches!(n, Note::CpuBound { .. }), "{n:?}");
                summaries += 1;
            }
        }
        assert!((9..=10).contains(&summaries), "{summaries}");
    }
}
