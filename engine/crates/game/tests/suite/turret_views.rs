//! Card VTURZ: a building turret's two views (`analysis/re-turret-draw.md`
//! 4 [O], `game::walk::WalkSim::camera`, `SceneGpu::view_draws`).
//!
//! 1. Third person, always: the gunner's camera turns with the seat's look
//!    (a turn of the stick moves the camera's yaw and pitch by the look's).
//! 2. R3 on a turret weapon with `ZoomMax` > 1: the camera stands at the eye
//!    point looking along the look, and the turret's whole building is left
//!    out of the gunner's own view while another viewer's still draws it;
//!    BF2's turret zooms on the left stick (`ZoomRate`), BF1's does not; the
//!    zoom steps off on a later press, and a pilot change ends it.
//!
//! 3. The zoomed eye is where the PS2's own zoomed camera stood, on its RAM
//!    rows (`lead_tmp/vturd/ps2/{nab1_gty,myg1_zoom}_rows.bin`, the turret's
//!    yaw / pitch and the camera matrix; `lead_tmp/vturz/mk_zoom_pairs.py`).
//!
//! The real-scene cases require bakes (the draw-list case also a GPU), as
//! `tests/suite/turret_pose.rs`. Synthetic input and class-parser controls
//! below run without either prerequisite (VTZT M2/M3).

use core::sim::{buttons, PadState};
use core::vehicle::VehicleHandle;
use glam::{Mat4, Vec3};

struct World {
    scene: game::assets::Scene,
    walk: game::walk::WalkSim,
    build: game::vehicle_pads::PadBuild,
}

/// One map's battle world as `tests/suite/turret_pose.rs` builds it, in a
/// `WalkSim` (its player is slot 0).
fn world(map: &str, factions: [&str; 2]) -> Option<World> {
    let dir = game::assets::baked_root().join(map);
    let mut scene = game::assets::load_scene(&dir).ok()?;
    scene.apply_mode_layer("con");
    let common = dir
        .parent()?
        .join(format!("common_{}", scene.manifest.source_game));
    if common.join("common.ron").exists() {
        scene.merge_common(game::assets::load_common(&common).ok()?);
    }
    let mut sim = core::sim::Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.heightfield = scene.heightfield();
    scene.install_collision(&mut sim);
    let classes = game::assets::baked_root().join("classes");
    let f = [factions[0].to_owned(), factions[1].to_owned()];
    let build = game::vehicle_pads::install(&mut sim, &scene.manifest, &dir, &classes, &f).ok()?;
    let t = game::walk::TeamTemplate::default_trooper();
    let mut walk = game::walk::WalkSim::with_teams(sim, None, Vec3::ZERO, [t.clone(), t], None);
    walk.vehicle_seats = build.seats.clone();
    Some(World { scene, walk, build })
}

/// The class's turrets and its gunner seat.
fn turrets(w: &World, map: &str, class: &str) -> (Vec<VehicleHandle>, u8) {
    let ci = w
        .build
        .classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(class))
        .unwrap_or_else(|| panic!("{map}: no {class} among {:?}", w.build.classes));
    let hs: Vec<VehicleHandle> = w
        .walk
        .sim
        .alive_vehicles()
        .filter(|(_, v)| v.class as usize == ci)
        .map(|(h, _)| h)
        .collect();
    let seat = w.walk.sim.seat_setup[ci]
        .as_ref()
        .and_then(|s| s.guns.iter().position(Option::is_some))
        .expect("a gunner seat with a gun");
    (hs, seat as u8)
}

fn ang(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d)
}

const CASES: [(&str, [&str; 2], &str); 2] = [
    ("bf1_nab1", ["rep", "cis"], "com_weap_gunturret"),
    ("bf2_myg1", ["rep", "cis"], "tur_bldg_recoilless_lg"),
];

