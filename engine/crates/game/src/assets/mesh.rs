//! Static and skinned GLB readers and their CPU-side mesh data.

use super::{CpuMesh, SpecPass};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Skinned mesh as loaded from a skinned GLB (`bake::skin_glb`).
pub struct CpuSkinnedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[u8; 4]>,
    /// Skeleton bone indices (skin joint order == skeleton order).
    pub joints: Vec<[u8; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// Per primitive: (start, count, material name, pass). The pass is the
    /// material's glTF `alphaMode` as a renderer layer, mapped as for
    /// [`CpuMesh::submeshes`]: `bake::skin_glb` writes `BLEND` on a segment
    /// the disc composites and no `alphaMode` — OPAQUE — on every other one
    /// (`bake::soldier::segment_blends`), so a soldier's parts are `Opaque`
    /// or `Blend`.
    pub submeshes: Vec<(u32, u32, String, render::DrawLayer)>,
    /// Per submesh (aligned with `submeshes`): its specular second pass, as
    /// [`CpuMesh::spec`] (queue SPEC2). Read through
    /// [`CpuSkinnedMesh::spec_pass`].
    pub spec: Vec<Option<SpecPass>>,
}

impl CpuSkinnedMesh {
    /// Submesh `i`'s specular second pass, or `None` when it draws none.
    pub fn spec_pass(&self, i: usize) -> Option<SpecPass> {
        self.spec.get(i).copied().flatten()
    }
}

pub struct SkinnedAsset {
    pub name: String,
    pub mesh: CpuSkinnedMesh,
    pub skeleton: crate::skin::Skeleton,
    pub clips: Vec<crate::skin::Clip>,
}

