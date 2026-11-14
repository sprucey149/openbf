//! A human drives a walker on a real map (card WALK, headless):
//! `cargo run -p game --release --example walker_drive -- [map] [class] [vehicles.ron]`
//! (defaults `bf2_hot1 imp_walk_atst`, the class's side bake beside the map;
//! pass a scratch `vehicles.ron` to try a bake before it lands).
//!
//! The walker stands at the first pad naming the class; a player is seated
//! at its pilot seat. Scripted pad: 2 s idle, 8 s full throttle, 4 s with
//! the stick pushed right, 6 s throttle + SPRINT (the boost), 3 s released,
//! 5 s with the aim stick pushed right (the idle turn steps), 2 s released.
//! Printed per phase: ground speed, the body's yaw rate, the clip, the
//! energy and the origin's height against the terrain under it. The pads
//! are replayed into a fresh sim: the two hashes must agree. `WALKER_TRACE=1`
//! prints the walker's position, the terrain under it and v.y every tick.
//!
//! `--frames <dir> [--every N] [--back M]` renders the drive (card WSKN): a
//! 640×360 chase view from M metres behind (default 16) of the walker every
//! N ticks (default 5), as the battle draws
//! it (`game::viewer::VehicleVisuals`), `<dir>/<tick>_<phase>.png`.
//! `--assets <root>` reads the map, classes and side bakes from `<root>`
//! instead of `assets_baked` (an overlay over a scratch bake).
//!
//! With the class's skin baked, each phase also prints the planted foot's
//! slip: per tick, the slower of the two FootBone toes' horizontal speed in
//! the world (the posed skeleton, `game::viewer::walker_pose`), its 10th
//! percentile, median and 90th percentile — a planted foot reads ~0. A
//! running mount has a foot down on few ticks of a stride (BF2's tauntaun:
//! median 14 m/s at 20 m/s), so its planted foot is the 10th percentile.
//! Each phase also prints the speed it settles at (its second half).
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::sim::{buttons, PadState, Sim};
use core::unit::UnitSpec;
use core::vehicle::walker::WalkerState;
use core::vehicle::{VehicleHandle, VehicleKind, VehicleSpec};
use game::classdb::ClassDb;
use game::vehicle_class::WalkerClass;
use glam::Vec3;
use std::path::{Path, PathBuf};

fn world(
    dir: &Path,
    root: &Path,
    class: &str,
    bake: Option<&Path>,
) -> anyhow::Result<(Sim, core::unit::UnitHandle, VehicleHandle)> {
    let mut scene = game::assets::load_scene(dir)?;
    scene.apply_mode_layer("con");
    let mut sim = Sim::empty(1);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let game_id = scene.manifest.source_game.clone();
    let mut db = ClassDb::default();
    db.load_map(dir)?;
    let map = dir.file_name().map(|n| n.to_string_lossy().into_owned());
    let extra = game::vehicle_pads::mission_side_files(&game_id, map.as_deref().unwrap_or(""));
    let files = game::roster::class_files(&game_id, &["all", "imp", "rep", "cis"]);
    for f in files.into_iter().chain(extra.iter().copied()) {
        let p = root.join("classes").join(f);
        if p.exists() {
            db.load(&p)?;
        }
    }
    let walker = match bake {
        Some(p) => {
            let vm: manifest::VehicleManifest = ron::from_str(&std::fs::read_to_string(p)?)?;
            vm.vehicles
                .into_iter()
                .find(|v| v.class.name.eq_ignore_ascii_case(class))
                .and_then(|v| v.walker)
        }
        None => game::vehicle_pads::walker_bakes(dir, &game_id).remove(class),
    };
    anyhow::ensure!(walker.is_some(), "{class}: no walker bake");
    let params = WalkerClass::from_db(&db, class)
        .ok_or_else(|| anyhow::anyhow!("{class}: not a walker class"))?
        .params(walker.as_ref());
    eprintln!(
        "{class}: {} leg pair(s) {:?}, {} building sphere(s), clips {}",
        params.leg_pairs.len(),
        params.leg_pairs,
        params.building.len(),
        params.clips.iter().filter(|c| c.is_some()).count()
    );
    sim.walker_params = vec![Some(params)];
    let ring = game::vehicle_class::seats(&db, class);
    sim.seat_setup = vec![Some(game::vehicle_pads::seat_setup(
        &db, class, &ring, &game_id,
    ))];
    let inst = scene
        .manifest
        .instances
        .iter()
        .filter(|i| i.class == game::vehicle_pads::PAD_CLASS)
        .find(|i| {
            i.props
                .iter()
                .any(|(_, v)| v.trim().eq_ignore_ascii_case(class))
        })
        .ok_or_else(|| anyhow::anyhow!("no pad for {class}"))?;
    let at = Vec3::from(inst.translation);
    let g = sim.ground_under(at, false);
    let v = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(at.x, g, at.z),
        yaw: 0.0,
        max_health: db.prop_f32(class, "MaxHealth").unwrap_or(1000.0),
        health_type: game::vehicle_class::health_type(&db, class),
        seats: ring.iter().map(|s| s.kind).collect(),
    });
    let p = sim
        .spawn(&UnitSpec::player(0, at + Vec3::new(0.0, 0.0, -10.0)))
        .ok_or_else(|| anyhow::anyhow!("no player"))?;
    anyhow::ensure!(sim.enter_vehicle(p, v, 0), "could not seat the player");
    Ok((sim, p, v))
}

