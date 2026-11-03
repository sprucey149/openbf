//! The leaf patches as the scene loads them (card LEAF,
//! `analysis/re-leafpatch.md` [O]): each baked `leafpatch` class
//! ([`manifest::ManifestLeafClass`]) becomes ONE sprite template, built as
//! B1 `FUN_001b38f8` / B2 `FUN_001cbf10` build it ([`template`], §4), and
//! each leaf object a [`render::LeafObject`]. The per-view quads, the sway
//! and the fades are `render::leaf`'s.
//!
//! **The template** (per sprite i of `NumParticles`; `u()` the PS2's LCG,
//! seeded `LCG(LCG(Seed ^ 0xc71c2830))`): a point in the upper half-ball of
//! `Radius` — x, z × 1.5 drawn into the quadrant a counter cycles through
//! (across every try), redrawn until inside — then y × Height/(S·R), and
//! narrowed toward the top `taper` metres; or uniform in the class's box; or
//! the vine's branching walk ([`vine`], §4.1). Then size `Min + u·(Max −
//! Min)` (not for the vine), grey `DarkMin + u·Δ/2 − (n · sun)·Δ/2` (n the
//! point's direction, so the sunward side is lighter), phase `u·100`, part
//! `i mod 4`, and the class `Offset` added (y ÷ S). The class bound is the
//! farthest sprite from the sprites' box centre plus half a size.
//!
//! **Unseeded classes** (the YAV, DAG1 and KAS2 ones) draw a seed from the
//! global LCG once per session on the PS2, so any seed is as faithful; the
//! port takes the FNV-1a of the class name (PROVISIONAL, reversible).
//!
//! **The objects** (§2, §5): every attach entry of a placed prop (world
//! instance or path prop) at `entry × instance`, every world instance of a
//! leaf class, and every FOLG `LeafPatch` record (its patch matrix, tint and
//! `prp_` distance, B1 `FUN_003465c0` 332700–332733). An object keeps only
//! its frame's up axis (× the class's S at draw) and origin: right =
//! `normalize((0,0,−1) × up)`, forward = `normalize(right × up)`. Its tint is
//! `0.7 + 0.2·u + 0.1·u′` per channel (u shared) from the global LCG
//! (56917–56931) — the port runs one LCG seeded by the map name over the
//! objects in load order (PROVISIONAL, reversible); a FOLG record's colour
//! is its own. Its distance is the class `MaxDistance`.

use crate::assets::Scene;
use glam::{Mat4, Vec3};
use manifest::foliage_bin::Record;
use manifest::{FoliageKind, LeafLayout, ManifestLeafClass};
use render::{LeafClass, LeafObject, LeafSprite, TextureHandle};
use std::collections::HashMap;

use super::grass::fnv1a_lower;
use core::lcg::GameLcg;

/// The seed hash's XOR (58029).
const SEED_XOR: u32 = 0xc71c_2830;

