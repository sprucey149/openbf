use super::*;

#[test]
fn donor_soldier_vertices_keep_bind_pose_and_use_canonical_joint_names() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let path = crate::join_ci(
        &root,
        "PC_swbf1_exe/Star Wars Battlefront (Classic 2004)/GameData/Data/_LVL_PC/SIDE/imp.lvl",
    );
    if !path.is_file() {
        return;
    }
    let (corrected, original) = crate::gameplay::read_with_original(&path).unwrap();
    let original = original.expect("the supplied PC source must exercise donor bone ordering");
    let (m, canonical) =
        find_soldier(mungefs::open(&corrected).unwrap(), "imp_inf_stormtrooper").unwrap();
    let (donor_m, donor) =
        find_soldier(mungefs::open(&original).unwrap(), "imp_inf_stormtrooper").unwrap();
    let expected = to_skinned_model(&donor_m, &donor).unwrap();
    let wrong_indices = to_skinned_model(&m, &canonical).unwrap();
    assert!(wrong_indices
        .segments
        .iter()
        .zip(&expected.segments)
        .any(|(a, b)| a.positions != b.positions));
    let actual = skinned_model_from_donor(&m, &canonical, &donor).unwrap();
    assert_eq!(
        actual
            .bones
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        canonical
            .bones
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>()
    );
    for (a, b) in actual.segments.iter().zip(&expected.segments) {
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.normals, b.normals);
        for (mapped, original) in a.joints.iter().zip(&b.joints) {
            assert_eq!(
                canonical.bones[mapped[0] as usize].crc,
                donor.bones[original[0] as usize].crc
            );
        }
    }
}

#[test]
fn model_space_blends_preserve_all_active_bones_and_normalize_weights() {
    let (joints, weights) =
        model_space_influences(&[7, 3], [4, 0, 252, 252], [128, 64, 0, 0], 8).unwrap();
    assert_eq!(joints, [3, 7, 0, 0]);
    assert_eq!(weights, [2.0 / 3.0, 1.0 / 3.0, 0.0, 0.0]);
    // Moving only the second joint moves the vertex by its authored share.
    assert!((weights[1] * 6.0 - 2.0).abs() < 1e-6);
    let (joints, weights) =
        model_space_influences(&[7, 3], [252, 4, 252, 252], [0, 64, 0, 0], 8).unwrap();
    assert_eq!(joints, [0, 3, 0, 0]);
    assert_eq!(weights, [0.0, 1.0, 0.0, 0.0]);
    assert!(model_space_influences(&[7], [4, 0, 0, 0], [255, 0, 0, 0], 8).is_err());
    assert!(model_space_influences(&[7], [0; 4], [255, 0, 0, 0], 7).is_err());
    assert!(model_space_influences(&[7], [0; 4], [0; 4], 8).is_err());
}

#[test]
fn model_space_softskin_reaches_glb_and_bone_space_stays_rigid() {
    use mungefs::model::{Aabb, RenderType, Segment};
    use mungefs::skel::SkelBone;
    let skel = Skeleton {
        name: "softskin".into(),
        bones: (0..2)
            .map(|i| SkelBone {
                name: format!("bone{i}"),
                crc: crc32_lower(&format!("bone{i}")),
                parent: None,
                rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                t: [0.0; 3],
            })
            .collect(),
    };
    let bounds = Aabb {
        min: [0.0; 3],
        max: [1.0; 3],
    };
    let mut model = Model {
        name: "softskin".into(),
        bone_space: false,
        vertex_box: bounds,
        vis_box: bounds,
        face_count: 1,
        sphere: None,
        segments: vec![Segment {
            vertex_count: 3,
            index_count: 3,
            bf2_material: true,
            pc_geometry: false,
            material_flags: 1,
            render_type: RenderType::Bf2(0),
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0; 2],
            textures: vec![],
            bone: None,
            group: None,
            bmap: vec![0, 1],
            bind: vec![[0, 4, 252, 252], [252, 4, 252, 252], [0, 252, 252, 252]],
            wght: vec![[192, 64, 0, 0], [0, 255, 0, 0], [255, 0, 0, 0]],
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            colors: None,
            triangles: vec![[0, 1, 2]],
        }],
    };
    let skin = rigid_or_skinned_model(&model, &skel).unwrap();
    assert_eq!(skin.segments[0].positions, model.segments[0].positions);
    assert_eq!(
        skin.segments[0].joints,
        [[0, 1, 0, 0], [0, 1, 0, 0], [0; 4]]
    );
    assert_eq!(
        skin.segments[0].weights,
        [
            [0.75, 0.25, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0]
        ]
    );
    let blob = skinned_to_glb(&skin, &[]).unwrap();
    let doc = gltf::Gltf::from_slice(&blob).unwrap();
    let primitive = doc.meshes().next().unwrap().primitives().next().unwrap();
    let reader = primitive.reader(|_| doc.blob.as_deref());
    assert_eq!(
        reader.read_weights(0).unwrap().into_f32().next().unwrap(),
        [0.75, 0.25, 0.0, 0.0]
    );
    assert_eq!(
        reader.read_joints(0).unwrap().into_u16().next().unwrap(),
        [0, 1, 0, 0]
    );
    // Bone-space models retain the former primary-influence conversion.
    model.bone_space = true;
    model.segments[0].bind[1][0] = 4;
    let legacy = rigid_or_skinned_model(&model, &skel).unwrap();
    assert_eq!(legacy.segments[0].joints, [[0; 4], [1, 0, 0, 0], [0; 4]]);
    assert_eq!(legacy.segments[0].weights, [[1.0, 0.0, 0.0, 0.0]; 3]);
}

