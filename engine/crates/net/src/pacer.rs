//! Game-thread pacing: [`InputPacer`] turns rendered frames into input ticks
//! and [`Playback`] decides how many received bundles to step each frame.
//! Pure logic; the caller supplies frame times, nothing reads a clock.
//!
//! Input pacer rules:
//! - The input clock runs at 30 Hz × (1 + dilation). Each frame's pad joins
//!   the open input tick; when the clock passes a tick boundary the tick's
//!   pad is sent. A frame long enough for several ticks sends the merged pad
//!   first, then repeats of its levels with the once-only fields cleared.
//! - Frame pads inside one tick merge by [`crate::bundle::merge_pads`]: mouse
//!   deltas sum, presses stay down for the tick, latest sticks and levels win,
//!   the first pick and its fire lag win. A frame whose merge would lose a
//!   press edge or a pick opens the next tick instead, so nothing is lost.
//! - Dilation is at most ±4%, never a jump. When the host publishes the
//!   slot's slack (how early its latest input of the last half second
//!   arrived, `scheduler`), the rate follows it proportionally:
//!   `-(slack - SLACK_TARGET_MS) / SLACK_TAU_S`, so the latest input lands
//!   just before its close - with a dead band one frame wide above the
//!   target (the frame time smoothed): inputs leave only as frames end, so
//!   the slack can only be one of the frame instants before a close, and a
//!   clock that kept chasing a target between two of them would send fewer
//!   (or more) inputs than ticks close and starve the queue every few
//!   seconds. Without a slack report the depth rules below
//!   apply. The host's FIFO keeps whatever
//!   standing queue the client's clock built, so the controller aims for a
//!   queue that runs dry now and then (LAN §4.4: a starved tick repeats the
//!   last input, which is harmless; a standing queue is a tick of delay on
//!   every input):
//!   - when the host's queue depth never dropped below 1 over a whole second
//!     (a tick of slack at every close), the clock runs 2% slow for the next
//!     second;
//!   - a lone starvation is a late input that slipped a tick (the queue
//!     already holds it): nothing changes;
//!   - [`STARVED_FAST`] or more starvations within one second mean the clock
//!     itself is behind: it runs 4% fast for 0.5 s.
//!
//! Playback rules:
//! - The target buffer is [`JITTER_MARGIN`] times the measured arrival
//!   jitter in ticks (0 on a clean LAN, at most 8). It rises at once and
//!   falls by 0.5 tick per second.
//! - The buffer's level is the bundles waiting minus the time already owed
//!   toward the next step, smoothed over [`LEVEL_SECS`]; over a steady
//!   stream its mean is the ticks a bundle waits minus half a tick. Normal
//!   play steps at 30 Hz, 5% fast when the level is more than
//!   [`LEVEL_BAND`] above target − 0.5 and 5% slow when it is more than
//!   that below, so a bundle waits about the target.
//! - More than target + 6 waiting: catch up at up to 4 steps a frame on top
//!   of the frame's own whole ticks, so a slow frame (a weak PC, a stall)
//!   never falls further behind while catching up (LAN §8b.1).
//! - Nothing waiting: hold (the render blend keeps the last tick drawn).

use crate::bundle::{merge_pads, repeat_pad};
use crate::TICK_HZ;
use core::sim::PadState;
use std::collections::VecDeque;

/// The largest input-clock dilation either way.
pub const MAX_INPUT_DILATION: f64 = 0.04;
const SPEED_SECS: f64 = 0.5;
const SLOW_RATE: f64 = 0.02;
const WINDOW_SECS: f64 = 1.0;
/// Starvations within one window that speed the input clock up.
pub const STARVED_FAST: u32 = 2;
/// The slack (ms) the input clock aims for: the latest input of a window
/// arrives this long before its close.
pub const SLACK_TARGET_MS: f64 = 2.0;
/// The slack controller's time constant (s).
pub const SLACK_TAU_S: f64 = 2.0;

/// Frame pads → input ticks, with host-driven time dilation.
#[derive(Clone, Debug)]
pub struct InputPacer {
    acc: f64,
    runs: VecDeque<PadState>,
    last_sent: PadState,
    rate: f64,
    speed_left: f64,
    slow_left: f64,
    window_left: f64,
    window_min: Option<u8>,
    /// Starvations reported in the current window.
    window_starved: u32,
    /// The host's latest slack report (ms).
    slack_ms: Option<f64>,
    /// The frame time, smoothed (ms).
    frame_ms: Option<f64>,
    last_starved: Option<u32>,
}

