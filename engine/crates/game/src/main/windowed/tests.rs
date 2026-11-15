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
        vehicle: None,
        mission_file: None,
        player_team: 0,
        heroes: Some(core::sim::heroes::HeroRules::default()),
        dlc_heroes: false,
        netplay_rows: false,
        opening_humans: [0; 2],
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

/// PR #102's and issue #128's pointer reaches the sim only through the
/// pad, in the shell's model: fresh motion over a row selects it (the pick
/// in the next pad, the sim untouched until the tick), a click on the row
/// selected accepts it, a click on another selects it, and a screen that
/// has only just opened takes no accept. On the spawn map the pointer is
/// hit-tested at the Map element's Scale it is given.
#[test]
fn spawn_pointer_hovers_selects_and_accepts_side_and_map_rows() {
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
        structures: Vec::new(),
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

    // Side: the pointer comes over team 2's row. The sim is untouched
    // until the tick.
    a.shell_pointer.moved_to(row(2.0));
    a.step_spawn_pointer_at(0.016, scale);
    assert_eq!(sim(&a).side_choice[0], SideChoice::Team1, "no direct write");
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Side(1)), "hover selects");
    assert_eq!(pad.buttons & SPAWN_ACCEPT, 0, "hover never accepts");
    assert_eq!(sim(&a).side_choice[0], SideChoice::Team2);
    assert_eq!(a.kbm.pad(PadState::default()).pick, None, "taken once");
    // A resting pointer does nothing; the row is not picked again.
    a.step_spawn_pointer_at(0.016, scale);
    assert_eq!(a.kbm.pad(PadState::default()).pick, None);
    // Clicked as the screen has only just opened: selects only.
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, scale);
    assert_eq!(a.kbm.pad(PadState::default()).buttons & SPAWN_ACCEPT, 0);
    tick(&mut a);
    // The click on the selected row accepts it.
    a.shell_pointer.click();
    a.step_spawn_pointer_at(1.0, scale);
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Side(1)));
    assert_ne!(pad.buttons & SPAWN_ACCEPT, 0, "one click accepts");
    assert_eq!(sim(&a).spawn_stage[0], SpawnStage::Map);
    let walk = &a.walk.as_ref().unwrap().0;
    assert_eq!(walk.sim.units[walk.player.index as usize].team, 1);

    // Map: it opens on team 2's first post (1, x = -200); its other
    // post (2, x = -300) is under the pointer where the 0.6 Scale draws it.
    tick(&mut a);
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(1))
    );
    let walk = &a.walk.as_ref().unwrap().0;
    let map = walk.minimap.as_ref().unwrap().spawn_model(walk, 0);
    let at = ui::hud_map::spawn_window_point(&map, k, [-300.0, 0.0], true, size).unwrap();
    let one = ui::hud_map::spawn_window_point(&map, k, [-200.0, 0.0], true, size).unwrap();
    // No HUD pass, no map drawn: the pointer finds nothing there.
    a.shell_pointer.moved_to(at);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.5, None);
    assert_eq!(a.kbm.pad(PadState::default()).pick, None);
    // A second click right after the side's accept only selects the post.
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, scale);
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Post(2)));
    assert_eq!(
        pad.buttons & SPAWN_ACCEPT,
        0,
        "a fresh screen takes no accept"
    );
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(2))
    );
    // Hovering back over post 1 selects it; clicking it goes on to the
    // class screen with it.
    a.shell_pointer.moved_to(one);
    a.step_spawn_pointer_at(1.0, scale);
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Post(1)));
    assert_eq!(pad.buttons & SPAWN_ACCEPT, 0);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, scale);
    let pad = tick(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Post(1)));
    assert_ne!(pad.buttons & SPAWN_ACCEPT, 0);
    assert_eq!(sim(&a).spawn_stage[0], SpawnStage::Class);
    assert_eq!(
        sim(&a).spawn_post_choice[0],
        Some(core::unit::SpawnPost::Post(1))
    );
    // The class screen with no line-up drawn and no HUD pass: a click there
    // finds no soldier and no help line.
    a.shell_pointer.click();
    a.step_spawn_pointer_at(1.0, scale);
    let pad = tick(&mut a);
    assert_eq!((pad.pick, pad.buttons & SPAWN_ACCEPT), (None, 0));
}

