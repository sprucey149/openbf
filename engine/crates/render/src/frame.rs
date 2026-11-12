//! One frame: `Renderer::render_frame` (and its `render` and
//! `render_with_overlay` shorthands) checks the draw lists, uploads the
//! globals, frustum-culls, sorts the static draws into instanced runs
//! (`draw_key`, `sort_runs`) and the blended ones — world and skinned together — far to
//! near (`blend_order`), fills the instance and palette buffers (growing them
//! on demand), then records the passes — the sky, the opaque / mask runs,
//! the skinned opaque draws, the water runs over them (card WADE), the grass
//! and leaf patches (`grass.rs`,
//! `leaf.rs`), the blend
//! pass, the laser bolts and particle effects (`bolt.rs`, `particles.rs`)
//! — and the overlay; the
//! specular runs sit between the masked and the water ones, the water's
//! speckle runs right after the water; BF2's terrain light pass
//! (`terrain_light.rs`) interrupts the runs after the terrain's layers — the
//! frame is copied and the pass drawn in its own render pass — and the map
//! pass resumes; the DomeInfo frame filter (`filter.rs`) runs over a copy of
//! the finished 3D frame before the overlay;
//! [`FrameStats`] is what it submitted. The draw-order tests live here.

mod submit;

use crate::gpu_timer::GpuTimer;
use crate::mesh::{GpuMesh, Instance};
use crate::{
    Camera, DrawCmd, DrawLayer, LightPath, RenderError, Renderer, SkinnedDrawCmd, TextureHandle,
    PALETTE_STRIDE,
};

/// What the last `render_frame` submitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// `draw_indexed` calls issued, across every pass.
    pub draw_calls: u32,
    /// Draws whose world AABB was entirely outside the frustum, or whose
    /// distance bands discard every fragment (`draw::all_discarded`).
    pub culled: u32,
    /// Draws submitted (list length minus `culled`).
    pub submitted: u32,
}

/// Overlay hook: draws UI into the frame after the 3D pass, before submit.
/// Args: device, queue, frame encoder, color view, (width, height).
pub type OverlayFn<'a> = &'a mut dyn FnMut(
    &wgpu::Device,
    &wgpu::Queue,
    &mut wgpu::CommandEncoder,
    &wgpu::TextureView,
    [u32; 2],
);

impl Renderer {
    // ── Frame ────────────────────────────────────────────────────────────────

    /// Render one frame. `clear` is LINEAR RGBA (the sRGB target encodes it);
    /// its alpha is ignored — the frame is always opaque (`FRAME_WRITES`).
    /// Windowed mode presents; a lost/outdated surface is rebuilt at the
    /// window's size (left alone while minimized) and the frame is skipped
    /// (returns Ok) — the next frame draws normally.
    pub fn render(
        &mut self,
        camera: &Camera,
        draws: &[DrawCmd],
        clear: [f64; 4],
    ) -> Result<(), RenderError> {
        self.render_with_overlay(camera, draws, clear, None)
    }

    /// What the last frame submitted — draw calls, and how many draws the
    /// frustum removed.
    pub fn frame_stats(&self) -> FrameStats {
        self.frame_stats
    }

