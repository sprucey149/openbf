use super::*;

#[test]
fn tracked_sabers_share_the_primary_grip_frame_and_collision_for_each_hand() {
    let root = assets::baked_root();
    if !pak::exists(root.join("side_bf2/heroes.ron")) {
        eprintln!("SKIP: BF2 hero bake absent");
        return;
    }
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep", "cis"]).unwrap();
    let grip = emitter_frame(&root, "rep_weap_inf_lightsabre", "hp_fire").unwrap();
    for (hero, weapon) in [
        ("rep_hero_aalya", "rep_weap_lightsaber_aalya"),
        ("cis_hero_grievous", "cis_weap_grievoussabers"),
    ] {
        let class = saber_class(&db, hero, weapon).unwrap();
        let bindings = blade_bindings(&root, &db, weapon).unwrap();
        let mut hands = [false; 2];
        for (binding, edge) in bindings.iter().zip(class.hand_edges) {
            let Some(hand) = binding.blade.tracked_hand() else {
                assert_eq!(binding.tracked_mount, Mat4::IDENTITY);
                continue;
            };
            assert!(!std::mem::replace(&mut hands[hand], true));
            let emitter = emitter_frame(
                &root,
                binding.geometry.as_deref().unwrap(),
                &binding.blade.node,
            )
            .unwrap();
            assert!(
                (binding.tracked_mount * emitter).abs_diff_eq(grip, 1e-5),
                "{hero} hand {hand}"
            );
            assert!(edge.start.abs_diff_eq(grip.w_axis.truncate(), 1e-5));
            assert!((edge.end - edge.start)
                .abs_diff_eq(grip.z_axis.truncate() * binding.blade.length, 1e-5));
        }
        assert_eq!(hands, [true; 2]);
    }
}

#[test]
fn tracked_cuts_defeat_full_health_targets_with_either_hand() {
    use core::sim::{PadState, Sim, TrackedCombat, TrackedHand};
    use core::unit::UnitSpec;
    use core::weapon::WeaponClass;
    use glam::Quat;

    let root = assets::baked_root();
    if !pak::exists(root.join("side_bf2/heroes.ron")) {
        eprintln!("SKIP: BF2 hero bake absent");
        return;
    }
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep", "all"]).unwrap();
    for (hero, weapon_name, hand, damage, bearing) in [
        (
            "rep_hero_macewindu",
            "rep_weap_lightsaber",
            1,
            300,
            Vec3::NEG_X,
        ),
        ("rep_hero_macewindu", "rep_weap_lightsaber", 1, 300, Vec3::X),
        (
            "rep_hero_aalya",
            "rep_weap_lightsaber_aalya",
            0,
            400,
            Vec3::NEG_Z,
        ),
    ] {
        let saber = saber_class(&db, hero, weapon_name).unwrap();
        assert_eq!(saber.damage, 0.0, "{hero}: normal cuts supply combo damage");
        let edge = saber
            .blades
            .iter()
            .zip(&saber.hand_edges)
            .find_map(|(blade, &edge)| (blade.tracked_hand() == Some(hand)).then_some(edge))
            .unwrap();
        let rotation = Quat::from_rotation_arc((edge.end - edge.start).normalize(), bearing);
        let target_pos = bearing * 0.9;
        let center = target_pos + Vec3::Y * 1.15 - rotation * edge.start.lerp(edge.end, 0.5);
        let mut sim = Sim::empty(7);
        let weapon = sim.add_weapon_class(WeaponClass {
            saber: Some(Box::new(saber)),
            ..WeaponClass::default()
        });
        let rifle = sim.add_weapon_class(WeaponClass::bf1_rifle());
        let wielder = sim
            .spawn(&UnitSpec {
                weapon_class: weapon,
                ..UnitSpec::player(0, Vec3::ZERO)
            })
            .unwrap();
        let target = sim
            .spawn(&UnitSpec {
                weapon_class: rifle,
                max_health: 1200.0,
                ..UnitSpec::trooper(1, target_pos)
            })
            .unwrap();
        for cut in 0..=1200 / damage {
            let offset = if cut % 2 == 0 { -0.7 } else { 0.7 };
            let mut hands = [None; 2];
            hands[hand] = Some(TrackedHand::new(
                center + bearing.cross(Vec3::Y) * offset,
                rotation,
            ));
            let pad = PadState {
                tracked_combat: Some(TrackedCombat {
                    hands,
                    ..TrackedCombat::default()
                }),
                ..PadState::default()
            };
            sim.step(&[pad]);
            assert_eq!(
                sim.units[target.index as usize].health.cur,
                (1200 - cut * damage) as f32,
                "{hero}: cut {cut}"
            );
            if cut > 0 {
                assert!(
                    sim.events.iter().any(
                        |event| matches!(event, core::sim::Event::SaberHandContact {
                    attacker, hand: side, blocked: false, ..
                } if *attacker == wielder && usize::from(*side) == hand)
                    ),
                    "{hero}: cut feedback"
                );
                for _ in 1..2 * core::TICK_HZ / 5 {
                    sim.step(&[pad]);
                }
            }
        }
        assert!(!sim.units[target.index as usize].alive, "{hero}");
    }
}

