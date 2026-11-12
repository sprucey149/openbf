//! Loading presentation, raw button edges and one owned CPU scene worker.
//! No renderer, simulation tick or surface operation runs on the worker.

use std::collections::HashSet;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// User contract: the second fresh press must precede this deadline.
const SKIP_WINDOW: Duration = Duration::from_secs(3);
/// PROVISIONAL presentation cadence, inherited from the flat slideshow;
/// trace: original BF1 transition/zoom/crosshair timing before replacing playback.
const STEP: Duration = Duration::from_secs(1);

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

    pub(super) fn step(&self, elapsed: Duration, steps: usize) -> usize {
        (self.skipped_at.unwrap_or(elapsed).as_secs_f64() / STEP.as_secs_f64())
            .floor()
            .min(steps.max(1).saturating_sub(1) as f64) as usize
    }

    pub(super) fn done(&self, elapsed: Duration, steps: usize) -> bool {
        self.skipped_at.is_some() || elapsed >= STEP * steps.max(1) as u32
    }

    pub(super) fn can_enter(
        &self,
        elapsed: Duration,
        steps: usize,
        shown: bool,
        ready: bool,
    ) -> bool {
        shown && ready && self.done(elapsed, steps)
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

/// Only direct shared locations get a complete BF1 sequence. Other locations
/// use the galaxy alone, never another planet's ground imagery. This flat
/// fallback is an openbf presentation choice, not original BF1 choreography.
fn equivalent(key: &str) -> Option<&str> {
    match key {
        "bf2_hot1" => Some("bf1_hot1"),
        "bf2_yav1" => Some("bf1_yav1"),
        "tat2" | "bf2_tat2" => Some("bf1_tat2"),
        _ => None,
    }
}

pub(super) fn presentation(root: &Path, key: &str) -> Option<game::assets::LoadScreenAssets> {
    // Identify the source by manifest content, including legacy map keys.
    let maps = game::assets::discover_maps(root);
    let own = maps
        .iter()
        .find(|(k, source, _)| k == key && source == "bf1")
        .map(|(_, _, dir)| dir.clone());
    let target = own.or_else(|| equivalent(key).map(|k| root.join(k)));
    if let Some(dir) = target {
        if let Some(ls) = game::assets::load_loadscreen(&dir).filter(|s| !s.pool) {
            return Some(ls);
        }
    }
    // discover_maps is sorted and also supports packed assets.
    for (_, source, dir) in maps {
        if source != "bf1" {
            continue;
        }
        if let Some(mut ls) = game::assets::load_loadscreen(&dir).filter(|s| !s.pool) {
            // An arbitrary first image is not necessarily a galaxy view.
            if ls
                .sequence
                .first()
                .is_some_and(|(name, _)| name.starts_with("load_galacticmap"))
            {
                ls.sequence.truncate(1);
                return Some(ls);
            }
        }
    }
    None // Missing/corrupt owned data: the UI still paints a loading frame.
}

pub(super) struct Loading {
    pub map_name: String,
    pub started: Instant,
    pub playback: Playback,
    pub textures: Vec<ui::egui::TextureHandle>,
    pub worker: Worker<Prepared>,
    pub ready: Option<Prepared>,
    pub shown: bool,
    pub beep: Option<audio::LoopHandle>,
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
