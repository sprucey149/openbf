//! The DLC Force ball's own weapon icon. The add-on tags Kit Fisto's ball
//! with the droideka's `hud_droideka_shield`, a droideka figure with only a
//! faint arc of shield behind it; the DLC bake retags it `hud_fisto_bubble`
//! (`bake::dlc`) and draws a ball-only sheet for it in the same ink
//! (`dlch_hud_fisto_bubble`). `[ours]`: the ball shows on the droideka's
//! own HUD disc, in the droideka icon's place in each weapon row, with that
//! sheet. Droidekas keep theirs.

use super::{Hud, HudArt, HudMesh, HudWeapon};

/// The ball's HUD key (the rewritten `HUDTag`).
pub(crate) const KEY: &str = "hud_fisto_bubble";
/// The model and placement it borrows.
pub(crate) const DONOR: &str = "hud_droideka_shield";
/// The DLC bake's sheet for it (`dlch_` like every DLC texture).
pub(crate) const SHEET: &str = "dlch_hud_fisto_bubble";

const ROWS: [&str; 2] = ["player1weapon1mesh", "player1weapon2mesh"];

/// Add the ball's mesh (the donor disc drawing [`SHEET`]) and give it the
/// donor's place in every weapon row that places the donor.
pub(super) fn install(hud: &mut Hud, art: &mut HudArt, donor: &HudMesh) {
    let mut mesh = donor.clone();
    for part in &mut mesh.parts {
        part.texture = SHEET.to_owned();
    }
    art.meshes.insert(KEY.to_owned(), mesh);
    for row in ROWS {
        let Some(i) = hud.find(row) else { continue };
        let infos = &mut hud.elems[i].mesh_info;
        if infos.iter().any(|p| p.mesh == KEY) {
            continue;
        }
        if let Some(mut pose) = infos.iter().find(|p| p.mesh == DONOR).cloned() {
            pose.mesh = KEY.to_owned();
            infos.push(pose);
        }
    }
}

/// The mesh weapon row `row` shows for `w` when it is the ball and the
/// ball's mesh is installed and placed there.
pub(super) fn mesh_for(hud: &Hud, art: &HudArt, row: u8, w: &HudWeapon) -> Option<&'static str> {
    if !w.key.eq_ignore_ascii_case(KEY) {
        return None;
    }
    let i = hud.find(ROWS.get(usize::from(row).checked_sub(1)?)?)?;
    (art.has_mesh(KEY) && hud.elems[i].mesh_info.iter().any(|p| p.mesh == KEY)).then_some(KEY)
}
