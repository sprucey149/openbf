use core::sim::{Event, Sim};
use core::unit::UnitSpec;
use core::weapon::DamageDesc;
use game::mission::MissionDef;
use glam::Vec3;
use std::path::PathBuf;

const MAPS: [(&str, &str, &str); 6] = [
    ("bf2_spa1", "imp", "all"),
    ("bf2_spa3", "rep", "cis"),
    ("bf2_spa6", "rep", "cis"),
    ("bf2_spa7", "rep", "cis"),
    ("bf2_spa8", "imp", "all"),
    ("bf2_spa9", "imp", "all"),
];

fn missions_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
}

fn mission(map: &str) -> Option<MissionDef> {
    let path = missions_dir().join(format!("{map}_ass.ron"));
    if !path.exists() {
        eprintln!(
            "SKIP: no {} (scripts/gen_missions.py --space --ps2-only)",
            path.display()
        );
        return None;
    }
    Some(MissionDef::load(&path).unwrap())
}

struct Battle {
    sim: Sim,
    mission: MissionDef,
    manifest: manifest::Manifest,
}

fn battle(map: &str, limit: u32) -> Option<Battle> {
    let mission = mission(map)?;
    let root = game::assets::baked_root();
    let dir = root.join(map);
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: no baked {}", dir.display());
        return None;
    }
    let mut scene = game::assets::load_scene(&dir).unwrap();
    scene.apply_mode_layer("ass");
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = layout.apply_mission(&mission);
    let factions = [0, 1].map(|i| mission.teams[i].faction.clone());
    let mut w = game::walk::WalkSim::new(1, Vec3::ZERO, scene.heightfield());
    for (team, faction) in w.teams.iter_mut().zip(&factions) {
        team.faction = faction.clone();
    }
    w.add_posts(&layout);
    w.sim.vacate_player_slot(0);
    let mut sim = w.sim;
    sim.landing_regions = applied.landing_regions.clone();
    sim.flight_limits = applied.flight_limits;
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    sim.battle = core::battle::Battle::new(mission.tickets());
    scene.install_collision(&mut sim);
    let def = mission.space_assault.clone().unwrap();
    game::vehicle_pads::install_with_objective(
        &mut sim,
        &scene.manifest,
        &dir,
        &root.join("classes"),
        &factions,
        Some((&def, limit)),
    )
    .unwrap();
    Some(Battle {
        sim,
        mission,
        manifest: scene.manifest,
    })
}

impl Battle {
    fn station(&self, name: &str) -> u16 {
        let instance = self
            .manifest
            .instances
            .iter()
            .position(|i| i.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("{name} is no instance")) as u32;
        self.sim
            .stations
            .iter()
            .position(|s| s.instance == Some(instance))
            .unwrap_or_else(|| panic!("{name} is no station")) as u16
    }

    fn turret(&self, name: &str) -> Option<core::vehicle::VehicleHandle> {
        let instance = self
            .manifest
            .instances
            .iter()
            .position(|i| i.name.eq_ignore_ascii_case(name))? as u32;
        self.sim
            .building_turrets
            .iter()
            .find(|t| t.instance == instance && t.down.is_none())
            .map(|t| t.handle)
    }

    fn soldier(&mut self, team: u8) -> core::unit::UnitHandle {
        self.sim
            .spawn(&UnitSpec::trooper(team, Vec3::new(0.0, 5000.0, 0.0)))
            .unwrap()
    }

    fn destroy(&mut self, name: &str, killer: Option<core::unit::UnitHandle>) {
        let s = self.station(name);
        let health = self.sim.stations[usize::from(s)].health;
        self.sim.stations[usize::from(s)].health.shield = 0.0;
        let desc = DamageDesc {
            damage: health.cur * 2.0 + 1.0,
            scales: [1.0; 7],
            shield_scale: 0.0,
        };
        self.sim.damage_station(s, &desc, 1.0, killer, None);
        assert!(self.sim.stations[usize::from(s)].destroyed, "{name}");
    }

    fn points(&self) -> [i32; 2] {
        self.sim.space_team_points().unwrap().0
    }
}

fn flyer_pad(b: &mut Battle, team: u8) -> core::vehicle::VehicleHandle {
    for _ in 0..(5 * core::TICK_HZ) {
        b.sim.step(&[]);
    }
    b.sim
        .alive_vehicles()
        .find(|(h, v)| {
            v.home_team == Some(team)
                && b.sim
                    .flyer_params
                    .get(v.class as usize)
                    .is_some_and(Option::is_some)
                && v.seats.first().is_some_and(|s| s.occupant.is_none())
                && b.sim.vehicle(*h).is_some()
        })
        .map(|(h, _)| h)
        .expect("a parked flyer")
}

fn pilot_at(b: &mut Battle, team: u8, pos: Vec3, target: Vec3) -> core::unit::UnitHandle {
    let v = flyer_pad(b, team);
    let p = b.sim.spawn(&UnitSpec::player(team, pos)).unwrap();
    assert!(b.sim.seat_unit(p, v, 0));
    let dir = (target - pos).normalize();
    let vehicle = b.sim.vehicle_mut(v).unwrap();
    vehicle.pos = pos;
    vehicle.vel = Vec3::ZERO;
    vehicle.rot = glam::Quat::from_rotation_arc(Vec3::Z, dir);
    p
}

#[test]
fn a_pilots_reticle_on_the_enemy_hull_picks_it_in_red_without_a_magnet() {
    let Some(mut b) = battle("bf2_spa8", 180) else {
        return;
    };
    let hull = b.station("all_cap1_body");
    let centre = b.sim.stations[usize::from(hull)].sphere_centre;
    let pos = centre + Vec3::new(0.0, 900.0, 2200.0);
    pilot_at(&mut b, 0, pos, centre);
    b.sim.step(&[]);
    let (pick, relation) = b.sim.reticle_pick[0].expect("the hull under the reticle");
    assert_eq!(relation, -1);
    assert_eq!(pick, core::sim::AimTarget::Station(hull));
    assert_eq!(b.sim.magnet[0], None);
}

#[test]
fn a_pilots_reticle_on_an_allied_hull_picks_it_as_a_friend() {
    let Some(mut b) = battle("bf2_spa8", 180) else {
        return;
    };
    let hull = b.station("imp_cap1_body1");
    let centre = b.sim.stations[usize::from(hull)].sphere_centre;
    let pos = centre + Vec3::new(0.0, 900.0, 2200.0);
    pilot_at(&mut b, 0, pos, centre);
    b.sim.step(&[]);
    let (pick, relation) = b.sim.reticle_pick[0].expect("the hull under the reticle");
    assert_eq!(relation, 1);
    assert_eq!(pick, core::sim::AimTarget::Station(hull));
    assert_eq!(b.sim.magnet[0], None);
}

