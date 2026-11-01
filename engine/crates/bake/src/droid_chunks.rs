//! [ours] BF1's SBD destruction on every map, plus five cuts of the Magnaguard's
//! own mesh (issue #80). Profiles, materials and launch frames come from the discs.
use anyhow::{ensure, Context, Result};
use glam::{Mat4, Vec3};
use manifest::{ClassDef, DroidChunkManifest, ManifestCollision, ManifestPrim};
use mungefs::{fnv1a_lowercase as hash, model::Model, skel::Skeleton};
use std::path::Path;

/// Generate only the droid overlay; never modifies an input LVL. `pc_cis` is
/// the PC install's CIS side LVL for the BF2 battle droid chunks' tier 2.
pub fn bake(bf1: &str, bf2: &str, common: &str, out: &Path, pc_cis: Option<&str>) -> Result<()> {
    let bf1_resolved = crate::resolve_ci(bf1);
    let bf2_resolved = crate::resolve_ci(bf2);
    let common_resolved = crate::resolve_ci(common);
    let pc_cis_resolved = pc_cis.map(crate::resolve_ci);
    let common = common_resolved.as_str();
    let pc_cis = pc_cis_resolved.as_deref();
    let b1 = std::fs::read(&bf1_resolved)?;
    let b2 = std::fs::read(&bf2_resolved)?;
    let c1 = mungefs::content_chunks(mungefs::open(&b1)?)?;
    let c2 = mungefs::content_chunks(mungefs::open(&b2)?)?;
    let classes = crate::classes::read_classes(
        &b1,
        &[b"entc", b"expc"],
        crate::classes::MalformedClass::Strict,
    )?;
    let donor = classes
        .iter()
        .find(|c| c.name == "cis_inf_battledroid")
        .context("BF1 SBD class")?;
    let wanted = ["ChunkFrequency", "ExplosionName"].map(hash);
    let mut props: Vec<_> = donor
        .props
        .iter()
        .filter(|(k, _)| wanted.contains(k))
        .cloned()
        .collect();
    let start = donor
        .props
        .iter()
        .position(|(k, _)| *k == hash("CHUNKSECTION"))
        .context("BF1 SBD chunk sections")?;
    // Section keys end before the soldier's sound properties.
    let chunk_keys = [
        "CHUNKSECTION",
        "ChunkGeometryName",
        "ChunkNodeName",
        "ChunkTerrainCollisions",
        "ChunkTerrainEffect",
        "ChunkTrailEffect",
        "ChunkSmokeEffect",
        "ChunkPhysics",
        "ChunkOmega",
        "ChunkSpeed",
        "ChunkUpFactor",
        "ChunkVelocityFactor",
        "ChunkLifetime",
        "ChunkBounciness",
        "ChunkStickiness",
        "ChunkGravity",
        "ChunkLinearDamping",
        "ChunkAngularDamping",
    ]
    .map(hash);
    props.extend(
        donor.props[start..]
            .iter()
            .take_while(|(k, _)| chunk_keys.contains(k))
            .cloned(),
    );
    ensure!(
        props
            .iter()
            .filter(|(k, _)| *k == hash("ChunkGeometryName"))
            .count()
            == 5,
        "BF1 SBD must author five chunks"
    );
    // Aliases keep the donor's sheets/rules from replacing unrelated BF2 effects.
    for (k, v) in &mut props {
        if *k == hash("ExplosionName") {
            *v = "openbf_droid_exp".into();
        }
        if *k == hash("ChunkTrailEffect") || *k == hash("ChunkTerrainEffect") {
            *v = format!("openbf_droid_{v}");
        }
    }
    let profile = |name: &str, props| ClassDef {
        name: name.into(),
        kind: "chunk_profile".into(),
        base: String::new(),
        props,
    };
    let mut magna_props = props.clone();
    for (k, v) in &mut magna_props {
        if *k == hash("ChunkGeometryName") {
            *v = v.replace("sbdroid", "magnaguard");
        }
    }
    let mut explosion = classes
        .iter()
        .find(|c| c.name == "com_inf_droid_exp")
        .context("BF1 droid explosion")?
        .clone();
    explosion.name = "openbf_droid_exp".into();
    for (k, v) in &mut explosion.props {
        if *k == hash("Effect") {
            *v = "openbf_droid_explosion".into();
        }
    }
    crate::write_ron(
        &out.join("classes/droid_chunks.ron"),
        &vec![
            profile("cis_inf_sbdroid", props),
            profile("cis_inf_magnaguard", magna_props),
            explosion,
        ],
    )?;

    for (game, chunks) in [("bf1", &c1), ("bf2", &c2)] {
        let side = out.join(format!("side_{game}/droid_chunks"));
        let mut meta = DroidChunkManifest::default();
        let skels = crate::hardpoints::Skeletons::of(chunks);
        for model in ["cis_inf_sbdroid", "cis_inf_magnaguard"] {
            if let Some(skel) = skels.find(model) {
                meta.nodes.push((model.into(), crate::hardpoints::of(skel)));
            }
        }
        // The battle droid's own chunks, from this game's disc. Baked here
        // rather than by `bake weapon`, which would overwrite the living
        // droid's sheet with an opaque copy.
        let (lvl, native, pc) = if game == "bf1" {
            (bf1, &c1, None)
        } else {
            (bf2, &c2, pc_cis)
        };
        for n in 1..=5 {
            aliased_chunk(lvl, native, &format!("bdroid_chunk{n}"), &side, pc)?;
        }
        // The same BF1 models and prims on both games' maps.
        for n in 1..=5 {
            let model = format!("sbdroid_chunk{n}");
            aliased_chunk(bf1, &c1, &model, &side, None)?;
            let prim = c1
                .iter()
                .filter(|c| &c.tag == b"prim")
                .map(mungefs::coll::Primitives::parse)
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .find(|p| p.model == model)
                .with_context(|| format!("{model} prim"))?;
            let (mut prims, unknown) = crate::map::manifest_prims(&prim);
            ensure!(
                unknown == 0 && prims.len() == 1,
                "{model}: one supported prim"
            );
            if let Some(skel) = crate::hardpoints::Skeletons::of(&c1).find(&model) {
                crate::bind_pose::prims_at_bind(skel, &mut prims);
            }
            meta.collision.push(ManifestCollision {
                geometry: model,
                node: String::new(),
                glb: None,
                prims,
            });
        }
        if game == "bf2" {
            let (body, skel) =
                crate::soldier::find_soldier(mungefs::open(&b2)?, "cis_inf_magnaguard")?;
            let pieces = cut_magnaguard(&body, &skel)?;
            std::fs::create_dir_all(side.join("weapons"))?;
            for (m, prim) in pieces {
                std::fs::write(
                    side.join(format!("weapons/{}.glb", m.name)),
                    crate::glb::model_to_glb(&m, &crate::alpha::legacy_per_texture_or(&m), false)?,
                )?;
                meta.collision.push(ManifestCollision {
                    geometry: m.name,
                    node: String::new(),
                    glb: None,
                    prims: vec![prim],
                });
            }
            // The cuts draw the living Magnaguard's own sheets from
            // side_bf2/textures (the soldier bake's, both tiers).
        }
        crate::write_ron(&side.join("droid_chunks.ron"), &meta)?;
        for entry in std::fs::read_dir(side.join("textures"))? {
            let name = entry?.file_name();
            ensure!(
                name.to_string_lossy().starts_with("openbf_droid_"),
                "{game} droid chunks: {name:?} would shadow a living soldier's sheet"
            );
        }
    }
    // The BF1 explosion and chunk trail retain BF1's particle rules and sheets.
    let bytes = std::fs::read(common)?;
    let inner = mungefs::open(&bytes)?;
    let chunks = mungefs::content_chunks(inner)?;
    let mut fx = crate::effects::Collected::from_chunks(chunks.iter())?;
    let wanted = ["droid_explosion", "chunksmoke", "dirtspray"].map(hash);
    fx.effects.retain(|e| wanted.contains(&e.name_hash));
    fx.beams.clear();
    ensure!(
        fx.effects.len() == 3,
        "BF1 droid explosion, trail and terrain effects"
    );
    let dir = out.join("droid_effects");
    let textures = fx.textures();
    let flags = textures
        .iter()
        .map(|t| (t.clone(), crate::detail::ALPHA_IS_MEANINGFUL))
        .collect();
    let have = crate::soldier::bake_textures_flagged(inner, &textures, &flags, &dir, None)?;
    ensure!(have.len() == textures.len(), "droid effect sheets missing");
    ensure!(fx.models().is_empty(), "droid effects require model baking");
    let mut set = fx
        .write(&dir, "bf1", &have, &[])?
        .context("droid effect set")?;
    for effect in &mut set.effects {
        let original = ["droid_explosion", "chunksmoke", "dirtspray"]
            .into_iter()
            .find(|name| hash(name) == effect.hash)
            .context("known donor effect hash")?;
        let name = if original == "droid_explosion" {
            "openbf_droid_explosion".into()
        } else {
            format!("openbf_droid_{original}")
        };
        effect.hash = hash(&name);
        effect.name = Some(name);
    }
    fn alias_sheets(em: &mut manifest::FxEmitter) {
        if let Some(texture) = &mut em.geometry.texture {
            *texture = format!("openbf_droid_{}", texture.to_ascii_lowercase());
        }
        if let Some(child) = &mut em.geometry.child {
            alias_sheets(child);
        }
        if let Some(sibling) = &mut em.sibling {
            alias_sheets(sibling);
        }
    }
    for effect in &mut set.effects {
        for em in &mut effect.emitters {
            alias_sheets(em);
        }
    }
    for texture in have {
        std::fs::rename(
            dir.join(format!("textures/{texture}.ktx2")),
            dir.join(format!("textures/openbf_droid_{texture}.ktx2")),
        )?;
    }
    crate::write_ron(&dir.join("effects.ron"), &set)?;
    Ok(())
}

