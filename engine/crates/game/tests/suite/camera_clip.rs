//! The chase camera against the world (`game::camera_collide`), playtest
//! 2026-10-28: "standing near these things to heal up the camera would do
//! weird things like go to the floor looking at my feet". On baked BF1
//! battles (Yavin 4: Arena for the healing droid, Naboo for walls, props and
//! a vehicle) the player is stood with his back to each kind of thing and
//! the camera swung behind it:
//! - a small prop (a health droid, a gun emplacement) or a vehicle never
//!   pulls the camera: it stays at its normal distance, height and view, and
//!   the thing is drawn see-through (hidden with the camera inside it);
//! - a big wall pulls the camera in toward the PS2's 1.5 m point, never
//!   behind the wall, with its view and at least that height (never at the
//!   floor), and his body is hidden when it comes into him; held still, it
//!   does not jitter.
//!
//! Data-gated: SKIP without the bake.
use core::collision::{HullFilter, Layers};
use core::sim::{buttons, PadState, Sim, SpawnStage};
use game::assets::Scene;
use game::camera_collide::{closest_on_triangle, BODY_HIDE_DIST, CAMERA_RADIUS};
use game::walk::{ClassVisual, TeamTemplate, WalkSim, CAMERA_PULL_Y, CAMERA_SIGHT_BODY_Y};
use glam::{Mat4, Vec3};

type Tri = [Vec3; 3];

/// A BF1 conquest battle on `map` with the player spawned.
fn battle(map: &str) -> Option<(WalkSim, Scene)> {
    let root = game::assets::baked_root();
    let dir = root.join(map);
    if !dir.join("manifest.ron").exists() {
        eprintln!("SKIP: camera clip needs baked {map}");
        return None;
    }
    let (scene, _) = game::assets::load_battle_scene(&dir, "con").unwrap();
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .unwrap()
            .join(format!("missions/{map}_con.ron")),
    )
    .unwrap();
    let factions = mission.factions();
    let sides = [factions[0].as_str(), factions[1].as_str()];
    let (rosters, _) =
        game::roster::FactionRoster::pair(&root.join("classes"), "bf1", sides, Some(&mission))
            .unwrap();
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
    // The pads read the posts' owners (`main/battle.rs`'s order).
    game::vehicle_pads::install(
        &mut walk.sim,
        &scene.manifest,
        &scene.dir,
        &root.join("classes"),
        &factions,
    )
    .unwrap();
    assert!(walk.sim.open_spawn_screens(0, false));
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
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Accepted);
    Some((walk, scene))
}

/// The first instance of `class` that has a drawn model.
fn instance(scene: &Scene, class: &str) -> usize {
    scene
        .manifest
        .instances
        .iter()
        .position(|i| {
            i.class.eq_ignore_ascii_case(class) && scene.resolve_model(&i.class).is_some()
        })
        .unwrap_or_else(|| panic!("no drawn {class}"))
}

/// Instance `ii`'s drawn triangles at `pose` (its placed matrix when None).
fn tris(scene: &Scene, ii: usize, pose: Option<Mat4>) -> Vec<Tri> {
    let inst = &scene.manifest.instances[ii];
    let m = pose.unwrap_or_else(|| game::viewer::instance_matrix(inst.rotation, inst.translation));
    let mesh = &scene.meshes[scene.resolve_model(&inst.class).unwrap()];
    let p: Vec<Vec3> = mesh
        .positions
        .iter()
        .map(|p| m.transform_point3(Vec3::from(*p)))
        .collect();
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t.iter().all(|&i| (i as usize) < p.len()))
        .map(|t| t.map(|i| p[i as usize]))
        .collect()
}

/// The live pose of the droid at manifest instance `ii`.
fn droid_pose(walk: &WalkSim, ii: usize) -> Mat4 {
    walk.sim
        .stations
        .iter()
        .find(|s| s.instance == Some(ii as u32))
        .expect("a sim station at the droid's instance")
        .pose()
}

fn clearance(p: Vec3, tris: &[Tri]) -> f32 {
    tris.iter()
        .map(|[a, b, c]| closest_on_triangle(p, *a, *b, *c).distance(p))
        .fold(f32::MAX, f32::min)
}

