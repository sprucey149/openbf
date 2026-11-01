//! The terrain foliage, placed as the PS2 places it (analysis/re-foliage.md,
//! [O]): `mungefs::foliage` reads the map's `FOLG` layer mask and `prp_`
//! file; this module runs the PS2's deterministic placement (§4), the
//! per-instance tint (§6) and the path-block walk (§9), and writes
//! `foliage.bin` ([`manifest::foliage_bin`]) plus the manifest's
//! [`ManifestFoliage`] — with every `grasspatch` class's properties and its
//! sprite atlas ([`grass_class`], [`grass_atlas`], §7; the template itself is
//! built at runtime, `game::viewer::grass`).
//!
//! **The random table.** Placement draws from a 256-float table in the ELF's
//! `.data` (BF1 va 0x5453c8, BF2 0x636ac8 — the same 1024 bytes). It is read
//! from the EXTRACTED ELF at bake time, found beside the disc's `lvl/` tree,
//! so no ELF byte goes into git; [`TABLE_FNV`] pins which bytes were read.
//! No ELF → no FOLG instances (the path props still bake), with a warning.
//!
//! **EE arithmetic.** The EE's FPU rounds every `add.s`/`mul.s` toward zero,
//! and its adder first drops the smaller operand's bits past one guard bit;
//! [`ee_add`] / [`ee_mul`] do the same (the ELF's own instruction order,
//! 0x345d54–0x345da4), so x and z come out as the PS2's own floats — which
//! matters because the tint's random seed is the float BIT PATTERN of x·z
//! (§6). Measured against the PS2's RAM (values from `lead_tmp/re/folg/
//! live_instances.json`, read out of PCSX2 save states): all 2,085 captured
//! instances bit-exact, 2,058 of the 2,062 settled tints exact (the four
//! misses are FEL1 models at a spot another instance shares — [H] a reused
//! pool slot's stale tint). Chop without the alignment lands 6/28 END1 and
//! 0/16 FEL1 instances on the bit (one ULP short), and IEEE round-to-nearest
//! misses by one ULP too (it also breaks the tint seed). The `live_*` tests
//! pin a slice as literals.
//!
//! What the bake does NOT do: height and tilt (the runtime takes both from
//! the terrain it already loads, `game::viewer::foliage`), streaming or the
//! PS2's pool cap (§5 — the port draws every instance inside its distance),
//! the BF2 split-screen cut (§10), sounds, collision (§11, queue FOLGC).

use anyhow::{bail, Result};
use manifest::foliage_bin::Record;
use manifest::{
    FoliageKind, ManifestClass, ManifestFoliage, ManifestFoliageMesh, ManifestGrassClass,
    ManifestInstance, ManifestPath,
};
use mungefs::fnv1a_lowercase;
use mungefs::foliage::{FolgMask, FoliageFile, FoliageLayer, MeshKind, LAYERS};
use mungefs::terrain::Terrain;
use std::path::{Path, PathBuf};

/// FNV-1a over the table's 1024 bytes, identical in both ELFs.
pub const TABLE_FNV: u32 = 0xf1fd_ba48;
/// BF1 FOLG: always 512 cells a side at 16 m (B1 `FUN_00343b08` 330628–330630).
const BF1_DIM: u32 = 512;
const BF1_CELL: f32 = 16.0;
/// `yaw = 6.1415 · rand` (sic — not 2π; B1 332148).
const YAW_RANGE: f32 = 6.1415;

