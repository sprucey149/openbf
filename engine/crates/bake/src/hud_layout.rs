//! Shared stock single-player HUD geometry, measured from the BF2 PS2 layout.
//! Only consumed geometric properties change; edition artwork, font bindings,
//! events, fades and other layouts remain native. See `analysis/re-hud.md` §8.

use manifest::hud::{HudArg, HudLayout, HudNode};

mod geometry;
mod text;
use geometry::{Geometry, CORRECTIONS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Ps2,
    Pc,
    Xbox,
}

fn name(node: &HudNode) -> &str {
    match node.args.first() {
        Some(HudArg::Str(s)) => s,
        _ => "",
    }
}

fn property(node: &mut HudNode, key: &str, args: Vec<HudArg>) {
    if let Some(p) = node
        .children
        .iter_mut()
        .find(|p| p.name.eq_ignore_ascii_case(key))
    {
        p.args = args;
    } else {
        node.children.push(HudNode {
            name: key.into(),
            args,
            children: Vec::new(),
        });
    }
}

fn position(node: &HudNode) -> Option<&[HudArg]> {
    node.children
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("position"))
        .map(|p| p.args.as_slice())
}

fn at(node: &HudNode, xyz: [f32; 3]) -> bool {
    position(node).is_some_and(|p| {
        p == [
            HudArg::Num(xyz[0]),
            HudArg::Num(xyz[1]),
            HudArg::Num(xyz[2]),
            HudArg::Str("Viewport".into()),
        ]
    })
}

fn stock(layout: &HudLayout, source: Source) -> bool {
    if layout.name != "1playerhud" {
        return false;
    }
    let find = |key: &str| layout.nodes.iter().find(|n| name(n) == key);
    let Some(weapons) = find("player1weaponinformation") else {
        return false;
    };
    match source {
        Source::Ps2 => false,
        Source::Pc => {
            at(weapons, [0.0, 1.0, 0.0])
                && find("player1map").is_some_and(|n| at(n, [0.917484, 0.110884, 0.004111]))
        }
        Source::Xbox => {
            at(weapons, [0.082181, 0.427927, 0.0])
                && find("player1map").is_some_and(|n| at(n, [0.303257, 0.477156, 0.0]))
        }
    }
}

fn weapon_wrapper(node: &mut HudNode) {
    const WRAPPER: &str = "player1weaponinformation_lineupwithhealth";
    if node.children.iter().any(|c| name(c) == WRAPPER) {
        return;
    }
    // The PC puts these children directly under the root. The neutral group
    // keeps the PS2's relative offset and its left-edge Hor+ root selection.
    let mut wrapper = HudNode {
        name: "Group".into(),
        args: vec![HudArg::Str(WRAPPER.into())],
        children: Vec::new(),
    };
    Geometry::Position([-0.070914, 0.0, 0.0], "Viewport").apply(&mut wrapper);
    property(
        &mut wrapper,
        "eventenable",
        vec![HudArg::Str("initialize".into())],
    );
    let mut properties = Vec::new();
    for child in std::mem::take(&mut node.children) {
        if !child.children.is_empty() {
            wrapper.children.push(child);
        } else {
            properties.push(child);
        }
    }
    properties.push(wrapper);
    node.children = properties;
}

fn walk(nodes: &mut [HudNode], parent: &str, source: Source, fonts: &[manifest::hud::HudFont]) {
    let mut mesh_names = std::collections::HashSet::new();
    for node in nodes.iter_mut().filter(|n| !n.children.is_empty()) {
        // Model3D uses the first MeshInfo for a binding. Retain unused later
        // duplicates byte-for-byte instead of letting them supply geometry.
        if node.name.eq_ignore_ascii_case("meshinfo")
            && !mesh_names.insert(name(node).to_ascii_lowercase())
        {
            continue;
        }
        for &(owner, key, geometry) in CORRECTIONS {
            if name(node).eq_ignore_ascii_case(key)
                && (owner.is_empty() || parent.eq_ignore_ascii_case(owner))
            {
                geometry.apply(node);
            }
        }
        if source == Source::Pc {
            text::normalize(node, fonts);
            if name(node) == "player1weapon2backdrop" {
                // Native PC mesh pivot is 1.8049331 model units below PS2;
                // 0.200001 scale and 1/(0.01*1.24) give 29.11197 frame px.
                let stock_mesh = node.children.iter().any(|p| {
                    p.name.eq_ignore_ascii_case("mesh") && name(p) == "hud_weapon2_backdrop"
                });
                if stock_mesh {
                    text::shift_y(node, -29.11197);
                }
            }
        }
        // These records are not interpreted as child elements at runtime.
        if matches!(
            node.name.to_ascii_lowercase().as_str(),
            "map" | "target" | "objectivelist" | "borderedbox" | "#86e6ef58"
        ) {
            continue;
        }
        let parent = name(node).to_owned();
        walk(&mut node.children, &parent, source, fonts);
    }
}

pub(crate) fn normalize(layout: &mut HudLayout, source: Source, fonts: &[manifest::hud::HudFont]) {
    if !stock(layout, source) {
        return;
    }
    if source == Source::Pc {
        let root = layout
            .nodes
            .iter_mut()
            .find(|n| name(n) == "player1weaponinformation")
            .unwrap();
        weapon_wrapper(root);
    }
    walk(&mut layout.nodes, "", source, fonts);
}

#[cfg(test)]
mod tests;
