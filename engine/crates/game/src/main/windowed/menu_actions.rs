//! Windowed launch settings and menu-setting persistence.

use game::walk::WalkSim;

/// Autosave the settings a menu action changed.
pub(super) fn persist_setting(
    profile: &mut game::profile::Profile,
    session: &mut ui::shell::SessionOptions,
    a: ui::menu::MenuAction,
    walk: &WalkSim,
) {
    use ui::menu::MenuAction;
    match a {
        MenuAction::SetAssist(_) => profile.settings.assist_intensity = walk.sim.assist_intensity,
        MenuAction::SetAiCap(_) => {
            profile.settings.team_size_pct = walk.team_size_pct;
            session.con_numbots = walk.team_size_pct as i32;
            session.num_bots = session.con_numbots;
        }
        MenuAction::SetReinforcements(_) => {
            profile.settings.reinforcement_pct = walk.reinforcement_pct;
            session.con_mult = walk.reinforcement_pct as i32;
        }
        MenuAction::SetFov(_) => profile.settings.fov_scale = walk.fov_scale,
        MenuAction::ResetGraphics => profile.settings.reset_graphics(),
        MenuAction::SetMusic(v) => profile.settings.music_volume = v,
        MenuAction::SetDifficulty(v) => {
            profile.settings.difficulty = game::shell_ia::port_difficulty(v);
            session.difficulty = v;
        }
        MenuAction::SetEffectsVolume(v) => profile.settings.effects_volume = v.clamp(0.0, 1.0),
        MenuAction::SetDialogueVolume(v) => profile.settings.dialogue_volume = v.clamp(0.0, 1.0),
        MenuAction::SetFpsCounter(v) => profile.settings.fps_counter = v as u8,
        MenuAction::SetNetworkMeter(on) => profile.settings.network_meter = on,
        MenuAction::SetAutoQuality(on) => profile.settings.auto_quality = on,
        MenuAction::SetCareerAwards(on) => profile.settings.career_awards = on,
        MenuAction::SetRumble(v) => profile.settings.rumble_strength = v.clamp(0.0, 1.0),
        MenuAction::ToggleRumbleOnFire => {
            profile.settings.rumble_on_fire = !profile.settings.rumble_on_fire;
        }
        MenuAction::SetMouseSensitivity(v) => {
            profile.settings.mouse_sensitivity = v.clamp(
                game::kbm::SENSITIVITY_RANGE.0,
                game::kbm::SENSITIVITY_RANGE.1,
            )
        }
        MenuAction::SetPadLookSensitivity(v) => {
            profile.settings.pad_look_sensitivity = game::profile::clamp_pad_look(v)
        }
        MenuAction::ToggleFlightControls => {
            profile.settings.flight_controls = profile.settings.flight_controls.toggled()
        }
        MenuAction::ToggleInvertY => profile.settings.invert_y = !profile.settings.invert_y,
        MenuAction::ToggleLowPowerResolution => {
            profile.settings.low_power_resolution = !profile.settings.low_power_resolution
        }
        MenuAction::CycleInputIcons(dir) => {
            profile.settings.input_icons = profile.settings.input_icons.cycled(dir)
        }
        MenuAction::ToggleViewpoint => {
            profile.settings.viewpoint = profile.settings.viewpoint.toggled()
        }
        _ => return,
    }
    if let Err(e) = profile.save() {
        eprintln!("profile autosave failed: {e:#}");
    }
}

/// One in-round pause-menu action, from a pad press or a pointer click alike:
/// the battle, the cheats, the saved settings, the keyboard and mouse, and the
/// paused music's level. True for Quit.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_pause_action(
    walk: &mut WalkSim,
    a: ui::menu::MenuAction,
    paused: &mut bool,
    menu: &mut game::pause::PauseMenu,
    cheats: &mut game::cheats::Session,
    profile: &mut game::profile::Profile,
    session: &mut ui::shell::SessionOptions,
    kbm: &mut game::kbm::Kbm,
    music_voice: Option<&mut audio::MusicVoice>,
) -> bool {
    game::pause::apply_menu_action(walk, a, paused, menu);
    cheats.follow(&walk.sim);
    persist_setting(profile, session, a, walk);
    kbm.sensitivity = profile.settings.mouse_sensitivity;
    kbm.invert_y = profile.settings.invert_y;
    // The music follows its slider; with the game bus paused the change is
    // heard on resume.
    if let (ui::menu::MenuAction::SetMusic(_), Some(m), Some(v)) =
        (a, walk.music.as_ref(), music_voice)
    {
        let level = m.level().map_or(0.0, |(_, l)| l);
        v.set_volume(game::sounds::level_db(level), 0.1);
    }
    a == ui::menu::MenuAction::Quit
}

/// Resolve only interactive direct-launch settings. Headless remains profile-free.
/// Team size and reinforcements apply to a conquest launch (`--mission`) only:
/// a plain `game <map>` walk stays as it was.
pub(super) fn resolve_direct_settings(
    setup: &mut super::BattleSetup,
    cli_difficulty: Option<core::skill::Difficulty>,
    settings: &game::profile::Settings,
) {
    setup.difficulty = cli_difficulty.unwrap_or(settings.difficulty);
    if setup.mission.is_none() {
        return;
    }
    if setup.ai.is_none() && setup.ai_cap.is_none() {
        setup.team_size_pct = Some(
            settings
                .team_size_pct
                .clamp(10, ui::population::MAX_TEAM_PCT),
        );
    }
    setup.ticket_pct = settings.reinforcement_pct.clamp(10, 500);
}
