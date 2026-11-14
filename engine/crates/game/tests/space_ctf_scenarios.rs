//! Space One-Flag CTF end-to-end integration scenarios (Issue #126).
//!
//! Covers:
//! 1. Space One-Flag mission definitions load for all six space maps (spa1..spa9).
//! 2. Manifest layers and capture/home regions resolve across baked space levels.
//! 3. Starfighter flag pickup and carrier 3D tracking across space flight.
//! 4. Flag drop on flyer crash, re-pickup cooldown, and 15s space return timeout.
//! 5. Capture region delivery, team point scoring, and score limit victory.
//! 6. Bot objective AI steering: seeking neutral flag and delivering to enemy base.
//! 7. Space CTF HUD status: 3D hint markers and capture score counters.

use core::sim::{CtfState, FlagStatus, Sim, SPACE_CARRIED_OFFSET};
use core::unit::{Controller, UnitHandle, UnitSpec};
use core::vehicle::flyer::{FlightPhase, FlyerParams};
use core::vehicle::gun::{Aimer, GunSetup, GunWeapon};
use core::vehicle::{SeatKind, SeatSetup, VehicleHandle, VehicleKind, VehicleSpec};
use core::weapon::HealthType;
use game::mission::MissionDef;
use glam::Vec3;
use std::path::PathBuf;

fn test_flyer_spec(team: u8, pos: Vec3) -> VehicleSpec {
    VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 1500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    }
}

fn setup_test_sim() -> Sim {
    let mut s = Sim::empty(42);
    s.flyer_params = vec![Some(FlyerParams {
        bf2: true,
        max_speed: 100.0,
        min_speed: 20.0,
        mid_speed: 60.0,
        turn_rate: 1.5,
        pitch_rate: 1.5,
        bank_angle: 0.8,
        strafe_speed: 10.0,
        takeoff_speed: 25.0,
        landing_speed: 20.0,
        ..Default::default()
    })];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 1,
        guns: vec![Some(GunSetup {
            weapons: vec![GunWeapon {
                class: 0,
                infinite: true,
                reserve_clips: 0,
                fire_points: Vec::new(),
                fire_limits: Vec::new(),
                fire_pivots: Vec::new(),
            }],
            fire_point: Vec3::ZERO,
            pivot: Vec3::ZERO,
            eye: Vec3::ZERO,
            aimer: Aimer::default(),
            pilot: true,
            turn: None,
            framed: false,
            aimer_bind: glam::Quat::IDENTITY,
            weapon_aimer: None,
            follows_owner: false,
            gunner: None,
        })],
    })];
    s
}

fn spawn_pilot_in_flyer(sim: &mut Sim, team: u8, pos: Vec3) -> (UnitHandle, VehicleHandle) {
    let u = sim.spawn(&UnitSpec::trooper(team, pos)).unwrap();
    sim.unit_mut(u).unwrap().soldier.pos = pos;
    let vh = sim.spawn_vehicle(&test_flyer_spec(team, pos));
    let v = sim.vehicle_mut(vh).unwrap();
    v.flyer.phase = FlightPhase::Flying;
    assert!(sim.enter_vehicle(u, vh, 0));
    (u, vh)
}

fn step_sim(sim: &mut Sim, secs: f32) {
    let count = (secs * core::TICK_HZ as f32).round().max(1.0) as usize;
    for _ in 0..count {
        sim.step(&[]);
    }
}