/// The two states a skinned segment can take, read off its `MTRL` word
/// alone, and the words the skinned path refuses. The first two rows are
/// every word a baked soldier segment carries on either disc
/// (`analysis/re-side-alpha.md` §0 item 3).
#[test]
fn segment_blends_draws_two_states_and_refuses_the_rest() {
    for word in [0x1, 0x31, 0x1_0001, 0x1_0089, 0x1_0101, 0x1_0010] {
        assert!(!segment_blends(word).unwrap(), "{word:#x} is opaque");
    }
    for word in [0x5, 0x1_0005] {
        assert!(segment_blends(word).unwrap(), "{word:#x} composites");
    }
    // The cutout bit in any company: the disc alpha-TESTS it.
    for word in [0x2, 0x3, 0x6, 0x1_0003] {
        assert!(segment_blends(word).is_err(), "{word:#x} is refused");
    }
    // A transparent word with a bit that changes its blend.
    for word in [0xc, 0x44, 0x85, 0x1_008d] {
        assert!(segment_blends(word).is_err(), "{word:#x} is refused");
    }
    // Those bits are harmless on an opaque segment (0x1_0089 above).
    assert!(!segment_blends(0xc8).unwrap());
}

/// Data-gated, queue SKA: `all_inf_wookie` bakes with its FUR segment
/// blended and its other two opaque, on both discs — BF1 segment 2 (word
/// `0x5`, 261 vertices), BF2 segment 1 (`0x10005`, 288)
/// (`analysis/re-side-alpha.md` §0 item 2, [O]) — and only that material
/// states an `alphaMode` in the GLB. The verdict is per SEGMENT: the fur
/// shares its sheet with an opaque segment in both games (BF1's third
/// segment is the face, `wookieface02`; BF2's three all name one sheet).
#[test]
fn the_wookies_fur_is_its_only_blended_segment_on_both_discs() {
    let side = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
    let cases = [
        ("bf1", ["wookieface02", "wookie", "wookie"], 2, 261),
        ("bf2", ["all_inf_wookie"; 3], 1, 288),
    ];
    for (game, sheets, fur, fur_verts) in cases {
        let all = format!("{side}/{game}/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL");
        if !std::path::Path::new(&all).exists() {
            eprintln!("SKIP: {all} not present");
            continue;
        }
        let buf = std::fs::read(&all).unwrap();
        let (m, skel) = find_soldier(mungefs::open(&buf).unwrap(), "all_inf_wookie").unwrap();
        let sm = to_skinned_model(&m, &skel).unwrap();
        let blends: Vec<bool> = sm.segments.iter().map(|s| s.blend).collect();
        let expected: Vec<bool> = (0..3).map(|k| k == fur).collect();
        assert_eq!(blends, expected, "{game}: only the fur composites");
        assert_eq!(
            sm.segments[fur].positions.len(),
            fur_verts,
            "{game}: the fur segment"
        );
        let named: Vec<&str> = sm.segments.iter().map(|s| s.texture.as_str()).collect();
        assert_eq!(named, sheets, "{game}: the sheets the segments name");

        let glb = skinned_to_glb(&sm, &[]).unwrap();
        let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + len]).unwrap();
        let modes: Vec<Option<&str>> = doc["materials"]
            .as_array()
            .unwrap()
            .iter()
            .map(|mat| mat.get("alphaMode").and_then(|v| v.as_str()))
            .collect();
        let expected: Vec<Option<&str>> = (0..3).map(|k| (k == fur).then_some("BLEND")).collect();
        assert_eq!(modes, expected, "{game}: the GLB's materials");
    }
}