#[test]
fn every_bf2_saber_hero_uses_authored_edges_and_moving_clip_tracks() {
    let root = assets::baked_root();
    if !pak::exists(root.join("side_bf2/heroes.ron")) {
        eprintln!("SKIP: BF2 hero bake absent; run bake heroes before this real-content check");
        return;
    }
    let db =
        crate::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    let mut count = 0;
    let mut root_moves = 0;
    let mut samples = 0;
    for soldier in db.names().filter(|n| n.split('_').nth(1) == Some("hero")) {
        let Some(weapon) = db
            .weapon_slots(soldier)
            .into_iter()
            .find(|s| s.channel == 0)
        else {
            continue;
        };
        if !db
            .chain(&weapon.weapon)
            .last()
            .is_some_and(|c| c.base.eq_ignore_ascii_case("melee"))
        {
            continue;
        }
        let saber = saber_class(&db, soldier, &weapon.weapon)
            .unwrap_or_else(|e| panic!("{soldier}: {e:#}"));
        count += 1;
        assert_eq!(saber.state_seconds.len(), saber.combo.states().count());
        let first_deflect = saber
            .combo
            .states()
            .position(|s| s.find(K::Deflect).is_some())
            .unwrap();
        for (animation, _) in saber.deflect_animations(first_deflect) {
            assert!(
                saber.deflect_attachment(animation).is_some(),
                "{soldier}: {animation}"
            );
        }
        let edges = match soldier {
            "rep_hero_aalya" => 2,
            "cis_hero_grievous" => 4,
            "cis_hero_darthmaul" => 3,
            _ => 1,
        };
        assert_eq!(saber.blades.len(), edges, "{soldier}");
        assert!(saber.clips.len() > 1, "{soldier}: no action clips");
        let primary: Vec<_> = saber
            .blades
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.bone.eq_ignore_ascii_case("hp_weapons").then_some(i))
            .collect();
        assert_eq!(saber.throw_edges.len(), primary.len());
        let first_bone = &saber.blades[0].bone;
        assert_eq!(
            saber.deflect_edges.len(),
            saber
                .blades
                .iter()
                .filter(|b| b.bone.eq_ignore_ascii_case(first_bone))
                .count()
        );
        assert!(!saber.deflect_edges.is_empty());
        assert!(saber.weapon_bind.is_finite());
        let throws: Vec<_> = db
            .weapon_slots(soldier)
            .into_iter()
            .filter(|s| db.engine_base(&s.weapon) == Some("meleethrow"))
            .map(|s| s.weapon)
            .collect();
        assert_eq!(saber.throw_bindings.len(), throws.len(), "{soldier}");
        for binding in &saber.throw_bindings {
            assert!(throws.contains(&binding.weapon));
            assert!(binding.attachment.is_finite());
            assert!((binding.seconds - 0.28).abs() < 1e-6);
            assert_eq!(binding.animation, "stand_throw");
        }
        if matches!(soldier, "rep_hero_yoda" | "cis_hero_grievous") {
            assert!(
                saber.throw_bindings.is_empty(),
                "actual no-throw loadout: {soldier}"
            );
        }
        if soldier == "rep_hero_anakin" {
            // Original saved constructor matrix, weapon1d28740+160.
            let at = saber.throw_bindings[0].attachment.w_axis.truncate();
            assert!(
                at.distance(Vec3::new(-0.44556487, 1.3824263, 0.78510904)) < 0.001,
                "{at:?}"
            );
        }
        if primary.is_empty() {
            assert!(db
                .weapon_slots(soldier)
                .iter()
                .all(|s| !s.weapon.contains("saberthrow")));
        }
        assert!(saber
            .clips
            .iter()
            .any(|c| c.name == "stand_idle" || c.name == "stand_idle_emote"));
        let mut moving = false;
        for clip in &saber.clips {
            assert_eq!(clip.root_frames.len(), clip.frames.len());
            assert_eq!(clip.weapon_frames.len(), clip.frames.len());
            assert!(clip.weapon_frames.iter().all(|m| m.is_finite()));
            assert!(clip.root_frames.iter().all(|p| p.is_finite()));
            assert_eq!(clip.root_frames.first().copied(), Some(Vec3::ZERO));
            root_moves += usize::from(clip.root_frames.windows(2).any(|p| p[0] != p[1]));
            samples += clip.frames.len();
            for (frame, attachment) in clip.frames.iter().zip(&clip.weapon_frames) {
                assert_eq!(frame.len(), edges, "{soldier}: {}", clip.name);
                for (segment, blade) in frame.iter().zip(&saber.blades) {
                    assert!(segment.start.is_finite() && segment.end.is_finite());
                    assert!(
                        ((segment.end - segment.start).length() - blade.length).abs() < 1e-4,
                        "{soldier}: {} edge {} has wrong length",
                        clip.name,
                        blade.node
                    );
                }
                for (&index, local) in primary.iter().zip(&saber.throw_edges) {
                    assert!(
                        frame[index]
                            .start
                            .distance(attachment.transform_point3(local.start))
                            < 1e-4
                    );
                    assert!(
                        frame[index]
                            .end
                            .distance(attachment.transform_point3(local.end))
                            < 1e-4
                    );
                }
            }
            if let (Some(a), Some(z)) = (clip.frames.first(), clip.frames.last()) {
                moving |= a.iter().zip(z).any(|(a, z)| a.end.distance(z.end) > 0.1);
            }
        }
        assert!(
            moving,
            "{soldier}: every saber endpoint stayed at its bind pose"
        );
        if soldier == "cis_hero_darthmaul" {
            assert_eq!(saber.blades[2].bone, "bone_l_calf");
            assert!(saber.blades[2].texture.is_empty());
            assert_eq!(saber.blades[2].length, 0.5);
        }
        if soldier == "rep_hero_aalya" {
            assert_eq!(saber.blades[0].bone, "hp_weapons");
            assert_eq!(saber.blades[1].bone, "bone_l_hand");
            let binding = blade_bindings(&root, &db, &weapon.weapon).unwrap();
            // Raw PS2 REP.LVL skel/XFRM, hp_fire_lightsabre's +Z row.
            let direction = (binding[1].local.end - binding[1].local.start).normalize();
            assert!(direction.abs_diff_eq(Vec3::new(0.38555932, -0.89098287, 0.23977822), 1e-6));
        }
    }
    // Includes Luke's pilot kit, both Mace kits and cloaked Anakin.
    assert_eq!(count, 15);
    assert!(
        root_moves > 0,
        "all original attack root profiles were discarded"
    );
    println!(
        "{count} saber hero classes, {samples} pose frames, {root_moves} moving root profiles"
    );
}

