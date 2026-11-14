//! The chase camera in doorways (issue: "even walking through doors puts the
//! camera in the trooper"): stands or walks a headless soldier through a
//! map's doors and logs the boom length per tick. No window, no GPU.
//!
//!   camera_doorway <map> spot <x,y,z,yaw,pitch>   what stops the track there
//!   camera_doorway <map> doors [--verbose]        walk through every door
//!
//! `boom` is the drawn camera's distance from the pull point, `ideal` the
//! unpulled one, `f` the stored track fraction, `raw` this frame's.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)
use anyhow::{Context, Result};
use core::sim::{buttons, PadState, Sim, SpawnStage};
use game::assets::Scene;
use game::camera_collide::CameraBlockers;
use game::walk::{ClassVisual, TeamTemplate, WalkSim, CAMERA_PULL_Y};
use glam::Vec3;

fn battle(map: &str) -> Result<(WalkSim, Scene)> {
    let root = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), "con")?;
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .context("assets root")?
            .join(format!("missions/{map}_con.ron")),
    )?;
    let factions = mission.factions();
    let source = &scene.manifest.source_game;
    let sides = [factions[0].as_str(), factions[1].as_str()];
    let (rosters, _) =
        game::roster::FactionRoster::pair(&root.join("classes"), source, sides, Some(&mission))?;
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(source);
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
    walk.camera_blockers = CameraBlockers::from_scene(&scene);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    walk.sim.death_regions = layout.apply_mission(&mission).death_regions;
    walk.setup_conquest(&layout, [0, 0], true);
    game::vehicle_pads::install(
        &mut walk.sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
    )?;
    anyhow::ensure!(walk.sim.open_spawn_screens(0, false), "no spawn screen");
    walk.sim.set_spawn_points(0, 12);
    for _ in 0..2 {
        walk.step_tick(&[PadState {
            buttons: buttons::SPAWN_ACCEPT,
            ..Default::default()
        }]);
        walk.step_tick(&[PadState::default()]);
    }
    for _ in 0..30 {
        walk.step_tick(&[PadState::default()]);
    }
    anyhow::ensure!(
        walk.sim.spawn_stage[0] == SpawnStage::Accepted,
        "player not spawned"
    );
    Ok((walk, scene))
}

fn label(scene: &Scene, ii: u32) -> String {
    let inst = &scene.manifest.instances[ii as usize];
    let base = scene
        .resolve_class(&inst.class)
        .map_or("?", |c| c.base.as_str());
    format!(
        "#{ii} {} (base {base}) at {:?}",
        inst.class, inst.translation
    )
}

/// One row: tick, feet, boom, ideal boom, stored fraction, door opens.
fn row(walk: &WalkSim) -> (Vec3, f32, f32, f32) {
    let me = walk.sim.unit(walk.player).unwrap();
    let feet = me.soldier.pos;
    let pull = feet + Vec3::Y * CAMERA_PULL_Y;
    let ideal = me.ideal_camera(walk.sim.ground_rules, walk.sim.views());
    (
        feet,
        walk.camera().position.distance(pull),
        ideal.position.distance(pull),
        walk.cam_pull,
    )
}

fn place(walk: &mut WalkSim, feet: Vec3, yaw: f32, pitch: f32) {
    let p = walk.player;
    let u = walk.sim.unit_mut(p).unwrap();
    u.soldier.pos = feet;
    u.soldier.yaw = yaw;
    u.soldier.pitch = pitch;
    u.soldier.vel = Vec3::ZERO;
}

fn spot(walk: &mut WalkSim, scene: &Scene, s: &str) -> Result<()> {
    let v: Vec<f32> = s.split(',').map(str::parse).collect::<Result<_, _>>()?;
    anyhow::ensure!(v.len() == 5, "x,y,z,yaw,pitch");
    place(walk, Vec3::new(v[0], v[1], v[2]), v[3], v[4]);
    for _ in 0..3 {
        walk.step_tick(&[PadState::default()]);
    }
    let me = walk.sim.unit(walk.player).unwrap();
    let feet = me.soldier.pos;
    let pull = feet + Vec3::Y * CAMERA_PULL_Y;
    let ideal = me.ideal_camera(walk.sim.ground_rules, walk.sim.views());
    println!("feet {feet:?} pull {pull:?} ideal {:?}", ideal.position);
    println!(
        "drawn {:?} boom {:.2} of {:.2} (stored f {:.2})",
        walk.camera().position,
        walk.camera().position.distance(pull),
        ideal.position.distance(pull),
        walk.cam_pull
    );
    for (ii, t) in walk
        .camera_blockers
        .track_hits(pull, ideal.position, &walk.sim.doors)
    {
        println!("  track stops at {t:.2} m on {}", label(scene, ii));
    }
    for d in &walk.sim.doors {
        let o = d.origin;
        if o.distance(feet) < 12.0 {
            println!(
                "  door {} open {:.2} triggers {:?} ({:.1} m away)",
                label(scene, d.instance as u32),
                d.open,
                d.triggers,
                o.distance(feet)
            );
        }
    }
    Ok(())
}

