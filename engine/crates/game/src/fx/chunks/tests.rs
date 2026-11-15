//! Destruction chunk launch, contacts, lifetimes and real-data checks.
use super::*;
use crate::classdb::{fnv1a_lowercase as hash, ClassDef};

fn spec(prim: Prim) -> Spec {
    Spec {
        model: "fixture".into(),
        prim,
        local: Mat4::IDENTITY,
        speed: 6.0,
        inherit: 0.5,
        up: 0.25,
        bounce: 0.6,
        stick: 0.7,
        linear_damping: 0.001,
        angular_damping: 0.004,
        gravity: Vec3::new(0.0, -9.8, 0.0),
        omega: Vec3::ZERO,
        lifetime: -1.0,
        mode: 0,
        budget: 0,
        terrain_effect: None,
        trails: Vec::new(),
        smoke: Vec::new(),
        node: None,
        node_name: None,
    }
}

fn chunks(spec: Spec) -> Chunks {
    Chunks {
        defs: HashMap::from([("fixture".into(), vec![Arc::new(spec)])]),
        ..Default::default()
    }
}

fn flat() -> WalkSim {
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    walk.sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 100.0,
        offset_x: 50.0,
        offset_z: 50.0,
        heights: vec![0.0; 4],
    });
    walk
}

fn drop(c: &mut Chunks, at: Vec3, vel: Vec3) {
    c.launch("fixture", at, Vec3::ZERO, Quat::IDENTITY, 1.0, 1.0, None);
    c.live.back_mut().unwrap()[0].vel = vel;
}

fn piece(c: &Chunks) -> &Piece {
    &c.live[0][0]
}

#[test]
fn a_chunk_fades_over_the_last_two_seconds_of_its_life() {
    let mut c = chunks(spec(Prim::Sphere(1.0)));
    drop(&mut c, Vec3::ZERO, Vec3::ZERO);
    let life = c.live[0][0].life();
    for (timer, opacity) in [
        (0.0, 1.0),
        (life - 2.0, 1.0),
        (life - 1.0, 0.5),
        (life, 0.0),
    ] {
        c.live[0][0].timer = timer;
        assert_eq!(c.live[0][0].opacity(), opacity, "at {timer}");
    }
}

#[test]
fn a_chunk_with_a_lit_model_leaves_the_particle_path() {
    struct Assets;
    impl FxAssets for Assets {
        fn texture(&mut self, _: &str) -> Option<render::TextureHandle> {
            None
        }
        fn model(&mut self, _: &str) -> Option<Arc<crate::fx::draw::EffectModel>> {
            Some(Arc::new(crate::fx::draw::EffectModel {
                parts: vec![(None, vec![(Vec3::X, [0.0; 2]); 3], 0)],
            }))
        }
    }
    let mut c = chunks(spec(Prim::Sphere(1.0)));
    drop(&mut c, Vec3::Y, Vec3::ZERO);
    let particles = |c: &Chunks| {
        let mut out = Vec::new();
        c.draws(
            Era::Bf2,
            Vec3::ZERO,
            Vec3::Z,
            1.0,
            0.0,
            &mut Assets,
            &mut out,
        );
        out.len()
    };
    assert_eq!(c.lit_poses().count(), 0);
    assert_eq!(particles(&c), 1);
    c.set_lit(["fixture".to_owned()]);
    assert_eq!(particles(&c), 0);
    let poses: Vec<_> = c.lit_poses().collect();
    assert_eq!(poses.len(), 1);
    assert_eq!(poses[0].0, "fixture");
    assert_eq!(poses[0].2, 1.0);
}

#[test]
fn authored_launch_inherits_velocity_and_droid_scale() {
    let v = launch_velocity(
        Vec3::X * 10.0,
        Vec3::Y,
        2.0,
        2.0,
        6.0,
        0.5,
        false,
        0.1,
        &mut Lcg(1),
    );
    assert!((v - Vec3::new(0.5, 0.6, 0.0)).length() < 0.00001);
    let v = launch_velocity(
        Vec3::ZERO,
        Vec3::X,
        0.5,
        1.0,
        -1.0,
        1.0,
        false,
        1.0,
        &mut Lcg(1),
    );
    assert!((4.0..6.0).contains(&v.x));
    assert!((2.0..3.0).contains(&v.y));
    let full = launch_velocity(
        Vec3::ZERO,
        Vec3::X,
        4.0,
        1.0,
        -1.0,
        1.0,
        false,
        1.0,
        &mut Lcg(7),
    );
    let leaf = launch_velocity(
        Vec3::ZERO,
        Vec3::X,
        4.0,
        1.0,
        -1.0,
        1.0,
        true,
        1.0,
        &mut Lcg(7),
    );
    assert!((15.0..30.0).contains(&full.x));
    assert!(leaf.x <= 7.5 && leaf.x >= 4.0, "{leaf}");
}

