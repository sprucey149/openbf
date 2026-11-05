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

#[path = "tests/assault.rs"]
mod assault;
