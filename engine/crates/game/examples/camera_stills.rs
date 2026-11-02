//! Headless stills of the chase camera (playtest 2026-10-28,
//! `analysis/camera-occlusion.md`): the player healing with his back against
//! a health droid on BF1 Yavin 4: Arena (the camera in the droid, and the
//! droid between the camera and him), and the default standing framing on a
//! BF1 map (Yavin 4: Arena) and a BF2 map (Naboo: Theed).
//!
//!   camera_stills <out_dir> [prefix]
//!
//! Frames: `<out_dir>/<prefix>_{droid_inside,droid_between,frame_bf1,frame_bf2}.png`
//! (prefix `after` by default). Each prints the camera's place against the
//! soldier's feet, its pitch and FOV, and what it sees through.
use anyhow::{anyhow, Context, Result};
use core::sim::{buttons, PadState, Sim, SpawnStage};
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::{Mat4, Vec3};
use render::Renderer;
use std::path::{Path, PathBuf};

fn battle(root: &Path, map: &str, game: &str) -> Result<(WalkSim, game::assets::Scene)> {
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), "con")?;
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .context("assets root")?
            .join(format!("missions/{map}_con.ron")),
    )?;
    let factions = mission.factions();
    let (rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        game,
        [factions[0].as_str(), factions[1].as_str()],
        Some(&mission),
    )?;
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
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
    walk.camera_blockers = game::camera_collide::CameraBlockers::from_scene(&scene);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    walk.sim.death_regions = layout.apply_mission(&mission).death_regions;
    walk.setup_conquest(&layout, [0, 0], true);
    // The pads and droids read the posts' owners (`main/battle.rs`'s order).
    game::vehicle_pads::install(
        &mut walk.sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
    )?;
    walk.sim.open_spawn_screens(0, false);
    walk.sim.set_spawn_points(0, 12);
    for _ in 0..2 {
        walk.step_tick(&[PadState {
            buttons: buttons::SPAWN_ACCEPT,
            ..Default::default()
        }]);
        walk.step_tick(&[PadState::default()]);
    }
    for _ in 0..60 {
        walk.step_tick(&[PadState::default()]);
    }
    anyhow::ensure!(walk.sim.spawn_stage[0] == SpawnStage::Accepted, "no spawn");
    Ok((walk, scene))
}

/// Put the player at `feet` facing `facing` (horizontal) at `pitch`.
fn stand(walk: &mut WalkSim, feet: Vec3, facing: Vec3, pitch: f32) {
    let u = walk.sim.unit_mut(walk.player).unwrap();
    u.soldier.pos = feet;
    u.soldier.vel = Vec3::ZERO;
    u.soldier.yaw = (-facing.x).atan2(-facing.z);
    u.soldier.pitch = pitch;
    walk.cam_pull = 1.0;
}

/// The world box of instance `ii`'s model at `pose`.
fn model_box(scene: &game::assets::Scene, ii: usize, pose: Mat4) -> (Vec3, Vec3) {
    let inst = &scene.manifest.instances[ii];
    let mesh = &scene.meshes[scene.resolve_model(&inst.class).unwrap()];
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &mesh.positions {
        let w = pose.transform_point3(Vec3::from(*p));
        (lo, hi) = (lo.min(w), hi.max(w));
    }
    (lo, hi)
}

/// Does the segment `a -> b` cross the box?
fn crosses_box(a: Vec3, b: Vec3, (lo, hi): (Vec3, Vec3)) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..3 {
        if d[k].abs() < 1e-6 {
            if a[k] < lo[k] || a[k] > hi[k] {
                return false;
            }
            continue;
        }
        let (mut n, mut f) = ((lo[k] - a[k]) / d[k], (hi[k] - a[k]) / d[k]);
        if n > f {
            std::mem::swap(&mut n, &mut f);
        }
        (t0, t1) = (t0.max(n), t1.min(f));
        if t0 > t1 {
            return false;
        }
    }
    true
}

struct Shot<'a> {
    r: &'a mut Renderer,
    out: &'a Path,
    prefix: &'a str,
}

