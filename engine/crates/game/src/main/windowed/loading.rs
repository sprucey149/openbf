//! Loading presentation, raw button edges and one owned CPU worker: the
//! scene parse, then the round's CPU side (`super::round_prep`). No renderer,
//! simulation tick or surface operation runs on the worker.

use std::collections::HashSet;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// User contract: the second fresh press must precede this deadline.
const SKIP_WINDOW: Duration = Duration::from_secs(3);
/// OURS: hold time for each still when the disc has no matching BF1 layout.
/// measure: the galaxy-only launch and plain frame on each source profile.
const FALLBACK_STEP: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct Playback {
    prompt_until: Option<Duration>,
    skipped_at: Option<Duration>,
    /// A LAN guest's loading (issue #168): presses neither prompt nor skip;
    /// only the host's skip ends its presentation early.
    no_skip: bool,
}

impl Playback {
    /// A LAN guest's playback: no press shortens it.
    pub(super) fn no_skip() -> Self {
        Self {
            no_skip: true,
            ..Self::default()
        }
    }

    pub(super) fn press(&mut self, elapsed: Duration) {
        if self.no_skip || self.skipped_at.is_some() {
            return;
        }
        if self.prompt(elapsed) {
            self.skipped_at = Some(elapsed);
            self.prompt_until = None;
        } else {
            self.prompt_until = Some(elapsed + SKIP_WINDOW);
        }
    }

    /// The LAN host skipped for everyone (issue #168): the presentation
    /// ends now, as a skip of this screen's own would end it.
    pub(super) fn host_skipped(&mut self, elapsed: Duration) {
        if self.skipped_at.is_none() {
            self.skipped_at = Some(elapsed);
            self.prompt_until = None;
        }
    }

    pub(super) fn prompt(&self, elapsed: Duration) -> bool {
        self.prompt_until.is_some_and(|until| elapsed < until) && self.skipped_at.is_none()
    }

    pub(super) fn skipped(&self) -> bool {
        self.skipped_at.is_some()
    }

    pub(super) fn time(&self, elapsed: Duration) -> Duration {
        self.skipped_at.unwrap_or(elapsed)
    }

    pub(super) fn done(&self, elapsed: Duration, duration: Duration) -> bool {
        self.skipped_at.is_some() || elapsed >= duration
    }

    pub(super) fn can_enter(
        &self,
        elapsed: Duration,
        duration: Duration,
        shown: bool,
        ready: bool,
    ) -> bool {
        shown && ready && self.done(elapsed, duration)
    }
}

/// What a loading frame does after drawing (issue #168 for a LAN round).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Next {
    /// Keep drawing.
    Draw,
    /// Enter the battle now.
    Enter,
    /// A LAN round start: install the battle and hold the last image.
    Hold,
    /// A LAN round start: show the held battle (the host's start).
    Show,
}

/// The loading gate. `net`: a LAN round's (a join or a rejoin, the start
/// moment once `Start` is heard); `presented`: [`Playback::can_enter`];
/// `built`: drawn once and the CPU side ready; `held`: a battle is held.
///
/// Single player enters at the end of its presentation. A LAN join enters
/// as soon as it is built (it catches up from the log). A LAN round start
/// plays its presentation (cut short on every peer when the host skips:
/// `presented` is then true), installs and holds until the start; one
/// that hears `Start` first (the barrier's timeout went on without it)
/// installs as soon as it is built and shows at once.
pub(super) fn next(
    net: Option<(bool, Option<Instant>)>,
    presented: bool,
    built: bool,
    held: bool,
    now: Instant,
) -> Next {
    let go = |b| if b { Next::Enter } else { Next::Draw };
    match net {
        None => go(presented),
        Some((true, _)) => go(built),
        Some((false, start)) if held => {
            if start.is_some_and(|t| now >= t) {
                Next::Show
            } else {
                Next::Draw
            }
        }
        Some((false, start)) => {
            if presented || (built && start.is_some()) {
                Next::Hold
            } else {
                Next::Draw
            }
        }
    }
}

/// Physical identities include unbound/unknown keys and controller codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Button {
    Key(winit::keyboard::PhysicalKey),
    Mouse(winit::event::MouseButton),
    Pad(gilrs::GamepadId, gilrs::ev::Code),
}

pub(super) struct Edges<B: Eq + Hash> {
    held: HashSet<B>,
    blocked: HashSet<B>,
}

