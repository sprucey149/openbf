use anyhow::{Context, Result};
use core::sim::{buttons, PadState, Sim, SpawnStage};
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;
use std::path::Path;

#[allow(dead_code)]
pub fn battle(root: &Path) -> Result<(WalkSim, game::assets::Scene)> {
    battle_on(root, "bf2_geo1", "bf2_geo1_con")
}

pub fn battle_on(root: &Path, map: &str, mission: &str) -> Result<(WalkSim, game::assets::Scene)> {
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), "con")?;
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .context("assets root")?
            .join(format!("missions/{mission}.ron")),
    )?;
    let (rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        &mission.source_game,
        [
            mission.teams[0].faction.as_str(),
            mission.teams[1].faction.as_str(),
        ],
        Some(&mission),
    )?;
    let mut sim = Sim::empty(42);
    sim.ground_rules = if mission.source_game == "bf1" {
        core::soldier::GroundRules::Bf1
    } else {
        core::soldier::GroundRules::Bf2
    };
    for (team, definition) in mission.teams.iter().enumerate() {
        sim.battle.teams[team] = core::battle::TeamState::new(definition.tickets);
    }
    let mut names = Vec::new();
    let mut team = 0;
    let templates = rosters.map(|roster| {
        let table: Vec<_> = roster
            .classes
            .iter()
            .map(|class| class.register(&mut sim, &mut names))
            .collect();
        let visuals = roster
            .classes
            .iter()
            .map(ClassVisual::from_roster)
            .collect();
        let template = TeamTemplate::from_roster(&roster, &table[0], visuals);
        sim.classes[team] = table;
        team += 1;
        template
    });
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut walk = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, templates, nav);
    scene.install_collision(&mut walk.sim);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    walk.sim.death_regions = layout.apply_mission(&mission).death_regions;
    walk.setup_conquest(&layout, [0, 0], true);
    let factions = [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()];
    let pads = game::vehicle_pads::install(
        &mut walk.sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
    )?;
    walk.vehicle_geometry = pads.geometry;
    walk.vehicle_seats = pads.seats;
    walk.vehicle_weapon_hud = pads.weapon_hud;
    walk.walker_shot_styles = pads.walker_shot_styles;
    walk.load_walker_views(&scene.manifest, root);
    walk.vehicle_riders = pads.riders;
    walk.vehicle_engines = pads.engines;
    walk.vehicle_sounds = pads.sounds;
    walk.vehicle_labels = pads.labels;
    walk.vehicle_classes = pads.classes;
    walk.sim.open_spawn_screens(0, false);
    walk.sim.set_spawn_points(0, 12);
    for _ in 0..2 {
        walk.step_tick(&[PadState {
            buttons: buttons::SPAWN_ACCEPT,
            ..Default::default()
        }]);
        walk.step_tick(&[PadState::default()]);
    }
    for _ in 0..90 {
        walk.step_tick(&[PadState::default()]);
    }
    anyhow::ensure!(walk.sim.spawn_stage[0] == SpawnStage::Accepted, "no spawn");
    Ok((walk, scene))
}

/// Derives the baked map directory name from a mission script name (`bf1_rhn1_con_cw` -> `bf1_rhn1`).
#[allow(dead_code)]
pub fn map_of(mission: &str) -> String {
    let words = if mission.starts_with("bf") { 2 } else { 1 };
    mission.split('_').take(words).collect::<Vec<_>>().join("_")
}

/// Starts a battle on `mission`, finds the first live `class` vehicle, and seats the player in `seat`.
#[allow(dead_code)]
pub fn board_vehicle(
    root: &Path,
    mission: &str,
    class: &str,
    seat: u8,
) -> Result<(WalkSim, game::assets::Scene, core::vehicle::VehicleHandle)> {
    let map = map_of(mission);
    let (mut walk, scene) = battle_on(root, &map, mission)?;
    let index = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(class))
        .with_context(|| format!("{class} not on {map}"))?;
    let slot = walk
        .sim
        .vehicles
        .iter()
        .position(|v| v.alive && v.class as usize == index)
        .context("no live vehicle")?;
    let handle = core::vehicle::VehicleHandle {
        index: slot as u32,
        gen: walk.sim.vehicles[slot].gen,
    };
    let player = walk.player;
    anyhow::ensure!(walk.sim.seat_unit(player, handle, seat), "cannot board");
    Ok((walk, scene, handle))
}