#[test]
fn a_pilot_gets_markers_for_lockable_ships_and_none_for_hulls_in_view() {
    let Some(mut b) = battle("bf2_spa8", 180) else {
        return;
    };
    let hull = b.station("all_cap1_body");
    let frigate = b.station("all_frig1");
    let target = b.sim.stations[usize::from(frigate)].sphere_centre;
    let pos = target + Vec3::new(0.0, 300.0, 1500.0);
    let _ = pilot_at(&mut b, 0, pos, target);
    let mut hints = core::sim::TargetHints::default();
    for _ in 0..30 {
        b.sim.tick += 1;
        b.sim.update_target_hints(&mut hints, 0);
    }
    let listed: Vec<_> = hints.records().iter().map(|h| h.target).collect();
    assert!(listed.contains(&core::sim::LockTarget::Station(frigate)));
    assert!(!listed.contains(&core::sim::LockTarget::Station(hull)));
    let frig = hints
        .records()
        .iter()
        .find(|h| h.target == core::sim::LockTarget::Station(frigate))
        .unwrap();
    assert!(!frig.friendly);
}

#[test]
fn every_space_mission_loads_with_infinite_reinforcements_and_proper_sides() {
    for (map, a, b) in MAPS {
        let Some(m) = mission(map) else {
            return;
        };
        assert_eq!(m.mode, "ass", "{map}");
        assert_eq!(m.factions(), [a.to_owned(), b.to_owned()], "{map}");
        assert_eq!(m.tickets(), [999_999, 999_999], "{map}");
        assert!(!m.landing_regions.is_empty(), "{map}");
        assert!(m.killed.is_empty(), "{map}: {:?}", m.killed);
        assert!(m.cps.is_empty(), "{map}: {:?}", m.cps);
        assert!(m.space_assault.is_some(), "{map}");
    }
}

#[test]
fn every_ship_declares_eighteen_engine_points_and_eighteen_per_system() {
    for (map, _, _) in MAPS {
        let Some(m) = mission(map) else {
            return;
        };
        let sa = m.space_assault.unwrap();
        let engines: Vec<u32> = sa
            .critical
            .iter()
            .filter(|(_, kind, _)| kind == "engines")
            .map(|(_, _, p)| *p)
            .collect();
        assert_eq!(engines.iter().sum::<u32>(), 36, "{map}: {engines:?}");
        assert!(sa
            .critical
            .iter()
            .filter(|(_, kind, _)| kind != "engines")
            .all(|(_, _, p)| *p == 18));
    }
}

#[test]
fn yavin_declares_half_a_million_shields_and_the_others_the_default() {
    for (map, _, _) in MAPS {
        let Some(m) = mission(map) else {
            return;
        };
        let want = if map == "bf2_spa1" {
            500_000.0
        } else {
            200_000.0
        };
        let sa = m.space_assault.unwrap();
        assert_eq!(sa.shields.len(), 2, "{map}");
        assert!(sa.shields.iter().all(|s| s.max == want && s.add == 1000.0));
    }
}

#[test]
fn every_declared_system_is_a_damageable_object_of_its_map() {
    for (map, _, _) in MAPS {
        let Some(b) = battle(map, 180) else {
            return;
        };
        let def = b.mission.space_assault.clone().unwrap();
        for (name, _, _) in &def.critical {
            b.station(name);
        }
        for s in &def.shields {
            b.station(&s.controller);
        }
        let sa = b.sim.space_assault.as_ref().unwrap();
        assert_eq!(sa.critical.len(), def.critical.len(), "{map}");
        assert_eq!(sa.shields.len(), 2, "{map}");
        assert!(sa.shields.iter().all(|g| g.controller.is_some()), "{map}");
        assert!(sa.turrets.iter().all(|t| t.mainframe.is_some()), "{map}");
        assert!(b.sim.battle.unlimited_reinforcements, "{map}");
    }
}

#[test]
fn hangars_hold_spawn_only_posts_and_flyers_on_their_pads() {
    for (map, _, _) in MAPS {
        let Some(mut b) = battle(map, 180) else {
            return;
        };
        assert!(b.sim.cps.len() >= 2, "{map}: {}", b.sim.cps.len());
        let pilot = b.soldier(0);
        let at = b.sim.cps[0].def.capture.center;
        let start = b.sim.cps[0].owner;
        b.sim.unit_mut(pilot).unwrap().soldier.pos = at;
        for _ in 0..300 {
            b.sim.step(&[]);
        }
        assert_eq!(b.sim.cps[0].owner, start, "{map}");
        for team in [0u8, 1] {
            assert!(
                b.sim
                    .alive_vehicles()
                    .any(|(_, v)| v.home_team == Some(team)),
                "{map}: team {team} has no flyers"
            );
        }
    }
}

#[test]
fn a_dogfight_kill_grants_three_and_one_for_the_pilot() {
    let Some(mut b) = battle("bf2_spa1", 180) else {
        return;
    };
    for _ in 0..600 {
        b.sim.step(&[]);
    }
    let flyer = b
        .sim
        .alive_vehicles()
        .find(|(_, v)| {
            v.home_team == Some(1)
                && b.sim
                    .flyer_params
                    .get(v.class as usize)
                    .is_some_and(Option::is_some)
        })
        .map(|(h, _)| h)
        .expect("a Rebel flyer");
    let pilot = b.soldier(1);
    assert!(b.sim.seat_unit(pilot, flyer, 0));
    let killer = b.soldier(0);
    let before = b.points();
    b.sim.destroy_vehicle(flyer, Some(killer));
    assert_eq!(b.points(), [before[0] + 4, before[1]]);
}

#[test]
fn destroying_the_internal_generator_drops_the_external_shields() {
    let Some(mut b) = battle("bf2_spa1", 180) else {
        return;
    };
    let hull = b.station("bridge_all");
    assert_eq!(b.sim.stations[usize::from(hull)].health.shield, 500_000.0);
    let killer = b.soldier(0);
    b.destroy("shieldgenALL", Some(killer));
    for name in ["bridge_all", "comms_all", "Engine_a01", "rebelcruiser"] {
        let s = b.station(name);
        assert_eq!(b.sim.stations[usize::from(s)].health.shield, 0.0, "{name}");
    }
    let imp = b.station("bridge_imp");
    assert_eq!(b.sim.stations[usize::from(imp)].health.shield, 500_000.0);
    assert_eq!(b.points(), [0, 0], "the generator itself scores nothing");
    assert_eq!(b.sim.capital_meters().unwrap()[1].shield, 0.0);
    assert_eq!(b.sim.capital_meters().unwrap()[0].shield, 1.0);
}

#[test]
fn destroying_a_critical_system_awards_eighteen() {
    let Some(mut b) = battle("bf2_spa1", 180) else {
        return;
    };
    let killer = b.soldier(0);
    b.destroy("bridge_all", Some(killer));
    assert_eq!(b.points(), [18, 0]);
    b.destroy("Engine_a01", Some(killer));
    assert_eq!(b.points(), [21, 0], "an engine is one sixth of 18");
}

#[test]
fn linked_systems_die_together_for_one_score() {
    let Some(mut b) = battle("bf2_spa1", 180) else {
        return;
    };
    let killer = b.soldier(0);
    b.destroy("life_int_all", Some(killer));
    assert!(b.sim.stations[usize::from(b.station("life_ext_all"))].destroyed);
    assert_eq!(b.points(), [18, 0]);
    for name in [
        "Engine_a01",
        "Engine_a02",
        "Engine_a03",
        "Engine_a04",
        "Engine_a05",
        "Engine_a06",
    ] {
        b.destroy(name, Some(killer));
    }
    assert_eq!(b.points(), [36, 0]);
    assert!(b.sim.stations[usize::from(b.station("engines_all"))].destroyed);
}