impl Default for InputPacer {
    fn default() -> Self {
        Self::new()
    }
}

impl InputPacer {
    pub fn new() -> Self {
        Self {
            acc: 0.0,
            runs: VecDeque::new(),
            last_sent: PadState::default(),
            rate: 0.0,
            speed_left: 0.0,
            slow_left: 0.0,
            window_left: WINDOW_SECS,
            window_min: None,
            window_starved: 0,
            slack_ms: None,
            frame_ms: None,
            last_starved: None,
        }
    }

    /// The current input clock dilation (−0.04..=0.04).
    pub fn dilation(&self) -> f64 {
        self.rate
    }

    /// Ticks of input clock not yet sent (0..1).
    pub fn phase(&self) -> f64 {
        self.acc
    }

    /// Pads queued but not yet sent (the open tick included).
    pub fn pending(&self) -> &VecDeque<PadState> {
        &self.runs
    }

    /// The open input tick's pad so far: the newest queued, or, when the
    /// last frame's pad went out with the tick it closed and no frame has
    /// joined the next one yet, the levels it held (the sent pad repeated,
    /// once-only fields cleared) - what the open tick will carry if nothing
    /// changes.
    pub fn open(&self) -> PadState {
        self.runs
            .back()
            .copied()
            .unwrap_or_else(|| repeat_pad(&self.last_sent))
    }

    /// The host's latest report for this slot: queue depth and starvation
    /// count. Calling it every frame with unchanged values is fine.
    pub fn feedback(&mut self, depth: u8, starved: u32) {
        match self.last_starved {
            None => self.last_starved = Some(starved),
            Some(prev) if starved != prev => {
                self.last_starved = Some(starved);
                self.window_starved += starved.wrapping_sub(prev).min(WINDOW_SECS as u32 * 30);
                self.slow_left = 0.0;
                if self.window_starved >= STARVED_FAST {
                    self.speed_left = SPEED_SECS;
                    self.window_left = WINDOW_SECS;
                    self.window_starved = 0;
                }
                self.window_min = Some(0);
                return;
            }
            _ => {}
        }
        self.window_min = Some(self.window_min.map_or(depth, |m| m.min(depth)));
    }

    /// The host's slack report for this slot (µs), when it sends one.
    pub fn slack(&mut self, slack_us: Option<i64>) {
        if let Some(us) = slack_us {
            self.slack_ms = Some(us as f64 / 1e3);
        }
    }

    fn advance_controller(&mut self, dt: f64) {
        let frame = dt * 1e3;
        let frame_ms = self.frame_ms.map_or(frame, |f| f + (frame - f) * 0.1);
        self.frame_ms = Some(frame_ms);
        if let Some(ms) = self.slack_ms {
            // Early by up to a frame past the target is on time.
            let off = ms - SLACK_TARGET_MS;
            let off = if off > 0.0 {
                (off - frame_ms).max(0.0)
            } else {
                off
            };
            self.rate = (-off / (1e3 * SLACK_TAU_S)).clamp(-MAX_INPUT_DILATION, MAX_INPUT_DILATION);
            return;
        }
        self.speed_left = (self.speed_left - dt).max(0.0);
        self.slow_left = (self.slow_left - dt).max(0.0);
        self.window_left -= dt;
        if self.window_left <= 0.0 {
            if self.speed_left == 0.0 && self.window_min.is_some_and(|m| m >= 1) {
                self.slow_left = WINDOW_SECS;
            }
            self.window_left = WINDOW_SECS;
            self.window_min = None;
            self.window_starved = 0;
        }
        self.rate = if self.speed_left > 0.0 {
            MAX_INPUT_DILATION
        } else if self.slow_left > 0.0 {
            -SLOW_RATE
        } else {
            0.0
        };
    }

    /// One rendered frame of `dt` seconds sampled `pad`. Returns the input
    /// ticks to send now, oldest first.
    pub fn frame(&mut self, dt: f64, pad: PadState) -> Vec<PadState> {
        let dt = dt.clamp(0.0, 1.0);
        self.advance_controller(dt);
        self.add(pad);
        self.acc += dt * TICK_HZ * (1.0 + self.rate);
        let mut out = Vec::new();
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let p = self
                .runs
                .pop_front()
                .unwrap_or_else(|| repeat_pad(&self.last_sent));
            self.last_sent = p;
            out.push(p);
        }
        out
    }

    fn add(&mut self, pad: PadState) {
        let n = self.runs.len();
        if n > 0 {
            let prior = if n >= 2 {
                self.runs[n - 2]
            } else {
                self.last_sent
            };
            if let Some(m) = merge_pads(&prior, &self.runs[n - 1], &pad, None) {
                self.runs[n - 1] = m;
                return;
            }
        }
        self.runs.push_back(pad);
    }
}

