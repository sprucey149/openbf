//! `tex_` chunk parsing + decode to RGBA8 (audit A2 §4).
//!
//! INFO 14 B: u16 w, u16 h, u16 format (4|8 bits per palette index),
//! u16 detailCompressed, f32 mipDistance, u16 mipCount.
//! pal_ = INFO (u16 colorCount, u16 bitsPerColor) + RLE BODY (elem 4 = RGBA).
//! One RLE BODY per mip (elem 1), 4-bit indices packed LOW nibble first (the
//! GS PSMT4 order: the left texel of each pair is the low nibble —
//! analysis/re-bolt.md §7; card NIBL).
//! 32-bpp textures (BF1 COMMON.LVL effects: lightning/boom/wing blur) carry
//! no `pal_`; each mip BODY is RLE with 4-byte elements = direct [A,B,G,R]
//! texels (same byte order as palette entries).
//!
//! PS2-isms applied here (empirically validated on TAT1 textures, see
//! RE_LOG.md "LVL textures"):
//! - Palette colors are stored [A, B, G, R] (u32 0xRRGGBBAA little-endian);
//!   alpha is FULL-RANGE 0..255 (browndust/tat1_dirt_blow_in fade in byte 0
//!   with constant RGB — not the GS 0..0x80 convention).
//! - CLUTs are stored LINEAR at every size — no CSM1 unswizzle. (Erratum
//!   2026-08-30: the bake applied a CSM1 unswizzle to 256-color CLUTs until
//!   both decodes were compared side by side; CSM1 speckles every 8-bit
//!   texture. See RE_LOG.md "ERRATUM — 256-color CLUTs are LINEAR".)

use crate::{halve, le, rle, Chunk, UcfbError};

#[derive(Debug)]
pub struct TextureInfo {
    pub width: u32,
    pub height: u32,
    /// Bits per texel: 4 or 8 (palette index) or 32 (direct color).
    pub bpp: u32,
    pub detail_compressed: bool,
    pub mip_distance: f32,
    pub mip_count: u32,
}

pub struct Texture {
    pub name: String,
    pub info: TextureInfo,
    /// RGBA8, alpha already scaled to 0..255.
    pub palette: Vec<[u8; 4]>,
    /// Decoded palette indices per mip (one byte per texel, 4-bit unpacked);
    /// for 32-bpp textures: RGBA8 texels (4 bytes per texel).
    pub mips: Vec<Vec<u8>>,
}

impl Texture {
    /// Parse from a `tex_` chunk.
    pub fn parse(tex: &Chunk<'_>) -> Result<Texture, UcfbError> {
        let bad = || UcfbError::Truncated(tex.offset);
        if tex.child(b"FMT_").is_some() {
            let pc = crate::texture_pc::Texture::parse(tex)?;
            let surface = pc.best().or_else(|| pc.surfaces.first()).ok_or_else(bad)?;
            let face = surface.faces.first().ok_or_else(bad)?;
            let mips: Vec<Vec<u8>> = face.mips.iter().map(|m| m.rgba.clone()).collect();
            let info = TextureInfo {
                width: surface.width,
                height: surface.height,
                bpp: 32,
                detail_compressed: false,
                mip_distance: 0.0,
                mip_count: mips.len() as u32,
            };
            return Ok(Texture {
                name: pc.name,
                info,
                palette: Vec::new(),
                mips,
            });
        }
        let name = tex.child_string(b"NAME").ok_or_else(bad)?;
        let info = tex.child(b"INFO").ok_or_else(bad)?;
        let ip = info.payload;
        if ip.len() < 14 {
            return Err(bad());
        }
        let u16le = |o: usize| le::u16_at(ip, o) as u32;
        let info = TextureInfo {
            width: u16le(0),
            height: u16le(2),
            bpp: u16le(4),
            detail_compressed: u16le(6) != 0,
            mip_distance: f32::from_le_bytes(ip[8..12].try_into().unwrap()),
            mip_count: u16le(12),
        };

        if !matches!(info.bpp, 4 | 8 | 32) {
            return Err(UcfbError::Unsupported(format!(
                "texture {name}: {} bpp",
                info.bpp
            )));
        }
        let mut palette: Vec<[u8; 4]> = Vec::new();
        if info.bpp != 32 {
            let pal = tex.child(b"pal_").ok_or_else(bad)?;
            let pal_info = pal.child(b"INFO").ok_or_else(bad)?.payload;
            if pal_info.len() < 2 {
                return Err(bad());
            }
            let color_count = u16::from_le_bytes([pal_info[0], pal_info[1]]) as usize;
            let raw = rle::decode(pal.child(b"BODY").ok_or_else(bad)?.payload, 4)?;
            if raw.len() != color_count * 4 {
                return Err(bad());
            }
            // Stored [A, B, G, R] -> emit [R, G, B, A]; alpha is full-range.
            palette = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| [c[3], c[2], c[1], c[0]])
                .collect();
        }

