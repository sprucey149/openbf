//! One hover on a real map, driven by a scripted pad (card HOVR, headless):
//! `cargo run -p game --release --example hover_drive -- [map] [class] [pad|-] [yaw°]`
//! (defaults `tat1 tat_hover_skiff`, the first pad naming the class, yaw 0 —
//! `Vehicle::rot` = quat_y(−yaw), so yaw −90 faces +x).
//!
//! The class's numbers come from the map's `classes.ron` through
//! `game::vehicle_class`; the vehicle stands at the pad on the soldier's
//! ground probe, as `Sim::tick_vehicle_pads` puts it, in a `Sim` with the
//! map's heightfield and collision world (the `con` layer and
//! `common_<game>` merged, as the sweep builds it), a player seated at its
//! pilot seat (since VSEAT the sim zeroes the input of a vehicle no human
//! drives, so the pad is the only way in). Then 20 s of that player's pad:
//! 0-5 s nothing (it settles), 5-10 s full throttle, 10-15 s throttle and
//! full turn, 15-20 s released. A pad driver's turn blends from PitchRate
//! over the first second of full stick (§6.10), so 12-14 s turns at the
//! class rate. Printed: the ride height over the ground
//! probe, the time to ForwardSpeed, the turn rate and circle, the coast-down,
//! the deepest spring-sphere penetration into a building beyond its spring
//! length, the impacts, and whether two runs hash equal.
//!
//! The class is the map's own or a side's (`classes/<game>_*.ron`). A class
//! with a BoostSpeed or a jump (BF2, card HOVX) runs 10 s more: 20-26 s
//! full throttle with the boost button (the top speed and the
//! energy it leaves), 26-30 s full throttle, the jump button held 27.0-27.4 s
//! (the highest the body rises over its ground-probe rest, and the energy).
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::sim::{buttons, PadState, Sim};
use core::unit::UnitSpec;
use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use game::classdb::ClassDb;
use game::vehicle_class::HoverClass;
use glam::Vec3;
use std::path::PathBuf;

const DT: f32 = 1.0 / 30.0;
const TICKS: u32 = 600;

struct Run {
    hash: u64,
    lines: Vec<String>,
}

fn world(dir: &std::path::Path) -> anyhow::Result<(Sim, game::assets::Scene)> {
    let mut scene = game::assets::load_scene(dir)?;
    scene.apply_mode_layer("con");
    let common = dir
        .parent()
        .expect("map dir has a parent")
        .join(format!("common_{}", scene.manifest.source_game));
    if common.join("common.ron").exists() {
        scene.merge_common(game::assets::load_common(&common)?);
    }
    let mut sim = Sim::empty(1);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    Ok((sim, scene))
}