fn phases() -> Vec<(&'static str, u32, PadState)> {
    let full = i16::MAX;
    vec![
        ("idle", 60, PadState::default()),
        (
            "throttle",
            240,
            PadState {
                ly: full,
                ..PadState::default()
            },
        ),
        (
            "stick right",
            120,
            PadState {
                lx: full,
                ..PadState::default()
            },
        ),
        (
            "boost",
            180,
            PadState {
                ly: full,
                buttons: buttons::SPRINT,
                ..PadState::default()
            },
        ),
        ("released", 90, PadState::default()),
        // Card WSKN: the aim swung right from a standstill — the body turns
        // in place and the legs catch up in turn steps (re-walker §4.7).
        (
            "aim right",
            150,
            PadState {
                rx: full,
                ..PadState::default()
            },
        ),
        ("settle", 60, PadState::default()),
    ]
}

fn yaw(q: glam::Quat) -> f32 {
    let f = q * Vec3::Z;
    core::fmath::atan2(f.x, f.z)
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI
}

/// The class's posed skeleton, for the planted foot's slip.
struct Feet {
    rig: game::viewer::walker_pose::WalkerRig,
    toes: [usize; 2],
    scale: f32,
}

impl Feet {
    fn load(root: &Path, sim: &Sim, class: &str) -> Option<Self> {
        let sides = std::fs::read_dir(root.join("side_bf2").join("vehicles"))
            .into_iter()
            .chain(std::fs::read_dir(root.join("side_bf1").join("vehicles")))
            .flatten()
            .flatten();
        for side in sides {
            let Ok(text) = std::fs::read_to_string(side.path().join("vehicles.ron")) else {
                continue;
            };
            let Ok(m) = ron::from_str::<manifest::VehicleManifest>(&text) else {
                continue;
            };
            let Some(w) = m
                .vehicles
                .iter()
                .find(|v| v.class.name.eq_ignore_ascii_case(class))
                .and_then(|v| v.walker.clone())
            else {
                continue;
            };
            let skin = w.skin.clone()?;
            let a = game::assets::load_skinned_glb(
                &side.path().join("walkers").join(format!("{skin}.glb")),
            )
            .ok()?;
            let p = sim.walker_params.first()?.as_ref()?;
            let toe = |n: &str| a.skeleton.index_of(n);
            let toes = [toe("bone_l_toe")?, toe("bone_r_toe")?];
            return Some(Self {
                rig: game::viewer::walker_pose::WalkerRig::new(a.skeleton, a.clips, &w),
                toes,
                scale: p.root_scale,
            });
        }
        None
    }

