//! Fingerprinted donor-only vehicle artwork instructions, after bind posing.
//! The TIE nose fans move together. The Hailfire's quads take the PS2's
//! diagonals and its crest loses two extra apexes; its −x missile pack turns
//! back into the mirror of its +x pack, both packs sit on the PS2 hull's rack
//! boxes, and each rack gets a donor-derived casing with a fitted perimeter,
//! keeping the donor's textured end caps. Collision, skeletons, hardpoints,
//! headers and PS2 output never change.

use anyhow::{bail, ensure, Result};
use glam::{Vec2, Vec3};
use mungefs::model::{Model, Segment};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const BOMBER: &str = "02a5e205c37cde239eb563f05b8a9c7de1fe745d88e0456c2b40ce32f702b67a";

/// One Hailfire donor's top model and its index-only instructions. Triangle
/// numbers count over all segments in order, in the donor as read.
struct Donor {
    hash: &'static str,
    /// Triangle pairs whose shared diagonal is the other one on the PS2: the
    /// same four welded vertices, split the PS2's way. Measured against each
    /// game's own PS2 model (`claude-align/hailfire`): the largest folds are
    /// the PC wheel rims (0.53 m between the diagonals, up to 8 cm off the
    /// hull) and the Xbox BF2 crest behind the racks (4 cm).
    quads: &'static [(u32, u32)],
    /// `(triangle, apex corner, target corner)`: the donors raise each side
    /// of the crest between the racks on an extra apex vertex the PS2 does
    /// not have (3 cm proud of its flat faces). Every stream copy of the apex
    /// moves onto the target corner, a neighbour on that triangle, and the
    /// faces that close up are dropped: what is left is the PS2's fan.
    apexes: &'static [(u32, u8, u8)],
}

const HAILFIRE: &[Donor] = &[
    // BF2 PC: the shared donor crest uses the same triangle locators as Xbox.
    Donor {
        hash: "0cb063cdeece779228cbe2e3dc6b245f0fa7ce34c05db0c40a31a59a5ba18229",
        quads: &[
            (504, 505),
            (512, 513),
            (532, 533),
            (576, 577),
            (667, 668),
            (687, 688),
            (689, 690),
            (691, 692),
            (808, 809),
            (952, 953),
            (966, 967),
        ],
        apexes: &[(564, 2, 1), (521, 2, 1)],
    },
    // BF1 PC, 6 segments.
    Donor {
        hash: "4bbf5501608e347bf8e9b11629f1e99d430e77df1fdbe7369071fcaa2a2f5fea",
        quads: &[
            (576, 577),
            (578, 1108),
            (618, 619),
            (627, 628),
            (666, 667),
            (704, 705),
            (774, 775),
            (787, 788),
            (809, 810),
            (813, 814),
            (823, 1115),
            (849, 851),
            (852, 853),
            (872, 873),
            (876, 877),
            (900, 901),
            (910, 911),
            (926, 927),
            (932, 1122),
            (933, 934),
            (936, 937),
            (944, 1118),
            (1068, 1069),
            (1077, 1078),
            (1136, 1137),
            (1151, 1152),
            (1164, 1165),
            (1172, 1173),
            (1200, 1201),
            (1220, 1221),
            (1226, 1227),
            (1242, 1243),
            (1246, 1247),
            (1335, 1336),
            (1337, 1338),
            (1339, 1340),
            (1341, 1342),
            (1400, 1401),
        ],
        apexes: &[(685, 2, 0), (518, 2, 1)],
    },
    // BF1 Xbox, 41 segments.
    Donor {
        hash: "fd6be09b1b9cf300136ef553e4812f2a3e83a97829072c45ea958922e88b2956",
        quads: &[(642, 643), (837, 838), (884, 885)],
        apexes: &[(661, 1, 2), (685, 1, 2)],
    },
    // BF2 Xbox, 4 segments (the PC1 profile's BF2 side too).
    Donor {
        hash: "e2ac2642593e5f14fe20a63c674157e0cf41f09133b9bd9a30911cc8dbb8bcd3",
        quads: &[
            (504, 505),
            (512, 513),
            (532, 533),
            (576, 577),
            (667, 668),
            (687, 688),
            (689, 690),
            (691, 692),
            (808, 809),
            (952, 953),
            (966, 967),
        ],
        apexes: &[(564, 2, 1), (521, 2, 1)],
    },
];

/// The donors' −x missile pack is their +x pack mirrored and then turned
/// +5.625° (1/64 of a turn) about the rack axis; the PS2's two rack hulls are
/// mirror images. Turning the −x pack back about its own centre puts its tube
/// grid on the PS2 hull's box (54.1° there, 59.6° before, 54.0° after).
const PACK_TURN_DEG: f32 = -5.625;

/// Then each pack (tubes and casing) moves onto its PS2 hull box, −x first:
/// both sit 16–17 mm high, and the −x one 10 mm out, in the measured donors
/// (the mean of the four corner offsets between the turned pack's
/// minimum-area box and the hull's box, the same to 0.1 mm per donor). The
/// +x hull box is 17 mm wider than its pack; the fingerprinted perimeter
/// offsets resolve that width and the remaining rack-mount gap.
const PACK_SHIFT: [[f32; 2]; 2] = [[-0.0103, -0.0160], [0.0020, -0.0171]];

