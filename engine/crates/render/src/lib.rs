//! Reusable wgpu renderer for the SWBF engine (P2: baked-map viewer).
//!
//! Two modes behind one `Renderer`:
//! - `Renderer::new_windowed(window)` — renders to a winit window surface.
//! - `Renderer::new_headless(w, h)` — renders to an offscreen `Rgba8UnormSrgb`
//!   texture with no window/display required (Vulkan compute-style adapter
//!   request), read back via [`Renderer::capture_rgba`] for PNG goldens.
//!
//! Adapted from the maintainer's existing wgpu-27-generation code (cited per item):
//! - device/adapter/surface setup: `an earlier project's source`
//! - camera math: `an earlier project's source` (see `camera.rs`)
//! - mesh/pipeline/bind-group shape: `an earlier project's source`
//! - 256-byte-padded readback: `an earlier project's source` `read_pixels`
//!
//! Coordinate convention: Y-up, right-handed, -Z forward — documented in [`camera`].
//!
//! **Sky shells** ([`SkyShell`], [`Placement`], [`SkyMode`]) follow both ELFs'
//! own rules as far as `analysis/re-sky-dome-placement.md` reads them: the
//! `DomeModel` Y lerp with no horizontal lag (§3/§4 [V]) and the per-material
//! GS state (§5 [V]). They are never FOGGED — the sky draw flag 0x20 excludes
//! them from the fog pass in both games (`analysis/re-fog.md` §2.4 [V]). A
//! `Reflect` shell (BF1 KAS1 / KAS2) is drawn mirrored under the eye just
//! before itself — [`Placement::Mirror`], `analysis/re-sky-reflect.md` §2/§5
//! [O] — Y-squashed by [`mirror_squash`] of [`Renderer::time`].
//! **NOT BUILT, and nothing here stands in for it:** `RotationSpeed` — the
//! yaw a BF2 shell spins by, which the bake does not parse out of the `sky_`
//! chunk (§8.2). The sky pass's DEPTH is also ours rather than
//! the disc's (PROVISIONAL): the ELF Z-writes an opaque shell inside its far-scene range
//! (§6 [H], not closed) while the port draws the sky first with depth off,
//! which is the same picture for everything drawn after it.
//!
//! ## Where the code lives
//!
//! Split by queue SPL6 (2026-09-24, a pure move) into seven parts, each
//! re-exported here so every `render::X` path is unchanged: `lighting` (the
//! sun, the ambient, the fog and the far fade), `sky` (the sky shells, their
//! material mode and their placement), `draw` (the draw list: `DrawCmd`,
//! `DrawLayer`, `SkinnedDrawCmd`), `device` (the constructors, the adapter,
//! the target, the uploads), `pipelines` (`finish_init` and `PipelineDesc`:
//! every pipeline), `frame` (`render_frame`: the draw order, the instance and
//! palette buffers, the passes) and `capture` (the headless readback and the
//! PNG encoders), plus `specular` (queue SPEC: the specular second pass's
//! table, its per-frame parameters and its tex-gen's CPU twin) and `grass`
//! (queue FOLGG: the grass patches' per-view sprite buffer and pass) and `bolt`
//! (card FXB: the laser bolts' two ribbons), `particles` (card FXP2: the
//! particle effects' shapes and the far-to-near list they share with the
//! bolts), `terrain_light` (card FXBL: BF2's omni lights projected onto
//! the terrain), `point_light` (card BPNT: BF2's local lights on models —
//! the list, the per-model gather, the shader's table), `leaf` (card LEAF:
//! the leaf patches, on the grass's buffers and pipeline), `filter` (card
//! BFLT: the DomeInfo frame filter), `light_beam` (card CPHOL: the
//! "lightbeam" particle's eye-facing fan), `affine` (BF2's screen-space
//! vertex-colour blend: the guard band, the layer bit and their CPU twin)
//! and `hud` (card BHUD2: the HUD pass). `hud` and `affine` are public
//! modules (`render::hud`, `render::affine`); every other part is reached
//! through its re-exports. This file
//! keeps what the parts share — `Renderer` itself
//! (the parts are child modules, so they read its private fields without
//! widening), its `Target`, the shader's `Globals` and `PALETTE_STRIDE` —
//! plus `RenderError` and `TIME_WRAP`.

