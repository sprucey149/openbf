use super::*;
use manifest::combo::ComboValue as V;

fn animation(name: &str, scoped: bool, aim: Option<&str>) -> ComboField {
    ComboField {
        keyword: K::Animation,
        args: vec![V::Text(name.into())],
        scoped,
        children: aim
            .map(|name| ComboField {
                keyword: K::AimType,
                args: vec![V::Text(name.into())],
                scoped: false,
                children: Vec::new(),
            })
            .into_iter()
            .collect(),
    }
}

#[test]
fn an_empty_scope_locks_none_while_forward_declarations_do_not_lock() {
    let program = |fields| ComboDef {
        name_hash: 1,
        fields,
    };
    let mut none = Descriptors::default();
    none.register(&program(vec![
        animation("same", false, None),
        animation("same", true, None),
        animation("same", true, Some("Torso")),
    ]));
    assert_eq!(none.upper("same").aim, AimKind::None);
    let mut torso = Descriptors::default();
    torso.register(&program(vec![
        animation("same", false, None),
        animation("same", true, Some("Torso")),
        animation("same", true, None),
    ]));
    assert_eq!(torso.upper("same").aim, AimKind::Torso);
    assert_eq!(torso.layers["same"][1].unwrap().aim, AimKind::None);
}

fn property(keyword: K, args: Vec<V>) -> ComboField {
    ComboField {
        keyword,
        args,
        scoped: false,
        children: Vec::new(),
    }
}

fn described(name: &str, fields: Vec<ComboField>) -> ComboField {
    let mut field = animation(name, true, None);
    field.children = fields;
    field
}

#[test]
fn consumed_metadata_keeps_last_recognized_aim_type() {
    let aim = |name: &str| property(K::AimType, vec![V::Text(name.into())]);
    let program = ComboDef {
        name_hash: 1,
        fields: vec![
            described("last_aim", vec![aim("Torso"), aim("Head"), aim("unknown")]),
            described("full_body", vec![aim("Head"), aim("FullBody")]),
        ],
    };
    let mut descriptors = Descriptors::default();
    descriptors.register(&program);
    assert_eq!(descriptors.upper("last_aim").aim, AimKind::Head);
    assert_eq!(descriptors.upper("full_body").aim, AimKind::None);
}

#[test]
fn consumed_metadata_blend_order_units_and_clamps_follow_native_pack() {
    let seconds = |value| property(K::BlendInTime, vec![V::Number(value)]);
    let mut frames = seconds(6.0);
    frames.args.push(V::Text("Frames".into()));
    let program = ComboDef {
        name_hash: 1,
        fields: vec![
            described("last_blend", vec![seconds(0.3), seconds(0.06)]),
            described("frames", vec![frames]),
            described("negative", vec![seconds(-1.0)]),
            described("saturated", vec![seconds(3.0)]),
            described(
                "alias",
                vec![property(K::BlendTimeIn, vec![V::Number(0.15)])],
            ),
        ],
    };
    let mut descriptors = Descriptors::default();
    descriptors.register(&program);
    // FUN279780 MIPS: optional Frames *0x3d088889, *100, clamp0..255,
    // CVT then SB(+3). FUN27f458 decodes the byte *0x3c23d70a using EE RZ.
    let actual = ["last_blend", "frames", "negative", "saturated", "alias"]
        .map(|name| descriptors.upper(name).blend_in.to_bits());
    assert_eq!(actual, [0x3d4c_cccc, 0x3e4c_cccc, 0, 0x4023_3332, 0]);
}

fn felucia() -> MissionDef {
    MissionDef::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions/bf2_fel1_con_cw.ron"),
    )
    .unwrap()
}