#[test]
fn original_cached_deflect_matrices_match_the_real_hero_rigs() {
    let Some(dir) = std::env::var_os("OPENBF_THROW_REFERENCE_DIR") else {
        eprintln!("SKIP: original hero reference directory not supplied");
        return;
    };
    let references: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(std::path::PathBuf::from(dir).join("native_deflect_poses.json"))
            .unwrap(),
    )
    .unwrap();
    let root = assets::baked_root();
    let db =
        crate::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    let mut checked = 0;
    let mut maximum = 0.0f32;
    for reference in references {
        let hero = reference["hero"].as_str().unwrap();
        let weapon = db
            .weapon_slots(hero)
            .into_iter()
            .find(|s| s.channel == 0)
            .unwrap()
            .weapon;
        let saber = saber_class(&db, hero, &weapon).unwrap();
        let first = reference["first_state"].as_u64().unwrap() as usize;
        let animations = saber.deflect_animations(first);
        let entries = reference["entries"].as_array().unwrap();
        assert_eq!(animations.len(), entries.len());
        for ((animation, mask), entry) in animations.into_iter().zip(entries) {
            assert_eq!(u64::from(mask), entry["direction_mask"].as_u64().unwrap());
            let actual = saber.deflect_attachment(animation).unwrap().to_cols_array();
            let original = entry["matrix"].as_array().unwrap();
            for (component, (a, b)) in actual.into_iter().zip(original).enumerate() {
                let b = b.as_f64().unwrap() as f32;
                let error = (a - b).abs();
                maximum = maximum.max(error);
                assert!(
                    error < 0.002,
                    "{hero} {animation} component{component}: {a} != {b} (error{error})"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 24);
    println!("{checked} native cached deflect matrices; maximum component error{maximum}");
}

#[test]
fn nonfinite_attached_blade_offsets_are_rejected() {
    fn parse(point: &str) -> anyhow::Result<Vec<Edge>> {
        let definition = crate::classdb::ClassDef {
            name: "hostile_saber".into(),
            kind: "wpnc".into(),
            base: "melee".into(),
            props: vec![
                (
                    crate::classdb::fnv1a_lowercase("AttachedFirePoint"),
                    point.into(),
                ),
                (
                    crate::classdb::fnv1a_lowercase("DamageEdgeLength"),
                    "1.2".into(),
                ),
                (
                    crate::classdb::fnv1a_lowercase("DamageEdgeWidth"),
                    "0.1".into(),
                ),
            ],
        };
        let path =
            std::env::temp_dir().join(format!("openbf-hero-finite-{}.ron", std::process::id()));
        std::fs::write(&path, ron::to_string(&vec![definition]).unwrap()).unwrap();
        let mut db = ClassDb::default();
        let loaded = db.load(&path);
        std::fs::remove_file(path).unwrap();
        loaded.unwrap();
        edges(&db, "hostile_saber")
    }
    assert!(
        parse("bone_r_hand 0 0 1 -0.5 0 0").is_ok(),
        "signed finite offsets stay valid"
    );
    let mut accepted = Vec::new();
    for axis in 0..3 {
        for value in ["NaN", "inf", "-inf"] {
            let mut values = ["0", "0", "0"];
            values[axis] = value;
            let point = format!("bone_r_hand 0 0 1 {}", values.join(" "));
            if parse(&point).is_ok() {
                accepted.push((axis, value));
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted nonfinite attached offsets: {accepted:?}"
    );
}

mod boundary;
