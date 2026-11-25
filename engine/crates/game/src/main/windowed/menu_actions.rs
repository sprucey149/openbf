//! Windowed launch settings and menu-setting persistence.

use game::walk::WalkSim;

/// Autosave the per-player settings a menu action changed into `profile`: the Game Menu
/// opener's own (`SPLITSCREEN.md` 18.1 item 6, [`ui::menu::RowScope::Player`]). `opener` is
/// the local player who opened the menu, swapped into `walk` (0 is the first player). Nothing
/// for any other action.
pub(super) fn persist_player(
    profile: &mut game::profile::Profile,
    a: ui::menu::MenuAction,
    walk: &WalkSim,
    opener: usize,
) {
    use ui::menu::MenuAction;
    match a {
        // A player after the first has their own slot's assist (`pause::apply_slot_setting`).
        MenuAction::SetAssist(_) if opener > 0 => {
            profile.settings.assist_intensity = walk
                .sim
                .assist_profile(usize::from(walk.local_slot))
                .intensity
        }
        MenuAction::SetAssist(_) => profile.settings.assist_intensity = walk.sim.assist_intensity,
        MenuAction::SetFov(_) => profile.settings.fov_scale = walk.fov_scale,
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

/// Autosave the match-wide settings a menu action changed into the host's (P1's) `profile`,
/// whoever opened the menu ([`ui::menu::RowScope::Host`]). Nothing for any other action.
pub(super) fn persist_host(
    profile: &mut game::profile::Profile,
    session: &mut ui::shell::SessionOptions,
    a: ui::menu::MenuAction,
    walk: &WalkSim,
) {
    use ui::menu::MenuAction;
    match a {
        MenuAction::SetAiCap(_) => {
            profile.settings.team_size_pct = walk.team_size_pct;
            session.con_numbots = walk.team_size_pct as i32;
            session.num_bots = session.con_numbots;
        }
        MenuAction::SetReinforcements(_) => {
            profile.settings.reinforcement_pct = walk.reinforcement_pct;
            session.con_mult = walk.reinforcement_pct as i32;
        }
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
        MenuAction::ToggleTerrainLod => {
            profile.settings.terrain_lod = !profile.settings.terrain_lod
        }
        MenuAction::ToggleHardShadows => {
            profile.settings.hard_shadows = !profile.settings.hard_shadows
        }
        MenuAction::ToggleSceneSoftness => {
            profile.settings.scene_softness = !profile.settings.scene_softness
        }
        MenuAction::ToggleLowPowerResolution => {
            profile.settings.low_power_resolution = !profile.settings.low_power_resolution
        }
        _ => return,
    }
    if let Err(e) = profile.save() {
        eprintln!("profile autosave failed: {e:#}");
    }
}

/// Issue #194 (B10): which local player a Game Menu action acts as, when `owner` opened the
/// menu (`None`: not split, or nobody in particular). A host row's action acts as the first
/// player whoever pressed it (its profile is the host's); everything else as the opener, so a
/// Respawn kills their soldier and a per-player row is theirs. Reset Graphics acts as the opener
/// too: it resets the host's graphics, but the FOV and view it resets are the opener's own
/// ([`apply_pause_action`]). 0 is the first player.
pub(super) fn acting_local(owner: Option<usize>, a: ui::menu::MenuAction) -> usize {
    match ui::menu::action_scope(a) {
        Some(ui::menu::RowScope::Host) if a != ui::menu::MenuAction::ResetGraphics => 0,
        _ => owner.unwrap_or(0),
    }
}

/// [`acting_local`] in a LAN round, whose cheats are per player (one slot each,
/// `GameCommand::SetSlotCheat`): a Cheats row acts as the opener, not the first player (issue
/// #194). Every other action as [`acting_local`] says.
pub(super) fn lan_acting_local(owner: Option<usize>, a: ui::menu::MenuAction) -> usize {
    match a {
        ui::menu::MenuAction::ToggleCheat(_) => owner.unwrap_or(0),
        _ => acting_local(owner, a),
    }
}

/// The slot of the Game Menu's opener `owner` (`None`, or one no longer a local of the walk: the
/// first player's), whose own the LAN Cheats page is (issue #194).
pub(super) fn opener_slot(walk: &WalkSim, owner: Option<usize>) -> u8 {
    walk.local_slot_of(owner.filter(|o| *o < walk.local_count()).unwrap_or(0))
}

/// The profile local player `opener` keeps their own settings in: the host's for the first
/// player, else their seat's (`App.split_profiles`, the battle's one copy of it, which the
/// round's record also writes). `None` for a seat with no profile (the developer flag): nothing
/// of theirs is saved, and only what the round itself keeps changes for the round (assist and FOV
/// on their slot and view, the viewpoint on their pad); the other per-player rows show and keep
/// the host's values, and the Controls editor does not open (`split::controls_for`).
pub(super) fn opener_profile<'a, Id: Copy + PartialEq>(
    host: &'a mut game::profile::Profile,
    seat_profiles: &'a mut [Option<game::profile::Profile>; 4],
    seats: Option<&game::local_seats::Seats<Id>>,
    opener: usize,
) -> Option<&'a mut game::profile::Profile> {
    if opener == 0 {
        return Some(host);
    }
    let seat = super::split::profile_seat(seats?, opener);
    seat_profiles
        .get_mut(seat)
        .filter(|_| seat != 0)
        .and_then(Option::as_mut)
}

