use anyhow::{Context, Result};
use core::sim::{buttons, PadState, Sim, SpawnStage};
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;
use std::path::Path;

pub fn battle(root: &Path) -> Result<(WalkSim, game::assets::Scene)> {
    let map = root.join("bf2_geo1");
    let (scene, _) = game::assets::load_battle_scene(&map, "con")?;
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .context("assets root")?
            .join("missions/bf2_geo1_con.ron"),
    )?;
    let (rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        "bf2",
        [
            mission.teams[0].faction.as_str(),
            mission.teams[1].faction.as_str(),
        ],
        Some(&mission),
    )?;
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
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