    /// GPU ms per section of the newest timed frame, in drawing order, when
    /// `OPENBF_GPU_TIMING=1` and the adapter has timestamp queries (empty
    /// otherwise, and until the first readback). A frame or two old.
    pub fn gpu_sections(&self) -> &[(&'static str, f64)] {
        self.gpu_timer.as_ref().map_or(&[], |t| t.last())
    }

    /// Whether this renderer's frames are timed (`OPENBF_GPU_TIMING=1`, on an
    /// adapter with timestamp queries).
    pub fn gpu_timing_on(&self) -> bool {
        self.gpu_timer.is_some()
    }

    /// Draw the world at this fraction of the window (1 = the window's own size,
    /// the default; see `world_scale`). The HUD stays at the window's size.
    pub fn set_world_scale(&mut self, scale: f32) {
        self.world_scale_now = scale.clamp(0.25, 1.0);
    }

    pub fn set_far_range(&mut self, far: f32) {
        self.far_range = far;
    }

    /// The fraction of the window the world draws at.
    pub fn world_scale(&self) -> f32 {
        self.world_scale_now
    }

    /// Read the timer's finished frames now, for a caller that waited for the
    /// GPU and wants the frame it just drew from [`Self::gpu_sections`].
    pub fn collect_gpu_timing(&mut self) {
        if let Some(t) = &mut self.gpu_timer {
            t.collect(&self.device);
        }
    }

    /// CPU ms the last frame waited for the window's next image (vsync and
    /// the frame-latency limit); 0 headless.
    pub fn acquire_ms(&self) -> f64 {
        self.acquire_ms
    }

    /// The bind group a draw's texture handle resolves to. `None` — and an
    /// out-of-range handle — take the 1x1 white texture. `render_frame`
    /// rejects a bad handle in `draws` and `skinned` with `BadHandle` before
    /// any pass starts; the sky shells, grass, leaves, bolts and particles
    /// are not checked, so a bad handle in one of those draws white.
    fn tex_bg(&self, texture: Option<TextureHandle>) -> &wgpu::BindGroup {
        match texture.and_then(|t| self.textures.get(t.0 as usize)) {
            Some(t) => &t.bind_group,
            None => &self.white.bind_group,
        }
    }

    /// Record skinned draw `d`, submitted at index `i`: its sheet, then its
    /// palette and its instance, both of which sit at the SUBMISSION index.
    /// The pipeline, the globals and the skinned instance buffer are the
    /// caller's to bind.
    fn draw_sky_slot(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        slot: usize,
        pipeline: &wgpu::RenderPipeline,
    ) -> u32 {
        let shell = &self.sky[slot];
        let Some(mesh) = self.meshes.get(shell.mesh.0 as usize) else {
            return 0;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, self.tex_bg(shell.texture), &[]);
        draw_mesh(pass, mesh, slot as u32..slot as u32 + 1);
        1
    }

    fn draw_skinned(&self, pass: &mut wgpu::RenderPass<'_>, d: &SkinnedDrawCmd, i: usize) {
        pass.set_bind_group(1, self.tex_bg(d.texture), &[]);
        pass.set_bind_group(2, &self.palette_bg, &[(i * PALETTE_STRIDE) as u32]);
        let inst = i as u32;
        draw_mesh(
            pass,
            &self.skinned_meshes[d.mesh.0 as usize],
            inst..inst + 1,
        );
    }

    /// Color format of the frame the overlay paints into.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// `render` plus an optional overlay pass (egui HUD/shell) recorded into
    /// the same frame after the 3D pass.
    pub fn render_with_overlay(
        &mut self,
        camera: &Camera,
        draws: &[DrawCmd],
        clear: [f64; 4],
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<(), RenderError> {
        self.render_frame(camera, draws, &[], clear, overlay)
    }
}

pub(super) fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    crate::resources::buffer(
        device,
        "instances",
        (capacity * std::mem::size_of::<Instance>()) as u64,
        wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    )
}

impl Renderer {
    fn draw_particle_runs(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        runs: &[crate::bolt::BoltRun],
        display: bool,
    ) -> u32 {
        let (Some((vbuf, _)), Some((ibuf, _))) = (&self.bolts.vertex_buf, &self.bolts.instance_buf)
        else {
            return 0;
        };
        if runs.is_empty() {
            return 0;
        }
        pass.set_bind_group(0, &self.globals_bg, &[]);
        pass.set_vertex_buffer(0, vbuf.slice(..));
        pass.set_vertex_buffer(1, ibuf.slice(..));
        let shown = self.particle_display.as_ref().filter(|_| display);
        let mut bound = None;
        let mut calls = 0;
        for run in runs {
            let kind = (run.glow, run.solid, run.cutout, run.premultiplied);
            if bound != Some(kind) {
                pass.set_pipeline(match (shown, kind) {
                    (Some(d), (true, _, _, _)) => &d.glow,
                    (Some(d), (false, true, _, _)) => &d.solid,
                    (Some(d), (false, false, true, _)) => &d.cutout,
                    (Some(d), (false, false, false, true)) => &d.atmosphere,
                    (Some(d), (false, false, false, false)) => &d.core,
                    (None, (true, _, _, _)) => &self.bolt_glow_pipeline,
                    (None, (false, true, _, _)) => &self.bolt_solid_pipeline,
                    (None, (false, false, true, _)) => &self.leaf_cutout_pipeline,
                    (None, (false, false, false, true)) => &self.atmosphere_pipeline,
                    (None, (false, false, false, false)) => &self.bolt_core_pipeline,
                });
                bound = Some(kind);
            }
            pass.set_bind_group(1, self.tex_bg(run.texture), &[]);
            pass.draw(run.vertices.clone(), 0..1);
            calls += 1;
        }
        calls
    }
}

/// Whether the surface frame of `format` may be viewed in its non-sRGB
/// twin: always when it has no sRGB twin, otherwise only when the surface
/// lists that twin (`display`, `Renderer::display_view_format`), which a
/// backend without surface view formats never does.
fn display_view_allowed(format: wgpu::TextureFormat, display: Option<wgpu::TextureFormat>) -> bool {
    let plain = format.remove_srgb_suffix();
    plain == format || display == Some(plain)
}

/// The GPU timer's mark at the end of section `label`, when the timer is on.
fn mark(timer: &mut Option<GpuTimer>, pass: &mut wgpu::RenderPass<'_>, label: &'static str) {
    if let Some(t) = timer {
        t.mark_pass(pass, label);
    }
}

/// The GPU timer's section for a depth-tested run (card ISPD): the base
/// terrain apart from the models, the terrain's layers, the specular pass,
/// the water.
fn run_section(d: &DrawCmd) -> &'static str {
    match d.layer {
        DrawLayer::Opaque | DrawLayer::Mask if d.light == LightPath::Terrain => "terrain",
        DrawLayer::Opaque | DrawLayer::Mask => "models",
        DrawLayer::TerrainLayer => "terrain layers",
        DrawLayer::FarSheet => "far sheet",
        DrawLayer::Spec => "specular",
        DrawLayer::Water | DrawLayer::Speckle => "water",
        DrawLayer::Blend => "blend",
    }
}

