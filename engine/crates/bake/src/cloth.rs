//! A BF2 soldier's cloth (the Hoth snowtroopers' skirt, the clone capes) as
//! extra skinned segments of its GLB, pinned rigid to its bones - card CLOTH
//! slice 1, `analysis/re-cloth.md` [O]. The hero closure also writes an
//! optional raw-physics companion for live cloth; the GLB stays a fallback.
//!
//! Who wears it (re-cloth 2): a cloth named in the model's `gmod` INFO list is
//! worn when the side LVL also holds a class of that name with `BASE` `cloth`
//! and a `CLTH` chunk of that name; any of the three missing draws nothing (the
//! Wookiee's dreads have no class). `ClothODF` is not read by the game.
//!
//! Where (re-cloth 1, 4.4): a CLTH position is in the cloth frame; in bind
//! model space it is `v . XFRM . Bind(PRNT)` (row vectors). A pinned vertex
//! (0..F-1) is skinned to its own bone (the PRNT bone when the chunk names
//! none); a free vertex to the bone of the pinned vertex nearest it in the rest
//! mesh, weight 1. The draw (re-cloth 5): the DATA texture, two-sided (the
//! renderer draws every face two-sided), the soldier's lighting; composited
//! only when the class sets `Transparent` (Chewbacca's fur). `HardEdge` is an
//! alpha test, which the skinned path does not draw, so it is refused as
//! `crate::soldier::segment_blends` refuses the 0x02 bit.

use crate::skin_glb::SkinnedSegment;
use anyhow::{anyhow, bail, Result};
use mungefs::cloth::Cloth;
use mungefs::entity::{EntityClass, GeomModel};
use mungefs::skel::{mul_row, xform_point, RowMat4, Skeleton};
use std::path::Path;

/// fnv1a("Transparent") / fnv1a("HardEdge"): EntityClothClass flag bits 36 and
/// 35 (re-cloth 3, B2 FUN_001da5b0).
const PROP_TRANSPARENT: u32 = 0xafe3_4731;
const PROP_HARDEDGE: u32 = 0x5caf_15e1;

/// One worn cloth, ready for the GLB.
pub struct WornCloth {
    pub name: String,
    pub segment: SkinnedSegment,
    pub physics: manifest::cloth::Cloth,
}

/// The cloths `model` wears in this side LVL (`inner` = its opened payload),
/// in the gmod list's order.
pub fn worn_cloths(inner: &[u8], model: &str, skel: &Skeleton) -> Result<Vec<WornCloth>> {
    let chunks = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
    let gmods: Vec<GeomModel> = chunks
        .iter()
        .filter(|c| &c.tag == b"gmod")
        .filter_map(|c| GeomModel::parse(c).ok())
        .collect();
    // The geometry the soldier classes name: the gmod of the model's name, else
    // one listing the model as a LOD.
    let gmod = gmods
        .iter()
        .find(|g| g.name.eq_ignore_ascii_case(model))
        .or_else(|| {
            gmods
                .iter()
                .find(|g| g.lods.iter().any(|l| l.model.eq_ignore_ascii_case(model)))
        });
    let Some(gmod) = gmod.filter(|g| !g.cloths.is_empty()) else {
        return Ok(Vec::new());
    };
    let classes: Vec<EntityClass> = chunks
        .iter()
        .filter(|c| &c.tag == b"entc")
        .filter_map(|c| EntityClass::parse(c).ok())
        .filter(|c| c.base.eq_ignore_ascii_case("cloth"))
        .collect();
    let cloths = mungefs::cloth::find_cloths(inner).map_err(|e| anyhow!("{e}"))?;
    let mut out = Vec::new();
    for name in &gmod.cloths {
        let Some(class) = classes.iter().find(|c| c.name.eq_ignore_ascii_case(name)) else {
            continue;
        };
        let Some(cloth) = cloths.iter().find(|c| c.name.eq_ignore_ascii_case(name)) else {
            continue;
        };
        if flag(class, PROP_HARDEDGE) {
            bail!("cloth {name} sets HardEdge: an alpha test, and the skinned path has none");
        }
        let segment = cloth_segment(cloth, skel, flag(class, PROP_TRANSPARENT))
            .map_err(|e| anyhow!("cloth {name} on {model}: {e}"))?;
        out.push(WornCloth {
            name: cloth.name.clone(),
            segment,
            physics: physics(cloth, class)?,
        });
    }
    Ok(out)
}