/// An EE result: exact in f64 for two f32s of these magnitudes (after
/// [`align`] for an add), then rounded TOWARD ZERO to f32.
fn chop(v: f64) -> f32 {
    let r = v as f32;
    if r.is_finite() && r != 0.0 && (r as f64).abs() > v.abs() {
        f32::from_bits(r.to_bits() - 1)
    } else {
        r
    }
}
fn ee_add(a: f32, b: f32) -> f32 {
    let (a, b) = align(a, b);
    chop(a as f64 + b as f64)
}
fn ee_sub(a: f32, b: f32) -> f32 {
    let (a, b) = align(a, b);
    chop(a as f64 - b as f64)
}
/// The EE adder's operand alignment: the smaller operand loses the mantissa
/// bits that would shift out past one guard bit (an exponent gap past 24
/// leaves only its sign).
fn align(a: f32, b: f32) -> (f32, f32) {
    let (ab, bb) = (a.to_bits(), b.to_bits());
    let gap = ((ab >> 23) & 0xff) as i32 - ((bb >> 23) & 0xff) as i32;
    let cut = |v: u32, g: i32| {
        if g > 24 {
            v & 0x8000_0000
        } else {
            v & (u32::MAX << (g - 1))
        }
    };
    match gap {
        g if g > 0 => (a, f32::from_bits(cut(bb, g))),
        g if g < 0 => (f32::from_bits(cut(ab, -g)), b),
        _ => (a, b),
    }
}
fn ee_mul(a: f32, b: f32) -> f32 {
    chop(a as f64 * b as f64)
}

fn fnv_bytes(b: &[u8]) -> u32 {
    b.iter().fold(0x811c_9dc5u32, |h, &x| {
        (h ^ x as u32).wrapping_mul(0x0100_0193)
    })
}

