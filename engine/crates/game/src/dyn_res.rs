//! The Low Power Resolution setting's controller: how much of the window the
//! world draws at, from how long frames have been taking. Off by default; the
//! renderer draws the HUD at the window's size either way (`render::world_scale`).
//!
//! A slow second steps the world down. Spare time cannot be read off a frame
//! under vsync, which rounds every frame up to the display's period, so stepping
//! up is a probe: after a while at budget it tries one step and, if that goes
//! over budget soon after, steps back and waits, twice as long each time in a
//! row. The steps and times are this game's.
//!
//! No part of this reaches the sim: it reads the wall clock and moves only the
//! renderer's target size.

/// The fractions of the window a battle can draw at, largest first. PROVISIONAL,
/// measure: where a handheld's panel starts to look soft, and whether its GPU
/// needs lower. 0.7 takes 39% off the GPU's frame on this PC's Intel GPU at 1080p
/// (BF2 Hoth, 8.87 -> 5.44 ms).
pub const STEPS: [f32; 4] = [1.0, 0.9, 0.8, 0.7];

/// The longest frame the setting aims for: 60 a second on a faster display.
const FASTEST_BUDGET_MS: f32 = 1000.0 / 60.0;
/// A frame longer than the display's period by this share is over budget.
const OVER: f32 = 1.05;
/// The smoothing of the frame time.
const EWMA: f32 = 0.06;
/// Seconds over budget before a step down.
const DOWN_AFTER_S: f32 = 1.0;
/// Seconds at budget before a step up is tried.
const UP_AFTER_S: f32 = 3.0;
/// A step up that holds this long has held.
const PROBE_S: f32 = 4.0;
/// Seconds after any change before the frame times count again.
const SETTLE_S: f32 = 1.5;
/// Seconds at a battle's start before anything counts: its first frames load.
const WARMUP_S: f32 = 4.0;
/// How long a failed step up waits before the next, doubled per failure in a
/// row, up to the limit.
const HOLD_S: f32 = 10.0;
const HOLD_MAX_S: f32 = 60.0;

/// PROVISIONAL, measure: the settle, probe and hold times on a handheld, where a
/// battery can move the GPU's clock; these are this PC's, and a Vita's before it.
#[derive(Debug, Clone, PartialEq)]
pub struct DynRes {
    step: usize,
    ewma_ms: f32,
    over_s: f32,
    under_s: f32,
    wait_s: f32,
    /// Seconds since a step up, while it is on trial.
    probing_s: Option<f32>,
    hold_s: f32,
    failures: u32,
}

/// What a frame is held to on a display refreshing `millihertz` times per
/// thousand seconds: 60 a second, or the display's own period where that is
/// slower. A 40 Hz or 50 Hz panel can never show a frame in 16.7 ms, and holding
/// it to that would leave the world at the lowest step for good. An unknown rate
/// is taken as fast enough.
pub fn display_budget_ms(millihertz: Option<u32>) -> f32 {
    millihertz
        .filter(|&mhz| mhz > 0)
        .map_or(FASTEST_BUDGET_MS, |mhz| {
            FASTEST_BUDGET_MS.max(1.0e6 / mhz as f32)
        })
}

impl Default for DynRes {
    fn default() -> Self {
        Self {
            step: 0,
            ewma_ms: 0.0,
            over_s: 0.0,
            under_s: 0.0,
            wait_s: WARMUP_S,
            probing_s: None,
            hold_s: 0.0,
            failures: 0,
        }
    }
}

impl DynRes {
    /// The fraction of the window to draw the next frame at, after a frame of
    /// `dt_ms` on a display showing a frame every `budget_ms`.
    pub fn frame(&mut self, dt_ms: f32, budget_ms: f32) -> f32 {
        let dt = dt_ms / 1000.0;
        self.ewma_ms = if self.ewma_ms == 0.0 {
            dt_ms
        } else {
            self.ewma_ms + (dt_ms - self.ewma_ms) * EWMA
        };
        self.hold_s = (self.hold_s - dt).max(0.0);
        if let Some(p) = &mut self.probing_s {
            *p += dt;
        }
        if self.wait_s > 0.0 {
            self.wait_s -= dt;
            return self.scale();
        }
        if self.probing_s.is_some_and(|p| p > PROBE_S) {
            self.probing_s = None;
            self.failures = 0;
        }
        let over = self.ewma_ms > budget_ms * OVER;
        if over {
            self.over_s += dt;
            self.under_s = 0.0;
        } else {
            self.under_s += dt;
            self.over_s = 0.0;
        }
        if self.over_s >= DOWN_AFTER_S && self.step + 1 < STEPS.len() {
            self.step += 1;
            if self.probing_s.take().is_some() {
                self.failures += 1;
                let hold = (1..self.failures).fold(HOLD_S, |h, _| (h * 2.0).min(HOLD_MAX_S));
                self.hold_s = hold;
            }
            self.changed();
        } else if self.under_s >= UP_AFTER_S && self.step > 0 && self.hold_s == 0.0 {
            self.step -= 1;
            self.probing_s = Some(0.0);
            self.changed();
        }
        self.scale()
    }

