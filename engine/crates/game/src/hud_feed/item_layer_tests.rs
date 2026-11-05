use super::*;
use manifest::hud::{HudArg, HudIndex, HudLayout, HudNode};

fn node(kind: &str, name: &str, children: Vec<HudNode>) -> HudNode {
    HudNode {
        name: kind.into(),
        args: vec![HudArg::Str(name.into())],
        children,
    }
}

fn number(name: &str, value: f32) -> HudNode {
    HudNode {
        name: name.into(),
        args: vec![HudArg::Num(value)],
        children: vec![],
    }
}

fn fixture(flat: bool, bitmap: bool) -> (HudView, Hud) {
    let mut children = Vec::new();
    for row in 1..=2 {
        children.push(node(
            "Model3D",
            &format!("player1weapon{row}backdrop"),
            vec![
                node("Mesh", &format!("pill{row}"), vec![]),
                number("ZOrder", 8.0),
                number("Alpha", 0.6),
            ],
        ));
        children.push(node(
            "Group",
            &format!("player1weapon{row}group"),
            vec![node(
                "Model3D",
                &format!("player1weapon{row}mesh"),
                vec![node(
                    "Mesh",
                    &format!("{}item{row}", if bitmap { "bitmap:" } else { "" }),
                    vec![],
                )],
            )],
        ));
    }
    children.push(node("Group", "unrelated", vec![number("ZOrder", 7.0)]));
    let mut layout = HudLayout {
        name: "1playerhud".into(),
        nodes: if flat {
            children
        } else {
            vec![node("Group", "lineup", children)]
        },
    };
    let transforms = HudLayout {
        name: "hudtransforms".into(),
        nodes: vec![],
    };
    let dir = std::env::temp_dir().join(format!(
        "openbf-item-layer-{}-{flat}-{bitmap}",
        std::process::id()
    ));
    let hud_dir = dir.join("common_bf2/hud");
    std::fs::create_dir_all(&hud_dir).unwrap();
    for (name, value) in [("1playerhud", &layout), ("hudtransforms", &transforms)] {
        std::fs::write(
            hud_dir.join(format!("{name}.ron")),
            ron::to_string(value).unwrap(),
        )
        .unwrap();
    }
    let index = HudIndex {
        layouts: vec![],
        fonts: vec![],
        textures: vec![],
    };
    std::fs::write(hud_dir.join("index.ron"), ron::to_string(&index).unwrap()).unwrap();
    let mut view = HudView::load(&dir, render::HEADLESS_FORMAT, None).unwrap();
    assert_eq!(
        read_ron::<HudLayout>(&hud_dir.join("1playerhud.ron")).unwrap(),
        layout
    );
    std::fs::remove_dir_all(dir).unwrap();
    ui::hud::add_leaving_line(&mut layout);
    let mut original = Hud::new(&layout, &transforms);
    for hud in [&mut view.hud, &mut original] {
        for elem in &mut hud.elems {
            elem.fader.show();
        }
    }
    for row in 1..=2 {
        for mesh in [format!("pill{row}"), format!("item{row}")] {
            view.art.meshes.insert(
                mesh.clone(),
                HudMesh {
                    parts: [0.0, 0.1]
                        .into_iter()
                        .map(|z| HudMeshPart {
                            texture: format!("{mesh}/{z}"),
                            positions: vec![[0.0, 0.0, z], [1.0, 0.0, z], [0.0, 1.0, z]],
                            uvs: vec![[0.0, 0.0]; 3],
                            colors: vec![[0.8, 0.7, 0.6, 0.5]; 3],
                            indices: vec![0, 1, 2],
                            ..Default::default()
                        })
                        .collect(),
                },
            );
        }
    }
    (view, original)
}

#[test]
fn weapon_pills_precede_both_rows_and_bitmap_fallback_in_nested_and_flat_layouts() {
    for flat in [false, true] {
        for bitmap in [false, true] {
            let (view, original) = fixture(flat, bitmap);
            let frame = ui::hud_draw::draw(&view.hud, &view.art, [1280.0, 720.0]);
            let old = ui::hud_draw::draw(&original, &view.art, [1280.0, 720.0]);
            for row in 1..=2 {
                let at = |frame: &ui::hud_draw::HudFrame, name: &str| {
                    frame
                        .batches
                        .iter()
                        .position(|b| b.texture.starts_with(name))
                        .unwrap()
                };
                assert!(at(&old, &format!("pill{row}/")) > at(&old, &format!("item{row}")));
                let pill = at(&frame, &format!("pill{row}/"));
                let item = at(&frame, &format!("item{row}"));
                assert!(pill < item, "flat={flat} bitmap={bitmap} row={row}");
                assert_eq!(frame.batches[item].depth, !bitmap);
                if !bitmap {
                    let depth = |batch: usize| {
                        let i = frame.indices[frame.batches[batch].first as usize];
                        frame.verts[i as usize].pos[2]
                    };
                    assert!(depth(pill) > depth(item));
                    assert!(depth(item) > depth(item + 1), "retain internal part depth");
                }
            }
            // Retain hierarchy, transforms, alpha and all other element state.
            let mut restored = view.hud.elems.clone();
            for row in 1..=2 {
                let i = view
                    .hud
                    .find(&format!("player1weapon{row}backdrop"))
                    .unwrap();
                restored[i].z_order = 8;
            }
            assert_eq!(format!("{restored:?}"), format!("{:?}", original.elems));
            assert_eq!(view.hud.roots, original.roots);
        }
    }
}