/// EntityClothClass constructor/property reader, B2 FUN_001da478/001da5b0.
/// The class labels Cross/Bend gate DATA's third/second lists respectively.
fn physics(c: &Cloth, class: &EntityClass) -> Result<manifest::cloth::Cloth> {
    let value = |key: u32| {
        class
            .props
            .iter()
            .rev()
            .find(|p| p.key_hash == key)
            .map(|p| p.value.as_str())
    };
    let scalar = |key, default| -> Result<f32> {
        value(key).map_or(Ok(default), |v| v.trim().parse::<f32>().map_err(Into::into))
    };
    let mut wind_direction = [1.0, 0.0, 0.0];
    if let Some(v) = value(0x415e_2b30) {
        let angles: Vec<f32> = v
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        if angles.len() != 2 {
            bail!("cloth {}: WindDirection needs two angles", c.name);
        }
        let (s, y) = angles[0].to_radians().sin_cos();
        let (z, x) = angles[1].to_radians().sin_cos();
        wind_direction = [-s * x, y, -s * z];
    }
    let mass = scalar(0x6eb9_57f3, 1.0)?;
    if !mass.is_finite() || mass <= 0.0 {
        bail!("cloth {}: invalid ParticleMass {mass}", c.name);
    }
    Ok(manifest::cloth::Cloth {
        name: c.name.clone(),
        part: 0,
        parent: c.parent_bone.clone(),
        frame: c.xfrm,
        positions: c.positions.clone(),
        uvs: c.uvs.clone(),
        pinned: c.pinned,
        pin_bones: c.pin_bones.clone(),
        triangles: c.triangles.clone(),
        constraints: [c.stretch.clone(), c.cross.clone(), c.bend.clone()],
        enabled: [
            scalar(0x8311_cd99, 1.0)? != 0.0,
            scalar(0x94dd_79b9, 1.0)? != 0.0,
            scalar(0xde11_2858, 1.0)? != 0.0,
        ],
        shapes: c
            .shapes
            .iter()
            .map(|s| manifest::cloth::Shape {
                bone: s.bone.clone(),
                kind: s.kind,
                params: s.params,
                frame: s.rows,
            })
            .collect(),
        wind_direction,
        wind_speed: scalar(0x1390_1a72, 10.0)?,
        dampening: scalar(0xc113_3964, 0.5)?,
        drag: scalar(0xb88c_0d43, -2.0)?,
        mass,
        max_acceleration: scalar(0x4e99_4b61, 20.0)?,
    })
}

/// Write the optional live-cloth companion. Only the hero closure calls this
/// until other cloth wearers receive their own real-content acceptance.
pub fn bake_companion(lvl: &Path, model: &str, out: &Path) -> Result<usize> {
    let data = std::fs::read(lvl)?;
    let inner = mungefs::open(&data).map_err(|e| anyhow!("{e}"))?;
    let (m, skel) = crate::soldier::find_soldier(inner, model)?;
    let first = m
        .segments
        .iter()
        .filter(|s| !s.triangles.is_empty())
        .count();
    let mut cloths: Vec<_> = worn_cloths(inner, model, &skel)?
        .into_iter()
        .map(|c| c.physics)
        .collect();
    for (k, c) in cloths.iter_mut().enumerate() {
        c.part = first + k;
    }
    let path = manifest::cloth::path(out, model);
    std::fs::write(path, ron::ser::to_string_pretty(&cloths, crate::pretty())?)?;
    Ok(cloths.len())
}

/// A class flag key's value: a number, non-zero = set (the disc writes `1`).
fn flag(class: &EntityClass, key: u32) -> bool {
    class
        .props
        .iter()
        .rev()
        .find(|p| p.key_hash == key)
        .and_then(|p| p.value.trim().parse::<f32>().ok())
        .is_some_and(|v| v != 0.0)
}

fn rows4(r: &[[f32; 3]; 4]) -> RowMat4 {
    [
        [r[0][0], r[0][1], r[0][2], 0.0],
        [r[1][0], r[1][1], r[1][2], 0.0],
        [r[2][0], r[2][1], r[2][2], 0.0],
        [r[3][0], r[3][1], r[3][2], 1.0],
    ]
}