/// Palette buffer for `capacity` skinned draws + its dynamic-offset bind group.
pub(super) fn create_palette_buffer(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = crate::resources::buffer(
        device,
        "palettes",
        (capacity * PALETTE_STRIDE) as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let bg = crate::made::bind_group(
        device,
        &wgpu::BindGroupDescriptor {
            label: Some("palette bg"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(PALETTE_STRIDE as u64),
                }),
            }],
        },
    );
    (buffer, bg)
}

/// Bind a mesh's vertex/index buffers and draw its whole index range for
/// `instances`. Vertex slot 1 (the instance buffer) and the bind groups are
/// the caller's — they change per pass, this does not.
pub(crate) fn draw_mesh(
    pass: &mut wgpu::RenderPass<'_>,
    mesh: &GpuMesh,
    instances: std::ops::Range<u32>,
) {
    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..mesh.index_count, 0, instances);
}

/// One draw of the blend pass: which list it came from, and its index there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blended {
    /// A world draw on [`DrawLayer::Blend`]: `draws[i]`.
    World(usize),
    /// A skinned draw on [`DrawLayer::Blend`]: `skinned[i]`, whose palette and
    /// instance sit at slot `i`.
    Skinned(usize),
}

/// The blend pass's draws, in the order it draws them: every world BLEND
/// draw the frustum `keep`s and every skinned BLEND draw, sorted TOGETHER far
/// to near. A tie keeps the world draw first, then submission order.
///
/// One pass rather than two because the disc draws both kinds through one
/// function with byte-identical state — BF2 `FUN_0058ed70` draws every model
/// segment, a skinned one included (`analysis/re-side-alpha.md` §3, `[O]`) —
/// and that function picks the draw bucket from the segment's own word: a
/// translucent one (`MTRL` 0x04 — the wookie's fur as much as Kamino's
/// glass) goes to bucket 0xc and an opaque one stays in bucket 5 (lines
/// 723420, `analysis/re-material-flags.md` §2; the default mode 5, 723381,
/// `analysis/re-fog.md` §2.2), unless the caller's
/// draw flags pick another (a soldier's and a world prop's flags words are
/// not resolved statically — re-side-alpha §7, [H]). The ORDER the disc draws
/// its buckets in is untraced (re-material-flags §8, [H]; PROVISIONAL):
/// opaque first and
/// the translucent bucket after it is the reading the world's own blend pass
/// has always rested on, and under it a soldier's opaque segments go down
/// with the world's, before any translucent draw [D]. Before queue SKO every
/// skinned draw came after the world's blend pass, so a soldier behind glass
/// painted over it.
fn blend_order(
    draws: &[DrawCmd],
    keep: &[bool],
    skinned: &[SkinnedDrawCmd],
    eye: glam::Vec3,
) -> Vec<Blended> {
    let mut order: Vec<Blended> = (0..draws.len())
        .filter(|&i| keep[i] && draws[i].layer == DrawLayer::Blend)
        .map(Blended::World)
        .chain(
            (0..skinned.len())
                .filter(|&i| skinned[i].layer == DrawLayer::Blend)
                .map(Blended::Skinned),
        )
        .collect();
    sort_far_to_near(&mut order, |b| {
        let (model, light) = match b {
            Blended::World(i) => (draws[i].model, draws[i].light),
            Blended::Skinned(i) => (skinned[i].model, skinned[i].light),
        };
        let eye = if light == crate::LightPath::Iface {
            glam::Vec3::ZERO
        } else {
            eye
        };
        (model.w_axis.truncate() - eye).length_squared()
    });
    order
}