        let mut mips = Vec::new();
        let (mut w, mut h) = (info.width as usize, info.height as usize);
        for body in tex.children_tagged(b"BODY") {
            if info.bpp == 32 {
                let data = rle::decode(body.payload, 4)?;
                if data.len() != w * h * 4 {
                    return Err(bad());
                }
                // Direct [A, B, G, R] texels -> RGBA8.
                let rgba: Vec<u8> = data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|c| [c[3], c[2], c[1], c[0]])
                    .collect();
                mips.push(rgba);
            } else {
                let data = rle::decode(body.payload, 1)?;
                if data.len() != w * h * info.bpp as usize / 8 {
                    return Err(bad());
                }
                mips.push(match info.bpp {
                    4 => unpack_4bit(&data),
                    _ => data,
                });
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        // No mip is no picture: `rgba8(0)` is what every consumer asks.
        if mips.is_empty() || mips.len() != info.mip_count as usize {
            return Err(bad());
        }
        Ok(Texture {
            name,
            info,
            palette,
            mips,
        })
    }

    /// Decode one mip to RGBA8 (row-major).
    pub fn rgba8(&self, mip: usize) -> Vec<u8> {
        if self.info.bpp == 32 {
            return self.mips[mip].clone();
        }
        let mut out = Vec::with_capacity(self.mips[mip].len() * 4);
        for &i in &self.mips[mip] {
            let c = self
                .palette
                .get(i as usize)
                .copied()
                .unwrap_or([255, 0, 255, 255]);
            out.extend_from_slice(&c);
        }
        out
    }

    pub fn mip_dims(&self, mip: usize) -> (u32, u32) {
        (halve(self.info.width, mip), halve(self.info.height, mip))
    }
}

