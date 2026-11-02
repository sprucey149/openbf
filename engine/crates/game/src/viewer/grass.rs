//! The grass patches as the scene loads them (analysis/re-foliage.md §6–§7,
//! [O]; queue FOLGG): each baked `grasspatch` class
//! ([`manifest::ManifestGrassClass`]) becomes ONE sprite template, built as
//! B1 `FUN_001ae120` builds it, and each patch — a FOLG `GrassPatch` record
//! or a world instance of the class — a [`render::GrassPatch`]. The per-view
//! sprite quads, the sway and the fade are `render::grass`'s.
//!
//! **The template** (per sprite i of `NumParticles`, the draws in the ELF's
//! order): a position uniform in a disc of `RadiusFadeMax` by rejection from
//! the square, with a falloff f = 1 inside `RadiusFadeMin` and linear to 0 at
//! `RadiusFadeMax` (a point at f = 0 is drawn again) — or, when the class
//! gives a rectangle, uniform in it with f = 1; size `MinSize + u·(MaxSize −
//! MinSize)`; grey `DarknessMin + u·(DarknessMax − DarknessMin)`; an unused
//! pair of draws; phase `u·10`; static lean `(u − 0.5)·2·0x97133eeb`. Then by
//! `NumParts`:
//! - 3 → part `i % (cycle + 2)`, above 1 a TALL blade with part 2;
//! - ≥ 5 → part `i % (cycle + 4)`, folded by −2 below 6, above 3 tall;
//!   there, a normal sprite is `size` high and `size·0xff7bb39a` wide with an
//!   up scale `0.8 + 0.2u`, standing at the class's y offset;
//! - anything else → part `i % 4`, `1.1·size` wide, `0.9·size` high, sunk by
//!   `MinSize·(1 − f)/2`.
//!
//! A tall blade is `size·0xa9b14958` across, raised by `MinSize·(r₀ + u·(r₁ −
//! r₀)·(1 − f))` (`0x231fcc79`), with a fixed random facing (a horizontal
//! draw pair redrawn while both are under 0.01, normalised, then y = (u −
//! 0.5)·0.3, normalised again). The class's bound is the largest distance
//! from the sprites' box centre plus a sprite's width.
//!
//! **The random numbers.** The PS2 draws them from its global LCG
//! (`0x19660d`, `0x3c6ef35f`; two steps a number, the high halves as a 23-bit
//! mantissa) at whatever state the class is first drawn in, so its layout
//! differs run to run. The port seeds the same generator with the FNV-1a of
//! the class name: the same layout every run, and a different one per class
//! (PROVISIONAL, reversible; re-foliage §13).
//!
//! **A patch** stands at (x, mean of three terrain heights, z) — BF1 at (x,
//! z+3), (x∓2.25, z−2.25), BF2 at (x, z+4), (x∓3, z−3) — tilted to that
//! triangle's normal lerped toward up by `Stiffness` (NOT renormalised: the
//! ELF uses the lerp as the up row), never yawed or scaled (B1
//! `FUN_00347610` 333186–333253, the patch branch's rows: right =
//! normalize((0,0,−1) × up), forward = normalize(right × up)). Its colour is
//! the record's tint and its distance the `prp_` entry's. A world instance of
//! a class draws at its own matrix, white, to the class's `MaxDistance`
//! (`FUN_001af3e0`).

use crate::assets::Scene;
use glam::{Mat4, Vec3};
use manifest::foliage_bin::Record;
use manifest::{FoliageKind, ManifestGrassClass};
use render::{GrassClass, GrassPatch, GrassSprite, TextureHandle};
use std::collections::HashMap;

use core::lcg::GameLcg;

pub(super) fn fnv1a_lower(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, c| {
        (h ^ c.to_ascii_lowercase() as u32).wrapping_mul(0x0100_0193)
    })
}

/// Tries at a disc point before giving up on a class whose disc is empty
/// (`RadiusFadeMax` ≤ 0 — the PS2 would loop forever; no class on either
/// disc does it).
const DISC_TRIES: u32 = 1000;

