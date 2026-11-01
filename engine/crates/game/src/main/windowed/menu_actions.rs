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
        MenuAction::SetMusic(v) => profile.settings.music_volume = v,
        MenuAction::SetMouseSensitivity(v) => {
            profile.settings.mouse_sensitivity = v.clamp(
                game::kbm::SENSITIVITY_RANGE.0,
                game::kbm::SENSITIVITY_RANGE.1,
            )
        }
        MenuAction::ToggleInvertY => profile.settings.invert_y = !profile.settings.invert_y,
        MenuAction::ToggleViewpoint => {
            profile.settings.viewpoint = profile.settings.viewpoint.toggled()
        }
        _ => return,
    }
    if let Err(e) = profile.save() {
        eprintln!("profile autosave failed: {e:#}");
    }
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
