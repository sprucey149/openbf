//! The windowed game (winit + gilrs): one `App` state machine for the
//! BF2-style shell, the BF1 loading screen and the battle's per-frame loop —
//! [`run_windowed`] for `--map`, [`run_shell`] for the shell — and the glue
//! only a window uses: event sounds, the pause menu's actions and cheat words,
//! the profile autosave and the title-bar line.

#[path = "windowed/events.rs"]
mod events;

use super::battle::{
    hud_model, load_and_upload, load_hud_view, make_walk, spawn_model, squad_icons, stats_pages,
    walk_draws,
};
use super::{BattleSetup, ViewerArgs};
use anyhow::Result;
use core::sim::PadState;
use game::viewer::{camera_from, upload_unit_cube, SceneGpu, CLEAR, GOLDEN_CAM};
use game::walk::WalkSim;
use render::{Camera, DrawCmd, MeshHandle, Renderer};
use std::path::PathBuf;
use std::sync::Arc;

/// Start take `take` of Music `music` from the disc, silent-safe: a take
/// the indexes do not hold, or an LVL that is not there, plays nothing.
fn start_take(
    audio: &mut audio::Audio,
    m: &game::music::BattleMusic,
    music: usize,
    take: usize,
    volume_db: f32,
) -> Option<audio::MusicVoice> {
    let track = m.track(music)?;
    let t = track.takes.get(take)?.clone();
    audio
        .play_stream(t, volume_db, track.def.looping)
        .map_err(|e| eprintln!("music: {e:#}"))
        .ok()
}
/// Start the sounds of the tick the sim just stepped (`game::event_sounds`).
fn play_event_sounds(
    audio: &mut audio::Audio,
    sl: &mut game::sounds::SoundLib,
    foley: Option<&mut game::foley::Foley>,
    engines: &mut game::engine_sound::EngineSounds,
    effects: &[(String, glam::Vec3)],
    aspect: f32,
    walk: &WalkSim,
) {
    let listener = game::event_sounds::listener(walk);
    game::event_sounds::each_off_handle(walk, listener.pos, |name, pos| {
        sl.play(audio, name, pos, &listener);
    });
    // A human's shots and empty clicks, which share a weapon's handle.
    let mut asks = Vec::new();
    game::event_sounds::weapon_asks(walk, |a| asks.push(a));
    sl.play_weapon_voices(audio, &asks, &listener);
    // The hurt and death cries, one voice per unit (card UCRY).
    let mut asks = Vec::new();
    game::event_sounds::cries(walk, |c| {
        let hash = c.name.map_or(0, game::classdb::fnv1a_lowercase);
        asks.push((c.unit, hash, c.pos));
    });
    sl.play_cries(audio, &asks, &listener);
    // The stations' ambient chatter, one handle per station (card PWST).
    sl.play_station_ambient(audio, &game::event_sounds::station_asks(walk), &listener);
    // The sounds of the explosions' effects (`Fx::take_sounds`).
    for (name, pos) in effects {
        sl.play(audio, name, *pos, &listener);
    }
    // The layers whose delay ends this tick (card WSND).
    sl.update(audio, core::TICK_DT, &listener);
    // The vehicles' engine layers (card VSND).
    let mut voices = Vec::new();
    engines.tick(walk, |h, l| sl.engine(h, l), &mut voices);
    sl.loop_voices(audio, &voices, &listener);
    // The jets' turn-on and turn-off (card JETP2), at the soldier.
    for s in engines.take_jet_shots() {
        match s.kind {
            game::engine_sound::JetShotKind::Boost { rising } => {
                sl.play_boost(audio, s.unit, &s.name, rising, s.pos, &listener);
            }
            _ => {
                sl.play(audio, &s.name, s.pos, &listener);
            }
        }
    }
    sl.play_repair_loops(audio, &walk.sim, &listener);
    sl.play_shield_loops(audio, &walk.sim, &listener);
    sl.walker_fire_loops(audio, walk, &listener);
    sl.guided_loops(audio, &walk.sim, &listener);
    sl.play_hero_blades(audio, walk, &listener);
    sl.play_hero_samples(
        audio,
        &game::event_sounds::hero_sample_asks(walk),
        &listener,
    );
    // The soldiers' steps and stance sounds (card FOLY).
    if let Some(foley) = foley {
        let view = game::foley::View::of(&walk.camera(), aspect);
        foley.tick(walk, &view, |s| {
            sl.play_by_hash(audio, s.hash, s.pos, &listener);
        });
    }
}

/// The world's ambience streams and loops for the tick the sim just
/// stepped (card AMBS); the battle's end stops every sound
/// (re-ambience-music 0.4).
fn play_ambience(
    audio: &mut audio::Audio,
    sl: &mut game::sounds::SoundLib,
    ambience: &game::ambience::Ambience,
    walk: &WalkSim,
) {
    let listener = game::event_sounds::listener(walk);
    if walk.sim.outcome.is_some() {
        sl.stop_ambient();
    } else {
        sl.ambient_voices(audio, ambience, &listener);
        if game::dev::on() && walk.sim.tick.is_multiple_of(10 * core::TICK_HZ as u64) {
            eprintln!(
                "ambience: {} of {} emitters sounding",
                sl.ambient_playing(),
                ambience.emitters.len()
            );
        }
    }
}

/// The playtest black box for the battle about to start — dev mode only
/// (`game::blackbox`, card BLKB); `None` in normal play.
fn start_blackbox(
    scene: &game::assets::Scene,
    walk: &WalkSim,
    mode: &str,
    setup: &BattleSetup,
    sounds: Option<&mut game::sounds::SoundLib>,
) -> Option<game::blackbox::BlackBox> {
    use game::blackbox::{BlackBox, Meta};
    if !game::blackbox::on() {
        return None;
    }
    // The baked directory's name (`bf2_kam1`): what the sweep calls the map.
    let map = scene.dir.file_name().map_or_else(
        || scene.manifest.map.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut meta = Meta::for_run(
        &map,
        mode,
        &scene.manifest.source_game,
        setup.seed,
        setup.difficulty.label(),
    );
    meta.mission = setup.mission_file.clone().unwrap_or_default();
    meta.player_team = setup.player_team;
    let dir = game::blackbox::default_dir();
    println!("blackbox: on, F9 writes {}/last.json", dir.display());
    let mut bb = BlackBox::start(true, meta, walk, scene.collision_body_labels(), dir)?;
    bb.install_panic_hook();
    // Its sound rows (card SNDB): the mixer's starts, by property name.
    if let Some(sl) = sounds {
        sl.handed = Some(Vec::new());
        let root = scene.dir.parent().unwrap_or(&scene.dir).to_path_buf();
        bb.sound_names(game::event_sounds::sound_names(&scene.dir, walk), root);
    }
    Some(bb)
}

/// The match log for the battle about to start (card DPRX,
/// `game::matchlog`): dev mode, unless `OPENBF_MATCHLOG=0`. Follows the
/// player's slot for the whole round.
fn start_matchlog(
    scene: &game::assets::Scene,
    walk: &WalkSim,
    mode: &str,
    setup: &BattleSetup,
) -> Option<game::matchlog::MatchLog> {
    if !game::matchlog::on() {
        return None;
    }
    let map = scene.dir.file_name().map_or_else(
        || scene.manifest.map.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut meta = game::blackbox::Meta::for_run(
        &map,
        mode,
        &scene.manifest.source_game,
        setup.seed,
        setup.difficulty.label(),
    );
    meta.mission = setup.mission_file.clone().unwrap_or_default();
    meta.player_team = setup.player_team;
    let cw = walk
        .teams
        .iter()
        .any(|t| t.faction == "rep" || t.faction == "cis");
    let dir = game::blackbox::default_dir().join("matches");
    let log = game::matchlog::MatchLog::start(
        &meta,
        walk,
        walk.player.index,
        "human",
        if cw { "cw" } else { "gcw" },
        &dir,
    );
    println!("matchlog: on, the round writes {}", log.path().display());
    Some(log)
}

/// Hand the black box the one-shots the mixer was given since the last call.
fn note_sounds(
    bb: &mut game::blackbox::BlackBox,
    sounds: Option<&mut game::sounds::SoundLib>,
    tick: u64,
) {
    if let Some(sl) = sounds {
        let loops = sl.loops_playing();
        if let Some(handed) = sl.handed.as_mut() {
            bb.sounds(tick, handed, loops);
        }
    }
}

/// `OPENBF_WINDOW_SIZE=WxH` in physical pixels (a measurement knob, card
/// ISPD: the same frame at a laptop panel's resolution).
fn window_size_env() -> Option<(u32, u32)> {
    let s = std::env::var("OPENBF_WINDOW_SIZE").ok()?;
    let (w, h) = s.split_once('x')?;
    Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?))
}

