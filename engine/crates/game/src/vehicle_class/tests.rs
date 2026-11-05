use super::*;

fn fixture_db(tag: &str, defs: Vec<ClassDef>) -> ClassDb {
    let path = std::env::temp_dir().join(format!("openbf-{tag}-{}.ron", std::process::id()));
    std::fs::write(&path, ron::to_string(&defs).unwrap()).unwrap();
    let mut db = ClassDb::default();
    db.load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    db
}

/// Card WSEAT: every section kind keeps its seat's keys, a walker's too
/// (the turret setter chain, [`walker::turret_claims`]).
#[test]
fn wseat_explicit_sections_keep_independent_seat_values() {
    for section in ["FLYERSECTION", "BuildingSection", "WalkerSection"] {
        let mut props = Vec::new();
        for (scope, pilot, n) in [
            ("BODY", "vehicle", 1),
            ("TURRET1", "remote", 2),
            ("TURRET8", "none", 8),
        ] {
            for (key, value) in [
                (section, scope.to_string()),
                ("PilotType", pilot.to_string()),
                ("EyePointOffset", format!("{n} 2 3")),
                ("TrackCenter", format!("4 {n} 6")),
                ("TrackOffset", format!("7 8 {n}")),
                ("TiltValue", n.to_string()),
                ("ThirdPersonFOV", (40 + n).to_string()),
                ("FirstPersonFOV", (60 + n).to_string()),
            ] {
                props.push((h(key), value));
            }
        }
        let def = ClassDef {
            name: "wseat_fixture".into(),
            kind: "entc".into(),
            base: "hover".into(),
            props,
        };
        let db = fixture_db(&format!("wseat-{section}"), vec![def]);
        let ring = seats(&db, "wseat_fixture");
        assert_eq!(
            ring.len(),
            3,
            "{section}: sparse sections must not create empty seats"
        );
        for (seat, kind, pilot, n) in [
            (&ring[0], SeatKind::Pilot, PilotType::Vehicle, 1.0),
            (&ring[1], SeatKind::Gunner(0), PilotType::Remote, 2.0),
            (&ring[2], SeatKind::Gunner(7), PilotType::None, 8.0),
        ] {
            assert_eq!(seat.kind, kind, "{section}");
            assert_eq!(seat.pilot_type, pilot, "{section} {kind:?}");
            assert_eq!(seat.eye_point_offset, Some([n, 2.0, 3.0]));
            assert_eq!(seat.track_center, Some([4.0, n, 6.0]));
            assert_eq!(seat.track_offset, Some([7.0, 8.0, n]));
            assert_eq!(seat.tilt_value, Some(n));
            assert_eq!(seat.third_person_fov, Some(40.0 + n));
            assert_eq!(seat.first_person_fov, Some(60.0 + n));
        }
    }
}

