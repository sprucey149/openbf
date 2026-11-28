//! The master limiter on the mixer's main track (card MIXH): a lookahead
//! peak limiter, stereo-linked, its ceiling at full scale. The PS2 has
//! none - its SPU2 clamps each core's sum (analysis/re-mix-headroom.md 1.1
//! claim 2) - and with the PS2's per-ear law the port's busiest fights
//! still clamp in up to 0.07 % of frames where the PS2's own read 0.02 %
//! (the port's battle stacks more starts beside the player). A modern
//! deviation, named in FEEL_NOTES: a mix that never reaches full scale (a
//! lone shot, any quiet stretch) passes untouched, only delayed by the
//! lookahead; a stack over full scale is turned down from `LOOKAHEAD`
//! before its peak and comes back over `RELEASE` instead of clipping.

/// The level no output sample exceeds (full scale).
pub const CEILING: f32 = 1.0;
/// How far ahead the gain sees a peak, seconds (PROVISIONAL, ours).
pub const LOOKAHEAD: f32 = 0.002;
/// The time constant of the gain's return to unity, seconds (PROVISIONAL,
/// ours).
pub const RELEASE: f32 = 0.08;

/// The limiter's state at one sample rate.
pub struct Limiter {
    /// The delayed frames and the gain each one needs to sit under the
    /// ceiling, a ring of lookahead + 1 (the frame going out is the oldest).
    frames: Vec<(f32, f32)>,
    need: Vec<f32>,
    at: usize,
    gain: f32,
    /// Per-sample release factor.
    release: f32,
}

impl Limiter {
    pub fn new(sample_rate: u32) -> Limiter {
        let sr = sample_rate.max(1) as f32;
        let n = (LOOKAHEAD * sr).round() as usize + 1;
        Limiter {
            frames: vec![(0.0, 0.0); n],
            need: vec![1.0; n],
            at: 0,
            gain: 1.0,
            release: (-1.0 / (RELEASE * sr)).exp(),
        }
    }

    /// Frames of delay (the lookahead).
    pub fn delay(&self) -> usize {
        self.frames.len() - 1
    }

    /// The gain applied to the last frame out.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// One frame in, the frame `delay()` frames older out, under the
    /// ceiling: the gain falls at once to the least any frame in the window
    /// needs (that frame included), and rises back by the release.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        // A non-finite sample (a NaN voice) is dropped from the mix rather
        // than reaching the device or the gain.
        let l = if l.is_finite() { l } else { 0.0 };
        let r = if r.is_finite() { r } else { 0.0 };
        let n = self.frames.len();
        let peak = l.abs().max(r.abs());
        self.frames[self.at] = (l, r);
        self.need[self.at] = if peak > CEILING { CEILING / peak } else { 1.0 };
        self.at = (self.at + 1) % n;
        let target = self.need.iter().copied().fold(1.0, f32::min);
        self.gain = target.min((1.0 - self.release) * target + self.release * self.gain);
        let (ol, or) = self.frames[self.at];
        (ol * self.gain, or * self.gain)
    }
}

/// The limiter as a kira effect on the main track.
#[cfg(feature = "backend")]
pub(crate) struct Effect {
    limiter: Limiter,
}

#[cfg(feature = "backend")]
pub(crate) struct Builder;

#[cfg(feature = "backend")]
impl kira::effect::EffectBuilder for Builder {
    type Handle = ();

    fn build(self) -> (Box<dyn kira::effect::Effect>, ()) {
        (
            Box::new(Effect {
                limiter: Limiter::new(48_000),
            }),
            (),
        )
    }
}

#[cfg(feature = "backend")]
impl kira::effect::Effect for Effect {
    fn init(&mut self, sample_rate: u32, _internal_buffer_size: usize) {
        self.limiter = Limiter::new(sample_rate);
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        self.limiter = Limiter::new(sample_rate);
    }

    fn process(&mut self, input: &mut [kira::Frame], _dt: f64, _info: &kira::info::Info) {
        for f in input {
            let (l, r) = self.limiter.process(f.left, f.right);
            *f = kira::Frame { left: l, right: r };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A take of `n` frames: a decaying 440 Hz burst at `peak`.
    fn take(n: usize, peak: f32) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                peak * (-t * 8.0).exp() * (t * 440.0 * std::f32::consts::TAU).sin()
            })
            .collect()
    }

    fn run(lim: &mut Limiter, l: &[f32], r: &[f32]) -> Vec<(f32, f32)> {
        let d = lim.delay();
        let mut out: Vec<(f32, f32)> = l
            .iter()
            .zip(r)
            .chain(std::iter::repeat_n((&0.0, &0.0), d))
            .map(|(a, b)| lim.process(*a, *b))
            .collect();
        out.drain(..d);
        out
    }

    /// Card MIXH: a mix that never reaches full scale comes out bit for bit
    /// (a lone shot at the loudest a take can be handed: its gain 1, both
    /// ears), only delayed; a stack over full scale never exceeds it, keeps
    /// its stereo balance and lets go within a few release times.
    #[test]
    fn a_lone_shot_passes_untouched_and_a_stack_stays_under_full_scale() {
        let mut lim = Limiter::new(48_000);
        assert_eq!(lim.delay(), 96);
        let shot = take(24_000, 0.999);
        let out = run(&mut lim, &shot, &shot);
        assert!(out.iter().zip(&shot).all(|(o, s)| o.0 == *s && o.1 == *s));
        assert_eq!(lim.gain(), 1.0);
        // Six shots stacked in one ear and three in the other.
        let l: Vec<f32> = shot.iter().map(|s| s * 6.0).collect();
        let r: Vec<f32> = shot.iter().map(|s| s * 3.0).collect();
        let out = run(&mut lim, &l, &r);
        let peak = out
            .iter()
            .map(|o| o.0.abs().max(o.1.abs()))
            .fold(0.0, f32::max);
        assert!(peak <= CEILING && peak > 0.95, "{peak}");
        // Linked: both ears get one gain, so the 2:1 stays.
        for (o, (a, b)) in out.iter().zip(l.iter().zip(&r)) {
            if a.abs() > 1e-3 {
                assert!((o.0 / a - o.1 / b).abs() < 1e-4);
            }
        }
        // Once the stack has decayed under full scale the gain comes back.
        let quiet = take(48_000, 0.3);
        run(&mut lim, &quiet, &quiet);
        assert!(lim.gain() > 0.999, "{}", lim.gain());
    }

    /// A NaN or infinite frame (a voice handed NaN) leaves as silence and
    /// leaves the gain where it was.
    #[test]
    fn a_non_finite_frame_is_dropped() {
        let mut lim = Limiter::new(48_000);
        let d = lim.delay();
        let mut out = vec![lim.process(f32::NAN, f32::INFINITY)];
        out.extend((0..d).map(|_| lim.process(0.5, -0.5)));
        assert_eq!(out[d], (0.0, 0.0));
        assert_eq!(lim.gain(), 1.0);
        assert_eq!(lim.process(0.0, 0.0), (0.5, -0.5));
    }
}
