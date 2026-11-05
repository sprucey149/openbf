use super::Hud;
use manifest::hud::{HudArg, HudLayout, HudNode};

fn node(name: &str, args: &[HudArg], children: Vec<HudNode>) -> HudNode {
    HudNode {
        name: name.into(),
        args: args.to_vec(),
        children,
    }
}

fn p(name: &str, args: &[HudArg]) -> HudNode {
    node(name, args, vec![])
}

fn s(v: &str) -> HudArg {
    HudArg::Str(v.into())
}

fn n(v: f32) -> HudArg {
    HudArg::Num(v)
}

pub(crate) fn target_hud() -> Hud {
    let vp = || s("Viewport");
    let ev = |k: &str| s(&format!("player1.weapon1.target.{k}"));
    let bar = |name: &str, own: Vec<HudNode>| {
        let mut c = vec![
            p("flashyscale", &[n(1.0)]),
            p("Bitmap", &[s("hud_jetpack_and_energybar_fill")]),
            p("texcoords", &[n(0.0), n(0.5), n(1.0), n(1.0)]),
            p("Viewport", &[n(1.0)]),
            p("colorchange", &[n(0.0), n(0.0), n(0.0)]),
            p("fadeouttime", &[n(1.0)]),
            p("eventenable", &[ev("healthFraction")]),
            p("eventdisable", &[ev("disable")]),
        ];
        c.extend(own);
        node("barbitmap", &[s(name)], c)
    };
    let layout = HudLayout {
        name: "1playerhud".into(),
        nodes: vec![
            node(
                "Group",
                &[s("player1targethealthbar")],
                vec![
                    p("propagatealpha", &[n(1.0)]),
                    p("Viewport", &[n(1.0)]),
                    p("Position", &[n(0.5), n(0.059272), n(0.0), vp()]),
                    p("Scale", &[n(0.263237), n(0.23), n(0.23)]),
                    p("alpha", &[n(0.8)]),
                    p("color", &[n(192.0), n(224.0), n(224.0)]),
                    p("eventenable", &[s("initialize")]),
                    bar(
                        "player1targethealth_blankfill",
                        vec![
                            p("value", &[n(1.0)]),
                            p(
                                "bitmaprect",
                                &[n(1.18923), n(0.181946), s("Center"), s("Center"), vp()],
                            ),
                            p("Position", &[n(0.0), n(0.253896), n(0.0), vp()]),
                            p("Scale", &[n(0.924622), n(0.498552), n(0.540316)]),
                            p("zorder", &[n(103.0)]),
                            p("color", &[n(39.0), n(40.0), n(46.0)]),
                            p("colorchangerate", &[n(0.0)]),
                            p("fadeintime", &[n(0.1)]),
                        ],
                    ),
                    bar(
                        "player1targethealth_fill",
                        vec![
                            p("#2b76908f", &[n(1.0)]),
                            p("#2deba703", &[n(1.0)]),
                            p("eventvalue", &[ev("healthFraction")]),
                            p(
                                "bitmaprect",
                                &[n(1.18923), n(0.181946), s("Center"), s("Top"), vp()],
                            ),
                            p("Position", &[n(0.0), n(0.227078), n(0.0), vp()]),
                            p("Scale", &[n(0.890714), n(0.222995), n(0.9)]),
                            p("zorder", &[n(1.0)]),
                            p("colorchangerate", &[n(0.01)]),
                            p("eventcolor", &[ev("teamColor")]),
                        ],
                    ),
                ],
            ),
            node(
                "Group",
                &[s("player1targetnamegroup")],
                vec![
                    p("Viewport", &[n(1.0)]),
                    p("Position", &[n(0.5), n(0.012799), n(0.0), vp()]),
                    p("eventenable", &[s("initialize")]),
                    node(
                        "Text",
                        &[s("player1targetinfo_classNameNEW")],
                        vec![
                            p("textalignment", &[s("Center"), s("Top")]),
                            p("textfont", &[s("gamefont_tiny")]),
                            p("eventtext", &[ev("className")]),
                            p("Position", &[n(0.0), n(0.065248), n(0.0), vp()]),
                            p("alpha", &[n(0.6)]),
                            p("fadeouttime", &[n(1.0)]),
                            p("eventenable", &[ev("className")]),
                            p("eventdisable", &[ev("disable")]),
                        ],
                    ),
                    node(
                        "Text",
                        &[s("player1targetinfo_shieldName")],
                        vec![
                            p("eventtext", &[ev("shieldName")]),
                            p("color", &[n(40.0), n(255.0), n(255.0)]),
                            p("fadeouttime", &[n(1.0)]),
                            p("eventenable", &[ev("shieldName")]),
                            p("eventdisable", &[ev("disableShieldName")]),
                        ],
                    ),
                ],
            ),
        ],
    };
    let none = HudLayout {
        name: "none".into(),
        nodes: vec![],
    };
    let mut hud = Hud::new(&layout, &none);
    hud.send("initialize", super::EventValue::None, &|_| false);
    hud.advance(0.0);
    hud
}

/// The 1-player reticle group's hit and critical-hit flashes as the
/// disc has them (`common_bf2/hud/1playerhud.ron`, `player1reticule`;
/// `re-target-hud.md` §12).
pub(crate) fn reticle_hud() -> Hud {
    let ev = |k: &str| s(&format!("player1.weapon1.target.{k}"));
    let flash = |name: &str, mesh: &str, own: Vec<HudNode>| {
        let mut c = vec![
            p("mesh", &[s(mesh)]),
            p("Scale", &[n(0.26), n(0.26), n(0.26)]),
            p("color", &[n(1.0), n(86.0), n(213.0)]),
            p("colorchangerate", &[n(0.01)]),
            p("eventcolor", &[ev("hitColor")]),
        ];
        c.extend(own);
        node("model3d", &[s(name)], c)
    };
    let layout = HudLayout {
        name: "1playerhud".into(),
        nodes: vec![node(
            "Group",
            &[s("player1reticule")],
            vec![
                p("eventposition", &[s("player1.weapon1.reticule.position")]),
                p("Position", &[n(0.5), n(0.410624), n(0.0), s("Viewport")]),
                p("alpha", &[n(0.65)]),
                p("eventenable", &[s("player1.weapon1.reticule.position")]),
                p("eventdisable", &[s("player1.weapon1.reticule.disable")]),
                flash(
                    "player1reticule_hit",
                    "hud_main_reticule_hit",
                    vec![
                        p("alpha", &[n(0.5)]),
                        p("fadeholdtime", &[n(0.33)]),
                        p("eventenable", &[ev("hit")]),
                    ],
                ),
                flash(
                    "player1reticule_criticalhit",
                    "hud_main_reticule_criticalhit",
                    vec![
                        p("alpha", &[n(0.6)]),
                        p("fadeholdtime", &[n(0.6)]),
                        p("fadesustaintime", &[n(0.5)]),
                        p("fadeouttime", &[n(0.01)]),
                        p("eventenable", &[ev("hitCritical")]),
                    ],
                ),
            ],
        )],
    };
    let none = HudLayout {
        name: "none".into(),
        nodes: vec![],
    };
    let mut hud = Hud::new(&layout, &none);
    hud.send("initialize", super::EventValue::None, &|_| false);
    hud.advance(0.0);
    hud
}