#[test]
fn the_auto_defense_mainframe_neutralises_the_exterior_turrets() {
    let Some(mut b) = battle("bf2_spa1", 180) else {
        return;
    };
    let turrets: Vec<_> = ["mpat1", "mpat3", "mpat5"]
        .iter()
        .filter_map(|n| b.turret(n))
        .collect();
    assert!(!turrets.is_empty());
    let killer = b.soldier(0);
    b.destroy("autodefense_all", Some(killer));
    b.sim.step(&[]);
    for h in &turrets {
        assert_eq!(b.sim.vehicle(*h).unwrap().team, None);
    }
    let enemy = b.turret("mpit1").unwrap();
    assert_eq!(b.sim.vehicle(enemy).unwrap().team, Some(0));
}

#[test]
fn a_frigates_death_takes_its_turrets_with_it() {
    let Some(mut b) = battle("bf2_spa1", 180) else {
        return;
    };
    let killer = b.soldier(0);
    let instance = b
        .manifest
        .instances
        .iter()
        .position(|i| i.name.eq_ignore_ascii_case("mpat10"))
        .unwrap() as u32;
    b.destroy("cor01", Some(killer));
    assert_eq!(b.points(), [18, 0]);
    assert!(b.sim.building_hidden(instance));
}

#[test]
fn reaching_the_limit_ends_the_round_with_victory() {
    let Some(mut b) = battle("bf2_spa1", 36) else {
        return;
    };
    let killer = b.soldier(1);
    b.destroy("bridge_imp", Some(killer));
    assert_eq!(b.sim.outcome, None);
    b.sim.events.clear();
    b.destroy("comms_imp", Some(killer));
    assert_eq!(b.sim.outcome, Some(1));
    assert!(b
        .sim
        .events
        .iter()
        .any(|e| matches!(e, Event::TeamDefeated { team: 0 })));
}

fn hub_past_the_offers(b: &Battle, team: u8) -> Vec3 {
    let (ship, _) = b
        .sim
        .alive_vehicles()
        .find(|(_, v)| {
            v.home_team == Some(team)
                && b.sim
                    .flyer_params
                    .get(v.class as usize)
                    .is_some_and(Option::is_some)
        })
        .expect("a parked flyer");
    let at = b.sim.vehicle(ship).unwrap().pos;
    let g = b.sim.nav.as_ref().expect("a route graph");
    g.hubs
        .iter()
        .map(|h| h.pos)
        .filter(|p| p.distance(at) > 60.0)
        .min_by(|a, c| a.distance(at).total_cmp(&c.distance(at)))
        .expect("a hub away from the ships")
}

#[test]
fn every_ship_on_spa8_carries_its_authored_damage_effects() {
    let Some(b) = battle("bf2_spa8", 1000) else {
        return;
    };
    let mut ships = 0;
    for (class, setup) in b.sim.flyer_params.iter().enumerate() {
        if setup.is_none() {
            continue;
        }
        let effects = &b.sim.vehicle_damage_effects[class];
        let placed = effects
            .iter()
            .filter(|e| e.local != glam::Mat4::IDENTITY)
            .count();
        println!(
            "class {class}: {} effects, {placed} on hardpoints",
            effects.len()
        );
        ships += 1;
        assert!(effects.len() <= 10, "the class holds ten slots");
    }
    assert!(ships > 0);
    assert!(b.sim.vehicle_damage_effects.iter().any(|e| !e.is_empty()));
}