/// Load a skinned GLB: the skin's joints define the skeleton (order, names,
/// parents from the node tree, bind = node transforms), the mesh primitives
/// carry JOINTS_0/WEIGHTS_0, animations become clips (one key per frame;
/// fps from the sampler times, `extras.fps` when present).
pub fn load_skinned_glb(path: &Path) -> Result<SkinnedAsset> {
    use crate::skin::{BoneXform, Clip, Skeleton};
    let (doc, buffers, _) =
        gltf::import_slice(pak::read(path).with_context(|| path.display().to_string())?)
            .map_err(|e| anyhow!("{e}"))?;
    let skin = doc
        .skins()
        .next()
        .ok_or_else(|| anyhow!("no skin in {}", path.display()))?;
    let joints: Vec<gltf::Node> = skin.joints().collect();
    let joint_of_node: HashMap<usize, usize> = joints
        .iter()
        .enumerate()
        .map(|(j, n)| (n.index(), j))
        .collect();
    let mut parent_of_node: HashMap<usize, usize> = HashMap::new();
    for n in doc.nodes() {
        for c in n.children() {
            parent_of_node.insert(c.index(), n.index());
        }
    }
    let names: Vec<String> = joints
        .iter()
        .map(|n| n.name().unwrap_or_default().to_string())
        .collect();
    let parents: Vec<Option<usize>> = joints
        .iter()
        .map(|n| {
            parent_of_node
                .get(&n.index())
                .and_then(|p| joint_of_node.get(p).copied())
        })
        .collect();
    let local_bind: Vec<glam::Mat4> = joints
        .iter()
        .map(|n| glam::Mat4::from_cols_array_2d(&n.transform().matrix()))
        .collect();
    let skeleton = Skeleton::new(names, parents, local_bind);

    let mut mesh = CpuSkinnedMesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        colors: vec![],
        joints: vec![],
        weights: vec![],
        indices: vec![],
        submeshes: vec![],
        spec: vec![],
    };
    // A skeleton-and-clips GLB (a walker's skin, card WSKN) has no mesh
    // node: its mesh stays empty and it is named by its skin.
    let mesh_node = doc
        .nodes()
        .find(|n| n.skin().is_some() && n.mesh().is_some());
    let name = mesh_node
        .as_ref()
        .map_or(skin.name(), |n| n.name())
        .unwrap_or_default()
        .to_string();
    let prims = mesh_node
        .iter()
        .filter_map(|n| n.mesh())
        .flat_map(|m| m.primitives());
    for prim in prims {
        let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
        let base = mesh.positions.len() as u32;
        let vertex_start = mesh.positions.len();
        mesh.positions.extend(
            reader
                .read_positions()
                .ok_or_else(|| anyhow!("no positions"))?,
        );
        let n = mesh.positions.len() - vertex_start;
        match reader.read_normals() {
            Some(it) => mesh.normals.extend(it),
            None => mesh.normals.extend(std::iter::repeat_n([0.0, 1.0, 0.0], n)),
        }
        match reader.read_tex_coords(0) {
            Some(tc) => mesh.uvs.extend(tc.into_f32()),
            None => mesh.uvs.extend(std::iter::repeat_n([0.0, 0.0], n)),
        }
        match reader.read_colors(0) {
            Some(c) => mesh.colors.extend(c.into_rgba_u8()),
            None => mesh.colors.extend(std::iter::repeat_n([255u8; 4], n)),
        }
        let j: Vec<[u16; 4]> = reader
            .read_joints(0)
            .ok_or_else(|| anyhow!("no JOINTS_0"))?
            .into_u16()
            .collect();
        if let Some(bad) = j
            .iter()
            .flatten()
            .find(|&&v| usize::from(v) >= skeleton.len())
        {
            bail!("joint {bad} out of range for {} bones", skeleton.len());
        }
        mesh.joints.extend(
            j.iter()
                .map(|v| [v[0] as u8, v[1] as u8, v[2] as u8, v[3] as u8]),
        );
        mesh.weights.extend(
            reader
                .read_weights(0)
                .ok_or_else(|| anyhow!("no WEIGHTS_0"))?
                .into_f32(),
        );
        let start = mesh.indices.len() as u32;
        let index_start = mesh.indices.len();
        mesh.indices.extend(
            reader
                .read_indices()
                .ok_or_else(|| anyhow!("no indices"))?
                .into_u32()
                .map(|i| i + base),
        );
        let count = (mesh.indices.len() - index_start) as u32;
        let mat = prim.material().name().unwrap_or_default().to_string();
        // A skinned draw is Opaque or Blend (`render::SkinnedDrawCmd::layer`:
        // the disc alpha-tests no soldier, and the bake refuses the cutout
        // bit, `bake::soldier::segment_blends`). A GLB asking for MASK fails
        // HERE, where the caller skips that one model (`soldier_vis`), rather
        // than in `render_frame`, which refuses the whole frame.
        let layer = layer_of(&prim);
        if layer == render::DrawLayer::Mask {
            bail!("material {mat:?}: alphaMode MASK — a skinned draw is Opaque or Blend");
        }
        mesh.submeshes.push((start, count, mat, layer));
        mesh.spec.push(mat_extras(&prim).and_then(|e| e.spec_pass));
    }

    let mut clips = Vec::new();
    for anim in doc.animations() {
        let mut tracks: Vec<Option<Vec<BoneXform>>> = vec![None; skeleton.len()];
        let mut frames = 0usize;
        let mut fps = 30.0f32;
        for ch in anim.channels() {
            let Some(&bone) = joint_of_node.get(&ch.target().node().index()) else {
                continue;
            };
            let reader = ch.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let times: Vec<f32> = reader
                .read_inputs()
                .ok_or_else(|| anyhow!("no sampler input"))?
                .collect();
            frames = frames.max(times.len());
            if times.len() >= 2 {
                // One key per frame at a fixed rate (bake writes i / fps).
                fps = (1.0 / (times[1] - times[0])).round();
            }
            let track = tracks[bone].get_or_insert_with(|| vec![BoneXform::IDENTITY; times.len()]);
            if track.len() < times.len() {
                track.resize(times.len(), BoneXform::IDENTITY);
            }
            match reader
                .read_outputs()
                .ok_or_else(|| anyhow!("no sampler output"))?
            {
                // `track` is sized from the sampler's INPUT accessor, but
                // read_outputs() iterates the OUTPUT accessor, and glTF's
                // CUBICSPLINE interpolation defines output.count ==
                // 3 * input.count — so any spec-valid cubic-spline clip would
                // run off the end. Our own bake never emits one, but a GLB
                // exported from a DCC tool and dropped into assets_baked
                // would. Take only what the track holds.
                gltf::animation::util::ReadOutputs::Rotations(r) => {
                    for (i, q) in r.into_f32().enumerate().take(track.len()) {
                        track[i].rotation = glam::Quat::from_array(q);
                    }
                }
                gltf::animation::util::ReadOutputs::Translations(t) => {
                    for (i, v) in t.enumerate().take(track.len()) {
                        track[i].translation = glam::Vec3::from_array(v);
                    }
                }
                _ => {}
            }
        }
        clips.push(Clip {
            name: anim.name().unwrap_or_default().to_string(),
            fps,
            frames,
            tracks,
        });
    }

    Ok(SkinnedAsset {
        name,
        mesh,
        skeleton,
        clips,
    })
}