/// Order a blend pass's draws FAR TO NEAR by their squared eye distance.
/// Alpha compositing is
/// order-dependent, so blended draws are sorted per draw rather than batched
/// — the standard approximation; intersecting surfaces can still resolve
/// wrongly. The sort is stable, so equal distances keep the input order.
fn sort_far_to_near<T: Copy>(order: &mut [T], distance: impl Fn(T) -> f32) {
    order.sort_by(|&a, &b| distance(b).total_cmp(&distance(a)));
}

/// A render pass over `view` and `depth` that keeps what is there — the
/// map pass after an interruption (the terrain light pass's copy).
pub(crate) fn continue_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    label: &str,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

/// Batching key: draws with equal keys share one instanced draw call.
/// Sort/instancing key for the depth-tested runs. The LAYER leads, so an
/// instanced run never spans two pipelines and the pass switches at most
/// six times (Opaque, Mask, TerrainLayer, FarSheet, Spec, Water, then
/// Speckle); mesh and texture group the run. Within a layer the far switch's
/// LOWDs (card LOD) come after every other draw (card LODP): a LOWD straddling
/// the switch spans the screen around an eye inside its building, and drawn
/// last its pixels behind the nearer models fail the depth test before they
/// are shaded (BF2 KAM1 at its spawn: models 9.9 -> 6.8 ms on an Intel iGPU
/// at 2880x1920). The model and its LOWD never share a pixel, so the order
/// moves no colour.
fn draw_key(d: &DrawCmd) -> (u32, u32, u32, u32) {
    (
        draw_key_layer(d.layer),
        matches!(d.lod, Some(l) if l.side == crate::draw::LodSide::Lowd) as u32,
        d.mesh.0,
        d.texture.map_or(u32::MAX, |t| t.0),
    )
}

/// Whether the skinned opaque draws go down before the run whose
/// [`draw_key`] layer is `next` (`None`: the runs are over): before the first
/// water or speckle run, so the water blends over a wading soldier as it does
/// over the bed (`analysis/re-water.md` §9.1, `[O]`; card WADE).
fn skinned_go_before(next: Option<u32>) -> bool {
    next.is_none_or(|l| l >= draw_key_layer(DrawLayer::Water))
}

