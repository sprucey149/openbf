//! Deterministic micro-scenarios for Space Battles Issue #122:
//! Space bots: dogfighting, capital-ship attacks and rounds that finish.
//!
//! Sub-second tests covering:
//! - Plain capital-ship critical system (Station) candidate admission in pilot_threats,
//!   secondary heavy ordnance channel selection (channel == 1), and firing when aligned.
//! - Armed building (priority 15) outranking equidistant plain building (priority 10).
//! - `never_crash_when_unpiloted` keeping unpiloted flyer flying vs normal flyer
//!   entering FlightPhase::Falling with 0.5s abandoned crash timer.
//! - `game::blackbox` recording FlyerRow with threat_kind == 2 (Station) and channel == 1.

use core::sim::{Sim, Station, TurretFall};
use core::unit::{Controller, UnitHandle, UnitSpec};
use core::vehicle::flyer::{FlightPhase, FlyerParams, FollowerParams, ThreatObject, THREAT_SEEN};
use core::vehicle::gun::{Aimer, GunSetup, GunWeapon};
use core::vehicle::{Mount, SeatKind, SeatSetup, VehicleHandle, VehicleKind, VehicleSpec};
use core::weapon::{Health, HealthType, OrdKind, WeaponClass};
use game::blackbox::{BlackBox, Meta, WORLD_PERIOD};
use game::walk::WalkSim;
use glam::{Quat, Vec3};

fn test_flyer_params() -> FlyerParams {
    FlyerParams {
        bf2: true,
        never_crash_when_unpiloted: false,
        has_chunks: true,
        trick_drain: 15.0,
        trick_side_strafe: 20.0,
        min_speed: 35.0,
        mid_speed: 70.0,
        max_speed: 100.0,
        ..FlyerParams::bf2()
    }
}

fn setup_weapons_and_seats(sim: &mut Sim) {
    sim.flyer_bots = true;
    sim.flyer_paths = vec![core::vehicle::flyer::FlyerPath {
        nodes: vec![
            Vec3::new(0.0, 0.0, 50.0),
            Vec3::new(0.0, 0.0, 300.0),
            Vec3::new(0.0, 0.0, 600.0),
            Vec3::new(0.0, 0.0, 1000.0),
        ],
        ..core::vehicle::flyer::FlyerPath::default()
    }];

    // Weapon 0: primary laser cannon (low damage vs Building)
    let mut wc0 = WeaponClass::bf1_rifle();
    wc0.name = "laser".into();
    wc0.ai_range = [0.0, 300.0, 800.0];
    wc0.ai_target_kind = 0x1f; // Target persons, vehicles, buildings
    wc0.ordnance.max_damage = 50.0;
    wc0.ordnance.scales.fill(0.1);
    wc0.ordnance.kind = OrdKind::Shell;
    wc0.rounds_per_clip = 0; // infinite clip / heat based
    wc0.shot_delay = [0.2, 0.2];
    let c0 = sim.add_weapon_class(wc0);

    // Weapon 1: secondary heavy ordnance (torpedoes/missiles, huge damage vs Building)
    let mut wc1 = WeaponClass::bf1_rifle();
    wc1.name = "torpedo".into();
    wc1.ai_range = [50.0, 300.0, 800.0];
    wc1.ai_target_kind = 0x1f;
    wc1.ordnance.max_damage = 2500.0;
    wc1.ordnance.scales.fill(1.0);
    wc1.ordnance.kind = OrdKind::Missile;
    wc1.rounds_per_clip = 6;
    wc1.reload_time = 0.0;
    wc1.shot_delay = [1.0, 1.0];
    let c1 = sim.add_weapon_class(wc1);

    // Flyer seat setup: seat 0 has pilot gun with both weapon 0 and weapon 1
    sim.seat_setup = vec![Some(SeatSetup {
        radius: 2.0,
        boardable: vec![true],
        body_active: vec![],
        exit_drop: 15.0,
        ai_size: 1,
        guns: vec![Some(GunSetup {
            weapons: vec![
                GunWeapon {
                    class: c0,
                    infinite: true,
                    reserve_clips: 0,
                    fire_points: Vec::new(),
                    fire_limits: Vec::new(),
                    fire_pivots: Vec::new(),
                },
                GunWeapon {
                    class: c1,
                    infinite: false,
                    reserve_clips: 4,
                    fire_points: Vec::new(),
                    fire_limits: Vec::new(),
                    fire_pivots: Vec::new(),
                },
            ],
            fire_point: Vec3::Z,
            pivot: Vec3::ZERO,
            eye: Vec3::ZERO,
            aimer: Aimer::default(),
            pilot: true,
            turn: None,
            framed: false,
            aimer_bind: Quat::IDENTITY,
            weapon_aimer: None,
            gunner: None,
        })],
    })];
}

