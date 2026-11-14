//! Regression tests for Space Battles live playtest fixes:
//! 1. Capital ship hull & shield-shell registration (`Surface::ShieldLarge`, `com_sfx_shieldimpact_xl`,
//!    `shield_impact_large` 3D sound, shield shell layer toggle, indestructible bare hulls).
//! 2. Flyer scrape/crash (`com_veh_collision_lg`), hurt, and death (`imp_weap_ord_exp_lg`) ODF sounds
//!    plus cockpit/chase/death-camera audio listener origin.
//! 3. Space `SetParticleLODBias` (`3000.0` / `15000.0`) far-fade extension for small `bounding_radius`
//!    missile flares (`com_sfx_weap_rockettrail`), smooth `LiveEffect::advance` burst path interpolation
//!    at `350 m/s`, and `render_blend` pose capture for trail-only non-laser ordnance.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::collision::{CollisionWorld, HullFilter};
use core::impact::{ImpactLook, Surface};
use core::projectile::{Bolt, Hit, Target, VisualShot};
use core::sim::{Event, Sim};
use core::unit::UnitSpec;
use core::vehicle::flyer::FlyerParams;
use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use core::weapon::{HealthType, OrdnanceClass, WeaponClass};
use game::fx::Fx;
use game::vehicle_class::{SeatSpec, VehicleSoundClass};
use game::walk::{TeamTemplate, WalkSim};
use glam::Vec3;
use std::path::{Path, PathBuf};

fn assets_dir() -> PathBuf {
    std::env::var_os("OPENBF_ASSETS_BAKED")
        .map(PathBuf::from)
        .unwrap_or_else(game::assets::baked_root)
}

fn have_space_mission(root: &Path, map: &str, mission: &Path) -> bool {
    let present = root.join(map).exists() && mission.exists();
    assert!(
        present || std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
        "missing {map} assets in {} or mission {}",
        root.display(),
        mission.display()
    );
    if !present {
        eprintln!("skipping {map}: local baked assets or mission unavailable");
    }
    present
}

#[test]
fn baked_space_missiles_hit_moving_fighter_hulls() {
    use core::missile::MissileTarget;
    use core::projectile::Shooter;
    for map in ["bf2_spa1"] {
        let root = assets_dir();
        let mission_path = missions_dir().join(format!("{map}_ass.ron"));
        if !have_space_mission(&root, map, &mission_path) {
            continue;
        }
        let mut scene = game::assets::load_scene(&root.join(map)).unwrap();
        scene.apply_mode_layer("ass").expect("the Assault layer");
        let mission = game::mission::MissionDef::load(&mission_path).unwrap();
        let mut setup = Sim::empty(99);
        let pads = game::vehicle_pads::install_with_objective(
            &mut setup,
            &scene.manifest,
            &scene.dir,
            &root.join("classes"),
            &mission.factions(),
            None,
        )
        .unwrap();
        let wc = setup
            .weapon_classes
            .iter()
            .position(|w| {
                w.ordnance.velocity == 350.0
                    && w.ordnance
                        .missile
                        .is_some_and(|m| m.turn_rate == 1.75 && m.waver_turn == 0.0)
            })
            .expect("the measured space missile class");
        let ord = setup.weapon_classes[wc].ordnance.clone();
        let targets: Vec<_> = pads
            .classes
            .iter()
            .enumerate()
            .filter(|(class, _)| setup.flyer_params[*class].is_some())
            .collect();
        assert!(targets.len() >= 8, "{map}: missing fighter classes");
        let mut shots = 0;
        for (class, name) in targets {
            if name == "all_fly_xwing_sc" {
                assert_eq!(setup.missile_target_points[class], [Vec3::ZERO]);
            }
            for speed in [70.0, 100.0, 120.0] {
                let angles: &[f32] = if speed == 70.0
                    && matches!(name.as_str(), "all_fly_ywing_sc" | "all_fly_xwing_sc")
                {
                    &[-20.0, 0.0, 20.0]
                } else {
                    &[0.0]
                };
                for &angle in angles {
                    let mut sim = Sim::empty(99);
                    sim.weapon_classes = setup.weapon_classes.clone();
                    sim.hull_parts = setup.hull_parts.clone();
                    sim.blast_setup = setup.blast_setup.clone();
                    sim.missile_target_points = setup.missile_target_points.clone();
                    let me = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
                    let vh = sim.spawn_vehicle(&VehicleSpec {
                        kind: VehicleKind::Flyer,
                        class: class as u32,
                        home_team: Some(1),
                        neutral_when_empty: false,
                        spawn_pad: None,
                        pos: Vec3::new(0.0, 1000.0, -500.0),
                        yaw: 0.0,
                        max_health: 10000.0,
                        health_type: HealthType::Vehicle,
                        seats: vec![SeatKind::Pilot],
                    });
                    let origin = Vec3::Y * 1000.0;
                    let (sin, cos) = core::fmath::sin_cos(angle.to_radians());
                    let mut bolt = Bolt::fire(
                        origin,
                        Vec3::new(sin, 0.0, -cos),
                        ord.velocity,
                        ord.life_span,
                        Shooter {
                            owner: me,
                            team: 0,
                            weapon_class: wc,
                            is_player: true,
                        },
                        None,
                        None,
                        &sim.collision,
                        HullFilter::All,
                    );
                    bolt.as_missile(
                        &ord,
                        &ord.missile.unwrap(),
                        Some(MissileTarget::Vehicle(vh)),
                        &mut sim.lcg,
                    );
                    sim.bolts.push(bolt);
                    for _ in 0..151 {
                        let v = sim.vehicle_mut(vh).unwrap();
                        v.pos.x += speed * core::TICK_DT;
                        v.vel = Vec3::X * speed;
                        sim.step(&[]);
                        if sim.bolts.is_empty() {
                            break;
                        }
                    }
                    assert!(
                        sim.vehicle(vh).unwrap().health.cur < 10000.0,
                        "{map} {name}: miss at {speed} m/s, launch angle {angle}"
                    );
                    shots += 1;
                }
            }
        }
        println!("{map}: {shots}/{shots} moving-hull shots hit");
    }
}