/// `vine(pos, dir, n)`: B1 `FUN_001b2d38` 57666–57958 / B2 `FUN_001cb340`
/// (re-leafpatch §4.1), appending (position, size) in placement order.
fn vine(
    g: &ManifestLeafClass,
    wob: [f32; 2],
    pos: Vec3,
    dir: Vec3,
    n: i32,
    rng: &mut GameLcg,
    out: &mut Vec<(Vec3, f32)>,
) {
    let mut k = n - 1;
    if k < 0 {
        return;
    }
    let (mn, mx) = (g.min_size, g.max_size);
    let step = g.vine_step;
    let norm = |v: Vec3| v * (1.0 / (v.x * v.x + v.y * v.y + v.z * v.z).sqrt());
    let (mut pos, mut dir) = (pos, dir);
    let f = (k as f32 + 1.0) * 0.25;
    let mut size = (mn + rng.unit() * (mx - mn)) * f.min(1.0);
    out.push((pos, size));
    let [lo, hi] = g.vine_run;
    let mut run = (rng.unit() * (hi - lo) as f32 + lo as f32) as i32;
    run = run.min(k);
    while run != 0 {
        let fs = ((k as f32 + 1.0) * 0.25).min(1.0);
        k -= 1;
        run -= 1;
        let (a, b) = (rng.unit(), rng.unit());
        let dx = (a - b) * (wob[0] * 0.5);
        let (a, b) = (rng.unit(), rng.unit());
        let dz = (a - b) * (wob[1] * 0.5);
        dir = norm(dir + Vec3::new(dx, 0.2, dz));
        pos += dir * size * step;
        size = (mn + rng.unit() * (mx - mn)) * fs;
        out.push((pos, size));
    }
    if k > 0 {
        let branch = |rng: &mut GameLcg| {
            let (a, b) = (rng.unit(), rng.unit());
            let x = (a - b) * wob[0];
            let (a, b) = (rng.unit(), rng.unit());
            let z = (a - b) * wob[1];
            norm(dir + (Vec3::new(x, 0.0, z) - dir) * 0.8)
        };
        let da = branch(rng);
        let na = (k as f32 * (rng.unit() * 0.5 + 0.25)) as i32;
        vine(g, wob, pos + da * size * step, da, na, rng, out);
        let db = branch(rng);
        vine(g, wob, pos + db * size * step, db, k - na, rng, out);
    }
}

/// A class's template sprites and bound (see the module doc). `sun` is the
/// direction the sunlight travels.
pub fn template(g: &ManifestLeafClass, sun: Vec3) -> (Vec<LeafSprite>, f32) {
    let seed = g.seed.unwrap_or_else(|| fnv1a_lower(&g.name));
    let mut rng = GameLcg::new(seed ^ SEED_XOR);
    rng.step();
    rng.step();
    let n = g.particles as usize;
    let s = g.scale;
    let inv_s = 1.0 / s;
    let r = g.radius;
    let mut walk = Vec::new();
    if let LeafLayout::Vine(wob) = g.layout {
        vine(g, wob, Vec3::ZERO, Vec3::Y, n as i32, &mut rng, &mut walk);
    }
    let mut q: u32 = 0;
    let mut sprites = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y, z, size);
        match g.layout {
            LeafLayout::Vine(_) => {
                let Some(&(p, sz)) = walk.get(i) else {
                    break;
                };
                (x, y, z, size) = (p.x, p.y, p.z, Some(sz));
            }
            LeafLayout::Box(b) => {
                x = (rng.unit() - 0.5) * b[0];
                y = (rng.unit() - 0.5) * b[1];
                z = (rng.unit() - 0.5) * b[2];
                size = None;
            }
            LeafLayout::Ball => {
                let (mut px, mut py, mut pz);
                loop {
                    px = rng.unit() * r * 1.5;
                    py = rng.unit() * r;
                    pz = rng.unit() * r * 1.5;
                    if q < 2 {
                        px = -px;
                    }
                    if q.wrapping_sub(1) < 2 {
                        pz = -pz;
                    }
                    q = (q + 1) % 4;
                    // Out only while r² < d² holds (a NaN ends the loop,
                    // as the ELF's `c.lt.s` branch does).
                    let d2 = px * px + py * py + pz * pz;
                    if (r * r).partial_cmp(&d2) != Some(std::cmp::Ordering::Less) {
                        break;
                    }
                }
                py *= (g.height * inv_s) / r;
                if g.taper > 0.0 && g.height - g.taper < py {
                    let k = (g.height - py) / g.taper;
                    px *= k;
                    pz *= k;
                }
                (x, y, z, size) = (px, py, pz, None);
            }
        }
        let size = size.unwrap_or_else(|| g.min_size + rng.unit() * (g.max_size - g.min_size));
        let dd = g.darkness_max - g.darkness_min;
        let u = rng.unit() * dd;
        let l2 = x * x + y * y + z * z;
        // The VU's RSQRT of 0 gives +MAX; × 0 = 0 (a vine's root).
        let ln = if l2 > 0.0 { 1.0 / l2.sqrt() } else { 0.0 };
        let dot = x * ln * sun.x + y * ln * sun.y + z * ln * sun.z;
        let grey = (g.darkness_min + u * 0.5) - dot * dd * 0.5;
        let phase = rng.unit() * 100.0;
        sprites.push(LeafSprite {
            pos: Vec3::new(x + g.offset[0], y + g.offset[1] * inv_s, z + g.offset[2]),
            size,
            grey,
            phase,
            part: (i % 4) as u8,
        });
    }
    let b = bound(&sprites, s);
    (sprites, b)
}

