use super::*;

fn node(kind: &str, key: &str, children: Vec<HudNode>) -> HudNode {
    HudNode {
        name: kind.into(),
        args: vec![HudArg::Str(key.into())],
        children,
    }
}

fn positioned(kind: &str, key: &str, p: [f32; 3]) -> HudNode {
    let mut n = node(kind, key, vec![]);
    Geometry::Position(p, "Viewport").apply(&mut n);
    n
}

fn fixture(source: Source) -> HudLayout {
    let (root, map) = match source {
        Source::Pc => ([0.0, 1.0, 0.0], [0.917484, 0.110884, 0.004111]),
        Source::Xbox => ([0.082181, 0.427927, 0.0], [0.303257, 0.477156, 0.0]),
        Source::Ps2 => ([0.068639, 0.427927, 0.0], [0.856385, 0.186248, 0.004111]),
    };
    let mut weapons = positioned("Group", "player1weaponinformation", root);
    weapons
        .children
        .push(node("eventenable", "player1.weaponsEnable", vec![]));
    weapons
        .children
        .push(node("eventdisable", "player1.weaponsDisable", vec![]));
    weapons
        .children
        .push(node("fadeouttime", "sentinel", vec![]));
    let mut model = node(
        "model3d",
        "player1weapon1mesh",
        vec![node("mesh", "native_art", vec![])],
    );
    model
        .children
        .push(positioned("meshinfo", "hud_vehicle_missile", [9.0; 3]));
    let row1 = node("Group", "player1weapon1group", vec![model]);
    let row2 = node(
        "Group",
        "player1weapon2group",
        vec![node(
            "model3d",
            "player1weapon2mesh",
            vec![positioned("meshinfo", "hud_vehicle_missile", [9.0; 3])],
        )],
    );
    weapons.children.extend([
        row1,
        row2,
        positioned("text", "player1weapon1ammo", [8.0; 3]),
    ]);
    if source != Source::Pc {
        let children = weapons.children.split_off(4);
        let mut wrapper = positioned(
            "Group",
            "player1weaponinformation_lineupwithhealth",
            [-0.070914, 0.0, 0.0],
        );
        wrapper
            .children
            .push(node("eventenable", "initialize", vec![]));
        wrapper.children.extend(children);
        weapons.children.push(wrapper);
    }
    HudLayout {
        name: "1playerhud".into(),
        nodes: vec![
            weapons,
            positioned("Map", "player1map", map),
            positioned("Group", "player1stats", [7.0; 3]),
            positioned("Group", "mod_control", [6.0; 3]),
        ],
    }
}

fn find<'a>(nodes: &'a [HudNode], key: &str) -> &'a HudNode {
    for n in nodes {
        if name(n) == key {
            return n;
        }
        if let Some(n) = search(&n.children, key) {
            return n;
        }
    }
    panic!("missing {key}");
}
fn search<'a>(nodes: &'a [HudNode], key: &str) -> Option<&'a HudNode> {
    nodes.iter().find_map(|n| {
        if name(n) == key {
            Some(n)
        } else {
            search(&n.children, key)
        }
    })
}

#[test]
fn stock_sources_share_anchors_and_normalize_once() {
    for source in [Source::Pc, Source::Xbox] {
        let mut l = fixture(source);
        let root_controls = l.nodes[0]
            .children
            .iter()
            .filter(|p| p.name.starts_with("event") || p.name == "fadeouttime")
            .cloned()
            .collect::<Vec<_>>();
        let unrelated = find(&l.nodes, "mod_control").clone();
        normalize(&mut l, source, &[]);
        assert!(at(
            find(&l.nodes, "player1weaponinformation"),
            [0.068639, 0.427927, 0.0]
        ));
        assert!(at(
            find(&l.nodes, "player1map"),
            [0.856385, 0.186248, 0.004111]
        ));
        assert!(at(
            find(&l.nodes, "player1stats"),
            [0.098092, 1.155109, 0.0]
        ));
        assert_eq!(find(&l.nodes, "mod_control"), &unrelated);
        let root = find(&l.nodes, "player1weaponinformation");
        for control in root_controls {
            assert!(root.children.contains(&control));
        }
        let model = find(&l.nodes, "player1weapon1mesh");
        assert_eq!(
            name(model.children.iter().find(|n| n.name == "mesh").unwrap()),
            "native_art"
        );
        assert!(at(
            find(&model.children, "hud_vehicle_missile"),
            [-0.238161, 0.353195, 0.00628]
        ));
        let row2 = find(&l.nodes, "player1weapon2mesh");
        assert!(at(
            find(&row2.children, "hud_vehicle_missile"),
            [-0.224235, 0.25043, 0.00628]
        ));
        if source == Source::Pc {
            let wrapper = find(&l.nodes, "player1weaponinformation_lineupwithhealth");
            assert!(at(wrapper, [-0.070914, 0.0, 0.0]));
            assert!(wrapper
                .children
                .iter()
                .any(|n| name(n) == "player1weapon1ammo"));
            assert!(root
                .children
                .iter()
                .all(|n| name(n) != "player1weapon1ammo"));
        }
        let once = l.clone();
        normalize(&mut l, source, &[]);
        assert_eq!(l, once);
    }
}

