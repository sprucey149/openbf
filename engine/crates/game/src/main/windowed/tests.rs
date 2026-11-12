use super::*;

pub(super) fn setup(cap: Option<u32>) -> BattleSetup {
    BattleSetup {
        seed: 42,
        difficulty: Default::default(),
        spawn: (0.0, 0.0),
        spawn_set: false,
        targets: false,
        ai: None,
        ai_cap: cap,
        team_size_pct: None,
        tickets: 250,
        tickets_set: false,
        ticket_pct: 100,
        time_limit: 0,
        space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
        cheats: Default::default(),
        teams: None,
        classes_dir: PathBuf::new(),
        mission: None,
        mission_file: None,
        player_team: 0,
        heroes: Some(core::sim::heroes::HeroRules::default()),
        dlc_heroes: false,
        score_limit: core::sim::deathmatch::ASSAULT_SCORE_LIMIT,
    }
}

fn mission() -> game::mission::MissionDef {
    ron::from_str(
        r#"(map: "bf1_nab2", mode: "con", source_game: "bf1", teams: [
        (faction: "rep", units: 20, tickets: 150), (faction: "cis", units: 20, tickets: 150)],
        ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25)"#,
    )
    .expect("mission parses")
}

#[test]
fn direct_settings_resolve_before_battle_construction() {
    use core::skill::Difficulty::{Easy, Elite};
    let settings = game::profile::Settings {
        difficulty: Elite,
        team_size_pct: 150,
        ..Default::default()
    };
    let settings = game::profile::Settings {
        reinforcement_pct: 200,
        ..settings
    };
    let mut battle = setup(Some(64));
    battle.mission = Some(mission());
    resolve_direct_settings(&mut battle, None, &settings);
    assert_eq!(battle.difficulty, Elite);
    assert_eq!(battle.ai_cap, Some(64));
    let mut battle = setup(None);
    battle.mission = Some(mission());
    resolve_direct_settings(&mut battle, Some(Easy), &settings);
    assert_eq!(battle.difficulty, Easy);
    assert_eq!(battle.ai_cap, None);
    assert_eq!(battle.team_size_pct, Some(150));
    assert_eq!(battle.ticket_pct, 200);
    // A plain `game <map>` launch (no mission) is no conquest: the
    // profile's team size and reinforcements stay out of it.
    let mut battle = setup(None);
    resolve_direct_settings(&mut battle, Some(Easy), &settings);
    assert_eq!(battle.difficulty, Easy);
    assert_eq!((battle.team_size_pct, battle.ticket_pct), (None, 100));
    let mut battle = setup(None);
    resolve_direct_settings(&mut battle, None, &game::profile::Settings::default());
    assert_eq!(battle.difficulty, core::skill::Difficulty::default());
    assert_eq!(battle.ai_cap, None, "leave the explicit CLI cap unset");
}

#[test]
fn gameplay_stick_holds_through_wobble_and_resets_on_controller_changes() {
    let mut latch = GameplayStick::<u8>::default();
    latch.select(Some(1));
    let mut previous = 0;
    let mut edges = 0;
    for y in [0.61, 0.59, 0.61, 0.34, 0.61] {
        latch.arms = input::stick_arms(0.0, y, latch.arms);
        edges += (latch.arms & !previous != 0) as u32;
        previous = latch.arms;
    }
    assert_eq!(edges, 2, "one initial edge, then one after release");
    latch.select(Some(1));
    assert_eq!(latch.arms, 1, "same controller retains the held arm");
    latch.select(Some(2));
    assert_eq!(latch.arms, 0);
    latch.arms = input::stick_arms(0.0, 0.59, latch.arms);
    assert_eq!(latch.arms, 0, "new controller must reach the on threshold");
    latch.arms = 1;
    latch.select(None);
    latch.select(Some(2));
    assert_eq!(input::stick_arms(0.0, 0.59, latch.arms), 0);
}

