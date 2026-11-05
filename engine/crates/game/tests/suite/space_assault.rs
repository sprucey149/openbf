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
fn a_dogfight_kill_grants_three_to_the_killers_team() {
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
    assert_eq!(b.points(), [before[0] + 3, before[1]]);
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