/// The window opens maximized (playtest A5), restoring to 1280x720
/// logical; a forced size opens at exactly that size, not maximized.
fn window_attributes(want: Option<(u32, u32)>) -> winit::window::WindowAttributes {
    let base = winit::window::Window::default_attributes().with_title("openbf");
    match want {
        Some((w, h)) if w > 0 && h > 0 => base.with_inner_size(winit::dpi::PhysicalSize::new(w, h)),
        _ => base
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .with_maximized(true),
    }
}

/// The weather's own random stream starts here (never the sim's).
const WEATHER_SEED: u32 = 0x7ea7_4e12;

/// Restart Mission's playlist: `row` plays next, then the rest in order.
fn requeue_front(
    playlist: &mut std::collections::VecDeque<ui::shell::Launch>,
    row: ui::shell::Launch,
) {
    playlist.push_front(row);
}

/// The velocity of what the player's camera tracks — the soldier, or the
/// vehicle ridden: what tilts a rain streak (re-weather.md §3.1).
fn tracked_velocity(walk: &WalkSim) -> glam::Vec3 {
    let Some(me) = walk.sim.unit(walk.player) else {
        return glam::Vec3::ZERO;
    };
    match me.mount.and_then(|m| walk.sim.vehicle(m.vehicle)) {
        Some(v) => v.vel,
        None => me.soldier.vel,
    }
}

/// This frame's rain splashes (re-weather.md §5): each point dropped by a
/// ray from 50 m over the camera, 100 m down, onto terrain or buildings —
/// the ordnance ray's mask — and the map's `GroundEffect` started there.
fn rain_splashes(
    w: &mut game::weather::Weather,
    fx: &mut game::fx::Fx,
    walk: &WalkSim,
    aspect: f32,
    dt: f32,
) {
    let Some(name) = w.precip.ground_effect.clone() else {
        return;
    };
    let view = game::weather::View::from_camera(&walk.camera(), aspect);
    let sim = &walk.sim;
    let down = glam::Vec3::NEG_Y;
    let hits = w.splashes(dt, &view, 1, &mut |top| {
        let body = sim
            .collision
            .ray(top, down, 100.0, core::collision::Layers::ORDNANCE)
            .map(|(t, _)| t);
        let ground = sim.terrain_ray(top, down, 100.0, 0.5);
        let t = match (body, ground) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a.or(b)?,
        };
        Some(top + down * t)
    });
    for hit in hits {
        fx.spawn(&name, game::fx::emitter::Frame::at(hit));
    }
}

/// Battle state as one line — the window title and the round-over stdout line
/// (the on-screen HUD is `ui::hud::draw_hud`).
fn hud_title(walk: &WalkSim) -> String {
    let b = &walk.sim.battle;
    let posts = |t: u8| walk.sim.cps.iter().filter(|c| c.owner == Some(t)).count();
    let me = &walk.sim.units[walk.player.index as usize];
    let status = match walk.sim.outcome {
        Some(core::sim::OUTCOME_DRAW) => "TIME UP — both teams win".to_string(),
        Some(w) => format!("ROUND OVER — team {} wins", w + 1),
        None if !me.alive => "respawning...".to_string(),
        None => format!(
            "HP {:.0} | ammo {}/{}",
            me.health.cur, me.weapon.clip, me.weapon.spare
        ),
    };
    format!(
        "openbf — tickets {}/{} | posts {}/{} | {}",
        b.teams[0].tickets(),
        b.teams[1].tickets(),
        posts(0),
        posts(1),
        status
    )
}

// ── Windowed viewer (winit 0.30 ApplicationHandler + gilrs) ──────────────────

#[path = "windowed/career.rs"]
mod career;
use career::record_round;
/// The BF2 PS2 shell's front screens (card BSHLA).
#[path = "windowed/front.rs"]
mod front;
use front::FrontShell;
#[path = "windowed/menu_actions.rs"]
mod menu_actions;
use menu_actions::{apply_pause_action, resolve_direct_settings};
#[path = "windowed/idle.rs"]
mod idle;
#[path = "windowed/mouse.rs"]
mod mouse;

enum Screen {
    Shell,
    Loading,
    Battle,
}

#[path = "windowed/loading.rs"]
mod loading;
use loading::Loading;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpawnTarget {
    Side(usize),
    Post(core::unit::SpawnPost),
}

#[derive(Debug, Default, Clone)]
struct SpawnClickTracker {
    last_target: Option<SpawnTarget>,
    last_pos: [f32; 2],
    timer: f32,
    prev_stage: Option<core::sim::SpawnStage>,
}

impl SpawnClickTracker {
    fn step(&mut self, dt: f32) {
        self.timer += dt;
    }

    fn check_double_click(&mut self, target: SpawnTarget, pos: [f32; 2]) -> bool {
        const DOUBLE_CLICK_TIME: f32 = 0.45;
        const DOUBLE_CLICK_DIST: f32 = 25.0;

        let is_double = self.last_target == Some(target) && self.timer <= DOUBLE_CLICK_TIME && {
            let dx = pos[0] - self.last_pos[0];
            let dy = pos[1] - self.last_pos[1];
            (dx * dx + dy * dy) <= DOUBLE_CLICK_DIST * DOUBLE_CLICK_DIST
        };

        if is_double {
            self.last_target = None;
            self.timer = f32::INFINITY;
            true
        } else {
            self.last_target = Some(target);
            self.last_pos = pos;
            self.timer = 0.0;
            false
        }
    }

    fn clear(&mut self) {
        self.last_target = None;
        self.timer = f32::INFINITY;
    }
}