fn spawn_ai_flyer(sim: &mut Sim, team: u8, pos: Vec3, rot: Quat) -> (VehicleHandle, UnitHandle) {
    let spec = VehicleSpec {
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
    };
    let vh = sim.spawn_vehicle(&spec);
    sim.arm(vh.index as usize);

    let pilot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, pos)
        })
        .expect("spawn ai pilot");

    assert!(sim.seat_unit(pilot, vh, 0));
    if let Some(v) = sim.vehicle_mut(vh) {
        v.rot = rot;
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
        v.flyer.lift = 1.0;
    }
    (vh, pilot)
}

#[test]
fn ai_pilot_acquires_plain_station_selects_heavy_ordnance_and_fires_when_aligned() {
    let mut sim = Sim::empty(42);
    sim.space_assault = Some(core::sim::SpaceAssault::new(180));
    sim.flyer_params = vec![Some(test_flyer_params())];
    setup_weapons_and_seats(&mut sim);

    // Spawn AI flyer on team 0 at origin facing +Z
    let (vh, pilot) = spawn_ai_flyer(&mut sim, 0, Vec3::ZERO, Quat::IDENTITY);

    // Spawn plain capital-ship critical system (Station) on team 1 along +Z at 300m
    let station_pos = Vec3::new(0.0, 0.0, 300.0);
    let mut st = Station::new(0, station_pos, 25.0);
    st.team = Some(1);
    st.health = Health::new(5000.0, HealthType::Building);
    st.sphere_centre = station_pos;
    st.sphere_radius = 25.0;
    let st_id = sim.stations.len() as u16;
    sim.stations.push(st);

    // Step 1: candidate discovery raycast populates threat slot with THREAT_SEEN
    sim.step(&[]);

    // Advance next_think so pilot immediately locks acquired target on next think
    let tick = sim.tick;
    if let Some(v) = sim.vehicle_mut(vh) {
        v.flyer.threats.next_think = tick;
    }
    // Step 2: pilot evaluates threats and selects visible target
    sim.step(&[]);
    // Step 3: flyer AI drive runs with acquired visible threat and selects secondary heavy ordnance
    sim.step(&[]);

    let v = sim.vehicle(vh).expect("flyer exists");
    // Target candidate admission: station must be admitted and acquired as visible threat
    assert_eq!(
        v.flyer.threats.visible_object(),
        Some(ThreatObject::Station(st_id)),
        "pilot threat tracking must acquire plain station critical system"
    );

    // Weapon selection: secondary heavy ordnance (channel 1) must be chosen for heavy station damage
    assert_eq!(
        v.flyer.ai.channel, 1,
        "pilot selects secondary heavy ordnance (channel 1) for capital-ship station"
    );
    assert_eq!(
        v.flyer.ai.armed_station,
        Some(st_id),
        "armed_station tracks target station id"
    );

    // Alignment test: when nose aligns with station within firing cone and range, armed_shot is true
    assert!(
        core::ai::armed_shot(&sim, pilot, &v.flyer.ai),
        "armed_shot reports ready to fire when aligned with station"
    );
}

#[test]
fn armed_building_priority_15_outranks_equidistant_plain_building_priority_10() {
    let mut sim = Sim::empty(42);
    sim.space_assault = Some(core::sim::SpaceAssault::new(180));
    sim.flyer_params = vec![Some(test_flyer_params())];
    setup_weapons_and_seats(&mut sim);

    // AI flyer at origin facing +Z
    let (vh, _) = spawn_ai_flyer(&mut sim, 0, Vec3::ZERO, Quat::IDENTITY);

    // Equidistant targets at distance sqrt(100^2 + 300^2) ≈ 316.2m
    // Target 1: Plain building station on team 1 at (-100, 0, 300) -> PS2 relative priority 10
    let plain_pos = Vec3::new(-100.0, 0.0, 300.0);
    let mut plain_st = Station::new(0, plain_pos, 20.0);
    plain_st.team = Some(1);
    plain_st.health = Health::new(4000.0, HealthType::Building);
    plain_st.sphere_centre = plain_pos;
    plain_st.sphere_radius = 20.0;
    sim.stations.push(plain_st);

    // Target 2: Armed building turret on team 1 at (100, 0, 300) -> PS2 relative priority 15
    let armed_pos = Vec3::new(100.0, 0.0, 300.0);
    let armed_spec = VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: Some(1),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: armed_pos,
        yaw: 0.0,
        max_health: 2000.0,
        health_type: HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    };
    let armed_vh = sim.add_building_turret(
        armed_spec,
        Quat::IDENTITY,
        Some((Vec3::ZERO, 20.0)),
        1,
        (TurretFall::Hidden, Vec::new()),
    );
    sim.arm(armed_vh.index as usize);

    // Step 1: candidate discovery raycast populates threat slots
    sim.step(&[]);

    // Advance next_think so pilot ranks threat slots
    let tick = sim.tick;
    if let Some(v) = sim.vehicle_mut(vh) {
        v.flyer.threats.next_think = tick;
    }
    // Step 2: pilot scores threat slots and locks the highest priority threat
    sim.step(&[]);

    let v = sim.vehicle(vh).expect("flyer exists");
    // Armed building (priority 15) must outrank plain building (priority 10) at equal distance
    assert_eq!(
        v.flyer.threats.visible_object(),
        Some(ThreatObject::Vehicle(armed_vh)),
        "armed building (relative priority 15) must outrank equidistant plain building (priority 10)"
    );
}