/// A double click's second click names the first click's pick even where
/// the pointer now finds another target (the class line-up slides the
/// clicked soldier to the front under it); too late or too far it is a
/// fresh click, and a new screen forgets it.
#[test]
fn a_double_click_keeps_its_first_clicks_pick() {
    let mut t = SpawnClickTracker::default();
    let pos = [100.0, 100.0];
    assert_eq!(t.double(pos), None);
    t.clicked(Some(SpawnTarget::Class(2)), pos);
    t.step(0.2);
    assert_eq!(t.double([110.0, 105.0]), Some(SpawnTarget::Class(2)));
    assert_eq!(t.double([200.0, 100.0]), None, "too far");
    t.step(0.3);
    assert_eq!(t.double(pos), None, "too late");
    t.clicked(Some(SpawnTarget::Class(1)), pos);
    t.clear();
    assert_eq!(t.double(pos), None, "a new screen");
    t.clicked(None, pos);
    assert_eq!(t.double(pos), None, "a click that picked nothing");
}

/// A battle app whose player is dead behind the spawn screen `stage`.
fn behind_spawn_screen(stage: core::sim::SpawnStage) -> App {
    let mut a = App::new(
        Default::default(),
        Default::default(),
        setup(None),
        None,
        None,
        None,
        Default::default(),
    );
    a.screen = Screen::Battle;
    let mut walk = game::walk::WalkSim::new(42, glam::Vec3::ZERO, None);
    walk.sim.spawn_stage[0] = stage;
    walk.sim.units[walk.player.index as usize].alive = false;
    a.walk = Some((walk, render::MeshHandle::default()));
    a
}

/// Issue #128 on a LAN: the pad's pick leaves the window (`Kbm::consumed`)
/// a round trip before the host's ticks show it, and a click on the row
/// just picked still accepts it. A sent pick the sim never shows is
/// forgotten after [`SENT_PICK_WAIT`]: the click then selects again.
#[test]
fn a_click_on_a_row_just_picked_accepts_before_the_host_shows_it() {
    use core::sim::{buttons::SPAWN_ACCEPT, PadState, ScreenPick, SideChoice, SpawnStage};
    let mut a = behind_spawn_screen(SpawnStage::Side);
    // The frame's pad goes out; the confirmed sim does not step.
    let send = |a: &mut App| {
        let pad = a.kbm.pad(PadState::default());
        a.kbm.consumed(&pad, 1);
        pad
    };
    let row = |k: f32| {
        ui::spawn_draw::window_point(
            [
                (ui::spawn_draw::SIDE_STACK_X[0] + ui::spawn_draw::SIDE_STACK_X[1]) * 0.5,
                ui::spawn_draw::SIDE_STACK_TOP + k * ui::spawn_draw::ROW_PITCH + 10.0,
            ],
            [640.0, 480.0],
        )
    };
    a.shell_pointer.moved_to(row(2.0));
    a.step_spawn_pointer_at(1.0, None);
    assert_eq!(send(&mut a).pick, Some(ScreenPick::Side(1)), "hover picks");
    a.step_spawn_pointer_at(0.5, None);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, None);
    let pad = send(&mut a);
    let sim = &a.walk.as_ref().unwrap().0.sim;
    assert_eq!(
        sim.side_choice[0],
        SideChoice::Team1,
        "the host shows no pick yet"
    );
    assert_eq!(pad.pick, Some(ScreenPick::Side(1)));
    assert_ne!(
        pad.buttons & SPAWN_ACCEPT,
        0,
        "the click accepts the row picked"
    );
    // Never shown: after the wait the same click only selects.
    a.step_spawn_pointer_at(SENT_PICK_WAIT + 0.1, None);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, None);
    let pad = send(&mut a);
    assert_eq!(pad.pick, Some(ScreenPick::Side(1)));
    assert_eq!(pad.buttons & SPAWN_ACCEPT, 0, "a stale pick is forgotten");
}