/// The class bound (+0x5c): the box of (x ± h, (y ± h)·S, z ± h), h = half a
/// size, from ±100; its centre; the largest |(x, y·S, z) − centre| +
/// max(h·S, h).
fn bound(sprites: &[LeafSprite], s: f32) -> f32 {
    let (mut lo, mut hi) = (Vec3::splat(100.0), Vec3::splat(-100.0));
    for p in sprites {
        let h = p.size * 0.5;
        lo = lo.min(Vec3::new(p.pos.x - h, (p.pos.y - h) * s, p.pos.z - h));
        hi = hi.max(Vec3::new(p.pos.x + h, (p.pos.y + h) * s, p.pos.z + h));
    }
    let c = (lo + hi) * 0.5;
    sprites
        .iter()
        .map(|p| {
            let h = p.size * 0.5;
            (Vec3::new(p.pos.x, p.pos.y * s, p.pos.z) - c).length() + (h * s).max(h)
        })
        .fold(0.0, f32::max)
}

/// An object's matrix from its frame's up axis and origin (§5): right =
/// normalize((0,0,−1) × up), forward = normalize(right × up), the up column
/// × the class's vertical scale.
pub fn object_matrix(up: Vec3, pos: Vec3, scale: f32) -> Mat4 {
    let right = Vec3::NEG_Z.cross(up).normalize_or_zero();
    let fwd = right.cross(up).normalize_or_zero();
    Mat4::from_cols(
        right.extend(0.0),
        (up * scale).extend(0.0),
        fwd.extend(0.0),
        pos.extend(1.0),
    )
}

/// An object's tint (see the module doc).
fn tint(rng: &mut GameLcg) -> [f32; 4] {
    let u = rng.unit() * 0.2;
    let r = u + rng.unit() * 0.1 + 0.7;
    let g = u + rng.unit() * 0.1 + 0.7;
    let b = u + rng.unit() * 0.1 + 0.7;
    [r, g, b, 1.0]
}

/// The scene's leaves: one [`LeafClass`] per baked class (its texture from
/// `texture`, `None` when the map lacks it) and every object — attached,
/// world-placed, and FOLG-placed.
pub fn build(
    scene: &Scene,
    records: &[Record],
    mut texture: impl FnMut(&str) -> anyhow::Result<Option<TextureHandle>>,
) -> anyhow::Result<(Vec<LeafClass>, Vec<LeafObject>)> {
    let m = &scene.manifest;
    if m.leaf.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let sun = Vec3::from(m.lighting.sun_dir);
    let lit = m.source_game == "bf2";
    let mut classes = Vec::with_capacity(m.leaf.len());
    for g in &m.leaf {
        let (sprites, bound) = template(g, sun);
        classes.push(LeafClass {
            texture: texture(&g.texture)?,
            sprites,
            whole_sheet: g.num_parts == 1,
            alpha: g.alpha,
            wind: g.wind,
            bound,
            vine: matches!(g.layout, LeafLayout::Vine(_)),
            lit,
        });
    }
    let ground = scene.render_heightfield();
    let objects = objects(m, records, ground.as_ref());
    Ok((classes, objects))
}

