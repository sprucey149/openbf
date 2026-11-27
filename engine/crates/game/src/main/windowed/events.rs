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
/// every human by the feed's names (`App.names`: the profile, the split seats'
/// profiles or the LAN session names; issue #194 B4), and a LAN round gives
/// them a ping; single player has no Ping column.
fn scoreboard_model(
    walk: &game::walk::WalkSim,
    feed: &game::event_feed::EventFeed,
    titles: [String; 2],
    net: Option<&lan::NetplaySession>,
) -> ui::scoreboard::Scoreboard {
    let roster = net.map(|n| n.roster()).unwrap_or_default();
    scoreboard_of(walk, feed, titles, net.map(|_| roster.as_slice()))
}

/// [`scoreboard_model`] from the roster alone (`None` off LAN), for tests.
pub(super) fn scoreboard_of(
    walk: &game::walk::WalkSim,
    feed: &game::event_feed::EventFeed,
    titles: [String; 2],
    roster: Option<&[game::netplay::lobby::LobbyPlayer]>,
) -> ui::scoreboard::Scoreboard {
    use ui::scoreboard::Ping;
    let sim = &walk.sim;
    let net = roster;
    let roster = roster.unwrap_or_default();
    let unit = |i: usize| feed.unit_name(sim, sim.handle(i));
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
        game::assets::scope_overlays_to_map(&map);
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
                    let mut walk = walk;
                    let lan_seats = self.lan_split_seats(&walk);
                    self.split = super::split::seat(
                        &mut walk,
                        self.setup.mission.as_ref(),
                        lan_seats.as_deref(),
                        self.profile.settings.viewpoint,
                        self.split_party.as_ref(),
                    );
                    self.say_split_seat_failed(&walk);
                    self.install_seat_profiles(&mut walk);
                    self.name_split_players(&walk);
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
                // Debug splitscreen (`--local-players`, issue #194): the views' cells in the
                // window. `None` for every ordinary game, and then no line below changes.
                let split_cells = self.split_cells();
                let split_on = split_cells.is_some();
                let split_keyboard = self
                    .split
                    .as_ref()
                    .is_some_and(|s| s.seats.keyboard_player().is_some());
                // The local player on the keyboard and mouse (0 when not split): the mouse's look
                // preview below is view 0's only when it is theirs.
                let keyboard_seat = self.keyboard_seat();
                // Players whose controller is gone: their cells show the notice, and nothing of
                // theirs is read (`WalkSim::is_suspended`).
                let mut view_away: Vec<bool> = self.away_flags();
                // Split: each player's own HUD model, by local index (`None` for a player who
                // is away). Presentation only; the scoring feed is `self.feed` alone.
                let mut split_models: Vec<Option<ui::hud::HudModel>> = Vec::new();
                // Split: whether each view's spawn screens are up (issue #194 B8: no name tags
                // there), by local index; view 0's is `spawn` itself.
                let mut split_spawn_up: Vec<bool> = Vec::new();
                let view_aspect = split_cells
                    .as_ref()
                    .map(|c| c[0][2] as f32 / c[0][3].max(1) as f32);
                // Split: what the HUD models project onto, the cell's 4:3 band
                // (`game::split_layout::ui_rect`, the part of the widened field the unwidened
                // camera covers; `ui::hud_draw::Frame::cell`'s `proj`), not the cell, which
                // `view_aspect` stays for `fit_fov_y`.
                let band_aspect = split_cells.as_ref().map(|c| {
                    let band = game::split_layout::ui_rect(c[0]);
                    band[2] as f32 / band[3].max(1) as f32
                });
                let mut menu: Option<ui::menu::MenuModel> = None;
                let mut edges = if self.walk.is_some() {
                    self.menu_edges()
                } else {
                    0
                };
                // Split at the round's end: each player's own menu edges, for their own stats
                // pages (issue #194 B9). Empty otherwise. Read every split frame so a button
                // held as the round ends is not a fresh press on its first frame.
                let seat_edges = if split_on {
                    let edges = self.seat_menu_edges();
                    let ended = self
                        .walk
                        .as_ref()
                        .is_some_and(|(w, _)| w.sim.outcome.is_some());
                    if ended {
                        edges
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
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
                // Split: Start from an unseated pad opens nothing; the opener owns the menu.
                let opener = split_on.then(|| self.pause_opener(self.key_menu_edges & 64 != 0));
                let start = edges & 64 != 0 && (self.paused || !matches!(opener, Some(None)));
                if self.walk.is_some() && self.controls.is_none() && start {
                    if !self.pause_menu.class_picker {
                        self.paused = !self.paused;
                        if self.paused {
                            self.pause_owner = opener.flatten();
                            // Esc with nobody on the keyboard: it opens the menu as the lowest
                            // seat still here (`split::free_keyboard_opener`), the keyboard keeps
                            // driving it, and the menu says the keyboard did.
                            let by_keys = self.key_menu_edges & 64 != 0
                                && self
                                    .split
                                    .as_ref()
                                    .is_some_and(|s| s.seats.keyboard_player().is_none());
                            self.pause_opened_by = self.pause_owner.map(|o| (o, by_keys));
                        }
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
                if !self.paused {
                    self.pause_owner = None;
                    self.pause_opened_by = None;
                }
                self.sync_grab();
                self.sync_pause_audio();
                // Split draws the same Game Menu as single player, over the whole window
                // (`game::split_ui`).
                let native_pause = self.hud_view.as_ref().is_some_and(|v| v.can_draw_pause());
                // Split: each player's own prompt icons, and the menu's are its opener's.
                let split_icons = if split_on {
                    self.split_icons()
                } else {
                    Vec::new()
                };
                // Split: the Game Menu's opener, whose profile its rows edit, and its driver now,
                // whose pad drives it and whose icons it shows (they differ once the opener's pad
                // is gone, `split::menu_driver`).
                let menu_owner = self
                    .pause_opened_by
                    .map(|(o, _)| o)
                    .or(self.pause_owner)
                    .filter(|_| split_on && self.paused);
                let menu_driver = self.pause_owner.filter(|_| split_on && self.paused);
                // Split: the mouse is the keyboard player's; the egui fallback menu's clicks
                // count only when the keyboard drives the menu (issue #194 B10).
                let egui_pointer_ok = !split_on || split_keyboard && self.pause_keyboard_drives();
                // Split: the Game Menu's opener (the keyboard's player when the menu opened some
                // other way, a lost focus).
                let split_pause_owner = self.pause_owner.unwrap_or_else(|| self.keyboard_seat());
                // A LAN round's Game Menu covers only its opener's cell (issue #194 B20).
                let lan_menu_view = self.lan_menu_view();
                let lan_menu_cell = lan_menu_view
                    .and_then(|v| split_cells.as_ref().and_then(|c| c.get(v).copied()));
                let icons = match menu_driver.and_then(|o| split_icons.get(o)) {
                    Some(i) if self.paused => *i,
                    _ => icons,
                };
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
                // Split: every player's own pad was sampled this frame (their boards follow it).
                let mut split_sampled = false;
                let camera = if self.walk.is_some() {
                    // The pointer is the keyboard player's, on their own cell and spawn pages
                    // (`App::step_spawn_pointer`); with nobody on the keyboard it acts on nothing.
                    if !split_on || split_keyboard {
                        self.step_spawn_pointer(dt);
                    }
                    // OpenBF VR drives one player's view: never a split battle's.
                    let ext_infantry = self.netplay.is_none()
                        && !split_on
                        && self
                            .display_ext
                            .as_ref()
                            .zip(self.walk.as_ref())
                            .is_some_and(|(ext, (walk, _))| {
                                ext.is_infantry(&super::xr::sim_view(walk))
                            });
                    let lan_split = split_on && self.netplay.is_some() && !self.autopilot;
                    // Which branch takes this frame's pads (`split::frame_pads`): the round's end
                    // comes ahead of the popups, as an offline round's end holds the game like
                    // one (`round_end_held`) yet split pads still come and go under it.
                    let round_over = self
                        .walk
                        .as_ref()
                        .is_some_and(|(walk, _)| walk.sim.outcome.is_some());
                    use super::split::FramePads;
                    let pad = match super::split::frame_pads(
                        self.paused,
                        lan_split,
                        popup_up,
                        round_over,
                    ) {
                        FramePads::LanMenu => {
                            // A LAN round runs on under the Game Menu (issue #194 B20): the players
                            // who did not open it keep playing on their own pads; the menu's driver
                            // (and the keyboard, when it drives the menu) stands still.
                            self.sample_split_pads();
                            split_sampled = true;
                            view_away = self.away_flags();
                            let driver = self
                                .pause_owner
                                .filter(|_| !self.lan_menu_by_free_keyboard());
                            let keys = self.pause_keyboard_drives();
                            if keys {
                                self.kbm.drop_taps();
                            }
                            if let Some(split) = self.split.as_mut() {
                                let keyboard = split.seats.keyboard_player();
                                for i in 0..split.pads.len() {
                                    if Some(i) == driver || (keys && keyboard == Some(i)) {
                                        game::pause::hold_for_resume(
                                            &mut split.inputs[i].resume_gate,
                                        );
                                        split.pads[i] = PadState::default();
                                        if let Some(l) = split.looks.get_mut(i) {
                                            *l = PadState::default();
                                        }
                                    }
                                }
                            }
                            // The first player's pad: the session makes it neutral while paused
                            // (`frame_pad`), so it is taken from the seats here.
                            self.split
                                .as_ref()
                                .and_then(|s| s.pads.first().copied())
                                .unwrap_or_default()
                        }
                        FramePads::Paused => {
                            self.kbm.drop_taps();
                            game::pause::hold_for_resume(&mut self.resume_gate);
                            if let Some(split) = self.split.as_mut() {
                                for p in &mut split.inputs {
                                    game::pause::hold_for_resume(&mut p.resume_gate);
                                }
                            }
                            PadState::default()
                        }
                        FramePads::RoundEnd => {
                            // Results and the banner use menu_edges only. Continue the
                            // scene with a neutral pad so the D-pad never gives orders.
                            self.kbm.drop_taps();
                            // Split: pads still come and go, so a player whose pad returns during
                            // the stats is seated and gets their own pages (issue #194 B9).
                            if split_on && !self.autopilot {
                                self.seat_split_pads();
                                view_away = self.away_flags();
                            }
                            PadState::default()
                        }
                        FramePads::Popup => {
                            self.kbm.drop_taps();
                            // The press that closes a popup is not also a jump:
                            // whatever is still down waits for its release.
                            game::pause::hold_for_resume(&mut self.resume_gate);
                            PadState::default()
                        }
                        FramePads::Play if split_on && !self.autopilot => {
                            // Each player's own pad (`split.rs`); the first one is this frame's.
                            self.sample_split_pads();
                            split_sampled = true;
                            view_away = self.away_flags();
                            if let Some(bb) = self.blackbox.as_mut() {
                                bb.stick(self.stick_sample.0, self.stick_sample.1);
                            }
                            self.split
                                .as_ref()
                                .map_or(PadState::default(), |s| s.pads[0])
                        }
                        FramePads::Play => {
                            let walk = &self.walk.as_ref().expect("battle walk").0;
                            let on_foot = walk.player_on_foot();
                            let space = walk.space_battle();
                            let pad = match self.autopilot {
                                true => {
                                    autopilot_pad(self.walk.as_ref().map_or(0, |w| w.0.sim.tick))
                                }
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
                                    let pad = self
                                        .display_ext
                                        .as_ref()
                                        .filter(|_| self.netplay.is_none())
                                        .map_or(pad, |ext| {
                                            let walk = &self.walk.as_ref().expect("battle walk").0;
                                            ext.seat_sticks(&super::xr::sim_view(walk), pad, look)
                                        });
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
                        }
                    };
                    if !split_on && self.view_toggle.pressed(&pad) && !ext_infantry {
                        let s = &mut self.profile.settings;
                        s.viewpoint = s.viewpoint.toggled();
                        if let Err(e) = self.profile.save() {
                            eprintln!("profile autosave failed: {e:#}");
                        }
                    }
                    // Split: each player's own viewpoint is stamped with the sim's pads.
                    let pad = if split_on {
                        pad
                    } else if ext_infantry {
                        game::viewpoint::Viewpoint::First.stamp(pad)
                    } else {
                        self.profile.settings.viewpoint.stamp(pad)
                    };
                    // The scoreboard is held on Select / View (never with the
                    // seat chord) or Tab, read as the key (LAN §5b, #182).
                    game::kbm::step_scoreboard(&mut self.score_hold, &self.kbm, &pad, dt);
                    // Split: each player's board is held on their own pad (issue #194 B9).
                    if split_on {
                        self.step_split_score_holds(split_sampled, dt);
                    }
                    // A LAN round: the pad is neutral while paused (the sim
                    // runs on), and carries a confirmed Respawn (issue #168).
                    // Split: the first player's pad is neutral only when they drive the menu
                    // (`lan_split` above sampled everyone else's).
                    let paused_now = self.paused
                        && !(lan_split
                            && (self.pause_owner.is_some_and(|o| o != 0)
                                || self.lan_menu_by_free_keyboard()));
                    let pad = match self.netplay.as_mut() {
                        Some(n) => n.frame_pad(pad, paused_now),
                        None => pad,
                    };
                    let paused = self.paused;
                    // Split: the Game Menu names its owner by profile name: the opener, or
                    // whoever it passed to once the opener's pad is gone.
                    let paused_by = self.paused_by_line(menu_owner, menu_driver);
                    // Split: the Splitscreen page's rows by player name (issue #194 B22).
                    let split_names = self
                        .split
                        .as_ref()
                        .filter(|_| split_on)
                        .map(|s| self.split_seat_names(&s.seats))
                        .unwrap_or_default();
                    // Split: the mouse is the keyboard player's; with nobody on it, or the menu
                    // someone else's, its clicks and wheel do nothing there.
                    let pause_pointer_ok =
                        !split_on || split_keyboard && self.pause_keyboard_drives();
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
                            // A LAN round: the opener's own slot's (issue #194).
                            cheats: match self.netplay.as_mut() {
                                Some(n) => n.cheat_entries(
                                    walk,
                                    super::menu_actions::opener_slot(walk, menu_owner),
                                ),
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
                            // Split: Respawn is the opener's own.
                            respawn_label: match menu_owner.filter(|o| *o > 0) {
                                Some(o) => walk
                                    .with_local(o, |w| game::pause::respawn_label(w, &self.feed)),
                                None => game::pause::respawn_label(walk, &self.feed),
                            },
                            in_shell: None,
                            paused_by: paused_by.clone(),
                            // Issue #194: a split match's Options has the Splitscreen page.
                            split: self.split.as_ref().filter(|_| split_on).map(|s| {
                                super::menu_actions::split_menu(
                                    &self.profile.settings.splitscreen,
                                    super::menu_actions::seated_of(&s.seats),
                                )
                            }),
                            split_names: split_names.clone(),
                            outcome_line: walk.sim.outcome.map(|w| match w {
                                core::sim::OUTCOME_DRAW => "Time up — both teams win".to_string(),
                                _ => format!(
                                    "Round over — {} wins",
                                    walk.teams[usize::from(w.min(1))].faction
                                ),
                            }),
                            shared_rows: false,
                            own_cheats: self.netplay.is_some(),
                        };
                        // Split: a player after the first sees their own settings (issue #194 B10).
                        if let (Some(o), Some(s)) = (menu_owner, self.split.as_ref()) {
                            let settings = super::split::seat_settings(
                                &self.profile,
                                &self.split_profiles,
                                &s.seats,
                                o,
                            );
                            let name = self
                                .split_profiles
                                .get(s.seats.seat_number(o))
                                .and_then(Option::as_ref)
                                .map(|p| p.name.as_str());
                            super::menu_actions::opener_model(&mut model, walk, settings, name, o);
                            // The view the opener plays in now, whatever their profile holds.
                            if let Some(input) = s.inputs.get(o) {
                                model.first_person = input.viewpoint().is_first();
                            }
                        }
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
                        let pointer = self.shell_pointer.take_frame().filter(|_| pause_pointer_ok);
                        let wheel = match pause_pointer_ok {
                            true => self.shell_pointer.take_wheel(),
                            false => {
                                self.shell_pointer.take_wheel();
                                0.0
                            }
                        };
                        let from = model.cursor;
                        let target = if native_pause {
                            // The menu's own frame: the window, or a LAN round's opener's cell.
                            let menu_frame = match (lan_menu_cell, self.renderer.as_ref()) {
                                (Some(c), _) => {
                                    Some(([c[0] as f32, c[1] as f32], [c[2] as f32, c[3] as f32]))
                                }
                                (None, Some(r)) => {
                                    let (w, h) = r.size();
                                    Some(([0.0, 0.0], [w as f32, h as f32]))
                                }
                                (None, None) => None,
                            };
                            let target = match (pointer, menu_frame) {
                                (Some((point, true)), Some((at, size))) => ui::pause_draw::target(
                                    &model,
                                    &self.pause_menu.animation,
                                    [point[0] - at[0], point[1] - at[1]],
                                    size,
                                ),
                                _ => None,
                            };
                            if target.is_none() && wheel != 0.0 {
                                if let Some((_, size)) = menu_frame {
                                    ui::pause_draw::scroll(
                                        &model,
                                        &self.pause_menu.animation,
                                        wheel,
                                        size,
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
                            // A LAN round: the word goes to the host (issue #168), for the
                            // opener's own slot (issue #194).
                            Some(n) => self.cheats.word(input).map(|w| {
                                let slot = super::menu_actions::opener_slot(walk, menu_owner);
                                n.morse_word(w, slot, walk, &mut self.cheats)
                            }),
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
                                // Split: the opener's own profile's layouts (issue #194 B10).
                                self.controls = super::split::controls_for(
                                    &self.profile,
                                    &self.split_profiles,
                                    self.split.as_ref(),
                                    menu_owner,
                                    device,
                                    context,
                                );
                                if self.controls.is_some() {
                                    binding_menu = true;
                                } else {
                                    self.feed.say(
                                        super::split::NO_PROFILE_BINDINGS.to_owned(),
                                        ui::hud::message_color::YELLOW,
                                        walk.sim.tick,
                                    );
                                }
                            }
                            self.restart_clicked |= a == ui::menu::MenuAction::Restart;
                            if let ui::menu::MenuAction::SetWindowMode(m) = a {
                                self.display.requested = Some(m);
                            }
                            // Split: the opener's own player takes the action (Respawn
                            // kills their soldier, not the first player's), and a per-player
                            // row writes their own profile; a host row acts as the first
                            // player on the host's (issue #194 B10, `menu_actions::acting_local`).
                            let opener = super::menu_actions::acting_local(menu_owner, a);
                            // A LAN round's cheats are the opener's own slot's (issue #194).
                            let lan_opener = super::menu_actions::lan_acting_local(menu_owner, a);
                            // A LAN round: a further player's Respawn is asked of their own
                            // view in the walk, which the frame puts in their pad
                            // (`local_seats::lan_pads`, issue #194 B20); the session's own takes the
                            // first player's.
                            // A further player's per-player rows write their own profile and
                            // slot (`NetplaySession::opener_pause_action`).
                            if self.netplay.as_mut().is_some_and(|n| {
                                n.opener_pause_action(
                                    walk,
                                    a,
                                    lan_opener,
                                    &mut self.paused,
                                    &mut self.pause_menu,
                                    &mut self.profile,
                                    &mut self.split_profiles,
                                    self.split.as_ref().map(|s| &s.seats),
                                    &mut self.session,
                                )
                            }) {
                                continue;
                            }
                            let act = |w: &mut game::walk::WalkSim| {
                                apply_pause_action(
                                    w,
                                    a,
                                    &mut self.paused,
                                    &mut self.pause_menu,
                                    &mut self.cheats,
                                    &mut self.profile,
                                    &mut self.session,
                                    &mut self.kbm,
                                    &mut self.split_profiles,
                                    self.split.as_mut().map(|s| (&s.seats, &mut s.inputs[..])),
                                    opener,
                                    self.music_voice.as_mut(),
                                )
                            };
                            quit |= walk.with_local(opener, act);
                            if matches!(a, ui::menu::MenuAction::SetRumble(_)) {
                                super::menu_actions::preview_rumble(
                                    self.gilrs.as_mut(),
                                    &self.active_pad,
                                    &mut self.rumble_effect,
                                    &self.profile,
                                    &self.split_profiles,
                                    self.split.as_mut(),
                                    opener,
                                );
                            }
                        }
                        if quit {
                            self.quit_clicked = true;
                        }
                        if !binding_menu {
                            let mut after = ui::menu::MenuModel {
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
                                    Some(n) => n.cheat_entries(
                                        walk,
                                        super::menu_actions::opener_slot(walk, menu_owner),
                                    ),
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
                                split: self.split.as_ref().filter(|_| split_on).map(|s| {
                                    super::menu_actions::split_menu(
                                        &self.profile.settings.splitscreen,
                                        super::menu_actions::seated_of(&s.seats),
                                    )
                                }),
                                split_names: split_names.clone(),
                                shared_rows: false,
                                ..model
                            };
                            if let (Some(o), Some(s)) = (menu_owner, self.split.as_ref()) {
                                let settings = super::split::seat_settings(
                                    &self.profile,
                                    &self.split_profiles,
                                    &s.seats,
                                    o,
                                );
                                let name = self
                                    .split_profiles
                                    .get(s.seats.seat_number(o))
                                    .and_then(Option::as_ref)
                                    .map(|p| p.name.as_str());
                                super::menu_actions::opener_model(
                                    &mut after, walk, settings, name, o,
                                );
                                if let Some(input) = s.inputs.get(o) {
                                    after.first_person = input.viewpoint().is_first();
                                }
                            }
                            menu = Some(after);
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
                            // Split: the first player's points carried over a dropped
                            // controller, as the others' (`WalkSim::first_gate_points`).
                            let points = match split_on {
                                true => walk.first_gate_points(points),
                                false => points,
                            };
                            game::unlocks::set_player_gate(walk, points, &self.profile.career);
                            // Split: every further player's gate from their own seat profile
                            // (issue #194); nothing with one local player.
                            if split_on {
                                game::netplay::gate_locals(walk, Some(self.feed.round()));
                            }
                        }
                        // Station serving is observed for every sim tick, even when a
                        // frame catches up several ticks before posing the scene.
                        if let Some(gpu) = self.gpu.as_mut() {
                            gpu.step_stations(&walk.sim);
                        }
                        // Split: every player's pad, by slot (`SplitState::finish_pads`).
                        let split_pads: Vec<PadState> = match self.split.as_ref() {
                            Some(s) if split_on => s.finish_pads(pad, walk),
                            _ => Vec::new(),
                        };
                        let pads_now: &[PadState] = if split_pads.is_empty() {
                            std::slice::from_ref(&pad)
                        } else {
                            &split_pads
                        };
                        let mut st = TickState::new(walk, self.profile.career.medals, pad, aspect);
                        // Split: each cell's aspect, for the foley's views (issue #194 B12).
                        if let Some(cells) = split_cells.as_ref() {
                            st.view_aspects = cells
                                .iter()
                                .map(|c| c[2] as f32 / c[3].max(1) as f32)
                                .collect();
                        }
                        // Split: the first player's rumble goes to their own pad.
                        let split_pad0 = self.split.as_ref().filter(|_| split_on).map(|s| {
                            let mut a = input::ActivePad::default();
                            a.id = s.seats.pad_of(0);
                            a
                        });
                        let split_feeds = &mut self.split_feeds;
                        let split_rumble = &mut self.split_rumble;
                        let split_pads_ids: Vec<Option<gilrs::GamepadId>> = match &self.split {
                            Some(s) if split_on => {
                                (0..walk.local_count()).map(|i| s.seats.pad_of(i)).collect()
                            }
                            _ => Vec::new(),
                        };
                        if split_on {
                            split_rumble.resize_with(walk.local_count() - 1, Default::default);
                        }
                        // Each player's own rumble settings, from their seat's profile.
                        let split_rumble_cfg: Vec<(f32, bool)> = match &self.split {
                            Some(s) if split_on => (0..walk.local_count())
                                .map(|i| {
                                    let p = super::split::seat_settings(
                                        &self.profile,
                                        &self.split_profiles,
                                        &s.seats,
                                        i,
                                    );
                                    (p.rumble_strength, p.rumble_on_fire)
                                })
                                .collect(),
                            _ => Vec::new(),
                        };
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
                            active_pad: split_pad0.as_ref().unwrap_or(&self.active_pad),
                            rumble_effect: match self.split.as_mut().filter(|_| split_on) {
                                Some(s) => &mut s.inputs[0].rumble_effect,
                                None => &mut self.rumble_effect,
                            },
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
                                // Split (issue #194 B20): every local's own pad, in the
                                // session's local order; one local keeps the frame as it was.
                                let local_pads: Vec<PadState> = if split_on {
                                    game::local_seats::lan_pads(&n.peer, walk, &split_pads)
                                } else {
                                    Vec::new()
                                };
                                let report = if local_pads.is_empty() {
                                    n.view.frame(
                                        &mut n.peer,
                                        &mut n.link,
                                        elapsed.fmin(1.0),
                                        net::bundle::strip_tracked_inputs(pad),
                                        walk,
                                        Some(feed),
                                        |w, _, f, muted| {
                                            if let Some(f) = f.filter(|_| !catching_up) {
                                                hook.tick(w, f, &mut st, true, u8::from(muted));
                                            }
                                        },
                                    )
                                } else {
                                    n.view.frame_locals(
                                        &mut n.peer,
                                        &mut n.link,
                                        elapsed.fmin(1.0),
                                        &local_pads,
                                        walk,
                                        Some(feed),
                                        |w, _, f, muted| {
                                            let Some(f) = f.filter(|_| !catching_up) else {
                                                return;
                                            };
                                            hook.tick(w, f, &mut st, true, muted);
                                            // Each further player's own feed reads their unit
                                            // for their messages; the round's one feed scored
                                            // every human (`netplay::score`), so nothing here
                                            // writes the sim. Their own pads rumble.
                                            for k in 1..w.local_count() {
                                                if w.is_suspended(k) {
                                                    continue;
                                                }
                                                let unit = w.local_unit(k);
                                                if let Some(sf) = split_feeds.get_mut(k - 1) {
                                                    sf.tick(&w.sim, unit);
                                                }
                                                if let Some(r) = split_rumble.get_mut(k - 1) {
                                                    let cfg = split_rumble_cfg
                                                        .get(k)
                                                        .copied()
                                                        .unwrap_or_default();
                                                    r.tick(w, k, &mut hook, &split_pads_ids, cfg);
                                                }
                                            }
                                            super::split::note_split_medals(
                                                w,
                                                f,
                                                &mut st.medals_seen,
                                                split_feeds,
                                                &mut st.views_seen,
                                            );
                                        },
                                    )
                                };
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
                                // Split: the keyboard player's own pad gives back their
                                // mouse look (as offline below).
                                let fed = match self.split.as_ref() {
                                    Some(s) if split_on => super::split::keyboard_pad(
                                        s.seats.keyboard_player(),
                                        &s.pads,
                                        pad,
                                    ),
                                    _ => pad,
                                };
                                self.kbm.consumed(&fed, 1);
                                walk.blend.consumed(1);
                                n.update_meter(Some(walk), report.ticks);
                                if let Some(bb) = self.blackbox.as_mut() {
                                    bb.net(walk.sim.tick, &n.meter);
                                }
                                // Own shot feedback from the prediction, at once: every
                                // local player's of this machine (issue #194 B20), each
                                // muted in the confirmed bundle's sounds (`hook.tick`).
                                let mut shots = n.view.shots().to_vec();
                                for k in 1..walk.local_count() {
                                    if !walk.is_suspended(k) {
                                        shots.extend_from_slice(
                                            n.view.shots_of_slot(walk.local_slot_of(k)),
                                        );
                                    }
                                }
                                let shots = &shots[..];
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
                                        let listener = game::event_sounds::listeners(walk);
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
                                let ticks = walk.advance_while(dt, pads_now, |walk| {
                                    hook.tick(walk, feed, &mut st, false, 0);
                                    // Split: the other players' feeds read their own unit for
                                    // their messages, and the positive points each returns feed
                                    // that player's medal career (as `heroes::tick` does the
                                    // first's; issue #194). The round's scores stay `feed`'s.
                                    for k in 1..walk.local_count() {
                                        if let (Some(f), false) =
                                            (split_feeds.get_mut(k - 1), walk.is_suspended(k))
                                        {
                                            let unit = walk.local_unit(k);
                                            let points = f.tick(&walk.sim, unit);
                                            walk.sim.score_medal_points(unit, points);
                                        }
                                        // Each seated player's own pad rumbles for their unit.
                                        if let Some(r) = split_rumble.get_mut(k - 1) {
                                            if !walk.is_suspended(k) {
                                                let cfg = split_rumble_cfg
                                                    .get(k)
                                                    .copied()
                                                    .unwrap_or_default();
                                                r.tick(walk, k, &mut hook, &split_pads_ids, cfg);
                                            }
                                        }
                                    }
                                    // Every view's medal lines, after all the points are in:
                                    // a later player's medal reaches the earlier views this
                                    // tick (issue #194 B4).
                                    if walk.local_count() > 1 {
                                        super::split::note_split_medals(
                                            walk,
                                            feed,
                                            &mut st.medals_seen,
                                            split_feeds,
                                            &mut st.views_seen,
                                        );
                                    }
                                    !walk.campaign.as_ref().is_some_and(|c| c.has_movies())
                                });
                                // The keyboard player's own pad gives back their mouse look.
                                let fed = match self.split.as_ref() {
                                    Some(s) if split_on => super::split::keyboard_pad(
                                        s.seats.keyboard_player(),
                                        &s.pads,
                                        pad,
                                    ),
                                    _ => pad,
                                };
                                self.kbm.consumed(&fed, ticks);
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
                        // A LAN round's further players flush into their own lent profiles even
                        // on one view (issue #194 B20).
                        if split_on || (self.netplay.is_some() && walk.local_count() > 1) {
                            let seats = super::split::round_seat_numbers(
                                self.split.as_ref(),
                                self.netplay.as_ref(),
                                self.split_party.as_ref(),
                                walk,
                            );
                            super::career::flush_split_careers(
                                &mut self.split_profiles,
                                walk,
                                &seats,
                            );
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
                        look = if popup_up || (split_on && !split_keyboard) {
                            self.kbm.clear_look();
                            PadState::default()
                        } else if split_on && keyboard_seat != 0 {
                            // The mouse is a later player's: their pad took its look.
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
                            rain_splashes(
                                w,
                                &mut self.fx,
                                walk,
                                aspect,
                                split_cells.as_deref(),
                                world_dt,
                            );
                        }
                        if let (Some(audio), Some(sl)) = (self.audio.as_mut(), self.sounds.as_mut())
                        {
                            // The commander: this frame's requests join the
                            // voice queue, and an idle voice starts its head.
                            // A squad answer plays the take drawn for it,
                            // at its unit.
                            // Every awake local player's ears; the voice follows the first
                            // awake one (issue #194 B12; `first_awake_unit` is the walk's own
                            // player with one local player, and `None` while everyone is away:
                            // the voice keeps its last side). OpenBF VR: the headset's ears.
                            let ears = self
                                .display_ext
                                .as_ref()
                                .filter(|_| self.netplay.is_none() && walk.local_count() == 1)
                                .and_then(|ext| ext.stereo_view(&super::xr::sim_view(walk)))
                                .map(|view| super::xr::listener(&view))
                                .unwrap_or_else(|| game::event_sounds::listeners(walk));
                            let voice_unit = walk.first_awake_unit();
                            if let Some(v) = walk.voice.as_mut() {
                                let tick = walk.sim.tick;
                                // The units' combat calls through the
                                // bank's gates join the queue first.
                                v.gate_calls(sl, &ears);
                                for name in v.gate_script_sounds(sl) {
                                    sl.play_voice(audio, &name);
                                }
                                v.frame(&walk.sim, voice_unit, dt, |line| {
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
                        // Split: the music follows the first local player who is not away
                        // (issue #194 B12); the walk's own player with one local player. With
                        // everyone away (`None`) it keeps the last side heard.
                        let music_unit = walk.first_awake_unit();
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
                            for c in m.frame(&walk.sim, music_unit, dt) {
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
                    if !self.paused
                        && self.netplay.is_none()
                        && !split_on
                        && super::xr::live_view(walk)
                    {
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
                        super::split::set_ctf_viewer(&mut self.vehicle_vis, walk);
                        self.vehicle_vis.set_flag_ribcage(
                            walk.sim
                                .ctf
                                .as_ref()
                                .and(self.soldier_vis.as_ref())
                                .map(|v| v.flag_ribcage_frames(walk)),
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
                        let aspect = view_aspect.unwrap_or_else(|| {
                            self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                                let (w, h) = r.size();
                                w as f32 / h as f32
                            })
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
                        // Split: each player's pages follow their own pad (issue #194 B9); the
                        // first player's shared round end takes P1's own presses, not any pad's.
                        let seat_pads: Vec<ui::stats::Pad> = seat_edges
                            .iter()
                            .map(|&e| super::split::stats_pad(e))
                            .collect();
                        let pad = match seat_pads.first() {
                            Some(&p0) if split_on => p0,
                            _ => super::split::stats_pad(edges),
                        };
                        let pad = super::xr::stats_pointer(
                            pad,
                            walk,
                            self.display_ext.as_deref().filter(|_| !split_on),
                            self.renderer.as_ref(),
                            self.hud_view.as_ref(),
                            &mut self.shell_pointer,
                        );
                        // Any player may skip the end movie.
                        let skip = seat_pads
                            .iter()
                            .chain(std::iter::once(&pad))
                            .any(|p| p.cross || p.triangle);
                        let music_secs = walk.music.as_ref().and_then(|m| {
                            let team = walk.sim.units.get(walk.player.index as usize)?.team;
                            m.end_music_secs(usize::from(team), walk.sim.outcome?)
                        });
                        let credits_after_outro = self.netplay.is_none()
                            && self.active_campaign
                                == Some(ui::campaign::RISE_OF_THE_EMPIRE.len() - 1)
                            && walk.sim.outcome == Some(0);
                        let (mut done, mut cue) =
                            walk.follow_round_end(dt, pad, music_secs, !credits_after_outro);
                        let first_away = split_on && walk.is_suspended(0);
                        if let (true, Some(r), Some(split)) =
                            (split_on, walk.round_end.as_mut(), self.split.as_mut())
                        {
                            let n = split.stats.len().min(seat_pads.len());
                            if n > 1 {
                                let views = r.step_views(
                                    &seat_pads[1..n],
                                    &mut split.stats[1..n],
                                    first_away,
                                );
                                cue = cue.or(views);
                                done |= r.done();
                            }
                        }
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
                                skip,
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
                            let players = walk.local_count();
                            if let Some(r) = walk.round_end.as_mut() {
                                r.stats = Some(pages);
                                // Split: the stats end once every player has left their own
                                // pages, not on the first Done (issue #194).
                                if split_on {
                                    r.hold_for_players(players);
                                }
                            }
                            // Split: every further player's own pages (issue #194 B9).
                            if let (true, Some(split)) = (split_on, self.split.as_mut()) {
                                split.stats = vec![None; players];
                            }
                        }
                        // Split, pages up: make any further player's that are missing (made
                        // now, or that player was away and is back) (issue #194 B9).
                        if let (true, Some(split), true) = (
                            split_on,
                            self.split.as_mut(),
                            walk.round_end
                                .as_ref()
                                .is_some_and(|r| r.stats.is_some() && !r.done()),
                        ) {
                            let seats: Vec<usize> = (0..walk.local_count())
                                .map(|k| split.seats.seat_number(k))
                                .collect();
                            super::split::fill_split_stats_pages(
                                walk,
                                &self.split_feeds,
                                &self.split_profiles,
                                &seats,
                                &mut split.stats,
                            );
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
                        let seats = super::split::round_seat_numbers(
                            self.split.as_ref(),
                            self.netplay.as_ref(),
                            self.split_party.as_ref(),
                            walk,
                        );
                        super::career::record_split_rounds(
                            &mut self.split_profiles,
                            walk,
                            &self.split_feeds,
                            &seats,
                        );
                        // A LAN round: the next round seats each further player on their
                        // profile as this one left it (issue #194 B20).
                        if let Some(n) = self.netplay.as_mut() {
                            n.refresh_local_profiles(&self.split_profiles);
                        }
                    }
                    // The tick hook above has read the events; the model
                    // carries the new lines to the HUD's message box.
                    let hud_aspect = band_aspect.unwrap_or_else(|| {
                        self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                            let (w, h) = r.size();
                            w as f32 / h.max(1) as f32
                        })
                    });
                    // A suspended first player's handle may name a bot: no HUD model reads it.
                    hud = Some(if view_away.first().copied().unwrap_or(false) {
                        ui::hud::HudModel::default()
                    } else {
                        let stereo_cam = stereo_view.as_ref().map(|v| (&v.camera, v.aspect));
                        hud_model_for_view(walk, &mut self.feed, hud_aspect, stereo_cam)
                    });
                    if split_on {
                        let feeds = &mut self.split_feeds;
                        split_models.push(
                            hud.clone()
                                .filter(|_| !view_away.first().copied().unwrap_or(false)),
                        );
                        split_spawn_up.push(false);
                        let stats = self.split.as_ref().map(|s| &s.stats[..]);
                        for (model, spawn_up) in super::split::further_view_models(
                            walk,
                            feeds,
                            &view_away,
                            hud_aspect,
                            stats.unwrap_or(&[]),
                        ) {
                            split_models.push(model);
                            split_spawn_up.push(spawn_up);
                        }
                    }
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
                    for m in split_models.iter_mut().flatten().filter(|_| paused) {
                        m.capture = None;
                    }
                    // Spawn screen while dead: the post and class it shows
                    // are the sim's (card SPSEL), moved by the pad bits.
                    spawn =
                        spawn_model(walk, &self.feed).map(|s| ui::spawn::SpawnModel { icons, ..s });
                    if let Some(up) = split_spawn_up.first_mut() {
                        *up = spawn.is_some();
                    }
                    let mut camera = self.fx.shake.apply(walk.camera());
                    // Split: a cell narrower than 4:3 keeps a 4:3 screen's width of view.
                    if let Some(aspect) = view_aspect {
                        camera.fov_y = game::split_layout::fit_fov_y(camera.fov_y, aspect);
                    }
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
                // Split: the cells of players who have left their stats pages, with the names
                // the round still waits for (issue #194).
                let split_waiting = match split_cells.as_ref() {
                    Some(cells) => self.split_waiting(cells, &view_away),
                    None => Vec::new(),
                };
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
                                    let mut points = v.flash_fire_points(walk);
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
                                // Split: view 0's rain at its cell's aspect and height (B13).
                                particles.extend(match split_cells.as_deref() {
                                    Some([c, ..]) => w.frame_draws_sized(
                                        r,
                                        &camera,
                                        tracked_velocity(walk),
                                        [c[2], c[3]],
                                    ),
                                    _ => w.frame_draws(r, &camera, tracked_velocity(walk)),
                                });
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
                    let mut reticle_at = None;
                    let ext_stereo = if let (Some(ext), Some(view), Some((walk, _))) =
                        (&self.display_ext, stereo_view, &self.walk)
                    {
                        let base_draws = draws.len();
                        reticle_at =
                            super::xr::reticle_point(&view, walk, self.soldier_vis.as_ref());
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
                        !ext_stereo
                            && !split_on
                            && self.display_ext.as_ref().is_some_and(|ext| ext.has_frame()),
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
                        // Split: the soldiers stand in P1's UI rectangle, not the window.
                        let ui_view = split_cells
                            .as_ref()
                            .and_then(|c| c.first())
                            .map(|cell| game::split_layout::ui_view(camera.fov_y, *cell));
                        self.spawn_preview.set_view_aspect(ui_view.map(|v| v.1));
                        let preview_camera = render::Camera {
                            fov_y: ui_view.map_or(camera.fov_y, |v| v.0),
                            ..camera
                        };
                        let (body, gun) = self.spawn_preview.draws_live(
                            r,
                            vis,
                            page,
                            &preview_camera,
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
                    // Issue #194 B8: every view's camera, once per frame and outside any blend
                    // (`game::render_blend::view_camera`); each further view's draw lists and every
                    // view's name tags read it. View 0 keeps the camera above (its shake
                    // included); each further view turns in its own shake (B13,
                    // `Fx::shake_of`). One view: that camera alone.
                    let view_cameras: Vec<render::Camera> =
                        match (split_cells.as_deref(), self.walk.as_mut()) {
                            (Some(cells), Some((walk, _))) => {
                                let looks = self
                                    .split
                                    .as_ref()
                                    .map(|s| s.looks.clone())
                                    .unwrap_or_default();
                                let fx = &self.fx;
                                let mut lan_view = self.netplay.as_mut().map(|n| &mut n.view);
                                (0..cells.len())
                                    .map(|i| match i {
                                        0 => camera,
                                        // Nothing of an away player's is read.
                                        _ if view_away.get(i).copied().unwrap_or(false) => {
                                            render::Camera::default()
                                        }
                                        // A LAN round: their look over their pending inputs
                                        // and their predicted pose (B20).
                                        _ if lan_view.is_some() => {
                                            game::render_blend::view_camera_with(
                                                walk,
                                                i,
                                                cells[i],
                                                fx.shake_of(i),
                                                lan_view.as_deref_mut().expect("LAN view"),
                                            )
                                        }
                                        _ => game::render_blend::view_camera(
                                            walk,
                                            i,
                                            cells[i],
                                            &looks.get(i).copied().unwrap_or_default(),
                                            // Their own shake (B13).
                                            fx.shake_of(i),
                                        ),
                                    })
                                    .collect()
                            }
                            _ => vec![camera],
                        };
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
                    // The other players' names over their heads (LAN §5b; split, issue #194 B8),
                    // each view's seen from its own camera, handed to that view's HUD model: the
                    // HUD pass draws them in the event feed's font under its panels (B22). Shown
                    // only where the HUD itself shows: not with the `hud` cheat, under a split
                    // Game Menu, behind a spawn screen, for an away player or at the round's end.
                    // A LAN Game Menu lays out the menu, not the HUD, so they hide under it too.
                    match (&self.walk, &self.netplay, hud.as_mut(), &split_cells) {
                        (Some((walk, _)), _, Some(_), Some(cells))
                            if self.profile.settings.splitscreen.name_tags =>
                        {
                            let names = &self.names;
                            let name = |slot: u8| Some(names.name_or_default(slot));
                            // A LAN round's menu covers its opener's cell only (B20).
                            let covered_at = |i: usize| {
                                hud_hidden
                                    || (split_on
                                        && native_pause
                                        && lan_menu_view.is_none_or(|v| v == i))
                            };
                            // One per view: a round with fewer views keeps no stale answers.
                            self.name_tags.resize_with(cells.len(), Default::default);
                            for (i, cell) in cells.iter().enumerate() {
                                let shown = !covered_at(i)
                                    && !view_away.get(i).copied().unwrap_or(true)
                                    && !split_spawn_up.get(i).copied().unwrap_or(true);
                                let (Some(tags), Some(cam)) =
                                    (self.name_tags.get_mut(i), view_cameras.get(i))
                                else {
                                    continue;
                                };
                                // An away view has no model: its tags still start fresh on return.
                                let Some(Some(model)) = split_models.get_mut(i) else {
                                    tags.clear();
                                    continue;
                                };
                                if !shown || model.round_end.is_some() {
                                    tags.clear();
                                    continue;
                                }
                                let viewer = walk.local_unit(i);
                                model.name_tags = game::name_tags::cell_tags(
                                    tags, &walk.sim, viewer, cam, *cell, dt, &name,
                                );
                            }
                        }
                        (Some((walk, _)), Some(n), Some(model), None)
                            if spawn.is_none() && model.round_end.is_none() && !hud_hidden =>
                        {
                            // Seated players only, named from the one table (`App.names`,
                            // the session names it was given this frame).
                            let roster = n.roster();
                            let names = &self.names;
                            let name = |slot: u8| {
                                roster
                                    .iter()
                                    .find(|p| p.slot == slot)
                                    .map(|p| names.get(slot).unwrap_or(&p.name).to_owned())
                            };
                            let seen = self.name_tags[0].update(
                                &walk.sim,
                                walk.player,
                                camera.position,
                                dt,
                                &name,
                            );
                            let size = [w as f32, h as f32];
                            model.name_tags = game::name_tags::placed(
                                &seen,
                                camera.view_proj(size[0] / size[1].max(1.0)),
                                size,
                            );
                        }
                        _ => self.name_tags.iter_mut().for_each(|t| t.clear()),
                    }
                    let mut reticle_shown = false;
                    let mut hud_view = match (&mut self.hud_view, &hud) {
                        // Split: each view's own HUD, or the Game Menu over them all, is laid out
                        // and painted in the views' overlays (`game::split_ui`).
                        (Some(hv), Some(_)) if split_on => Some(hv),
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
                                    reticle_shown =
                                        hv.update_detached_reticle(model, hud_dt, [w, h]);
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
                    if let (true, Some(view), Some(point), Some(hv), Some(ext)) = (
                        reticle_shown,
                        stereo_view,
                        reticle_at,
                        hud_view.as_mut(),
                        self.display_ext.as_mut(),
                    ) {
                        let paint: render::OverlayFn<'_> = &mut |dev, q, enc, view, size| {
                            hv.paint_reticle(dev, q, enc, view, size)
                        };
                        if let Err(error) = ext.set_reticle(r, &view, point, paint) {
                            self.failure = Some(error);
                            event_loop.exit();
                            return;
                        }
                    }
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
                            // The frame's own work is the main thread's time on
                            // a CPU since the last frame, where the platform
                            // reports it. Without that clock it is the frame's
                            // time less the wait for the window's image, and only
                            // once this platform has shown it waits there (a
                            // frame with time to spare under vsync does): a
                            // driver that blocks in the present instead would
                            // show every GPU-bound frame as busy, so there the
                            // governor is told nothing and treats every slow
                            // frame as the GPU's. The wait is the last present's
                            // (its first view's, in split). `quality_governor`,
                            // CPU-bound rule.
                            let wait = r.acquire_ms() as f32;
                            self.acquire_seen |= wait > 2.0;
                            let cpu_now = game::frame_profile::thread_cpu_ms();
                            let on_cpu = cpu_now
                                .zip(self.governor_cpu.take())
                                .map(|(now, then)| (now - then) as f32);
                            self.governor_cpu = cpu_now;
                            let busy = on_cpu.or_else(|| {
                                (cpu_now.is_none() && self.acquire_seen)
                                    .then(|| (elapsed * 1000.0 - wait).fmax(0.0))
                            });
                            let q = if replaying || frozen {
                                self.governor.quality(tiers)
                            } else {
                                self.governor.frame(
                                    elapsed * 1000.0,
                                    busy,
                                    Some(wait),
                                    budget,
                                    tiers,
                                )
                            };
                            let views = split_cells.as_ref().map_or(1, |c| c.len());
                            for note in self.governor.take_notes() {
                                eprintln!("{}", note.line(&self.quality_map, views));
                            }
                            self.tiers.apply(q.tier_down);
                            self.quality_note = Some(q.label());
                            q.scale
                        }
                        Some(budget) => {
                            self.governor = Default::default();
                            self.governor_cpu = None;
                            self.quality_note = None;
                            self.dyn_res.frame(dt * 1000.0, budget)
                        }
                        None => {
                            if !auto {
                                self.governor = Default::default();
                            }
                            self.governor_cpu = None;
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
                    // Split: each view's own board, held on its own pad and drawn in its own
                    // cell (issue #194 B9), under the same rules as the single view's, and
                    // sized with that cell's HUD.
                    let board_k = self.profile.settings.splitscreen.hud_size.k();
                    let split_boards: Vec<(
                        game::split_layout::Cell,
                        usize,
                        ui::scoreboard::Scoreboard,
                    )> = match (&split_cells, self.walk.as_mut(), self.split.as_ref()) {
                        (Some(cells), Some((walk, _)), Some(split))
                            if walk.campaign.is_none() && menu.is_none() =>
                        {
                            let mut boards = Vec::new();
                            for (i, cell) in cells.iter().enumerate() {
                                let Some(model) = split_models.get(i).and_then(Option::as_ref)
                                else {
                                    continue;
                                };
                                let feed = match i {
                                    0 => Some(&self.feed),
                                    _ => self.split_feeds.get(i - 1),
                                };
                                let shown = split.score_holds.get(i).is_some_and(|h| h.shown());
                                if let (true, Some(feed), None, None) =
                                    (shown, feed, &model.spawn, &model.round_end)
                                {
                                    let titles = model.team_names.clone();
                                    let net = self.netplay.as_ref();
                                    let board = walk
                                        .with_local(i, |w| scoreboard_model(w, feed, titles, net));
                                    boards.push((*cell, i, board));
                                }
                            }
                            boards
                        }
                        _ => Vec::new(),
                    };
                    let scoreboard = match (&self.walk, &hud) {
                        (Some((walk, _)), Some(model))
                            if !split_on
                                && walk.campaign.is_none()
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
                    let render_t0 = std::time::Instant::now();
                    // `native_pause` already implies a menu (`native_pause && menu.is_some()`
                    // above), so this hides the split cells' tags, timer and banner exactly when
                    // the single view hides `draw_hud`.
                    let split_menu_up = split_on && native_pause && lan_menu_view.is_none();
                    // A LAN round's Game Menu covers its opener's cell only (issue #194 B20).
                    let menu_cell_of =
                        |i: usize| split_on && native_pause && lan_menu_view == Some(i);
                    let hud_k = self.profile.settings.splitscreen.hud_size.k();
                    // Split: the cells of players who have left their stats pages, with the
                    // names the round still waits for (issue #194).
                    let waiting = match split_menu_up {
                        true => Vec::new(),
                        false => split_waiting
                            .into_iter()
                            .filter(|(c, _)| Some(*c) != lan_menu_cell.filter(|_| native_pause))
                            .collect(),
                    };
                    let result = match (&mut self.overlay, &self.window, hud) {
                        (Some(overlay), Some(window), Some(model)) => {
                            let mut clicked: Vec<ui::menu::MenuAction> = Vec::new();
                            // Each away view's lost-controller notice, painted in its cell's
                            // HUD pass (`SplitUi::lost`) with the button glyphs.
                            let notices: Vec<Option<ui::shell_ps2::players::LostCell>> =
                                match &split_cells {
                                    // Under the Game Menu the away cells stay dark without the
                                    // notice (it would cover the menu).
                                    Some(cells) if !split_menu_up => (0..cells.len())
                                        .map(|i| {
                                            let away = view_away.get(i).copied().unwrap_or(false);
                                            if !away || menu_cell_of(i) {
                                                return None;
                                            }
                                            let mut lines = super::split::lost_notice({
                                                let slot = self
                                                    .walk
                                                    .as_ref()
                                                    .map_or(i as u8, |(w, _)| w.local_slot_of(i));
                                                // Unnamed: the number from the Players
                                                // screen, as `battle_names` would give.
                                                match (self.names.get(slot), &self.split) {
                                                    (Some(n), _) => n.to_owned(),
                                                    (None, Some(s)) => super::split::local_name(
                                                        &s.seats,
                                                        i,
                                                        &self.profile.name,
                                                        &self.split_profiles,
                                                    ),
                                                    (None, None) => {
                                                        self.names.name_or_default(slot)
                                                    }
                                                }
                                            });
                                            // A returning pad asking whose it is: say so.
                                            if let Some(l) = super::split::pick_line(
                                                self.split.as_ref(),
                                                self.gilrs.as_ref(),
                                                &self.profile,
                                                &self.split_profiles,
                                                self.icon_device.device(),
                                                i,
                                                &lines.name,
                                            ) {
                                                (lines.glyph, lines.action) = (Some(l.0), l.1);
                                            }
                                            Some(lines)
                                        })
                                        .collect(),
                                    _ => Vec::new(),
                                };
                            // Split: the fps counter follows the FPS Counter option as in single
                            // player; when it is up, it adds the view count and the previous
                            // frames' render time (the CPU time of the views' draw lists, submits
                            // and present, smoothed) to read the real cost.
                            let split_note = split_cells.as_ref().map(|c| {
                                format!(
                                    "{} views, render {:.1} ms{}",
                                    c.len(),
                                    self.split_render_ms,
                                    self.quality_note
                                        .as_deref()
                                        .map_or(String::new(), |n| format!(" · {n}"))
                                )
                            });
                            let paint = overlay.begin(window, |ctx| {
                                for (cell, text) in &waiting {
                                    game::split_ui::draw_waiting_line(ctx, *cell, text);
                                }
                                ui::menu::draw_fps_counter_note(
                                    ctx,
                                    ui::menu::FpsCounter::from_index(
                                        self.profile.settings.fps_counter,
                                    ),
                                    self.frame_seconds,
                                    split_note.as_deref().or(self.quality_note.as_deref()),
                                );
                                // Split (issue #194 B8): each awake view's round timer and banner
                                // in its own cell, under its team counts.
                                if let (Some(cells), false) =
                                    (split_cells.as_ref(), hud_hidden || split_menu_up)
                                {
                                    for (i, cell) in cells.iter().enumerate() {
                                        if menu_cell_of(i) {
                                            continue;
                                        }
                                        if let Some(Some(m)) = split_models.get(i) {
                                            let t = game::name_tags::cell_text(ctx, *cell, hud_k);
                                            ui::hud::draw_hud_in(
                                                ctx,
                                                m,
                                                t.rect,
                                                i + 1,
                                                t.scale,
                                                t.timer_at,
                                            );
                                        }
                                    }
                                }
                                if !hud_hidden && !native_pause && !split_on {
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
                                    // Split: the mouse is the keyboard player's; on a menu
                                    // someone else drives, its clicks do nothing (as the
                                    // native pause's, issue #194 B10).
                                    if !egui_pointer_ok {
                                        clicked.clear();
                                        binding_click = None;
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
                                let ppp = ctx.pixels_per_point().fmax(0.1);
                                for (cell, i, b) in &split_boards {
                                    let [x, y, w, h] = cell.map(|v| v as f32 / ppp);
                                    let rect = ui::egui::Rect::from_min_size(
                                        ui::egui::pos2(x, y),
                                        ui::egui::vec2(w, h),
                                    );
                                    ui::scoreboard::draw_scoreboard_in(
                                        ctx,
                                        rect,
                                        i + 1,
                                        board_k,
                                        b,
                                    );
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
                                        // Split: the opener's own profile's layouts (B10).
                                        self.controls = super::split::controls_for(
                                            &self.profile,
                                            &self.split_profiles,
                                            self.split.as_ref(),
                                            menu_owner,
                                            device,
                                            context,
                                        );
                                        if self.controls.is_none() {
                                            self.feed.say(
                                                super::split::NO_PROFILE_BINDINGS.to_owned(),
                                                ui::hud::message_color::YELLOW,
                                                walk.sim.tick,
                                            );
                                        }
                                    }
                                    self.restart_clicked |= a == ui::menu::MenuAction::Restart;
                                    if let ui::menu::MenuAction::SetWindowMode(m) = a {
                                        self.display.requested = Some(m);
                                    }
                                    let opener = super::menu_actions::acting_local(menu_owner, a);
                                    // A LAN round's cheats are the opener's own slot's (issue #194).
                                    let lan_opener =
                                        super::menu_actions::lan_acting_local(menu_owner, a);
                                    // A further player's LAN rows are theirs (B20, as above).
                                    if self.netplay.as_mut().is_some_and(|n| {
                                        n.opener_pause_action(
                                            walk,
                                            a,
                                            lan_opener,
                                            &mut self.paused,
                                            &mut self.pause_menu,
                                            &mut self.profile,
                                            &mut self.split_profiles,
                                            self.split.as_ref().map(|s| &s.seats),
                                            &mut self.session,
                                        )
                                    }) {
                                        continue;
                                    }
                                    let act = |w: &mut game::walk::WalkSim| {
                                        apply_pause_action(
                                            w,
                                            a,
                                            &mut self.paused,
                                            &mut self.pause_menu,
                                            &mut self.cheats,
                                            &mut self.profile,
                                            &mut self.session,
                                            &mut self.kbm,
                                            &mut self.split_profiles,
                                            self.split
                                                .as_mut()
                                                .map(|s| (&s.seats, &mut s.inputs[..])),
                                            opener,
                                            self.music_voice.as_mut(),
                                        )
                                    };
                                    self.quit_clicked |= walk.with_local(opener, act);
                                    if matches!(a, ui::menu::MenuAction::SetRumble(_)) {
                                        super::menu_actions::preview_rumble(
                                            self.gilrs.as_mut(),
                                            &self.active_pad,
                                            &mut self.rumble_effect,
                                            &self.profile,
                                            &self.split_profiles,
                                            self.split.as_mut(),
                                            opener,
                                        );
                                    }
                                }
                            }
                            match (split_cells.as_deref(), self.walk.as_mut()) {
                                (Some(cells), Some((walk, cube))) => {
                                    // Split: the egui overlay (pause menu, counters) paints
                                    // once, over the whole window, after the last view.
                                    let window = r.size();
                                    let split_ui = game::split_ui::SplitUi {
                                        models: &split_models,
                                        icons: &split_icons,
                                        dt: hud_dt,
                                        hidden: hud_hidden,
                                        pause: menu.as_ref().filter(|_| native_pause).map(|m| {
                                            game::split_ui::SplitPause {
                                                owner: split_pause_owner,
                                                menu: m,
                                                anim: &self.pause_menu.animation,
                                                in_view: lan_menu_view,
                                            }
                                        }),
                                        window: [window.0, window.1],
                                        views: cells.len(),
                                        hud_k: self.profile.settings.splitscreen.hud_size.k(),
                                        lost: &notices,
                                    };
                                    let looks = self
                                        .split
                                        .as_ref()
                                        .map(|s| s.looks.clone())
                                        .unwrap_or_default();
                                    // Each view's spawn page, for its soldiers (as view 0's).
                                    let split_pages: Vec<Option<ui::spawn_draw::SpawnPage>> =
                                        split_models
                                            .iter()
                                            .map(|m| {
                                                m.as_ref()
                                                    .and_then(|m| m.spawn.clone())
                                                    .filter(|_| !hud_hidden)
                                            })
                                            .collect();
                                    let mut parts = super::split::ViewParts {
                                        walk,
                                        cube: *cube,
                                        gpu,
                                        soldier_vis: self.soldier_vis.as_mut(),
                                        vehicle_vis: &mut self.vehicle_vis,
                                        deka_vis: &mut self.deka_vis,
                                        fx: &mut self.fx,
                                        holos: &self.holos,
                                        weather: self.weather.as_mut(),
                                        bolts: &mut self.bolts,
                                        fx_quad: self.fx_quad,
                                        fx_sprite: self.fx_sprite,
                                        squad_icons: self.squad_icons,
                                        previews: &mut self.split_previews,
                                        pages: &split_pages,
                                        preview_dt: if frozen { 0.0 } else { dt },
                                        // A LAN round's views draw their predicted poses (B20).
                                        lan_view: self.netplay.as_mut().map(|n| &mut n.view),
                                    };
                                    super::split::render_views(
                                        &mut parts,
                                        r,
                                        cells,
                                        (&draws, &skinned),
                                        &view_cameras,
                                        &looks,
                                        &view_away,
                                        clear,
                                        Some(&mut |dev, q, enc, view, _size| {
                                            overlay.paint(
                                                &paint,
                                                dev,
                                                q,
                                                enc,
                                                view,
                                                [window.0, window.1],
                                            )
                                        }),
                                        Some(&mut |i, cell, dev, q, enc, view| {
                                            if let Some(hv) = hud_view.as_mut() {
                                                split_ui.paint(hv, i, cell, dev, q, enc, view);
                                            }
                                        }),
                                    )
                                    .map_err(anyhow::Error::from)
                                }
                                _ => {
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
                            }
                        }
                        _ => match (split_cells.as_deref(), self.walk.as_mut()) {
                            (Some(cells), Some((walk, cube))) => {
                                let looks = self
                                    .split
                                    .as_ref()
                                    .map(|s| s.looks.clone())
                                    .unwrap_or_default();
                                let mut parts = super::split::ViewParts {
                                    walk,
                                    cube: *cube,
                                    gpu,
                                    soldier_vis: self.soldier_vis.as_mut(),
                                    vehicle_vis: &mut self.vehicle_vis,
                                    deka_vis: &mut self.deka_vis,
                                    fx: &mut self.fx,
                                    holos: &self.holos,
                                    weather: self.weather.as_mut(),
                                    bolts: &mut self.bolts,
                                    fx_quad: self.fx_quad,
                                    fx_sprite: self.fx_sprite,
                                    squad_icons: self.squad_icons,
                                    previews: &mut self.split_previews,
                                    pages: &[],
                                    preview_dt: 0.0,
                                    lan_view: self.netplay.as_mut().map(|n| &mut n.view),
                                };
                                super::split::render_views(
                                    &mut parts,
                                    r,
                                    cells,
                                    (&draws, &skinned),
                                    &view_cameras,
                                    &looks,
                                    &view_away,
                                    clear,
                                    None,
                                    None,
                                )
                                .map_err(anyhow::Error::from)
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
                        },
                    };
                    if let Some(p) = self.frame_profile.as_mut() {
                        p.lap("render_ms");
                        let tick = self.walk.as_ref().map_or(0, |w| w.0.sim.tick);
                        p.end(f64::from(dt) * 1000.0, tick, r);
                    }
                    if split_on {
                        let ms = render_t0.elapsed().as_secs_f32() * 1000.0;
                        self.split_render_ms += (ms - self.split_render_ms) * 0.1;
                        if let Some(hv) = hud_view.as_mut() {
                            hv.select_player(0);
                        }
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
