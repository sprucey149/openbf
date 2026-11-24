//! Native window lifecycle and event dispatch for the application.

use super::spawn_feedback::draw_snapshot_notice;
use super::tick_hook::{TickHook, TickState};
use super::{
    apply_pause_action, autopilot_pad, hud_model_for_view, hud_title, lan, mouse, note_sounds,
    rain_splashes, record_round, spawn_model, start_take, stats_pages, tracked_velocity,
    walk_draws, window_attributes, window_size_env, world_objective_draws, App, Arc, DrawCmd,
    PadState, Screen, CLEAR,
};
use core::fmath::FloatExt;

// The six low menu-edge bits (`game::kbm::menu_key` / `menu_bits`): D-pad and
// arrows, Cross, Back and Start. `game::kbm` names the higher shoulder / face
// bits (MENU_L1, MENU_SQUARE, MENU_DETAILS, ...) but not these, so event
// dispatch names them here instead of repeating magic numbers.
const MENU_UP: u32 = 1;
const MENU_DOWN: u32 = 2;
const MENU_CONFIRM: u32 = 16;
const MENU_BACK: u32 = 32;
const MENU_START: u32 = 64;

/// The in-battle scoreboard of `walk` (LAN §5b, `game::round_stats::scoreboard`):
/// a LAN round names its humans by their session names and gives them a
/// ping; single player has no Ping column.
fn scoreboard_model(
    walk: &game::walk::WalkSim,
    feed: &game::event_feed::EventFeed,
    titles: [String; 2],
    net: Option<&lan::NetplaySession>,
) -> ui::scoreboard::Scoreboard {
    use ui::scoreboard::Ping;
    let sim = &walk.sim;
    let roster = net.map(|n| n.roster()).unwrap_or_default();
    let unit = |i: usize| {
        sim.units[i]
            .spec
            .player_slot
            .filter(|_| net.is_some())
            .and_then(|s| roster.iter().find(|p| p.slot == s))
            .map_or_else(|| feed.unit_name(sim, sim.handle(i)), |p| p.name.clone())
    };
    let ping = |slot: u8| match roster.iter().find(|p| p.slot == slot) {
        Some(p) => p.ping_ms.map_or(Ping::Host, Ping::Ms),
        None => Ping::None,
    };
    let ping: Option<&dyn Fn(u8) -> Ping> = net.map(|_| &ping as &dyn Fn(u8) -> Ping);
    game::round_stats::scoreboard(sim, feed.round(), walk.player, &unit, titles, ping)
}

/// The drawn view back after the draw lists: a LAN peer's prediction and
/// blend, or the single-player blend (issue #168).
fn draw_out(netplay: &mut Option<lan::NetplaySession>, walk: &mut game::walk::WalkSim) {
    match netplay.as_mut() {
        Some(n) => n.view.draw_out(walk),
        None => walk.blend_out(),
    }
}

impl App {
    /// One console line per change of the window's size, display scale or
    /// frame size (issue #168: both Windows guests of the first VPN
    /// playtest saw the menus off their art; the next report says what
    /// their windows were). Called only once the renderer has its size
    /// (opened, resized, a frame), never from a scale change itself, whose
    /// new inner size would print against the frame before its resize.
    fn note_window_size(&mut self, why: &str) {
        let (Some(w), Some(r)) = (&self.window, &self.renderer) else {
            return;
        };
        let inner = w.inner_size();
        let now = (inner.width, inner.height, w.scale_factor(), r.size());
        if self.window_note != Some(now) {
            self.window_note = Some(now);
            println!(
                "window: {why}: inner {}x{}, scale {:.2}, frame {}x{}",
                now.0, now.1, now.2, now.3 .0, now.3 .1
            );
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() || self.closing {
            return;
        }
        // Issue #172: the saved display mode, unless a size is forced.
        self.display.exclusive = super::display::exclusive_supported(event_loop);
        self.display.forced = window_size_env().is_some();
        let mode = self.shown_window_mode();
        let fullscreen = super::display::fullscreen_for(
            mode,
            event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next()),
            self.display.exclusive,
        );
        let window = Arc::new(
            event_loop
                .create_window(window_attributes(window_size_env(), fullscreen))
                .expect("create window"),
        );
        self.display_opened(mode);
        self.idle = Some(super::idle::IdleInhibit::new(&window));
        let mut renderer = match self.renderer_for_window(window.clone()) {
            Ok(renderer) => {
                crate::extensions_host::clear_startup_error(&self.data_dir);
                renderer
            }
            Err(error) => {
                self.failure = Some(error);
                self.window = None;
                event_loop.exit();
                return;
            }
        };
        self.overlay = Some(ui::overlay::EguiOverlay::new(
            &window,
            renderer.device(),
            renderer.surface_format(),
        ));
        // Every egui text in the menu face (`ui::typography`).
        if let Some(overlay) = &self.overlay {
            let face = game::hud_feed::menu_face(&self.assets_dir);
            ui::typography::face::install(overlay.ctx(), &face);
        }
        window.set_cursor(winit::window::CursorIcon::Default);
        if let Some(overlay) = &mut self.overlay {
            overlay.set_window_cursor(mouse::menu_cursor(event_loop, window.scale_factor()));
            if let Some((rgba, side, hot)) = mouse::software_cursor(window.scale_factor()) {
                overlay.set_software_cursor(rgba, side, hot);
            }
        }
        self.focused = window.has_focus();
        let Some(map) = self.map.clone() else {
            // Shell start: no map yet; a LAN launch waits for its round.
            if self.netplay.is_some() {
                self.screen = Screen::NetWait;
            } else {
                self.init_shell();
            }
            self.window = Some(window);
            self.renderer = Some(renderer);
            self.note_window_size("opened");
            self.last = std::time::Instant::now();
            return;
        };
        let preload = game::assets::prefetch::load_key(&map, &self.mode);
        let warm = std::thread::Builder::new()
            .name("loading-prefetch".into())
            .spawn({
                let key = preload.clone();
                move || game::assets::prefetch::warm(&key)
            });
        let sound_banks = self
            .setup
            .mission
            .as_ref()
            .map(|m| m.sound.clone())
            .unwrap_or_default();
        self.sounds = game::sounds::SoundLib::load_with_fallback(&map, &sound_banks);
        self.sync_audio_levels();
        self.music_voice = None;
        if let Some(sl) = &self.sounds {
            println!(
                "sound library loaded ({})",
                if sl.has("all_weap_inf_rifle_fire") || sl.has("rep_weap_inf_rifle_fire") {
                    "weapon set present"
                } else {
                    "weapon set partial"
                }
            );
        }
        let layers = self
            .setup
            .mission
            .as_ref()
            .map(|m| m.layers.clone())
            .unwrap_or_default();
        let kills = self
            .setup
            .mission
            .as_ref()
            .map(|m| m.scene_kills().to_vec())
            .unwrap_or_default();
        if let Ok(n) = warm.map(|h| h.join().unwrap_or(0)) {
            println!("loading: prefetch key={preload} decoded={n}");
        }
        let _preload = game::assets::prefetch::record(&preload);
        match super::super::battle::load_and_upload_with_layers(
            &mut renderer,
            &map,
            &self.mode,
            &layers,
            &kills,
        ) {
            Ok((gpu, scene)) => {
                if self.walk_requested {
                    let (walk, cube) = match self.battle_resources(&mut renderer, &scene, None) {
                        Ok(resources) => resources,
                        Err(error) => {
                            eprintln!("failed to start battle: {error:#}");
                            self.request_close(super::CloseReason::Error);
                            return;
                        }
                    };
                    if let Err(e) = self.profile.save() {
                        eprintln!("profile autosave failed: {e:#}");
                    }
                    println!(
                        "profile '{}': {} rounds, {} kills / {} deaths",
                        self.profile.name,
                        self.profile.career.rounds,
                        self.profile.career.kills,
                        self.profile.career.deaths
                    );
                    self.walk = Some((walk, cube));
                    self.start_frame_record();
                }
                self.gpu = Some(gpu);
            }
            Err(e) => {
                eprintln!("failed to load map: {e:#}");
                self.request_close(super::CloseReason::Error);
                return;
            }
        }
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.note_window_size("opened");
        self.last = std::time::Instant::now();
    }