#[test]
fn never_crash_when_unpiloted_keeps_flyer_from_falling_crash() {
    let mut sim = Sim::empty(42);
    let mut normal_params = test_flyer_params();
    normal_params.never_crash_when_unpiloted = false;
    let mut protected_params = test_flyer_params();
    protected_params.never_crash_when_unpiloted = true;
    sim.flyer_params = vec![Some(normal_params), Some(protected_params)];

    // 1. Normal flyer (class 0: never_crash_when_unpiloted = false)
    let normal_spec = VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 200.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    };
    let v_normal = sim.spawn_vehicle(&normal_spec);
    if let Some(v) = sim.vehicle_mut(v_normal) {
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
    }

    // Step unpiloted normal flyer: must trigger 0.5s abandoned crash into FlightPhase::Falling
    sim.step(&[]);
    let vn = sim.vehicle(v_normal).expect("normal flyer");
    assert_eq!(
        vn.flyer.phase,
        FlightPhase::Falling,
        "abandoned standard flyer enters Falling phase"
    );
    assert!(
        (vn.flyer.crash_timer - (0.5 - core::TICK_DT)).abs() < 1e-4,
        "abandoned flyer crash timer starts at 0.5s (PS2 0x3f000000) and steps by dt: got {}",
        vn.flyer.crash_timer
    );
    assert!(
        !vn.health.alive(),
        "abandoned flyer health is zeroed upon entering crash"
    );

    // 2. Transport / gunship flyer (class 1: never_crash_when_unpiloted = true)
    let protected_spec = VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 1,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 200.0, 100.0),
        yaw: 0.0,
        max_health: 2000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    };
    let v_protected = sim.spawn_vehicle(&protected_spec);
    if let Some(v) = sim.vehicle_mut(v_protected) {
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
    }

    // Step unpiloted protected flyer: must remain Flying and alive
    sim.step(&[]);
    let vp = sim.vehicle(v_protected).expect("protected flyer");
    assert_eq!(
        vp.flyer.phase,
        FlightPhase::Flying,
        "never_crash_when_unpiloted flyer remains in Flying phase when unpiloted"
    );
    assert!(
        vp.health.alive(),
        "never_crash_when_unpiloted flyer retains health when unpiloted"
    );
}