/// Does the segment `a -> b` pass through a triangle?
fn crosses(a: Vec3, b: Vec3, tris: &[Tri]) -> bool {
    let (d, len) = ((b - a).normalize_or_zero(), (b - a).length());
    tris.iter().any(|[p, q, r]| {
        let (e1, e2) = (*q - *p, *r - *p);
        let h = d.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-9 {
            return false;
        }
        let s = a - *p;
        let u = s.dot(h) / det;
        let qv = s.cross(e1);
        let v = d.dot(qv) / det;
        let t = e2.dot(qv) / det;
        (0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && t > 0.0 && t < len
    })
}

/// Put the player at `feet` facing along `facing` (horizontal) with `pitch`,
/// forget the camera's stored pull, and return the drawn camera.
fn stand(walk: &mut WalkSim, feet: Vec3, facing: Vec3, pitch: f32) -> render::Camera {
    let p = walk.player;
    let u = walk.sim.unit_mut(p).unwrap();
    u.soldier.pos = feet;
    // forward = (-sin yaw, 0, -cos yaw).
    u.soldier.yaw = (-facing.x).atan2(-facing.z);
    u.soldier.pitch = pitch;
    walk.cam_pull = 1.0;
    walk.camera()
}

/// The player's ideal (unpulled) camera now: its position and pitch.
fn ideal(walk: &WalkSim) -> (Vec3, f32) {
    let me = walk.sim.unit(walk.player).unwrap();
    let c = me.ideal_camera(walk.sim.ground_rules, walk.sim.views());
    let pitch = c.forward.y.atan2(c.forward.with_y(0.0).length());
    (c.position, pitch)
}

/// The track centre the camera is pulled toward: the eye point.
fn centre(walk: &WalkSim) -> Vec3 {
    let me = walk.sim.unit(walk.player).unwrap();
    me.soldier.pos + Vec3::Y * me.section().eye_y
}

/// The point the camera is pulled in toward: the PS2's 1.5 m point.
fn pull(walk: &WalkSim) -> Vec3 {
    walk.soldier().pos + Vec3::Y * CAMERA_PULL_Y
}

/// Does no big world model stand between the pull point and the ideal
/// camera?
fn walls_clear(walk: &WalkSim) -> bool {
    let (want, _) = ideal(walk);
    walk.camera_blockers
        .fraction(pull(walk), want, None, |_| None, &walk.sim.doors)
        >= 1.0
}

/// A bearing's unit vector (12 around).
fn bearing(k: usize) -> Vec3 {
    let a = std::f32::consts::TAU * k as f32 / 12.0;
    Vec3::new(a.cos(), 0.0, a.sin())
}

/// `out` turned by `turn` radians about +Y.
fn turned(out: Vec3, turn: f32) -> Vec3 {
    Vec3::new(
        out.x * turn.cos() - out.z * turn.sin(),
        0.0,
        out.x * turn.sin() + out.z * turn.cos(),
    )
}

/// The reported case, on BF1 Yavin 4: Arena: the player heals with his back
/// against a health droid, the camera behind him in and behind the droid.
/// The droid never pulls the camera in: it stays at its normal distance,
/// above his head, looking where he looks (never at the floor), and the
/// droid is drawn see-through - hidden while the camera is inside it. Held
/// still for 2 s while the droid turns, nothing changes.
#[test]
fn the_healing_droid_never_pulls_the_camera_and_is_seen_through() {
    let Some((mut walk, scene)) = battle("bf1_yav2") else {
        return;
    };
    let ii = instance(&scene, "com_item_healthrecharge");
    assert_eq!(walk.camera_blockers.is_prop(ii as u32), Some(true));
    let c = Vec3::from(scene.manifest.instances[ii].translation);
    let s = walk
        .sim
        .stations
        .iter()
        .position(|s| s.instance == Some(ii as u32))
        .expect("the droid's station");
    let (mut checked, mut inside) = (0, 0);
    let mut held = None;
    for yaw in [0.0f32, 0.9, 2.3] {
        walk.sim.stations[s].motion.yaw = yaw;
        for k in 0..12 {
            let out = bearing(k);
            let solid = tris(&scene, ii, Some(droid_pose(&walk, ii)));
            let reach = solid
                .iter()
                .flatten()
                .map(|p| (*p - c).dot(out))
                .fold(0.0f32, f32::max);
            for d in [0.3f32, 0.6, 1.0, 1.6, 2.2] {
                for turn in [-0.4f32, 0.0, 0.4] {
                    for pitch in [-0.4f32, -0.2, core::unit::SPAWN_PITCH, 0.0, 0.25, 0.45] {
                        let feet = c + out * (reach + d);
                        let cam = stand(&mut walk, feet, turned(out, turn), pitch);
                        let (want, want_pitch) = ideal(&walk);
                        let eye = centre(&walk);
                        let in_the_way = clearance(want, &solid) < CAMERA_RADIUS
                            || crosses(eye, want, &solid)
                            || crosses(feet + Vec3::Y * CAMERA_SIGHT_BODY_Y, want, &solid);
                        if !in_the_way || !walls_clear(&walk) {
                            continue;
                        }
                        checked += 1;
                        let at = format!(
                            "droid yaw {yaw}, bearing {k}, {d} m out, turn {turn}, pitch \
                             {pitch}: feet {feet:.2?} camera {:.2?} ideal {want:.2?}",
                            cam.position
                        );
                        assert!(
                            (cam.position.distance(eye) - want.distance(eye)).abs() < 0.01,
                            "{at}: the droid pulled the camera in"
                        );
                        // Aiming up, the ideal camera itself drops under his
                        // head (the PS2's own): its view is unchanged.
                        if pitch <= 0.0 {
                            assert!(cam.position.y > feet.y + 2.1, "{at}: under his head");
                        }
                        assert!(
                            (cam.pitch - want_pitch).abs() < 1e-4,
                            "{at}: the view tilts"
                        );
                        assert!(
                            (cam.yaw - walk.soldier().yaw).abs() < 1e-5,
                            "{at}: the view turns"
                        );
                        assert!(!walk.player_body_hidden(), "{at}: his body is hidden");
                        let see = walk.camera_see_through();
                        let got = see.props.iter().find(|(i, _)| *i == ii as u32);
                        assert!(got.is_some(), "{at}: the droid is drawn whole: {see:?}");
                        if clearance(cam.position, &solid) < CAMERA_RADIUS {
                            inside += 1;
                            assert_eq!(got.unwrap().1, 0.0, "{at}: inside it, it shows");
                        }
                        if held.is_none()
                            && d == 0.3
                            && turn == 0.0
                            && pitch == core::unit::SPAWN_PITCH
                        {
                            held = Some((feet, out, pitch, cam.position));
                        }
                    }
                }
            }
        }
    }
    eprintln!("health droid: {checked} placements in the way, {inside} with the camera in it");
    assert!(checked > 50 && inside > 10, "{checked} / {inside}");

    // Healing, held still against it for 2 s while the droid turns.
    let (feet, out, pitch, first) = held.expect("a placement against the droid");
    stand(&mut walk, feet, out, pitch);
    for tick in 0..60 {
        walk.step_tick(&[PadState::default()]);
        let u = walk.sim.unit_mut(walk.player).unwrap();
        (u.soldier.pos, u.soldier.pitch) = (feet, pitch);
        let cam = walk.camera();
        assert!(
            cam.position.distance(first) < 0.02,
            "tick {tick}: the camera moved {first:.2?} -> {:.2?}",
            cam.position
        );
        assert!(
            walk.camera_see_through()
                .props
                .iter()
                .any(|(i, _)| *i == ii as u32),
            "tick {tick}: the droid is drawn whole"
        );
    }
}

/// A wall behind the player (Naboo's ruin wall, Gungan head bunker and the
/// tall ruin column, all big): the camera is pulled in, never behind the wall
/// and clear of it by the near plane; it keeps its pitch and view, stays at
/// the 1.5 m pull point's height or above (never down at the floor), and his
/// body is hidden once it comes into him.
#[test]
fn a_wall_behind_the_player_pulls_the_camera_in_keeping_height_and_pitch() {
    let Some((mut walk, scene)) = battle("bf1_nab1") else {
        return;
    };
    for class in [
        "nab1_prop_ruins1",
        "nab1_prop_ruins_column1",
        "nab1_prop_gunganhead_bunker",
    ] {
        let ii = instance(&scene, class);
        assert_eq!(
            walk.camera_blockers.is_prop(ii as u32),
            Some(false),
            "{class}"
        );
        let c = Vec3::from(scene.manifest.instances[ii].translation);
        let floor = walk
            .sim
            .heightfield
            .as_ref()
            .map_or(c.y, |h| h.height_at(c.x, c.z));
        let solid = tris(&scene, ii, None);
        let (mut checked, mut pulled) = (0, 0);
        for k in 0..12 {
            let out = bearing(k);
            let reach = solid
                .iter()
                .flatten()
                .map(|p| (*p - c).dot(out))
                .fold(0.0f32, f32::max);
            for d in [0.35f32, 0.6, 1.0, 1.6, 2.4] {
                for turn in [-0.5f32, 0.0, 0.5] {
                    for pitch in [-0.6f32, -0.2, 0.0] {
                        let feet = Vec3::new(c.x, floor, c.z) + out * (reach + d);
                        let cam = stand(&mut walk, feet, turned(out, turn), pitch);
                        let eye = pull(&walk);
                        if clearance(eye, &solid) < CAMERA_RADIUS {
                            continue; // he would stand in it
                        }
                        checked += 1;
                        let (want, want_pitch) = ideal(&walk);
                        let e = cam.position;
                        if e.distance(eye) < want.distance(eye) - 0.05 {
                            pulled += 1;
                        }
                        let at = format!(
                            "{class}: bearing {k}, {d} m out, turn {turn}, pitch {pitch}: \
                             feet {feet:.2?} camera {e:.2?} ideal {want:.2?}"
                        );
                        let gap = clearance(e, &solid);
                        assert!(gap >= cam.near, "{at}: {gap:.3} m from it");
                        assert!(!crosses(eye, e, &solid), "{at}: behind it");
                        assert!(
                            (cam.pitch - want_pitch).abs() < 1e-4,
                            "{at}: the view tilts"
                        );
                        assert!((cam.yaw - walk.soldier().yaw).abs() < 1e-5, "{at}: turned");
                        assert!(e.y >= eye.y - 1e-3, "{at}: under the 1.5 m point");
                        assert!(
                            e.distance(eye) <= want.distance(eye) + 1e-3,
                            "{at}: pushed out"
                        );
                        if e.distance(eye) < BODY_HIDE_DIST {
                            assert!(walk.player_body_hidden(), "{at}: the camera is in his body");
                        }
                    }
                }
            }
        }
        eprintln!("{class}: {checked} placements, {pulled} pulled in");
        assert!(
            checked > 100 && pulled > 20,
            "{class}: {checked} / {pulled}"
        );
    }
}

/// Issue #71: a wall behind a vehicle pulls its camera in toward the PS2's pull point
/// on the vehicle (`walk::vehicle_pull_height`), and the camera never ends behind the
/// wall or further out than it wants to be.
#[test]
fn a_wall_behind_a_vehicle_pulls_its_camera_in_toward_the_pull_point() {
    let Some((mut walk, scene)) = battle("bf1_nab1") else {
        return;
    };
    let top = 2.5;
    let height = game::walk::vehicle_pull_height(core::vehicle::VehicleKind::Hover, Some(top));
    walk.vehicle_tops = vec![Some(top); 1];
    let h = walk.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
        kind: core::vehicle::VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![core::vehicle::SeatKind::Pilot],
    });
    let player = walk.player;
    assert!(walk.sim.seat_unit(player, h, 0));
    let ii = instance(&scene, "nab1_prop_ruins1");
    let c = Vec3::from(scene.manifest.instances[ii].translation);
    let floor = walk
        .sim
        .heightfield
        .as_ref()
        .map_or(c.y, |h| h.height_at(c.x, c.z));
    let solid = tris(&scene, ii, None);
    let (mut checked, mut pulled) = (0, 0);
    for k in 0..12 {
        let out = bearing(k);
        let reach = solid
            .iter()
            .flatten()
            .map(|p| (*p - c).dot(out))
            .fold(0.0f32, f32::max);
        for d in [1.0f32, 2.0, 4.0, 6.0] {
            for yaw in 0..8 {
                let pos = Vec3::new(c.x, floor, c.z) + out * (reach + d);
                let v = walk.sim.vehicle_mut(h).unwrap();
                v.pos = pos;
                v.rot = glam::Quat::from_rotation_y(yaw as f32 * std::f32::consts::FRAC_PI_4);
                let origin = pos + v.rot * (Vec3::Y * height);
                let v = walk.sim.vehicle(h).unwrap();
                let (_, want) = game::walk::vehicle_camera(v, None, None, 1.0, Vec3::Y * height);
                walk.cam_pull = 1.0;
                let e = walk.camera().position;
                if clearance(origin, &solid) < CAMERA_RADIUS {
                    continue;
                }
                checked += 1;
                let at = format!("bearing {k}, {d} m out, yaw {yaw}: camera {e:.2?}");
                assert!(!crosses(origin, e, &solid), "{at}: behind the wall");
                let (got, whole) = (e.distance(origin), want.position.distance(origin));
                assert!(got <= whole + 0.35, "{at}: {got} of {whole}");
                pulled += usize::from(got < whole - 0.05);
            }
        }
    }
    eprintln!("{checked} placements, {pulled} pulled in");
    assert!(checked > 100 && pulled > 20, "{checked} / {pulled}");
}

