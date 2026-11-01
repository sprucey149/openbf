//! Xbox (`_LVL_XBOX`) `tex_` chunk parsing + decode to RGBA8.
//!
//! Simpler than the PC's [`crate::texture_pc`]: one surface, every mip back to
//! back in one `BODY`. Layout, **[V]** verified on all 6,267 `tex_` of both
//! Xbox discs (the body length of every one walks to its last byte):
//!
//! ```text
//! tex_
//!   NAME  "gonk\0"
//!   INFO  20 B: u16 width, u16 height, u16 depth, u16 mipCount,
//!               u32 kind (1 plain, 2 cube, 3 volume),
//!               u32 Xbox D3DFORMAT, u32 bodyBytes
//!   BODY  face 0 mip 0, face 0 mip 1, ..., face 1 mip 0, ...
//! ```
//!
//! - Formats use the Xbox `D3DFORMAT` enum, not the PC's: see [`pixel_format`].
//! - Linear formats are stored **swizzled** (Morton order, each mip on its
//!   own), DXT blocks are not.
//! - Mip size follows [`PixelFormat::body_len`]'s truncating block count, as on
//!   the PC: a D3D `ceil` count overshoots the 16 non-square chains that reach
//!   an 8×2 mip (`sky_bespin1`, 512×128, by exactly one block).
//! - A volume (`attenuation_volume`, depth 32) keeps its first slice: the
//!   engine has no volume texture and nothing draws one.

use crate::texture_pc::{decode, PixelFormat};
use crate::{le, Chunk, UcfbError};

/// `INFO`'s length: the Xbox tell, next to the PS2's 14 B.
pub const INFO_LEN: usize = 20;

/// The surface kinds in `INFO`.
pub const KIND_PLAIN: u32 = 1;
pub const KIND_CUBE: u32 = 2;
pub const KIND_VOLUME: u32 = 3;

/// Refuse an image no shipped surface comes close to before decoding it.
const MAX_EDGE: u32 = 8192;

/// The A8 surfaces whose colour is black, not white. A8 drops the colour the
/// artist painted and the munger only keeps it when it is uniform, so the white
/// the Xbox reads it as is right for the other 159 same-size twins of a PS2
/// texture. These four twins are black on the PS2, and the black shadow blob
/// and map raster have to stay black.
const BLACK_A8: [&str; 4] = [
    "hud_map_raster",
    "opaque_black",
    "player_shadow",
    "tat3_prop_fan_alpha",
];

/// Decode shape of a format. The 0x0E (DXT2/DXT3) block is the DXT3 block, and
/// it is NOT premultiplied here: on 641 same-size PS2 twins with partial alpha,
/// 616 match the PS2's straight colour (`grid_64` is one of the 25 that do not).
/// A8 is coloured by the caller, because `texture_pc`'s A8 is D3D9's (0, 0, 0, a).
pub fn pixel_format(xbox_format: u32) -> Option<PixelFormat> {
    Some(match xbox_format {
        0x00 => PixelFormat::L8,
        0x02 => PixelFormat::A1R5G5B5,
        0x04 => PixelFormat::A4R4G4B4,
        0x05 => PixelFormat::R5G6B5,
        0x06 => PixelFormat::A8R8G8B8,
        0x07 => PixelFormat::X8R8G8B8,
        0x0C => PixelFormat::Dxt1,
        0x0E => PixelFormat::Dxt3,
        0x0F => PixelFormat::Dxt5,
        0x19 => PixelFormat::A8,
        0x1A => PixelFormat::A8L8,
        _ => return None,
    })
}

/// True when this `tex_` is in the Xbox layout: no PC `FMT_`, no PS2 `pal_`, a
/// 20-byte `INFO` and exactly one `BODY` as long as `INFO` says.
pub fn is_xbox(tex: &Chunk<'_>) -> bool {
    if tex.child(b"FMT_").is_some() || tex.child(b"pal_").is_some() {
        return false;
    }
    let Some(info) = tex.child(b"INFO").filter(|i| i.payload.len() == INFO_LEN) else {
        return false;
    };
    let mut bodies = tex.children_tagged(b"BODY");
    let (Some(body), None) = (bodies.next(), bodies.next()) else {
        return false;
    };
    le::u32_at(info.payload, 16) as usize == body.payload.len()
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub kind: u32,
    /// The Xbox `D3DFORMAT` from `INFO`.
    pub format: u32,
    /// Per face (one unless a cube), per mip: RGBA8 row-major.
    pub faces: Vec<Vec<Vec<u8>>>,
}