#[derive(Clone, Copy)]
enum Recipe {
    Bomber,
    Hailfire(&'static Donor),
}

pub(crate) struct Instructions(HashMap<String, Recipe>);
impl Instructions {
    pub(crate) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = HashMap::new();
        if let Some(original) = original {
            for c in mungefs::content_chunks(mungefs::open(original)?)? {
                if c.tag != *b"modl" {
                    continue;
                }
                let Some(name) = c.child_string(b"NAME") else {
                    continue;
                };
                if !name.eq_ignore_ascii_case("imp_fly_tiebomber")
                    && !name.eq_ignore_ascii_case("cis_tread_hailfire")
                {
                    continue;
                }
                let hash = format!("{:x}", Sha256::digest(c.payload));
                if name.eq_ignore_ascii_case("imp_fly_tiebomber") && hash == BOMBER {
                    selected.insert(name.to_ascii_lowercase(), Recipe::Bomber);
                } else if name.eq_ignore_ascii_case("cis_tread_hailfire") {
                    if let Some(donor) = HAILFIRE.iter().find(|d| d.hash == hash) {
                        selected.insert(name.to_ascii_lowercase(), Recipe::Hailfire(donor));
                    }
                }
            }
        }
        Ok(Self(selected))
    }
    pub(crate) fn apply(&self, model: &mut Model) -> Result<()> {
        let Some(&recipe) = self.0.get(&model.name.to_ascii_lowercase()) else {
            return Ok(());
        };
        ensure!(
            !model.bone_space,
            "vehicle drawing instruction requires bind posing"
        );
        let mut result = model.clone();
        match recipe {
            Recipe::Hailfire(donor) => {
                let apexes = apexes(&result, donor.apexes)?;
                diagonals(&mut result, donor.quads)?;
                collapse(&mut result, &apexes)?;
                racks(&mut result)?;
            }
            Recipe::Bomber => nose(&mut result)?,
        }
        *model = result;
        Ok(())
    }
}

/// Re-triangulate only the casing caps after the fitted perimeter has moved.
/// A fan from one corner can cross a scalloped edge; ears derived from the
/// player's corrected ring keep every cap triangle inside that ring.
impl Instructions {
    pub(crate) fn finish(&self, model: &mut Model) -> Result<()> {
        if !matches!(
            self.0.get(&model.name.to_ascii_lowercase()),
            Some(Recipe::Hailfire(_))
        ) {
            return Ok(());
        }
        for segment in model.segments.iter_mut().rev().take(2) {
            let n = 64;
            ensure!(
                segment.triangles.len() == 4 * n - 4,
                "changed fitted Hailfire casing"
            );
            let side_start = segment.triangles.len() - 2 * n;
            let sides = segment.triangles[side_start..].to_vec();
            let ring: Vec<Vec2> = (0..n)
                .map(|i| Vec3::from(segment.positions[sides[2 * i][0] as usize]).truncate())
                .collect();
            let ears = cap_ears(&ring)?;
            let cap_uv = segment.uvs[segment.triangles[0][0] as usize];
            let mut caps = Vec::new();
            for front in [false, true] {
                let mut indices = Vec::new();
                for i in 0..n {
                    let side_index = if front {
                        sides[2 * i + 1][2]
                    } else {
                        sides[2 * i][0]
                    };
                    let source = side_index as usize;
                    let index = segment.positions.len() as u32;
                    segment.positions.push(segment.positions[source]);
                    segment.normals.push([0., 0., if front { 1. } else { -1. }]);
                    // Both fills use the donor casing's existing brown panel.
                    segment.uvs.push(cap_uv);
                    indices.push(index);
                }
                for &[a, b, c] in &ears {
                    caps.push(if front {
                        [indices[a], indices[b], indices[c]]
                    } else {
                        [indices[a], indices[c], indices[b]]
                    });
                }
            }
            caps.extend(sides);
            segment.vertex_count = segment.positions.len() as u32;
            segment.triangles = caps;
            segment.index_count = (segment.triangles.len() * 3) as u32;
        }
        Ok(())
    }
}

fn cap_ears(ring: &[Vec2]) -> Result<Vec<[usize; 3]>> {
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);
    let area: f32 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.perp_dot(*b))
        .sum();
    ensure!(
        ring.len() >= 3 && area.is_finite() && area > 0.,
        "fitted casing ring must be CCW"
    );
    let mut remaining: Vec<usize> = (0..ring.len()).collect();
    let mut triangles = Vec::new();
    while remaining.len() > 3 {
        let n = remaining.len();
        let ear = (0..n)
            .find(|&i| {
                let [a, b, c] = [
                    remaining[(i + n - 1) % n],
                    remaining[i],
                    remaining[(i + 1) % n],
                ];
                let turn = cross(ring[a], ring[b], ring[c]);
                // Collinear interpolants close up without changing the polygon.
                if turn.abs() < 1e-7 {
                    return true;
                }
                if turn < 0. {
                    return false;
                }
                !remaining.iter().any(|&j| {
                    j != a
                        && j != b
                        && j != c
                        && cross(ring[a], ring[b], ring[j]) > -1e-8
                        && cross(ring[b], ring[c], ring[j]) > -1e-8
                        && cross(ring[c], ring[a], ring[j]) > -1e-8
                })
            })
            .ok_or_else(|| anyhow::anyhow!("fitted Hailfire casing is not a simple CCW ring"))?;
        let [a, b, c] = [
            remaining[(ear + n - 1) % n],
            remaining[ear],
            remaining[(ear + 1) % n],
        ];
        if cross(ring[a], ring[b], ring[c]).abs() >= 1e-7 {
            triangles.push([a, b, c]);
        }
        remaining.remove(ear);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}

/// The triangle-index locator shared by the index-only instructions.
fn locate(model: &Model, t: u32) -> Result<(usize, usize)> {
    let mut t = t as usize;
    for (si, s) in model.segments.iter().enumerate() {
        if t < s.triangles.len() {
            return Ok((si, t));
        }
        t -= s.triangles.len();
    }
    bail!("changed Hailfire donor triangle count")
}