pub fn load_glb(path: &Path) -> Result<CpuMesh> {
    load_glb_joints(path).map(|(mesh, _)| mesh)
}

/// [`load_glb`], plus each vertex's `JOINTS_0` bone — a walker model's
/// (card WSKN, `bake::glb::model_to_glb_joints`); empty when no primitive
/// carries joints, and `[0; 4]` for a vertex of one that does not.
pub fn load_glb_joints(path: &Path) -> Result<(CpuMesh, Vec<[u8; 4]>)> {
    let (doc, buffers, _) =
        gltf::import_slice(pak::read(path).with_context(|| path.display().to_string())?)
            .map_err(|e| anyhow!("{e}"))?;
    let mut joints: Vec<[u8; 4]> = Vec::new();
    let mut mesh = CpuMesh {
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        colors: vec![],
        indices: vec![],
        submeshes: vec![],
        spec: vec![],
        light: vec![],
        scroll: vec![],
        sphere: None,
        vis_top: None,
        render_grid: None,
        contact_surface: false,
    };
    for m in doc.meshes() {
        if let Some(raw) = m.extras() {
            let value: serde_json::Value = serde_json::from_str(raw.get())?;
            if let Some(flag) = value.get("contact_surface") {
                mesh.contact_surface |= flag
                    .as_bool()
                    .ok_or_else(|| anyhow!("contact surface flag must be a boolean"))?;
            }
            let offset = value.get("render_grid_offset");
            let size = value.get("render_grid_size");
            if offset.is_some() || size.is_some() {
                let pair = offset
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|offset| usize::try_from(offset).ok())
                    .zip(
                        size.and_then(serde_json::Value::as_u64)
                            .and_then(|size| usize::try_from(size).ok()),
                    )
                    .ok_or_else(|| anyhow!("invalid render terrain grid metadata"))?;
                if mesh.render_grid.is_some() || !mesh.positions.is_empty() {
                    return Err(anyhow!("render terrain grid must belong to the first mesh"));
                }
                mesh.render_grid = Some(pair);
            }
        }
        #[derive(serde::Deserialize)]
        struct MeshExtras {
            sphere: Option<[f32; 4]>,
            vis_top: Option<f32>,
        }
        if let Some(e) = m
            .extras()
            .as_ref()
            .and_then(|raw| serde_json::from_str::<MeshExtras>(raw.get()).ok())
        {
            mesh.sphere = e.sphere.or(mesh.sphere);
            mesh.vis_top = e.vis_top.or(mesh.vis_top);
        }
        for prim in m.primitives() {
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let base = mesh.positions.len() as u32;
            let vertex_start = mesh.positions.len();
            mesh.positions.extend(
                reader
                    .read_positions()
                    .ok_or_else(|| anyhow!("no positions"))?,
            );
            let n = mesh.positions.len() - vertex_start;
            match reader.read_normals() {
                Some(it) => mesh.normals.extend(it),
                None => mesh.normals.extend(std::iter::repeat_n([0.0, 1.0, 0.0], n)),
            }
            match reader.read_tex_coords(0) {
                Some(tc) => mesh.uvs.extend(tc.into_f32()),
                None => mesh.uvs.extend(std::iter::repeat_n([0.0, 0.0], n)),
            }
            match reader.read_colors(0) {
                Some(c) => mesh.colors.extend(c.into_rgba_u8()),
                None => mesh.colors.extend(std::iter::repeat_n([255u8; 4], n)),
            }
            if let Some(j) = reader.read_joints(0) {
                joints.resize(base as usize, [0; 4]);
                joints.extend(j.into_u16().map(|v| v.map(|b| b.min(255) as u8)));
            }
            let start = mesh.indices.len() as u32;
            let index_start = mesh.indices.len();
            mesh.indices.extend(
                reader
                    .read_indices()
                    .ok_or_else(|| anyhow!("no indices"))?
                    .into_u32()
                    .map(|i| i + base),
            );
            let count = (mesh.indices.len() - index_start) as u32;
            let mat = prim.material().name().unwrap_or_default().to_string();
            let extras = mat_extras(&prim).unwrap_or_default();
            mesh.submeshes.push((
                start,
                count,
                mat,
                layer_of(&prim),
                extras.flags.unwrap_or(0),
            ));
            mesh.spec.push(extras.spec_pass);
            mesh.light.push(light_of(extras.light.as_deref()));
            mesh.scroll.push(extras.uv_scroll.unwrap_or_default());
        }
    }
    if !joints.is_empty() {
        joints.resize(mesh.positions.len(), [0; 4]);
    }
    if mesh.render_grid.is_some() && super::render_ground::from_mesh(&mesh).is_none() {
        return Err(anyhow!("invalid render terrain grid"));
    }
    Ok((mesh, joints))
}