/// A class's template sprites and bound (see the module doc).
pub fn template(g: &ManifestGrassClass) -> (Vec<GrassSprite>, f32) {
    let mut rng = GameLcg::new(fnv1a_lower(&g.name));
    let n = g.particles as usize;
    let mut sprites = Vec::with_capacity(n);
    for i in 0..n {
        let (mut pos, f) = if g.rect[0] > 0.0 || g.rect[1] > 0.0 {
            let x = (rng.unit() - 0.5) * g.rect[0];
            let z = (rng.unit() - 0.5) * g.rect[1];
            (Vec3::new(x, 0.0, z), 1.0)
        } else {
            let mut out = (Vec3::ZERO, 1.0);
            for _ in 0..DISC_TRIES {
                let x = (rng.unit() * 2.0 - 1.0) * g.radius_fade_max;
                let z = (rng.unit() * 2.0 - 1.0) * g.radius_fade_max;
                let d = (x * x + z * z).sqrt();
                let f = if d < g.radius_fade_min {
                    1.0
                } else if d < g.radius_fade_max {
                    1.0 - (d - g.radius_fade_min) / (g.radius_fade_max - g.radius_fade_min)
                } else {
                    0.0
                };
                if f != 0.0 {
                    out = (Vec3::new(x, 0.0, z), f);
                    break;
                }
            }
            out
        };
        let size = g.min_size + rng.unit() * (g.max_size - g.min_size);
        let grey = g.darkness_min + rng.unit() * (g.darkness_max - g.darkness_min);
        // A draw pair the template does not keep (redrawn while both are 0).
        loop {
            let (a, b) = (rng.unit() - 0.5, rng.unit() - 0.5);
            if a != 0.0 || b != 0.0 {
                break;
            }
        }
        let phase = rng.unit() * 10.0;
        let lean = (rng.unit() - 0.5) * (g.lean + g.lean);
        let (part, tall) = match g.num_parts {
            3 => {
                let p = i as i32 % (g.cycle + 2);
                if p > 1 {
                    (2, true)
                } else {
                    (p, false)
                }
            }
            np if np > 4 => {
                let mut p = i as i32 % (g.cycle + 4);
                while p >= 6 {
                    p -= 2;
                }
                (p, p > 3)
            }
            _ => (i as i32 % 4, false),
        };
        let (width, height, up_scale, facing);
        if g.num_parts == 3 || g.num_parts > 4 {
            pos.y = g.y_offset;
            if tall {
                width = size * g.tall_size;
                height = width;
                up_scale = rng.unit() * 0.1 + 0.05;
                pos.y +=
                    g.min_size * (g.raise[0] + rng.unit() * (g.raise[1] - g.raise[0]) * (1.0 - f));
                let mut d;
                loop {
                    d = Vec3::new(rng.unit() - 0.5, 0.0, rng.unit() - 0.5);
                    if d.x.abs() >= 0.01 || d.z.abs() >= 0.01 {
                        break;
                    }
                }
                let mut d = d.normalize();
                d.y = (rng.unit() - 0.5) * 0.3;
                facing = Some(d.normalize());
            } else {
                height = size;
                width = size * g.thin;
                up_scale = rng.unit() * 0.2 + 0.8;
                facing = None;
            }
        } else {
            width = size * 1.1;
            height = size * 0.9;
            up_scale = 1.0;
            pos.y = g.y_offset - g.min_size * (1.0 - f) * 0.5;
            facing = None;
        }
        let uv = g
            .atlas
            .get(part.max(0) as usize)
            .or(g.atlas.first())
            .copied()
            .unwrap_or([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        sprites.push(GrassSprite {
            pos,
            width,
            height,
            up_scale,
            lean,
            phase,
            index: i as u32,
            tall: facing,
            uv,
            color: [grey, grey, grey, g.alpha],
        });
    }
    let b = bound(&sprites);
    (sprites, b)
}

/// The template's radius (+0x5c): the sprites' box — x and z ± width/2, y ±
/// width — its centre, and the largest distance from it plus a width.
fn bound(sprites: &[GrassSprite]) -> f32 {
    let (mut lo, mut hi) = (Vec3::splat(100.0), Vec3::splat(-100.0));
    for s in sprites {
        let e = Vec3::new(s.width * 0.5, s.width, s.width * 0.5);
        lo = lo.min(s.pos - e);
        hi = hi.max(s.pos + e);
    }
    let c = (lo + hi) * 0.5;
    sprites
        .iter()
        .map(|s| (s.pos - c).length() + s.width)
        .fold(0.0, f32::max)
}

/// A patch's matrix at (x, z) on `ground` (see the module doc).
pub fn patch_matrix(
    x: f32,
    z: f32,
    stiffness: f32,
    bf2: bool,
    ground: &core::heightfield::Heightfield,
) -> Mat4 {
    let (e, d) = if bf2 { (4.0, 3.0) } else { (3.0, 2.25) };
    let a = Vec3::new(x, ground.height_at(x, z + e), z + e);
    let b = Vec3::new(x - d, ground.height_at(x - d, z - d), z - d);
    let c = Vec3::new(x + d, ground.height_at(x + d, z - d), z - d);
    let y = (a.y + b.y + c.y) * 0.333_333_34;
    let mut up = (a - (b + c) * 0.5).cross(c - b).normalize();
    if stiffness > 0.0 {
        up = up * (1.0 - stiffness) + Vec3::Y * stiffness;
    }
    let right = Vec3::NEG_Z.cross(up).normalize();
    let fwd = right.cross(up).normalize();
    Mat4::from_cols(
        right.extend(0.0),
        up.extend(0.0),
        fwd.extend(0.0),
        Vec3::new(x, y, z).extend(1.0),
    )
}

/// The scene's grass: one [`GrassClass`] per baked class (its texture from
/// `texture`, `None` when the map lacks it) and every patch — the FOLG
/// `GrassPatch` records standing on the terrain, then the world instances of
/// a grass class.
pub fn build(
    scene: &Scene,
    records: &[Record],
    mut texture: impl FnMut(&str) -> anyhow::Result<Option<TextureHandle>>,
) -> anyhow::Result<(Vec<GrassClass>, Vec<GrassPatch>)> {
    let Some(f) = &scene.manifest.foliage else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut classes = Vec::with_capacity(f.grass.len());
    let mut index: HashMap<String, u32> = HashMap::new();
    for (k, g) in f.grass.iter().enumerate() {
        let (sprites, bound) = template(g);
        classes.push(GrassClass {
            texture: texture(&g.texture)?,
            sprites,
            tall_lean: g.tall_lean,
            still: g.still,
            bound,
        });
        index.entry(g.name.to_ascii_lowercase()).or_insert(k as u32);
    }
    let class_of = |name: &str| index.get(&name.to_ascii_lowercase()).copied();
    let mut patches = Vec::new();
    if let Some(ground) = scene.render_heightfield() {
        let bf2 = scene.manifest.source_game == "bf2";
        for r in records {
            let Some(mesh) = f.meshes.get(r.mesh as usize) else {
                continue;
            };
            if mesh.kind != FoliageKind::GrassPatch {
                continue;
            }
            let Some(class) = mesh.name.as_deref().and_then(class_of) else {
                continue;
            };
            patches.push(GrassPatch {
                class,
                matrix: patch_matrix(r.x, r.z, mesh.stiffness, bf2, &ground),
                color: r.rgba.map(|c| c as f32 / 255.0),
                distance: mesh.distance,
            });
        }
    }
    for inst in &scene.manifest.instances {
        let Some(class) = class_of(&inst.class) else {
            continue;
        };
        patches.push(GrassPatch {
            class,
            matrix: super::instance_matrix(inst.rotation, inst.translation),
            color: [1.0; 4],
            distance: f.grass[class as usize].max_distance,
        });
    }
    Ok((classes, patches))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A class as the disc spells it (`classes.ron`), with its atlas's part
    /// count.
    fn class(
        name: &str,
        particles: u32,
        sizes: [f32; 2],
        radii: [f32; 2],
        parts: i32,
    ) -> ManifestGrassClass {
        ManifestGrassClass {
            name: name.into(),
            texture: name.into(),
            particles,
            min_size: sizes[0],
            max_size: sizes[1],
            y_offset: 0.0,
            alpha: 1.0,
            max_distance: 40.0,
            radius_fade_min: radii[0],
            radius_fade_max: radii[1],
            darkness_min: 0.6,
            darkness_max: 1.0,
            num_parts: parts,
            rect: [-1.0, -1.0],
            raise: [0.1, 0.5],
            tall_size: 2.0,
            tall_lean: 0.2,
            shadow: 0.0,
            lean: 0.1,
            cycle: 6,
            thin: 0.3,
            still: true,
            atlas: vec![
                [[0.0; 2]; 4];
                match parts {
                    1 | 4 => 4,
                    3 => 3,
                    _ => 6,
                }
            ],
        }
    }

    fn census(sprites: &[GrassSprite], atlas: &ManifestGrassClass) -> (Vec<usize>, usize) {
        let mut parts = vec![0; atlas.atlas.len()];
        for i in 0..sprites.len() {
            let p = match atlas.num_parts {
                3 => (i % 8).min(2),
                np if np > 4 => {
                    let mut p = i % 10;
                    while p >= 6 {
                        p -= 2;
                    }
                    p
                }
                _ => i % 4,
            };
            parts[p] += 1;
        }
        (parts, sprites.iter().filter(|s| s.tall.is_some()).count())
    }

    /// The template against the RAM templates of re-foliage §7 (5 classes
    /// read out of PCSX2 states of BF1 END1 / KAS1 / NAB1): every sprite
    /// inside `RadiusFadeMax`; a `NumParts` 4 / 1 sprite 1.1 × size wide and
    /// 0.9 × size high, sunk by at most `MinSize`/2; the parts {0..3} equal
    /// for 4 and 1; END1 `end_fern` (NumParts 3, 25) {0: 4, 1: 3, 2: 18} with
    /// the 18 tall; NAB1 `nab1_grass` (NumParts 6, 60) {0..3: 6, 4: 18, 5: 18}
    /// with 36 tall.
    #[test]
    fn templates_match_the_ps2_s_ram_statistics() {
        // KAS1 kas_prop_grass (NumParts 4 by default) and kas_prop_reeds (1).
        for g in [
            class("kas_prop_grass", 60, [1.2, 1.6], [3.0, 4.0], 4),
            class("kas_prop_reeds", 60, [1.3, 1.7], [4.0, 5.0], 1),
        ] {
            let (s, bound) = template(&g);
            assert_eq!(s.len(), 60);
            for p in &s {
                assert!(p.pos.x.hypot(p.pos.z) <= g.radius_fade_max, "{p:?}");
                let size = p.height / 0.9;
                assert!((p.width - size * 1.1).abs() < 1e-5);
                assert!((g.min_size..=g.max_size + 1e-5).contains(&size));
                assert!(p.pos.y <= 0.0 && p.pos.y >= -g.min_size / 2.0);
                assert!(p.tall.is_none() && p.up_scale == 1.0);
                assert!((0.6..=1.0).contains(&p.color[0]));
            }
            assert_eq!(census(&s, &g).0, vec![15; 4]);
            assert!(bound > g.radius_fade_max && bound < 2.0 * (g.radius_fade_max + g.max_size));
        }
        let fern = class("end_fern", 25, [1.2, 1.8], [4.5, 5.5], 3);
        let (s, _) = template(&fern);
        assert_eq!(census(&s, &fern), (vec![4, 3, 18], 18));
        for p in s.iter().filter(|p| p.tall.is_some()) {
            let f = p.tall.unwrap();
            assert!((f.length() - 1.0).abs() < 1e-5 && f.y.abs() <= 0.16);
            assert!(p.pos.y >= fern.min_size * 0.1 - 1e-6 && p.pos.y <= fern.min_size * 0.5);
        }
        let nab = class("nab1_grass", 60, [0.9, 1.4], [2.5, 3.0], 6);
        let (s, _) = template(&nab);
        assert_eq!(census(&s, &nab), (vec![6, 6, 6, 6, 18, 18], 36));
        for p in s.iter().filter(|p| p.tall.is_none()) {
            assert!((p.width - p.height * 0.3).abs() < 1e-5 && p.pos.y == 0.0);
            assert!((0.8..=1.0).contains(&p.up_scale));
        }
    }

    /// A rectangle class (BF1/BF2 NAB2 `nab_prop_flowers`, 2.8 × 2.8)
    /// places inside it with no sink.
    #[test]
    fn a_rectangle_class_fills_its_rectangle() {
        let mut g = class("nab_prop_flowers", 35, [1.2, 1.5], [4.0, 5.0], 4);
        g.rect = [2.8, 2.8];
        let (s, _) = template(&g);
        assert!(s
            .iter()
            .all(|p| p.pos.x.abs() <= 1.4 && p.pos.z.abs() <= 1.4 && p.pos.y == 0.0));
    }

    /// The same class name gives the same template; the PS2's generator
    /// makes a number from two steps' high halves.
    #[test]
    fn the_template_is_the_same_every_run() {
        let g = class("dag1_prop_grass", 60, [1.2, 1.6], [3.0, 4.0], 4);
        assert_eq!(template(&g), template(&g));
        let mut r = GameLcg::new(0);
        let (a, b) = (
            0x3c6e_f35fu32,
            0x3c6e_f35fu32
                .wrapping_mul(0x19660d)
                .wrapping_add(0x3c6e_f35f),
        );
        let want = f32::from_bits((((a >> 16) | (b & 0xffff_0000)) >> 9) | 0x3f80_0000) - 1.0;
        assert_eq!(r.unit(), want);
    }

    /// On flat ground a patch is upright and unrotated at the height; on a
    /// slope rising along x it tilts to the three samples' normal.
    #[test]
    fn a_patch_stands_on_its_three_samples() {
        let flat = core::heightfield::Heightfield {
            size: 16,
            unit: 1.0,
            offset_x: 8.0,
            offset_z: 8.0,
            heights: vec![2.0; 256],
        };
        let m = patch_matrix(0.5, 0.5, 0.0, false, &flat);
        assert!(
            m.abs_diff_eq(Mat4::from_translation(Vec3::new(0.5, 2.0, 0.5)), 1e-5),
            "{m}"
        );
        let slope = core::heightfield::Heightfield {
            heights: (0..256).map(|i| (i % 16) as f32 * 0.5).collect(),
            ..flat
        };
        let m = patch_matrix(0.5, 0.5, 0.0, true, &slope);
        let up = m.y_axis.truncate();
        assert!(
            up.abs_diff_eq(Vec3::new(-0.5, 1.0, 0.0).normalize(), 1e-4),
            "{up}"
        );
        assert!((m.w_axis.y - slope.height_at(0.5, 0.5)).abs() < 1e-4);
    }
}
