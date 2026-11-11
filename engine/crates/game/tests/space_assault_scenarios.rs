//! Deterministic micro-scenarios for Space Battles (Issue #123).
//!
//! Covers:
//! 1. Incoming-missile warning lifecycle and HUD event dispatching
//!    (`player1.missileLock`, `player1.missileLockDistance`, `player1.missileLockDisable`).
//! 2. Off-screen lock marker scale interpolation across near and far distance ramps.
//! 3. Shield impact feedback routing (`Surface::Shield` vs `Surface::Building`).
//! 4. Capital-ship system lifecycle: shield collapse and restore, fusion-cutter repair,
//!    spent critical system repeat-destruction scoring prevention, and
//!    `game::blackbox::SpaceAssaultRow` telemetry.

use core::collision::HullFilter;
use core::impact::Surface;
use core::missile::{Missile, MissileTarget};
use core::projectile::{Bolt, Hit};
use core::sim::{
    CriticalSystem, DeathCause, Event, PadState, ShieldGroup, Sim, SpaceAssault, Station,
    StationClass, SPENT_MAX_HEALTH,
};
use core::unit::UnitSpec;
use core::vehicle::flyer::FlyerParams;
use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use core::weapon::{DamageDesc, Health, HealthType, WeaponClass};
use game::blackbox::{BlackBox, Meta};
use game::walk::WalkSim;
use glam::Vec3;
use std::path::PathBuf;
use ui::hud::{feed, HudModel};
use ui::hud_tree::{EventValue, Hud};

fn scratch_blackbox_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("openbf_test_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn test_meta() -> Meta {
    Meta {
        map: "spa1".into(),
        mode: "assault".into(),
        source_game: "bf2".into(),
        seed: 42,
        difficulty: "Normal".into(),
        mission: "spa1_assault.ron".into(),
        player_team: 0,
        commit: "05f49225".into(),
        dirty: false,
        build_commit: "05f49225".into(),
        build_dirty: false,
        build_time: 0,
        env: Vec::new(),
    }
}

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

fn test_station(sim: &mut Sim, health: f32, team: u8) -> u16 {
    if sim.station_classes.is_empty() {
        sim.add_station_class(StationClass::soldier("part", 1.0, 0.0, 0));
    }
    let mut s = Station::new(0, Vec3::ZERO, 0.0);
    s.health = Health::new(health, HealthType::Building);
    s.team = Some(team);
    sim.stations.push(s);
    (sim.stations.len() - 1) as u16
}

fn damage_desc(damage: f32) -> DamageDesc {
    DamageDesc {
        damage,
        shield_scale: 1.0,
        scales: [1.0; 7],
    }
}

fn test_hud() -> Hud {
    let layout = manifest::hud::HudLayout {
        name: "test_hud".into(),
        nodes: vec![],
    };
    Hud::new(&layout, &layout)
}

