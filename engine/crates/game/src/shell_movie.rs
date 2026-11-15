//! The BF2 shell's background movie (card BSHLA, `analysis/re-bf2-shell.md`
//! §4): the menu loops the bake transcoded to frame packs
//! (`manifest::shell`), played by the movie property a screen names and
//! decoded on a worker thread so the window never waits on a JPEG.
//!
//! A property plays its segments in the `mcfg`'s order and loops; a screen
//! that names the property already playing keeps it going
//! (`ifelem_shellscreen_fnStartMovie` plays only when it is not already
//! playing, §4 [V]). `[H]` PROVISIONAL: the order - the property's field
//! 0x4d5af670 may pick a segment at random (§4); in order is what the
//! frames cannot tell apart.

use manifest::shell::{FramePack, ShellIndex};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};

/// The shell bake's directory under the assets.
pub fn shell_dir(assets_dir: &Path) -> PathBuf {
    assets_dir.join("common_bf2").join("shell")
}

/// The bake's index; `None` before the card's bake.
pub fn load_index(assets_dir: &Path) -> Option<ShellIndex> {
    let text = pak::read_to_string(shell_dir(assets_dir).join("shell.ron")).ok()?;
    ron::from_str(&text).ok()
}

/// Decode one frame to RGBA8.
pub fn decode(jpeg: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(jpeg), opts);
    let rgba = d.decode().ok()?;
    let info = d.info()?;
    Some((rgba, u32::from(info.width), u32::from(info.height)))
}

type Job = (Arc<FramePack>, usize, u64);

#[derive(Default)]
struct JobSlot {
    pending: Option<Job>,
    shutdown: bool,
}

type Jobs = Arc<(Mutex<JobSlot>, Condvar)>;
type Decoded = (u64, Vec<u8>, u32, u32);

/// One movie property's packs, loaded.
struct Property {
    name: String,
    paths: Vec<PathBuf>,
    packs: Vec<Option<Arc<FramePack>>>,
    audio: Option<PathBuf>,
}

impl Property {
    fn pack(&mut self, seg: usize) -> Option<&Arc<FramePack>> {
        if let Some(slot) = self.packs.get_mut(seg) {
            if slot.is_none() {
                if let Some(path) = self.paths.get(seg) {
                    *slot = pak::read(path)
                        .ok()
                        .and_then(FramePack::parse)
                        .filter(|p| !p.is_empty())
                        .map(Arc::new);
                }
            }
            return slot.as_ref();
        }
        None
    }

    fn unload_packs(&mut self) {
        for slot in &mut self.packs {
            *slot = None;
        }
    }
}

pub struct ShellMovie {
    properties: Vec<Property>,
    playing: Option<usize>,
    segment: usize,
    /// Seconds into the playing segment.
    t: f32,
    looping: bool,
    finished: bool,
    /// The frame last asked of the worker, by (property, segment, frame).
    asked: Option<(usize, usize, usize)>,
    /// Changes on each property transition, including A → B → A.
    generation: u64,
    job: Jobs,
    worker: Option<std::thread::JoinHandle<()>>,
    done: mpsc::Receiver<Decoded>,
    /// Whether any frame has been shown since the property started.
    pub shown: bool,
}

// One decoder, with only the newest pending request retained. Decoding never
// holds the slot lock, so a property change or shutdown can always wake it.
fn run_worker(
    jobs: Jobs,
    tx: mpsc::Sender<Decoded>,
    mut decode_job: impl FnMut(Job) -> Option<Decoded>,
) {
    loop {
        let (lock, wake) = &*jobs;
        let mut slot = lock.lock().unwrap_or_else(|e| e.into_inner());
        while slot.pending.is_none() && !slot.shutdown {
            slot = wake.wait(slot).unwrap_or_else(|e| e.into_inner());
        }
        if slot.shutdown {
            return;
        }
        let job = slot.pending.take().expect("a job");
        drop(slot);
        let decoded = decode_job(job);
        // Drop may have arrived during decoding. Do not publish or take a
        // pending job after shutdown.
        if lock.lock().unwrap_or_else(|e| e.into_inner()).shutdown {
            return;
        }
        if let Some(decoded) = decoded {
            if tx.send(decoded).is_err() {
                return;
            }
        }
    }
}