impl Shot<'_> {
    fn take(
        &mut self,
        name: &str,
        root: &Path,
        walk: &WalkSim,
        scene: &game::assets::Scene,
    ) -> Result<()> {
        let r = &mut *self.r;
        let mut gpu = game::viewer::upload_scene(r, scene)?;
        let mut vis = game::soldier_vis::SoldierVisuals::for_walk(r, root, scene, walk);
        for _ in 0..4 {
            vis.update(walk, core::TICK_DT);
        }
        let cam = walk.camera();
        gpu.pose_stations(&walk.sim);
        gpu.pose_turrets(&walk.sim, &walk.vehicle_seats);
        let see = walk.camera_see_through();
        gpu.see_through(&see.props);
        println!("{name}: sees through {:?}", see.props);
        let mut draws = gpu.view_draws(&walk.sim, walk.player);
        draws.extend(vis.weapon_draws(walk));
        let skinned = vis.draws(walk);
        gpu.animate_water_live(r, cam.position)?;
        r.render_frame(&cam, &draws, &skinned, gpu.clear, None)
            .map_err(|e| anyhow!("render: {e}"))?;
        let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        let png = render::encode_png(&rgba, w, h).map_err(|e| anyhow!("png: {e}"))?;
        let file = self.out.join(format!("{}_{name}.png", self.prefix));
        std::fs::write(&file, png)?;
        let feet = walk.soldier().pos;
        println!(
            "{name}: feet {feet:.2?}, camera {:.2?} from them ({:.2} m up), pitch {:.2} deg, fov {:.2} deg, body hidden {}",
            cam.position - feet,
            cam.position.y - feet.y,
            cam.pitch.to_degrees(),
            cam.fov_y.to_degrees(),
            walk.player_body_hidden()
        );
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().context("camera_stills <out_dir> [prefix]")?);
    let prefix = args.next().unwrap_or_else(|| "after".into());
    std::fs::create_dir_all(&out)?;
    let root = game::assets::baked_root();
    let mut r = Renderer::new_headless(960, 540).map_err(|e| anyhow!("renderer: {e}"))?;
    let mut shot = Shot {
        r: &mut r,
        out: &out,
        prefix: &prefix,
    };

    // The healing droid: the first health droid on Yavin 4: Arena with a
    // bearing whose ideal camera lands in it (and one where it stands
    // between), no wall between the player and that camera.
    let (mut walk, scene) = battle(&root, "bf1_yav2", "bf1")?;
    let droids: Vec<usize> = scene
        .manifest
        .instances
        .iter()
        .enumerate()
        .filter(|(_, i)| i.class.eq_ignore_ascii_case("com_item_healthrecharge"))
        .map(|(k, _)| k)
        .collect();
    let mut found: [Option<(Vec3, Vec3, f32)>; 2] = [None, None];
    'search: for &ii in &droids {
        let Some(st) = walk
            .sim
            .stations
            .iter()
            .find(|s| s.instance == Some(ii as u32))
        else {
            continue;
        };
        let pose = st.pose();
        let c = Vec3::from(scene.manifest.instances[ii].translation);
        let bx = model_box(&scene, ii, pose);
        for k in 0..12 {
            let a = std::f32::consts::TAU * k as f32 / 12.0;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            for d in (2..=18).map(|i| i as f32 * 0.2) {
                let feet = c + out * d;
                let body = feet + Vec3::Y;
                let margin = Vec3::splat(0.4);
                if body.cmpge(bx.0 - margin).all() && body.cmple(bx.1 + margin).all() {
                    continue; // he would stand in it
                }
                // In it: aiming a little up (the camera at chest height);
                // between: at the spawn pitch.
                for (slot, pitch) in [(0, 0.2), (1, core::unit::SPAWN_PITCH)] {
                    stand(&mut walk, feet, out, pitch);
                    let me = walk.sim.unit(walk.player).unwrap();
                    let ideal = me.ideal_camera(walk.sim.ground_rules, 1);
                    let pivot = feet + Vec3::Y * 1.5;
                    let clear = walk.camera_blockers.fraction(
                        pivot,
                        ideal.position,
                        Some(ii as u32),
                        |_| None,
                        &walk.sim.doors,
                    ) >= 1.0;
                    let p = ideal.position;
                    let inside = p.cmpge(bx.0).all() && p.cmple(bx.1).all();
                    let hit = match slot {
                        0 => inside,
                        _ => !inside && crosses_box(p, ideal.center, bx),
                    };
                    if clear && hit && found[slot].is_none() {
                        found[slot] = Some((feet, out, pitch));
                    }
                }
                if found.iter().all(Option::is_some) {
                    break 'search;
                }
            }
        }
    }
    for (name, f) in ["droid_inside", "droid_between"].iter().zip(found) {
        let (feet, out, pitch) = f.with_context(|| format!("no {name} placement"))?;
        stand(&mut walk, feet, out, pitch);
        shot.take(name, &root, &walk, &scene)?;
    }

    // The default framing: standing where he spawned, at the spawn pitch.
    for (name, map, game) in [
        ("frame_bf1", "bf1_yav2", "bf1"),
        ("frame_bf2", "bf2_nab2", "bf2"),
    ] {
        let (mut walk, scene) = battle(&root, map, game)?;
        let feet = walk.soldier().pos;
        let yaw = walk.soldier().yaw;
        stand(
            &mut walk,
            feet,
            Vec3::new(-yaw.sin(), 0.0, -yaw.cos()),
            core::unit::SPAWN_PITCH,
        );
        shot.take(name, &root, &walk, &scene)?;
    }
    Ok(())
}