struct App {
    /// Direct launch target (--map); None = started in the shell.
    map: Option<PathBuf>,
    screen: Screen,
    /// The Instant Action playlist still to play after the battle in play
    /// (`analysis/re-bf2-menus.md` §7: the PS2 plays the whole list in
    /// order), the row in play and the session options it launched with.
    playlist: std::collections::VecDeque<ui::shell::Launch>,
    current: Option<ui::shell::Launch>,
    session: ui::shell::SessionOptions,
    /// The round ended and its end screens were left (the next playlist
    /// row loads); a pause-menu Quit leaves it false and ends the list.
    round_over: bool,
    assets_dir: PathBuf,
    missions_dir: PathBuf,
    loading: Option<Loading>,
    loading_edges: loading::Edges<loading::Button>,
    loading_pad_blocked: bool,
    audio: Option<audio::Audio>,
    sounds: Option<game::sounds::SoundLib>,
    /// The playing take of the battle's music (card AMBM); dropping it
    /// stops it.
    music_voice: Option<audio::MusicVoice>,
    /// The battle's soldier foley (card FOLY), when the map has a table.
    foley: Option<game::foley::Foley>,
    /// The battle's vehicle engines (card VSND).
    engines: game::engine_sound::EngineSounds,
    /// The world's fixed sound emitters (card AMBS).
    ambience: game::ambience::Ambience,
    cam: Camera,
    walk_requested: bool,
    setup: BattleSetup,
    mode: String,
    frames: u64,
    outcome_reported: bool,
    overlay: Option<ui::overlay::EguiOverlay>,
    /// The message box's lines (kills, posts, the player's orders).
    feed: game::event_feed::EventFeed,
    paused: bool,
    pause_menu: game::pause::PauseMenu,
    /// Placeholder muzzle/impact effects (visual only — `game::fx`).
    fx: game::fx::Fx,
    /// The map's precipitation and wind (visual only — `game::weather`).
    weather: Option<game::weather::Weather>,
    /// The posts' holograms (visual only — `game::holo`).
    holos: game::holo::Holos,
    /// The laser bolts' ribbons (card FXB — `game::bolt_vis`).
    bolts: game::bolt_vis::BoltVis,
    fx_quad: Option<MeshHandle>,
    fx_sprite: Option<render::TextureHandle>,
    /// D11's two icons: `squad_cmd_highlight` over a commanded bot,
    /// `squad_cmd_cross` on a Hold's anchor.
    squad_icons: (Option<render::TextureHandle>, Option<render::TextureHandle>),
    /// BF2's HUD for the battle in play (a fresh one per battle: its fades
    /// start over).
    hud_view: Option<game::hud_feed::HudView>,
    /// The playtest black box (dev mode; `None` in normal play) and an F9
    /// press waiting for the frame to write it.
    blackbox: Option<game::blackbox::BlackBox>,
    /// Card DPRX: the whole round's record of the player (dev mode).
    matchlog: Option<game::matchlog::MatchLog>,
    snapshot_requested: bool,
    /// Quit chosen in the pause menu; handled after the frame.
    quit_clicked: bool,
    /// Restart Mission confirmed in the pause menu; handled after the frame.
    restart_clicked: bool,
    /// The frame record (dev mode, card HITCH) and `OPENBF_FRAME_PROFILE=<csv>`'s
    /// row of CPU / GPU ms per battle frame (`game::frame_profile`).
    frame_profile: Option<game::frame_profile::FrameProfile>,
    /// Low Power Resolution's controller (`game::dyn_res`).
    dyn_res: game::dyn_res::DynRes,
    /// What it holds this battle's frames to, read from the display once.
    dyn_res_budget_ms: Option<f32>,
    profile: game::profile::Profile,
    round_recorded: bool,
    /// Previous frame's menu-relevant buttons (edge detection).
    prev_menu_buttons: u32,
    /// Separate from menu/D-pad state; reset when the sampled controller changes.
    gameplay_stick: GameplayStick<gilrs::GamepadId>,
    /// The left stick's learned travel (#142); reset with `gameplay_stick`.
    stick_range: input::StickRange,
    /// The last sampled raw left stick and its full-push travel, for the
    /// black box's human row.
    stick_sample: ([f32; 2], [f32; 2]),
    /// The pad that last sent input; read instead of the first (issue #56).
    active_pad: input::ActivePad<gilrs::GamepadId>,
    /// Which device the Auto input icons follow.
    icon_device: game::input_icons::DeviceTracker,
    /// The menu bits the keyboard holds (the shell only: arrows, Enter as
    /// Cross + Start, Escape / Backspace as Back; card BSHLA).
    keys_held: u32,
    /// The bits pressed since the last frame read them: a tap that is let
    /// go inside one frame still counts.
    keys_tapped: u32,
    /// A held value step repeats on the pause menu, as a held arrow in the shell.
    pause_repeat: ui::shell_feel::HoldRepeat,
    /// The shell pointer: position, fresh motion and a waiting click.
    shell_pointer: ui::cursor::Pointer,
    spawn_clicks: SpawnClickTracker,
    kbm: game::kbm::Kbm,
    focused: bool,
    /// The PS2 shell's front screens (`None` until the shell first draws).
    front: Option<FrontShell>,
    /// One click of L3 sprints (`input::SprintLatch`).
    sprint_latch: input::SprintLatch,
    /// The buttons held through a resume, kept from play until released
    /// (`game::pause::resume_pad`).
    resume_gate: core::sim::HeldEat,
    view_toggle: game::viewpoint::ViewToggle,
    /// `OPENBF_AUTOPILOT=1` in dev mode: [`autopilot_pad`] drives the player.
    autopilot: bool,
    /// `OPENBF_QUIT_AT=<s>` in dev mode: the game quits (as the window's
    /// close does) when the battle's clock reaches it - an unattended run's
    /// end with its exit snapshot and frame summary.
    quit_at: Option<u64>,
    /// The session's cheats and the pause menu's Morse recogniser (card
    /// CHEAT2): copied into every battle, all off back at the main menu.
    cheats: game::cheats::Session,
    /// Keeps the display awake while a round is played (playtest A1);
    /// declared before `window` so it goes first.
    idle: Option<idle::IdleInhibit>,
    window: Option<Arc<winit::window::Window>>,
    renderer: Option<Renderer>,
    gpu: Option<SceneGpu>,
    walk: Option<(WalkSim, MeshHandle)>,
    /// Animated soldier models (per team) — `None` before a battle starts.
    soldier_vis: Option<game::soldier_vis::SoldierVisuals>,
    /// The spawn screens' 3-D soldiers (card SSELB).
    spawn_preview: game::spawn_preview::Preview,
    /// The battle's vehicle models (card VDRAW) — empty before a battle.
    vehicle_vis: game::viewer::VehicleVisuals,
    deka_vis: game::viewer::DroidekaVisuals,
    gilrs: Option<gilrs::Gilrs>,
    /// A connected pad's dead camera stick / FIRE (`input::log_pads`), shown
    /// on every screen until a pad that resolves replaces it (card PADM).
    pad_warning: Option<String>,
    last: std::time::Instant,
}

/// `OPENBF_AUTOPILOT=1` (dev, card HITCH): the player's pad walks forward,
/// wanders left and right, fires in bursts and presses the spawn screens'
/// accept and the use button now and then, so an unattended windowed run
/// moves the view over the map (the frame recorder's replays). A function of
/// the sim tick only.
fn autopilot_pad(tick: u64) -> PadState {
    use core::sim::buttons as b;
    let t = tick as f32 * core::TICK_DT;
    let mut buttons = 0;
    if t % 5.0 < 1.5 {
        buttons |= b::FIRE;
    }
    if t % 0.5 < 0.25 {
        buttons |= b::SPAWN_ACCEPT;
    }
    if t % 7.0 < 0.2 {
        buttons |= b::ACTION;
    }
    PadState {
        lx: 0,
        ly: PadState::quantize(1.0),
        rx: PadState::quantize(
            0.35 * core::fmath::sin_cos(t * 0.4).0 + 0.1 * core::fmath::sin_cos(t * 1.7).0,
        ),
        ry: 0,
        buttons,
        ..PadState::default()
    }
}

/// Decode every take the battle can play, behind the loading (card HITCH:
/// a take decoded at its first play cost its frame up to 5 ms).
fn preload_sounds(sl: &mut game::sounds::SoundLib) {
    let (n, bytes, ms) = sl.preload();
    println!(
        "sounds: {n} takes decoded at load ({:.1} MB, {ms:.0} ms)",
        bytes as f64 / (1024.0 * 1024.0)
    );
}

/// `OPENBF_QUIT_AT=<seconds>` (dev mode): the battle tick the game quits at.
fn quit_at() -> Option<u64> {
    let s: f32 = std::env::var("OPENBF_QUIT_AT").ok()?.parse().ok()?;
    game::dev::on().then_some((s * core::TICK_HZ as f32) as u64)
}