/// How far from `eye` a draw starts, for the near-first order. A model's
/// primitives share one sphere (`DrawCmd::lit`), so they stay together and keep
/// their mesh order whatever the eye does: two coplanar parts of one model
/// resolve the same way every frame.
fn sort_distance(d: &DrawCmd, near: f32, eye: glam::Vec3) -> f32 {
    d.lit
        .map_or(near, |s| (s.centre.distance(eye) - s.radius).max(0.0))
}

/// A run's place among the others: a quarter of a distance doubling, coarse
/// enough that a run does not move for every step of the eye.
fn near_bucket(near: f32) -> u32 {
    (near.max(1.0).log2() * 4.0) as u32
}

/// Where a run splits: every three distance doublings. A run ranks by its
/// nearest instance, so one mesh placed both at the eye and across the map
/// would draw its far instances ahead of what hides them (BF2 Mygeeto from the
/// spawn, +0.37 ms on the Intel GPU at 1080p unsplit). A finer split buys no
/// more there and adds draw calls.
fn split_band(near: f32) -> u32 {
    near_bucket(near) / 12
}

/// Whether a draw may move for depth: Opaque and Mask write depth and composite
/// nothing. The terrain's layers and the water composite in their key order,
/// and the sheet and the adds gain nothing. On a map with a far fade an opaque
/// draw composites too, where the fade has begun: the sky shows through it and
/// what it covers must already be down. Only a draw wholly short of the fade
/// (`far_begin`) is order-free; the rest keep their mesh order, after it.
fn may_move(d: &DrawCmd, far: f32, far_begin: Option<f32>) -> bool {
    matches!(d.layer, DrawLayer::Opaque | DrawLayer::Mask) && far_begin.is_none_or(|b| far < b)
}

/// A draw's world box and its near and far distance from the eye.
type WorldBox = (glam::Vec3, glam::Vec3, (f32, f32));

/// What a frame decided about one draw, for its place in the sort.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DrawSort {
    /// How far it starts, for a draw that may move ([`may_move`]); `None`
    /// keeps its mesh order.
    near: Option<f32>,
    /// Whether it needs the pipeline that can discard
    /// ([`crate::draw::may_discard`]). Last in the key, so a run splits for it
    /// only inside one (mesh, texture): the mesh order stands.
    may_discard: bool,
}

/// [`runs_key`]'s key: layer, LOWD, whether the draw keeps its mesh order, mesh,
/// texture, band, may discard.
type RunKey = (u32, u32, u32, u32, u32, u32, bool);

/// One draw's run: [`draw_key`], its place and its band. Draws with equal keys
/// are one instanced draw call.
fn runs_key(d: &DrawCmd, s: DrawSort) -> (RunKey, f32) {
    let (layer, lowd, mesh, texture) = draw_key(d);
    let fixed = u32::from(s.near.is_none());
    let band = s.near.map_or(0, split_band);
    (
        (layer, lowd, fixed, mesh, texture, band, s.may_discard),
        s.near.unwrap_or(0.0),
    )
}

/// Sort the depth-tested draws into instanced runs: [`draw_key`] groups them,
/// and within each opaque or masked layer the runs and each run's instances go
/// nearest first, so what hides a later draw is already in the depth buffer
/// when the later draw's pixels would shade. The frame's walk finds each run's
/// end by [`runs_key`].
fn sort_runs(
    kept: impl Iterator<Item = usize>,
    draws: &[DrawCmd],
    sorts: &[DrawSort],
) -> Vec<usize> {
    let mut keyed: Vec<(u128, u32)> = kept
        .map(|i| {
            let (key, place) = runs_key(&draws[i], sorts[i]);
            (packed_key(key, place), i as u32)
        })
        .collect();
    keyed.sort_unstable();
    // Each run ranks by its nearest instance, which the sort just put first.
    let mut runs: Vec<(u128, std::ops::Range<usize>)> = Vec::new();
    let mut at = 0;
    while at < keyed.len() {
        let key = keyed[at].0 >> 32;
        let end = at + keyed[at..].iter().take_while(|k| k.0 >> 32 == key).count();
        let rank = if key >> KEY_FIXED & 1 == 0 {
            near_bucket(f32::from_bits(from_total_order(keyed[at].0 as u32)))
        } else {
            0
        };
        // Layer, LOWD and fixed, then the rank, then the rest of the key.
        let rest = KEY_FIXED;
        let head = key >> rest;
        let tail = key & ((1u128 << rest) - 1);
        runs.push((
            (head << (32 + rest)) | (u128::from(rank) << rest) | tail,
            at..end,
        ));
        at = end;
    }
    runs.sort_unstable_by_key(|r| r.0);
    runs.into_iter()
        .flat_map(|(_, r)| r)
        .map(|k| keyed[k].1 as usize)
        .collect()
}