    fn changed(&mut self) {
        self.over_s = 0.0;
        self.under_s = 0.0;
        self.wait_s = SETTLE_S;
    }

    pub fn scale(&self) -> f32 {
        STEPS[self.step]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGET: f32 = 16.67;

    /// `seconds` of frames of `ms`; returns the last scale.
    fn run(c: &mut DynRes, seconds: f32, ms: f32) -> f32 {
        let mut t = 0.0;
        let mut scale = c.scale();
        while t < seconds {
            scale = c.frame(ms, BUDGET);
            t += ms / 1000.0;
        }
        scale
    }

    /// A slower panel is held to its own period, and a battle at that rate
    /// stays at full size; a faster one is still held to 60 a second.
    #[test]
    fn the_budget_is_the_slower_of_sixty_a_second_and_the_display() {
        assert_eq!(display_budget_ms(None), FASTEST_BUDGET_MS);
        assert_eq!(display_budget_ms(Some(0)), FASTEST_BUDGET_MS);
        assert_eq!(display_budget_ms(Some(144_000)), FASTEST_BUDGET_MS);
        assert_eq!(display_budget_ms(Some(40_000)), 25.0);
        let budget = display_budget_ms(Some(40_000));
        let mut c = DynRes::default();
        let mut scale = c.scale();
        for _ in 0..2400 {
            scale = c.frame(25.0, budget);
        }
        assert_eq!(scale, 1.0, "a minute at the panel's own rate");
        let mut held_to_sixty = DynRes::default();
        for _ in 0..2400 {
            scale = held_to_sixty.frame(25.0, FASTEST_BUDGET_MS);
        }
        assert_eq!(scale, *STEPS.last().unwrap(), "the trap this avoids");
    }

    #[test]
    fn nothing_counts_while_a_battle_loads() {
        let mut c = DynRes::default();
        assert_eq!(run(&mut c, WARMUP_S - 0.5, 200.0), 1.0);
    }

    #[test]
    fn a_slow_stretch_steps_down_to_the_floor_and_no_further() {
        let mut c = DynRes::default();
        run(&mut c, WARMUP_S, 16.67);
        assert_eq!(
            run(&mut c, 2.0, 33.3),
            STEPS[1],
            "one step for one slow second"
        );
        assert_eq!(run(&mut c, 60.0, 33.3), *STEPS.last().unwrap());
    }

    #[test]
    fn a_frame_at_budget_never_steps_down() {
        let mut c = DynRes::default();
        assert_eq!(run(&mut c, 60.0, BUDGET), 1.0);
    }

    #[test]
    fn a_light_stretch_probes_up_and_a_held_probe_clears_the_failures() {
        let mut c = DynRes::default();
        run(&mut c, WARMUP_S, BUDGET);
        run(&mut c, 2.0, 33.3);
        assert_eq!(c.scale(), STEPS[1]);
        assert_eq!(run(&mut c, 30.0, BUDGET), 1.0, "up once the time is spare");
        assert_eq!(c.probing_s, None, "the probe held");
    }

    #[test]
    fn a_failed_probe_steps_back_and_waits_longer_each_time() {
        let mut c = DynRes::default();
        run(&mut c, WARMUP_S, BUDGET);
        run(&mut c, 2.0, 33.3);
        // Light again until it tries the step up.
        while c.scale() != STEPS[0] {
            c.frame(BUDGET, BUDGET);
        }
        // That step is too much: back down, with a hold.
        while c.scale() == STEPS[0] {
            c.frame(33.3, BUDGET);
        }
        assert_eq!((c.failures, c.hold_s > HOLD_S * 0.9), (1, true));
        // A light frame again does not try before the hold runs out.
        assert_eq!(run(&mut c, HOLD_S - 3.0, BUDGET), STEPS[1]);
        // The hold over, it tries again; failing doubles the hold.
        while c.scale() != STEPS[0] {
            c.frame(BUDGET, BUDGET);
        }
        while c.scale() == STEPS[0] {
            c.frame(33.3, BUDGET);
        }
        assert_eq!(c.failures, 2);
        assert!(c.hold_s > HOLD_S * 1.5, "{}", c.hold_s);
    }
}
