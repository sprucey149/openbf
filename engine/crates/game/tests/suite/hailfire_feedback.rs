use super::seat_camera::{map, missions_of};
use core::sim::{buttons, PadState};
use core::vehicle::VehicleSpec;

#[test]
fn bf1_hailfire_secondary_passes_out_of_the_terrain_at_the_reported_pose() {
    use core::sim::Event;
    use glam::Vec3;
    for pitch in [0.0, -0.07] {
        let root = game::assets::baked_root();
        let mission = missions_of("bf1_geo1").into_iter().next().unwrap();
        let mut m = map(&root.join("bf1_geo1"), &mission.factions()).unwrap();
        let ci = m
            .build
            .classes
            .iter()
            .position(|c| c == "cis_tread_hailfire")
            .unwrap();
        let mut spec = m
            .build
            .pads
            .iter()
            .flat_map(|p| p.classes.iter().flatten())
            .find(|c| c.spec.class as usize == ci)
            .unwrap()
            .spec
            .clone();
        // Snapshot a4ad72f9+e200d8e8c20e7a41, exit tick 583, vehicle 10.
        spec.pos = Vec3::new(194.37, -63.86, -29.81);
        // Spec yaw is negated when the vehicle rotation is built.
        spec.yaw = -1.93;
        let h = m.walk.sim.spawn_vehicle(&spec);
        let player = m.walk.player;
        assert!(m.walk.sim.seat_unit(player, h, 0));
        m.walk.sim.step(&[PadState::default()]);
        m.walk.sim.vehicle_mut(h).unwrap().guns[0]
            .as_mut()
            .unwrap()
            .look
            .1 = pitch;
        for _ in 0..60 {
            m.walk.sim.step(&[PadState::default()]);
        }
        m.walk.sim.step(&[PadState {
            buttons: buttons::SECONDARY_FIRE,
            ..Default::default()
        }]);
        let (origin, dir, weapon) = m
            .walk
            .sim
            .events
            .iter()
            .find_map(|e| match e {
                Event::Shot {
                    unit,
                    origin,
                    dir,
                    weapon_class,
                    ..
                } if *unit == player => Some((*origin, *dir, *weapon_class as usize)),
                _ => None,
            })
            .expect("the secondary fires");
        let hf = m.walk.sim.heightfield.as_ref().unwrap();
        assert!(
            origin.y < hf.height_at(origin.x, origin.z),
            "the original's root muzzle is below the sheet here"
        );
        assert!(origin.distance(m.walk.sim.vehicle(h).unwrap().pos) < 0.01);
        let ord = &m.walk.sim.weapon_classes[weapon].ordnance;
        let reach = ord.velocity * ord.life_span;
        let old = core::collision::ray_heightfield(
            hf,
            &m.walk.sim.collision.cutters,
            origin,
            dir,
            reach,
            0.5,
        )
        .expect("old two-sided ray hits the outward crossing");
        let bolt = m
            .walk
            .sim
            .bolts
            .iter()
            .find(|b| b.owner == player && b.weapon_class == weapon)
            .expect("the bolt survives the first tick");
        let travel = origin.distance(bolt.end);
        assert!(
            travel > old + 1.0,
            "outward crossing {old} must not stop the shot at {travel}"
        );
        let native = core::collision::ray_terrain_ps2(
            hf,
            &m.walk.sim.collision.cutters,
            origin,
            origin + dir * reach,
        );
        if bolt.hit == core::projectile::Hit::Terrain {
            let f =
                native.expect("the original terrain ray must also find the later ground impact");
            assert!(
                (travel - f * reach).abs() < 0.3,
                "native terrain {}, port {travel}",
                f * reach
            );
        } else if let Some(f) = native {
            assert!(
                travel <= f * reach + 0.3,
                "a world obstacle must precede the terrain"
            );
        }
        eprintln!("BF1 secondary pitch {pitch}: old underside hit {old:.3} m, corrected travel {travel:.3} m ({:?})",bolt.hit);
    }
}