fn missions_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OPENBF_MISSIONS") {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions")
}

#[test]
fn baked_republic_secondary_keeps_its_lock_and_hits_moving_fighters() {
    use core::missile::MissileTarget;
    use core::sim::{buttons, PadState};
    use core::vehicle::flyer::FlightPhase;
    let root = assets_dir();
    let mission_path = missions_dir().join("bf2_spa6_1flag.ron");
    if !have_space_mission(&root, "bf2_spa6", &mission_path) {
        return;
    }
    let mut scene = game::assets::load_scene(&root.join("bf2_spa6")).unwrap();
    scene.apply_mode_layer("1flag").unwrap();
    let mission = game::mission::MissionDef::load(&mission_path).unwrap();
    let mut setup = Sim::empty(290);
    let build = game::vehicle_pads::install_with_objective(
        &mut setup,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &mission.factions(),
        None,
    )
    .unwrap();
    let own_class = build
        .classes
        .iter()
        .position(|n| n == "rep_fly_anakinstarfighter_sc")
        .unwrap();
    let wc = setup.seat_setup[own_class].as_ref().unwrap().guns[0]
        .as_ref()
        .unwrap()
        .weapons[1]
        .class;
    assert_eq!(setup.weapon_classes[wc].name, "rep_weap_fly_anakin_missile");
    let missile = setup.weapon_classes[wc].ordnance.missile.unwrap();
    assert_eq!(
        (missile.turn_rate, missile.waver_rate, missile.waver_turn),
        (0.5, 2.0, 0.25)
    );
    let mut hits = [0; 4];
    let mut cases = [0; 4];
    for (target_class, name) in build
        .classes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.starts_with("cis_fly_"))
    {
        for seed in 0..16 {
            let motion = seed as usize / 4;
            let mut sim = Sim::empty(seed);
            sim.weapon_classes = setup.weapon_classes.clone();
            sim.hull_parts = setup.hull_parts.clone();
            sim.blast_setup = setup.blast_setup.clone();
            sim.missile_target_points = setup.missile_target_points.clone();
            sim.seat_setup = setup.seat_setup.clone();
            sim.flyer_params = vec![None; build.classes.len()];
            let mut p = setup.flyer_params[own_class].clone().unwrap();
            p.min_speed = 0.0;
            p.mid_speed = 0.0;
            p.max_speed = 0.0;
            sim.flyer_params[own_class] = Some(p);
            let spec = |class, team, pos| VehicleSpec {
                kind: VehicleKind::Flyer,
                class: class as u32,
                home_team: Some(team),
                neutral_when_empty: false,
                spawn_pad: None,
                pos,
                yaw: 0.0,
                max_health: 10000.0,
                health_type: HealthType::Vehicle,
                seats: vec![SeatKind::Pilot],
            };
            let own = sim.spawn_vehicle(&spec(own_class, 0, Vec3::Y * 100.0));
            let target = sim.spawn_vehicle(&spec(target_class, 1, Vec3::new(0.0, 100.0, 500.0)));
            let me = sim.spawn(&UnitSpec::player(0, Vec3::Y * 100.0)).unwrap();
            assert!(sim.seat_unit(me, own, 0));
            sim.vehicle_mut(own).unwrap().flyer.phase = FlightPhase::Flying;
            sim.step(&[PadState {
                buttons: buttons::RELOAD,
                ..Default::default()
            }]);
            for _ in 0..100 {
                sim.step(&[PadState::default()]);
            }
            assert_eq!(sim.lock_on_charge(0, 1), Some(1.0));
            let mut rounds = 0;
            let mut guided = 0;
            for tick in 0..180 {
                let velocity = [Vec3::ZERO, Vec3::X * 35.0, Vec3::X * 70.0, Vec3::Z * 70.0][motion];
                let v = sim.vehicle_mut(target).unwrap();
                v.vel = velocity;
                v.pos += v.vel * core::TICK_DT;
                sim.step(&[PadState {
                    buttons: if tick < 12 {
                        buttons::SECONDARY_FIRE
                    } else {
                        0
                    },
                    ..Default::default()
                }]);
                for b in sim.bolts.iter().filter(|b| {
                    b.weapon_class == wc && b.visual_shot.is_some_and(|s| s.tick + 1 == sim.tick)
                }) {
                    rounds += 1;
                    guided += usize::from(
                        b.missile
                            .is_some_and(|m| m.target == Some(MissileTarget::Vehicle(target))),
                    );
                }
            }
            let hp = sim.vehicle(target).unwrap().health.cur;
            assert_eq!(
                rounds, 6,
                "{name} seed {seed}: the actual paired salvos fire"
            );
            assert_eq!(
                guided, rounds,
                "{name} seed {seed}: every round retains the locked handle"
            );
            hits[motion] += usize::from(hp < 10000.0);
            cases[motion] += 1;
        }
    }
    println!(
        "Republic hitting cases {hits:?} of {cases:?}: stationary, crossing 35/70, receding 70 m/s"
    );
    assert!(hits[1] > 0, "the secondary strikes crossing fighters");
    assert!(hits[3] > 0, "the secondary strikes receding fighters");
}

fn walk_from_sim(sim: Sim) -> WalkSim {
    let t = TeamTemplate::default_trooper();
    WalkSim::with_teams(sim, None, Vec3::ZERO, [t.clone(), t], None)
}