/// A flyer banked beside a wall leans its tall model's pull point at the wall. The
/// camera stays on the flyer's side of it.
#[test]
fn a_flyer_banked_beside_a_wall_keeps_its_camera_on_its_side() {
    let Some((mut walk, scene)) = battle("bf1_nab1") else {
        return;
    };
    walk.vehicle_tops = vec![Some(8.0); 1];
    let h = walk.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
        kind: core::vehicle::VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![core::vehicle::SeatKind::Pilot],
    });
    let player = walk.player;
    assert!(walk.sim.seat_unit(player, h, 0));
    let ii = instance(&scene, "nab1_prop_ruins1");
    let c = Vec3::from(scene.manifest.instances[ii].translation);
    let floor = walk
        .sim
        .heightfield
        .as_ref()
        .map_or(c.y, |h| h.height_at(c.x, c.z));
    let solid = tris(&scene, ii, None);
    let (mut checked, mut wrong) = (0, Vec::new());
    for k in 0..12 {
        let out = bearing(k);
        let reach = solid
            .iter()
            .flatten()
            .map(|p| (*p - c).dot(out))
            .fold(0.0f32, f32::max);
        for d in [1.0f32, 2.0, 4.0] {
            for yaw in 0..8 {
                for roll in [-1.2f32, -0.8, 0.8, 1.2] {
                    let pos = Vec3::new(c.x, floor + 2.0, c.z) + out * (reach + d);
                    let turn =
                        glam::Quat::from_rotation_y(yaw as f32 * std::f32::consts::FRAC_PI_4);
                    let v = walk.sim.vehicle_mut(h).unwrap();
                    v.pos = pos;
                    v.rot = turn * glam::Quat::from_rotation_z(roll);
                    let low = pos + v.rot * (Vec3::Y * game::walk::CAMERA_PULL_Y);
                    if clearance(low, &solid) < CAMERA_RADIUS {
                        continue;
                    }
                    walk.cam_pull = 1.0;
                    let e = walk.camera().position;
                    checked += 1;
                    if crosses(low, e, &solid) {
                        wrong.push(format!(
                            "bearing {k}, {d} m, yaw {yaw}, roll {roll}: {e:.2?}"
                        ));
                    }
                }
            }
        }
    }
    assert!(checked > 100, "{checked}");
    // The rule point's stop is a straight sweep and so is the camera's from there, so
    // around a jagged edge the two can bend past a piece the line from the hull meets.
    // Without the stop this is 74 of 1152.
    assert!(
        wrong.len() * 100 <= checked,
        "{} of {checked}: {wrong:#?}",
        wrong.len()
    );
}

