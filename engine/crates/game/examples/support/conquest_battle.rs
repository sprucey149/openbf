//! A real baked conquest round for the headless sim examples (issue #168):
//! a human in slot 0, `bots` per side, and the human's scripted pads.

use anyhow::{Context, Result};
use core::sim::{buttons, PadState, Sim};
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;
use std::path::Path;

/// Build `key`'s conquest round from the baked assets under `root`.
pub fn battle(root: &Path, key: &str, bots: u32) -> Result<WalkSim> {
    let (scene, _) = game::assets::load_battle_scene(&root.join(key), "con")?;
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .context("assets root")?
            .join(format!("missions/{key}_con.ron")),
    )?;
    let game_key = &key[..3];
    let (rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        game_key,
        [
            mission.teams[0].faction.as_str(),
            mission.teams[1].faction.as_str(),
        ],
        Some(&mission),
    )?;
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    sim.ai_alive_cap = bots;
    for (team, definition) in mission.teams.iter().enumerate() {
        sim.battle.teams[team] = core::battle::TeamState::new(definition.tickets);
    }
    let mut names = Vec::new();
    let mut team = 0;
    let templates: [TeamTemplate; 2] = rosters.map(|roster| {
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
    walk.setup_conquest(&layout, [bots, bots], true);
    let factions = [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()];
    game::vehicle_pads::install(
        &mut walk.sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
    )?;
    Ok(walk)
}

/// The human's pads: walk forward turning, fire in bursts.
pub fn pad(t: u64) -> PadState {
    PadState {
        ly: i16::MAX / 2,
        rx: if (t / 60).is_multiple_of(2) {
            4000
        } else {
            -4000
        },
        buttons: if t % 40 < 10 { buttons::FIRE } else { 0 },
        ..Default::default()
    }
}
