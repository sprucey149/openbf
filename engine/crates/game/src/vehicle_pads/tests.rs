use super::*;

#[test]
fn unsupported_faction_does_not_hide_the_other_teams_pad_key() {
    let key = fnv1a_lowercase("ClassImpDef");
    assert_eq!(class_key_team(key, &["gun".into(), "imp".into()]), Some(1));
    assert_eq!(
        class_key_team(
            fnv1a_lowercase("ClassEmpire"),
            &["imp".into(), "gun".into()]
        ),
        Some(0)
    );
    assert_eq!(class_key_team(key, &["gun".into(), "ewk".into()]), None);
    assert_eq!(
        class_key_team(
            fnv1a_lowercase("ClassNeutral"),
            &["gun".into(), "imp".into()]
        ),
        None
    );
}

#[test]
fn aimer_overrides_keep_defaults_sort_limits_and_set_rates_independently() {
    let default = Aimer::default();
    let a = aimer(None, None, None, None);
    assert_eq!(
        (a.yaw, a.pitch, a.yaw_rate, a.pitch_rate),
        (
            default.yaw,
            default.pitch,
            default.yaw_rate,
            default.pitch_rate
        )
    );
    let d = std::f32::consts::PI / 180.0;
    let a = aimer(Some([40.0, -20.0]), Some([15.0, -5.0]), Some(7.0), None);
    assert_eq!(a.yaw, [-20.0 * d, 40.0 * d]);
    assert_eq!(a.pitch, [-5.0 * d, 15.0 * d]);
    assert_eq!((a.yaw_rate, a.pitch_rate), (7.0, default.pitch_rate));
    let a = aimer(None, None, None, Some(9.0));
    assert_eq!((a.yaw_rate, a.pitch_rate), (default.yaw_rate, 9.0));
}

#[test]
fn duplicate_side_classes_select_each_fields_first_present_value() {
    let vm = |vehicles: &str| {
        ron::from_str::<manifest::VehicleManifest>(&format!(r#"(
        source: "test", vehicles: [{vehicles}], geometries: [], models: [], textures: [], collision: []
    )"#)).unwrap()
    };
    let row = |name: &str, sphere: &str, floor: &str| {
        format!(
            r#"(
        class: (name: "{name}", base: "hover", geometry: None), chunks: [],
        sphere: {sphere}, vis_min_y: {floor}
    )"#
        )
    };
    let sides = vec![
        ("a".into(), vm(&row("Shared", "None", "Some(-2.0)"))),
        (
            "b".into(),
            vm(&row("shared", "Some((1.0, 2.0, 3.0, 4.0))", "Some(-8.0)")),
        ),
        (
            "c".into(),
            vm(&row("SHARED", "Some((5.0, 6.0, 7.0, 8.0))", "None")),
        ),
    ];
    assert_eq!(vis_floors_from(&sides)["shared"], -2.0);
    assert_eq!(spheres_from(&sides)["shared"], [1.0, 2.0, 3.0, 4.0]);
    assert!(walker_bakes_from(&sides).is_empty());
}

/// Card HTGT: a turret's acquisition sphere is the box around its own
/// building's bodies only, its centre in the turret's turned frame.
#[test]
fn a_turrets_sphere_bounds_its_own_building() {
    use core::collision::{Body, CollisionWorld, Shape};
    let body = |at: Vec3, instance: u32| {
        let mut b = Body::new(
            glam::Mat4::from_translation(at),
            Shape::Box {
                half: Vec3::splat(1.0),
            },
        );
        b.instance = Some(instance);
        b
    };
    let world = CollisionWorld::new(
        vec![
            body(Vec3::new(10.0, 1.0, 0.0), 5),
            body(Vec3::new(14.0, 1.0, 0.0), 5),
            body(Vec3::new(40.0, 1.0, 0.0), 6),
        ],
        Vec::new(),
    );
    let turn = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let (c, r) = building_sphere(&world, 5, Vec3::new(10.0, 0.0, 0.0), turn).unwrap();
    // The box (9..15, 0..2, -1..1): centre (12, 1, 0), half-diagonal.
    assert!((turn * c - Vec3::new(2.0, 1.0, 0.0)).length() < 1e-5, "{c}");
    assert!((r - Vec3::new(6.0, 2.0, 2.0).length() * 0.5).abs() < 1e-5);
    assert_eq!(building_sphere(&world, 7, Vec3::ZERO, turn), None);
}

