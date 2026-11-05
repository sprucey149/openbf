//! BF2's HUD as baked (card BHUD, `analysis/re-hud.md`): the five INGAME.LVL
//! `hud_` layouts as named element trees, CORE.LVL's four bitmap fonts, and an
//! index of what the bake wrote. Files, under the BF2 common bake:
//!
//! * `hud/index.ron` — [`HudIndex`];
//! * `hud/<layout>.ron` — [`HudLayout`] (`hudtransforms`, `1playerhud` …);
//! * `fonts/<font>.ron` — [`HudFont`], its pages as `fonts/<page>.ktx2`;
//! * the sheets the layouts name that INGAME.LVL lacks (BF2 COMMON.LVL's
//!   `hud_icon_flash`, `opaque_black`, …; CORE.LVL's `white`) in `textures/`
//!   beside INGAME's own, alpha kept.
//!
//! And, under BOTH games' common bakes (card EVMSG,
//! `analysis/re-event-messages.md`): `locl/english.ron` — [`HudLocl`], the
//! English text table of that game's CORE.LVL.

use serde::{Deserialize, Serialize};

/// Optional locally baked BF2 Light faces. Each pair is (menu, legacy role).
/// All sizes share the same distance pages; older bakes keep their own fonts.
pub const MENU_FONTS: [(&str, &str); 3] = [
    ("menu_gamefont_large", "gamefont_large"),
    ("menu_gamefont_small", "gamefont_small"),
    ("menu_gamefont_tiny", "gamefont_tiny"),
];

pub const MENU_ATLAS: &str = "menu_face";

/// `hud/index.ron`: everything the HUD bake wrote, by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudIndex {
    /// Layout names; each is `hud/<name>.ron`.
    pub layouts: Vec<String>,
    /// Font names; each is `fonts/<name>.ron`.
    pub fonts: Vec<String>,
    /// The sheets pulled from outside INGAME.LVL into `textures/`, with the
    /// LVL each came from (`"COMMON.LVL"`, `"CORE.LVL"`).
    pub textures: Vec<(String, String)>,
}

/// One `hud_` chunk: the tree a `.hud` source file spells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudLayout {
    /// From the layout's `FileInfo("…")` record.
    pub name: String,
    /// The top-level records, `fileinfo` first.
    pub nodes: Vec<HudNode>,
}

/// One DATA record and the SCOP after it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudNode {
    /// The record's name as recovered (`mungefs::hud::NAMES`; the disc stores
    /// only a case-blind hash, so compare case-insensitively), or `#` + the
    /// hash in 8 hex digits for the 11 that stay unnamed.
    pub name: String,
    pub args: Vec<HudArg>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<HudNode>,
}

/// A record argument: the config format stores numbers as f32.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HudArg {
    Num(f32),
    Str(String),
}

/// `fonts/<name>.ron`: one CORE.LVL `font`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudFont {
    pub name: String,
    /// Line height in texels.
    pub line_height: u8,
    /// The glyph sheets in page order; each is `fonts/<page>.ktx2`.
    pub pages: Vec<String>,
    pub glyphs: Vec<HudGlyph>,
}

/// One glyph: its box in texels on its page and that box's UVs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HudGlyph {
    pub code: u16,
    pub page: u8,
    /// Pen advance in texels.
    pub advance: u8,
    pub x0: u8,
    pub x1: u8,
    pub y0: u8,
    pub y1: u8,
    /// `[u0, u1, v0, v1]`.
    pub uv: [f32; 4],
}

/// Optional `fonts/<font>.sdf.ron`: scalable artwork paired with a HudFont,
/// which remains the sole layout authority. Source fonts use player glyphs;
/// the treated menu family shares its own pages across all three roles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudDistanceFont {
    pub pages: Vec<String>,
    pub glyphs: Vec<HudDistanceGlyph>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudDistanceGlyph {
    pub code: u16,
    pub page: u8,
    /// Padded artwork bounds in the original font's layout units.
    pub rect: [f32; 4],
    /// u0, v0, u1, v1 on the generated page.
    pub uv: [f32; 4],
}

/// `locl/<language>.ron`: one CORE.LVL `Locl` table. The disc keeps no key
/// strings: each text sits under the FNV-1a hash of its lower-cased key path
/// ([`locl_key`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudLocl {
    pub language: String,
    /// `(key hash, text)`, sorted by hash.
    pub entries: Vec<(u32, String)>,
}

/// The hash a Locl key path is stored under (FNV-1a over the lower-cased
/// bytes — `mungefs::fnv1a_lowercase`).
pub fn locl_key(path: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in path.bytes() {
        h ^= u32::from(b | 0x20);
        h = h.wrapping_mul(16_777_619);
    }
    h
}

impl HudLocl {
    /// The text under `hash`.
    pub fn get(&self, hash: u32) -> Option<&str> {
        self.entries
            .binary_search_by_key(&hash, |(h, _)| *h)
            .ok()
            .map(|i| self.entries[i].1.as_str())
    }

    /// The text under the key path `path`.
    pub fn text(&self, path: &str) -> Option<&str> {
        self.get(locl_key(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hashes `re-event-messages.md` reads out of the ELF and the disc.
    #[test]
    fn locl_keys_hash_as_the_disc_does() {
        assert_eq!(locl_key("game.infomsg.captured"), 0xdd27_9cfc);
        assert_eq!(locl_key("Game.InfoMsg.Lost"), 0xae1e_c790);
        let l = HudLocl {
            language: "english".into(),
            entries: vec![(1, "a".into()), (0xdd27_9cfc, "%s captured the %s".into())],
        };
        assert_eq!(l.text("game.infomsg.captured"), Some("%s captured the %s"));
        assert_eq!(l.get(2), None);
    }
}