/// A gun emplacement (2.6 m across, a prop) between the camera and the
/// player: it never pulls the camera in, and is drawn see-through whenever
/// it stands in the camera's sight of his eye or body.
#[test]
fn a_prop_between_the_camera_and_the_player_is_seen_through() {
    let Some((mut walk, scene)) = battle("bf1_nab1") else {
        return;
    };
    let ii = instance(&scene, "com_weap_gunturret");
    assert_eq!(walk.camera_blockers.is_prop(ii as u32), Some(true));
    let c = Vec3::from(scene.manifest.instances[ii].translation);
    let solid = tris(&scene, ii, None);
    let (mut checked, mut seen) = (0, 0);
    for k in 0..12 {
        let out = bearing(k);
        let reach = solid
            .iter()
            .flatten()
            .map(|p| (*p - c).dot(out))
            .fold(0.0f32, f32::max);
        for d in [0.4f32, 1.0, 1.8] {
            for pitch in [-0.4f32, -0.07, 0.2] {
                let feet = c + out * (reach + d);
                let cam = stand(&mut walk, feet, out, pitch);
                let eye = centre(&walk);
                let body = feet + Vec3::Y * CAMERA_SIGHT_BODY_Y;
                if clearance(eye, &solid) < CAMERA_RADIUS || !walls_clear(&walk) {
                    continue;
                }
                checked += 1;
                let (want, _) = ideal(&walk);
                let at = format!(
                    "bearing {k}, {d} m out, pitch {pitch}: camera {:.2?}",
                    cam.position
                );
                assert!(
                    (cam.position.distance(eye) - want.distance(eye)).abs() < 0.01,
                    "{at}: the emplacement pulled the camera in"
                );
                let e = cam.position;
                let hides = crosses(e, eye, &solid)
                    || crosses(e, body, &solid)
                    || clearance(e, &solid) < CAMERA_RADIUS;
                let see = walk.camera_see_through();
                let got = see.props.iter().any(|(i, _)| *i == ii as u32);
                if hides {
                    seen += 1;
                    assert!(got, "{at}: it hides him and is drawn whole");
                }
            }
        }
    }
    eprintln!("gun emplacement: {checked} placements, {seen} with it in the way");
    assert!(checked > 50 && seen > 20, "{checked} / {seen}");
}

