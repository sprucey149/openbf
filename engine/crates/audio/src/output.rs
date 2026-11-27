//! The mixer's output: a kira backend that keeps the sound going for the
//! whole session (OURS - the originals write a console's fixed SPU2, which
//! never goes away).
//!
//! kira's own cpal backend restarts its stream only on a
//! `DeviceNotAvailable` error, and only if the default device can be
//! opened at that instant; any other stream error (ALSA's POLLERR loop when
//! the sound server drops the node) is ignored, and a failed restart
//! panics its thread with the renderer inside it. Each of those leaves the
//! game silent until it is restarted. Here the renderer (every track,
//! sound, music voice and handle the game holds) lives outside the stream:
//! a stream is only a pump, and a watchdog replaces the pump whenever it
//! stops pulling frames, reports its device gone, or the default device
//! changes - retrying until one opens, and mixing into nowhere meanwhile so
//! sounds keep time. A panic inside the renderer is caught in the callback
//! (cpal's worker has no catch of its own and would die with it): that
//! buffer is silence and the next one renders again. The game never sees
//! any of it.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};

use kira::backend::{Backend, Renderer};

/// How often the watchdog looks at the stream: OURS, kira's own interval.
/// measure: reopen latency after a sound-server restart on the couch box.
pub const CHECK_INTERVAL: Duration = Duration::from_millis(500);
/// OURS: a stream that has pulled no frame for this long is replaced. An
/// output callback runs every few ms; a second without one is not a
/// scheduling hiccup. measure: longest callback gap of a healthy session
/// (suspend excluded) before lowering.
pub const STALL_AFTER: Duration = Duration::from_millis(1500);
/// OURS: the wait between attempts to open an output while none opens,
/// and the first wait after a stream that opened but never pulled.
/// measure: how long a sound-server restart takes to offer a device.
pub const RETRY_EVERY: Duration = Duration::from_secs(1);
/// OURS: the longest wait between streams that open but never pull (it
/// doubles from [`RETRY_EVERY`]), and how often such a run, or caught
/// renderer panics, are logged as a count. measure: none needed - a retry
/// and log rate, not a mix value.
pub const QUIET_FOR: Duration = Duration::from_secs(30);
/// Stream errors logged per stream; the rest are counted.
const ERRORS_LOGGED: u64 = 4;
/// OURS: the most mixed into nowhere per watchdog look while no stream is
/// open, so waking from suspend does not mix hours at once.
const UNHEARD_MAX: Duration = Duration::from_secs(1);

/// What the stream callback pulls frames through: (buffer, channels, the
/// stream's sample rate).
pub type Render = Box<dyn FnMut(&mut [f32], u16, u32) + Send + 'static>;

/// Where a stream reports its errors (the cpal error callback's thread).
#[derive(Default)]
pub struct Errors {
    log: Mutex<Vec<String>>,
    count: AtomicU64,
    gone: AtomicBool,
}

impl Errors {
    /// Note an error; `gone` when it says the device went away.
    pub fn report(&self, message: String, gone: bool) {
        let n = self.count.fetch_add(1, Ordering::AcqRel);
        if gone {
            self.gone.store(true, Ordering::Release);
        }
        if n < ERRORS_LOGGED {
            if let Ok(mut log) = self.log.lock() {
                log.push(message);
            }
        }
    }
}

/// A source of output streams: the sound card through cpal, or a test's.
pub trait Output: Send + 'static {
    /// The device a new stream would open now: (name, sample rate).
    fn probe(&mut self) -> Result<(String, u32), String>;
    /// Open and start a stream that pulls frames through `render`,
    /// reporting its errors to `errors`. Dropping the returned value stops
    /// it. Returns it with the device's name and sample rate.
    fn open(
        &mut self,
        render: Render,
        errors: Arc<Errors>,
    ) -> Result<(Box<dyn std::any::Any>, String, u32), String>;
    /// Whether a changed default device should move the stream (true for
    /// the system default; a test's output keeps its stream).
    fn follows_default(&self) -> bool {
        true
    }
}

/// What the watchdog shares with the stream callbacks and the game.
pub struct Shared {
    /// The renderer and the sample rate it is set to.
    renderer: Mutex<Option<(Renderer, u32)>>,
    /// Frames pulled since start, by any stream.
    frames: AtomicU64,
    /// The stream allowed to pull: a replaced stream that wakes up again
    /// writes silence instead of racing the new one.
    epoch: AtomicU64,
    /// Streams opened after the first.
    reopens: AtomicU64,
    /// Buffers whose render panicked (written as silence).
    panics: AtomicU64,
    stop: AtomicBool,
    /// A test's way to make the next render panic.
    #[cfg(test)]
    panic_next: AtomicBool,
}

