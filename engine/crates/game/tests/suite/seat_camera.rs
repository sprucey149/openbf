//! A seat's settled camera stands on the player's side of the terrain sheet
//! (playtest 2026-10-28, BF2 Geonosis: "when I got in a geonosis turret, the
//! camera went underground"). `WalkSim::settle_camera` keeps a camera on the
//! side of the sheet its pull point is on, so the pull point must be on the
//! player's side. It was not, twice:
//!
//! - `tur_bldg_geoturret` authors `EyePointOffset` (0, 2.5, 0), `TrackCenter`
//!   (0, 0, -2) and `TrackOffset` (0, 3.6, 2). The PS2 reads the last two on the
//!   aim frame at the turret node (`analysis/re-turret-draw.md` 4); the port
//!   hung them on the building's origin, so the pull point lay 2 m behind the
//!   origin at its height, under the sheet where the ground rises.
//! - `cis_tread_snailtank` authors `TrackCenter` (0, -12, 1), 12 m below its
//!   origin, with `TrackOffset` (0, 16.5, 14) lifting the camera back out.
//!
//! The pull point is now the section's fixed (0, 1.5, 0), the PS2's camera
//! collision origin (`analysis/re-aim-assist.md` 10.6). Under uncut terrain a
//! seat camera stays under it, as BF2 GEO1's hangar needs:
//! `walk::tests::seat_camera_under_the_sheet_stays_under_it`.

use core::sim::Sim;
use core::vehicle::{VehicleHandle, VehicleKind, VehicleSpec};
use game::mission::MissionDef;
use glam::Vec3;

pub(super) struct Map {
    pub(super) walk: game::walk::WalkSim,
    pub(super) build: game::vehicle_pads::PadBuild,
}

pub(super) fn map(dir: &std::path::Path, factions: &[String; 2]) -> Option<Map> {
    let mut scene = game::assets::load_scene(dir).ok()?;
    scene.apply_mode_layer("con");
    let root = game::assets::baked_root();
    let common = root.join(format!("common_{}", scene.manifest.source_game));
    scene.merge_common(game::assets::load_common(&common).ok()?);
    let mut sim = Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.heightfield = Some(scene.heightfield()?);
    scene.install_collision(&mut sim);
    let build = game::vehicle_pads::install(
        &mut sim,
        &scene.manifest,
        dir,
        &root.join("classes"),
        factions,
    )
    .ok()?;
    let t = game::walk::TeamTemplate::default_trooper();
    let mut walk =
        game::walk::WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, [t.clone(), t], None);
    walk.vehicle_seats = build.seats.clone();
    walk.vehicle_geometry = build.geometry.clone();
    walk.load_walker_views(&scene.manifest, &root);
    Some(Map { walk, build })
}

/// The sheet's height at (x, z), `None` over a cutter's hole.
fn sheet(sim: &Sim, p: Vec3) -> Option<f32> {
    let y = sim.heightfield.as_ref()?.height_at(p.x, p.z);
    (!sim.collision.is_cut(Vec3::new(p.x, y, p.z))).then_some(y)
}

/// The looks the seat can reach: its rest, and for a turn law its limits.
fn looks(walk: &game::walk::WalkSim, ci: usize, seat: usize) -> Vec<(f32, f32)> {
    let gun = walk
        .sim
        .seat_setup
        .get(ci)
        .and_then(Option::as_ref)
        .and_then(|s| s.guns.get(seat))
        .and_then(Option::as_ref);
    let Some(gun) = gun else {
        return vec![(0.0, 0.0)];
    };
    let (rest, _, _) = gun.rest();
    let mut out = vec![rest];
    if let Some(t) = gun.turn {
        for k in 0..8 {
            let yaw = if t.yaw_wrap {
                k as f32 * std::f32::consts::FRAC_PI_4
            } else {
                t.yaw[0] + (t.yaw[1] - t.yaw[0]) * k as f32 / 7.0
            };
            let (pitch_lo, pitch_hi) = if t.pitch_wrap {
                (-1.0, 1.0)
            } else {
                (t.pitch[0], t.pitch[1])
            };
            out.extend([pitch_lo, rest.1, pitch_hi].map(|p| (yaw, p)));
        }
    }
    out
}

/// A seat's camera over its looks: the highest and the lowest it stands over
/// the sheet (negative = under), and the look of the lowest.
fn camera_range(
    walk: &mut game::walk::WalkSim,
    h: VehicleHandle,
    seat: u8,
) -> Option<(f32, f32, (f32, f32))> {
    let player = walk.player;
    if !walk.sim.seat_unit(player, h, seat) {
        return None;
    }
    let ci = walk.sim.vehicle(h)?.class as usize;
    let mut range = None::<(f32, f32, (f32, f32))>;
    for look in looks(walk, ci, usize::from(seat)) {
        if let Some(g) = walk.sim.vehicle_mut(h)?.guns.get_mut(usize::from(seat)) {
            if let Some(g) = g.as_mut() {
                g.look = look;
            }
        }
        let cam = walk.camera();
        let Some(y) = sheet(&walk.sim, cam.position) else {
            continue;
        };
        let over = cam.position.y - y;
        range = Some(match range {
            Some((hi, lo, l)) if over >= lo => (hi.max(over), lo, l),
            Some((hi, _, _)) => (hi.max(over), over, look),
            None => (over, over, look),
        });
    }
    walk.sim.unseat_unit(player);
    range
}