/// Acceptance 1. The player seated in a turret holds the stick right and up
/// for one second: the camera's yaw and pitch move by what the seat's
/// look moved (the building does not turn), and both moved.
/// Mutation: the seat camera fed no look -> it fails.
#[test]
fn the_gunners_camera_turns_with_the_look() {
    let mut ran = 0;
    for (map, factions, class) in CASES {
        let Some(mut w) = world(map, factions) else {
            eprintln!("SKIP {map}: not baked");
            continue;
        };
        let (hs, seat) = turrets(&w, map, class);
        let player = w.walk.player;
        assert!(w.walk.sim.seat_unit(player, hs[0], seat));
        w.walk.step_tick(&[PadState::default()]);
        let look = |w: &World| {
            w.walk.sim.vehicle(hs[0]).unwrap().guns[usize::from(seat)]
                .as_ref()
                .unwrap()
                .look
        };
        let (l0, c0) = (look(&w), w.walk.camera());
        for _ in 0..30 {
            w.walk.step_tick(&[PadState {
                rx: PadState::quantize(0.8),
                ry: PadState::quantize(0.6),
                ..Default::default()
            }]);
        }
        let (l1, c1) = (look(&w), w.walk.camera());
        let (dly, dlp) = (ang(l1.0, l0.0), l1.1 - l0.1);
        let (dcy, dcp) = (ang(c1.yaw, c0.yaw), c1.pitch - c0.pitch);
        println!(
            "{map} {class}: look moved ({:.2}, {:.2}) deg, camera ({:.2}, {:.2}) deg",
            dly.to_degrees(),
            dlp.to_degrees(),
            dcy.to_degrees(),
            dcp.to_degrees()
        );
        assert!(
            dly > 0.1 && dlp.abs() > 0.05,
            "{map}: the look did not turn"
        );
        assert!((dcy - dly).abs() < 1e-3, "{map}: camera yaw");
        assert!((dcp - dlp).abs() < 1e-3, "{map}: camera pitch");
        assert!(w.walk.sim.seat_zoom(player).is_none(), "no zoom unpressed");
        ran += 1;
    }
    println!("{ran} of {} cases ran", CASES.len());
}

/// One press of `bit`: a tick held, a tick released.
fn tap(w: &mut World, bit: u32) {
    w.walk.step_tick(&[PadState {
        buttons: bit,
        ..Default::default()
    }]);
    w.walk.step_tick(&[PadState::default()]);
}

