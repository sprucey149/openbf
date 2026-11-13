//! Native window lifecycle and event dispatch for the application.

use super::spawn_feedback::{draw_snapshot_notice, Selection};
use super::{
    apply_pause_action, autopilot_pad, hud_model, hud_title, load_and_upload, mouse, note_sounds,
    play_ambience, play_event_sounds, rain_splashes, record_round, spawn_model, start_take,
    stats_pages, tracked_velocity, walk_draws, window_attributes, window_size_env, App, Arc,
    DrawCmd, PadState, Renderer, Screen, CLEAR,
};

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(window_attributes(window_size_env()))
                .expect("create window"),
        );
        self.idle = Some(super::idle::IdleInhibit::new(&window));
        let mut renderer = Renderer::new_windowed(window.clone()).expect("renderer");
        self.overlay = Some(ui::overlay::EguiOverlay::new(
            &window,
            renderer.device(),
            renderer.surface_format(),
        ));
        window.set_cursor(winit::window::CursorIcon::Default);
        if let Some(overlay) = &mut self.overlay {
            overlay.set_window_cursor(mouse::menu_cursor(event_loop, window.scale_factor()));
        }
        self.focused = window.has_focus();
        let Some(map) = self.map.clone() else {
            // Shell start: no map yet.
            self.init_shell();
            self.window = Some(window);
            self.renderer = Some(renderer);
            self.last = std::time::Instant::now();
            return;
        };
        self.sounds = game::sounds::SoundLib::load(&map);
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
        match load_and_upload(&mut renderer, &map, &self.mode) {
            Ok((gpu, scene)) => {
                if self.walk_requested {
                    let (walk, cube) = match self.battle_resources(&mut renderer, &scene, None) {
                        Ok(resources) => resources,
                        Err(error) => {
                            eprintln!("failed to start battle: {error:#}");
                            event_loop.exit();
                            return;
                        }
                    };
                    self.profile.settings.last_map = Some(scene.manifest.map.clone());
                    self.profile.settings.last_mode = Some(self.mode.clone());
                    self.profile.settings.last_teams =
                        Some([walk.teams[0].faction.clone(), walk.teams[1].faction.clone()]);
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
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.last = std::time::Instant::now();
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
        if game::input_icons::is_keyboard_or_mouse_use(&event) {
            self.icon_device.keyboard_or_mouse();
        }
        if self.loading_window_event(&event) {
            return;
        }
        self.sync_binding_context();
        if self.capture_keyboard(&event) {
            if let WindowEvent::KeyboardInput { event, .. } = &event {
                if !event.state.is_pressed() {
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        self.keys_held &= !game::kbm::menu_key(code);
                        self.kbm.press(game::kbm::Trigger::Key(code), false);
                    }
                }
            }
            return;
        }
        if let (Some(o), Some(w)) = (&mut self.overlay, &self.window) {
            let _ = o.on_window_event(w, &event);
        }
        if let WindowEvent::ScaleFactorChanged { scale_factor, .. } = &event {
            if let Some(overlay) = &mut self.overlay {
                overlay.set_window_cursor(mouse::menu_cursor(event_loop, *scale_factor));
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
            WindowEvent::CloseRequested => {
                self.finish_loading();
                if let Some((walk, _)) = self.walk.as_mut() {
                    if !self.round_recorded {
                        self.round_recorded = true;
                        record_round(&mut self.profile, walk, self.feed.battle_stats());
                    }
                }
                self.blackbox_exit();
                event_loop.exit()
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
                    let shell = matches!(self.screen, Screen::Shell);
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
            }
            WindowEvent::RedrawRequested => {
                let elapsed = self.last.elapsed().as_secs_f32();
                let dt = elapsed.min(0.1);
                self.last = std::time::Instant::now();
                if matches!(self.screen, Screen::Loading) {
                    self.frame_seconds = 0.0;
                } else if self.frame_seconds == 0.0 {
                    self.frame_seconds = elapsed;
                } else {
                    self.frame_seconds += (elapsed - self.frame_seconds) * (elapsed * 3.0).min(1.0);
                }
                self.sync_audio_levels();
                // The renderer's animation clock, for UV-scrolling materials
                // (a map's water). Wrapped so `uv + velocity * time` keeps
                // its f32 precision in a long session — see Renderer::time.
                if let Some(r) = self.renderer.as_mut() {
                    (r.time, r.water_time) = render::advance_clocks(r.time, r.water_time, dt);
                    if matches!(self.screen, Screen::Battle) && !self.paused {
                        let atmo = r.atmosphere_time.unwrap_or(0.0) + f64::from(dt);
                        r.atmosphere_time = Some(atmo);
                    }
                }

                self.sync_grab();
                self.sync_pause_audio();
                let awake = super::idle::wants_idle_inhibit(self.in_battle(), self.paused);
                if let Some(idle) = self.idle.as_mut() {
                    idle.set(awake);
                }
                if matches!(self.screen, Screen::Shell) {
                    self.redraw_shell(event_loop, dt);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                if matches!(self.screen, Screen::Loading) {
                    if let Err(error) = self.redraw_loading() {
                        eprintln!("render: {error:#}");
                        event_loop.exit();
                        return;
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
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
                let mut spawn: Option<ui::spawn::SpawnModel> = None;
                // A death camera or a dead rider's held seat view is up.
                let mut death_view = false;
                let icons = self.current_icons();
                let mut menu: Option<ui::menu::MenuModel> = None;
                let edges = if self.walk.is_some() {
                    self.menu_edges()
                } else {
                    0
                };
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
                    if let Some(mut model) = self.controls_model(icons).filter(|_| !input.back) {
                        if native_pause {
                            if wheel != 0.0 {
                                ui::pause_draw::scroll(&mut model, wheel);
                                if let Some(controls) = self.controls.as_mut() {
                                    controls.cursor = model.cursor;
                                }
                            }
                            if let (Some((point, true)), Some(r)) =
                                (pointer, self.renderer.as_ref())
                            {
                                let (w, h) = r.size();
                                if let Some(ui::pause_draw::Target::Row(row)) =
                                    ui::pause_draw::target(
                                        &model,
                                        &self.pause_menu.animation,
                                        point,
                                        [w as f32, h as f32],
                                    )
                                {
                                    sound = self.activate_control_row(row).or(sound);
                                }
                            }
                        }
                    }
                    if let (Some(sound), Some(lib)) = (sound, self.sounds.as_mut()) {
                        lib.play_2d(self.audio.as_mut(), sound.name());
                    }
                    menu = self.controls_model(icons);
                }
                let camera = if self.walk.is_some() {
                    self.step_spawn_pointer(dt);
                    let pad = if self.paused {
                        self.kbm.drop_taps();
                        game::pause::hold_for_resume(&mut self.resume_gate);
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
                                    walk.pad_look_for_battle(
                                        self.profile.settings.pad_look_sensitivity,
                                    )
                                };
                                let pad = self.sample_pad(look, on_foot && !space);
                                if let Some(bb) = self.blackbox.as_mut() {
                                    bb.stick(self.stick_sample.0, self.stick_sample.1);
                                }
                                let pad = match controls {
                                    Some(layout) => self.kbm.flyer_pad(layout.map_sticks(pad)),
                                    None => self.kbm.pad(pad),
                                };
                                game::pause::resume_pad(&mut self.resume_gate, pad)
                            }
                        };
                        self.walk
                            .as_ref()
                            .expect("battle walk")
                            .0
                            .apply_sprint_latch(&mut self.sprint_latch, pad)
                    };
                    if self.view_toggle.pressed(&pad) {
                        let s = &mut self.profile.settings;
                        s.viewpoint = s.viewpoint.toggled();
                        if let Err(e) = self.profile.save() {
                            eprintln!("profile autosave failed: {e:#}");
                        }
                    }
                    let pad = self.profile.settings.viewpoint.stamp(pad);
                    let paused = self.paused;
                    let (walk, cube) = self.walk.as_mut().unwrap();
                    // A FIRE press fires where the reticle was drawn.
                    let pad = walk.blend.stamp(pad);
                    let mut stuck_alarm = false;
                    let mut look = PadState::default();
                    if paused && binding_menu {
                        // The controls editor consumed the frame's menu input above.
                    } else if paused {
                        if !self.audio.as_ref().is_some_and(|a| a.game_paused()) {
                            if let Some(sl) = self.sounds.as_mut() {
                                sl.stop_loops();
                            }
                        }
                        let mut model = ui::menu::MenuModel {
                            class_picker_available: false,
                            bindings: None,
                            class_choices: self.pause_menu.class_picker_model(walk),
                            cheats: walk.sim.cheats.entries().to_vec(),
                            assist_intensity: walk.sim.assist_intensity,
                            difficulty: game::shell_ia::ia_level(self.profile.settings.difficulty),
                            effects_volume: self.profile.settings.effects_volume,
                            dialogue_volume: self.profile.settings.dialogue_volume,
                            fps_counter: ui::menu::FpsCounter::from_index(
                                self.profile.settings.fps_counter,
                            ),
                            team_size_pct: walk.team_size_pct,
                            reinforcements_pct: walk.reinforcement_pct,
                            population_note: game::conquest_defaults::menu_note(
                                walk.team_size_pct,
                                walk.reinforcement_pct,
                            ),
                            team_size_off: walk.conquest_defaults.is_none(),
                            fov_scale: walk.fov_scale,
                            fov_deg: walk.fov_y().to_degrees(),
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
                            scene_softness: self.profile.settings.scene_softness,
                            input_icons_label: self.profile.settings.input_icons.label(),
                            icons,
                            first_person: self.profile.settings.viewpoint.is_first(),
                            cursor: self.pause_menu.cursor,
                            page: Some(self.pause_menu.page),
                            popup: self.pause_menu.confirm,
                            restart_available: false,
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
                        let adjust = (edges
                            | self.pause_repeat.step(self.prev_menu_buttons & ADJUST, dt))
                            & ADJUST;
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
                                ui::pause_draw::scroll(&mut model, wheel);
                            }
                            self.pause_menu.animation.advance(dt);
                            target
                        } else {
                            None
                        };
                        let (cursor, actions) = match target.filter(|_| !rows_input.back) {
                            Some(hit) => ui::pause_draw::click(&mut model, hit),
                            None => ui::menu::navigate(&model, rows_input),
                        };
                        let click = game::pause::pause_sound(rows_input, from, cursor, &actions);
                        if let (Some(c), Some(sl)) = (click, self.sounds.as_mut()) {
                            sl.play_2d(self.audio.as_mut(), c.name());
                        }
                        // The PS2 matcher runs beside the menu on the same
                        // edges (the cursor wiggle while typing is authentic).
                        if let Some(line) = (self.pause_menu.page == ui::menu::PausePage::Cheats)
                            .then(|| self.cheats.feed(input, &mut walk.sim))
                            .flatten()
                        {
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
                                class_choices: self.pause_menu.class_picker_model(walk),
                                cursor: self.pause_menu.cursor,
                                page: model.page.map(|_| self.pause_menu.page),
                                popup: self.pause_menu.confirm,
                                cheats: walk.sim.cheats.entries().to_vec(),
                                assist_intensity: walk.sim.assist_intensity,
                                team_size_pct: walk.team_size_pct,
                                reinforcements_pct: walk.reinforcement_pct,
                                population_note: game::conquest_defaults::menu_note(
                                    walk.team_size_pct,
                                    walk.reinforcement_pct,
                                ),
                                team_size_off: walk.conquest_defaults.is_none(),
                                fov_scale: walk.fov_scale,
                                fov_deg: walk.fov_y().to_degrees(),
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
                                scene_softness: self.profile.settings.scene_softness,
                                input_icons_label: self.profile.settings.input_icons.label(),
                                first_person: self.profile.settings.viewpoint.is_first(),
                                ..model
                            });
                        }
                    } else {
                        // Each stepped tick's events are read in the hook,
                        // once: a frame steps no tick (every other one at
                        // 60 Hz), one or several, and `Sim::events` keeps
                        // only the last tick's (card EVTK).
                        self.bolts.age(walk, dt);
                        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                            let (w, h) = r.size();
                            w as f32 / h.max(1) as f32
                        });
                        // The class screen's unlocks read the player's
                        // points as the feed has scored them (card SSELB,
                        // re-spawn-flow §4.3) and its career's costs (UNLK3).
                        let points = self.feed.stats(&walk.sim, walk.player).points;
                        game::unlocks::set_player_gate(walk, points, &self.profile.career);
                        // Station serving is observed for every sim tick, even when a
                        // frame catches up several ticks before posing the scene.
                        if let Some(gpu) = self.gpu.as_mut() {
                            gpu.step_stations(&walk.sim);
                        }
                        let mut tick_pad = pad;
                        let mut selection = Selection::read(walk);
                        let mut medals_seen = self.profile.career.medals;
                        let mut previous_mount = walk.sim.unit(walk.player).and_then(|u| u.mount);
                        let ticks = walk.advance_with(dt, std::slice::from_ref(&pad), |walk| {
                            let hook_t0 = std::time::Instant::now();
                            if let Some(gpu) = self.gpu.as_mut() {
                                gpu.step_stations(&walk.sim);
                            }
                            self.fx.ingest_posed(walk, self.soldier_vis.as_ref());
                            let effect_sounds = self.fx.take_sounds();
                            let buff_loops = self.fx.buff_loops(&walk.sim);
                            if let Some(vis) = self.soldier_vis.as_mut() {
                                vis.note_shots(walk);
                                let mut points = vis.fire_points(walk);
                                self.deka_vis.fire_points(&walk.sim, &mut points);
                                self.bolts.set_fire_points(points);
                            }
                            self.bolts.track(walk);
                            if let (Some(audio), Some(sl)) =
                                (self.audio.as_mut(), self.sounds.as_mut())
                            {
                                let next = Selection::read(walk);
                                if let Some(cue) =
                                    selection.movement_cue(next, walk.sim.ground_rules)
                                {
                                    sl.play_2d(Some(audio), cue);
                                }
                                selection = next;
                                play_event_sounds(
                                    audio,
                                    sl,
                                    self.foley.as_mut(),
                                    &mut self.engines,
                                    &effect_sounds,
                                    aspect,
                                    walk,
                                );
                                // The auras' loops (the War Hero's crackle).
                                let listener = game::event_sounds::listener(walk);
                                sl.play_buff_loops(audio, &buff_loops, &listener);
                                play_ambience(audio, sl, &self.ambience, walk);
                            }
                            game::heroes::tick(walk, &mut self.feed);
                            self.feed
                                .note_medals(&walk.sim, walk.player, &mut medals_seen);
                            if let Some(bb) = self.blackbox.as_mut() {
                                stuck_alarm |= bb.tick(walk, Some(tick_pad));
                                note_sounds(bb, self.sounds.as_mut(), walk.sim.tick);
                            }
                            let strength = self.profile.settings.rumble_strength;
                            if strength > 0.0 {
                                if let Some(pulse) = self.rumble_tick.pulse(
                                    &walk.sim,
                                    walk.player,
                                    previous_mount,
                                    self.profile.settings.rumble_on_fire,
                                ) {
                                    if let Some(g) = self.gilrs.as_mut() {
                                        if let Some((id, _)) =
                                            input::active_gamepad(g, &self.active_pad)
                                        {
                                            self.rumble_effect =
                                                input::rumble(g, id, pulse, strength);
                                        }
                                    }
                                }
                            }
                            previous_mount = walk.sim.unit(walk.player).and_then(|u| u.mount);
                            if let Some(m) = self.matchlog.as_mut() {
                                m.tick(&walk.sim, &tick_pad);
                                if walk.sim.outcome.is_some() && !m.written() {
                                    if let Some(p) = m.write(&walk.sim, "outcome") {
                                        println!("matchlog: wrote {}", p.display());
                                    }
                                }
                            }
                            if let Some(p) = self.frame_profile.as_mut() {
                                let hook = hook_t0.elapsed().as_secs_f64() * 1000.0;
                                p.tick(walk.sim.ai_wall_ms, hook, &walk.sim.tick_cost, |u| {
                                    game::frame_profile::unit_name(walk, u)
                                });
                            }
                            tick_pad.clear_once();
                        });
                        self.kbm.consumed(&pad, ticks);
                        walk.blend.consumed(ticks);
                        if walk.flush_career_progress(&mut self.profile) {
                            if let Err(e) = self.profile.save() {
                                eprintln!("profile autosave failed: {e:#}");
                            }
                        }
                        look = self.kbm.look();
                        // A controller's preview is its held stick (the
                        // next tick's turn, `Sim::look_preview`).
                        if !look.has_look() && !look.mouse_aim() {
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
                            .advance_frame(self.soldier_vis.as_mut(), walk, &look, dt);
                        if let Some(w) = self.weather.as_mut() {
                            w.update(dt);
                            rain_splashes(w, &mut self.fx, walk, aspect, dt);
                        }
                        if let (Some(audio), Some(sl)) = (self.audio.as_mut(), self.sounds.as_mut())
                        {
                            // The commander: this frame's requests join the
                            // voice queue, and an idle voice starts its head.
                            // A squad answer plays the take drawn for it,
                            // at its unit.
                            let ears = game::event_sounds::listener(walk);
                            if let Some(v) = walk.voice.as_mut() {
                                let tick = walk.sim.tick;
                                // The units' combat calls through the
                                // bank's gates join the queue first.
                                v.gate_calls(sl, ears.pos);
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
                    game::render_blend::blend_in_looking(walk, &look);
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
                        extra.extend(self.vehicle_vis.draws(r, &walk.sim, &see.vehicles));
                        extra.extend(self.vehicle_vis.chunk_draws(self.fx.lit_chunk_poses()));
                        self.deka_vis.eye = Some(walk.camera().position);
                        extra.extend(self.deka_vis.draws(r, &walk.sim));
                    }
                    if let Some(vis) = self.soldier_vis.as_mut() {
                        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, |r| {
                            let (w, h) = r.size();
                            w as f32 / h as f32
                        });
                        vis.prepare_view(walk, &walk.camera(), aspect);
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
                        let music_secs = walk.music.as_ref().and_then(|m| {
                            let team = walk.sim.units.get(walk.player.index as usize)?.team;
                            m.end_music_secs(usize::from(team), walk.sim.outcome?)
                        });
                        let (done, cue) = walk.follow_round_end(dt, pad, music_secs);
                        if let (Some(cue), Some(sounds)) = (cue, self.sounds.as_mut()) {
                            sounds.play_2d(self.audio.as_mut(), cue.name());
                        }
                        if done {
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
                    hud = Some(hud_model(walk, &mut self.feed, hud_aspect));
                    // The capture icon is off under the pause menu, as the
                    // PS2's (card CPBAR, re-capture-display 3.4).
                    if let Some(m) = hud.as_mut().filter(|_| paused) {
                        m.capture = None;
                    }
                    // Spawn screen while dead: the post and class it shows
                    // are the sim's (card SPSEL), moved by the pad bits.
                    spawn =
                        spawn_model(walk, &self.feed).map(|s| ui::spawn::SpawnModel { icons, ..s });
                    death_view = walk.death_cam.is_some() || walk.holds_seat_view();
                    let camera = walk.camera();
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
                        extra.extend(game::squad_vis::squad_draws(
                            &walk.sim,
                            0,
                            quad,
                            self.squad_icons.0,
                            self.squad_icons.1,
                            &camera,
                        ));
                    }
                }

                if let (Some(r), Some(gpu)) = (&mut self.renderer, &mut self.gpu) {
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
                            gpu.pose_turrets(&walk.sim, &walk.vehicle_seats);
                            gpu.pose_stations(&walk.sim);
                            gpu.see_through(&see.props);
                            gpu.pose_ctf(&walk.sim, walk.player);
                            self.holos.advance(walk);
                            game::point_light::submit(
                                r,
                                gpu,
                                walk,
                                &mut self.bolts,
                                camera.position,
                                &self.holos,
                            );
                            let flashes = self
                                .soldier_vis
                                .as_ref()
                                .map(|v| {
                                    let mut points = v.fire_points(walk);
                                    self.deka_vis.fire_points(&walk.sim, &mut points);
                                    v.flash_draws_at(walk, &points)
                                })
                                .unwrap_or_default();
                            let mut particles = flashes;
                            if let Some(vis) = &self.soldier_vis {
                                particles.extend(vis.saber_draws(walk));
                            }
                            particles.extend(self.deka_vis.bubbles(
                                &walk.sim,
                                &camera,
                                r.lighting.sun_dir,
                            ));
                            particles.extend(self.holos.draws(&gpu.holo, walk, &camera));
                            if let Some(w) = self.weather.as_mut() {
                                particles.extend(w.frame_draws(r, &camera, tracked_velocity(walk)));
                            }
                            self.fx.submit_walk(r, &camera, walk, particles);
                        }
                        None => {
                            r.set_leaf_time(None);
                            r.set_leaf_actors(Vec::new());
                            r.set_bolts(Vec::new());
                            r.set_particles(Vec::new());
                            r.set_terrain_lights(Default::default());
                        }
                    }
                    // The player's own view: a zoomed turret gunner's
                    // building is left out of it (card VTURZ).
                    let mut draws: Vec<DrawCmd> = match &self.walk {
                        Some((walk, _)) => gpu.view_draws(&walk.sim, walk.player),
                        None => gpu.draws.clone(),
                    };
                    draws.append(&mut extra);
                    if let (Some(vis), Some((walk, _))) = (self.soldier_vis.as_ref(), &self.walk) {
                        let (arms, gun) = vis.first_person_draws(walk, &camera);
                        skinned.extend(arms);
                        draws.extend(gun);
                    }
                    // The PS2's spawn screens (the HUD pass's, card SSELB):
                    // the soldier standing in the camera shot.
                    let page = spawn
                        .as_ref()
                        .and_then(|s| s.page.as_ref())
                        .filter(|_| self.hud_view.is_some() && !self.cheats.hud_hidden);
                    if let Some(vis) = self.soldier_vis.as_ref() {
                        let step = if self.paused { 0.0 } else { dt };
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
                        walk.blend_out();
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
                    let hud_dt = if self.paused { 0.0 } else { dt };
                    let mut hud_view = match (&mut self.hud_view, &hud) {
                        (Some(hv), Some(model)) => {
                            hv.set_icons(icons);
                            if let Some(m) = menu.as_ref().filter(|_| native_pause) {
                                hv.update_pause(model, m, &self.pause_menu.animation, [w, h]);
                                Some(hv)
                            } else {
                                hv.update(model, hud_dt, [w, h]);
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
                    let scale = if self.profile.settings.low_power_resolution && self.walk.is_some()
                    {
                        let window = &self.window;
                        let budget = *self.dyn_res_budget_ms.get_or_insert_with(|| {
                            game::dyn_res::display_budget_ms(
                                window
                                    .as_ref()
                                    .and_then(|w| w.current_monitor())
                                    .and_then(|m| m.refresh_rate_millihertz()),
                            )
                        });
                        self.dyn_res.frame(dt * 1000.0, budget)
                    } else {
                        self.dyn_res = game::dyn_res::DynRes::default();
                        self.dyn_res_budget_ms = None;
                        1.0
                    };
                    r.set_world_scale(scale);
                    let mut binding_click = None;
                    let result = match (&mut self.overlay, &self.window, hud) {
                        (Some(overlay), Some(window), Some(model)) => {
                            let mut clicked: Vec<ui::menu::MenuAction> = Vec::new();
                            let paint = overlay.begin(window, |ctx| {
                                ui::menu::draw_fps_counter(
                                    ctx,
                                    ui::menu::FpsCounter::from_index(
                                        self.profile.settings.fps_counter,
                                    ),
                                    self.frame_seconds,
                                );
                                if !hud_hidden && !native_pause {
                                    // The death camera and the spawn screen
                                    // own the death: the egui banner is only
                                    // for launches with neither.
                                    let m = ui::hud::HudModel {
                                        dead: model.dead && spawn.is_none() && !death_view,
                                        ..model.clone()
                                    };
                                    ui::hud::draw_hud(ctx, &m);
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
                            r.render_frame(
                                &camera,
                                &draws,
                                &skinned,
                                clear,
                                Some(&mut |dev, q, enc, view, size| {
                                    if let Some(hv) = hud_view.as_mut() {
                                        hv.paint(dev, q, enc, view, size);
                                    }
                                    overlay.paint(&paint, dev, q, enc, view, size)
                                }),
                            )
                        }
                        _ => r.render_frame(&camera, &draws, &skinned, clear, None),
                    };
                    if let Some(p) = self.frame_profile.as_mut() {
                        p.lap("render_ms");
                        let tick = self.walk.as_ref().map_or(0, |w| w.0.sim.tick);
                        p.end(f64::from(dt) * 1000.0, tick, r);
                    }
                    if let Err(e) = result {
                        eprintln!("render: {e}");
                        event_loop.exit();
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
                    walk.blend_out();
                }
                if let (Some(at), Some((walk, _))) = (self.quit_at, self.walk.as_ref()) {
                    self.quit_clicked |= walk.sim.tick >= at;
                }
                if std::mem::take(&mut self.restart_clicked) {
                    self.restart_round();
                }
                if self.quit_clicked {
                    self.quit_clicked = false;
                    if self.map.is_some() {
                        self.blackbox_exit();
                        event_loop.exit();
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
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}