impl Drop for ShellMovie {
    fn drop(&mut self) {
        let (lock, wake) = &*self.job;
        {
            let mut slot = lock.lock().unwrap_or_else(|e| e.into_inner());
            slot.shutdown = true;
            slot.pending = None;
            wake.notify_one();
        }
        // A decode already running may finish; join only after releasing the
        // lock it needs to observe shutdown.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ShellMovie {
    /// Load every menu loop of the bake under `assets_dir`; `None` when the
    /// shell is not baked (the shell then draws over black).
    pub fn load(assets_dir: &Path) -> Option<ShellMovie> {
        let index = load_index(assets_dir)?;
        let dir = shell_dir(assets_dir).join("movies");
        let mut properties = Vec::new();
        for m in &index.movies {
            let paths: Vec<PathBuf> = m
                .segments
                .iter()
                .map(|f| dir.join(f))
                .filter(|p| pak::is_file(p))
                .collect();
            if paths.is_empty() {
                continue;
            }
            let packs = vec![None; paths.len()];
            let audio = m
                .audio
                .as_ref()
                .map(|a| dir.join(a))
                .filter(|p| pak::is_file(p));
            properties.push(Property {
                name: m.name.clone(),
                paths,
                packs,
                audio,
            });
        }
        let job = Arc::new((Mutex::new(JobSlot::default()), Condvar::new()));
        let (tx, done) = mpsc::channel();
        let jobs = Arc::clone(&job);
        let worker = std::thread::Builder::new()
            .name("shell movie".into())
            .spawn(move || {
                run_worker(jobs, tx, |(pack, k, generation)| {
                    let (rgba, w, h) = pack.frame(k).and_then(decode)?;
                    Some((generation, rgba, w, h))
                });
            })
            .ok()?;
        Some(ShellMovie {
            properties,
            playing: None,
            segment: 0,
            t: 0.0,
            looping: true,
            finished: false,
            asked: None,
            generation: 0,
            job,
            worker: Some(worker),
            done,
            shown: false,
        })
    }

    fn reset_active(&mut self, next: Option<usize>) {
        self.generation = self.generation.wrapping_add(1);
        // Supersede any pending request; an in-flight result is rejected
        // by its generation when it returns.
        self.job.0.lock().unwrap_or_else(|e| e.into_inner()).pending = None;
        if let Some(prev) = self.playing {
            // Keep the light menu loops cached; drop heavy one-shot cutscene packs.
            let prop = &mut self.properties[prev];
            if !prop.paths.is_empty()
                && !matches!(prop.name.as_str(), "shell_main" | "shell_sub_left")
            {
                prop.unload_packs();
            }
        }
        self.playing = next;
        self.segment = 0;
        self.t = 0.0;
        self.asked = None;
        self.shown = false;
        self.finished = false;
    }

    fn start(&mut self, name: &str, looping: bool, restart_if_same: bool) {
        let k = self.properties.iter().position(|p| p.name == name);
        let switch = k != self.playing || (k.is_some() && (restart_if_same || self.finished));
        if switch {
            self.reset_active(k);
        }
        self.looping = looping;
    }

    /// Stop the active movie, release any heavy cutscene packs, and clear playback state.
    pub fn stop(&mut self) {
        if self.playing.is_some() || self.finished {
            self.reset_active(None);
        }
    }

    /// Play `name` looping (from its start unless it is the one playing).
    pub fn play(&mut self, name: &str) {
        self.start(name, true, false);
    }

    /// Play `name` once from its start unless already active; `is_finished()` becomes true after the last frame.
    pub fn play_once(&mut self, name: &str) {
        self.start(name, false, false);
    }

    /// Whether a `play_once` movie has reached its end.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Currently active movie property name, if any.
    pub fn current(&self) -> Option<&str> {
        self.playing.map(|i| self.properties[i].name.as_str())
    }

    /// Whether movie `name` exists in the baked shell index.
    pub fn has(&self, name: &str) -> bool {
        self.properties.iter().any(|p| p.name == name)
    }

    /// Path to the optional WAV audio track synced with movie `name`.
    pub fn audio_path(&self, name: &str) -> Option<&Path> {
        self.properties
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.audio.as_deref())
    }

    /// Which (segment, frame) the playing property shows `dt` seconds on.
    fn advance(&mut self, dt: f32) -> Option<(usize, usize)> {
        if self.finished {
            return None;
        }
        let p_idx = self.playing?;
        self.t += dt;
        loop {
            let p = &mut self.properties[p_idx];
            let segs = p.packs.len();
            let Some(pack) = p.pack(self.segment) else {
                self.finished = true;
                return None;
            };
            let secs = pack.seconds();
            if self.t < secs {
                let k = (self.t * pack.fps.0 as f32 / pack.fps.1 as f32) as usize;
                return Some((self.segment, k.min(pack.len() - 1)));
            }
            self.t -= secs;
            if self.segment + 1 < segs {
                self.segment += 1;
            } else if self.looping {
                self.segment = 0;
            } else {
                self.finished = true;
                return None;
            }
        }
    }

