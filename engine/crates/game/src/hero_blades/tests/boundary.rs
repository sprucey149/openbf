use super::*;
use crate::skin::BoneXform;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);
struct Fixture {
    root: std::path::PathBuf,
    db: ClassDb,
}
impl Fixture {
    fn new(defs: Vec<crate::classdb::ClassDef>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "openbf-hero-input-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("classes")).unwrap();
        let path = root.join("classes/bf2_input.ron");
        std::fs::write(&path, ron::to_string(&defs).unwrap()).unwrap();
        let mut db = ClassDb::default();
        db.load(&path).unwrap();
        Self { root, db }
    }
    fn write<T: serde::Serialize>(&self, path: &std::path::Path, value: &T) {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, ron::to_string(value).unwrap()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn blade_parser_accepts_eight_edges_and_refuses_nine_before_loading_assets() {
    let make = |count: usize, declared: Option<usize>| {
        let prop = |name: &str, value: String| (crate::classdb::fnv1a_lowercase(name), value);
        let mut props = Vec::new();
        if let Some(declared) = declared {
            props.push(prop("NumDamageEdges", declared.to_string()));
        }
        for edge in 0..count {
            props.extend([
                prop("AttachedFirePoint", format!("bone_{edge} 0 0 1 0 0 0")),
                prop("DamageEdgeLength", "1.2".into()),
                prop("DamageEdgeWidth", "0.1".into()),
                prop("LightSaberTexture", "lightsabre".into()),
            ]);
        }
        Fixture::new(vec![crate::classdb::ClassDef {
            name: "boundary_saber".into(),
            kind: "wpnc".into(),
            base: "melee".into(),
            props,
        }])
    };
    for declared in [None, Some(8)] {
        let fixture = make(8, declared);
        let blades = blade_bindings(&fixture.root, &fixture.db, "boundary_saber").unwrap();
        assert_eq!(blades.len(), core::hero_weapon::MAX_BLADE_EDGES);
    }
    for (count, declared) in [(9, None), (9, Some(9)), (1, Some(usize::MAX))] {
        let fixture = make(count, declared);
        let error = blade_bindings(&fixture.root, &fixture.db, "boundary_saber").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("boundary_saber"), "{message}");
        assert!(
            message.contains('8') && message.contains("visibility mask"),
            "{message}"
        );
    }
}

#[test]
fn untextured_zero_length_melee_edges_keep_their_axis_for_combo_reach() {
    let prop = |name: &str, value: &str| (crate::classdb::fnv1a_lowercase(name), value.into());
    let edge_props = vec![
        prop("AttachedFirePoint", "bone_head 1 0 0 0 0 0"),
        prop("LightSaberLength", "0.0"),
        prop("LightSaberWidth", "0.08"),
    ];
    let fixture = Fixture::new(vec![
        crate::classdb::ClassDef {
            name: "paw".into(),
            kind: "wpnc".into(),
            base: "melee".into(),
            props: edge_props,
        },
        crate::classdb::ClassDef {
            name: "visible_paw".into(),
            kind: "wpnc".into(),
            base: "paw".into(),
            props: vec![prop("LightSaberTexture", "lightsabre")],
        },
    ]);
    let bindings = blade_bindings(&fixture.root, &fixture.db, "paw").unwrap();
    let binding = &bindings[0];
    assert_eq!(binding.blade.length, 0.0);
    assert_eq!(binding.local.end - binding.local.start, Vec3::X);
    assert!(blade_bindings(&fixture.root, &fixture.db, "visible_paw").is_err());
}

#[test]
fn weapon_node_float_inputs_are_rejected_before_creating_physical_edges() {
    let prop = |name: &str, value: &str| (crate::classdb::fnv1a_lowercase(name), value.into());
    let f = Fixture::new(vec![crate::classdb::ClassDef {
        name: "hostile_saber".into(),
        kind: "wpnc".into(),
        base: "melee".into(),
        props: vec![
            prop("GeometryName", "hostile_hilt"),
            prop("LightSaberLength", "1.2"),
            prop("LightSaberWidth", "0.1"),
        ],
    }]);
    let path = manifest::weapon_nodes_path(std::path::Path::new("side_bf2"), "hostile_hilt");
    let node = manifest::ManifestHardpoint {
        name: "hp_fire".into(),
        pos: [-0.5, 0.0, 0.0],
        axes: Some([[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]),
    };
    f.write(&path, &vec![node.clone()]);
    let valid = blade_bindings(&f.root, &f.db, "hostile_saber").unwrap();
    assert_eq!(valid[0].local.start, Vec3::new(-0.5, 0.0, 0.0));
    assert!((valid[0].local.end.x - 0.7).abs() < 1e-6);
    let mut accepted = Vec::new();
    for component in 0..12 {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = node.clone();
            if component < 3 {
                bad.pos[component] = value;
            } else {
                bad.axes.as_mut().unwrap()[(component - 3) / 3][(component - 3) % 3] = value;
            }
            f.write(&path, &vec![bad]);
            if blade_bindings(&f.root, &f.db, "hostile_saber").is_ok() {
                accepted.push((component, value));
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted nonfinite node inputs {accepted:?}"
    );
}

fn binding() -> BladeBinding {
    BladeBinding {
        blade: Blade {
            node: "hp_fire".into(),
            bone: "hp_weapons".into(),
            length: 1.2,
            width: 0.1,
            texture: String::new(),
            trail_color: None,
        },
        geometry: None,
        glow_texture: None,
        local: BladeSegment {
            start: Vec3::ZERO,
            end: Vec3::Z * 1.2,
        },
    }
}

#[test]
fn sampled_pose_float_inputs_cannot_become_nonfinite_edges_or_group_zero() {
    let skel = Skeleton::new(vec!["hp_weapons".into()], vec![None], vec![Mat4::IDENTITY]);
    let mut accepted = Vec::new();
    for component in 0..7 {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut key = BoneXform::IDENTITY;
            if component < 3 {
                key.translation[component] = value;
            } else {
                let mut q = key.rotation.to_array();
                q[component - 3] = value;
                key.rotation = glam::Quat::from_array(q);
            }
            if sampled_edges(&skel, &Pose(vec![key]), &[binding()], Some(0)).is_ok() {
                accepted.push((component, value));
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted nonfinite pose inputs {accepted:?}"
    );
    // A tiny but invertible bind is valid; no arbitrary determinant epsilon.
    let tiny = Skeleton::new(
        vec!["hp_weapons".into()],
        vec![None],
        vec![Mat4::from_scale(Vec3::splat(0.000001))],
    );
    assert!(sampled_edges(
        &tiny,
        &Pose(vec![BoneXform::IDENTITY]),
        &[binding()],
        Some(0)
    )
    .is_ok());
}

impl Fixture {
    /// `None` (a printed SKIP) when the BF2 base bake is absent; a base
    /// bake without the hero outputs still fails (a stale bake).
    fn maul() -> Option<Self> {
        let root = assets::baked_root();
        if !root.join("classes/bf2_cis.ron").is_file() {
            eprintln!("SKIP: classes/bf2_cis.ron not baked (actual BF2 hero classes required)");
            return None;
        }
        let original = crate::roster::load_classdb(&root.join("classes"), "bf2", &["cis"]).unwrap();
        let f = Self::new(
            original
                .names()
                .map(|name| original.get(name).unwrap().clone())
                .collect(),
        );
        let model = "cis_inf_darthmaul";
        std::fs::create_dir_all(f.root.join("side_bf2/soldiers")).unwrap();
        for suffix in ["glb", "combos.ron", "sets.ron", "motion.ron"] {
            let path = format!("side_bf2/soldiers/{model}.{suffix}");
            std::fs::write(f.root.join(&path), pak::read(root.join(&path)).unwrap()).unwrap();
        }
        for edge in edges(&f.db, &f.weapon()).unwrap() {
            if let Some(model) = edge.geometry {
                let path = manifest::weapon_nodes_path(std::path::Path::new("side_bf2"), &model);
                std::fs::create_dir_all(f.root.join(&path).parent().unwrap()).unwrap();
                std::fs::write(f.root.join(&path), pak::read(root.join(&path)).unwrap()).unwrap();
            }
        }
        Some(f)
    }
    fn weapon(&self) -> String {
        self.db
            .weapon_slots("cis_hero_darthmaul")
            .into_iter()
            .find(|slot| slot.channel == 0)
            .unwrap()
            .weapon
    }
    fn saber(&self) -> anyhow::Result<SaberClass> {
        saber_class(&self.db, "cis_hero_darthmaul", &self.weapon())
    }
}

#[test]
fn real_hero_root_float_inputs_are_rejected_at_the_class_boundary() {
    let Some(f) = Fixture::maul() else {
        return;
    };
    assert!(f.saber().is_ok());
    let prefix = "side_bf2/soldiers/cis_inf_darthmaul";
    let motions_path = std::path::PathBuf::from(format!("{prefix}.motion.ron"));
    let sets_path = std::path::PathBuf::from(format!("{prefix}.sets.ron"));
    let original_motions: manifest::anim_sets::ClipMotions =
        ron::from_str(&pak::read_to_string(f.root.join(&motions_path)).unwrap()).unwrap();
    let original_sets: manifest::anim_sets::SoldierSets =
        ron::from_str(&pak::read_to_string(f.root.join(&sets_path)).unwrap()).unwrap();
    let name = "maul_sabre_jumpattack_end_full";
    assert!(original_motions.contains_key(name));
    let mut accepted = Vec::new();
    for family in 0..2 {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut motions = original_motions.clone();
            let mut sets = original_sets.clone();
            if family == 0 {
                motions.get_mut(name).unwrap()[0][2] = value;
            } else {
                sets.root_speed.insert(name.into(), [0.0, value]);
            }
            f.write(&motions_path, &motions);
            f.write(&sets_path, &sets);
            if f.saber().is_ok() {
                accepted.push((family, value));
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted nonfinite real hero roots {accepted:?}"
    );
}

#[test]
fn real_maul_implicit_animated_move_uses_the_native_half_clip_packed_speed() {
    let Some(f) = Fixture::maul() else {
        return;
    };
    let saber = f.saber().unwrap();
    let clip = saber
        .clips
        .iter()
        .find(|c| c.name == "jumpattack_end")
        .unwrap();
    assert_eq!(
        clip.root_speed.map(f32::to_bits),
        [0, 0x3ecccccc],
        "native282700 measures first half, then decodes40cm/s with EE*.01"
    );
}

#[test]
fn class_damage_and_combo_float_inputs_are_rejected_before_runtime_install() {
    let Some(mut f) = Fixture::maul() else {
        return;
    };
    let definitions: Vec<_> =
        f.db.names()
            .map(|name| f.db.get(name).unwrap().clone())
            .collect();
    let weapon = f.weapon();
    let path = std::path::Path::new("classes/bf2_input.ron");
    let mut accepted = Vec::new();
    for value in ["NaN", "inf", "-inf", "invalid"] {
        let mut defs = definitions.clone();
        let class = defs.iter_mut().find(|c| c.name == weapon).unwrap();
        let key = crate::classdb::fnv1a_lowercase("Damage");
        class.props.retain(|p| p.0 != key);
        class.props.push((key, value.into()));
        f.write(path, &defs);
        f.db = ClassDb::default();
        f.db.load(&f.root.join(path)).unwrap();
        if f.saber().is_ok() {
            accepted.push(value);
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted malformed base damage {accepted:?}"
    );
}

#[test]
fn serialized_combo_numbers_cannot_bypass_the_source_parser_finite_guard() {
    let Some(f) = Fixture::maul() else {
        return;
    };
    let path = std::path::Path::new("side_bf2/soldiers/cis_inf_darthmaul.combos.ron");
    let original: ComboSet =
        ron::from_str(&pak::read_to_string(f.root.join(path)).unwrap()).unwrap();
    fn change(fields: &mut [ComboField], value: f32) -> bool {
        for field in fields {
            for arg in &mut field.args {
                if let manifest::combo::ComboValue::Number(n) = arg {
                    *n = value;
                    return true;
                }
            }
            if change(&mut field.children, value) {
                return true;
            }
        }
        false
    }
    let bank = scalar(&f.db, &f.weapon(), "ComboAnimationBank").unwrap();
    let wanted = crate::classdb::fnv1a_lowercase(bank.split_whitespace().nth(2).unwrap());
    let source = original
        .combos
        .iter()
        .find(|c| c.name_hash == wanted)
        .unwrap_or_else(|| panic!("real Maul definition absent"));
    let mut accepted = Vec::new();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut programs = original.clone();
        let combo = programs
            .combos
            .iter_mut()
            .find(|c| c.name_hash == source.name_hash)
            .unwrap();
        assert!(change(&mut combo.fields, value));
        f.write(path, &programs);
        if f.saber().is_ok() {
            accepted.push(value);
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted malformed compiled combo {accepted:?}"
    );
}

#[test]
fn real_glb_animation_float_inputs_cannot_poison_physical_tracks() {
    let Some(f) = Fixture::maul() else {
        return;
    };
    let path = f.root.join("side_bf2/soldiers/cis_inf_darthmaul.glb");
    let original = std::fs::read(&path).unwrap();
    let json_len = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
    let doc: serde_json::Value = serde_json::from_slice(&original[20..20 + json_len]).unwrap();
    let animation = doc["animations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "maul_sabre_jumpattack_end_full")
        .unwrap();
    let bin = 20 + json_len + 8;
    let mut accepted = Vec::new();
    for kind in ["translation", "rotation"] {
        let channel = animation["channels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["target"]["path"] == kind)
            .unwrap();
        let sampler = channel["sampler"].as_u64().unwrap() as usize;
        let accessor =
            &doc["accessors"][animation["samplers"][sampler]["output"].as_u64().unwrap() as usize];
        assert_eq!(accessor["componentType"], 5126);
        let view = &doc["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
        let offset = bin
            + view["byteOffset"].as_u64().unwrap_or(0) as usize
            + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = original.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            std::fs::write(&path, bad).unwrap();
            if f.saber().is_ok() {
                accepted.push((kind, value));
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted nonfinite GLB track bytes {accepted:?}"
    );
}

#[test]
fn real_maul_implicit_animated_move_reaches_the_native_packed_runtime_velocity() {
    let Some(f) = Fixture::maul() else {
        return;
    };
    let saber = f.saber().unwrap();
    let state = saber
        .combo
        .states()
        .position(|s| s.text(0) == Some("JUMPATTACK_END"))
        .unwrap();
    let clip = saber
        .clips
        .iter()
        .find(|c| c.name == "jumpattack_end")
        .unwrap();
    let mut runtime = core::combo::ComboRuntime::default();
    runtime.state = state;
    // Stage the authored airborne state; the consumer, program and root keys
    // are real. This is not a normal-input combo/unlock acceptance claim.
    let out = runtime.step(
        &saber.combo,
        &saber.state_seconds,
        core::combo::ComboInput {
            posture: core::combo::POSTURE_JUMP,
            energy: 100.0,
            animation_velocity: clip.root_speed,
            ..Default::default()
        },
        core::TICK_DT,
    );
    assert_eq!(runtime.state, state);
    assert_eq!(
        out.movement.unwrap().velocity.map(f32::to_bits),
        [0, 0x3ec28f5b],
        "40*.01,50*.02,product,*50,CVT19,19*.02 under EE rounding"
    );
}

#[test]
fn native_half_profile_uses_the_quantized_scalar_phase() {
    let mut root = vec![[0.0; 3]; 32];
    root[0][2] = f32::from_bits(0x3efead48);
    root[16][2] = 1.0;
    // Independent282700→2783b8→532b28/532c80 scalar oracle:
    // nativeframe15.499998 gives0cm/s; ideal15.5 incorrectly packs1cm/s.
    assert_eq!(
        root_speed::measure(&root).unwrap().map(f32::to_bits),
        [0, 0]
    );
}
