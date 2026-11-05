use super::*;

#[test]
fn hud_files_round_trip() {
    use crate::hud::*;
    let layout = HudLayout {
        name: "1playerhud".into(),
        nodes: vec![
            HudNode {
                name: "fileinfo".into(),
                args: vec![HudArg::Str("1playerhud".into())],
                children: vec![],
            },
            HudNode {
                name: "Group".into(),
                args: vec![HudArg::Str("player1health".into())],
                children: vec![
                    HudNode {
                        name: "Position".into(),
                        args: vec![HudArg::Num(0.06), HudArg::Num(0.84), HudArg::Num(0.0)],
                        children: vec![],
                    },
                    HudNode {
                        name: "#2b76908f".into(),
                        args: vec![HudArg::Num(1.0), HudArg::Str("1".into())],
                        children: vec![],
                    },
                ],
            },
        ],
    };
    let text = to_ron(&layout);
    let back: HudLayout = ron::from_str(&text).expect("read back layout");
    assert_eq!(back, layout);
    assert_eq!(text, to_ron(&back));

    let font = HudFont {
        name: "gamefont_medium".into(),
        line_height: 18,
        pages: vec!["gamefont_medium_tex0".into(), "gamefont_medium_tex1".into()],
        glyphs: vec![HudGlyph {
            code: 33,
            page: 0,
            advance: 5,
            x0: 0,
            x1: 4,
            y0: 0,
            y1: 16,
            uv: [0.1328125, 0.1640625, 0.0078125, 0.1328125],
        }],
    };
    let back: HudFont = ron::from_str(&to_ron(&font)).expect("read back font");
    assert_eq!(back, font);

    let index = HudIndex {
        layouts: vec!["hudtransforms".into(), "1playerhud".into()],
        fonts: vec!["gamefont_medium".into()],
        textures: vec![("white".into(), "CORE.LVL".into())],
    };
    let back: HudIndex = ron::from_str(&to_ron(&index)).expect("read back index");
    assert_eq!(back, index);
}
