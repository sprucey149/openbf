use core::vehicle::{gun::GunState, SeatKind, Vehicle, VehicleKind, VehicleSpec};
use game::skin::{BoneXform, Clip, Skeleton};
use game::vehicle_class::{seats, SeatSpec};
use game::viewer::walker_pose::{skin, WalkerRig};
use glam::{Mat4, Quat, Vec3};

fn specs(props: &[(&str, &str)]) -> Vec<SeatSpec> {
    let def = manifest::ClassDef {
        name: "test_vehicle".into(),
        kind: "entc".into(),
        base: "walker".into(),
        props: props
            .iter()
            .map(|(k, v)| (game::classdb::fnv1a_lowercase(k), v.to_string()))
            .collect(),
    };
    let path = std::env::temp_dir().join(format!(
        "walker67-{}-{:?}.ron",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, ron::to_string(&vec![def]).unwrap()).unwrap();
    let mut db = game::classdb::ClassDb::default();
    db.load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    seats(&db, "test_vehicle")
}

fn vehicle() -> Vehicle {
    VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    }
    .build(1)
}

fn aim(v: &mut Vehicle, seat: usize, yaw: f32, pitch: f32) {
    v.guns.resize_with(2, || None);
    v.guns[seat] = Some(GunState {
        look: (yaw, pitch),
        aim: (yaw, pitch),
        rate: (0.0, 0.0),
        rel: (0.0, 0.0),
        weapons: Vec::new(),
        zoom: None,
    });
}

fn rig(spec: Vec<SeatSpec>) -> WalkerRig {
    let names = [
        "cube5",
        "barrel",
        "leg",
        "neck",
        "bone_root",
        "hp_pilot",
        "mouth",
    ];
    let local = [
        Vec3::Y,
        Vec3::Z,
        Vec3::NEG_Y,
        Vec3::Y * 2.0,
        Vec3::ZERO,
        Vec3::new(0.0, 0.2, -0.4),
        Vec3::Z * 0.5,
    ]
    .map(Mat4::from_translation);
    let skel = Skeleton::new(
        names.map(String::from).to_vec(),
        vec![Some(3), Some(6), Some(4), Some(4), None, Some(0), Some(0)],
        local.to_vec(),
    );
    let mut tracks = vec![None; names.len()];
    tracks[2] = Some(
        [0.3, 0.6]
            .map(|angle| BoneXform {
                rotation: Quat::from_rotation_x(angle),
                translation: Vec3::NEG_Y,
            })
            .to_vec(),
    );
    tracks[3] = Some(vec![
        BoneXform {
            rotation: Quat::IDENTITY,
            translation: Vec3::new(0.15, 2.2, 0.0)
        };
        2
    ]);
    let clip = Clip {
        name: "walk".into(),
        fps: 30.0,
        frames: 2,
        tracks,
    };
    let walker = manifest::ManifestWalker {
        bank: "t".into(),
        clips: vec![Some(manifest::ManifestWalkerClip {
            name: "walk".into(),
            frames: 2,
            duration: 1.0 / 30.0,
            looping: true,
            root_z: vec![0.0; 2],
        })],
        prims: vec![],
        skin: Some("t".into()),
        layer_bones: vec![],
    };
    WalkerRig::new(skel, vec![clip], &walker).with_seats(spec)
}

fn atst() -> WalkerRig {
    rig(specs(&[
        ("AimerNodeName", "cube5"),
        ("AimerYawLimits", "-90 90"),
        ("AimerPitchLimits", "-5 20"),
        ("AimerNodeName", "mouth"),
        ("AimerYawLimits", "0 0"),
        ("AimerPitchLimits", "-30 20"),
        ("BarrelNodeName", "barrel"),
    ]))
}