impl Texture {
    pub fn parse(tex: &Chunk<'_>) -> Result<Texture, UcfbError> {
        let bad = || UcfbError::Truncated(tex.offset);
        let name = tex.child_string(b"NAME").ok_or_else(bad)?;
        let info = tex.child(b"INFO").ok_or_else(bad)?.payload;
        if info.len() != INFO_LEN {
            return Err(bad());
        }
        let width = le::u16_at(info, 0) as u32;
        let height = le::u16_at(info, 2) as u32;
        let depth = le::u16_at(info, 4) as u32;
        let mip_count = le::u16_at(info, 6) as u32;
        let kind = le::u32_at(info, 8);
        let format = le::u32_at(info, 12);
        let body = tex.child(b"BODY").ok_or_else(bad)?.payload;
        if le::u32_at(info, 16) as usize != body.len() {
            return Err(bad());
        }
        let pixel = pixel_format(format).ok_or_else(|| {
            UcfbError::Unsupported(format!("xbox texture {name}: format {format:#x}"))
        })?;
        if width == 0
            || height == 0
            || width > MAX_EDGE
            || height > MAX_EDGE
            || mip_count == 0
            || mip_count > 16
        {
            return Err(UcfbError::Malformed(format!(
                "xbox texture {name}: {width}x{height}, {mip_count} mips"
            )));
        }
        let face_count = match kind {
            KIND_PLAIN | KIND_VOLUME => 1,
            KIND_CUBE => 6,
            other => {
                return Err(UcfbError::Unsupported(format!(
                    "xbox texture {name}: kind {other}"
                )))
            }
        };
        // A volume's mip 0 is `depth` slices; the rest is not read.
        let slices = if kind == KIND_VOLUME {
            depth.max(1) as usize
        } else {
            1
        };
        let mut at = 0usize;
        let mut faces = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            let mut mips = Vec::with_capacity(mip_count as usize);
            for level in 0..mip_count {
                let (w, h) = ((width >> level).max(1), (height >> level).max(1));
                let len = pixel.body_len(w, h);
                let span = if level == 0 { len * slices } else { len };
                let bytes = body.get(at..at + len).ok_or_else(bad)?;
                if kind == KIND_VOLUME && level > 0 {
                    break;
                }
                mips.push(decode_surface(pixel, w, h, bytes, &name)?);
                at += span;
            }
            faces.push(mips);
        }
        Ok(Texture {
            name,
            width,
            height,
            depth,
            kind,
            format,
            faces,
        })
    }
}

fn decode_surface(
    pixel: PixelFormat,
    w: u32,
    h: u32,
    bytes: &[u8],
    name: &str,
) -> Result<Vec<u8>, UcfbError> {
    let mut rgba = if pixel.is_block() {
        decode(pixel.d3d_id(), w, h, bytes)?
    } else {
        // The swizzle is only a permutation of a power-of-two surface.
        if !w.is_power_of_two() || !h.is_power_of_two() {
            return Err(UcfbError::Malformed(format!(
                "xbox texture {name}: a {w}x{h} swizzled mip"
            )));
        }
        let linear = unswizzle(bytes, w as usize, h as usize, pixel.bytes_per_texel());
        decode(pixel.d3d_id(), w, h, &linear)?
    };
    if pixel == PixelFormat::A8 && !BLACK_A8.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        for texel in rgba.as_chunks_mut::<4>().0 {
            texel[..3].fill(255);
        }
    }
    Ok(rgba)
}

