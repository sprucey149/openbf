use super::*;
use glam::Vec3;
use manifest::hud::HudLayout;
use ui::hud_tree::MeshInfo;

fn fixture() -> Hud {
    let layout: HudLayout = ron::from_str(
        r#"(name:"test", nodes:[
            (name:"Model3D", args:["player1weapon1mesh"], children:[
                (name:"MeshInfo", args:["rep_weap_inf_fusioncutter"], children:[
                    (name:"Position", args:[1,2,3]),
                    (name:"Rotation", args:[4,5,6]),
                    (name:"Scale", args:[7,8,9]),
                ]),
                (name:"MeshInfo", args:["other_repair"], children:[
                    (name:"Position", args:[10,11,12]),
                ]),
            ]),
            (name:"Model3D", args:["player1weapon2mesh"], children:[
                (name:"MeshInfo", args:["hud_thermaldetonator"], children:[
                    (name:"Position", args:[13,14,15]),
                    (name:"Rotation", args:[16,17,18]),
                    (name:"Scale", args:[19,20,21]),
                ]),
            ]),
        ])"#,
    )
    .unwrap();
    Hud::new(
        &layout,
        &HudLayout {
            name: String::new(),
            nodes: vec![],
        },
    )
}

#[test]
fn secondary_cutter_uses_authored_pose_and_row_position_only_once() {
    let mut hud = fixture();
    let before = hud.elems.clone();
    place_secondary_cutters(&mut hud, |_| true);
    let info = hud.elems[1].mesh_info.last().unwrap();
    assert_eq!(info.mesh, "rep_weap_inf_fusioncutter");
    assert_eq!(info.pos, Vec3::new(13.0, 14.0, 15.0));
    assert_eq!(info.rot, Vec3::new(4.0, 5.0, 6.0));
    assert_eq!(info.scale, Vec3::new(7.0, 8.0, 9.0));
    assert_eq!(hud.elems[1].mesh_info.len(), before[1].mesh_info.len() + 1);
    place_secondary_cutters(&mut hud, |_| true);
    assert_eq!(hud.elems[1].mesh_info.len(), before[1].mesh_info.len() + 1);
    hud.elems[1].mesh_info.pop();
    assert_eq!(format!("{:?}", hud.elems), format!("{before:?}"));
}

#[test]
fn secondary_cutter_keeps_missing_model_pose_and_existing_placement_fallbacks() {
    for missing in [
        "model",
        "primary",
        "anchor",
        "primary_position",
        "anchor_position",
        "element",
    ] {
        let mut hud = fixture();
        match missing {
            "primary" => hud.elems[0].mesh_info.clear(),
            "anchor" => hud.elems[1].mesh_info.clear(),
            "primary_position" => hud.elems[0].mesh_info[0].pos = Vec3::ZERO,
            "anchor_position" => hud.elems[1].mesh_info[0].pos = Vec3::ZERO,
            "element" => hud.elems[1].name = "other".into(),
            _ => {}
        }
        let before = format!("{:?}", hud.elems);
        place_secondary_cutters(&mut hud, |_| missing != "model");
        assert_eq!(format!("{:?}", hud.elems), before, "{missing}");
    }
    let mut hud = fixture();
    hud.elems[1].mesh_info.push(MeshInfo {
        mesh: "rep_weap_inf_fusioncutter".into(),
        pos: Vec3::splat(22.0),
        rot: Vec3::splat(23.0),
        scale: Vec3::splat(24.0),
    });
    let before = format!("{:?}", hud.elems);
    place_secondary_cutters(&mut hud, |_| true);
    assert_eq!(format!("{:?}", hud.elems), before);
}

#[test]
#[ignore = "requires UI_SHARP_ASSETS owned source bake"]
fn owned_cutters_draw_the_indexed_bf2_geometry() {
    let assets = PathBuf::from(std::env::var_os("UI_SHARP_ASSETS").expect("UI_SHARP_ASSETS"));
    let mut view = HudView::load(&assets, render::HEADLESS_FORMAT, None).unwrap();
    for side in ["rep", "cis", "all", "imp"] {
        let name = format!("{side}_weap_inf_fusioncutter");
        let file = assets.join(format!("side_bf2/weapons/{name}.glb"));
        assert_eq!(view.mesh_files[&name], file);
        let weapon = HudWeapon {
            key: name.clone(),
            ..Default::default()
        };
        view.update(
            &HudModel {
                weapon: Some(weapon.clone()),
                weapon2: Some(weapon),
                ..Default::default()
            },
            0.0,
            [1920, 1080],
        );
        let expected = load_mesh(&file).unwrap();
        let actual = &view.art.meshes[&name];
        assert_eq!(actual.parts.len(), expected.parts.len());
        for (actual, expected) in actual.parts.iter().zip(&expected.parts) {
            assert!(
                actual.texture == expected.texture
                    && actual.positions == expected.positions
                    && actual.indices == expected.indices
                    && actual.uvs == expected.uvs
                    && actual.colors == expected.colors
                    && actual.scroll == expected.scroll,
                "{name}: drawn geometry differs"
            );
        }
        println!(
            "{name}: indexed and drawn from {} ({} parts)",
            file.display(),
            actual.parts.len()
        );
    }
}