#[test]
fn piloted_atst_left_sixty_moves_head_vertices_and_keeps_animated_legs() {
    let r = atst();
    let mut v = vehicle();
    v.seats[0].occupant = Some(core::unit::UnitHandle { index: 0, gen: 1 });
    aim(&mut v, 0, 60f32.to_radians(), 0.0);
    let plain = r.palette(&v.walker, 1.0);
    let posed = r.aimed_palette(&v, 1.0);
    assert_eq!(plain[2], posed[2]);
    assert_ne!(plain[2], Mat4::IDENTITY);
    let head = r.node_now(&v, 1.0, "cube5").unwrap();
    assert!((head.z_axis.x.atan2(head.z_axis.z).to_degrees() - 60.0).abs() < 0.1);
    let p = (r.skeleton.global_bind[0] * Vec3::Z.extend(1.0))
        .truncate()
        .to_array();
    let (a, _) = skin(&plain, &[p], &[[0.0, 0.0, 1.0]], &[[0, 0, 0, 0]]);
    let (b, _) = skin(&posed, &[p], &[[0.0, 0.0, 1.0]], &[[0, 0, 0, 0]]);
    assert!(Vec3::from(a[0]).distance(Vec3::from(b[0])) > 0.9);
}

#[test]
fn parent_first_barrel_and_rider_use_walk_posed_head() {
    let r = atst();
    let mut v = vehicle();
    aim(&mut v, 0, 1.0, 0.1);
    let now = r.nodes(&v, 1.0);
    assert!(now[0]
        .w_axis
        .truncate()
        .abs_diff_eq(Vec3::new(0.15, 3.2, 0.0), 1e-5));
    let b = game::viewer::turret_pose::barrel_node(&now[6], Vec3::Z, 0.0);
    assert!(now[1].abs_diff_eq(b, 1e-5));
    assert!(now[5].abs_diff_eq(now[0] * r.skeleton.local_bind[5], 1e-5));
    assert_eq!(r.node_now(&v, 1.0, "hp_pilot").unwrap(), now[5]);
}

#[test]
fn aimer_limits_clamp_in_the_animated_parent_frame() {
    let r = atst();
    let mut v = vehicle();
    v.walker.h = (0.2f32.cos(), -0.2f32.sin());
    for (yaw, pitch, want_y, want_p) in [(2.0, 0.9, 90.0, 20.0), (-2.4, -0.9, -90.0, -5.0)] {
        aim(&mut v, 0, yaw, pitch);
        let now = r.nodes(&v, 1.0);
        let frame = now[3] * r.skeleton.local_bind[0];
        let d = frame
            .inverse()
            .transform_vector3(now[0].z_axis.truncate())
            .normalize();
        assert!((d.x.atan2(d.z).to_degrees() - want_y).abs() < 1e-3);
        assert!((d.y.asin().to_degrees() - want_p).abs() < 1e-3);
    }
}

#[test]
fn never_aimed_and_zero_aim_palettes_are_bit_identical_to_walk() {
    let r = atst();
    let mut v = vehicle();
    for angle in [0.0f32, 0.2, -0.4] {
        v.walker.h = ((angle * 0.5).cos(), (angle * 0.5).sin());
        v.walker.pitch = 0.08;
        v.walker.clip_time = 0.02;
        assert_eq!(r.aimed_palette(&v, 1.1), r.palette(&v.walker, 1.1));
        aim(&mut v, 0, 0.0, 0.0);
        assert_eq!(r.aimed_palette(&v, 1.1), r.palette(&v.walker, 1.1));
        v.guns.clear();
    }
}

#[test]
fn bf1_atat_body_aimer_wins_over_gunner_turret_on_same_head() {
    let r = rig(specs(&[
        ("AimerNodeName", "cube5"),
        ("AimerYawLimits", "-90 90"),
        ("WalkerSection", "TURRET1"),
        ("TurretNodeName", "cube5"),
        ("PitchTurnFactor", "0"),
        ("AimerNodeName", "mouth"),
    ]));
    let mut v = vehicle();
    aim(&mut v, 0, 0.7, 0.0);
    aim(&mut v, 1, -0.6, 0.0);
    let d = r.node_now(&v, 1.0, "cube5").unwrap().z_axis.truncate();
    assert!((d.x.atan2(d.z) - 0.7).abs() < 0.001);
}