/// Bake one chunk model to `side` with every sheet it writes renamed
/// `openbf_droid_<name>`, so it can never replace a living soldier's sheet.
fn aliased_chunk(
    lvl: &str,
    chunks: &[mungefs::Chunk<'_>],
    model: &str,
    side: &Path,
    pc: Option<&str>,
) -> Result<()> {
    let baked = crate::soldier::bake_weapon(lvl, model, side.to_str().context("output path")?, pc)?;
    let mesh = chunks
        .iter()
        .filter(|c| &c.tag == b"modl")
        .map(Model::parse)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .find(|m| m.name.eq_ignore_ascii_case(model))
        .with_context(|| format!("{model} mesh"))?;
    let mut mesh = if mesh.bone_space {
        crate::bind_pose::BindPose::of(chunks)
            .pose(&mesh)
            .map_err(|e| anyhow::anyhow!("{model}: {e}"))?
            .with_context(|| format!("{model}: bone-space with no skeleton"))?
    } else {
        mesh
    };
    for seg in &mut mesh.segments {
        for (_, texture) in &mut seg.textures {
            *texture = format!("openbf_droid_{}", texture.to_ascii_lowercase());
        }
    }
    std::fs::write(
        side.join(format!("weapons/{}.glb", mesh.name)),
        crate::glb::model_to_glb(&mesh, &crate::alpha::legacy_per_texture_or(&mesh), false)?,
    )?;
    let dir = side.join("textures");
    for texture in baked.textures {
        for ext in ["ktx2", "tier2.ktx2", "dtl.ktx2"] {
            let from = dir.join(format!("{texture}.{ext}"));
            if from.is_file() {
                std::fs::rename(from, dir.join(format!("openbf_droid_{texture}.{ext}")))?;
            }
        }
    }
    Ok(())
}

// Five anatomical groups: torso/head, right arm, left arm, right leg, left leg.
// Each triangle stays intact; majority bone wins, then the lowest group on ties.
fn group(name: &str) -> usize {
    match name {
        n if n.starts_with("bone_r_") && (n.contains("arm") || n.contains("hand")) => 1,
        n if n.starts_with("bone_l_") && (n.contains("arm") || n.contains("hand")) => 2,
        n if n.starts_with("bone_r_")
            && (n.contains("thigh")
                || n.contains("calf")
                || n.contains("foot")
                || n.contains("toe")) =>
        {
            3
        }
        n if n.starts_with("bone_l_")
            && (n.contains("thigh")
                || n.contains("calf")
                || n.contains("foot")
                || n.contains("toe")) =>
        {
            4
        }
        _ => 0,
    }
}

const ANCHORS: [&str; 5] = [
    "bone_a_spine",
    "bone_r_upperarm",
    "bone_l_forearm",
    "bone_r_thigh",
    "bone_l_calf",
];

fn cut_magnaguard(body: &Model, skel: &Skeleton) -> Result<Vec<(Model, ManifestPrim)>> {
    let binds = skel.bind_matrices()?;
    let mut pieces: Vec<Model> = (1..=5)
        .map(|n| {
            let mut m = body.clone();
            m.name = format!("magnaguard_chunk{n}");
            m.bone_space = false;
            m.segments.clear();
            m
        })
        .collect();
    for seg in &body.segments {
        let (positions, normals, bones) = mungefs::model::to_model_space(body, seg, skel)?;
        for (g, piece) in pieces.iter_mut().enumerate() {
            let anchor = skel
                .index_of(ANCHORS[g])
                .with_context(|| format!("missing {}", ANCHORS[g]))?;
            let inv = Mat4::from_cols_array_2d(&binds[anchor]).inverse();
            let mut s = seg.clone();
            s.positions.clear();
            s.normals.clear();
            s.uvs.clear();
            s.triangles.clear();
            s.colors = None;
            s.bmap.clear();
            s.bind.clear();
            s.wght.clear();
            s.bone = None;
            let mut remap = std::collections::BTreeMap::new();
            for tri in &seg.triangles {
                let mut votes = [0u8; 5];
                for &v in tri {
                    votes[group(&skel.bones[bones[v as usize]].name)] += 1;
                }
                let winner = (0..5)
                    .max_by_key(|&i| (votes[i], std::cmp::Reverse(i)))
                    .unwrap();
                if winner != g {
                    continue;
                }
                let mut out = [0; 3];
                for (k, &v) in tri.iter().enumerate() {
                    out[k] = *remap.entry(v).or_insert_with(|| {
                        let i = v as usize;
                        let index = s.positions.len() as u32;
                        s.positions
                            .push(inv.transform_point3(Vec3::from(positions[i])).to_array());
                        s.normals.push(
                            inv.transform_vector3(Vec3::from(normals[i]))
                                .normalize_or_zero()
                                .to_array(),
                        );
                        s.uvs.push(seg.uvs[i]);
                        index
                    });
                }
                s.triangles.push(out);
            }
            if !s.triangles.is_empty() {
                s.vertex_count = s.positions.len() as u32;
                s.index_count = s.triangles.len() as u32 * 3;
                piece.segments.push(s);
            }
        }
    }
    pieces
        .into_iter()
        .map(|mut m| {
            ensure!(!m.segments.is_empty(), "{} empty anatomical cut", m.name);
            let (lo, hi) = crate::gltf_bin::min_max3(m.segments.iter().flat_map(|s| &s.positions));
            m.vertex_box = mungefs::model::Aabb { min: lo, max: hi };
            m.vis_box = m.vertex_box;
            m.face_count = m.segments.iter().map(|s| s.triangles.len() as u32).sum();
            m.sphere = None;
            let half = (Vec3::from(hi) - Vec3::from(lo)) * 0.5;
            ensure!(half.min_element() > 0.0, "{} degenerate bounds", m.name);
            let prim = ManifestPrim {
                name: format!("{}0", m.name),
                parent: String::new(),
                kind: "sphere".into(),
                dims: [half.max_element(), 0.0, 0.0],
                translation: ((Vec3::from(hi) + Vec3::from(lo)) * 0.5).to_array(),
                rotation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                mask: 0,
                mask_own: false,
            };
            println!(
                "{}: {} triangles, sphere {:?} at {:?}",
                m.name, m.face_count, prim.dims, prim.translation
            );
            Ok((m, prim))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn files(dir: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
        fn visit(
            base: &Path,
            dir: &Path,
            out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
        ) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(base, &path, out);
                } else {
                    out.insert(
                        path.strip_prefix(base).unwrap().into(),
                        std::fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut out = Default::default();
        visit(dir, dir, &mut out);
        out
    }

    #[test]
    fn real_droid_bake_is_deterministic_and_cuts_every_body_triangle_once() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../extracted");
        let b1 = root.join("bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL");
        let b2 = root.join("bf2/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL");
        let common = root.join("bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL");
        if !b1.is_file() || !b2.is_file() || !common.is_file() {
            eprintln!("SKIP: both CIS LVLs and BF1 COMMON.LVL required");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openbf_droid_chunks_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for pass in ["a", "b"] {
            bake(
                b1.to_str().unwrap(),
                b2.to_str().unwrap(),
                common.to_str().unwrap(),
                &dir.join(pass),
                None,
            )
            .unwrap();
        }
        let a = files(&dir.join("a"));
        assert!(!a.is_empty());
        assert_eq!(a, files(&dir.join("b")), "every output byte");
        for n in 1..=5 {
            assert_eq!(
                a[Path::new(&format!(
                    "side_bf1/droid_chunks/weapons/sbdroid_chunk{n}.glb"
                ))],
                a[Path::new(&format!(
                    "side_bf2/droid_chunks/weapons/sbdroid_chunk{n}.glb"
                ))]
            );
        }
        let bytes = std::fs::read(b2).unwrap();
        let (body, skel) =
            crate::soldier::find_soldier(mungefs::open(&bytes).unwrap(), "cis_inf_magnaguard")
                .unwrap();
        let meta: DroidChunkManifest = ron::from_str(
            &std::fs::read_to_string(dir.join("a/side_bf2/droid_chunks/droid_chunks.ron")).unwrap(),
        )
        .unwrap();
        let profiles: Vec<ClassDef> = ron::from_str(
            &std::fs::read_to_string(dir.join("a/classes/droid_chunks.ron")).unwrap(),
        )
        .unwrap();
        let profile = profiles
            .iter()
            .find(|p| p.name == "cis_inf_magnaguard")
            .unwrap();
        let anchors: Vec<_> = profile
            .props
            .iter()
            .filter(|(k, _)| *k == hash("ChunkNodeName"))
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(anchors, ANCHORS);
        // Compare unordered triangle corners in body space; bone-local cuts
        // must neither lose a face nor duplicate one at a mixed-bone seam.
        let mut original = Vec::new();
        for seg in &body.segments {
            let (pos, _, _) = mungefs::model::to_model_space(&body, seg, &skel).unwrap();
            for tri in &seg.triangles {
                original.push((
                    tri.map(|v| Vec3::from(pos[v as usize])),
                    tri.map(|v| seg.uvs[v as usize]),
                    crate::alpha::segment_texture(seg)
                        .unwrap()
                        .to_ascii_lowercase(),
                ));
            }
        }
        let source_triangles = original.len();
        let binds = skel.bind_matrices().unwrap();
        let mut restored = Vec::new();
        let mut max_error = 0f32;
        for (i, anchor) in anchors.into_iter().enumerate() {
            let name = format!("magnaguard_chunk{}", i + 1);
            let collision = meta.collision.iter().find(|c| c.geometry == name).unwrap();
            assert_eq!(collision.prims.len(), 1);
            assert_eq!(collision.prims[0].kind, "sphere");
            assert!(collision.prims[0].dims[0] > 0.05);
            let bind = Mat4::from_cols_array_2d(&binds[skel.index_of(anchor).unwrap()]);
            let (doc, buffers, _) =
                gltf::import(dir.join(format!("a/side_bf2/droid_chunks/weapons/{name}.glb")))
                    .unwrap();
            for prim in doc.meshes().flat_map(|m| m.primitives()) {
                let reader = prim.reader(|b| Some(buffers[b.index()].0.as_slice()));
                let positions: Vec<_> = reader.read_positions().unwrap().collect();
                let uvs: Vec<_> = reader.read_tex_coords(0).unwrap().into_f32().collect();
                let indices: Vec<_> = reader.read_indices().unwrap().into_u32().collect();
                assert_eq!(indices.len() % 3, 0);
                let texture = prim.material().name().unwrap().to_ascii_lowercase();
                for tri in indices.chunks_exact(3) {
                    let tri = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
                    restored.push((
                        tri.map(|v| bind.transform_point3(Vec3::from(positions[v]))),
                        tri.map(|v| uvs[v]),
                        texture.clone(),
                    ));
                }
            }
        }
        assert_eq!(source_triangles, restored.len());
        for (positions, uvs, textures) in restored {
            // A floating inverse/bind round trip can cross a decimal rounding
            // boundary. Match whole faces within 1 micrometre, with exact UVs,
            // materials and multiplicity, rather than quantizing positions.
            let at = original
                .iter()
                .position(|(p, uv, tex)| {
                    *uv == uvs
                        && *tex == textures
                        && p.iter().zip(positions).all(|(a, b)| a.distance(b) < 1e-6)
                })
                .unwrap_or_else(|| panic!("lost or moved source face: {positions:?}"));
            let (p, _, _) = original.swap_remove(at);
            for (a, b) in p.into_iter().zip(positions) {
                max_error = max_error.max(a.distance(b));
            }
        }
        assert!(original.is_empty());
        println!("five cuts, {source_triangles} source triangles, max reconstruction error {max_error:e} m; {} deterministic files", a.len());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