pub mod affine;
mod bindings;
mod bloom;
mod bolt;
mod camera;
mod capture;
mod device;
mod draw;
mod falling;
mod filter;
mod frame;
mod frame_ab;
mod frustum;
mod gpu_timer;
mod grass;
pub mod hud;
mod leaf;
mod light_beam;
mod lighting;
pub mod made;
mod mesh;
mod particles;
mod pipelines;
mod point_light;
mod resources;
mod shadow;
pub use shadow::ShadowMesh;
mod sky;
mod specular;
mod stars;
mod terrain_light;
mod texture;
mod world_scale;

pub use bloom::Bloom;
pub use bolt::{BoltDraw, RibbonVertex};
pub use camera::Camera;
pub use capture::{encode_png, encode_png_rgb};
pub use device::HEADLESS_FORMAT;
pub use draw::{
    fragment_kept, DrawCmd, DrawLayer, DrawOrder, Fade, LightPath, LodSide, LodSwitch,
    SkinnedDrawCmd, IFACE_AMBIENT, IFACE_DIR_CAMERA,
};
pub use falling::{ForestBirdType, LeafActor, LeafEmitter};
pub use filter::{box_side, FrameFilter};
pub use frame::{FrameStats, OverlayFn};
pub use frame_ab::FrameAb;
pub use grass::{sprite_quad, sway, GrassClass, GrassPatch, GrassSprite, ViewAxes};
pub use leaf::{LeafClass, LeafObject, LeafSprite};
pub use light_beam::{Beam, Cross, TOP_ALPHA as BEAM_TOP_ALPHA};
pub use lighting::{
    encode as srgb_encode, light_sum, DirLight, FarFade, Fog, Lighting, WaterShading,
};
pub use mesh::{
    MeshData, MeshHandle, SkinnedMeshData, SkinnedMeshHandle, SkinnedVertex, Vertex, MAX_JOINTS,
};
pub use particles::{ParticleDraw, ParticleShape};
pub use point_light::{LitSphere, PointLight, SpotCone, MAX_POINT_LIGHTS};
pub use sky::{mirror_squash, Placement, SkyMode, SkyShell, MIRROR_LIFT};
pub use specular::spec_texcoord;
pub use stars::{half_size as star_half_size, star_rgb, Star, StarField};
pub use terrain_light::{
    LitPatch, TerrainLight, TerrainLights, LIGHTS_PER_PATCH, MAX_LIT_PATCHES, MAX_TERRAIN_LIGHTS,
};
pub use texture::{DetailDesc, SamplerMode, TextureDesc, TextureHandle, WrapV};

use mesh::GpuMesh;
use std::sync::Arc;
use texture::GpuTexture;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no suitable GPU adapter: {0}")]
    Adapter(String),
    #[error("device creation failed: {0}")]
    Device(String),
    #[error("surface error: {0}")]
    Surface(String),
    #[error("bad mesh data: {0}")]
    BadMesh(String),
    #[error("bad texture data: {0}")]
    BadTexture(String),
    #[error("invalid handle: {0}")]
    BadHandle(String),
    #[error("operation requires a headless renderer")]
    HeadlessOnly,
    #[error("readback failed: {0}")]
    Readback(String),
    #[error("png encode failed: {0}")]
    Png(String),
}

/// Bytes per skinned draw in the palette buffer (a multiple of 256, the
/// dynamic-offset alignment).
const PALETTE_STRIDE: usize = MAX_JOINTS * 64;