#[test]
fn ps2_and_nonstock_layouts_are_byte_preserved() {
    for source in [Source::Ps2, Source::Pc, Source::Xbox] {
        let mut control = fixture(source);
        if source != Source::Ps2 {
            Geometry::Position([0.2, 0.3, 0.0], "Viewport").apply(&mut control.nodes[0]);
        }
        let bytes = ron::to_string(&control).unwrap();
        normalize(&mut control, source, &[]);
        assert_eq!(ron::to_string(&control).unwrap(), bytes);
        for layout in ["2playerhud", "3playerhud", "4playerhud", "hudtransforms"] {
            let mut l = fixture(source);
            l.name = layout.into();
            let old = l.clone();
            normalize(&mut l, source, &[]);
            assert_eq!(l, old);
        }
    }
    let mut l = fixture(Source::Pc);
    let before = l.clone();
    normalize(&mut l, Source::Xbox, &[]);
    assert_eq!(l, before);
    l.nodes[1].children.clear();
    let before = l.clone();
    normalize(&mut l, Source::Pc, &[]);
    assert_eq!(l, before);
}

#[test]
fn only_the_first_duplicate_mesh_binding_is_consumed() {
    let mut layout = fixture(Source::Pc);
    let model = layout.nodes[0]
        .children
        .iter_mut()
        .find(|n| name(n) == "player1weapon1group")
        .unwrap()
        .children
        .first_mut()
        .unwrap();
    let duplicate = positioned("meshinfo", "hud_vehicle_missile", [77.0, 88.0, 99.0]);
    model.children.push(duplicate.clone());
    normalize(&mut layout, Source::Pc, &[]);
    let model = find(&layout.nodes, "player1weapon1mesh");
    let bindings = model
        .children
        .iter()
        .filter(|n| n.name == "meshinfo")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    assert!(at(bindings[0], [-0.238161, 0.353195, 0.00628]));
    assert_eq!(bindings[1], &duplicate);
    let ammo = find(&layout.nodes, "player1weapon1ammo");
    assert_eq!(
        name(ammo.children.iter().find(|p| p.name == "textfont").unwrap()),
        "gamefont_small"
    );
}

#[test]
fn pc_cap_origin_uses_only_the_selected_font_metrics() {
    for bearing in [5, 9] {
        let fonts = vec![manifest::hud::HudFont {
            name: "gamefont_small".into(),
            line_height: 17,
            pages: vec![],
            glyphs: vec![manifest::hud::HudGlyph {
                code: 48,
                page: 0,
                advance: 8,
                x0: 0,
                x1: 7,
                y0: bearing,
                y1: bearing + 10,
                uv: [0.0; 4],
            }],
        }];
        let before = fonts.clone();
        let mut l = fixture(Source::Pc);
        let untouched = positioned("text", "custom_mod_label", [0.2, 0.3, 0.0]);
        l.nodes.push(untouched.clone());
        normalize(&mut l, Source::Pc, &fonts);
        let text = find(&l.nodes, "player1weapon1ammo");
        let Some(HudArg::Num(y)) = position(text).and_then(|p| p.get(1)) else {
            panic!("text origin")
        };
        // Both independently varied PC bearings land at the PS2 cap's three
        // frame pixels below its uncorrected authored origin.
        let cap = *y * 448.0 + f32::from(bearing) * 1.2;
        assert!((cap - (0.340939 * 448.0 + 3.0)).abs() < 0.0001);
        assert_eq!(find(&l.nodes, "custom_mod_label"), &untouched);
        assert_eq!(fonts, before);
    }
}
