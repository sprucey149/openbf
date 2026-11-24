//! PC (`_LVL_PC`) `tex_` chunk parsing + decode to RGBA8.
//!
//! The PC games store textures as raw Direct3D 9 surfaces, not the PS2
//! RLE/palette scheme in [`crate::texture`] (audit A2 §5). One `tex_` holds
//! the *same image* re-encoded in several D3DFORMATs (the munger emits a
//! compressed form plus uncompressed fallbacks for weak cards), so the
//! parser keeps every one and [`Texture::best`] picks the highest-fidelity
//! surface. This is the reader the classic-plus overlay uses to pull HD
//! textures out of the PC remaster LVLs and name-match them onto baked PS2
//! maps.
//!
//! Layout — **[V]** verified byte-for-byte on local PC LVLs, **[X]**
//! cross-confirmed against LibSWBF2 `src/Chunks/LVL/tex_/`, **[H]** hypothesis:
//!
//! ```text
//! tex_                                                        [V][X]
//!   NAME  "gonk\0"                       NUL-terminated, padded to 4
//!   INFO  u32 formatCount, u32 d3dFormat[formatCount]         [V]
//!   FMT_  (one per declared format, same order as INFO)       [V]
//!     INFO 16 B: u32 d3dFormat, u16 width, u16 height,
//!                u16 depth, u16 mipCount, u32 typeFlags       [V]
//!     FACE  (1 face for a 2D texture, 6 for a cube map)       [V]
//!       LVL_  (one per mip, low index = largest)              [V]
//!         INFO 8 B: u32 mipLevel, u32 bodySize                [V]
//!         BODY  raw D3D surface bytes, no compression         [V]
//! ```
//!
//! Verified on `BS1/data/_LVL_PC/ingame.lvl` (Harrisonfog BS1 addon), whose
//! single `tex_` "gonk" starts at file offset 0x8:
//! `NAME`@0x10 (5 B "gonk\0"), `INFO`@0x20 (12 B = count 2, `DXT1`
//! 0x31545844, `R5G6B5` 23), `FMT_`@0x34 with `INFO`@0x3C =
//! `44 58 54 31 | 00 01 | 00 01 | 01 00 | 09 00 | 01 00 00 00` (DXT1,
//! 256×256×1, 9 mips, typeFlags 1), `FACE`@0x54, `LVL_`@0x5C,
//! `INFO`@0x64 = mipLevel 0 / bodySize 0x8000, `BODY`@0x78 with 32768 B =
//! 64×64 DXT1 blocks. The second `FMT_` (R5G6B5) starts at 0xAC34.
//!
//! Sample used for every count below: 244 `tex_` / 351 `FMT_` / 3 060 mip
//! bodies from six local PC LVLs — BS1 `ingame.lvl`, `SIDE/BES.lvl`,
//! `BS1/BS1.lvl`, HF5 `common.lvl`, `inshell.lvl`, and the Mos Eisley
//! remake `tat2.lvl`.
//!
//! - `typeFlags` **[V]**: 1 = plain 2D (one FACE) on 349 surfaces, 2 = cube
//!   map (six FACEs) on 2 — both are `mos_envmap` in `tat2.lvl` @0xF835100,
//!   the only cube map in the whole sample. A volume-texture flag presumably
//!   exists but `depth` was 1 on all 351 surfaces, so volumes are **[H]** and
//!   are reported through [`Texture::unsupported`] rather than guessed at.
//! - **Mip body size [V]**: block formats store
//!   `max(1, (w / 4) * (h / 4))` blocks with *truncating* division — not the
//!   `ceil` count Direct3D would use. The two disagree only when exactly one
//!   dimension is below 4 and the other is 8 or more, e.g. the 8×2 mip 6 of
//!   `tat_transit` (tat2.lvl) is 16 B = one DXT3 block where `ceil` wants
//!   two. All 3 060 bodies match the truncating rule; three (all in
//!   `tat2.lvl`) contradict `ceil`. Short blocks are therefore decoded as far
//!   as the data goes and the remaining texels stay zero.
//! - Every `FACE` carries exactly `mipCount` `LVL_` chunks, and `bodySize`
//!   always equals the `BODY` payload length **[V]** (3 060/3 060).
//! - `A8` maps to `(0, 0, 0, alpha)` per D3D9 semantics **[H]** — no A8
//!   texture turned up locally, so that choice is unverified.
//!
//! Picking a format **[V]**: the munger writes its preferred encoding first
//! ([`Texture::preferred`]), but that is not always the most faithful one, so
//! [`Texture::best`] uses a ranking measured against the seven local textures
//! that ship an `A8R8G8B8` copy alongside lossy ones (BES.lvl `cis_maf`,
//! tat2.lvl `tat2_bldg_crashed_spaces` ×3, `tat2_bldg_tent`, `tat_skiff01`,
//! `tat_barge`): versus that ground truth, DXT3 scores 3.0–4.8 RGB RMSE and
//! its `A4R4G4B4` sibling 8.0–8.2, and DXT3 wins on alpha too — block
//! compression is *not* the worse copy here. Re-quantising the same
//! `A8R8G8B8` images by hand gives R5G6B5 2.9 RMSE and A4R4G4B4 6.8, which
//! places plain R5G6B5 just above DXT1 and leaves A4R4G4B4 last. `L8` and
//! `A8L8` outrank everything but true-colour because they only ever appear
//! first, i.e. on a genuinely greyscale source, where they are lossless.

use crate::{halve, le, Chunk, UcfbError};

/// Pixel formats this module can decode, identified by their `D3DFORMAT` id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 21 — 32 bpp, stored B,G,R,A.
    A8R8G8B8,
    /// 22 — 32 bpp, stored B,G,R,x; alpha forced opaque.
    X8R8G8B8,
    /// 23 — 16 bpp 5:6:5.
    R5G6B5,
    /// 25 — 16 bpp 1:5:5:5.
    A1R5G5B5,
    /// 26 — 16 bpp 4:4:4:4.
    A4R4G4B4,
    /// 28 — 8 bpp alpha only.
    A8,
    /// 50 — 8 bpp luminance.
    L8,
    /// 51 — 16 bpp alpha + luminance.
    A8L8,
    /// 60 — 16 bpp, two signed bump-map components, U then V.
    V8U8,
    /// `DXT1` fourcc — BC1, 8 B / 4×4 block, 1-bit alpha.
    Dxt1,
    /// `DXT3` fourcc — BC2, 16 B / block, explicit 4-bit alpha.
    Dxt3,
    /// `DXT5` fourcc — BC3, 16 B / block, interpolated alpha.
    Dxt5,
}