/// Card VTURB: an armed building's `BuildingSection` lines split its
/// ODF like a vehicle's; its ring is the gunner sections alone, its hull
/// a building's; it stands at its instance with the instance's turn and
/// team; a class spelled in another case still resolves; an armed
/// building with no section (BF2 POL1's shields) is no turret.
#[test]
fn an_armed_building_instance_becomes_a_gunner_only_turret() {
    use crate::classdb::ClassDef;
    use core::vehicle::SeatKind;
    use core::weapon::HealthType;
    let k = fnv1a_lowercase;
    let def = |name: &str, props: &[(&str, &str)]| ClassDef {
        name: name.into(),
        kind: "entc".into(),
        base: "armedbuilding".into(),
        props: props.iter().map(|(a, b)| (k(a), b.to_string())).collect(),
    };
    let classes = vec![
        def(
            "t_GunTurret",
            &[
                ("BuildingSection", "BODY"),
                ("MaxHealth", "300.0"),
                ("YawLimits", "-1 1"),
                ("BuildingSection", "TURRET1"),
                ("YawLimits", "-180.0 180.0"),
                ("WeaponName", "t_weap"),
                ("FirePointName", "hp_fire"),
            ],
        ),
        def(
            "t_shield",
            &[("BuildingSection", "BODY"), ("MaxHealth", "50")],
        ),
        {
            let mut d = def(
                "t_buildable",
                &[("BuildingSection", "TURRET1"), ("WeaponName", "t_weap")],
            );
            d.props
                .push((manifest::BUILD_ZONE_PROP, "com_prop_buildzone".into()));
            d
        },
    ];
    let path = std::env::temp_dir().join(format!("openbf-vturb-{}.ron", std::process::id()));
    std::fs::write(&path, ron::to_string(&classes).unwrap()).unwrap();
    let mut db = ClassDb::default();
    db.load(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let ring = vehicle_class::seats(&db, "t_GunTurret");
    assert_eq!(ring.len(), 1, "{ring:#?}");
    assert_eq!(ring[0].kind, SeatKind::Gunner(0));
    assert_eq!(ring[0].yaw_limits, Some([-180.0, 180.0]), "TURRET1's own");
    assert_eq!(ring[0].fire_point.as_deref(), Some("hp_fire"));
    assert_eq!(
        vehicle_class::health_type(&db, "t_GunTurret"),
        HealthType::Building
    );

    // Turned 90° (the model's +x along world −z), PS2 team 2.
    let inst = |class: &str, team: &str| ManifestInstance {
        class: class.into(),
        name: "gun1".into(),
        rotation: [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        translation: [10.0, 2.0, -5.0],
        props: vec![(k("Team"), team.into())],
    };
    let mut out = PadBuild::default();
    add_turrets(
        &mut out,
        &[
            inst("t_gunturret", "2"),
            inst("t_shield", "0"),
            inst("T_GUNTURRET", "0"),
        ],
        &db,
        true,
    );
    assert_eq!(out.classes, ["t_GunTurret"]);
    assert_eq!(out.geometry, [None], "the world draws the building");
    assert_eq!(out.turrets.len(), 2, "the shield has no seat");
    let t = &out.turrets[0];
    assert_eq!(t.spec.kind, VehicleKind::Turret);
    assert_eq!(t.spec.seats, [SeatKind::Gunner(0)]);
    assert_eq!(t.spec.pos, Vec3::new(10.0, 2.0, -5.0));
    assert_eq!((t.spec.max_health, t.spec.home_team), (300.0, Some(1)));
    assert_eq!(out.turrets[1].spec.home_team, None);
    // Each knows its instance in the manifest (its building's bodies).
    let of: Vec<u32> = out.turrets.iter().map(|t| t.instance).collect();
    assert_eq!(of, [0, 2]);
    assert!((t.spec.yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert!((t.rot * Vec3::X).distance(Vec3::new(0.0, 0.0, -1.0)) < 1e-5);
    let v = t.spec.build(1);
    assert!(v.pilot_seat().is_none());
    assert!(v.rot.angle_between(t.rot) < 1e-4, "yaw and matrix agree");

    // Both games retain a repair target; only BF2 starts down.
    for zones in [true, false] {
        let mut out = PadBuild::default();
        add_turrets(&mut out, &[inst("t_buildable", "0")], &db, zones);
        assert_eq!(out.turrets.len(), 1, "unbuilt_zones {zones}");
        assert_eq!(out.turrets[0].unbuilt, zones);
    }
}

fn gcw() -> [String; 2] {
    ["all".into(), "imp".into()]
}

/// The side table as re_check word 0x5183a0 decodes it: Atk only for
/// team 1, Def only for team 2, the full side name for either.
#[test]
fn atk_is_team_one_and_def_is_team_two() {
    let k = fnv1a_lowercase;
    let f = gcw();
    assert_eq!(class_key_team(k("ClassAllAtk"), &f), Some(0));
    assert_eq!(class_key_team(k("ClassAllDef"), &f), None);
    assert_eq!(class_key_team(k("ClassImpDef"), &f), Some(1));
    assert_eq!(class_key_team(k("ClassImpAtk"), &f), None);
    assert_eq!(class_key_team(k("ClassEmpire"), &f), Some(1));
    assert_eq!(class_key_team(k("ClassAlliance"), &f), Some(0));
    assert_eq!(class_key_team(k("ClassRepAtk"), &f), None, "CW keys");
    assert_eq!(class_key_team(k("ClassCISDef"), &f), None);
    assert_eq!(class_key_team(k("ClassLocals"), &f), None);
    let cw: [String; 2] = ["rep".into(), "cis".into()];
    assert_eq!(class_key_team(k("ClassCISDef"), &cw), Some(1));
    assert_eq!(class_key_team(k("ClassRepAtk"), &cw), Some(0));
}