/// A parked vehicle: never pulls the camera in (ours: the PS2's camera mask
/// has no vehicle layer either), and is drawn see-through while its hull
/// stands between the camera and the player.
#[test]
fn a_vehicle_is_seen_through_and_never_pulls() {
    let Some((mut walk, _scene)) = battle("bf1_nab1") else {
        return;
    };
    // The tallest hull among the pads' classes but a walker's (its legs leave
    // the sight lines clear; a speeder bike hides nothing), spawned on its
    // pad.
    let has_hull = |class: u32| {
        walk.sim
            .hull_parts
            .get(class as usize)
            .is_some_and(|h| h.as_ref().is_some_and(|h| !h.is_empty()))
    };
    let specs: Vec<_> = walk
        .sim
        .vehicle_pads
        .iter()
        .flat_map(|p| {
            p.def
                .classes
                .iter()
                .flatten()
                .filter(|c| {
                    has_hull(c.spec.class) && c.spec.kind != core::vehicle::VehicleKind::Walker
                })
                .map(|c| {
                    let mut spec = c.spec.clone();
                    (spec.pos, spec.yaw) = (p.def.pos, p.def.yaw);
                    spec
                })
        })
        .collect();
    assert!(!specs.is_empty(), "a pad class with a hull on Naboo");
    let mut best: Option<(f32, core::vehicle::VehicleHandle)> = None;
    for spec in &specs {
        let h = walk.sim.spawn_vehicle(spec);
        walk.step_tick(&[PadState::default()]);
        let top = walk
            .sim
            .collision
            .hull(h)
            .iter()
            .map(|b| b.aabb.max.y)
            .fold(f32::MIN, f32::max);
        let tall = top - walk.sim.vehicle(h).expect("spawned").pos.y;
        match best {
            Some((t, _)) if t >= tall => walk.sim.remove_vehicle(h),
            _ => {
                if let Some((_, old)) = best {
                    walk.sim.remove_vehicle(old);
                }
                best = Some((tall, h));
            }
        }
    }
    let (tall, vh) = best.unwrap();
    walk.step_tick(&[PadState::default()]);
    let c = walk.sim.vehicle(vh).expect("spawned").pos;
    eprintln!("vehicle: hull top {tall:.2} m over its origin");
    let hull_hit = |w: &WalkSim, o: Vec3, d: Vec3, len: f32| {
        w.sim
            .collision
            .ray_with(o, d, len, Layers::ORDNANCE, HullFilter::Only(vh))
            .map(|h| h.t)
    };
    let (mut checked, mut seen) = (0, 0);
    for k in 0..12 {
        let out = bearing(k);
        // The hull's reach along this bearing at his middle, from outside in.
        let from = c + Vec3::Y * CAMERA_SIGHT_BODY_Y + out * 20.0;
        let reach = hull_hit(&walk, from, -out, 20.0).map_or(0.0, |t| 20.0 - t);
        for d in [0.4f32, 0.8, 1.4, 2.2, 3.0] {
            for pitch in [-0.6f32, -0.2, 0.0, 0.3] {
                let feet = c + out * (reach + d);
                let cam = stand(&mut walk, feet, out, pitch);
                let eye = centre(&walk);
                if !walls_clear(&walk) {
                    continue;
                }
                checked += 1;
                let (want, _) = ideal(&walk);
                let at = format!(
                    "bearing {k}, {d} m out, pitch {pitch}: camera {:.2?}",
                    cam.position
                );
                assert!(
                    (cam.position.distance(eye) - want.distance(eye)).abs() < 0.01,
                    "{at}: the vehicle pulled the camera in"
                );
                let body = feet + Vec3::Y * CAMERA_SIGHT_BODY_Y;
                let behind = [eye, body].iter().any(|&s| {
                    let to = s - cam.position;
                    hull_hit(&walk, cam.position, to.normalize(), to.length() - 0.5).is_some()
                });
                // The camera inside the hull: it meets the hull every way.
                let inside = [
                    Vec3::X,
                    Vec3::NEG_X,
                    Vec3::Y,
                    Vec3::NEG_Y,
                    Vec3::Z,
                    Vec3::NEG_Z,
                ]
                .iter()
                .all(|d| hull_hit(&walk, cam.position, *d, 30.0).is_some());
                let got = walk
                    .camera_see_through()
                    .vehicles
                    .into_iter()
                    .find(|(h, _)| *h == vh);
                if behind || inside {
                    seen += 1;
                    assert!(got.is_some(), "{at}: the hull hides him and is drawn whole");
                }
                if inside {
                    assert_eq!(got.unwrap().1, 0.0, "{at}: inside the hull, it shows");
                }
            }
        }
    }
    eprintln!(
        "vehicle: {checked} placements, {seen} with the hull in the way or around the camera"
    );
    assert!(checked > 50 && seen > 20, "{checked} / {seen}");
}