/// Issue #128: the class screen's window path. A click on a line-up
/// soldier where the last draw stood him (`Preview::class_at`) puts his
/// class in the next pad's pick; a click on him again spawns him.
#[test]
fn a_click_on_a_drawn_line_up_soldier_picks_his_class() {
    use core::sim::{buttons::SPAWN_ACCEPT, PadState, ScreenPick, SpawnStage};
    let mut a = behind_spawn_screen(SpawnStage::Class);
    let page = ui::spawn_draw::SpawnPage::Class {
        title: String::new(),
        lines: Vec::new(),
        back: String::new(),
        accept: String::new(),
        team: 0,
        class: 0,
        classes: vec![0, 1, 2],
        refused: 0,
    };
    let cam = render::Camera {
        fov_y: 2.0 * 0.4f32.atan(),
        ..Default::default()
    };
    let size = [640.0, 480.0];
    assert_eq!(
        a.spawn_preview.class_at([320.0, 240.0], size),
        None,
        "none drawn"
    );
    a.spawn_preview
        .draws(&Default::default(), Some(&page), &cam, 0.0);
    // A soldier the pointer finds that is not the front one.
    let (at, class) = (0..480)
        .step_by(4)
        .flat_map(|y| (0..640).step_by(4).map(move |x| [x as f32, y as f32]))
        .find_map(|p| {
            a.spawn_preview
                .class_at(p, size)
                .filter(|&c| c != 0)
                .map(|c| (p, c))
        })
        .expect("a neighbour of the front soldier under some window point");
    a.shell_pointer.moved_to(at);
    a.step_spawn_pointer_at(1.0, None);
    assert_eq!(
        a.kbm.pending_pick(),
        None,
        "hover does not pick on the line-up"
    );
    a.step_spawn_pointer_at(1.0, None);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, None);
    let pad = a.kbm.pad(PadState::default());
    assert_eq!(pad.pick, Some(ScreenPick::Class(class)));
    assert_eq!(pad.buttons & SPAWN_ACCEPT, 0, "the first click selects");
    a.kbm.consumed(&pad, 1);
    a.step_spawn_pointer_at(0.5, None);
    a.shell_pointer.click();
    a.step_spawn_pointer_at(0.1, None);
    let pad = a.kbm.pad(PadState::default());
    assert_eq!(pad.pick, Some(ScreenPick::Class(class)));
    assert_ne!(pad.buttons & SPAWN_ACCEPT, 0, "a click on him again spawns");
}

/// Issue #128's pointer rules, screen by screen: the class line-up selects
/// by click only (its ring turns under a hovering pointer), a click on the
/// selected soldier or on "Select Character" spawns, "Back" backs, and
/// nothing accepts or backs before the screen has settled.
#[test]
fn spawn_pointer_rules_select_spawn_and_back() {
    use ui::spawn_draw::Help;
    let class = |c| Some(SpawnTarget::Class(c));
    let act = spawn_pointer_act;
    // The line-up: hover does nothing, a click selects, a click on the
    // selected class spawns it.
    assert_eq!(act(false, class(2), class(0), None, false, true), None);
    assert_eq!(
        act(false, class(2), class(0), None, true, true),
        Some(SpawnPointerAct::Pick(SpawnTarget::Class(2)))
    );
    assert_eq!(
        act(false, class(2), class(2), None, true, true),
        Some(SpawnPointerAct::Accept(class(2)))
    );
    // The help lines.
    assert_eq!(
        act(false, None, class(0), Some(Help::Accept), true, true),
        Some(SpawnPointerAct::Accept(None))
    );
    assert_eq!(
        act(false, None, class(0), Some(Help::Back), true, true),
        Some(SpawnPointerAct::Back)
    );
    assert_eq!(
        act(false, None, class(0), Some(Help::Back), false, true),
        None
    );
    // Unsettled: selects only.
    assert_eq!(
        act(false, None, class(0), Some(Help::Accept), true, false),
        None
    );
    assert_eq!(
        act(false, class(0), class(0), None, true, false),
        Some(SpawnPointerAct::Pick(SpawnTarget::Class(0)))
    );
    // Side rows and posts: hover selects a row not yet selected.
    let side = |r| Some(SpawnTarget::Side(r));
    assert_eq!(
        act(true, side(1), side(0), None, false, true),
        Some(SpawnPointerAct::Pick(SpawnTarget::Side(1)))
    );
    assert_eq!(act(true, side(1), side(1), None, false, true), None);
    assert_eq!(act(true, None, side(1), None, true, true), None);
}