#[test]
fn parser_keeps_each_aimer_limits_and_barrels_in_its_section() {
    let s = specs(&[
        ("AimerNodeName", "head"),
        ("AimerYawLimits", "-90 90"),
        ("AimerNodeName", "chin"),
        ("AimerYawLimits", "0 0"),
        ("BarrelNodeName", "barrel"),
        ("BarrelRecoil", "0.3"),
        ("WalkerSection", "TURRET2"),
        ("AimerNodeName", "ear"),
        ("AimerPitchLimits", "-20 35"),
    ]);
    assert_eq!(s[0].aimers.len(), 2);
    assert_eq!(s[0].aimers[0].yaw, [-90.0, 90.0]);
    assert_eq!(s[0].aimers[1].yaw, [0.0, 0.0]);
    assert_eq!(s[0].aimers[1].barrels, vec![("barrel".into(), 0.3)]);
    assert_eq!(s[1].aimers[0].pitch, [-20.0, 35.0]);
    assert_eq!(s[0].aimer_yaw_limits, Some([-90.0, 90.0]));
}

#[test]
fn drawing_does_not_mutate_vehicle_hash() {
    let r = atst();
    let mut v = vehicle();
    aim(&mut v, 0, 1.0, 0.2);
    let hash = |v: &Vehicle| {
        let mut bytes = Vec::new();
        v.hash_into(&mut |b| bytes.extend_from_slice(b));
        bytes
    };
    let before = hash(&v);
    for _ in 0..4 {
        r.aimed_palette(&v, 1.0);
        r.node_now(&v, 1.0, "barrel");
    }
    assert_eq!(before, hash(&v));
}

#[test]
#[ignore = "requires local disc bakes"]
fn real_atte_mount_and_all_walker_class_aimers() {
    let root = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&root.join("bf2_geo1"), "con").unwrap();
    for (game, side, class, node) in [
        ("bf1", "imp", "imp_walk_atst", "cube5"),
        ("bf1", "imp", "imp_walk_atat", "aimer_head"),
        ("bf1", "cis", "cis_walk_spider", "turret_y_1"),
        ("bf1", "rep", "rep_walk_atte", "aimer_maincannon_x"),
        ("bf2", "imp", "imp_walk_atst", "cube5"),
        ("bf2", "imp", "imp_walk_atat", "aimer_head"),
        ("bf2", "cis", "cis_walk_spider", "turret_y_1"),
        ("bf2", "rep", "rep_walk_atte", "aimer_maincannon_x"),
        ("bf2", "rep", "rep_walk_oneman_atst", "aimer_x"),
    ] {
        let mut m = scene.manifest.clone();
        m.source_game = game.into();
        let mut db = game::classdb::ClassDb::default();
        db.load(&root.join(format!("classes/{game}_{side}.ron")))
            .unwrap();
        let geometry = db.prop(class, "GeometryName").unwrap().to_string();
        let rigs = game::viewer::walker_rigs(&m, &root, &[Some(geometry)]);
        let rig = rigs[0]
            .as_ref()
            .unwrap_or_else(|| panic!("missing {game} {class}"));
        let mut v = vehicle();
        aim(&mut v, 0, 0.8, 0.1);
        aim(&mut v, 1, 0.6, 0.2);
        let n = rig
            .skeleton
            .index_of(node)
            .unwrap_or_else(|| panic!("missing {class} {node}"));
        let now = rig.nodes(&v, 1.0);
        assert!(now[n].is_finite(), "{class}");
        assert_ne!(now[n], rig.skeleton.global_bind[n], "{game} {class}");
        if class == "rep_walk_atte" {
            for aimed in [false, true] {
                if !aimed {
                    v.guns.clear();
                } else {
                    aim(&mut v, 1, 0.6, 0.2);
                }
                let now = rig.nodes(&v, 1.0);
                let parent = rig.skeleton.parents[n].unwrap();
                let local = now[parent]
                    .inverse()
                    .transform_point3(now[n].w_axis.truncate());
                assert!(
                    local.abs_diff_eq(rig.skeleton.local_bind[n].w_axis.truncate(), 1e-4),
                    "{game} {local:?}"
                );
            }
        }
    }
}

