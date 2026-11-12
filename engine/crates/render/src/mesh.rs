//! CPU mesh description + GPU mesh buffers.
//!
//! Vertex/buffer layout adapted from an earlier project's renderer (`MeshVertex::desc()`
//! and `GpuMesh::upload` via `create_buffer_init`), extended with uv + unorm8 color
//! (terrain lightmap rides in as vertex color) and per-instance model matrices.

use crate::RenderError;

/// Interleaved vertex, 36 bytes, no padding (so `Pod` is layout-exact).
///
/// ROADMAP (classic-plus, PLAN §Classic-plus assets): normal mapping is a
/// confirmed requirement (remaster ships A8L8 _bump maps) — this format will
/// grow a tangent attribute (Float32x4, handedness in w; generated at bake).
/// Budgeted now so adding it is one attribute + shader change, not a redo.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Unorm8x4 in the shader (0..255 -> 0..1). White = no tint.
    pub color: [u8; 4],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3, // position
        1 => Float32x3, // normal
        2 => Float32x2, // uv
        3 => Unorm8x4,  // color
    ];

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Per-draw data fed through an instance-rate vertex buffer (slot 1) instead of a
/// per-object uniform bind group: one buffer write per frame, no bind-group churn.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Instance {
    /// Model matrix columns (glam is column-major; `to_cols_array_2d` matches).
    pub model: [[f32; 4]; 4],
    /// Flat tint multiplied over texture * vertex color. White = untinted.
    pub color: [f32; 4],
    /// Metres from the eye past which this instance's fragments are DISCARDED
    /// — the far-scene cull ([`DrawCmd::cull`](crate::DrawCmd::cull)) — as a
    /// band `[lo, hi]`: the edge is `mix(lo, hi, u)` with `u` the pixel's 4×4
    /// ordered-dither value, so a band crossfades by screen-door and a draw
    /// with `lo == hi` has one hard edge ([`instance_cull`]). [`NO_CULL`] for
    /// a draw with no cull, which is every draw until a caller asks for one.
    pub cull: [f32; 2],
    /// Metres from the eye NEARER than which this instance's fragments are
    /// DISCARDED — the far scene's near edge
    /// ([`DrawCmd::cull_near`](crate::DrawCmd::cull_near)) — a band like
    /// [`Instance::cull`], read with the SAME `u`. [`NO_CULL_NEAR`] for a
    /// draw with no near edge, which is every draw but the `LOWR` sheet and a
    /// geometry's LOWD.
    pub cull_near: [f32; 2],
    /// How the shader lights this instance: the low byte is the
    /// [`LightPath`](crate::LightPath)'s `u32` discriminant
    /// ([`DrawCmd::light`](crate::DrawCmd::light)); bit 0x100 is
    /// [`affine::AFFINE_BIT`](crate::affine::AFFINE_BIT), set by the draw's
    /// layer. Compare the path with `light & 0xff`, never the whole word.
    pub light: u32,
    /// The per-instance fade, `[begin, end, near]` metres and its opacity
    /// ([`DrawCmd::fade`](crate::DrawCmd::fade)); [`NO_FADE`] for a draw
    /// with none.
    pub fade: [f32; 4],
    /// The light-table records this instance's vertices add (card BPNT,
    /// `crate::point_light::gather`); 0 = none.
    pub lights: [u16; 4],
    pub region_top: [f32; 4],
    pub region_bottom: [f32; 4],
}

/// "This draw never fades": a begin no horizontal distance reaches, and no
/// near fade, at full opacity.
pub(crate) const NO_FADE: [f32; 4] = [NO_CULL, NO_CULL, 0.0, 1.0];

/// "This draw is never culled by distance." A finite metre value rather than
/// an infinity: it travels as an interpolated vertex output, and a finite one
/// cannot come out of the interpolator as a NaN. Nothing in a PS2 map is a
/// million kilometres from the eye (the widest baked mesh is Kamino's 20 km
/// water plane).
pub(crate) const NO_CULL: f32 = 1.0e9;