#[test]
fn particle_lod_bias_keeps_small_bounding_radius_missile_flares_visible_at_space_ranges() {
    // Without bias, bounding_radius = 5.0 (Spark_Emitter and 10m Flare in com_sfx_weap_rockettrail)
    // culls completely past 50 m.
    let unbiassed = game::fx::draw::view_terms_with_bias(500.0, 5.0, 1.0, 0.0);
    assert!(
        unbiassed.fade <= 0.0,
        "ground default should cull bounding_radius=5 at 500m"
    );

    // With space SetParticleLODBias(3000.0), far_distance = 3050.0 m -> full alpha at 500 m.
    assert_eq!(game::fx::draw::far_distance_with_bias(5.0, 3000.0), 3050.0);
    let biassed = game::fx::draw::view_terms_with_bias(500.0, 5.0, 1.0, 3000.0);
    assert!(
        (biassed.fade - 1.0).abs() < 1e-5,
        "expected full visibility at 500m with bias=3000, got fade={}",
        biassed.fade
    );
}

#[test]
fn flyer_scrape_death_and_hurt_events_play_odf_sounds_and_listener_tracks_flyer_camera() {
    let mut sim = Sim::empty(7);
    sim.collision = CollisionWorld::new(Vec::new(), Vec::new());
    sim.flyer_params = vec![Some(FlyerParams {
        bf2: true,
        has_chunks: true,
        ..FlyerParams::default()
    })];
    let vh = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(600.0, 180.0, -950.0),
        yaw: 0.5,
        max_health: 750.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });

    let mut walk = walk_from_sim(sim);
    walk.vehicle_seats = vec![vec![SeatSpec::new(SeatKind::Pilot)]];
    walk.vehicle_sounds = vec![VehicleSoundClass {
        collision_sound: Some("com_veh_collision_lg".to_owned()),
        hurt_sound: Some("imp_fly_hurt".to_owned()),
        death_sound: Some("imp_weap_ord_exp_lg".to_owned()),
        cockpit_sound: None,
    }];

    // Mount player in the distant flyer and verify listener moves from hangar origin to flyer camera.
    assert!(walk.sim.seat_unit(walk.player, vh, 0));
    let ears = game::event_sounds::listener(&walk);
    assert!(
        ears.pos.distance(Vec3::new(600.0, 180.0, -950.0)) < 60.0,
        "mounted listener should track flyer camera near flyer, got {:?}",
        ears.pos
    );

    walk.sim.events.clear();
    walk.sim.events.push(Event::FlyerImpact {
        vehicle: vh,
        pos: Vec3::new(600.0, 180.0, -950.0),
        normal: Vec3::Y,
        speed: 40.0,
        damage: 60.0,
        lethal: false,
    });
    walk.sim.events.push(Event::VehicleHit {
        shooter: None,
        vehicle: vh,
        damage: 60.0,
        dir: None,
    });
    walk.sim.events.push(Event::VehicleDestroyed {
        vehicle: vh,
        class: 0,
        pos: Vec3::new(600.0, 180.0, -950.0),
        vel: Vec3::ZERO,
        killer: None,
    });

    let mut played = Vec::new();
    game::event_sounds::each(&walk, ears.pos, |name, _| played.push(name.to_owned()));
    assert!(
        played.iter().any(|s| s == "com_veh_collision_lg"),
        "expected com_veh_collision_lg in {played:?}"
    );
    assert!(
        played.iter().any(|s| s == "imp_fly_hurt"),
        "expected imp_fly_hurt in {played:?}"
    );
    assert!(
        played.iter().any(|s| s == "imp_weap_ord_exp_lg"),
        "expected imp_weap_ord_exp_lg in {played:?}"
    );
}