#[test]
fn pilots_turn_on_armed_buildings_and_fire_their_secondaries() {
    let Some(mut b) = battle("bf2_spa8", 1000) else {
        return;
    };
    if std::env::var("OPENBF_MEASURE_POS").is_ok() {
        for s in &b.sim.stations {
            println!(
                "station at {:?} hp {} max {} team {:?} instance {:?} class {} sphere {:?} {}",
                s.pos,
                s.health.cur,
                s.health.max,
                s.team,
                s.instance,
                s.class,
                s.transform.transform_point3(s.sphere_centre),
                s.sphere_radius
            );
        }
        for t in &b.sim.building_turrets {
            if let Some(v) = b.sim.vehicle(t.handle) {
                let (c, r) = b.sim.vehicle_blast_sphere(v);
                println!(
                    "turret class {} team {:?} centre {c:?} radius {r}",
                    v.class, v.team
                );
            }
        }
        for (class, setup) in b.sim.seat_setup.iter().enumerate() {
            let Some(g) = setup
                .as_ref()
                .and_then(|s| s.guns.first())
                .and_then(Option::as_ref)
            else {
                continue;
            };
            if b.sim.flyer_params.get(class).is_some_and(Option::is_some) {
                for w in &g.weapons {
                    let wc = &b.sim.weapon_classes[w.class];
                    println!(
                        "flyer {class} weapon {} range {:?} kind mask {:b}",
                        wc.name, wc.ai_range, wc.ai_target_kind
                    );
                }
            }
        }
    }
    let crew = std::env::var("OPENBF_MEASURE_CREW")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let dir = game::assets::baked_root().join("bf2_spa8");
    b.sim.nav = game::layout::nav_graph(&b.manifest, &dir);
    for _ in 0..(5 * core::TICK_HZ) {
        b.sim.step(&[]);
    }
    for team in 0..2 {
        let at = hub_past_the_offers(&b, team);
        for _ in 0..crew {
            b.sim
                .spawn(&UnitSpec {
                    controller: core::unit::Controller::Ai,
                    ..UnitSpec::trooper(team, at)
                })
                .unwrap();
        }
    }
    if let Some(seed) = std::env::var("OPENBF_MEASURE_SEED")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
    {
        for _ in 0..seed {
            b.sim.lcg.uniform(0.0, 1.0);
        }
    }
    let warm: u32 = std::env::var("OPENBF_MEASURE_WARM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    for _ in 0..(warm * core::TICK_HZ) {
        b.sim.step(&[]);
    }
    let flying = |v: &core::vehicle::Vehicle| {
        v.kind == core::vehicle::VehicleKind::Flyer
            && v.flyer.phase == core::vehicle::flyer::FlightPhase::Flying
    };
    let (mut samples, mut second, mut armed, mut aimed) = (0u32, 0u32, 0u32, 0u32);
    let (mut first_shots, mut second_shots) = (0u32, 0u32);
    let (mut crashing, mut destroyed) = (0u32, 0u32);
    let (mut lost, mut by_pilot, mut by_turret, mut by_other) = (0.0f32, 0u32, 0u32, 0u32);
    if std::env::var("OPENBF_MEASURE_HUMAN").is_ok() {
        let hull = b.station("all_cap1_body");
        let centre = b.sim.stations[usize::from(hull)].sphere_centre;
        let near = b
            .sim
            .building_turrets
            .iter()
            .filter(|t| t.down.is_none() && !t.self_pilots.is_empty())
            .filter_map(|t| b.sim.vehicle(t.handle))
            .filter(|v| v.team != Some(0))
            .map(|v| v.pos)
            .min_by(|a, c| a.distance(centre).total_cmp(&c.distance(centre)))
            .expect("an enemy turret");
        let pos = near + (near - centre).normalize() * 300.0;
        let p = pilot_at(&mut b, 0, pos, centre);
        let ship = b.sim.unit(p).unwrap().mount.unwrap().vehicle;
        let start = b.sim.tick;
        let full = b.sim.vehicle(ship).map_or(0.0, |v| v.health.cur);
        let mut first = None;
        let mut on_me = 0u64;
        let mut samples = 0u64;
        while b.sim.tick - start < 120 * u64::from(core::TICK_HZ) {
            let Some(v) = b
                .sim
                .vehicle_mut(ship)
                .filter(|v| v.alive && v.health.alive())
            else {
                break;
            };
            v.pos = pos;
            v.vel = Vec3::ZERO;
            b.sim.step(&[]);
            if first.is_none() && b.sim.vehicle(ship).is_some_and(|v| v.health.cur < full) {
                first = Some(b.sim.tick - start);
            }
            samples += 1;
            on_me += b
                .sim
                .building_turrets
                .iter()
                .flat_map(|t| &t.self_pilots)
                .filter(|sp| sp.target == Some(core::autoturret::Aim::Vehicle(ship)))
                .count() as u64;
        }
        println!(
            "human hold: alive for {:.1} s, first damage {:?} s, turrets on me {:.2}",
            (b.sim.tick - start) as f32 / core::TICK_HZ as f32,
            first.map(|f| f as f32 / core::TICK_HZ as f32),
            on_me as f32 / samples.max(1) as f32
        );
        return;
    }
    let mut positions = std::env::var("OPENBF_MEASURE_POS")
        .ok()
        .map(|_| String::new());
    let mut hull = std::collections::HashMap::new();
    let mut was_crashing = std::collections::HashSet::new();
    let mut pilot_hit_rows: Vec<(f32, f32, f32, bool, u8)> = Vec::new();
    let mut near_hist = [0u32; 6];
    let (mut hits_pilot, mut hits_turret, mut hits_other) = (0u32, 0u32, 0u32);
    let (mut dmg_pilot, mut dmg_turret, mut dmg_other) = (0.0f32, 0.0f32, 0.0f32);
    let (mut turret_shots, mut held_flyer, mut held_turrets, mut turret_ticks) =
        (0u32, 0u32, 0u32, 0u32);
    let mut held_at: Vec<f32> = Vec::new();
    let (mut roll_starts, mut roll_ticks, mut roll_speed) = (0u32, 0u32, 0.0f32);
    let mut rolling_was = std::collections::HashSet::new();
    let mut roll_lengths: Vec<u32> = Vec::new();
    let mut roll_began = std::collections::HashMap::new();
    let mut armed_now: std::collections::HashMap<u32, (u64, f32)> =
        std::collections::HashMap::new();
    let (mut armed_lens, mut armed_starts): (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());
    let mut turret_times: std::collections::HashMap<u32, Vec<u64>> =
        std::collections::HashMap::new();
    let mut hold_now: std::collections::HashMap<(u32, u8), (core::autoturret::Aim, u64, u32)> =
        std::collections::HashMap::new();
    let mut turret_shot_counts: std::collections::HashMap<u32, u32> =
        std::collections::HashMap::new();
    let (mut err_bins, mut start_ang, mut reach, mut ang_now) = (
        [0u32; 13],
        Vec::new(),
        [Vec::new(), Vec::new(), Vec::new()],
        std::collections::HashMap::new(),
    );
    let (mut hold_lens, mut hold_shots, mut hold_empty, mut hold_count) =
        (Vec::new(), 0u32, 0u32, 0u32);
    let mut last_guns: std::collections::HashMap<(u32, u8), u32> = std::collections::HashMap::new();
    let mut first_shot: Vec<f32> = Vec::new();
    let seconds = std::env::var("OPENBF_MEASURE_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(90u32);
    for _ in 0..(seconds * core::TICK_HZ) {
        b.sim.step(&[]);
        for (h, v) in b.sim.alive_vehicles() {
            if v.kind != core::vehicle::VehicleKind::Flyer {
                continue;
            }
            let at = v
                .flyer
                .ai
                .armed_station
                .map(|i| b.sim.stations[usize::from(i)].sphere_centre)
                .or_else(|| {
                    v.flyer
                        .ai
                        .armed_target
                        .and_then(|t| b.sim.vehicle(t))
                        .map(|t| t.pos)
                });
            match (at, armed_now.contains_key(&h.index)) {
                (Some(p), false) => {
                    armed_now.insert(h.index, (b.sim.tick, p.distance(v.pos)));
                }
                (None, true) => {
                    if let Some((t, d)) = armed_now.remove(&h.index) {
                        armed_lens.push((b.sim.tick - t) as f32 / core::TICK_HZ as f32);
                        armed_starts.push(d);
                    }
                }
                _ => {}
            }
            if v.flyer.in_roll {
                roll_ticks += 1;
                roll_speed += v.vel.length();
                if rolling_was.insert(h) {
                    roll_starts += 1;
                    roll_began.insert(h, b.sim.tick);
                }
            } else if rolling_was.remove(&h) {
                if let Some(at) = roll_began.remove(&h) {
                    roll_lengths.push((b.sim.tick - at) as u32);
                }
            }
        }
        for e in &b.sim.events {
            if let Event::Shot { unit, .. } = e {
                if unit.index == u32::MAX {
                    *turret_shot_counts.entry(unit.gen).or_insert(0u32) += 1;
                }
            }
        }
        for t in &b.sim.building_turrets {
            if t.down.is_some() {
                continue;
            }
            let Some(bv) = b.sim.vehicle(t.handle) else {
                continue;
            };
            for p in &t.self_pilots {
                let key = (t.handle.index, p.seat);
                let flyer = match p.target {
                    Some(core::autoturret::Aim::Vehicle(a)) => b
                        .sim
                        .vehicle(a)
                        .filter(|v| v.kind == core::vehicle::VehicleKind::Flyer)
                        .map(|_| core::autoturret::Aim::Vehicle(a)),
                    _ => None,
                };
                if let (Some(core::autoturret::Aim::Vehicle(a)), Some(st)) = (
                    flyer,
                    bv.guns.get(usize::from(p.seat)).and_then(Option::as_ref),
                ) {
                    if let Some(tv) = b.sim.vehicle(a) {
                        let dir = bv.rot * core::vehicle::gun::dir(st.look.0, st.look.1);
                        let ang = dir
                            .dot((tv.pos - bv.pos).normalize_or_zero())
                            .clamp(-1.0, 1.0)
                            .acos()
                            .to_degrees();
                        err_bins[((ang / 5.0) as usize).min(12)] += 1;
                        let tick = b.sim.tick;
                        let e = ang_now.entry(key).or_insert((a, tick, [false; 3]));
                        if e.0 != a {
                            *e = (a, tick, [false; 3]);
                        }
                        if e.1 == tick {
                            start_ang.push(ang);
                        }
                        for (i, k) in [5.0f32, 10.0, 20.0].into_iter().enumerate() {
                            if ang < k && !e.2[i] {
                                e.2[i] = true;
                                reach[i].push((tick - e.1) as f32 / core::TICK_HZ as f32);
                            }
                        }
                    }
                } else {
                    ang_now.remove(&key);
                }
                let shots_now = turret_shot_counts
                    .get(&t.handle.index)
                    .copied()
                    .unwrap_or(0);
                let before = last_guns.insert(key, shots_now).unwrap_or(shots_now);
                let mut ended = false;
                if let Some(h) = hold_now.get_mut(&key) {
                    if Some(h.0) == flyer {
                        if shots_now > before {
                            if h.2 == 0 {
                                first_shot.push((b.sim.tick - h.1) as f32 / core::TICK_HZ as f32);
                            }
                            h.2 += shots_now - before;
                        }
                    } else {
                        ended = true;
                    }
                }
                if ended {
                    if let Some(h) = hold_now.remove(&key) {
                        hold_lens.push((b.sim.tick - h.1) as f32 / core::TICK_HZ as f32);
                        hold_shots += h.2;
                        hold_count += 1;
                        if h.2 == 0 {
                            hold_empty += 1;
                        }
                    }
                }
                if let (Some(a), false) = (flyer, hold_now.contains_key(&key)) {
                    hold_now.insert(key, (a, b.sim.tick, 0));
                }
                turret_ticks += 1;
                if p.target.is_some() {
                    held_turrets += 1;
                }
                if let Some(core::autoturret::Aim::Vehicle(a)) = p.target {
                    if let Some(tv) = b.sim.vehicle(a) {
                        if tv.kind == core::vehicle::VehicleKind::Flyer {
                            held_flyer += 1;
                            if b.sim.tick % 15 == 0 {
                                held_at.push(tv.pos.distance(bv.pos));
                            }
                        }
                    }
                }
            }
        }
        for (h, v) in b.sim.alive_vehicles() {
            if let Some(rows) = positions.as_mut().filter(|_| b.sim.tick % 15 == 0) {
                if v.kind == core::vehicle::VehicleKind::Flyer {
                    rows.push_str(&format!(
                        "{},{:?},{},{},{},{}
",
                        v.team.map_or(-1, i32::from),
                        v.flyer.phase,
                        v.pos.x,
                        v.pos.y,
                        v.pos.z,
                        v.has_pilot()
                    ));
                }
            }
            if flying(v) && v.has_pilot() {
                samples += 1;
                second += u32::from(v.flyer.ai.channel == 1);
                armed += u32::from(
                    v.flyer.ai.armed_target.is_some() || v.flyer.ai.armed_station.is_some(),
                );
                let before = hull.insert(h, v.health.cur).unwrap_or(v.health.cur);
                if v.health.cur > 0.0 {
                    lost += (before - v.health.cur).max(0.0);
                }
                let pilot = v.seats[0].occupant.and_then(|p| b.sim.unit(p));
                aimed += u32::from(pilot.is_some_and(|u| {
                    u.ai.target
                        .and_then(|f| f.unit())
                        .and_then(|e| b.sim.unit(e))
                        .and_then(|e| e.mount)
                        .and_then(|m| b.sim.vehicle(m.vehicle))
                        .is_some_and(|e| e.kind == core::vehicle::VehicleKind::Flyer)
                }));
            }
            if v.flyer.phase == core::vehicle::flyer::FlightPhase::Falling && was_crashing.insert(h)
            {
                crashing += 1;
            }
        }
        if b.sim.tick % 6 == 0 {
            let fl: Vec<(u8, glam::Vec3)> = b
                .sim
                .alive_vehicles()
                .filter(|(_, v)| {
                    v.kind == core::vehicle::VehicleKind::Flyer
                        && v.flyer.phase == core::vehicle::flyer::FlightPhase::Flying
                })
                .filter_map(|(_, v)| v.team.map(|t| (t, v.pos)))
                .collect();
            for &(t, p) in &fl {
                let d = fl
                    .iter()
                    .filter(|o| o.0 != t)
                    .map(|o| o.1.distance(p))
                    .fold(f32::MAX, f32::min);
                let k = [100.0, 200.0, 400.0, 800.0, 1600.0]
                    .iter()
                    .position(|x| d < *x)
                    .unwrap_or(5);
                near_hist[k] += 1;
            }
        }
        for e in &b.sim.events {
            match e {
                Event::VehicleHit {
                    shooter,
                    vehicle,
                    damage,
                    ..
                } if b
                    .sim
                    .vehicle(*vehicle)
                    .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer) =>
                {
                    let by = shooter.and_then(|s| b.sim.unit(s)).and_then(|u| u.mount);
                    match (shooter, by) {
                        (_, Some(m))
                            if b.sim
                                .vehicle(m.vehicle)
                                .is_some_and(|k| k.kind == core::vehicle::VehicleKind::Flyer) =>
                        {
                            hits_pilot += 1;
                            dmg_pilot += *damage;
                            let (sv, tv) = (b.sim.vehicle(m.vehicle), b.sim.vehicle(*vehicle));
                            if let (Some(sv), Some(tv)) = (sv, tv) {
                                let to = tv.pos - sv.pos;
                                pilot_hit_rows.push((
                                    *damage,
                                    to.length(),
                                    to.normalize_or_zero().dot(sv.rot * glam::Vec3::Z),
                                    sv.flyer.ai.armed_target == Some(*vehicle),
                                    sv.flyer.ai.channel,
                                ));
                            }
                        }
                        (None, _) => {
                            hits_other += 1;
                            dmg_other += *damage;
                        }
                        _ => {
                            hits_turret += 1;
                            dmg_turret += *damage;
                        }
                    }
                }
                Event::Shot {
                    unit, weapon_class, ..
                } if b.sim.unit(*unit).is_none_or(|u| {
                    u.mount.is_none_or(|m| {
                        b.sim
                            .vehicle(m.vehicle)
                            .is_none_or(|v| v.kind != core::vehicle::VehicleKind::Flyer)
                    })
                }) =>
                {
                    let _ = weapon_class;
                    turret_shots += 1;
                    if let Event::Shot { origin, .. } = e {
                        let key = (origin.x / 8.0) as i32 as u32
                            ^ ((origin.z / 8.0) as i32 as u32).rotate_left(12)
                            ^ ((origin.y / 8.0) as i32 as u32).rotate_left(22);
                        turret_times.entry(key).or_default().push(b.sim.tick);
                    }
                    let _ = unit;
                }
                Event::Shot {
                    unit, weapon_class, ..
                } => {
                    let Some(u) = b.sim.unit(*unit) else {
                        continue;
                    };
                    let Some(m) = u.mount.filter(|m| m.seat == 0) else {
                        continue;
                    };
                    let Some(v) = b.sim.vehicle(m.vehicle).filter(|v| flying(v)) else {
                        continue;
                    };
                    let setup = b.sim.seat_setup[v.class as usize].as_ref().unwrap();
                    let gun = setup.guns[0].as_ref().unwrap();
                    if gun
                        .weapons
                        .first()
                        .is_some_and(|w| w.class as u32 == *weapon_class)
                    {
                        first_shots += 1;
                    } else {
                        second_shots += 1;
                    }
                }
                Event::VehicleDestroyed { killer, .. } => {
                    destroyed += 1;
                    match killer.and_then(|k| b.sim.unit(k)).and_then(|k| k.mount) {
                        Some(m)
                            if b.sim
                                .vehicle(m.vehicle)
                                .is_some_and(|k| k.kind == core::vehicle::VehicleKind::Flyer) =>
                        {
                            by_pilot += 1
                        }
                        Some(_) => by_turret += 1,
                        None => by_other += 1,
                    }
                }
                _ => {}
            }
        }
    }
    println!(
        "pilots: samples {samples} channel 1 {:.2} armed target {:.2} flyer target {:.2}; shots {first_shots}/{second_shots} per s {:.1}/{:.1}; crashes {crashing} destroyed {destroyed} (flyers {by_pilot}, seats {by_turret}, other {by_other}) in {seconds} s; hull lost per flyer per s {:.1}",
        f64::from(second) / f64::from(samples.max(1)),
        f64::from(armed) / f64::from(samples.max(1)),
        f64::from(aimed) / f64::from(samples.max(1)),
        f64::from(first_shots) / f64::from(seconds),
        f64::from(second_shots) / f64::from(seconds),
        f64::from(lost) / f64::from(samples.max(1)) * f64::from(core::TICK_HZ),
    );
    {
        let (mut within, mut pauses) = (Vec::new(), Vec::new());
        let mut lens = std::collections::BTreeMap::new();
        for t in turret_times.values() {
            let mut n = 1;
            for w in t.windows(2) {
                let g = (w[1] - w[0]) as f32 / core::TICK_HZ as f32;
                if g < 0.75 {
                    n += 1;
                    within.push(g);
                } else {
                    *lens.entry(n).or_insert(0) += 1;
                    n = 1;
                    pauses.push(g);
                }
            }
            *lens.entry(n).or_insert(0) += 1;
        }
        within.sort_by(f32::total_cmp);
        pauses.sort_by(f32::total_cmp);
        let q = |l: &Vec<f32>, x: f32| l.get((l.len() as f32 * x) as usize).copied().unwrap_or(0.0);
        println!(
            "bursts: shooters {} lens {lens:?}; within {:.2} {:.2} {:.2}; pause {:.2} {:.2} {:.2}",
            turret_times.len(),
            q(&within, 0.1),
            q(&within, 0.5),
            q(&within, 0.9),
            q(&pauses, 0.1),
            q(&pauses, 0.5),
            q(&pauses, 0.9),
        );
    }
    {
        start_ang.sort_by(f32::total_cmp);
        let qa =
            |l: &Vec<f32>, x: f32| l.get((l.len() as f32 * x) as usize).copied().unwrap_or(0.0);
        println!(
            "start angle {:.1} {:.1} {:.1} {:.1} n {}",
            qa(&start_ang, 0.25),
            qa(&start_ang, 0.5),
            qa(&start_ang, 0.75),
            qa(&start_ang, 0.9),
            start_ang.len()
        );
        for r in &mut reach {
            r.sort_by(f32::total_cmp);
            println!(
                "reach n {} p50 {:.2} p90 {:.2}",
                r.len(),
                qa(r, 0.5),
                qa(r, 0.9)
            );
        }
        let held_sum: u32 = err_bins.iter().sum();
        println!(
            "error share {:?}",
            err_bins
                .iter()
                .map(|x| (*x as f32 / held_sum.max(1) as f32 * 1000.0).round() / 1000.0)
                .collect::<Vec<_>>()
        );
        hold_lens.sort_by(f32::total_cmp);
        first_shot.sort_by(f32::total_cmp);
        let q = |l: &Vec<f32>, x: f32| l.get((l.len() as f32 * x) as usize).copied().unwrap_or(0.0);
        let total: f32 = hold_lens.iter().sum();
        println!(
            "holds {hold_count} total {total:.0} s; len {:.2} {:.2} {:.2}; shots {hold_shots} per held s {:.2}; no shot {:.2}; first shot {:.2} {:.2} {:.2}",
            q(&hold_lens, 0.1), q(&hold_lens, 0.5), q(&hold_lens, 0.9),
            hold_shots as f32 / total.max(0.01),
            hold_empty as f32 / hold_count.max(1) as f32,
            q(&first_shot, 0.1), q(&first_shot, 0.5), q(&first_shot, 0.9),
        );
    }
    {
        armed_lens.sort_by(f32::total_cmp);
        armed_starts.sort_by(f32::total_cmp);
        let q = |l: &Vec<f32>, x: f32| l.get((l.len() as f32 * x) as usize).copied().unwrap_or(0.0);
        println!(
            "pilot holds {} len {:.1} {:.1} {:.1}; start dist {:.0} {:.0} {:.0}",
            armed_lens.len(),
            q(&armed_lens, 0.1),
            q(&armed_lens, 0.5),
            q(&armed_lens, 0.9),
            q(&armed_starts, 0.1),
            q(&armed_starts, 0.5),
            q(&armed_starts, 0.9),
        );
    }
    roll_lengths.sort_unstable();
    println!(
        "rolls {roll_starts}; length median {:.2} s; speed in roll {:.1}; repair scale {:?}",
        f64::from(
            roll_lengths
                .get(roll_lengths.len() / 2)
                .copied()
                .unwrap_or(0)
        ) / f64::from(core::TICK_HZ),
        roll_speed / roll_ticks.max(1) as f32,
        b.sim
            .flyer_params
            .iter()
            .flatten()
            .map(|p| (
                p.pilot_repair_scale,
                p.trick_drain,
                p.trick_side_strafe,
                p.trick_roll_speed
            ))
            .next(),
    );
    held_at.sort_by(f32::total_cmp);
    {
        let mut seen = std::collections::BTreeMap::new();
        for (_, v) in b.sim.alive_vehicles() {
            if v.kind == core::vehicle::VehicleKind::Flyer {
                let setup = b
                    .sim
                    .seat_setup
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .map(|s| s.radius);
                let top = b
                    .sim
                    .flyer_params
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .map(|p| p.max_speed);
                seen.insert(v.class, (setup, b.sim.vehicle_blast_sphere(v).1, top));
            }
        }
        println!("flyer radii class setup, blast, top: {seen:?}");
        let total: u32 = near_hist.iter().sum();
        println!(
            "nearest hostile flyer bins <100,<200,<400,<800,<1600,rest {:?}",
            near_hist
                .iter()
                .map(|x| (*x as f32 / total.max(1) as f32 * 1000.0).round() / 1000.0)
                .collect::<Vec<_>>()
        );
        println!(
            "pilot hit rows dmg,dist,cos,targeted,channel: {:?}",
            pilot_hit_rows
                .iter()
                .map(|r| (r.0 as i32, r.1 as i32, (r.2 * 100.0) as i32, r.3, r.4))
                .collect::<Vec<_>>()
        );
    }
    println!(
        "hits on flyers: pilot {hits_pilot} ({dmg_pilot:.0}), turret or foot {hits_turret} ({dmg_turret:.0}), none {hits_other} ({dmg_other:.0}); turret shots per s {:.1}; turrets seats {:.1}, holding {:.2}, holding a flyer {:.2}; hold distance median {:.0} p90 {:.0}",
        f64::from(turret_shots) / f64::from(seconds),
        f64::from(turret_ticks) / f64::from(seconds * core::TICK_HZ),
        f64::from(held_turrets) / f64::from(seconds * core::TICK_HZ),
        f64::from(held_flyer) / f64::from(seconds * core::TICK_HZ),
        held_at.get(held_at.len() / 2).copied().unwrap_or(0.0),
        held_at.get(held_at.len() * 9 / 10).copied().unwrap_or(0.0),
    );
    if let (Some(rows), Ok(path)) = (positions, std::env::var("OPENBF_MEASURE_POS")) {
        std::fs::write(path, rows).unwrap();
    }
    assert!(
        armed > 0,
        "no pilot took an armed building in {samples} samples"
    );
    assert!(second > 0, "no pilot flipped to the secondary channel");
    assert!(second_shots > 0, "no pilot fired a secondary");
}

