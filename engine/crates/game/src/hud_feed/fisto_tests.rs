use super::*;
use glam::Vec3;
use manifest::hud::HudLayout;

fn rows() -> Hud {
    let layout: HudLayout = ron::from_str(
        r#"(name:"test", nodes:[
            (name:"Model3D", args:["player1weapon1mesh"], children:[
                (name:"MeshInfo", args:["hud_droideka_shield"], children:[
                    (name:"Position", args:[1,2,3]),
                    (name:"Rotation", args:[4,5,6]),
                    (name:"Scale", args:[7,8,9]),
                ]),
            ]),
            (name:"Model3D", args:["player1weapon2mesh"], children:[
                (name:"MeshInfo", args:["hud_droideka_shield"], children:[
                    (name:"Position", args:[13,14,15]),
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

fn weapon(key: &str) -> HudWeapon {
    HudWeapon {
        key: key.into(),
        class: "dlch_dlc_weap_force_bubble".into(),
        bf2: true,
        icon: None,
        name: String::new(),
        total: None,
        clip_fraction: 1.0,
    }
}

/// Kit Fisto's ball shows its own sheet on the droideka icon's disc, in the
/// droideka icon's place in either row; the droideka's own key and mesh are
/// untouched, and without the installed mesh nothing changes.
#[test]
fn fisto_weapon_mesh_takes_the_droideka_shields_place() {
    let mut hud = rows();
    let mut art = HudArt::default();
    let ball = weapon(fisto::KEY);
    assert_eq!(fisto::mesh_for(&hud, &art, 2, &ball), None);
    let donor = HudMesh {
        parts: vec![ui::hud_draw::HudMeshPart {
            texture: fisto::DONOR.into(),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            ..Default::default()
        }],
    };
    fisto::install(&mut hud, &mut art, &donor);
    fisto::install(&mut hud, &mut art, &donor);
    for row in [1u8, 2] {
        assert_eq!(fisto::mesh_for(&hud, &art, row, &ball), Some(fisto::KEY));
    }
    assert_eq!(fisto::mesh_for(&hud, &art, 2, &weapon(fisto::DONOR)), None);
    assert_eq!(art.meshes[fisto::KEY].parts[0].texture, fisto::SHEET);
    for (i, at) in [
        (0, Vec3::new(1.0, 2.0, 3.0)),
        (1, Vec3::new(13.0, 14.0, 15.0)),
    ] {
        let infos = &hud.elems[i].mesh_info;
        assert_eq!(infos.len(), 2, "placed once");
        let donor = infos.iter().find(|p| p.mesh == fisto::DONOR).unwrap();
        let ball = infos.iter().find(|p| p.mesh == fisto::KEY).unwrap();
        assert_eq!(
            (ball.pos, ball.rot, ball.scale),
            (at, donor.rot, donor.scale)
        );
    }
}