/// The renderer pass a primitive's glTF `alphaMode` asks for. A GLB baked
/// before ALPHA RULE v2 states no `alphaMode`, and the `gltf` crate reports
/// the spec default OPAQUE for it — which is also what such a bake meant.
fn layer_of(prim: &gltf::Primitive<'_>) -> render::DrawLayer {
    match prim.material().alpha_mode() {
        gltf::material::AlphaMode::Opaque => render::DrawLayer::Opaque,
        gltf::material::AlphaMode::Mask => render::DrawLayer::Mask,
        gltf::material::AlphaMode::Blend => render::DrawLayer::Blend,
    }
}

/// A primitive's `extras.light` (`bake::glb::LightPath::as_str`) as the
/// renderer's path. Absent — every non-map GLB, every GLB baked before queue
/// LITEB — is `Lit`, the lighting such a bake was drawn with; an unknown
/// spelling is refused the same way rather than guessed at.
fn light_of(light: Option<&str>) -> render::LightPath {
    use render::LightPath as P;
    match light {
        Some("prelit") => P::Prelit,
        Some("prelit_add") => P::PrelitAdd,
        Some("unlit") => P::Unlit,
        Some("unlit_add") => P::UnlitAdd,
        Some("vertex_lit") => P::VertexLit,
        Some("glow") => P::Glow,
        Some("unscaled") => P::Unscaled,
        _ => P::Lit,
    }
}

/// The fields of a GLB material's `extras` the runtime reads (`bake::glb`).
#[derive(Default, serde::Deserialize)]
struct MatExtras {
    flags: Option<u32>,
    spec_pass: Option<SpecPass>,
    light: Option<String>,
    uv_scroll: Option<[f32; 2]>,
}