/// GPU-side globals, layout mirrored in shader.wgsl `Globals` (368 bytes —
/// `globals_matches_the_wgsl_uniform_size` pins it).
/// Pattern from an earlier wgpu project `CameraUniform` (camera_m.rs): repr(C) + explicit pads.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    /// [`affine::guard`]'s x, for the shader's `affine_w`.
    guard_x: f32,
    sun_dir: [f32; 3],
    /// 1 = BF1's `((1 + n·l)/2)⁴` sun term ([`Lighting::sun_d4`]), else 0.
    sun_d4: f32,
    sun_color: [f32; 3],
    /// [`affine::guard`]'s y.
    guard_y: f32,
    ambient_sky: [f32; 3],
    /// BF2 material animation seconds, wrapped at 255.
    material_time: f32,
    ambient_ground: [f32; 3],
    /// 0 = fog off, 1 = fog on (a bool in a uniform is a float).
    fog_on: f32,
    fog_color: [f32; 3],
    fog_near: f32,
    fog_far: f32,
    /// Seconds, WRAPPED — see [`Renderer::time`].
    time: f32,
    /// [`FarFade`]'s b and c, metres. `fade_end <= fade_begin` (0, 0 when the
    /// map has no fade) is the OFF state the shader tests — the pair sits in
    /// what used to be two pad floats, so the uniform's size is unchanged.
    fade_begin: f32,
    fade_end: f32,
    /// [`Lighting::water_fog_begin`], metres. **0 = the water is never
    /// fogged**, which is the state every non-water map is in and the one the
    /// shader tests; no map's water crossover is 0 (it is `NearSceneRange`'s
    /// c, ≥ 20 m on all twelve water maps).
    water_fog_begin: f32,
    /// [`Lighting::water_lit`]: the weight of the sun's diffuse term on a water
    /// draw; **0 = unlit**, the state the shader tests.
    water_lit: f32,
    /// [`Lighting::water_dim`]: begin (metres of planar depth) and the loss
    /// per metre past it. Rate 0 = no dim.
    water_dim_begin: f32,
    water_dim_rate: f32,
    /// [`DirLight`]'s direction; zero colour when the map has no Light2.
    light2_dir: [f32; 3],
    /// 1 = the sun is `Static()` ([`Lighting::sun_static`]), else 0.
    sun_static: f32,
    light2_color: [f32; 3],
    /// 1 = Light2 is `Static()`, else 0.
    light2_static: f32,
    /// [`WaterShading`], field for field, each vec3 padded to a row.
    water_eye_shift: [f32; 3],
    /// [`Lighting::glow_gain`].
    glow_gain: f32,
    water_specular: [f32; 4],
    speckle_ambient: [f32; 4],
    speckle_specular: [f32; 4],
    speckle_axis: [f32; 3],
    _pad8: f32,
    speckle_light: [f32; 3],
    _pad9: f32,
    speckle_shift: [f32; 3],
    _pad10: f32,
    /// [`Lighting::character_ambient`], read by [`LightPath::Character`]
    /// draws when `character_ambient_on` is 1.
    character_ambient: [f32; 3],
    character_ambient_on: f32,
    /// [`IFACE_DIR_CAMERA`] in the world, for [`LightPath::Iface`] draws.
    iface_dir: [f32; 3],
    _pad11: f32,
}

/// The world's depth-writing pipelines with the shader's `CULLS` off, for a draw
/// that cannot discard this frame (`draw::may_discard`). A shader that may
/// discard writes depth only after it has shaded, so the next draw loses the
/// early reject its pixels would have had.
struct NoDiscard {
    opaque: wgpu::RenderPipeline,
    mask: wgpu::RenderPipeline,
    fade: wgpu::RenderPipeline,
    fade_mask: wgpu::RenderPipeline,
    terrain_layer: wgpu::RenderPipeline,
    water: wgpu::RenderPipeline,
    skinned: wgpu::RenderPipeline,
    fade_skinned: wgpu::RenderPipeline,
}

