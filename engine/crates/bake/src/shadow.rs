//! Authored shadow pieces beside the visual model; BF1 runtime alone draws them.
use anyhow::Result;
use std::path::Path;

pub fn write_model(
    c: &mungefs::Chunk<'_>,
    out: &Path,
    skels: &crate::hardpoints::Skeletons,
) -> Result<usize> {
    // BF2 keeps the resource but its draw vtable slot (0x6500dc) is a stub.
    if c.children_tagged(b"segm")
        .any(|s| s.child(b"MTRL").is_some_and(|m| m.payload.len() >= 25))
    {
        return Ok(0);
    }
    let pieces: Vec<_> = c.children_tagged(b"shdw").collect();
    if pieces.is_empty() {
        return Ok(0);
    }
    let name = c
        .child_string(b"NAME")
        .ok_or_else(|| anyhow::anyhow!("shadow model has no name"))?;
    // A shadow is a look, never a reason to refuse a build: this model goes
    // without one and the rest of the bake carries on.
    if pieces.iter().any(mungefs::shadow::is_skinned) {
        eprintln!("shadows: {name}: skinned across bones on this version, no shadow");
        return Ok(0);
    }
    let parsed: Result<Vec<_>, _> = pieces
        .iter()
        .map(mungefs::shadow::ShadowPiece::parse)
        .collect();
    let mut pieces: Vec<_> = match parsed {
        Ok(pieces) => pieces
            .into_iter()
            .map(|s| manifest::shadow::ShadowPiece {
                node: s.node,
                positions: s.positions,
                faces: s.faces,
            })
            .collect(),
        Err(e) => {
            eprintln!("shadows: {name}: {e}, no shadow");
            return Ok(0);
        }
    };
    if let Some(skeleton) = skels.find_model(&name, false) {
        let bind = match skeleton.bind_matrices() {
            Ok(bind) => bind,
            Err(e) => {
                eprintln!("shadows: {name}: {e}, no shadow");
                return Ok(0);
            }
        };
        for piece in &mut pieces {
            if let Some(bone) = skeleton.index_of(&piece.node) {
                for point in &mut piece.positions {
                    *point = mungefs::skel::xform_point(&bind[bone], *point);
                }
            }
        }
    }
    std::fs::create_dir_all(out.join("shadows"))?;
    crate::write_ron(&out.join("shadows").join(format!("{name}.ron")), &pieces)?;
    Ok(pieces.len())
}