/// Open gilrs through `input::open_gilrs` and log how each pad resolved.
fn open_pads(mapping: &input::Mapping) -> (Option<gilrs::Gilrs>, Option<String>) {
    match input::open_gilrs() {
        Ok(g) => {
            let w = input::log_pads_with(&g, mapping, &input::ActivePad::default());
            (Some(g), w)
        }
        Err(e) => {
            eprintln!("gilrs unavailable ({e}); no controller input");
            (None, None)
        }
    }
}

struct GameplayStick<Id> {
    controller: Option<Id>,
    arms: u32,
}

impl<Id> Default for GameplayStick<Id> {
    fn default() -> Self {
        Self {
            controller: None,
            arms: 0,
        }
    }
}

impl<Id: PartialEq> GameplayStick<Id> {
    fn select(&mut self, controller: Option<Id>) {
        if self.controller != controller || controller.is_none() {
            self.arms = 0;
        }
        self.controller = controller;
    }
}

/// The left-stick travel a sample reads as a full push, per axis: the pad's
/// learned range for a soldier on foot in a ground mode (#142),
/// [`input::STICK_FULL`] in every seat and anywhere in a space mode, so
/// ground vehicle, walker, flyer and all space input never change with the
/// range.
fn stick_full(range: &input::StickRange, learned: bool) -> [f32; 2] {
    if learned {
        range.full()
    } else {
        [input::STICK_FULL; 2]
    }
}

/// Run the event loop. A loop that ends in an error has already dropped the
/// display connection the idle inhibit borrows, so that is let go without
/// its teardown requests (the display drops the inhibit with the client).
fn run_app(event_loop: winit::event_loop::EventLoop<()>, app: &mut App) -> Result<()> {
    if let Err(e) = event_loop.run_app(app) {
        if let Some(idle) = app.idle.take() {
            std::mem::forget(idle);
        }
        return Err(e.into());
    }
    Ok(())
}

pub(super) fn run_windowed(v: ViewerArgs) -> Result<()> {
    let event_loop = winit::event_loop::EventLoop::new()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let profile = game::profile::Profile::load_or_default();
    let (gilrs, pad_warning) = open_pads(&profile.settings.controller_bindings);
    let setup = BattleSetup::from_args(&v, v.seed)?;
    let audio = audio::Audio::new();
    let frame_profile = game::frame_profile::FrameProfile::from_env();
    let autopilot = game::dev::on() && std::env::var("OPENBF_AUTOPILOT").is_ok_and(|v| v == "1");
    let quit_at = quit_at();
    let mut setup = setup;
    resolve_direct_settings(&mut setup, v.difficulty, &profile.settings);
    let assets_dir = v
        .map
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let mut app = App::new(
        assets_dir,
        PathBuf::from("missions"),
        setup,
        gilrs,
        pad_warning,
        audio,
        profile,
    );
    app.frame_profile = frame_profile;
    app.autopilot = autopilot;
    app.quit_at = quit_at;
    app.map = Some(v.map);
    app.screen = Screen::Battle;
    app.cam = camera_from(v.cam);
    app.walk_requested = v.walk;
    app.mode = v.mode;
    run_app(event_loop, &mut app)
}

impl App {
    fn new(
        assets_dir: PathBuf,
        missions_dir: PathBuf,
        setup: BattleSetup,
        gilrs: Option<gilrs::Gilrs>,
        pad_warning: Option<String>,
        audio: Option<audio::Audio>,
        profile: game::profile::Profile,
    ) -> Self {
        let mut kbm = game::kbm::Kbm::default();
        kbm.follow(&profile.settings);
        // `--cheat` starts the session with its flags.
        let cheats = game::cheats::Session::new(setup.cheats);
        Self {
            map: None,
            screen: Screen::Shell,
            playlist: std::collections::VecDeque::new(),
            current: None,
            session: ui::shell::SessionOptions::default(),
            round_over: false,
            loading: None,
            loading_edges: Default::default(),
            loading_pad_blocked: false,
            audio,
            sounds: None,
            music_voice: None,
            foley: None,
            engines: Default::default(),
            ambience: Default::default(),
            assets_dir,
            missions_dir,
            cam: camera_from(GOLDEN_CAM),
            walk_requested: true,
            setup,
            mode: "con".to_string(),
            frames: 0,
            outcome_reported: false,
            overlay: None,
            feed: game::event_feed::EventFeed::default(),
            paused: false,
            pause_menu: game::pause::PauseMenu::default(),
            quit_clicked: false,
            restart_clicked: false,
            frame_profile: None,
            dyn_res: game::dyn_res::DynRes::default(),
            dyn_res_budget_ms: None,
            profile,
            round_recorded: false,
            fx: game::fx::Fx::default(),
            weather: None,
            holos: game::holo::Holos::default(),
            bolts: game::bolt_vis::BoltVis::default(),
            fx_quad: None,
            fx_sprite: None,
            squad_icons: (None, None),
            hud_view: None,
            blackbox: None,
            matchlog: None,
            snapshot_requested: false,
            prev_menu_buttons: 0,
            gameplay_stick: GameplayStick::default(),
            stick_range: input::StickRange::default(),
            stick_sample: ([0.0; 2], [0.0; 2]),
            active_pad: input::ActivePad::default(),
            icon_device: game::input_icons::DeviceTracker::default(),
            keys_held: 0,
            keys_tapped: 0,
            pause_repeat: Default::default(),
            shell_pointer: ui::cursor::Pointer::default(),
            spawn_clicks: SpawnClickTracker::default(),
            focused: true,
            kbm,
            front: None,
            sprint_latch: input::SprintLatch::default(),
            resume_gate: core::sim::HeldEat::default(),
            view_toggle: game::viewpoint::ViewToggle::default(),
            autopilot: false,
            quit_at: None,
            cheats,
            idle: None,
            window: None,
            renderer: None,
            gpu: None,
            walk: None,
            soldier_vis: None,
            spawn_preview: Default::default(),
            vehicle_vis: Default::default(),
            deka_vis: Default::default(),
            gilrs,
            pad_warning,
            last: std::time::Instant::now(),
        }
    }

    /// A click on the spawn screens: the pick goes into the next tick's pad
    /// (`Kbm::set_pick`, the sim applies it, `PadState::pick`), and a double
    /// click adds the screens' accept. The window writes no sim state.
    fn step_spawn_pointer(&mut self, dt: f32) {
        let scale = self
            .hud_view
            .as_ref()
            .and_then(game::hud_feed::HudView::spawn_map_scale);
        self.step_spawn_pointer_at(dt, scale);
    }