/// Acceptance 2. The player in one turret, a bot in another (the second
/// view). Unzoomed both views draw the player's building. After R3: the
/// seat is zoomed at the weapon's `ZoomMin`, the camera stands at the eye
/// looking along the look with a narrower FOV, and the player's view lacks
/// exactly his building's draws while the bot's view has them; the bot's
/// own building stays in the player's view. Further presses step the zoom
/// off; zoomed again, leaving the seat (a pilot change) ends it.
/// Mutations: `hidden_for` keyed on any zoomed gunner, not the viewer -> the
/// bot's view fails; the unseat's clear removed -> the pilot change fails;
/// the seat's press read after the buttons are kept -> no zoom.
#[test]
fn the_zoomed_gunner_does_not_see_his_own_turret() {
    let mut ran = 0;
    for (map, factions, class) in CASES {
        let Some(mut w) = world(map, factions) else {
            eprintln!("SKIP {map}: not baked");
            continue;
        };
        let rigged = w
            .scene
            .manifest
            .classes
            .iter()
            .any(|k| k.name.eq_ignore_ascii_case(class) && k.turret_rig.is_some());
        if !rigged {
            eprintln!("SKIP {map}: {class} has no rig in this bake (older than card VTURD)");
            continue;
        }
        let (hs, seat) = turrets(&w, map, class);
        assert!(hs.len() >= 2, "{map}: {} {class}", hs.len());
        let player = w.walk.player;
        let bot = w
            .walk
            .sim
            .spawn(&core::unit::UnitSpec {
                controller: core::unit::Controller::Ai,
                ..core::unit::UnitSpec::trooper(1, Vec3::new(0.0, 500.0, 0.0))
            })
            .expect("a bot");
        assert!(w.walk.sim.seat_unit(player, hs[0], seat));
        assert!(w.walk.sim.seat_unit(bot, hs[1], seat));
        w.walk.step_tick(&[PadState::default()]);
        let wc = {
            let ci = w.walk.sim.vehicle(hs[0]).unwrap().class as usize;
            let g = w.walk.sim.seat_setup[ci].as_ref().unwrap().guns[usize::from(seat)]
                .clone()
                .unwrap();
            w.walk.sim.weapon_classes[g.weapons[0].class].clone()
        };
        println!(
            "{map} {class}: weapon zoom {} .. {}, ZoomRate {}",
            wc.zoom_min, wc.zoom_max, wc.zoom_rate
        );
        assert!(wc.zoom_max > 1.0, "{map}: the turret's weapon must zoom");

        let mut r = render::Renderer::new_headless(64, 64).expect("headless renderer");
        let mut gpu = game::viewer::upload_scene(&mut r, &w.scene).expect("upload");
        let at = |h: VehicleHandle, w: &World| w.walk.sim.vehicle(h).unwrap().pos;
        // Identify the building before posing, from its manifest placement.
        // Do not take the expected range from TurretDraw::whole: that is
        // the field under test (VTZT M1 drops its first draw).
        let building = |p: Vec3| {
            let inst = w
                .scene
                .manifest
                .instances
                .iter()
                .find(|i| i.class.eq_ignore_ascii_case(class) && Vec3::from(i.translation) == p)
                .expect("the sim turret's manifest instance");
            let model = game::viewer::instance_matrix(inst.rotation, inst.translation);
            building_draws(&gpu.draws, model)
        };
        let mine = building(at(hs[0], &w));
        let theirs = building(at(hs[1], &w));
        gpu.pose_turrets(&w.walk.sim, &w.build.seats);
        assert!(!mine.is_empty() && !theirs.is_empty());
        let all = gpu.draws.len();
        // Does `viewer`'s view hold the draws of `range`? (the view's list
        // is the scene's less one range, so its draws' models line up)
        let sees =
            |gpu: &game::viewer::SceneGpu, w: &World, viewer, range: &std::ops::Range<usize>| {
                let d = gpu.view_draws(&w.walk.sim, viewer);
                let hidden = gpu.hidden_for(&w.walk.sim, viewer);
                assert_view_draws(&d, &gpu.draws, hidden.clone());
                match hidden {
                    None => {
                        assert_eq!(d.len(), all);
                        true
                    }
                    Some(h) => {
                        assert_eq!(d.len(), all - h.len());
                        h.start > range.start || h.end < range.end
                    }
                }
            };
        let fov_plain = w.walk.camera().fov_y;
        assert!(sees(&gpu, &w, player, &mine) && sees(&gpu, &w, bot, &mine));

        // Hold R3 across multiple updates: one press must toggle once.
        for _ in 0..4 {
            w.walk.step_tick(&[PadState {
                buttons: buttons::ZOOM,
                ..Default::default()
            }]);
            assert_eq!(
                w.walk.sim.seat_zoom(player).map(|z| z.value),
                Some(wc.zoom_min)
            );
        }
        w.walk.step_tick(&[PadState::default()]);
        let z = w.walk.sim.seat_zoom(player).expect("R3 zooms the turret");
        assert_eq!((z.step, z.value), (0, wc.zoom_min));
        assert!(z.value > 1.0);
        for _ in 0..60 {
            w.walk.step_tick(&[PadState::default()]);
        }
        let look = w.walk.sim.seat_look(player).unwrap();
        let cam = w.walk.camera();
        println!(
            "{map}: zoomed camera at {:?} (eye {:?}), fov {:.2} -> {:.2} deg; my building {} draws of {all}",
            cam.position,
            look.scope_eye,
            fov_plain.to_degrees(),
            cam.fov_y.to_degrees(),
            mine.len()
        );
        assert!(
            cam.position.distance(look.scope_eye) < 1e-4,
            "{map}: at the eye"
        );
        assert!(
            look.eye.distance(look.scope_eye) < 1e-4,
            "{map}: the shot's ray leaves it"
        );
        assert!(
            cam.forward().dot(look.view) > 0.99999,
            "{map}: along the look"
        );
        // The PS2's: vertical tan = tan(FirstPersonFOV / 2) x 0.7666 / S,
        // the class's FirstPersonFOV or the Controllable's 48 (re-zoom 5).
        let first = w.build.seats[w.walk.sim.vehicle(hs[0]).unwrap().class as usize]
            [usize::from(seat)]
        .first_person_fov;
        assert_eq!(
            first, None,
            "this real-class case checks the 48 degree default"
        );
        let want = 2.0 * ((48.0f32.to_radians() / 2.0).tan() * 0.7666 / z.value).atan();
        assert!(
            (cam.fov_y - want).abs() < 0.05f32.to_radians(),
            "{map}: zoomed fov {:.2} deg, the PS2's {:.2}",
            cam.fov_y.to_degrees(),
            want.to_degrees()
        );
        assert_eq!(gpu.hidden_for(&w.walk.sim, player), Some(mine.clone()));
        assert!(
            !sees(&gpu, &w, player, &mine),
            "{map}: his own building is left out"
        );
        assert!(
            sees(&gpu, &w, player, &theirs),
            "{map}: the other building stays"
        );
        assert!(
            sees(&gpu, &w, bot, &mine),
            "{map}: the other view still draws it"
        );

        // BF2's turret zooms on the left stick while zoomed (`ZoomRate`):
        // forward a second, then back a second; BF1's holds `ZoomMin`.
        let value = |w: &World| w.walk.sim.seat_zoom(player).map(|z| z.value);
        let hold = |w: &mut World, ly: f32| {
            for _ in 0..30 {
                w.walk.step_tick(&[PadState {
                    ly: PadState::quantize(ly),
                    ..Default::default()
                }]);
            }
        };
        hold(&mut w, 1.0);
        let fwd = value(&w).unwrap();
        hold(&mut w, -1.0);
        let back = value(&w).unwrap();
        println!("{map}: the left stick held forward 1 s: zoom {fwd}, back 1 s: {back}");
        if map.starts_with("bf2") && wc.zoom_rate != 0.0 {
            // From min, forward: z' = z (1 + rate dt) a tick, to the clamp.
            let want =
                (wc.zoom_min * (1.0 + wc.zoom_rate * core::TICK_DT).powi(30)).min(wc.zoom_max);
            assert!(
                (fwd - want).abs() < want * 1e-3,
                "{map}: {fwd} against {want}"
            );
            assert!(back < fwd && back >= wc.zoom_min, "{map}: zoomed back out");
        } else {
            assert_eq!(
                (fwd, back),
                (wc.zoom_min, wc.zoom_min),
                "{map}: no stick zoom"
            );
        }

        // Further presses: on to ZoomMax when the two differ and the class
        // has no ZoomRate, then off; with a ZoomRate the next press is off.
        tap(&mut w, buttons::ZOOM);
        if wc.zoom_min != wc.zoom_max && wc.zoom_rate == 0.0 && back == wc.zoom_min {
            assert_eq!(value(&w), Some(wc.zoom_max));
            tap(&mut w, buttons::ZOOM);
        }
        assert!(w.walk.sim.seat_zoom(player).is_none(), "{map}: zoomed off");
        assert!(sees(&gpu, &w, player, &mine));

        // A pilot change ends it: out of the seat and back in.
        tap(&mut w, buttons::ZOOM);
        assert!(w.walk.sim.seat_zoom(player).is_some());
        w.walk.sim.unseat_unit(player);
        assert!(w.walk.sim.seat_unit(player, hs[0], seat));
        assert!(
            w.walk.sim.seat_zoom(player).is_none(),
            "{map}: the pilot change ends the zoom"
        );
        assert!(sees(&gpu, &w, player, &mine));
        ran += 1;
    }
    println!("{ran} of {} cases ran", CASES.len());
}