    /// Step the clock by `dt`; the newest decoded frame when one came back
    /// since the last call.
    pub fn frame(&mut self, dt: f32) -> Option<(Vec<u8>, u32, u32)> {
        if let (Some(p), Some((seg, k))) = (self.playing, self.advance(dt)) {
            if self.asked != Some((p, seg, k)) {
                self.asked = Some((p, seg, k));
                if let Some(pack) = self.properties[p].pack(seg).cloned() {
                    let (lock, wake) = &*self.job;
                    lock.lock().unwrap_or_else(|e| e.into_inner()).pending =
                        Some((pack, k, self.generation));
                    wake.notify_one();
                }
            }
        }
        let mut newest = None;
        while let Ok((generation, rgba, w, h)) = self.done.try_recv() {
            if self.playing.is_some() && generation == self.generation {
                newest = Some((rgba, w, h));
            }
        }
        if newest.is_some() {
            self.shown = true;
        }
        newest
    }

    /// Decode frame `k` of the playing property's segment `seg` now, on
    /// this thread (the goldens and tests).
    pub fn frame_now(&self, name: &str, seg: usize, k: usize) -> Option<(Vec<u8>, u32, u32)> {
        let p = self.properties.iter().find(|p| p.name == name)?;
        if let Some(Some(pack)) = p.packs.get(seg) {
            return pack.frame(k).and_then(decode);
        }
        let path = p.paths.get(seg)?;
        let pack = FramePack::parse(pak::read(path).ok()?).filter(|fp| !fp.is_empty())?;
        pack.frame(k).and_then(decode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> Arc<FramePack> {
        Arc::new(FramePack::parse(FramePack::write(1, 1, (1, 1), &[vec![0]])).unwrap())
    }

    fn synthetic() -> (ShellMovie, mpsc::Sender<Decoded>) {
        let (tx, done) = mpsc::channel();
        (
            ShellMovie {
                properties: ["A", "B"]
                    .into_iter()
                    .map(|name| Property {
                        name: name.into(),
                        paths: Vec::new(),
                        packs: vec![Some(pack())],
                        audio: None,
                    })
                    .collect(),
                playing: None,
                segment: 0,
                t: 0.0,
                looping: true,
                finished: false,
                asked: None,
                generation: 0,
                job: Arc::new((Mutex::new(JobSlot::default()), Condvar::new())),
                worker: None,
                done,
                shown: false,
            },
            tx,
        )
    }

    #[test]
    fn property_transitions_reject_completed_and_inflight_old_frames() {
        let (mut movie, tx) = synthetic();
        movie.play("A");
        movie.frame(0.0); // A requested; completion can arrive after play(B).
        let first_a = movie.generation;
        movie.play("A");
        assert_eq!(movie.generation, first_a, "the playing property continues");
        // A completed result already in the queue and a late in-flight result
        // must both be rejected after the transition.
        tx.send((first_a, vec![0], 1, 1)).unwrap();
        movie.play("B");
        tx.send((first_a, vec![1], 1, 1)).unwrap();
        assert!(movie.frame(0.0).is_none());
        assert!(!movie.shown);
        let b = movie.generation;
        tx.send((b, vec![2], 1, 1)).unwrap();
        assert_eq!(movie.frame(0.0).unwrap().0, [2]);
        assert!(movie.shown);
        movie.play("A");
        // Neither the first A nor B belongs to this second start of A.
        tx.send((first_a, vec![1], 1, 1)).unwrap();
        tx.send((b, vec![2], 1, 1)).unwrap();
        assert!(movie.frame(0.0).is_none());
        assert!(!movie.shown);
        // Accept newest available in this generation even if a newer frame
        // has already been requested: do not require per-frame serial equality.
        tx.send((movie.generation, vec![3], 1, 1)).unwrap();
        tx.send((movie.generation, vec![4], 1, 1)).unwrap();
        assert_eq!(movie.frame(1.0).unwrap().0, [4]);
        let a = movie.generation;
        movie.play("unknown");
        tx.send((a, vec![5], 1, 1)).unwrap();
        assert!(movie.frame(0.0).is_none());
        assert!(!movie.shown);
    }

    #[test]
    fn pending_requests_keep_only_the_latest_frame() {
        let (mut movie, _) = synthetic();
        movie.play("A");
        movie.frame(0.0);
        let a = movie.generation;
        assert_eq!(movie.job.0.lock().unwrap().pending.as_ref().unwrap().2, a);
        movie.play("B");
        assert!(movie.job.0.lock().unwrap().pending.is_none());
        movie.frame(0.0);
        let b = movie.generation;
        let pending = movie.job.0.lock().unwrap().pending.take().unwrap();
        assert_eq!(pending.2, b);
        assert_ne!(a, b);
        assert!(Arc::ptr_eq(
            &pending.0,
            movie.properties[1].packs[0].as_ref().unwrap()
        ));
    }

    #[test]
    fn dropping_idle_and_inflight_workers_terminates_them() {
        use std::time::Duration;
        for inflight in [false, true] {
            let (mut movie, _) = synthetic();
            let (decoded, done) = mpsc::channel();
            movie.done = done;
            let (entered_tx, entered) = mpsc::channel();
            let (release, released) = mpsc::channel();
            let jobs = Arc::clone(&movie.job);
            movie.worker = Some(std::thread::spawn(move || {
                run_worker(jobs, decoded, |(_, _, generation)| {
                    entered_tx.send(()).unwrap();
                    released.recv_timeout(Duration::from_secs(2)).unwrap();
                    Some((generation, vec![1], 1, 1))
                });
            }));
            if inflight {
                movie.play("A");
                movie.frame(0.0);
                entered.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            let jobs = Arc::clone(&movie.job);
            let (finished, completion) = mpsc::channel();
            let dropper = std::thread::spawn(move || {
                drop(movie);
                finished.send(()).unwrap();
            });
            if inflight {
                // Observe the actual shutdown predicate before releasing the
                // decoder, rather than relying on thread-count timing.
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while !jobs.0.lock().unwrap().shutdown {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
                assert!(completion.try_recv().is_err());
                release.send(()).unwrap();
            }
            completion.recv_timeout(Duration::from_secs(2)).unwrap();
            dropper.join().unwrap();
            assert!(jobs.0.lock().unwrap().shutdown);
            assert!(jobs.0.lock().unwrap().pending.is_none());
        }
    }

    /// Acceptance 3 of card BSHLA: the menu loops decode - segment 0's
    /// frames 0 and 300 are 512 x 448 pictures (pinned by checksum, the
    /// first one of the letterboxed movie's black band) - and the playback
    /// steps through `shell_main`'s two segments. Skips, saying so, on a
    /// bake from before the card.
    #[test]
    fn the_menu_loops_decode_and_play_in_order() {
        let assets = crate::assets::baked_root();
        let Some(mut m) = ShellMovie::load(&assets) else {
            eprintln!(
                "skip: no shell bake under {} (bake common)",
                assets.display()
            );
            return;
        };
        let sum = |rgba: &[u8]| {
            rgba.iter().fold(0x811c_9dc5u32, |h, b| {
                (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
            })
        };
        let mut sums = Vec::new();
        for k in [0, 300] {
            let (rgba, w, h) = m.frame_now("shell_main", 0, k).expect("frame decodes");
            assert_eq!((w, h), (512, 448));
            // The movie is letterboxed: its top rows are black; 10 s in,
            // its middle is not (frame 0 is the loop's fade from black).
            let row = |y: usize| &rgba[y * 512 * 4..(y + 1) * 512 * 4];
            assert!(row(4)
                .chunks(4)
                .all(|p| p[0] < 24 && p[1] < 24 && p[2] < 24));
            let lit = row(224)
                .chunks(4)
                .any(|p| p[0] > 40 || p[1] > 40 || p[2] > 40);
            assert_eq!(lit, k == 300, "frame {k}");
            sums.push(sum(&rgba));
        }
        assert_ne!(sums[0], sums[1], "frames 0 and 300 differ");
        eprintln!(
            "segment 0 frames 0 / 300 checksums: {:08x} / {:08x}",
            sums[0], sums[1]
        );
        m.play("shell_main");
        let playing = m.playing.unwrap();
        let first = m.properties[playing].pack(0).unwrap().seconds();
        assert_eq!(m.advance(0.0), Some((0, 0)));
        assert_eq!(m.advance(first + 0.05).map(|(s, _)| s), Some(1));
        // Back to segment 0 after the second.
        let second = m.properties[playing].pack(1).unwrap().seconds();
        assert_eq!(m.advance(second).map(|(s, _)| s), Some(0));
        // The worker hands a frame back.
        m.play("shell_sub_left");
        let mut got = None;
        for _ in 0..200 {
            got = got.or(m.frame(0.0));
            if got.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(got.is_some_and(|(_, w, h)| (w, h) == (512, 448)));
    }

    #[test]
    fn play_once_stops_at_end_and_exposes_audio_path() {
        let (mut m, _) = synthetic();
        m.properties[0].audio = Some(PathBuf::from("movies/A.wav"));
        assert!(m.has("A"));
        assert_eq!(m.audio_path("A"), Some(Path::new("movies/A.wav")));
        assert_eq!(m.audio_path("B"), None);
        m.play_once("A");
        let gen = m.generation;
        assert_eq!(m.current(), Some("A"));
        assert!(!m.is_finished());
        assert_eq!(m.advance(0.5), Some((0, 0)));
        assert!(!m.is_finished());
        m.play_once("A");
        assert_eq!(m.generation, gen, "calling play_once per frame keeps progress");
        assert!((m.t - 0.5).abs() < 1e-6);
        assert_eq!(m.advance(0.6), None);
        assert!(m.is_finished());
        m.stop();
        assert_eq!(m.current(), None);
        assert!(!m.is_finished());
    }
}