/// The cloth in bind model space, rigid-skinned (see the module header).
pub fn cloth_segment(cloth: &Cloth, skel: &Skeleton, transparent: bool) -> Result<SkinnedSegment> {
    let bind = skel.bind_matrices().map_err(|e| anyhow!("{e}"))?;
    let bone = |name: &str| {
        skel.index_of(name)
            .ok_or_else(|| anyhow!("bone {name} not in skel {}", skel.name))
    };
    let parent = bone(&cloth.parent_bone)?;
    let frame = mul_row(&rows4(&cloth.xfrm), &bind[parent]);
    let positions: Vec<[f32; 3]> = cloth
        .positions
        .iter()
        .map(|&p| xform_point(&frame, p))
        .collect();

    let pins: Vec<usize> = if cloth.pin_bones.is_empty() {
        vec![parent; cloth.pinned]
    } else {
        cloth
            .pin_bones
            .iter()
            .map(|b| bone(b))
            .collect::<Result<_>>()?
    };
    let dist2 = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>();
    let joint_of = |i: usize| -> usize {
        if i < cloth.pinned {
            return pins[i];
        }
        let p = cloth.positions[i];
        (0..cloth.pinned)
            .min_by(|&a, &b| dist2(cloth.positions[a], p).total_cmp(&dist2(cloth.positions[b], p)))
            .map_or(parent, |k| pins[k])
    };
    let joints = (0..positions.len())
        .map(|i| {
            let j = joint_of(i);
            u8::try_from(j)
                .map(|j| [j, 0, 0, 0])
                .map_err(|_| anyhow!("bone {j} over 255"))
        })
        .collect::<Result<Vec<_>>>()?;

    // Per-vertex normals as the PS2 builds them (re-cloth 4.6): the sum of
    // each triangle's (b - a) x (c - a), normalised.
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for t in &cloth.triangles {
        let [a, b, c] = t.map(|i| positions[i as usize]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &i in t {
            for k in 0..3 {
                normals[i as usize][k] += n[k];
            }
        }
    }
    for n in &mut normals {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        *n = if len > 0.0 {
            n.map(|x| x / len)
        } else {
            [0.0, 1.0, 0.0]
        };
    }
    Ok(SkinnedSegment {
        positions,
        normals,
        uvs: cloth.uvs.clone(),
        weights: vec![[1.0, 0.0, 0.0, 0.0]; joints.len()],
        joints,
        triangles: cloth.triangles.clone(),
        texture: cloth.texture.clone(),
        blend: transparent,
        spec: None,
        cloth: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::skel::SkelBone;

    fn bone(name: &str, parent: Option<usize>, t: [f32; 3]) -> SkelBone {
        SkelBone {
            name: name.into(),
            crc: mungefs::anim::crc32_lower(name),
            parent,
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t,
        }
    }

    /// Pins on their own bone, free vertices on the nearest pin's bone, the
    /// frame = XFRM . Bind(PRNT): a pin at the cloth origin lands on the PRNT
    /// bone's bind position plus XFRM's offset.
    #[test]
    fn places_and_skins_the_rest_mesh() {
        let skel = Skeleton {
            name: "m".into(),
            bones: vec![
                bone("bone_root", None, [0.0, 0.0, 0.0]),
                bone("bone_pelvis", Some(0), [0.0, 1.0, 0.0]),
                bone("bone_l_thigh", Some(1), [0.2, -0.1, 0.0]),
            ],
        };
        let cloth = Cloth {
            name: "c".into(),
            owner: "m".into(),
            parent_bone: "bone_pelvis".into(),
            xfrm: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.5, 0.0],
            ],
            texture: "t".into(),
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.9, -1.0, 0.0],
                [0.1, -1.0, 0.0],
            ],
            uvs: vec![[0.0; 2]; 4],
            pinned: 2,
            pin_bones: vec!["bone_pelvis".into(), "bone_l_thigh".into()],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            stretch: vec![],
            cross: vec![],
            bend: vec![],
            shapes: vec![],
        };
        let s = cloth_segment(&cloth, &skel, false).unwrap();
        assert_eq!(s.positions[0], [0.0, 1.5, 0.0]);
        assert_eq!(s.positions[2], [0.9, 0.5, 0.0]);
        let j: Vec<u8> = s.joints.iter().map(|j| j[0]).collect();
        assert_eq!(j, vec![1, 2, 2, 1]);
        // (1,0,0) x (0.9,-1,0) points down -z.
        assert!(s.normals[1][2] < -0.99, "{:?}", s.normals[1]);
        assert!(!s.blend);
    }
}