use PixelFormat::*;

impl PixelFormat {
    /// Map a raw `D3DFORMAT` id (or fourcc) to a decodable format.
    pub fn from_d3d(id: u32) -> Option<PixelFormat> {
        Some(match id {
            21 => A8R8G8B8,
            22 => X8R8G8B8,
            23 => R5G6B5,
            25 => A1R5G5B5,
            26 => A4R4G4B4,
            28 => A8,
            50 => L8,
            51 => A8L8,
            60 => V8U8,
            0x3154_5844 => Dxt1,
            0x3354_5844 => Dxt3,
            0x3554_5844 => Dxt5,
            _ => return None,
        })
    }

    /// The `D3DFORMAT` id this format is stored under.
    pub fn d3d_id(self) -> u32 {
        match self {
            A8R8G8B8 => 21,
            X8R8G8B8 => 22,
            R5G6B5 => 23,
            A1R5G5B5 => 25,
            A4R4G4B4 => 26,
            A8 => 28,
            L8 => 50,
            A8L8 => 51,
            V8U8 => 60,
            Dxt1 => 0x3154_5844,
            Dxt3 => 0x3354_5844,
            Dxt5 => 0x3554_5844,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            A8R8G8B8 => "A8R8G8B8",
            X8R8G8B8 => "X8R8G8B8",
            R5G6B5 => "R5G6B5",
            A1R5G5B5 => "A1R5G5B5",
            A4R4G4B4 => "A4R4G4B4",
            A8 => "A8",
            L8 => "L8",
            A8L8 => "A8L8",
            V8U8 => "V8U8",
            Dxt1 => "DXT1",
            Dxt3 => "DXT3",
            Dxt5 => "DXT5",
        }
    }

    /// Bytes per 4×4 block for block-compressed formats, else 0.
    pub fn block_bytes(self) -> usize {
        match self {
            Dxt1 => 8,
            Dxt3 | Dxt5 => 16,
            _ => 0,
        }
    }

    /// Bytes per texel for linear formats, else 0.
    pub fn bytes_per_texel(self) -> usize {
        match self {
            A8R8G8B8 | X8R8G8B8 => 4,
            R5G6B5 | A1R5G5B5 | A4R4G4B4 | A8L8 | V8U8 => 2,
            A8 | L8 => 1,
            _ => 0,
        }
    }

    pub fn is_block(self) -> bool {
        self.block_bytes() != 0
    }

    /// Bytes the munger writes for a `w`×`h` surface (module header:
    /// truncating block count, not `ceil`).
    pub fn body_len(self, w: u32, h: u32) -> usize {
        if self.is_block() {
            let blocks = ((w as usize / 4) * (h as usize / 4)).max(1);
            blocks * self.block_bytes()
        } else {
            w as usize * h as usize * self.bytes_per_texel()
        }
    }

    /// Higher is closer to the artist's source image; [`Texture::best`]
    /// maximises this. Ordering is measured, not assumed — see the
    /// "picking a format" note in the module header.
    fn fidelity(self) -> u8 {
        match self {
            A8R8G8B8 => 100,
            X8R8G8B8 => 96,
            A8L8 => 92,
            V8U8 => 92,
            L8 => 88,
            Dxt5 => 80,
            Dxt3 => 76,
            R5G6B5 => 72,
            A1R5G5B5 => 70,
            Dxt1 => 68,
            A4R4G4B4 => 40,
            A8 => 30,
        }
    }
}

/// One decoded mip level: RGBA8, row-major, `width * height * 4` bytes.
#[derive(Debug, Clone)]
pub struct Mip {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One `FACE`: a full mip chain. Cube maps have six, in `FACE` order.
#[derive(Debug, Clone)]
pub struct Face {
    pub mips: Vec<Mip>,
}

/// One decoded `FMT_`: the whole texture in a single D3D format.
#[derive(Debug, Clone)]
pub struct Surface {
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u32,
    pub type_flags: u32,
    pub faces: Vec<Face>,
}

impl Surface {
    /// Cube map (six faces / `typeFlags` bit 1).
    pub fn is_cube(&self) -> bool {
        self.type_flags & 0x2 != 0 || self.faces.len() == 6
    }

    /// Mip 0 of face 0 — what an HD override normally wants.
    pub fn mip0(&self) -> Option<&Mip> {
        self.faces.first()?.mips.first()
    }

    /// Dimensions of mip `level`, floor-halved and clamped to 1. Levels past
    /// the end of the chain saturate at 1x1 rather than overflowing.
    pub fn mip_dims(&self, level: usize) -> (u32, u32) {
        (halve(self.width, level), halve(self.height, level))
    }
}

/// A parsed PC `tex_` chunk.
#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    /// Raw `D3DFORMAT` ids from the `tex_ INFO` list, in munger order.
    pub declared: Vec<u32>,
    /// Every `FMT_` that decoded, in file order.
    pub surfaces: Vec<Surface>,
    /// `D3DFORMAT` ids of `FMT_` chunks skipped because this module has no
    /// decoder for them, or because they are volume textures (`depth > 1`).
    pub unsupported: Vec<u32>,
}

