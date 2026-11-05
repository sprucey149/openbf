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
        c.draws(Era::Bf2, Vec3::ZERO, Vec3::Z, 1.0, &mut Assets, &mut out);
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
        while c.len() > 0 {
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
                (8.0, 4, Vec3::new(-4.0, -4.0, 4.0))
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
