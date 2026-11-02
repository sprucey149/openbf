//! Shared fixtures for the render crate's GPU referees.
//!
//! The eleven test modules of the suite binary — `alpha_blend`, `detail_map`, `frame_filter`, `gpu_smoke`,
//! `light_path`, `lod_switch`, `skinned`, `sky_shell`, `specular`, `uv_scroll`,
//! `water_pass` — all build the same
//! three things before they can assert anything: a 64x64 headless renderer,
//! a view-spanning textured quad, and a pixel read out of the readback buffer.
//! One copy of each lives here so the fixtures cannot drift apart.
//!
//! [`headless`] is the same constructor call every test already made, not a
//! shared device. Each renderer holds a turn at the GPU for its whole life
//! (`gpu_turns`), so these tests wait for each other whatever the thread count.

use render::{
    MeshData, MeshHandle, Renderer, SamplerMode, SkinnedMeshData, SkinnedMeshHandle, TextureDesc,
    TextureHandle, WrapV,
};

/// Every referee renders at this size.
pub const N: u32 = 64;

/// The renderer each referee opens: `N x N`, headless, its own wgpu device.
pub fn headless() -> Renderer {
    Renderer::new_headless(N, N).expect("headless renderer")
}

/// The pixel at `(x, y)` of a captured RGBA buffer `w` texels wide.
pub fn pixel(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

/// Capture the frame just rendered and read one pixel out of it.
pub fn capture_pixel(r: &mut Renderer, x: u32, y: u32) -> [u8; 4] {
    let (buf, w, _) = r.capture_rgba().expect("capture");
    pixel(&buf, w, x, y)
}

/// A nearest-sampled sRGB upload with every optional slot off — the shape the
/// fixtures want. Set `detail`, `scroll` or `wrap_v` on the returned struct
/// for the three that test one of those slots.
pub fn tex_desc<'a>(
    width: u32,
    height: u32,
    mips: &'a [&'a [u8]],
    label: &'a str,
) -> TextureDesc<'a> {
    TextureDesc {
        width,
        height,
        mips,
        sampler: SamplerMode::Nearest,
        wrap_v: WrapV::Repeat,
        srgb: true,
        detail: None,
        scroll: [0.0, 0.0],
        uv_scale: [1.0, 1.0],
        label: Some(label),
    }
}

/// [`tex_desc`] uploaded: one mip level, no detail map, no scroll.
pub fn upload_tex(
    r: &mut Renderer,
    width: u32,
    height: u32,
    texels: &[u8],
    label: &str,
) -> TextureHandle {
    r.add_texture(&tex_desc(width, height, &[texels], label))
        .expect("texture upload")
}

/// The quad the pass fixtures draw: 8x8 at `z`, facing the camera, uv 0..1
/// with v = 0 at the TOP of the frame. At [`render::Camera::default`] it
/// spans the whole view from z = -1 out to z = -5.
pub fn view_quad(r: &mut Renderer, z: f32) -> MeshHandle {
    r.add_mesh(&MeshData {
        positions: &[
            [-4.0, -4.0, z],
            [4.0, -4.0, z],
            [4.0, 4.0, z],
            [-4.0, 4.0, z],
        ],
        normals: &[[0.0, 0.0, 1.0]; 4],
        uvs: &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        colors: None,
        indices: &[0, 1, 2, 0, 2, 3],
    })
    .expect("mesh upload")
}

/// A skinned quad filling the view at `z` (the shape of `common::view_quad`),
/// every vertex on joint 0 alone, so the identity palette leaves it in place.
pub fn skinned_quad(r: &mut Renderer, z: f32) -> SkinnedMeshHandle {
    r.add_skinned_mesh(&SkinnedMeshData {
        positions: &[
            [-4.0, -4.0, z],
            [4.0, -4.0, z],
            [4.0, 4.0, z],
            [-4.0, 4.0, z],
        ],
        normals: &[[0.0, 0.0, 1.0]; 4],
        uvs: &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        colors: None,
        joints: &[[0, 0, 0, 0]; 4],
        weights: &[[1.0, 0.0, 0.0, 0.0]; 4],
        indices: &[0, 1, 2, 0, 2, 3],
    })
    .expect("skinned mesh upload")
}