#[test]
fn space_one_flag_mission_definitions_load() {
    let missions_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("missions");

    let expected_maps = [
        (
            "bf2_spa1_1flag.ron",
            "home_1flag",
            "Team1capture",
            "Team2capture",
        ),
        (
            "bf2_spa3_1flag.ron",
            "home_1flag",
            "Team1capture",
            "Team2capture",
        ),
        (
            "bf2_spa6_1flag.ron",
            "1flag_home",
            "1flag_rep_cap",
            "1flag_cis_cap",
        ),
        (
            "bf2_spa7_1flag.ron",
            "1flag_home",
            "1flag_rep_cap",
            "1flag_cis_cap",
        ),
        (
            "bf2_spa8_1flag.ron",
            "home_1flag",
            "imp_cap_1flag",
            "all_cap_1flag",
        ),
        (
            "bf2_spa9_1flag.ron",
            "home_1flag",
            "imp_cap_1flag",
            "all_cap_1flag",
        ),
    ];

    for (file, home, att_cap, def_cap) in expected_maps {
        let p = missions_dir.join(file);
        if !p.exists() {
            continue;
        }
        let m = MissionDef::load(&p).unwrap_or_else(|e| panic!("failed to load {file}: {e:#}"));
        assert_eq!(m.mode, "1flag", "mode for {file}");
        assert_eq!(m.source_game, "bf2", "source game for {file}");
        assert_eq!(m.teams.len(), 2, "teams for {file}");
        assert_eq!(m.ai_alive_cap, 16, "ai_alive_cap for {file}");
        assert!(m.landing_regions.len() >= 2, "landing regions for {file}");

        let ctf = m
            .ctf
            .as_ref()
            .unwrap_or_else(|| panic!("missing ctf for {file}"));
        assert_eq!(ctf.flag, "cmn_flag", "flag name for {file}");
        assert_eq!(ctf.home_region, home, "home region for {file}");
        assert_eq!(ctf.capture_region_att, att_cap, "att cap for {file}");
        assert_eq!(ctf.capture_region_def, def_cap, "def cap for {file}");
        assert!(ctf.hide_cps, "hide_cps for {file}");
        assert_eq!(ctf.score_limit, Some(5), "score limit for {file}");
    }
}

#[test]
fn space_one_flag_manifest_layers_and_regions_resolve() {
    let baked_root = game::assets::baked_root();
    let expected_maps = [
        ("bf2_spa1", "home_1flag", "Team1capture", "Team2capture"),
        ("bf2_spa3", "home_1flag", "Team1capture", "Team2capture"),
        ("bf2_spa6", "1flag_home", "1flag_rep_cap", "1flag_cis_cap"),
        ("bf2_spa7", "1flag_home", "1flag_rep_cap", "1flag_cis_cap"),
        ("bf2_spa8", "home_1flag", "imp_cap_1flag", "all_cap_1flag"),
        ("bf2_spa9", "home_1flag", "imp_cap_1flag", "all_cap_1flag"),
    ];

    for (map_dir, home, att_cap, def_cap) in expected_maps {
        let path = baked_root.join(map_dir);
        if !path.join("manifest.ron").exists() {
            eprintln!("SKIP {map_dir}: not baked");
            continue;
        }
        let (scene, load) = game::assets::load_battle_scene(&path, "1flag")
            .unwrap_or_else(|e| panic!("load_battle_scene {map_dir}: {e:#}"));
        assert!(load.layer.is_some(), "{map_dir} applied 1flag layer");

        let find_region = |name: &str| {
            scene
                .manifest
                .regions
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(name))
        };
        assert!(
            find_region(home).is_some(),
            "found home {home} in {map_dir}"
        );
        assert!(
            find_region(att_cap).is_some(),
            "found att cap {att_cap} in {map_dir}"
        );
        assert!(
            find_region(def_cap).is_some(),
            "found def cap {def_cap} in {map_dir}"
        );

        // Flag item instance cmn_flag exists in manifest instances
        let flag_inst = scene
            .manifest
            .instances
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case("cmn_flag"));
        assert!(flag_inst.is_some(), "found cmn_flag instance in {map_dir}");
    }
}

#[test]
fn space_one_flag_region_names_match_their_markers() {
    let baked_root = game::assets::baked_root();
    // The region RECORD name is what `GetRegion` resolves; the region's
    // `name` property is swapped on spa1/spa6 and must not drive the lookup.
    // The capture dummy object sits at the same point as its region.
    let expected = [
        (
            "bf2_spa1",
            [
                ("Team1capture", "imp_cap_marker"),
                ("Team2capture", "all_cap_marker"),
            ],
        ),
        (
            "bf2_spa6",
            [
                ("1flag_rep_cap", "1flag_rep_marker"),
                ("1flag_cis_cap", "1flag_cis_marker"),
            ],
        ),
    ];
    for (map_dir, pairs) in expected {
        let path = baked_root.join(map_dir);
        if !path.join("manifest.ron").exists() {
            eprintln!("SKIP {map_dir}: not baked");
            continue;
        }
        let (scene, _) = game::assets::load_battle_scene(&path, "1flag")
            .unwrap_or_else(|e| panic!("load_battle_scene {map_dir}: {e:#}"));
        for (region, marker) in pairs {
            let r = scene
                .manifest
                .regions
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(region))
                .unwrap_or_else(|| panic!("{map_dir}: region {region}"));
            let i = scene
                .manifest
                .instances
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(marker))
                .unwrap_or_else(|| panic!("{map_dir}: marker {marker}"));
            let at = Vec3::from(r.translation);
            let marker_at = Vec3::from(i.translation);
            assert!(
                at.distance(marker_at) < 1e-3,
                "{map_dir}: {region} at {at:?} is not {marker} at {marker_at:?}"
            );
        }
    }
}