fn mat_extras(prim: &gltf::Primitive<'_>) -> Option<MatExtras> {
    prim.material()
        .extras()
        .as_ref()
        .and_then(|raw| serde_json::from_str::<MatExtras>(raw.get()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two primitives share one buffer; each keeps its indices, defaults and material policy.
    #[test]
    fn glb_streaming_preserves_primitive_offsets_and_whole_extras_failure() {
        let make = |extras: &str, mesh_extras: &str| {
            let json = format!(
                r#"{{
                "asset":{{"version":"2.0"}},
                "buffers":[{{"byteLength":44}}],
                "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],
                "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],
                "materials":[{{"name":"material","alphaMode":"BLEND"{extras}}}],
                "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0}},{{"attributes":{{"POSITION":0}},"indices":1,"material":0}}]{mesh_extras}}}]
            }}"#
            );
            let mut json = json.into_bytes();
            while json.len() % 4 != 0 {
                json.push(b' ');
            }
            let mut bin = Vec::new();
            for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
                bin.extend(v.to_le_bytes());
            }
            for v in [0u16, 1, 2] {
                bin.extend(v.to_le_bytes());
            }
            bin.resize(44, 0);
            let mut glb = Vec::new();
            for v in [
                0x46546c67u32,
                2,
                (12 + 8 + json.len() + 8 + bin.len()) as u32,
                json.len() as u32,
                0x4e4f534a,
            ] {
                glb.extend(v.to_le_bytes());
            }
            glb.extend(json);
            glb.extend((bin.len() as u32).to_le_bytes());
            glb.extend(0x004e4942u32.to_le_bytes());
            glb.extend(bin);
            glb
        };
        for (n, extras, flags, light) in [
            (0, "", 0, render::LightPath::Lit),
            (
                1,
                r#", "extras":{"flags":17,"light":"unlit","spec_pass":{"power":42,"rgb":[0.25,0.5,0.75]}}"#,
                17,
                render::LightPath::Unlit,
            ),
            (
                2,
                r#", "extras":{"flags":17,"light":"unknown"}"#,
                17,
                render::LightPath::Lit,
            ),
            (
                3,
                r#", "extras":{"flags":17,"light":3}"#,
                0,
                render::LightPath::Lit,
            ),
        ] {
            let path =
                std::env::temp_dir().join(format!("openbf-stream-{}-{n}.glb", std::process::id()));
            std::fs::write(&path, make(extras, "")).unwrap();
            let loaded = load_glb(&path);
            std::fs::remove_file(&path).unwrap();
            let mesh = loaded.unwrap();
            assert!(!mesh.contact_surface);
            assert_eq!(
                mesh.positions,
                [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].repeat(2)
            );
            assert_eq!(mesh.indices, [0, 1, 2, 3, 4, 5]);
            assert_eq!(mesh.normals, [[0.0, 1.0, 0.0]; 6]);
            assert_eq!(mesh.uvs, [[0.0, 0.0]; 6]);
            assert_eq!(mesh.colors, [[255; 4]; 6]);
            assert_eq!(
                mesh.submeshes,
                vec![
                    (0, 3, "material".into(), render::DrawLayer::Blend, flags),
                    (3, 3, "material".into(), render::DrawLayer::Blend, flags)
                ]
            );
            assert_eq!(mesh.light, vec![light; 2]);
            let spec = (n == 1).then_some(SpecPass {
                power: 42,
                rgb: Some([0.25, 0.5, 0.75]),
            });
            assert_eq!(mesh.spec, vec![spec; 2]);
        }
        for (n, value, expected) in [
            (0, "true", Some(true)),
            (1, "false", Some(false)),
            (2, "\"true\"", None),
            (3, "null", None),
        ] {
            let path =
                std::env::temp_dir().join(format!("openbf-contact-{}-{n}.glb", std::process::id()));
            let extra = format!(r#", "extras":{{"contact_surface":{value}}}"#);
            std::fs::write(&path, make("", &extra)).unwrap();
            let loaded = load_glb(&path);
            std::fs::remove_file(&path).unwrap();
            match expected {
                Some(flag) => assert_eq!(loaded.unwrap().contact_surface, flag),
                None => assert!(loaded.is_err(), "accepted non-boolean contact flag {value}"),
            }
        }
    }
}
