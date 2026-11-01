use manifest::hud::{HudArg, HudLayout, HudNode};

fn property(node: &mut HudNode, name: &str, args: Vec<HudArg>) {
    if let Some(p) = node
        .children
        .iter_mut()
        .find(|p| p.name.eq_ignore_ascii_case(name))
    {
        p.args = args;
    } else {
        node.children.push(HudNode {
            name: name.into(),
            args,
            children: Vec::new(),
        });
    }
}

fn numbers(node: &mut HudNode, name: &str, values: &[f32]) {
    property(
        node,
        name,
        values.iter().copied().map(HudArg::Num).collect(),
    );
}

fn position(node: &mut HudNode, x: f32, y: f32, z: f32) {
    property(
        node,
        "Position",
        vec![
            HudArg::Num(x),
            HudArg::Num(y),
            HudArg::Num(z),
            HudArg::Str("Viewport".into()),
        ],
    );
}

fn name(node: &HudNode) -> &str {
    match node.args.first() {
        Some(HudArg::Str(s)) => s,
        _ => "",
    }
}

fn font(node: &mut HudNode, font: &str, scale: [f32; 2]) {
    property(node, "textfont", vec![HudArg::Str(font.into())]);
    numbers(node, "textscale", &scale);
}

fn walk(nodes: &mut [HudNode]) {
    for node in nodes.iter_mut().filter(|n| !n.children.is_empty()) {
        let pos = match name(node) {
            "player1weaponinformation" => Some((-0.0023, 0.4279, 0.0)),
            "player1weapon1ammo" => Some((0.0753, 0.3409, 0.0)),
            "player1weapon2ammo" => Some((0.0765, 0.2866, 0.0)),
            "player1weapon1ammoInfinite" => Some((0.0849, 0.3605, 0.0)),
            "player1weapon2ammoInfinite" => Some((0.0851, 0.3064, 0.0354)),
            "player1weapon1name" => Some((0.0769, 0.2270, 0.0)),
            "player1weapon2name" => Some((0.0751, 0.1870, 0.0)),
            "player1weapon1backdrop" => Some((0.2078, 0.3562, -0.0028)),
            "player1weapon2backdrop" => Some((0.2076, 0.3698, -0.0251)),
            "player1weapon1group" => Some((0.4037, 0.0052, 0.4844)),
            "player1weapon2group" => Some((0.4037, 0.0474, -0.0004)),
            "rep_weap_inf_rifle" if node.name.eq_ignore_ascii_case("meshinfo") => {
                Some((-0.1900, 0.3598, 0.0033))
            }
            "cis_weap_inf_rifle" if node.name.eq_ignore_ascii_case("meshinfo") => {
                Some((-0.2174, 0.3599, 0.0019))
            }
            "imp_weap_inf_rifle" if node.name.eq_ignore_ascii_case("meshinfo") => {
                Some((-0.2170, 0.3605, 0.0034))
            }
            "all_weap_inf_rifle" if node.name.eq_ignore_ascii_case("meshinfo") => {
                Some((-0.1904, 0.3613, 0.0146))
            }
            "player1healthbar" => Some((0.0649, 0.8275, 0.0)),
            "player1energybar" => Some((0.0645, 0.8544, 0.0)),
            "player1jetpackbar" => Some((0.0632, 0.4755, 0.0)),
            "player1herobar" => Some((0.0630, 0.8264, 0.0)),
            "player1vehiclehealthbar" => Some((0.0647, 0.8275, 0.0)),
            "player1targetnamegroup" => Some((0.5, 0.0128, 0.0)),
            "player1targethealthbar" => Some((0.5, 0.0593, 0.0)),
            "player1reticule" => Some((0.5, 0.4106, 0.0)),
            "player1map" => Some((0.8564, 0.1862, 0.0041)),
            "player1team1_group" => Some((-0.1749, 0.0275, 0.0)),
            "player1team2_group" => Some((0.1731, 0.0305, 0.0)),
            "player1teams_team1icongroup" | "player1teams_team2icongroup" => Some((0.0, 0.0, 0.0)),
            "player1teams_team1reinforcementsgroup" => Some((-0.0519, -0.0199, 0.0)),
            "player1teams_team2reinforcementsgroup" => Some((0.0524, -0.0205, 0.0)),
            "player1teams_team1pointsgroup" => Some((0.0049, 0.0305, 0.0)),
            "player1teams_team2pointsgroup" => Some((0.0040, 0.0309, 0.0)),
            _ => None,
        };
        if let Some((x, y, z)) = pos {
            position(node, x, y, z);
        }
        match name(node) {
            "player1reticule" => numbers(node, "Scale", &[1.0; 3]),
            "player1map" => numbers(node, "Scale", &[0.9; 3]),
            "player1teams_team1icon" | "player1teams_team2icon" => {
                numbers(node, "Scale", &[0.75; 3])
            }
            "player1health_fill" => position(node, 0.0, 0.0, 0.0),
            "player1weapon1ammo" | "player1weapon2ammo" => {
                font(node, "gamefont_small", [1.43, 1.2]);
            }
            "player1weapon1name" | "player1weapon2name" => {
                font(node, "gamefont_tiny", [1.5, 1.14]);
            }
            "player1teams_team1reinforcements"
            | "player1teams_team2reinforcements"
            | "player1teams_team1points"
            | "player1teams_team2points" => {
                font(node, "gamefont_medium", [1.25, 1.09]);
            }
            _ => {}
        }
        walk(&mut node.children);
    }
}

pub(crate) fn normalize(layout: &mut HudLayout, pc: bool) {
    if !pc || layout.name != "1playerhud" {
        return;
    }
    let stock_layout = layout
        .nodes
        .iter()
        .find(|n| name(n) == "player1weaponinformation")
        .and_then(|n| {
            n.children
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case("Position"))
        })
        .is_some_and(|p| p.args.get(1) == Some(&HudArg::Num(1.0)));
    if stock_layout {
        walk(&mut layout.nodes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(y: f32) -> HudLayout {
        let mut n = HudNode {
            name: "Group".into(),
            args: vec![HudArg::Str("player1weaponinformation".into())],
            children: Vec::new(),
        };
        position(&mut n, 0.0, y, 0.0);
        HudLayout {
            name: "1playerhud".into(),
            nodes: vec![n],
        }
    }

    #[test]
    fn ps2_layout_is_unchanged() {
        for y in [0.427927, 1.0] {
            let mut l = layout(y);
            let before = l.clone();
            normalize(&mut l, false);
            assert_eq!(l, before);
        }
    }

    #[test]
    fn pc_layout_is_normalized_once() {
        let mut l = layout(1.0);
        let before = l.clone();
        normalize(&mut l, true);
        assert_ne!(l, before);
        let after = l.clone();
        normalize(&mut l, true);
        assert_eq!(l, after);
    }

    #[test]
    fn pc_multiplayer_layout_is_unchanged() {
        for name in ["2playerhud", "3playerhud", "4playerhud", "hudtransforms"] {
            let mut l = layout(1.0);
            l.name = name.into();
            let before = l.clone();
            normalize(&mut l, true);
            assert_eq!(l, before);
        }
    }
}