/// Card WSEAT: inside a `WalkerSection` the aimer, camera, seat and
/// turn keys stay on the section through a child's BASE chain; the
/// walker's own keys after a section still reach the body, and a
/// `BODY` value closes the section.
#[test]
fn a_walker_section_keeps_its_aimer_camera_and_seat_keys() {
    let def = |name: &str, base: &str, props: &[(&str, &str)]| ClassDef {
        name: name.into(),
        kind: "entc".into(),
        base: base.into(),
        props: props.iter().map(|(k, v)| (h(k), v.to_string())).collect(),
    };
    let parent = def(
        "wseat_walker_parent",
        "walker",
        &[
            ("EyePointOffset", "0 9 0"),
            ("FirePointName", "hp_pilot_gun"),
            ("WalkerSection", "TURRET1"),
            ("PilotType", "remote"),
            ("EyePointOffset", "1 2 3"),
            ("TrackCenter", "0 3 -7"),
            ("ThirdPersonFOV", "55"),
            ("AimerNodeName", "aimer_gun"),
            ("AimerYawLimits", "-35 110"),
            ("FirePointName", "hp_fire_gun"),
            ("TurnRate", "0.75"),
            ("PitchRate", "0.5"),
            ("MaxSpeed", "7"),
            ("LegPairCount", "1"),
        ],
    );
    let child = def(
        "wseat_walker_child",
        "wseat_walker_parent",
        &[
            ("WalkerSection", "TURRET1"),
            ("ThirdPersonFOV", "60"),
            ("WalkerSection", "BODY"),
            ("TiltValue", "4"),
        ],
    );
    let db = fixture_db("wseat-walker", vec![child, parent]);
    let ring = seats(&db, "wseat_walker_child");
    assert_eq!(ring.len(), 2, "pilot + TURRET1");
    let (pilot, gun) = (&ring[0], &ring[1]);
    assert_eq!(gun.kind, SeatKind::Gunner(0));
    assert_eq!(pilot.pilot_type, SeatSpec::new(SeatKind::Pilot).pilot_type);
    assert_eq!(gun.pilot_type, PilotType::Remote);
    assert_eq!(pilot.eye_point_offset, Some([0.0, 9.0, 0.0]));
    assert_eq!(gun.eye_point_offset, Some([1.0, 2.0, 3.0]));
    assert_eq!(
        (pilot.track_center, gun.track_center),
        (None, Some([0.0, 3.0, -7.0]))
    );
    assert_eq!(
        (pilot.third_person_fov, gun.third_person_fov),
        (None, Some(60.0))
    );
    assert_eq!(pilot.fire_point.as_deref(), Some("hp_pilot_gun"));
    assert_eq!(gun.fire_point.as_deref(), Some("hp_fire_gun"));
    assert_eq!(
        (pilot.aimer_node.as_deref(), gun.aimer_node.as_deref()),
        (None, Some("aimer_gun"))
    );
    assert_eq!(gun.aimer_yaw_limits, Some([-35.0, 110.0]));
    assert_eq!((pilot.turn_speed, gun.turn_speed), (None, Some(0.75)));
    assert_eq!((pilot.pitch_rate, gun.pitch_rate), (None, Some(0.5)));
    // After `WalkerSection BODY` the camera key is the pilot's.
    assert_eq!((pilot.tilt_value, gun.tilt_value), (Some(4.0), None));
    // The walker's own keys inside the section are the body's.
    let body: Vec<u32> = scoped(db.get("wseat_walker_parent").unwrap())
        .into_iter()
        .filter(|(s, _, _)| *s == Scope::Body)
        .map(|(_, k, _)| k)
        .collect();
    assert!(body.contains(&h("MaxSpeed")) && body.contains(&h("LegPairCount")));
    assert!(!body.contains(&h("AimerNodeName")) && !body.contains(&h("PilotType")));
}

/// Card VRIDE: the seat setter's pose keys by the hashes it compares
/// (B2 `FUN_002f6eb8`, re-vehicle-riders 2.1), a walker section's.
#[test]
fn the_seat_pose_keys_are_the_setters_hashes() {
    for (n, want) in [
        ("PilotPosition", 0x51ca39a6u32),
        ("AnimatedPilotPosition", 0x11ce2025),
        ("PilotAnimation", 0x6e4fc069),
        ("Pilot9Pose", 0xa976d065),
        ("IsPilotExposed", 0x2b60e217),
    ] {
        assert_eq!(h(n), want, "{n}");
        assert!(walker::turret_claims(want));
    }
    for n in ["MaxSpeed", "LegPairCount", "WalkerLegPair", "DamageEffect"] {
        assert!(!walker::turret_claims(h(n)), "{n} is the walker's");
    }
}

/// Card VTURG (re-turret-draw.md 3.1): a seat's pose key and
/// `IsPilotExposed` decide whether its soldier is drawn - the exposure
/// only on BF2 - and a seat no soldier boards has none to draw.
#[test]
fn a_seat_draws_its_gunner_by_its_pose_key() {
    let seat = |keys: &[(&str, &str)]| {
        let mut s = SeatSpec::new(SeatKind::Gunner(0));
        for (k, v) in keys {
            s.set(h(k), v);
        }
        s
    };
    let nine = seat(&[("Pilot9Pose", "minigun_9pose")]);
    assert_eq!(
        nine.pilot_pose,
        Some(PilotPose {
            name: "minigun_9pose".into(),
            nine: true
        })
    );
    assert_eq!(
        nine.pilot_pose.as_ref().unwrap().clip(),
        "human_minigun_9pose"
    );
    assert_eq!(nine.gunner_draw(false), GunnerDraw::Drawn);
    assert_eq!(nine.gunner_draw(true), GunnerDraw::Drawn);
    let drive = seat(&[("PilotAnimation", " drive ")]);
    assert_eq!(
        drive.pilot_pose.as_ref().map(|p| (p.clip(), p.nine)),
        Some(("human_drive".into(), false))
    );
    assert_eq!(seat(&[]).gunner_draw(true), GunnerDraw::Hidden);
    let shut = seat(&[("PilotAnimation", "drive"), ("IsPilotExposed", "0")]);
    assert_eq!(shut.gunner_draw(true), GunnerDraw::Hidden, "BF2 hides him");
    assert_eq!(
        shut.gunner_draw(false),
        GunnerDraw::Drawn,
        "BF1 reads it for the body only"
    );
    assert_eq!(
        seat(&[("PilotAnimation", "drive"), ("IsPilotExposed", "1")]).gunner_draw(true),
        GunnerDraw::Drawn
    );
    for t in ["self", "remote", "none"] {
        let s = seat(&[("PilotType", t), ("PilotAnimation", "drive")]);
        assert_eq!(s.gunner_draw(true), GunnerDraw::NoSoldier, "{t}");
    }
    let s = seat(&[("PilotType", "vehicleself"), ("Pilot9Pose", "barc_9pose")]);
    assert_eq!(s.gunner_draw(true), GunnerDraw::Drawn);
}