/// Issue #194 (B10): the Game Menu as local player `opener` sees it: their own values in every
/// per-player row ([`ui::menu::RowScope::Player`]) from `settings` (their seat's profile,
/// `split::seat_settings`) and their own slot (assist, FOV), and the host's rows marked shared
/// ([`ui::menu::MenuModel::shared_rows`]). Nothing for the first player (0), whose menu is the
/// host profile's as it always was.
pub(super) fn opener_model(
    model: &mut ui::menu::MenuModel,
    walk: &mut WalkSim,
    settings: &game::profile::Settings,
    name: Option<&str>,
    opener: usize,
) {
    if opener == 0 || opener >= walk.local_count() {
        return;
    }
    let slot = usize::from(walk.local_slot_of(opener));
    model.assist_intensity = walk.sim.assist_profile(slot).intensity;
    model.fov_scale = walk.with_local(opener, |w| w.fov_scale);
    model.mouse_sensitivity = settings.mouse_sensitivity;
    model.pad_look_sensitivity = settings.pad_look_sensitivity;
    model.rumble_strength = settings.rumble_strength;
    model.rumble_on_fire = settings.rumble_on_fire;
    model.career_awards_disabled = !settings.career_awards;
    model.rc_flight_controls = settings.flight_controls == input::FlightControls::RcMode2;
    model.invert_y = settings.invert_y;
    model.input_icons_label = settings.input_icons.label();
    model.first_person = settings.viewpoint.is_first();
    if let Some(name) = name {
        model.player_name = name.to_owned();
    }
    model.shared_rows = true;
}

/// The Rumble row's preview buzz: on the first player's active pad at the host's strength, or
/// for a split player after the first on their own pad at their own strength (issue #194 B10).
#[allow(clippy::too_many_arguments)]
pub(super) fn preview_rumble(
    gilrs: Option<&mut gilrs::Gilrs>,
    active: &input::ActivePad<gilrs::GamepadId>,
    effect: &mut Option<gilrs::ff::Effect>,
    host: &game::profile::Profile,
    seat_profiles: &[Option<game::profile::Profile>; 4],
    split: Option<&mut super::split::SplitState>,
    opener: usize,
) {
    match split.filter(|_| opener > 0) {
        Some(s) => {
            let strength =
                super::split::seat_settings(host, seat_profiles, &s.seats, opener).rumble_strength;
            let id = s.seats.pad_of(opener);
            if let Some(input) = s.inputs.get_mut(opener) {
                game::rumble::preview_pad(gilrs, id, &mut input.rumble_effect, strength);
            }
        }
        None => game::rumble::preview(gilrs, active, effect, host.settings.rumble_strength),
    }
}