#[test]
fn soldiers_who_start_out_of_reach_crew_the_ships_and_the_ships_shoot() {
    let Some(mut b) = battle("bf2_spa8", 1000) else {
        return;
    };
    let dir = game::assets::baked_root().join("bf2_spa8");
    b.sim.nav = game::layout::nav_graph(&b.manifest, &dir);
    for _ in 0..(5 * core::TICK_HZ) {
        b.sim.step(&[]);
    }
    for team in 0..2 {
        let at = hub_past_the_offers(&b, team);
        for _ in 0..6 {
            b.sim
                .spawn(&UnitSpec {
                    controller: core::unit::Controller::Ai,
                    ..UnitSpec::trooper(team, at)
                })
                .unwrap();
        }
    }
    let mut pilot_shots = 0;
    let mut untargeted = 0;
    for _ in 0..(90 * core::TICK_HZ) {
        b.sim.step(&[]);
        for e in &b.sim.events {
            let Event::Shot { unit, .. } = e else {
                continue;
            };
            let Some(u) = b.sim.unit(*unit) else {
                continue;
            };
            let flying = u
                .mount
                .filter(|m| m.seat == 0)
                .and_then(|m| b.sim.vehicle(m.vehicle))
                .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer);
            if flying {
                pilot_shots += 1;
                if u.ai.target.is_none() {
                    untargeted += 1;
                }
            }
        }
    }
    let crews = b
        .sim
        .alive_vehicles()
        .filter(|(_, v)| {
            v.kind == core::vehicle::VehicleKind::Flyer
                && v.seats.first().is_some_and(|s| s.occupant.is_some())
        })
        .count();
    assert!(crews >= 4, "only {crews} of 12 soldiers flying");
    assert!(pilot_shots > 100, "{pilot_shots} shots");
    assert!(untargeted > 0, "{pilot_shots} shots, none free");
}