#[test]
fn original_felucia_and_assault_orders_keep_different_shared_aayla_descriptors() {
    let root = crate::assets::baked_root();
    if !root.join("side_bf2/combos/rep.ron").is_file() {
        eprintln!("SKIP: actual BF2 combo bake required");
        return;
    }
    let mut mission = felucia();
    let own = for_mission(&root.join("classes"), &mission).unwrap();
    // Fresh original FeluciaCW: all three states share upper 0x05220094.
    assert_eq!(own.upper("stand_attack1c").aim, AimKind::Torso);
    assert_eq!(own.layers["stand_attack1c"][1].unwrap().aim, AimKind::None);
    assert_eq!(own.upper("stand_attack1c").blend_in, 5.0 * 0.01);
    assert_eq!(own.upper("stand_attack1a").aim, AimKind::None);
    assert_eq!(own.upper("stand_dashattack").aim, AimKind::None);
    // Exact tat2g_eli ReadDataFile SIDE call/argument order from PS2 bytecode.
    mission.class_load_order = [
        (
            "all",
            &[
                "all_hero_luke_jedi",
                "all_hero_hansolo_tat",
                "all_hero_leia",
                "all_hero_chewbacca",
            ][..],
        ),
        (
            "imp",
            &[
                "imp_hero_darthvader",
                "imp_hero_emperor",
                "imp_hero_bobafett",
            ][..],
        ),
        (
            "rep",
            &[
                "rep_hero_yoda",
                "rep_hero_macewindu",
                "rep_hero_anakin",
                "rep_hero_aalya",
                "rep_hero_kiyadimundi",
                "rep_hero_obiwan",
            ][..],
        ),
        (
            "cis",
            &[
                "cis_hero_grievous",
                "cis_hero_darthmaul",
                "cis_hero_countdooku",
                "cis_hero_jangofett",
            ][..],
        ),
    ]
    .into_iter()
    .flat_map(|(side, names)| {
        names.iter().map(move |name| crate::mission::SideClassLoad {
            side: side.into(),
            request: (*name).into(),
        })
    })
    .collect();
    let shared = for_mission(&root.join("classes"), &mission).unwrap();
    // Original assault Aayla: shared upper 0x05200095, locked by an earlier hero.
    assert_eq!(shared.upper("stand_attack1c").aim, AimKind::None);
    mission.class_load_order.insert(
        0,
        crate::mission::SideClassLoad {
            side: "rep".into(),
            request: "rep_hero_aalya".into(),
        },
    );
    assert_eq!(
        for_mission(&root.join("classes"), &mission)
            .unwrap()
            .upper("stand_attack1c")
            .aim,
        AimKind::Torso
    );
}

#[test]
fn every_bf2_mission_resolves_only_its_ordered_native_resource_programs() {
    let root = crate::assets::baked_root();
    if !root.join("side_bf2/combos/rep.ron").is_file() {
        eprintln!("SKIP: actual BF2 combo bake required");
        return;
    }
    let missions = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions");
    let mut count = 0;
    for path in pak::read_dir(&missions).unwrap() {
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let mission = MissionDef::load(&path).unwrap();
        if mission.source_game != "bf2" {
            continue;
        }
        for_mission(&root.join("classes"), &mission)
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        count += 1;
    }
    assert_eq!(count, 34);
}

#[test]
fn aayla_default_changes_only_her_shared_third_attack_aiming() {
    let root = crate::assets::baked_root();
    if !root.join("side_bf2/combos/rep.ron").is_file() {
        eprintln!("SKIP: actual BF2 combo bake required");
        return;
    }
    let mission = felucia();
    let classes = root.join("classes");
    let native = for_mission(&classes, &mission).unwrap();
    assert_eq!(native.upper("stand_attack1c").aim, AimKind::Torso);
    let mut expected = native.clone();
    expected.layers.get_mut("stand_attack1c").unwrap()[0]
        .as_mut()
        .unwrap()
        .aim = AimKind::None;
    assert_eq!(
        for_hero(&classes, &mission, "REP_HERO_AALYA").unwrap(),
        expected
    );
    assert_eq!(
        for_hero(&classes, &mission, "rep_hero_obiwan").unwrap(),
        native
    );
    assert_eq!(for_mission(&classes, &mission).unwrap(), native);
}