/// Held still in front of a still wall that pulls the camera in, the camera
/// does not move at all.
#[test]
fn the_pulled_in_camera_does_not_jitter() {
    let Some((mut walk, scene)) = battle("bf1_nab1") else {
        return;
    };
    let wall = instance(&scene, "nab1_prop_ruins1");
    let w = Vec3::from(scene.manifest.instances[wall].translation);
    let solid = tris(&scene, wall, None);
    let floor = walk
        .sim
        .heightfield
        .as_ref()
        .map_or(w.y, |h| h.height_at(w.x, w.z));
    let mut seen = Vec::new();
    for k in 0..12 {
        let out = bearing(k);
        let reach = solid
            .iter()
            .flatten()
            .map(|p| (*p - w).dot(out))
            .fold(0.0f32, f32::max);
        stand(
            &mut walk,
            Vec3::new(w.x, floor, w.z) + out * (reach + 0.5),
            out,
            -0.2,
        );
        let eye = pull(&walk);
        if clearance(eye, &solid) > CAMERA_RADIUS && crosses(eye, ideal(&walk).0, &solid) {
            for _ in 0..60 {
                walk.step_tick(&[PadState::default()]);
                seen.push(walk.camera().position);
            }
            break;
        }
    }
    assert!(!seen.is_empty(), "no bearing of the wall blocks the camera");
    let settled = &seen[10..];
    assert!(
        settled.windows(2).all(|p| p[0].distance(p[1]) < 1e-4),
        "a still camera at a still wall moves: {:?}",
        &settled[..4]
    );
}