#[test]
fn prim_inertia_bound_and_support_follow_the_ps2_shapes() {
    let b = Prim::Box(Vec3::new(1.0, 2.0, 3.0));
    assert!((b.inertia() - Vec3::new(13.0, 10.0, 5.0) * 0.33333).length() < 1e-5);
    assert!((b.bound() - 14f32.sqrt()).abs() < 1e-6);
    assert_eq!(Prim::Sphere(2.0).inertia(), Vec3::splat(1.6));
    let c = Prim::Cylinder {
        radius: 1.0,
        height: 3.0,
    };
    assert!(
        (c.inertia() - Vec3::new(0.25 + 9.0 * 0.08333, 0.5, 0.25 + 9.0 * 0.08333)).length() < 1e-6
    );
    assert!((c.bound() - 3.25f32.sqrt()).abs() < 1e-6);
    assert_eq!(
        c.support(Quat::IDENTITY, Vec3::NEG_Y),
        Vec3::new(0.0, -1.5, 0.0)
    );
    let tipped = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    assert!((c.support(tipped, Vec3::NEG_Y).y + 1.0).abs() < 1e-5);
    assert_eq!(
        b.support(Quat::IDENTITY, Vec3::new(-1.0, -1.0, 1.0)),
        Vec3::new(-1.0, -2.0, 3.0)
    );
    assert_eq!(
        Prim::Sphere(2.0).support(Quat::IDENTITY, Vec3::X * 5.0),
        Vec3::X * 2.0
    );
}

fn class(props: &[(&str, &str)]) -> ClassDef {
    ClassDef {
        name: "fixture".into(),
        kind: "entc".into(),
        base: "hover".into(),
        props: props.iter().map(|(k, v)| (hash(k), (*v).into())).collect(),
    }
}

#[test]
fn chunk_sections_inherit_by_slot_without_sharing_rates() {
    let base = class(&[
        ("CHUNKSECTION", "CHUNK1"),
        ("ChunkGeometryName", "a"),
        ("ChunkSpeed", "2"),
        ("CHUNKSECTION", "CHUNK2"),
        ("ChunkGeometryName", "b"),
        ("ChunkSpeed", "9"),
    ]);
    let child = class(&[("CHUNKSECTION", "CHUNK1"), ("ChunkSpeed", "6")]);
    let slots = chunk_properties(vec![&child, &base]);
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].props[&hash("ChunkGeometryName")], "a");
    assert_eq!(slots[0].props[&hash("ChunkSpeed")], "6");
    assert_eq!(slots[1].props[&hash("ChunkSpeed")], "9");
}

#[test]
fn trails_and_smoke_keep_three_each_and_a_node_names_the_latest() {
    let def = class(&[
        ("CHUNKSECTION", "CHUNK1"),
        ("ChunkGeometryName", "a"),
        ("ChunkTrailEffect", "t1"),
        ("ChunkTrailEffect", "t2"),
        ("ChunkTrailEffect", "t3"),
        ("ChunkTrailEffect", "t4"),
        ("ChunkSmokeEffect", "s1"),
        ("ChunkSmokeNodeName", "hp_smoke1"),
        ("ChunkSmokeEffect", "s2"),
        ("CHUNKSECTION", "CHUNK2"),
        ("ChunkGeometryName", "b"),
        ("ChunkSmokeNodeName", "orphan"),
    ]);
    let slots = chunk_properties(vec![&def]);
    let names = |l: &[(String, Option<String>)]| -> Vec<(String, Option<String>)> { l.to_vec() };
    assert_eq!(
        names(&slots[0].trails),
        vec![
            ("t1".into(), None),
            ("t2".into(), None),
            ("t3".into(), None)
        ]
    );
    assert_eq!(
        names(&slots[0].smoke),
        vec![("s1".into(), Some("hp_smoke1".into())), ("s2".into(), None)]
    );
    assert!(slots[1].smoke.is_empty() && slots[1].trails.is_empty());
}

#[test]
fn vehicle_event_launches_then_expires_without_touching_the_sim() {
    let mut c = chunks(spec(Prim::Sphere(0.2)));
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    walk.vehicle_classes = vec!["fixture".into()];
    walk.sim.events.push(Event::VehicleDestroyed {
        vehicle: core::vehicle::VehicleHandle { index: 0, gen: 0 },
        class: 0,
        pos: Vec3::Y * 10.0,
        vel: Vec3::X * 10.0,
        killer: None,
    });
    let hash = walk.sim.hash();
    c.tick(&walk, None);
    assert_eq!(c.len(), 1);
    assert_eq!(walk.sim.hash(), hash);
    assert!((piece(&c).vel - Vec3::new(5.0, 6.0, 0.0)).length() < 0.00001);
    walk.sim.events.clear();
    let ticks = (3.0 / core::TICK_DT).ceil() as usize;
    for _ in 0..ticks - 2 {
        c.tick(&walk, None);
    }
    assert_eq!(c.len(), 1, "no terrain: the timer runs in the air");
    for _ in 0..3 {
        c.tick(&walk, None);
    }
    assert_eq!(c.len(), 0, "the default life is BF1's 3 s on BF2 too");
}