/// Targeted sidecar bake: no model, texture or gameplay output is rewritten.
pub fn bake(lvl: &Path, out: &Path) -> Result<usize> {
    let bytes = std::fs::read(lvl)?;
    let chunks = mungefs::content_chunks(mungefs::open(&bytes)?)?;
    let skels = crate::hardpoints::Skeletons::of(&chunks);
    let mut names = std::collections::HashSet::new();
    let mut count = 0;
    for c in chunks.iter().filter(|c| &c.tag == b"modl") {
        if let Some(name) = c.child_string(b"NAME") {
            if names.insert(name.to_ascii_lowercase()) && write_model(c, out, &skels)? > 0 {
                count += 1;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }

    /// One closed tetrahedron on `dummyroot`, inside a model named `broken`.
    fn model_with_a_shadow() -> Vec<u8> {
        let faces = [[0u16, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let edges: Vec<(u16, u16)> = faces
            .iter()
            .flat_map(|f| (0..3).map(|i| (f[i], f[(i + 1) % 3])))
            .collect();
        let mut indices = Vec::new();
        for (i, &(a, b)) in edges.iter().enumerate() {
            let twin = edges.iter().position(|&e| e == (b, a)).unwrap() as u16;
            for n in [a, (i / 3 * 3 + (i + 1) % 3) as u16, twin, u16::MAX] {
                indices.extend(n.to_le_bytes());
            }
        }
        let positions: Vec<u8> = [[0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let shadow = [
            chunk(b"NAME", b"dummyroot\0"),
            chunk(b"INFO", &[4u32.to_le_bytes(), 48u32.to_le_bytes()].concat()),
            chunk(b"SHDV", &positions),
            chunk(b"SHDI", &indices),
        ]
        .concat();
        [chunk(b"NAME", b"broken\0"), chunk(b"shdw", &shadow)].concat()
    }

    /// CRATES.md: a shadow is never a reason to fail a bake, and that includes
    /// a skeleton whose parents loop.
    #[test]
    fn a_skeleton_that_will_not_bind_costs_only_the_shadow() {
        let bone = |name: &str, parent| mungefs::skel::SkelBone {
            name: name.into(),
            crc: 0,
            parent,
            rows: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            t: [0.; 3],
        };
        let skeleton = mungefs::skel::Skeleton {
            name: "broken".into(),
            bones: vec![bone("dummyroot", Some(1)), bone("other", Some(0))],
        };
        assert!(skeleton.bind_matrices().is_err());
        let skels = crate::hardpoints::Skeletons::from_skeleton(skeleton);
        let bytes = model_with_a_shadow();
        let model = mungefs::Chunk {
            tag: *b"modl",
            payload: &bytes,
            offset: 0,
        };
        let out = std::env::temp_dir().join(format!("openbf-shadow-skel-{}", std::process::id()));
        assert_eq!(super::write_model(&model, &out, &skels).unwrap(), 0);
        assert!(!out.join("shadows/broken.ron").exists());
    }

    /// Regression: a BF1 side from the Xbox disc or a PC install used to end the
    /// whole bake at its first shadow, so those builds could not finish.
    #[test]
    #[ignore = "requires the extracted Xbox BF1 disc and a PC BF1 install"]
    fn other_versions_of_a_bf1_side_bake_their_rigid_shadows() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let scratch = root.join("engine/artifacts/lead_tmp/combat/shadow_versions");
        let read = |dir: &std::path::Path, model: &str| -> Vec<manifest::shadow::ShadowPiece> {
            let path = dir.join("shadows").join(format!("{model}.ron"));
            ron::de::from_bytes(&std::fs::read(path).unwrap()).unwrap()
        };
        let ps2 = scratch.join("ps2");
        super::bake(
            &root.join("extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"),
            &ps2,
        )
        .unwrap();
        for (tree, lvl) in [
            ("xbox", "extracted/xbox_bf1/side/imp.lvl"),
            (
                "pc",
                "PC_swbf1_exe/Star Wars Battlefront (Classic 2004)/GameData/Data/_LVL_PC/SIDE/imp.lvl",
            ),
        ] {
            let out = scratch.join(tree);
            assert!(super::bake(&root.join(lvl), &out).unwrap() > 0, "{tree}");
            // The same vehicle's points. The versions are re-exports, a few cm apart.
            let ours = read(&out, "imp_fly_tiefighter");
            let theirs = read(&ps2, "imp_fly_tiefighter");
            assert_eq!(ours.len(), theirs.len(), "{tree}");
            for (a, b) in ours.iter().zip(&theirs) {
                assert_eq!(a.node, b.node, "{tree}");
                assert_eq!(a.positions.len(), b.positions.len(), "{tree}");
                for (p, q) in a.positions.iter().zip(&b.positions) {
                    let off = p.iter().zip(q).map(|(u, v)| (u - v).abs());
                    assert!(off.fold(0.0, f32::max) < 0.1, "{tree}: {p:?} against {q:?}");
                }
            }
            assert!(
                !out.join("shadows/imp_walk_atst.ron").exists(),
                "{tree}: a skinned walker has no rigid shadow to write"
            );
        }
    }

    #[test]
    #[ignore = "requires BF1 extracted ALL.LVL"]
    fn real_xwing_shadow_bake_is_deterministic() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let input = root.join("extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL");
        let bytes = std::fs::read(&input).unwrap();
        let chunks = mungefs::content_chunks(mungefs::open(&bytes).unwrap()).unwrap();
        let skels = crate::hardpoints::Skeletons::of(&chunks);
        let out = root.join("engine/artifacts/lead_tmp/shadows/bake");
        let c = chunks
            .iter()
            .find(|c| &c.tag == b"modl" && c.child_str(b"NAME") == Some("all_fly_xwing"))
            .unwrap();
        super::write_model(c, &out, &skels).unwrap();
        let path = out.join("shadows/all_fly_xwing.ron");
        let first = std::fs::read(&path).unwrap();
        super::write_model(c, &out, &skels).unwrap();
        assert_eq!(first, std::fs::read(&path).unwrap());
        let pieces: Vec<manifest::shadow::ShadowPiece> = ron::de::from_bytes(&first).unwrap();
        assert_eq!(pieces.len(), 9);
        assert!(pieces.iter().any(|p| p.node == "dummyroot"));
        assert!(pieces
            .iter()
            .all(|p| !p.positions.is_empty() && !p.faces.is_empty()));
    }
}