#[test]
fn seat_setup_carries_the_body_exposure_to_physical_hero_queries() {
    let mut ring = Vec::new();
    for keys in [
        vec![],
        vec![("PilotAnimation", "drive")],
        vec![("PilotAnimation", "drive"), ("IsPilotExposed", "0")],
        vec![("PilotAnimation", "drive"), ("PilotType", "none")],
    ] {
        let mut seat = SeatSpec::new(SeatKind::Pilot);
        for (key, value) in keys {
            seat.set(h(key), value);
        }
        ring.push(seat);
    }
    let setup = crate::vehicle_pads::seat_setup(&ClassDb::default(), "fixture", &ring, "bf2");
    assert_eq!(setup.body_active, [false, true, false, false]);
    assert_eq!(setup.boardable, [true, true, true, false]);
}

#[test]
fn section_values_map_to_the_ps2_indices() {
    assert_eq!(section_scope("BODY"), Scope::Body);
    assert_eq!(section_scope("TURRET"), Scope::Turret(0));
    assert_eq!(section_scope("TURRET1"), Scope::Turret(0));
    assert_eq!(section_scope("turret2"), Scope::Turret(1));
    assert_eq!(section_scope("TURRET8"), Scope::Turret(7));
    assert_eq!(section_scope("TURRET9"), Scope::Body);
    assert_eq!(section_scope("WINGS"), Scope::Body);
}

#[test]
fn spring_point_defaults_follow_the_lever_arm() {
    let p = spring_point([-1.8, -0.7, 1.7], 2.0);
    assert_eq!(p.length, 1.0);
    assert_eq!((p.omega_x_spring, p.omega_z_spring), (-1.0, -1.0));
    let p = spring_point([0.0, 0.3, -2.0], 3.0);
    assert_eq!((p.omega_x_spring, p.omega_z_spring), (1.0, 0.0));
    assert_eq!(
        (
            p.velocity_spring,
            p.velocity_damp,
            p.omega_x_damp,
            p.omega_z_damp
        ),
        (1.0, 1.0, 1.0, 1.0)
    );
}

/// The setter's slot pointer: a Start with no Effect is overwritten by
/// the next Start; a Start resets Stop; chunk-section props never reach
/// a slot; the band is `stop < f ≤ start`.
#[test]
fn damage_slots_follow_the_setters_pointer() {
    let props: Vec<(u32, String)> = [
        ("DamageStopPercent", "50"), // no slot yet: dropped
        ("DamageStartPercent", "90"),
        ("DamageStopPercent", "70"), // orphaned with its Start
        ("DamageStartPercent", "60.0"),
        ("DamageStopPercent", "20.0"),
        ("DamageEffect", "vehiclespark"),
        ("DamageEffectScale", "0.4"),
        ("DamageInheritVelocity", "0.75"),
        ("DamageAttachPoint", "hp_damage_1"),
        ("DamageStartPercent", "2.0"),
        ("DamageEffect", "vehicleflame"),
        ("CHUNKSECTION", "CHUNK1"),
        ("DamageStartPercent", "40"),
        ("DamageEffect", "chunkfx"),
    ]
    .iter()
    .map(|(k, v)| (h(k), v.to_string()))
    .collect();
    let def = ClassDef {
        name: "t_hover".into(),
        kind: "entc".into(),
        base: "hover".into(),
        props,
    };
    let path = std::env::temp_dir().join(format!("openbf-dmgslots-{}.ron", std::process::id()));
    std::fs::write(&path, ron::to_string(&vec![def]).unwrap()).unwrap();
    let mut db = ClassDb::default();
    db.load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let fx = damage_effects(&db, "t_hover");
    assert_eq!(fx.len(), 2, "{fx:#?}");
    let pct = |p: f32| p * 0.01;
    assert_eq!((fx[0].start, fx[0].stop), (pct(60.0), pct(20.0)));
    assert_eq!(fx[0].effect, "vehiclespark");
    assert_eq!(fx[0].attach.as_deref(), Some("hp_damage_1"));
    assert_eq!((fx[0].scale, fx[0].inherit_velocity), (0.4, 0.75));
    assert_eq!((fx[1].start, fx[1].stop, fx[1].scale), (pct(2.0), 0.0, 1.0));
    assert_eq!(fx[1].attach, None);
    assert!(!fx[0].active(1.0) && fx[0].active(fx[0].start) && fx[0].active(0.3));
    assert!(!fx[0].active(fx[0].stop) && !fx[0].active(0.1));
    assert!(fx[1].active(0.01) && !fx[1].active(0.0));
}