fn render_row(view: &mut HudView, r: &mut render::Renderer, size: [u32; 2]) -> Vec<u8> {
    let frame = ui::hud_draw::draw(&view.hud, &view.art, size.map(|v| v as f32));
    view.install_frame(frame);
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.12, 0.12, 0.12, 1.0],
        Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
    )
    .unwrap();
    assert!(view.unresolved_sheets().next().is_none());
    r.capture_rgba().unwrap().0
}

/// Real source geometry and textures, deliberately centred on the faded pill.
/// The item-only arm identifies opaque item pixels; the pill must not change
/// those pixels in the candidate, while the original order must shade them.
#[test]
#[ignore = "requires GPU, HUD_ITEM_ASSETS read-only PS2 bake and HUD_ITEM_OUT"]
fn weapon_pills_real_content_overlap() {
    let assets = PathBuf::from(std::env::var_os("HUD_ITEM_ASSETS").expect("HUD_ITEM_ASSETS"));
    let out = PathBuf::from(std::env::var_os("HUD_ITEM_OUT").expect("HUD_ITEM_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    for size in [[512, 448], [1920, 1080]] {
        let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        for bitmap in [false, true] {
            let weapon = |key: &str| HudWeapon {
                key: if bitmap {
                    "missing_weapon_for_bitmap_test"
                } else {
                    key
                }
                .into(),
                class: String::new(),
                bf2: !bitmap,
                icon: bitmap.then(|| "grenade_icon".into()),
                name: String::new(),
                total: Some(4),
                clip_fraction: 0.7,
            };
            let model = HudModel {
                weapon: Some(weapon("rep_weap_inf_rifle")),
                weapon2: Some(weapon("rep_weap_inf_thermaldetonator")),
                ..Default::default()
            };
            for row in 1..=2 {
                let mut view = HudView::for_renderer(&assets, &r).unwrap();
                view.update(&model, 0.0, size);
                let pill_name = format!("player1weapon{row}backdrop");
                let item_name = format!("player1weapon{row}mesh");
                let pill = view.hud.find(&pill_name).unwrap();
                let item = view.hud.find(&item_name).unwrap();
                let candidate_order = view.hud.elems[pill].z_order;
                assert_eq!(
                    view.hud.elems[item]
                        .mesh
                        .as_ref()
                        .unwrap()
                        .starts_with("bitmap:"),
                    bitmap
                );
                let pb = view.bounds(&pill_name).expect("pill bounds");
                let ib = view.bounds(&item_name).expect("item bounds");
                let scale = ui::hud_draw::frame_scale(size.map(|v| v as f32));
                let parent = view.hud.elems[item].parent.unwrap();
                for axis in 0..2 {
                    let delta = (pb[axis] + pb[axis + 2] - ib[axis] - ib[axis + 2]) * 0.5;
                    view.hud.elems[parent].pos[axis] += delta / scale[axis];
                }
                let mut keep = HashSet::new();
                for start in [pill, item] {
                    let mut current = Some(start);
                    while let Some(i) = current {
                        keep.insert(i);
                        current = view.hud.elems[i].parent;
                    }
                }
                for (i, elem) in view.hud.elems.iter_mut().enumerate() {
                    elem.fader.state = if keep.contains(&i) { 2 } else { 0 };
                }
                let stem = format!(
                    "row{row}_{}_{}x{}",
                    if bitmap { "bitmap" } else { "mesh" },
                    size[0],
                    size[1]
                );
                println!("{stem}: source pill={pb:?}, source item={ib:?}, candidate order={candidate_order}");
                let mut frames = Vec::new();
                for (arm, show_pill, show_item, order) in [
                    ("item", false, true, 255),
                    ("pill", true, false, 255),
                    ("before", true, true, 8),
                    ("after", true, true, candidate_order),
                ] {
                    view.hud.elems[pill].fader.state = if show_pill { 2 } else { 0 };
                    view.hud.elems[item].fader.state = if show_item { 2 } else { 0 };
                    view.hud.elems[pill].z_order = order;
                    let rgba = render_row(&mut view, &mut r, size);
                    std::fs::write(
                        out.join(format!("{stem}_{arm}.png")),
                        render::encode_png(&rgba, size[0], size[1]).unwrap(),
                    )
                    .unwrap();
                    frames.push(rgba);
                }
                let mut changed = 0;
                let mut restored = 0;
                for (((item, pill), before), after) in frames[0]
                    .chunks_exact(4)
                    .zip(frames[1].chunks_exact(4))
                    .zip(frames[2].chunks_exact(4))
                    .zip(frames[3].chunks_exact(4))
                {
                    if item != pill && before != after {
                        changed += 1;
                        if item == after {
                            restored += 1;
                        }
                    }
                }
                println!("{stem}: changed={changed}, item pixels restored={restored}");
                assert!(restored > 10, "{stem}: no demonstrated item/pill overlap");
            }
        }
    }
}