#[test]
#[ignore = "requires local Geonosis assets and a headless GPU"]
fn bf1_hailfire_draws_scrolling_wheels_and_moving_suspension() {
    use glam::Vec3;
    let root = game::assets::baked_root();
    let dir = root.join("bf1_geo1");
    let mission = missions_of("bf1_geo1").into_iter().next().unwrap();
    let scene = game::assets::load_scene(&dir).unwrap();
    let mut m = map(&dir, &mission.factions()).unwrap();
    let ci = m
        .build
        .classes
        .iter()
        .position(|c| c == "cis_tread_hailfire")
        .unwrap();
    let mut spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| p.classes.iter().flatten())
        .find(|c| c.spec.class as usize == ci)
        .unwrap()
        .spec
        .clone();
    spec.pos = Vec3::new(0.0, -1.0, -18.0);
    spec.yaw = 0.0;
    m.walk.sim.vehicles.clear();
    let h = m.walk.sim.spawn_vehicle(&spec);
    let mut r = render::Renderer::new_headless(640, 480).unwrap();
    let mut vis = game::viewer::VehicleVisuals::load(&mut r, &scene, &root, &m.build.geometry);
    vis.set_treads(m.build.hover_treads.clone());
    let cam = render::Camera {
        position: Vec3::new(0.0, 4.0, 0.0),
        pitch: -0.1,
        ..Default::default()
    };
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/hailfire-bf1");
    std::fs::create_dir_all(&out).unwrap();
    let mut capture = |sim: &core::sim::Sim, label: &str| {
        let draws = vis.draws(&mut r, sim, &[]);
        assert!(!draws.is_empty());
        r.render_frame(&cam, &draws, &[], [0.12, 0.15, 0.2, 1.0], None)
            .unwrap();
        let (rgba, w, height) = r.capture_rgba().unwrap();
        std::fs::write(
            out.join(format!("{label}.png")),
            render::encode_png(&rgba, w, height).unwrap(),
        )
        .unwrap();
        rgba
    };
    let rest = capture(&m.walk.sim, "rest");
    m.walk.sim.vehicle_mut(h).unwrap().vel = Vec3::Z * 8.0;
    m.walk.sim.vehicle_mut(h).unwrap().hover.omega.y = 0.3;
    m.walk.sim.tick += 15;
    let rolling = capture(&m.walk.sim, "rolling");
    let changed = |a: &[u8], b: &[u8]| {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
    };
    let wheel_pixels = changed(&rest, &rolling);
    assert!(
        wheel_pixels > 100,
        "wheel scroll changed only {wheel_pixels} pixels"
    );
    m.walk.sim.vehicle_mut(h).unwrap().hover.suspension = Some(core::vehicle::hover::Suspension {
        s: -0.4,
        v: 0.0,
        t: -0.5,
    });
    let sunk = capture(&m.walk.sim, "compressed");
    let suspension_pixels = changed(&rolling, &sunk);
    assert!(
        suspension_pixels > 100,
        "suspension changed only {suspension_pixels} pixels"
    );
    assert_eq!(
        sunk,
        capture(&m.walk.sim, "frozen"),
        "drawing twice at one tick must hold the pose"
    );
    eprintln!("BF1 Hailfire: {wheel_pixels} wheel pixels; {suspension_pixels} suspension pixels");
}