    /// [`App::step_spawn_pointer`] with the spawn map drawn at the Map
    /// element's `map_scale` (`None`: no HUD pass draws the map, so a click
    /// there picks no post).
    fn step_spawn_pointer_at(&mut self, dt: f32, map_scale: Option<glam::Vec3>) {
        self.spawn_clicks.step(dt);
        if self.paused {
            return;
        }
        let Some((walk, _)) = self.walk.as_ref() else {
            return;
        };
        // The player's unit even while dead behind the screens, as the
        // spawn page reads it (`Sim::unit` sees only a live one, which put
        // every click on team 1's posts).
        let me = walk.sim.units.get(walk.player.index as usize);
        let slot = me.map_or(0, |u| u.spec.player_slot.map_or(0, usize::from));
        let team = me.map_or(0, |u| u.team);
        let stage = walk.sim.spawn_stage.get(slot).copied().unwrap_or_default();
        if self.spawn_clicks.prev_stage != Some(stage) {
            self.spawn_clicks.prev_stage = Some(stage);
            self.spawn_clicks.clear();
        }
        if stage == core::sim::SpawnStage::Accepted {
            self.spawn_clicks.clear();
            return;
        }
        let Some((pos, click)) = self.shell_pointer.take_frame() else {
            return;
        };
        if !click {
            return;
        }
        let size = self.renderer.as_ref().map_or([640.0, 480.0], |r| {
            let (w, h) = r.size();
            [w as f32, h as f32]
        });

        match stage {
            core::sim::SpawnStage::Side => {
                if let Some(row) = ui::spawn_draw::side_row_at(pos, size, 3) {
                    let double = self
                        .spawn_clicks
                        .check_double_click(SpawnTarget::Side(row), pos);
                    self.kbm
                        .set_pick(core::sim::ScreenPick::Side(row.min(2) as u8));
                    if double {
                        self.kbm.tap_bits(core::sim::buttons::SPAWN_ACCEPT);
                        if let Some(sl) = self.sounds.as_mut() {
                            sl.play_2d(
                                self.audio.as_mut(),
                                ui::shell_feel::MenuSound::Accept.name(),
                            );
                        }
                    } else if let Some(sl) = self.sounds.as_mut() {
                        sl.play_2d(
                            self.audio.as_mut(),
                            ui::shell_feel::MenuSound::Select.name(),
                        );
                    }
                }
            }
            core::sim::SpawnStage::Map => {
                if let (Some(minimap), Some(scale)) = (&walk.minimap, map_scale) {
                    let map = minimap.spawn_model(walk, slot);
                    let posts = walk.sim.player_spawn_posts(team);
                    if let Some(post) =
                        ui::hud_map::hit_spawn_post(&map, scale, &posts, pos, size, 28.0)
                    {
                        let double = self
                            .spawn_clicks
                            .check_double_click(SpawnTarget::Post(post), pos);
                        self.kbm.set_pick(core::sim::ScreenPick::post(post));
                        if double {
                            self.kbm.tap_bits(core::sim::buttons::SPAWN_ACCEPT);
                            if let Some(sl) = self.sounds.as_mut() {
                                sl.play_2d(
                                    self.audio.as_mut(),
                                    ui::shell_feel::MenuSound::Accept.name(),
                                );
                            }
                        } else if let Some(sl) = self.sounds.as_mut() {
                            sl.play_2d(
                                self.audio.as_mut(),
                                ui::shell_feel::MenuSound::Select.name(),
                            );
                        }
                    }
                }
            }
            core::sim::SpawnStage::Class | core::sim::SpawnStage::Alive => {}
            core::sim::SpawnStage::Accepted => {}
        }
    }

    /// Fly-cam: left stick moves (yaw-relative), right stick looks, triggers
    /// fly down/up, south button = speed boost.
    fn drive_fly_camera(&mut self, dt: f32) {
        let Some(g) = &mut self.gilrs else { return };
        let was = self.active_pad.id;
        let map = &self.profile.settings.controller_bindings;
        if input::pump_tracking(g, &mut self.active_pad) {
            self.pad_warning = input::log_pads_with(g, map, &self.active_pad);
            self.gameplay_stick.select(None);
            self.stick_range.reset();
        } else if self.active_pad.id != was {
            self.pad_warning = input::active_pad_warning(g, map, &self.active_pad);
        }
        let Some((_, pad)) = input::active_gamepad(g, &self.active_pad) else {
            return;
        };
        let dead = |v: f32| if v.abs() < 0.15 { 0.0 } else { v };
        let lx = dead(pad.value(gilrs::Axis::LeftStickX));
        let ly = dead(pad.value(gilrs::Axis::LeftStickY));
        let rx = dead(pad.value(gilrs::Axis::RightStickX));
        let ry = dead(pad.value(gilrs::Axis::RightStickY));
        let up = pad
            .button_data(gilrs::Button::RightTrigger2)
            .map_or(0.0, |b| b.value());
        let down = pad
            .button_data(gilrs::Button::LeftTrigger2)
            .map_or(0.0, |b| b.value());
        let boost = if pad.is_pressed(gilrs::Button::South) {
            4.0
        } else {
            1.0
        };

        const LOOK_SPEED: f32 = 2.2;
        const MOVE_SPEED: f32 = 60.0;
        self.cam.yaw -= rx * LOOK_SPEED * dt;
        self.cam.pitch = (self.cam.pitch + ry * LOOK_SPEED * dt).clamp(-1.55, 1.55);
        let fwd = self.cam.forward();
        let right = self.cam.right();
        self.cam.position +=
            (fwd * ly + right * lx + glam::Vec3::Y * (up - down)) * (MOVE_SPEED * boost * dt);
    }

    /// Back to the shell's screens (the Instant Action screen keeps its
    /// playlist and options in `FrontShell`).
    fn init_shell(&mut self) {
        self.screen = Screen::Shell;
        self.sync_grab();
    }

    /// LAUNCH: the playlist in order, the first row now (§1.4, §7).
    fn start_playlist(&mut self, rows: Vec<ui::shell::Launch>, session: ui::shell::SessionOptions) {
        self.session = session;
        self.playlist = rows.into();
        self.next_round();
    }

    /// The playlist's next row to the loading screen; `false` when none is
    /// left.
    fn next_round(&mut self) -> bool {
        while let Some(row) = self.playlist.pop_front() {
            if self.begin_loading(row) {
                return true;
            }
        }
        false
    }

    /// Shell → loading screen (BF1 sequence + beep), then the battle of
    /// `row`; `false` when its map is not baked.
    fn begin_loading(&mut self, row: ui::shell::Launch) -> bool {
        // The shell music stops with the shell.
        if let Some(f) = self.front.as_mut() {
            f.stop_music();
        }
        let maps = game::assets::discover_maps(&self.assets_dir);
        let Some((name, _, dir)) = maps.into_iter().find(|(k, ..)| *k == row.key) else {
            eprintln!("playlist: {} is not baked; skipped", row.key);
            return false;
        };
        let mode = row.mode.subst().to_owned();
        // The measured CPU parse overlaps playback; upload/setup stay here.
        let worker = match loading::prepare(dir, mode.clone()) {
            Ok(worker) => worker,
            Err(e) => {
                eprintln!("loading worker: {e:#}");
                return false;
            }
        };
        let art_started = std::time::Instant::now();
        let mut textures = Vec::new();
        if let (Some(overlay), Some(ls)) = (
            &self.overlay,
            loading::presentation(&self.assets_dir, &row.key),
        ) {
            for (tex_name, t) in ls.sequence {
                let img = ui::egui::ColorImage::from_rgba_unmultiplied(
                    [t.width as usize, t.height as usize],
                    &t.rgba,
                );
                textures.push(overlay.ctx().load_texture(
                    tex_name,
                    img,
                    ui::egui::TextureOptions::LINEAR,
                ));
            }
        }
        println!(
            "loading: presentation_assets_ms={:.3}",
            art_started.elapsed().as_secs_f64() * 1000.0
        );
        self.mode = mode;
        self.current = Some(row);
        // The launch edge was already drained by the shell. Seed held raw pad
        // codes too: a held accept (including an unbound button) is not new.
        self.seed_loading_pads();
        self.clear_loading_input();
        // BF1 loading beep (load_xtracking = the 440 Hz ui_load_lp at pitch
        // 2.1, baked by `bake beep`).
        let beep = self.audio.as_mut().and_then(|a| {
            a.play_loop(&self.assets_dir.join("audio/load_xtracking.ogg"), 1.0, -8.0)
                .ok()
        });
        self.loading = Some(Loading {
            map_name: name,
            started: std::time::Instant::now(),
            playback: Default::default(),
            textures,
            worker,
            ready: None,
            shown: false,
            beep,
        });
        self.screen = Screen::Loading;
        self.sync_grab();
        true
    }