impl Texture {
    /// Parse a PC `tex_` chunk and decode every supported `FMT_` to RGBA8.
    ///
    /// Returns [`UcfbError::Unsupported`] only when *nothing* decoded and at
    /// least one `FMT_` was skipped; a texture with a usable format always
    /// parses, with the exotic fallbacks listed in [`Texture::unsupported`].
    pub fn parse(tex: &Chunk<'_>) -> Result<Texture, UcfbError> {
        let bad = || UcfbError::Truncated(tex.offset);
        let name = tex.child_string(b"NAME").ok_or_else(bad)?;
        let info = tex.child(b"INFO").ok_or_else(bad)?;
        let ip = info.payload;
        if ip.len() < 4 {
            return Err(bad());
        }
        let count = u32::from_le_bytes(ip[0..4].try_into().unwrap()) as usize;
        if ip.len() < 4 + count * 4 {
            return Err(bad());
        }
        let declared: Vec<u32> = ip[4..4 + count * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();

        let mut surfaces = Vec::new();
        let mut unsupported = Vec::new();
        for fmt in tex.children_tagged(b"FMT_") {
            let fi = fmt.child(b"INFO").ok_or_else(bad)?.payload;
            if fi.len() < 16 {
                return Err(bad());
            }
            let u16le = |o: usize| le::u16_at(fi, o) as u32;
            let id = u32::from_le_bytes(fi[0..4].try_into().unwrap());
            let (width, height) = (u16le(4), u16le(6));
            let (depth, mip_count) = (u16le(8), u16le(10));
            let type_flags = u32::from_le_bytes(fi[12..16].try_into().unwrap());

            let Some(format) = PixelFormat::from_d3d(id) else {
                unsupported.push(id);
                continue;
            };
            if depth != 1 {
                // Volume textures: never observed, layout of the extra
                // slices unproven — report instead of guessing.
                unsupported.push(id);
                continue;
            }
            if width == 0 || height == 0 {
                return Err(bad());
            }

            let mut faces = Vec::new();
            for face in fmt.children_tagged(b"FACE") {
                let mut mips = Vec::new();
                for lvl in face.children_tagged(b"LVL_") {
                    let li = lvl.child(b"INFO").ok_or_else(bad)?.payload;
                    if li.len() < 8 {
                        return Err(bad());
                    }
                    let level = u32::from_le_bytes(li[0..4].try_into().unwrap()) as usize;
                    let body = lvl.child(b"BODY").ok_or_else(bad)?.payload;
                    let (w, h) = (halve(width, level), halve(height, level));
                    mips.push(Mip {
                        width: w,
                        height: h,
                        rgba: decode(id, w, h, body)?,
                    });
                }
                faces.push(Face { mips });
            }
            surfaces.push(Surface {
                format,
                width,
                height,
                depth,
                mip_count,
                type_flags,
                faces,
            });
        }

        if surfaces.is_empty() && !unsupported.is_empty() {
            let list: Vec<String> = unsupported.iter().map(|&i| fourcc_or_id(i)).collect();
            return Err(UcfbError::Unsupported(format!(
                "texture {name}: no decodable format (have {})",
                list.join(", ")
            )));
        }
        Ok(Texture {
            name,
            declared,
            surfaces,
            unsupported,
        })
    }

    /// The surface the PC game itself renders: the first format in the
    /// `tex_ INFO` list. Use this to match the original look.
    pub fn preferred(&self) -> Option<&Surface> {
        let first = *self.declared.first()?;
        self.surfaces
            .iter()
            .find(|s| s.format.d3d_id() == first)
            .or_else(|| self.surfaces.first())
    }

    /// The decoded surface closest to the artist's source image (see the
    /// module header's measured ranking). Use this when re-encoding, e.g.
    /// baking an HD override; use [`Texture::preferred`] to reproduce what
    /// the PC game shows.
    pub fn best(&self) -> Option<&Surface> {
        self.surfaces.iter().max_by_key(|s| s.format.fidelity())
    }

    /// The surface stored in a specific format, if present.
    pub fn surface(&self, format: PixelFormat) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.format == format)
    }
}

/// Render a `D3DFORMAT` id for diagnostics: `DXT2`-style fourccs print as
/// text, numeric ids as `D3DFMT#nn`.
fn fourcc_or_id(id: u32) -> String {
    let b = id.to_le_bytes();
    if b.iter().all(|&c| (0x20..0x7f).contains(&c)) {
        String::from_utf8_lossy(&b).into_owned()
    } else {
        format!("D3DFMT#{id}")
    }
}

/// Decode one raw D3D surface to RGBA8.
///
/// `body` shorter than the full surface is decoded as far as it goes (see the
/// truncating block-count quirk in the module header); anything shorter than
/// the munger's own size rule is [`UcfbError::Truncated`], and an id with no
/// decoder is [`UcfbError::Unsupported`].
pub fn decode(d3d_format: u32, w: u32, h: u32, body: &[u8]) -> Result<Vec<u8>, UcfbError> {
    let Some(format) = PixelFormat::from_d3d(d3d_format) else {
        return Err(UcfbError::Unsupported(format!(
            "D3DFORMAT {} ({w}x{h})",
            fourcc_or_id(d3d_format)
        )));
    };
    if body.len() < format.body_len(w, h) {
        return Err(UcfbError::Truncated(body.len()));
    }
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; w * h * 4];
    if format.is_block() {
        decode_blocks(format, w, h, body, &mut out);
    } else {
        decode_linear(format, w * h, body, &mut out);
    }
    Ok(out)
}

