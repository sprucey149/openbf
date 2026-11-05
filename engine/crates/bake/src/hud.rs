//! BF2's HUD (card BHUD, `analysis/re-hud.md`): bakes INGAME.LVL's five
//! `hud_` layouts, CORE.LVL's four bitmap fonts and the sheets the layouts
//! name that INGAME.LVL lacks, into the BF2 common bake (schema and file
//! layout: `manifest::hud`).
//!
//! [`bake_hud`] runs from `crate::map::bake_common` on every common LVL and
//! does nothing for one with no `hud_` chunk (BF1's COMMON.LVL: BF1 builds its
//! HUD in code). CORE.LVL and COMMON.LVL are read as the common LVL's
//! siblings (`_LVL_PS2/`), so the rebake needs no new step.
//!
//! The extra sheets are found by data, not by a list: every string argument
//! of every layout that names a `tex_` in COMMON.LVL or CORE.LVL and is not
//! already an INGAME texture. On the disc that is ten: `hud_icon_flash`,
//! `border_3_pieces`, `opaque_black`, `stats_kills`, `btn_directional_pad`,
//! `btn_directional_pad_lr`, `optionl`, `optionr` (COMMON) and `white` (CORE)
//! — `re-hud.md` §1.2 — plus COMMON's `points`, which the layouts name only
//! as an element's own name (`Bitmap("points")`): a small spare sheet, and
//! the spawn screens' button pieces and pad icons ([`SCREEN_SHEETS`]). The
//! side-LVL `hud_*` meshes (`hud_cis_trishot`, …) are not baked here: they
//! belong with the side weapons (card BHUD2).
//!
//! [`bake_locl`] (card EVMSG) writes the English `Locl` table of a game's
//! CORE.LVL — the HUD's event lines, team, post and soldier names — as
//! `locl/english.ron` under that game's common bake (`manifest::hud::HudLocl`).
//!
//! [`bake_fonts`] writes a CORE.LVL's bitmap fonts: [`bake_hud`] runs it for
//! BF2, and `bake fonts <core> <out> <prefix>` for BF1, whose fonts go into the
//! BF1 common bake as `bf1_<font>` (card SQHUD).
//! The unprefixed BF2 bake also writes the BF2 Light menu family from local
//! glyphs, separately from the native HUD (`menu`, issue #114).

use anyhow::{anyhow, Context, Result};
use manifest::hud::{HudArg, HudFont, HudGlyph, HudIndex, HudLayout, HudLocl, HudNode};
use mungefs::config::ConfigNode;
use std::collections::BTreeSet;
use std::path::Path;

mod art;
mod distance;
mod fonts;
mod menu;

/// Generate native UI sheet sidecars, accepting only larger copies which
/// pass the existing same-art gate. No layout or gameplay record is changed.
pub fn bake_native_art(core: &Path, out: &Path, donors: &[std::path::PathBuf]) -> Result<usize> {
    art::bake(core, out, donors)
}

/// The sibling LVLs the extra sheets come from, searched in this order.
const SHEET_SOURCES: [&str; 2] = ["COMMON.LVL", "CORE.LVL"];
/// The sibling LVL holding the fonts.
const FONT_SOURCE: &str = "CORE.LVL";

/// The sheets the spawn screens' shell widgets draw that no layout names
/// (card SSELB, `analysis/re-bf2-shell.md` §1, §3.2, §3.4): the vertical
/// button stack's eight 64x64 pieces and the help lines' pad icons, Cross
/// (`btna`) and Triangle (`btnb`), all in COMMON.LVL; then the Instant
/// Action and Session Options widgets' (`analysis/re-bf2-menus.md` §8): the
/// four era icons, the list focus box and scroll arrows, the options' info
/// box and the flashy text's sweep square.
pub const SCREEN_SHEETS: [&str; 18] = [
    "bf2_buttons_topleft",
    "bf2_buttons_title_center",
    "bf2_buttons_topright",
    "bf2_buttons_midleft",
    "bf2_buttons_items_center",
    "bf2_buttons_midright",
    "bf2_buttons_botleft",
    "bf2_buttons_botright",
    "btna",
    "btnb",
    "all_icon",
    "cis_icon",
    "imp_icon",
    "rep_icon",
    "listbox_cursor",
    "listbox_topbot_more",
    "white_rect",
    "cursor",
];

fn node(n: &ConfigNode) -> HudNode {
    HudNode {
        name: mungefs::hud::name(n.hash).map_or_else(|| format!("#{:08x}", n.hash), str::to_owned),
        args: (0..n.values.len())
            .map(|i| match n.str_arg(i) {
                Some(s) => HudArg::Str(s.to_owned()),
                None => HudArg::Num(f32::from_bits(n.values[i])),
            })
            .collect(),
        children: n.children.iter().map(node).collect(),
    }
}