impl<B: Eq + Hash> Default for Edges<B> {
    fn default() -> Self {
        Self {
            held: HashSet::new(),
            blocked: HashSet::new(),
        }
    }
}

impl<B: Eq + Hash + Copy> Edges<B> {
    /// Always update releases, but never count OS repeats as a fresh press.
    pub(super) fn update(&mut self, button: B, down: bool, repeat: bool) -> (bool, bool) {
        let blocked = self.blocked.contains(&button);
        let fresh = if down {
            self.held.insert(button) && !repeat
        } else {
            self.held.remove(&button);
            self.blocked.remove(&button);
            false
        };
        (fresh && !blocked, blocked)
    }

    pub(super) fn quarantine(&mut self) {
        self.blocked.extend(self.held.iter().copied());
    }
}

/// Completed CPU data remains owned by its round. Drop joins (an error, a new
/// round), so a later playlist row cannot overlap an abandoned parse. The
/// game's close alone [`Worker::abandon`]s it: the process ends next, and the
/// parse and sound decode may take many seconds on a slow PC.
pub(super) struct Worker<T> {
    handle: Option<JoinHandle<anyhow::Result<T>>>,
    /// Set when the task has returned or panicked (the [`Heartbeat`]'s
    /// `worker=` reading, from its own thread).
    done: Arc<AtomicBool>,
}

/// Sets the worker's done flag however its task ends, a panic included.
struct DoneOnDrop(Arc<AtomicBool>);
impl Drop for DoneOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl<T: Send + 'static> Worker<T> {
    pub(super) fn spawn(
        task: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<Self> {
        let done = Arc::new(AtomicBool::new(false));
        let flag = DoneOnDrop(done.clone());
        let handle = std::thread::Builder::new()
            .name("loading-scene".into())
            .spawn(move || {
                let _flag = flag;
                task()
            })?;
        Ok(Self {
            handle: Some(handle),
            done,
        })
    }

    /// The close's way out (`close.rs`): the thread is left running
    /// unjoined and ends with the process. It holds CPU data only (the
    /// scene's parse and the round's CPU side, no GPU objects).
    pub(super) fn abandon(&mut self) {
        drop(self.handle.take());
    }

    pub(super) fn poll(&mut self) -> Option<anyhow::Result<T>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        Some(
            self.handle
                .take()
                .unwrap()
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("scene worker panicked"))),
        )
    }

    /// Whether the task has ended (read from any thread).
    pub(super) fn done_flag(&self) -> Arc<AtomicBool> {
        self.done.clone()
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// `[ours]` How often a loading in progress prints its heartbeat line.
/// measure: a slow machine's report; one line per 10 s is cheap.
pub(super) const BEAT_EVERY: Duration = Duration::from_secs(10);
/// `[ours]` No loading frame drawn for this long (a hidden or minimised
/// window that gets no redraws): the event loop steps the loading itself,
/// without painting (`App::loading_unpainted`).
/// measure: frame gaps on a minimised macOS window.
pub(super) const STALL: Duration = Duration::from_secs(1);
/// How often that unpainted step runs while redraws stay away.
const UNPAINTED_EVERY: Duration = Duration::from_millis(100);

/// What a loading is doing, for its heartbeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    /// The presentation plays; the worker parses and builds.
    Presenting = 0,
    /// The event thread uploads the scene to the GPU and installs the round.
    Upload = 1,
    /// A LAN round start: installed and held for the host's start.
    Held = 2,
}

impl Phase {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Upload,
            2 => Self::Held,
            _ => Self::Presenting,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Presenting => "presenting",
            Self::Upload => "gpu-upload",
            Self::Held => "held",
        }
    }
}

/// The heartbeat line: enough to tell a slow parse (`worker=running`,
/// frames climbing), a stalled event thread (frames stuck, the last frame
/// long ago) and a stuck upload (`phase=gpu-upload`) apart in a log.
pub(super) fn beat_line(
    map: &str,
    elapsed: Duration,
    phase: Phase,
    worker_done: bool,
    frames: u64,
    since_frame: Option<Duration>,
) -> String {
    let last =
        since_frame.map_or_else(|| "never".to_owned(), |d| format!("{:.1}", d.as_secs_f64()));
    format!(
        "loading: heartbeat map={map} elapsed_s={:.1} phase={} worker={} frames={frames} \
         last_frame_s_ago={last}",
        elapsed.as_secs_f64(),
        phase.name(),
        if worker_done { "done" } else { "running" },
    )
}

