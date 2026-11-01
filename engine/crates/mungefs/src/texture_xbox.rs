//! Xbox `tex_` chunks, decoded for the original Xbox button icons
//! (`bake::xbox_icons`): only the formats the icons use.
//!
//! Layout, read off BF2's Xbox `common.lvl` `[M]` (the decoded pictures are
//! the buttons their names say): `NAME`, `INFO` 20 B, `BODY`.
//! INFO: u16 w, u16 h, u16 1, u16 1, u32 1, u32 format, u32 body bytes.
//!
//! Formats are the Xbox D3D ids: `0x0C` DXT1, `0x0E` DXT2/3, `0x0F` DXT4/5,
//! stored in block order, and `0x1A` A8L8, stored swizzled (Morton order) as
//! luminance then alpha. Anything else is refused.

use crate::texture::{Texture, TextureInfo};
use crate::texture_pc::{decode, PixelFormat};
use crate::{Chunk, UcfbError};

const INFO_LEN: usize = 20;
const XBOX_DXT1: u32 = 0x0c;
const XBOX_DXT3: u32 = 0x0e;
const XBOX_DXT5: u32 = 0x0f;
const XBOX_A8L8_SWIZZLED: u32 = 0x1a;

/// Whether a `tex_` has the Xbox shape: a console PS2 `INFO` is 14 bytes and
/// a PC texture has `FMT_` chunks instead of a `BODY`.
pub fn is_xbox(tex: &Chunk<'_>) -> bool {
    tex.child(b"FMT_").is_none()
        && tex
            .child(b"INFO")
            .is_some_and(|info| info.payload.len() == INFO_LEN)
}

/// Parse an Xbox `tex_` into a 32-bpp [`Texture`] of its one stored mip.
pub fn parse(tex: &Chunk<'_>) -> Result<Texture, UcfbError> {
    let bad = || UcfbError::Truncated(tex.offset);
    let name = tex.child_string(b"NAME").ok_or_else(bad)?;
    if !is_xbox(tex) {
        return Err(UcfbError::Unsupported(format!(
            "texture {name}: not an Xbox texture"
        )));
    }
    let ip = tex.child(b"INFO").ok_or_else(bad)?.payload;
    let u16le = |o: usize| u32::from(u16::from_le_bytes([ip[o], ip[o + 1]]));
    let (width, height) = (u16le(0), u16le(2));
    if width == 0 || height == 0 {
        return Err(UcfbError::Unsupported(format!(
            "texture {name}: {width}x{height}"
        )));
    }
    let format = u32::from_le_bytes([ip[12], ip[13], ip[14], ip[15]]);
    let body = tex.child(b"BODY").ok_or_else(bad)?.payload;
    let dxt = |f: PixelFormat| decode(f.d3d_id(), width, height, body);
    let rgba = match format {
        XBOX_DXT1 => dxt(PixelFormat::Dxt1)?,
        XBOX_DXT3 => dxt(PixelFormat::Dxt3)?,
        XBOX_DXT5 => dxt(PixelFormat::Dxt5)?,
        XBOX_A8L8_SWIZZLED => a8l8_swizzled(width, height, body).ok_or_else(bad)?,
        other => {
            return Err(UcfbError::Unsupported(format!(
                "texture {name}: Xbox format 0x{other:02x}"
            )))
        }
    };
    Ok(Texture {
        name,
        info: TextureInfo {
            width,
            height,
            bpp: 32,
            detail_compressed: false,
            mip_distance: 0.0,
            mip_count: 1,
        },
        palette: Vec::new(),
        mips: vec![rgba],
    })
}

/// A texel's place in a square power-of-two Morton-ordered surface.
fn morton(x: u32, y: u32) -> usize {
    let spread = |v: u32| {
        let mut out = 0usize;
        for bit in 0..16 {
            out |= (((v >> bit) & 1) as usize) << (2 * bit);
        }
        out
    };
    spread(x) | (spread(y) << 1)
}