#[test]
fn a_terrain_hit_under_the_budget_bounces_by_the_restitution() {
    let mut s = spec(Prim::Sphere(0.2));
    s.budget = 3;
    let mut c = chunks(s);
    let walk = flat();
    drop(&mut c, Vec3::Y * 0.21, Vec3::NEG_Y * 10.0);
    c.tick(&walk, None);
    let p = piece(&c);
    assert_eq!(p.count, 1);
    let vn = -10.0 - 9.8 * core::TICK_DT;
    assert!((p.vel.y - (-0.6 * vn)).abs() < 0.05, "{}", p.vel.y);
    assert!((p.pos.y - 0.2).abs() < 1e-4, "{}", p.pos.y);
    assert!(
        p.omega.length() < 1e-6,
        "a centred hit gives a sphere no spin"
    );
}

#[test]
fn friction_spins_a_sliding_sphere_without_slowing_it() {
    let mut s = spec(Prim::Sphere(0.2));
    s.budget = 10;
    let mut c = chunks(s);
    let walk = flat();
    drop(&mut c, Vec3::Y * 0.19, Vec3::X * 5.0);
    c.tick(&walk, None);
    let p = piece(&c);
    assert!(p.omega.z < -1.0, "{}", p.omega);
    assert!((p.vel.x - 5.0).abs() < 0.01, "{}", p.vel.x);
}

#[test]
fn the_budget_hit_gets_no_impulse_and_the_chunk_sinks_and_settles() {
    let mut c = chunks(spec(Prim::Sphere(0.2)));
    let walk = flat();
    drop(&mut c, Vec3::Y * 0.25, Vec3::NEG_Y * 1.0);
    let mut bounced = false;
    for _ in 0..60 {
        c.tick(&walk, None);
        bounced |= piece(&c).vel.y > 0.0;
        if !piece(&c).moving {
            break;
        }
    }
    let p = piece(&c);
    assert!(!bounced);
    assert!(!p.moving);
    assert_eq!(p.count, 1);
    assert!(p.pos.y < 0.75 * 0.2 * 1.001 && p.pos.y > 0.0, "{}", p.pos.y);
}

#[test]
fn a_resting_chunk_lives_three_seconds_on_both_games() {
    for (bf1, life) in [(false, 3.0), (true, 3.0)] {
        let mut c = chunks(spec(Prim::Sphere(0.2)));
        c.bf1 = bf1;
        let walk = flat();
        drop(&mut c, Vec3::Y * 0.2, Vec3::ZERO);
        c.live[0][0].moving = false;
        let mut ticks = 0;
        while !c.is_empty() {
            c.tick(&walk, None);
            ticks += 1;
        }
        let (mut t, mut want) = (0f32, 0);
        while t < life {
            t += core::TICK_DT;
            want += 1;
        }
        assert_eq!(ticks, want, "bf1 {bf1}");
    }
}

#[test]
fn chunk_omega_spins_the_body_and_angular_damping_slows_it() {
    let mut s = spec(Prim::Box(Vec3::new(0.5, 0.2, 1.0)));
    s.omega = Vec3::new(0.0, 4.0, 0.0);
    let mut c = chunks(s);
    let walk = WalkSim::new(1, Vec3::ZERO, None);
    c.launch(
        "fixture",
        Vec3::Y * 50.0,
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
    );
    c.tick(&walk, None);
    let p = piece(&c);
    let (axis, angle) = p.rot.to_axis_angle();
    let want = 4.0 * (1.0 - 0.004 * core::TICK_DT) * core::TICK_DT;
    assert!((axis - Vec3::Y).length() < 1e-4);
    assert!((angle - want).abs() < 1e-5, "{angle} {want}");
}

#[test]
fn static_chunks_never_move_and_rest_fifty_three_seconds() {
    let mut s = spec(Prim::Sphere(0.2));
    s.mode = 2;
    let mut c = chunks(s);
    let walk = flat();
    c.launch(
        "fixture",
        Vec3::Y * 5.0,
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
    );
    for _ in 0..30 {
        c.tick(&walk, None);
    }
    assert_eq!(piece(&c).pos, Vec3::Y * 5.0);
    assert_eq!(piece(&c).life(), 53.0);
}

#[test]
fn the_pool_holds_eight_clusters_and_clear_removes_every_piece() {
    let mut c = chunks(spec(Prim::Sphere(0.2)));
    for _ in 0..12 {
        c.launch(
            "fixture",
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            1.0,
            1.0,
            None,
        );
    }
    assert_eq!(c.live.len(), 8);
    assert_eq!(c.len(), 8);
    c.clear();
    assert_eq!(c.len(), 0);
}

