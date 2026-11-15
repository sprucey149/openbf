//! `[ours]` The menu face for egui: the same outlines the native pass draws.
//!
//! egui takes TrueType data only, and the bake holds the face as signed
//! distance pages. At load time each metric set's glyphs are traced from
//! those pages at the native pass's threshold (half the distance range, as
//! `render::hud`'s `fs_distance`) into polygon outlines, and written into a
//! minimal TrueType font: one em is the set's line height, the ascent is the
//! whole em, and every x is the role's stretch × the shell frame's own 4:3
//! shape (a shell unit is 448/384 as wide as it is tall), so egui text at
//! [`super::egui_font`]'s size lands exactly where native text would.
//! Advances are the metric set's own.

use super::{Face, TextStyle, ROLES};
use manifest::hud::{HudDistanceFont, HudFont};

/// Font units per em.
const UPEM: u16 = 2048;
/// A shell unit's width over its height on a 4:3 box (512 × 448 shown 4:3).
const UNIT_ASPECT: f32 = (4.0 / 3.0) * (448.0 / 512.0);
/// The native pass's coverage edge: half the distance range.
const EDGE: f32 = 127.5;
/// Outline points closer than this to a straight line through their
/// neighbours are dropped (texels).
const SIMPLIFY: f32 = 0.12;

/// A glyph for the font file: its code, its advance and its closed outlines
/// (font units, y up).
type Glyph = (u16, u16, Vec<Vec<(i16, i16)>>);

/// One distance page: RGBA8 rows, the distance in alpha.
pub struct Page<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}

/// The stretch a face is installed with: its roles' own (the roles of one
/// face share it; `tests::roles_of_one_face_share_their_stretch`).
pub fn stretch(face: Face) -> f32 {
    ROLES
        .iter()
        .map(|(_, s)| s)
        .find(|s: &&TextStyle| s.face == face)
        .map_or(1.0, |s| s.stretch)
}