/// "This draw has no near edge." Zero, so the test is the same comparison for
/// every draw and nothing in front of the eye is ever discarded by it.
pub(crate) const NO_CULL_NEAR: f32 = 0.0;

impl Instance {
    // Locations 4..=8 continue after the mesh attributes; the cull skips to
    // 11 because `SkinnedVertex` already owns 9 and 10 and shares this
    // instance buffer.
    const ATTRIBS: [wgpu::VertexAttribute; 12] = wgpu::vertex_attr_array![
        4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, // model cols
        8 => Float32x4, // color
        11 => Float32x2, // cull band [lo, hi], metres
        12 => Float32x2, // near-edge band [lo, hi], metres
        13 => Uint32,   // light path
        14 => Float32x4, // fade [begin, end, near] metres, opacity
        15 => Uint16x4, // light-table records, 0 = none
        // Rigid-only, after the fields every pipeline shares: the region
        // light's top and bottom planes (`region_lit`).
        9 => Float32x4,
        10 => Float32x4,
    ];

    pub(crate) fn rigid_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS[..10],
        }
    }
}

/// SoA borrow of CPU mesh data; interleaved on upload.
/// All per-vertex slices must have equal length; `colors: None` defaults to white.
#[derive(Debug, Clone, Copy)]
pub struct MeshData<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: &'a [[f32; 3]],
    pub uvs: &'a [[f32; 2]],
    pub colors: Option<&'a [[u8; 4]]>,
    pub indices: &'a [u32],
}

/// Index of an uploaded mesh. `Default` is index 0 — the FIRST mesh a
/// renderer uploaded, not a null sentinel; it exists so callers that build a
/// draw list without a GPU (unit tests) can name a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MeshHandle(pub(crate) u32);

pub(crate) struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    /// Highest joint index referenced (skinned meshes; 0 for static).
    pub max_joint: u32,
    /// Model-space AABB (min, max) — what frustum culling tests. An empty
    /// mesh gets a degenerate box at the origin, which culls as a point.
    pub bounds: (glam::Vec3, glam::Vec3),
}