fn strings(ns: &[HudNode], out: &mut BTreeSet<String>) {
    for n in ns {
        for a in &n.args {
            if let HudArg::Str(s) = a {
                out.insert(s.to_ascii_lowercase());
            }
        }
        strings(&n.children, out);
    }
}

/// Bake every `font` chunk of the CORE.LVL at `core_path` into
/// `<out>/fonts`, each font and its pages named `<prefix><name>`. Answers the
/// baked fonts' names, sorted.
pub fn bake_fonts(core_path: &Path, out: &Path, prefix: &str) -> Result<Vec<String>> {
    bake_fonts_with_donors(core_path, out, prefix, &[])
}

/// Donors contribute only glyph art. Original metrics and bitmap pages stay
/// byte-for-byte compatible, including fonts absent from another edition.
pub fn bake_fonts_with_donors(
    core_path: &Path,
    out: &Path,
    prefix: &str,
    donor_paths: &[std::path::PathBuf],
) -> Result<Vec<String>> {
    bake_fonts_with_menu_source(core_path, out, prefix, donor_paths, None)
}

/// Optional PS2 CORE.LVL supplies menu glyphs only. Native HUD metrics and
/// art donor selection remain those of `core_path` and `donor_paths`.
pub fn bake_fonts_with_menu_source(
    core_path: &Path,
    out: &Path,
    prefix: &str,
    donor_paths: &[std::path::PathBuf],
    menu_core: Option<&Path>,
) -> Result<Vec<String>> {
    anyhow::ensure!(
        menu_core.is_none() || prefix.is_empty(),
        "--menu-font-core is only for unprefixed BF2 fonts"
    );
    let menu_source = menu_core.map(menu::source).transpose()?;
    let core = std::fs::read(core_path).with_context(|| core_path.display().to_string())?;
    let core_inner = mungefs::open(&core).map_err(|e| anyhow!("{}: {e}", core_path.display()))?;
    let fonts_dir = out.join("fonts");
    std::fs::create_dir_all(&fonts_dir)?;
    let donors = donor_paths
        .iter()
        .map(|p| fonts::read_fonts(p))
        .collect::<Result<Vec<_>>>()?;
    let sources = std::iter::once(core_path)
        .chain(donor_paths.iter().map(|p| p.as_path()))
        .map(fonts::fingerprint)
        .collect::<Result<Vec<_>>>()?;
    let mut fonts = Vec::new();
    for c in mungefs::content_chunks(core_inner)
        .map_err(|e| anyhow!("{e}"))?
        .iter()
        .filter(|c| &c.tag == b"font")
    {
        let f = mungefs::font::parse(c).map_err(|e| anyhow!("font 0x{:X}: {e}", c.offset))?;
        let pages: Vec<String> = f
            .pages
            .iter()
            .map(|p| format!("{prefix}{}", p.name))
            .collect();
        for (p, name) in f.pages.iter().zip(&pages) {
            crate::detail::write_ktx2(&fonts_dir, &format!("{name}.ktx2"), p)?;
        }
        let hf = HudFont {
            name: format!("{prefix}{}", f.name),
            line_height: f.line_height,
            pages,
            glyphs: f
                .glyphs
                .iter()
                .map(|g| HudGlyph {
                    code: g.code,
                    page: g.page,
                    advance: g.advance,
                    x0: g.x0,
                    x1: g.x1,
                    y0: g.y0,
                    y1: g.y1,
                    uv: g.uv,
                })
                .collect(),
        };
        crate::write_ron(&fonts_dir.join(format!("{}.ron", hf.name)), &hf)?;
        fonts::bake(&f, &donors, &fonts_dir, prefix, &sources)?;
        fonts.push(hf.name);
    }
    if prefix.is_empty() {
        let selected;
        let source = if let Some(source) = &menu_source {
            source
        } else {
            selected = fonts::read_fonts(core_path)?;
            &selected
        };
        let fingerprint = fonts::fingerprint(menu_core.unwrap_or(core_path))?;
        menu::bake(&fonts_dir, source, &fingerprint, menu_core.is_some())?;
    }
    fonts.sort();
    Ok(fonts)
}