/// World position of `bone` at frame 0 of `tracks` (DummyRoot at its
/// rest): the skinned draw's forward kinematics.
fn fk_frame0(skel: &Skeleton, tracks: &[ClipTrack], bone: &str) -> Vec3 {
    let binds = bones_of(skel);
    let local = |i: usize| {
        let (mut t, mut r) = (binds[i].translation, binds[i].rotation);
        if let Some(k) = tracks.iter().find(|k| k.bone == i) {
            (t, r) = (k.translations[0], k.rotations[0]);
        }
        glam::Mat4::from_rotation_translation(Quat::from_array(r), Vec3::from_array(t))
    };
    let mut i = skel.bones.iter().position(|b| b.name == bone).unwrap();
    let mut m = local(i);
    while let Some(p) = skel.bones[i].parent {
        m = local(p) * m;
        i = p;
    }
    m.w_axis.truncate()
}

/// Card USKN acceptance 1 (`analysis/re-soldier-skeleton-skins.md` §2-§4,
/// §10): an own-skeleton class is posed on its rig's bone lengths with
/// bone_root x SkeletonRootScale, the idle frame 0 head at the PS2 rule's
/// height (`lead_tmp/uskn/fk_idle.txt`); the clone trooper (human rig) is
/// unchanged; the listed bones hold the rig's JONT rest in every frame.
/// Control: without the rule every model's head is the human 1.605 / 1.643.
#[test]
fn own_rig_classes_pose_at_their_rig_lengths() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
    let b1 = format!("{root}/bf1/lvl/DATA/_LVL_PS2");
    let b2 = format!("{root}/bf2/lvl/DATA/_LVL_PS2");
    let a1 = format!("{b1}/COMMON/ENG/COMMON.LVL");
    let a2 = format!("{b2}/INGAME.LVL");
    if !Path::new(&a1).exists() || !Path::new(&a2).exists() {
        eprintln!("SKIP: COMMON.LVL / INGAME.LVL not present");
        return;
    }
    // (game, side, model, idle clip, rig, scale, PS2 head y, human head y)
    let rows = [
        (
            1,
            "ALL",
            "all_inf_wookie",
            "all_inf_wookie",
            1.15,
            1.852,
            1.605,
        ),
        (
            1,
            "CIS",
            "cis_inf_bdroid",
            "cis_inf_bdroid",
            1.0,
            1.726,
            1.605,
        ),
        (1, "REP", "rep_inf_trooper", "", 1.0, 1.605, 1.605),
        (
            2,
            "CIS",
            "cis_inf_magnaguard",
            "magnaguard",
            1.24,
            1.865,
            1.643,
        ),
        (2, "CIS", "cis_inf_bdroid", "bdroid", 1.0, 1.758, 1.643),
        (2, "ALL", "all_inf_wookie", "wookie", 1.15, 1.892, 1.643),
    ];
    let abufs = [std::fs::read(&a1).unwrap(), std::fs::read(&a2).unwrap()];
    for (game, side, model, rig, scale, ps2, human) in rows {
        let (dir, abuf) = if game == 1 {
            (&b1, &abufs[0])
        } else {
            (&b2, &abufs[1])
        };
        let buf = std::fs::read(format!("{dir}/SIDE/{side}.LVL")).unwrap();
        let inner = mungefs::open(&buf).unwrap();
        let ainner = mungefs::open(abuf).unwrap();
        let (_, skel) = find_soldier(inner, model).unwrap();
        let mut rigs = mungefs::anim::find_rigs(inner).unwrap();
        rigs.extend(mungefs::anim::find_rigs(ainner).unwrap());
        let classes = anim_sets::bf2::side_classes(&buf).unwrap();
        let pose = pose_rig(&classes, model, &rigs, game == 2);
        match &pose {
            None => assert_eq!(rig, "", "{model}: no pose rig"),
            Some(p) => {
                assert!(
                    p.rig.name.eq_ignore_ascii_case(rig),
                    "{model}: {}",
                    p.rig.name
                );
                assert_eq!(p.root_scale, scale, "{model}");
            }
        }
        let banks = mungefs::anim::find_banks(ainner).unwrap();
        let clip = [
            "human_rifle_stand_idle_emote",
            "human_rifle_stand_idle_emote_full",
        ]
        .iter()
        .find_map(|n| {
            banks
                .iter()
                .find_map(|b| Some(b.decode_clip(b.clip_index(n)?)))
        })
        .unwrap()
        .unwrap();
        let (tracks, _) = clip_to_tracks(&clip, &skel, pose.as_ref());
        let head = fk_frame0(&skel, &tracks, "bone_head").y;
        assert!(
            (head - ps2).abs() < 0.01,
            "{model}: head {head} vs PS2 {ps2}"
        );
        let (plain, _) = clip_to_tracks(&clip, &skel, None);
        let head0 = fk_frame0(&skel, &plain, "bone_head").y;
        assert!(
            (head0 - human).abs() < 0.01,
            "{model}: control head {head0}"
        );
        let Some(p) = &pose else { continue };
        let mut listed = 0;
        for t in &tracks {
            let b = &skel.bones[t.bone];
            if !p.takes_rest(&b.name) {
                continue;
            }
            let Some(j) = p.rig.joint(b.crc) else {
                continue;
            };
            listed += 1;
            assert!(
                t.translations.iter().all(|v| *v == j.rest_pos),
                "{model} {}",
                b.name
            );
        }
        assert!(listed >= 22, "{model}: {listed} bones on the rig's rest");
    }
}