/// Jitter-buffer controller for received bundles.
#[derive(Clone, Debug, Default)]
pub struct Playback {
    acc: f64,
    target: f64,
    held: bool,
    /// The smoothed buffer level (module rules), once a frame has run.
    level: Option<f64>,
}

/// The jitter buffer's target per tick of measured arrival jitter.
pub const JITTER_MARGIN: f64 = 1.0;
/// The buffer level's smoothing time.
pub const LEVEL_SECS: f64 = 0.5;
/// How far (ticks) the smoothed level may stray from its aim before the
/// playback clock dilates.
pub const LEVEL_BAND: f64 = 0.25;

/// The playback dilation either way.
pub const PLAYBACK_DILATION: f64 = 0.05;
/// More than this many ticks past the target triggers catch-up.
pub const CATCH_UP_BEHIND: f64 = 6.0;
/// The most steps one frame takes while catching up, beyond the frame's
/// own whole ticks.
pub const CATCH_UP_STEPS: usize = 4;

impl Playback {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current buffer target in ticks.
    pub fn target(&self) -> f64 {
        self.target
    }

    /// The smoothed buffer level in ticks (module rules; 0 before a frame).
    pub fn level(&self) -> f64 {
        self.level.unwrap_or(0.0)
    }

    /// How far between the last stepped tick and the next one the frame
    /// should draw (0..1).
    pub fn blend(&self) -> f64 {
        if self.held {
            1.0
        } else {
            self.acc.clamp(0.0, 1.0)
        }
    }