/// Whether the event loop should step a loading without painting: no
/// frame (or, before the first, no start) for [`STALL`], and none of its
/// own steps in the last [`UNPAINTED_EVERY`].
pub(super) fn unpainted_due(
    started: Instant,
    last_paint: Option<Instant>,
    last_unpainted: Option<Instant>,
    now: Instant,
) -> bool {
    now.saturating_duration_since(last_paint.unwrap_or(started)) >= STALL
        && last_unpainted.is_none_or(|t| now.saturating_duration_since(t) >= UNPAINTED_EVERY)
}

struct BeatState {
    map: String,
    started: Instant,
    stop: AtomicBool,
    frames: AtomicU64,
    /// Milliseconds after `started` of the last frame drawn; `u64::MAX`
    /// before the first.
    last_frame_ms: AtomicU64,
    phase: AtomicU8,
    worker_done: Arc<AtomicBool>,
}

impl BeatState {
    fn line(&self) -> String {
        let elapsed = self.started.elapsed();
        let last = self.last_frame_ms.load(Ordering::Relaxed);
        beat_line(
            &self.map,
            elapsed,
            Phase::from_u8(self.phase.load(Ordering::Relaxed)),
            self.worker_done.load(Ordering::Acquire),
            self.frames.load(Ordering::Relaxed),
            (last != u64::MAX).then(|| elapsed.saturating_sub(Duration::from_millis(last))),
        )
    }
}

/// A loading's heartbeat: its own thread prints [`beat_line`] every
/// [`BEAT_EVERY`] while the loading lives, so a log shows the loading's
/// progress even when the event thread itself is stuck. The default has no
/// thread (the tests' loadings). It also keeps the event thread's paint and
/// unpainted-step times. Drop stops the thread at once.
#[derive(Default)]
pub(super) struct Heartbeat {
    state: Option<Arc<BeatState>>,
    thread: Option<JoinHandle<()>>,
    pub last_paint: Option<Instant>,
    pub last_unpainted: Option<Instant>,
}

impl Heartbeat {
    pub(super) fn start(map: &str, started: Instant, worker_done: Arc<AtomicBool>) -> Self {
        Self::start_with(map, started, worker_done, BEAT_EVERY, |line| {
            println!("{line}")
        })
    }