fn decode_linear(format: PixelFormat, texels: usize, body: &[u8], out: &mut [u8]) {
    let bpt = format.bytes_per_texel();
    for (i, px) in body.chunks_exact(bpt).take(texels).enumerate() {
        let c = match format {
            // Stored B, G, R, A (little-endian 0xAARRGGBB).
            A8R8G8B8 => [px[2], px[1], px[0], px[3]],
            X8R8G8B8 => [px[2], px[1], px[0], 255],
            R5G6B5 => {
                let v = u16::from_le_bytes([px[0], px[1]]);
                let c = rgb565(v);
                [c[0], c[1], c[2], 255]
            }
            A1R5G5B5 => {
                let v = u16::from_le_bytes([px[0], px[1]]);
                [
                    expand5(((v >> 10) & 0x1F) as u8),
                    expand5(((v >> 5) & 0x1F) as u8),
                    expand5((v & 0x1F) as u8),
                    if v & 0x8000 != 0 { 255 } else { 0 },
                ]
            }
            A4R4G4B4 => {
                let v = u16::from_le_bytes([px[0], px[1]]);
                [
                    expand4(((v >> 8) & 0xF) as u8),
                    expand4(((v >> 4) & 0xF) as u8),
                    expand4((v & 0xF) as u8),
                    expand4(((v >> 12) & 0xF) as u8),
                ]
            }
            // D3D9 reads A8 as (0, 0, 0, a).
            A8 => [0, 0, 0, px[0]],
            L8 => [px[0], px[0], px[0], 255],
            // Low byte luminance, high byte alpha.
            A8L8 => [px[0], px[0], px[0], px[1]],
            V8U8 => {
                // The normal-water shader samples linear RGBA. Store the two
                // signed components as UNORM and reconstruct the up component.
                let signed = |byte: u8| f32::from((byte as i8).max(-127)) / 127.0;
                let unorm = |component: f32| ((component * 0.5 + 0.5) * 255.0).round() as u8;
                let u = signed(px[0]);
                let v = signed(px[1]);
                let up = (1.0 - u * u - v * v).max(0.0).sqrt();
                [unorm(u), unorm(v), unorm(up), 255]
            }
            Dxt1 | Dxt3 | Dxt5 => unreachable!("block format in decode_linear"),
        };
        out[i * 4..i * 4 + 4].copy_from_slice(&c);
    }
}

fn decode_blocks(format: PixelFormat, w: usize, h: usize, body: &[u8], out: &mut [u8]) {
    let bb = format.block_bytes();
    // Grid the surface *should* have; short bodies simply stop early and
    // leave the tail texels at zero.
    let bw = w.div_ceil(4).max(1);
    let bh = h.div_ceil(4).max(1);
    for (n, block) in body.chunks_exact(bb).take(bw * bh).enumerate() {
        let (bx, by) = (n % bw, n / bw);
        let mut texels = [[0u8; 4]; 16];
        match format {
            Dxt1 => {
                let pal = dxt_colors(&block[0..8], true);
                let bits = u32::from_le_bytes(block[4..8].try_into().unwrap());
                for (t, px) in texels.iter_mut().enumerate() {
                    *px = pal[(bits >> (2 * t)) as usize & 3];
                }
            }
            Dxt3 => {
                let pal = dxt_colors(&block[8..16], false);
                let bits = u32::from_le_bytes(block[12..16].try_into().unwrap());
                for (t, px) in texels.iter_mut().enumerate() {
                    *px = pal[(bits >> (2 * t)) as usize & 3];
                    // Explicit alpha: one nibble per texel, low nibble first.
                    let nib = (block[t / 2] >> (4 * (t % 2))) & 0xF;
                    px[3] = expand4(nib);
                }
            }
            Dxt5 => {
                let pal = dxt_colors(&block[8..16], false);
                let bits = u32::from_le_bytes(block[12..16].try_into().unwrap());
                let alpha = dxt5_alphas(block[0], block[1]);
                let idx = u64::from_le_bytes([
                    block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
                ]);
                for (t, px) in texels.iter_mut().enumerate() {
                    *px = pal[(bits >> (2 * t)) as usize & 3];
                    px[3] = alpha[(idx >> (3 * t)) as usize & 7];
                }
            }
            _ => unreachable!("linear format in decode_blocks"),
        }
        for ty in 0..4 {
            let y = by * 4 + ty;
            if y >= h {
                break;
            }
            for tx in 0..4 {
                let x = bx * 4 + tx;
                if x >= w {
                    break;
                }
                let o = (y * w + x) * 4;
                out[o..o + 4].copy_from_slice(&texels[ty * 4 + tx]);
            }
        }
    }
}

/// The four-entry color palette of a BC1/BC2/BC3 color block. `punchthrough`
/// enables DXT1's `c0 <= c1` 1-bit-alpha mode; BC2/BC3 always use the
/// four-opaque-color mode.
fn dxt_colors(block: &[u8], punchthrough: bool) -> [[u8; 4]; 4] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let a = rgb565(c0);
    let b = rgb565(c1);
    let mut pal = [[0u8; 4]; 4];
    pal[0] = [a[0], a[1], a[2], 255];
    pal[1] = [b[0], b[1], b[2], 255];
    if punchthrough && c0 <= c1 {
        for i in 0..3 {
            pal[2][i] = ((a[i] as u16 + b[i] as u16) / 2) as u8;
        }
        pal[2][3] = 255;
        pal[3] = [0, 0, 0, 0];
    } else {
        for i in 0..3 {
            pal[2][i] = ((2 * a[i] as u16 + b[i] as u16) / 3) as u8;
            pal[3][i] = ((a[i] as u16 + 2 * b[i] as u16) / 3) as u8;
        }
        pal[2][3] = 255;
        pal[3][3] = 255;
    }
    pal
}

/// The eight-entry alpha ramp of a BC3 alpha block.
fn dxt5_alphas(a0: u8, a1: u8) -> [u8; 8] {
    let mut a = [0u8; 8];
    a[0] = a0;
    a[1] = a1;
    let (x, y) = (a0 as u16, a1 as u16);
    if a0 > a1 {
        for (i, slot) in a.iter_mut().enumerate().take(8).skip(2) {
            let k = i as u16 - 1;
            *slot = (((7 - k) * x + k * y) / 7) as u8;
        }
    } else {
        for (i, slot) in a.iter_mut().enumerate().take(6).skip(2) {
            let k = i as u16 - 1;
            *slot = (((5 - k) * x + k * y) / 5) as u8;
        }
        a[6] = 0;
        a[7] = 255;
    }
    a
}

fn rgb565(v: u16) -> [u8; 3] {
    [
        expand5(((v >> 11) & 0x1F) as u8),
        expand6(((v >> 5) & 0x3F) as u8),
        expand5((v & 0x1F) as u8),
    ]
}

fn expand4(v: u8) -> u8 {
    v << 4 | v
}
fn expand5(v: u8) -> u8 {
    v << 3 | v >> 2
}
fn expand6(v: u8) -> u8 {
    v << 2 | v >> 4
}