pub(super) fn missions_of(key: &str) -> Vec<MissionDef> {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    ["_con.ron", "_con_cw.ron"]
        .iter()
        .filter_map(|s| MissionDef::load(&dir.join(format!("{key}{s}"))).ok())
        .collect()
}

/// Each building turret and each pad's vehicle on every map of every mission,
/// each seat, with a building turret's look swept over its limits. Standing on
/// the sheet, the camera is above it. At least 5 m under it (a hangar), under it.
/// Before the fix: the geoturret and snailtank rows, 0.30 m under
/// (`CAMERA_TERRAIN_GAP`). Mutation: the vehicle's pull point back on its
/// `TrackCenter` -> the snailtank rows fail.
#[test]
fn every_seat_camera_stands_on_its_vehicles_side_of_the_terrain() {
    let maps = game::assets::discover_maps(&game::assets::baked_root());
    if maps.is_empty() {
        eprintln!("SKIP: no baked maps");
        return;
    }
    let mut rows = std::collections::BTreeSet::new();
    let (mut wrong, mut buried) = (Vec::new(), 0);
    for (key, _, dir) in &maps {
        for mission in missions_of(key) {
            let Some(mut m) = map(dir, &mission.factions()) else {
                continue;
            };
            let built: Vec<VehicleHandle> = m
                .walk
                .sim
                .alive_vehicles()
                .filter(|(_, v)| v.kind == VehicleKind::Turret)
                .map(|(h, _)| h)
                .collect();
            let specs: Vec<_> = m
                .build
                .pads
                .iter()
                .flat_map(|p| {
                    p.classes.iter().flatten().map(|c| VehicleSpec {
                        pos: p.pos,
                        yaw: p.yaw,
                        ..c.spec.clone()
                    })
                })
                .collect();
            let spawned: Vec<_> = specs.iter().map(|s| m.walk.sim.spawn_vehicle(s)).collect();
            for &h in built.iter().chain(&spawned) {
                let v = m.walk.sim.vehicle(h).unwrap().clone();
                let Some(ground) = sheet(&m.walk.sim, v.pos) else {
                    continue;
                };
                let class = &m.build.classes[v.class as usize];
                for seat in 0..v.seats.len() as u8 {
                    let Some((highest, lowest, look)) = camera_range(&mut m.walk, h, seat) else {
                        continue;
                    };
                    rows.insert((key.clone(), class.clone(), seat));
                    if v.pos.y >= ground - 0.5 {
                        if lowest <= 0.0 {
                            wrong.push(format!(
                                "{key} {class} seat {seat} at {}: {:.2} m under at look {look:?}",
                                v.pos, -lowest
                            ));
                        }
                    } else if v.pos.y < ground - 5.0 {
                        buried += 1;
                        if highest >= 0.0 {
                            wrong.push(format!(
                                "{key} {class} seat {seat} at {}: {highest:.2} m over, from under the sheet",
                                v.pos
                            ));
                        }
                    }
                }
            }
            for h in spawned {
                m.walk.sim.remove_vehicle(h);
            }
        }
    }
    let games = |g: &str| rows.iter().filter(|r| r.0.starts_with(g)).count();
    println!(
        "{} bf1 and {} bf2 map/class/seat rows; {buried} seats under the sheet",
        games("bf1_"),
        games("bf2_")
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(games("bf1_") >= 40 && games("bf2_") >= 40, "too few rows");
}

/// The playtest's turret: BF2 GEO1 seed 48, the camera was at (-9.10, -61.76,
/// 76.86), 2 m from the turret at its base's height, 0.30 m under the sheet.
/// Mutation: the turret back on `vehicle_camera` -> the numbers do not match.
#[test]
fn the_playtests_geonosis_turret_camera_stands_up_on_the_aim_frame() {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_geo1");
    let mission = missions_of("bf2_geo1").into_iter().next();
    let Some(mut m) = mission.and_then(|mission| map(&dir, &mission.factions())) else {
        eprintln!("SKIP: bf2_geo1 is not baked");
        return;
    };
    let at = Vec3::new(-7.42, -61.63, 77.93);
    let (h, v) = m
        .walk
        .sim
        .alive_vehicles()
        .find(|(_, v)| v.pos.distance(at) < 1.0)
        .map(|(h, v)| (h, v.clone()))
        .expect("the playtest's turret");
    assert_eq!(m.build.classes[v.class as usize], "tur_bldg_geoturret");
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 0));
    let cam = m.walk.camera();
    let look = m.walk.sim.seat_look(player).unwrap();
    println!(
        "pivot {}, look {:?}: camera {}, {:.2} m over the base",
        look.gun.pivot,
        look.look,
        cam.position,
        cam.position.y - v.pos.y
    );
    // The class's numbers on the aim frame at the node, `TiltValue` 5 deg:
    // M x TrackCenter + R x TrackOffset, R the frame pitched down by the tilt.
    let [_, y, z] = core::vehicle::gun::frame(look.look);
    let (st, ct) = 5f32.to_radians().sin_cos();
    let (fwd, up) = (z * ct - y * st, y * ct + z * st);
    let want = look.gun.pivot + z * -2.0 - fwd * 2.0 + up * 3.6;
    let want = v.pos + v.rot * want;
    assert!(
        cam.position.distance(want) < 0.01,
        "camera at {}, the class's numbers put it at {want}",
        cam.position
    );
    assert!(cam.position.y > v.pos.y + 3.0, "camera at {}", cam.position);
}