/// The TrueType bytes of one metric set, or `None` when it has no outline.
pub fn build<'a>(
    font: &HudFont,
    distance: &HudDistanceFont,
    page: impl Fn(&str) -> Option<Page<'a>>,
    stretch: f32,
) -> Option<Vec<u8>> {
    let lh = f32::from(font.line_height.max(1));
    let ky = f32::from(UPEM) / lh;
    let kx = ky * stretch * UNIT_ASPECT;
    let mut glyphs: Vec<Glyph> = Vec::new();
    let mut codes: Vec<u16> = font.glyphs.iter().map(|g| g.code).collect();
    codes.sort_unstable();
    codes.dedup();
    let mut outlined = 0;
    for code in codes {
        let Some(g) = font.glyphs.iter().find(|g| g.code == code) else {
            continue;
        };
        let advance = (f32::from(g.advance) * kx).round().clamp(0.0, 65535.0) as u16;
        let contours = distance
            .glyphs
            .iter()
            .find(|d| d.code == code)
            .and_then(|d| {
                let name = distance.pages.get(usize::from(d.page))?;
                let p = page(name)?;
                Some(trace(&p, d.rect, d.uv))
            })
            .unwrap_or_default();
        let contours: Vec<Vec<(i16, i16)>> = contours
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .map(|(x, y)| {
                        (
                            (x * kx).round().clamp(-32000.0, 32000.0) as i16,
                            ((lh - y) * ky).round().clamp(-32000.0, 32000.0) as i16,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|c: &Vec<(i16, i16)>| c.len() >= 3)
            .collect();
        if !contours.is_empty() {
            outlined += 1;
        }
        glyphs.push((code, advance, contours));
    }
    (outlined > 0).then(|| write_ttf(&glyphs))
}

/// The glyph's outlines in its layout units (x right, y down from the
/// line's top), traced from its page at the native threshold.
fn trace(p: &Page<'_>, rect: [f32; 4], uv: [f32; 4]) -> Vec<Vec<(f32, f32)>> {
    let (w, h) = (p.width as f32, p.height as f32);
    let (px0, py0) = ((uv[0] * w).round() as i64, (uv[1] * h).round() as i64);
    let (px1, py1) = ((uv[2] * w).round() as i64, (uv[3] * h).round() as i64);
    let (nx, ny) = ((px1 - px0).max(0) as usize, (py1 - py0).max(0) as usize);
    if nx == 0 || ny == 0 || p.rgba.len() < (p.width * p.height * 4) as usize {
        return Vec::new();
    }
    // Samples at texel centres, one ring of outside around them.
    let (gw, gh) = (nx + 2, ny + 2);
    let mut f = vec![-EDGE; gw * gh];
    for j in 0..ny {
        for i in 0..nx {
            let (x, y) = (px0 + i as i64, py0 + j as i64);
            if x < 0 || y < 0 || x >= i64::from(p.width) || y >= i64::from(p.height) {
                continue;
            }
            let a = p.rgba[((y as usize) * p.width as usize + x as usize) * 4 + 3];
            f[(j + 1) * gw + i + 1] = f32::from(a) - EDGE;
        }
    }
    let at = |i: usize, j: usize| f[j * gw + i];
    // Texel-grid point → layout units: the quad's rect spans the uv box.
    let to_layout = |gx: f32, gy: f32| {
        let tx = gx - 1.0 + 0.5; // texel-centre sample → texel coordinate
        let ty = gy - 1.0 + 0.5;
        (
            rect[0] + tx / nx as f32 * (rect[2] - rect[0]),
            rect[1] + ty / ny as f32 * (rect[3] - rect[1]),
        )
    };
    // Edge keys: (i, j, 0) the edge from (i, j) to (i+1, j); (i, j, 1) to
    // (i, j+1).
    type Key = (usize, usize, u8);
    let cross = |k: Key| -> (f32, f32) {
        let (i, j, d) = k;
        let (i2, j2) = if d == 0 { (i + 1, j) } else { (i, j + 1) };
        let (a, b) = (at(i, j), at(i2, j2));
        let t = (a / (a - b)).clamp(0.0, 1.0);
        (
            i as f32 + t * (i2 as f32 - i as f32),
            j as f32 + t * (j2 as f32 - j as f32),
        )
    };
    let mut next: std::collections::HashMap<Key, Key> = std::collections::HashMap::new();
    for j in 0..gh - 1 {
        for i in 0..gw - 1 {
            // Corners clockwise on screen (y down) and the edges between.
            let c = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let inside = c.map(|(x, y)| at(x, y) > 0.0);
            let edges: [Key; 4] = [(i, j, 0), (i + 1, j, 1), (i, j + 1, 0), (i, j, 1)];
            // Walking the corners in order: (edge, entering the inside).
            let mut hits: Vec<(Key, bool)> = Vec::new();
            for k in 0..4 {
                let (a, b) = (inside[k], inside[(k + 1) % 4]);
                if a != b {
                    hits.push((edges[k], b));
                }
            }
            match hits.len() {
                2 => {
                    let (enter, exit) = if hits[0].1 {
                        (hits[0].0, hits[1].0)
                    } else {
                        (hits[1].0, hits[0].0)
                    };
                    next.insert(enter, exit);
                }
                4 => {
                    // Rotate so the walk starts entering: E1 X1 E2 X2.
                    let s = if hits[0].1 { 0 } else { 1 };
                    let h = |k: usize| hits[(s + k) % 4].0;
                    let centre = c.iter().map(|&(x, y)| at(x, y)).sum::<f32>() > 0.0;
                    if centre {
                        next.insert(h(2), h(1));
                        next.insert(h(0), h(3));
                    } else {
                        next.insert(h(0), h(1));
                        next.insert(h(2), h(3));
                    }
                }
                _ => {}
            }
        }
    }
    let mut loops = Vec::new();
    let mut keys: Vec<Key> = next.keys().copied().collect();
    keys.sort_unstable();
    for start in keys {
        if !next.contains_key(&start) {
            continue;
        }
        let mut pts = Vec::new();
        let mut k = start;
        while let Some(n) = next.remove(&k) {
            pts.push(cross(k));
            k = n;
            if k == start {
                break;
            }
        }
        if pts.len() >= 3 {
            loops.push(simplify(&pts));
        }
    }
    // TrueType fills clockwise outlines (y up): the outer loop, the one of
    // largest area, sets which way this trace runs.
    let area = |l: &[(f32, f32)]| {
        l.iter()
            .zip(l.iter().cycle().skip(1))
            .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
            .sum::<f32>()
            * 0.5
    };
    let outer = loops
        .iter()
        .map(|l| area(l))
        .fold(0.0_f32, |m, a| if a.abs() > m.abs() { a } else { m });
    // y down here: a positive shoelace area is clockwise on screen, which is
    // anticlockwise once y points up.
    let flip = outer > 0.0;
    loops
        .into_iter()
        .filter(|l| l.len() >= 3)
        .map(|mut l| {
            if flip {
                l.reverse();
            }
            l.into_iter().map(|(x, y)| to_layout(x, y)).collect()
        })
        .collect()
}

/// Douglas-Peucker over a closed loop.
fn simplify(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    fn dp(p: &[(f32, f32)], out: &mut Vec<(f32, f32)>) {
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        let mut far = (0.0, 0);
        for (k, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
            let d = if len > 1e-6 {
                ((q.0 - a.0) * dy - (q.1 - a.1) * dx).abs() / len
            } else {
                ((q.0 - a.0).powi(2) + (q.1 - a.1).powi(2)).sqrt()
            };
            if d > far.0 {
                far = (d, k);
            }
        }
        if far.0 > SIMPLIFY {
            dp(&p[..=far.1], out);
            dp(&p[far.1..], out);
        } else {
            out.push(a);
        }
    }
    let mut closed = pts.to_vec();
    closed.push(pts[0]);
    let mut out = Vec::new();
    dp(&closed, &mut out);
    out
}

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn bei16(v: &mut Vec<u8>, x: i16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// A minimal TrueType font: `.notdef` then one glyph per code, on-curve
/// polygon outlines, a format 4 character map.
fn write_ttf(glyphs: &[Glyph]) -> Vec<u8> {
    let n = glyphs.len() + 1;
    let (mut glyf, mut loca) = (Vec::new(), Vec::new());
    let mut bounds = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
    let (mut max_points, mut max_contours) = (0u16, 0u16);
    let mut hmtx = Vec::new();
    // .notdef: empty (its start and end), half an em.
    be32(&mut loca, 0);
    be32(&mut loca, 0);
    be16(&mut hmtx, UPEM / 2);
    bei16(&mut hmtx, 0);
    let mut max_adv = UPEM / 2;
    for (_, advance, contours) in glyphs {
        max_adv = max_adv.max(*advance);
        let mut lsb = 0;
        if !contours.is_empty() {
            let pts = contours.iter().flatten();
            let b = pts.fold((i16::MAX, i16::MAX, i16::MIN, i16::MIN), |b, p| {
                (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
            });
            lsb = b.0;
            bounds = (
                bounds.0.min(b.0),
                bounds.1.min(b.1),
                bounds.2.max(b.2),
                bounds.3.max(b.3),
            );
            let count: usize = contours.iter().map(Vec::len).sum();
            max_points = max_points.max(count as u16);
            max_contours = max_contours.max(contours.len() as u16);
            bei16(&mut glyf, contours.len() as i16);
            for v in [b.0, b.1, b.2, b.3] {
                bei16(&mut glyf, v);
            }
            let mut end = 0usize;
            for c in contours {
                end += c.len();
                be16(&mut glyf, (end - 1) as u16);
            }
            be16(&mut glyf, 0); // no instructions
            glyf.extend(std::iter::repeat_n(1u8, count)); // on curve, long deltas
            let mut last = 0i16;
            for p in contours.iter().flatten() {
                bei16(&mut glyf, p.0.wrapping_sub(last));
                last = p.0;
            }
            last = 0;
            for p in contours.iter().flatten() {
                bei16(&mut glyf, p.1.wrapping_sub(last));
                last = p.1;
            }
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
        be32(&mut loca, glyf.len() as u32);
        be16(&mut hmtx, *advance);
        bei16(&mut hmtx, lsb);
    }
    if bounds.0 > bounds.2 {
        bounds = (0, 0, 0, 0);
    }

    let mut head = Vec::new();
    be32(&mut head, 0x0001_0000);
    be32(&mut head, 0x0001_0000);
    be32(&mut head, 0); // checksum adjustment
    be32(&mut head, 0x5F0F_3CF5);
    be16(&mut head, 0x000B);
    be16(&mut head, UPEM);
    head.extend_from_slice(&[0; 16]); // created, modified
    for v in [bounds.0, bounds.1, bounds.2, bounds.3] {
        bei16(&mut head, v);
    }
    be16(&mut head, 0); // mac style
    be16(&mut head, 6); // lowest rec ppem
    bei16(&mut head, 2);
    bei16(&mut head, 1); // long loca
    bei16(&mut head, 0);

    let mut hhea = Vec::new();
    be32(&mut hhea, 0x0001_0000);
    bei16(&mut hhea, UPEM as i16); // ascent: the whole line
    bei16(&mut hhea, 0);
    bei16(&mut hhea, 0);
    be16(&mut hhea, max_adv);
    bei16(&mut hhea, bounds.0.min(0));
    bei16(&mut hhea, 0);
    bei16(&mut hhea, bounds.2);
    bei16(&mut hhea, 1);
    bei16(&mut hhea, 0);
    bei16(&mut hhea, 0);
    hhea.extend_from_slice(&[0; 8]);
    bei16(&mut hhea, 0);
    be16(&mut hhea, n as u16);

    let mut maxp = Vec::new();
    be32(&mut maxp, 0x0001_0000);
    be16(&mut maxp, n as u16);
    be16(&mut maxp, max_points);
    be16(&mut maxp, max_contours);
    be16(&mut maxp, 0);
    be16(&mut maxp, 0);
    be16(&mut maxp, 2);
    for _ in 0..9 {
        be16(&mut maxp, 0);
    }

    // cmap: format 4, one segment per code.
    let mut segs: Vec<(u16, i16)> = glyphs
        .iter()
        .enumerate()
        .filter(|(_, g)| g.0 != 0xFFFF)
        .map(|(k, g)| (g.0, (k as i32 + 1 - i32::from(g.0)) as i16))
        .collect();
    segs.push((0xFFFF, 1));
    let sc = segs.len() as u16;
    let mut sub = Vec::new();
    be16(&mut sub, 4);
    be16(&mut sub, 16 + 8 * sc);
    be16(&mut sub, 0);
    be16(&mut sub, sc * 2);
    let pow = 1u16 << (15 - sc.leading_zeros().min(15));
    let pow = if pow > sc { pow >> 1 } else { pow }.max(1);
    be16(&mut sub, pow * 2);
    be16(&mut sub, pow.trailing_zeros() as u16);
    be16(&mut sub, sc * 2 - pow * 2);
    for s in &segs {
        be16(&mut sub, s.0);
    }
    be16(&mut sub, 0);
    for s in &segs {
        be16(&mut sub, s.0);
    }
    for s in &segs {
        bei16(&mut sub, s.1);
    }
    for _ in &segs {
        be16(&mut sub, 0);
    }
    let mut cmap = Vec::new();
    be16(&mut cmap, 0);
    be16(&mut cmap, 1);
    be16(&mut cmap, 3);
    be16(&mut cmap, 1);
    be32(&mut cmap, 12);
    cmap.extend(sub);

    let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
    ];
    tables.sort_by_key(|t| *t.0);
    let nt = tables.len() as u16;
    let mut out = Vec::new();
    be32(&mut out, 0x0001_0000);
    be16(&mut out, nt);
    let pow = 1u16 << (15 - nt.leading_zeros());
    let pow = if pow > nt { pow >> 1 } else { pow };
    be16(&mut out, pow * 16);
    be16(&mut out, pow.trailing_zeros() as u16);
    be16(&mut out, nt * 16 - pow * 16);
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    for (tag, data) in &tables {
        let sum = data
            .chunks(4)
            .map(|c| {
                let mut w = [0u8; 4];
                w[..c.len()].copy_from_slice(c);
                u32::from_be_bytes(w)
            })
            .fold(0u32, u32::wrapping_add);
        out.extend_from_slice(*tag);
        be32(&mut out, sum);
        be32(&mut out, offset as u32);
        be32(&mut out, data.len() as u32);
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = 12 + 16 * tables.len() + body.len();
    }
    out.extend(body);
    out
}

/// The installed fonts: each metric set's TrueType bytes.
pub type Fonts = Vec<(Face, Vec<u8>)>;

/// Install the face in `ctx`: one family per metric set, and egui's own
/// proportional and monospace text in the small set, so every egui text is
/// the menu face. egui's face stays behind each as the fallback for a
/// character the menu face lacks.
pub fn install(ctx: &egui::Context, fonts: &[(Face, Vec<u8>)]) {
    if fonts.is_empty() {
        return;
    }
    let mut defs = egui::FontDefinitions::default();
    let fallback_prop = defs
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (face, bytes) in fonts {
        let name = face.family().to_owned();
        defs.font_data.insert(
            name.clone(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes.clone())),
        );
        let mut family = vec![name];
        family.extend(fallback_prop.iter().cloned());
        defs.families
            .insert(egui::FontFamily::Name(face.family().into()), family);
    }
    let body = fonts
        .iter()
        .find(|(f, _)| *f == super::BODY.face)
        .or(fonts.first())
        .map(|(f, _)| f.family().to_owned());
    if let Some(body) = body {
        for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            defs.families
                .entry(fam)
                .or_default()
                .insert(0, body.clone());
        }
    }
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use manifest::hud::{HudDistanceGlyph, HudGlyph};

    /// A 32 × 32 page holding one ring (an "O": outer radius 12, inner 6).
    fn ring_page() -> Vec<u8> {
        let mut rgba = vec![0u8; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                let d = (((x as f32 + 0.5 - 16.0).powi(2) + (y as f32 + 0.5 - 16.0).powi(2))
                    .sqrt()
                    - 9.0)
                    .abs();
                // Inside the band |r - 9| < 3: distance above the edge.
                let a = (127.5 + (3.0 - d) * 30.0).clamp(0.0, 255.0);
                rgba[(y * 32 + x) * 4 + 3] = a as u8;
            }
        }
        rgba
    }

    fn fonts() -> (HudFont, HudDistanceFont) {
        let font = HudFont {
            name: "t".into(),
            line_height: 16,
            pages: vec!["p".into()],
            glyphs: vec![
                HudGlyph {
                    code: u16::from(b'O'),
                    page: 0,
                    advance: 16,
                    x0: 0,
                    x1: 16,
                    y0: 0,
                    y1: 16,
                    uv: [0.0, 1.0, 0.0, 1.0],
                },
                HudGlyph {
                    code: u16::from(b' '),
                    page: 0,
                    advance: 6,
                    x0: 0,
                    x1: 0,
                    y0: 0,
                    y1: 0,
                    uv: [0.0; 4],
                },
            ],
        };
        let distance = HudDistanceFont {
            pages: vec!["p".into()],
            glyphs: vec![HudDistanceGlyph {
                code: u16::from(b'O'),
                page: 0,
                rect: [0.0, 0.0, 16.0, 16.0],
                uv: [0.0, 0.0, 1.0, 1.0],
            }],
        };
        (font, distance)
    }

    #[test]
    fn a_traced_ring_has_an_outer_loop_and_an_opposite_hole() {
        let rgba = ring_page();
        let p = Page {
            rgba: &rgba,
            width: 32,
            height: 32,
        };
        let loops = trace(&p, [0.0, 0.0, 16.0, 16.0], [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(loops.len(), 2, "outer and hole");
        let area = |l: &Vec<(f32, f32)>| {
            l.iter()
                .zip(l.iter().cycle().skip(1))
                .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
                .sum::<f32>()
                * 0.5
        };
        let (a, b) = (area(&loops[0]), area(&loops[1]));
        assert!(a * b < 0.0, "the hole runs the other way: {a} {b}");
        // Outer radius 12 texels = 6 units: area about pi * 36.
        let outer = a.abs().max(b.abs());
        assert!((outer - std::f32::consts::PI * 36.0).abs() < 8.0, "{outer}");
    }

    #[test]
    fn the_built_font_parses_with_the_metric_sets_advances_and_one_em_per_line() {
        let rgba = ring_page();
        let (font, distance) = fonts();
        let bytes = build(
            &font,
            &distance,
            |name| {
                (name == "p").then_some(Page {
                    rgba: &rgba,
                    width: 32,
                    height: 32,
                })
            },
            1.0,
        )
        .expect("outlined");
        let f = ab_glyph_check(&bytes);
        assert_eq!(f.0, UPEM);
        // O's advance: 16 units × (2048 / 16) × 448/384.
        let expect = 16.0 * 128.0 * UNIT_ASPECT;
        assert!((f32::from(f.1) - expect).abs() <= 1.0, "{} {expect}", f.1);
        assert!(f.2 > 0, "O has an outline");
    }

    /// (units per em, O's advance, O's outline point count) as egui's
    /// font loader reads them.
    fn ab_glyph_check(bytes: &[u8]) -> (u16, u16, usize) {
        let mut defs = egui::FontDefinitions::empty();
        defs.font_data.insert(
            "t".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes.to_vec())),
        );
        defs.families
            .insert(egui::FontFamily::Proportional, vec!["t".into()]);
        let ctx = egui::Context::default();
        ctx.set_fonts(defs);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let w = ctx.fonts_mut(|f| f.glyph_width(&egui::FontId::proportional(16.0), 'O'));
        // 16 points per em: O is 16 × 448/384 points wide.
        assert!((w - 16.0 * UNIT_ASPECT).abs() < 0.1, "{w}");
        let row = ctx.fonts_mut(|f| f.row_height(&egui::FontId::proportional(16.0)));
        assert!((row - 16.0).abs() < 0.5, "one em per line: {row}");
        let header = |o: usize| u16::from_be_bytes([bytes[o], bytes[o + 1]]);
        let nt = usize::from(header(4));
        let table = |tag: &[u8]| {
            (0..nt)
                .map(|k| 12 + 16 * k)
                .find(|&r| &bytes[r..r + 4] == tag)
                .map(|r| u32::from_be_bytes(bytes[r + 8..r + 12].try_into().unwrap()) as usize)
                .unwrap()
        };
        let head = table(b"head");
        let hmtx = table(b"hmtx");
        let loca = table(b"loca");
        let at32 = |o: usize| u32::from_be_bytes(bytes[o..o + 4].try_into().unwrap());
        // .notdef and ' ' are empty; 'O' (glyph 2) holds the outline.
        assert_eq!([at32(loca), at32(loca + 4), at32(loca + 8)], [0, 0, 0]);
        assert!(at32(loca + 12) > 0);
        let glyf = table(b"glyf");
        let upem = header(head + 18);
        // Glyph 1 is the first code: ' ' (32) sorts before 'O' (79).
        let o_adv = header(hmtx + 4 * 2);
        let contours = i16::from_be_bytes([bytes[glyf], bytes[glyf + 1]]);
        (upem, o_adv, contours as usize)
    }
}