/// FUN_00221e48's AISizeType: case-insensitive words, 1 for anything else.
#[test]
fn ai_size_reads_the_six_words_in_any_case() {
    let classes: Vec<ClassDef> = [
        ("a", Some("MEDIUM")),
        ("b", Some("hover")),
        ("c", Some("Huge")),
        ("d", Some("SOLDIER")),
        ("e", Some("tank")),
        ("f", None),
    ]
    .iter()
    .map(|&(n, v)| ClassDef {
        name: n.into(),
        kind: "entc".into(),
        base: "hover".into(),
        props: v
            .map(|v| (h("AISizeType"), v.to_string()))
            .into_iter()
            .collect(),
    })
    .collect();
    let path = std::env::temp_dir().join(format!("openbf-aisize-{}.ron", std::process::id()));
    std::fs::write(&path, ron::to_string(&classes).unwrap()).unwrap();
    let mut db = ClassDb::default();
    db.load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let got: Vec<u32> = ["a", "b", "c", "d", "e", "f"]
        .iter()
        .map(|c| ai_size(&db, c))
        .collect();
    assert_eq!(got, [8, 2, 0x10, 1, 1, 1]);
}

#[test]
fn values_with_a_trailing_comment_parse() {
    assert_eq!(float("3//low"), Some(3.0));
    assert_eq!(float(" .85"), Some(0.85));
    assert_eq!(float("-0.01"), Some(-0.01));
    assert_eq!(
        PilotType::parse("VehicleSelf"),
        Some(PilotType::VehicleSelf)
    );
    assert_eq!(PilotType::parse("bogus"), None);
}

#[test]
fn a_chunk_section_does_not_close_the_seat_it_follows() {
    let db = fixture_db(
        "chunkseat",
        vec![ClassDef {
            name: "ship".to_owned(),
            kind: "entc".to_owned(),
            base: "flyer".to_owned(),
            props: vec![
                (h("FLYERSECTION"), "BODY".to_owned()),
                (h("TiltValue"), "3".to_owned()),
                (h("CHUNKSECTION"), "CHUNK1".to_owned()),
                (h("TrackCenter"), "0 5 -8".to_owned()),
                (h("TrackOffset"), "0 1 10".to_owned()),
            ],
        }],
    );
    let got = seats(&db, "ship");
    assert_eq!(got[0].track_center, Some([0.0, 5.0, -8.0]));
    assert_eq!(got[0].track_offset, Some([0.0, 1.0, 10.0]));
}

#[test]
fn a_flyer_explodes_by_its_destruct_key_and_a_hover_by_its_name() {
    let def = |name: &str, base: &str, key: &str| ClassDef {
        name: name.to_owned(),
        kind: "entc".to_owned(),
        base: base.to_owned(),
        props: vec![(h(key), format!("{name}_exp"))],
    };
    let db = fixture_db(
        "explosionkeys",
        vec![
            def("ship", "flyer", "ExplosionDestruct"),
            def("gunship", "commandflyer", "ExplosionDestruct"),
            def("bike", "hover", "ExplosionName"),
        ],
    );
    assert_eq!(explosion_name(&db, "ship").as_deref(), Some("ship_exp"));
    assert_eq!(
        explosion_name(&db, "gunship").as_deref(),
        Some("gunship_exp")
    );
    assert_eq!(explosion_name(&db, "bike").as_deref(), Some("bike_exp"));
}