enum Target {
    Window {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Headless {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
    },
}

pub struct Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    target: Target,
    /// Color format of the render target (surface or headless texture).
    format: wgpu::TextureFormat,
    depth_view: wgpu::TextureView,
    shadows: shadow::ShadowState,
    pipeline: wgpu::RenderPipeline,
    /// The opaque pipeline through `fs_mask`: the same depth state and the
    /// same instanced sort, plus the shader's MASK test (`MASK_CUTOFF`).
    mask_pipeline: wgpu::RenderPipeline,
    /// Alpha-composited pass for materials the bake judged BLEND.
    blend_pipeline: wgpu::RenderPipeline,
    /// The WATER pass: alpha-composited with the depth WRITTEN and a
    /// `LessEqual` test — the PS2's near main water record. Built once, used
    /// only by the twelve maps that ship a `WATR`; see `finish_init`
    /// (`pipelines.rs`).
    water_pipeline: wgpu::RenderPipeline,
    /// The water's SPECKLE layer ([`DrawLayer::Speckle`]): additive, depth
    /// tested `LessEqual`, no depth write — see `finish_init`.
    speckle_pipeline: wgpu::RenderPipeline,
    /// The `LOWR` sheet's pass ([`DrawLayer::FarSheet`]): the world pass's
    /// state with `fs_far_sheet` — unlit, tested at the sheet's own AREF.
    far_sheet_pipeline: wgpu::RenderPipeline,
    /// The terrain layers' pass ([`DrawLayer::TerrainLayer`]): src-alpha
    /// "over", `LessEqual`, no depth write, `fs_terrain_layer`.
    terrain_layer_pipeline: wgpu::RenderPipeline,
    /// The depth-writing passes again without `far_cull`'s discards.
    no_discard: NoDiscard,
    /// The five pipelines a map with a [`FarFade`] uses instead of
    /// `pipeline` / `mask_pipeline` / `blend_pipeline` / `skinned_pipeline` /
    /// `skinned_blend_pipeline`: the same state with the target BLENDED, so
    /// the fade alpha the `_fade` fragment entry points write composites the
    /// near scene over the sky already in the framebuffer. Nothing selects
    /// them on a map without a fade, so every other map's picture is
    /// bit-for-bit what it was.
    fade_pipeline: wgpu::RenderPipeline,
    fade_mask_pipeline: wgpu::RenderPipeline,
    fade_blend_pipeline: wgpu::RenderPipeline,
    fade_skinned_pipeline: wgpu::RenderPipeline,
    fade_skinned_blend_pipeline: wgpu::RenderPipeline,
    /// Unlit, depth-free pass for the map's sky shells, indexed
    /// `[SkyMode::index()]` — see [`SkyMode`] and `finish_init` (`pipelines.rs`).
    sky_pipelines: [wgpu::RenderPipeline; 2],
    /// The map's sky shells, redrawn around the camera every frame.
    sky: Vec<SkyShell>,
    /// What the last frame actually submitted. Read by `--bench`: the number
    /// that moves when culling or batching changes is DRAW CALLS, not the
    /// length of the draw list.
    frame_stats: FrameStats,
    /// GPU time per section of the frame, when `OPENBF_GPU_TIMING=1` and the
    /// adapter has timestamp queries (`gpu_timer.rs`); read by
    /// [`Renderer::gpu_sections`].
    gpu_timer: Option<gpu_timer::GpuTimer>,
    /// CPU ms the last `render_frame` waited to acquire the window's frame
    /// (the vsync / frame-latency wait); 0 headless.
    acquire_ms: f64,
    /// Frustum culling on/off. On by default; `--no-cull` in the bench turns
    /// it off so the two can be compared on the same scene.
    pub cull: bool,
    /// How the next frame differs from the game's own (`frame_ab`, the
    /// bench's pricing states).
    pub frame_ab: FrameAb,
    /// The fraction of the window the world draws at (`world_scale`).
    world_scale_now: f32,
    world_scale: world_scale::WorldScale,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    /// BF2's local lights (`point_light.rs`, card BPNT): the shader's table
    /// (binding 1 of the globals group), the frame's lights
    /// ([`Renderer::set_point_lights`]) and whether the table holds none.
    points_buf: wgpu::Buffer,
    point_lights: Vec<PointLight>,
    point_table_clear: bool,
    texture_bgl: wgpu::BindGroupLayout,
    meshes: Vec<GpuMesh>,
    textures: Vec<GpuTexture>,
    /// 1x1 white texture serving `DrawCmd { texture: None, .. }`.
    white: GpuTexture,
    instance_buf: wgpu::Buffer,
    instance_capacity: usize,
    // Skinned path: own pipelines (vs_skinned), meshes, per-draw palettes in a
    // dynamic-offset uniform (PALETTE_STRIDE bytes per draw), own instance buffer.
    skinned_pipeline: wgpu::RenderPipeline,
    /// The skinned [`DrawLayer::Blend`] pass: `skinned_pipeline` composited,
    /// with the depth write OFF — see `finish_init` (`pipelines.rs`).
    skinned_blend_pipeline: wgpu::RenderPipeline,
    skinned_meshes: Vec<GpuMesh>,
    palette_bgl: wgpu::BindGroupLayout,
    palette_buf: wgpu::Buffer,
    palette_bg: wgpu::BindGroup,
    /// Skinned draws the palette/instance buffers can hold.
    palette_capacity: usize,
    /// The frame's palettes on their way to `palette_buf`, kept between frames.
    palette_upload: Vec<u8>,
    skinned_instance_buf: wgpu::Buffer,
    /// The specular second pass ([`DrawLayer::Spec`], `specular.rs`): its
    /// additive pipeline, the layout of its group 3, the per-frame
    /// `SpecParams` buffer, and the table's bind group — `None` until
    /// [`Renderer::set_specular_table`] installs one, and while it is `None`
    /// every `Spec` draw is skipped.
    spec_pipeline: wgpu::RenderPipeline,
    /// `spec_pipeline` through `vs_spec_skinned`: a soldier's or a side
    /// model's second pass, skinned (queue SPEC2).
    spec_skinned_pipeline: wgpu::RenderPipeline,
    spec_bgl: wgpu::BindGroupLayout,
    /// The grass patches (`grass.rs`, queue FOLGG): their pipeline (vs/fs
    /// `grass.wgsl`, blended, no depth write) and the scene's classes and
    /// patches, empty until [`Renderer::set_grass`].
    grass_pipeline: wgpu::RenderPipeline,
    grass: grass::GrassState,
    leaves: leaf::LeafState,
    falling: falling::FallingState,
    /// The laser bolts (`bolt.rs`, card FXB): the core's NORMAL and the
    /// glow's ADDITIVE pipeline (vs `grass.wgsl`, fs `bolt.wgsl`, no depth
    /// write) and this frame's bolts, set by [`Renderer::set_bolts`].
    bolt_core_pipeline: wgpu::RenderPipeline,
    bolt_glow_pipeline: wgpu::RenderPipeline,
    bolt_solid_pipeline: wgpu::RenderPipeline,
    leaf_cutout_pipeline: wgpu::RenderPipeline,
    particle_display: Option<bolt::DisplayPipelines>,
    bolts: bolt::BoltState,
    /// The sky's star field (`stars.rs`, issue #16): vs `grass.wgsl`, fs
    /// `fs_bolt_glow`, additive with no depth, in the sky pass; the
    /// field set by [`Renderer::set_stars`].
    star_pipeline: wgpu::RenderPipeline,
    stars: stars::StarState,
    /// This frame's particle effects (`particles.rs`, card FXP2), set by
    /// [`Renderer::set_particles`]; drawn with the bolts' pipelines.
    particles: particles::ParticleState,
    /// BF2's terrain light pass (`terrain_light.rs`, card FXBL): additive
    /// over the frame's copy, `LessEqual`, no depth write; its group-1
    /// layout and this frame's lights ([`Renderer::set_terrain_lights`]).
    terrain_light_pipeline: wgpu::RenderPipeline,
    terrain_light_bgl: wgpu::BindGroupLayout,
    terrain_lights: terrain_light::TerrainLightState,
    /// The DomeInfo frame filter (`filter.rs`, card BFLT): its pipeline,
    /// this map's values ([`Renderer::set_frame_filter`]) and the frame copy.
    filter: filter::FilterState,
    bloom: bloom::BloomState,
    spec_buf: wgpu::Buffer,
    spec_bg: Option<wgpu::BindGroup>,
    /// The disc's mirrored specular light (`analysis/re-specular-pass.md` §4,
    /// `[O]`): on by default; off takes the consistent `d = L·R`. PROVISIONAL
    /// — how the mismatch LOOKS on the PS2 wants an in-map capture.
    pub spec_mirror: bool,
    pub lighting: Lighting,
    /// Animation clock in seconds: the water surface's UV scroll, the water's
    /// speckle, the mirrored sky shell's squash ([`mirror_squash`]) and the
    /// grass and leaf sway.
    ///
    /// **Wrapped at [`TIME_WRAP`]** by the caller, because
    /// `uv + velocity * time` loses precision once `time` grows: an f32 has ~7
    /// significant digits, so an hour-long session at 0.1 UV/s would quantise
    /// the scroll visibly. The wrap period is chosen so it is SEAMLESS for the
    /// shipped data: every water scroll the bake writes (`fx__ Velocity /
    /// INFO5` — 0.1, 0.05, 0.02, 0.0125, 0.01, 0.001 and zero UV/s over the
    /// twelve water maps) times 2000 s is a whole number of texture repeats,
    /// so the reset lands exactly where the scroll already was. That holds
    /// for the scrolls only: the sky's squash and the grass and leaf sway
    /// restart their phase at the wrap, a one-frame jump every 2000 s (said
    /// at [`mirror_squash`], `grass::sway` and `leaf::sway`).
    ///
    /// Default 0.0 — headless captures and the goldens leave it there, so a
    /// still frame is reproducible.
    pub time: f32,
    /// Unwrapped render-side water seconds. FFT frequencies have no common
    /// `TIME_WRAP` period; f64 keeps their phase continuous in long sessions.
    /// Default zero preserves still captures. The simulation never reads it.
    pub water_time: f64,
    /// Which GPU this renderer actually got. Reported by `--bench`, because a
    /// timing number without the adapter beside it means nothing.
    adapter_info: wgpu::AdapterInfo,
    /// Held until every field above has dropped its share of the device, so
    /// this field stays last.
    _gpu_turn: device::GpuTurn,
}

