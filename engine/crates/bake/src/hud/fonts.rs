//! Scalable glyph art, separate from the source font's immutable layout metrics.
use super::distance::{field, PAD, SCALE};
use anyhow::{Context, Result};
use manifest::hud::{HudDistanceFont, HudDistanceGlyph};
use mungefs::font::{Font, Glyph};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) fn read_fonts(path: &Path) -> Result<Vec<Font>> {
    let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
    mungefs::content_chunks(mungefs::open(&bytes)?)?
        .iter()
        .filter(|c| &c.tag == b"font")
        .map(|c| mungefs::font::parse(c).map_err(Into::into))
        .collect()
}

const SIDE: usize = 1024;
const MAX_GLYPH_SIDE: usize = SIDE / SCALE - PAD * 2;

struct GlyphMask {
    alpha: Vec<u8>,
    width: usize,
    height: usize,
    /// Padded mask edges relative to the glyph's original UV rectangle.
    rect: [f32; 4],
    sample_area: f32,
}

struct CropAxis {
    samples: std::ops::Range<usize>,
    edges: [f32; 2],
    span: f32,
}

fn crop_axis(low: f32, high: f32, size: usize) -> Option<CropAxis> {
    if !(0.0..=1.0).contains(&low) || !(low..=1.0).contains(&high) {
        return None;
    }
    let (low, high) = (low * size as f32, high * size as f32);
    let span = high - low;
    // Glyph UVs can fall at texel centres. Include both boundary samples
    // there, and keep their sub-texel offset relative to the layout quad.
    let start = (low - 0.5).ceil() as usize;
    let end = ((high + 0.5).floor() as usize).min(size);
    if span < 0.5 || end <= start || end - start > MAX_GLYPH_SIDE {
        return None;
    }
    Some(CropAxis {
        samples: start..end,
        edges: [
            (start as f32 - low - PAD as f32) / span,
            (end as f32 - low + PAD as f32) / span,
        ],
        span,
    })
}

fn mask(font: &Font, g: &Glyph) -> Option<GlyphMask> {
    let p = font.pages.get(usize::from(g.page))?;
    let (pw, ph) = (p.info.width as usize, p.info.height as usize);
    let [u0, u1, v0, v1] = g.uv;
    let x = crop_axis(u0, u1, pw)?;
    let y = crop_axis(v0, v1, ph)?;
    p.mips.first()?;
    let rgba = p.rgba8(0);
    if rgba.len() != pw.checked_mul(ph)?.checked_mul(4)? {
        return None;
    }
    let alpha = y
        .samples
        .clone()
        .flat_map(|row| x.samples.clone().map(move |col| (row * pw + col) * 4 + 3))
        .map(|i| rgba[i])
        .collect();
    Some(GlyphMask {
        alpha,
        width: x.samples.len(),
        height: y.samples.len(),
        rect: [x.edges[0], y.edges[0], x.edges[1], y.edges[1]],
        sample_area: x.span * y.span,
    })
}