#[test]
fn space_one_flag_pickup_and_carrier_tracking() {
    let mut sim = setup_test_sim();
    let home = Vec3::new(0.0, 100.0, 0.0);
    let att_goal = Vec3::new(1000.0, 100.0, 0.0);
    let def_goal = Vec3::new(-1000.0, 100.0, 0.0);

    let ctf = CtfState::new_space_one_flag(5, home, def_goal, att_goal);
    sim.install_ctf(ctf);

    let (pilot, vh) = spawn_pilot_in_flyer(&mut sim, 0, home + Vec3::new(5.0, 0.0, 0.0));
    step_sim(&mut sim, 0.1);

    // Flag should be picked up by pilot inside the 20m flyer pickup radius
    let ctf_state = sim.ctf.as_ref().expect("ctf installed");
    assert_eq!(
        ctf_state.flags[0].status,
        FlagStatus::Carried { carrier: pilot }
    );
    assert_eq!(ctf_state.flags[0].carrier(), Some(pilot));

    // Vehicle moves forward in space
    let v = sim.vehicle_mut(vh).unwrap();
    v.pos += Vec3::new(50.0, 0.0, 0.0);
    step_sim(&mut sim, 0.1);

    // Carried flag position tracks vehicle world position with space offset
    let ctf_state = sim.ctf.as_ref().unwrap();
    assert_eq!(
        ctf_state.flags[0].current_pos,
        sim.vehicle(vh).unwrap().pos + SPACE_CARRIED_OFFSET
    );
}

#[test]
fn space_one_flag_drop_on_flyer_crash_and_return_timeout() {
    let mut sim = setup_test_sim();
    let home = Vec3::new(0.0, 0.0, 0.0);
    let att_goal = Vec3::new(500.0, 0.0, 0.0);
    let def_goal = Vec3::new(-500.0, 0.0, 0.0);

    let ctf = CtfState::new_space_one_flag(3, home, def_goal, att_goal);
    sim.install_ctf(ctf);

    let (pilot, vh) = spawn_pilot_in_flyer(&mut sim, 0, home);
    step_sim(&mut sim, 0.1);

    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].carrier(), Some(pilot));

    // Flyer destroyed / crashes in space
    let drop_pos = Vec3::new(200.0, 50.0, -100.0);
    sim.vehicle_mut(vh).unwrap().pos = drop_pos;
    sim.destroy_vehicle(vh, None);
    step_sim(&mut sim, 0.1);

    // Flag is dropped at crash coordinates
    let ctf_state = sim.ctf.as_ref().unwrap();
    match ctf_state.flags[0].status {
        FlagStatus::Dropped {
            timer,
            pickup_cooldown,
        } => {
            assert!((timer - 15.0).abs() < 0.2);
            assert!(pickup_cooldown > 0.0);
        }
        other => panic!("expected dropped status, got {other:?}"),
    }
    assert_eq!(ctf_state.flags[0].current_pos, drop_pos);

    // Cannot re-pickup during cooldown
    let (pilot2, vh2) = spawn_pilot_in_flyer(&mut sim, 1, drop_pos);
    step_sim(&mut sim, 0.1);
    assert!(matches!(
        sim.ctf.as_ref().unwrap().flags[0].status,
        FlagStatus::Dropped { .. }
    ));

    // Wait past cooldown (0.5s total), now pilot2 can pick it up
    step_sim(&mut sim, 0.5);
    sim.vehicle_mut(vh2).unwrap().pos = drop_pos;
    step_sim(&mut sim, 0.1);
    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].carrier(), Some(pilot2));

    // Drop again and verify 15.0s return timeout
    sim.unit_mut(pilot2).unwrap().alive = false;
    step_sim(&mut sim, 0.1);
    assert!(matches!(
        sim.ctf.as_ref().unwrap().flags[0].status,
        FlagStatus::Dropped { .. }
    ));

    // Advance 15.5 seconds without pickup
    step_sim(&mut sim, 15.5);
    let ctf_state = sim.ctf.as_ref().unwrap();
    assert_eq!(ctf_state.flags[0].status, FlagStatus::AtHome);
    assert_eq!(ctf_state.flags[0].current_pos, home);
}

