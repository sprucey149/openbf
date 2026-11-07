//! Regression tests for Space Battles live playtest fixes:
//! 1. Capital ship hull & shield-shell registration (`Surface::ShieldLarge`, `com_sfx_shieldimpact_xl`,
//!    `shield_impact_large` 3D sound, shield shell layer toggle, indestructible bare hulls).
//! 2. Flyer scrape/crash (`com_veh_collision_lg`), hurt, and death (`imp_weap_ord_exp_lg`) ODF sounds
//!    plus cockpit/chase/death-camera audio listener origin.
//! 3. Space `SetParticleLODBias` (`3000.0` / `15000.0`) far-fade extension for small `bounding_radius`
//!    missile flares (`com_sfx_weap_rockettrail`), smooth `LiveEffect::advance` burst path interpolation
//!    at `350 m/s`, and `render_blend` pose capture for trail-only non-laser ordnance.

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
    if let Ok(dir) = std::env::var("OPENBF_ASSETS_BAKED") {
        return PathBuf::from(dir);
    }
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
    if fallback.exists() {
        fallback
    } else {
        game::assets::baked_root()
    }
}

fn missions_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OPENBF_MISSIONS") {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions")
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