/// Card USKN acceptance 3 (re-soldier-skeleton-skins 9): over the four
/// soldier sides of both discs, every class with an `OverrideTexture`
/// whose model is in its side LVL has that model's `override_texture`
/// group to draw it on (and `override_texture2` for a second), and every
/// override texture is in the side LVL for the bake to write; a second
/// override without a first is never taken.
#[test]
fn every_class_override_has_its_group_and_its_texture() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
    let mut checked = 0;
    let mut no_group = Vec::new();
    for game in ["bf1", "bf2"] {
        for side in ["ALL", "IMP", "REP", "CIS"] {
            let path = format!("{root}/{game}/lvl/DATA/_LVL_PS2/SIDE/{side}.LVL");
            let Ok(buf) = std::fs::read(&path) else {
                eprintln!("SKIP: {path}");
                return;
            };
            let inner = mungefs::open(&buf).unwrap();
            let classes = anim_sets::bf2::side_classes(&buf).unwrap();
            let chunks = mungefs::content_chunks(inner).unwrap();
            let texs: Vec<String> = chunks
                .iter()
                .filter(|c| &c.tag == b"tex_")
                .filter_map(|c| mungefs::texture::Texture::parse(c).ok())
                .map(|t| t.name.to_ascii_lowercase())
                .collect();
            for c in classes.iter().filter(|c| c.kind == "entc") {
                let skin = class_skin(&classes, &c.name);
                let Some(tex) = &skin.texture else {
                    continue;
                };
                let Ok((m, _)) = find_soldier(inner, &skin.geometry) else {
                    continue; // not a soldier model of this side
                };
                let skins = model_skins(&m, &classes);
                let has = |g: &str| skins.groups.get(g).is_some_and(|l| !l.is_empty());
                if !has("override_texture") {
                    // The model has no such group: the PS2's override
                    // list finds nothing to replace.
                    no_group.push(format!("{game} {}", c.name));
                    continue;
                }
                assert!(texs.contains(tex), "{game} {}: {tex} not in {side}", c.name);
                if let Some(t2) = &skin.texture2 {
                    assert!(has("override_texture2"), "{game} {}: {}", c.name, m.name);
                    assert!(texs.contains(t2), "{game} {}: {t2} not in {side}", c.name);
                }
                assert!(skins
                    .overrides
                    .contains(&(tex.clone(), "override_texture".into())));
                checked += 1;
            }
        }
    }
    // 16 BF1 IA classes and 9 BF2 ones author one (unit-audit-bf1 /
    // bf2 3.3), plus the classes outside the IA lists.
    assert!(checked >= 25, "{checked} classes with an override");
    println!("{checked} classes draw an override; none to replace: {no_group:?}");
    // [D]: `all_inf_engineer` has no MNAM group, so the snow engineer's
    // `all_inf_engineer_snow` replaces nothing on the PS2 either.
    assert_eq!(no_group, ["bf2 all_inf_engineer_snow"]);
}

