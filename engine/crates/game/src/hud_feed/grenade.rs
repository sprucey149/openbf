//! BF1 grenade variants lightly tint BF2's round grenade HUD artwork.

use super::{Hud, HudArt, HudMesh, HudWeapon};

const VARIANTS: [(&str, &str, [f32; 3]); 3] = [
    ("thermaldetonator", "hud_bf1_thermal", [0.82, 0.93, 1.0]),
    ("crackgrenade", "hud_bf1_concussion", [1.0, 0.93, 0.79]),
    ("haywiredetonator", "hud_bf1_haywire", [0.95, 0.84, 1.0]),
];

pub(super) fn install(hud: &mut Hud, art: &mut HudArt, thermal: &HudMesh) {
    for (_, name, tint) in VARIANTS {
        let mut mesh = thermal.clone();
        for part in &mut mesh.parts {
            for color in &mut part.colors {
                for (channel, factor) in color.iter_mut().zip(tint) {
                    *channel *= factor;
                }
            }
        }
        art.meshes.insert(name.to_owned(), mesh);
        for row in ["player1weapon1mesh", "player1weapon2mesh"] {
            let Some(i) = hud.find(row) else { continue };
            let infos = &mut hud.elems[i].mesh_info;
            if let Some(mut pose) = infos
                .iter()
                .find(|p| p.mesh == "hud_thermaldetonator")
                .cloned()
            {
                pose.mesh = name.to_owned();
                infos.push(pose);
            }
        }
    }
}

pub(super) fn mesh_for(hud: &Hud, art: &HudArt, row: u8, w: &HudWeapon) -> Option<&'static str> {
    if w.bf2 || w.key != crate::roster::BF2_GRENADE_HUD_KEY {
        return None;
    }
    let suffix = w.class.rsplit('_').next()?;
    let (_, name, ..) = VARIANTS.iter().find(|v| v.0 == suffix)?;
    let i = hud.find(match row {
        1 => "player1weapon1mesh",
        2 => "player1weapon2mesh",
        _ => return None,
    })?;
    (art.has_mesh(name) && hud.elems[i].mesh_info.iter().any(|p| p.mesh == *name)).then_some(*name)
}
