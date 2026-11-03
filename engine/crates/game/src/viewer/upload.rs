//! GPU upload primitives: a CPU mesh as per-submesh render meshes, each with
//! only the vertices it uses ([`upload_submeshes`]; skinned: [`upload_skinned`]),
//! a decoded texture with its detail companion ([`upload_cpu_texture`]), a
//! baked `.ktx2` from disk ([`upload_texture_file`]), and the procedural unit
//! cube, billboard quad and effect sprite.

use crate::assets::{CpuMesh, CpuSkinnedMesh, CpuTexture};
use anyhow::{anyhow, Result};
use glam::Vec3;
use render::{
    DetailDesc, DrawLayer, MeshData, MeshHandle, Renderer, SamplerMode, SkinnedMeshData,
    SkinnedMeshHandle, TextureDesc, TextureHandle,
};
use std::path::Path;

/// Unit cube (±0.5, face normals) for fallback markers and debug draws.
pub fn upload_unit_cube(r: &mut Renderer) -> Result<MeshHandle> {
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    // (normal, u-axis, v-axis) per face
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    for (n, u, v) in faces {
        let base = positions.len() as u32;
        let (n, u, v) = (Vec3::from(n), Vec3::from(u), Vec3::from(v));
        for (su, sv) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
            positions.push((n * 0.5 + u * su + v * sv).to_array());
            normals.push(n.to_array());
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    r.add_mesh(&MeshData {
        positions: &positions,
        normals: &normals,
        uvs: &vec![[0.0, 0.0]; positions.len()],
        colors: None,
        indices: &indices,
    })
    .map_err(|e| anyhow!("cube upload: {e}"))
}

/// Unit quad in the XY plane (+-0.5), for camera-facing billboards
/// (`game::fx`). Its normals point INTO the sun, so the lambert term is
/// constant on a lit draw as the camera turns. Draws may also select the
/// renderer's unlit path.
pub fn upload_billboard_quad(r: &mut Renderer) -> Result<MeshHandle> {
    let n = (-render::Lighting::default().sun_dir).to_array();
    r.add_mesh(&MeshData {
        positions: &[
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ],
        normals: &[n; 4],
        uvs: &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        colors: None,
        indices: &[0, 1, 2, 0, 2, 3],
    })
    .map_err(|e| anyhow!("billboard quad upload: {e}"))
}

/// Procedural sprite for placeholder effects (`game::fx`): a radial blob,
/// bright in the middle, with a soft radial alpha and RGB falloff for
/// composited effect draws.
pub fn upload_fx_sprite(r: &mut Renderer) -> Result<TextureHandle> {
    const N: u32 = 32;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = (
                (x as f32 + 0.5) / N as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / N as f32 * 2.0 - 1.0,
            );
            let d = (dx * dx + dy * dy).sqrt();
            // A soft round falloff, now that effects are COMPOSITED rather
            // than alpha-tested: colour stays bright to the rim and the ALPHA
            // carries the shape, so there is no hard circular edge and no
            // dark halo. The 0.5 exponent keeps a wide bright core.
            const R: f32 = 0.95;
            let a = (1.0 - (d / R).clamp(0.0, 1.0)).powf(0.5);
            let k = 0.55 + 0.45 * a;
            let v = (k * 255.0) as u8;
            rgba.extend_from_slice(&[v, v, v, (a * 255.0) as u8]);
        }
    }
    r.add_texture(&TextureDesc {
        width: N,
        height: N,
        mips: &[&rgba],
        sampler: SamplerMode::Linear,
        wrap_v: render::WrapV::Repeat,
        srgb: true,
        detail: None,
        scroll: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
        label: Some("fx sprite"),
    })
    .map_err(|e| anyhow!("fx sprite upload: {e}"))
}

/// UV multiplier for PS2 detail maps. 1.0 = the detail map is sampled at the
/// base texture's UVs, which is what the disc data shows: every `<name>_dtl`
/// is exactly 4x its base in each dimension and its features register 1:1
/// with the base (RE_LOG "BF1 detail textures"). Kept as a named constant so
/// a genuinely tiling pair can be handled without hunting through shaders.
pub const DETAIL_UV_SCALE: f32 = 1.0;

/// Upload one decoded texture (+ its detail companion, when the bake wrote
/// one) and return its handle.
pub fn upload_cpu_texture(r: &mut Renderer, t: &CpuTexture, label: &str) -> Result<TextureHandle> {
    upload_cpu_texture_inner(r, t, label, [0.0, 0.0], [1.0, 1.0], render::WrapV::Repeat)
}