#[test]
fn a_chunk_node_and_the_prim_frame_place_the_body() {
    let mut s = spec(Prim::Sphere(0.2));
    s.node = Some(Mat4::from_translation(Vec3::new(0.0, 2.0, 0.0)));
    s.local = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
    let mut c = chunks(s);
    c.launch(
        "fixture",
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::ZERO,
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        1.0,
        1.0,
        None,
    );
    let p = piece(&c);
    assert!(
        (p.pos - Vec3::new(10.0, 2.0, -1.0)).length() < 1e-5,
        "{}",
        p.pos
    );
    assert!(
        p.model()
            .w_axis
            .truncate()
            .distance(Vec3::new(10.0, 2.0, 0.0))
            < 1e-5
    );
}

#[test]
#[ignore = "requires the targeted real droid chunk overlay"]
fn real_bf2_class_resolves_five_droid_pieces() {
    let root = crate::assets::baked_root();
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["cis", "rep"]).unwrap();
    let c = Chunks::load(
        &db,
        &["cis_inf_rifleman".into()],
        &[root.join("side_bf2/droid_chunks/weapons")],
        Era::Bf2,
    );
    assert_eq!(c.defs["cis_inf_rifleman"].len(), 5);
}

#[test]
#[ignore = "requires the baked BF2 side vehicles"]
fn real_bf2_chunks_take_their_prims_trails_and_only_skeleton_nodes() {
    let root = crate::assets::baked_root();
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep", "cis"]).unwrap();
    let dirs: Vec<_> = ["all", "cis", "imp", "rep"]
        .iter()
        .map(|s| root.join(format!("side_bf2/vehicles/{s}/models")))
        .collect();
    let names = [
        "rep_fly_gunship".to_string(),
        "cis_tread_hailfire".to_string(),
    ];
    let c = Chunks::load(&db, &names, &dirs, Era::Bf2);
    let gunship = &c.defs["rep_fly_gunship"];
    assert!(gunship.iter().any(|s| matches!(s.prim, Prim::Box(_))));
    assert!(gunship.iter().all(|s| !s.trails.is_empty()));
    assert!(gunship.iter().all(|s| s.node.is_none()));
    assert!(c.defs["cis_tread_hailfire"]
        .iter()
        .any(|s| s.node.is_some()));
}

#[test]
#[ignore = "requires bake droid-chunks from both discs"]
fn real_sbd_and_magnaguard_profiles_launch_five_pieces_at_the_authored_frequency() {
    use core::unit::UnitSpec;
    let root = crate::assets::baked_root();
    for (game, era, class, prefix) in [
        ("bf2", Era::Bf2, "cis_inf_rifleman", "sbdroid_chunk"),
        ("bf1", Era::Bf1, "cis_inf_battledroid", "sbdroid_chunk"),
        ("bf2", Era::Bf2, "cis_inf_officer", "magnaguard_chunk"),
        ("bf2", Era::Bf2, "cis_inf_officer_hunt", "magnaguard_chunk"),
    ] {
        let db = crate::roster::load_classdb(&root.join("classes"), game, &["cis", "rep"]).unwrap();
        assert_eq!(
            db.prop(class, "ChunkGeometryName"),
            Some(format!("{prefix}1").as_str()),
            "{game} {class}: resolved chunk geometry"
        );
        let roster = crate::roster::ClassRoster::build(&db, class, 0, None).unwrap();
        let mut walk = WalkSim::new(7, Vec3::ZERO, None);
        walk.sim.ground_rules = if game == "bf1" {
            core::soldier::GroundRules::Bf1
        } else {
            core::soldier::GroundRules::Bf2
        };
        let registered = roster.register(&mut walk.sim, &mut Vec::new());
        assert_eq!(
            registered.chunk.as_ref().unwrap().frequency,
            crate::roster::DROID_CHUNK_CHANCE
        );
        assert_eq!(
            registered
                .chunk
                .as_ref()
                .unwrap()
                .explosion
                .as_ref()
                .unwrap()
                .look
                .effects,
            ["openbf_droid_explosion"]
        );
        walk.sim.classes[1] = vec![registered];
        walk.teams[1].classes = vec![crate::walk::ClassVisual::from_roster(&roster)];
        let dirs = [root.join(format!("side_{game}/droid_chunks/weapons"))];
        let mut c = Chunks::load(&db, &[class.into()], &dirs, era);
        let specs = &c.defs[class];
        assert_eq!(specs.len(), 5);
        for (i, spec) in specs.iter().enumerate() {
            assert_eq!(spec.model, format!("{prefix}{}", i + 1));
            assert!(spec.node.is_some(), "{} launch bone", spec.model);
            assert!(matches!(spec.prim, Prim::Sphere(_)));
            assert_eq!(
                (spec.speed, spec.budget, spec.omega),
                (8.0, 4, Vec3::new(4.0, 4.0, 4.0))
            );
        }
        let spec = UnitSpec::trooper(1, Vec3::Y * 10.0);
        let h = walk.sim.spawn(&spec).unwrap();
        let mut bursts = 0;
        for _ in 0..1000 {
            walk.sim.units[h.index as usize] = spec.build(h.gen, &walk.sim.weapon_classes);
            let u = &mut walk.sim.units[h.index as usize];
            u.health.cur = -50.0;
            u.alive = false;
            u.body = true;
            walk.sim.step(&[]);
            c.clear();
            c.tick(&walk, None);
            if walk.sim.units[h.index as usize].burst {
                bursts += 1;
                assert_eq!(
                    c.len(),
                    5,
                    "{game} {class}: event must reach chunk renderer"
                );
                assert!(!walk.sim.units[h.index as usize].body);
            } else {
                assert_eq!(c.len(), 0, "a plain death must not launch chunks");
            }
        }
        assert!((554..=646).contains(&bursts), "{class}: {bursts}/1000");
        println!("{game} {class}: {bursts}/1000 deaths, five {prefix} pieces per burst");
        let control = if game == "bf1" {
            "cis_inf_assault"
        } else {
            "cis_inf_rocketeer"
        };
        assert_eq!(db.prop(control, "ChunkGeometryName"), Some("bdroid_chunk1"));
    }
}

