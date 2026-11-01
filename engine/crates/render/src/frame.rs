//! One frame: `Renderer::render_frame` (and its `render` and
//! `render_with_overlay` shorthands) checks the draw lists, uploads the
//! globals, frustum-culls, sorts the static draws into instanced runs
//! (`draw_key`) and the blended ones — world and skinned together — far to
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

use crate::frustum;
use crate::gpu_timer::GpuTimer;
use crate::mesh::{self, GpuMesh, Instance};
use crate::{
    Camera, DrawCmd, DrawLayer, Globals, LightPath, RenderError, Renderer, SkinnedDrawCmd, SkyMode,
    Target, TextureHandle, MAX_JOINTS, PALETTE_STRIDE,
};
use glam::Mat4;

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
    /// Windowed mode presents; a lost/outdated surface reconfigures and skips
    /// the frame (returns Ok) — the next frame draws normally.
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

    /// Full frame: the static opaque and masked draws (instanced runs), the
    /// skinned opaque draws (one call each with its palette), the water
    /// runs over both, then
    /// ONE blend pass — the world's blended draws and the skinned ones, far to
    /// near together (`blend_order`) — then the overlay.
    pub fn render_frame(
        &mut self,
        camera: &Camera,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
        clear: [f64; 4],
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<(), RenderError> {
        let leaves = self.falling_draws(camera);
        for d in skinned {
            let Some(mesh) = self.skinned_meshes.get(d.mesh.0 as usize) else {
                return Err(RenderError::BadHandle(format!("skinned mesh {}", d.mesh.0)));
            };
            if !matches!(
                d.layer,
                DrawLayer::Opaque | DrawLayer::Blend | DrawLayer::Spec
            ) {
                return Err(RenderError::BadMesh(format!(
                    "a skinned draw on the {:?} pass: skinned draws are Opaque, Blend or Spec",
                    d.layer
                )));
            }
            if d.palette.len() > MAX_JOINTS || d.palette.len() <= mesh.max_joint as usize {
                return Err(RenderError::BadMesh(format!(
                    "palette of {} joints for a mesh using joint {} (max {MAX_JOINTS})",
                    d.palette.len(),
                    mesh.max_joint
                )));
            }
            if let Some(t) = d.texture {
                if t.0 as usize >= self.textures.len() {
                    return Err(RenderError::BadHandle(format!("texture {}", t.0)));
                }
            }
        }
        for d in draws {
            if d.mesh.0 as usize >= self.meshes.len() {
                return Err(RenderError::BadHandle(format!("mesh {}", d.mesh.0)));
            }
            if let Some(t) = d.texture {
                if t.0 as usize >= self.textures.len() {
                    return Err(RenderError::BadHandle(format!("texture {}", t.0)));
                }
            }
        }

        let (width, height) = self.size();
        let guard = crate::affine::guard(width as f32 / height as f32);
        let view_proj = camera.view_proj(width as f32 / height as f32);
        let frustum = frustum::Frustum::from_view_proj(&view_proj);
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: camera.position.to_array(),
            guard_x: guard[0],
            sun_dir: self.lighting.sun_dir.normalize_or_zero().to_array(),
            sun_d4: f32::from(u8::from(self.lighting.sun_d4)),
            sun_color: self.lighting.sun_color,
            guard_y: guard[1],
            ambient_sky: self.lighting.ambient_sky,
            material_time: crate::material_scroll_time(self.water_time),
            ambient_ground: self.lighting.ambient_ground,
            fog_on: if self.lighting.fog.is_some() {
                1.0
            } else {
                0.0
            },
            fog_color: self.lighting.fog.map(|f| f.color).unwrap_or([0.0; 3]),
            fog_near: self.lighting.fog.map(|f| f.near).unwrap_or(0.0),
            fog_far: self.lighting.fog.map(|f| f.far).unwrap_or(1.0),
            time: self.time,
            fade_begin: self.lighting.far_fade.map(|f| f.begin).unwrap_or(0.0),
            fade_end: self.lighting.far_fade.map(|f| f.end).unwrap_or(0.0),
            water_fog_begin: self.lighting.water_fog_begin.unwrap_or(0.0),
            water_lit: self.lighting.water_lit,
            water_dim_begin: self.lighting.water_dim.map_or(0.0, |d| d[0]),
            water_dim_rate: self.lighting.water_dim.map_or(0.0, |d| d[1]),
            light2_dir: self
                .lighting
                .light2
                .map_or([0.0, -1.0, 0.0], |l| l.dir.normalize_or_zero().to_array()),
            sun_static: f32::from(u8::from(self.lighting.sun_static)),
            light2_color: self.lighting.light2.map_or([0.0; 3], |l| l.color),
            light2_static: self
                .lighting
                .light2
                .map_or(0.0, |l| f32::from(u8::from(l.is_static))),
            water_eye_shift: self.lighting.water.eye_shift.to_array(),
            glow_gain: self.lighting.glow_gain,
            water_specular: self.lighting.water.specular,
            speckle_ambient: self.lighting.water.speckle_ambient,
            speckle_specular: self.lighting.water.speckle_specular,
            speckle_axis: self.lighting.water.speckle_axis.to_array(),
            _pad8: 0.0,
            speckle_light: self.lighting.water.speckle_light.to_array(),
            _pad9: 0.0,
            speckle_shift: self.lighting.water.speckle_shift.to_array(),
            _pad10: 0.0,
            character_ambient: self.lighting.character_ambient.unwrap_or([0.0; 3]),
            character_ambient_on: f32::from(u8::from(self.lighting.character_ambient.is_some())),
            iface_dir: iface_dir(camera).to_array(),
            _pad11: 0.0,
        };
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
        // BF2's local lights (`point_light.rs`): the view's list, uploaded
        // as the shader's table; each lit draw gathers from it below.
        let filed = crate::point_light::file(&self.point_lights, camera.position, |p, r| {
            frustum.intersects_aabb(p - glam::Vec3::splat(r), p + glam::Vec3::splat(r))
        });
        if !(filed.is_empty() && self.point_table_clear) {
            self.queue.write_buffer(
                &self.points_buf,
                0,
                bytemuck::cast_slice(&crate::point_light::table(&filed)),
            );
            self.point_table_clear = filed.is_empty();
        }
        // A skinned draw is never prelit.
        let skinned_lights = |lit: Option<crate::LitSphere>| -> [u16; 4] {
            match lit {
                Some(s) if !filed.is_empty() => {
                    crate::point_light::gather(&filed, s, camera.position, false)
                }
                _ => [0; 4],
            }
        };
        let spec = crate::specular::SpecParams {
            cam_right: camera.right().normalize_or_zero().to_array(),
            mirror: if self.spec_mirror { 1.0 } else { 0.0 },
        };
        self.queue
            .write_buffer(&self.spec_buf, 0, bytemuck::bytes_of(&spec));

        // Opaque, masked and water draws sort by `draw_key` (layer, mesh,
        // texture) so identical ones collapse into instanced runs — free for
        // the opaque and masked runs, which write depth and composite nothing;
        // the water runs composite, so the key's layer rank and handle order
        // carry their place. Blended draws CANNOT be reordered for batching:
        // alpha compositing is order-dependent, so they go in their own list
        // sorted back-to-front by distance from the eye and are drawn one at a
        // time.
        // Sorting per draw (not per triangle) is the approximation here, and
        // it is the standard one; intersecting transparent surfaces can still
        // resolve wrongly.
        // Frustum culling: drop draws whose world AABB is entirely outside the
        // view. Conservative (see `frustum`), so a committed golden must be
        // byte-identical with it on — that, not the unit tests, is the proof
        // it removes nothing visible. The same box drops a draw whose
        // distance bands discard every fragment it could make
        // (`draw::all_discarded`, card LODP).
        let mut culled = 0u32;
        let eye = camera.position;
        let visible = |d: &DrawCmd, f: &frustum::Frustum, meshes: &[GpuMesh]| -> bool {
            let Some(m) = meshes.get(d.mesh.0 as usize) else {
                return true;
            };
            let (lo, hi) = frustum::transformed_aabb(&d.model, m.bounds.0, m.bounds.1);
            let (near, far) = frustum::distance_range(lo, hi, eye);
            let (cull, cull_near) = crate::draw::instance_cull(d);
            f.intersects_aabb(lo, hi) && !crate::draw::all_discarded(cull, cull_near, near, far)
        };
        // The far scene's switch (card LOD) drops the side of a geometry
        // not drawn from this eye, with or without culling: its model and
        // LOWD must never both draw.
        let switched = |d: &DrawCmd| d.lod.is_none_or(|l| l.drawn(eye));
        let keep: Vec<bool> = if self.cull {
            draws
                .iter()
                .map(|d| {
                    let v = switched(d) && visible(d, &frustum, &self.meshes);
                    if !v {
                        culled += 1;
                    }
                    v
                })
                .collect()
        } else {
            draws.iter().map(switched).collect()
        };

        let mut order: Vec<usize> = (0..draws.len())
            .filter(|&i| keep[i] && draws[i].layer != DrawLayer::Blend)
            .collect();
        order.sort_by_key(|&i| draw_key(&draws[i]));
        // The blend pass holds the skinned blended draws too; the world ones'
        // instances follow the runs' in the pass's own order.
        let blended = blend_order(draws, &keep, skinned, camera.position);
        let blend_base = order.len();
        order.extend(blended.iter().filter_map(|b| match *b {
            Blended::World(i) => Some(i),
            Blended::Skinned(_) => None,
        }));
        // The sky shells take the first slots of the same instance buffer, so
        // every other index below is offset by `sky_base`.
        let sky_base = self.sky.len();
        let mut instances: Vec<Instance> = self
            .sky
            .iter()
            .map(|s| Instance {
                model: Self::sky_model(s, camera.position, self.time).to_cols_array_2d(),
                color: s.color,
                // The sky is never distance-culled: on the PS2 it is drawn by
                // the sky tick under draw flag 0x20, outside both scene
                // ranges (`analysis/re-fog.md` 2.4 [V]).
                cull: [mesh::NO_CULL; 2],
                cull_near: [mesh::NO_CULL_NEAR; 2],
                light: LightPath::Lit as u32,
                fade: mesh::NO_FADE,
                lights: [0; 4],
            })
            .collect();
        // A model's primitives are submitted together and share its sphere:
        // one gather serves the run.
        let mut last: Option<(crate::LitSphere, bool, [u16; 4])> = None;
        let mut draw_lights = vec![[0u16; 4]; draws.len()];
        if !filed.is_empty() {
            for (i, d) in draws.iter().enumerate() {
                let prelit = match d.light {
                    LightPath::Lit | LightPath::Character => false,
                    LightPath::Prelit | LightPath::PrelitAdd | LightPath::VertexLit => true,
                    _ => continue,
                };
                let (Some(s), true) = (d.lit, keep[i]) else {
                    continue;
                };
                draw_lights[i] = match last {
                    Some((ls, lp, l)) if ls == s && lp == prelit => l,
                    _ => {
                        let l = crate::point_light::gather(&filed, s, camera.position, prelit);
                        last = Some((s, prelit, l));
                        l
                    }
                };
            }
        }
        instances.extend(order.iter().map(|&i| {
            let (cull, cull_near) = crate::draw::instance_cull(&draws[i]);
            Instance {
                model: draws[i].model.to_cols_array_2d(),
                color: draws[i].color,
                cull,
                cull_near,
                light: draws[i].light as u32 | crate::affine::layer_bit(draws[i].layer),
                fade: draws[i]
                    .fade
                    .map_or(mesh::NO_FADE, |f| [f.begin, f.end, f.near, f.opacity]),
                lights: draw_lights[i],
            }
        }));
        if !instances.is_empty() {
            if instances.len() > self.instance_capacity {
                self.instance_capacity = instances.len().next_power_of_two();
                self.instance_buf = create_instance_buffer(&self.device, self.instance_capacity);
            }
            self.queue
                .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&instances));
        }

        if !skinned.is_empty() {
            if skinned.len() > self.palette_capacity {
                self.palette_capacity = skinned.len().next_power_of_two();
                let (buf, bg) =
                    create_palette_buffer(&self.device, &self.palette_bgl, self.palette_capacity);
                self.palette_buf = buf;
                self.palette_bg = bg;
                self.skinned_instance_buf =
                    create_instance_buffer(&self.device, self.palette_capacity);
            }
            let mut palettes = vec![0u8; skinned.len() * PALETTE_STRIDE];
            for (i, d) in skinned.iter().enumerate() {
                for (j, m) in d.palette.iter().enumerate() {
                    let at = i * PALETTE_STRIDE + j * 64;
                    palettes[at..at + 64]
                        .copy_from_slice(bytemuck::bytes_of(&m.to_cols_array_2d()));
                }
            }
            self.queue.write_buffer(&self.palette_buf, 0, &palettes);
            let inst: Vec<Instance> = skinned
                .iter()
                .map(|d| Instance {
                    model: d.model.to_cols_array_2d(),
                    color: d.color,
                    // Soldiers carry no cull: `SkinnedDrawCmd` has no `cull`
                    // field, because every skinned draw the port makes is a
                    // unit inside the near scene. Stated in the item 5 report.
                    cull: [mesh::NO_CULL; 2],
                    cull_near: [mesh::NO_CULL_NEAR; 2],
                    // A skinned draw carries no COL0: the PS2's normal light
                    // set (hemisphere + Light1 + Light2), with BF1's
                    // character ambient where the map has one — every
                    // skinned draw the port makes is a soldier — or, for a
                    // spawn screen's soldier, the interface set.
                    light: d.light as u32 | crate::affine::layer_bit(d.layer),
                    fade: mesh::NO_FADE,
                    lights: skinned_lights(d.lit),
                })
                .collect();
            self.queue
                .write_buffer(&self.skinned_instance_buf, 0, bytemuck::cast_slice(&inst));
        }
        // The skinned opaque draws, in submission order. Every skinned draw
        // keeps its palette and instance slot at its SUBMISSION index, so the
        // passes below move only the order.
        let skin_opaque: Vec<usize> = (0..skinned.len())
            .filter(|&i| skinned[i].layer == DrawLayer::Opaque)
            .collect();
        // The skinned specular passes (queue SPEC2), in submission order —
        // none without a table, as the world's.
        let skin_spec: Vec<usize> = match self.spec_bg {
            Some(_) => (0..skinned.len())
                .filter(|&i| skinned[i].layer == DrawLayer::Spec)
                .collect(),
            None => Vec::new(),
        };

        // The grass (`grass.rs`): this view's sprite buffers and the visible
        // patches, far to near, uploaded before the pass opens; then the
        // leaves (`leaf.rs`), which ride the same buffers and pipeline.
        let mut grass =
            crate::grass::build(&self.grass.classes, &self.grass.patches, camera, self.time);
        crate::leaf::build(
            &mut grass,
            &self.leaves.classes,
            &self.leaves.objects,
            self.leaves.near,
            camera,
            self.time,
        );
        if !grass.runs.is_empty() {
            let vb = bytemuck::cast_slice(&grass.vertices);
            let ib = bytemuck::cast_slice(&grass.instances);
            crate::grass::ensure(
                &self.device,
                &mut self.grass.vertex_buf,
                vb.len(),
                "grass vertices",
            );
            crate::grass::ensure(
                &self.device,
                &mut self.grass.instance_buf,
                ib.len(),
                "grass instances",
            );
            if let (Some((v, _)), Some((i, _))) = (&self.grass.vertex_buf, &self.grass.instance_buf)
            {
                self.queue.write_buffer(v, 0, vb);
                self.queue.write_buffer(i, 0, ib);
            }
        }

        // The laser bolts (`bolt.rs`) and the particle effects
        // (`particles.rs`): this view's ribbons and sprites, far to near.
        let mut particles = std::borrow::Cow::Borrowed(&self.particles.particles[..]);
        if !leaves.is_empty() {
            particles.to_mut().extend(leaves);
        }
        let bolts = crate::particles::build(&self.bolts.bolts, &particles, camera);
        if !bolts.runs.is_empty() {
            let vb = bytemuck::cast_slice(&bolts.vertices);
            let inst = [crate::bolt::identity_instance()];
            let ib: &[u8] = bytemuck::cast_slice(&inst);
            crate::grass::ensure(
                &self.device,
                &mut self.bolts.vertex_buf,
                vb.len(),
                "bolt vertices",
            );
            crate::grass::ensure(
                &self.device,
                &mut self.bolts.instance_buf,
                ib.len(),
                "bolt instance",
            );
            if let (Some((v, _)), Some((i, _))) = (&self.bolts.vertex_buf, &self.bolts.instance_buf)
            {
                self.queue.write_buffer(v, 0, vb);
                self.queue.write_buffer(i, 0, ib);
            }
        }

        // The star field (`stars.rs`): this view's quads.
        let mut star_vertices = std::mem::take(&mut self.stars.vertices);
        crate::stars::build(&self.stars, camera, self.time, &mut star_vertices);
        if !star_vertices.is_empty() {
            let vb = bytemuck::cast_slice(&star_vertices);
            let inst = [crate::bolt::identity_instance()];
            let ib: &[u8] = bytemuck::cast_slice(&inst);
            crate::grass::ensure(
                &self.device,
                &mut self.stars.vertex_buf,
                vb.len(),
                "star vertices",
            );
            crate::grass::ensure(
                &self.device,
                &mut self.stars.instance_buf,
                ib.len(),
                "star instance",
            );
            if let (Some((v, _)), Some((i, _))) = (&self.stars.vertex_buf, &self.stars.instance_buf)
            {
                self.queue.write_buffer(v, 0, vb);
                self.queue.write_buffer(i, 0, ib);
            }
        }
        let star_count = star_vertices.len() as u32;
        self.stars.vertices = star_vertices;

        // BF2's terrain light pass (`terrain_light.rs`): its parameters and
        // the frame's copy, before the frame is acquired.
        let terrain_light = self.terrain_light_wanted();
        if terrain_light {
            self.prepare_terrain_light(width, height);
        }

        // The DomeInfo frame filter (`filter.rs`): its parameters and copy.
        let bloom = self.prepare_bloom(width, height);
        let filter = self.filter_wanted();
        if filter {
            self.prepare_filter(width, height);
        }

        // Acquire the frame's color view.
        let acquire = std::time::Instant::now();
        let (frame, view) = match &mut self.target {
            Target::Window { surface, config } => match surface.get_current_texture() {
                Ok(frame) => {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    (Some(frame), view)
                }
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    surface.configure(&self.device, config);
                    return Ok(());
                }
                Err(wgpu::SurfaceError::Timeout) => return Ok(()),
                Err(e) => return Err(RenderError::Surface(e.to_string())),
            },
            Target::Headless { view, .. } => (None, view.clone()),
        };
        self.acquire_ms = acquire.elapsed().as_secs_f64() * 1000.0;

        let mut draw_calls = 0u32;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });
        // The GPU timer, when on (`gpu_timer.rs`): out of `self` for the
        // frame, so the passes can mark it while they borrow the rest.
        let mut timer = self.gpu_timer.take();
        if let Some(t) = &mut timer {
            t.begin(&self.device, &mut encoder);
        }
        // The texture the terrain light pass copies (the frame so far).
        let frame_texture: &wgpu::Texture = match (&frame, &self.target) {
            (Some(f), _) => &f.texture,
            (None, Target::Headless { texture, .. }) => texture,
            (None, Target::Window { .. }) => unreachable!("a window frame was acquired"),
        };
        {
            // Render pass shape from an earlier wgpu project's main.rs "Main Render Pass".
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0],
                            g: clear[1],
                            b: clear[2],
                            // The frame is opaque whatever the caller asks:
                            // no pipeline writes alpha (`FRAME_WRITES`), so
                            // this is the alpha every pixel keeps.
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Sky first: unlit shells around the camera, no depth at all. The
            // stars go after the base dome's shells (its mirror included) and
            // under the rest (a cloud deck covers them), as both ELFs order
            // the sky pass (`analysis/re-stars.md` §3).
            let star_slot = self.stars.slot.min(self.sky.len());
            if sky_base > 0 || star_count > 0 {
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                let mut bound: Option<SkyMode> = None;
                for item in self.sky.iter().enumerate().map(Some).chain([None]) {
                    if item.map_or(self.sky.len(), |(i, _)| i) == star_slot && star_count > 0 {
                        if let (Some((vbuf, _)), Some((ibuf, _))) =
                            (&self.stars.vertex_buf, &self.stars.instance_buf)
                        {
                            pass.set_pipeline(&self.star_pipeline);
                            pass.set_bind_group(1, self.tex_bg(None), &[]);
                            pass.set_vertex_buffer(0, vbuf.slice(..));
                            pass.set_vertex_buffer(1, ibuf.slice(..));
                            pass.draw(0..star_count, 0..1);
                            draw_calls += 1;
                            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                            bound = None;
                        }
                    }
                    let Some((slot, shell)) = item else {
                        break;
                    };
                    let Some(mesh) = self.meshes.get(shell.mesh.0 as usize) else {
                        continue;
                    };
                    // Two pipelines, one per material mode; shells keep
                    // their authored order (a cloud deck composites over the
                    // dome, and an OPAQUE shell PAINTS over what is under
                    // it), so switch rather than sort.
                    if bound != Some(shell.mode) {
                        pass.set_pipeline(&self.sky_pipelines[shell.mode.index()]);
                        bound = Some(shell.mode);
                    }
                    pass.set_bind_group(1, self.tex_bg(shell.texture), &[]);
                    draw_mesh(&mut pass, mesh, slot as u32..slot as u32 + 1);
                    draw_calls += 1;
                }
            }
            mark(&mut timer, &mut pass, "sky");

            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));

            // The map's fade is per MAP, not per draw: one bool for the whole
            // frame picks the `_fade` twin of every world pipeline below.
            let fade = self.lighting.far_fade.is_some();

            // Walk sorted order in runs of equal (layer, mesh, texture): one
            // instanced draw per run, and — because the layer leads the key —
            // at most six pipeline switches: Opaque → Mask → TerrainLayer →
            // FarSheet → Spec → Water → Speckle (`draw_key`), the skinned
            // opaque draws between Spec and Water.
            let mut run_start = 0;
            let mut bound_layer: Option<DrawLayer> = None;
            // The base terrain's runs (mesh, instances) for the terrain light
            // pass, which ends this pass once the terrain's layers are down
            // (before the first run past `TerrainLayer`) and resumes it.
            let mut terrain_runs: Vec<(usize, std::ops::Range<u32>)> = Vec::new();
            let mut lights_pending = terrain_light;
            let mut skin_pending = true;
            // The GPU timer's section of the runs being drawn (`run_section`).
            let mut section: Option<&'static str> = None;
            while run_start <= blend_base {
                let key = (run_start < blend_base).then(|| draw_key(&draws[order[run_start]]));
                if lights_pending
                    && key.is_none_or(|k| k.0 > draw_key_layer(DrawLayer::TerrainLayer))
                {
                    lights_pending = false;
                    if let Some(l) = section.take() {
                        mark(&mut timer, &mut pass, l);
                    }
                    drop(pass);
                    draw_calls +=
                        self.terrain_light_pass(&mut encoder, frame_texture, &view, &terrain_runs);
                    if let Some(t) = &mut timer {
                        t.mark(&mut encoder, "terrain lights");
                    }
                    pass =
                        continue_pass(&mut encoder, &view, &self.depth_view, "map pass, resumed");
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                    pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                    bound_layer = None;
                }
                // The skinned opaque draws, one call each with its palette,
                // and their specular passes: before the first water run (or
                // at the end of the runs, on a map with no water), as both
                // PS2s draw them — an opaque model segment is bucket 5, the
                // near flat water bucket 8 (BF1) / 7 (BF2), kicked in
                // ascending order, and the water blends over what is down
                // with Z test AND Z write (`analysis/re-water.md` §9.1,
                // `[O]`; card WADE). Drawn after it, a wading soldier failed
                // the water's depth below the surface and ended at the belt.
                if skin_pending && skinned_go_before(key.map(|k| k.0)) {
                    skin_pending = false;
                    if let Some(l) = section.take() {
                        mark(&mut timer, &mut pass, l);
                    }
                    if !skin_opaque.is_empty() {
                        pass.set_pipeline(if fade {
                            &self.fade_skinned_pipeline
                        } else {
                            &self.skinned_pipeline
                        });
                        pass.set_vertex_buffer(1, self.skinned_instance_buf.slice(..));
                        for &i in &skin_opaque {
                            self.draw_skinned(&mut pass, &skinned[i], i);
                            draw_calls += 1;
                        }
                    }
                    mark(&mut timer, &mut pass, "soldiers");
                    // Their specular passes, added over them before anything
                    // composites: the disc draws a segment's pass right
                    // after its base (`analysis/re-specular-pass.md` §7,
                    // `[O]`), and every spec segment the side bake writes is
                    // an opaque one (BF2 skips 0x04, `bake::spec`; no BF1
                    // side segment carries 0x04 and 0x20 together).
                    // `draw_skinned` binds each draw's palette.
                    if let (Some(spec_bg), false) = (&self.spec_bg, skin_spec.is_empty()) {
                        pass.set_pipeline(&self.spec_skinned_pipeline);
                        pass.set_bind_group(3, spec_bg, &[]);
                        pass.set_vertex_buffer(1, self.skinned_instance_buf.slice(..));
                        for &i in &skin_spec {
                            self.draw_skinned(&mut pass, &skinned[i], i);
                            draw_calls += 1;
                        }
                    }
                    mark(&mut timer, &mut pass, "soldiers spec");
                    // Back to the world's instances for the water runs.
                    pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                    bound_layer = None;
                }
                let Some(key) = key else { break };
                let mut run_end = run_start + 1;
                while run_end < blend_base && draw_key(&draws[order[run_end]]) == key {
                    run_end += 1;
                }

                let d = &draws[order[run_start]];
                let run = run_section(d);
                if section != Some(run) {
                    if let Some(l) = section.replace(run) {
                        mark(&mut timer, &mut pass, l);
                    }
                }
                if terrain_light
                    && d.light == LightPath::Terrain
                    && d.layer != DrawLayer::TerrainLayer
                {
                    terrain_runs.push((
                        d.mesh.0 as usize,
                        (sky_base + run_start) as u32..(sky_base + run_end) as u32,
                    ));
                }
                if d.layer == DrawLayer::Spec {
                    // The specular pass: skipped without a table; else its
                    // additive pipeline, the palette at offset 0 (unread) and
                    // the table at group 3 — see `specular.rs`.
                    let Some(spec_bg) = &self.spec_bg else {
                        run_start = run_end;
                        continue;
                    };
                    if bound_layer != Some(DrawLayer::Spec) {
                        pass.set_pipeline(&self.spec_pipeline);
                        pass.set_bind_group(2, &self.palette_bg, &[0]);
                        pass.set_bind_group(3, spec_bg, &[]);
                        bound_layer = Some(DrawLayer::Spec);
                    }
                } else if bound_layer != Some(d.layer) {
                    pass.set_pipeline(match (d.layer, fade) {
                        // The water pass carries its own fade (see
                        // `water_pipeline`), so it takes the same pipeline
                        // either way.
                        (DrawLayer::Water, _) => &self.water_pipeline,
                        // Additive, faded in its own entry point like the
                        // water it lies on (see `speckle_pipeline`).
                        (DrawLayer::Speckle, _) => &self.speckle_pipeline,
                        // No fade twin: no sheet is drawn on a fade map.
                        (DrawLayer::FarSheet, _) => &self.far_sheet_pipeline,
                        // Folds the fade in its own entry point.
                        (DrawLayer::TerrainLayer, _) => &self.terrain_layer_pipeline,
                        (DrawLayer::Spec, _) => unreachable!("bound above"),
                        (DrawLayer::Mask, false) => &self.mask_pipeline,
                        (DrawLayer::Mask, true) => &self.fade_mask_pipeline,
                        (_, false) => &self.pipeline,
                        (_, true) => &self.fade_pipeline,
                    });
                    bound_layer = Some(d.layer);
                }
                pass.set_bind_group(1, self.tex_bg(d.texture), &[]);
                draw_mesh(
                    &mut pass,
                    &self.meshes[d.mesh.0 as usize],
                    (sky_base + run_start) as u32..(sky_base + run_end) as u32,
                );
                draw_calls += 1;

                run_start = run_end;
            }
            if let Some(l) = section.take() {
                mark(&mut timer, &mut pass, l);
            }

            // The grass patches (queue FOLGG) and then the leaf patches (card
            // LEAF): after every opaque draw, before the blend pass, each run
            // one instanced draw of its class's sprite buffer.
            if let (false, Some((vbuf, _)), Some((ibuf, _))) = (
                grass.runs.is_empty(),
                &self.grass.vertex_buf,
                &self.grass.instance_buf,
            ) {
                pass.set_pipeline(&self.grass_pipeline);
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.set_vertex_buffer(1, ibuf.slice(..));
                for run in &grass.runs {
                    pass.set_bind_group(1, self.tex_bg(run.texture), &[]);
                    pass.draw(run.vertices.clone(), run.instances.clone());
                    draw_calls += 1;
                }
            }
            mark(&mut timer, &mut pass, "grass");

            // The blend pass: after every opaque draw, far to near, one at a
            // time — the world's blended draws (their instances sit at
            // blend_base.. of the instance buffer, in this order) and the
            // skinned ones where their distance puts them. Passing from one
            // kind to the other rebinds the pipeline and the instance buffer.
            let mut world_slot = blend_base;
            let mut bound_skinned: Option<bool> = None;
            for &b in &blended {
                let skin = matches!(b, Blended::Skinned(_));
                if bound_skinned != Some(skin) {
                    pass.set_pipeline(match (skin, fade) {
                        (false, false) => &self.blend_pipeline,
                        (false, true) => &self.fade_blend_pipeline,
                        (true, false) => &self.skinned_blend_pipeline,
                        (true, true) => &self.fade_skinned_blend_pipeline,
                    });
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                    pass.set_vertex_buffer(
                        1,
                        if skin {
                            self.skinned_instance_buf.slice(..)
                        } else {
                            self.instance_buf.slice(..)
                        },
                    );
                    bound_skinned = Some(skin);
                }
                match b {
                    Blended::World(i) => {
                        debug_assert_eq!(order[world_slot], i);
                        let d = &draws[i];
                        let inst = (sky_base + world_slot) as u32;
                        world_slot += 1;
                        pass.set_bind_group(1, self.tex_bg(d.texture), &[]);
                        draw_mesh(&mut pass, &self.meshes[d.mesh.0 as usize], inst..inst + 1);
                    }
                    Blended::Skinned(i) => self.draw_skinned(&mut pass, &skinned[i], i),
                }
                draw_calls += 1;
            }
            mark(&mut timer, &mut pass, "blend");

            // The laser bolts (card FXB) and the particles (card FXP2):
            // after the blend pass, far to near, each bolt's core (NORMAL)
            // then its glow (ADDITIVE), each particle by its blend.
            if self.particle_display.is_none() {
                draw_calls += self.draw_particle_runs(&mut pass, &bolts.runs, false);
                mark(&mut timer, &mut pass, "bolts+particles");
            }
        }
        if let (Some(display), false) = (&self.particle_display, bolts.runs.is_empty()) {
            let display_view = frame_texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(display.format),
                ..Default::default()
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("particle pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &display_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            draw_calls += self.draw_particle_runs(&mut pass, &bolts.runs, true);
            mark(&mut timer, &mut pass, "bolts+particles");
        }
        if bloom {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom gather"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.bloom_gather_view(),
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            for (slot, &i) in order.iter().enumerate() {
                let d = &draws[i];
                if !matches!(d.light, LightPath::Glow | LightPath::Unscaled)
                    || !matches!(
                        d.layer,
                        DrawLayer::Opaque | DrawLayer::Mask | DrawLayer::Blend
                    )
                {
                    continue;
                }
                pass.set_pipeline(match d.layer {
                    DrawLayer::Mask => &self.bloom.gather.mask,
                    DrawLayer::Blend => &self.bloom.gather.blend,
                    _ => &self.bloom.gather.world,
                });
                pass.set_bind_group(1, self.tex_bg(d.texture), &[]);
                let inst = (sky_base + slot) as u32;
                draw_mesh(&mut pass, &self.meshes[d.mesh.0 as usize], inst..inst + 1);
                draw_calls += 1;
            }
            if let (Some((vbuf, _)), Some((ibuf, _))) =
                (&self.bolts.vertex_buf, &self.bolts.instance_buf)
            {
                pass.set_pipeline(&self.bloom.gather.particle);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.set_vertex_buffer(1, ibuf.slice(..));
                for run in bolts.runs.iter().filter(|r| r.glow) {
                    pass.set_bind_group(1, self.tex_bg(run.texture), &[]);
                    pass.draw(run.vertices.clone(), 0..1);
                    draw_calls += 1;
                }
            }
            drop(pass);
            draw_calls += self.bloom_pass(&mut encoder, frame_texture, &view);
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "bloom");
            }
        }
        // The frame filter lifts the finished 3D frame, before the overlay
        // (the PS2 draws its HUD after it, `analysis/re-glow.md` §3.3).
        if filter {
            draw_calls += self.filter_pass(&mut encoder, frame_texture, &view);
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "filter");
            }
        }
        self.frame_stats = FrameStats {
            draw_calls,
            culled,
            submitted: (draws.len() as u32).saturating_sub(culled),
        };
        if let Some(overlay) = overlay {
            overlay(
                &self.device,
                &self.queue,
                &mut encoder,
                &view,
                [width, height],
            );
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "hud");
            }
        }
        if let Some(t) = &mut timer {
            t.resolve(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        if let Some(mut t) = timer {
            t.submitted();
            self.gpu_timer = Some(t);
        }
        if let Some(frame) = frame {
            frame.present();
        }
        Ok(())
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
            let kind = (run.glow, run.solid, run.cutout);
            if bound != Some(kind) {
                pass.set_pipeline(match (shown, kind) {
                    (Some(d), (true, _, _)) => &d.glow,
                    (Some(d), (false, true, _)) => &d.solid,
                    (Some(d), (false, false, true)) => &d.cutout,
                    (Some(d), (false, false, false)) => &d.core,
                    (None, (true, _, _)) => &self.bolt_glow_pipeline,
                    (None, (false, true, _)) => &self.bolt_solid_pipeline,
                    (None, (false, false, true)) => &self.leaf_cutout_pipeline,
                    (None, (false, false, false)) => &self.bolt_core_pipeline,
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
    sort_far_to_near(&mut order, eye, |b| match b {
        Blended::World(i) => draws[i].model,
        Blended::Skinned(i) => skinned[i].model,
    });
    order
}

/// Order a blend pass's draws FAR TO NEAR by the distance from `eye` to each
/// draw's model origin (`model_of(d)` for draw `d`). Alpha compositing is
/// order-dependent, so blended draws are sorted per draw rather than batched
/// — the standard approximation; intersecting surfaces can still resolve
/// wrongly. The sort is stable, so equal distances keep the input order.
fn sort_far_to_near<T: Copy>(order: &mut [T], eye: glam::Vec3, model_of: impl Fn(T) -> Mat4) {
    order.sort_by(|&a, &b| {
        let da = (model_of(a).w_axis.truncate() - eye).length_squared();
        let db = (model_of(b).w_axis.truncate() - eye).length_squared();
        db.total_cmp(&da) // far first
    });
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
mod tests {
    use super::*;
    use crate::{MeshHandle, SkinnedMeshHandle};

    /// The interface light turns with the camera: looking down -Z it is the
    /// camera-frame direction itself; turned a quarter left (looking down
    /// -X) the camera's x axis is the world's -Z and its z axis +X.
    #[test]
    fn the_interface_light_is_fixed_to_the_camera() {
        let k = 1.0 / 3f32.sqrt();
        let close = |a: glam::Vec3, b: glam::Vec3| (a - b).length() < 1e-5;
        let mut cam = Camera::default();
        assert!(close(iface_dir(&cam), glam::Vec3::new(-k, -k, -k)));
        cam.yaw = std::f32::consts::FRAC_PI_2;
        assert!(close(iface_dir(&cam), glam::Vec3::new(-k, -k, k)));
        cam.position = glam::Vec3::new(5.0, 2.0, -3.0);
        assert!(close(iface_dir(&cam), glam::Vec3::new(-k, -k, k)));
    }

    /// The blend pass composites far to near through [`sort_far_to_near`],
    /// and a tie keeps the input order (a stable sort), so two draws at one
    /// origin always composite the same way round.
    #[test]
    fn blend_draws_sort_far_to_near_and_ties_keep_submission_order() {
        let at = |z: f32| Mat4::from_translation(glam::Vec3::new(0.0, 0.0, z));
        let models = [at(-2.0), at(-9.0), at(-5.0), at(-9.0), at(3.0)];
        let mut order = vec![0, 1, 2, 3, 4];
        sort_far_to_near(&mut order, glam::Vec3::ZERO, |i| models[i]);
        assert_eq!(order, vec![1, 3, 2, 4, 0]);
    }

    /// Queue SKO: the blend pass is ONE list — the world's BLEND draws the
    /// frustum keeps and the skinned BLEND draws, interleaved far to near, a
    /// tie keeping the world draw first. Opaque draws of either kind and a
    /// culled world draw are not in it. (The GPU referee
    /// `skinned::world_and_skinned_blend_draws_composite_far_to_near_together`
    /// holds the pass to this order.)
    #[test]
    fn the_blend_pass_interleaves_world_and_skinned_draws_far_to_near() {
        let at = |z: f32| Mat4::from_translation(glam::Vec3::new(0.0, 0.0, z));
        let (m, t) = (MeshHandle::default(), TextureHandle(0));
        let draws = [
            DrawCmd::textured_blended(m, t, at(-2.0)),
            DrawCmd::textured(m, t, at(-20.0)),
            DrawCmd::textured_blended(m, t, at(-9.0)),
            DrawCmd::textured_blended(m, t, at(-30.0)),
        ];
        let keep = [true, true, true, false];
        let skin = |z: f32, layer| SkinnedDrawCmd {
            mesh: SkinnedMeshHandle(0),
            texture: None,
            color: [1.0; 4],
            model: at(z),
            palette: Vec::new(),
            layer,
            light: LightPath::Character,
            lit: None,
        };
        let skinned = [
            skin(-9.0, DrawLayer::Blend),
            skin(-25.0, DrawLayer::Opaque),
            skin(-5.0, DrawLayer::Blend),
        ];
        assert_eq!(
            blend_order(&draws, &keep, &skinned, glam::Vec3::ZERO),
            vec![
                Blended::World(2),
                Blended::Skinned(0),
                Blended::Skinned(2),
                Blended::World(0),
            ]
        );
    }

    /// Every draw is unculled unless a caller asks, and the ask survives the
    /// pass-selecting constructors — the far-scene cull is opt-in per draw,
    /// so a renderer with no policy behind it renders exactly as before.
    #[test]
    fn a_draw_carries_no_cull_until_one_is_asked_for() {
        let m = MeshHandle::default();
        let t = TextureHandle(0);
        assert_eq!(DrawCmd::textured(m, t, Mat4::IDENTITY).cull, None);
        assert_eq!(DrawCmd::textured_masked(m, t, Mat4::IDENTITY).cull, None);
        assert_eq!(DrawCmd::textured_blended(m, t, Mat4::IDENTITY).cull, None);
        assert_eq!(DrawCmd::flat(m, [1.0; 4], Mat4::IDENTITY).cull, None);

        let d = DrawCmd::textured_masked(m, t, Mat4::IDENTITY).with_cull(Some(1500.0));
        assert_eq!(d.cull, Some(1500.0));
        assert_eq!(d.layer, DrawLayer::Mask, "the cull does not move the pass");
        // `None` is a real value a caller can set back, not just the default.
        assert_eq!(d.with_cull(None).cull, None);

        // A cull is per INSTANCE, so two draws that differ only in it still
        // batch into one instanced call: `draw_key` must not see it.
        assert_eq!(
            draw_key(&DrawCmd::textured(m, t, Mat4::IDENTITY)),
            draw_key(&DrawCmd::textured(m, t, Mat4::IDENTITY).with_cull(Some(800.0)))
        );
    }

    /// Card LODP: the far switch's LOWDs sort after every other draw of
    /// their layer, and still before the next layer.
    #[test]
    fn a_lowd_sorts_last_in_its_layer() {
        let (m, t) = (MeshHandle(0), TextureHandle(0));
        let side = |side| {
            Some(crate::LodSwitch {
                at: 90.0,
                anchor: glam::Vec3::ZERO,
                radius: 1.0,
                side,
            })
        };
        let lowd =
            draw_key(&DrawCmd::textured(m, t, Mat4::IDENTITY).with_lod(side(crate::LodSide::Lowd)));
        let model = draw_key(
            &DrawCmd::textured(MeshHandle(9), t, Mat4::IDENTITY)
                .with_lod(side(crate::LodSide::Model)),
        );
        let plain = draw_key(&DrawCmd::textured(MeshHandle(9), t, Mat4::IDENTITY));
        let masked = draw_key(&DrawCmd::textured_masked(m, t, Mat4::IDENTITY));
        assert!(model < lowd && plain < lowd && lowd < masked);
        assert_eq!(model, plain, "the model side batches as any draw");
    }

    /// The specular pass adds over its base, so it sorts after every opaque
    /// and masked run, and before the water that composites over a model's
    /// finished batch (`analysis/re-specular-pass.md` §7). It carries its
    /// colour and power in the instance colour.
    #[test]
    fn the_specular_pass_sorts_between_the_solid_world_and_the_water() {
        let m = MeshHandle::default();
        let t = TextureHandle(0);
        let rank = |d: DrawCmd| draw_key(&d).0;
        let spec = DrawCmd::specular(m, t, [1.0, 0.5, 0.25], 51, Mat4::IDENTITY);
        assert_eq!(spec.layer, DrawLayer::Spec);
        assert_eq!(spec.color, [1.0, 0.5, 0.25, 0.2]);
        let spec = rank(spec);
        assert!(rank(DrawCmd::textured(m, t, Mat4::IDENTITY)) < spec);
        assert!(rank(DrawCmd::textured_masked(m, t, Mat4::IDENTITY)) < spec);
        assert!(spec < rank(DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY)));
    }

    /// The water pass's position in the sort is what makes it work: it must
    /// draw AFTER every opaque and masked run, so the world it composites
    /// over is already in the framebuffer — `draw_key`'s layer rank enforces
    /// that. That it draws BEFORE every blended draw (so a blended prop over
    /// the sea sees the water's depth) is `render_frame`'s pass order, not the
    /// key's — a blended draw never reaches `draw_key` — and the GPU referee
    /// `water_pass::the_water_still_occludes_what_is_drawn_after_it` holds it.
    #[test]
    fn the_water_pass_sorts_after_the_solid_world() {
        let m = MeshHandle::default();
        let t = TextureHandle(0);
        let rank = |d: DrawCmd| draw_key(&d).0;
        let opaque = rank(DrawCmd::textured(m, t, Mat4::IDENTITY));
        let mask = rank(DrawCmd::textured_masked(m, t, Mat4::IDENTITY));
        let water = rank(DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY));
        assert!(opaque < water && mask < water, "the water composites OVER");

        // The still plane's two layers are one mesh drawn twice with different
        // textures, the base uploaded first: the key's texture field keeps the
        // foam second within the pass. (A stepped ocean's foam rides its own
        // mesh, uploaded after the base's, so there the mesh field does —
        // `game::water_anim`.)
        let base = draw_key(&DrawCmd::water(
            m,
            TextureHandle(0),
            [1.0; 4],
            Mat4::IDENTITY,
        ));
        let foam = draw_key(&DrawCmd::water(
            m,
            TextureHandle(1),
            [1.0; 4],
            Mat4::IDENTITY,
        ));
        assert!(base < foam, "the foam layer draws over the base");

        // The tint is the instance colour, and the pass does not move.
        let d = DrawCmd::water(m, t, [0.08, 0.09, 0.08, 0.74], Mat4::IDENTITY);
        assert_eq!(d.color, [0.08, 0.09, 0.08, 0.74]);
        assert_eq!(d.layer, DrawLayer::Water);
        assert_eq!(d.cull, None, "the water is unculled until asked");
    }

    /// Card WADE: the skinned opaque draws go down after the last solid or
    /// specular run and before the first water one — and at the end of the
    /// runs on a map with no water. The picture is
    /// `skinned::an_opaque_skinned_draw_under_the_water_shows_through_it`.
    #[test]
    fn the_skinned_opaque_draws_precede_the_first_water_run() {
        for solid in [
            DrawLayer::Opaque,
            DrawLayer::Mask,
            DrawLayer::TerrainLayer,
            DrawLayer::FarSheet,
            DrawLayer::Spec,
        ] {
            assert!(!skinned_go_before(Some(draw_key_layer(solid))), "{solid:?}");
        }
        for wet in [DrawLayer::Water, DrawLayer::Speckle] {
            assert!(skinned_go_before(Some(draw_key_layer(wet))), "{wet:?}");
        }
        assert!(skinned_go_before(None), "no water run: after the last run");
    }

    /// The water's speckle adds over the FINISHED water, so it sorts after
    /// every water run (queue W3). That it draws before the blend pass is
    /// `render_frame`'s pass order, not the key's: a blended draw never
    /// reaches `draw_key`.
    #[test]
    fn the_speckle_sorts_after_the_water() {
        let rank = |d: DrawCmd| draw_key(&d).0;
        let (m, t) = (MeshHandle::default(), TextureHandle(0));
        let s = DrawCmd::speckle(m, t, [0.5, 0.5, 0.5, 0.25], Mat4::IDENTITY);
        assert_eq!(s.layer, DrawLayer::Speckle);
        assert_eq!(s.color, [0.5, 0.5, 0.5, 0.25]);
        let s = rank(s);
        assert!(rank(DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY)) < s);
    }
}