/// As [`upload_cpu_texture`], but the material scrolls its UVs — a map's
/// water surface, whose velocity its `fx__ Effect("Water")` authors. Uploaded as its
/// OWN handle rather than through the scene's shared texture cache, so a
/// model that happens to use the same texture keeps a still one.
/// `uv_scale` multiplies the mesh UV first — read only by the water's speckle
/// layer (`render::TextureDesc::uv_scale`); `[1.0, 1.0]` for the water itself.
pub fn upload_cpu_texture_scrolling(
    r: &mut Renderer,
    t: &CpuTexture,
    label: &str,
    scroll: [f32; 2],
    uv_scale: [f32; 2],
) -> Result<TextureHandle> {
    upload_cpu_texture_inner(r, t, label, scroll, uv_scale, render::WrapV::Repeat)
}

pub(super) fn upload_flat_water_texture(
    r: &mut Renderer,
    t: &CpuTexture,
    label: &str,
    scroll: [f32; 2],
    uv_scale: [f32; 2],
) -> Result<TextureHandle> {
    let mips: Vec<&[u8]> = std::iter::once(t.rgba.as_slice())
        .chain(t.extra_mips.iter().map(Vec::as_slice))
        .collect();
    r.add_texture(&TextureDesc {
        width: t.width,
        height: t.height,
        mips: &mips,
        sampler: SamplerMode::Linear,
        wrap_v: render::WrapV::Repeat,
        srgb: false,
        detail: None,
        scroll,
        uv_scale,
        label: Some(label),
    })
    .map_err(|e| anyhow!("water texture {label}: {e}"))
}

/// `wrap_v` is [`render::WrapV::Repeat`] for every caller but a clamped sky
/// shell (see [`super::scene::sky_wrap_v`]).
pub(super) fn upload_cpu_texture_inner(
    r: &mut Renderer,
    t: &CpuTexture,
    label: &str,
    scroll: [f32; 2],
    uv_scale: [f32; 2],
    wrap_v: render::WrapV,
) -> Result<TextureHandle> {
    let mut mips: Vec<&[u8]> = vec![&t.rgba];
    mips.extend(t.extra_mips.iter().map(Vec::as_slice));
    let detail_mips: Vec<&[u8]> = t
        .detail
        .as_ref()
        .map(|d| {
            let mut m: Vec<&[u8]> = vec![&d.rgba];
            m.extend(d.extra_mips.iter().map(Vec::as_slice));
            m
        })
        .unwrap_or_default();
    r.add_texture(&TextureDesc {
        width: t.width,
        height: t.height,
        mips: &mips,
        sampler: SamplerMode::Linear,
        wrap_v,
        srgb: true,
        detail: t.detail.as_ref().map(|d| DetailDesc {
            width: d.width,
            height: d.height,
            mips: &detail_mips,
            scale: DETAIL_UV_SCALE,
        }),
        scroll,
        uv_scale,
        label: Some(label),
    })
    .map_err(|e| anyhow!("texture {label}: {e}"))
}

/// Upload the baked texture `<dir>/<name>.ktx2`; None when it was not baked.
pub fn upload_texture_file(
    r: &mut Renderer,
    dir: &Path,
    name: &str,
) -> Result<Option<TextureHandle>> {
    upload_texture_file_scrolling(r, dir, name, [0.0, 0.0])
}

pub fn upload_texture_file_scrolling(
    r: &mut Renderer,
    dir: &Path,
    name: &str,
    scroll: [f32; 2],
) -> Result<Option<TextureHandle>> {
    if !pak::exists(dir.join(format!("{name}.ktx2"))) {
        return Ok(None);
    }
    let t0 = std::time::Instant::now();
    let t = crate::assets::load_texture(dir, name)?;
    let h = upload_cpu_texture_scrolling(r, &t, name, scroll, [1.0, 1.0])?;
    // The frame recorder's (card HITCH): a file loaded in a battle frame.
    let bytes = t.rgba.len() + t.extra_mips.iter().map(Vec::len).sum::<usize>();
    render::made::note(
        "texture file",
        Some(name),
        bytes as u64,
        t0.elapsed().as_secs_f32() * 1000.0,
    );
    Ok(Some(h))
}

/// One uploaded submesh: its mesh handle, the material name (`None` when the
/// segment named none), the ALPHA RULE v2 pass the world path routes on, and
/// the raw PS2 MTRL word only the sky path reads (`render::SkyMode`).
pub type SubmeshPart<'a> = (MeshHandle, Option<&'a str>, DrawLayer, u32);