/// Every PC `tex_` in a ucfb payload, `lvl_` sub-level wrappers flattened
/// (same traversal as [`crate::content_chunks`]). Chunks that fail to parse
/// are skipped — call [`Texture::parse`] directly when errors matter.
pub fn find_textures(inner: &[u8]) -> Vec<Texture> {
    let Ok(chunks) = crate::content_chunks(inner) else {
        return Vec::new();
    };
    chunks
        .iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| Texture::parse(c).ok())
        .collect()
}

/// Look one texture up by name, ASCII-case-insensitively (munged names are
/// lowercase, but callers match against PS2 names of either case).
pub fn find_texture(inner: &[u8], name: &str) -> Option<Texture> {
    let Ok(chunks) = crate::content_chunks(inner) else {
        return None;
    };
    chunks
        .iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| Texture::parse(c).ok())
        .find(|t| t.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_chunk as chunk;

    /// `tex_ { NAME, INFO, FMT_ { INFO, FACE { LVL_ { INFO, BODY } } } }`.
    fn synth_tex(name: &str, d3d: u32, w: u16, h: u16, mips: &[Vec<u8>]) -> Vec<u8> {
        let mut face = Vec::new();
        for (i, body) in mips.iter().enumerate() {
            let mut info = Vec::new();
            info.extend((i as u32).to_le_bytes());
            info.extend((body.len() as u32).to_le_bytes());
            let mut lvl = chunk(b"INFO", &info);
            lvl.extend(chunk(b"BODY", body));
            face.extend(chunk(b"LVL_", &lvl));
        }
        let mut finfo = Vec::new();
        finfo.extend(d3d.to_le_bytes());
        finfo.extend(w.to_le_bytes());
        finfo.extend(h.to_le_bytes());
        finfo.extend(1u16.to_le_bytes());
        finfo.extend((mips.len() as u16).to_le_bytes());
        finfo.extend(1u32.to_le_bytes());
        let mut fmt = chunk(b"INFO", &finfo);
        fmt.extend(chunk(b"FACE", &face));

        let mut info = Vec::new();
        info.extend(1u32.to_le_bytes());
        info.extend(d3d.to_le_bytes());
        let mut name_z = name.as_bytes().to_vec();
        name_z.push(0);
        let mut tex = chunk(b"NAME", &name_z);
        tex.extend(chunk(b"INFO", &info));
        tex.extend(chunk(b"FMT_", &fmt));
        chunk(b"tex_", &tex)
    }

    fn only_chunk(buf: &[u8]) -> Chunk<'_> {
        crate::chunks(buf).next().unwrap().unwrap()
    }

    /// Hand-built DXT1 block: c0 = pure red, c1 = pure blue, c0 > c1 so the
    /// opaque four-color mode applies. Row 0 walks the whole palette.
    #[test]
    fn dxt1_opaque_block_palette() {
        let c0: u16 = 0xF800; // R5=31, G6=0, B5=0
        let c1: u16 = 0x001F; // R5=0,  G6=0, B5=31
        assert!(c0 > c1);
        let mut block = Vec::new();
        block.extend(c0.to_le_bytes());
        block.extend(c1.to_le_bytes());
        // Texel indices 0,1,2,3 on row 0; index 0 everywhere else.
        block.extend(0b1110_0100u32.to_le_bytes());
        let rgba = decode(Dxt1.d3d_id(), 4, 4, &block).unwrap();
        let px = |i: usize| &rgba[i * 4..i * 4 + 4];
        assert_eq!(px(0), [255, 0, 0, 255]);
        assert_eq!(px(1), [0, 0, 255, 255]);
        // 2/3 red + 1/3 blue, and 1/3 red + 2/3 blue.
        assert_eq!(px(2), [170, 0, 85, 255]);
        assert_eq!(px(3), [85, 0, 170, 255]);
        // Rest of the block is index 0.
        for i in 4..16 {
            assert_eq!(px(i), [255, 0, 0, 255], "texel {i}");
        }
    }

    /// c0 <= c1 switches DXT1 to 3 colors + punch-through alpha.
    #[test]
    fn dxt1_punchthrough_block() {
        let mut block = Vec::new();
        block.extend(0x001Fu16.to_le_bytes()); // blue
        block.extend(0xF800u16.to_le_bytes()); // red  (c0 < c1)
        block.extend(0b1110_0100u32.to_le_bytes());
        let rgba = decode(Dxt1.d3d_id(), 4, 4, &block).unwrap();
        let px = |i: usize| &rgba[i * 4..i * 4 + 4];
        assert_eq!(px(0), [0, 0, 255, 255]);
        assert_eq!(px(1), [255, 0, 0, 255]);
        assert_eq!(px(2), [127, 0, 127, 255]); // 50/50 blend
        assert_eq!(px(3), [0, 0, 0, 0]); // transparent black
    }

    /// DXT3's 4-bit alpha nibbles are low-nibble-first, and its color block
    /// never takes the punch-through branch even when c0 <= c1.
    #[test]
    fn dxt3_explicit_alpha_and_no_punchthrough() {
        let mut block = vec![0u8; 8];
        block[0] = 0xF0; // texel 0 -> 0x0, texel 1 -> 0xF
        block[1] = 0x8_1; // texel 2 -> 0x1, texel 3 -> 0x8
        block.extend(0x001Fu16.to_le_bytes()); // c0 blue
        block.extend(0xF800u16.to_le_bytes()); // c1 red -> c0 <= c1
        block.extend(0b1110_0100u32.to_le_bytes());
        let rgba = decode(Dxt3.d3d_id(), 4, 4, &block).unwrap();
        let px = |i: usize| &rgba[i * 4..i * 4 + 4];
        assert_eq!(px(0), [0, 0, 255, 0x00]);
        assert_eq!(px(1), [255, 0, 0, 0xFF]);
        // Index 2/3 are the 1/3 and 2/3 blends, NOT the punch-through pair.
        assert_eq!(px(2), [85, 0, 170, 0x11]);
        assert_eq!(px(3), [170, 0, 85, 0x88]);
    }

    /// DXT5 alpha: a0 > a1 gives the 8-value ramp, a0 <= a1 the 6-value ramp
    /// plus 0 and 255.
    #[test]
    fn dxt5_alpha_ramps() {
        assert_eq!(dxt5_alphas(255, 0), [255, 0, 218, 182, 145, 109, 72, 36]);
        assert_eq!(dxt5_alphas(0, 255), [0, 255, 51, 102, 153, 204, 0, 255]);
        let mut block = vec![0u8; 16];
        block[0] = 200;
        block[1] = 100;
        // 3-bit indices, LSB first: texel0 = 0, texel1 = 1, texel2 = 2.
        let idx: u64 = 1 << 3 | 2 << 6;
        block[2..8].copy_from_slice(&idx.to_le_bytes()[0..6]);
        block[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes()); // c0 white
        block[10..12].copy_from_slice(&0u16.to_le_bytes()); // c1 black
        let rgba = decode(Dxt5.d3d_id(), 4, 4, &block).unwrap();
        assert_eq!(&rgba[0..4], [255, 255, 255, 200]);
        assert_eq!(&rgba[4..8], [255, 255, 255, 100]);
        assert_eq!(
            &rgba[8..12],
            [255, 255, 255, ((6 * 200u32 + 100) / 7) as u8]
        );
    }

    #[test]
    fn linear_formats_channel_order() {
        // A8R8G8B8 is stored B, G, R, A.
        assert_eq!(
            decode(21, 1, 1, &[0x11, 0x22, 0x33, 0x44]).unwrap(),
            [0x33, 0x22, 0x11, 0x44]
        );
        // X8R8G8B8 ignores the top byte.
        assert_eq!(
            decode(22, 1, 1, &[0x11, 0x22, 0x33, 0x44]).unwrap(),
            [0x33, 0x22, 0x11, 0xFF]
        );
        // R5G6B5 pure green.
        assert_eq!(
            decode(23, 1, 1, &0x07E0u16.to_le_bytes()).unwrap(),
            [0, 255, 0, 255]
        );
        // A1R5G5B5: alpha bit clear -> transparent, red channel full.
        assert_eq!(
            decode(25, 1, 1, &0x7C00u16.to_le_bytes()).unwrap(),
            [255, 0, 0, 0]
        );
        assert_eq!(
            decode(25, 1, 1, &0xFC00u16.to_le_bytes()).unwrap(),
            [255, 0, 0, 255]
        );
        // A4R4G4B4 0xA987 -> a=A r=9 g=8 b=7, each nibble doubled.
        assert_eq!(
            decode(26, 1, 1, &0xA987u16.to_le_bytes()).unwrap(),
            [0x99, 0x88, 0x77, 0xAA]
        );
        // A8, L8, A8L8 (low byte luminance, high byte alpha).
        assert_eq!(decode(28, 1, 1, &[0x7F]).unwrap(), [0, 0, 0, 0x7F]);
        assert_eq!(decode(50, 1, 1, &[0x7F]).unwrap(), [0x7F, 0x7F, 0x7F, 255]);
        assert_eq!(
            decode(51, 1, 1, &[0x40, 0x80]).unwrap(),
            [0x40, 0x40, 0x40, 0x80]
        );
    }

    #[test]
    fn unknown_format_is_a_typed_error() {
        // DXT2 is a real fourcc we deliberately do not decode.
        let e = decode(0x3254_5844, 4, 4, &[0u8; 16]).unwrap_err();
        assert!(
            matches!(e, UcfbError::Unsupported(ref m) if m.contains("DXT2")),
            "{e}"
        );
        let e = decode(999, 1, 1, &[0u8; 4]).unwrap_err();
        assert!(
            matches!(e, UcfbError::Unsupported(ref m) if m.contains("D3DFMT#999")),
            "{e}"
        );
        // A tex_ whose only FMT_ is undecodable errors instead of panicking.
        let buf = synth_tex("weird", 0x3254_5844, 4, 4, &[vec![0u8; 16]]);
        let e = Texture::parse(&only_chunk(&buf)).unwrap_err();
        assert!(matches!(e, UcfbError::Unsupported(_)), "{e}");
    }

    #[test]
    fn decodes_signed_bump_vectors_for_linear_normal_sampling() {
        let pixels = decode(60, 3, 1, &[0, 0, 127, 0, 0, 128]).unwrap();
        assert_eq!(
            pixels,
            [128, 128, 255, 255, 255, 128, 128, 255, 128, 0, 128, 255]
        );
        assert!(matches!(
            decode(60, 1, 1, &[0]),
            Err(UcfbError::Truncated(1))
        ));
    }

    /// A corrupt `mipLevel` must not blow up the shift.
    #[test]
    fn absurd_mip_level_saturates() {
        assert_eq!(halve(256, 0), 256);
        assert_eq!(halve(256, 8), 1);
        assert_eq!(halve(256, 9), 1);
        assert_eq!(halve(256, 999_999), 1);
        let mut buf = synth_tex("bogus", Dxt1.d3d_id(), 8, 8, &[vec![0u8; 8]]);
        // Rewrite the LVL_ INFO mipLevel from 0 to 0xFFFF_FFFF in place:
        // LVL_ header (8 B) + INFO header (8 B) lands on the field.
        let lvl = buf.windows(4).position(|w| w == b"LVL_").unwrap();
        buf[lvl + 16..lvl + 20].copy_from_slice(&u32::MAX.to_le_bytes());
        let t = Texture::parse(&only_chunk(&buf)).unwrap();
        let m = &t.surfaces[0].faces[0].mips[0];
        assert_eq!((m.width, m.height), (1, 1));
    }

    #[test]
    fn short_body_is_truncated_not_a_panic() {
        assert!(matches!(
            decode(Dxt1.d3d_id(), 8, 8, &[0u8; 16]).unwrap_err(),
            UcfbError::Truncated(16)
        ));
        assert!(matches!(
            decode(21, 4, 4, &[0u8; 12]).unwrap_err(),
            UcfbError::Truncated(12)
        ));
    }

    /// The munger's truncating block count (module header): an 8x2 DXT3 mip
    /// is one block, not the two Direct3D would allocate.
    #[test]
    fn body_len_uses_truncating_block_count() {
        assert_eq!(Dxt1.body_len(256, 256), 32768);
        assert_eq!(Dxt1.body_len(4, 4), 8);
        assert_eq!(Dxt1.body_len(1, 1), 8);
        assert_eq!(Dxt3.body_len(8, 2), 16);
        assert_eq!(Dxt3.body_len(8, 4), 32);
        assert_eq!(R5G6B5.body_len(2, 2), 8);
        assert_eq!(A8R8G8B8.body_len(2, 2), 16);
        // ...and a short 8x2 body decodes the block it has, zeroing the rest.
        let mut block = vec![0u8; 8];
        block.extend(0xFFFFu16.to_le_bytes());
        block.extend(0xFFFFu16.to_le_bytes());
        block.extend(0u32.to_le_bytes());
        let rgba = decode(Dxt3.d3d_id(), 8, 2, &block).unwrap();
        assert_eq!(rgba.len(), 8 * 2 * 4);
        assert_eq!(&rgba[0..4], [255, 255, 255, 0]); // covered by the block
        assert_eq!(&rgba[16..20], [0, 0, 0, 0]); // x=4: no data
    }

    #[test]
    fn parses_synthetic_texture_and_mip_chain() {
        // 8x8 DXT1: mip0 = 4 blocks, mip1 = 1 block (4x4), mip2 = 1 (2x2).
        let mut solid = Vec::new();
        solid.extend(0xF800u16.to_le_bytes());
        solid.extend(0xF800u16.to_le_bytes());
        solid.extend(0u32.to_le_bytes());
        let mip0: Vec<u8> = solid.iter().cycle().take(32).copied().collect();
        let buf = synth_tex("red8", Dxt1.d3d_id(), 8, 8, &[mip0, solid.clone(), solid]);
        let t = Texture::parse(&only_chunk(&buf)).unwrap();
        assert_eq!(t.name, "red8");
        assert_eq!(t.declared, vec![Dxt1.d3d_id()]);
        assert!(t.unsupported.is_empty());
        let s = t.best().unwrap();
        assert_eq!(s.format, Dxt1);
        assert_eq!((s.width, s.height, s.mip_count), (8, 8, 3));
        assert!(!s.is_cube());
        let mips = &s.faces[0].mips;
        assert_eq!(mips.len(), 3);
        assert_eq!(
            mips.iter().map(|m| (m.width, m.height)).collect::<Vec<_>>(),
            vec![(8, 8), (4, 4), (2, 2)]
        );
        assert_eq!(mips[0].rgba.len(), 8 * 8 * 4);
        assert_eq!(&mips[0].rgba[0..4], [255, 0, 0, 255]);
        assert_eq!(s.mip_dims(2), (2, 2));
        assert_eq!(s.mip0().unwrap().width, 8);
        assert!(t.surface(Dxt5).is_none());
    }

    #[test]
    fn format_ranking_and_ids() {
        // Measured order (module header): true colour > DXT3 > A4R4G4B4.
        assert!(A8R8G8B8.fidelity() > Dxt5.fidelity());
        assert!(Dxt3.fidelity() > A4R4G4B4.fidelity());
        assert!(R5G6B5.fidelity() > Dxt1.fidelity());
        assert_eq!(PixelFormat::from_d3d(0x3154_5844), Some(Dxt1));
        assert_eq!(PixelFormat::from_d3d(24), None);
        for f in [
            A8R8G8B8, X8R8G8B8, R5G6B5, A1R5G5B5, A4R4G4B4, A8, L8, A8L8, Dxt1, Dxt3, Dxt5,
        ] {
            assert_eq!(PixelFormat::from_d3d(f.d3d_id()), Some(f), "{}", f.name());
        }
    }

    /// Data-gated: real PC LVLs from the maintainer's local (gitignored, read-only)
    /// `community_mods/harrisonfog/` collection. The LVLs live inside the
    /// mod zips, so the test streams one member out with `unzip -p` and never
    /// writes anything; it SKIPs when the archive (or `unzip`) is missing.
    fn pc_lvl(zip: &str, member: &str) -> Option<Vec<u8>> {
        let zip = format!(
            "{}/../../../community_mods/harrisonfog/{zip}",
            env!("CARGO_MANIFEST_DIR")
        );
        if !std::path::Path::new(&zip).exists() {
            eprintln!("SKIP: {zip} not present");
            return None;
        }
        let out = match std::process::Command::new("unzip")
            .args(["-p", &zip, member])
            .output()
        {
            Ok(out) => out,
            Err(e) => {
                eprintln!("SKIP: cannot run unzip ({e})");
                return None;
            }
        };
        if !out.status.success() || out.stdout.is_empty() {
            eprintln!("SKIP: unzip -p {zip} {member} produced nothing");
            return None;
        }
        Some(out.stdout)
    }

    #[test]
    fn bs1_pc_lvls_decode() {
        const BS1: &str = "10-addon-maps-keep/BS1_extract_in_addon_folder.2.zip";
        // ingame.lvl: one texture, "gonk", DXT1 + R5G6B5 (module header
        // documents every offset in this file).
        let Some(buf) = pc_lvl(BS1, "BS1/data/_LVL_PC/ingame.lvl") else {
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let texs = find_textures(inner);
        assert_eq!(texs.len(), 1);
        let gonk = &texs[0];
        assert_eq!(gonk.name, "gonk");
        assert_eq!(gonk.declared, vec![Dxt1.d3d_id(), R5G6B5.d3d_id()]);
        assert!(gonk.unsupported.is_empty());
        assert_eq!(gonk.surfaces.len(), 2);
        for s in &gonk.surfaces {
            assert_eq!((s.width, s.height, s.mip_count), (256, 256, 9));
            assert_eq!(s.faces.len(), 1);
            assert_eq!(s.faces[0].mips.len(), 9);
            assert_eq!(s.faces[0].mips[0].rgba.len(), 256 * 256 * 4);
            assert_eq!(s.faces[0].mips[8].rgba.len(), 4);
        }
        // The two encodings of the same image must agree to within DXT1's
        // and R5G6B5's quantisation, and the image is not blank.
        let dxt = gonk.surface(Dxt1).unwrap().mip0().unwrap();
        let raw = gonk.surface(R5G6B5).unwrap().mip0().unwrap();
        let mut worst = 0i32;
        for (a, b) in dxt.rgba.iter().zip(&raw.rgba) {
            worst = worst.max((*a as i32 - *b as i32).abs());
        }
        assert!(worst < 128, "DXT1 vs R5G6B5 differ by {worst}");
        assert!(dxt
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 40 || p[1] > 40));
        // The game renders the DXT1 copy; R5G6B5 is the more faithful one.
        assert_eq!(gonk.preferred().unwrap().format, Dxt1);
        assert_eq!(gonk.best().unwrap().format, R5G6B5);
        assert!(find_texture(inner, "GONK").is_some());
        assert!(find_texture(inner, "nope").is_none());

        // BES.lvl: 13 textures across six formats, some nested in lvl_ subs.
        let Some(buf) = pc_lvl(BS1, "BS1/data/_LVL_PC/SIDE/BES.lvl") else {
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let mut count = 0;
        let mut formats = std::collections::BTreeSet::new();
        for c in crate::content_chunks(inner).unwrap() {
            if &c.tag != b"tex_" {
                continue;
            }
            let t = Texture::parse(&c).unwrap_or_else(|e| panic!("tex_ @0x{:X}: {e}", c.offset));
            count += 1;
            assert!(t.unsupported.is_empty(), "{}: {:?}", t.name, t.unsupported);
            for s in &t.surfaces {
                formats.insert(s.format.name());
                assert_eq!(s.faces.len(), if s.is_cube() { 6 } else { 1 });
                for f in &s.faces {
                    assert_eq!(f.mips.len(), s.mip_count as usize, "{}", t.name);
                    for (i, m) in f.mips.iter().enumerate() {
                        assert_eq!((m.width, m.height), s.mip_dims(i), "{}", t.name);
                        assert_eq!(m.rgba.len(), (m.width * m.height * 4) as usize);
                    }
                }
            }
        }
        assert_eq!(count, 13);
        assert_eq!(
            formats.into_iter().collect::<Vec<_>>(),
            ["A1R5G5B5", "A4R4G4B4", "A8R8G8B8", "DXT1", "DXT3", "R5G6B5"]
        );
        // cis_maf @0xE2EB78 ships all three of DXT3, A4R4G4B4 and
        // A8R8G8B8, so the true-colour copy is ground truth for the block
        // decoder: mip0 BODY@0x122EF14, 4 MiB = 1024*1024*4.
        let maf = find_texture(inner, "cis_maf").expect("cis_maf");
        assert_eq!(
            maf.declared,
            vec![Dxt3.d3d_id(), A4R4G4B4.d3d_id(), A8R8G8B8.d3d_id()]
        );
        assert_eq!(maf.preferred().unwrap().format, Dxt3);
        let truth = maf.best().unwrap();
        assert_eq!(truth.format, A8R8G8B8);
        assert_eq!(
            (truth.width, truth.height, truth.mip_count),
            (1024, 1024, 11)
        );
        let t0 = truth.mip0().unwrap();
        assert_eq!(t0.rgba.len(), 1024 * 1024 * 4);
        // Real artwork: both colour and alpha use the full range.
        assert_eq!(t0.rgba.iter().step_by(4).min(), Some(&0));
        assert_eq!(t0.rgba.iter().step_by(4).max(), Some(&255));
        assert_eq!(t0.rgba[3..].iter().step_by(4).min(), Some(&0));
        assert_eq!(t0.rgba[3..].iter().step_by(4).max(), Some(&255));
        // The DXT3 and A4R4G4B4 re-encodings of that same image must land
        // near it. RGBA RMSE is 4.08 / 7.98 here (3.34 / 7.77 on RGB alone,
        // the numbers quoted in the module header) — and the block-
        // compressed copy is the closer of the two, which is what
        // `fidelity()` encodes.
        let rmse = |fmt: PixelFormat| {
            let m = maf.surface(fmt).unwrap().mip0().unwrap();
            let se: f64 = m
                .rgba
                .iter()
                .zip(&t0.rgba)
                .map(|(a, b)| {
                    let d = *a as f64 - *b as f64;
                    d * d
                })
                .sum();
            (se / m.rgba.len() as f64).sqrt()
        };
        let (dxt3, a4) = (rmse(Dxt3), rmse(A4R4G4B4));
        assert!((4.0..4.2).contains(&dxt3), "DXT3 rmse {dxt3}");
        assert!((7.9..8.1).contains(&a4), "A4R4G4B4 rmse {a4}");
        assert!(dxt3 < a4);
    }

    /// Data-gated: the only cube map in everything sampled (six FACEs).
    #[test]
    fn tat2_envmap_is_a_cube_map() {
        let Some(buf) = pc_lvl(
            "20-superseded-by-installer/Mos_Eisley_Remake_by_HarrisonFog.zip",
            "Mos Eisley Remake/tat2.lvl",
        ) else {
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let env = find_texture(inner, "mos_envmap").expect("mos_envmap");
        assert_eq!(env.declared, vec![Dxt1.d3d_id(), A1R5G5B5.d3d_id()]);
        for s in &env.surfaces {
            assert!(s.is_cube());
            assert_eq!(s.type_flags, 2);
            assert_eq!(s.faces.len(), 6);
            assert_eq!((s.width, s.height, s.mip_count), (64, 64, 7));
            for f in &s.faces {
                assert_eq!(f.mips.len(), 7);
                assert_eq!(f.mips[0].rgba.len(), 64 * 64 * 4);
            }
        }
        // Sky faces differ from one another (not six copies of one image).
        let s = env.surface(Dxt1).unwrap();
        assert_ne!(s.faces[0].mips[0].rgba, s.faces[1].mips[0].rgba);
    }
}