    /// [`Heartbeat::start`] with its period and its line's sink.
    pub(super) fn start_with(
        map: &str,
        started: Instant,
        worker_done: Arc<AtomicBool>,
        every: Duration,
        sink: impl Fn(String) + Send + 'static,
    ) -> Self {
        let state = Arc::new(BeatState {
            map: map.to_owned(),
            started,
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            last_frame_ms: AtomicU64::new(u64::MAX),
            phase: AtomicU8::new(Phase::Presenting as u8),
            worker_done,
        });
        let beat = state.clone();
        let thread = std::thread::Builder::new()
            .name("loading-heartbeat".into())
            .spawn(move || {
                let mut next = beat.started + every;
                loop {
                    let now = Instant::now();
                    if beat.stop.load(Ordering::Acquire) {
                        return;
                    }
                    if now >= next {
                        sink(beat.line());
                        next += every;
                        continue;
                    }
                    std::thread::park_timeout(next - now);
                }
            });
        let thread = match thread {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("loading: no heartbeat: {e}");
                None
            }
        };
        Self {
            state: Some(state),
            thread,
            last_paint: None,
            last_unpainted: None,
        }
    }

    /// A loading frame was drawn.
    pub(super) fn painted(&mut self, now: Instant) {
        self.last_paint = Some(now);
        if let Some(s) = &self.state {
            s.frames.fetch_add(1, Ordering::Relaxed);
            let ms = now.saturating_duration_since(s.started).as_millis();
            s.last_frame_ms
                .store(u64::try_from(ms).unwrap_or(u64::MAX - 1), Ordering::Relaxed);
        }
    }

    pub(super) fn set_phase(&self, phase: Phase) {
        if let Some(s) = &self.state {
            s.phase.store(phase as u8, Ordering::Relaxed);
        }
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        if let Some(s) = &self.state {
            s.stop.store(true, Ordering::Release);
        }
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

pub(super) struct Prepared {
    pub scene: game::assets::Scene,
    pub load: game::assets::BattleLoad,
    pub cpu_ms: f64,
    /// The round's CPU side, built after the parse on the same worker
    /// (`super::round_prep`); `None` when the loading asked for the scene
    /// alone, and the event thread builds the round as before.
    pub round: Option<super::round_prep::PreparedRound>,
}

/// The scene alone (the tests' event-thread build).
#[cfg(test)]
pub(super) fn prepare(dir: PathBuf, mode: String) -> anyhow::Result<Worker<Prepared>> {
    prepare_round(dir, mode, None)
}

/// [`prepare`], then the round's CPU side on the same worker when `job` is
/// given (`super::round_prep`, LAN §10.5).
pub(super) fn prepare_round(
    dir: PathBuf,
    mode: String,
    job: Option<super::round_prep::RoundJob>,
) -> anyhow::Result<Worker<Prepared>> {
    Worker::spawn(move || {
        let key = game::assets::prefetch::load_key(&dir, &mode);
        std::thread::scope(|s| {
            let warm = std::thread::Builder::new()
                .name("loading-prefetch".into())
                .spawn_scoped(s, || {
                    let t = Instant::now();
                    let n = game::assets::prefetch::warm(&key);
                    (n, t.elapsed().as_secs_f64() * 1000.0)
                });
            let prepared = (|| -> anyhow::Result<Prepared> {
                let started = Instant::now();
                let layers = job
                    .as_ref()
                    .map(|j| {
                        let script = j
                            .campaign_index
                            .map(|i| ui::campaign::RISE_OF_THE_EMPIRE[i].script);
                        super::super::round_setup::campaign_layers_for(
                            &j.missions_dir,
                            &j.name,
                            &mode,
                            script,
                        )
                    })
                    .unwrap_or_default();
                let (mut scene, load) =
                    game::assets::load_battle_scene_with_layers(&dir, &mode, &layers)?;
                if let Some(j) = &job {
                    let map = j
                        .net
                        .as_ref()
                        .map_or(j.name.as_str(), |(r, _)| r.map.as_str());
                    scene.kill_objects(&game::mission::scene_kills(&j.missions_dir, map, &mode));
                }
                let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
                let round = job.map(|j| j.build(&scene)).transpose()?;
                Ok(Prepared {
                    scene,
                    load,
                    cpu_ms,
                    round,
                })
            })();
            if let Ok((n, ms)) = warm.map(|h| h.join().unwrap_or((0, 0.0))) {
                println!("loading: prefetch key={key} decoded={n} warm_ms={ms:.3}");
            }
            if prepared.is_err() {
                game::assets::prefetch::discard();
            }
            prepared
        })
    })
}

/// BF2 locations reuse an authored sequence from the same planet. Prefer
/// the matching location where BF1 has one. Space maps use their planet's
/// ground sequence.
fn bf1_donor(key: &str) -> Option<&str> {
    match key {
        "bf2_end1" => Some("bf1_end1"),
        "bf2_geo1" => Some("bf1_geo1"),
        "bf2_hot1" | "bf2_spa8" => Some("bf1_hot1"),
        "bf2_kam1" => Some("bf1_kam1"),
        "bf2_kas2" | "bf2_spa3" => Some("bf1_kas2"),
        "bf2_nab2" => Some("bf1_nab2"),
        "bf2_yav1" | "bf2_spa1" => Some("bf1_yav1"),
        "tat2" | "bf2_tat2" => Some("bf1_tat2"),
        "bf2_tat3" | "bf2_spa9" => Some("tat1"),
        _ => None,
    }
}

/// BF2-only destinations share orbit views across ground and space maps.
fn bf2_orbit(key: &str) -> Option<&'static str> {
    match key {
        "bf2_cor1" => Some("orbit_coruscant"),
        "bf2_dag1" => Some("orbit_dagobah"),
        "bf2_dea1" => Some("orbit_death_star"),
        "bf2_fel1" | "bf2_spa7" => Some("orbit_felucia"),
        "bf2_mus1" => Some("orbit_mustafar"),
        "bf2_myg1" | "bf2_spa6" => Some("orbit_mygeeto"),
        "bf2_pol1" => Some("orbit_polismassa"),
        "bf2_uta1" => Some("orbit_utapau"),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum Motion {
    Authored,
    Arrival,
    Still,
}

struct Presentation {
    sequence: Vec<(String, game::assets::CpuTexture)>,
    motion: Motion,
}

fn presentation(root: &Path, key: &str) -> Option<Presentation> {
    // Identify the source by manifest content, including legacy map keys.
    let maps = game::assets::discover_maps(root);
    let own = maps.iter().find(|(k, _, _)| k == key);
    let bf1 = own
        .filter(|(_, source, _)| source == "bf1")
        .map(|(_, _, dir)| dir.clone())
        .or_else(|| bf1_donor(key).map(|k| root.join(k)));
    if let Some(assets) = bf1
        .and_then(|dir| game::assets::load_loadscreen(&dir))
        .filter(|s| !s.pool)
    {
        return Some(Presentation {
            sequence: assets.sequence,
            motion: Motion::Authored,
        });
    }
    // The shared space pool mixes eras. Choose Clone Wars ships for the
    // two BF2-only stock space maps; select the first still on ground maps.
    let preferred = match key {
        "bf2_spa6" => "spa_6",
        "bf2_spa7" => "spa_7",
        _ => "",
    };
    let arrival = own
        .filter(|(_, source, _)| source == "bf2")
        .and_then(|(_, _, dir)| game::assets::load_loading_still(dir, preferred));
    let galaxy = maps
        .into_iter()
        .filter(|(_, source, _)| source == "bf1")
        .find_map(|(_, _, dir)| game::assets::load_loading_galaxy(&dir));
    let orbit = if key == "bf2_tan1" {
        // Tantive IV was boarded over Tatooine. Keep its own interior art
        // as the destination and borrow only Tatooine's view from orbit.
        game::assets::load_loading_image(&root.join("tat1"), "load_locale_tatooine_1".into())
    } else {
        bf2_orbit(key).and_then(|name| {
            game::assets::load_loading_image(&root.join("common_bf2"), name.into())
        })
    };
    let sequence: Vec<_> = galaxy.into_iter().chain(orbit).chain(arrival).collect();
    let motion = if sequence.len() > 1 {
        Motion::Arrival
    } else {
        Motion::Still
    };
    (!sequence.is_empty()).then_some(Presentation { sequence, motion })
}

#[derive(Default)]
pub(super) struct Display {
    textures: Vec<ui::egui::TextureHandle>,
    authored: Option<AuthoredDisplay>,
}

struct AuthoredDisplay {
    motion: ui::loading::Choreography,
    sprites: ui::loading::Sprites,
}

impl Display {
    pub(super) fn load(root: &Path, key: &str, ctx: &ui::egui::Context) -> Self {
        let Some(assets) = presentation(root, key) else {
            return Self::default();
        };
        let authored = Self::load_motion(root, key, &assets, ctx);
        let textures = assets
            .sequence
            .into_iter()
            .filter_map(|(name, texture)| {
                upload(ctx, name, texture, ui::egui::TextureOptions::LINEAR)
            })
            .collect();
        Self { textures, authored }
    }

    fn load_motion(
        root: &Path,
        key: &str,
        assets: &Presentation,
        ctx: &ui::egui::Context,
    ) -> Option<AuthoredDisplay> {
        use ui::egui::{pos2, Rect};
        let dir = root.join("common_bf1/load");
        let config = pak::read_to_string(dir.join("layout.ron")).ok()?;
        let layout: manifest::LoadingLayout = ron::from_str(&config).ok()?;
        let rects = match assets.motion {
            Motion::Arrival => {
                // OURS: BF2 arrivals use a centered scan for each transition
                // with BF1's cue timing. measure: map-specific crop framing.
                let centered = Rect::from_min_max(pos2(0.25, 0.25), pos2(0.75, 0.75));
                let mut rects = vec![centered; assets.sequence.len().checked_sub(1)?];
                if key == "bf2_tan1" && rects.len() == 2 {
                    // Preserve BF1's authored Tatooine orbit target when its
                    // exact plate is present.
                    if let Some(target) =
                        layout.maps.get("tat1").and_then(|targets| targets.first())
                    {
                        let (name, texture) = &assets.sequence[0];
                        if target.image == *name {
                            rects[0] = target_rect(target, texture);
                        }
                    }
                }
                rects
            }
            Motion::Authored => {
                let key = bf1_donor(key).unwrap_or(key);
                let targets = layout.maps.get(key.strip_prefix("bf1_").unwrap_or(key))?;
                let (last, targets) = targets.split_last()?;
                if targets.len() + 1 != assets.sequence.len()
                    || last.image != assets.sequence.last()?.0
                {
                    return None;
                }
                targets
                    .iter()
                    .zip(&assets.sequence)
                    .map(|(target, (name, texture))| {
                        if target.image != *name {
                            return None;
                        }
                        Some(target_rect(target, texture))
                    })
                    .collect::<Option<Vec<_>>>()?
            }
            Motion::Still => return None,
        };
        let motion =
            ui::loading::Choreography::new(layout.tracking_seconds, layout.zoom_seconds, rects)?;
        use ui::egui::TextureOptions;
        let read = |name, options| load_texture(ctx, &dir, name, options);
        let [h, v, c] = layout.edges;
        let noise = layout.noise;
        let sprites = ui::loading::Sprites {
            disk: read(layout.disk, TextureOptions::LINEAR)?,
            led: read(layout.led, TextureOptions::LINEAR)?,
            edges: [
                read(h, TextureOptions::LINEAR)?,
                read(v, TextureOptions::LINEAR)?,
                read(c, TextureOptions::LINEAR)?,
            ],
            scanline: read(layout.scanline, TextureOptions::LINEAR_REPEAT)?,
            noise: ui::loading::Noise {
                frames: noise
                    .frames
                    .into_iter()
                    .filter_map(|name| read(name, TextureOptions::LINEAR_REPEAT))
                    .collect(),
                frame_rate: noise.frame_rate,
                uv_start: noise.uv_start,
            },
            label: loading_label(ctx, &root.join("common_bf1/fonts")),
        };
        Some(AuthoredDisplay { motion, sprites })
    }

    pub(super) fn duration(&self) -> Duration {
        self.authored.as_ref().map_or_else(
            || FALLBACK_STEP * self.textures.len().max(1) as u32,
            |display| display.motion.duration(),
        )
    }

    pub(super) fn sample(&self, time: Duration) -> Option<ui::loading::Frame> {
        Some(self.authored.as_ref()?.motion.sample(time))
    }

    pub(super) fn draw(
        &self,
        ctx: &ui::egui::Context,
        caption: &str,
        time: Duration,
        frame: Option<&ui::loading::Frame>,
        prompt: bool,
    ) {
        let image = |index| self.textures.get(index).map(ui::egui::TextureHandle::id);
        if let (Some(display), Some(frame)) = (&self.authored, frame) {
            ui::loading::draw_choreography(
                ctx,
                [image(frame.step), image(frame.step + 1)],
                frame,
                &display.sprites,
                time,
                prompt,
            );
        } else {
            let step = (time.as_secs() as usize).min(self.textures.len().saturating_sub(1));
            ui::loading::draw_loading_with_skip(
                ctx,
                image(step),
                caption,
                step,
                self.textures.len(),
                prompt,
            );
        }
    }
}

fn target_rect(
    target: &manifest::LoadingTarget,
    texture: &game::assets::CpuTexture,
) -> ui::egui::Rect {
    let [x, y, w, h] = target.rect;
    let (tw, th) = (texture.width as f32, texture.height as f32);
    ui::egui::Rect::from_min_max(
        ui::egui::pos2(x / tw, y / th),
        ui::egui::pos2((x + w) / tw, (y + h) / th),
    )
}

fn loading_label(ctx: &ui::egui::Context, dir: &Path) -> Option<ui::loading::Label> {
    let source = pak::read_to_string(dir.join("bf1_gamefont_large.ron")).ok()?;
    let font: manifest::hud::HudFont = ron::from_str(&source).ok()?;
    let pages = font
        .pages
        .iter()
        .map(|name| load_texture(ctx, dir, name.clone(), ui::egui::TextureOptions::LINEAR))
        .collect::<Option<Vec<_>>>()?;
    Some(ui::loading::Label { font, pages })
}

fn load_texture(
    ctx: &ui::egui::Context,
    dir: &Path,
    name: String,
    options: ui::egui::TextureOptions,
) -> Option<ui::egui::TextureHandle> {
    let bytes = pak::read(dir.join(format!("{name}.ktx2"))).ok()?;
    upload(ctx, name, game::assets::load_ktx2(&bytes).ok()?, options)
}

fn upload(
    ctx: &ui::egui::Context,
    name: String,
    texture: game::assets::CpuTexture,
    options: ui::egui::TextureOptions,
) -> Option<ui::egui::TextureHandle> {
    let size = [texture.width as usize, texture.height as usize];
    if size.contains(&0) || size[0].checked_mul(size[1])?.checked_mul(4)? != texture.rgba.len() {
        return None;
    }
    Some(ctx.load_texture(
        name,
        ui::egui::ColorImage::from_rgba_unmultiplied(size, &texture.rgba),
        options,
    ))
}

/// OURS: loading-bus gain matched to the PS2 captures on Bespin and Hoth.
/// measure: the original loading bus and SPU2 output gain directly.
const LOADING_OUTPUT_DB: f32 = -8.5;
// A 70% linear gain for the coordinate tracking cues.
const TRACKING_GAIN_DB: f32 = -3.0980394;

pub(super) struct Cues {
    vertical: Option<audio::SoundData>,
    horizontal: Option<audio::SoundData>,
    zoom: Option<audio::SoundData>,
    current: Option<(usize, ui::loading::Cue)>,
    tracking: Option<audio::Voice>,
    shot: Option<audio::Shot>,
}

impl Cues {
    pub(super) fn load(root: &Path) -> Self {
        let track = |axis| {
            audio::decode(&root.join(format!("audio/load_{axis}tracking.ogg")))
                .or_else(|_| audio::decode(&root.join(format!("audio/load_{axis}tracking.wav"))))
                .ok()
        };
        Self {
            vertical: track("y"),
            horizontal: track("x"),
            zoom: audio::decode(&root.join("core_bf1/sounds/1dff06ae/b75d27cd.wav")).ok(),
            current: None,
            tracking: None,
            shot: None,
        }
    }

    pub(super) fn update(&mut self, audio: &mut audio::Audio, frame: Option<&ui::loading::Frame>) {
        use ui::loading::Cue;
        let next = frame.and_then(|f| f.cue.map(|cue| (f.step, cue)));
        if next == self.current {
            return;
        }
        self.current = next;
        self.tracking = None;
        self.shot = None;
        match next.map(|(_, cue)| cue) {
            Some(Cue::Zoom) => {
                self.shot = self
                    .zoom
                    .as_ref()
                    .and_then(|s| audio.play_shot(s, LOADING_OUTPUT_DB, 1.5, 0.0));
            }
            Some(Cue::Vertical | Cue::Horizontal) => {
                let source = if matches!(next, Some((_, Cue::Vertical))) {
                    &self.vertical
                } else {
                    &self.horizontal
                };
                self.tracking = source.as_ref().and_then(|s| {
                    audio.play_voice(s, LOADING_OUTPUT_DB + TRACKING_GAIN_DB, 1.0, 0.0)
                });
            }
            None => {}
        }
    }
}

pub(super) struct Loading {
    pub map_name: String,
    pub started: Instant,
    pub playback: Playback,
    pub display: Display,
    pub worker: Worker<Prepared>,
    pub ready: Option<Prepared>,
    pub shown: bool,
    pub cues: Option<Cues>,
    /// A LAN round start's installed battle, held behind the last image
    /// until the host's start (issue #168).
    pub held: Option<super::Held>,
    /// The heartbeat line, the frames drawn, and when this loading was
    /// last drawn or stepped unpainted.
    pub beat: Heartbeat,
}

/// Use the input layer's existing unified/raw HID filtering on loading too:
/// one physical controller exposed twice must not turn one press into a skip.
fn usable_pads(g: &gilrs::Gilrs) -> Vec<gilrs::GamepadId> {
    input::usable_pads(
        g.gamepads()
            .map(|(id, p)| (id, input::pad_kind(&p), input::hw_id(&p))),
    )
    .into_iter()
    .map(|(id, _)| id)
    .collect()
}

pub(super) fn pad_buttons_down(g: &gilrs::Gilrs) -> bool {
    usable_pads(g)
        .into_iter()
        .any(|id| g.gamepad(id).state().buttons().any(|(_, b)| b.is_pressed()))
}

/// A skip's held controller buttons stay neutral until a released sample.
/// Raw skip edges are still tracked individually during loading.
pub(super) fn suppress_pad_until_release(blocked: &mut bool, down: bool) -> bool {
    *blocked &= down;
    *blocked
}

impl super::App {
    pub(super) fn clear_loading_input(&mut self) {
        self.keys_held = 0;
        self.keys_tapped = 0;
        self.prev_menu_buttons = 0;
        self.kbm.release_keys();
        self.shell_pointer.clear();
        self.spawn_clicks.clear();
    }

    pub(super) fn any_pad_button_down(&self) -> bool {
        self.gilrs.as_ref().is_some_and(pad_buttons_down)
    }

    pub(super) fn seed_loading_pads(&mut self) {
        // Retain keyboard/mouse held state, replace possibly stale pad state.
        self.loading_edges
            .held
            .retain(|b| !matches!(b, Button::Pad(..)));
        self.loading_edges
            .blocked
            .retain(|b| !matches!(b, Button::Pad(..)));
        if let Some(g) = &self.gilrs {
            for id in usable_pads(g) {
                for (code, b) in g.gamepad(id).state().buttons() {
                    if b.is_pressed() {
                        self.loading_edges
                            .update(Button::Pad(id, code), true, false);
                    }
                }
            }
        }
    }

    fn loading_press(&mut self) {
        if let Some(l) = self.loading.as_mut() {
            l.playback.press(l.started.elapsed());
        }
    }

    /// Called before egui, menu bindings, F9 and gameplay. Even unbound buttons
    /// belong to loading; quarantined holds cannot choose a side afterward.
    pub(super) fn loading_window_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        use winit::event::{ElementState, WindowEvent};
        let button = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => Some((
                Button::Key(event.physical_key),
                event.state,
                event.repeat || *is_synthetic,
            )),
            WindowEvent::MouseInput { button, state, .. } => {
                Some((Button::Mouse(*button), *state, false))
            }
            WindowEvent::Focused(false) => {
                self.loading_edges
                    .held
                    .retain(|b| matches!(b, Button::Pad(..)));
                self.loading_edges
                    .blocked
                    .retain(|b| matches!(b, Button::Pad(..)));
                None
            }
            _ => None,
        };
        if let Some((button, state, repeat)) = button {
            let (fresh, blocked) =
                self.loading_edges
                    .update(button, state == ElementState::Pressed, repeat);
            let loading = matches!(self.screen, super::Screen::Loading);
            if loading || blocked {
                if let (Some(o), Some(w)) = (&mut self.overlay, &self.window) {
                    let _ = o.on_window_event(w, event);
                }
            }
            if loading {
                if fresh && self.focused {
                    self.loading_press();
                }
                return true;
            }
            return blocked;
        }
        false
    }

    pub(super) fn pump_loading_pads(&mut self) {
        use gilrs::EventType as E;
        let Some(g) = self.gilrs.as_mut() else { return };
        let usable = usable_pads(g);
        let skipped: Vec<_> = g
            .gamepads()
            .map(|(id, _)| id)
            .filter(|id| !usable.contains(id))
            .collect();
        let mut presses = Vec::new();
        let mut changed = false;
        while let Some(ev) = g.next_event() {
            match ev.event {
                E::ButtonPressed(_, code) | E::ButtonReleased(_, code)
                    if !skipped.contains(&ev.id) =>
                {
                    let down = matches!(ev.event, E::ButtonPressed(..));
                    let (fresh, _) =
                        self.loading_edges
                            .update(Button::Pad(ev.id, code), down, false);
                    if down {
                        self.active_pad.input(ev.id, 1.0, Instant::now());
                    }
                    // Keep individual tap times: two queued taps must not become
                    // one menu bit, and repeats/axes never become skip presses.
                    if fresh && self.focused {
                        presses.push(Instant::now());
                    }
                }
                E::Disconnected => {
                    self.active_pad.disconnected(ev.id);
                    self.loading_edges
                        .held
                        .retain(|b| !matches!(b, Button::Pad(id, _) if *id == ev.id));
                    changed = true;
                }
                E::Connected => changed = true,
                _ => {}
            }
        }
        if changed {
            self.pad_warning = input::log_pads_with(
                g,
                &self.profile.settings.controller_bindings,
                &self.active_pad,
            );
            self.gameplay_stick.select(None);
        }
        for at in presses {
            if let Some(l) = self.loading.as_mut() {
                l.playback.press(at.saturating_duration_since(l.started));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    include!("loading/tests.rs");
}