/// Upload one CPU mesh as per-submesh render meshes (shared vertex arrays,
/// per-material index ranges), returning
/// (handle, material name, alpha mode, raw MTRL flags) per submesh. The world
/// path routes on the alpha mode; only the SKY path reads the flags
/// (`render::SkyMode`).
pub fn upload_submeshes<'a>(r: &mut Renderer, cpu: &'a CpuMesh) -> Result<Vec<SubmeshPart<'a>>> {
    let mut out = Vec::new();
    let single = cpu.submeshes.len() == 1;
    for (start, count, mat, layer, flags) in &cpu.submeshes {
        let idx = &cpu.indices[*start as usize..(*start + *count) as usize];
        let mesh = if single {
            // One submesh already owns every vertex; compacting would copy
            // the arrays for nothing.
            r.add_mesh(&cpu.mesh_data(idx))
        } else {
            // Each submesh gets ONLY the vertices its own indices touch.
            //
            // This used to hand every submesh the whole model's vertex
            // arrays and vary only the index range, which cost twice: the
            // vertex buffer was uploaded once PER SUBMESH (a ten-material
            // model uploaded its vertices ten times), and `GpuMesh::upload`
            // takes the mesh AABB from the positions it is given, so every
            // submesh inherited the WHOLE model's bounds and frustum culling
            // could only ever cull all of a model or none of it. Compacting
            // is what lets a split mesh — the terrain — be culled piecewise.
            let sub = compact_submesh(cpu, idx);
            r.add_mesh(&sub.mesh_data())
        }
        .map_err(|e| anyhow!("mesh upload: {e}"))?;
        out.push((
            mesh,
            (!mat.is_empty()).then_some(mat.as_str()),
            *layer,
            *flags,
        ));
    }
    Ok(out)
}

/// Vertices referenced by one submesh, in first-use order, with the indices
/// rebased onto them. Order is deterministic (it follows the index stream),
/// so the same model always uploads the same buffer.
pub(super) struct CompactSubmesh {
    pub(super) positions: Vec<[f32; 3]>,
    pub(super) normals: Vec<[f32; 3]>,
    pub(super) uvs: Vec<[f32; 2]>,
    pub(super) colors: Vec<[u8; 4]>,
    pub(super) indices: Vec<u32>,
}

impl CompactSubmesh {
    pub(super) fn mesh_data(&self) -> MeshData<'_> {
        MeshData {
            positions: &self.positions,
            normals: &self.normals,
            uvs: &self.uvs,
            colors: Some(&self.colors),
            indices: &self.indices,
        }
    }
}

pub(super) fn compact_submesh(cpu: &CpuMesh, idx: &[u32]) -> CompactSubmesh {
    let mut slot = vec![u32::MAX; cpu.positions.len()];
    let mut sub = CompactSubmesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        colors: Vec::new(),
        indices: Vec::with_capacity(idx.len()),
    };
    for &i in idx {
        let i = i as usize;
        if i >= slot.len() {
            continue; // a malformed GLB; the renderer would reject it anyway
        }
        if slot[i] == u32::MAX {
            slot[i] = sub.positions.len() as u32;
            sub.positions.push(cpu.positions[i]);
            sub.normals
                .push(cpu.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]));
            sub.uvs.push(cpu.uvs.get(i).copied().unwrap_or([0.0, 0.0]));
            sub.colors
                .push(cpu.colors.get(i).copied().unwrap_or([255; 4]));
        }
        sub.indices.push(slot[i]);
    }
    sub
}

/// One uploaded skinned submesh: its mesh handle, the material name (`None`
/// when the segment named none) and the pass its material asks for
/// (`CpuSkinnedMesh::submeshes` — `Opaque` or `Blend`), which the caller
/// puts on its `render::SkinnedDrawCmd`.
pub type SkinnedPart<'a> = (SkinnedMeshHandle, Option<&'a str>, DrawLayer);

/// Upload one skinned CPU mesh as per-submesh skinned render meshes (shared
/// vertex arrays incl. joints/weights, per-material index ranges).
pub fn upload_skinned<'a>(
    r: &mut Renderer,
    cpu: &'a CpuSkinnedMesh,
) -> Result<Vec<SkinnedPart<'a>>> {
    let mut out = Vec::new();
    for (start, count, mat, layer) in &cpu.submeshes {
        let idx = &cpu.indices[*start as usize..(*start + *count) as usize];
        let mesh = r
            .add_skinned_mesh(&SkinnedMeshData {
                positions: &cpu.positions,
                normals: &cpu.normals,
                uvs: &cpu.uvs,
                colors: Some(&cpu.colors),
                joints: &cpu.joints,
                weights: &cpu.weights,
                indices: idx,
            })
            .map_err(|e| anyhow!("skinned mesh upload: {e}"))?;
        out.push((mesh, (!mat.is_empty()).then_some(mat.as_str()), *layer));
    }
    Ok(out)
}