#[test]
fn blackbox_records_flyer_row_with_station_threat_and_secondary_channel() {
    let dir = std::env::temp_dir().join(format!("openbf_bb_space_bots_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let meta = Meta {
        map: "spa8".into(),
        mode: "ass".into(),
        source_game: "bf2".into(),
        seed: 42,
        difficulty: "Normal".into(),
        mission: "spa8_ass.ron".into(),
        player_team: 0,
        commit: "05f49225".into(),
        dirty: false,
        build_commit: "05f49225".into(),
        build_dirty: false,
        build_time: 0,
        env: vec![],
    };

    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let mut bb =
        BlackBox::start(true, meta, &walk, Vec::new(), dir.clone()).expect("start blackbox");

    let v_spec = VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 500.0, 0.0),
        yaw: 0.0,
        max_health: 1500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    };
    let vh = walk.sim.spawn_vehicle(&v_spec);
    walk.sim.units[walk.player.index as usize].mount = Some(Mount {
        vehicle: vh,
        seat: 0,
    });
    if let Some(v) = walk.sim.vehicle_mut(vh) {
        v.seats[0].occupant = Some(walk.player);
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.ai.channel = 1;
        v.flyer.threats.slots[0] = Some(core::vehicle::flyer::PilotThreat {
            object: ThreatObject::Station(3),
            status: THREAT_SEEN,
            tested_at: 0,
            seen_at: 0,
        });
        v.flyer.threats.visible = Some(0);
    }

    // Step past WORLD_PERIOD ticks to ensure world frame is recorded
    for _ in 0..=WORLD_PERIOD {
        walk.step_tick(&[core::sim::PadState::default()]);
        bb.tick(&walk, Some(core::sim::PadState::default()));
    }

    let snap = bb.snapshot("space_bots_test", Some(&walk), None);
    assert!(!snap.world.is_empty(), "recorded world frames");
    let wf = snap.world.last().expect("last world frame");
    let v_row = wf
        .vehicles
        .iter()
        .find(|v| v.id == vh.index)
        .expect("flyer vehicle row in blackbox frame");
    let fl = v_row.flyer.as_ref().expect("flyer telemetry row");

    // Must record threat_kind == 2 (Station) and channel == 1 (secondary heavy ordnance)
    assert_eq!(fl.threat_kind, 2, "threat_kind 2 indicates Station target");
    assert_eq!(fl.threat_id, 3, "threat_id indicates station index 3");
    assert_eq!(
        fl.channel, 1,
        "channel 1 indicates secondary ordnance selected"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_transport_fallback_admission_without_transport_paths() {
    let mut sim = Sim::empty(42);
    sim.flyer_bots = true;
    sim.flyer_params = vec![
        Some(FlyerParams {
            bf2: true,
            mid_speed: 60.0,
            max_speed: 80.0,
            follower: FollowerParams {
                class_id: core::ai::flyer_drive::TRANSPORT_CLASS,
                ..FollowerParams::default()
            },
            ..FlyerParams::default()
        }),
        Some(FlyerParams {
            bf2: true,
            mid_speed: 80.0,
            max_speed: 110.0,
            follower: FollowerParams {
                class_id: 0x1111,
                ..FollowerParams::default()
            },
            ..FlyerParams::default()
        }),
    ];
    // Add only a fighter flyer path (without TRANSPORT_CLASS)
    sim.flyer_paths = vec![core::vehicle::flyer::FlyerPath {
        enabled: true,
        max_entities: 4,
        frequency: 1.0,
        nodes: vec![
            Vec3::new(0.0, 0.0, 50.0),
            Vec3::new(0.0, 0.0, 300.0),
            Vec3::new(0.0, 0.0, 600.0),
        ],
        classes: vec![(0x1111, 4)],
        ..core::vehicle::flyer::FlyerPath::default()
    }];

    assert!(!core::ai::flyer_drive::has_transport_paths(&sim));

    // Spawn a friendly fighter of class 1 so fallback_transport_classes discovers 0x1111
    let _fighter = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 1,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(50.0, 0.0, 0.0),
        yaw: 0.0,
        max_health: 1500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });

    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 5000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let pilot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    assert!(sim.seat_unit(pilot, vh, 0));
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
        v.flyer.lift = 1.0;
    }

    let v = sim.vehicle(vh).unwrap();
    assert!(
        core::ai::flyer_drive::can_fly(&sim, v),
        "transports are admitted on general paths when map lacks explicit transport splines"
    );

    sim.step(&[]);
    let v = sim.vehicle(vh).unwrap();
    assert!(
        v.flyer.ai.cursor.is_some(),
        "transport AI joins fallback fighter spline when map lacks transport splines"
    );
}

#[test]
fn test_stuck_kill_wedged_flyer() {
    let mut sim = Sim::empty(42);
    sim.flyer_bots = true;
    sim.flyer_params = vec![Some(FlyerParams {
        bf2: true,
        mid_speed: 60.0,
        max_speed: 90.0,
        ..FlyerParams::default()
    })];
    sim.flyer_paths = vec![core::vehicle::flyer::FlyerPath {
        enabled: true,
        max_entities: 4,
        frequency: 1.0,
        nodes: vec![Vec3::new(0.0, 50.0, 100.0), Vec3::new(0.0, 50.0, 400.0)],
        ..core::vehicle::flyer::FlyerPath::default()
    }];
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(10.0, 50.0, 10.0),
        yaw: 0.0,
        max_health: 1500.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let pilot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(10.0, 50.0, 10.0))
        })
        .unwrap();
    assert!(sim.seat_unit(pilot, vh, 0));
    {
        let v = sim.vehicle_mut(vh).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.initialized = true;
        v.flyer.lift = 1.0;
        v.flyer.ai.wedges = core::ai::flyer_drive::WEDGED_GIVEUP;
    }

    assert!(sim.vehicle(vh).is_some());
    sim.step(&[]);
    assert!(
        sim.vehicle(vh).is_none(),
        "flyer wedged past WEDGED_GIVEUP threshold is destroyed by stuck-kill"
    );
    assert!(
        sim.unit(pilot).is_none_or(|u| !u.alive),
        "pilot aboard wedged BF2 flyer is recycled with the destroyed craft"
    );
}