/// How far `class`'s attached leaf patches draw: the nearest `MaxDistance`
/// of its attach entries' leaf classes, `None` when it attaches none. Past
/// it the model is a bare trunk, so the LOD switch must not keep the model
/// (`look::LodBands::at`).
pub fn attach_reach(m: &manifest::Manifest, class: &str) -> Option<f32> {
    let k = m
        .classes
        .iter()
        .find(|k| k.name.eq_ignore_ascii_case(class))?;
    k.attach
        .iter()
        .filter_map(|a| {
            m.leaf
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(&a.class))
                .map(|g| g.max_distance)
        })
        .reduce(f32::min)
}

/// Every leaf object of the manifest: attached to a placed prop (world
/// instance or path prop), a world instance of a leaf class, and — when the
/// terrain is given — every FOLG `LeafPatch` record (see the module doc).
/// `class` indexes `m.leaf`.
pub fn objects(
    m: &manifest::Manifest,
    records: &[Record],
    ground: Option<&core::heightfield::Heightfield>,
) -> Vec<LeafObject> {
    let index: HashMap<String, u32> = m
        .leaf
        .iter()
        .enumerate()
        .rev()
        .map(|(k, g)| (g.name.to_ascii_lowercase(), k as u32))
        .collect();
    let class_of = |name: &str| index.get(&name.to_ascii_lowercase()).copied();
    let props: HashMap<String, &manifest::ManifestClass> = m
        .classes
        .iter()
        .filter(|k| !k.attach.is_empty())
        .map(|k| (k.name.to_ascii_lowercase(), k))
        .collect();
    let mut rng = GameLcg::new(fnv1a_lower(&m.map));
    let mut objects = Vec::new();
    let mut place = |class: u32, up: Vec3, pos: Vec3, rng: &mut GameLcg| {
        let g = &m.leaf[class as usize];
        objects.push(LeafObject {
            class,
            matrix: object_matrix(up, pos, g.scale),
            color: tint(rng),
            distance: g.max_distance,
        });
    };
    let path_props = m.foliage.as_ref().map_or(&[][..], |f| &f.path_props[..]);
    for inst in m.instances.iter().chain(path_props) {
        let world = super::instance_matrix(inst.rotation, inst.translation);
        if let Some(c) = class_of(&inst.class) {
            place(
                c,
                Vec3::from(inst.rotation[1]),
                Vec3::from(inst.translation),
                &mut rng,
            );
        }
        let Some(k) = props.get(&inst.class.to_ascii_lowercase()) else {
            continue;
        };
        for a in &k.attach {
            let Some(c) = class_of(&a.class) else {
                continue;
            };
            let up = world.transform_vector3(Vec3::from(a.up));
            let pos = world.transform_point3(Vec3::from(a.pos));
            place(c, up, pos, &mut rng);
        }
    }
    if let (Some(f), Some(ground)) = (&m.foliage, ground) {
        let bf2 = m.source_game == "bf2";
        for r in records {
            let Some(mesh) = f.meshes.get(r.mesh as usize) else {
                continue;
            };
            if mesh.kind != FoliageKind::LeafPatch {
                continue;
            }
            let Some(class) = mesh.name.as_deref().and_then(class_of) else {
                continue;
            };
            let mut matrix = super::grass::patch_matrix(r.x, r.z, mesh.stiffness, bf2, ground);
            matrix.y_axis *= m.leaf[class as usize].scale;
            objects.push(LeafObject {
                class,
                matrix,
                color: r.rgba.map(|c| c as f32 / 255.0),
                distance: mesh.distance,
            });
        }
    }
    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RAM vectors the RE read out of 9 PCSX2 states
    /// (`lead_tmp/re/leaf/vectors/`, gitignored).
    fn vectors(file: &str) -> Option<serde_json::Value> {
        let p = format!(
            "{}/../../artifacts/lead_tmp/re/leaf/vectors/{file}",
            env!("CARGO_MANIFEST_DIR")
        );
        let s = match pak::read_to_string(&p) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("SKIP: {p} not present (0 vector sets tested, 1 skipped)");
                return None;
            }
            Err(e) => panic!("read {p}: {e}"),
        };
        Some(serde_json::from_str(&s).unwrap_or_else(|e| panic!("parse {p}: {e}")))
    }

    fn f(v: &serde_json::Value) -> f32 {
        v.as_f64().unwrap() as f32
    }

    fn v3(v: &serde_json::Value) -> Vec3 {
        Vec3::new(f(&v[0]), f(&v[1]), f(&v[2]))
    }

    /// A class from the RAM fields (`ram.cls_fields`).
    fn class_of(name: &str, fl: &serde_json::Value) -> ManifestLeafClass {
        let layout = match fl["mode"].as_i64().unwrap() {
            2 => LeafLayout::Box(v3(&fl["box"]).to_array()),
            3 => LeafLayout::Vine([f(&fl["vine"][0]), f(&fl["vine"][1])]),
            _ => LeafLayout::Ball,
        };
        ManifestLeafClass {
            name: name.into(),
            texture: String::new(),
            falling_count: 0,
            bird_count: 0,
            particles: fl["n"].as_u64().unwrap() as u32,
            min_size: f(&fl["minsize"]),
            max_size: f(&fl["maxsize"]),
            radius: f(&fl["radius"]),
            height: f(&fl["height"]),
            scale: f(&fl["S"]),
            offset: v3(&fl["offset"]).to_array(),
            taper: f(&fl["taper"]),
            seed: Some(fl["seed"].as_u64().unwrap() as u32),
            layout,
            vine_run: [
                fl["fc_100"][0].as_i64().unwrap() as i32,
                fl["fc_100"][1].as_i64().unwrap() as i32,
            ],
            vine_step: f(&fl["f104"]),
            darkness_min: f(&fl["dark"][0]),
            darkness_max: f(&fl["dark"][1]),
            alpha: f(&fl["alpha"]),
            max_distance: f(&fl["maxdist"]),
            num_parts: fl["parts"].as_i64().unwrap() as i32,
            wind: [f(&fl["wind"][0]), f(&fl["wind"][1])],
        }
    }

    /// Acceptance (1) of card LEAF: every live class template in 9 PCSX2
    /// states of both games (23 classes, incl. both NAB2 vines) — positions
    /// ≤ 1e-4 m, sizes and greys ≤ 1e-5, parts exact, the phases, and the
    /// class bound. Data-gated.
    #[test]
    fn templates_reproduce_the_ps2_s_ram() {
        let Some(t) = vectors("templates.json") else {
            return;
        };
        let mut n = 0;
        for (state, st) in t.as_object().unwrap() {
            let sun = v3(&st["sun"]);
            for (name, c) in st["classes"].as_object().unwrap() {
                let g = class_of(name, &c["fields"]);
                let (sprites, bound) = template(&g, sun);
                let live = c["sprites"].as_array().unwrap();
                assert_eq!(sprites.len(), live.len(), "{state} {name}");
                let (mut dp, mut ds, mut dg, mut dph) = (0f32, 0f32, 0f32, 0f32);
                for (p, l) in sprites.iter().zip(live) {
                    dp = dp.max((p.pos - v3(&l["pos"])).abs().max_element());
                    ds = ds.max((p.size - f(&l["size"])).abs());
                    dg = dg.max((p.grey - f(&l["rgba"][0])).abs());
                    dph = dph.max((p.phase - f(&l["phase0"])).abs());
                    assert_eq!(p.part as u64, l["part"].as_u64().unwrap(), "{state} {name}");
                }
                let db = (bound - f(&c["fields"]["bound"])).abs();
                eprintln!("{state} {name}: pos {dp:.1e} size {ds:.1e} grey {dg:.1e} phase {dph:.1e} bound {db:.1e}");
                assert!(dp <= 1e-4 && ds <= 1e-5 && dg <= 1e-5, "{state} {name}");
                assert!(dph <= 1e-2 && db <= 1e-4, "{state} {name}");
                n += 1;
            }
        }
        assert_eq!(n, 23);
    }

    /// Every live object's matrix (2,883 in 9 states) is its own up axis and
    /// origin put through [`object_matrix`]: the right and forward rows are
    /// the PS2's (§5).
    #[test]
    fn object_frames_are_the_ps2_s() {
        let Some(o) = vectors("objects.json") else {
            return;
        };
        let mut n = 0;
        for (state, objs) in o.as_object().unwrap() {
            for ob in objs.as_array().unwrap() {
                let rows = &ob["rows"];
                let m = object_matrix(v3(&rows[1]), v3(&rows[3]), 1.0);
                let e = (m.x_axis.truncate() - v3(&rows[0]))
                    .abs()
                    .max_element()
                    .max((m.z_axis.truncate() - v3(&rows[2])).abs().max_element());
                assert!(e < 1e-5, "{state} {}: {e}", ob["cls"]);
                n += 1;
            }
        }
        assert!(n > 2800, "{n}");
    }

    /// Acceptance (1) of card LEAF: every live leaf object in 9 PCSX2 states
    /// (3,078; re-leafpatch §2.3) is one [`objects`] predicts from the baked
    /// manifest — same class, origin within 1 cm, up within 1e-3 — and the
    /// port places no leaf object the PS2 lacks. Data-gated on the vectors
    /// and a bake with leaves (`OPENBF_LEAF_BAKE`, default the shared one).
    #[test]
    fn objects_are_the_ps2_s_live_ones() {
        let Some(o) = vectors("objects.json") else {
            return;
        };
        let bake = std::env::var("OPENBF_LEAF_BAKE")
            .unwrap_or_else(|_| format!("{}/../../assets_baked", env!("CARGO_MANIFEST_DIR")));
        let (mut tested, mut skipped) = (0, 0);
        for (state, live) in o.as_object().unwrap() {
            let map: Vec<&str> = state.splitn(3, '_').take(2).collect();
            let path = format!("{bake}/{}/manifest.ron", map.join("_"));
            let Ok(text) = pak::read_to_string(&path) else {
                eprintln!("SKIP: no {path}");
                skipped += 1;
                continue;
            };
            let m: manifest::Manifest = ron::from_str(&text).unwrap();
            if m.leaf.is_empty() {
                eprintln!("SKIP: {path} predates LEAF");
                skipped += 1;
                continue;
            }
            let ours = objects(&m, &[], None);
            let mut used = vec![false; ours.len()];
            let live = live.as_array().unwrap();
            for l in live {
                let (name, rows) = (l["cls"].as_str().unwrap(), &l["rows"]);
                let (p, up) = (v3(&rows[3]), v3(&rows[1]));
                let hit = ours.iter().enumerate().position(|(i, ob)| {
                    let g = &m.leaf[ob.class as usize];
                    !used[i]
                        && g.name.eq_ignore_ascii_case(name)
                        && (ob.matrix.w_axis.truncate() - p).length() < 0.01
                        && (ob.matrix.y_axis.truncate() / g.scale - up).length() < 1e-3
                });
                let Some(i) = hit else {
                    panic!("{state}: live {name} at {p} not predicted");
                };
                used[i] = true;
            }
            eprintln!("{state}: {} live, {} predicted", live.len(), ours.len());
            assert_eq!(live.len(), ours.len(), "{state}");
            tested += 1;
        }
        assert!(tested + skipped > 0, "empty leaf object vectors");
        eprintln!("leaf objects: {tested} states tested, {skipped} skipped");
    }
}