#[test]
fn render_blend_captures_trail_only_missile_bolts_with_no_mesh() {
    let mut sim = Sim::empty(11);
    let mut look = ImpactLook::ctor(true);
    look.trail = Some("com_sfx_weap_rockettrail".to_owned());
    let wc = sim.add_weapon_class(WeaponClass {
        ordnance: OrdnanceClass {
            laser: None,
            geometry: None,
            impact: Some(look),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    });
    let shooter = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    sim.bolts.push(Bolt {
        visual_shot: Some(VisualShot { tick: 1, event: 0 }),
        pos: Vec3::new(0.0, 0.0, -10.0),
        vel: Vec3::new(0.0, 0.0, -350.0),
        remaining: 2.0,
        age: core::TICK_DT,
        owner: shooter,
        team: 0,
        weapon_class: wc,
        hit: Hit::None,
        end: Vec3::new(0.0, 0.0, -1000.0),
        shooter_is_player: true,
        hulls: HullFilter::Static,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    });

    let mut walk = walk_from_sim(sim);
    let pose0 = game::render_blend::Pose::capture(&walk);
    walk.sim.bolts[0].pos = Vec3::new(0.0, 0.0, -21.666_668);
    walk.sim.bolts[0].age = core::TICK_DT * 2.0;
    let pose1 = game::render_blend::Pose::capture(&walk);
    let mid = game::render_blend::blend(&pose0, &pose1, 0.5);
    mid.write(&mut walk);
    assert!(
        (walk.sim.bolts[0].pos.z - (-15.833_334)).abs() < 0.05,
        "expected midpoint interpolation for trail-only missile bolt, got {}",
        walk.sim.bolts[0].pos.z
    );
}

#[test]
fn baked_spa1_capital_hull_shields_and_missile_trail_interpolation() {
    let root = assets_dir();
    let map_dir = root.join("bf2_spa1");
    let mission_path = missions_dir().join("bf2_spa1_ass.ron");
    if !map_dir.exists() || !mission_path.exists() {
        return;
    }

    let mut scene = game::assets::load_scene(&map_dir).expect("load bf2_spa1");
    scene.apply_mode_layer("ass");
    let common_dir = root.join("common_bf2");
    if common_dir.join("common.ron").exists() {
        if let Ok(c) = game::assets::load_common(&common_dir) {
            scene.merge_common(c);
        }
    }
    let m = game::mission::MissionDef::load(&mission_path).expect("load bf2_spa1_ass.ron");

    let mut sim = Sim::empty(99);
    scene.install_collision(&mut sim);
    let classes_dir = root.join("classes");
    let factions = m.factions();
    let sa_def = m.space_assault.as_ref().expect("space_assault def");
    let pads = game::vehicle_pads::install_with_objective(
        &mut sim,
        &scene.manifest,
        &scene.dir,
        &classes_dir,
        &factions,
        Some((sa_def, 180)),
    )
    .expect("install_with_objective");

    // Verify TIE Interceptor sound properties loaded from real ODFs.
    let tie_idx = pads
        .classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case("imp_fly_tieinterceptor"))
        .expect("imp_fly_tieinterceptor class present");
    assert_eq!(
        pads.sounds[tie_idx].collision_sound.as_deref(),
        Some("com_veh_collision_lg")
    );
    assert_eq!(
        pads.sounds[tie_idx].death_sound.as_deref(),
        Some("imp_weap_ord_exp_lg")
    );

    // Verify non-turret Star Destroyer hull is registered in ShieldGroup with 500k shield.
    let hull_station = sim
        .stations
        .iter()
        .position(|s| sim.station_classes[usize::from(s.class)].name == "imp_cap_stardestroyer1")
        .expect("Star Destroyer hull prop registered in sim.stations")
        as u16;
    assert_eq!(
        sim.stations[usize::from(hull_station)].health.max,
        999_999.0
    );
    assert_eq!(
        sim.stations[usize::from(hull_station)].health.shield,
        500_000.0
    );
    let hull_team = sim.stations[usize::from(hull_station)]
        .team
        .expect("hull team");
    let enemy_team = 1 - hull_team;

    // Fire a bolt into the shielded hull station and verify Surface::ShieldLarge + com_sfx_shieldimpact_xl + shield_impact_large.
    let mut impact_look = ImpactLook::ctor(true);
    impact_look.trail = Some("com_sfx_weap_rockettrail".to_owned());
    let wc = sim.add_weapon_class(WeaponClass {
        ordnance: OrdnanceClass {
            max_damage: 2500.0,
            impact: Some(impact_look),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    });
    let attacker = sim
        .spawn(&UnitSpec::player(enemy_team, Vec3::ZERO))
        .unwrap();
    let hull_pos = sim.stations[usize::from(hull_station)].pos;
    sim.bolts.push(Bolt {
        visual_shot: Some(VisualShot { tick: 1, event: 0 }),
        pos: hull_pos + Vec3::Z * 10.0,
        vel: Vec3::NEG_Z * 500.0,
        remaining: 0.005,
        age: 0.0,
        owner: attacker,
        team: enemy_team,
        weapon_class: wc,
        hit: Hit::Target(Target::Station(hull_station)),
        end: hull_pos,
        shooter_is_player: false,
        hulls: HullFilter::Static,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    });

    let mut walk = walk_from_sim(sim);
    walk.particle_lod_bias = 3000.0;
    walk.step_tick(&[core::sim::PadState::default()]);

    assert!(
        walk.sim.events.iter().any(|e| matches!(
            e,
            Event::Impact {
                surface: Surface::ShieldLarge,
                ..
            }
        )),
        "hitting shielded capital hull must emit Surface::ShieldLarge, events={:?}",
        walk.sim.events
    );
    assert!(
        walk.sim.stations[usize::from(hull_station)].health.shield < 500_000.0,
        "hitting capital hull must drain shared ShieldGroup pool"
    );

    let mut fx = Fx::load(&root, &map_dir, "bf2", &["imp", "all"]);
    fx.particle_lod_bias = 3000.0;
    fx.ingest(&walk);
    let sounds = fx.take_sounds();
    assert!(
        sounds.iter().any(|(s, _)| s == "shield_impact_large"),
        "expected shield_impact_large sound queued from com_sfx_shieldimpact_xl, got {sounds:?}"
    );
    assert!(
        fx.live()
            .iter()
            .any(|e| e.name() == Some("com_sfx_shieldimpact_xl")),
        "expected live com_sfx_shieldimpact_xl effect"
    );

    // Verify smooth burst path interpolation across a 12 m step for com_sfx_weap_rockettrail.
    let mut trail_sim = Sim::empty(5);
    let mut rlook = ImpactLook::ctor(true);
    rlook.trail = Some("com_sfx_weap_rockettrail".to_owned());
    let rwc = trail_sim.add_weapon_class(WeaponClass {
        ordnance: OrdnanceClass {
            impact: Some(rlook),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    });
    let owner = trail_sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    trail_sim.tick = 1;
    let shot = Some(VisualShot { tick: 1, event: 0 });
    trail_sim.bolts.push(Bolt {
        visual_shot: shot,
        pos: Vec3::ZERO,
        vel: Vec3::new(0.0, 0.0, -360.0),
        remaining: 2.0,
        age: core::TICK_DT,
        owner,
        team: 0,
        weapon_class: rwc,
        hit: Hit::None,
        end: Vec3::new(0.0, 0.0, -1000.0),
        shooter_is_player: true,
        hulls: HullFilter::Static,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    });
    let mut twalk = walk_from_sim(trail_sim);
    let mut mfx = Fx::load(&root, &map_dir, "bf2", &["imp", "all"]);
    mfx.ingest(&twalk);
    mfx.update(core::TICK_DT);
    twalk.sim.tick = 2;
    twalk.sim.bolts[0].pos = twalk.sim.bolts[0].vel * core::TICK_DT;
    twalk.sim.bolts[0].age = core::TICK_DT * 2.0;
    mfx.ingest(&twalk);
    mfx.update(core::TICK_DT);

    let smoke = &mfx.live()[0].particles()[0];
    assert!(
        smoke.len() >= 4,
        "expected >=4 smoke particles, got {}",
        smoke.len()
    );
    let mid_band = smoke
        .iter()
        .filter(|p| p.pos.z < -2.0 && p.pos.z > -10.0)
        .count();
    assert!(
        mid_band >= 1,
        "expected interpolated smoke bursts between -2m and -10m (not clumped at 0 and -12m), positions={:?}",
        smoke.iter().map(|p| p.pos.z).collect::<Vec<_>>()
    );
}

#[test]
fn baked_spa1_tie_audio_keys_and_bomber_gunner_camera() {
    let root = assets_dir();
    let map_dir = root.join("bf2_spa1");
    let mission_path = missions_dir().join("bf2_spa1_ass.ron");
    if !map_dir.exists() || !mission_path.exists() {
        return;
    }
    let mut scene = game::assets::load_scene(&map_dir).expect("load bf2_spa1");
    scene.apply_mode_layer("ass");
    let m = game::mission::MissionDef::load(&mission_path).expect("load mission");
    let mut sim = Sim::empty(5);
    let factions = m.factions();
    let def = m.space_assault.as_ref().expect("space_assault def");
    let pads = game::vehicle_pads::install_with_objective(
        &mut sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
        Some((def, 180)),
    )
    .expect("install");
    let class = |name: &str| {
        pads.classes
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("{name}: {:?}", pads.classes))
    };
    let tie = class("imp_fly_tiefighter_sc");
    let bomber = class("imp_fly_tiebomber_sc");
    let engine = pads.engines[tie].as_ref().expect("TIE engine");
    assert!(engine.boost.iter().any(|b| b.rising));
    assert!(engine.boost.iter().any(|b| !b.rising));
    let chatter = pads.sounds[tie].cockpit_sound.clone().expect("cockpit");
    assert_eq!(chatter, "imp_inf_com_space_chatter");

    let mut sl = game::sounds::SoundLib::load_for(&map_dir, &m).expect("sound bank");
    let ears = Vec3::new(1000.0, 0.0, 0.0);
    for b in engine.boost.iter().filter(|b| b.rising) {
        assert!(
            sl.boost(u32::MAX, &b.sound, true, ears, ears).is_some(),
            "{} starts",
            b.sound
        );
    }

    let handle = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: bomber as u32,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: ears,
        yaw: 0.0,
        max_health: 750.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    let index = handle.index as usize;
    sim.arm(index);
    let mut walk = walk_from_sim(sim);
    walk.vehicle_seats = pads.seats;
    walk.vehicle_sounds = pads.sounds;
    let attitude = glam::Quat::from_rotation_x(-0.7);
    walk.sim.vehicles[index].rot = attitude;

    assert!(walk.sim.seat_unit(walk.player, handle, 1));
    let cam = walk.camera();
    let look = walk.sim.seat_look(walk.player).expect("bomber gunner gun");
    let along = attitude * core::vehicle::gun::dir(look.look.0, look.look.1);
    assert!(
        cam.forward().dot(along) > 0.95,
        "gunner view follows the craft: {} / {along}",
        cam.forward()
    );
    let behind = walk.sim.vehicles[index].pos - cam.position;
    assert!(
        behind.dot(along) > 15.0 && behind.length() < 25.0,
        "TrackOffset 20 stands behind the frame: {behind}"
    );

    let mut commander = game::announce::Commander::new(
        core::announcer::BattleLines::default(),
        ["all", "imp"],
        0,
        Vec::new(),
        Vec::new(),
    );
    commander.cockpit(&walk.sim, walk.player, &walk.vehicle_sounds);
    assert!(commander.calls_asked.is_empty(), "a gunner has no chatter");
    walk.sim.unseat_unit(walk.player);
    assert!(walk.sim.seat_unit(walk.player, handle, 0));
    let mut queued = 0;
    let mut started = Vec::new();
    let secs = 120;
    for _ in 0..secs * core::TICK_HZ as usize {
        commander.cockpit(&walk.sim, walk.player, &walk.vehicle_sounds);
        queued += commander
            .gate_calls(&mut sl, ears)
            .iter()
            .filter(|(_, r)| r.is_ok())
            .count();
        commander.queue.step(core::TICK_DT, |l| {
            let length = sl.line_secs(l);
            started.push(length);
            length
        });
        sl.tick(core::TICK_DT, ears, &mut Vec::new());
    }
    assert!(
        (8..=14).contains(&queued),
        "play_interval (12, 3) over {secs} s, got {queued}"
    );
    assert!(
        started.len() >= 5 && started.iter().all(|s| s.is_some_and(|s| s > 0.0)),
        "cockpit chatter has real takes: {started:?}"
    );
}

#[test]
fn baked_spa1_hangar_loudspeaker_plays_inside_its_region_only() {
    let root = assets_dir();
    let map_dir = root.join("bf2_spa1");
    let mission_path = missions_dir().join("bf2_spa1_ass.ron");
    if !map_dir.exists() || !mission_path.exists() {
        return;
    }
    let scene = game::assets::load_scene(&map_dir).expect("load bf2_spa1");
    let m = game::mission::MissionDef::load(&mission_path).expect("load mission");
    let triggers = game::ambience::sound_triggers(&scene.manifest);
    let imp = game::classdb::fnv1a_lowercase("imp_loudspeaker");
    let hangar = triggers
        .iter()
        .find(|t| t.name == imp)
        .expect("imp_loudspeaker region");
    let inside = hangar.region.center;
    let outside = inside + Vec3::new(0.0, 500.0, 0.0);
    assert!(hangar.region.contains(inside) && !hangar.region.contains(outside));

    let mut sl = game::sounds::SoundLib::load_for(&map_dir, &m).expect("sound bank");
    assert_eq!(sl.trigger_sound(imp, 1.0, 1), Some(1776054091));
    assert_eq!(sl.trigger_sound(imp, 0.1, 1), Some(1776054091));

    let mut walk = walk_from_sim(Sim::empty(5));
    let team = usize::from(walk.sim.unit(walk.player).expect("player").team).min(1);
    walk.teams[team].faction = "imp".into();
    walk.teams[1 - team].faction = "all".into();
    walk.sim.battle.teams[0].reinforcements = 150.0;
    walk.sim.battle.teams[1].reinforcements = 150.0;
    let index = walk.player.index as usize;
    let mut commander = game::announce::Commander::new(
        core::announcer::BattleLines::default(),
        ["all", "imp"],
        0,
        Vec::new(),
        Vec::new(),
    );
    let ears = Vec3::new(1000.0, 0.0, 0.0);

    let mut run = |walk: &mut game::walk::WalkSim, at: Vec3, secs: usize| {
        walk.sim.units[index].soldier.pos = at;
        let mut lines = Vec::new();
        for _ in 0..secs * core::TICK_HZ as usize {
            commander.loudspeakers(&walk.sim, walk.player, &triggers, &walk.teams);
            commander.gate_calls(&mut sl, ears);
            commander.queue.step(core::TICK_DT, |l| {
                let length = sl.line_secs(l);
                lines.push(length);
                length
            });
            sl.tick(core::TICK_DT, ears, &mut Vec::new());
        }
        lines
    };
    assert!(run(&mut walk, outside, 60).is_empty(), "silent outside");
    let heard = run(&mut walk, inside, 120);
    let played: Vec<f32> = heard.iter().flatten().copied().collect();
    assert!(
        played.len() >= 3 && played.iter().all(|s| *s > 0.0),
        "the hangar speaker repeats real takes: {heard:?}"
    );
}

#[test]
fn baked_spa1_bomber_gunner_steers_its_guided_rocket_and_craft_board_by_their_sphere() {
    let root = assets_dir();
    let map_dir = root.join("bf2_spa1");
    let mission_path = missions_dir().join("bf2_spa1_ass.ron");
    if !map_dir.exists() || !mission_path.exists() {
        return;
    }
    let mut scene = game::assets::load_scene(&map_dir).expect("load bf2_spa1");
    scene.apply_mode_layer("ass");
    let m = game::mission::MissionDef::load(&mission_path).expect("load mission");
    let mut sim = Sim::empty(5);
    let factions = m.factions();
    let def = m.space_assault.as_ref().expect("space_assault def");
    let pads = game::vehicle_pads::install_with_objective(
        &mut sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
        Some((def, 180)),
    )
    .expect("install");
    let class = |name: &str| {
        pads.classes
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("{name}: {:?}", pads.classes))
    };
    let bomber = class("imp_fly_tiebomber_sc");
    let tie = class("imp_fly_tiefighter_sc");
    let radius = sim.seat_setup[tie].as_ref().unwrap().radius;
    let blast = sim.blast_setup[tie].as_ref().unwrap().sphere_radius;
    assert_eq!(radius, blast, "boarding reads the class's collision sphere");
    assert!(radius > 3.0, "not the old fallback: {radius}");

    let at = Vec3::new(5000.0, 2000.0, 0.0);
    let handle = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: bomber as u32,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: at,
        yaw: 0.0,
        max_health: 750.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    let index = handle.index as usize;
    sim.arm(index);
    sim.vehicles[index].flyer.phase = core::vehicle::flyer::FlightPhase::Flying;
    let mut walk = walk_from_sim(sim);
    walk.vehicle_seats = pads.seats;
    assert!(walk.sim.seat_unit(walk.player, handle, 1));
    let setup = walk.sim.seat_setup[bomber].as_ref().unwrap();
    let gun = setup.guns[1].as_ref().expect("bomber gunner gun");
    assert!(
        walk.sim.weapon_classes[gun.weapons[0].class]
            .guided
            .is_some(),
        "the gunner's weapon is the guided rocket"
    );

    let unit = walk.player.index as usize;
    let press = |buttons: u32, rx: f32| {
        let mut pads = vec![core::sim::PadState::default(); unit + 1];
        pads[unit].buttons = buttons;
        pads[unit].rx = core::sim::PadState::quantize(rx);
        pads
    };
    for _ in 0..core::TICK_HZ as usize {
        walk.sim.step(&press(core::sim::buttons::FIRE, 0.0));
    }
    assert_eq!(walk.sim.guided.len(), 1, "the press launches one rocket");
    let rocket = walk.sim.controlled_guided(walk.player).expect("controlled");
    assert_eq!(rocket.seat.map(|s| s.0), Some(handle));
    let start = rocket.rot;
    let cam = walk.camera();
    assert!(
        cam.position.distance(at) > 1.0,
        "the view follows the rocket: {}",
        cam.position
    );
    for _ in 0..core::TICK_HZ as usize {
        walk.sim.step(&press(0, 1.0));
    }
    let rocket = walk
        .sim
        .controlled_guided(walk.player)
        .expect("still flying");
    assert!(
        rocket.rot.dot(start).abs() < 0.999,
        "the stick steers the rocket"
    );
    assert!(walk.sim.unit(walk.player).unwrap().mount.is_some());

    walk.sim
        .step(&press(core::sim::buttons::SECONDARY_FIRE, 0.0));
    walk.sim.step(&press(0, 0.0));
    assert!(walk.sim.guided.is_empty(), "the secondary button ends it");
    let mount = walk.sim.unit(walk.player).unwrap().mount.expect("back in");
    assert_eq!(mount.seat, 1);
}

