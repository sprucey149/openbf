//! Issue #182 on the real bake (data-gated: skipped when the bake is absent):
//! a soldier who rolls at the wall side of a Theed stair stays on the steps.
//!
//! The playtest black box of 2026-11-16 (BF1 Theed, Conquest, Elite, seed 56,
//! F9 at tick 6464) shows the player rolling up the street stair of
//! `nab2_bldg_i_20` at (-98.5, -48.8, 80.7), along its wall. He dropped
//! through the slot under the wall-side balustrade to the street at
//! (-100.3, -50.0, 81.9) and stood under the steps until F9 at
//! (-105.3, -50.0, 81.5). `blackbox_read --replay-spot 62` gives the spot.
//! `game::collision_fixes` closes the slot. These tests replay the
//! roll at that spot, then the same roll across every wall-side stair of the
//! model on both Theed maps.

use core::sim::{buttons, PadState, Sim};
use glam::{Mat4, Vec3};
use std::path::Path;

/// One soldier on the map's collision, as the leak census builds it.
fn build_sim(root: &Path, key: &str) -> game::walk::WalkSim {
    let path = root.join(key);
    let (scene, _) = game::assets::load_battle_scene(&path, "con").unwrap();
    let missions = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mission = game::mission::MissionDef::load(&missions.join(format!("{key}_con.ron"))).ok();
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));
    let factions = mission
        .as_ref()
        .map(game::mission::MissionDef::factions)
        .unwrap_or_else(|| {
            game::roster::default_factions(&scene.manifest.source_game).map(str::to_owned)
        });
    let mut team = game::walk::TeamTemplate::default_trooper();
    if let Ok((rs, _)) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    ) {
        let f = rs[1].fill_class();
        team.consts = f.consts;
        team.name = f.class.clone();
    }
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut sim = Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    let mut w = game::walk::WalkSim::with_teams(
        sim,
        scene.heightfield(),
        Vec3::ZERO,
        [team.clone(), team],
        nav,
    );
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
    }
    w
}

/// The `nab2_bldg_i_20` instances of a Theed map, as their manifest places
/// them: the one by the west street, then its turned twin.
fn stair_buildings(key: &str) -> [Mat4; 2] {
    let upright = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let turned = [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
    let at: [[f32; 3]; 2] = if key == "bf1_nab2" {
        [[-119.838, -50.193, 79.823], [-40.172, -50.316, 79.832]]
    } else {
        [[48.063, 7.782, 80.262], [127.73, 7.659, 80.271]]
    };
    [
        game::viewer::instance_matrix(upright, at[0]),
        game::viewer::instance_matrix(turned, at[1]),
    ]
}

/// A wall-side stair in model space: the low end of its wall-side step
/// edge, the up-slope direction and the direction from the wall into it.
/// The steps rise 0.5 m per metre, 16 m long, 4.6 m wide.
struct Stair {
    low: Vec3,
    up: Vec3,
    inward: Vec3,
}

const STAIRS: [Stair; 2] = [
    Stair {
        low: Vec3::new(24.0, 0.0, 0.7),
        up: Vec3::NEG_X,
        inward: Vec3::Z,
    },
    Stair {
        low: Vec3::new(-0.71, 0.0, -24.0),
        up: Vec3::Z,
        inward: Vec3::NEG_X,
    },
];

/// Where a soldier ended relative to a stair: `Some(depth)` when he is
/// under the steps or in the wall-side slot, more than 1 m below the step
/// surface over him.
fn under_stair(building: Mat4, stair: &Stair, pos: Vec3) -> Option<f32> {
    let l = building.inverse().transform_point3(pos) - stair.low;
    let (along, across) = (l.dot(stair.up), l.dot(stair.inward));
    let depth = along * 0.5 - l.y;
    (along > 0.5 && along < 15.5 && across > -0.75 && across < 4.6 && depth > 1.0).then_some(depth)
}

/// Stand the soldier at `start` (on the floor under it), facing `yaw`,
/// run forward and roll on tick `roll_at`, then let him settle.
fn roll_from(
    w: &mut game::walk::WalkSim,
    start: Vec3,
    yaw: f32,
    sprint: bool,
    roll_at: u32,
) -> Vec3 {
    let pi = w.player.index as usize;
    let floor = w.sim.ground_under(start + Vec3::Y * 2.0, false);
    let u = &mut w.sim.units[pi];
    u.soldier = core::soldier::Soldier::new(Vec3::new(start.x, floor, start.z));
    u.soldier.yaw = yaw;
    for t in 0..150u32 {
        let mut pad = PadState::default();
        if t < 60 {
            pad.ly = i16::MAX;
            if sprint {
                pad.buttons |= buttons::SPRINT;
            }
        }
        if t == roll_at {
            pad.buttons |= buttons::ROLL_STANCE;
        }
        w.sim.step(&[pad]);
        assert!(w.sim.units[pi].alive, "died rolling from {start}");
    }
    w.sim.units[pi].soldier.pos
}

fn bake_or_skip(key: &str) -> Option<std::path::PathBuf> {
    let root = game::assets::baked_root();
    if !root.join(key).join("manifest.ron").exists() || !root.join("classes").exists() {
        eprintln!("skipped: no {key} bake");
        return None;
    }
    Some(root)
}

/// The playtest spot: rolls from the street stair's wall side, where the
/// player rolled, in every direction.
#[test]
fn a_roll_at_the_theed_street_stair_wall_stays_on_the_steps() {
    let Some(root) = bake_or_skip("bf1_nab2") else {
        return;
    };
    let mut w = build_sim(&root, "bf1_nab2");
    let [building, _] = stair_buildings("bf1_nab2");
    let mut runs = 0;
    for start in [Vec3::new(-98.5, -48.0, 80.9), Vec3::new(-96.0, -49.0, 81.6)] {
        for step in 0..24 {
            let yaw = step as f32 * 15f32.to_radians();
            for (sprint, roll_at) in [(false, 0), (true, 6), (true, 12)] {
                let end = roll_from(&mut w, start, yaw, sprint, roll_at);
                runs += 1;
                assert_eq!(
                    under_stair(building, &STAIRS[0], end),
                    None,
                    "rolled from {start} at yaw {yaw:.2} (sprint {sprint}, roll on tick {roll_at}) to {end}"
                );
            }
        }
    }
    assert_eq!(runs, 144);
}

/// The same roll along every wall-side stair of both buildings, on both
/// Theed maps (one mesh in both games).
#[test]
fn rolls_along_every_theed_wall_stair_stay_on_the_steps() {
    for key in ["bf1_nab2", "bf2_nab2"] {
        let Some(root) = bake_or_skip(key) else {
            continue;
        };
        let mut w = build_sim(&root, key);
        let mut runs = 0;
        for building in stair_buildings(key) {
            for stair in &STAIRS {
                for along in [4.0f32, 8.0, 12.0] {
                    for across in [0.3f32, 1.0, 2.8] {
                        let local = stair.low + stair.up * along + stair.inward * across;
                        let start = building.transform_point3(local + Vec3::Y * along * 0.5);
                        for step in 0..12 {
                            let yaw = step as f32 * 30f32.to_radians();
                            for (sprint, roll_at) in [(false, 0), (true, 6)] {
                                let end = roll_from(&mut w, start, yaw, sprint, roll_at);
                                runs += 1;
                                assert_eq!(
                                    under_stair(building, stair, end),
                                    None,
                                    "{key}: rolled from {start} at yaw {yaw:.2} (sprint {sprint}) to {end}"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(runs, 4 * 9 * 24, "{key}");
    }
}