/// Walk through one door along `dir` (the door's axis across its frame): from
/// 7 m before it to 7 m past it, on the real pad. Returns the table's cells.
fn walk_door(walk: &mut WalkSim, door: usize, dir: Vec3, verbose: bool) -> Option<String> {
    let o = walk.sim.doors[door].origin;
    let start = o - dir * 7.0;
    let yaw = (-dir.x).atan2(-dir.z);
    place(walk, start + Vec3::Y * 0.5, yaw, -0.1);
    for _ in 0..20 {
        walk.step_tick(&[PadState::default()]);
    }
    let settle = walk.sim.unit(walk.player)?.soldier.pos;
    if settle.distance(start) > 3.0 {
        return None;
    }
    let (mut min_boom, mut ideal_min, mut through) = (f32::MAX, f32::MAX, 0.0f32);
    let mut collapsed = 0;
    let mut open_max = 0.0f32;
    for tick in 0..120 {
        walk.step_tick(&[PadState {
            ly: 32767,
            ..Default::default()
        }]);
        let (feet, boom, ideal, f) = row(walk);
        open_max = open_max.max(walk.sim.doors[door].open);
        through = through.max((feet - o).dot(dir));
        min_boom = min_boom.min(boom);
        ideal_min = ideal_min.min(ideal);
        if boom < 1.2 {
            collapsed += 1;
        }
        if verbose {
            let me = walk.sim.unit(walk.player)?;
            let want = me.ideal_camera(walk.sim.ground_rules, walk.sim.views());
            let hits = walk.camera_blockers.track_hits(
                feet + Vec3::Y * CAMERA_PULL_Y,
                want.position,
                &walk.sim.doors,
            );
            println!(
                "    t{tick:3} along {:6.2} boom {boom:5.2}/{ideal:4.2} f {f:4.2} open {:4.2} hits {hits:?}",
                (feet - o).dot(dir),
                walk.sim.doors[door].open
            );
        }
    }
    Some(format!(
        "min boom {min_boom:.2} of {ideal_min:.2}, {collapsed} ticks under 1.2 m, walked to {through:+.1} m, door opened to {open_max:.2}"
    ))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let map = args.first().context("map")?;
    let (mut walk, scene) = battle(map)?;
    match args.get(1).map(String::as_str) {
        Some("spot") => spot(&mut walk, &scene, args.get(2).context("x,y,z,yaw,pitch")?)?,
        Some("doors") => {
            let verbose = args.iter().any(|a| a == "--verbose");
            println!("{map}: {} doors", walk.sim.doors.len());
            for i in 0..walk.sim.doors.len() {
                // The axis across the doorway: square to the leaves' travel.
                let d = walk.sim.doors[i].clone();
                let inst = &scene.manifest.instances[d.instance];
                let travel = d
                    .bodies
                    .first()
                    .map(|b| b.frames.last().unwrap().1 - b.frames[0].1)
                    .unwrap_or(Vec3::X);
                let along = travel.with_y(0.0).normalize_or_zero();
                println!("door {i} {} at {:?}", inst.class, d.origin);
                // A leaf that only rises says nothing of the doorway's axis:
                // try the world's two.
                let axes = if along == Vec3::ZERO {
                    vec![Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z]
                } else {
                    let across = Vec3::new(-along.z, 0.0, along.x);
                    vec![across, -across]
                };
                for dir in axes {
                    match walk_door(&mut walk, i, dir, verbose) {
                        Some(cells) => println!("  dir {dir:.2?}: {cells}"),
                        None => println!("  dir {dir:.2?}: could not stand at the start"),
                    }
                }
            }
        }
        _ => anyhow::bail!("spot or doors"),
    }
    Ok(())
}