    /// Poll CPU work and paint independently of the presentation clock. A skip
    /// keeps drawing until CPU data is ready. GPU upload and setup still block
    /// the event thread at the explicitly timed final boundary.
    fn redraw_loading(&mut self) -> Result<()> {
        self.pump_loading_pads();
        let Some(l) = self.loading.as_mut() else {
            self.init_shell();
            return Ok(());
        };
        if let Some(result) = l.worker.poll() {
            match result {
                Ok(prepared) => l.ready = Some(prepared),
                Err(e) => {
                    eprintln!("failed to prepare {}: {e:#}", l.map_name);
                    self.finish_loading();
                    self.loading_failed();
                    return Ok(());
                }
            }
        }
        let steps = l.textures.len().max(1);
        let elapsed = l.started.elapsed();
        let step = l.playback.step(elapsed, steps);
        let finish = l
            .playback
            .can_enter(elapsed, steps, l.shown, l.ready.is_some());
        let image = l.textures.get(step).map(|t| t.id());
        let caption = l.map_name.to_uppercase();
        let prompt = l.playback.prompt(elapsed);
        if let (Some(r), Some(overlay), Some(window)) =
            (&mut self.renderer, &mut self.overlay, &self.window)
        {
            let paint = overlay.begin(window, |ctx| {
                ui::loading::draw_loading_with_skip(ctx, image, &caption, step, steps, prompt);
            });
            if let Err(e) = r.render_with_overlay(
                &Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, view, size| overlay.paint(&paint, dev, q, enc, view, size)),
            ) {
                self.finish_loading();
                return Err(e.into());
            }
        }
        if finish {
            let mut l = self.finish_loading().expect("loading round");
            let prepared = l.ready.take().expect("ready CPU scene");
            println!(
                "loading: cpu_parse_ms={:.3} presentation_wall_ms={:.3}",
                prepared.cpu_ms,
                l.started.elapsed().as_secs_f64() * 1000.0
            );
            if let Err(e) = self.start_battle(l.map_name.clone(), prepared) {
                eprintln!("failed to start {}: {e:#}", l.map_name);
                self.loading_failed();
            }
        } else if let Some(l) = self.loading.as_mut() {
            l.shown = true;
        }
        Ok(())
    }

    fn finish_loading(&mut self) -> Option<Loading> {
        let mut l = self.loading.take()?;
        if let (Some(a), Some(b)) = (self.audio.as_mut(), l.beep.as_mut()) {
            a.stop(b);
        }
        self.loading_edges.quarantine();
        self.loading_pad_blocked = self.any_pad_button_down();
        self.clear_loading_input();
        Some(l)
    }

    fn loading_failed(&mut self) {
        if let Some(r) = self.renderer.as_mut() {
            r.clear_scene();
        }
        self.current = None;
        if !self.next_round() {
            self.init_shell();
        }
    }

    /// Build resources in launch order. Shell supplies its newly resolved setup;
    /// direct launch uses the setup already stored on the App.
    fn battle_resources(
        &mut self,
        renderer: &mut Renderer,
        scene: &game::assets::Scene,
        setup: Option<&BattleSetup>,
    ) -> Result<(WalkSim, MeshHandle)> {
        let setup = setup.unwrap_or(&self.setup);
        let mut walk = make_walk(scene, setup)?;
        let cube = upload_unit_cube(renderer)?;
        self.fx_quad = game::viewer::upload_billboard_quad(renderer).ok();
        self.fx_sprite = game::viewer::upload_fx_sprite(renderer).ok();
        let dir = self.assets_dir.clone();
        self.squad_icons = squad_icons(renderer, &dir);
        self.hud_view = load_hud_view(&dir, renderer);
        self.bolts = game::bolt_vis::BoltVis::new(&self.assets_dir, &scene.manifest.source_game);
        renderer.set_sky_seed(
            game::classdb::fnv1a_lowercase(&scene.manifest.map) ^ setup.seed.rotate_left(11),
        );
        if let Some(mission) = setup.mission.as_ref() {
            if let Err(error) = game::forest::configure(renderer, scene, Some(mission)) {
                eprintln!("forest: {error:#}");
            }
        }
        // `[ours]` The session's cheats, whatever the setup says (CHEAT2).
        self.cheats.install(&mut walk.sim);
        walk.blend = game::render_blend::Blend::from_env();
        self.foley = game::foley::Foley::load(&scene.dir, &walk);
        self.engines = game::engine_sound::EngineSounds::new(&scene.dir, &walk);
        self.ambience =
            game::ambience::Ambience::load(&scene.manifest, &scene.dir, &setup.classes_dir);
        if let Some(sl) = self.sounds.as_mut() {
            sl.for_walk(&scene.dir, &walk);
            preload_sounds(sl);
            sl.rate_mul = self.cheats.sfx_rate();
        }
        self.blackbox = start_blackbox(scene, &walk, &self.mode, setup, self.sounds.as_mut());
        self.matchlog = start_matchlog(scene, &walk, &self.mode, setup);
        self.feed = game::event_feed::EventFeed::for_walk(
            &self.assets_dir,
            &scene.manifest.source_game,
            &walk,
        );
        self.fx = game::fx::Fx::for_walk(
            &self.assets_dir,
            &scene.dir,
            &scene.manifest.source_game,
            &walk,
        );
        self.fx.set_water(&scene.manifest);
        self.fx.set_scene_effects(scene);
        self.fx.preload(renderer);
        self.weather = game::weather::Weather::load(
            &scene.manifest,
            &self.assets_dir,
            &scene.dir,
            WEATHER_SEED,
        );
        self.soldier_vis = Some(game::soldier_vis::SoldierVisuals::for_walk(
            renderer,
            &self.assets_dir,
            scene,
            &walk,
        ));
        self.vehicle_vis = game::viewer::VehicleVisuals::load(
            renderer,
            scene,
            &self.assets_dir,
            &walk.vehicle_geometry,
        );
        let lit_chunks = self.vehicle_vis.load_chunks(
            renderer,
            scene,
            &self.assets_dir,
            &self.fx.chunk_models(),
        );
        self.fx.set_lit_chunks(lit_chunks);
        self.deka_vis =
            game::viewer::DroidekaVisuals::load(renderer, scene, &self.assets_dir, &walk);
        // What the battle can show later, behind the loading (card HITCH).
        self.vehicle_vis.preload(renderer, &walk.sim);
        self.bolts.preload(renderer, &walk);
        walk.sim.assist_intensity = self.profile.settings.assist_intensity;
        walk.sim.pad_look_sensitivity =
            walk.pad_look_for_battle(self.profile.settings.pad_look_sensitivity);
        walk.fov_scale = self.profile.settings.fov_scale;
        if let Some(m) = walk.music.as_mut() {
            m.volume = game::music::volume_from_env(self.profile.settings.music_volume);
        }
        // THE MAINTAINER'S RULE: the aim assist is the PLAYER'S switch — no difficulty
        // tier reads it (the PS2 BF2's Elite clears it, RE §9.3; not ours).
        walk.sim.assist_armed = self.profile.settings.aim_assist;
        walk.sim.team_handicap_on = self.profile.settings.team_handicap_on;
        Ok((walk, cube))
    }

    /// Load the map + build the battle with the shell's settings.
    fn start_battle(&mut self, name: String, prepared: loading::Prepared) -> Result<()> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no renderer"))?;
        let upload_started = std::time::Instant::now();
        let scene = prepared.scene;
        let gpu = super::battle::upload_battle_scene(renderer, &scene, &prepared.load)?;
        let upload_ms = upload_started.elapsed().as_secs_f64() * 1000.0;
        let setup_started = std::time::Instant::now();
        self.sounds = game::sounds::SoundLib::load(&scene.dir);
        self.music_voice = None;
        // The mission whose pair was picked (Republic against CIS runs the
        // map's Clone Wars script), the player's army on its own team there;
        // a cross-era pair runs the map's default mission (card ERAM).
        // The playlist row's armies (its era, or `[ours]` a cross-era pair).
        let row_armies = self.current.as_ref().map_or_else(
            || ["all".to_owned(), "imp".to_owned()],
            |r| r.armies.clone(),
        );
        let armies = [row_armies[0].as_str(), row_armies[1].as_str()];
        let pick = game::mission::pick(&self.missions_dir, &name, &self.mode, armies);
        let player_team = pick.as_ref().map_or(0, |p| p.player_team);
        let mut teams = armies.map(str::to_string);
        if player_team == 1 {
            teams.swap(0, 1);
        }
        let mission_file = pick.as_ref().and_then(|p| super::file_name(&p.path));
        match &pick {
            Some(p) => println!(
                "mission: {} for {} against {} ({}), the player on team {}",
                p.path.display(),
                armies[0],
                armies[1],
                if p.own {
                    "its own pair"
                } else {
                    "a cross-era pair"
                },
                player_team + 1
            ),
            None => println!("mission: none for {name} {}", self.mode),
        }
        let mut mission = pick.map(|p| p.mission);
        anyhow::ensure!(
            self.mode != "eli" || mission.is_some(),
            "Hero Assault requires a mission"
        );
        anyhow::ensure!(
            mission.as_ref().is_none_or(|m| m.mode == self.mode),
            "mission mode differs from {}",
            self.mode
        );
        let assault = mission.as_ref().is_some_and(|m| m.is_hero_assault());
        if let Some(m) = mission.as_mut().filter(|_| assault) {
            m.ai_alive_cap = self.session.ass_numbots.max(0) as u32;
        }
        let space = self.mode == "ass";
        let setup = BattleSetup {
            seed: 42u32.wrapping_add(self.profile.career.rounds),
            // The shell round takes the PROFILE's tier (no CLI flag here);
            // Session Options' AI Difficulty writes it.
            difficulty: self.profile.settings.difficulty,
            spawn: (0.0, 0.0),
            spawn_set: false,
            targets: false,
            // Map-relative team size, including the human.
            ai: None,
            // Explicit CLI cap is separate from the percentage setting.
            ai_cap: space.then_some(self.session.ass_numbots.max(0) as u32),
            team_size_pct: (!assault && !space).then_some(self.session.con_numbots as u32),
            // Calibrated map tickets, scaled by the shell percentage.
            tickets: 250,
            tickets_set: false,
            ticket_pct: if assault || space {
                100
            } else {
                self.session.con_mult.max(10) as u32
            },
            time_limit: if assault || space {
                0
            } else {
                self.session.con_timer.max(0) as u32
            },
            space_score_limit: self.session.ass_score.max(1) as u32,
            cheats: core::sim::Cheats::default(),
            teams: Some(teams.clone()),
            classes_dir: self.assets_dir.join("classes"),
            mission,
            mission_file,
            player_team,
            heroes: game::heroes::rules(&self.session),
            dlc_heroes: self.session.dlc_heroes,
            score_limit: self.session.ass_score.max(0) as u32,
        };
        // Release the renderer field borrow while installing the other App resources.
        let mut renderer = self.renderer.take().expect("loaded renderer");
        let resources = self.battle_resources(&mut renderer, &scene, Some(&setup));
        self.renderer = Some(renderer);
        let (mut walk, cube) = resources?;
        super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
        self.profile.settings.last_map = Some(name);
        self.profile.settings.last_mode = Some(self.mode.clone());
        self.profile.settings.last_teams = Some(teams);
        if let Err(e) = self.profile.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
        self.walk = Some((walk, cube));
        self.gpu = Some(gpu);
        self.setup = setup;
        self.paused = false;
        if let Some(r) = self.renderer.as_mut() {
            r.atmosphere_time = Some(0.0);
        }
        self.kbm.release_keys();
        self.resume_gate = core::sim::HeldEat::default();
        self.outcome_reported = false;
        self.round_recorded = false;
        self.screen = Screen::Battle;
        self.sync_grab();
        self.start_frame_record();
        println!(
            "loading: upload_ms={upload_ms:.3} setup_ms={:.3}",
            setup_started.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    }

    /// A battle's first frame is next: the frame record starts over and the
    /// black box writes it into its snapshots (card HITCH).
    fn start_frame_record(&mut self) {
        if let Some(p) = self.frame_profile.as_mut() {
            p.reset();
            if let Some(bb) = self.blackbox.as_mut() {
                bb.frame_record(p.record());
            }
        }
    }

    /// The black box's `exit` snapshot, written before the battle goes, and
    /// the frame record's summary line.
    fn blackbox_exit(&mut self) {
        if let Some(h) = self.frame_profile.as_ref().and_then(|p| p.hitches()) {
            println!("frame record: {}", h.summary());
        }
        if let (Some(bb), Some((walk, _))) = (self.blackbox.as_mut(), self.walk.as_ref()) {
            bb.write("exit", Some(walk), true);
        }
        self.blackbox = None;
        // Card DPRX: a battle left before its outcome still writes its log.
        if let (Some(m), Some((walk, _))) = (self.matchlog.as_mut(), self.walk.as_ref()) {
            if let Some(p) = m.write(&walk.sim, "quit") {
                println!("matchlog: wrote {}", p.display());
            }
        }
        self.matchlog = None;
    }

    fn game_audio_paused(&self) -> bool {
        self.in_battle() && self.paused
    }

    /// Pause or resume the game's mixer bus to match [`App::game_audio_paused`].
    /// Run every frame, so every way out of the pause (Resume, a respawn,
    /// leaving the battle) brings the sound back.
    fn sync_pause_audio(&mut self) {
        let paused = self.game_audio_paused();
        if let Some(a) = self.audio.as_mut() {
            a.set_game_paused(paused);
        }
    }

    /// Restart Mission: the battle in play again from its loading screen, the
    /// rest of the playlist after it and the session's cheats kept. The left
    /// round is recorded as a Quit's is.
    fn restart_round(&mut self) {
        let Some(row) = self.current.clone() else {
            return;
        };
        self.leave_battle();
        requeue_front(&mut self.playlist, row);
        if !self.next_round() {
            self.playlist.clear();
            self.cheats.end();
        }
    }

    /// Battle → shell (records the round).
    fn leave_battle(&mut self) {
        self.blackbox_exit();
        if let Some((walk, _)) = self.walk.as_mut() {
            if !self.round_recorded {
                self.round_recorded = true;
                record_round(&mut self.profile, walk, self.feed.battle_stats());
            }
        }
        self.walk = None;
        self.music_voice = None;
        self.gpu = None;
        // The map's sky and look stay in the renderer otherwise, and the
        // shell's frames draw them behind the movie.
        if let Some(r) = self.renderer.as_mut() {
            r.clear_scene();
        }
        self.hud_view = None;
        // Left from the pause menu: drop the held battle sounds rather than
        // resume them over the shell. (Space never pauses the bus.)
        if let Some(a) = self.audio.as_mut().filter(|a| a.game_paused()) {
            a.clear_game_bus();
        }
        self.paused = false;
        self.kbm.release_keys();
        self.resume_gate = core::sim::HeldEat::default();
        self.release_mouse();
        if let Some(sl) = self.sounds.as_mut() {
            sl.stop_loops();
        }
        self.sync_pause_audio();
        self.init_shell();
    }

    /// The control prompts' icons this frame.
    fn current_icons(&mut self) -> ui::input_icons::Icons {
        self.icon_device.sync(self.gilrs.as_ref(), &self.active_pad);
        let settings = &self.profile.settings;
        game::input_icons::icons(
            self.icon_device.set(settings.input_icons),
            &settings.controller_bindings,
        )
    }

    /// Raw menu buttons this frame as edge bits: 1 up, 2 down, 4 left,
    /// 8 right, 16 confirm (Cross), 32 back (Circle), 64 Start, 128 L1,
    /// 256 R1, 512 Square, 1024 / 2048 a value step left / right.
    fn menu_edges(&mut self) -> u32 {
        let mut now = 0u32;
        if let Some(g) = &mut self.gilrs {
            let was = self.active_pad.id;
            let map = &self.profile.settings.controller_bindings;
            if input::pump_tracking(g, &mut self.active_pad) {
                self.pad_warning = input::log_pads_with(g, map, &self.active_pad);
                self.gameplay_stick.select(None);
                self.stick_range.reset();
                if let Some(front) = self.front.as_mut() {
                    front.reset_control_device();
                }
            } else if self.active_pad.id != was {
                self.pad_warning = input::active_pad_warning(g, map, &self.active_pad);
            }
            if self.loading_pad_blocked {
                loading::suppress_pad_until_release(
                    &mut self.loading_pad_blocked,
                    loading::pad_buttons_down(g),
                );
            }
            if let Some((_, pad)) =
                input::active_gamepad(g, &self.active_pad).filter(|_| !self.loading_pad_blocked)
            {
                use gilrs::Button;
                let bits = [
                    (Button::DPadUp, 1),
                    (Button::DPadDown, 2),
                    (Button::DPadLeft, 4),
                    (Button::DPadRight, 8),
                    (Button::South, 16),
                    (Button::East, 32),
                    (Button::Start, 64),
                    (Button::LeftTrigger, game::kbm::MENU_L1),
                    (Button::RightTrigger, game::kbm::MENU_R1),
                    (Button::West, game::kbm::MENU_SQUARE),
                ];
                for (b, bit) in bits {
                    if pad.is_pressed(b) {
                        now |= bit;
                    }
                }
                // The shell's Back is Triangle, as its help line shows
                // (re-bf2-shell §3.4), and the stats pages' (issue #50);
                // Circle too, as everywhere else.
                let stats = self
                    .walk
                    .as_ref()
                    .is_some_and(|(w, _)| w.round_end.as_ref().is_some_and(|r| r.stats.is_some()));
                if pad.is_pressed(Button::North) {
                    // Held North is remembered everywhere; it backs out of
                    // the ground pause menu only on its own fresh press
                    // (`kbm::menu_edges`).
                    now |= game::kbm::MENU_NORTH;
                    if matches!(self.screen, Screen::Shell) || stats {
                        now |= 32;
                    }
                }
                // The left stick presses the same arms as the D-pad.
                let stick = input::stick_arms(
                    pad.value(gilrs::Axis::LeftStickX),
                    pad.value(gilrs::Axis::LeftStickY),
                    game::kbm::stick_latch(self.prev_menu_buttons),
                );
                now |= stick;
                // The stick's Left / Right are the pause menu's value steps.
                if stick & 4 != 0 {
                    now |= game::kbm::MENU_ADJUST_LEFT;
                }
                if stick & 8 != 0 {
                    now |= game::kbm::MENU_ADJUST_RIGHT;
                }
            }
        }
        let shell = matches!(self.screen, Screen::Shell);
        now |= game::kbm::menu_bits(self.keys_held, shell);
        let tapped = game::kbm::menu_bits(std::mem::take(&mut self.keys_tapped), shell);
        let north_backs = self.paused;
        let edges = game::kbm::menu_edges(now, self.prev_menu_buttons, tapped, north_backs);
        self.prev_menu_buttons = now;
        edges
    }

    /// Sample the active gamepad into a sim PadState (walk mode); `look` is
    /// the right stick's look sensitivity. `learned`: the soldier's stick is
    /// scaled by the pad's learned range (on foot in a ground mode);
    /// otherwise the fixed [`input::STICK_FULL`] keeps vehicle, flyer and
    /// space input as it was.
    fn sample_pad(&mut self, look: f32, learned: bool) -> PadState {
        let Some(g) = &mut self.gilrs else {
            self.gameplay_stick.select(None);
            self.stick_range.reset();
            return PadState::default();
        };
        let was = self.active_pad.id;
        let map = &self.profile.settings.controller_bindings;
        if input::pump_tracking(g, &mut self.active_pad) {
            self.pad_warning = input::log_pads_with(g, map, &self.active_pad);
            // A disconnect/reconnect can reuse the same gilrs ID between frames.
            self.gameplay_stick.select(None);
            self.stick_range.reset();
        } else if self.active_pad.id != was {
            self.pad_warning = input::active_pad_warning(g, map, &self.active_pad);
        }
        if self.loading_pad_blocked
            && loading::suppress_pad_until_release(
                &mut self.loading_pad_blocked,
                loading::pad_buttons_down(g),
            )
        {
            return PadState::default();
        }
        match input::active_gamepad(g, &self.active_pad) {
            Some((id, pad)) => {
                if self.gameplay_stick.controller != Some(id) {
                    self.stick_range.reset();
                }
                self.gameplay_stick.select(Some(id));
                let raw = [
                    pad.value(gilrs::Axis::LeftStickX),
                    pad.value(gilrs::Axis::LeftStickY),
                ];
                self.stick_range.feed(raw[0], raw[1]);
                let full = stick_full(&self.stick_range, learned);
                self.stick_sample = (raw, full);
                let (state, arms) = input::pad_from_gamepad_with_arms(
                    &pad,
                    &self.profile.settings.controller_bindings,
                    self.gameplay_stick.arms,
                    look,
                    full,
                );
                self.gameplay_stick.arms = arms;
                state
            }
            None => {
                self.gameplay_stick.select(None);
                self.stick_range.reset();
                self.stick_sample = ([0.0; 2], [0.0; 2]);
                PadState::default()
            }
        }
    }
}