#[test]
fn walker_vertices_move_between_ticks_and_tick_state_restores_exactly() {
    let r = atst();
    let mut walk = game::walk::WalkSim::new(1, Vec3::ZERO, None);
    let mut v = vehicle();
    aim(&mut v, 0, 0.2, 0.1);
    v.walker.clip_time = 0.0;
    walk.sim.vehicles.push(v);
    let mut blend = game::render_blend::Blend::default();
    blend.on = true;
    blend.before_tick(&walk);
    let v = &mut walk.sim.vehicles[0];
    v.walker.clip_time = 1.0 / 30.0;
    v.walker.h = (0.2f32.cos(), 0.2f32.sin());
    v.walker.roll = (0.1f32.cos(), 0.1f32.sin());
    v.walker.pitch = 0.08;
    v.walker.layer_blend[0] = 0.5;
    v.guns[0].as_mut().unwrap().aim = (0.8, 0.3);
    let hash = walk.sim.hash();
    let tick = game::render_blend::Pose::capture(&walk);
    let mut previous = None;
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert!(blend.apply(&mut walk, t));
        let v = &walk.sim.vehicles[0];
        assert!((v.walker.clip_time - t / 30.0).abs() < 1e-6);
        let p = r.aimed_palette(v, 1.0)[2].transform_point3(Vec3::new(0.0, -2.0, 0.0));
        if let Some(old) = previous {
            assert!(p.distance(old) > 0.01);
        }
        previous = Some(p);
        blend.restore(&mut walk);
        assert_eq!(game::render_blend::Pose::capture(&walk), tick);
        assert_eq!(walk.sim.hash(), hash);
    }
}

#[test]
fn a_new_clip_or_vehicle_never_interpolates_an_unrelated_clock() {
    let mut walk = game::walk::WalkSim::new(1, Vec3::ZERO, None);
    walk.sim.vehicles.push(vehicle());
    let mut blend = game::render_blend::Blend::default();
    blend.on = true;
    walk.sim.vehicles[0].walker.clip_time = 0.8;
    blend.before_tick(&walk);
    walk.sim.vehicles[0].walker.anim = 1;
    walk.sim.vehicles[0].walker.clip_time = 0.02;
    let hash = walk.sim.hash();
    blend.apply(&mut walk, 0.5);
    assert_eq!(walk.sim.vehicles[0].walker.clip_time, 0.02);
    blend.restore(&mut walk);
    assert_eq!(walk.sim.hash(), hash);
}

fn aat_turret() -> Vec<SeatSpec> {
    specs(&[
        ("FLYERSECTION", "TURRET1"),
        ("TurretNodeName", "cube5"),
        ("BarrelNodeName", "barrel"),
        ("BarrelRecoil", "0.75"),
        ("FirePointName", "hp_pilot"),
        ("WeaponName", "test_launcher"),
        ("AimerNodeName", "mouth"),
        ("AimerPitchLimits", "-30 90"),
        ("AimerYawLimits", "0 0"),
    ])
}

#[test]
fn a_barrel_authored_before_its_weapon_is_that_weapons_aimers() {
    let seats = aat_turret();
    let gunner = &seats[1];
    assert_eq!(gunner.aimers.len(), 1);
    let a = &gunner.aimers[0];
    assert_eq!((a.node.as_str(), a.weapon), ("mouth", 0));
    assert_eq!(a.barrels, vec![("barrel".to_string(), 0.75)]);
    assert_eq!(a.muzzles(), vec!["hp_pilot"]);
}

#[test]
fn a_fired_vehicle_barrel_slides_back_by_its_state_times_its_recoil() {
    let r = rig(aat_turret());
    let mut v = vehicle();
    aim(&mut v, 1, 0.4, 0.2);
    let barrel = r.skeleton.index_of("barrel").unwrap();
    let mouth = r.skeleton.index_of("mouth").unwrap();
    let node = |state: f32| {
        let kick = move |seat: usize, weapon: usize, point: u64, len: u64| {
            if (seat, weapon, point, len) == (1, 0, 0, 1) {
                state
            } else {
                0.0
            }
        };
        let p = r.aimed_palette_kicked(&v, 1.0, &kick);
        (
            p[barrel] * r.skeleton.global_bind[barrel],
            p[mouth] * r.skeleton.global_bind[mouth],
        )
    };
    let (rest, aimer) = node(0.0);
    assert_eq!(
        r.aimed_palette(&v, 1.0)[barrel] * r.skeleton.global_bind[barrel],
        rest
    );
    for (state, slide) in [(1.0, 0.75), (0.5, 0.375), (-3.0, 0.0)] {
        let (kicked, _) = node(state);
        let moved = kicked.w_axis.truncate() - rest.w_axis.truncate();
        let back = -aimer.z_axis.truncate().normalize() * slide;
        assert!(moved.abs_diff_eq(back, 1e-5), "{state}: {moved} vs {back}");
        assert!(kicked.z_axis.abs_diff_eq(aimer.z_axis, 1e-6));
    }
}