#[test]
fn spawn_click_tracker_distinguishes_single_and_double_clicks() {
    let mut tracker = SpawnClickTracker::default();
    let pos = [100.0, 100.0];

    // First click
    assert!(!tracker.check_double_click(SpawnTarget::Side(1), pos));

    // Second click within threshold
    tracker.step(0.1);
    assert!(tracker.check_double_click(SpawnTarget::Side(1), pos));

    // Third click is fresh single click
    tracker.step(0.05);
    assert!(!tracker.check_double_click(SpawnTarget::Side(1), pos));

    // Click on a different target is a single click
    tracker.step(0.1);
    assert!(!tracker.check_double_click(SpawnTarget::Side(2), pos));

    // Click after timeout is single click
    tracker.step(1.0);
    assert!(!tracker.check_double_click(SpawnTarget::Side(2), pos));
}

/// PR #102's clicks reach the sim only through the pad: a click leaves
/// the sim untouched and puts its pick in the next pad, a double click
/// adds the accept, and the stepped tick applies both. On the spawn map
/// the click is hit-tested at the Map element's Scale it is given.
#[test]
fn step_spawn_pointer_puts_side_and_map_picks_in_the_pad() {
    use core::sim::{buttons::SPAWN_ACCEPT, PadState, ScreenPick, SideChoice, SpawnStage};
    let setup = setup(None);
    let mut a = App::new(
        Default::default(),
        Default::default(),
        setup,
        None,
        None,
        None,
        Default::default(),
    );
    a.screen = Screen::Battle;
    let mut walk = game::walk::WalkSim::new(42, glam::Vec3::ZERO, None);
    for (x, team) in [(0.0, 0u8), (-200.0, 1), (-300.0, 1)] {
        let pos = glam::Vec3::new(x, 0.0, 0.0);
        walk.sim.add_cp(
            core::cp::CpDef {
                name: format!("cp{x}"),
                pos,
                capture: core::region::Region::sphere(pos, 10.0),
                control: None,
                value_bleed: 1,
                value_atk: [1, 1],
                value_def: [1, 1],
                spawn_nodes: vec![(pos + glam::Vec3::Z, 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(team),
        );
    }
    assert!(walk.sim.open_spawn_screens(0, true));
    walk.minimap = Some(game::minimap::MapStatic {
        sheet_dir: PathBuf::new(),
        image: None,
        bounds: Some(([0.0, 0.0], 400.0)),
        north: 0.0,
        turn: 0.0,
        posts: std::collections::HashMap::new(),
        soldiers: vec![game::minimap::ClassIcon {
            texture: None,
            scale: 1.0,
            view: [50.0; 4],
        }],
        vehicles: Vec::new(),
        droids: std::collections::HashMap::new(),
    });
    a.walk = Some((walk, render::MeshHandle::default()));
    fn sim(a: &App) -> &core::sim::Sim {
        &a.walk.as_ref().unwrap().0.sim
    }
    // One frame's pad through one tick, as the battle frame steps it.
    let tick = |a: &mut App| {
        let pad = a.kbm.pad(PadState::default());
        a.walk.as_mut().unwrap().0.step_tick(&[pad]);
        a.kbm.consumed(&pad, 1);
        pad
    };
    let k = glam::Vec3::splat(0.6);
    let scale = Some(k);
    let size = [640.0, 480.0];
    let row = |k: f32| {
        ui::spawn_draw::window_point(
            [
                (ui::spawn_draw::SIDE_STACK_X[0] + ui::spawn_draw::SIDE_STACK_X[1]) * 0.5,
                ui::spawn_draw::SIDE_STACK_TOP + k * ui::spawn_draw::ROW_PITCH + 10.0,
            ],
            size,
        )
    };

    // Side: click team 2's row. The sim is untouched until the tick.
    a.shell_pointer.moved_to(row(2.0));
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.016, scale);
    assert_eq!(sim(&a).side_choice[0], SideChoice::Team1, "no direct write");
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Side(1)));
    assert_eq!(pad.buttons & SPAWN_ACCEPT, 0, "one click selects only");
    assert_eq!(sim(&a).side_choice[0], SideChoice::Team2);
    assert_eq!(a.kbm.pad(PadState::default()).pick, None, "taken once");
    // Double click (the first click well after the last): the pick and
    // the accept in one pad.
    a.shell_pointer.click();
    a.step_spawn_pointer_at(1.0, scale);
    assert_eq!(a.kbm.pad(PadState::default()).buttons & SPAWN_ACCEPT, 0);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, scale);
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Side(1)));
    assert_ne!(pad.buttons & SPAWN_ACCEPT, 0);
    assert_eq!(sim(&a).spawn_stage[0], SpawnStage::Map);
    let walk = &a.walk.as_ref().unwrap().0;
    assert_eq!(walk.sim.units[walk.player.index as usize].team, 1);

    // Map: it opens on team 2's first post (1, x = -200); its other
    // post (2, x = -300) is clicked where the 0.6 Scale draws it.
    tick(&mut a);
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(1))
    );
    let walk = &a.walk.as_ref().unwrap().0;
    let map = walk.minimap.as_ref().unwrap().spawn_model(walk, 0);
    let at = ui::hud_map::spawn_window_point(&map, k, [-300.0, 0.0], true, size).unwrap();
    // No HUD pass, no map drawn: a click there picks nothing.
    a.shell_pointer.moved_to(at);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.5, None);
    assert_eq!(a.kbm.pad(PadState::default()).pick, None);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(1.0, scale);
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(1)),
        "no direct write"
    );
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Post(2)));
    assert_eq!(pad.buttons & SPAWN_ACCEPT, 0);
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(2))
    );
    // Double click on the map goes to the class screen.
    a.shell_pointer.click();
    a.step_spawn_pointer_at(1.0, scale);
    assert_eq!(a.kbm.pad(PadState::default()).buttons & SPAWN_ACCEPT, 0);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, scale);
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Post(2)));
    assert_ne!(pad.buttons & SPAWN_ACCEPT, 0);
    assert_eq!(sim(&a).spawn_stage[0], SpawnStage::Class);
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(2))
    );
}