/// The ELF beside a disc's `lvl/` tree: the first ancestor of `lvl_path`
/// holding the game's executable.
pub fn elf_for(lvl_path: &str, game: &str) -> Option<PathBuf> {
    let name = if game == "bf2" {
        "SWBFRNT2.ELF"
    } else {
        "SWBFRONT.ELF"
    };
    Path::new(lvl_path)
        .ancestors()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// `len` bytes at virtual address `va` of an ELF32 image, through its LOAD
/// program headers.
fn elf_bytes(elf: &[u8], va: u32, len: u32) -> Result<&[u8]> {
    let u16_at = |o: usize| elf.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |o: usize| {
        elf.get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    if elf.get(..4) != Some(b"\x7fELF") {
        bail!("not an ELF");
    }
    let (Some(phoff), Some(phsz), Some(phn)) = (u32_at(0x1c), u16_at(0x2a), u16_at(0x2c)) else {
        bail!("short ELF header");
    };
    for i in 0..phn as usize {
        let h = phoff as usize + i * phsz as usize;
        let (Some(kind), Some(off), Some(vaddr), Some(filesz)) =
            (u32_at(h), u32_at(h + 4), u32_at(h + 8), u32_at(h + 16))
        else {
            bail!("short program header");
        };
        if kind != 1 || va < vaddr || va as u64 + len as u64 > vaddr as u64 + filesz as u64 {
            continue;
        }
        let at = off as usize + (va - vaddr) as usize;
        let Some(b) = elf.get(at..at + len as usize) else {
            bail!("{va:#x} past the end of the ELF");
        };
        return Ok(b);
    }
    bail!("no LOAD segment holds {va:#x}")
}

/// The placement table out of an ELF32 image, by its virtual address,
/// checked against [`TABLE_FNV`].
pub fn rng_table(elf: &[u8], game: &str) -> Result<[f32; 256]> {
    let va: u32 = if game == "bf2" { 0x636ac8 } else { 0x5453c8 };
    let b = elf_bytes(elf, va, 1024)?;
    if fnv_bytes(b) != TABLE_FNV {
        bail!(
            "the table at {va:#x} is not the foliage table (FNV {:#x})",
            fnv_bytes(b)
        );
    }
    Ok(std::array::from_fn(|k| {
        f32::from_le_bytes(b[4 * k..4 * k + 4].try_into().unwrap())
    }))
}

/// FNV-1a over the grass atlas block's 0x110 bytes, identical in both ELFs.
pub const ATLAS_FNV: u32 = 0x000b_e48d;

/// The grass sprite atlases (re-foliage §7): one block of `.data`, never
/// written — BF1 0x518948..0x518a58, BF2 0x5ffb20.. (the same bytes; B1
/// `FUN_002eca58` 276350–276362, B2 `FUN_003c6788` 422412–422424 pick the
/// same offsets). Per part four corners of u16 (u, v) in 1/2048. Indexed by
/// `NumParts`: 1 → +0xd0 (4 parts, every second one u-mirrored), 3 → +0x00
/// (3), 4 → +0x90 (the four quarters), anything else → +0x30 (6).
pub fn grass_atlas(elf: &[u8], game: &str, num_parts: i32) -> Result<Vec<[[f32; 2]; 4]>> {
    let va: u32 = if game == "bf2" { 0x5ffb20 } else { 0x518948 };
    let b = elf_bytes(elf, va, 0x110)?;
    if fnv_bytes(b) != ATLAS_FNV {
        bail!(
            "the block at {va:#x} is not the grass atlas (FNV {:#x})",
            fnv_bytes(b)
        );
    }
    let (at, parts) = match num_parts {
        1 => (0xd0, 4),
        3 => (0x00, 3),
        4 => (0x90, 4),
        _ => (0x30, 6),
    };
    let uv = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as f32 / 2048.0;
    Ok((0..parts)
        .map(|p| {
            std::array::from_fn(|c| {
                let o = at + p * 16 + c * 4;
                [uv(o), uv(o + 2)]
            })
        })
        .collect())
}

/// The same two tables as numbers (`scripts/gameplay_foliage.py` writes them
/// from the PS2 executables): what a disc with no such executable bakes from.
const SHIPPED_TABLES: &str = include_str!("../../manifest/data/gameplay/foliage-v1.json");

/// One game's row of [`SHIPPED_TABLES`].
fn shipped_row(game: &str) -> Result<serde_json::Value> {
    let all: serde_json::Value = serde_json::from_str(SHIPPED_TABLES)?;
    all["tables"]
        .as_array()
        .and_then(|t| t.iter().find(|r| r["game"] == game).cloned())
        .ok_or_else(|| anyhow::anyhow!("no foliage table for {game}"))
}

/// [`rng_table`] from the shipped numbers, under the same checksum.
pub fn shipped_rng_table(game: &str) -> Result<[f32; 256]> {
    let row = shipped_row(game)?;
    let values = row["placement_values"]
        .as_array()
        .filter(|v| v.len() == 256)
        .ok_or_else(|| anyhow::anyhow!("{game}: the shipped table is not 256 values"))?;
    let table: [f32; 256] = std::array::from_fn(|k| values[k].as_f64().unwrap_or(f64::NAN) as f32);
    let bytes: Vec<u8> = table.iter().flat_map(|v| v.to_le_bytes()).collect();
    if fnv_bytes(&bytes) != TABLE_FNV {
        bail!("{game}: the shipped table fails the executable's checksum");
    }
    Ok(table)
}

/// [`grass_atlas`] from the shipped numbers.
pub fn shipped_grass_atlas(game: &str, num_parts: i32) -> Result<Vec<[[f32; 2]; 4]>> {
    let row = shipped_row(game)?;
    let key = match num_parts {
        1 | 3 | 4 => num_parts.to_string(),
        _ => "default".to_owned(),
    };
    let rows: Vec<[[f32; 2]; 4]> =
        serde_json::from_value(row["atlas_uv_by_num_parts"][key].clone())?;
    if rows.is_empty() {
        bail!("{game}: no shipped grass atlas for NumParts {num_parts}");
    }
    Ok(rows)
}

/// A `grasspatch` class's manifest row from its ODF properties
/// (`(fnv hash, value)` in PROP order, a later one overriding an earlier as
/// the setter B1 `FUN_001adcd8` does), defaults from the constructor B1
/// `FUN_001ad820` 54532–54557. A hash the setter does not handle is ignored
/// (BF2 FEL1's `0x14c8ab0a`, `0x3b1981f3`). The atlas is left empty for
/// [`grass_atlas`] (it depends on the final `NumParts`).
pub fn grass_class(name: &str, props: &[(u32, String)]) -> ManifestGrassClass {
    let mut g = ManifestGrassClass {
        name: name.to_owned(),
        texture: "grass".into(),
        particles: 50,
        min_size: 10.0,
        max_size: 15.0,
        y_offset: 0.0,
        alpha: 0.5,
        max_distance: 100.0,
        radius_fade_min: -1.0,
        radius_fade_max: -1.0,
        darkness_min: 1.0,
        darkness_max: 1.0,
        num_parts: 4,
        rect: [-1.0, -1.0],
        raise: [0.2, 0.2],
        tall_size: 2.0,
        tall_lean: 0.5,
        shadow: 0.0,
        lean: 0.1,
        cycle: 6,
        thin: 0.3,
        still: false,
        atlas: Vec::new(),
    };
    for (h, v) in props {
        let mut f = v.split_whitespace().map(|t| t.parse::<f32>().ok());
        let mut one = || f.next().flatten();
        // `%d` reads an integer prefix; the disc's are plain integers.
        let int = |v: &str| v.trim().parse::<f32>().ok().map(|x| x as i32);
        match *h {
            0xd133_f7da => g.particles = int(v).map_or(g.particles, |n| n.max(0) as u32),
            0xeaeb_cf6e => g.min_size = one().unwrap_or(g.min_size),
            0x80ba_1308 => g.max_size = one().unwrap_or(g.max_size),
            0x3d5d_0aef => g.y_offset = one().unwrap_or(g.y_offset),
            0x5d8b_6dab => g.alpha = one().unwrap_or(g.alpha),
            0xe691_5cf6 => g.max_distance = one().unwrap_or(g.max_distance),
            0x3c64_68f4 => g.texture = v.trim().to_owned(),
            0x8088_7a6f => g.radius_fade_min = one().unwrap_or(g.radius_fade_min),
            0x6e74_d661 => g.radius_fade_max = one().unwrap_or(g.radius_fade_max),
            0x13fa_c25e => g.darkness_min = one().unwrap_or(g.darkness_min),
            0x25e5_c3fc => g.darkness_max = one().unwrap_or(g.darkness_max),
            0x5ac0_2207 => g.num_parts = int(v).unwrap_or(g.num_parts),
            0x35a4_393b => {
                g.rect = [one().unwrap_or(g.rect[0]), one().unwrap_or(g.rect[1])];
            }
            0x231f_cc79 => {
                g.raise = [one().unwrap_or(g.raise[0]), one().unwrap_or(g.raise[1])];
            }
            0xa9b1_4958 => g.tall_size = one().unwrap_or(g.tall_size),
            0xf658_2b9c => g.tall_lean = one().unwrap_or(g.tall_lean),
            0xc752_63e1 => g.shadow = one().unwrap_or(g.shadow),
            0x9713_3eeb => g.lean = one().unwrap_or(g.lean),
            0x4b6d_5171 => g.cycle = int(v).unwrap_or(g.cycle),
            0xff7b_b39a => g.thin = one().unwrap_or(g.thin),
            0x604d_b82e => g.still = true,
            _ => {}
        }
    }
    g
}

/// The FOLG grid: cells a side and metres a cell (re-foliage §2).
pub fn grid(game: &str, terrain: &Terrain) -> (u32, f32) {
    if game == "bf2" {
        (terrain.info.grid_size / 2, 2.0 * terrain.info.grid_unit)
    } else {
        (BF1_DIM, BF1_CELL)
    }
}

/// One placed instance, before the tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Index into the file's meshes, layers in order ([`mesh_rows`]).
    pub mesh: u16,
    /// World x, z (z = −z_f, re-foliage §4) and the yaw.
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
}

/// re-foliage §4, exactly: blocks of 2×2 cells; per block and layer whose
/// mask bit is set, the table RNG seeded with `bz·D8 + bx + L + 2·bz` and read
/// pre-incremented; per cell (z outer, x inner) with the bit, `count` times:
/// skip without a draw when `spread > 1` and x or z is off the spread grid;
/// pick the first mesh whose cumulative frequency ≥ r; then rx, rz, ryaw.
/// B1 `FUN_00345008` / `FUN_00344a68` / `FUN_00345c78`; B2 `FUN_0042b330` /
/// `FUN_0042add0` / `FUN_0042bf00`.
pub fn place(mask: &FolgMask, file: &FoliageFile, cell: f32, table: &[f32; 256]) -> Vec<Placed> {
    let dim = mask.dim as i32;
    let half = dim >> 1;
    let d8 = (dim + 1) / 2;
    let base: Vec<u16> = file
        .layers
        .iter()
        .scan(0u16, |n, l| {
            let b = *n;
            *n += l.meshes.len() as u16;
            Some(b)
        })
        .collect();
    let mut out = Vec::new();
    for bz in 0..d8 {
        for bx in 0..d8 {
            let mut bits = 0u8;
            for zz in 0..2 {
                for xx in 0..2 {
                    bits |= mask.layers(2 * bx + xx, 2 * bz + zz);
                }
            }
            for (li, lay) in file.layers.iter().enumerate() {
                if bits >> li & 1 == 0 || lay.meshes.is_empty() {
                    continue;
                }
                let mut idx = (bz * d8 + bx + li as i32 + 2 * bz) as u32;
                let mut rng = || {
                    idx = idx.wrapping_add(1);
                    table[(idx & 0xff) as usize]
                };
                let s = lay.spread as i32;
                let sf = lay.spread as f32;
                for zz in 0..2 {
                    for xx in 0..2 {
                        let (x, z) = (2 * bx + xx, 2 * bz + zz);
                        if mask.layers(x, z) >> li & 1 == 0 {
                            continue;
                        }
                        for _ in 0..lay.count {
                            if s > 1 && (x % s > 0 || z % s > 0) {
                                continue;
                            }
                            let r = rng();
                            let Some(k) = lay.meshes.iter().position(|m| r <= m.cumulative) else {
                                continue;
                            };
                            let (rx, rz, ry) = (rng(), rng(), rng());
                            let at = |c: i32, u: f32| {
                                let t = ee_add((c - half) as f32, 0.5);
                                let t = ee_sub(t, ee_mul(sf, 0.45));
                                let t = ee_add(t, ee_mul(ee_mul(u, sf), 0.9));
                                ee_mul(t, cell)
                            };
                            out.push(Placed {
                                mesh: base[li] + k as u16,
                                x: at(x, rx),
                                z: -at(z, rz),
                                yaw: ee_mul(ry, YAW_RANGE),
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

/// The terrain's LUMI as the PS2 samples it: B1 `FUN_00473bc0` /
/// `FUN_00473d28` (537170–537210), B2 `FUN_0055f068` / `FUN_0055f1d0`. The
/// disc buffer is indexed **x-major** — `iz + ix·n` — which
/// `mungefs::terrain` transposes to [z][x] (card TLUMT), so this maps the
/// PS2's flat index back through the transpose; the PS2's reading reproduces the RAM tints of every
/// instance with no ColorVariation (lead, `lumicheck.py`, BF1 211/211).
pub struct Lumi<'a> {
    bytes: &'a [u8],
    n: usize,
    unit: f32,
    inv: f32,
    ox: f32,
    oz: f32,
}

impl<'a> Lumi<'a> {
    pub fn of(t: &'a Terrain) -> Option<Lumi<'a>> {
        let bytes = t.lumi.as_deref()?;
        let n = t.info.grid_size as usize;
        let unit = t.info.grid_unit;
        let half = (n / 2) as f32;
        (unit > 0.0).then(|| Lumi {
            bytes,
            n,
            unit,
            inv: 1.0 / unit,
            ox: half * unit,
            oz: (half - 1.0) * unit,
        })
    }

    fn at(&self, x: f32, z: f32) -> f32 {
        let iz = ((z + self.oz) * self.inv + 0.5) as i32;
        let ix = ((x + self.ox) * self.inv + 0.5) as i32;
        // The PS2's flat index, wrap-arounds at the edges included, then
        // where that disc byte sits in the transposed grid.
        let n = self.n;
        let i = iz as i64 + ix as i64 * n as i64;
        usize::try_from(i)
            .ok()
            .filter(|&i| i < n * n)
            .and_then(|i| self.bytes.get((i % n) * n + i / n))
            .map_or(0.0, |&b| b as f32 * 0.003921569)
    }

    /// LUMI / 255, bilinear, at world (x, z).
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        let h = self.unit * 0.5;
        let (zl, xl) = (z - h, x - h);
        let fz = (z - ((((zl + self.oz) * self.inv + 0.5) as i32) as f32 * self.unit - self.oz))
            * self.inv;
        let fx = (x - ((((xl + self.ox) * self.inv + 0.5) as i32) as f32 * self.unit - self.ox))
            * self.inv;
        let a = self.at(xl, zl);
        let b = self.at(x - h, z + h);
        let c = self.at(x + h, z - h);
        let d = self.at(x + h, z + h);
        let a = a + (b - a) * fz;
        a + ((c + (d - c) * fz) - a) * fx
    }
}

/// The per-instance tint (re-foliage §6; B1 `FUN_00346b70` 332750–332796, B2
/// `FUN_0042cdb0`): `c = min(0.7·(L + 0.5 for a model) + 0.6, 1)`; each of R,
/// G, B = `trunc(c · (1 − cv·u) · 255)`, u from the LCG `0x19660d/0x3c6ef35f`
/// seeded with the float BITS of `x·z` (the EE's chopped product) xor
/// 0xc71c2830 — steps 3–4 give R, 5–6 G, 7–8 B; A = 255.
pub fn tint(lumi: f32, model: bool, cv: f32, x: f32, z: f32) -> [u8; 4] {
    let l = if model { lumi + 0.5 } else { lumi };
    let c = (l * 0.7 + 0.6).min(1.0);
    let mut s = ee_mul(x, z).to_bits() ^ 0xc71c_2830;
    let mut seq = [0u32; 8];
    for v in &mut seq {
        s = s.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
        *v = s;
    }
    let u =
        |a: u32, b: u32| f32::from_bits(((a >> 16) | (b & 0xffff_0000)) >> 9 | 0x3f80_0000) - 1.0;
    let ch = |a: u32, b: u32| (c * (1.0 - cv * u(a, b)) * 255.0) as i32 as u8;
    [
        ch(seq[2], seq[3]),
        ch(seq[4], seq[5]),
        ch(seq[6], seq[7]),
        255,
    ]
}

/// The path block's props (re-foliage §9; B1 `FUN_003483f8` 333927–334274, B2
/// `FUN_0042e838`): walking each named path node to node, while the next node
/// is farther than `Distance`, step `Distance` along the segment and place a
/// class of the line at the step's MIDPOINT (the path's own height), upright,
/// its x axis along the path and its z axis `dir × up` (negated by the flip).
/// A line whose path or classes this LVL lacks places nothing. The class of a
/// line with several is the PS2's global LCG from the state 0 the level load
/// resets it to (`analysis/re-leafpatch.md`'s audit note: B1 `FUN_00173a98`,
/// B2 `FUN_00174b28`) — PROVISIONAL, since how many draws the load makes
/// before the path block is not counted; every shipped line names one class,
/// so it never matters on the disc. Ours: a line whose `Distance` is not
/// positive places nothing (the walk would never end), and a step on a
/// vertical segment, which has no side axis, places no prop.
pub fn path_props(
    file: &FoliageFile,
    paths: &[ManifestPath],
    classes: &[ManifestClass],
) -> Vec<ManifestInstance> {
    let mut lcg = 0u32;
    let mut out = Vec::new();
    for line in &file.paths {
        let names: Vec<&str> = line
            .classes
            .iter()
            .filter_map(|h| {
                classes
                    .iter()
                    .find(|c| fnv1a_lowercase(&c.name) == *h)
                    .map(|c| c.name.as_str())
            })
            .collect();
        let Some(path) = paths
            .iter()
            .find(|p| fnv1a_lowercase(&p.name) == line.path_hash)
        else {
            continue;
        };
        if names.is_empty() || path.nodes.is_empty() {
            continue;
        }
        let dist = line.distance;
        if dist.is_nan() || dist <= 0.0 {
            continue;
        }
        let mut cur = glam::Vec3::from(path.nodes[0].0);
        for node in &path.nodes[1..] {
            let next = glam::Vec3::from(node.0);
            while (next - cur).length_squared() > dist * dist {
                let dir = (next - cur).normalize();
                let side = dir.cross(glam::Vec3::Y) * if line.flip { -1.0 } else { 1.0 };
                if side.length_squared() == 0.0 {
                    cur += dir * dist;
                    continue;
                }
                let z_axis = side.normalize();
                let x_axis = glam::Vec3::Y.cross(side).normalize();
                let y_axis = z_axis.cross(x_axis);
                let a = lcg.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
                lcg = a.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
                let pick = ((a >> 16) | (lcg & 0xffff_0000)) as usize % names.len();
                out.push(ManifestInstance {
                    class: names[pick].to_owned(),
                    name: String::new(),
                    rotation: [x_axis.to_array(), y_axis.to_array(), z_axis.to_array()],
                    translation: (cur + dir * dist * 0.5).to_array(),
                    props: Vec::new(),
                });
                cur += dir * dist;
            }
        }
    }
    out
}

/// The manifest's mesh table, layers in order — what [`Placed::mesh`] and
/// `foliage.bin`'s `mesh` index. A `File` resolves against the LVL's `modl`
/// names, a patch against its classes; either may resolve to nothing.
pub fn mesh_rows(
    file: &FoliageFile,
    models: &[String],
    classes: &[ManifestClass],
) -> Vec<(ManifestFoliageMesh, f32)> {
    let mut rows = Vec::new();
    for (li, l) in file.layers.iter().enumerate().take(LAYERS) {
        for m in &l.meshes {
            let (kind, name) = match m.kind {
                Some(MeshKind::Model) => (
                    FoliageKind::Model,
                    models
                        .iter()
                        .find(|n| fnv1a_lowercase(n) == m.name_hash)
                        .cloned(),
                ),
                Some(k) => (
                    if k == MeshKind::GrassPatch {
                        FoliageKind::GrassPatch
                    } else {
                        FoliageKind::LeafPatch
                    },
                    classes
                        .iter()
                        .find(|c| fnv1a_lowercase(&c.name) == m.name_hash)
                        .map(|c| c.name.clone()),
                ),
                None => (FoliageKind::Model, None),
            };
            rows.push((
                ManifestFoliageMesh {
                    layer: li as u8,
                    kind,
                    name,
                    distance: m.distance,
                    near_fade: m.near_fade,
                    scale: m.scale,
                    stiffness: m.stiffness,
                    collision: m.collision,
                },
                m.color_variation,
            ));
        }
    }
    rows
}

/// Everything the foliage bake reads.
pub struct Inputs<'a> {
    pub lvl_path: &'a str,
    pub game: &'a str,
    pub terrain: Option<&'a Terrain>,
    pub mask: Option<&'a FolgMask>,
    pub file: Option<&'a FoliageFile>,
    pub models: &'a [String],
    pub classes: &'a [ManifestClass],
    pub paths: &'a [ManifestPath],
    /// Every `grasspatch`-based `entc` of the LVL: (name, PROPs).
    pub grass: &'a [(String, Vec<(u32, String)>)],
}

/// Place, tint and write `foliage.bin`, and bake the grass classes; `None`
/// when nothing is placed and the LVL ships no grass class.
pub fn bake(out: &Path, i: &Inputs<'_>) -> Result<Option<ManifestFoliage>> {
    let elf = match elf_for(i.lvl_path, i.game).map(std::fs::read) {
        Some(Ok(b)) => Some(b),
        _ => None,
    };
    // An Xbox disc ships no PS2 executable, and needs its numbers just the same.
    let shipped = elf.is_none() && crate::xbox::lvl_is_xbox(i.lvl_path).unwrap_or(false);
    if elf.is_none() && !shipped {
        eprintln!(
            "foliage: no {} ELF beside {} — no FOLG placement, no grass atlas",
            i.game, i.lvl_path
        );
    }
    let empty = FoliageFile {
        layers: std::array::from_fn(|_| FoliageLayer::empty()),
        paths: Vec::new(),
    };
    let file = i.file.unwrap_or(&empty);
    let rows = mesh_rows(file, i.models, i.classes);
    let mut records = Vec::new();
    if let (Some(t), Some(mask), Some(_)) = (i.terrain, i.mask, i.file) {
        let (dim, cell) = grid(i.game, t);
        let table = match &elf {
            Some(b) => Some(rng_table(b, i.game)?),
            None if shipped => Some(shipped_rng_table(i.game)?),
            None => None,
        };
        let mask = if mask.dim == dim {
            Some(mask)
        } else {
            eprintln!(
                "foliage: FOLG is {}² but the {} grid is {dim}² — not placed",
                mask.dim, i.game
            );
            None
        };
        if let (Some(table), Some(mask), Some(lumi)) = (table, mask, Lumi::of(t)) {
            for p in place(mask, file, cell, &table) {
                let (row, cv) = &rows[p.mesh as usize];
                if row.name.is_none() {
                    continue;
                }
                let model = row.kind == FoliageKind::Model;
                records.push(Record {
                    mesh: p.mesh,
                    x: p.x,
                    z: p.z,
                    yaw: p.yaw,
                    rgba: tint(lumi.sample(p.x, p.z), model, *cv, p.x, p.z),
                });
            }
        }
    }
    let path_props = path_props(file, i.paths, i.classes);
    // A class without its atlas draws nothing, so no ELF → no grass.
    let mut grass = Vec::new();
    if elf.is_some() || shipped {
        for (name, props) in i.grass {
            let mut g = grass_class(name, props);
            g.atlas = match &elf {
                Some(elf) => grass_atlas(elf, i.game, g.num_parts)?,
                None => shipped_grass_atlas(i.game, g.num_parts)?,
            };
            grass.push(g);
        }
    }
    if records.is_empty() && path_props.is_empty() && grass.is_empty() {
        return Ok(None);
    }
    let name = "foliage.bin";
    std::fs::write(out.join(name), manifest::foliage_bin::write(&records))?;
    Ok(Some(ManifestFoliage {
        instances: name.into(),
        count: records.len() as u32,
        meshes: rows.into_iter().map(|(m, _)| m).collect(),
        path_props,
        grass,
    }))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod shipped_tests {
    use super::*;

    /// Real data: the shipped numbers are the executables' numbers, bit for bit.
    #[test]
    fn the_shipped_tables_are_the_executables_tables() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        for (game, elf) in [("bf1", "bf1/SWBFRONT.ELF"), ("bf2", "bf2/SWBFRNT2.ELF")] {
            assert!(shipped_rng_table(game).is_ok());
            let Ok(elf) = std::fs::read(format!("{root}/{elf}")) else {
                eprintln!("SKIP: no {game} executable");
                continue;
            };
            let bits = |t: [f32; 256]| t.map(f32::to_bits);
            assert_eq!(
                bits(shipped_rng_table(game).unwrap()),
                bits(rng_table(&elf, game).unwrap())
            );
            for parts in [1, 3, 4, 2] {
                assert_eq!(
                    shipped_grass_atlas(game, parts).unwrap(),
                    grass_atlas(&elf, game, parts).unwrap(),
                    "{game} NumParts {parts}"
                );
            }
        }
    }
}