fn run(dir: &std::path::Path, class: &str, pad: Option<&str>, yaw: f32) -> anyhow::Result<Run> {
    let (mut sim, scene) = world(dir)?;
    let mut db = ClassDb::default();
    db.load_map(dir)?;
    let classes = game::assets::baked_root().join("classes");
    let game = scene.manifest.source_game.to_ascii_lowercase();
    for f in game::roster::class_files(&game, &["all", "imp", "rep", "cis"]) {
        if classes.join(f).exists() {
            db.load(&classes.join(f))?;
        }
    }
    let hc = HoverClass::from_db(&db, class)
        .ok_or_else(|| anyhow::anyhow!("{class}: not a hover class in {}", dir.display()))?;
    // Its hull bodies as a battle gives them (card HHUL).
    let p = core::vehicle::hover::HoverParams {
        hull: game::vehicle_pads::hover_hull(&scene.manifest, dir, &game, class, &hc),
        ..hc.params()
    };
    let inst = scene
        .manifest
        .instances
        .iter()
        .filter(|i| i.class == game::vehicle_pads::PAD_CLASS)
        .find(|i| match pad {
            Some(n) => i.name.eq_ignore_ascii_case(n),
            None => i
                .props
                .iter()
                .any(|(_, v)| v.trim().eq_ignore_ascii_case(class)),
        })
        .ok_or_else(|| anyhow::anyhow!("no pad for {class}"))?;
    let at = Vec3::from(inst.translation);
    let g = sim.ground_under(at, false);
    sim.hover_params = vec![Some(p.clone())];
    let h = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(at.x, g, at.z),
        yaw: yaw.to_radians(),
        max_health: db.prop_f32(class, "MaxHealth").unwrap_or(1000.0),
        health_type: game::vehicle_class::health_type(&db, class),
        seats: vec![SeatKind::Pilot],
    });
    let driver = sim
        .spawn(&UnitSpec::player(0, Vec3::new(at.x, g, at.z)))
        .ok_or_else(|| anyhow::anyhow!("no player"))?;
    anyhow::ensure!(
        sim.seat_unit(driver, h, 0),
        "the player did not take the seat"
    );
    let mut lines = vec![format!(
        "{class} at pad {} ({:.1}, {:.1}, {:.1}), ground {g:.2}; {} springs, {} hull bodies",
        inst.name,
        at.x,
        at.y,
        at.z,
        p.springs.len(),
        p.hull.len()
    )];

    let (mut rest, mut rest_lo, mut rest_hi) = (0.0f32, f32::MAX, f32::MIN);
    let mut reach: Option<f32> = None;
    let (mut top, mut deepest, mut hits) = (0.0f32, 0.0f32, 0usize);
    let mut turn_start = None;
    let mut coast_start = 0.0;
    let heading = |f: Vec3| core::fmath::atan2(f.x, f.z);
    let extra = p.boost_speed > 0.0 || p.jump.force > 0.0;
    let ticks = if extra { TICKS + 300 } else { TICKS };
    let (mut boost_top, mut boost_energy, mut jump_rise) = (0.0f32, 0.0f32, 0.0f32);
    for t in 0..ticks {
        let secs = t as f32 * DT;
        let mut pad = PadState::default();
        if (5.0..15.0).contains(&secs) || secs >= 20.0 {
            pad.ly = i16::MAX;
        }
        if (10.0..15.0).contains(&secs) {
            pad.rx = -i16::MAX;
        }
        if (20.0..26.0).contains(&secs) {
            pad.buttons |= buttons::SPRINT;
        }
        if (27.0..27.4).contains(&secs) {
            pad.buttons |= buttons::JUMP;
        }
        sim.events.clear();
        sim.step(&[pad]);
        hits += sim
            .events
            .iter()
            .filter(|e| matches!(e, core::sim::Event::VehicleHit { .. }))
            .count();
        let Some(v) = sim.vehicle(h).filter(|v| v.alive) else {
            lines.push(format!(
                "destroyed at {:.2} s ({hits} impact events)",
                secs + DT
            ));
            break;
        };
        let (pos, rot, vel) = (v.pos, v.rot, v.vel);
        let fwd = rot * Vec3::Z;
        let above = pos.y - sim.ground_under(pos, false);
        if (3.0..5.0).contains(&secs) {
            rest += above / 60.0;
            rest_lo = rest_lo.min(above);
            rest_hi = rest_hi.max(above);
        }
        if secs >= 5.0 && reach.is_none() && fwd.dot(vel) >= p.forward_speed - 0.05 {
            reach = Some(secs + DT - 5.0);
        }
        if secs < 20.0 {
            top = top.max(vel.length());
        } else if secs < 26.0 {
            boost_top = boost_top.max(fwd.dot(vel));
            boost_energy = v.hover.energy;
        } else if secs >= 27.0 {
            jump_rise = jump_rise.max(above - rest);
        }
        if (secs - 12.0).abs() < DT * 0.5 {
            turn_start = Some((heading(fwd), vel.length()));
        }
        if (secs - 14.0).abs() < DT * 0.5 {
            let (h0, s0) = turn_start.expect("12 s");
            let mut d = heading(fwd) - h0;
            while d < 0.0 {
                d += std::f32::consts::TAU;
            }
            let rate = d / 2.0;
            lines.push(format!(
                "turn 12-14 s: {rate:.3} rad/s at {s0:.2}-{:.2} m/s, circle radius {:.1} m",
                vel.length(),
                vel.length() / rate
            ));
        }
        if (secs - 15.0).abs() < DT * 0.5 {
            coast_start = vel.length();
        }
        if (secs - 16.0).abs() < DT * 0.5 {
            lines.push(format!(
                "coast 15-16 s: {coast_start:.2} -> {:.2} m/s",
                vel.length()
            ));
        }
        let e = glam::Mat3::from_quat(rot);
        for s in &p.springs {
            let mut out = Vec::new();
            sim.collision.sphere_contacts(
                pos + e * s.pos,
                s.radius,
                core::collision::Layers::SOLDIER,
                &mut out,
            );
            for (d, n) in out {
                if (d - s.length > 0.3 || n.y < 0.5) && std::env::var_os("HOVER_DEBUG").is_some() {
                    eprintln!(
                        "t {secs:.2} spring {:?} depth {d:.3} n {n:.3} pos {pos:.2} vel {vel:.2}",
                        s.pos
                    );
                }
                deepest = deepest.max(d - s.length);
            }
        }
        if std::env::var_os("HOVER_DEBUG").is_some() && t % 15 == 0 {
            let e = glam::Mat3::from_quat(rot);
            let sp: Vec<String> = p
                .springs
                .iter()
                .map(|s| {
                    let c = pos + e * s.pos;
                    format!(
                        "c.y-h {:.2} r {:.2}",
                        c.y - sim.ground_under(c, false),
                        s.radius
                    )
                })
                .collect();
            eprintln!("t {secs:.2} g {:.3} {}", v.hover.g, sp.join(" | "));
        }
        if t % 60 == 59 {
            lines.push(format!(
                "t {:5.2} s  pos ({:8.2}, {:7.2}, {:8.2})  above {above:5.2}  speed {:5.2}  up.y {:.4}  w.yaw {:+.3}",
                secs + DT,
                pos.x,
                pos.y,
                pos.z,
                vel.length(),
                (rot * Vec3::Y).y,
                v.hover.omega.y
            ));
        }
    }
    lines.push(format!(
        "rest 3-5 s: {rest:.3} m over the ground (min {rest_lo:.3}, max {rest_hi:.3})"
    ));
    lines.push(format!(
        "0 -> {} m/s: {} ; top {top:.2} m/s",
        p.forward_speed,
        reach.map_or("never".to_owned(), |s| format!("{s:.2} s"))
    ));
    lines.push(format!(
        "deepest spring sphere in a building past its length: {deepest:.3} m; impact events {hits}"
    ));
    if let Some(v) = sim.vehicle(h).filter(|v| v.alive && extra) {
        lines.push(format!(
            "boost 20-26 s (BoostSpeed {}): top forward speed {boost_top:.2} m/s, energy left {boost_energy:.1}",
            p.boost_speed
        ));
        lines.push(format!(
            "jump 27-30 s (JumpForce {}): rose {jump_rise:.2} m over the rest height; energy at 30 s {:.1}",
            p.jump.force, v.hover.energy
        ));
    }
    Ok(Run {
        hash: sim.hash(),
        lines,
    })
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let map = args.first().map_or("tat1", String::as_str);
    let class = args.get(1).map_or("tat_hover_skiff", String::as_str);
    let pad = args.get(2).map(String::as_str).filter(|p| *p != "-");
    let yaw: f32 = args.get(3).map_or(Ok(0.0), |y| y.parse())?;
    let dir: PathBuf = game::assets::baked_root().join(map);
    let a = run(&dir, class, pad, yaw)?;
    let b = run(&dir, class, pad, yaw)?;
    for l in &a.lines {
        println!("{l}");
    }
    println!(
        "hash {:016x} / {:016x}: {}",
        a.hash,
        b.hash,
        if a.hash == b.hash { "EQUAL" } else { "DIFFER" }
    );
    Ok(())
}