fn spa1_bomber_walk() -> Option<(WalkSim, usize)> {
    let root = assets_dir();
    let map_dir = root.join("bf2_spa1");
    let mission_path = missions_dir().join("bf2_spa1_ass.ron");
    if !map_dir.exists() || !mission_path.exists() {
        return None;
    }
    let mut scene = game::assets::load_scene(&map_dir).expect("load bf2_spa1");
    scene.apply_mode_layer("ass");
    let m = game::mission::MissionDef::load(&mission_path).expect("load mission");
    let mut sim = Sim::empty(5);
    let def = m.space_assault.as_ref().expect("space_assault def");
    let pads = game::vehicle_pads::install_with_objective(
        &mut sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &m.factions(),
        Some((def, 180)),
    )
    .expect("install");
    let bomber = pads
        .classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case("imp_fly_tiebomber_sc"))
        .expect("bomber class");
    let handle = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: bomber as u32,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(3000.0, 500.0, 0.0),
        yaw: 0.0,
        max_health: 750.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    let index = handle.index as usize;
    sim.arm(index);
    let mut walk = walk_from_sim(sim);
    walk.vehicle_seats = pads.seats;
    assert!(walk.sim.seat_unit(walk.player, handle, 0));
    Some((walk, index))
}

#[test]
fn baked_spa1_bomber_seat_switch_runs_the_ps2_handover_blend() {
    use core::sim::{buttons, PadState};
    use glam::{Mat3, Quat};
    let Some((mut walk, index)) = spa1_bomber_walk() else {
        return;
    };
    let pose = |walk: &WalkSim| {
        let cam = walk.camera();
        let up = cam.right().cross(cam.forward());
        let rot = Quat::from_mat3(&Mat3::from_cols(cam.right(), up, -cam.forward()));
        (cam.position, rot)
    };
    let r1 = PadState {
        buttons: buttons::SWITCH_WEAPON,
        ..Default::default()
    };
    let wait = |walk: &mut WalkSim| {
        for _ in 0..90 {
            walk.step_tick(&[PadState::default()]);
        }
    };
    walk.sim.vehicles[index].rot = Quat::from_rotation_z(0.6);
    wait(&mut walk);
    let pilot = pose(&walk);
    walk.step_tick(&[r1]);
    assert_eq!(walk.sim.unit(walk.player).unwrap().mount.unwrap().seat, 1);
    wait(&mut walk);
    let gunner = pose(&walk);
    let v = &walk.sim.vehicles[index];
    let local = v.rot.inverse() * (gunner.0 - v.pos);
    assert!(
        local.abs_diff_eq(Vec3::new(0.0, -3.47, -19.70), 0.05),
        "TrackOffset 20 under TiltValue -10: {local}"
    );
    walk.step_tick(&[r1]);
    assert_eq!(walk.sim.unit(walk.player).unwrap().mount.unwrap().seat, 0);
    wait(&mut walk);
    assert!(pose(&walk).0.abs_diff_eq(pilot.0, 0.1));
    let pilot = pose(&walk);
    walk.step_tick(&[r1]);
    let held = pose(&walk);
    assert!(
        held.0.abs_diff_eq(pilot.0, 1e-3),
        "the handover's first frame is the last camera"
    );
    assert!(held.1.angle_between(pilot.1) < 1e-3);
    walk.step_tick(&[PadState::default()]);
    let f = 2.0 * core::TICK_DT;
    let seat = gunner.0 + (pilot.0 - gunner.0) * (-30.0f32 * core::TICK_DT).exp();
    let mid = (pilot.0 + seat) * 0.5
        + pilot.1.slerp(gunner.1, 0.5) * Vec3::Z * (pilot.0.distance(seat) * 0.5 * (1.0 - f));
    let expected = pilot.0 + (mid - pilot.0) * f;
    let got = pose(&walk);
    assert!(got.0.abs_diff_eq(expected, 0.05), "{} vs {expected}", got.0);
    assert!(got.1.angle_between(pilot.1.slerp(gunner.1, f * f)) < 2e-3);
    for _ in 0..14 {
        assert!(walk.cam_handover.is_some_and(|h| h.age.is_finite()));
        walk.step_tick(&[PadState::default()]);
    }
    assert!(
        walk.cam_handover.is_some_and(|h| h.age.is_infinite()),
        "half a second, then the seat's own camera"
    );
    walk.step_tick(&[PadState::default()]);
    assert!(walk.cam_handover.is_none());
}

