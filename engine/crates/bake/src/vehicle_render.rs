//! Fingerprinted donor-only vehicle artwork instructions, after bind posing.
//! The TIE nose fans move together; Hailfire rack casings are constructed
//! from the donor missile envelope and keep the donor's textured end caps.
//! Collision, skeletons, hardpoints, headers and PS2 output never change.

use anyhow::{ensure, Result};
use glam::Vec3;
use mungefs::model::{Model, Segment};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const BOMBER: &str = "02a5e205c37cde239eb563f05b8a9c7de1fe745d88e0456c2b40ce32f702b67a";
const HAILFIRE: &[&str] = &[
    "4bbf5501608e347bf8e9b11629f1e99d430e77df1fdbe7369071fcaa2a2f5fea",
    "fd6be09b1b9cf300136ef553e4812f2a3e83a97829072c45ea958922e88b2956",
    "e2ac2642593e5f14fe20a63c674157e0cf41f09133b9bd9a30911cc8dbb8bcd3",
];

pub(crate) struct Instructions(HashMap<String, bool>);
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
                    selected.insert(name.to_ascii_lowercase(), false);
                } else if name.eq_ignore_ascii_case("cis_tread_hailfire")
                    && HAILFIRE.contains(&hash.as_str())
                {
                    selected.insert(name.to_ascii_lowercase(), true);
                }
            }
        }
        Ok(Self(selected))
    }
    pub(crate) fn apply(&self, model: &mut Model) -> Result<()> {
        let Some(&rack) = self.0.get(&model.name.to_ascii_lowercase()) else {
            return Ok(());
        };
        ensure!(
            !model.bone_space,
            "vehicle drawing instruction requires bind posing"
        );
        let mut result = model.clone();
        if rack {
            racks(&mut result)?;
        } else {
            nose(&mut result)?;
        }
        *model = result;
        Ok(())
    }
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
fn racks(model: &mut Model) -> Result<()> {
    ensure!(
        matches!(model.segments.len(), 4 | 6 | 41),
        "changed Hailfire donor segmentation"
    );
    let parts = missiles(model);
    ensure!(
        parts.len() == 32,
        "changed Hailfire donor missile components: {}",
        parts.len()
    );
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
        let ring = hull(points.iter().map(|p| [p[0], p[1]]).collect());
        ensure!(
            (6..=32).contains(&ring.len()),
            "changed Hailfire donor rack envelope"
        );
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
            let normal = (triangle[1] - triangle[0])
                .cross(triangle[2] - triangle[0])
                .normalize_or_zero();
            if normal == Vec3::ZERO {
                continue;
            }
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
        let instructions = Instructions(HashMap::from([("imp_fly_tiebomber".into(), false)]));
        let mut model = bomber(73);
        let before = model.segments[1].positions.clone();
        assert!(instructions.apply(&mut model).is_err());
        assert_eq!(model.segments[1].positions, before);
    }
    #[test]
    fn only_selected_nose_positions_move_and_no_recipe_leaves_them_alone() {
        let mut model = bomber(74);
        let before = model.clone();
        Instructions::of(None).unwrap().apply(&mut model).unwrap();
        assert_eq!(model.segments[1].positions, before.segments[1].positions);
        Instructions(HashMap::from([("imp_fly_tiebomber".into(), false)]))
            .apply(&mut model)
            .unwrap();
        assert_eq!(model.segments[0].positions, before.segments[0].positions);
        assert_eq!(
            &model.segments[1].positions[74..],
            &before.segments[1].positions[74..]
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
}