/// One resolved apex instruction: segment, apex position, target position.
type Apex = (usize, [f32; 3], [f32; 3]);

/// Each apex instruction as an [`Apex`],
/// read before any triangle changes.
fn apexes(model: &Model, list: &[(u32, u8, u8)]) -> Result<Vec<Apex>> {
    list.iter()
        .map(|&(t, a, b)| {
            let (si, ti) = locate(model, t)?;
            let s = &model.segments[si];
            ensure!(a < 3 && b < 3 && a != b, "bad Hailfire apex instruction");
            let tri = s.triangles[ti];
            let (pa, pb) = (
                s.positions[tri[a as usize] as usize],
                s.positions[tri[b as usize] as usize],
            );
            ensure!(pa != pb, "changed Hailfire donor apex {t}");
            Ok((si, pa, pb))
        })
        .collect()
}

/// Move every stream copy of each apex onto its target and drop the faces
/// that close up. Each apex must lose at least one face and keep none that
/// still reaches its old position.
fn collapse(model: &mut Model, list: &[Apex]) -> Result<()> {
    for &(si, apex, target) in list {
        let s = &mut model.segments[si];
        let mut moved = vec![false; s.positions.len()];
        for (p, m) in s.positions.iter_mut().zip(&mut moved) {
            if *p == apex {
                *p = target;
                *m = true;
            }
        }
        ensure!(moved.contains(&true), "changed Hailfire donor apex");
        let before = s.triangles.len();
        let positions = &s.positions;
        // Only faces on a moved copy can close up; any the donor already
        // carries degenerate stay as they were.
        s.triangles.retain(|t| {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            !t.iter().any(|&i| moved[i as usize]) || (a != b && b != c && a != c)
        });
        ensure!(
            s.triangles.len() < before,
            "Hailfire donor apex closed no face"
        );
        s.index_count = (s.triangles.len() * 3) as u32;
    }
    Ok(())
}

/// Re-split each listed quad along its other diagonal. Both triangles must be
/// in one segment and share an edge by stream index, so every corner keeps its
/// own normal and UV; the winding stays the pair's.
fn diagonals(model: &mut Model, quads: &[(u32, u32)]) -> Result<()> {
    for &(a, b) in quads {
        let (sa, ta) = locate(model, a)?;
        let (sb, tb) = locate(model, b)?;
        ensure!(sa == sb, "changed Hailfire donor quad {a}/{b}");
        let s = &mut model.segments[sa];
        let (x, y) = (s.triangles[ta], s.triangles[tb]);
        // x = (p, q, c) with the shared edge p→q; y walks it q→p to d.
        let Some(r) = (0..3).find(|&r| {
            let (p, q) = (x[r], x[(r + 1) % 3]);
            (0..3).any(|k| y[k] == q && y[(k + 1) % 3] == p)
        }) else {
            bail!("changed Hailfire donor quad {a}/{b}");
        };
        let (p, q, c) = (x[r], x[(r + 1) % 3], x[(r + 2) % 3]);
        let Some(&d) = y.iter().find(|&&v| v != p && v != q) else {
            bail!("degenerate Hailfire donor quad {a}/{b}");
        };
        ensure!(c != d, "degenerate Hailfire donor quad {a}/{b}");
        // The loop is p, d, q, c; the new diagonal is d–c.
        s.triangles[ta] = [p, d, c];
        s.triangles[tb] = [d, q, c];
    }
    Ok(())
}

fn nose(model: &mut Model) -> Result<()> {
    ensure!(model.segments.len() == 2, "changed TIE donor segmentation");
    let s = &mut model.segments[1];
    ensure!(s.positions.len() == 2022, "changed TIE donor vertex stream");
    let mut changed = 0;
    for p in &mut s.positions {
        // The two small nose fans and their inner rings are 0.5378 m too
        // far forward. Move those complete donor fan vertices, not the pod rims.
        let radius2 = [-1.3994_f32, 1.4006]
            .into_iter()
            .map(|x| (p[0] - x).powi(2) + (p[1] - 2.68334).powi(2))
            .fold(f32::INFINITY, f32::min);
        if p[2] < -2.2 && radius2 < 0.25_f32.powi(2) {
            p[2] += 0.5378;
            changed += 1;
        }
    }
    ensure!(changed == 74, "changed TIE donor nose channels: {changed}");
    // The brace's two upper corner welds share the nose's authored Z offset.
    // Its other six corners already agree; moving the entire brace would
    // disturb those contacts. Four stream vertices represent the two welds.
    let mut brace = 0;
    for p in &mut s.positions {
        if (p[0] - 1.11134).abs() < 0.001 && (p[1] - 1.977598).abs() < 0.001 && p[2] < -0.7 {
            p[2] += 0.5378;
            brace += 1;
        }
    }
    ensure!(brace == 4, "changed TIE donor brace welds: {brace}");
    Ok(())
}