/// Where the fixed bit sits in a packed key's run part ([`packed_key`]).
const KEY_FIXED: u32 = 73;

/// A run key and a draw's place as one integer that sorts as the pair does:
/// layer 6 bits, LOWD, fixed, mesh, texture, band 8 bits, may discard, place.
/// The sort is most of a frame's own work and compares these many times over.
fn packed_key(key: RunKey, place: f32) -> u128 {
    let (layer, lowd, fixed, mesh, texture, band, may_discard) = key;
    debug_assert!(layer < 64);
    u128::from(layer) << 107
        | u128::from(lowd) << 106
        | u128::from(fixed) << 105
        | u128::from(mesh) << 73
        | u128::from(texture) << 41
        | u128::from(band.min(255)) << 33
        | u128::from(may_discard) << 32
        | u128::from(total_order(place))
}

/// `f32::total_cmp`'s order as an unsigned integer.
fn total_order(f: f32) -> u32 {
    let bits = f.to_bits();
    if bits >> 31 == 0 {
        bits | 1 << 31
    } else {
        !bits
    }
}

/// The float [`total_order`] was given, by its bits.
fn from_total_order(order: u32) -> u32 {
    if order >> 31 == 1 {
        order & !(1 << 31)
    } else {
        !order
    }
}

/// The layer's place in [`draw_key`].
fn draw_key_layer(layer: DrawLayer) -> u32 {
    match layer {
        DrawLayer::Opaque => 0,
        DrawLayer::Mask => 1,
        // The terrain's layers composite over the base terrain (an Opaque
        // draw), each on the one before it: the mesh handle, next in the
        // key, keeps the ascending order the caller uploaded them in.
        DrawLayer::TerrainLayer => 2,
        // The `LOWR` sheet writes depth like Opaque and Mask; it only needs
        // its own pipeline, so it takes a later run (queue CLN5).
        DrawLayer::FarSheet => 3,
        // The specular pass adds over its base, so it follows every opaque
        // and masked run; an add is order-free, so its runs instance like
        // theirs. It precedes the water, which the PS2 composites over a
        // model's finished batch (`analysis/re-specular-pass.md` §7).
        DrawLayer::Spec => 4,
        // The water composites over the solid world and writes depth, so it
        // draws after every opaque, masked and specular run and before every
        // blended one — the only pass whose sort position is load-bearing
        // for what shows THROUGH it.
        DrawLayer::Water => 5,
        // The water's speckle adds over the finished water (the PS2 issues it
        // after the main layer, tile by tile — `analysis/re-water.md` §1);
        // an add is order-free, so its runs instance like the others.
        DrawLayer::Speckle => 6,
        // Blend draws are sorted far-to-near instead and never reach here.
        DrawLayer::Blend => 7,
    }
}

/// [`crate::IFACE_DIR_CAMERA`] turned into the world by `camera`'s axes.
fn iface_dir(camera: &Camera) -> glam::Vec3 {
    let [x, y, z] = crate::IFACE_DIR_CAMERA;
    let (right, forward) = (camera.right().normalize_or_zero(), camera.forward());
    let up = right.cross(forward);
    (right * x + up * y - forward * z).normalize_or_zero()
}

#[cfg(test)]
mod tests;