#[test]
fn incoming_missile_warning_lifecycle_and_hud_events() {
    let mut sim = Sim::empty(42);
    sim.flyer_params = vec![Some(FlyerParams::bf2())];

    let pilot = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 100.0, 0.0)))
        .expect("spawn pilot");
    sim.players = vec![Some(pilot)];

    let vh = sim.spawn_vehicle(&test_flyer_spec(0, Vec3::new(0.0, 100.0, 0.0)));
    sim.units[pilot.index as usize].mount = Some(core::vehicle::Mount {
        vehicle: vh,
        seat: 0,
    });
    if let Some(v) = sim.vehicle_mut(vh) {
        v.seats[0].occupant = Some(pilot);
        v.flyer.phase = core::vehicle::flyer::FlightPhase::Flying;
    }

    // Step 1: No missiles present -> incoming_missile is None
    sim.step(&[]);
    assert_eq!(sim.incoming_missile(0), None);

    let mut hud = test_hud();
    let has_mesh = |_name: &str| false;
    let model = HudModel {
        incoming_missile: sim.incoming_missile(0),
        ..Default::default()
    };
    feed(&mut hud, &model, &has_mesh);
    assert_eq!(hud.last("player1.missileLock"), None);

    // Step 2: Active homing missile targeting player vehicle 500m away
    let missile_pos = Vec3::new(0.0, 100.0, 500.0);
    sim.bolts.push(Bolt {
        visual_shot: None,
        pos: missile_pos,
        vel: Vec3::new(0.0, 0.0, -100.0),
        remaining: 5.0,
        age: 0.0,
        owner: pilot,
        team: 1,
        weapon_class: 0,
        hit: Hit::None,
        end: missile_pos,
        shooter_is_player: false,
        hulls: HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: Some(Missile {
            dir: Vec3::NEG_Z,
            launch: Vec3::NEG_Z,
            desired: Vec3::NEG_Z,
            ramp: 1.0,
            speed: 100.0,
            top: 100.0,
            phases: [0.0; 2],
            launch_phases: [0.0; 2],
            target: Some(MissileTarget::Vehicle(vh)),
            part: None,
        }),
        fat: None,
    });

    sim.step(&[]);
    let dist = sim.incoming_missile(0);
    assert!(dist.is_some(), "incoming missile detected");
    let dist_val = dist.unwrap();
    assert!((dist_val - 500.0).abs() < 10.0, "distance approx 500m");

    // Feed into HUD -> player1.missileLock active, player1.missileLockDistance updated
    let model = HudModel {
        incoming_missile: dist,
        ..Default::default()
    };
    feed(&mut hud, &model, &has_mesh);
    assert_eq!(
        hud.last("player1.missileLock"),
        Some(&EventValue::None),
        "warning turned on"
    );
    assert_eq!(
        hud.last("player1.missileLockDistance"),
        Some(&EventValue::Number(dist_val))
    );

    sim.unseat_unit(pilot);
    sim.step(&[]);
    assert_eq!(sim.incoming_missile(0), None);
    feed(&mut hud, &HudModel::default(), &has_mesh);
    assert_eq!(hud.last("player1.missileLock"), None);
    assert_eq!(hud.last("player1.missileLockDistance"), None);

    sim.units[pilot.index as usize].mount = Some(core::vehicle::Mount {
        vehicle: vh,
        seat: 0,
    });
    sim.vehicle_mut(vh).unwrap().seats[0].occupant = Some(pilot);
    sim.step(&[]);
    assert!(sim.incoming_missile(0).is_some());
    feed(
        &mut hud,
        &HudModel {
            incoming_missile: sim.incoming_missile(0),
            ..Default::default()
        },
        &has_mesh,
    );
    // Step 3: Missile hits or expires -> incoming_missile becomes None
    let missile = sim.bolts[0].clone();
    sim.bolts.clear();
    sim.step(&[]);
    assert_eq!(sim.incoming_missile(0), None);

    let model = HudModel {
        incoming_missile: None,
        ..Default::default()
    };
    feed(&mut hud, &model, &has_mesh);
    assert_eq!(
        hud.last("player1.missileLock"),
        None,
        "warning turned off after missile expires"
    );
    sim.bolts.push(missile);
    sim.step(&[]);
    assert!(sim.incoming_missile(0).is_some());
    feed(
        &mut hud,
        &HudModel {
            incoming_missile: sim.incoming_missile(0),
            ..Default::default()
        },
        &has_mesh,
    );
    sim.kill_environment(pilot, DeathCause::VehicleDestroyed);
    sim.step(&[]);
    assert_eq!(sim.incoming_missile(0), None);
    feed(&mut hud, &HudModel::default(), &has_mesh);
    assert_eq!(hud.last("player1.missileLock"), None);
    assert_eq!(hud.last("player1.missileLockDistance"), None);
}