/// Wrap period for [`Renderer::time`], in seconds. See that field for why
/// 2000 is seamless for every water scroll the two discs author (1000 was
/// not: BF1 KAS2's 0.0125 UV/s left the reset half a repeat out, a visible
/// jump every 1000 s of play).
pub const TIME_WRAP: f32 = 2000.0;

/// One frame of the renderer's two clocks: [`Renderer::time`] wraps at
/// [`TIME_WRAP`], [`Renderer::water_time`] does not. Returns the new
/// `(time, water_time)`.
pub fn advance_clocks(time: f32, water_time: f64, dt: f32) -> (f32, f64) {
    ((time + dt) % TIME_WRAP, water_time + f64::from(dt))
}

/// BF2 rates are integer bytes / 255: this period wraps by whole repeats.
/// Takes the unwrapped water clock, so the [`TIME_WRAP`] reset of `time` cannot jump a material.
pub fn material_scroll_time(seconds: f64) -> f32 {
    seconds.rem_euclid(255.0) as f32
}

impl Renderer {
    /// Drop the map's look: everything drawn without a draw command (the
    /// sky, stars, grass, leaves, lights, effects, filter) and its lighting.
    /// A frame after this is the clear colour plus its draws, as before any
    /// map was loaded (the shell after a battle).
    pub fn clear_scene(&mut self) {
        self.set_sky(Vec::new());
        self.set_stars(None, 0);
        self.set_grass(Vec::new(), Vec::new());
        self.set_leaves(Vec::new(), Vec::new(), None);
        self.set_point_lights(Vec::new());
        self.set_bolts(Vec::new());
        self.set_particles(Vec::new());
        self.set_terrain_lights(Default::default());
        self.set_frame_filter(None);
        self.set_bloom(None);
        self.set_leaf_fall(Vec::new(), None, None);
        self.lighting = Lighting::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_clock_is_continuous_at_the_water_clock_wrap() {
        for byte in [1, 50, 200, 255] {
            let rate = -f64::from(byte) / 255.0;
            for t in [254.99, 255.01, 1999.99, 2000.01, 36000.0] {
                let expected = (rate * t).rem_euclid(1.0);
                let got = (rate * f64::from(material_scroll_time(t))).rem_euclid(1.0);
                let error = (expected - got).abs();
                assert!(
                    error.min(1.0 - error) < 0.00002,
                    "{byte} at {t}: {got} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn globals_matches_the_wgsl_uniform_size() {
        // mat4 (64) + seven 16-byte rows (camera, sun dir, sun colour, sky
        // ambient, ground ambient + fog_on, fog colour + fog_near, fog_far +
        // time + the two fade distances) = 176 bytes, plus the row the
        // water's fog crossover opened (its begin, then the lit weight and the
        // two dim terms that took its three pads) = 192, plus Light2's two rows
        // (direction + the sun's Static flag, colour + its own) = 224, plus
        // the water's seven ([`WaterShading`]: the eye shift, the specular,
        // the two speckle colours, the speckle's axis, light and shift) = 336,
        // plus the character ambient's row (colour + its on flag) = 352, plus
        // the interface light's direction = 368.
        assert_eq!(std::mem::size_of::<Globals>(), 368);
        assert_eq!(std::mem::size_of::<Globals>() % 16, 0);
    }
}