/// Issue #194 (B7): the Splitscreen page's actions on the host profile's `settings`
/// (`SPLITSCREEN.md` 18.1 item 4). `seated` is which Players-screen seats are in the match (a
/// screen row step moves among them, `split_layout::move_seat`). True when the action was one
/// of them (the caller saves the profile).
pub(super) fn apply_split_setting(
    settings: &mut game::profile::SplitscreenSettings,
    a: ui::menu::MenuAction,
    seated: [bool; 4],
) -> bool {
    use game::profile::{HudSize, TwoPlayerSplit};
    use ui::menu::MenuAction;
    match a {
        MenuAction::SetSplitTwoPlayer(top) => {
            settings.two_player = if top {
                TwoPlayerSplit::TopAndBottom
            } else {
                TwoPlayerSplit::SideBySide
            }
        }
        MenuAction::MoveSplitScreen { seat, right } => {
            settings.cells =
                game::split_layout::move_seat(settings.cells, seated, usize::from(seat), right)
        }
        MenuAction::SetSplitNameTags(on) => settings.name_tags = on,
        MenuAction::SetSplitHudSize(i) => {
            settings.hud_size = match i {
                1 => HudSize::Smaller,
                2 => HudSize::Smallest,
                _ => HudSize::Full,
            }
        }
        _ => return false,
    }
    true
}

/// The Splitscreen page's values from the host profile's settings and the seated seats.
pub(super) fn split_menu(
    settings: &game::profile::SplitscreenSettings,
    seated: [bool; 4],
) -> ui::menu::SplitMenu {
    use game::profile::{HudSize, TwoPlayerSplit};
    let mut s = *settings;
    s.normalize();
    ui::menu::SplitMenu {
        top_and_bottom: s.two_player == TwoPlayerSplit::TopAndBottom,
        seated,
        cells: s.cells,
        name_tags: s.name_tags,
        hud_size: match s.hud_size {
            HudSize::Smaller => 1,
            HudSize::Smallest => 2,
            HudSize::Full | HudSize::Unknown => 0,
        },
    }
}

/// Which Players-screen seats the match's `seats` hold.
pub(super) fn seated_of<Id: Copy + PartialEq>(seats: &game::local_seats::Seats<Id>) -> [bool; 4] {
    let mut seated = [false; 4];
    for i in 0..seats.players() {
        if let Some(s) = seated.get_mut(seats.seat_number(i)) {
            *s = true;
        }
    }
    seated
}