#[test]
fn a_human_ship_in_a_turrets_view_draws_its_fire() {
    let Some(mut b) = battle("bf2_spa8", 1000) else {
        return;
    };
    for _ in 0..(5 * core::TICK_HZ) {
        b.sim.step(&[]);
    }
    let turrets: Vec<core::vehicle::VehicleHandle> = b
        .sim
        .building_turrets
        .iter()
        .filter(|t| t.down.is_none() && !t.self_pilots.is_empty())
        .map(|t| t.handle)
        .filter(|h| b.sim.vehicle(*h).is_some_and(|v| v.team != Some(0)))
        .collect();
    let p = pilot_at(&mut b, 0, Vec3::new(0.0, 5000.0, 0.0), Vec3::ZERO);
    let ship = b.sim.unit(p).unwrap().mount.unwrap().vehicle;
    let full = b.sim.vehicle(ship).unwrap().health.cur;
    let mut drawn = 0;
    let mut hit = false;
    for t in &turrets {
        let v = b.sim.vehicle(*t).unwrap();
        let Some(st) = v.guns.first().and_then(Option::as_ref) else {
            continue;
        };
        let pos = v.pos + v.rot * core::vehicle::gun::dir(st.look.0, st.look.1) * 300.0;
        let mut fired = 0;
        for _ in 0..(10 * core::TICK_HZ) {
            let Some(s) = b.sim.vehicle_mut(ship) else {
                break;
            };
            s.pos = pos;
            s.vel = Vec3::ZERO;
            b.sim.step(&[]);
            fired += b
                .sim
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { unit, .. } if unit.index == u32::MAX))
                .count();
        }
        drawn += usize::from(fired > 0);
        hit |= b.sim.vehicle(ship).is_none_or(|s| s.health.cur < full);
        if hit {
            break;
        }
    }
    assert!(drawn > 0, "{} turrets, none fired", turrets.len());
    assert!(hit, "{drawn} turrets fired, no hit");
}