/// The PS2's zoomed camera on its own RAM rows: (capture, the turret's place,
/// its yaw and its pitch in the port's sense (up +), the camera's position
/// and its forward). Every 15th zoomed row of the two captures; row 30 of
/// `myg1_zoom` - the first zoomed update, its camera still the third
/// person's - is left out.
type ZoomRow = (&'static str, [f32; 3], f32, f32, [f32; 3], [f32; 3]);

const PS2_ZOOM_ROWS: [ZoomRow; 8] = [
    (
        "bf1_nab1",
        [37.165, -99.676, 195.742],
        0.92472,
        -0.691084,
        [35.6383, -98.5575, 194.9356],
        [-0.68135, -0.63737, -0.35988],
    ),
    (
        "bf1_nab1",
        [37.165, -99.676, 195.742],
        0.74132,
        -0.691086,
        [35.8109, -98.5575, 194.6707],
        [-0.60429, -0.63737, -0.4781],
    ),
    (
        "bf1_nab1",
        [37.165, -99.676, 195.742],
        0.534521,
        -0.691088,
        [36.0597, -98.5575, 194.4155],
        [-0.49325, -0.63738, -0.59199],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        1.177409,
        -0.349066,
        [-116.1106, 6.7855, 308.1167],
        [-0.43353, -0.34202, -0.83371],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        0.982964,
        -0.349066,
        [-117.3291, 6.7855, 308.3974],
        [-0.26427, -0.34202, -0.90177],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        0.747408,
        -0.349066,
        [-118.8428, 6.7855, 308.4147],
        [-0.04652, -0.34202, -0.93854],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        0.641852,
        -0.249066,
        [-119.5499, 6.1163, 308.9516],
        [0.05427, -0.2465, -0.96762],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        0.641852,
        -0.082399,
        [-119.6013, 4.8689, 309.8677],
        [0.05581, -0.08231, -0.99504],
    ),
];

/// Acceptance 2's camera against the PS2: the player seated in the turret
/// standing at each row's place, zoomed, his gun's look set to the row's;
/// the port's zoomed camera stands within 2 cm of the PS2's and looks
/// within half a degree of its forward. The pitch moves the eye (the
/// offset is on the AIM frame): BF2's rows go from 6.79 m up at -20 deg to
/// 4.87 m at -5 deg; the camera ray of the shot leaves the same eye.
/// Mutations: the offset turned by the yaw alone -> off by up to 2.6 m;
/// the zoomed ray from the unzoomed eye -> the ray check fails.
#[test]
fn the_zoomed_eye_is_where_the_ps2s_camera_stood() {
    let mut ran = 0;
    let mut worst = (0.0f32, 0.0f32);
    for (map, at, yaw, pitch, eye, fwd) in PS2_ZOOM_ROWS {
        let Some(mut w) = world(map, ["rep", "cis"]) else {
            eprintln!("SKIP {map}: not baked");
            continue;
        };
        let at = Vec3::from(at);
        let (h, v) = w
            .walk
            .sim
            .alive_vehicles()
            .filter(|(_, v)| v.kind == core::vehicle::VehicleKind::Turret)
            .min_by(|a, b| a.1.pos.distance(at).total_cmp(&b.1.pos.distance(at)))
            .map(|(h, v)| (h, v.pos))
            .expect("a turret");
        assert!(
            v.distance(at) < 0.01,
            "{map}: the PS2's turret at {at}, the port's at {v}"
        );
        let ci = w.walk.sim.vehicle(h).unwrap().class as usize;
        let seat = w.walk.sim.seat_setup[ci]
            .as_ref()
            .and_then(|s| s.guns.iter().position(Option::is_some))
            .expect("a gunner seat") as u8;
        let player = w.walk.player;
        assert!(w.walk.sim.seat_unit(player, h, seat));
        w.walk.step_tick(&[PadState::default()]);
        tap(&mut w, buttons::ZOOM);
        let g = w.walk.sim.vehicle_mut(h).unwrap().guns[usize::from(seat)]
            .as_mut()
            .unwrap();
        assert!(g.zoom.is_some(), "{map}: zoomed");
        g.look = (yaw, pitch);
        let cam = w.walk.camera();
        let off = cam.position.distance(Vec3::from(eye));
        let ang = cam.forward().angle_between(Vec3::from(fwd)).to_degrees();
        println!("{map} look ({yaw:.3}, {pitch:.3}): eye {:?} against the PS2's {eye:?}: {off:.4} m, {ang:.3} deg", cam.position);
        worst = (worst.0.max(off), worst.1.max(ang));
        assert!(off < 0.02, "{map}: {off} m from the PS2's camera");
        // The shot's camera ray leaves the same eye (the scope's centre).
        let ray = w.walk.sim.seat_look(player).unwrap().eye;
        assert!(
            ray.distance(cam.position) < 1e-4,
            "{map}: the ray leaves {ray}"
        );
        assert!(ang < 0.5, "{map}: {ang} deg off the PS2's forward");
        ran += 1;
    }
    println!(
        "{ran} of {} rows ran; worst {:.4} m, {:.3} deg",
        PS2_ZOOM_ROWS.len(),
        worst.0,
        worst.1
    );
}

type ThirdPersonRow = (&'static str, [f32; 3], (f32, f32), [f32; 3], [f32; 3]);

/// The unzoomed camera on the PS2's settled RAM rows, the same captures as
/// [`PS2_ZOOM_ROWS`]: `(map, turret, look, camera, forward)`. BF1 NAB1's
/// `1_rest` and `a_settle`, BF2 MYG1's `1_rest` and `6_after`. The rows taken
/// mid-turn are left out: the PS2's camera had not caught up with them.
const PS2_THIRD_PERSON_ROWS: [ThirdPersonRow; 4] = [
    (
        "bf1_nab1",
        [37.165, -99.076, 195.742],
        (0.0, 0.0),
        [37.7152, -96.9471, 199.1485],
        [-0.1588, -0.0872, -0.9834],
    ),
    (
        "bf1_nab1",
        [37.165, -99.076, 195.742],
        (0.44958, -0.691101),
        [37.9106, -95.2362, 196.8094],
        [-0.4078, -0.7021, -0.5838],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        (1.200001, -0.349066),
        [-118.5744, 5.1776, 303.1233],
        [-0.4523, -0.342, -0.8237],
    ),
    (
        "bf2_myg1",
        [-118.159, 0.0, 302.01],
        (0.641852, -0.015732),
        [-119.1935, 4.6388, 304.383],
        [0.056, -0.0157, -0.9983],
    ),
];

/// Issue #71: `TrackCenter` and `TrackOffset` on the aim frame at the turret node
/// put the third-person camera where the PS2's stood, in both games.
/// Mutation: the turret back on `vehicle_camera` -> off by metres.
#[test]
fn the_third_person_camera_is_where_the_ps2s_camera_stood() {
    for (map, at, look, eye, fwd) in PS2_THIRD_PERSON_ROWS {
        let Some(mut w) = world(map, ["rep", "cis"]) else {
            eprintln!("SKIP {map}: not baked");
            continue;
        };
        let at = Vec3::from(at);
        let h = w
            .walk
            .sim
            .alive_vehicles()
            .filter(|(_, v)| v.kind == core::vehicle::VehicleKind::Turret)
            .min_by(|a, b| a.1.pos.distance(at).total_cmp(&b.1.pos.distance(at)))
            .map(|(h, _)| h)
            .expect("a turret");
        let ci = w.walk.sim.vehicle(h).unwrap().class as usize;
        let seat = w.walk.sim.seat_setup[ci]
            .as_ref()
            .and_then(|s| s.guns.iter().position(Option::is_some))
            .expect("a gunner seat");
        let player = w.walk.player;
        assert!(w.walk.sim.seat_unit(player, h, seat as u8));
        w.walk.step_tick(&[PadState::default()]);
        w.walk.sim.vehicle_mut(h).unwrap().guns[seat]
            .as_mut()
            .unwrap()
            .look = look;
        let cam = w.walk.camera();
        let off = cam.position.distance(Vec3::from(eye));
        let ang = cam.forward().angle_between(Vec3::from(fwd)).to_degrees();
        println!("{map} look {look:?}: {off:.4} m, {ang:.3} deg from the PS2's camera");
        assert!(off < 0.01, "{map}: {off} m from the PS2's camera");
        assert!(ang < 0.1, "{map}: {ang} deg off the PS2's forward");
    }
}

/// Check the returned commands, including the transforms that distinguish
/// two buildings sharing a mesh, against the scene with only `hidden` removed.
fn assert_view_draws(
    actual: &[render::DrawCmd],
    scene: &[render::DrawCmd],
    hidden: Option<std::ops::Range<usize>>,
) {
    let expected: Vec<_> = scene
        .iter()
        .enumerate()
        .filter(|(i, _)| !hidden.as_ref().is_some_and(|r| r.contains(i)))
        .map(|(_, d)| d)
        .collect();
    assert_eq!(actual.len(), expected.len(), "view draw count");
    for (i, (got, want)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            (
                got.mesh,
                got.texture,
                got.model,
                got.color,
                got.layer,
                got.light,
                got.cull,
                got.cull_near,
                got.lod,
                got.fade,
                got.lit
            ),
            (
                want.mesh,
                want.texture,
                want.model,
                want.color,
                want.layer,
                want.light,
                want.cull,
                want.cull_near,
                want.lod,
                want.fade,
                want.lit
            ),
            "view draw {i} identity, transform and appearance"
        );
    }
}

#[test]
fn own_view_draw_check_rejects_removing_a_different_equal_length_range() {
    let scene: Vec<_> = (0..4)
        .map(|i| {
            render::DrawCmd::flat(
                render::MeshHandle::default(),
                [1.0; 4],
                Mat4::from_translation(Vec3::new(i as f32, 0.0, 0.0)),
            )
        })
        .collect();
    assert_view_draws(&scene, &scene, None);
    let correct = vec![scene[0], scene[3]];
    assert_view_draws(&correct, &scene, Some(1..3));
    for wrong in [&scene[..2], &scene[2..]] {
        assert!(
            std::panic::catch_unwind(|| assert_view_draws(wrong, &scene, Some(1..3))).is_err(),
            "a view with the right count but the wrong buildings must fail"
        );
    }
}

/// All bind-pose draws at one manifest placement, independently of the
/// turret's recorded hidden range. Equal meshes at other placements differ.
fn building_draws(draws: &[render::DrawCmd], model: Mat4) -> std::ops::Range<usize> {
    let indices: Vec<_> = draws
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            (d.model == model
                && d.cull_near.is_none()
                && !matches!(d.lod, Some(l) if l.side == render::LodSide::Lowd))
            .then_some(i)
        })
        .collect();
    let start = *indices.first().expect("the placed building has draws");
    let end = indices.last().unwrap() + 1;
    assert_eq!(
        indices,
        (start..end).collect::<Vec<_>>(),
        "building draws are contiguous"
    );
    start..end
}