fn a8l8_swizzled(width: u32, height: u32, body: &[u8]) -> Option<Vec<u8>> {
    // Only a square surface is plain Morton order; the icons are all square.
    if width != height || !width.is_power_of_two() {
        return None;
    }
    let texels = width as usize * height as usize;
    if body.len() < texels * 2 {
        return None;
    }
    let mut out = Vec::with_capacity(texels * 4);
    for y in 0..height {
        for x in 0..width {
            let at = morton(x, y) * 2;
            let (l, a) = (body[at], body[at + 1]);
            out.extend_from_slice(&[l, l, l, a]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `tex_` chunk's bytes: its children, each padded to 4.
    fn chunk_bytes(children: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (tag, payload) in children {
            out.extend_from_slice(*tag);
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
            out.resize(out.len().next_multiple_of(4), 0);
        }
        out
    }

    fn info(w: u16, h: u16, format: u32, body: u32) -> Vec<u8> {
        let mut i = Vec::new();
        i.extend_from_slice(&w.to_le_bytes());
        i.extend_from_slice(&h.to_le_bytes());
        i.extend_from_slice(&1u16.to_le_bytes());
        i.extend_from_slice(&1u16.to_le_bytes());
        i.extend_from_slice(&1u32.to_le_bytes());
        i.extend_from_slice(&format.to_le_bytes());
        i.extend_from_slice(&body.to_le_bytes());
        i
    }

    fn parsed(children: Vec<(&[u8; 4], Vec<u8>)>) -> Result<Texture, UcfbError> {
        let payload = chunk_bytes(&children);
        let tex = Chunk {
            tag: *b"tex_",
            offset: 0,
            payload: &payload,
        };
        parse(&tex)
    }

    /// Texels come back from Morton order: in a 2 x 2 the stored order is
    /// (0,0) (1,0) (0,1) (1,1).
    #[test]
    fn a8l8_is_unswizzled_luminance_then_alpha() {
        let body = vec![10, 110, 20, 120, 30, 130, 40, 140];
        let t = parsed(vec![
            (b"NAME", b"t\0".to_vec()),
            (b"INFO", info(2, 2, XBOX_A8L8_SWIZZLED, 8)),
            (b"BODY", body),
        ])
        .unwrap();
        assert_eq!(
            t.rgba8(0),
            [10, 10, 10, 110, 20, 20, 20, 120, 30, 30, 30, 130, 40, 40, 40, 140]
        );
    }

    #[test]
    fn morton_goes_past_the_first_square() {
        assert_eq!(morton(3, 0), 5);
        assert_eq!(morton(0, 3), 10);
        assert_eq!(morton(3, 3), 15);
    }

    #[test]
    fn an_unknown_format_or_short_body_is_refused() {
        let unknown = parsed(vec![
            (b"NAME", b"t\0".to_vec()),
            (b"INFO", info(4, 4, 0x77, 8)),
            (b"BODY", vec![0; 8]),
        ]);
        assert!(matches!(unknown, Err(UcfbError::Unsupported(_))));
        let short = parsed(vec![
            (b"NAME", b"t\0".to_vec()),
            (b"INFO", info(2, 2, XBOX_A8L8_SWIZZLED, 8)),
            (b"BODY", vec![0; 4]),
        ]);
        assert!(matches!(short, Err(UcfbError::Truncated(_))));
    }

    /// A surface with no texels is no icon: refused, not written out empty,
    /// whichever side is zero.
    #[test]
    fn a_zero_sized_surface_is_refused() {
        for (w, h, format) in [
            (0, 0, XBOX_DXT1),
            (0, 4, XBOX_DXT1),
            (4, 0, XBOX_DXT5),
            (0, 0, XBOX_A8L8_SWIZZLED),
        ] {
            let got = parsed(vec![
                (b"NAME", b"t\0".to_vec()),
                (b"INFO", info(w, h, format, 16)),
                (b"BODY", vec![0; 16]),
            ]);
            assert!(
                matches!(got, Err(UcfbError::Unsupported(_))),
                "{w}x{h} format {format:#x}"
            );
        }
    }

    /// A PS2 `INFO` is 14 bytes: never read as Xbox.
    #[test]
    fn a_ps2_texture_is_not_xbox() {
        let refused = parsed(vec![
            (b"NAME", b"t\0".to_vec()),
            (b"INFO", vec![0; 14]),
            (b"BODY", vec![0; 8]),
        ]);
        assert!(matches!(refused, Err(UcfbError::Unsupported(_))));
    }
}