    /// The two toes in the world.
    fn toes(&self, sim: &Sim, v: VehicleHandle) -> [Vec3; 2] {
        let w = sim.vehicle(v).expect("alive");
        let pal = self.rig.palette(&w.walker, self.scale);
        let m = glam::Mat4::from_rotation_translation(w.rot, w.pos);
        self.toes.map(|b| {
            let bind = self.rig.skeleton.global_bind[b].w_axis.truncate();
            m.transform_point3(pal[b].transform_point3(bind))
        })
    }
}

/// The drive drawn (`--frames`): the map uploaded, the walker's class model.
struct Frames {
    r: render::Renderer,
    gpu: game::viewer::SceneGpu,
    vis: game::viewer::VehicleVisuals,
    out: PathBuf,
    every: u32,
    /// The chase camera's distance behind (m); the rest scales with it.
    back: f32,
}

impl Frames {
    fn new(
        dir: &Path,
        root: &Path,
        class: &str,
        out: PathBuf,
        every: u32,
        back: f32,
    ) -> anyhow::Result<Self> {
        let mut r = render::Renderer::new_headless(640, 360).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut scene = game::assets::load_scene(dir)?;
        scene.apply_mode_layer("con");
        let gpu = game::viewer::upload_scene(&mut r, &scene)?;
        let mut db = ClassDb::default();
        db.load_map(dir)?;
        for f in
            game::roster::class_files(&scene.manifest.source_game, &["all", "imp", "rep", "cis"])
        {
            let p = root.join("classes").join(f);
            if p.exists() {
                db.load(&p)?;
            }
        }
        let geometry = db.prop(class, "GeometryName").map(str::to_owned);
        let vis = game::viewer::VehicleVisuals::load(&mut r, &scene, root, &[geometry]);
        std::fs::create_dir_all(&out)?;
        Ok(Self {
            r,
            gpu,
            vis,
            out,
            every,
            back,
        })
    }

