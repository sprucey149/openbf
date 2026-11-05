//! BF2 Light: the player's BF2 glyphs, native advances, six R8 levels of
//! SDF alpha erosion, and a left parenthesis mirrored from the right.
//! PS2 input reproduces the approved preview. Other editions receive the same
//! treatment of different source glyphs; no external face or art donor is used.
use super::fonts::{self, DistanceBake, SIDE};
use anyhow::{ensure, Context, Result};
use manifest::hud::{HudDistanceGlyph, HudFont, HudGlyph, MENU_ATLAS, MENU_FONTS};
use mungefs::font::Font;
use std::path::Path;

// Maintainer-approved presentation, quantized from the preview's 0.024 shift.
const EROSION: u8 = 6;

/// Explicit menu-only inputs must be PS2 BF2 fonts, not higher-resolution
/// Xbox/PC donors. Identification uses the embedded PS2 palette structure.
pub(super) fn source(path: &Path) -> Result<Vec<Font>> {
    let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
    let chunks = mungefs::content_chunks(mungefs::open(&bytes)?)?;
    let fonts = fonts::read_fonts(path)?;
    ensure!(
        fonts.len() == 4
            && fonts.iter().any(|f| f.name == "gamefont_medium")
            && MENU_FONTS
                .iter()
                .all(|(_, role)| fonts.iter().any(|f| f.name == *role)),
        "menu font source needs the four PS2 BF2 font roles"
    );
    ensure!(
        chunks.iter().filter(|c| &c.tag == b"font").all(|c| {
            c.child(b"FTEX").is_some_and(|ftex| {
                ftex.children_tagged(b"tex_")
                    .all(|tex| tex.child(b"pal_").is_some())
            })
        }),
        "--menu-font-core requires a local PS2 BF2 CORE.LVL (PS2 font palettes)"
    );
    Ok(fonts)
}

fn bounds(glyph: &HudDistanceGlyph) -> [usize; 4] {
    glyph.uv.map(|v| (v * SIDE as f32).round() as usize)
}

fn mirror_parenthesis(baked: &mut DistanceBake) -> Result<()> {
    let Some(left_index) = baked.font.glyphs.iter().position(|g| g.code == 40) else {
        return Ok(());
    };
    let Some(right) = baked.font.glyphs.iter().find(|g| g.code == 41).cloned() else {
        // An unusable mate leaves this symbol to the legacy bitmap fallback.
        baked.font.glyphs.remove(left_index);
        return Ok(());
    };
    let left = &baked.font.glyphs[left_index];
    let [x0, y0, x1, y1] = bounds(&right);
    let [lx0, ly0, lx1, ly1] = bounds(left);
    let (w, h) = (x1 - x0, y1 - y0);
    let original = &baked.pages[usize::from(right.page)];
    let mirrored: Vec<_> = (y0..y1)
        .flat_map(|y| {
            (x0..x1).rev().flat_map(move |x| {
                original[(y * SIDE + x) * 4..(y * SIDE + x + 1) * 4]
                    .iter()
                    .copied()
            })
        })
        .collect();
    let (page, x, y) = if (lx1 - lx0, ly1 - ly0) == (w, h) {
        (usize::from(left.page), lx0, ly0)
    } else {
        // Unequal native boxes keep their original layout metrics while the
        // left artwork gets an exact mirrored mask, without resampling it.
        let page = baked.pages.len();
        baked.pages.push(vec![0; SIDE * SIDE * 4]);
        let left = &mut baked.font.glyphs[left_index];
        left.page = u8::try_from(page).context("too many menu distance pages")?;
        left.uv = [0.0, 0.0, w as f32 / SIDE as f32, h as f32 / SIDE as f32];
        (page, 0, 0)
    };
    for row in 0..h {
        baked.pages[page][((y + row) * SIDE + x) * 4..((y + row) * SIDE + x + w) * 4]
            .copy_from_slice(&mirrored[row * w * 4..(row + 1) * w * 4]);
    }
    Ok(())
}

fn remove(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => Ok(result?),
    }
}

fn invalidate(dir: &Path) -> Result<()> {
    for (name, _) in MENU_FONTS {
        remove(&dir.join(format!("{name}.ron")))?;
    }
    Ok(())
}