#[test]
fn bf1_spawn_team_preview_keeps_the_command_post_before_unit_selection() {
    use super::spawn_feedback::Selection;
    use core::sim::{buttons, PadState, ScreenPick};
    use core::soldier::GroundRules;
    use ui::spawn_draw::SpawnPage;

    let mut walk = game::walk::WalkSim::new(42, glam::Vec3::ZERO, None);
    walk.sim.ground_rules = GroundRules::Bf1;
    let troop = &walk.teams[0];
    let class = core::unit::UnitClass {
        consts: troop.consts,
        max_health: troop.max_health,
        health_type: troop.health_type,
        weapon_class: troop.weapon_class,
        total_clips: troop.total_clips,
        weapon_slot: troop.weapon_slot,
        slots: troop.slots.clone(),
        jet: false,
        capture_posts: true,
        drops: troop.drops,
        chunk: None,
        min: 0,
        max: None,
        unlock_points: 0,
        droideka: None,
    };
    walk.sim.classes[0] = vec![class; 2];
    let preview = walk.teams[0].classes[0].clone();
    walk.teams[0].classes.push(preview);
    let at = glam::Vec3::ZERO;
    walk.sim.add_cp(
        core::cp::CpDef {
            name: "Home".into(),
            pos: at,
            capture: core::region::Region::sphere(at, 10.0),
            control: None,
            value_bleed: 1,
            value_atk: [1, 1],
            value_def: [1, 1],
            spawn_nodes: vec![(at, 0.0)],
            capture_time: 10.0,
            neutralize_time: 10.0,
        },
        Some(0),
    );
    let other_post = glam::Vec3::X * 100.0;
    let mut def = walk.sim.cps[0].def.clone();
    def.name = "Second".into();
    def.pos = other_post;
    def.capture = core::region::Region::sphere(other_post, 10.0);
    def.spawn_nodes = vec![(other_post, 0.0)];
    walk.sim.add_cp(def, Some(0));
    assert!(walk.sim.open_spawn_screens(0, true));

    let feed = game::event_feed::EventFeed::for_walk(std::path::Path::new(""), "bf1", &walk);
    let accept = PadState {
        buttons: buttons::SPAWN_ACCEPT,
        ..Default::default()
    };
    let side = game::spawn_page::page(&walk, &feed, 0).unwrap();
    assert!(matches!(side, SpawnPage::Side { .. }));
    assert_eq!(game::spawn_preview::shown(&side), Some((0, 0)));
    let starting_side = Selection::read(&walk);
    walk.step_tick(&[PadState {
        buttons: buttons::SPAWN_POST_DOWN,
        ..Default::default()
    }]);
    let other = game::spawn_page::page(&walk, &feed, 0).unwrap();
    assert_eq!(game::spawn_preview::shown(&other), Some((1, 0)));
    let changed_side = Selection::read(&walk);
    assert_eq!(
        starting_side.movement_cue(changed_side, GroundRules::Bf1),
        Some("shell_select_unit")
    );
    assert_eq!(
        starting_side.movement_cue(changed_side, GroundRules::Bf2),
        Some("shell_select_change")
    );
    assert_eq!(
        changed_side.movement_cue(changed_side, GroundRules::Bf1),
        None
    );

    walk.step_tick(&[PadState {
        pick: Some(ScreenPick::Side(0)),
        ..accept
    }]);
    let post = game::spawn_page::page(&walk, &feed, 0).unwrap();
    assert!(matches!(post, SpawnPage::Map { .. }));
    assert!(!game::spawn_preview::stands(&post));

    let first_post = Selection::read(&walk);
    walk.step_tick(&[PadState {
        pick: Some(ScreenPick::Post(1)),
        ..Default::default()
    }]);
    assert_eq!(
        first_post.movement_cue(Selection::read(&walk), GroundRules::Bf1),
        Some("shell_select_change")
    );

    walk.step_tick(&[PadState::default()]);
    walk.step_tick(&[accept]);
    let class = game::spawn_page::page(&walk, &feed, 0).unwrap();
    assert!(matches!(class, SpawnPage::Class { .. }));
    assert!(game::spawn_preview::stands(&class));
    let first_class = Selection::read(&walk);
    walk.step_tick(&[PadState {
        buttons: buttons::SPAWN_CLASS_NEXT,
        ..Default::default()
    }]);
    assert_eq!(
        first_class.movement_cue(Selection::read(&walk), GroundRules::Bf1),
        Some("shell_select_unit")
    );
}