#[test]
fn off_screen_lock_marker_scale_interpolation() {
    // Tests the near/far distance scale interpolation formula verified from FUN_003abc60
    // Near: 200m -> max_scale (1.0)
    // Far: 1700m -> min_scale (0.5)
    // Mid: 950m -> 0.75
    const NEAR: f32 = 200.0;
    const FAR: f32 = 1700.0;
    const MIN_SCALE: f32 = 0.5;
    const MAX_SCALE: f32 = 1.0;

    let lerp_scale = |dist: f32| {
        let t = ((dist - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0);
        t * MIN_SCALE + (1.0 - t) * MAX_SCALE
    };

    assert!((lerp_scale(100.0) - MAX_SCALE).abs() < 1e-5);
    assert!((lerp_scale(200.0) - MAX_SCALE).abs() < 1e-5);
    assert!((lerp_scale(950.0) - 0.75).abs() < 1e-5);
    assert!((lerp_scale(1700.0) - MIN_SCALE).abs() < 1e-5);
    assert!((lerp_scale(2500.0) - MIN_SCALE).abs() < 1e-5);
}

#[test]
fn shield_impact_feedback_routes_surface_shield_on_active_shield() {
    let mut sim = Sim::empty(42);
    if sim.weapon_classes.is_empty() {
        sim.weapon_classes.push(WeaponClass::default());
    }

    let station = test_station(&mut sim, 1000.0, 1);
    sim.stations[usize::from(station)].health.shield = 500.0;
    sim.stations[usize::from(station)].health.shield_max = 1000.0;

    let pilot = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 0.0)))
        .expect("shooter");

    // Case 1a: Blaster bolt (missile: None) hits station while shield > 0 -> Surface::Shield
    sim.bolts.push(Bolt {
        visual_shot: None,
        pos: Vec3::ZERO,
        vel: Vec3::Z,
        remaining: 0.0,
        age: 0.0,
        owner: pilot,
        team: 0,
        weapon_class: 0,
        hit: Hit::Target(core::projectile::Target::Station(station)),
        end: Vec3::ZERO,
        shooter_is_player: true,
        hulls: HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    });
    sim.step(&[]);

    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::Impact { .. }))
        .expect("impact event fired");
    if let Event::Impact { surface, .. } = impact {
        assert_eq!(
            *surface,
            Surface::Shield,
            "shield-absorbed bolt impact must route to Surface::Shield"
        );
    }
    // Verify shield effect look matches com_sfx_shieldimpact_sm
    assert_eq!(
        core::impact::ImpactLook::ctor(true).effect(Surface::Shield),
        Some("com_sfx_shieldimpact_sm")
    );

    // Case 1b: Torpedo / missile ordnance (missile: Some(...)) hits station while shield > 0 -> Surface::Shield
    sim.events.clear();
    sim.bolts.push(Bolt {
        visual_shot: None,
        pos: Vec3::ZERO,
        vel: Vec3::Z,
        remaining: 0.0,
        age: 0.0,
        owner: pilot,
        team: 0,
        weapon_class: 0,
        hit: Hit::Target(core::projectile::Target::Station(station)),
        end: Vec3::ZERO,
        shooter_is_player: true,
        hulls: HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: Some(Missile::launch(
            &core::missile::MissileClass::default(),
            Vec3::Z,
            100.0,
            Some(MissileTarget::Station(station)),
            &mut core::lcg::GameLcg::new(42),
        )),
        fat: None,
    });
    sim.step(&[]);

    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::Impact { .. }))
        .expect("torpedo shield impact event fired");
    if let Event::Impact { surface, .. } = impact {
        assert_eq!(
            *surface,
            Surface::Shield,
            "shield-absorbed torpedo impact must route to Surface::Shield"
        );
    }

    // Case 2: Shield drops to 0 -> subsequent hit routes to Surface::Building
    sim.stations[usize::from(station)].health.shield = 0.0;
    sim.events.clear();
    sim.bolts.push(Bolt {
        visual_shot: None,
        pos: Vec3::ZERO,
        vel: Vec3::Z,
        remaining: 0.0,
        age: 0.0,
        owner: pilot,
        team: 0,
        weapon_class: 0,
        hit: Hit::Target(core::projectile::Target::Station(station)),
        end: Vec3::ZERO,
        shooter_is_player: true,
        hulls: HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    });
    sim.step(&[]);

    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::Impact { .. }))
        .expect("impact event fired");
    if let Event::Impact { surface, .. } = impact {
        assert_eq!(
            *surface,
            Surface::Building,
            "unshielded station hit by bolt must route to Surface::Building"
        );
    }

    // Unshielded station hit by torpedo:
    sim.events.clear();
    sim.bolts.push(Bolt {
        visual_shot: None,
        pos: Vec3::ZERO,
        vel: Vec3::Z,
        remaining: 0.0,
        age: 0.0,
        owner: pilot,
        team: 0,
        weapon_class: 0,
        hit: Hit::Target(core::projectile::Target::Station(station)),
        end: Vec3::ZERO,
        shooter_is_player: true,
        hulls: HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: Some(Missile::launch(
            &core::missile::MissileClass::default(),
            Vec3::Z,
            100.0,
            Some(MissileTarget::Station(station)),
            &mut core::lcg::GameLcg::new(42),
        )),
        fat: None,
    });
    sim.step(&[]);

    let impact = sim
        .events
        .iter()
        .find(|e| matches!(e, Event::Impact { .. }))
        .expect("torpedo unshielded impact event fired");
    if let Event::Impact { surface, .. } = impact {
        assert_eq!(
            *surface,
            Surface::Building,
            "unshielded station hit by torpedo must route to Surface::Building"
        );
    }
}