    /// One frame of `dt` seconds with `buffered` bundles waiting and arrival
    /// jitter `jitter_ticks`. Returns how many bundles to step now.
    pub fn frame(&mut self, dt: f64, buffered: usize, jitter_ticks: f64) -> usize {
        let dt = dt.clamp(0.0, 1.0);
        let raw = (jitter_ticks * JITTER_MARGIN).clamp(0.0, 8.0);
        if raw > self.target {
            self.target = raw;
        } else {
            self.target = (self.target - 0.5 * dt).max(raw);
        }
        let b = buffered as f64;
        if b > self.target + CATCH_UP_BEHIND {
            self.acc = 0.0;
            self.held = false;
            self.level = None;
            let due = (dt * TICK_HZ).floor() as usize;
            return buffered.min(CATCH_UP_STEPS + due);
        }
        let aim = self.target - 0.5;
        let rate = match self.level {
            Some(l) if l > aim + LEVEL_BAND => PLAYBACK_DILATION,
            Some(l) if l < aim - LEVEL_BAND => -PLAYBACK_DILATION,
            _ => 0.0,
        };
        self.acc += dt * TICK_HZ * (1.0 + rate);
        let want = self.acc.floor() as usize;
        let steps = want.min(buffered);
        self.acc -= steps as f64;
        self.held = steps < want;
        if self.held {
            // Starved: hold at the tick boundary, do not bank time.
            self.acc = self.acc.min(1.0);
        }
        let level = (buffered - steps) as f64 - self.acc;
        let k = (dt / LEVEL_SECS).min(1.0);
        self.level = Some(self.level.map_or(level, |l| l + (level - l) * k));
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sim::buttons::FIRE;

    #[test]
    fn pacer_merges_frames_within_a_tick() {
        let mut p = InputPacer::new();
        let dt = 1.0 / 120.0;
        let mut sent = Vec::new();
        let mut sum = 0;
        for i in 0..1200 {
            let pad = PadState {
                mx: i % 7 - 3,
                buttons: if (i / 3) % 2 == 0 { FIRE } else { 0 },
                ..PadState::default()
            };
            sum += pad.mx;
            sent.extend(p.frame(dt, pad));
        }
        sent.extend(p.pending().iter().copied());
        assert!((295..=450).contains(&sent.len()), "{}", sent.len());
        assert_eq!(sent.iter().map(|p| p.mx).sum::<i32>(), sum);
        let mut prev = 0;
        let edges = sent
            .iter()
            .filter(|p| {
                let e = p.buttons & FIRE != 0 && prev & FIRE == 0;
                prev = p.buttons;
                e
            })
            .count();
        assert_eq!(edges, 200);
    }

    #[test]
    fn pacer_low_fps_repeats_without_once_fields() {
        let mut p = InputPacer::new();
        let out = p.frame(
            0.1,
            PadState {
                mx: 9,
                lx: 5,
                ..PadState::default()
            },
        );
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].mx, 9);
        assert_eq!((out[1].mx, out[1].lx), (0, 5));
    }

    #[test]
    fn pacer_dilates_within_bounds() {
        let mut p = InputPacer::new();
        p.feedback(1, 0);
        // A lone starvation is a slipped input: no change.
        p.feedback(0, 1);
        p.frame(0.01, PadState::default());
        assert_eq!(p.dilation(), 0.0);
        // A second one within the window: the clock is behind.
        p.feedback(0, 2);
        p.frame(0.01, PadState::default());
        assert_eq!(p.dilation(), MAX_INPUT_DILATION);
        // A tick of slack at every close for a whole second: slow down.
        for _ in 0..300 {
            p.feedback(1, 2);
            p.frame(0.01, PadState::default());
        }
        assert!(p.dilation() < 0.0 && p.dilation() >= -MAX_INPUT_DILATION);
    }

    /// With slack reports the rate follows them: early inputs (more than a
    /// frame past the target) slow the clock, late ones speed it, within
    /// the bound.
    #[test]
    fn pacer_follows_the_hosts_slack() {
        let mut p = InputPacer::new();
        p.slack(Some(52_000));
        p.frame(0.01, PadState::default());
        assert!((p.dilation() + 0.02).abs() < 1e-9, "{}", p.dilation());
        // Early by less than a frame past the target: on time.
        p.slack(Some(11_000));
        p.frame(0.01, PadState::default());
        assert_eq!(p.dilation(), 0.0);
        p.slack(Some(-500_000));
        p.frame(0.01, PadState::default());
        assert_eq!(p.dilation(), MAX_INPUT_DILATION);
        p.slack(None);
        p.slack(Some(2_000));
        p.frame(0.01, PadState::default());
        assert_eq!(p.dilation(), 0.0);
    }

    /// The jitter buffer steps a bundle about when it arrives on a clean
    /// link: bundles arriving every tick wait well under a tick on average.
    #[test]
    fn playback_on_a_clean_link_keeps_the_wait_short() {
        let mut pb = Playback::new();
        let dt = 1.0 / 144.0;
        let (mut arrived, mut stepped, mut buffered) = (0u64, 0u64, 0usize);
        let mut waits = 0.0;
        let mut t = 0.0;
        let mut arrivals = std::collections::VecDeque::new();
        for _ in 0..(144 * 20) {
            t += dt;
            while (arrived as f64 + 1.0) / TICK_HZ <= t {
                arrived += 1;
                buffered += 1;
                arrivals.push_back(t);
            }
            let n = pb.frame(dt, buffered, 0.0);
            buffered -= n;
            for _ in 0..n {
                waits += t - arrivals.pop_front().unwrap();
                stepped += 1;
            }
        }
        let mean_ms = waits / stepped as f64 * 1e3;
        assert!(stepped + 2 >= arrived, "{stepped} of {arrived}");
        assert!(mean_ms < 20.0, "mean wait {mean_ms:.1} ms");
    }

    #[test]
    fn playback_holds_steps_and_catches_up() {
        let mut pb = Playback::new();
        assert_eq!(pb.frame(0.1, 0, 0.0), 0);
        assert_eq!(pb.blend(), 1.0);
        let mut total = 0;
        for _ in 0..60 {
            total += pb.frame(1.0 / 60.0, 5, 0.0);
        }
        assert!((30..=34).contains(&total), "{total}");
        assert_eq!(pb.frame(1.0 / 60.0, 20, 0.0), CATCH_UP_STEPS);
        pb.frame(0.0, 0, 3.0);
        assert_eq!(pb.target(), 3.0 * JITTER_MARGIN);
    }

    /// A slow PC catching up (5 frames a second, 40 bundles behind) steps
    /// its frame's own 6 ticks plus the catch-up's 4 each frame, so it
    /// gains on the stream instead of falling further behind.
    #[test]
    fn slow_frames_still_gain_while_catching_up() {
        let mut pb = Playback::new();
        let mut behind = 40usize;
        let mut frames = 0;
        while behind as f64 > CATCH_UP_BEHIND {
            // Six new bundles arrive during each 200 ms frame.
            behind += 6;
            let steps = pb.frame(0.2, behind, 0.0);
            assert_eq!(steps, 10.min(behind));
            behind -= steps;
            frames += 1;
            assert!(frames < 20, "never caught up");
        }
        assert_eq!(pb.frame(1.0, 100, 0.0), CATCH_UP_STEPS + 30);
    }
}