impl Shared {
    fn new() -> Shared {
        Shared {
            renderer: Mutex::new(None),
            frames: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            reopens: AtomicU64::new(0),
            panics: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            #[cfg(test)]
            panic_next: AtomicBool::new(false),
        }
    }

    /// Frames the output has pulled.
    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Acquire)
    }

    /// Times the output was reopened.
    pub fn reopens(&self) -> u64 {
        self.reopens.load(Ordering::Acquire)
    }

    /// Buffers written as silence because the renderer panicked.
    pub fn panics(&self) -> u64 {
        self.panics.load(Ordering::Acquire)
    }

    fn render(&self, epoch: u64, out: &mut [f32], channels: u16, rate: u32) {
        let channels = channels.max(1);
        if self.epoch.load(Ordering::Acquire) != epoch {
            out.fill(0.0);
            return;
        }
        // Only the live stream and the watchdog (while no stream is open)
        // lock it, so this never waits. Renders catch their panics, so it
        // is not poisoned by one; if it ever is, it is used as it is.
        let mut guard = match self.renderer.try_lock() {
            Ok(g) => g,
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
            Err(TryLockError::WouldBlock) => {
                out.fill(0.0);
                return;
            }
        };
        self.mix(guard.as_mut(), out, channels, rate);
        drop(guard);
        self.frames
            .fetch_add((out.len() / channels as usize) as u64, Ordering::AcqRel);
    }

    /// Mix one buffer at `rate` (the renderer follows a new rate before the
    /// buffer, so a stream at another rate never plays at the wrong pitch).
    /// A panic is caught here: the buffer is silence.
    fn mix(&self, r: Option<&mut (Renderer, u32)>, out: &mut [f32], channels: u16, rate: u32) {
        let Some((r, at)) = r else {
            out.fill(0.0);
            return;
        };
        let done = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            if self.panic_next.swap(false, Ordering::AcqRel) {
                panic!("test: the renderer panics once");
            }
            if *at != rate {
                r.on_change_sample_rate(rate);
                *at = rate;
            }
            r.on_start_processing();
            r.process(out, channels);
        }));
        if done.is_err() {
            out.fill(0.0);
            self.panics.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Mix `frames` frames into nowhere while no stream is open, so sounds
    /// end on time and the buses drain instead of everything queued in the
    /// outage starting at once when an output opens.
    fn mix_unheard(&self, scratch: &mut [f32], gap: Duration) {
        let mut guard = self.renderer.lock().unwrap_or_else(|p| p.into_inner());
        let Some(rate) = guard.as_ref().map(|r| r.1) else {
            return;
        };
        let mut frames = (gap.as_secs_f64() * f64::from(rate)) as usize;
        while frames > 0 {
            let n = frames.min(scratch.len() / 2);
            self.mix(guard.as_mut(), &mut scratch[..n * 2], 2, rate);
            frames -= n;
        }
    }
}

/// The watchdog's timings.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub check: Duration,
    pub stall: Duration,
    pub retry: Duration,
    pub quiet: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            check: CHECK_INTERVAL,
            stall: STALL_AFTER,
            retry: RETRY_EVERY,
            quiet: QUIET_FOR,
        }
    }
}

#[derive(Default)]
pub struct Settings<O> {
    pub output: O,
    pub timing: Timing,
}

/// The kira backend ([`Output`] behind a watchdog).
pub struct Watched<O: Output> {
    output: Option<O>,
    timing: Timing,
    /// The rate the renderer was made at.
    sample_rate: u32,
    shared: Arc<Shared>,
}

impl<O: Output> Watched<O> {
    pub fn shared(&self) -> Arc<Shared> {
        self.shared.clone()
    }
}

impl<O: Output> Drop for Watched<O> {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
    }
}

impl<O: Output> Backend for Watched<O> {
    type Settings = Settings<O>;
    type Error = String;