    fn shot(&mut self, sim: &Sim, v: VehicleHandle, tick: u32, phase: &str) -> anyhow::Result<()> {
        if !tick.is_multiple_of(self.every) {
            return Ok(());
        }
        let w = sim.vehicle(v).expect("alive");
        let fwd = w.rot * Vec3::Z;
        let right = w.rot * Vec3::X;
        let k = self.back / 16.0;
        let eye = w.pos + (-fwd * 16.0 + right * 9.0 + Vec3::Y * 6.0) * k;
        let cam = game::viewer::look_at(eye, w.pos + Vec3::Y * 3.0 * k);
        let mut draws = self.gpu.draws.clone();
        draws.extend(self.vis.draws(&mut self.r, sim, &[]));
        self.r
            .render_frame(&cam, &draws, &[], self.gpu.clear, None)
            .map_err(|e| anyhow::anyhow!("render: {e}"))?;
        let (rgba, pw, ph) = self.r.capture_rgba().map_err(|e| anyhow::anyhow!("{e}"))?;
        let png = render::encode_png(&rgba, pw, ph).map_err(|e| anyhow::anyhow!("{e}"))?;
        let name = phase.replace(' ', "_");
        std::fs::write(self.out.join(format!("{tick:04}_{name}.png")), png)?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| {
        let i = args.iter().position(|a| a == name)?;
        let v = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        v
    };
    let frames_dir = flag("--frames").map(PathBuf::from);
    let every: u32 = flag("--every").and_then(|v| v.parse().ok()).unwrap_or(5);
    let back: f32 = flag("--back").and_then(|v| v.parse().ok()).unwrap_or(16.0);
    let root = flag("--assets").map_or_else(game::assets::baked_root, PathBuf::from);
    let map = args.first().map_or("bf2_hot1", String::as_str);
    let class = args.get(1).map_or("imp_walk_atst", String::as_str);
    let bake = args.get(2).map(PathBuf::from);
    let dir: PathBuf = root.join(map);

    let (mut sim, _, v) = world(&dir, &root, class, bake.as_deref())?;
    let feet = Feet::load(&root, &sim, class);
    let mut frames = frames_dir
        .map(|out| Frames::new(&dir, &root, class, out, every.max(1), back))
        .transpose()?;
    let mut tick = 0u32;
    let dt = core::TICK_DT;
    for (name, ticks, pad) in phases() {
        let w0 = sim.vehicle(v).expect("alive").clone();
        let (mut max_rate, mut prev) = (0.0f32, yaw(w0.rot));
        let mut anims = Vec::new();
        let mut slips = Vec::new();
        let mut steps = Vec::new();
        let mut toes0 = feet.as_ref().map(|f| f.toes(&sim, v));
        for _ in 0..ticks {
            let from = sim.vehicle(v).expect("alive").pos;
            sim.step(&[pad]);
            tick += 1;
            let step = sim.vehicle(v).expect("alive").pos - from;
            steps.push(Vec3::new(step.x, 0.0, step.z).length() / dt);
            if let Some(f) = frames.as_mut() {
                f.shot(&sim, v, tick, name)?;
            }
            if let (Some(f), Some(t0)) = (feet.as_ref(), toes0.as_mut()) {
                let t1 = f.toes(&sim, v);
                let speed = |i: usize| {
                    let d = t1[i] - t0[i];
                    Vec3::new(d.x, 0.0, d.z).length() / dt
                };
                slips.push(speed(0).min(speed(1)));
                *t0 = t1;
            }
            let w = sim.vehicle(v).expect("alive");
            let y = yaw(w.rot);
            max_rate = max_rate.max(wrap(y - prev).abs() / dt);
            prev = y;
            if std::env::var_os("WALKER_TRACE").is_some() {
                let g = sim
                    .heightfield
                    .as_ref()
                    .map_or(f32::NAN, |h| h.height_at(w.pos.x, w.pos.z));
                eprintln!(
                    "{name} {:.2} {:.2} {:.2} terrain {:.2} vy {:.2}",
                    w.pos.x, w.pos.y, w.pos.z, g, w.vel.y
                );
            }
            if anims.last() != Some(&w.walker.anim) {
                anims.push(w.walker.anim);
            }
        }
        let w = sim.vehicle(v).expect("alive");
        let d = w.pos - w0.pos;
        let secs = ticks as f32 * dt;
        let ground = sim
            .heightfield
            .as_ref()
            .map_or(f32::NAN, |h| h.height_at(w.pos.x, w.pos.z));
        let st: &WalkerState = &w.walker;
        if !slips.is_empty() {
            slips.sort_by(f32::total_cmp);
            println!(
                "{name:12} planted foot slip: p10 {:.3} m/s, median {:.3} m/s, p90 {:.3} m/s",
                slips[slips.len() / 10],
                slips[slips.len() / 2],
                slips[slips.len() * 9 / 10]
            );
        }
        // The phase's second half, tick by tick: the speed it settles at.
        let mut late = steps.split_off(steps.len() / 2);
        late.sort_by(f32::total_cmp);
        println!(
            "{name:12} steady {:.2} m/s (the second half's median tick; mean {:.2})",
            late[late.len() / 2],
            late.iter().sum::<f32>() / late.len() as f32
        );
        println!(
            "{name:12} {secs:4.1} s  {:6.2} m/s (xz)  yaw {:+6.2} rad (max {:.3} rad/s)  anims {:?}  energy {:5.1}{}  y−terrain {:+.3}  aim {:+.2}",
            Vec3::new(d.x, 0.0, d.z).length() / secs,
            wrap(yaw(w.rot) - yaw(w0.rot)),
            max_rate,
            anims,
            st.energy,
            if st.exhausted { " (exhausted)" } else { "" },
            w.pos.y - ground,
            st.aim_yaw,
        );
    }
    let hash = sim.hash();

    // The replay: the same pads into a fresh sim.
    let (mut again, _, _) = world(&dir, &root, class, bake.as_deref())?;
    for (_, ticks, pad) in phases() {
        for _ in 0..ticks {
            again.step(&[pad]);
        }
    }
    anyhow::ensure!(again.hash() == hash, "replay diverged");
    println!("replay hash {hash:016x} agrees");
    Ok(())
}