#[test]
#[ignore = "requires local disc bakes"]
fn real_aat_turret_and_side_gun_barrels_recoil_when_fired() {
    let root = game::assets::baked_root();
    let (scene, _) = game::assets::load_battle_scene(&root.join("bf2_myg1"), "con").unwrap();
    let mut db = game::classdb::ClassDb::default();
    db.load(&root.join("classes/bf2_cis.ron")).unwrap();
    let geometry = db
        .prop("cis_hover_aat", "GeometryName")
        .unwrap()
        .to_string();
    let rigs = game::viewer::hover_rigs(&scene.manifest, &root, &[Some(geometry)]);
    let rig = rigs[0].as_ref().expect("cis_hover_aat rig");
    let mut v = vehicle();
    aim(&mut v, 0, 0.1, 0.05);
    aim(&mut v, 1, 0.6, 0.2);
    let at = |kick: &dyn Fn(usize, usize, u64, u64) -> f32, name: &str| {
        let n = rig.skeleton.index_of(name).unwrap();
        (rig.hover_palette_kicked(&v, None, kick)[n] * rig.skeleton.global_bind[n])
            .w_axis
            .truncate()
    };
    for (seat, point, len, barrel, recoil) in [
        (1, 0, 1, "recoil_3", 0.75),
        (0, 0, 2, "recoil_1", 0.25),
        (0, 1, 2, "recoil_2", 0.25),
    ] {
        let rest = at(&|_, _, _, _| 0.0, barrel);
        let fired = at(
            &move |s, w, p, l| {
                if (s, w, p, l) == (seat, 0, point, len) {
                    1.0
                } else {
                    0.0
                }
            },
            barrel,
        );
        assert!(
            (rest.distance(fired) - recoil).abs() < 1e-4,
            "{barrel}: {}",
            rest.distance(fired)
        );
    }
}

/// BF2's AT-ST passenger section, as authored: its aimer keys come before
/// the weapon, `MountPos` places the turret and key 0x865061fc makes it
/// look along the walker.
#[test]
fn the_atst_passenger_seat_reads_its_mount_its_eye_and_its_early_aimer() {
    let def = |extra: &[(&str, &str)]| {
        let mut p = vec![
            ("WALKERSECTION", "TURRET1"),
            ("TrackCenter", "0.0 9.25 -3.0"),
            ("EyePointOffset", "0.0 0.0 0.0"),
            ("TrackOffset", "0.0 1.0 5.0"),
            ("AimerYawLimits", "0 0"),
            ("AimerPitchLimits", "0 0"),
            ("AimerNodeName", "aimer_Reargun"),
            ("WeaponName", "test_guided_rocket"),
        ];
        p.extend_from_slice(extra);
        specs(&p)
    };
    let seats = def(&[]);
    let s = &seats[1];
    assert_eq!(s.mount_pos, None);
    assert!(!s.follows_owner);
    assert_eq!(s.eye_point_offset, Some([0.0; 3]));
    assert_eq!(s.track_center, Some([0.0; 3]));
    assert_eq!(s.aimers.len(), 1);
    let a = &s.aimers[0];
    assert_eq!(
        (a.node.as_str(), a.yaw, a.pitch),
        ("aimer_Reargun", [0.0; 2], [0.0; 2])
    );
    let seats = def(&[("MountPos", "0.0 8.0 0.0")]);
    assert_eq!(seats[1].mount_pos, Some([0.0, 8.0, 0.0]));
    let mut props = vec![("WALKERSECTION", "TURRET1"), ("MountPos", "0 8 0")];
    props.push(("TurretNodeName", "cube5"));
    assert_eq!(specs(&props)[1].mount_pos, None);
}
