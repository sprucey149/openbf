//! PC font-role sizes and cap bearings in the selected native atlas. PS2
//! reference metrics are numeric measurements, not a required console donor.
use super::{name, position, property, HudArg, HudNode};
use manifest::hud::HudFont;

fn numbers(node: &HudNode, key: &str) -> Vec<f32> {
    node.children
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(key))
        .map(|p| {
            p.args
                .iter()
                .filter_map(|a| match a {
                    HudArg::Num(n) => Some(*n),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
fn string<'a>(node: &'a HudNode, key: &str, arg: usize) -> Option<&'a str> {
    node.children
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(key))
        .and_then(|p| p.args.get(arg))
        .and_then(|a| match a {
            HudArg::Str(s) => Some(s.as_str()),
            _ => None,
        })
}

pub(super) fn shift_y(node: &mut HudNode, pixels: f32) {
    if pixels == 0.0 {
        return;
    }
    let mut args = position(node).map(<[HudArg]>::to_vec).unwrap_or_else(|| {
        vec![
            HudArg::Num(0.0),
            HudArg::Num(0.0),
            HudArg::Num(0.0),
            HudArg::Str("Pixels".into()),
        ]
    });
    let viewport = match args.get(3) {
        Some(HudArg::Str(s)) => !s.eq_ignore_ascii_case("pixels"),
        Some(HudArg::Num(n)) => *n != 0.0,
        None => false,
    };
    if let Some(HudArg::Num(y)) = args.get_mut(1) {
        *y += if viewport { pixels / 448.0 } else { pixels };
    }
    property(node, "Position", args);
}

fn role(node: &HudNode, owner: &str) -> String {
    match name(node) {
        "player1weapon1ammo"
        | "player1weapon2ammo"
        | "player1statspoints"
        | "player1statspointsDelta"
        | "player1statskills"
        | "player1statskillsDelta"
        | "player1.heroSelectTimer" => "gamefont_small",
        "player1weapon1name"
        | "player1weapon2name"
        | "player1lockon.distance"
        | "player1missilelockdistance" => "gamefont_tiny",
        "player1teams_team1reinforcements"
        | "player1teams_team2reinforcements"
        | "player1teams_team1points"
        | "player1teams_team2points" => "gamefont_medium",
        "Format" if owner == "player1systemMessages" => "gamefont_tiny",
        _ => string(node, "textfont", 0).unwrap_or("gamefont_tiny"),
    }
    .to_owned()
}

/// Returns the local-pixel displacement needed for the cap top. For a multiline
/// format the caller moves its containing element, not the Format record.
fn format(
    node: &mut HudNode,
    owner: &str,
    fonts: &[HudFont],
    multiline_height: Option<f32>,
) -> f32 {
    let role = role(node, owner);
    // Original PC compatibility scales for tiny/small/medium. Large's stock
    // numeric advance 10→13 and cap height 12→15 supply its measured scale.
    let (factor, reference_line, reference_top) = match role.as_str() {
        "gamefont_tiny" => ([1.5, 1.14], 15.0, 4.0),
        "gamefont_small" => ([1.43, 1.2], 18.0, 3.0),
        "gamefont_medium" => ([1.25, 1.09], 18.0, 3.0),
        "gamefont_large" => ([1.3, 1.25], 22.0, 3.0),
        _ => return 0.0,
    };
    let old = numbers(node, "textscale");
    let base = [
        old.first().copied().unwrap_or(1.0),
        old.get(1).copied().unwrap_or(1.0),
    ];
    let scale = [base[0] * factor[0], base[1] * factor[1]];
    let align = match string(node, "textalignment", 1)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("center") => 0.5,
        Some("bottom") => 1.0,
        _ => 0.0,
    };
    property(node, "textfont", vec![HudArg::Str(role.clone())]);
    property(
        node,
        "textscale",
        scale.into_iter().map(HudArg::Num).collect(),
    );
    let Some(font) = fonts.iter().find(|f| f.name == role) else {
        return 0.0;
    };
    let Some(cap) = font.glyphs.iter().find(|g| g.code == u16::from(b'0')) else {
        return 0.0;
    };
    let target_height = reference_line * base[1];
    let native_height = f32::from(font.line_height) * scale[1];
    let hang = match multiline_height {
        Some(h) if h > 0.0 => {
            let pitch = |line: f32| h / (h / line).floor().max(1.0);
            ((pitch(target_height) - target_height) - (pitch(native_height) - native_height))
                * align
        }
        _ => (native_height - target_height) * align,
    };
    (reference_top * base[1] - f32::from(cap.y0) * scale[1]) + hang
}

const STOCK_TEXT: &[&str] = &[
    "defeattext",
    "defeattimer",
    "objectivetimer",
    "player1.flag.enemy.carried.number",
    "player1.flag.enemy.dropped.number",
    "player1.flag.friend.carried.number",
    "player1.flag.friend.dropped.number",
    "player1.heroSelectMessage",
    "player1.heroSelectTimer",
    "player1.vehicle.hackedTime",
    "player1.vehicle.hackingTime",
    "player1ObjectivePopupText.myText",
    "player1ObjectivePopupText.popup_title",
    "player1ObjectivePopupText.press_to_continue",
    "player1ObjectivesUpdatedReminder",
    "player1PressSelectToReturn",
    "player1SelectionPopupText.configAccept",
    "player1SelectionPopupText.configDecline",
    "player1SelectionPopupText.popup_text",
    "player1ShowHints",
    "player1ShowObjectiveDetails",
    "player1lockon.className",
    "player1lockon.distance",
    "player1lockon.name",
    "player1mapkey_CPText",
    "player1missilelock",
    "player1missilelockdistance",
    "player1spawninfo",
    "player1spawnmessage",
    "player1spawnvehicle",
    "player1statskills",
    "player1statskillsDelta",
    "player1statspoints",
    "player1statspointsDelta",
    "player1targetinfo_classNameNEW",
    "player1targetinfo_shieldName",
    "player1teams_team1points",
    "player1teams_team1reinforcements",
    "player1teams_team2points",
    "player1teams_team2reinforcements",
    "player1weapon1ammo",
    "player1weapon1name",
    "player1weapon1overheat",
    "player1weapon2ammo",
    "player1weapon2name",
    "victorytext",
    "victorytimer",
];

pub(super) fn normalize(node: &mut HudNode, fonts: &[HudFont]) {
    if node.name.eq_ignore_ascii_case("text") && STOCK_TEXT.contains(&name(node)) {
        let dy = format(node, "", fonts, None);
        let scale = numbers(node, "Scale").get(1).copied().unwrap_or(1.0);
        shift_y(node, dy * scale);
    } else if node.name.eq_ignore_ascii_case("multilinetext")
        && name(node) == "player1systemMessages"
    {
        let owner = name(node).to_owned();
        let rect = node
            .children
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case("rect"));
        let h = rect
            .and_then(|r| r.args.get(1))
            .and_then(|a| match a {
                HudArg::Num(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0.0);
        let relative = rect.and_then(|r| r.args.get(4)).is_some_and(|a| match a {
            HudArg::Str(s) => !s.eq_ignore_ascii_case("pixels"),
            HudArg::Num(n) => *n != 0.0,
        });
        if let Some(f) = node
            .children
            .iter_mut()
            .find(|n| n.name.eq_ignore_ascii_case("format"))
        {
            let dy = format(f, &owner, fonts, Some(if relative { h * 448.0 } else { h }));
            shift_y(node, dy);
        }
    }
}