/// Synthetic setter and input fixture: its authored 72 degrees deliberately
/// differs from the 48 degree default. It makes no disc-census claim.
fn fixture_walk(first: Option<f32>) -> game::walk::WalkSim {
    use core::vehicle::gun::{Aimer, GunSetup, GunWeapon, Slew, Turn};
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use game::classdb::{fnv1a_lowercase as hash, ClassDb, ClassDef};
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let mut props = vec![
        (hash("BuildingSection"), "TURRET1".into()),
        (hash("WeaponName"), "fixture_weapon".into()),
    ];
    if let Some(fov) = first {
        props.push((hash("FirstPersonFOV"), fov.to_string()));
    }
    let defs = vec![ClassDef {
        name: "fixture_turret".into(),
        kind: "entc".into(),
        base: "armedbuilding".into(),
        props,
    }];
    let path = std::env::temp_dir().join(format!(
        "openbf-vtzt-{}-{}.ron",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut file, ron::to_string(&defs).unwrap().as_bytes()).unwrap();
    let mut db = ClassDb::default();
    let loaded = db.load(&path);
    std::fs::remove_file(&path).unwrap();
    loaded.unwrap();
    let ring = game::vehicle_class::seats(&db, "fixture_turret");
    assert_eq!(ring.len(), 1);
    assert_eq!(
        ring[0].first_person_fov, first,
        "authored key survives parsing"
    );
    let mut sim = core::sim::Sim::empty(1);
    // A BF1 map, as world() does from the manifest; the chase camera's
    // third-person FOV is BF2's 65 deg on every map (VTZT).
    sim.ground_rules = core::soldier::GroundRules::Bf1;
    let mut weapon = core::weapon::WeaponClass::bf1_rifle();
    weapon.zoom_min = 3.0;
    weapon.zoom_max = 3.0;
    weapon.zoom_rate = 0.0;
    let weapon_class = sim.add_weapon_class(weapon);
    let mut setup = game::vehicle_pads::seat_setup(&db, "fixture_turret", &ring, "bf1");
    setup.guns = vec![Some(GunSetup {
        weapons: vec![GunWeapon {
            class: weapon_class,
            infinite: true,
            reserve_clips: 0,
            fire_points: Vec::new(),
            fire_limits: Vec::new(),
            fire_pivots: Vec::new(),
        }],
        fire_point: Vec3::Z,
        pivot: Vec3::ZERO,
        eye: Vec3::Y,
        aimer: Aimer::default(),
        pilot: false,
        framed: false,
        aimer_bind: glam::Quat::IDENTITY,
        weapon_aimer: None,
        follows_owner: false,
        gunner: None,
        turn: Some(Turn::from_class(
            None,
            None,
            (2.0, 0.5),
            (10.0, 10.0),
            Slew::Exp,
        )),
    })];
    sim.seat_setup = vec![Some(setup)];
    let h = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    });
    let t = game::walk::TeamTemplate::default_trooper();
    let mut walk = game::walk::WalkSim::with_teams(sim, None, Vec3::ZERO, [t.clone(), t], None);
    walk.vehicle_seats = vec![ring];
    assert!(walk.sim.seat_unit(walk.player, h, 0));
    walk.step_tick(&[PadState::default()]);
    walk
}