/// Pick by actual glyph sample area, then coverage precision. A donor must
/// carry the same font name and code point; an absent glyph keeps its source.
/// Metrics always come from `base`, even when a cleaner/larger donor wins.
pub(super) fn bake(
    base: &Font,
    donors: &[Vec<Font>],
    out: &Path,
    prefix: &str,
    sources: &[String],
) -> Result<()> {
    let mut page = vec![0u8; SIDE * SIDE * 4];
    let mut pages = Vec::new();
    let mut glyphs = Vec::new();
    let (mut x, mut y, mut row_h) = (0, 0, 0);
    let mut choices = std::collections::BTreeMap::<usize, usize>::new();
    let page_name = |i| format!("{prefix}{}.sdf{i}", base.name);
    for g in &base.glyphs {
        if g.x1 <= g.x0 || g.y1 <= g.y0 {
            continue;
        }
        let mut candidates = Vec::new();
        for (source, f) in std::iter::once((0, base)).chain(
            donors
                .iter()
                .enumerate()
                .filter_map(|(i, fs)| fs.iter().find(|f| f.name == base.name).map(|f| (i + 1, f))),
        ) {
            let Some(d) = f.glyphs.iter().find(|d| d.code == g.code) else {
                continue;
            };
            if let Some(crop) = mask(f, d) {
                let levels = crop
                    .alpha
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len();
                candidates.push((crop.sample_area, levels, source, crop));
            }
        }
        // Stable sort keeps the primary source when both quality measures tie.
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
        let Some((_, _, source, crop)) = candidates.into_iter().next() else {
            continue;
        };
        let Some((rgba, dw, dh)) = field(&crop.alpha, crop.width, crop.height) else {
            continue;
        };
        if x + dw > SIDE {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        if y + dh > SIDE {
            let name = page_name(pages.len());
            std::fs::write(
                out.join(format!("{name}.ktx2")),
                crate::ktx2w::write_rgba8(SIDE as u32, SIDE as u32, &[page]),
            )?;
            pages.push(name);
            page = vec![0; SIDE * SIDE * 4];
            x = 0;
            y = 0;
            row_h = 0;
        }
        let page_index = u8::try_from(pages.len()).context("too many distance font pages")?;
        for yy in 0..dh {
            page[((y + yy) * SIDE + x) * 4..((y + yy) * SIDE + x + dw) * 4]
                .copy_from_slice(&rgba[yy * dw * 4..(yy + 1) * dw * 4]);
        }
        let left = f32::from(g.x0);
        let top = f32::from(g.y0);
        let width = f32::from(g.x1) - left;
        let height = f32::from(g.y1) - top;
        glyphs.push(HudDistanceGlyph {
            code: g.code,
            page: page_index,
            rect: [
                left + crop.rect[0] * width,
                top + crop.rect[1] * height,
                left + crop.rect[2] * width,
                top + crop.rect[3] * height,
            ],
            uv: [
                x as f32 / SIDE as f32,
                y as f32 / SIDE as f32,
                (x + dw) as f32 / SIDE as f32,
                (y + dh) as f32 / SIDE as f32,
            ],
        });
        *choices.entry(source).or_default() += 1;
        x += dw;
        row_h = row_h.max(dh);
    }
    let name = page_name(pages.len());
    std::fs::write(
        out.join(format!("{name}.ktx2")),
        crate::ktx2w::write_rgba8(SIDE as u32, SIDE as u32, &[page]),
    )?;
    pages.push(name);
    crate::write_ron(
        &out.join(format!("{prefix}{}.sdf.ron", base.name)),
        &HudDistanceFont { pages, glyphs },
    )?;
    // Provenance is generated beside the player's art, never a shipped asset.
    let receipt = serde_json::json!({"font":base.name,"encoding":"signed half-coverage contour with texel-centre UVs, 2 samples/texel, range/pad 4 source texels","source_sha256":sources,"glyphs_per_source":choices});
    std::fs::write(
        out.join(format!("{prefix}{}.sdf.json", base.name)),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(())
}

pub(super) fn fingerprint(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(std::fs::read(path)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn font(size: u32) -> Font {
        Font {
            name: "fixture".into(),
            line_height: 8,
            pages: vec![mungefs::texture::Texture {
                name: "page".into(),
                info: mungefs::texture::TextureInfo {
                    width: size,
                    height: size,
                    bpp: 32,
                    detail_compressed: false,
                    mip_distance: 0.0,
                    mip_count: 1,
                },
                palette: Vec::new(),
                mips: vec![vec![255; (size * size * 4) as usize]],
            }],
            glyphs: vec![Glyph {
                code: 65,
                page: 0,
                advance: 5,
                x0: 1,
                x1: 5,
                y0: 2,
                y1: 6,
                uv: [0.0, 1.0, 0.0, 1.0],
            }],
        }
    }
    #[test]
    fn mask_keeps_the_glyph_strokes_on_texel_centre_uv_edges() {
        let mut source = font(8);
        let glyph = &mut source.glyphs[0];
        glyph.uv = [1.5 / 8.0, 3.5 / 8.0, 2.5 / 8.0, 4.5 / 8.0];
        let pixels = &mut source.pages[0].mips[0];
        for (i, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            pixel[3] = i as u8;
        }
        let crop = mask(&source, &source.glyphs[0]).unwrap();
        assert_eq!((crop.width, crop.height), (3, 3));
        assert_eq!(crop.alpha, [17, 18, 19, 25, 26, 27, 33, 34, 35]);
        assert_eq!(crop.rect, [-2.25, -2.25, 3.25, 3.25]);
    }

    #[test]
    fn distance_donor_changes_art_without_moving_layout_metrics() {
        let base = font(4);
        let donor = font(8);
        let before = base.glyphs.clone();
        let out =
            std::env::temp_dir().join(format!("openbf-distance-donor-{}", std::process::id()));
        std::fs::create_dir_all(&out).unwrap();
        bake(
            &base,
            &[vec![donor]],
            &out,
            "",
            &["base".into(), "donor".into()],
        )
        .unwrap();
        assert_eq!(base.glyphs, before);
        let receipt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join("fixture.sdf.json")).unwrap()).unwrap();
        assert_eq!(receipt["glyphs_per_source"]["1"], 1);
        let f: HudDistanceFont =
            ron::from_str(&std::fs::read_to_string(out.join("fixture.sdf.ron")).unwrap()).unwrap();
        assert_eq!(f.glyphs[0].rect, [-1.0, 0.0, 7.0, 8.0]);
        assert_eq!(f.glyphs[0].code, 65);
        std::fs::remove_dir_all(out).unwrap();
    }
}