#[test]
#[ignore = "requires bake droid-chunks from both discs"]
fn real_battle_droid_bursts_into_its_own_aliased_chunks() {
    let root = crate::assets::baked_root();
    for (game, era, class) in [
        ("bf1", Era::Bf1, "cis_inf_assault"),
        ("bf2", Era::Bf2, "cis_inf_rocketeer"),
    ] {
        let db = crate::roster::load_classdb(&root.join("classes"), game, &["cis", "rep"]).unwrap();
        let side = root.join(format!("side_{game}"));
        // The library's own order: the droid layer, then the side's weapons.
        let dirs = [side.join("droid_chunks/weapons"), side.join("weapons")];
        let c = Chunks::load(&db, &[class.into()], &dirs, era);
        let specs = &c.defs[class];
        assert_eq!(specs.len(), 5, "{game} {class}");
        for (i, spec) in specs.iter().enumerate() {
            assert_eq!(spec.model, format!("bdroid_chunk{}", i + 1));
            let glb = side.join(format!("droid_chunks/weapons/{}.glb", spec.model));
            let mesh = crate::assets::load_glb(&glb).unwrap();
            for (_, _, material, _, _) in &mesh.submeshes {
                let m = material.to_ascii_lowercase();
                assert!(m.starts_with("openbf_droid_"), "{game} {}: {m}", spec.model);
                assert!(pak::is_file(
                    side.join(format!("droid_chunks/textures/{m}.ktx2"))
                ));
            }
        }
        // The living droid's sheet is not the chunk bake's opaque copy.
        let living = std::fs::read(side.join("textures/cis_inf_battledroid.ktx2")).unwrap();
        let chunk =
            std::fs::read(side.join("droid_chunks/textures/openbf_droid_cis_inf_battledroid.ktx2"))
                .unwrap();
        assert_ne!(
            living, chunk,
            "{game}: living sheet replaced by the chunk's"
        );
    }
}

#[test]
fn a_droid_piece_uses_the_last_drawn_bone_frame_before_the_bind_fallback() {
    let mut s = spec(Prim::Sphere(0.2));
    s.node = Some(Mat4::from_translation(Vec3::Y));
    s.node_name = Some("bone_a_spine".into());
    s.local = Mat4::from_translation(Vec3::X);
    let mut c = chunks(s);
    let nodes = [(
        "bone_a_spine".into(),
        Mat4::from_rotation_translation(
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::Y * 3.0,
        ),
    )];
    c.launch_posed(
        "fixture",
        Vec3::X * 10.0,
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
        Some(&nodes),
    );
    assert!(piece(&c).pos.distance(Vec3::new(10.0, 4.0, 0.0)) < 1e-5);
    assert!(
        piece(&c)
            .model()
            .w_axis
            .truncate()
            .distance(Vec3::new(10.0, 3.0, 0.0))
            < 1e-5
    );
}

#[test]
fn a_piece_without_baked_launch_nodes_keeps_its_original_origin() {
    let mut s = spec(Prim::Sphere(0.2));
    s.node_name = Some("bone_a_spine".into());
    let mut c = chunks(s);
    let nodes = [("bone_a_spine".into(), Mat4::from_translation(Vec3::Y * 3.0))];
    c.launch_posed(
        "fixture",
        Vec3::X * 10.0,
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
        Some(&nodes),
    );
    assert_eq!(piece(&c).model().w_axis.truncate(), Vec3::X * 10.0);
}