    fn setup(settings: Settings<O>, _internal_buffer_size: usize) -> Result<(Self, u32), String> {
        let mut output = settings.output;
        let (_, sample_rate) = output.probe()?;
        Ok((
            Watched {
                output: Some(output),
                timing: settings.timing,
                sample_rate,
                shared: Arc::new(Shared::new()),
            },
            sample_rate,
        ))
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), String> {
        let mut output = self.output.take().ok_or("output already started")?;
        *self
            .shared
            .renderer
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some((renderer, self.sample_rate));
        let (shared, timing) = (self.shared.clone(), self.timing);
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || {
                let mut pump = Pump::new(shared, timing);
                match pump.open(&mut output) {
                    Ok(()) => {
                        let _ = tx.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
                pump.watch(&mut output);
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
}

/// The watchdog thread's state: the live stream and how it is doing.
struct Pump {
    shared: Arc<Shared>,
    timing: Timing,
    stream: Option<(Box<dyn std::any::Any>, String, u32)>,
    errors: Arc<Errors>,
    /// The frame count when the live stream opened.
    opened_at: u64,
    last_frames: u64,
    last_progress: Instant,
    /// Why the stream was dropped, until one opens again.
    lost: Option<String>,
    next_try: Instant,
    failures: u32,
    /// Streams in a row replaced without pulling a frame.
    dead: u32,
    /// Reopens not logged one by one (a run of dead streams), and when
    /// that count was last logged.
    unlogged: u64,
    quiet_since: Instant,
    /// Renderer panics already logged.
    panics_logged: u64,
    panics_at: Option<Instant>,
    /// Up to when the renderer was mixed into nowhere (no stream open).
    unheard_to: Instant,
    scratch: Vec<f32>,
}

impl Pump {
    fn new(shared: Arc<Shared>, timing: Timing) -> Pump {
        let now = Instant::now();
        Pump {
            shared,
            timing,
            stream: None,
            errors: Arc::new(Errors::default()),
            opened_at: 0,
            last_frames: 0,
            last_progress: now,
            lost: None,
            next_try: now,
            failures: 0,
            dead: 0,
            unlogged: 0,
            quiet_since: now,
            panics_logged: 0,
            panics_at: None,
            unheard_to: now,
            scratch: vec![0.0; 1024 * 2],
        }
    }

    fn open(&mut self, output: &mut impl Output) -> Result<(), String> {
        let epoch = self.shared.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let errors = Arc::new(Errors::default());
        let shared = self.shared.clone();
        let render: Render =
            Box::new(move |out, channels, rate| shared.render(epoch, out, channels, rate));
        self.stream = Some(output.open(render, errors.clone())?);
        self.errors = errors;
        self.opened_at = self.shared.frames();
        self.last_frames = self.opened_at;
        self.last_progress = Instant::now();
        Ok(())
    }

    fn drop_stream(&mut self, why: String) {
        let pulled = self.shared.frames() != self.opened_at;
        // Stop the old stream pulling before it is torn down.
        self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        let old = self.stream.take();
        // cpal joins its worker on drop and unwraps the join.
        if catch_unwind(AssertUnwindSafe(|| drop(old))).is_err() {
            eprintln!("audio: the old output stream panicked while closing");
        }
        self.lost = Some(why);
        let now = Instant::now();
        self.unheard_to = now;
        self.failures = 0;
        // A stream that opened but never pulled: wait longer each time
        // (a sound server that accepts streams it never runs).
        self.dead = if pulled { 0 } else { self.dead + 1 };
        self.next_try = match self.dead {
            0 => now,
            d => now + (self.timing.retry * (1u32 << (d - 1).min(16))).min(self.timing.quiet),
        };
    }

    fn drain_errors(&mut self) {
        let logged: Vec<String> = self
            .errors
            .log
            .lock()
            .map(|mut l| std::mem::take(&mut *l))
            .unwrap_or_default();
        for e in logged {
            eprintln!("audio: output error: {e}");
        }
    }

    /// Log caught renderer panics: the first at once, then a count at most
    /// every [`Timing::quiet`].
    fn log_panics(&mut self) {
        let n = self.shared.panics();
        if n == self.panics_logged {
            return;
        }
        let first = self.panics_logged == 0;
        if first
            || self
                .panics_at
                .is_none_or(|t| t.elapsed() >= self.timing.quiet)
        {
            eprintln!(
                "audio: the mixer panicked in {} buffer(s){}; they were silence and it renders on",
                n - self.panics_logged,
                if first { "" } else { " since the last count" }
            );
            self.panics_logged = n;
            self.panics_at = Some(Instant::now());
        }
    }

    fn watch(&mut self, output: &mut impl Output) {
        while !self.shared.stop.load(Ordering::Acquire) {
            std::thread::sleep(self.timing.check);
            if self.shared.stop.load(Ordering::Acquire) {
                break;
            }
            self.step(output);
        }
    }

    /// One look at the output.
    fn step(&mut self, output: &mut impl Output) {
        self.drain_errors();
        self.log_panics();
        if let Some((_, name, rate)) = &self.stream {
            let frames = self.shared.frames();
            if frames != self.last_frames {
                self.last_frames = frames;
                self.last_progress = Instant::now();
                if self.dead > 0 {
                    eprintln!(
                        "audio: output '{name}' pulls again after {} stream(s) that never did",
                        self.dead
                    );
                    (self.dead, self.unlogged) = (0, 0);
                }
            }
            let idle = self.last_progress.elapsed();
            let why = if self.errors.gone.load(Ordering::Acquire) {
                Some(format!("device '{name}' went away"))
            } else if idle >= self.timing.stall {
                let n = self.errors.count.load(Ordering::Acquire);
                Some(format!(
                    "'{name}' stopped pulling frames for {:.1} s ({n} stream errors)",
                    idle.as_secs_f32()
                ))
            } else if output.follows_default() {
                match output.probe() {
                    Ok((now, r)) if now != *name || r != *rate => Some(format!(
                        "the default output moved from '{name}' ({rate} Hz) to '{now}' ({r} Hz)"
                    )),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(why) = why {
                self.drop_stream(why);
            }
        }
        if self.stream.is_none() {
            self.mix_unheard();
        }
        if self.stream.is_none() && Instant::now() >= self.next_try {
            match self.open(output) {
                Ok(()) => self.opened(),
                Err(e) => {
                    if self.failures == 0 && self.dead <= 1 {
                        eprintln!(
                            "audio: output lost ({}); no output opens yet ({e}), retrying every {:.1} s",
                            self.lost.as_deref().unwrap_or("unknown"),
                            self.timing.retry.as_secs_f32()
                        );
                    }
                    self.failures += 1;
                    self.next_try = Instant::now() + self.timing.retry;
                }
            }
        }
    }

    /// Mix the time since the last look into nowhere (no stream is open).
    fn mix_unheard(&mut self) {
        let now = Instant::now();
        let gap = now.duration_since(self.unheard_to).min(UNHEARD_MAX);
        self.unheard_to = now;
        self.shared.mix_unheard(&mut self.scratch, gap);
    }

    /// A stream opened: log it, or count it in a run of dead streams.
    fn opened(&mut self) {
        self.shared.reopens.fetch_add(1, Ordering::AcqRel);
        let (_, name, rate) = self.stream.as_ref().expect("just opened");
        let why = self.lost.take().unwrap_or_default();
        if self.dead <= 1 {
            eprintln!("audio: output reopened on '{name}' ({rate} Hz): {why}");
            self.quiet_since = Instant::now();
            return;
        }
        self.unlogged += 1;
        if self.quiet_since.elapsed() >= self.timing.quiet {
            eprintln!(
                "audio: output reopened on '{name}' {} more time(s) in {:.0} s without a frame pulled; next wait up to {:.0} s",
                self.unlogged,
                self.quiet_since.elapsed().as_secs_f32(),
                self.timing.quiet.as_secs_f32()
            );
            self.unlogged = 0;
            self.quiet_since = Instant::now();
        }
    }
}

/// The system's default output through cpal (what kira's default backend
/// opens).
#[derive(Default)]
pub struct Cpal;

impl Cpal {
    fn device_and_config() -> Result<(cpal::Device, cpal::StreamConfig), String> {
        use cpal::traits::{DeviceTrait, HostTrait};
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| "no default output device".to_string())?;
        let config = device
            .default_output_config()
            .map_err(|e| e.to_string())?
            .config();
        Ok((device, config))
    }

    fn name(device: &cpal::Device) -> String {
        use cpal::traits::DeviceTrait;
        device.name().unwrap_or_else(|_| "unnamed device".into())
    }
}

impl Output for Cpal {
    fn probe(&mut self) -> Result<(String, u32), String> {
        let (device, config) = Self::device_and_config()?;
        Ok((Self::name(&device), config.sample_rate.0))
    }

    fn open(
        &mut self,
        mut render: Render,
        errors: Arc<Errors>,
    ) -> Result<(Box<dyn std::any::Any>, String, u32), String> {
        use cpal::traits::{DeviceTrait, StreamTrait};
        let (device, config) = Self::device_and_config()?;
        let (channels, rate) = (config.channels, config.sample_rate.0);
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _| render(data, channels, rate),
                move |e| {
                    let gone = matches!(e, cpal::StreamError::DeviceNotAvailable);
                    errors.report(e.to_string(), gone);
                },
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((Box::new(stream), Self::name(&device), rate))
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! An output whose pump a test can stall, like a sound server that
    //! stops asking for frames.
    use super::*;

    #[derive(Default)]
    pub struct Control {
        pub stalled: AtomicBool,
        pub opens: AtomicU64,
        /// Opens to refuse before one succeeds.
        pub refuse: AtomicU64,
        /// Opens that succeed but never pull, before one that does.
        pub dead: AtomicU64,
    }

    pub struct Fake(pub Arc<Control>);

    struct Running {
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    impl Output for Fake {
        fn probe(&mut self) -> Result<(String, u32), String> {
            Ok(("fake".into(), 48_000))
        }

        fn open(
            &mut self,
            mut render: Render,
            _errors: Arc<Errors>,
        ) -> Result<(Box<dyn std::any::Any>, String, u32), String> {
            let refuse = self.0.refuse.load(Ordering::Acquire);
            if refuse > 0 {
                self.0.refuse.store(refuse - 1, Ordering::Release);
                return Err("refused".into());
            }
            self.0.opens.fetch_add(1, Ordering::AcqRel);
            // A new stream pulls until the test stalls it again, unless it
            // is one of the dead ones.
            let dead = self.0.dead.load(Ordering::Acquire);
            if dead > 0 {
                self.0.dead.store(dead - 1, Ordering::Release);
            }
            self.0.stalled.store(dead > 0, Ordering::Release);
            let stop = Arc::new(AtomicBool::new(false));
            let (ctl, stop2) = (self.0.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                let mut buf = vec![0.0f32; 480 * 2];
                while !stop2.load(Ordering::Acquire) {
                    if !ctl.stalled.load(Ordering::Acquire) {
                        render(&mut buf, 2, 48_000);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            });
            Ok((
                Box::new(Running {
                    stop,
                    thread: Some(thread),
                }),
                "fake".into(),
                48_000,
            ))
        }

        fn follows_default(&self) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Control, Fake};
    use super::*;

    fn quick() -> Timing {
        Timing {
            check: Duration::from_millis(20),
            stall: Duration::from_millis(150),
            retry: Duration::from_millis(40),
            quiet: Duration::from_millis(400),
        }
    }

    /// A mixer on the fake output, through the master limiter like the
    /// game's, with a long silent loop playing on it.
    fn mixer(
        ctl: &Arc<Control>,
    ) -> (
        kira::AudioManager<Watched<Fake>>,
        Arc<Shared>,
        kira::sound::static_sound::StaticSoundHandle,
    ) {
        let mut m = kira::AudioManager::<Watched<Fake>>::new(kira::AudioManagerSettings {
            main_track_builder: kira::track::MainTrackBuilder::new()
                .with_effect(crate::limiter::Builder),
            backend_settings: Settings {
                output: Fake(ctl.clone()),
                timing: quick(),
            },
            capacities: Default::default(),
            internal_buffer_size: 128,
        })
        .expect("fake output opens");
        let shared = m.backend_mut().shared();
        let data = kira::sound::static_sound::StaticSoundData {
            sample_rate: 48_000,
            frames: vec![kira::Frame::ZERO; 48_000].into(),
            settings: Default::default(),
            slice: None,
        }
        .loop_region(..);
        let h = m.play(data).expect("plays");
        (m, shared, h)
    }

    fn wait(what: &str, mut ok: impl FnMut() -> bool) {
        let t = Instant::now();
        while !ok() {
            assert!(t.elapsed() < Duration::from_secs(5), "timed out: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The output stops pulling (a sound server that dropped the stream
    /// without an error the backend acts on): the watchdog opens a new
    /// stream, the same renderer plays on and the handle the game holds
    /// still drives its sound - nothing is rebuilt above the output.
    #[test]
    fn a_stalled_output_is_reopened_and_the_same_sounds_play_on() {
        let ctl = Arc::new(Control::default());
        let (_m, shared, h) = mixer(&ctl);
        wait("first frames", || h.position() > 0.05);
        ctl.stalled.store(true, Ordering::Release);
        let frozen = shared.frames();
        std::thread::sleep(Duration::from_millis(60));
        // Nothing pulls while stalled (a callback in flight may finish).
        assert!(shared.frames() - frozen <= 480);
        wait("reopen", || shared.reopens() == 1);
        assert_eq!(ctl.opens.load(Ordering::Acquire), 2);
        let (pos, frames) = (h.position(), shared.frames());
        wait("frames after reopen", || shared.frames() > frames + 4800);
        wait("the sound advances", || h.position() > pos + 0.05);
        assert_eq!(h.state(), kira::sound::PlaybackState::Playing);
    }

    /// While no output opens the watchdog keeps trying, and the first that
    /// opens takes over.
    #[test]
    fn a_lost_output_is_retried_until_one_opens() {
        let ctl = Arc::new(Control::default());
        let (_m, shared, h) = mixer(&ctl);
        wait("first frames", || shared.frames() > 0);
        ctl.refuse.store(3, Ordering::Release);
        ctl.stalled.store(true, Ordering::Release);
        wait("reopen", || shared.reopens() == 1);
        assert_eq!(ctl.refuse.load(Ordering::Acquire), 0);
        let pos = h.position();
        wait("the sound advances", || h.position() > pos + 0.05);
    }

    /// While no output opens the mixer runs into nowhere: sounds keep
    /// time instead of all queued ones starting when an output returns.
    #[test]
    fn sounds_keep_time_while_no_output_opens() {
        let ctl = Arc::new(Control::default());
        let (_m, shared, h) = mixer(&ctl);
        wait("first frames", || shared.frames() > 0);
        ctl.refuse.store(u64::MAX, Ordering::Release);
        ctl.stalled.store(true, Ordering::Release);
        let pos = h.position();
        wait("the sound advances unheard", || h.position() > pos + 0.3);
        assert_eq!(shared.reopens(), 0);
        ctl.refuse.store(0, Ordering::Release);
        wait("reopen", || shared.reopens() == 1);
    }

    /// Streams that open but never pull are replaced with a doubling wait,
    /// and the first that pulls ends the run.
    #[test]
    fn streams_that_never_pull_back_off() {
        let ctl = Arc::new(Control::default());
        let (_m, shared, h) = mixer(&ctl);
        wait("first frames", || shared.frames() > 0);
        ctl.dead.store(3, Ordering::Release);
        let t = Instant::now();
        ctl.stalled.store(true, Ordering::Release);
        wait("a stream that pulls", || shared.reopens() == 4);
        // Four stalls (the first stream and three dead ones) plus waits of
        // 40, 80 and 160 ms; without the back-off it is about 660 ms.
        assert!(
            t.elapsed() >= Duration::from_millis(800),
            "{:?}",
            t.elapsed()
        );
        assert_eq!(ctl.opens.load(Ordering::Acquire), 5);
        let pos = h.position();
        wait("the sound advances", || h.position() > pos + 0.05);
    }

    /// A panic inside the renderer is one silent buffer: the stream lives,
    /// is not replaced, and the same sounds play on.
    #[test]
    fn a_renderer_panic_is_silence_and_the_mix_plays_on() {
        let ctl = Arc::new(Control::default());
        let (_m, shared, h) = mixer(&ctl);
        wait("first frames", || h.position() > 0.05);
        shared.panic_next.store(true, Ordering::Release);
        wait("the panic", || shared.panics() == 1);
        let pos = h.position();
        wait("the sound advances", || h.position() > pos + 0.1);
        assert_eq!(shared.reopens(), 0);
        assert_eq!(ctl.opens.load(Ordering::Acquire), 1);
    }

    /// A healthy output is never replaced.
    #[test]
    fn a_healthy_output_is_never_reopened() {
        let ctl = Arc::new(Control::default());
        let (_m, shared, _h) = mixer(&ctl);
        std::thread::sleep(quick().stall * 4);
        assert!(shared.frames() > 48_000 / 10);
        assert_eq!(shared.reopens(), 0);
        assert_eq!(ctl.opens.load(Ordering::Acquire), 1);
    }

    /// A stream replaced while it slept writes silence if it wakes up
    /// again: two streams never pull one renderer.
    #[test]
    fn a_replaced_stream_writes_silence() {
        let shared = Shared::new();
        shared.epoch.store(2, Ordering::Release);
        let mut out = [1.0f32; 8];
        shared.render(1, &mut out, 2, 48_000);
        assert_eq!(out, [0.0; 8]);
        assert_eq!(shared.frames(), 0);
    }
}