/// The second override counts only with the first (both draws test it
/// inside the first's branch).
#[test]
fn a_second_override_alone_is_not_drawn() {
    let def = |name: &str, base: &str, props: &[(&str, &str)]| manifest::ClassDef {
        name: name.into(),
        kind: "entc".into(),
        base: base.into(),
        props: props
            .iter()
            .map(|(k, v)| (mungefs::fnv1a_lowercase(k), (*v).to_owned()))
            .collect(),
    };
    let classes = [
        def(
            "a",
            "soldier",
            &[("GeometryName", "m"), ("OverrideTexture2", "x2")],
        ),
        def("b", "a", &[("OverrideTexture", "X1")]),
    ];
    assert_eq!(class_skin(&classes, "a").texture2, None);
    let b = class_skin(&classes, "b");
    assert_eq!(
        (
            b.geometry.as_str(),
            b.texture.as_deref(),
            b.texture2.as_deref()
        ),
        ("m", Some("x1"), Some("x2"))
    );
}

/// re-softskin.md §8 acceptance: the `human` rig's rest pose (zaf_ JONT
/// quats/positions) equals the model skel's bind pose bone for bone.
#[test]
fn human_rig_rest_matches_stormtrooper_bind() {
    let imp = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
    );
    let common = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL"
    );
    if !std::path::Path::new(imp).exists() || !std::path::Path::new(common).exists() {
        eprintln!("SKIP: IMP.LVL / COMMON.LVL not present");
        return;
    }
    let ibuf = std::fs::read(imp).unwrap();
    let (_, skel) = find_soldier(mungefs::open(&ibuf).unwrap(), "imp_inf_stormtrooper").unwrap();
    let cbuf = std::fs::read(common).unwrap();
    let rigs = mungefs::anim::find_rigs(mungefs::open(&cbuf).unwrap()).unwrap();
    let rig = rigs.iter().find(|r| r.name == "human_animation_0").unwrap();
    let bones = bones_of(&skel);
    let mut matched = 0;
    for (b, sb) in bones.iter().zip(&skel.bones) {
        let Some(j) = rig.joint(sb.crc) else { continue };
        matched += 1;
        let q = Quat::from_array(b.rotation);
        let r = Quat::from_array(j.rest_rot);
        assert!(
            q.dot(r).abs() > 0.999,
            "{}: model {q:?} vs rig {r:?}",
            sb.name
        );
        let dp = Vec3::from_array(b.translation) - Vec3::from_array(j.rest_pos);
        assert!(dp.length() < 1e-3, "{}: {dp:?}", sb.name);
    }
    assert_eq!(matched, 25, "every model bone has a rig joint");
}