#[test]
fn spawn_selection_cues_resolve_in_the_reported_battle_sound_banks() {
    let Some(root) = std::env::var_os("SPAWN_PREVIEW_ASSETS") else {
        eprintln!("SKIP: SPAWN_PREVIEW_ASSETS must name the owned mixed bake");
        return;
    };
    let root = std::path::PathBuf::from(root);
    for map in ["bf1_bes1", "bf2_spa7"] {
        let mut sounds = game::sounds::SoundLib::load(&root.join(map)).unwrap();
        for cue in ["shell_select_change", "shell_select_unit"] {
            assert!(!sounds.play_2d(None, cue).is_empty(), "{map}: {cue}");
        }
    }
}

#[path = "tests/assault.rs"]
mod assault;

#[test]
fn pause_audio_follows_the_battle_pause_only() {
    let mut a = App::new(
        Default::default(),
        Default::default(),
        setup(None),
        None,
        None,
        None,
        Default::default(),
    );
    assert!(!a.game_audio_paused(), "shell");
    a.paused = true;
    assert!(!a.game_audio_paused(), "shell, paused flag left over");
    a.screen = Screen::Battle;
    a.walk = Some((
        game::walk::WalkSim::new(42, glam::Vec3::ZERO, None),
        render::MeshHandle::default(),
    ));
    assert!(a.game_audio_paused(), "battle pause menu");
    a.paused = false;
    assert!(!a.game_audio_paused(), "resumed");
    a.paused = true;
    a.screen = Screen::Shell;
    assert!(!a.game_audio_paused(), "left from the pause menu");
    a.screen = Screen::Battle;
    let region = core::region::Region::sphere(glam::Vec3::ZERO, 10.0);
    a.walk.as_mut().unwrap().0.sim.landing_regions.push(region);
    assert!(a.game_audio_paused(), "space battle");
    let sim = &mut a.walk.as_mut().unwrap().0.sim;
    sim.landing_regions.clear();
    assert!(a.game_audio_paused(), "ground again");
    let ctf = core::sim::CtfState::new_space_one_flag(
        1,
        glam::Vec3::ZERO,
        glam::Vec3::X,
        glam::Vec3::NEG_X,
    );
    a.walk.as_mut().unwrap().0.sim.install_ctf(ctf);
    assert!(a.game_audio_paused(), "space CTF without landing regions");
}