/// Issue #128: a click on a line-up soldier names his row. The front
/// soldier (the pick) and his neighbours each take the clicks on their own
/// body; the floor between them and the sky above take none, and so does a
/// soldier waiting unseen at the back.
#[test]
fn a_click_on_a_line_up_soldier_names_his_row() {
    use game::spawn_preview::{lineup_at, lineup_fit, lineup_row_at, ring_at, PICK_BOX};
    let size = [1280.0, 720.0];
    let (aspect, tan_v) = (size[0] / size[1], 0.4f32);
    let mut outline = game::soldier_vis::Outline::default();
    outline.add(glam::Vec3::new(-0.4, 0.0, 0.0));
    outline.add(glam::Vec3::new(0.4, 0.0, 0.0));
    let fit = lineup_fit(aspect, tan_v, &outline);
    let (n, pick) = (6, 1);
    // Where row `j`'s soldier is at `up` of his height, in window pixels.
    let point = |j: usize, up: f32| {
        let f = fit.feet(lineup_at(ring_at(j, pick, pick, n, 1.0)).0);
        let y = f.y + up * PICK_BOX[1] * fit.scale;
        [
            (f.x / f.z / (tan_v * aspect) + 1.0) * 0.5 * size[0],
            (1.0 - y / f.z / tan_v) * 0.5 * size[1],
        ]
    };
    let at = |p: [f32; 2]| lineup_row_at(p, size, tan_v, &fit, pick, pick, n, 1.0);
    for j in [pick, pick + 1, pick - 1] {
        assert_eq!(at(point(j, 0.5)), Some(j), "row {j}'s chest");
    }
    assert_eq!(at(point(pick, 1.15)), None, "above the front soldier");
    assert_eq!(at([2.0, size[1] - 2.0]), None, "the window's corner");
    // Row 4 of six waits at the back (place 5, unseen).
    assert_eq!(game::spawn_preview::ring_place(4, pick, n), 5);
    assert_ne!(at(point(4, 0.5)), Some(4));
}