#[test]
fn baked_spa1_bomber_seat_switch_and_exit_draw_without_a_snap_or_a_stall() {
    use core::sim::{buttons, PadState};
    use core::vehicle::flyer::FlightPhase;
    let Some((mut walk, index)) = spa1_bomber_walk() else {
        return;
    };
    walk.blend.on = true;
    walk.sim.vehicles[index].flyer.phase = FlightPhase::Flying;
    walk.sim.vehicles[index].vel = Vec3::Z * 60.0;
    let hz = 144;
    let tick = (hz as f32 * core::TICK_DT).ceil() as usize;
    let mut last: Option<Vec3> = None;
    let mut steps = Vec::new();
    let mut stalls = 0;
    let mut exit_lag = false;
    let mut blended = false;
    for frame in 0..hz * 4 {
        let mut pad = PadState {
            lx: if frame > hz { i16::MAX } else { 0 },
            ..Default::default()
        };
        if frame == hz * 2 {
            pad.buttons |= buttons::SWITCH_WEAPON;
        }
        if frame == hz * 3 {
            walk.sim.vehicles[index].vel = Vec3::ZERO;
            walk.sim.vehicles[index].flyer.phase = FlightPhase::Landed;
        }
        if (hz * 3 + 30..hz * 3 + 34).contains(&frame) {
            pad.buttons |= buttons::ACTION;
        }
        walk.advance(1.0 / hz as f32, &[pad]);
        game::render_blend::blend_in_looking(&mut walk, &PadState::default());
        let cam = walk.camera();
        exit_lag |= walk.cam_exit.is_some();
        blended |= walk
            .cam_handover
            .is_some_and(|h| h.age > 0.0 && h.age.is_finite());
        if let Some(l) = last {
            let step = (cam.position - l).length();
            if (hz + 10..hz * 3).contains(&frame) {
                steps.push(step);
                stalls += usize::from(step < 1e-3);
            } else if frame > hz * 3 + 30 {
                steps.push(step);
            }
        }
        last = Some(cam.position);
        walk.blend_out();
    }
    assert!(
        walk.sim.unit(walk.player).unwrap().mount.is_none(),
        "got out"
    );
    assert!(exit_lag, "the soldier's camera eases in from the seat's");
    assert!(blended, "the player camera blends the handover");
    let snaps = steps
        .windows(3)
        .filter(|w| w[1] > 0.2 && w[1] > 1.5 * w[0].max(w[2]))
        .count();
    assert_eq!(snaps, 0, "frames that jump against both neighbours");
    assert!(
        stalls <= tick,
        "{stalls} still frames: only the handover's first, held frame"
    );
}