/// Shell entry: window + shell screen; battles start from the menu.
pub(super) fn run_shell(assets_dir: PathBuf, missions_dir: PathBuf) -> Result<()> {
    let event_loop = winit::event_loop::EventLoop::new()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let profile = game::profile::Profile::load_or_default();
    let (gilrs, pad_warning) = open_pads(&profile.settings.controller_bindings);
    let audio = audio::Audio::new();
    let setup = BattleSetup {
        seed: 42,
        difficulty: core::skill::Difficulty::default(),
        spawn: (0.0, 0.0),
        spawn_set: false,
        targets: false,
        ai: None,
        ai_cap: None,
        team_size_pct: None,
        tickets: 250,
        tickets_set: false,
        ticket_pct: 100,
        time_limit: 0,
        space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
        cheats: core::sim::Cheats::default(),
        teams: None,
        classes_dir: assets_dir.join("classes"),
        mission: None,
        mission_file: None,
        player_team: 0,
        heroes: Some(core::sim::heroes::HeroRules::default()),
        dlc_heroes: false,
        score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
    };
    let autopilot = game::dev::on() && std::env::var("OPENBF_AUTOPILOT").is_ok_and(|v| v == "1");
    let quit_at = quit_at();
    let frame_profile = game::frame_profile::FrameProfile::from_env();
    let mut app = App::new(
        assets_dir,
        missions_dir,
        setup,
        gilrs,
        pad_warning,
        audio,
        profile,
    );
    app.frame_profile = frame_profile;
    app.autopilot = autopilot;
    app.quit_at = quit_at;
    run_app(event_loop, &mut app)
}

#[path = "windowed/tests.rs"]
#[cfg(test)]
mod tests;