/// Issue #128: the help lines' pointer boxes follow their drawn icons and
/// labels, at every aspect: "Back" at the left, "Select Character" at the
/// right of the class screen, "Select" at the right of the side screen; the
/// spawn map has none.
#[test]
fn spawn_help_lines_take_their_clicks() {
    use ui::spawn_draw::{help_at, help_icon_point, Help, SpawnPage};
    let class = SpawnPage::Class {
        title: "Rifleman".into(),
        lines: Vec::new(),
        back: "Back".into(),
        accept: "Select Character".into(),
        team: 0,
        class: 0,
        classes: vec![0, 1],
        refused: 0,
    };
    let side = SpawnPage::Side {
        title: "Select a team".into(),
        rows: vec!["A".into(), "B".into(), "Auto".into()],
        selected: 0,
        select: "Select".into(),
    };
    let map = SpawnPage::Map {
        objective: String::new(),
        legend: Default::default(),
    };
    for size in [[640.0, 480.0], [1920.0, 1080.0], [2560.0, 1080.0]] {
        let (left, right) = (help_icon_point(size, false), help_icon_point(size, true));
        assert_eq!(
            help_at(None, &class, left, size),
            Some(Help::Back),
            "{size:?}"
        );
        assert_eq!(help_at(None, &class, right, size), Some(Help::Accept));
        assert_eq!(help_at(None, &side, right, size), Some(Help::Accept));
        assert_eq!(
            help_at(None, &side, left, size),
            None,
            "no Back on the side screen"
        );
        assert_eq!(help_at(None, &map, right, size), None);
        // Along the label, inward of the icon.
        let k = size[1] / 448.0;
        let label = [right[0] - 40.0 * k, right[1]];
        assert_eq!(help_at(None, &class, label, size), Some(Help::Accept));
        assert_eq!(
            help_at(None, &class, [right[0], right[1] - 40.0 * k], size),
            None
        );
        assert_eq!(
            help_at(None, &class, [size[0] * 0.5, right[1]], size),
            None,
            "the middle of the footer"
        );
    }
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
#[ignore = "requires the owned mixed bake and missions"]
fn bespin_platforms_initial_spawn_pages_use_each_selection_once() {
    use core::sim::{buttons::SPAWN_ACCEPT, ScreenPick, SpawnStage};
    use ui::spawn_draw::SpawnPage;

    let assets = PathBuf::from(std::env::var_os("SPAWN_PREVIEW_ASSETS").unwrap());
    let (scene, _) = game::assets::load_battle_scene(&assets.join("bf1_bes1"), "con").unwrap();
    let missions = assets.join("../missions");
    for armies in [["rep", "cis"], ["all", "imp"]] {
        let mission = game::mission::pick(&missions, "bf1_bes1", "con", armies).unwrap();
        assert!(mission.own);
        let mut setup = setup(None);
        setup.classes_dir = assets.join("classes");
        setup.teams = Some(mission.mission.factions());
        setup.mission = Some(mission.mission);
        setup.player_team = mission.player_team;
        let mut walk = super::super::battle::make_walk(&scene, &setup).unwrap();
        let mut feed = game::event_feed::EventFeed::for_walk(&assets, "bf1", &walk);
        super::super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
        assert!(matches!(
            super::super::battle::hud_model(&walk, &mut feed, 16.0 / 9.0).spawn,
            Some(SpawnPage::Side { .. })
        ));

        let accept = PadState {
            buttons: SPAWN_ACCEPT,
            ..PadState::default()
        };
        walk.step_tick(&[PadState {
            pick: Some(ScreenPick::Side(mission.player_team)),
            ..accept
        }]);
        for pad in [accept, PadState::default()] {
            walk.step_tick(&[pad]);
            let model = super::super::battle::hud_model(&walk, &mut feed, 16.0 / 9.0);
            assert!(
                matches!(model.spawn, Some(SpawnPage::Map { .. })),
                "{armies:?}"
            );
            assert_eq!(model.map.unwrap().view, ui::hud_map::MapView::Spawn);
        }
        walk.step_tick(&[accept]);
        assert!(matches!(
            super::super::battle::hud_model(&walk, &mut feed, 16.0 / 9.0).spawn,
            Some(SpawnPage::Class { .. })
        ));
        walk.step_tick(&[PadState::default()]);
        walk.step_tick(&[accept]);
        assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Accepted);
        assert!(walk.sim.unit(walk.player).is_some());
    }
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
    let a = window_attributes(None, None);
    assert!(a.maximized);
    assert_eq!(
        a.inner_size,
        Some(winit::dpi::LogicalSize::new(1280, 720).into())
    );
    assert!(
        window_attributes(Some((0, 600)), None).maximized,
        "a zero size is ignored"
    );
    let a = window_attributes(Some((800, 600)), None);
    assert!(!a.maximized);
    assert_eq!(
        a.inner_size,
        Some(winit::dpi::PhysicalSize::new(800, 600).into())
    );
}