pub(super) fn bake(dir: &Path, source: &[Font], fingerprint: &str, explicit: bool) -> Result<()> {
    let mut roles = Vec::new();
    let mut pages = Vec::new();
    for (name, role) in MENU_FONTS {
        let Some(base) = source.iter().find(|f| f.name == role) else {
            // Partial native fonts retain the legacy family as a whole.
            invalidate(dir)?;
            return Ok(());
        };
        let mut baked = fonts::distance(base, &[], "")?;
        mirror_parenthesis(&mut baked)?;
        let offset = pages.len();
        for glyph in &mut baked.font.glyphs {
            glyph.page =
                u8::try_from(usize::from(glyph.page) + offset).context("too many menu pages")?;
        }
        for mut page in baked.pages {
            for pixel in page.chunks_exact_mut(4) {
                pixel[3] = pixel[3].saturating_sub(EROSION);
            }
            pages.push(page);
        }
        let glyphs = base
            .glyphs
            .iter()
            .filter_map(|g| {
                let distance = baked.font.glyphs.iter().find(|d| d.code == g.code);
                if g.x1 > g.x0 && g.y1 > g.y0 && distance.is_none() {
                    return None;
                }
                Some(HudGlyph {
                    code: g.code,
                    page: distance.map_or(offset as u8, |d| d.page),
                    advance: g.advance,
                    x0: g.x0,
                    x1: g.x1,
                    y0: g.y0,
                    y1: g.y1,
                    uv: g.uv,
                })
            })
            .collect();
        roles.push((
            HudFont {
                name: name.into(),
                line_height: base.line_height,
                pages: Vec::new(),
                glyphs,
            },
            baked.font,
        ));
    }
    let names: Vec<_> = (0..pages.len())
        .map(|i| format!("{MENU_ATLAS}.sdf{i}"))
        .collect();
    // Generate all payloads first. During installation missing role manifests
    // disable the optional family; pages and SDFs precede every metric manifest.
    // A failed/interrupted bake therefore retains readable legacy fallback.
    let mut payloads = Vec::new();
    for (name, page) in names.iter().zip(pages) {
        payloads.push((
            format!("{name}.ktx2"),
            crate::ktx2w::write_rgba8(SIDE as u32, SIDE as u32, &[page]),
        ));
    }
    let mut metrics = Vec::new();
    for (mut font, mut distance) in roles {
        font.pages = names.clone();
        distance.pages = names.clone();
        let pretty = ron::ser::PrettyConfig::default();
        payloads.push((
            format!("{}.sdf.ron", font.name),
            ron::ser::to_string_pretty(&distance, pretty.clone())?.into_bytes(),
        ));
        metrics.push((
            format!("{}.ron", font.name),
            ron::ser::to_string_pretty(&font, pretty)?.into_bytes(),
        ));
    }
    payloads.push(("menu_face.json".into(), serde_json::to_vec_pretty(&serde_json::json!({
        "name": "BF2 Light", "source_sha256": fingerprint, "explicit_ps2_source": explicit,
        "source": if explicit { "local user-owned PS2 BF2 CORE.LVL" } else { "selected local BF2 CORE.LVL; native glyphs without donors" },
        "treatment": "PS2 reproduces the approved preview; other editions are a similar treatment using different source glyphs",
        "erosion_alpha_8bit": EROSION, "parentheses": "left is exact horizontal mirror of right", "pages": names
    }))?));
    invalidate(dir)?;
    remove(&dir.join("OpenBF-Menu-OFL.txt"))?;
    let temporary = dir.join(format!(".menu-write-{}", std::process::id()));
    let result = (|| {
        for (name, bytes) in payloads.into_iter().chain(metrics) {
            std::fs::write(&temporary, bytes)?;
            std::fs::rename(&temporary, dir.join(name))?;
        }
        Ok(())
    })();
    let _ = remove(&temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use manifest::hud::HudDistanceFont;

    /// Compare decoded role data, including every native metric and UV, and
    /// exact atlas/mip bytes. Textual RON line wrapping carries no font data.
    #[test]
    #[ignore = "requires BF2_LIGHT_REFERENCE_FONTS and BF2_LIGHT_CASE_FONTS local outputs"]
    fn approved_overlay_roles_and_atlases() {
        let reference = std::path::PathBuf::from(
            std::env::var_os("BF2_LIGHT_REFERENCE_FONTS").expect("reference fonts"),
        );
        let cases = std::env::var_os("BF2_LIGHT_CASE_FONTS").expect("fresh font cases");
        let cases: Vec<_> = std::env::split_paths(&cases).collect();
        assert!(!cases.is_empty(), "no font cases");
        for case in cases {
            for (name, _) in MENU_FONTS {
                let metrics = |dir: &Path| -> HudFont {
                    ron::from_str(
                        &std::fs::read_to_string(dir.join(format!("{name}.ron"))).unwrap(),
                    )
                    .unwrap()
                };
                let distance = |dir: &Path| -> HudDistanceFont {
                    ron::from_str(
                        &std::fs::read_to_string(dir.join(format!("{name}.sdf.ron"))).unwrap(),
                    )
                    .unwrap()
                };
                let expected = metrics(&reference);
                assert_eq!(
                    metrics(&case),
                    expected,
                    "{} / {name} metrics",
                    case.display()
                );
                assert_eq!(
                    distance(&case),
                    distance(&reference),
                    "{} / {name} distance",
                    case.display()
                );
                for page in &expected.pages {
                    assert_eq!(
                        std::fs::read(case.join(format!("{page}.ktx2"))).unwrap(),
                        std::fs::read(reference.join(format!("{page}.ktx2"))).unwrap(),
                        "{} / {page} bytes",
                        case.display()
                    );
                }
            }
            println!(
                "{}: six decoded roles and three atlas/mip payloads match approved overlay",
                case.display()
            );
        }
    }
    fn directory(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("openbf-light-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn partial_source_disables_old_menu_family_and_keeps_native_files() {
        let dir = directory("partial");
        for (name, _) in MENU_FONTS {
            std::fs::write(dir.join(format!("{name}.ron")), "old menu").unwrap();
        }
        std::fs::write(dir.join("gamefont_large.ron"), "native").unwrap();
        bake(&dir, &[], "fixture", false).unwrap();
        assert!(MENU_FONTS
            .iter()
            .all(|(name, _)| !dir.join(format!("{name}.ron")).exists()));
        assert_eq!(
            std::fs::read_to_string(dir.join("gamefont_large.ron")).unwrap(),
            "native"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn explicit_bad_source_fails_before_changing_existing_fonts() {
        let dir = directory("bad-source");
        let source = dir.join("bad-core.lvl");
        std::fs::write(&source, b"not a core").unwrap();
        std::fs::write(dir.join("menu_gamefont_large.ron"), "old menu").unwrap();
        assert!(
            super::super::bake_fonts_with_menu_source(&source, &dir, "", &[], Some(&source))
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("menu_gamefont_large.ron")).unwrap(),
            "old menu"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn explicit_source_rejects_bf1_pc_and_xbox_fonts() {
        for relative in [
            "extracted/bf1/lvl/DATA/_LVL_PS2/CORE.LVL",
            "extracted/xbox_bf2/core.lvl",
            "PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/core.lvl",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join(relative);
            if !path.exists() {
                eprintln!("SKIP: local {relative} absent");
                continue;
            }
            assert!(
                source(&path).is_err(),
                "wrong game/edition accepted: {relative}"
            );
        }
    }

    #[test]
    fn ps2_treatment_preserves_metrics_and_full_mip_chain() {
        let core = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/CORE.LVL"
        ));
        if !core.exists() {
            eprintln!("SKIP: local PS2 BF2 CORE.LVL absent");
            return;
        }
        let source = source(core).unwrap();
        let dir = directory("ps2");
        bake(&dir, &source, &fonts::fingerprint(core).unwrap(), true).unwrap();
        for (index, (name, role)) in MENU_FONTS.iter().enumerate() {
            let base = source.iter().find(|f| f.name == *role).unwrap();
            let native = fonts::distance(base, &[], "").unwrap();
            assert_eq!(
                native.pages.len(),
                1,
                "approved PS2 preview has one page per role"
            );
            let metrics: HudFont =
                ron::from_str(&std::fs::read_to_string(dir.join(format!("{name}.ron"))).unwrap())
                    .unwrap();
            let distance: HudDistanceFont = ron::from_str(
                &std::fs::read_to_string(dir.join(format!("{name}.sdf.ron"))).unwrap(),
            )
            .unwrap();
            assert_eq!(metrics.line_height, base.line_height);
            assert_eq!(metrics.pages.len(), 3);
            for g in &metrics.glyphs {
                let original = base.glyphs.iter().find(|b| b.code == g.code).unwrap();
                assert_eq!(
                    (g.advance, g.x0, g.y0, g.x1, g.y1, g.uv),
                    (
                        original.advance,
                        original.x0,
                        original.y0,
                        original.x1,
                        original.y1,
                        original.uv
                    )
                );
                assert_eq!(g.page as usize, index);
            }
            let mut expected = native.pages[0].clone();
            let left = native.font.glyphs.iter().find(|g| g.code == 40).unwrap();
            let right = native.font.glyphs.iter().find(|g| g.code == 41).unwrap();
            let [l, t, r, b] = bounds(left);
            let [x, y, x1, y1] = bounds(right);
            assert_eq!((r - l, b - t), (x1 - x, y1 - y));
            for row in 0..b - t {
                for col in 0..r - l {
                    expected
                        [((t + row) * SIDE + l + col) * 4..((t + row) * SIDE + l + col + 1) * 4]
                        .copy_from_slice(
                            &native.pages[0][((y + row) * SIDE + x1 - col - 1) * 4
                                ..((y + row) * SIDE + x1 - col) * 4],
                        );
                }
            }
            for pixel in expected.chunks_exact_mut(4) {
                pixel[3] = pixel[3].saturating_sub(6);
            }
            let bytes = std::fs::read(dir.join(format!("menu_face.sdf{index}.ktx2"))).unwrap();
            assert_eq!(bytes, crate::ktx2w::write_rgba8(1024, 1024, &[expected]));
            let reader = ktx2::Reader::new(&bytes).unwrap();
            assert_eq!(reader.header().level_count, 11);
            assert_eq!(distance.glyphs.len(), native.font.glyphs.len());
        }
        // Force an installation failure after the first page is replaced.
        // No stale metric manifest can select a mixed family afterward.
        let blocked = dir.join("menu_face.sdf1.ktx2");
        std::fs::remove_file(&blocked).unwrap();
        std::fs::create_dir(&blocked).unwrap();
        assert!(bake(&dir, &source, "fixture", true).is_err());
        assert!(MENU_FONTS
            .iter()
            .all(|(name, _)| !dir.join(format!("{name}.ron")).exists()));
        assert!(!dir
            .join(format!(".menu-write-{}", std::process::id()))
            .exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