/// One in-round pause-menu action, from a pad press or a pointer click alike:
/// the battle, the cheats, the saved settings, the keyboard and mouse, and the
/// paused music's level. True for Quit. The mouse then plays with the
/// keyboard player's own settings: their seat's profile when split
/// (`seat_profiles`, `split`; `split::keyboard_settings_of`), else the
/// host's as the action left them.
///
/// Issue #194 (B10): `opener` is the local player the action acts as ([`acting_local`]), already
/// swapped into `walk` by the caller. A per-player row writes that player's own profile
/// ([`opener_profile`], [`persist_player`]) and sets their own slot
/// (`pause::apply_opener_action`); their viewpoint follows at once in split. A host row
/// writes `profile` ([`persist_host`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_pause_action<Id: Copy + PartialEq>(
    walk: &mut WalkSim,
    a: ui::menu::MenuAction,
    paused: &mut bool,
    menu: &mut game::pause::PauseMenu,
    cheats: &mut game::cheats::Session,
    profile: &mut game::profile::Profile,
    session: &mut ui::shell::SessionOptions,
    kbm: &mut game::kbm::Kbm,
    seat_profiles: &mut [Option<game::profile::Profile>; 4],
    split: Option<(
        &game::local_seats::Seats<Id>,
        &mut [super::split::PlayerInput],
    )>,
    opener: usize,
    music_voice: Option<&mut audio::MusicVoice>,
) -> bool {
    game::pause::apply_opener_action(walk, a, paused, menu, opener);
    cheats.follow(&walk.sim);
    let (seats, inputs) = split.unzip();
    match ui::menu::action_scope(a) {
        Some(ui::menu::RowScope::Player) => {
            if let Some(own) = opener_profile(profile, seat_profiles, seats, opener) {
                persist_player(own, a, walk, opener);
            }
        }
        // A player after the first resets the host's graphics but their own FOV and view: the
        // host's FOV and view are P1's.
        _ if a == ui::menu::MenuAction::ResetGraphics && opener > 0 => {
            let defaults = game::profile::Settings::default();
            let s = &mut profile.settings;
            let (fov, view) = (s.fov_scale, s.viewpoint);
            s.reset_graphics();
            (s.fov_scale, s.viewpoint) = (fov, view);
            if let Err(e) = profile.save() {
                eprintln!("profile autosave failed: {e:#}");
            }
            if let Some(own) = opener_profile(profile, seat_profiles, seats, opener) {
                own.settings.fov_scale = defaults.fov_scale;
                own.settings.viewpoint = defaults.viewpoint;
                if let Err(e) = own.save() {
                    eprintln!("profile autosave failed: {e:#}");
                }
            }
        }
        _ => persist_host(profile, session, a, walk),
    }
    // Issue #194: the Splitscreen page edits the host's profile; the next frame re-lays out.
    if let Some(seats) = seats {
        if apply_split_setting(&mut profile.settings.splitscreen, a, seated_of(seats)) {
            if let Err(e) = profile.save() {
                eprintln!("profile autosave failed: {e:#}");
            }
        }
    }
    // Split: each player's viewpoint is stamped from their own input, so a menu change of it
    // (theirs, or their reset of the graphics) shows at once. A seat with no profile keeps it on
    // the input alone.
    if let (true, Some(seats), Some(inputs)) = (
        matches!(
            a,
            ui::menu::MenuAction::ToggleViewpoint | ui::menu::MenuAction::ResetGraphics
        ),
        seats,
        inputs,
    ) {
        let saved = opener_profile(profile, seat_profiles, Some(seats), opener).is_some();
        if let Some(input) = inputs.get_mut(opener) {
            let v = match a {
                _ if saved => {
                    super::split::seat_settings(profile, seat_profiles, seats, opener).viewpoint
                }
                ui::menu::MenuAction::ToggleViewpoint => input.viewpoint().toggled(),
                _ => game::profile::Settings::default().viewpoint,
            };
            input.set_viewpoint(v);
        }
    }
    kbm.follow(super::split::keyboard_settings_of(
        profile,
        seat_profiles,
        seats,
    ));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #194 B7: the Splitscreen page's actions land in the host's options, and the next
    /// frame's cells follow them (a swap, top and bottom, the HUD size, name tags).
    #[test]
    fn the_splitscreen_page_edits_the_hosts_options_and_the_cells_follow() {
        use ui::menu::MenuAction;
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(1);
        o.join_pad(2);
        o.join_pad(3);
        o.leave(1);
        let seats = game::local_seats::Seats::from_owners(o, 2);
        let seated = seated_of(&seats);
        assert_eq!(seated, [true, false, true, false]);
        let mut opts = game::profile::SplitscreenSettings::default();
        let seat_of = |i: usize| seats.seat_number(i);
        let before = super::super::split::split_cells_for(&opts, 2, 1280, 720, seat_of);
        assert_eq!(
            before,
            game::split_layout::cells(2, 1280, 720, game::split_layout::TwoPlayer::SideBySide)
        );
        // P3's row (seat 2), either arrow: the two screens swap.
        assert!(apply_split_setting(
            &mut opts,
            MenuAction::MoveSplitScreen {
                seat: 2,
                right: false
            },
            seated
        ));
        let after = super::super::split::split_cells_for(&opts, 2, 1280, 720, seat_of);
        assert_eq!(after, vec![before[1], before[0]]);
        assert_eq!(split_menu(&opts, seated).cells, opts.cells);
        assert!(apply_split_setting(
            &mut opts,
            MenuAction::SetSplitTwoPlayer(true),
            seated
        ));
        let top = super::super::split::split_cells_for(&opts, 2, 1280, 720, seat_of);
        assert_eq!(top[1], [0, 0, 1280, 359], "P1 now bottom, P3 top");
        assert!(split_menu(&opts, seated).top_and_bottom);
        assert!(apply_split_setting(
            &mut opts,
            MenuAction::SetSplitHudSize(2),
            seated
        ));
        assert_eq!(opts.hud_size.k(), 0.7);
        assert!(apply_split_setting(
            &mut opts,
            MenuAction::SetSplitNameTags(false),
            seated
        ));
        assert!(!opts.name_tags);
        assert!(!apply_split_setting(
            &mut opts,
            MenuAction::ToggleInvertY,
            seated
        ));
    }

    /// Issue #194 B3 review: the keyboard on P2 keeps P2's own mouse settings through any Game
    /// Menu action (here Resume), never the host's.
    #[test]
    fn a_game_menu_action_keeps_the_keyboard_players_mouse_settings() {
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(1);
        o.join_keyboard();
        let seats = game::local_seats::Seats::from_owners(o, 2);
        assert_eq!(seats.keyboard_player(), Some(1));
        let mut host = game::profile::Profile::default();
        host.settings.mouse_sensitivity = 1.0;
        host.settings.invert_y = false;
        let mut han = game::profile::Profile::default().into_seat();
        han.settings.mouse_sensitivity = 2.5;
        han.settings.invert_y = true;
        let mut profiles: [Option<game::profile::Profile>; 4] = Default::default();
        profiles[seats.seat_number(1)] = Some(han);
        let mut kbm = game::kbm::Kbm::default();
        kbm.follow(super::super::split::keyboard_settings_of(
            &host,
            &profiles,
            Some(&seats),
        ));
        let mut walk = WalkSim::new(7, glam::Vec3::ZERO, None);
        let mut paused = true;
        let quit = apply_pause_action(
            &mut walk,
            ui::menu::MenuAction::Resume,
            &mut paused,
            &mut game::pause::PauseMenu::default(),
            &mut game::cheats::Session::default(),
            &mut host,
            &mut ui::shell::SessionOptions::default(),
            &mut kbm,
            &mut profiles,
            Some((&seats, &mut [][..])),
            0,
            None,
        );
        assert!(!quit);
        assert_eq!((kbm.sensitivity, kbm.invert_y), (2.5, true), "P2's own");
        // Without split the host's, as before.
        apply_pause_action::<u8>(
            &mut walk,
            ui::menu::MenuAction::Resume,
            &mut paused,
            &mut game::pause::PauseMenu::default(),
            &mut game::cheats::Session::default(),
            &mut host,
            &mut ui::shell::SessionOptions::default(),
            &mut kbm,
            &mut profiles,
            None,
            0,
            None,
        );
        assert_eq!((kbm.sensitivity, kbm.invert_y), (1.0, false));
    }

    /// A split battle for the B10 tests: P1 (seat 0, the host) and P2 (seat 1, local 1 on slot
    /// 1) on their own profiles, and each player's input.
    struct Two {
        walk: WalkSim,
        host: game::profile::Profile,
        profiles: [Option<game::profile::Profile>; 4],
        seats: game::local_seats::Seats<u8>,
        inputs: Vec<super::super::split::PlayerInput>,
    }

    fn two() -> Two {
        let mut walk = WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(1);
        o.join_pad(2);
        let seats = game::local_seats::Seats::from_owners(o, 2);
        let mut host = game::profile::Profile::default().into_seat();
        host.name = "Luke".into();
        let mut han = game::profile::Profile::default().into_seat();
        han.name = "Han".into();
        let mut profiles: [Option<game::profile::Profile>; 4] = Default::default();
        profiles[1] = Some(han);
        let p = game::netplay::PlayerProfile::from_profile(profiles[1].as_ref().unwrap());
        game::netplay::install_local_profile(&mut walk, 1, &p);
        let inputs = (0..2)
            .map(|_| super::super::split::PlayerInput::new(game::viewpoint::Viewpoint::Third))
            .collect();
        Two {
            walk,
            host,
            profiles,
            seats,
            inputs,
        }
    }

    impl Two {
        /// The Game Menu opened by local player `owner` takes `a`, as the battle loop runs it.
        fn act(&mut self, owner: usize, a: ui::menu::MenuAction) {
            let opener = acting_local(Some(owner), a);
            let Two {
                walk,
                host,
                profiles,
                seats,
                inputs,
            } = self;
            walk.with_local(opener, |w| {
                apply_pause_action(
                    w,
                    a,
                    &mut true,
                    &mut game::pause::PauseMenu::default(),
                    &mut game::cheats::Session::default(),
                    host,
                    &mut ui::shell::SessionOptions::default(),
                    &mut game::kbm::Kbm::default(),
                    profiles,
                    Some((&*seats, &mut inputs[..])),
                    opener,
                    None,
                )
            });
        }

        fn han(&self) -> &game::profile::Profile {
            self.profiles[1].as_ref().unwrap()
        }
    }

    /// Issue #194 B10: P2's invert writes P2's own profile only, the one `App.split_profiles`
    /// entry the round's record also writes, so the round-end save keeps it.
    #[test]
    fn p2s_invert_writes_p2s_profile_only_and_the_round_keeps_it() {
        let mut t = two();
        let host_before = t.host.settings.clone();
        t.act(1, ui::menu::MenuAction::ToggleInvertY);
        assert!(t.han().settings.invert_y, "P2's own");
        assert_eq!(t.host.settings, host_before, "P1's untouched");
        // The round's record writes the same entry: the change is still there after it.
        super::super::career::record_split_rounds(&mut t.profiles, &mut t.walk, &[], &[0, 1]);
        let han = t.han();
        assert_eq!(han.career.rounds, 1);
        assert!(han.settings.invert_y);
        // P1's own press is P1's, as before.
        t.act(0, ui::menu::MenuAction::ToggleInvertY);
        assert!(t.host.settings.invert_y);
        assert!(t.han().settings.invert_y);
    }

    /// Issue #194 B10: P2's assist, pad look and award bonuses are slot 1's own; the session's
    /// (the first player's) stay as they were.
    #[test]
    fn p2s_assist_sets_their_slot_and_leaves_the_session() {
        let mut t = two();
        let session = (t.walk.sim.assist_intensity, t.walk.sim.pad_look_sensitivity);
        let p1 = t.walk.sim.assist_profile(0);
        t.act(1, ui::menu::MenuAction::SetAssist(0.25));
        t.act(1, ui::menu::MenuAction::SetPadLookSensitivity(1.4));
        let slot = t
            .walk
            .sim
            .slot_assist
            .get(1)
            .copied()
            .flatten()
            .expect("slot 1's own");
        assert_eq!(slot.intensity, 0.25);
        assert_eq!(slot.pad_look_sensitivity, 1.4);
        assert_eq!(
            (t.walk.sim.assist_intensity, t.walk.sim.pad_look_sensitivity),
            session
        );
        assert_eq!(t.walk.sim.assist_profile(0), p1);
        assert_eq!(t.han().settings.assist_intensity, 0.25);
        assert_eq!(t.han().settings.pad_look_sensitivity, 1.4);
        assert_eq!(t.walk.netplay.profiles[&1].assist.intensity, 0.25);
        assert_ne!(t.host.settings.assist_intensity, 0.25);
        // Award bonuses: P2's preference, P1's slot untouched.
        let p1_bonus = t.walk.sim.slot_career_award_bonuses(0);
        t.act(1, ui::menu::MenuAction::SetCareerAwards(false));
        assert!(!t.han().settings.career_awards);
        assert!(t.host.settings.career_awards);
        assert!(!t.walk.netplay.profiles[&1].award_bonuses);
        assert_eq!(t.walk.sim.slot_career_award_bonuses(0), p1_bonus);
    }

    /// Issue #194 B10: a host row pressed by P2 writes P1's (seat 0's) profile, and P2's stays
    /// as it was; the model marks the host rows shared for P2 and shows P2's own values.
    #[test]
    fn a_host_row_from_p2_writes_p1s_profile() {
        let mut t = two();
        let han_before = t.han().settings.clone();
        t.act(1, ui::menu::MenuAction::SetDifficulty(4));
        t.act(1, ui::menu::MenuAction::SetEffectsVolume(0.3));
        assert_eq!(t.host.settings.effects_volume, 0.3);
        assert_eq!(
            t.host.settings.difficulty,
            game::shell_ia::port_difficulty(4)
        );
        assert_eq!(t.han().settings, han_before, "P2's own untouched");
        t.act(1, ui::menu::MenuAction::SetSplitNameTags(false));
        assert!(!t.host.settings.splitscreen.name_tags);
        // P2's menu: their own values, the host rows shared.
        t.act(1, ui::menu::MenuAction::ToggleInvertY);
        let mut model = ui::menu::MenuModel {
            invert_y: t.host.settings.invert_y,
            ..Default::default()
        };
        let han = t.profiles[1].clone().unwrap();
        opener_model(&mut model, &mut t.walk, &han.settings, Some(&han.name), 1);
        assert!(model.invert_y && model.shared_rows);
        assert_eq!(model.player_name, "Han");
        let mut p1 = ui::menu::MenuModel::default();
        opener_model(&mut p1, &mut t.walk, &han.settings, Some(&han.name), 0);
        assert!(!p1.shared_rows && !p1.invert_y, "P1's menu as it was");
    }

    /// Issue #194 B10: FOV is each player's own (`LocalView`), and P2's viewpoint row changes
    /// P2's profile and the view stamped on P2's pad at once; P1's stays.
    #[test]
    fn p2s_fov_and_viewpoint_are_their_own() {
        let mut t = two();
        let p1_fov = t.walk.fov_scale;
        t.act(1, ui::menu::MenuAction::SetFov(1.2));
        assert_eq!(t.walk.fov_scale, p1_fov, "P1's FOV stays");
        assert_eq!(t.walk.with_local(1, |w| w.fov_scale), 1.2);
        assert_eq!(t.han().settings.fov_scale, 1.2);
        t.act(1, ui::menu::MenuAction::ToggleViewpoint);
        assert!(t.han().settings.viewpoint.is_first());
        assert!(t.inputs[1].viewpoint().is_first());
        assert!(!t.inputs[0].viewpoint().is_first());
        assert!(!t.host.settings.viewpoint.is_first());
    }

    /// A split player's pad view toggle reaches their own profile (`split::keep_viewpoint`), so
    /// the Game Menu's Viewpoint row shows the view they play in and pressing it changes the view.
    /// P1's toggle keeps the host profile's, a seat with no profile keeps it for the round.
    #[test]
    fn a_pad_view_toggle_keeps_the_players_own_profile_in_step() {
        use game::viewpoint::Viewpoint;
        let mut t = two();
        let pad_toggle = |t: &mut Two, i: usize| {
            let v = t.inputs[i].viewpoint().toggled();
            t.inputs[i].set_viewpoint(v);
            super::super::split::keep_viewpoint(&mut t.host, &mut t.profiles, &t.seats, i, v);
        };
        pad_toggle(&mut t, 1);
        assert_eq!(
            t.han().settings.viewpoint,
            Viewpoint::First,
            "P2's profile follows the pad"
        );
        assert_eq!(
            t.host.settings.viewpoint,
            Viewpoint::Third,
            "P1's untouched"
        );
        let han = t.profiles[1].clone().unwrap();
        let mut model = ui::menu::MenuModel::default();
        opener_model(&mut model, &mut t.walk, &han.settings, Some(&han.name), 1);
        assert!(model.first_person, "the row shows the view P2 plays in");
        // Pressing the row now goes back to third person, in the profile and on the pad.
        t.act(1, ui::menu::MenuAction::ToggleViewpoint);
        assert_eq!(t.han().settings.viewpoint, Viewpoint::Third);
        assert_eq!(t.inputs[1].viewpoint(), Viewpoint::Third);
        pad_toggle(&mut t, 0);
        assert_eq!(
            t.host.settings.viewpoint,
            Viewpoint::First,
            "P1's toggle is the host's"
        );
        assert_eq!(t.han().settings.viewpoint, Viewpoint::Third);
        t.profiles[1] = None;
        pad_toggle(&mut t, 1);
        assert_eq!(
            t.inputs[1].viewpoint(),
            Viewpoint::First,
            "a seat with no profile plays on"
        );
        assert_eq!(
            t.host.settings.viewpoint,
            Viewpoint::First,
            "and never writes the host's"
        );
    }

    /// Issue #194 B10: the scope rule for the acting player: host rows act as P1, the rest as
    /// the opener.
    #[test]
    fn host_rows_act_as_the_first_player() {
        use ui::menu::MenuAction;
        assert_eq!(acting_local(Some(2), MenuAction::SetDifficulty(1)), 0);
        assert_eq!(acting_local(Some(2), MenuAction::ResetGraphics), 2);
        assert_eq!(acting_local(Some(2), MenuAction::ToggleInvertY), 2);
        assert_eq!(acting_local(Some(2), MenuAction::RequestRespawn), 2);
        assert_eq!(acting_local(None, MenuAction::ToggleInvertY), 0);
    }

    /// Issue #194: in a LAN round a Cheats row acts as its opener (cheats are per slot there);
    /// every other host row still acts as the first player.
    #[test]
    fn lan_cheat_rows_act_as_the_opener() {
        use ui::menu::MenuAction;
        assert_eq!(acting_local(Some(2), MenuAction::ToggleCheat(1)), 0);
        assert_eq!(lan_acting_local(Some(2), MenuAction::ToggleCheat(1)), 2);
        assert_eq!(lan_acting_local(None, MenuAction::ToggleCheat(1)), 0);
        assert_eq!(lan_acting_local(Some(2), MenuAction::SetDifficulty(1)), 0);
        assert_eq!(lan_acting_local(Some(2), MenuAction::ToggleInvertY), 2);
        let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
        walk.adopt_local(3);
        assert_eq!(opener_slot(&walk, Some(1)), 3);
        assert_eq!(
            opener_slot(&walk, Some(3)),
            walk.local_slot_of(0),
            "gone: the first's"
        );
        assert_eq!(opener_slot(&walk, None), walk.local_slot_of(0));
    }

    /// Issue #194 B10 review: the first player keeps the host's profile whichever seat they sit
    /// in (seat 0 empty closes up, so local 0 is seat 1), and the Controls editor's seat follows
    /// the same rule as their settings.
    #[test]
    fn the_first_player_keeps_the_hosts_profile_in_any_seat() {
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(1);
        o.join_pad(2);
        o.join_pad(3);
        o.leave(0);
        let seats = game::local_seats::Seats::from_owners(o, 2);
        assert_eq!((seats.seat_number(0), seats.seat_number(1)), (1, 2));
        let split = super::super::split::profile_seat;
        assert_eq!((split(&seats, 0), split(&seats, 1)), (0, 2));
        let mut host = game::profile::Profile::default();
        host.settings.invert_y = true;
        let mut profiles: [Option<game::profile::Profile>; 4] = Default::default();
        profiles[1] = Some(game::profile::Profile::default().into_seat());
        profiles[2] = Some(game::profile::Profile::default().into_seat());
        let own = super::super::split::seat_settings(&host, &profiles, &seats, 0);
        assert!(own.invert_y, "the host's settings");
        let own = opener_profile(&mut host, &mut profiles, Some(&seats), 0).unwrap();
        assert!(own.settings.invert_y, "the host's profile");
        let p2 = opener_profile(&mut host, &mut profiles, Some(&seats), 1).unwrap();
        assert!(!p2.settings.invert_y, "seat 2's own");
    }

    /// Issue #194 B10 review: P2's Reset to Defaults resets the host's graphics but P2's own FOV
    /// and view; P1's FOV and view stay.
    #[test]
    fn p2s_reset_graphics_resets_their_own_fov_and_view() {
        let defaults = game::profile::Settings::default();
        let mut t = two();
        t.walk.fov_scale = 1.3;
        t.host.settings.fov_scale = 1.3;
        t.host.settings.viewpoint = defaults.viewpoint.toggled();
        t.inputs[0].set_viewpoint(defaults.viewpoint.toggled());
        t.host.settings.network_meter = !defaults.network_meter;
        t.act(1, ui::menu::MenuAction::SetFov(1.2));
        t.act(1, ui::menu::MenuAction::ToggleViewpoint);
        assert_ne!(t.inputs[1].viewpoint(), defaults.viewpoint);
        t.act(1, ui::menu::MenuAction::ResetGraphics);
        assert_eq!(t.host.settings.network_meter, defaults.network_meter);
        assert_eq!(t.host.settings.fov_scale, 1.3, "P1's FOV stays");
        assert_eq!(t.host.settings.viewpoint, defaults.viewpoint.toggled());
        assert_eq!(t.walk.fov_scale, 1.3);
        assert_eq!(t.inputs[0].viewpoint(), defaults.viewpoint.toggled());
        assert_eq!(t.han().settings.fov_scale, defaults.fov_scale);
        assert_eq!(t.han().settings.viewpoint, defaults.viewpoint);
        assert_eq!(t.walk.with_local(1, |w| w.fov_scale), defaults.fov_scale);
        assert_eq!(t.inputs[1].viewpoint(), defaults.viewpoint);
    }

    /// Issue #194 B10 review: a seat with no profile (the developer flag) toggles its own view
    /// for the round, never copies P1's.
    #[test]
    fn a_seat_with_no_profile_toggles_its_own_view() {
        let mut t = two();
        t.profiles[1] = None;
        let p1 = t.inputs[0].viewpoint();
        let before = t.inputs[1].viewpoint();
        t.act(1, ui::menu::MenuAction::ToggleViewpoint);
        assert_eq!(t.inputs[1].viewpoint(), before.toggled());
        t.act(1, ui::menu::MenuAction::ToggleViewpoint);
        assert_eq!(t.inputs[1].viewpoint(), before);
        assert_eq!(t.inputs[0].viewpoint(), p1);
    }
}