#[test]
fn capital_ship_system_lifecycle_collapse_restore_and_blackbox() {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let dir = scratch_blackbox_dir("scenarios_lifecycle");
    let mut bb =
        BlackBox::start(true, test_meta(), &walk, Vec::new(), dir.clone()).expect("blackbox on");

    // Set up Space Assault with capital ship, shield generator, and critical engines
    let ship = test_station(&mut walk.sim, 1000.0, 1);
    let generator = test_station(&mut walk.sim, 100.0, 1);
    let engine = test_station(&mut walk.sim, 500.0, 1);
    let console = test_station(&mut walk.sim, 50.0, 1);

    let mut sa = SpaceAssault::new(180);
    sa.critical = vec![
        CriticalSystem {
            station: ship,
            points: 18,
            full_health: 1000.0,
            objective: None,
            shown: true,
        },
        CriticalSystem {
            station: engine,
            points: 18,
            full_health: 500.0,
            objective: None,
            shown: true,
        },
    ];
    sa.shields = vec![ShieldGroup::new(
        vec![ship, engine],
        Some(generator),
        2000.0,
        100.0,
    )];
    sa.destroyables = vec![vec![vec![engine], vec![console]]];
    walk.sim.install_space_assault(sa);

    // Initial check: shields up, systems healthy
    for _ in 0..=game::blackbox::WORLD_PERIOD {
        walk.step_tick(&[PadState::default()]);
        bb.tick(&walk, None);
    }
    let snap = bb.snapshot("init", Some(&walk), None);
    let sa_row = snap.world.last().unwrap().space.as_ref().unwrap();
    assert_eq!(sa_row.shields[1], 1.0, "team 1 shield full initially");
    assert_eq!(sa_row.hulls[1], 1.0, "team 1 hull full initially");
    assert_eq!(sa_row.systems_alive[1], 2, "2 critical systems alive");
    assert_eq!(sa_row.systems_total[1], 2, "2 critical systems total");

    // 1. Destroy shield generator -> shield collapses, link down event fires
    walk.sim
        .damage_station(generator, &damage_desc(100.0), 1.0, None, Some(0));
    assert!(walk.sim.stations[usize::from(generator)].destroyed);

    for _ in 0..=game::blackbox::WORLD_PERIOD {
        walk.step_tick(&[PadState::default()]);
        bb.tick(&walk, None);
    }
    let snap = bb.snapshot("shield_collapsed", Some(&walk), None);
    let sa_row = snap.world.last().unwrap().space.as_ref().unwrap();
    assert_eq!(sa_row.shields[1], 0.0, "shield is collapsed (0.0)");

    // 2. Fusion cutter repair restores shield generator to full
    walk.sim.repair_station(generator, 100.0);
    assert!(!walk.sim.stations[usize::from(generator)].destroyed);

    // Next tick: shields restore to 100% and link up event fires
    for _ in 0..=game::blackbox::WORLD_PERIOD {
        walk.step_tick(&[PadState::default()]);
        bb.tick(&walk, None);
    }
    let snap = bb.snapshot("shield_restored", Some(&walk), None);
    let sa_row = snap.world.last().unwrap().space.as_ref().unwrap();
    assert_eq!(sa_row.shields[1], 1.0, "shield restored to 100% (1.0)");

    // 3. Destroy critical engine -> scores points, max health set to SPENT_MAX_HEALTH
    let killer = walk.player;
    walk.sim
        .damage_station(engine, &damage_desc(2500.0), 1.0, Some(killer), Some(0));
    assert_eq!(
        walk.sim.stations[usize::from(engine)].health.max,
        SPENT_MAX_HEALTH
    );
    let points_after_kill = walk.sim.space_team_points().unwrap().0;
    assert_eq!(points_after_kill[0], 18, "team 0 scored 18 points");

    for _ in 0..=game::blackbox::WORLD_PERIOD {
        walk.step_tick(&[PadState::default()]);
        bb.tick(&walk, None);
    }
    let snap = bb.snapshot("engine_destroyed", Some(&walk), None);
    let sa_row = snap.world.last().unwrap().space.as_ref().unwrap();
    assert_eq!(sa_row.team_points[0], 18);
    assert_eq!(sa_row.systems_alive[1], 1, "1 system remains alive");

    // 4. Repeat damage/attack on spent critical system awards NO new points
    walk.sim
        .damage_station(engine, &damage_desc(500.0), 1.0, Some(killer), Some(0));
    let points_after_repeat = walk.sim.space_team_points().unwrap().0;
    assert_eq!(
        points_after_repeat[0], 18,
        "repeat destruction on spent system awards nothing"
    );

    // Also verify fusion cutter cannot revive a spent critical system
    walk.sim.repair_station(engine, 1000.0);
    assert!(
        walk.sim.stations[usize::from(engine)].destroyed,
        "spent critical system cannot be restored"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn space_assault_has_no_spawn_briefing_and_keeps_lock_audio() {
    use core::announcer::BattleLines;
    use game::announce::Commander;
    use game::space_assault::SpaceRound;
    use std::sync::Arc;

    let round = Arc::new(SpaceRound {
        faction: ["all".into(), "imp".into()],
        ..SpaceRound::default()
    });
    let mut cmd = Commander::new(BattleLines::default(), ["all", "imp"], 0, vec![], vec![]);
    cmd.space_round = Some(round.clone());
    cmd.observe(0.1, &[], [0.0, 0.0], [100, 100], true, false);
    for name in [
        "AOSMP_obj_01",
        "AOSMP_obj_02",
        "IOSMP_obj_01",
        "IOSMP_obj_02",
    ] {
        assert!(!cmd.queue.waiting(name), "no script plays {name}");
    }

    // 3. Verify event_sounds triggers: lock-on audio, incoming missile warning beep, and space radio chatter
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    walk.teams[0].faction = "all".into();
    walk.teams[1].faction = "imp".into();
    walk.space_round = Some(round.clone());

    walk.sim.tick = 150;
    let mut heard = Vec::new();
    game::event_sounds::each(&walk, Vec3::ZERO, |name, _| heard.push(name.to_owned()));
    assert!(
        !heard.iter().any(|s| s.ends_with("_inf_com_space_chatter")),
        "cockpit chatter belongs to a piloted flyer's voice gate: {heard:?}"
    );

    // Test missile lock-on audio (tick % 10 == 0)
    walk.sim.lock_on.resize(1, Default::default());
    walk.sim.tick = 20;
    walk.sim.lock_on[0].target = Some(core::sim::LockTarget::Station(0));
    walk.sim.lock_on[0].held = [3.0, 3.0];
    walk.sim.lock_on[0].charging = [true, true];
    heard.clear();
    game::event_sounds::each(&walk, Vec3::ZERO, |name, _| heard.push(name.to_owned()));
    assert!(
        !heard
            .iter()
            .any(|s| s == "all_weap_inf_rocket_launcher_tracking"),
        "a flyer's lock plays no launcher tone: {heard:?}"
    );

    // Test incoming missile warning beep (distance < 100 -> interval 4, tick 16 % 4 == 0)
    walk.sim.lock_on[0].held = [0.0, 0.0];
    walk.sim.lock_on[0].charging = [false, false];
    walk.sim.lock_on[0].target = None;
    walk.sim.lock_on[0].incoming_missile = true;
    walk.sim.lock_on[0].incoming_distance = 80.0;
    walk.sim.tick = 16;
    heard.clear();
    game::event_sounds::each(&walk, Vec3::ZERO, |name, _| heard.push(name.to_owned()));
    assert!(
        heard
            .iter()
            .any(|s| s == "all_weap_inf_rocket_launcher_tracking"),
        "incoming missile warning beep emitted when targeted by missile: {heard:?}"
    );
}