/// Issue #172: the saved fullscreen mode opens the window fullscreen, still
/// maximized beneath; OPENBF_WINDOW_SIZE keeps it a window at that size.
#[test]
fn window_attributes_open_the_saved_fullscreen_unless_a_size_is_forced() {
    let borderless = Some(winit::window::Fullscreen::Borderless(None));
    let a = window_attributes(None, borderless.clone());
    assert_eq!(a.fullscreen, borderless);
    assert!(a.maximized);
    let a = window_attributes(Some((800, 600)), borderless);
    assert_eq!(a.fullscreen, None);
    assert!(!a.maximized);
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

#[test]
fn ground_vehicle_death_waits_without_the_egui_spawn_placeholder() {
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    let mut walk = WalkSim::new(42, glam::Vec3::ZERO, None);
    walk.sim.spawn_screens = true;
    let hull = walk.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: glam::Vec3::new(0.0, 2.0, 0.0),
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let mut seat = game::vehicle_class::SeatSpec::new(SeatKind::Pilot);
    seat.track_center = Some([0.0, 4.0, -6.0]);
    seat.track_offset = Some([0.0; 3]);
    walk.vehicle_seats = vec![vec![seat]];
    walk.vehicle_tops = vec![Some(2.0)];
    assert!(walk.sim.seat_unit(walk.player, hull, 0));
    walk.step_tick(&[PadState::default()]);
    let camera = walk.camera().position;
    walk.sim.destroy_vehicle(hull, None);
    walk.step_tick(&[PadState::default()]);
    assert!(walk.holds_seat_view());
    assert!(walk.death_cam.is_none());
    assert_eq!(walk.camera().position, camera);
    let feed = game::event_feed::EventFeed::default();
    let waiting = spawn_model(&walk, &feed);
    if let Some(model) = &waiting {
        let ctx = ui::egui::Context::default();
        for _ in 0..2 {
            let output = ctx.run(Default::default(), |ctx| ui::spawn::draw_spawn(ctx, model));
            for shape in output.shapes {
                if let ui::egui::Shape::Text(text) = shape.shape {
                    println!("waiting panel: {}", text.galley.text());
                }
            }
        }
    }
    assert!(
        waiting.is_none(),
        "no legacy panel before the native spawn map"
    );
    for _ in 0..4 * core::TICK_HZ + 2 {
        walk.step_tick(&[PadState::default()]);
    }
    assert_eq!(walk.camera().position, camera);
    assert_eq!(walk.sim.spawn_stage[0], core::sim::SpawnStage::Map);
    assert!(matches!(
        spawn_model(&walk, &feed).unwrap().page,
        Some(ui::spawn_draw::SpawnPage::Map { .. })
    ));
}

/// A2: a window mode chosen on the title screen, before any profile is
/// picked, goes to the profile picked next and is saved there.
#[test]
fn a_title_screen_window_mode_goes_to_the_picked_profile() {
    let dir = std::env::temp_dir().join(format!("openbf-held-mode-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = game::profile::Store::new(dir.clone());
    // A new profile starts fullscreen; F11 on the title screen chose a window.
    let mut picked = store.create("Kaz", Default::default()).unwrap();
    assert_eq!(
        picked.settings.window_mode,
        ui::menu::WindowMode::Borderless
    );
    let mut held = Some(ui::menu::WindowMode::Windowed);
    super::display::hold_window_mode(&mut held, &mut picked);
    assert_eq!(held, None);
    assert_eq!(picked.settings.window_mode, ui::menu::WindowMode::Windowed);
    assert_eq!(
        picked.settings.last_fullscreen,
        ui::menu::WindowMode::Borderless
    );
    let id = store.list().unwrap().names[0].0;
    let back = store.select(id).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(back.settings.window_mode, ui::menu::WindowMode::Windowed);
}

/// The maintainer, 2026-11-14: a first launch (no profile yet) opens the
/// shell fullscreen; with profiles it opens in the mode of the profile the
/// picker offers first, so a player who chose a window keeps it.
#[test]
fn the_shell_opens_fullscreen_first_then_in_the_last_profiles_mode() {
    use ui::menu::WindowMode;
    let dir = std::env::temp_dir().join(format!("openbf-shell-mode-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = game::profile::Store::new(dir.clone());
    let first = super::shell_profile(&store);
    assert_eq!(first.settings.window_mode, WindowMode::Borderless);
    assert!(!first.persists());
    assert!(store.list().unwrap().names.is_empty(), "nothing created");

    for (name, saved, last) in [
        ("Win", WindowMode::Windowed, WindowMode::Borderless),
        ("Excl", WindowMode::Exclusive, WindowMode::Exclusive),
        ("Full", WindowMode::Borderless, WindowMode::Borderless),
    ] {
        let mut p = store.create(name, Default::default()).unwrap();
        p.settings.window_mode = saved;
        p.settings.last_fullscreen = last;
        p.save().unwrap();
        let shell = super::shell_profile(&store);
        assert_eq!(shell.settings.window_mode, saved);
        assert_eq!(shell.settings.last_fullscreen, last);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A single-player battle names slot 0 by the profile; a LAN round leaves it
/// to the session names; a blank profile name keeps the "Player 1" fallback.
#[test]
fn single_player_rounds_take_the_profiles_name() {
    assert_eq!(single_player_name(false, "Dawg").as_deref(), Some("Dawg"));
    assert_eq!(single_player_name(false, "  Kai ").as_deref(), Some("Kai"));
    assert_eq!(single_player_name(true, "Dawg"), None);
    assert_eq!(single_player_name(false, "   "), None);
    // The LAN name filter: at most 24 characters, no control characters.
    let long = "A".repeat(40);
    assert_eq!(
        single_player_name(false, &long).map(|n| n.chars().count()),
        Some(net::msg::MAX_NAME_CHARS)
    );
    assert_eq!(single_player_name(false, "Da\u{7}wg"), None);
}

#[test]
fn leave_battle_clears_pause_state_and_sounds() {
    let mut a = App::new(
        Default::default(),
        Default::default(),
        setup(None),
        None,
        None,
        None,
        Default::default(),
    );
    a.screen = Screen::Battle;
    a.walk = Some((
        game::walk::WalkSim::new(42, glam::Vec3::ZERO, None),
        render::MeshHandle::default(),
    ));
    a.paused = true;
    a.pause_menu.cursor = 3;
    a.front = Some(front::FrontShell::new(&PathBuf::new()));

    a.leave_battle();

    assert!(matches!(a.screen, Screen::Shell));
    assert!(!a.paused);
    assert_eq!(a.pause_menu.cursor, 0);
    assert!(a.sounds.is_none());
    assert!(a.foley.is_none());
    assert!(a.weather.is_none());
}

#[test]
fn campaign_relaunch_preserves_class_whitelist_and_side_rules() {
    let missions = std::path::Path::new("../../missions");
    let assets = std::path::Path::new("../../assets_baked");
    if !missions.join("campaign").is_dir() || !assets.join("bf2_geo1c").is_dir() {
        return;
    }
    let entry = &ui::campaign::RISE_OF_THE_EMPIRE[0];
    let probe = super::super::round_setup::campaign_setup(
        missions,
        assets,
        entry,
        42,
        core::skill::Difficulty::Normal,
    )
    .expect("campaign setup");
    let layers: &[String] = probe
        .mission
        .as_ref()
        .map(|m| m.layers.as_slice())
        .unwrap_or(&[]);
    let (scene, _) = game::assets::load_battle_scene_with_layers(
        &assets.join(entry.map),
        "campaign",
        layers,
    )
    .expect("geonosis scene loads");
    for run in 0..2 {
        let setup = super::super::round_setup::campaign_setup(
            missions,
            assets,
            entry,
            42 + run,
            core::skill::Difficulty::Normal,
        )
        .expect("campaign setup");
        let mut walk = super::super::battle::make_walk(&scene, &setup).expect("make_walk");
        super::super::battle::open_spawn_screens(&mut walk, setup.mission.as_ref());
        assert_eq!(
            walk.sim.spawn_stage[0],
            core::sim::SpawnStage::Map,
            "run {run}: Geonosis skips side select"
        );
        assert!(
            !walk.sim.ai_spawn_allowed[0],
            "run {run}: Geonosis holds ally AI spawn until player spawns"
        );
        assert_eq!(
            walk.sim.spawn_class_indices(0, 0).len(),
            1,
            "run {run}: Geonosis initially restricts player to 1 whitelisted class"
        );
    }
}