/// Xbox swizzle to row-major: x and y bits alternate from the low end, x first,
/// and once the shorter edge runs out the longer one's remaining bits follow in
/// order. `bytes` is exactly `w * h * texel` long ([`PixelFormat::body_len`]).
pub fn unswizzle(bytes: &[u8], w: usize, h: usize, texel: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * texel];
    for y in 0..h {
        for x in 0..w {
            let from = morton(x, y, w, h) * texel;
            let to = (y * w + x) * texel;
            out[to..to + texel].copy_from_slice(&bytes[from..from + texel]);
        }
    }
    out
}

fn morton(x: usize, y: usize, w: usize, h: usize) -> usize {
    let (mut mask_x, mut mask_y) = (1usize, 1usize);
    let mut bit = 1usize;
    let mut out = 0usize;
    while mask_x < w || mask_y < h {
        if mask_x < w {
            if x & mask_x != 0 {
                out |= bit;
            }
            bit <<= 1;
            mask_x <<= 1;
        }
        if mask_y < h {
            if y & mask_y != 0 {
                out |= bit;
            }
            bit <<= 1;
            mask_y <<= 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut c = Vec::new();
        c.extend_from_slice(tag);
        c.extend((body.len() as u32).to_le_bytes());
        c.extend_from_slice(body);
        while c.len() % 4 != 0 {
            c.push(0);
        }
        c
    }

    fn tex(name: &str, w: u16, h: u16, mips: u16, kind: u32, fmt: u32, body: &[u8]) -> Vec<u8> {
        let mut info = Vec::new();
        for v in [w, h, 1, mips] {
            info.extend(v.to_le_bytes());
        }
        info.extend(kind.to_le_bytes());
        info.extend(fmt.to_le_bytes());
        info.extend((body.len() as u32).to_le_bytes());
        let mut name_z = name.as_bytes().to_vec();
        name_z.push(0);
        let mut p = build_chunk(b"NAME", &name_z);
        p.extend(build_chunk(b"INFO", &info));
        p.extend(build_chunk(b"BODY", body));
        build_chunk(b"tex_", &p)
    }

    fn first(buf: &[u8]) -> Chunk<'_> {
        crate::chunks(buf).next().unwrap().unwrap()
    }

    #[test]
    fn morton_order_alternates_then_runs_on() {
        // 4x4: the classic Z order.
        let order: Vec<usize> = (0..4)
            .flat_map(|y| (0..4).map(move |x| morton(x, y, 4, 4)))
            .collect();
        assert_eq!(
            order,
            [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15]
        );
        // 8x2: one alternating pair, then x's remaining bits in order.
        let row0: Vec<usize> = (0..8).map(|x| morton(x, 0, 8, 2)).collect();
        assert_eq!(row0, [0, 1, 4, 5, 8, 9, 12, 13]);
        assert_eq!(morton(0, 1, 8, 2), 2);
    }

    #[test]
    fn linear_texels_are_unswizzled_and_channels_ordered() {
        // A8R8G8B8 2x2, stored B,G,R,A in Morton order (0,0) (1,0) (0,1) (1,1).
        let mut body = Vec::new();
        for v in [10u8, 20, 30, 40] {
            body.extend([v, v + 1, v + 2, v + 3]);
        }
        let buf = tex("t", 2, 2, 1, KIND_PLAIN, 0x06, &body);
        let t = Texture::parse(&first(&buf)).unwrap();
        assert_eq!(
            t.faces[0][0],
            [12, 11, 10, 13, 22, 21, 20, 23, 32, 31, 30, 33, 42, 41, 40, 43]
        );
    }

    #[test]
    fn a8_reads_as_white_plus_alpha() {
        let buf = tex("t", 2, 1, 1, KIND_PLAIN, 0x19, &[7, 9]);
        let t = Texture::parse(&first(&buf)).unwrap();
        assert_eq!(t.faces[0][0], [255, 255, 255, 7, 255, 255, 255, 9]);
    }

    #[test]
    fn the_black_a8_surfaces_stay_black() {
        let buf = tex("player_shadow", 2, 1, 1, KIND_PLAIN, 0x19, &[7, 9]);
        let t = Texture::parse(&first(&buf)).unwrap();
        assert_eq!(t.faces[0][0], [0, 0, 0, 7, 0, 0, 0, 9]);
    }

    #[test]
    fn dxt1_chain_uses_the_truncating_block_count() {
        // 8x2 (one block) then 4x1 (one) then 2x1 (one) then 1x1 (one).
        let body = vec![0u8; 8 * 4];
        let buf = tex("t", 8, 2, 4, KIND_PLAIN, 0x0C, &body);
        let t = Texture::parse(&first(&buf)).unwrap();
        assert_eq!(t.faces[0].len(), 4);
        assert_eq!(t.faces[0][0].len(), 8 * 2 * 4);
        assert_eq!(t.faces[0][3].len(), 4);
    }

    #[test]
    fn cube_has_six_faces_and_volume_keeps_its_first_slice() {
        let buf = tex("c", 2, 2, 1, KIND_CUBE, 0x00, &[1u8; 24]);
        assert_eq!(Texture::parse(&first(&buf)).unwrap().faces.len(), 6);
        let mut vol = tex("v", 2, 2, 1, KIND_VOLUME, 0x00, &[1u8; 8]);
        // depth lives at INFO +4: 8 (tex_) + 8 + 4 (NAME "v\0" padded) + 8 + 4.
        let depth_at = 32;
        assert_eq!(&vol[depth_at..depth_at + 2], &[1, 0]);
        vol[depth_at] = 2;
        let t = Texture::parse(&first(&vol)).unwrap();
        assert_eq!(t.faces[0].len(), 1);
        assert_eq!(t.faces[0][0].len(), 2 * 2 * 4);
    }

    #[test]
    fn a_swizzled_surface_must_be_a_power_of_two() {
        let buf = tex("t", 3, 5, 1, KIND_PLAIN, 0x00, &[0u8; 15]);
        assert!(matches!(
            Texture::parse(&first(&buf)),
            Err(UcfbError::Malformed(_))
        ));
    }

    #[test]
    fn a_short_body_and_an_unknown_format_are_errors_not_panics() {
        let buf = tex("t", 4, 4, 1, KIND_PLAIN, 0x0C, &[0u8; 4]);
        assert!(matches!(
            Texture::parse(&first(&buf)),
            Err(UcfbError::Truncated(_))
        ));
        let buf = tex("t", 4, 4, 1, KIND_PLAIN, 0x77, &[0u8; 8]);
        assert!(matches!(
            Texture::parse(&first(&buf)),
            Err(UcfbError::Unsupported(_))
        ));
        let buf = tex("t", 0, 4, 1, KIND_PLAIN, 0x0C, &[0u8; 8]);
        assert!(matches!(
            Texture::parse(&first(&buf)),
            Err(UcfbError::Malformed(_))
        ));
    }

    /// Real data: every `tex_` on both discs decodes, and the layout test
    /// tells it apart from a PS2 one.
    #[test]
    fn every_real_xbox_texture_decodes() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        let mut lvls = Vec::new();
        for tree in ["xbox_bf1", "xbox_bf2"] {
            collect_lvls(std::path::Path::new(root).join(tree).as_path(), &mut lvls);
        }
        if lvls.is_empty() {
            eprintln!("SKIP: no extracted Xbox tree");
            return;
        }
        let mut seen = 0;
        for lvl in &lvls {
            let buf = std::fs::read(lvl).unwrap();
            let inner = crate::open(&buf).unwrap();
            for c in crate::content_chunks(inner).unwrap() {
                if &c.tag != b"tex_" {
                    continue;
                }
                assert!(is_xbox(&c), "{} @{:#x}", lvl.display(), c.offset);
                let t = Texture::parse(&c)
                    .unwrap_or_else(|e| panic!("{} @{:#x}: {e}", lvl.display(), c.offset));
                assert_eq!(
                    t.faces[0][0].len(),
                    (t.width * t.height * 4) as usize,
                    "{}",
                    t.name
                );
                seen += 1;
            }
        }
        assert!(seen > 4000, "{seen} textures");
    }

    fn collect_lvls(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_lvls(&p, out);
            } else if p.extension().is_some_and(|x| x == "lvl") {
                out.push(p);
            }
        }
    }
}