/// 4-bit indices, low nibble first: the GS PSMT4 order, left texel in the
/// low nibble (analysis/re-bolt.md §7). Until NIBL the port took the high
/// nibble first (A2 §4's reading of unmunge promote_4bit_refs), which
/// swapped every horizontal texel pair.
fn unpack_4bit(packed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packed.len() * 2);
    for &b in packed {
        out.push(b & 0x0F);
        out.push(b >> 4);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_order_low_first() {
        assert_eq!(unpack_4bit(&[0xAB, 0x01]), vec![0xB, 0xA, 0x1, 0x0]);
    }

    /// Data-gated: BF1 COMMON.LVL's 32-bpp effect textures decode (no pal_).
    #[test]
    fn common_lvl_32bpp_textures_decode() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: COMMON.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let mut seen = 0;
        for c in crate::content_chunks(inner).unwrap() {
            if &c.tag != b"tex_" {
                continue;
            }
            let t = Texture::parse(&c).unwrap_or_else(|e| panic!("tex_ @0x{:X}: {e}", c.offset));
            if t.info.bpp == 32 {
                seen += 1;
                let rgba = t.rgba8(0);
                assert_eq!(
                    rgba.len(),
                    (t.info.width * t.info.height * 4) as usize,
                    "{}",
                    t.name
                );
                assert_eq!(t.mips.len(), t.info.mip_count as usize, "{}", t.name);
                // An effect sprite: transparent somewhere, opaque somewhere.
                let alphas: Vec<u8> = rgba.as_chunks::<4>().0.iter().map(|c| c[3]).collect();
                assert!(
                    alphas.iter().any(|&a| a < 8) && alphas.iter().any(|&a| a > 200),
                    "{}",
                    t.name
                );
            }
        }
        assert_eq!(seen, 5, "lightning1/2, boom4/5, geo_fly_geowingsblur");
    }

    /// Data-gated (NIBL): with the PS2's low-nibble-first order the 4-bit
    /// textures on BF2 INGAME.LVL decode locally smooth; swapping each
    /// horizontal texel pair (the old high-first order) roughens them.
    /// Roughness = mean |RGBA step| between horizontal neighbours of mip 0.
    #[test]
    fn ingame_4bit_textures_decode_smooth() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/INGAME.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: INGAME.LVL not present");
            return;
        };
        let rough = |rgba: &[u8], w: usize, swap: bool| -> f64 {
            let at = |x: usize, y: usize, k: usize| {
                let x = if swap { x ^ 1 } else { x };
                rgba[(y * w + x) * 4 + k] as f64
            };
            let h = rgba.len() / 4 / w;
            let mut sum = 0.0;
            for y in 0..h {
                for x in 1..w {
                    sum += (0..4)
                        .map(|k| (at(x, y, k) - at(x - 1, y, k)).abs())
                        .sum::<f64>();
                }
            }
            sum / ((w - 1) * h) as f64
        };
        let inner = crate::open(&buf).unwrap();
        let (mut n, mut smoother_swapped, mut flag) = (0, 0, false);
        for c in crate::content_chunks(inner).unwrap() {
            if &c.tag != b"tex_" {
                continue;
            }
            let Ok(t) = Texture::parse(&c) else { continue };
            let w = t.info.width as usize;
            if t.info.bpp != 4 || w < 8 {
                continue;
            }
            let rgba = t.rgba8(0);
            let (r0, r1) = (rough(&rgba, w, false), rough(&rgba, w, true));
            n += 1;
            if r1 < 0.9 * r0 {
                smoother_swapped += 1;
            }
            if t.name == "hud_target_flag_onscreen" {
                flag = true;
                assert!(r0 < 0.8 * r1, "{}: r0 {r0:.2} r1 {r1:.2}", t.name);
            }
        }
        eprintln!("NIBL: {smoother_swapped} of {n} 4-bit textures smoother when swapped");
        assert!(flag, "hud_target_flag_onscreen not found");
        assert!(n >= 50, "only {n} 4-bit textures");
        assert!(
            smoother_swapped * 20 <= n,
            "{smoother_swapped} of {n} smoother when swapped"
        );
    }

    /// `pal_ INFO` is file data: a payload shorter than the u16 colour count
    /// used to be indexed unchecked (same guard as the `ip.len() < 14` above).
    #[test]
    fn short_pal_info_errors() {
        use crate::build_chunk;
        let mut info = Vec::new();
        info.extend(4u16.to_le_bytes()); // width
        info.extend(4u16.to_le_bytes()); // height
        info.extend(8u16.to_le_bytes()); // bpp -> needs a pal_
        info.extend(0u16.to_le_bytes()); // detailCompressed
        info.extend(0f32.to_le_bytes()); // mipDistance
        info.extend(1u16.to_le_bytes()); // mipCount
        for pal_info in [&[][..], &[1][..]] {
            let mut pal = build_chunk(b"INFO", pal_info);
            pal.extend(build_chunk(b"BODY", &[]));
            let mut body = build_chunk(b"NAME", b"t\0");
            body.extend(build_chunk(b"INFO", &info));
            body.extend(build_chunk(b"pal_", &pal));
            let buf = build_chunk(b"tex_", &body);
            let c = crate::chunks(&buf).next().unwrap().unwrap();
            let Err(e) = Texture::parse(&c) else {
                panic!("expected an error")
            };
            assert!(matches!(e, UcfbError::Truncated(_)), "{e}");
        }
    }

    /// `width >> mip` panics in debug for mip >= 32; mip_dims must clamp like
    /// `texture_pc::halve` does.
    #[test]
    fn mip_dims_past_the_shift_width_clamps() {
        let t = Texture {
            name: "t".into(),
            info: TextureInfo {
                width: 64,
                height: 32,
                bpp: 8,
                detail_compressed: false,
                mip_distance: 0.0,
                mip_count: 1,
            },
            palette: Vec::new(),
            mips: Vec::new(),
        };
        assert_eq!(t.mip_dims(0), (64, 32));
        assert_eq!(t.mip_dims(3), (8, 4));
        assert_eq!(t.mip_dims(31), (1, 1));
        assert_eq!(t.mip_dims(32), (1, 1));
        assert_eq!(t.mip_dims(usize::MAX), (1, 1));
    }
    #[test]
    fn pc_texture_parses_via_texture_parse() {
        use crate::build_chunk as chunk;
        let name = chunk(b"NAME", b"pc_test\0");
        let info = chunk(b"INFO", &[1, 0, 0, 0, 21, 0, 0, 0]);
        let mut fmt_info = vec![0u8; 16];
        fmt_info[0..4].copy_from_slice(&21u32.to_le_bytes());
        fmt_info[4..6].copy_from_slice(&1u16.to_le_bytes());
        fmt_info[6..8].copy_from_slice(&1u16.to_le_bytes());
        fmt_info[8..10].copy_from_slice(&1u16.to_le_bytes());
        fmt_info[10..12].copy_from_slice(&1u16.to_le_bytes());
        fmt_info[12..16].copy_from_slice(&1u32.to_le_bytes());
        let mut lvl_info = vec![0u8; 8];
        lvl_info[0..4].copy_from_slice(&0u32.to_le_bytes());
        lvl_info[4..8].copy_from_slice(&4u32.to_le_bytes());
        let body = chunk(b"BODY", &[0, 255, 0, 255]);
        let mut lvl = chunk(b"INFO", &lvl_info);
        lvl.extend(body);
        let lvl = chunk(b"LVL_", &lvl);
        let face = chunk(b"FACE", &lvl);
        let mut fmt = chunk(b"INFO", &fmt_info);
        fmt.extend(face);
        let fmt = chunk(b"FMT_", &fmt);
        let mut tex = name;
        tex.extend(info);
        tex.extend(fmt);
        let bytes = chunk(b"tex_", &tex);
        let c = crate::chunks(&bytes).next().unwrap().unwrap();
        let t = Texture::parse(&c).unwrap();
        assert_eq!(t.name, "pc_test");
        assert_eq!(t.info.width, 1);
        assert_eq!(t.info.height, 1);
        assert_eq!(t.info.bpp, 32);
        assert_eq!(t.rgba8(0), vec![0, 255, 0, 255]);
    }
}