#[test]
fn stock_hangar_shields_keep_bolts_and_missile_damage_on_their_side() {
    use core::collision::Layers;
    use core::projectile::Shooter;
    use core::sim::{PadState, SpaceAssault};

    let root = assets_dir();
    let maps = [
        "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
    ];
    if !maps.iter().all(|map| root.join(map).is_dir()) {
        assert!(
            std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
            "missing stock space maps"
        );
        return;
    }
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["all"]).unwrap();
    let ship_missile = game::roster::weapon_from_db(&db, "all_weap_fly_xwing_ptorpedo").unwrap();
    let ordnance = &ship_missile.ordnance;
    let guidance = ordnance.missile.unwrap();
    let explosion = ordnance.explosion.as_ref().unwrap();
    let mut control = Sim::empty(1);
    control.install_space_assault(SpaceAssault::new(180));
    let witness = control.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
    let unit = control.unit_mut(witness).unwrap();
    unit.soldier.pos = Vec3::ZERO;
    let health = unit.health.cur;
    control.explode(explosion, Vec3::new(4.2, 1.0, 0.0), None, Some(0));
    assert!(
        control.unit(witness).is_none_or(|u| u.health.cur < health),
        "the same distance takes blast damage with no barrier"
    );
    let mut shield_only = 0;
    let mut directions = 0;
    let mut missiles = 0;
    for map in maps {
        for mode in ["ass", "1flag"] {
            let (scene, _) = game::assets::load_battle_scene(&root.join(map), mode).unwrap();
            let mut sim = Sim::empty(1);
            sim.install_space_assault(SpaceAssault::new(180));
            scene.install_collision(&mut sim);
            let weapon = sim.add_weapon_class(ship_missile.clone());
            let attacker = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
            let victim = sim.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
            let shooter = Shooter {
                owner: attacker,
                team: 0,
                weapon_class: weapon,
                is_player: false,
            };
            let mut shields = 0;
            for (index, instance) in scene.manifest.instances.iter().enumerate() {
                let Some(class) = scene.resolve_class(&instance.class) else {
                    continue;
                };
                if !class
                    .collision
                    .iter()
                    .any(|(k, v)| k == "vehicle" && v == "clear")
                    || !class
                        .collision
                        .iter()
                        .any(|(k, _)| k == "flyer_uses_vehicle")
                {
                    continue;
                }
                let geom = class.geometry.as_deref().unwrap().to_ascii_lowercase();
                let mesh = scene.collision[&geom].mesh.as_ref().unwrap();
                let xf = game::viewer::instance_matrix(instance.rotation, instance.translation);
                let mut largest = None;
                for tri in mesh.indices.chunks_exact(3) {
                    let [a, b, c] = [tri[0], tri[1], tri[2]]
                        .map(|i| xf.transform_point3(Vec3::from(mesh.positions[i as usize])));
                    let normal = (b - a).cross(c - a);
                    let area = normal.length_squared();
                    let Some(dir) = normal.try_normalize() else {
                        continue;
                    };
                    let at = (a + b + c) / 3.0;
                    if largest.is_none_or(|(size, _, _)| area > size) {
                        largest = Some((area, at, dir));
                    }
                    for dir in [dir, -dir] {
                        let start = at - dir;
                        let bolt = Bolt::fire(
                            start,
                            dir,
                            100.0,
                            0.02,
                            shooter,
                            None,
                            None,
                            &sim.collision,
                            HullFilter::All,
                        );
                        assert_ne!(
                            bolt.hit,
                            Hit::None,
                            "{map}/{mode} {} at {at}",
                            instance.name
                        );
                        assert!(
                            sim.collision
                                .ray_with(start, dir, 2.0, Layers::FLYER, HullFilter::Static)
                                .is_none_or(|r| r.instance != Some(index as u32)),
                            "{map}/{mode} {} blocks a flyer",
                            instance.name
                        );
                        if sim
                            .collision
                            .ray_with(start, dir, 2.0, Layers::ORDNANCE, HullFilter::Static)
                            .is_some_and(|hit| hit.instance == Some(index as u32))
                            && sim
                                .collision
                                .ray_past(
                                    start,
                                    dir,
                                    2.0,
                                    Layers::ORDNANCE,
                                    HullFilter::Static,
                                    [Some(index as u32), None],
                                )
                                .is_none()
                        {
                            shield_only += 1;
                        }
                        directions += 1;
                    }
                }

                // Fire a stock ship missile into the boundary in each direction.
                // Its nearby witness is on the other side and inside blast range.
                let (_, at, dir) = largest.expect("nondegenerate collision mesh");
                for dir in [dir, -dir] {
                    let from = at - dir * 2.0;
                    let u = sim.unit_mut(victim).unwrap();
                    u.soldier.pos = at + dir * 2.0 - Vec3::Y;
                    u.soldier.vel = Vec3::ZERO;
                    let health = u.health.cur;
                    let mut missile = Bolt::fire(
                        from,
                        dir,
                        ordnance.velocity,
                        ordnance.life_span,
                        shooter,
                        None,
                        None,
                        &sim.collision,
                        HullFilter::All,
                    );
                    missile.as_missile(ordnance, &guidance, None, &mut sim.lcg);
                    sim.bolts.push(missile);
                    let mut struck = false;
                    for _ in 0..10 {
                        sim.step(&[PadState::default()]);
                        struck |= sim
                            .events
                            .iter()
                            .any(|e| matches!(e, Event::BoltExpired { struck: true, .. }));
                        if struck {
                            break;
                        }
                    }
                    assert!(
                        struck,
                        "{map}/{mode} {} must stop the missile",
                        instance.name
                    );
                    assert_eq!(
                        sim.unit(victim).unwrap().health.cur,
                        health,
                        "{map}/{mode} {} must keep the blast on its side",
                        instance.name
                    );
                    missiles += 1;
                }
                shields += 1;
            }
            assert_eq!(shields, 2, "{map}/{mode} should have both hangar shields");
        }
    }
    assert!(
        shield_only > 0,
        "the exposed opening needs the shield collision"
    );
    eprintln!("hangar shields: {shield_only} stop on the shield alone, {directions} directional bolt casts and {missiles} missile impacts");
}