#[test]
fn a_space_round_replays_to_the_same_hash() {
    let run = || {
        let mut b = battle("bf2_spa1", 180)?;
        for _ in 0..300 {
            b.sim.step(&[]);
        }
        Some(b.sim.hash())
    };
    let Some(a) = run() else {
        return;
    };
    assert_eq!(Some(a), run());
}

#[test]
fn every_door_opens_for_a_walker_and_lets_it_through() {
    for (map, _, _) in MAPS {
        let Some(mut b) = battle(map, 180) else {
            return;
        };
        let pad = core::sim::PadState {
            ly: i16::MAX,
            ..Default::default()
        };
        for k in 0..b.sim.doors.len() {
            let origin = b.sim.doors[k].origin;
            let instance = b.sim.doors[k].instance;
            let name = b.manifest.instances[instance].name.clone();
            let rotation = b.manifest.instances[instance].rotation;
            let across = Vec3::new(rotation[2][0], 0.0, rotation[2][2]).normalize();
            let mut crossings = 0;
            for side in [1.0f32, -1.0] {
                for d in &mut b.sim.doors {
                    d.open = 0.0;
                }
                b.sim.step(&[]);
                let above = origin + across * side * 6.0 + Vec3::Y * 3.0;
                let floor = b
                    .sim
                    .collision
                    .ray_with(
                        above,
                        Vec3::NEG_Y,
                        6.0,
                        core::collision::Layers::SOLDIER,
                        core::collision::HullFilter::Static,
                    )
                    .map_or(3.0, |h| h.t);
                let from = above - Vec3::Y * (floor - 0.1);
                let walk = -across * side;
                let mut spec = UnitSpec::player(0, from);
                spec.yaw = (-walk.x).atan2(-walk.z);
                let pilot = b.sim.spawn(&spec).unwrap();
                let mut peak = 0.0f32;
                let mut crossed = -6.0f32;
                for _ in 0..90 {
                    b.sim.step(std::slice::from_ref(&pad));
                    peak = peak.max(b.sim.doors[k].open);
                    let Some(u) = b.sim.unit(pilot) else {
                        break;
                    };
                    crossed = crossed.max((u.soldier.pos - origin).dot(walk));
                }
                if peak > 0.5 && crossed > 1.0 {
                    crossings += 1;
                }
                b.sim.vacate_player_slot(0);
            }
            assert!(crossings > 0, "{map} {name}: no side lets a walker through");
        }
    }
}