struct Part {
    segment: usize,
    faces: Vec<usize>,
    lo: Vec3,
    hi: Vec3,
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
fn join(parent: &mut [usize], a: usize, b: usize) {
    let a = root(parent, a);
    let b = root(parent, b);
    parent[a] = b;
}
fn missiles(model: &Model) -> Vec<Part> {
    let mut out = Vec::new();
    for (si, s) in model.segments.iter().enumerate() {
        let mut parent: Vec<_> = (0..s.positions.len()).collect();
        let mut weld = HashMap::new();
        for (i, p) in s.positions.iter().enumerate() {
            let key = p.map(|x| (x * 1000.).round() as i32);
            if let Some(&j) = weld.get(&key) {
                join(&mut parent, i, j);
            } else {
                weld.insert(key, i);
            }
        }
        for t in &s.triangles {
            join(&mut parent, t[0] as usize, t[1] as usize);
            join(&mut parent, t[0] as usize, t[2] as usize);
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (fi, t) in s.triangles.iter().enumerate() {
            groups
                .entry(root(&mut parent, t[0] as usize))
                .or_default()
                .push(fi);
        }
        for faces in groups.into_values() {
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for &fi in &faces {
                for &i in &s.triangles[fi] {
                    let p = Vec3::from(s.positions[i as usize]);
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
            let center = (lo + hi) * 0.5;
            if faces.len() == 16 && center.y > 3.1 && center.x.abs() < 2.4 && hi.z - lo.z > 2. {
                out.push(Part {
                    segment: si,
                    faces,
                    lo,
                    hi,
                });
            }
        }
    }
    out
}
fn hull(mut p: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (b[0] as f64 - a[0] as f64) * (c[1] as f64 - a[1] as f64)
            - (b[1] as f64 - a[1] as f64) * (c[0] as f64 - a[0] as f64)
    };
    let mut lo = Vec::new();
    let mut hi = Vec::new();
    for &q in &p {
        while lo.len() > 1 && cross(lo[lo.len() - 2], lo[lo.len() - 1], q) <= 0. {
            lo.pop();
        }
        lo.push(q);
    }
    for &q in p.iter().rev() {
        while hi.len() > 1 && cross(hi[hi.len() - 2], hi[hi.len() - 1], q) <= 0. {
            hi.pop();
        }
        hi.push(q);
    }
    lo.pop();
    hi.pop();
    lo.extend(hi);
    lo
}
fn empty(source: &Segment) -> Segment {
    let mut s = source.clone();
    s.positions.clear();
    s.normals.clear();
    s.uvs.clear();
    s.triangles.clear();
    s.colors = None;
    s.bmap.clear();
    s.bind.clear();
    s.wght.clear();
    s.bone = None;
    s
}
fn tube_centre(p: &Part) -> Vec2 {
    Vec2::new(p.lo.x + p.hi.x, p.lo.y + p.hi.y) * 0.5
}

/// Turn the −x pack's tubes (positions and normals) by [`PACK_TURN_DEG`] about
/// the rack axis through the pack's XY box centre, require the turned grid to
/// be the +x grid's mirror image to 2 mm, tube for tube, then move both packs
/// by [`PACK_SHIFT`].
fn turn_pack(model: &mut Model, parts: &mut [Part]) -> Result<()> {
    let left: Vec<usize> = (0..parts.len())
        .filter(|&i| parts[i].lo.x + parts[i].hi.x < 0.)
        .collect();
    ensure!(left.len() == 16, "changed Hailfire donor rack ownership");
    let lo = left.iter().fold(Vec2::splat(f32::INFINITY), |a, &i| {
        a.min(parts[i].lo.truncate())
    });
    let hi = left.iter().fold(Vec2::splat(f32::NEG_INFINITY), |a, &i| {
        a.max(parts[i].hi.truncate())
    });
    let pivot = (lo + hi) * 0.5;
    let rot = Vec2::from_angle(PACK_TURN_DEG.to_radians());
    let mut moved: Vec<Vec<bool>> = model
        .segments
        .iter()
        .map(|s| vec![false; s.positions.len()])
        .collect();
    for &i in &left {
        let part = &mut parts[i];
        let s = &mut model.segments[part.segment];
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for &fi in &part.faces {
            for v in s.triangles[fi] {
                let v = v as usize;
                if !moved[part.segment][v] {
                    moved[part.segment][v] = true;
                    let p = s.positions[v];
                    let xy = pivot + rot.rotate(Vec2::new(p[0], p[1]) - pivot);
                    s.positions[v] = [xy.x, xy.y, p[2]];
                    if let Some(n) = s.normals.get_mut(v) {
                        let r = rot.rotate(Vec2::new(n[0], n[1]));
                        *n = [r.x, r.y, n[2]];
                    }
                }
                let p = Vec3::from(s.positions[v]);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        part.lo = lo;
        part.hi = hi;
    }
    // A tube vertex shared with anything outside its tube would have moved
    // that too: the donor's tubes are closed components, so none is.
    let centres = |right: bool| -> Vec<Vec2> {
        parts
            .iter()
            .filter(|p| (p.lo.x + p.hi.x > 0.) == right)
            .map(|p| {
                let c = tube_centre(p);
                if right {
                    Vec2::new(-c.x, c.y)
                } else {
                    c
                }
            })
            .collect()
    };
    let (l, r) = (centres(false), centres(true));
    let mean = |v: &[Vec2]| v.iter().copied().sum::<Vec2>() / v.len() as f32;
    let (ml, mr) = (mean(&l), mean(&r));
    for c in &l {
        let near = r
            .iter()
            .map(|m| (*c - ml - (*m - mr)).length())
            .fold(f32::INFINITY, f32::min);
        ensure!(near < 0.002, "Hailfire donor packs are not mirror images");
    }
    for part in parts.iter_mut() {
        let shift = PACK_SHIFT[usize::from(part.lo.x + part.hi.x > 0.)];
        let s = &mut model.segments[part.segment];
        let mut seen = vec![false; s.positions.len()];
        for &fi in &part.faces {
            for v in s.triangles[fi] {
                if !std::mem::replace(&mut seen[v as usize], true) {
                    let p = &mut s.positions[v as usize];
                    p[0] += shift[0];
                    p[1] += shift[1];
                }
            }
        }
        let d = Vec3::new(shift[0], shift[1], 0.);
        part.lo += d;
        part.hi += d;
    }
    Ok(())
}

/// The minimum-area rectangle around a convex ring (counter-clockwise), as a
/// counter-clockwise ring of four corners.
fn min_box(ring: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let pts: Vec<Vec2> = ring.iter().map(|&p| Vec2::from(p)).collect();
    let mut best: Option<(f32, Vec2, f32, f32, f32, f32)> = None;
    for i in 0..pts.len() {
        let u = (pts[(i + 1) % pts.len()] - pts[i]).normalize_or_zero();
        if u == Vec2::ZERO {
            continue;
        }
        let v = u.perp();
        let (mut a0, mut a1, mut b0, mut b1) = (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        );
        for p in &pts {
            a0 = a0.min(p.dot(u));
            a1 = a1.max(p.dot(u));
            b0 = b0.min(p.dot(v));
            b1 = b1.max(p.dot(v));
        }
        let area = (a1 - a0) * (b1 - b0);
        if best.is_none_or(|b| area < b.0) {
            best = Some((area, u, a0, a1, b0, b1));
        }
    }
    let (_, u, a0, a1, b0, b1) = best.expect("a ring has an edge");
    let v = u.perp();
    [(a0, b0), (a1, b0), (a1, b1), (a0, b1)]
        .into_iter()
        .map(|(a, b)| (u * a + v * b).to_array())
        .collect()
}

fn racks(model: &mut Model) -> Result<()> {
    ensure!(
        matches!(model.segments.len(), 4 | 6 | 41),
        "changed Hailfire donor segmentation"
    );
    let mut parts = missiles(model);
    ensure!(
        parts.len() == 32,
        "changed Hailfire donor missile components: {}",
        parts.len()
    );
    turn_pack(model, &mut parts)?;
    let original = model.clone();
    for (si, s) in model.segments.iter_mut().enumerate() {
        let keep: Vec<_> = s
            .triangles
            .iter()
            .enumerate()
            .filter_map(|(fi, &t)| {
                let Some(p) = parts
                    .iter()
                    .find(|p| p.segment == si && p.faces.contains(&fi))
                else {
                    return Some(t);
                };
                let zs = t.map(|i| s.positions[i as usize][2]);
                let lo = zs.into_iter().fold(f32::INFINITY, f32::min);
                let hi = zs.into_iter().fold(f32::NEG_INFINITY, f32::max);
                // Preserve the original donor's missile mouths. Their long sides
                // are hidden by the casing and are replaced by its outer surface.
                (hi - lo < 0.001 && ((lo - p.lo.z).abs() < 0.001 || (lo - p.hi.z).abs() < 0.001))
                    .then_some(t)
            })
            .collect();
        s.triangles = keep;
        s.index_count = (s.triangles.len() * 3) as u32;
    }
    for sign in [-1., 1.] {
        let group: Vec<_> = parts
            .iter()
            .filter(|p| (p.lo.x + p.hi.x) * sign > 0.)
            .collect();
        ensure!(group.len() == 16, "changed Hailfire donor rack ownership");
        let mut points = Vec::new();
        for p in &group {
            let s = &original.segments[p.segment];
            for &fi in &p.faces {
                for &i in &s.triangles[fi] {
                    points.push(s.positions[i as usize]);
                }
            }
        }
        let envelope = hull(points.iter().map(|p| [p[0], p[1]]).collect());
        ensure!(
            (6..=32).contains(&envelope.len()),
            "changed Hailfire donor rack envelope"
        );
        // The PS2 hull's rack is a box about the tube grid (1.24 × 1.18 m on
        // −x against the pack's own 1.24 × 1.18 m), so the casing is the pack's
        // minimum-area box rather than its convex envelope, whose cut corners
        // left the hull's corners 6 cm bare.
        // A fitted perimeter needs intermediate vertices at the scalloped
        // PS2 shell's changes of direction. Subdivide the donor's own box;
        // the fingerprinted offset table moves these existing interpolants.
        let corners = min_box(&envelope);
        let ring: Vec<[f32; 2]> = (0..4)
            .flat_map(|i| {
                let (a, b) = (Vec2::from(corners[i]), Vec2::from(corners[(i + 1) % 4]));
                (0..16).map(move |j| a.lerp(b, j as f32 / 16.).to_array())
            })
            .collect();
        // Recess the fill behind the donor end caps by 2 mm to avoid z-fights.
        let back = points.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min) + 0.002;
        let front = points
            .iter()
            .map(|p| p[2])
            .fold(f32::NEG_INFINITY, f32::max)
            - 0.002;
        let n = ring.len();
        let v: Vec<_> = [back, front]
            .into_iter()
            .flat_map(|z| ring.iter().map(move |p| Vec3::new(p[0], p[1], z)))
            .collect();
        let mut faces = Vec::new();
        for i in 1..n - 1 {
            faces.push([0, i + 1, i]);
            faces.push([n, n + i, n + i + 1]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push([i, j, n + j]);
            faces.push([i, n + j, n + i]);
        }
        let ymin = ring.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let ymax = ring.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        let mut s = empty(&original.segments[group[0].segment]);
        for face in faces {
            let triangle = face.map(|i| v[i]);
            // Keep every temporary cap fan triangle, including collinear
            // interpolants. Dropping only exactly-zero cross products made
            // the fingerprinted stream shape depend on float rounding. The
            // fitted cap pass replaces all these temporary fans later.
            let cap = face.iter().all(|&i| i < n) || face.iter().all(|&i| i >= n);
            let normal = if cap {
                Vec3::Z * if face[0] < n { -1. } else { 1. }
            } else {
                (triangle[1] - triangle[0])
                    .cross(triangle[2] - triangle[0])
                    .normalize_or_zero()
            };
            ensure!(normal != Vec3::ZERO, "collapsed Hailfire casing side");
            let first = s.positions.len() as u32;
            for p in triangle {
                s.positions.push(p.to_array());
                s.normals.push(normal.to_array());
                // The donor atlas's upper-right brown panel supplies the new
                // case walls. Original missile circles keep their authored UVs.
                s.uvs.push(if normal.z.abs() < 0.5 {
                    [
                        0.60 + (p.z - back) / (front - back) * 0.35,
                        0.03 + (p.y - ymin) / (ymax - ymin) * 0.09,
                    ]
                } else {
                    [0.8, 0.05]
                });
            }
            s.triangles.push([first, first + 1, first + 2]);
        }
        s.vertex_count = s.positions.len() as u32;
        s.index_count = (s.triangles.len() * 3) as u32;
        model.segments.push(s);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::model::{Aabb, RenderType};

    fn bomber(selected: usize) -> Model {
        let mut p = vec![[0.; 3]; 2022];
        p[..selected].fill([1.4006, 2.68334, -3.1041]);
        p[100..104].copy_from_slice(&[
            [1.11134, 1.977598, -1.75835],
            [1.11134, 1.977598, -1.32811],
            [1.11134, 1.977598, -1.75835],
            [1.11134, 1.977598, -1.32811],
        ]);
        let segment = Segment {
            vertex_count: 2022,
            index_count: 0,
            bf2_material: false,
            pc_geometry: true,
            material_flags: 0,
            render_type: RenderType::Absent,
            spec_power: 0,
            spec_color: 0,
            uv_anim: [0; 2],
            textures: vec![],
            bone: None,
            group: None,
            bmap: vec![],
            bind: vec![],
            wght: vec![],
            positions: p,
            normals: vec![[0., 0., -1.]; 2022],
            uvs: vec![[0.5, 0.5]; 2022],
            colors: None,
            triangles: vec![],
        };
        Model {
            name: "imp_fly_tiebomber".into(),
            bone_space: false,
            vertex_box: Aabb {
                min: [-5.; 3],
                max: [5.; 3],
            },
            vis_box: Aabb {
                min: [-5.; 3],
                max: [5.; 3],
            },
            face_count: 0,
            sphere: None,
            segments: vec![segment.clone(), segment],
        }
    }
    #[test]
    fn a_changed_channel_count_rejects_without_partial_vertex_edits() {
        let instructions = Instructions(HashMap::from([(
            "imp_fly_tiebomber".into(),
            Recipe::Bomber,
        )]));
        let mut model = bomber(73);
        let before = model.segments[1].positions.clone();
        assert!(instructions.apply(&mut model).is_err());
        assert_eq!(model.segments[1].positions, before);
    }
    #[test]
    fn a_changed_brace_count_rejects_after_nose_work_without_partial_edits() {
        let mut model = bomber(74);
        model.segments[1].positions[100][1] = 0.;
        let before = model.segments[1].positions.clone();
        let instructions = Instructions(HashMap::from([(
            "imp_fly_tiebomber".into(),
            Recipe::Bomber,
        )]));
        assert!(instructions.apply(&mut model).is_err());
        assert_eq!(model.segments[1].positions, before);
    }
    #[test]
    fn only_selected_nose_and_brace_positions_move_and_no_recipe_leaves_them_alone() {
        let mut model = bomber(74);
        let before = model.clone();
        Instructions::of(None).unwrap().apply(&mut model).unwrap();
        assert_eq!(model.segments[1].positions, before.segments[1].positions);
        Instructions(HashMap::from([(
            "imp_fly_tiebomber".into(),
            Recipe::Bomber,
        )]))
        .apply(&mut model)
        .unwrap();
        assert_eq!(model.segments[0].positions, before.segments[0].positions);
        assert_eq!(
            &model.segments[1].positions[74..100],
            &before.segments[1].positions[74..100]
        );
        assert_eq!(
            &model.segments[1].positions[104..],
            &before.segments[1].positions[104..]
        );
        assert!(
            (model.segments[1].positions[100][2] - before.segments[1].positions[100][2] - 0.5378)
                .abs()
                < 1e-6
        );
        assert_eq!(model.segments[1].normals, before.segments[1].normals);
        assert_eq!(model.segments[1].uvs, before.segments[1].uvs);
        assert!(
            (model.segments[1].positions[0][2] - before.segments[1].positions[0][2] - 0.5378).abs()
                < 1e-6
        );
        assert_eq!(model.vertex_box.min, before.vertex_box.min);
    }
    #[test]
    fn a_donor_name_without_its_fingerprint_selects_nothing() {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = tag.to_vec();
            out.extend((payload.len() as u32).to_le_bytes());
            out.extend(payload);
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
            out
        }
        let name = chunk(b"NAME", b"imp_fly_tiebomber\0");
        let model = chunk(b"modl", &name);
        let file = chunk(b"ucfb", &model);
        assert!(Instructions::of(Some(&file)).unwrap().0.is_empty());
    }

    fn quad_model(triangles: Vec<[u32; 3]>, positions: Vec<[f32; 3]>) -> Model {
        let mut m = bomber(0);
        m.name = "cis_tread_hailfire".into();
        let n = positions.len();
        let s = &mut m.segments[1];
        s.vertex_count = n as u32;
        s.positions = positions;
        s.normals = vec![[0., 0., 1.]; n];
        s.uvs = vec![[0., 0.]; n];
        s.index_count = (triangles.len() * 3) as u32;
        s.triangles = triangles;
        m.segments[0].triangles = vec![[0, 1, 2]];
        m
    }
    #[test]
    fn a_listed_quad_takes_the_other_diagonal_with_its_winding() {
        // Segment 0 holds triangle 0, so the quad is triangles 1 and 2.
        let p = vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.3], [0., 1., 0.]];
        let mut m = quad_model(vec![[0, 1, 2], [0, 2, 3]], p);
        diagonals(&mut m, &[(1, 2)]).unwrap();
        let t = &m.segments[1].triangles;
        let normal = |t: [u32; 3]| {
            let v = t.map(|i| Vec3::from(m.segments[1].positions[i as usize]));
            (v[1] - v[0]).cross(v[2] - v[0]).z
        };
        assert!(t.iter().all(|t| t.contains(&1) && t.contains(&3)), "{t:?}");
        assert!(t.iter().all(|&t| normal(t) > 0.), "{t:?}");
        // Triangles that share no edge are refused.
        let mut m = quad_model(vec![[0, 1, 2], [3, 4, 5]], vec![[0.; 3]; 6]);
        assert!(diagonals(&mut m, &[(1, 2)]).is_err());
        // Pairs in different segments are refused.
        let mut m = quad_model(vec![[0, 1, 2]], vec![[0.; 3]; 3]);
        assert!(diagonals(&mut m, &[(0, 1)]).is_err());
    }
    #[test]
    fn an_apex_closes_its_faces_onto_its_neighbour_and_keeps_old_degenerates() {
        // A fan of four faces around apex 4, plus a face the donor already
        // carries degenerate (vertices 5 and 6 coincide).
        let p = vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [0.5, 0.5, 0.2],
            [3., 3., 3.],
            [3., 3., 3.],
            [4., 3., 3.],
            [0.5, 0.5, 0.2],
        ];
        let tris = vec![[0, 1, 4], [1, 2, 4], [2, 3, 8], [3, 0, 4], [5, 6, 7]];
        let mut m = quad_model(tris, p);
        // Triangle 1 (segment 1's first) has apex at corner 2, target vertex 0.
        let a = apexes(&m, &[(1, 2, 0)]).unwrap();
        collapse(&mut m, &a).unwrap();
        let s = &m.segments[1];
        assert_eq!(s.positions[4], [0., 0., 0.]);
        assert_eq!(s.positions[8], [0., 0., 0.]);
        assert_eq!(s.triangles, vec![[1, 2, 4], [2, 3, 8], [5, 6, 7]]);
        assert_eq!(s.index_count, 9);
    }
    #[test]
    fn the_casing_box_is_the_minimum_area_rectangle_counter_clockwise() {
        let (c, s) = (0.6_f32.cos(), 0.6_f32.sin());
        let ring: Vec<[f32; 2]> = [[0., 0.], [2., 0.], [2., 1.], [0., 1.]]
            .iter()
            .map(|p| [p[0] * c - p[1] * s + 5., p[0] * s + p[1] * c - 1.])
            .collect();
        let b = min_box(&ring);
        assert_eq!(b.len(), 4);
        let area = |r: &[[f32; 2]]| {
            (0..r.len())
                .map(|i| {
                    let (a, b) = (r[i], r[(i + 1) % r.len()]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f32>()
                / 2.
        };
        assert!((area(&b) - 2.).abs() < 1e-4, "{b:?}");
        for p in &ring {
            assert!(b.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 1e-4));
        }
        // A ring whose edges give boxes of different areas: a 4 × 1 bar with
        // its corners cut at 45°. Only the bar's own sides give the 4 × 1 box;
        // the cut edges give a 3.5 × 3.5 square's worth of area.
        let bar: Vec<[f32; 2]> = [
            [0., 0.2],
            [0.2, 0.],
            [3.8, 0.],
            [4., 0.2],
            [4., 0.8],
            [3.8, 1.],
            [0.2, 1.],
            [0., 0.8],
        ]
        .iter()
        .map(|p| [p[0] * c - p[1] * s, p[0] * s + p[1] * c])
        .collect();
        let b = min_box(&bar);
        assert!((area(&b) - 4.).abs() < 1e-3, "{b:?}");
    }

    #[test]
    fn scalloped_caps_keep_the_notch_outside_and_preserve_winding() {
        let ring: Vec<Vec2> = [[0., 0.], [2., 0.], [2., 2.], [1., 1.], [0., 2.]]
            .into_iter()
            .map(Vec2::from)
            .collect();
        let triangles = cap_ears(&ring).unwrap();
        let area: f32 = triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| ring[i]);
                let twice = (b - a).perp_dot(c - a);
                assert!(twice > 0.);
                twice * 0.5
            })
            .sum();
        assert!((area - 3.).abs() < 1e-6);
        let notch = Vec2::new(1., 1.75);
        assert!(triangles.iter().all(|t| {
            let [a, b, c] = t.map(|i| ring[i]);
            (b - a).perp_dot(notch - a) < 0.
                || (c - b).perp_dot(notch - b) < 0.
                || (a - c).perp_dot(notch - c) < 0.
        }));
        let reversed: Vec<_> = ring.into_iter().rev().collect();
        assert!(cap_ears(&reversed).is_err());
    }

    #[test]
    fn fitted_caps_keep_the_panel_uv_and_do_not_rewrite_side_vertices() {
        let mut model = bomber(0);
        model.name = "cis_tread_hailfire".into();
        let mut segment = empty(&model.segments[1]);
        let corners = [Vec2::ZERO, Vec2::X * 2., Vec2::splat(2.), Vec2::Y * 2.];
        let n = 64;
        for z in [-1., 1.] {
            for i in 0..n {
                let p = corners[i / 16].lerp(corners[(i / 16 + 1) % 4], (i % 16) as f32 / 16.);
                segment.positions.push([p.x, p.y, z]);
                segment.normals.push([0., 0., z]);
                segment
                    .uvs
                    .push(if i == 0 { [0.8, 0.05] } else { [0.6, 0.03] });
            }
        }
        for i in 1..n - 1 {
            segment.triangles.push([0, (i + 1) as u32, i as u32]);
            segment
                .triangles
                .push([n as u32, (n + i) as u32, (n + i + 1) as u32]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            segment.triangles.push([i as u32, j as u32, (n + j) as u32]);
            segment
                .triangles
                .push([i as u32, (n + j) as u32, (n + i) as u32]);
        }
        let old = segment.clone();
        model.segments = vec![segment.clone(), segment];
        let instructions = Instructions(HashMap::from([(
            model.name.clone(),
            Recipe::Hailfire(&HAILFIRE[0]),
        )]));
        instructions.finish(&mut model).unwrap();
        for segment in &model.segments {
            assert_eq!(&segment.positions[..2 * n], old.positions.as_slice());
            assert_eq!(&segment.uvs[..2 * n], old.uvs.as_slice());
            assert!(segment.uvs[2 * n..].iter().all(|uv| *uv == [0.8, 0.05]));
            assert_eq!(segment.positions.len(), 4 * n);
            assert_eq!(segment.vertex_count as usize, segment.positions.len());
            assert_eq!(segment.index_count as usize, 3 * segment.triangles.len());
            // The side strip's original vertex indices and their UVs survive.
            assert_eq!(
                &segment.triangles[segment.triangles.len() - 2 * n..],
                &old.triangles[old.triangles.len() - 2 * n..]
            );
        }
    }

    /// Two 4 × 4 tube grids for [`turn_pack`]: the +x pack at `angle`, and
    /// the −x pack its mirror image turned by `left_turn` degrees. Each tube
    /// is one 0.4 mm triangle in segment 1, long in z like the donor's.
    fn packs(angle: f32, left_turn: f32) -> (Model, Vec<Part>) {
        let mut tris = Vec::new();
        let mut pos = Vec::new();
        let mut parts = Vec::new();
        let grid: Vec<Vec2> = (0..16)
            .map(|i| Vec2::new((i % 4) as f32 - 1.5, (i / 4) as f32 - 1.5) * 0.29)
            .collect();
        for side in [1., -1.] {
            let centre = Vec2::new(1.4 * side, 3.85);
            let turn = Vec2::from_angle(if side > 0. {
                angle.to_radians()
            } else {
                (-angle + left_turn).to_radians()
            });
            for g in &grid {
                let g = if side > 0. { *g } else { Vec2::new(-g.x, g.y) };
                let c = centre + turn.rotate(g);
                let first = pos.len() as u32;
                for (dx, dy, z) in [
                    (-0.0002, -0.0002, -1.3),
                    (0.0002, -0.0002, 1.3),
                    (0., 0.0002, 0.),
                ] {
                    pos.push([c.x + dx, c.y + dy, z]);
                }
                tris.push([first, first + 1, first + 2]);
                parts.push(Part {
                    segment: 1,
                    faces: vec![tris.len() - 1],
                    lo: Vec3::new(c.x - 0.0002, c.y - 0.0002, -1.3),
                    hi: Vec3::new(c.x + 0.0002, c.y + 0.0002, 1.3),
                });
            }
        }
        (quad_model(tris, pos), parts)
    }
    #[test]
    fn the_minus_x_pack_turns_back_into_the_plus_x_packs_mirror_and_both_shift() {
        let (mut m, mut parts) = packs(36., -PACK_TURN_DEG);
        let before = m.segments[1].positions.clone();
        turn_pack(&mut m, &mut parts).unwrap();
        let after = &m.segments[1].positions;
        // +x tubes only shift, by the +x offset.
        for (a, b) in before.iter().zip(after).filter(|(a, _)| a[0] > 0.) {
            assert!((b[0] - a[0] - PACK_SHIFT[1][0]).abs() < 1e-5);
            assert!((b[1] - a[1] - PACK_SHIFT[1][1]).abs() < 1e-5);
            assert_eq!(a[2], b[2]);
        }
        // Each −x tube, less its own shift, now mirrors a +x tube, less its
        // shift, to under a millimetre: the turn was about the pack's centre.
        let [[lx, ly], [rx, ry]] = PACK_SHIFT;
        let shift = Vec2::new(lx + rx, ly - ry);
        for l in parts.iter().filter(|p| p.lo.x < 0.) {
            let c = tube_centre(l) - shift;
            let near = parts
                .iter()
                .filter(|p| p.lo.x > 0.)
                .map(|p| {
                    let r = tube_centre(p);
                    (Vec2::new(-r.x, r.y) - c).length()
                })
                .fold(f32::INFINITY, f32::min);
            assert!(near < 0.001, "{near}");
        }
        // A −x pack that is already a mirror image, or turned the other way,
        // fails the mirror check instead of being turned off the hull.
        for turn in [0., PACK_TURN_DEG] {
            let (mut m, mut parts) = packs(36., turn);
            assert!(turn_pack(&mut m, &mut parts).is_err(), "{turn}");
        }
    }
}