/// Model-space AABB of a position list.
pub(crate) fn aabb_of(positions: impl IntoIterator<Item = [f32; 3]>) -> (glam::Vec3, glam::Vec3) {
    let mut lo = glam::Vec3::splat(f32::INFINITY);
    let mut hi = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for p in positions {
        any = true;
        let v = glam::Vec3::from(p);
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if !any {
        (glam::Vec3::ZERO, glam::Vec3::ZERO)
    } else {
        (lo, hi)
    }
}

impl GpuMesh {
    pub fn upload(device: &wgpu::Device, data: &MeshData<'_>) -> Result<Self, RenderError> {
        Self::upload_as(device, data, wgpu::BufferUsages::VERTEX)
    }

    /// As [`GpuMesh::upload`], with a vertex buffer the queue can rewrite —
    /// see [`crate::Renderer::update_vertices`].
    pub fn upload_dynamic(device: &wgpu::Device, data: &MeshData<'_>) -> Result<Self, RenderError> {
        Self::upload_as(
            device,
            data,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        )
    }

    fn upload_as(
        device: &wgpu::Device,
        data: &MeshData<'_>,
        vertex_usage: wgpu::BufferUsages,
    ) -> Result<Self, RenderError> {
        let n = data.positions.len();
        if data.normals.len() != n
            || data.uvs.len() != n
            || data.colors.is_some_and(|c| c.len() != n)
        {
            return Err(RenderError::BadMesh(format!(
                "attribute length mismatch: {} positions, {} normals, {} uvs, {:?} colors",
                n,
                data.normals.len(),
                data.uvs.len(),
                data.colors.map(<[_]>::len),
            )));
        }
        if let Some(&bad) = data.indices.iter().find(|&&i| i as usize >= n) {
            return Err(RenderError::BadMesh(format!(
                "index {bad} out of range for {n} vertices"
            )));
        }
        if !data.indices.len().is_multiple_of(3) {
            return Err(RenderError::BadMesh(format!(
                "index count {} is not a multiple of 3",
                data.indices.len()
            )));
        }

        let white = [255u8; 4];
        let vertices: Vec<Vertex> = (0..n)
            .map(|i| Vertex {
                position: data.positions[i],
                normal: data.normals[i],
                uv: data.uvs[i],
                color: data.colors.map_or(white, |c| c[i]),
            })
            .collect();

        let vertex_buffer = crate::made::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("mesh vb"),
                contents: bytemuck::cast_slice(&vertices),
                usage: vertex_usage,
            },
        );
        let index_buffer = crate::made::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("mesh ib"),
                contents: bytemuck::cast_slice(data.indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        );
        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: data.indices.len() as u32,
            max_joint: 0,
            bounds: aabb_of(data.positions.iter().copied()),
        })
    }

    pub fn upload_skinned(
        device: &wgpu::Device,
        data: &SkinnedMeshData<'_>,
    ) -> Result<Self, RenderError> {
        let n = data.positions.len();
        if data.normals.len() != n
            || data.uvs.len() != n
            || data.joints.len() != n
            || data.weights.len() != n
            || data.colors.is_some_and(|c| c.len() != n)
        {
            return Err(RenderError::BadMesh(format!(
                "skinned attribute length mismatch: {} positions, {} normals, {} uvs, {} joints, {} weights",
                n,
                data.normals.len(),
                data.uvs.len(),
                data.joints.len(),
                data.weights.len(),
            )));
        }
        if let Some(&bad) = data.indices.iter().find(|&&i| i as usize >= n) {
            return Err(RenderError::BadMesh(format!(
                "index {bad} out of range for {n} vertices"
            )));
        }
        if !data.indices.len().is_multiple_of(3) {
            return Err(RenderError::BadMesh(format!(
                "index count {} is not a multiple of 3",
                data.indices.len()
            )));
        }
        let max_joint = data
            .joints
            .iter()
            .flat_map(|j| j.iter().copied())
            .max()
            .unwrap_or(0);
        if usize::from(max_joint) >= MAX_JOINTS {
            return Err(RenderError::BadMesh(format!(
                "joint index {max_joint} exceeds the palette ({MAX_JOINTS} joints)"
            )));
        }

        let white = [255u8; 4];
        let vertices: Vec<SkinnedVertex> = (0..n)
            .map(|i| SkinnedVertex {
                position: data.positions[i],
                normal: data.normals[i],
                uv: data.uvs[i],
                color: data.colors.map_or(white, |c| c[i]),
                joints: data.joints[i],
                weights: data.weights[i],
            })
            .collect();
        let vertex_buffer = crate::made::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("skinned mesh vb"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
        let index_buffer = crate::made::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("skinned mesh ib"),
                contents: bytemuck::cast_slice(data.indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        );
        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: data.indices.len() as u32,
            max_joint: u32::from(max_joint),
            bounds: aabb_of(data.positions.iter().copied()),
        })
    }
}

/// Joints per skinned draw (one palette slot each). PS2 soldier rigs carry
/// ~26 animated bones; 96 leaves room for full skeletons with fingers.
pub const MAX_JOINTS: usize = 96;

/// Skinned vertex: `Vertex` + 4 joint indices + 4 weights (56 bytes).
/// Joint indices address the draw's palette (`SkinnedDrawCmd::palette`).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub joints: [u8; 4],
    pub weights: [f32; 4],
}

impl SkinnedVertex {
    // Locations 0..=3 mirror `Vertex`; 9/10 follow the instance attributes 4..=8.
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x3, // position
        1 => Float32x3, // normal
        2 => Float32x2, // uv
        3 => Unorm8x4,  // color
        9 => Uint8x4,   // joints
        10 => Float32x4, // weights
    ];

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkinnedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// SoA borrow of skinned CPU mesh data. `joints[i]` index the draw palette,
/// `weights[i]` should sum to 1 (unused slots: joint 0, weight 0).
#[derive(Debug, Clone, Copy)]
pub struct SkinnedMeshData<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: &'a [[f32; 3]],
    pub uvs: &'a [[f32; 2]],
    pub colors: Option<&'a [[u8; 4]]>,
    pub joints: &'a [[u8; 4]],
    pub weights: &'a [[f32; 4]],
    pub indices: &'a [u32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkinnedMeshHandle(pub(crate) u32);