#[test]
fn ships_pass_every_hangar_field_and_soldiers_do_not() {
    use core::collision::{HullFilter, Layers};
    for (map, _, _) in MAPS {
        let Some(b) = battle(map, 180) else {
            return;
        };
        let fields: Vec<_> = b
            .manifest
            .instances
            .iter()
            .filter(|i| {
                let class = i.class.to_ascii_lowercase();
                class == "imp_cap_stardestroyer_shield" || class == "cis_fedcruiser_shield_blue"
            })
            .collect();
        assert_eq!(fields.len(), 2, "{map}");
        for field in fields {
            let across = Vec3::from(field.rotation[0]);
            let up = Vec3::from(field.rotation[1]);
            let (plane, height) = if field
                .class
                .eq_ignore_ascii_case("cis_fedcruiser_shield_blue")
            {
                (109.3, 15.0)
            } else {
                (0.1, 12.0)
            };
            let centre = Vec3::from(field.translation) + up * height + across * plane;
            let from = centre - across * 3.0;
            let ship =
                b.sim
                    .collision
                    .ray_with(from, across, 6.0, Layers::VEHICLE, HullFilter::Static);
            let soldier =
                b.sim
                    .collision
                    .ray_with(from, across, 6.0, Layers::SOLDIER, HullFilter::Static);
            let flyer =
                b.sim
                    .collision
                    .ray_with(from, across, 6.0, Layers::FLYER, HullFilter::Static);
            assert!(flyer.is_none(), "{map} {}", field.name);
            assert!(
                ship.is_none(),
                "{map} {}: {:?}",
                field.name,
                ship.map(|h| h.t)
            );
            let t = soldier
                .unwrap_or_else(|| panic!("{map} {} lets a soldier by", field.name))
                .t;
            assert!((t - 3.0).abs() < 0.5, "{map} {}: {t}", field.name);
        }
    }
}

#[test]
fn a_pad_holds_one_ship_until_it_flies() {
    for (map, _, _) in MAPS {
        let Some(mut b) = battle(map, 180) else {
            return;
        };
        for _ in 0..(40 * core::TICK_HZ) {
            b.sim.step(&[]);
        }
        let flyers = b
            .sim
            .vehicle_pads
            .iter()
            .filter(|p| p.def.spawn_count > 1)
            .collect::<Vec<_>>();
        assert!(!flyers.is_empty(), "{map}");
        for pad in flyers {
            assert!(
                pad.vehicles.len() <= 1,
                "{map} {}: {}",
                pad.def.name,
                pad.vehicles.len()
            );
            assert!(
                pad.def
                    .classes
                    .iter()
                    .flatten()
                    .all(|c| c.vis_box.is_some()),
                "{map} {}",
                pad.def.name
            );
        }
    }
}

#[test]
fn every_space_map_has_its_sublevels_sky() {
    for (map, _, _) in MAPS {
        let Some(b) = battle(map, 180) else {
            return;
        };
        let sky = b
            .manifest
            .sky
            .as_ref()
            .unwrap_or_else(|| panic!("{map} has no sky"));
        assert!(
            sky.models.len() >= 2,
            "{map}: {} dome models",
            sky.models.len()
        );
        assert!(
            sky.models.iter().any(|m| m.geometry.contains("sky")),
            "{map}"
        );
        assert!(sky.stars.is_some(), "{map} has no stars");
    }
}

#[test]
fn every_space_mission_opens_its_hangar_barriers() {
    for (map, _, _) in MAPS {
        let Some(b) = battle(map, 180) else {
            return;
        };
        assert!(!b.mission.barriers.is_empty(), "{map}");
        let dir = game::assets::baked_root().join(map);
        let mut nav = game::layout::nav_graph(&b.manifest, &dir).unwrap();
        for (name, enabled) in &b.mission.barriers {
            assert!(!enabled, "{map} {name}");
            let hash = game::classdb::fnv1a_lowercase(name);
            assert!(
                nav.set_barriers(hash, false) > 0,
                "{map} has no barrier {name}"
            );
        }
        assert!(b.mission.door_locks.is_empty(), "{map}");
    }
}

#[test]
fn every_objective_names_real_text_and_voice_lines() {
    let baked = missions_dir().parent().unwrap().join("assets_baked");
    let texts = game::event_feed::Texts::load(&baked, "bf2");
    for map in [
        "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
    ] {
        let Some(m) = mission(map) else {
            return;
        };
        let Ok(names) = std::fs::read_to_string(baked.join(map).join("sounds/names.ron")) else {
            return;
        };
        let def = m.space_assault.as_ref().unwrap();
        assert_eq!(def.objectives.len(), 2);
        for side in &def.objectives {
            assert!(texts.bf2_text(&side.text).is_some(), "{map} {}", side.text);
            assert!(names.contains(&side.win_vo), "{map} {}", side.win_vo);
            assert!(names.contains(&side.lose_vo), "{map} {}", side.lose_vo);
            for s in &side.systems {
                for key in [
                    format!("level.spa.hangar.{}.atk.down", s.kind),
                    format!("level.spa.hangar.{}.def.down", s.kind),
                ] {
                    assert!(texts.bf2_text(&key).is_some(), "{map} {key}");
                }
                assert!(names.contains(&s.att_vo), "{map} {}", s.att_vo);
                assert!(names.contains(&s.def_vo), "{map} {}", s.def_vo);
            }
        }
    }
}

#[test]
fn ship_weapons_keep_the_discs_clips_and_reloads() {
    let classes = game::assets::baked_root().join("classes");
    if !classes.join("bf2_imp.ron").exists() {
        eprintln!("SKIP: no baked BF2 classes");
        return;
    }
    let mut db = game::classdb::ClassDb::default();
    db.load(&classes.join("bf2_imp.ron")).unwrap();
    db.load(&classes.join("bf2_rep.ron")).unwrap();
    let mut sim = Sim::empty(1);
    let pilot_gun = |sim: &mut Sim, name: &str| {
        let seats = game::vehicle_class::seats(&db, name);
        game::vehicle_pads::seat_gun(sim, &db, &seats[0], &[]).unwrap()
    };

    let bomber = pilot_gun(&mut sim, "imp_fly_tiebomber_sc");
    let bomb = &bomber.weapons[1];
    let class = &sim.weapon_classes[bomb.class];
    assert_eq!(class.name, "imp_weap_fly_tiebomber_bomb");
    assert_eq!(class.rounds_per_clip, 6);
    assert_eq!(class.reload_time, 5.0);
    assert!(bomb.infinite);

    let gunship = pilot_gun(&mut sim, "rep_fly_gunship_sc");
    let missile = &gunship.weapons[1];
    assert_eq!(sim.weapon_classes[missile.class].rounds_per_clip, 6);
    assert_eq!(missile.reserve_clips, 5);
    assert!(!missile.infinite);
}

#[test]
fn flyers_carry_the_aim_boxes_the_ps2_holds() {
    let boxes = [
        ("imp_fly_tiefighter", 9.541),
        ("IMP_FLY_TIEBOMBER", 12.362),
        ("imp_fly_tieinterceptor", 9.606),
        ("all_fly_ywing", 19.037),
        ("all_fly_awing", 12.08),
        ("all_fly_xwing", 16.2),
    ];
    for (model, size) in boxes {
        assert_eq!(game::vehicle_pads::flyer_aim_box(model), size, "{model}");
    }
    assert_eq!(game::vehicle_pads::flyer_aim_box("not_a_ship"), 0.0);
    let Some(b) = battle("bf2_spa8", 1000) else {
        return;
    };
    let sizes: Vec<f32> = b
        .sim
        .flyer_params
        .iter()
        .flatten()
        .map(|p| p.aim_box)
        .collect();
    assert!(!sizes.is_empty());
    assert!(sizes.iter().all(|s| *s > 0.0), "{sizes:?}");
}