#[test]
fn hailfire_suspension_and_wheels_reach_the_battle_rig() {
    let root = game::assets::baked_root();
    for key in ["bf1_geo1", "bf2_geo1"] {
        let mission = missions_of(key)
            .into_iter()
            .next()
            .expect("Geonosis mission");
        let dir = root.join(key);
        let scene = game::assets::load_scene(&dir).expect("Geonosis bake");
        let mut m = map(&dir, &mission.factions()).expect("battle setup");
        let ci = m
            .build
            .classes
            .iter()
            .position(|c| c == "cis_tread_hailfire")
            .expect("Hailfire class");
        assert_eq!(m.build.hover_treads[ci].len(), 2, "{key}: scrolling wheels");
        let params = m.walk.sim.hover_params[ci].as_ref().unwrap();
        let suspension = params.suspension.expect("suspension class numbers");
        assert_eq!((suspension.min, suspension.rest), (-0.5, -0.125));
        let rigs = game::viewer::hover_rigs(&scene.manifest, &root, &m.build.geometry);
        let rig = rigs[ci]
            .as_ref()
            .expect("the rendered Hailfire must have a skin");
        let spec = m
            .build
            .pads
            .iter()
            .flat_map(|p| p.classes.iter().flatten())
            .find(|c| c.spec.class as usize == ci)
            .expect("Hailfire pad")
            .spec
            .clone();
        let h = m.walk.sim.spawn_vehicle(&spec);
        let v = m.walk.sim.vehicle_mut(h).unwrap();
        let rest = rig.hover_nodes(v, None);
        let centre = rig.skeleton.index_of("CIS_HailFire_Droid_center").unwrap();
        assert!(
            (rest[centre].w_axis.y - rig.skeleton.global_bind[centre].w_axis.y + 0.125).abs()
                < 1e-5
        );
        if key == "bf1_geo1" {
            if let Some(path) = std::env::var_os("HAILFIRE_NATIVE_NODES") {
                let native: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                for (label, name) in [
                    ("centre", "CIS_HailFire_Droid_center"),
                    ("left", "armL"),
                    ("right", "armR"),
                ] {
                    let n = rig.skeleton.index_of(name).unwrap();
                    let expected = native["vehicles"][0]["nodes"][label].as_array().unwrap();
                    let error = rest[n]
                        .to_cols_array()
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| (*a - b.as_f64().unwrap() as f32).abs())
                        .fold(0.0, f32::max);
                    eprintln!("BF1 slot 5 {label}: max matrix error {error}");
                    assert!(error < 1e-5, "{label}: {error}");
                }
            }
        }
        v.hover.suspension = Some(core::vehicle::hover::Suspension {
            s: -0.4,
            v: 0.0,
            t: -0.5,
        });
        let sunk = rig.hover_nodes(v, None);
        assert_ne!(sunk[centre], rest[centre]);
        for name in ["armL", "armR"] {
            let arm = rig.skeleton.index_of(name).unwrap();
            assert_ne!(sunk[arm], rest[arm]);
            assert_eq!(
                sunk[arm].w_axis, rest[arm].w_axis,
                "the arm pivot stays fixed"
            );
        }
    }
}

#[test]
fn hailfire_gatling_has_an_audible_loop_from_the_players_seat() {
    let root = game::assets::baked_root();
    let key = "bf2_geo1";
    let mission = missions_of(key)
        .into_iter()
        .next()
        .expect("Geonosis mission");
    let mut m = map(&root.join(key), &mission.factions()).expect("Geonosis bake");
    m.walk.walker_shot_styles = m.build.walker_shot_styles.clone();
    let spec = m
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
        .find(|s| m.build.classes[s.class as usize] == "cis_tread_hailfire")
        .expect("Hailfire pad");
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 0));
    let mut heard = 0;
    let lib = game::sounds::SoundLib::load(&root.join(key)).expect("Geonosis sound bank");
    for _ in 0..60 {
        m.walk.sim.step(&[PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        for (unit, _, name, pos) in game::walker_shots::loops(&m.walk) {
            if unit != player {
                continue;
            }
            let (_, _, samples, _) = lib.audio_of_name(name).expect("gatling loop definition");
            assert!(!samples.is_empty(), "gatling loop must name a bank sample");
            let take = lib
                .take_path_by_hash(game::classdb::fnv1a_lowercase(name), 0)
                .expect("baked gatling loop take");
            assert!(take.is_file(), "{}", take.display());
            if heard == 0 {
                let decoded = audio::decode(&take).expect("decodable gatling loop");
                assert!(decoded.duration_secs() > 0.0 && decoded.frames() > 0);
            }
            let listener = game::sounds::Listener {
                pos: m.walk.camera().position,
                yaw: m.walk.camera().yaw,
                more: [None; 3],
                vels: None,
            };
            let voice = game::engine_sound::Voice {
                vehicle: h.index,
                soldier: false,
                layer: 0,
                record: 0,
                sound: game::classdb::fnv1a_lowercase(name),
                pos,
                gain_factor: 1.0,
                pitch_factor: 1.0,
                speed: 0.0,
            };
            let ((left, right), rate) = lib
                .engine_level(&voice, &listener)
                .expect("audible from seat");
            assert!(left.max(right) > 0.0 && rate > 0.0);
            heard += 1;
        }
    }
    assert!(heard >= 10, "only {heard} firing ticks have a loop");
    m.walk.sim.step(&[PadState::default()]);
    assert!(game::walker_shots::loops(&m.walk)
        .iter()
        .all(|(u, ..)| *u != player));
}