#[test]
fn space_one_flag_capture_scoring_and_win() {
    let mut sim = setup_test_sim();
    let home = Vec3::new(0.0, 0.0, 0.0);
    let team0_goal = Vec3::new(500.0, 0.0, 0.0);
    let team1_goal = Vec3::new(-500.0, 0.0, 0.0);

    // Score limit 2
    let ctf = CtfState::new_space_one_flag(2, home, team0_goal, team1_goal);
    sim.install_ctf(ctf);

    let (pilot, vh) = spawn_pilot_in_flyer(&mut sim, 0, home);
    step_sim(&mut sim, 0.1);
    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].carrier(), Some(pilot));

    // Fly into enemy goal
    sim.vehicle_mut(vh).unwrap().pos = team0_goal;
    step_sim(&mut sim, 0.1);

    // Score 1 point, flag returns home
    let ctf_state = sim.ctf.as_ref().unwrap();
    assert_eq!(ctf_state.team_points[0], 1);
    assert_eq!(ctf_state.flags[0].status, FlagStatus::AtHome);
    assert!(sim.outcome.is_none());

    // Pick up again and deliver 2nd capture
    sim.vehicle_mut(vh).unwrap().pos = home;
    step_sim(&mut sim, 0.1);
    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].carrier(), Some(pilot));

    sim.vehicle_mut(vh).unwrap().pos = team0_goal;
    step_sim(&mut sim, 0.1);

    // Score reaches limit -> Team 0 Victory!
    let ctf_state = sim.ctf.as_ref().unwrap();
    assert_eq!(ctf_state.team_points[0], 2);
    assert_eq!(sim.outcome, Some(0));
}

#[test]
fn space_one_flag_bots_can_carry_a_flag_without_chasing_it() {
    let mut sim = setup_test_sim();
    let home = Vec3::new(0.0, 0.0, 0.0);
    let team0_goal = Vec3::new(800.0, 0.0, 0.0);
    let team1_goal = Vec3::new(-800.0, 0.0, 0.0);

    let ctf = CtfState::new_space_one_flag(5, home, team0_goal, team1_goal);
    sim.install_ctf(ctf);
    sim.flyer_bots = true;

    // Bot flyer spawned 200m away facing away from flag
    let (bot, vh) = spawn_pilot_in_flyer(&mut sim, 0, Vec3::new(200.0, 0.0, 0.0));
    sim.unit_mut(bot).unwrap().controller = Controller::Ai;
    sim.vehicle_mut(vh).unwrap().rot = glam::Quat::from_rotation_y(std::f32::consts::PI);

    // The pilot plans a flight without using the neutral flag as a goal.
    step_sim(&mut sim, 0.1);
    let v = sim.vehicle(vh).unwrap();
    assert!(v.flyer.ai.curve.is_some());
    assert_ne!(v.flyer.ai.goal, Some(home));

    // A bot that flies through the flag carries it, but the PS2's space
    // one-flag gives `CTFOffense` weight 0 (`AIGoalWeight = 0.0` in every
    // `spa*_1flag.luac`), so it takes no capture goal.
    sim.vehicle_mut(vh).unwrap().pos = home;
    step_sim(&mut sim, 0.1);
    let occupant = sim.vehicle(vh).unwrap().seats[0].occupant.unwrap();
    assert_eq!(sim.ctf.as_ref().unwrap().flags[0].carrier(), Some(occupant));

    step_sim(&mut sim, 0.1);
    let v = sim.vehicle(vh).unwrap();
    assert_ne!(v.flyer.ai.goal, Some(team0_goal));
    assert_ne!(v.flyer.ai.goal, Some(team1_goal));
}