    /// A close asked for during the events (a Quit, a failure, the
    /// console's close) runs here, once the events are handled.
    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if super::close::console_close_requested() {
            self.request_close(super::CloseReason::Console);
        }
        // A loading whose window gets no redraws still advances.
        if !self.closing && self.close_pending.is_none() {
            if let Err(error) = self.loading_unpainted() {
                eprintln!("render: {error:#}");
                self.request_close(super::CloseReason::Error);
            }
        }
        if let Some(reason) = self.close_pending.take() {
            self.close_now(event_loop, reason);
        }
    }

    fn exiting(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.idle = None;
        self.release_mouse();
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            self.last_input.raw_motion(delta.0, delta.1);
            self.kbm.motion(delta.0, delta.1);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;
        if self.closing {
            return;
        }
        // First, before any screen's own key handling can take the event.
        if matches!(event, WindowEvent::CloseRequested) {
            self.close_now(event_loop, super::CloseReason::Window);
            return;
        }
        if game::input_icons::is_keyboard_or_mouse_use(&event) {
            self.icon_device.keyboard_or_mouse();
        }
        self.last_input.window_event(&event);
        // Issue #172: loading owns every other key.
        if matches!(self.screen, Screen::Loading) && self.display_hotkey(&event) {
            return;
        }
        if self.loading_window_event(&event) {
            return;
        }
        if self.profile_keyboard(&event) {
            return;
        }
        self.sync_binding_context();
        if self.capture_keyboard(&event) {
            if let WindowEvent::KeyboardInput { event, .. } = &event {
                if !event.state.is_pressed() {
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        self.keys_held &= !game::kbm::menu_key(code);
                        // Through the chord rule, so a Command, waiting or
                        // swallowed key released here leaves no state behind.
                        self.kbm.key_released(code, std::time::Instant::now());
                    }
                }
            }
            return;
        }
        if self.display_hotkey(&event) {
            return;
        }
        if let (Some(o), Some(w)) = (&mut self.overlay, &self.window) {
            let _ = o.on_window_event(w, &event);
        }
        if let WindowEvent::ScaleFactorChanged { scale_factor, .. } = &event {
            if let Some(overlay) = &mut self.overlay {
                overlay.set_window_cursor(mouse::menu_cursor(event_loop, *scale_factor));
                if let Some((rgba, side, hot)) = mouse::software_cursor(*scale_factor) {
                    overlay.set_software_cursor(rgba, side, hot);
                }
            }
        }
        self.mouse_window_event(&event);
        match event {
            WindowEvent::Focused(false) => {
                self.rumble_effect = None;
                self.keys_held = 0;
                self.keys_tapped = 0;
                self.prev_menu_buttons = 0;
            }
            // F9: the playtest black box writes its snapshot (dev mode).
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F9),
                        state: winit::event::ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => self.snapshot_requested = self.blackbox.is_some(),
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F10),
                        state: winit::event::ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } if self.display_ext.is_some() => {
                if let Some(ext) = self.display_ext.as_mut() {
                    ext.recenter();
                }
            }
            // F7: the LAN net meter (issue #168; also Options → Network meter).
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F7),
                        state: winit::event::ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                let s = &mut self.profile.settings;
                s.network_meter = !s.network_meter;
                if let Err(e) = self.profile.save() {
                    eprintln!("profile autosave failed: {e:#}");
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(code),
                        state,
                        repeat,
                        ..
                    },
                ..
            } => {
                let bit = game::kbm::menu_key(code);
                if state == winit::event::ElementState::Pressed {
                    self.keys_held |= bit;
                    // The system's key repeat steps the arrows, never a
                    // confirm or a back; the shell repeats a held arrow at
                    // the PS2's rate itself (`ui::shell_feel::HoldRepeat`).
                    let shell = matches!(self.screen, Screen::Shell | Screen::Lobby);
                    if !repeat || (bit < 16 && !shell) {
                        self.keys_tapped |= bit;
                    }
                } else {
                    self.keys_held &= !bit;
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
                self.display_resized();
                self.note_window_size("resized");
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.begin_display_ext_frame() {
                    self.failure = Some(error);
                    event_loop.exit();
                    return;
                }
                // A scale change without a resize shows at its next frame.
                self.note_window_size("scale changed");
                let elapsed = self.last.elapsed().as_secs_f32();
                let dt = elapsed.fmin(0.1);
                self.last = std::time::Instant::now();
                if matches!(self.screen, Screen::Loading) {
                    self.frame_seconds = 0.0;
                } else if self.frame_seconds == 0.0 {
                    self.frame_seconds = elapsed;
                } else {
                    self.frame_seconds +=
                        (elapsed - self.frame_seconds) * (elapsed * 3.0).fmin(1.0);
                }
                self.sync_audio_levels();
                self.queue_battle_movies();
                // The renderer's animation clock, for UV-scrolling materials
                // (a map's water). Wrapped so `uv + velocity * time` keeps
                // its f32 precision in a long session — see Renderer::time.
                let frozen = self.frozen();
                if let Some(r) = self.renderer.as_mut() {
                    (r.time, r.water_time) = render::advance_clocks(r.time, r.water_time, dt);
                    if matches!(self.screen, Screen::Battle) && !frozen {
                        let atmo = r.atmosphere_time.unwrap_or(0.0) + f64::from(dt);
                        r.atmosphere_time = Some(atmo);
                    }
                }

                if let Some(mode) = self.display.requested.take() {
                    self.set_window_mode(mode);
                }
                self.sync_grab();
                self.sync_pause_audio();
                let awake = super::idle::wants_idle_inhibit(self.in_battle(), self.paused);
                if let Some(idle) = self.idle.as_mut() {
                    idle.set(awake);
                }
                if matches!(self.screen, Screen::Shell) {
                    self.redraw_shell(dt);
                    self.finish_redraw(event_loop);
                    return;
                }
                if matches!(self.screen, Screen::Lobby) {
                    if let Err(error) = self.redraw_lobby(dt) {
                        eprintln!("render: {error:#}");
                        self.request_close(super::CloseReason::Error);
                        return;
                    }
                    self.finish_redraw(event_loop);
                    return;
                }
                if matches!(self.screen, Screen::NetWait) {
                    if let Err(error) = self.redraw_net_wait() {
                        eprintln!("render: {error:#}");
                        self.request_close(super::CloseReason::Error);
                        return;
                    }
                    self.finish_redraw(event_loop);
                    return;
                }
                if matches!(self.screen, Screen::Loading) {
                    if let Err(error) = self.redraw_loading() {
                        eprintln!("render: {error:#}");
                        self.request_close(super::CloseReason::Error);
                        return;
                    }
                    self.finish_redraw(event_loop);
                    return;
                }
                if self.battle_movie_pending() {
                    if let Err(error) = self.redraw_battle_movie(dt) {
                        eprintln!("render: {error:#}");
                        self.request_close(super::CloseReason::Error);
                    } else {
                        self.finish_redraw(event_loop);
                    }
                    return;
                }

                // Issue #168: the LAN session's events before the battle
                // frame borrows the walk (a new round leaves for loading).
                if self.netplay.is_some() {
                    self.net_idle();
                    if !matches!(self.screen, Screen::Battle) {
                        self.finish_redraw(event_loop);
                        return;
                    }
                    if self.netplay.as_ref().is_some_and(|n| n.ended_done()) {
                        self.quit_clicked = true;
                    }
                }
                if let Some(p) = self.frame_profile.as_mut() {
                    p.begin();
                }
                let mut extra: Vec<DrawCmd> = Vec::new();
                let mut skinned: Vec<render::SkinnedDrawCmd> = Vec::new();
                let mut see = game::camera_collide::SeeThrough::default();
                // The loaded map's own clear colour (its fog, where it authors
                // fog); the shell and the pre-map screens keep the default.
                let clear = self.gpu.as_ref().map_or(CLEAR, |g| g.clear);
                let mut hud: Option<ui::hud::HudModel> = None;
                let mut credits_handoff = false;
                let mut spawn: Option<ui::spawn::SpawnModel> = None;
                // A death camera or a dead rider's held seat view is up.
                let icons = self.current_icons();
                let mut menu: Option<ui::menu::MenuModel> = None;
                let mut edges = if self.walk.is_some() {
                    self.menu_edges()
                } else {
                    0
                };
                let campaign_browser = self.netplay.is_none()
                    && self.walk.as_ref().is_some_and(|(w, _)| {
                        w.campaign.is_some()
                            && w.sim.outcome.is_none()
                            && w.sim.units[w.player.index as usize].alive
                    });
                if !campaign_browser {
                    self.campaign_map = None;
                }
                if campaign_browser && !self.paused {
                    let popup = self
                        .walk
                        .as_ref()
                        .and_then(|(w, _)| w.campaign.as_ref())
                        .is_some_and(|c| c.popup().is_some());
                    let close = game::kbm::MENU_MAP | MENU_START | MENU_BACK;
                    if self.campaign_map.is_some() && edges & close != 0 {
                        self.campaign_map = None;
                        if popup {
                            if let Some(c) =
                                self.walk.as_mut().and_then(|(w, _)| w.campaign.as_mut())
                            {
                                c.close_popup();
                            }
                        }
                        self.kbm.clear_look();
                        edges &= !close;
                    } else if !popup && edges & game::kbm::MENU_MAP != 0 {
                        if let Some(c) = self.walk.as_ref().and_then(|(w, _)| w.campaign.as_ref()) {
                            self.campaign_map = Some(
                                c.objectives()
                                    .iter()
                                    .rposition(|o| o.state == 2)
                                    .unwrap_or(0),
                            );
                            self.kbm.clear_look();
                        }
                    } else if !popup {
                        if let (Some(selected), Some(c)) = (
                            self.campaign_map.as_mut(),
                            self.walk.as_mut().and_then(|(w, _)| w.campaign.as_mut()),
                        ) {
                            if edges & MENU_UP != 0 {
                                *selected = selected.saturating_sub(1);
                            }
                            if edges & MENU_DOWN != 0 {
                                *selected =
                                    (*selected + 1).min(c.objectives().len().saturating_sub(1));
                            }
                            if edges & (MENU_CONFIRM | game::kbm::MENU_DETAILS) != 0 {
                                c.show_details(*selected);
                                edges &= !(MENU_CONFIRM | game::kbm::MENU_DETAILS);
                            } else if edges & (game::kbm::MENU_SQUARE | game::kbm::MENU_HINTS) != 0
                            {
                                c.show_hint();
                                edges &= !(game::kbm::MENU_SQUARE | game::kbm::MENU_HINTS);
                            }
                        }
                    }
                }
                if let Some(sounds) = self.sounds.as_mut() {
                    sounds.advance_menu_clock(elapsed);
                }
                if self.walk.is_some() && self.controls.is_none() && edges & 64 != 0 {
                    if !self.pause_menu.class_picker {
                        self.paused = !self.paused;
                    }
                    self.pause_menu.reset();
                    self.shell_pointer.take_frame();
                    self.shell_pointer.take_wheel();
                    // Start opens the pause menu with the accept click
                    // (menuref "In battle").
                    if self.paused {
                        if let Some(sl) = self.sounds.as_mut() {
                            sl.play_2d(
                                self.audio.as_mut(),
                                ui::shell_feel::MenuSound::Accept.name(),
                            );
                        }
                    }
                }
                self.sync_grab();
                self.sync_pause_audio();
                let native_pause = self.hud_view.as_ref().is_some_and(|v| v.can_draw_pause());
                let mut binding_menu = self.controls.is_some();
                if binding_menu {
                    let input = ui::menu::MenuInput {
                        up: edges & 1 != 0,
                        down: edges & 2 != 0,
                        confirm: edges & 16 != 0,
                        back: edges & 32 != 0,
                        ..Default::default()
                    };
                    let mut sound = self.step_controls(input);
                    let pointer = self.shell_pointer.take_frame();
                    let wheel = self.shell_pointer.take_wheel();
                    // The list's view eases every frame it is open: the
                    // paused branch below is skipped while it is.
                    let model = self
                        .controls_model(icons)
                        .filter(|_| !input.back && native_pause);
                    let size = self.renderer.as_ref().map(|r| {
                        let (w, h) = r.size();
                        [w as f32, h as f32]
                    });
                    let click = pointer.and_then(|(point, down)| down.then_some(point));
                    if let Some(row) = game::pause::binding_list_frame(
                        model.as_ref(),
                        &mut self.pause_menu.animation,
                        wheel,
                        click,
                        size,
                        dt,
                    ) {
                        sound = self.activate_control_row(row).or(sound);
                    }
                    if let (Some(sound), Some(lib)) = (sound, self.sounds.as_mut()) {
                        lib.play_2d(self.audio.as_mut(), sound.name());
                    }
                    menu = self.controls_model(icons);
                }
                let mut stereo_view = None;
                // A mission popup holds single player (`FUN_0038c7a0` sets
                // `DAT_00600b44`) until the player presses to continue
                // (`FUN_0038cb28`); the frame's audio runs on under it.
                let campaign_popup_up = self.netplay.is_none()
                    && self.walk.as_ref().is_some_and(|(w, _)| {
                        w.sim.outcome.is_none()
                            && w.campaign.as_ref().is_some_and(|c| c.popup().is_some())
                    });
                let popup_up =
                    campaign_popup_up || self.campaign_map.is_some() || self.round_end_held();
                if campaign_popup_up && !self.paused && edges & 16 != 0 {
                    if let Some((w, _)) = self.walk.as_mut() {
                        if let Some(c) = w.campaign.as_mut() {
                            c.dismiss_popup(&mut w.sim);
                        }
                    }
                    if let Some(sl) = self.sounds.as_mut() {
                        sl.play_2d(
                            self.audio.as_mut(),
                            ui::shell_feel::MenuSound::Accept.name(),
                        );
                    }
                }
                let camera = if self.walk.is_some() {
                    self.step_spawn_pointer(dt);
                    let ext_infantry = self.netplay.is_none()
                        && self
                            .display_ext
                            .as_ref()
                            .zip(self.walk.as_ref())
                            .is_some_and(|(ext, (walk, _))| {
                                ext.is_infantry(&super::xr::sim_view(walk))
                            });
                    let pad = if self.paused {
                        self.kbm.drop_taps();
                        game::pause::hold_for_resume(&mut self.resume_gate);
                        PadState::default()
                    } else if popup_up {
                        self.kbm.drop_taps();
                        PadState::default()
                    } else if self
                        .walk
                        .as_ref()
                        .is_some_and(|(walk, _)| walk.sim.outcome.is_some())
                    {
                        // Results and the banner use menu_edges only. Continue the
                        // scene with a neutral pad so the D-pad never gives orders.
                        self.kbm.drop_taps();
                        PadState::default()
                    } else {
                        let walk = &self.walk.as_ref().expect("battle walk").0;
                        let on_foot = walk.player_on_foot();
                        let space = walk.space_battle();
                        let pad = match self.autopilot {
                            true => autopilot_pad(self.walk.as_ref().map_or(0, |w| w.0.sim.tick)),
                            false => {
                                let controls =
                                    walk.flight_controls(self.profile.settings.flight_controls);
                                let look = if controls.is_some() {
                                    1.0
                                } else {
                                    walk.pad_look_for_seat(
                                        self.profile.settings.pad_look_sensitivity,
                                    )
                                };
                                let pad = self.sample_pad(look, on_foot && !space);
                                if let Some(bb) = self.blackbox.as_mut() {
                                    bb.stick(self.stick_sample.0, self.stick_sample.1);
                                }
                                let pad = match controls {
                                    Some(_) => self.kbm.flyer_pad(
                                        self.walk
                                            .as_ref()
                                            .expect("battle walk")
                                            .0
                                            .map_flight_sticks(
                                                self.profile.settings.flight_controls,
                                                pad,
                                            )
                                            .expect("flight controls selected"),
                                    ),
                                    None => self.kbm.pad(pad),
                                };
                                let pad = self
                                    .display_ext
                                    .as_ref()
                                    .filter(|_| self.netplay.is_none())
                                    .map_or(pad, |ext| {
                                        let walk = &self.walk.as_ref().expect("battle walk").0;
                                        ext.transform_pad(&super::xr::sim_view(walk), pad)
                                    });
                                game::pause::resume_pad(&mut self.resume_gate, pad)
                            }
                        };
                        let walk = &self.walk.as_ref().expect("battle walk").0;
                        if let Some(ext) = self.display_ext.as_ref().filter(|_| ext_infantry) {
                            ext.apply_sprint_latch(
                                &super::xr::sim_view(walk),
                                &mut self.sprint_latch,
                                pad,
                            )
                        } else {
                            walk.apply_sprint_latch(&mut self.sprint_latch, pad)
                        }
                    };
                    if self.view_toggle.pressed(&pad) && !ext_infantry {
                        let s = &mut self.profile.settings;
                        s.viewpoint = s.viewpoint.toggled();
                        if let Err(e) = self.profile.save() {
                            eprintln!("profile autosave failed: {e:#}");
                        }
                    }
                    let viewpoint = if ext_infantry {
                        game::viewpoint::Viewpoint::First
                    } else {
                        self.profile.settings.viewpoint
                    };
                    let pad = viewpoint.stamp(pad);
                    // The scoreboard is held on Select / View (never with the
                    // seat chord) or Tab, read as the key (LAN §5b, #182).
                    game::kbm::step_scoreboard(&mut self.score_hold, &self.kbm, &pad, dt);
                    // A LAN round: the pad is neutral while paused (the sim
                    // runs on), and carries a confirmed Respawn (issue #168).
                    let paused_now = self.paused;
                    let pad = match self.netplay.as_mut() {
                        Some(n) => n.frame_pad(pad, paused_now),
                        None => pad,
                    };
                    let paused = self.paused;
                    let (walk, cube) = self.walk.as_mut().unwrap();
                    // A FIRE press fires where the reticle was drawn.
                    let mut pad = walk.blend.stamp(pad);
                    if ext_infantry {
                        pad.fire_lag = 0;
                    }
                    let mut stuck_alarm = false;
                    let mut look = PadState::default();
                    if paused && binding_menu {
                        // The controls editor consumed the frame's menu input above.
                    } else if paused {
                        if self.netplay.is_none()
                            && !self.audio.as_ref().is_some_and(|a| a.game_paused())
                        {
                            if let Some(sl) = self.sounds.as_mut() {
                                sl.stop_loops();
                            }
                        }
                        let mut model = ui::menu::MenuModel {
                            class_picker_available: false,
                            bindings: None,
                            class_choices: self.pause_menu.class_picker_model(walk),
                            cheats: match self.netplay.as_mut() {
                                Some(n) => n.cheat_entries(walk),
                                None => walk.sim.cheats.entries().to_vec(),
                            },
                            assist_intensity: walk.sim.assist_intensity,
                            difficulty: game::shell_ia::ia_level(self.profile.settings.difficulty),
                            effects_volume: self.profile.settings.effects_volume,
                            dialogue_volume: self.profile.settings.dialogue_volume,
                            fps_counter: ui::menu::FpsCounter::from_index(
                                self.profile.settings.fps_counter,
                            ),
                            network_meter: self.profile.settings.network_meter,
                            player_name: self.profile.name.clone(),
                            team_size_pct: walk.team_size_pct,
                            reinforcements_pct: walk.reinforcement_pct,
                            population_note: game::conquest_defaults::menu_note(
                                walk.team_size_pct,
                                walk.reinforcement_pct,
                            ),
                            // A guest's battle size is the host's (issue #168).
                            team_size_off: walk.conquest_defaults.is_none()
                                || self.netplay.as_ref().is_some_and(|n| !n.is_host()),
                            fov_scale: walk.fov_scale,
                            base_fov_deg: game::walk::chase_fov_y().to_degrees(),
                            music_volume: walk
                                .music
                                .as_ref()
                                .map_or(self.profile.settings.music_volume, |m| m.volume),
                            mouse_sensitivity: self.profile.settings.mouse_sensitivity,
                            pad_look_sensitivity: self.profile.settings.pad_look_sensitivity,
                            rumble_strength: self.profile.settings.rumble_strength,
                            rumble_on_fire: self.profile.settings.rumble_on_fire,
                            career_awards_disabled: !self.profile.settings.career_awards,
                            rc_flight_controls: self.profile.settings.flight_controls
                                == input::FlightControls::RcMode2,
                            invert_y: self.profile.settings.invert_y,
                            low_power_resolution: self.profile.settings.low_power_resolution,
                            terrain_lod: self.profile.settings.terrain_lod,
                            hard_shadows: self.profile.settings.hard_shadows,
                            scene_softness: self.profile.settings.scene_softness,
                            auto_quality: self.profile.settings.auto_quality,
                            window_mode: self.display.shown(self.profile.settings.window_mode),
                            exclusive_fullscreen: self.display.exclusive,
                            input_icons_label: self.profile.settings.input_icons.label(),
                            icons,
                            first_person: self.profile.settings.viewpoint.is_first(),
                            cursor: self.pause_menu.cursor,
                            page: Some(self.pause_menu.page),
                            popup: self.pause_menu.confirm,
                            restart_available: self.current.is_some()
                                || self.active_campaign.is_some(),
                            texts: game::pause::texts(&self.feed),
                            respawn_label: game::pause::respawn_label(walk, &self.feed),
                            in_shell: None,
                            outcome_line: walk.sim.outcome.map(|w| match w {
                                core::sim::OUTCOME_DRAW => "Time up — both teams win".to_string(),
                                _ => format!(
                                    "Round over — {} wins",
                                    walk.teams[usize::from(w.min(1))].faction
                                ),
                            }),
                        };
                        const ADJUST: u32 =
                            game::kbm::MENU_ADJUST_LEFT | game::kbm::MENU_ADJUST_RIGHT;
                        const PAGE: u32 = game::kbm::MENU_L1 | game::kbm::MENU_R1;
                        let adjust = edges
                            | self
                                .pause_repeat
                                .step(self.prev_menu_buttons & (ADJUST | PAGE), dt);
                        let page_right = match adjust & PAGE {
                            game::kbm::MENU_L1 => Some(false),
                            game::kbm::MENU_R1 => Some(true),
                            _ => None,
                        };
                        // The D-pad's and arrows' presses: a step is not a Morse key.
                        let input = ui::menu::MenuInput {
                            up: edges & 1 != 0,
                            down: edges & 2 != 0,
                            left: edges & 4 != 0 && edges & game::kbm::MENU_ADJUST_LEFT == 0,
                            right: edges & 8 != 0 && edges & game::kbm::MENU_ADJUST_RIGHT == 0,
                            confirm: edges & 16 != 0,
                            back: edges & 32 != 0,
                        };
                        let rows_input = game::pause::rows_input(
                            input,
                            adjust & game::kbm::MENU_ADJUST_LEFT != 0,
                            adjust & game::kbm::MENU_ADJUST_RIGHT != 0,
                            model.popup.is_some()
                                || model.class_choices.is_some()
                                || matches!(self.pause_menu.page, ui::menu::PausePage::Settings(_)),
                        );
                        let pointer = self.shell_pointer.take_frame();
                        let wheel = self.shell_pointer.take_wheel();
                        let from = model.cursor;
                        let target = if native_pause {
                            let target = match (pointer, self.renderer.as_ref()) {
                                (Some((point, true)), Some(r)) => {
                                    let (w, h) = r.size();
                                    ui::pause_draw::target(
                                        &model,
                                        &self.pause_menu.animation,
                                        point,
                                        [w as f32, h as f32],
                                    )
                                }
                                _ => None,
                            };
                            if target.is_none() && wheel != 0.0 {
                                if let Some((w, h)) = self.renderer.as_ref().map(|r| r.size()) {
                                    ui::pause_draw::scroll(
                                        &model,
                                        &self.pause_menu.animation,
                                        wheel,
                                        [w as f32, h as f32],
                                    );
                                }
                            }
                            self.pause_menu.animation.advance(dt);
                            target
                        } else {
                            None
                        };
                        let (cursor, actions) = game::pause::navigate(
                            &mut model,
                            rows_input,
                            target,
                            page_right,
                            &self.pause_menu.animation,
                        );
                        let click = game::pause::pause_sound(rows_input, from, cursor, &actions);
                        if let (Some(c), Some(sl)) = (click, self.sounds.as_mut()) {
                            sl.play_2d(self.audio.as_mut(), c.name());
                        }
                        // The PS2 matcher runs beside the menu on the same
                        // edges (the cursor wiggle while typing is authentic).
                        let on_cheats = self.pause_menu.page == ui::menu::PausePage::Cheats;
                        let word_line = match self.netplay.as_mut().filter(|_| on_cheats) {
                            // A LAN round: the word goes to the host (issue #168).
                            Some(n) => self
                                .cheats
                                .word(input)
                                .map(|w| n.morse_word(w, walk, &mut self.cheats)),
                            None => on_cheats
                                .then(|| self.cheats.feed(input, &mut walk.sim))
                                .flatten(),
                        };
                        if let Some(line) = word_line {
                            if let Some(sl) = self.sounds.as_mut() {
                                sl.rate_mul = self.cheats.sfx_rate();
                                sl.play_2d(
                                    self.audio.as_mut(),
                                    ui::shell_feel::MenuSound::Accept.name(),
                                );
                            }
                            eprintln!("{line}");
                            self.feed
                                .say(line, ui::hud::message_color::YELLOW, walk.sim.tick);
                        }
                        self.pause_menu.cursor = cursor;
                        let mut quit = false;
                        for a in actions {
                            if let ui::menu::MenuAction::EditBindings(device, context) = a {
                                self.controls = Some(super::front::Controls::new(
                                    device,
                                    context,
                                    &self.profile.settings,
                                ));
                                binding_menu = true;
                            }
                            self.restart_clicked |= a == ui::menu::MenuAction::Restart;
                            if let ui::menu::MenuAction::SetWindowMode(m) = a {
                                self.display.requested = Some(m);
                            }
                            if self.netplay.as_mut().is_some_and(|n| {
                                n.pause_action(
                                    walk,
                                    a,
                                    &mut self.paused,
                                    &mut self.pause_menu,
                                    &mut self.profile,
                                    &mut self.session,
                                )
                            }) {
                                continue;
                            }
                            quit |= apply_pause_action(
                                walk,
                                a,
                                &mut self.paused,
                                &mut self.pause_menu,
                                &mut self.cheats,
                                &mut self.profile,
                                &mut self.session,
                                &mut self.kbm,
                                self.music_voice.as_mut(),
                            );
                            if matches!(a, ui::menu::MenuAction::SetRumble(_)) {
                                game::rumble::preview(
                                    self.gilrs.as_mut(),
                                    &self.active_pad,
                                    &mut self.rumble_effect,
                                    self.profile.settings.rumble_strength,
                                );
                            }
                        }
                        if quit {
                            self.quit_clicked = true;
                        }
                        if !binding_menu {
                            menu = Some(ui::menu::MenuModel {
                                difficulty: game::shell_ia::ia_level(
                                    self.profile.settings.difficulty,
                                ),
                                effects_volume: self.profile.settings.effects_volume,
                                dialogue_volume: self.profile.settings.dialogue_volume,
                                fps_counter: ui::menu::FpsCounter::from_index(
                                    self.profile.settings.fps_counter,
                                ),
                                network_meter: self.profile.settings.network_meter,
                                player_name: self.profile.name.clone(),
                                class_choices: self.pause_menu.class_picker_model(walk),
                                cursor: self.pause_menu.cursor,
                                page: model.page.map(|_| self.pause_menu.page),
                                popup: self.pause_menu.confirm,
                                cheats: match self.netplay.as_mut() {
                                    Some(n) => n.cheat_entries(walk),
                                    None => walk.sim.cheats.entries().to_vec(),
                                },
                                assist_intensity: walk.sim.assist_intensity,
                                team_size_pct: walk.team_size_pct,
                                reinforcements_pct: walk.reinforcement_pct,
                                population_note: game::conquest_defaults::menu_note(
                                    walk.team_size_pct,
                                    walk.reinforcement_pct,
                                ),
                                team_size_off: walk.conquest_defaults.is_none()
                                    || self.netplay.as_ref().is_some_and(|n| !n.is_host()),
                                fov_scale: walk.fov_scale,
                                base_fov_deg: game::walk::chase_fov_y().to_degrees(),
                                music_volume: walk
                                    .music
                                    .as_ref()
                                    .map_or(self.profile.settings.music_volume, |m| m.volume),
                                mouse_sensitivity: self.profile.settings.mouse_sensitivity,
                                pad_look_sensitivity: self.profile.settings.pad_look_sensitivity,
                                rumble_strength: self.profile.settings.rumble_strength,
                                rumble_on_fire: self.profile.settings.rumble_on_fire,
                                career_awards_disabled: !self.profile.settings.career_awards,
                                rc_flight_controls: self.profile.settings.flight_controls
                                    == input::FlightControls::RcMode2,
                                invert_y: self.profile.settings.invert_y,
                                low_power_resolution: self.profile.settings.low_power_resolution,
                                terrain_lod: self.profile.settings.terrain_lod,
                                hard_shadows: self.profile.settings.hard_shadows,
                                scene_softness: self.profile.settings.scene_softness,
                                auto_quality: self.profile.settings.auto_quality,
                                window_mode: self.display.shown(self.profile.settings.window_mode),
                                input_icons_label: self.profile.settings.input_icons.label(),
                                first_person: self.profile.settings.viewpoint.is_first(),
                                ..model
                            });
                        }
                    }
                    // A LAN round steps on under its pause menu (issue #168).
                    if !paused || self.netplay.is_some() {
                        // Each stepped tick's events are read in the hook,
                        // once: a frame steps no tick (every other one at
                        // 60 Hz), one or several, and `Sim::events` keeps
                        // only the last tick's (card EVTK).
                        // The world holds still under a popup; only the
                        // voice and music frames below run on.
                        let world_dt = if popup_up { 0.0 } else { dt };
                        // The held round end (`App::round_end_held`) steps no
                        // tick to keep the loops' voices current, and BF2
                        // stops the battle's sounds as its banner goes up
                        // (`FUN_00380c30`, re-round-end.md §2.2): they stop.
                        if walk.sim.outcome.is_some() && self.netplay.is_none() {
                            if let Some(sl) = self.sounds.as_mut() {
                                sl.stop_loops();
                            }
                        }
                        self.bolts.age(walk, world_dt);
                        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                            let (w, h) = r.size();
                            w as f32 / h.max(1) as f32
                        });
                        if self.netplay.is_none() {
                            // The class screen's unlocks read the player's
                            // points as the feed has scored them (card SSELB,
                            // re-spawn-flow §4.3) and its career's costs
                            // (UNLK3). A LAN round gates every slot itself
                            // (`netplay::gate`).
                            let points = self.feed.stats(&walk.sim, walk.player).points;
                            game::unlocks::set_player_gate(walk, points, &self.profile.career);
                        }
                        // Station serving is observed for every sim tick, even when a
                        // frame catches up several ticks before posing the scene.
                        if let Some(gpu) = self.gpu.as_mut() {
                            gpu.step_stations(&walk.sim);
                        }
                        let mut st = TickState::new(walk, self.profile.career.medals, pad, aspect);
                        let mut hook = TickHook {
                            gpu: self.gpu.as_mut(),
                            fx: &mut self.fx,
                            soldier_vis: self.soldier_vis.as_mut(),
                            deka_vis: &mut self.deka_vis,
                            bolts: &mut self.bolts,
                            audio: self.audio.as_mut(),
                            sounds: self.sounds.as_mut(),
                            foley: self.foley.as_mut(),
                            engines: &mut self.engines,
                            ambience: &self.ambience,
                            blackbox: self.blackbox.as_mut(),
                            rumble_strength: self.profile.settings.rumble_strength,
                            rumble_on_fire: self.profile.settings.rumble_on_fire,
                            rumble_tick: &mut self.rumble_tick,
                            gilrs: self.gilrs.as_mut(),
                            active_pad: &self.active_pad,
                            rumble_effect: &mut self.rumble_effect,
                            matchlog: self.matchlog.as_mut(),
                            session_stats: self.session_stats.as_mut(),
                            frame_profile: self.frame_profile.as_mut(),
                            display_ext: self.display_ext.as_deref(),
                        };
                        let feed = &mut self.feed;
                        match self.netplay.as_mut() {
                            Some(n) => {
                                let catching_up = n.catching_up();
                                if catching_up {
                                    n.banner = Some((
                                        format!(
                                            "Catching up with the battle: tick {}",
                                            n.peer.next_tick()
                                        ),
                                        std::time::Instant::now(),
                                    ));
                                }
                                // Every frame's pad joins the input clock's
                                // open tick: the mouse's motion and the
                                // presses are taken now, and a FIRE press's
                                // lag is the open input tick's share.
                                // Sim first (LAN §8b.1): the frame's whole
                                // wall time, so a late frame steps every
                                // bundle that came due before it draws.
                                let report = n.view.frame(
                                    &mut n.peer,
                                    &mut n.link,
                                    elapsed.fmin(1.0),
                                    net::bundle::strip_tracked_inputs(pad),
                                    walk,
                                    Some(feed),
                                    |w, _, f, muted| {
                                        if let Some(f) = f.filter(|_| !catching_up) {
                                            hook.tick(w, f, &mut st, true, muted);
                                        }
                                    },
                                );
                                if let Some(at) = report.corrected {
                                    // The host's game was replaced by its
                                    // replay (LAN §4.7): the prediction and
                                    // the render blend started over in the
                                    // view; the drawn bolts were the old
                                    // game's.
                                    self.bolts.clear();
                                    println!(
                                        "lan: the game was corrected from the check at tick {at}"
                                    );
                                }
                                self.kbm.consumed(&pad, 1);
                                walk.blend.consumed(1);
                                n.update_meter(Some(walk), report.ticks);
                                if let Some(bb) = self.blackbox.as_mut() {
                                    bb.net(walk.sim.tick, &n.meter);
                                }
                                // Own shot feedback from the prediction, at once.
                                let shots = n.view.shots();
                                if !shots.is_empty() {
                                    if let (Some(psim), Some(audio), Some(sl)) = (
                                        n.view.predictor.sim(),
                                        self.audio.as_mut(),
                                        self.sounds.as_mut(),
                                    ) {
                                        let events: Vec<core::sim::Event> = shots
                                            .iter()
                                            .map(|s| core::sim::Event::Shot {
                                                unit: s.unit,
                                                weapon_class: s.weapon_class,
                                                origin: s.origin,
                                                dir: s.dir,
                                            })
                                            .collect();
                                        let mut asks = Vec::new();
                                        game::event_sounds::weapon_asks_of(psim, &events, |a| {
                                            asks.push(a)
                                        });
                                        let listener = game::event_sounds::listener(walk);
                                        sl.play_weapon_voices(audio, &asks, &listener);
                                    }
                                    if let Some(vis) = self.soldier_vis.as_mut() {
                                        for s in shots {
                                            let wc = s.weapon_class as usize;
                                            if !walk.sim.is_item_gun(wc) {
                                                vis.predicted_shot(
                                                    s.unit.index,
                                                    s.unit.gen,
                                                    walk.sim.tick,
                                                    wc,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            None if popup_up => {}
                            None => {
                                let ticks =
                                    walk.advance_while(dt, std::slice::from_ref(&pad), |walk| {
                                        hook.tick(walk, feed, &mut st, false, false);
                                        !walk.campaign.as_ref().is_some_and(|c| c.has_movies())
                                    });
                                self.kbm.consumed(&pad, ticks);
                                walk.blend.consumed(ticks);
                                if let Some(ext) = self.display_ext.as_mut() {
                                    ext.on_sim_ticks_consumed(ticks);
                                }
                            }
                        }
                        stuck_alarm |= st.stuck_alarm;
                        if walk.flush_career_progress(&mut self.profile) {
                            if let Err(e) = self.profile.save() {
                                eprintln!("profile autosave failed: {e:#}");
                            }
                        }
                        if let Some(c) = walk.campaign.as_mut() {
                            for msg in c.take_messages() {
                                let text = feed.localize_or_raw(&msg);
                                feed.say(text, ui::hud::message_color::YELLOW, walk.sim.tick);
                            }
                            let _ = c.take_sounds();
                            let calls = c.take_music();
                            if let Some(m) = walk.music.as_mut() {
                                m.script_calls.extend(calls);
                            }
                        }
                        look = if popup_up {
                            self.kbm.clear_look();
                            PadState::default()
                        } else {
                            self.kbm.look()
                        };
                        // A controller's preview is its held stick (the
                        // next tick's turn, `Sim::look_preview`).
                        if !popup_up && !look.has_look() && !look.mouse_aim() {
                            look = PadState {
                                mx: 0,
                                my: 0,
                                ..pad
                            };
                        }
                        if let Some(p) = self.frame_profile.as_mut() {
                            p.lap("sim_ms");
                        }
                        self.fx
                            .advance_frame(self.soldier_vis.as_mut(), walk, &look, world_dt);
                        if let Some(w) = self.weather.as_mut() {
                            w.update(world_dt);
                            rain_splashes(w, &mut self.fx, walk, aspect, world_dt);
                        }
                        if let (Some(audio), Some(sl)) = (self.audio.as_mut(), self.sounds.as_mut())
                        {
                            // The commander: this frame's requests join the
                            // voice queue, and an idle voice starts its head.
                            // A squad answer plays the take drawn for it,
                            // at its unit.
                            let ears = self
                                .display_ext
                                .as_ref()
                                .filter(|_| self.netplay.is_none())
                                .and_then(|ext| ext.stereo_view(&super::xr::sim_view(walk)))
                                .map(|view| super::xr::listener(&view))
                                .unwrap_or_else(|| game::event_sounds::listener(walk));
                            if let Some(v) = walk.voice.as_mut() {
                                let tick = walk.sim.tick;
                                // The units' combat calls through the
                                // bank's gates join the queue first.
                                v.gate_calls(sl, ears.pos);
                                for name in v.gate_script_sounds(sl) {
                                    sl.play_voice(audio, &name);
                                }
                                v.frame(&walk.sim, walk.player, dt, |line| {
                                    if let Some(bb) = self.blackbox.as_mut() {
                                        bb.voice(tick, &line.name);
                                    }
                                    match (line.take, line.at) {
                                        (Some(take), Some(at)) => sl.play_take(
                                            audio,
                                            &line.name,
                                            usize::from(take),
                                            if line.mode_3d { at } else { ears.pos },
                                            &ears,
                                        ),
                                        _ => sl.play_voice(audio, &line.name),
                                    }
                                });
                            }
                            if let Some(bb) = self.blackbox.as_mut() {
                                note_sounds(bb, Some(sl), walk.sim.tick);
                            }
                        }
                        // The music: the script's Musics by the
                        // reinforcements, one streamed take at a time. The
                        // frame runs with or without a sound device (card
                        // MUSV: the dev log is the window's referee); only
                        // the voice needs one.
                        if let (Some(m), Some(sl)) = (walk.music.as_mut(), self.sounds.as_ref()) {
                            if !m.bound() {
                                let missing = m.bind(sl);
                                if !missing.is_empty() {
                                    eprintln!("music: not in the bank: {missing:?}");
                                }
                            }
                            let missing = m.apply_script(sl);
                            if !missing.is_empty() {
                                eprintln!("music: not in the bank: {missing:?}");
                            }
                            let db = game::sounds::level_db;
                            let tickets = [0, 1].map(|t| walk.sim.battle.teams[t].tickets());
                            for c in m.frame(&walk.sim, walk.player, dt) {
                                if game::dev::on() {
                                    eprintln!("{}", m.log_line(&c, tickets));
                                }
                                self.music_voice = match (c, self.audio.as_mut()) {
                                    (core::music::Change::Started { music, take }, Some(audio)) => {
                                        start_take(audio, m, music, take, db(0.0))
                                    }
                                    _ => None,
                                };
                            }
                            if let (Some(v), Some((_, level))) =
                                (self.music_voice.as_mut(), m.level())
                            {
                                v.set_volume(db(level), dt);
                            }
                        }
                    }
                    // The black box: this frame's time, and a snapshot when
                    // F9 was pressed or the stuck alarm fired — written on its
                    // own thread. F9 reports the outcome above the menus.
                    if let Some(bb) = self.blackbox.as_mut() {
                        if let Some(saved) = bb.poll_hotkey_write() {
                            self.snapshot_notice = Some((saved, std::time::Instant::now()));
                        }
                        let size = self.renderer.as_ref().map_or((0, 0), |r| r.size());
                        bb.frame(dt, [size.0, size.1]);
                        let reason = if std::mem::take(&mut self.snapshot_requested) {
                            Some("hotkey")
                        } else {
                            stuck_alarm.then_some("stuck")
                        };
                        if let Some(reason) = reason {
                            bb.write(reason, Some(walk), false);
                            if reason == "stuck" {
                                self.feed.say(
                                    format!(
                                        "snapshot saved ({reason}, {:.1} s)",
                                        walk.sim.tick as f32 * core::TICK_DT
                                    ),
                                    ui::hud::message_color::YELLOW,
                                    walk.sim.tick,
                                );
                            }
                        }
                    }
                    if let Some(p) = self.frame_profile.as_mut() {
                        p.lap("update_ms");
                    }
                    // From here to the draw lists' end the walk holds the
                    // pose blended between its last two ticks (card RINT):
                    // the camera, units and vehicles drawn at the display's
                    // rate. Restored below, before anything else reads it.
                    match self.netplay.as_mut() {
                        // Issue #168: the look over every pending input, then
                        // the predicted local pose (`netplay::view`).
                        Some(n) => n.view.draw_in(walk),
                        None => game::render_blend::blend_in_looking(walk, &look),
                    }
                    if !self.paused && self.netplay.is_none() && super::xr::live_view(walk) {
                        stereo_view = self
                            .display_ext
                            .as_ref()
                            .and_then(|ext| ext.stereo_view(&super::xr::sim_view(walk)));
                    }
                    let pending = walk
                        .class_picker
                        .as_mut()
                        .map(|p| std::mem::take(&mut p.pending_assets))
                        .unwrap_or_default();
                    for request in pending {
                        self.feed.extend_class_names(walk);
                        if let Some(bb) = self.blackbox.as_mut() {
                            bb.extend_class_names(walk);
                        }
                        if let Some(log) = self.matchlog.as_mut() {
                            log.extend_class_names(walk);
                        }
                        if let Some(picker) = walk.class_picker.as_ref() {
                            self.fx.extend_test_classes(
                                &self.assets_dir,
                                &picker.scene.dir,
                                request.game,
                                walk,
                            );
                        }
                        if let (Some(r), Some(scene)) = (
                            self.renderer.as_mut(),
                            walk.class_picker.as_ref().map(|p| &p.scene),
                        ) {
                            if let Some(vis) = self.soldier_vis.as_mut() {
                                vis.append_test_class(r, &self.assets_dir, walk, &request);
                            }
                            let lit_chunks = self.vehicle_vis.load_chunks(
                                r,
                                scene,
                                &self.assets_dir,
                                &self.fx.chunk_models(),
                            );
                            self.fx.set_lit_chunks(lit_chunks);
                            self.deka_vis.append_test_class(
                                r,
                                scene,
                                &self.assets_dir,
                                walk,
                                &request,
                            );
                        }
                    }
                    walk_draws(*cube, walk, &mut extra, self.soldier_vis.as_ref());
                    // The props and vehicles in the camera's way are drawn
                    // see-through (`game::camera_collide`).
                    see = walk.camera_see_through();
                    if let Some(r) = self.renderer.as_mut() {
                        r.set_sun_shadows(self.profile.settings.hard_shadows);
                        self.vehicle_vis
                            .set_tick_share(game::render_blend::share(walk));
                        self.vehicle_vis
                            .set_ctf_viewer(walk.sim.unit(walk.player).map(|u| u.team));
                        self.vehicle_vis.set_flag_ribcage(
                            walk.sim
                                .ctf
                                .as_ref()
                                .and(self.soldier_vis.as_ref())
                                .map(|v| v.jet_frames(walk)),
                        );
                        extra.extend(self.vehicle_vis.draws(r, &walk.sim, &see.vehicles));
                        extra.extend(self.vehicle_vis.campaign_flags(walk));
                        extra.extend(self.vehicle_vis.chunk_draws(self.fx.lit_chunk_poses()));
                        self.deka_vis.eye = Some(
                            stereo_view
                                .map_or_else(|| walk.camera().position, |v| v.camera.position),
                        );
                        extra.extend(self.deka_vis.draws(r, &walk.sim));
                    }
                    if let Some(vis) = self.soldier_vis.as_mut() {
                        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                            let (w, h) = r.size();
                            w as f32 / h as f32
                        });
                        if let Some(view) = stereo_view {
                            super::xr::prepare_soldiers(&view, walk, vis);
                        } else {
                            vis.prepare_view(walk, &walk.camera(), aspect);
                        }
                        skinned = vis.draws(walk);
                        extra.extend(vis.weapon_draws(walk));
                        if let Some(r) = self.renderer.as_mut() {
                            extra.extend(vis.cloth_draws(r, walk));
                        }
                    }
                    self.frames += 1;
                    if self.frames.is_multiple_of(10) {
                        if let Some(w) = &self.window {
                            w.set_title(&hud_title(walk));
                        }
                    }
                    // The end of the round (issue #33): its camera shot and
                    // banner, then BF2's stats pages (issue #50), made once
                    // the banner is down; when they are left, the next
                    // playlist map or the shell.
                    if walk.sim.outcome.is_some() && !self.paused {
                        let pad = ui::stats::Pad {
                            up: edges & 1 != 0,
                            down: edges & 2 != 0,
                            left: edges & 4 != 0,
                            right: edges & 8 != 0,
                            cross: edges & 16 != 0,
                            triangle: edges & 32 != 0,
                            l1: edges & game::kbm::MENU_L1 != 0,
                            r1: edges & game::kbm::MENU_R1 != 0,
                            square: edges & game::kbm::MENU_SQUARE != 0,
                        };
                        let pad = super::xr::stats_pointer(
                            pad,
                            walk,
                            self.display_ext.as_deref(),
                            self.renderer.as_ref(),
                            self.hud_view.as_ref(),
                            &mut self.shell_pointer,
                        );
                        let music_secs = walk.music.as_ref().and_then(|m| {
                            let team = walk.sim.units.get(walk.player.index as usize)?.team;
                            m.end_music_secs(usize::from(team), walk.sim.outcome?)
                        });
                        let credits_after_outro = self.netplay.is_none()
                            && self.active_campaign
                                == Some(ui::campaign::RISE_OF_THE_EMPIRE.len() - 1)
                            && walk.sim.outcome == Some(0);
                        let (mut done, cue) =
                            walk.follow_round_end(dt, pad, music_secs, !credits_after_outro);
                        if let (Some(cue), Some(sounds)) = (cue, self.sounds.as_mut()) {
                            sounds.play_2d(self.audio.as_mut(), cue.name());
                        }
                        if let Some(r) = walk.round_end.as_mut() {
                            let frame = game::round_end::step_end_movie(
                                r,
                                self.front.as_mut().and_then(|f| f.movie.as_mut()),
                                self.audio.as_mut(),
                                &mut self.end_movie_voice,
                                self.profile.settings.dialogue_volume,
                                dt,
                                pad.cross || pad.triangle,
                            );
                            if let (Some((rgba, w, h)), Some(v)) = (frame, self.hud_view.as_mut()) {
                                v.set_movie_frame(rgba, w, h);
                            }
                            done |= r.done();
                        }
                        credits_handoff = done && credits_after_outro;
                        if done && self.netplay.is_some() {
                            // A LAN round: the lobby's ready check (issue #168).
                            self.net_round_over = true;
                        } else if done {
                            self.quit_clicked = true;
                            self.round_over = true;
                        }
                        if walk.round_end.as_ref().is_some_and(|r| r.wants_stats()) {
                            let pages = stats_pages(walk, &self.feed, &self.profile.career);
                            if let Some(r) = walk.round_end.as_mut() {
                                r.stats = Some(pages);
                            }
                        }
                    }
                    if walk.sim.outcome.is_some() && !self.outcome_reported {
                        self.outcome_reported = true;
                        if let Some(n) = self.netplay.as_mut() {
                            n.peer.remember_guest_medals(walk);
                            n.round_decided();
                        }
                        println!("{}", hud_title(walk));
                    }
                    if walk.sim.outcome.is_some() && !self.round_recorded {
                        self.round_recorded = true;
                        record_round(&mut self.profile, walk, self.feed.battle_stats());
                    }
                    // The tick hook above has read the events; the model
                    // carries the new lines to the HUD's message box.
                    let hud_aspect = self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                        let (w, h) = r.size();
                        w as f32 / h.max(1) as f32
                    });
                    let stereo_cam = stereo_view.as_ref().map(|v| (&v.camera, v.aspect));
                    hud = Some(hud_model_for_view(
                        walk,
                        &mut self.feed,
                        hud_aspect,
                        stereo_cam,
                    ));
                    // Issue #182: the teammate view's hint names the device in use.
                    if let Some(sp) = hud.as_mut().and_then(|m| m.spectate.as_mut()) {
                        if !sp.hint.is_empty() {
                            if let game::input_icons::Device::Pad(family) =
                                self.icon_device.device()
                            {
                                use input::PadFamily as F;
                                let xbox = matches!(family, F::XboxOriginal | F::XboxOne);
                                let set = self.icon_device.set(self.profile.settings.input_icons);
                                sp.hint = game::input_icons::spectate_pad_hint(
                                    set,
                                    &self.profile.settings.controller_bindings,
                                )
                                .unwrap_or_else(|| {
                                    crate::battle::pad_spectate_hint(xbox).to_owned()
                                });
                            }
                        }
                    }
                    // The capture icon is off under the pause menu, as the
                    // PS2's (card CPBAR, re-capture-display 3.4).
                    if let Some(m) = hud.as_mut().filter(|_| paused) {
                        m.capture = None;
                    }
                    // Spawn screen while dead: the post and class it shows
                    // are the sim's (card SPSEL), moved by the pad bits.
                    spawn =
                        spawn_model(walk, &self.feed).map(|s| ui::spawn::SpawnModel { icons, ..s });
                    let camera = self.fx.shake.apply(walk.camera());
                    let camera = stereo_view.map_or(camera, |view| view.camera);
                    if let (Some(p), Some(me)) =
                        (self.frame_profile.as_mut(), walk.sim.unit(walk.player))
                    {
                        p.drawn(camera.position, me.soldier.pos);
                    }
                    camera
                } else {
                    self.drive_fly_camera(dt);
                    self.cam
                };
                let menu = if binding_menu {
                    self.controls_model(icons)
                } else {
                    menu
                };
                let menu = menu.filter(|_| self.paused);
                let native_pause = native_pause && menu.is_some();
                if let Some(quad) = self.fx_quad {
                    extra.extend(self.fx.draws(quad, self.fx_sprite, &camera));
                    if let Some((walk, _)) = &self.walk {
                        // The view's own squad (issue #173: slot 0 drew the
                        // host's squad markers on a LAN guest's screen).
                        let slot = walk.sim.units[walk.player.index as usize]
                            .spec
                            .player_slot
                            .unwrap_or(0);
                        extra.extend(game::squad_vis::squad_draws(
                            &walk.sim,
                            slot,
                            quad,
                            self.squad_icons.0,
                            self.squad_icons.1,
                            &camera,
                        ));
                    }
                }

                let frozen = self.frozen();
                if let (false, Some(r), Some(gpu)) =
                    (credits_handoff, &mut self.renderer, &mut self.gpu)
                {
                    gpu.apply_render_options(
                        r,
                        self.profile.settings.terrain_lod,
                        self.profile.settings.scene_softness,
                    );
                    // The water moves on the renderer's clock (queue OC).
                    if let Err(e) = gpu.animate_water_live(r, camera.position) {
                        eprintln!("water: {e:#}");
                    }
                    // The drawn doors ride the sim's (queue DRV), the bolts
                    // the sim's bolts (card FXB).
                    match &self.walk {
                        Some((walk, _)) => {
                            game::forest::submit_actors(r, &walk.sim);
                            gpu.pose_doors(&walk.sim.doors);
                            gpu.hide_animated(&walk.sim.props);
                            if let Some(poses) = walk.world_instance_poses() {
                                gpu.pose_world_anims(poses);
                            }
                            gpu.pose_turrets(&walk.sim, &walk.vehicle_seats);
                            gpu.pose_stations(&walk.sim);
                            gpu.see_through(&see.props);
                            gpu.pose_ctf(&walk.sim, walk.player);
                            self.holos.advance(walk);
                            let items = self.vehicle_vis.campaign_lights(walk);
                            game::point_light::submit(
                                r,
                                gpu,
                                walk,
                                &mut self.bolts,
                                camera.position,
                                &self.holos,
                                &items,
                            );
                            let flashes = self
                                .soldier_vis
                                .as_ref()
                                .map(|v| {
                                    let mut points = v.fire_points(walk);
                                    self.deka_vis.fire_points(&walk.sim, &mut points);
                                    if let Some(view) = stereo_view {
                                        super::xr::place_tracked_muzzle(
                                            &view,
                                            walk,
                                            v,
                                            &mut points,
                                        );
                                    }
                                    v.flash_draws_at(walk, &points)
                                })
                                .unwrap_or_default();
                            let mut particles = flashes;
                            if let Some(vis) = &self.soldier_vis {
                                particles.extend(vis.saber_draws(walk));
                                if let Some(view) = stereo_view {
                                    vis.tracked_saber_draws(walk, view.hand_mounts, &mut particles);
                                }
                            }
                            particles.extend(self.deka_vis.bubbles(
                                &walk.sim,
                                &camera,
                                r.lighting.sun_dir,
                            ));
                            particles.extend(self.holos.draws(&gpu.holo, walk, &camera));
                            particles.extend(self.vehicle_vis.campaign_glows(walk, &camera));
                            if let Some(w) = self.weather.as_mut() {
                                particles.extend(w.frame_draws(r, &camera, tracked_velocity(walk)));
                            }
                            self.fx.submit_walk(r, &camera, walk, particles);
                        }
                        None => {
                            r.set_leaf_time(None);
                            r.set_leaf_actors(Vec::new());
                            r.set_bolts(Vec::new());
                            r.set_underwater(None);
                            r.set_particles(Vec::new());
                            r.set_terrain_lights(Default::default());
                        }
                    }
                    // The player's own view: a zoomed turret gunner's
                    // building is left out of it (card VTURZ).
                    let mut draws: Vec<DrawCmd> = match &self.walk {
                        Some((walk, _)) => gpu.view_draws_with_hidden_instances(
                            &walk.sim,
                            walk.player,
                            walk.campaign.as_ref().map(|c| c.hidden_instances()),
                        ),
                        None => gpu.draws.clone(),
                    };
                    draws.append(&mut extra);
                    let ext_stereo = if let (Some(ext), Some(view), Some((walk, _))) =
                        (&self.display_ext, stereo_view, &self.walk)
                    {
                        let base_draws = draws.len();
                        if let Some(vis) = self.soldier_vis.as_ref() {
                            vis.tracked_draws(walk, view.hand_mounts, &mut draws);
                            if let Some((class, at)) = view.grenade {
                                vis.tracked_grenade_draws(walk, class, at, &mut draws);
                            }
                        }
                        if let (Some(quad), Some(markers)) =
                            (self.fx_quad, self.world_markers.as_ref())
                        {
                            draws.extend(world_objective_draws(walk, &view.camera, quad, markers));
                        }
                        let result = ext.draw_stereo_eyes(r, &view, &draws, &skinned, clear);
                        draws.truncate(base_draws);
                        if let Err(error) = result {
                            self.failure = Some(error);
                            event_loop.exit();
                            return;
                        }
                        true
                    } else {
                        false
                    };
                    r.capture_flat_frame(
                        !ext_stereo && self.display_ext.as_ref().is_some_and(|ext| ext.has_frame()),
                    );
                    if !ext_stereo {
                        if let (Some(vis), Some((walk, _))) =
                            (self.soldier_vis.as_ref(), &self.walk)
                        {
                            let (arms, gun) = vis.first_person_draws(walk, &camera);
                            skinned.extend(arms);
                            draws.extend(gun);
                        }
                    }
                    // The PS2's spawn screens (the HUD pass's, card SSELB):
                    // the soldier standing in the camera shot.
                    let page = spawn
                        .as_ref()
                        .and_then(|s| s.page.as_ref())
                        .filter(|_| self.hud_view.is_some() && !self.cheats.hud_hidden);
                    if let Some(vis) = self.soldier_vis.as_ref() {
                        let step = if frozen { 0.0 } else { dt };
                        let (body, gun) = self.spawn_preview.draws_live(
                            r,
                            vis,
                            page,
                            &camera,
                            step,
                            |r, team, class, model, dt| {
                                let c = self
                                    .walk
                                    .as_ref()?
                                    .0
                                    .sim
                                    .classes
                                    .get(usize::from(team))?
                                    .get(usize::from(class))?
                                    .droideka
                                    .as_deref()?;
                                Some(self.deka_vis.preview_draws(r, team, class, c, model, dt))
                            },
                        );
                        skinned.extend(body);
                        draws.extend(gun);
                    }
                    // The tick's own pose back (card RINT): the menu's
                    // actions and the next tick read it.
                    if let Some((walk, _)) = self.walk.as_mut() {
                        draw_out(&mut self.netplay, walk);
                    }
                    let hud_hidden = self.cheats.hud_hidden;
                    let pad_warning = self.pad_warning.clone();
                    let snapshot_notice = self
                        .snapshot_notice
                        .filter(|(_, shown)| shown.elapsed().as_secs_f32() < 3.0)
                        .map(|(saved, _)| saved);
                    // BF2's own HUD under the egui lines and the spawn
                    // screen (dead, it is the counts and the message box:
                    // the line that says who killed you); the `hud` cheat
                    // hides it. It is fed either way — a model's event
                    // lines are handed over once. Paused, its fades hold.
                    let (w, h) = r.size();
                    let hud_dt = if self.paused && self.netplay.is_none() {
                        0.0
                    } else {
                        dt
                    };
                    let end_movie = self.walk.as_ref().is_some_and(|(w, _)| {
                        w.round_end
                            .as_ref()
                            .is_some_and(|r| r.playing_movie().is_some())
                    });
                    let mut hud_view = match (&mut self.hud_view, &hud) {
                        (Some(hv), Some(model)) => {
                            hv.set_icons(icons);
                            if end_movie {
                                hv.update_end_movie([w, h]);
                                Some(hv)
                            } else if let Some(m) = menu.as_ref().filter(|_| native_pause) {
                                hv.update_pause(model, m, &self.pause_menu.animation, [w, h]);
                                Some(hv)
                            } else if let (Some(selected), Some(c)) = (
                                self.campaign_map.filter(|_| !campaign_popup_up),
                                self.walk.as_ref().and_then(|(w, _)| w.campaign.as_ref()),
                            ) {
                                let page = ui::hud_draw::campaign::Model {
                                    selected,
                                    objectives: c
                                        .objectives()
                                        .iter()
                                        .map(|o| ui::hud_draw::campaign::Objective {
                                            text: self.feed.localize_or_raw(&o.text),
                                            active: o.state == 2,
                                        })
                                        .collect(),
                                    footer: [
                                        "game.objectives.showdetails",
                                        "game.objectives.showhints",
                                        "game.mapdisplay.pressToReturn",
                                    ]
                                    .map(|key| self.feed.localize_or_raw(key)),
                                };
                                hv.update_campaign_map(model, &page, [w, h]);
                                Some(hv)
                            } else {
                                if ext_stereo {
                                    hv.update_without_reticle(model, hud_dt, [w, h]);
                                } else {
                                    hv.update(model, hud_dt, [w, h]);
                                }
                                game::round_end::hud_pass_drawn(
                                    model.round_end.is_some(),
                                    hud_hidden,
                                )
                                .then_some(hv)
                            }
                        }
                        _ => None,
                    };
                    if let Some(p) = self.frame_profile.as_mut() {
                        p.lap("draws_ms");
                    }
                    // Low Power Resolution scales the world while the HUD draws at window resolution.
                    // Automatic quality (LAN §8b.2, `game::quality_governor`)
                    // when on; off, Low Power Resolution's own choice. Both
                    // move only the renderer, never the sim.
                    let auto = self.profile.settings.auto_quality;
                    let low_power = self.profile.settings.low_power_resolution;
                    let in_battle = self.walk.is_some();
                    let budget = (in_battle && (auto || low_power)).then(|| {
                        let window = &self.window;
                        *self.dyn_res_budget_ms.get_or_insert_with(|| {
                            game::dyn_res::display_budget_ms(
                                window
                                    .as_ref()
                                    .and_then(|w| w.current_monitor())
                                    .and_then(|m| m.refresh_rate_millihertz()),
                            )
                        })
                    });
                    let scale = match budget {
                        Some(budget) if auto => {
                            use game::quality_governor::TierHook;
                            self.dyn_res = game::dyn_res::DynRes::default();
                            let tiers = self.tiers.steps();
                            // A joiner replaying the round is busy by design:
                            // its frames say nothing about the GPU.
                            let replaying = self.netplay.as_ref().is_some_and(|n| n.catching_up());
                            // The frame's own work is its time less the last
                            // wait for the window's image, once this platform
                            // has shown it waits there (a frame with time to
                            // spare under vsync does);
                            // where it never does, the governor is told
                            // nothing and treats every slow frame as the
                            // GPU's (`quality_governor`, CPU-bound rule).
                            let wait = r.acquire_ms() as f32;
                            self.acquire_seen |= wait > 2.0;
                            let busy = self
                                .acquire_seen
                                .then(|| (elapsed * 1000.0 - wait).fmax(0.0));
                            let q = if replaying || frozen {
                                self.governor.quality(tiers)
                            } else {
                                self.governor.frame(elapsed * 1000.0, busy, budget, tiers)
                            };
                            self.tiers.apply(q.tier_down);
                            self.quality_note = Some(q.label());
                            q.scale
                        }
                        Some(budget) => {
                            self.governor = Default::default();
                            self.quality_note = None;
                            self.dyn_res.frame(dt * 1000.0, budget)
                        }
                        None => {
                            if !auto {
                                self.governor = Default::default();
                            }
                            self.quality_note = None;
                            self.dyn_res = game::dyn_res::DynRes::default();
                            self.dyn_res_budget_ms = None;
                            1.0
                        }
                    };
                    r.set_world_scale(scale);
                    let mut binding_click = None;
                    let net_meter = self
                        .netplay
                        .as_ref()
                        .filter(|_| self.profile.settings.network_meter)
                        .map(|n| n.meter.clone());
                    let net_banner = self
                        .netplay
                        .as_ref()
                        .and_then(|n| n.banner_text().map(str::to_owned));
                    // Not on the spawn screens (Select is the previous
                    // class there), under the menu or at the round's end.
                    let scoreboard = match (&self.walk, &hud) {
                        (Some((walk, _)), Some(model))
                            if walk.campaign.is_none()
                                && self.score_hold.shown()
                                && spawn.is_none()
                                && menu.is_none()
                                && model.round_end.is_none() =>
                        {
                            Some(scoreboard_model(
                                walk,
                                &self.feed,
                                model.team_names.clone(),
                                self.netplay.as_ref(),
                            ))
                        }
                        _ => None,
                    };
                    // A LAN round: the other players' names over their
                    // heads, seen from this camera (LAN §5b).
                    let name_tags = match (&self.walk, &self.netplay, &hud) {
                        (Some((walk, _)), Some(n), Some(model))
                            if spawn.is_none() && model.round_end.is_none() =>
                        {
                            let roster = n.roster();
                            let name = |slot: u8| {
                                roster
                                    .iter()
                                    .find(|p| p.slot == slot)
                                    .map(|p| p.name.clone())
                            };
                            let tags = self.name_tags.update(
                                &walk.sim,
                                walk.player,
                                camera.position,
                                dt,
                                &name,
                            );
                            Some((tags, camera.view_proj(w as f32 / h.max(1) as f32)))
                        }
                        _ => {
                            self.name_tags.clear();
                            None
                        }
                    };
                    let result = match (&mut self.overlay, &self.window, hud) {
                        (Some(overlay), Some(window), Some(model)) => {
                            let mut clicked: Vec<ui::menu::MenuAction> = Vec::new();
                            let paint = overlay.begin(window, |ctx| {
                                ui::menu::draw_fps_counter_note(
                                    ctx,
                                    ui::menu::FpsCounter::from_index(
                                        self.profile.settings.fps_counter,
                                    ),
                                    self.frame_seconds,
                                    self.quality_note.as_deref(),
                                );
                                if let Some((tags, vp)) = name_tags.as_ref().filter(|_| !hud_hidden)
                                {
                                    lan::draw_name_tags(ctx, tags, *vp);
                                }
                                if !hud_hidden && !native_pause {
                                    ui::hud::draw_hud(ctx, &model);
                                    if let Some(s) = &spawn {
                                        // The PS2's screen is the HUD pass's;
                                        // without one the panel stands in.
                                        match hud_view.is_some() {
                                            true => ui::spawn::draw_spawn(ctx, s),
                                            false => ui::spawn::draw_spawn(
                                                ctx,
                                                &ui::spawn::SpawnModel {
                                                    page: None,
                                                    ..s.clone()
                                                },
                                            ),
                                        }
                                    }
                                }
                                if let Some(m) = menu.as_ref().filter(|_| !native_pause) {
                                    if let Some(b) = m.bindings.as_ref() {
                                        binding_click = ui::menu::draw_controls(
                                            ctx,
                                            &b.labels,
                                            m.cursor,
                                            b.capture,
                                            b.message.as_deref(),
                                        );
                                    } else {
                                        clicked = ui::menu::draw_menu(ctx, m);
                                    }
                                }
                                if let Some(w) = &pad_warning {
                                    ui::hud::draw_pad_warning(ctx, w);
                                }
                                if let Some(saved) = snapshot_notice {
                                    draw_snapshot_notice(ctx, saved);
                                }
                                if let Some(m) = &net_meter {
                                    lan::draw_meter(ctx, m, self.quality_note.as_deref());
                                }
                                if let Some(t) = &net_banner {
                                    lan::draw_banner(ctx, t);
                                }
                                if let Some(b) = &scoreboard {
                                    ui::scoreboard::draw_scoreboard(ctx, b);
                                }
                            });
                            let (n, bytes) = paint.texture_uploads();
                            if n > 0 {
                                render::made::note("egui texture", Some("egui"), bytes, 0.0);
                            }
                            if let Some((walk, _)) = self.walk.as_mut() {
                                // A click sounds as the pad's press of it.
                                let none = ui::menu::MenuInput::default();
                                if let (Some(c), Some(sl)) = (
                                    game::pause::pause_sound(none, 0, 0, &clicked),
                                    self.sounds.as_mut(),
                                ) {
                                    sl.play_2d(self.audio.as_mut(), c.name());
                                }
                                for a in clicked {
                                    if let ui::menu::MenuAction::EditBindings(device, context) = a {
                                        self.controls = Some(super::front::Controls::new(
                                            device,
                                            context,
                                            &self.profile.settings,
                                        ));
                                    }
                                    self.restart_clicked |= a == ui::menu::MenuAction::Restart;
                                    if let ui::menu::MenuAction::SetWindowMode(m) = a {
                                        self.display.requested = Some(m);
                                    }
                                    if self.netplay.as_mut().is_some_and(|n| {
                                        n.pause_action(
                                            walk,
                                            a,
                                            &mut self.paused,
                                            &mut self.pause_menu,
                                            &mut self.profile,
                                            &mut self.session,
                                        )
                                    }) {
                                        continue;
                                    }
                                    self.quit_clicked |= apply_pause_action(
                                        walk,
                                        a,
                                        &mut self.paused,
                                        &mut self.pause_menu,
                                        &mut self.cheats,
                                        &mut self.profile,
                                        &mut self.session,
                                        &mut self.kbm,
                                        self.music_voice.as_mut(),
                                    );
                                    if matches!(a, ui::menu::MenuAction::SetRumble(_)) {
                                        game::rumble::preview(
                                            self.gilrs.as_mut(),
                                            &self.active_pad,
                                            &mut self.rumble_effect,
                                            self.profile.settings.rumble_strength,
                                        );
                                    }
                                }
                            }
                            let draw_overlay: render::OverlayFn<'_> =
                                &mut |dev, q, enc, view, size| {
                                    if let Some(hv) = hud_view.as_mut() {
                                        hv.paint(dev, q, enc, view, size);
                                    }
                                    overlay.paint(&paint, dev, q, enc, view, size)
                                };
                            super::xr::render_battle(
                                r,
                                &camera,
                                &draws,
                                &skinned,
                                clear,
                                Some(draw_overlay),
                                self.display_ext.as_deref_mut().filter(|_| ext_stereo),
                            )
                        }
                        _ => super::xr::render_battle(
                            r,
                            &camera,
                            &draws,
                            &skinned,
                            clear,
                            None,
                            self.display_ext.as_deref_mut().filter(|_| ext_stereo),
                        ),
                    };
                    if let Some(p) = self.frame_profile.as_mut() {
                        p.lap("render_ms");
                        let tick = self.walk.as_ref().map_or(0, |w| w.0.sim.tick);
                        p.end(f64::from(dt) * 1000.0, tick, r);
                    }
                    if let Err(error) = result {
                        eprintln!("render: {error:#}");
                        self.failure = Some(error);
                        self.request_close(super::CloseReason::Error);
                        return;
                    }
                    if let Some(sound) =
                        binding_click.and_then(|row| self.activate_control_row(row))
                    {
                        if let Some(lib) = self.sounds.as_mut() {
                            lib.play_2d(self.audio.as_mut(), sound.name());
                        }
                    }
                }
                // Without a renderer the draw window above is skipped.
                if let Some((walk, _)) = self.walk.as_mut() {
                    draw_out(&mut self.netplay, walk);
                }
                if let (Some(at), Some((walk, _))) = (self.quit_at, self.walk.as_ref()) {
                    self.quit_clicked |= walk.sim.tick >= at;
                }
                if std::mem::take(&mut self.restart_clicked) {
                    self.restart_round();
                }
                if std::mem::take(&mut self.net_round_over) && self.netplay.is_some() {
                    self.net_round_finished();
                }
                if self.quit_clicked {
                    self.quit_clicked = false;
                    if self.map.is_some() {
                        self.request_close(super::CloseReason::Quit);
                    } else {
                        self.leave_battle();
                        // The round's end screens were left: the playlist's
                        // next row (§7); a Quit from the pause menu ends it.
                        if !std::mem::take(&mut self.round_over) || !self.next_round() {
                            self.playlist.clear();
                            // `[ours]` Back at the main menu: the session's
                            // cheats all go off (CHEAT2).
                            self.cheats.end();
                        }
                    }
                }
                self.finish_redraw(event_loop);
            }
            _ => {}
        }
    }
}