/// Bake the HUD of the common LVL at `lvl_path` into `outdir`; `None` when
/// the LVL has no `hud_` chunk.
pub fn bake_hud(lvl_path: &str, outdir: &str) -> Result<Option<HudIndex>> {
    let buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{lvl_path}: {e}"))?;
    let content = mungefs::content_chunks(inner).map_err(|e| anyhow!("{lvl_path}: {e}"))?;
    let geometry = |detect: fn(&mungefs::Chunk<'_>) -> bool| {
        content.iter().filter(|c| &c.tag == b"modl").any(|model| {
            model
                .children_tagged(b"segm")
                .any(|segment| detect(&segment))
        })
    };
    let source = if geometry(mungefs::model::is_xbox_geometry) {
        crate::hud_layout::Source::Xbox
    } else if geometry(mungefs::model::is_pc_geometry) {
        crate::hud_layout::Source::Pc
    } else {
        crate::hud_layout::Source::Ps2
    };
    let mut layouts = Vec::new();
    for c in content.iter().filter(|c| &c.tag == b"hud_") {
        let l = mungefs::hud::parse(c).map_err(|e| anyhow!("hud_ 0x{:X}: {e}", c.offset))?;
        let name = l
            .file_name()
            .ok_or_else(|| anyhow!("hud_ 0x{:X}: no FileInfo name", c.offset))?
            .to_owned();
        let layout = HudLayout {
            name,
            nodes: l.nodes.iter().map(node).collect(),
        };
        layouts.push(layout);
    }
    if layouts.is_empty() {
        return Ok(None);
    }
    let out = Path::new(outdir);
    let dir = Path::new(lvl_path)
        .parent()
        .ok_or_else(|| anyhow!("{lvl_path}: no parent dir"))?;
    std::fs::create_dir_all(out.join("hud"))?;
    // Keep the selected font metrics native. PC cap-bearing correction needs
    // only these freshly baked metrics, never a console input or font donor.
    let fonts = bake_fonts(&crate::join_ci(dir, FONT_SOURCE), out, "")?;
    let native_fonts = fonts
        .iter()
        .map(|name| {
            let p = out.join(format!("fonts/{name}.ron"));
            ron::from_str::<HudFont>(&std::fs::read_to_string(&p)?).map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    for layout in &mut layouts {
        crate::hud_layout::normalize(layout, source, &native_fonts);
        crate::write_ron(
            &out.join("hud").join(format!("{}.ron", layout.name)),
            layout,
        )?;
    }

    // The sheets the layouts name that INGAME lacks.
    let mut wanted = BTreeSet::new();
    for l in &layouts {
        strings(&l.nodes, &mut wanted);
    }
    wanted.extend(SCREEN_SHEETS.map(str::to_owned));
    for c in content.iter().filter(|c| &c.tag == b"tex_") {
        if let Some(n) = c.child_str(b"NAME") {
            wanted.remove(&n.to_ascii_lowercase());
        }
    }
    let tex_dir = out.join("textures");
    std::fs::create_dir_all(&tex_dir)?;
    let mut textures = Vec::new();
    for src in SHEET_SOURCES {
        let p = crate::join_ci(dir, src);
        let bytes = std::fs::read(&p).with_context(|| p.display().to_string())?;
        let inner = mungefs::open(&bytes).map_err(|e| anyhow!("{}: {e}", p.display()))?;
        for c in mungefs::content_chunks(inner)
            .map_err(|e| anyhow!("{e}"))?
            .iter()
            .filter(|c| &c.tag == b"tex_")
        {
            let Some(n) = c.child_str(b"NAME") else {
                continue;
            };
            if !wanted.remove(&n.to_ascii_lowercase()) {
                continue;
            }
            let t = mungefs::texture::Texture::parse(c)
                .map_err(|e| anyhow!("{src} tex_ 0x{:X}: {e}", c.offset))?;
            crate::detail::write_ktx2(&tex_dir, &format!("{}.ktx2", t.name), &t)?;
            textures.push((t.name, src.to_owned()));
        }
    }
    textures.sort();

    let index = HudIndex {
        layouts: layouts.into_iter().map(|l| l.name).collect(),
        fonts,
        textures,
    };
    crate::write_ron(&out.join("hud").join("index.ron"), &index)?;
    bake_native_art(&crate::join_ci(dir, FONT_SOURCE), out, &[])?;
    Ok(Some(index))
}

/// The language the port shows.
pub const LOCL_LANGUAGE: &str = "english";

/// Bake the English text table of the CORE.LVL at `core_path` into
/// `outdir/locl/english.ron`, sorted by key hash.
pub fn bake_locl(core_path: &str, outdir: &str) -> Result<HudLocl> {
    let buf = std::fs::read(core_path).with_context(|| core_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{core_path}: {e}"))?;
    let content = mungefs::content_chunks(inner).map_err(|e| anyhow!("{core_path}: {e}"))?;
    let table = mungefs::locl::find(&content, LOCL_LANGUAGE)
        .map_err(|e| anyhow!("{core_path}: Locl: {e}"))?
        .ok_or_else(|| anyhow!("{core_path}: no {LOCL_LANGUAGE} Locl table"))?;
    let mut entries = table.entries;
    entries.sort_by_key(|(h, _)| *h);
    entries.dedup_by_key(|(h, _)| *h);
    let locl = HudLocl {
        language: table.language,
        entries,
    };
    let dir = Path::new(outdir).join("locl");
    std::fs::create_dir_all(&dir)?;
    crate::write_ron(&dir.join(format!("{LOCL_LANGUAGE}.ron")), &locl)?;
    Ok(locl)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-gated (card EVMSG): both games' tables bake and read back, with
    /// the lines and names the event messages are built from.
    #[test]
    fn both_games_bake_their_english_text() {
        for (game, line, name, first) in [
            ("bf1", "%s CAPTURED THE %s", "REBELS", "RAYCE"),
            ("bf2", "%s captured the %s", "Rebels", "Rayce"),
        ] {
            let p = format!(
                "{}/../../../extracted/{game}/lvl/DATA/_LVL_PS2/CORE.LVL",
                env!("CARGO_MANIFEST_DIR")
            );
            if !Path::new(&p).exists() {
                eprintln!("SKIP: extracted {game} CORE.LVL not present");
                continue;
            }
            let out =
                std::env::temp_dir().join(format!("openbf-locl-{game}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&out);
            bake_locl(&p, out.to_str().unwrap()).unwrap();
            let back: HudLocl =
                ron::from_str(&std::fs::read_to_string(out.join("locl/english.ron")).unwrap())
                    .unwrap();
            assert!(back.entries.windows(2).all(|w| w[0].0 < w[1].0), "sorted");
            assert_eq!(back.text("game.infomsg.captured"), Some(line));
            assert_eq!(back.text("common.sides.all.name"), Some(name));
            assert_eq!(back.text("common.sides.all.firstnames.01"), Some(first));
            let _ = std::fs::remove_dir_all(&out);
        }
    }

    /// Data-gated (card BHUD's acceptance): the five layouts, four fonts and
    /// the COMMON / CORE sheets, read back from what the bake wrote.
    #[test]
    fn bf2_ingame_bakes_the_hud() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/INGAME.LVL"
        );
        if !Path::new(p).exists() {
            eprintln!("SKIP: extracted BF2 INGAME.LVL not present");
            return;
        }
        let out = std::env::temp_dir().join(format!("openbf-hud-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let index = bake_hud(p, out.to_str().unwrap())
            .unwrap()
            .expect("BF2 has a HUD");
        assert_eq!(
            index.layouts,
            [
                "hudtransforms",
                "1playerhud",
                "2playerhud",
                "3playerhud",
                "4playerhud"
            ]
        );
        assert_eq!(
            index.fonts,
            [
                "gamefont_large",
                "gamefont_medium",
                "gamefont_small",
                "gamefont_tiny"
            ]
        );
        let sheets: Vec<&str> = index.textures.iter().map(|(n, _)| n.as_str()).collect();
        for want in [
            "hud_icon_flash",
            "border_3_pieces",
            "opaque_black",
            "stats_kills",
            "white",
        ]
        .into_iter()
        .chain(SCREEN_SHEETS)
        {
            assert!(sheets.contains(&want), "{want} missing from {sheets:?}");
        }
        assert!(sheets.iter().any(|s| s.starts_with("btn_directional_pad")));
        for (n, _) in &index.textures {
            assert!(
                out.join("textures").join(format!("{n}.ktx2")).exists(),
                "{n}"
            );
        }

        fn count(ns: &[HudNode]) -> usize {
            ns.iter().map(|n| 1 + count(&n.children)).sum()
        }
        let one: HudLayout =
            ron::from_str(&std::fs::read_to_string(out.join("hud/1playerhud.ron")).unwrap())
                .unwrap();
        assert_eq!(count(&one.nodes), 2343);
        assert_eq!(
            one.nodes.iter().filter(|n| n.name != "fileinfo").count(),
            42
        );

        let medium: HudFont =
            ron::from_str(&std::fs::read_to_string(out.join("fonts/gamefont_medium.ron")).unwrap())
                .unwrap();
        assert_eq!(medium.glyphs.len(), 226);
        for p in &medium.pages {
            assert!(out.join("fonts").join(format!("{p}.ktx2")).exists(), "{p}");
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    /// BF1's COMMON.LVL has no `hud_` chunk: the bake writes nothing.
    #[test]
    fn bf1_common_has_no_hud() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL"
        );
        if !Path::new(p).exists() {
            eprintln!("SKIP: extracted BF1 COMMON.LVL not present");
            return;
        }
        let out = std::env::temp_dir().join(format!("openbf-hud-bf1-{}", std::process::id()));
        assert!(bake_hud(p, out.to_str().unwrap()).unwrap().is_none());
        assert!(!out.exists());
    }
}