/// Playtest A5: the window opens maximized; a forced size stays exact.
#[test]
fn window_attributes_maximize_unless_a_size_is_forced() {
    let a = window_attributes(None);
    assert!(a.maximized);
    assert_eq!(
        a.inner_size,
        Some(winit::dpi::LogicalSize::new(1280, 720).into())
    );
    assert!(
        window_attributes(Some((0, 600))).maximized,
        "a zero size is ignored"
    );
    let a = window_attributes(Some((800, 600)));
    assert!(!a.maximized);
    assert_eq!(
        a.inner_size,
        Some(winit::dpi::PhysicalSize::new(800, 600).into())
    );
}

/// Playtest A1: the display is held awake only in a round being played.
#[test]
fn idle_inhibit_only_in_an_unpaused_battle() {
    assert!(idle::wants_idle_inhibit(true, false));
    assert!(!idle::wants_idle_inhibit(true, true));
    assert!(!idle::wants_idle_inhibit(false, false));
    assert!(!idle::wants_idle_inhibit(false, true));
}

#[test]
fn idle_inhibit_set_is_idempotent_without_a_backend() {
    let mut i = idle::IdleInhibit::none();
    for hold in [true, true, false, false, true] {
        i.set(hold);
        assert_eq!(i.held(), hold);
    }
}

/// Playtest A6: Restart Mission plays the battle in play again next, then the
/// rest of the playlist in its order.
#[test]
fn restart_requeues_the_battle_in_play_first() {
    let row = |key: &str| ui::shell::Launch {
        key: key.to_owned(),
        game: ui::shell::Game::Bf2,
        mode: ui::shell::Mode::Con,
        armies: ["rep".to_owned(), "cis".to_owned()],
        label: key.to_owned(),
    };
    let mut playlist: std::collections::VecDeque<_> = [row("tat2"), row("hot1")].into();
    requeue_front(&mut playlist, row("cor1"));
    let keys: Vec<_> = playlist.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["cor1", "tat2", "hot1"]);
    let mut empty = std::collections::VecDeque::new();
    requeue_front(&mut empty, row("cor1"));
    assert_eq!(empty, [row("cor1")]);
}

/// #142: a pad's learned stick range scales the soldier's stick only when
/// the caller says so (on foot in a ground mode). In any seat (vehicle,
/// walker, flyer) and anywhere in a space mode the scale is the fixed
/// `STICK_FULL` on both axes whatever the pad has shown, so that input is
/// unchanged.
#[test]
fn a_seated_or_flying_player_keeps_the_fixed_stick_scale() {
    let fixed = [input::STICK_FULL; 2];
    let mut r = input::StickRange::default();
    assert_eq!(stick_full(&r, true), fixed);
    assert_eq!(stick_full(&r, false), fixed);
    r.feed(0.0, 0.8);
    assert_eq!(stick_full(&r, true)[0], input::STICK_FULL);
    assert!((stick_full(&r, true)[1] - 0.8).abs() < 1e-6);
    assert_eq!(stick_full(&r, false), fixed);
    r.feed(0.3, 0.0);
    assert_eq!(stick_full(&r, false), fixed);
}