/// Data-gated: the side bake writes BOTH tiers side by side, and the
/// classic-plus install cannot reach the base file — the PS2 pixels are
/// what `<name>.ktx2` holds whether or not a PC SIDE LVL was supplied,
/// and the override sits beside it as `<name>.tier2.ktx2`.
///
/// The super battle droid is the case with a PC counterpart AND a sheet a
/// specular pass draws, so both files must carry the PS2's gloss mask.
#[test]
fn side_textures_write_both_tiers_and_the_base_is_always_ps2() {
    let cis = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL"
    );
    let pc_cis = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/side/cis.lvl"
    );
    if !std::path::Path::new(cis).exists() || !std::path::Path::new(pc_cis).exists() {
        eprintln!("SKIP: BF2 CIS.LVL or the PC side/cis.lvl is not present");
        return;
    }
    let root = std::env::temp_dir().join(format!("openbf_tier2_{}", std::process::id()));
    let (with_pc, ps2_only) = (root.join("with_pc"), root.join("ps2_only"));
    let _ = std::fs::remove_dir_all(&root);
    bake_soldier_anims(
        cis,
        "cis_inf_sbdroid",
        None,
        with_pc.to_str().unwrap(),
        Some(pc_cis),
    )
    .unwrap();
    bake_soldier(cis, "cis_inf_sbdroid", ps2_only.to_str().unwrap()).unwrap();

    let base = with_pc.join("textures/cis_inf_sbdroid.ktx2");
    let tier2 = with_pc.join("textures/cis_inf_sbdroid.tier2.ktx2");
    let dims = |p: &std::path::Path| {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let r = ktx2::Reader::new(&bytes).unwrap();
        (r.header().pixel_width, r.header().pixel_height)
    };
    assert_eq!(
        dims(&base),
        (256, 256),
        "the base is the disc's own texture"
    );
    assert_eq!(dims(&tier2), (512, 512), "the PC override sits beside it");
    assert_eq!(
        std::fs::read(&base).unwrap(),
        std::fs::read(ps2_only.join("textures/cis_inf_sbdroid.ktx2")).unwrap(),
        "the PC install must not change one byte of the base file"
    );
    // The droid's sheet is drawn by a specular pass, so BOTH tiers carry
    // the PS2's gloss mask (queue DSHN: disc mean 34/255) — the override's
    // own alpha is a different channel (see `tier`), so it takes the PS2's.
    for p in [&base, &tier2] {
        let bytes = std::fs::read(p).unwrap();
        let r = ktx2::Reader::new(&bytes).unwrap();
        let level0 = r.levels().next().unwrap();
        let n = level0.data.len() / 4;
        let mean = level0
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| f64::from(px[3]))
            .sum::<f64>()
            / n as f64;
        assert!(
            (20.0..50.0).contains(&mean),
            "{}: the PS2 gloss mask, got alpha mean {mean:.1}",
            p.display()
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// Data-gated regression, both halves of the droid's sheet. Its alpha is
/// NOT a cutout: every texel is below the 0.5 alpha test the renderer
/// once ran on every draw (the whole model went invisible), so the GLB
/// material stays OPAQUE — no segment's word has 0x02 or 0x04. But it IS
/// the specular pass's gloss weight (queue DSHN, `analysis/re-spec-gloss.md`
/// §3: disc mean 34/255), so the baked sheet keeps it; at 255 the pass adds
/// at full weight and the droid reads as chrome.
#[test]
fn a_pass_sheet_keeps_its_gloss_alpha_and_draws_opaque() {
    let cis = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL"
    );
    if !std::path::Path::new(cis).exists() {
        eprintln!("SKIP: CIS.LVL not present");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openbf_alpha_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    bake_soldier(cis, "cis_inf_sbdroid", dir.to_str().unwrap()).unwrap();
    let bytes = std::fs::read(dir.join("textures/cis_inf_sbdroid.ktx2")).unwrap();
    let r = ktx2::Reader::new(&bytes).unwrap();
    let level0 = r.levels().next().unwrap();
    let alpha: Vec<u8> = level0
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect();
    let mean = alpha.iter().map(|&a| f64::from(a)).sum::<f64>() / alpha.len() as f64;
    assert!(
        (20.0..50.0).contains(&mean),
        "the sheet keeps the disc's gloss mask (mean 34), got mean {mean:.1}"
    );
    let glb = std::fs::read(dir.join("soldiers/cis_inf_sbdroid.glb")).unwrap();
    assert!(
        !glb.windows(9).any(|w| w == b"alphaMode"),
        "the droid's materials stay OPAQUE"
    );
    // And the flag words really are what decide it: no cutout or
    // transparent material, and a specular pass on the sheet.
    let buf = std::fs::read(cis).unwrap();
    let inner = mungefs::open(&buf).unwrap();
    let m = mungefs::content_chunks(inner)
        .unwrap()
        .iter()
        .filter(|c| &c.tag == b"modl")
        .map(|c| mungefs::model::Model::parse(c).unwrap())
        .find(|m| m.name == "cis_inf_sbdroid")
        .expect("droid model");
    assert!(
        m.segments
            .iter()
            .all(|s| s.material_flags & crate::detail::ALPHA_IS_MEANINGFUL == 0),
        "droid segments claim no cutout/transparent material"
    );
    assert!(crate::spec::pass_textures([&m]).contains("cis_inf_sbdroid"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Data-gated: a BF1 base texture with detailCompressed gets its `_dtl`
/// companion baked as `<base>.dtl.ktx2` at 4x the base size, and the
/// companion does not also appear under its own source name.
#[test]
fn bf1_soldier_textures_get_detail_companions() {
    let all = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL"
    );
    if !std::path::Path::new(all).exists() {
        eprintln!("SKIP: ALL.LVL not present");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openbf_detail_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let b = bake_soldier(all, "all_inf_soldier", dir.to_str().unwrap()).unwrap();
    assert!(b.textures.contains(&"all_inf_trooper".to_string()));
    let tex = dir.join("textures");
    let dims = |file: &str| {
        let bytes = std::fs::read(tex.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let r = ktx2::Reader::new(&bytes).unwrap();
        (r.header().pixel_width, r.header().pixel_height)
    };
    assert_eq!(dims("all_inf_trooper.ktx2"), (64, 64));
    assert_eq!(dims("all_inf_trooper.dtl.ktx2"), (256, 256));
    assert_eq!(dims("rebel_face01.ktx2"), (32, 32));
    assert_eq!(dims("rebel_face01.dtl.ktx2"), (128, 128));
    assert!(
        !tex.join("all_inf_trooper_dtl.ktx2").exists(),
        "the companion is baked as <base>.dtl.ktx2, not as its own texture"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Clips bake onto the stormtrooper: in place (DummyRoot stripped), root
/// motion recorded, idle 71 fr / run 19 fr.
#[test]
fn stormtrooper_bakes_with_clips() {
    let imp = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
    );
    let common = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL"
    );
    if !std::path::Path::new(imp).exists() || !std::path::Path::new(common).exists() {
        eprintln!("SKIP: IMP.LVL / COMMON.LVL not present");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openbf_soldier_anims_{}", std::process::id()));
    let b = bake_soldier_anims(
        imp,
        "imp_inf_stormtrooper",
        Some(common),
        dir.to_str().unwrap(),
        None,
    )
    .unwrap();
    // BF1's banks hold every seat clip but BF2's two (card VRIDE).
    assert_eq!(
        b.missing_clips,
        ["human_barc_9pose", "human_drive_1manatst"],
        "missing clips"
    );
    // The list and the eight seat clips first, in their order, then the
    // other clips of BF1's sets: 151 in the tables, 33 of them in the
    // list (`human_rifle_die` is in no table), none a seat clip.
    // Four one-frame hit poses are also outside BF1's special table.
    assert_eq!(b.clips, 164);
    let meta: SoldierAnims = ron::from_str(
        &std::fs::read_to_string(dir.join("soldiers/imp_inf_stormtrooper.anims.ron")).unwrap(),
    )
    .unwrap();
    for (c, name) in meta.clips.iter().zip(
        SOLDIER_CLIPS
            .iter()
            .chain(SEAT_CLIPS)
            .filter(|n| !b.missing_clips.iter().any(|m| m == *n)),
    ) {
        assert_eq!(c.name, *name);
    }
    let get = |n: &str| meta.clips.iter().find(|c| c.name == n).unwrap().clone();
    for direction in ["front", "back", "left", "right"] {
        assert_eq!(get(&format!("human_rifle_stand_hit{direction}")).frames, 1);
    }
    // The seat clips (cards VTURG, VRIDE): the turrets' 9, 9 and 1
    // frames, the riders' (re-vehicle-riders 4.2: `human_ride` 61), from
    // BF1's bank.
    for (n, f) in [
        ("human_minigun_9pose", 9),
        ("human_dishcannon_9pose", 9),
        ("human_drive", 1),
        ("human_speederbike_9pose", 9),
        ("human_ride", 61),
    ] {
        let c = get(n);
        println!("{n}: {} frames, bank {}", c.frames, c.bank);
        assert_eq!((c.frames, c.bank.as_str()), (f, "human_animation_1"), "{n}");
    }
    for n in ["human_stap_ride", "human_man_minigun", "human_standing"] {
        let c = get(n);
        println!("{n}: {} frames, bank {}", c.frames, c.bank);
    }
    assert_eq!(get("human_rifle_stand_idle_emote").frames, 71);
    let run = get("human_rifle_stand_runforward");
    assert_eq!(run.frames, 19);
    assert!(
        (run.root_motion[2] - 4.014).abs() < 0.01,
        "{:?}",
        run.root_motion
    );
    assert!((get("human_rifle_diveforward").root_motion[2] - 5.404).abs() < 0.01);
    let (doc, _, _) = gltf::import(dir.join("soldiers/imp_inf_stormtrooper.glb")).unwrap();
    assert_eq!(doc.animations().count(), 164);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Data-gated Tier-1: the BF1 stormtrooper bakes to a 25-joint skinned
/// GLB whose model-space extents equal the modl vis box, and its texture.
#[test]
fn stormtrooper_bakes() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
    );
    if !std::path::Path::new(p).exists() {
        eprintln!("SKIP: IMP.LVL not present");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openbf_soldier_{}", std::process::id()));
    let b = bake_soldier(p, "imp_inf_stormtrooper", dir.to_str().unwrap()).unwrap();
    assert_eq!((b.bones, b.vertices, b.triangles), (25, 1387, 1630));
    assert!((b.aabb.1[1] - 2.08769).abs() < 1e-3, "{:?}", b.aabb);
    assert!((b.aabb.0[0] + 0.929476).abs() < 1e-3);
    assert_eq!(b.textures, vec!["imp_inf_trooper".to_string()]);
    let glb = dir.join("soldiers/imp_inf_stormtrooper.glb");
    let (doc, buffers, _) = gltf::import(&glb).unwrap();
    let skin = doc.skins().next().unwrap();
    assert_eq!(skin.joints().count(), 25);
    let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
    let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
    assert_eq!(reader.read_positions().unwrap().count(), 1387);
    let joints: Vec<[u16; 4]> = reader.read_joints(0).unwrap().into_u16().collect();
    assert_eq!(joints[0], [15, 0, 0, 0]);
    assert!(dir.join("textures/imp_inf_trooper.ktx2").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The BF1 stormtrooper rifle bakes as a static model with its texture.
#[test]
fn imp_rifle_bakes() {
    let imp = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
    );
    if !std::path::Path::new(imp).exists() {
        eprintln!("SKIP: IMP.LVL not present");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openbf_weapon_{}", std::process::id()));
    let w = bake_weapon(imp, "imp_weap_inf_rifle", dir.to_str().unwrap(), None).unwrap();
    assert!(
        w.vertices > 50 && w.triangles > 30,
        "{} / {}",
        w.vertices,
        w.triangles
    );
    assert!(!w.textures.is_empty(), "rifle texture");
    assert!(dir.join("weapons/imp_weap_inf_rifle.glb").exists());
    assert!(dir.join("weapons/imp_weap_inf_rifle.nodes.ron").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Card FXMF: the fire points equal the PS2 RAM's class+0x30
/// (`analysis/re-muzzle-flash.md` §4.1, `ramflash.py`).
#[test]
fn weapon_fire_points_are_the_ram_values() {
    let side = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/"
    );
    for (lvl, model, want) in [
        ("REP.LVL", "rep_weap_inf_rifle", [0.0, 0.0533, 1.097]),
        ("CIS.LVL", "cis_weap_inf_pistol", [0.0103, 0.0394, 0.2601]),
        ("ALL.LVL", "all_weap_inf_rifle", [0.0, 0.0597, 0.6926]),
        ("IMP.LVL", "imp_weap_inf_rifle", [0.0, 0.0422, 0.4082]),
    ] {
        let Ok(buf) = std::fs::read(format!("{side}{lvl}")) else {
            eprintln!("SKIP: {lvl} not present");
            continue;
        };
        let inner = mungefs::open(&buf).unwrap();
        let chunks = mungefs::content_chunks(inner).unwrap();
        let nodes = weapon_nodes(&chunks, model);
        let fp = nodes
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case("hp_fire"))
            .unwrap_or_else(|| panic!("{model}: no hp_fire in {nodes:?}"));
        for (got, want) in fp.pos.iter().zip(want) {
            assert!((got - want).abs() < 5e-4, "{model}: {:?}", fp.pos);
        }
    }
}