/// A class db from in-memory classes, through the real loader.
fn db_of(defs: &[ClassDef]) -> ClassDb {
    let path = std::env::temp_dir().join(format!(
        "openbf-chunks-{}-{:?}.ron",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, ron::to_string(defs).unwrap()).unwrap();
    let mut db = ClassDb::default();
    db.load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    db
}

fn blast_db() -> ClassDb {
    let named = |name: &str, base: &str, props: &[(&str, &str)]| ClassDef {
        name: name.into(),
        base: base.into(),
        ..class(props)
    };
    db_of(&[
        named("tur_x", "turret", &[("ExplosionDestruct", "tur_x_exp")]),
        named(
            "tur_x_exp",
            "explosion",
            &[
                ("ChunkBounciness", "0.9"),
                ("ChunkGeometryName", "tur_x_chunk"),
                ("ChunkTerrainCollisions", "10"),
                ("ChunkTrailEffect", "trail"),
                ("NumChunks", "12"),
                ("ChunkSpeed", "10.0 13.0"),
                ("PushRadius", "5.0"),
            ],
        ),
        named(
            "tur_y_exp",
            "tur_x_exp",
            &[("ChunkSpeed", "-1 -1"), ("ChunkStartDistance", "2.0")],
        ),
    ])
}

/// A vehicle's death throws its own pieces only, on both ground and space
/// rules; its explosion's debris goes off with the explosion
/// ([`Chunks::blast`]), `NumChunks` of them, at most 10 (B2:288248).
#[test]
fn a_death_throws_its_own_pieces_and_the_explosion_its_debris() {
    let db = blast_db();
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    walk.vehicle_classes = vec!["tur_x".into()];
    walk.sim.events.push(Event::VehicleDestroyed {
        vehicle: core::vehicle::VehicleHandle { index: 0, gen: 0 },
        class: 0,
        pos: Vec3::Y * 10.0,
        vel: Vec3::ZERO,
        killer: None,
    });
    for space in [false, true] {
        let mut c = Chunks::load_for(&db, &["tur_x".into()], &[], Era::Bf2, space);
        c.tick(&walk, None);
        assert_eq!(c.len(), 0, "space {space}");
        c.blast("TUR_X_EXP", Vec3::Y * 10.0, None);
        assert_eq!(c.len(), 10, "space {space}");
    }
}

/// §6.1 with no impact normal: a set `ChunkSpeed` range throws each piece at
/// U(min, max) along normalize(2(½ − U), U + ½, 2(½ − U)), from the blast's
/// own point; the template keeps no bounce whatever the class authors
/// before its `ChunkGeometryName`, and the class's other numbers.
#[test]
fn explosion_debris_flies_up_and_out_within_its_speed_range() {
    let c = Chunks::load_for(&blast_db(), &[], &[], Era::Bf2, false);
    let b = &c.blasts["tur_x_exp"];
    assert_eq!(
        (b.count, b.speed, b.start, b.push_radius),
        (10, [10.0, 13.0], 0.0, 5.0)
    );
    assert_eq!((b.spec.bounce, b.spec.stick, b.spec.budget), (0.0, 0.0, 10));
    assert_eq!(b.spec.trails, vec![("trail".to_string(), None)]);
    let mut c = c;
    let at = Vec3::new(3.0, 10.0, -4.0);
    c.blast("tur_x_exp", at, None);
    for p in c.live.iter().flatten() {
        let speed = p.vel.length();
        assert!((10.0..=13.0).contains(&speed), "{speed}");
        let dir = p.vel / speed;
        assert!(
            dir.y > 0.0 && dir.y * 2.0 >= dir.x.abs().max(dir.z.abs()) * 0.5,
            "{dir}"
        );
        assert_eq!(p.pos, at);
        assert!(p.blast);
    }
}

/// With no speed range the pieces fly at U(1, 2)·PushRadius along
/// normalize(2(½ − U), 4(U + 1), 2(½ − U)), starting `ChunkStartDistance`
/// out along it; the child keeps its parent's template.
#[test]
fn explosion_debris_without_a_range_rides_the_push_radius() {
    let mut c = Chunks::load_for(&blast_db(), &[], &[], Era::Bf2, false);
    let at = Vec3::new(3.0, 10.0, -4.0);
    c.blast("tur_y_exp", at, None);
    assert_eq!(c.len(), 10);
    for p in c.live.iter().flatten() {
        let speed = p.vel.length();
        assert!((5.0..=10.0).contains(&speed), "{speed}");
        let dir = p.vel / speed;
        assert!(dir.y >= 4.0 / 4.5_f32.hypot(1.0) * 0.99, "{dir}");
        assert!((p.pos - (at + dir * 2.0)).length() < 1e-4);
    }
}

/// Explosion debris rests 5 s on both rules: settling starts its timer 5 s
/// short of the piece's life (B2:83830-83835).
#[test]
fn explosion_debris_rests_five_seconds() {
    for space in [false, true] {
        let mut c = Chunks::load_for(&blast_db(), &[], &[], Era::Bf2, space);
        let walk = flat();
        c.blast("tur_x_exp", Vec3::Y * 0.6, None);
        let mut rest = vec![None; c.len()];
        for _ in 0..3000 {
            c.tick(&walk, None);
            for (r, p) in rest.iter_mut().zip(c.live.iter().flatten()) {
                if r.is_none() && !p.moving {
                    *r = Some(p.life() - p.timer);
                }
            }
            if rest.iter().all(Option::is_some) {
                break;
            }
        }
        assert!(
            rest.iter()
                .all(|r| r.is_some_and(|r| (r - BLAST_REST).abs() < 0.05)),
            "space {space}: {rest:?}"
        );
    }
}

/// The BARC's `rep_hover_speederbike_exp` (BF2 side rep): 8 `com_smallchunk1`
/// at 10–13 m/s from the blast point, spun 4 4 4 with the
/// `com_sfx_chunktrail_xs` trail and no bounce.
#[test]
#[ignore = "requires the baked BF2 classes and common models"]
fn real_barc_explosion_throws_eight_small_chunks() {
    let root = crate::assets::baked_root();
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
    let c = Chunks::load_for(&db, &[], &[root.join("common_bf2/models")], Era::Bf2, false);
    let b = &c.blasts["rep_hover_speederbike_exp"];
    assert_eq!(
        (b.count, b.speed, b.start, b.push_radius),
        (8, [10.0, 13.0], 0.0, 5.0)
    );
    assert_eq!(b.spec.model, "com_smallchunk1");
    assert_eq!(b.spec.omega, Vec3::splat(4.0));
    assert_eq!(
        b.spec.trails,
        vec![("com_sfx_chunktrail_xs".to_string(), None)]
    );
    assert_eq!(b.spec.bounce, 0.0);
}

#[test]
fn ground_settle_ray_ignores_vehicle_hulls() {
    use core::collision::HullPart;
    for space in [false, true] {
        let mut s = spec(Prim::Sphere(0.5));
        s.gravity = Vec3::ZERO;
        let mut c = chunks(s);
        c.space = space;
        let mut walk = WalkSim::new(1, Vec3::ZERO, None);
        walk.sim.heightfield = Some(core::heightfield::Heightfield {
            size: 2,
            unit: 100.0,
            offset_x: 50.0,
            offset_z: 50.0,
            heights: vec![-100.0; 4],
        });
        let hull = HullPart::new(
            Mat4::IDENTITY,
            Shape::Box {
                half: Vec3::new(5.0, 0.5, 5.0),
            },
            Layers::SOLDIER | Layers::VEHICLE,
        );
        walk.sim.collision.set_hull(
            core::vehicle::VehicleHandle { index: 1, gen: 0 },
            &[hull],
            Mat4::IDENTITY,
        );
        drop(&mut c, Vec3::Y * 0.8, Vec3::ZERO);
        c.live[0][0].count = 1;
        c.tick(&walk, None);
        assert_eq!(piece(&c).moving, !space, "space {space}");
    }
}

#[test]
fn a_space_round_keeps_a_piece_eight_seconds_and_fades_its_last_two() {
    let mut s = spec(Prim::Sphere(0.5));
    s.gravity = Vec3::ZERO;
    s.speed = 0.03;
    let mut c = chunks(s);
    c.space = true;
    let walk = WalkSim::new(1, Vec3::ZERO, None);
    c.launch(
        "fixture",
        Vec3::Y * 50.0,
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
    );
    assert_eq!(piece(&c).life(), 8.0);
    let (mut t, mut want) = (0f32, 0);
    while t < 8.0 {
        t += core::TICK_DT;
        want += 1;
    }
    let mut ticks = 0;
    while !c.is_empty() {
        if ticks == 179 {
            assert_eq!(piece(&c).opacity(), 1.0);
        }
        if ticks == 210 {
            let o = piece(&c).opacity();
            assert!((o - 0.5).abs() < 0.02, "{o}");
        }
        c.tick(&walk, None);
        ticks += 1;
    }
    assert_eq!(ticks, want);
    let mut ground = chunks(spec(Prim::Sphere(0.5)));
    drop(&mut ground, Vec3::Y, Vec3::ZERO);
    assert_eq!(piece(&ground).life(), 3.0);
}

#[test]
fn a_whitespace_split_chunk_number_reads_its_leading_float_as_the_ps2_does() {
    let number = crate::vehicle_class::float;
    assert_eq!(number("4 .00"), Some(4.0));
    assert_eq!(number(" 4.5"), Some(4.5));
    assert_eq!(number("-1e-3"), Some(-1e-3));
    assert_eq!(number("+0.25"), Some(0.25));
    assert_eq!(number("full"), None);
    assert_eq!(number(""), None);
}

#[test]
fn authored_omega_turns_with_the_launch_frame_as_the_ps2_cruiser_piece_does() {
    let mut s = spec(Prim::Box(Vec3::new(80.0, 40.0, 140.0)));
    s.gravity = Vec3::ZERO;
    s.omega = Vec3::new(0.03, 0.01, 0.01);
    let mut c = chunks(s);
    c.space = true;
    let frame = Mat3::from_cols(
        Vec3::new(0.98772, -0.15626, -0.00033),
        Vec3::new(0.15626, 0.98772, 0.001),
        Vec3::new(0.00017, -0.00104, 1.0),
    );
    let rot = Quat::from_mat3(&frame).normalize();
    c.launch("fixture", Vec3::ZERO, Vec3::ZERO, rot, 1.0, 1.0, None);
    let p = piece(&c);
    assert!(
        (p.omega - Vec3::new(0.03119, 0.00518, 0.01)).length() < 2e-4,
        "{}",
        p.omega
    );
    let walk = WalkSim::new(1, Vec3::ZERO, None);
    let mut plain = chunks({
        let mut s = spec(Prim::Sphere(1.0));
        s.gravity = Vec3::ZERO;
        s.omega = Vec3::new(0.03, 0.01, 0.01);
        s
    });
    plain.space = true;
    plain.launch(
        "fixture",
        Vec3::ZERO,
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
    );
    plain.tick(&walk, None);
    let x = piece(&plain).model().x_axis.truncate();
    assert!(
        (x.y - 0.000333).abs() < 2e-5 && (x.z + 0.000333).abs() < 2e-5,
        "{x}"
    );
}

#[test]
fn map_frigate_chunk_specs_keep_authored_prims_and_omega_across_all_maps() {
    let root = crate::assets::baked_root();
    let maps = [
        (
            "bf2_spa1",
            [
                ("all_prop_blockaderunner", 2),
                ("spa1_prop_minidestroyer2", 3),
                ("spa1_prop_correliancruiser", 4),
            ]
            .as_slice(),
        ),
        (
            "bf2_spa3",
            [
                ("spa2_prop_minifedcruiser", 4),
                ("spa2_prop_minirepassaultship", 5),
            ]
            .as_slice(),
        ),
        (
            "bf2_spa6",
            [
                ("spa2_prop_minifedcruiser", 4),
                ("spa2_prop_minirepassaultship", 5),
            ]
            .as_slice(),
        ),
        (
            "bf2_spa7",
            [
                ("spa2_prop_minifedcruiser", 4),
                ("spa2_prop_minirepassaultship", 5),
            ]
            .as_slice(),
        ),
        (
            "bf2_spa8",
            [
                ("spa1_prop_minidestroyer2", 3),
                ("spa1_prop_correliancruiser", 4),
            ]
            .as_slice(),
        ),
        (
            "bf2_spa9",
            [
                ("all_prop_blockaderunner", 2),
                ("spa1_prop_minidestroyer2", 3),
            ]
            .as_slice(),
        ),
    ];
    if !maps
        .iter()
        .all(|(map, _)| root.join(map).join("manifest.ron").is_file())
    {
        assert!(
            std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
            "space assets required"
        );
        eprintln!("skipping map frigate prims: local baked assets unavailable");
        return;
    }
    for (map_name, classes) in maps {
        let map = root.join(map_name);
        let mut db = ClassDb::default();
        db.load_map(&map).unwrap();
        for (name, piece_count) in classes {
            let c = Chunks::load(&db, &[(*name).into()], &[map.join("models")], Era::Bf2);
            let specs = c.defs.get(*name).expect("map frigate chunk definitions");
            let slots = chunk_properties(db.chain(name));
            assert_eq!(specs.len(), *piece_count, "{map_name} {name} piece count");
            assert_eq!(slots.len(), *piece_count, "{map_name} {name} slot count");
            for (slot, spec) in slots.iter().zip(specs) {
                let omega = floats(slot.props[&hash("ChunkOmega")].as_str());
                assert_eq!(omega.len(), 3, "{map_name} {name} authored omega");
                let expected = Vec3::new(omega[0], omega[1], omega[2]);
                assert!(
                    (spec.omega - expected).length() < 1e-5,
                    "{map_name} {name} omega: {:?} != {expected}",
                    spec.omega
                );
                assert!(
                    !matches!(spec.prim, Prim::Sphere(_)),
                    "{map_name} {name} {} must use its authored map prim",
                    spec.model
                );
            }
            if *name == "spa2_prop_minifedcruiser" {
                assert_eq!(specs[0].speed, 4.0, "{map_name} ChunkSpeed = 4 .00");
            }
            if map_name == "bf2_spa8" && *name == "spa1_prop_correliancruiser" {
                let Prim::Box(half) = specs[0].prim else {
                    panic!("expected the authored box prim, got {:?}", specs[0].prim);
                };
                assert!((half - Vec3::new(64.03713, 10.072614, 108.157265)).length() < 1e-3);
                assert!(
                    (specs[0].local.w_axis.truncate() - Vec3::new(5.107916, 19.034204, 72.342255))
                        .length()
                        < 1e-3
                );
                assert!((specs[0].local.x_axis.truncate().y + 0.15658604).abs() < 1e-5);
            }
        }
    }
}
