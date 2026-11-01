//! Headless PS2-class flight through the ordinary player pad and Sim::step.
//! cargo run -p game --example flyer_drive -- [tat1] [all_fly_xwing]
//! Optional third argument supplies an additional baked PS2 class RON.
//! No window, renderer or shared bake writes. Reports a deterministic trace;
//! this is a port check, not a live PS2 parity measurement.

use anyhow::{ensure, Context, Result};
use core::sim::{buttons, PadState, Sim};
use core::unit::UnitSpec;
use core::vehicle::flyer::FlightPhase;
use core::vehicle::{SeatKind, VehicleHandle, VehicleKind, VehicleSpec};
use game::classdb::ClassDb;
use game::vehicle_class::FlyerClass;
use glam::Vec3;
use std::path::Path;

fn build(map: &Path, class: &str, extra: Option<&Path>) -> Result<(Sim, VehicleHandle)> {
    let mut scene = game::assets::load_scene(map)?;
    scene.apply_mode_layer("con");
    let source = scene.manifest.source_game.to_ascii_lowercase();
    let common = map
        .parent()
        .context("map parent")?
        .join(format!("common_{source}"));
    if common.join("common.ron").exists() {
        scene.merge_common(game::assets::load_common(&common)?);
    }
    let mut db = ClassDb::default();
    if let Some(extra) = extra {
        db.load(extra)?;
    }
    db.load_map(map)?;
    let classes = game::assets::baked_root().join("classes");
    for name in game::roster::class_files(&source, &["all", "imp", "rep", "cis"]) {
        db.load(&classes.join(name))?;
    }
    let flight =
        FlyerClass::from_db(&db, class, source == "bf2").context("class is not a loaded flyer")?;
    let pad = scene
        .manifest
        .instances
        .iter()
        .find(|i| {
            i.class == game::vehicle_pads::PAD_CLASS
                && i.props.iter().any(|(_, v)| v.eq_ignore_ascii_case(class))
        })
        .context("no authored pad for this class on the selected map")?;
    let mut sim = Sim::empty(42);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let point = Vec3::from(pad.translation);
    let surface = sim.ground_under(point, false);
    ensure!(surface > core::collision::NO_GROUND, "pad has no floor");
    let floors = game::vehicle_pads::vis_floors(map, &source);
    ensure!(
        db.prop_f32(class, "LandedHeight").is_some() || floors.contains_key(class),
        "probe needs authored LandedHeight or the baked model's rest bounds"
    );
    let height = game::vehicle_class::landed_height(&db, class, |k| floors.get(k).copied())
        .context("flyer rest height")?;
    let params = flight.params(height);
    println!("{source} {class}: speed {:.3}/{:.3}/{:.3}, acceleration {:.3}, rest {:.3}, takeoff {:.3}m/{:.3}s/{:.3}mps",
        params.min_speed, params.mid_speed, params.max_speed, params.acceleration,
        height, params.takeoff_height, params.takeoff_time, params.takeoff_speed);
    sim.flyer_params = vec![Some(params)];
    let at = Vec3::new(point.x, surface + height, point.z);
    let handle = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: true,
        spawn_pad: None,
        pos: at,
        yaw: core::fmath::atan2(pad.rotation[0][2], pad.rotation[0][0]),
        max_health: db.prop_f32(class, "MaxHealth").context("MaxHealth")?,
        health_type: game::vehicle_class::health_type(&db, class),
        seats: vec![SeatKind::Pilot],
    });
    let pilot = sim.spawn(&UnitSpec::player(0, at)).context("pilot spawn")?;
    ensure!(sim.seat_unit(pilot, handle, 0), "pilot boarding");
    Ok((sim, handle))
}

fn run(map: &Path, class: &str, extra: Option<&Path>, print: bool) -> Result<u64> {
    let (mut sim, handle) = build(map, class, extra)?;
    let origin = sim.vehicle(handle).context("spawned flyer")?.pos;
    let mut flown = false;
    for tick in 0..600 {
        let mut pad = PadState::default();
        if tick == 30 {
            pad.buttons = buttons::JUMP;
        }
        if tick >= 300 {
            pad.ly = i16::MAX;
        }
        if (420..480).contains(&tick) {
            pad.rx = i16::MAX / 3;
        }
        if (480..510).contains(&tick) {
            pad.ry = i16::MAX / 4;
        }
        sim.step(&[pad]);
        let v = sim
            .vehicle(handle)
            .context("flyer destroyed during scripted flight")?;
        ensure!(
            v.pos.is_finite() && v.vel.is_finite() && v.rot.is_finite(),
            "nonfinite flight state"
        );
        flown |= v.flyer.phase == FlightPhase::Flying;
        if print && tick % 30 == 29 {
            println!(
                "tick {} {:?} lift {:.4} pos {:.3},{:.3},{:.3} speed {:.3} hash {:016x}",
                tick + 1,
                v.flyer.phase,
                v.flyer.lift,
                v.pos.x,
                v.pos.y,
                v.pos.z,
                v.vel.length(),
                sim.hash()
            );
        }
    }
    ensure!(flown, "never reached Flying");
    ensure!(
        sim.vehicle(handle).unwrap().pos.distance(origin) > 50.0,
        "did not leave the pad"
    );
    Ok(sim.hash())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let map = game::assets::baked_root().join(args.first().map_or("tat1", String::as_str));
    let class = args.get(1).map_or("all_fly_xwing", String::as_str);
    let extra = args.get(2).map(Path::new);
    let first = run(&map, class, extra, true)?;
    let second = run(&map, class, extra, false)?;
    ensure!(first == second, "identical input produced different hashes");
    println!("flight replay identical: {first:016x}");
    Ok(())
}
