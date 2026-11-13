//! Loading presentation, raw button edges and one owned CPU scene worker.
//! No renderer, simulation tick or surface operation runs on the worker.

use std::collections::HashSet;
use std::hash::Hash;
use std::path::{Path, PathBuf};
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
}

impl Playback {
    pub(super) fn press(&mut self, elapsed: Duration) {
        if self.skipped_at.is_some() {
            return;
        }
        if self.prompt(elapsed) {
            self.skipped_at = Some(elapsed);
            self.prompt_until = None;
        } else {
            self.prompt_until = Some(elapsed + SKIP_WINDOW);
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

/// Completed CPU data remains owned by its round. Drop joins even on close/error,
/// so a later playlist row cannot overlap an abandoned parse. File IO itself is
/// not cancellable; a close may wait for that one outstanding read/parse.
pub(super) struct Worker<T>(Option<JoinHandle<anyhow::Result<T>>>);

impl<T: Send + 'static> Worker<T> {
    pub(super) fn spawn(
        task: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<Self> {
        Ok(Self(Some(
            std::thread::Builder::new()
                .name("loading-scene".into())
                .spawn(task)?,
        )))
    }

    pub(super) fn poll(&mut self) -> Option<anyhow::Result<T>> {
        if !self.0.as_ref()?.is_finished() {
            return None;
        }
        Some(
            self.0
                .take()
                .unwrap()
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("scene worker panicked"))),
        )
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            let _ = handle.join();
        }
    }
}

pub(super) struct Prepared {
    pub scene: game::assets::Scene,
    pub load: game::assets::BattleLoad,
    pub cpu_ms: f64,
}

pub(super) fn prepare(dir: PathBuf, mode: String) -> anyhow::Result<Worker<Prepared>> {
    Worker::spawn(move || {
        let started = Instant::now();
        let (scene, load) = game::assets::load_battle_scene(&dir, &mode)?;
        Ok(Prepared {
            scene,
            load,
            cpu_ms: started.elapsed().as_secs_f64() * 1000.0,
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
                    audio.play_voice(s, LOADING_OUTPUT_DB + 20.0 * 0.7_f32.log10(), 1.0, 0.0)
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
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;

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