#[test]
fn held_r3_toggles_once_until_released() {
    let mut walk = fixture_walk(None);
    let held = PadState {
        buttons: buttons::ZOOM,
        ..Default::default()
    };
    for expected in [Some(3.0), None, Some(3.0)] {
        for tick in 0..4 {
            walk.step_tick(&[held]);
            assert_eq!(
                walk.sim.seat_zoom(walk.player).map(|z| z.value),
                expected,
                "held tick {tick}"
            );
        }
        walk.step_tick(&[PadState::default()]);
        assert_eq!(
            walk.sim.seat_zoom(walk.player).map(|z| z.value),
            expected,
            "release preserves zoom"
        );
    }
}

#[test]
fn authored_and_default_first_person_fovs_reach_the_zoomed_camera() {
    for (authored, degrees) in [(None, 48.0f32), (Some(72.0), 72.0)] {
        let mut walk = fixture_walk(authored);
        walk.step_tick(&[PadState {
            buttons: buttons::ZOOM,
            ..Default::default()
        }]);
        for _ in 0..60 {
            walk.step_tick(&[PadState::default()]);
        }
        assert_eq!(walk.sim.seat_zoom(walk.player).unwrap().value, 3.0);
        let want = 2.0 * ((degrees.to_radians() / 2.0).tan() * 0.7666 / 3.0).atan();
        let got = walk.camera().fov_y;
        eprintln!(
            "{authored:?}: zoomed camera FOV {:.3} deg, want {:.3} deg",
            got.to_degrees(),
            want.to_degrees()
        );
        assert!(
            (got - want).abs() < 0.05f32.to_radians(),
            "{authored:?}: camera FOV {:.3} deg, want {:.3} deg",
            got.to_degrees(),
            want.to_degrees()
        );
    }
}