// ── Dynamic meshes ───────────────────────────────────────────────────────────
//
// A mesh whose VERTICES change every frame and whose topology does not — the
// ocean's FFT patch (`game::water_anim`). The two methods sit here beside the
// buffer code rather than in `device.rs`'s Resources block; `Renderer`'s fields
// are visible to this child module.
impl crate::Renderer {
    /// As [`crate::Renderer::add_mesh`], but the vertex buffer can be
    /// rewritten afterwards with [`crate::Renderer::update_vertices`]. The
    /// indices are fixed at upload.
    pub fn add_dynamic_mesh(&mut self, data: &MeshData<'_>) -> Result<MeshHandle, RenderError> {
        let mesh = GpuMesh::upload_dynamic(&self.device, data)?;
        self.meshes.push(mesh);
        Ok(MeshHandle(self.meshes.len() as u32 - 1))
    }

    /// Replace every vertex of a mesh made by
    /// [`crate::Renderer::add_dynamic_mesh`] — one queue write, seen by the
    /// next frame submitted — and recompute the AABB frustum culling tests.
    /// `vertices` must be exactly as many as the upload had.
    pub fn update_vertices(
        &mut self,
        mesh: MeshHandle,
        vertices: &[Vertex],
    ) -> Result<(), RenderError> {
        let Some(m) = self.meshes.get_mut(mesh.0 as usize) else {
            return Err(RenderError::BadHandle(format!("mesh {}", mesh.0)));
        };
        if !m
            .vertex_buffer
            .usage()
            .contains(wgpu::BufferUsages::COPY_DST)
        {
            return Err(RenderError::BadMesh(format!(
                "mesh {} was not uploaded with add_dynamic_mesh",
                mesh.0
            )));
        }
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        if bytes.len() as u64 != m.vertex_buffer.size() {
            return Err(RenderError::BadMesh(format!(
                "{} vertices for mesh {}, which was uploaded with {}",
                vertices.len(),
                mesh.0,
                m.vertex_buffer.size() / std::mem::size_of::<Vertex>() as u64
            )));
        }
        self.queue.write_buffer(&m.vertex_buffer, 0, bytes);
        m.bounds = aabb_of(vertices.iter().map(|v| v.position));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_struct_sizes_match_the_shader_layout() {
        // 3+3+2 f32 + 4 u8 = 36 bytes, matching Vertex::layout() stride.
        assert_eq!(std::mem::size_of::<Vertex>(), 36);
        // The rigid path adds two vec4 region planes after the 124-byte prefix
        // shared with the skinned path.
        assert_eq!(std::mem::size_of::<Instance>(), 156);
        assert_eq!(std::mem::offset_of!(Instance, region_top), 124);
        assert_eq!(std::mem::offset_of!(Instance, region_bottom), 140);
        assert_eq!(Instance::rigid_layout().attributes[10].offset, 124);
        assert_eq!(Instance::rigid_layout().attributes[11].offset, 140);
        // Vertex + 4 u8 joints + 4 f32 weights = 56 bytes.
        assert_eq!(std::mem::size_of::<SkinnedVertex>(), 56);
        // Palette stride must satisfy the 256-B dynamic-offset alignment.
        assert_eq!((MAX_JOINTS * 64) % 256, 0);
    }

    /// The shader's palette array IS `MAX_JOINTS` long: raising the constant
    /// alone would let `upload_skinned` and `render_frame` accept joints the
    /// fixed WGSL array cannot index — silently wrong skinning.
    #[test]
    fn the_shader_palette_holds_max_joints() {
        let decl = format!("array<mat4x4<f32>, {MAX_JOINTS}>");
        assert!(
            include_str!("shader.wgsl").contains(&decl),
            "shader.wgsl's palette is not `{decl}`"
        );
    }
}
