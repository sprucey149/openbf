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

use crate::frustum;
use crate::gpu_timer::GpuTimer;
use crate::mesh::{self, GpuMesh, Instance};
use crate::{
    Camera, DrawCmd, DrawLayer, DrawOrder, Globals, LightPath, RenderError, Renderer,
    SkinnedDrawCmd, Target, TextureHandle, MAX_JOINTS, PALETTE_STRIDE,
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
        self.advance_sky();
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

        let ab = self.frame_ab;
        let (frame_width, frame_height) = self.size();
        // The world's own size (`world_scale`); the projection keeps the frame's shape.
        let (width, height) = crate::world_scale::scaled(
            (frame_width, frame_height),
            ab.world_scale.unwrap_or(self.world_scale_now),
        );
        let aspect = frame_width as f32 / frame_height as f32;
        let guard = crate::affine::guard(aspect);
        let view_proj = camera.view_proj(aspect);
        self.shadows.prepare(&self.device, &self.queue, view_proj);
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
        // the opaque and masked runs, which write depth and composite nothing
        // and go nearest first (`sort_runs`); the water runs composite, so the
        // key's layer rank and handle order carry their place. Blended draws CANNOT be reordered for batching:
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
        // Each draw's world box: the frustum test, the distance bands and the
        // near-first order below all read it, so a draw computes it once.
        let boxed = |d: &DrawCmd, meshes: &[GpuMesh]| {
            let m = meshes.get(d.mesh.0 as usize)?;
            let (lo, hi) = frustum::transformed_aabb(&d.model, m.bounds.0, m.bounds.1);
            Some((lo, hi, frustum::distance_range(lo, hi, eye)))
        };
        // Foliage past its fade: on a forest map, most of the instances.
        let shows = |d: &DrawCmd| {
            ab.foliage_unfiltered
                || d.fade.is_none_or(|fade| {
                    let at = glam::Vec2::new(d.model.w_axis.x - eye.x, d.model.w_axis.z - eye.z);
                    fade.shows(at.length())
                })
        };
        let visible = |d: &DrawCmd, b: Option<WorldBox>, f: &frustum::Frustum| {
            let Some((lo, hi, (near, far))) = b else {
                return true;
            };
            let (cull, cull_near) = crate::draw::instance_cull(d);
            f.intersects_aabb(lo, hi) && !crate::draw::all_discarded(cull, cull_near, near, far)
        };
        // The far scene's switch (card LOD) drops the side of a geometry
        // not drawn from this eye, with or without culling: its model and
        // LOWD must never both draw.
        let switched = |d: &DrawCmd| d.lod.is_none_or(|l| l.drawn(eye));
        let far_begin = self.lighting.far_fade.map(|f| f.begin);
        let mut keep = Vec::with_capacity(draws.len());
        let mut sorts: Vec<DrawSort> = Vec::with_capacity(draws.len());
        for d in draws {
            // Past its switch or its fade (on a forest map, most of the
            // instances) a draw needs no box, and nothing reads its sort.
            if self.cull
                && (!switched(d) || (self.meshes.get(d.mesh.0 as usize).is_some() && !shows(d)))
            {
                culled += 1;
                keep.push(false);
                sorts.push(DrawSort {
                    near: None,
                    may_discard: false,
                });
                continue;
            }
            let b = boxed(d, &self.meshes);
            let mut v = switched(d);
            if self.cull {
                v = v && visible(d, b, &frustum);
                culled += u32::from(!v);
            }
            keep.push(v);
            let (near, far) = b.map_or((0.0, 0.0), |b| b.2);
            sorts.push(DrawSort {
                near: (v && ab.order == DrawOrder::Near && may_move(d, far, far_begin))
                    .then(|| sort_distance(d, near, eye)),
                may_discard: ab.discard || crate::draw::may_discard(d, near, far, eye),
            });
        }

        let mut order = sort_runs(
            (0..draws.len()).filter(|&i| keep[i] && draws[i].layer != DrawLayer::Blend),
            draws,
            &sorts,
        );
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
                model: Self::sky_model(s, camera.position, self.atmosphere_time(), self.water_time)
                    .to_cols_array_2d(),
                color: Self::sky_color(s),
                // The sky is never distance-culled: on the PS2 it is drawn by
                // the sky tick under draw flag 0x20, outside both scene
                // ranges (`analysis/re-fog.md` 2.4 [V]).
                cull: [mesh::NO_CULL; 2],
                cull_near: [mesh::NO_CULL_NEAR; 2],
                light: s.emission.unwrap_or(LightPath::Lit) as u32,
                fade: mesh::NO_FADE,
                lights: [0; 4],
            })
            .collect();
        let object_blend = |slot: usize| {
            self.sky[slot].depth && Self::sky_mode(&self.sky[slot]) == crate::SkyMode::Blend
        };
        let mut sky_order: Vec<_> = (0..sky_base).filter(|&slot| !object_blend(slot)).collect();
        let mut blended_sky: Vec<_> = (0..sky_base)
            .filter(|&slot| object_blend(slot))
            .map(|slot| {
                let center = self
                    .meshes
                    .get(self.sky[slot].mesh.0 as usize)
                    .map_or(glam::Vec3::ZERO, |m| (m.bounds.0 + m.bounds.1) * 0.5);
                let point =
                    glam::Mat4::from_cols_array_2d(&instances[slot].model).transform_point3(center);
                (slot, point.distance_squared(camera.position))
            })
            .collect();
        blended_sky.sort_by(|a, b| b.1.total_cmp(&a.1));
        let sky_solid_count = sky_order.len();
        sky_order.extend(blended_sky.iter().map(|(slot, _)| *slot));
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
            // Only the joints a draw has: a slot's stride leaves room for
            // every joint a mesh could use, and those past its palette are
            // never read (checked above).
            let size = (skinned.len() * PALETTE_STRIDE) as wgpu::BufferAddress;
            let mut palettes = self
                .queue
                .write_buffer_with(
                    &self.palette_buf,
                    0,
                    wgpu::BufferSize::new(size).expect("skinned is not empty"),
                )
                .expect("the palette buffer holds every skinned draw");
            for (i, d) in skinned.iter().enumerate() {
                for (j, m) in d.palette.iter().enumerate() {
                    let at = i * PALETTE_STRIDE + j * 64;
                    palettes[at..at + 64]
                        .copy_from_slice(bytemuck::bytes_of(&m.to_cols_array_2d()));
                }
            }
            drop(palettes);
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
        let foliage = crate::grass::FoliageView {
            frustum: (!ab.foliage_unfiltered).then_some(&frustum),
        };
        let mut grass = crate::grass::build(
            &self.grass.classes,
            &self.grass.patches,
            camera,
            self.time,
            foliage,
        );
        crate::leaf::build(
            &mut grass,
            &self.leaves.classes,
            &self.leaves.objects,
            self.leaves.near,
            camera,
            self.time,
            foliage,
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
        if let Some(cloud) = &self.particles.cloud {
            particles
                .to_mut()
                .extend(cloud.draws(camera, self.atmosphere_time()));
        }
        let mut bolts = crate::particles::build(&[], &self.particles.sky, camera);
        let sky_particle_runs = bolts.runs.len();
        let mut world_particles = crate::particles::build(&self.bolts.bolts, &particles, camera);
        let base = bolts.vertices.len() as u32;
        for run in &mut world_particles.runs {
            run.vertices.start += base;
            run.vertices.end += base;
        }
        bolts.vertices.extend(world_particles.vertices);
        bolts.runs.extend(world_particles.runs);
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
        // A scaled world draws in its own target, frame and depth, until the
        // upscale below.
        let world_target = ((width, height) != (frame_width, frame_height)).then(|| {
            self.world_scale
                .target(&self.device, self.format, [width, height])
        });
        let frame_view = view.clone();
        let (frame_texture, view) = match &world_target {
            Some(t) => (&t.texture, t.view.clone()),
            None => (frame_texture, view),
        };
        let full_depth = world_target
            .as_ref()
            .map(|t| std::mem::replace(&mut self.depth_view, t.depth.clone()));
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

            // Sky first: background domes and depth-tested object hulls. The
            // stars go after the base dome's shells (its mirror included) and
            // under the rest (a cloud deck covers them), as both ELFs order
            // the sky pass (`analysis/re-stars.md` §3).
            let star_slot = self.stars.slot.min(self.sky.len());
            let mut sky_fx_drawn = false;
            if (sky_base > 0 || star_count > 0) && !ab.skips("sky") {
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                let mut bound = None;
                for item in sky_order[..sky_solid_count]
                    .iter()
                    .map(|&slot| Some((slot, &self.sky[slot])))
                    .chain([None])
                {
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
                    if !sky_fx_drawn && item.is_none_or(|(_, shell)| shell.depth) {
                        draw_calls += self.draw_particle_runs(
                            &mut pass,
                            &bolts.runs[..sky_particle_runs],
                            false,
                        );
                        sky_fx_drawn = true;
                        pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                        bound = None;
                    }
                    let Some((slot, shell)) = item else {
                        break;
                    };
                    let Some(mesh) = self.meshes.get(shell.mesh.0 as usize) else {
                        continue;
                    };
                    // Shells keep their authored order
                    // (a cloud deck composites over the dome, and an OPAQUE shell
                    // PAINTS over what is under it), so switch rather than sort.
                    let pipeline_idx = (
                        usize::from(shell.depth),
                        usize::from(shell.lit),
                        Self::sky_mode(shell).index(),
                    );
                    if bound != Some(pipeline_idx) {
                        pass.set_pipeline(
                            &self.sky_pipelines[pipeline_idx.0][pipeline_idx.1][pipeline_idx.2],
                        );
                        bound = Some(pipeline_idx);
                    }
                    pass.set_bind_group(1, self.tex_bg(shell.texture), &[]);
                    draw_mesh(&mut pass, mesh, slot as u32..slot as u32 + 1);
                    draw_calls += 1;
                }
            }
            if !sky_fx_drawn {
                draw_calls +=
                    self.draw_particle_runs(&mut pass, &bolts.runs[..sky_particle_runs], false);
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
            let mut bound_layer: Option<(DrawLayer, bool)> = None;
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
                    if !skin_opaque.is_empty() && !ab.skips("soldiers") {
                        pass.set_pipeline(match (fade, ab.discard) {
                            (false, false) => &self.no_discard.skinned,
                            (true, false) => &self.no_discard.fade_skinned,
                            (false, true) => &self.skinned_pipeline,
                            (true, true) => &self.fade_skinned_pipeline,
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
                    // BF1 bucket 6: all opaque geometry is down, water is not.
                    if !self.shadows.is_empty() {
                        drop(pass);
                        draw_calls +=
                            self.shadows
                                .draw(&mut encoder, frame_texture, &view, &self.depth_view);
                        pass = continue_pass(
                            &mut encoder,
                            &view,
                            &self.depth_view,
                            "after vehicle shadows",
                        );
                        mark(&mut timer, &mut pass, "vehicle shadows");
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                    }
                    // Back to the world's instances for the water runs.
                    pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                    bound_layer = None;
                }
                if key.is_none() {
                    break;
                }
                let mut run_end = run_start + 1;
                let run_key = runs_key(&draws[order[run_start]], sorts[order[run_start]]).0;
                while run_end < blend_base
                    && runs_key(&draws[order[run_end]], sorts[order[run_end]]).0 == run_key
                {
                    run_end += 1;
                }

                let d = &draws[order[run_start]];
                let may_discard = sorts[order[run_start]].may_discard;
                let run = run_section(d);
                if ab.skips(run) {
                    run_start = run_end;
                    continue;
                }
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
                    if bound_layer.map(|b| b.0) != Some(DrawLayer::Spec) {
                        pass.set_pipeline(&self.spec_pipeline);
                        pass.set_bind_group(2, &self.palette_bg, &[0]);
                        pass.set_bind_group(3, spec_bg, &[]);
                        bound_layer = Some((DrawLayer::Spec, may_discard));
                    }
                } else if bound_layer != Some((d.layer, may_discard)) {
                    let clean = &self.no_discard;
                    pass.set_pipeline(match (d.layer, fade) {
                        (DrawLayer::Water, _) if !may_discard => &clean.water,
                        (DrawLayer::TerrainLayer, _) if !may_discard => &clean.terrain_layer,
                        (DrawLayer::Mask, false) if !may_discard => &clean.mask,
                        (DrawLayer::Mask, true) if !may_discard => &clean.fade_mask,
                        (DrawLayer::Opaque, false) if !may_discard => &clean.opaque,
                        (DrawLayer::Opaque, true) if !may_discard => &clean.fade,
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
                    bound_layer = Some((d.layer, may_discard));
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
                grass.runs.is_empty() || ab.skips("grass"),
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
            if !ab.skips("sky") {
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                for &slot in &sky_order[sky_solid_count..] {
                    let shell = &self.sky[slot];
                    let Some(mesh) = self.meshes.get(shell.mesh.0 as usize) else {
                        continue;
                    };
                    pass.set_pipeline(&self.sky_pipelines[1][usize::from(shell.lit)][1]);
                    pass.set_bind_group(1, self.tex_bg(shell.texture), &[]);
                    let inst = slot as u32;
                    draw_mesh(&mut pass, mesh, inst..inst + 1);
                    draw_calls += 1;
                }
            }
            for &b in blended.iter().filter(|_| !ab.skips("blend")) {
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
            if self.particle_display.is_none() && !ab.skips("bolts+particles") {
                draw_calls +=
                    self.draw_particle_runs(&mut pass, &bolts.runs[sky_particle_runs..], false);
                mark(&mut timer, &mut pass, "bolts+particles");
            }
        }
        if let (Some(display), false) = (
            &self.particle_display,
            bolts.runs.is_empty() || ab.skips("bolts+particles"),
        ) {
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
            draw_calls +=
                self.draw_particle_runs(&mut pass, &bolts.runs[sky_particle_runs..], true);
            mark(&mut timer, &mut pass, "bolts+particles");
        }
        if bloom && !ab.skips("bloom") {
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
            if !ab.skips("sky") && self.sky.iter().any(|s| s.emission.is_some()) {
                for &slot in &sky_order {
                    let shell = &self.sky[slot];
                    let Some(mesh) = self.meshes.get(shell.mesh.0 as usize) else {
                        continue;
                    };
                    pass.set_pipeline(&self.bloom.gather.sky[Self::sky_mode(shell).index()]);
                    pass.set_bind_group(1, self.tex_bg(shell.texture), &[]);
                    let inst = slot as u32;
                    draw_mesh(&mut pass, mesh, inst..inst + 1);
                    draw_calls += 1;
                }
            }
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
        if filter && !ab.skips("filter") {
            draw_calls += self.filter_pass(&mut encoder, frame_texture, &view);
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "filter");
            }
        }
        if let Some(depth) = full_depth {
            self.depth_view = depth;
        }
        if let Some(t) = &world_target {
            self.world_scale.upscale(&mut encoder, t, &frame_view);
            draw_calls += 1;
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "upscale");
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
                &frame_view,
                [frame_width, frame_height],
            );
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "hud");
            }
        }
        if let Some(t) = &mut timer {
            t.resolve(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        self.shadows.clear();
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
            palette: Vec::new().into(),
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

    /// A section the frame's walk names can be left out by name: a label
    /// missing from `frame_ab`'s table could never be priced.
    #[test]
    fn every_run_section_can_be_left_out() {
        let d = DrawCmd::textured(MeshHandle(0), TextureHandle(0), Mat4::IDENTITY);
        for layer in [
            DrawLayer::Opaque,
            DrawLayer::Mask,
            DrawLayer::TerrainLayer,
            DrawLayer::FarSheet,
            DrawLayer::Spec,
            DrawLayer::Water,
            DrawLayer::Speckle,
            DrawLayer::Blend,
        ] {
            let d = DrawCmd { layer, ..d };
            let section = run_section(&d);
            let ab = crate::FrameAb::parse(&format!("no:{section}"));
            assert!(ab.is_ok_and(|ab| ab.skips(section)), "{section}");
        }
    }

    fn sorts_at(near: &[Option<f32>]) -> Vec<DrawSort> {
        near.iter()
            .map(|&near| DrawSort {
                near,
                may_discard: false,
            })
            .collect()
    }

    /// A draw that may discard takes its own run, with its own pipeline, but
    /// stays beside its mesh: the mesh order between runs does not move.
    #[test]
    fn a_draw_that_may_discard_splits_its_run_inside_its_mesh() {
        let t = TextureHandle(0);
        let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
        let draws = [d(1), d(1), d(2)];
        let at = |may_discard| DrawSort {
            near: Some(5.0),
            may_discard,
        };
        let sorts = [at(true), at(false), at(false)];
        assert_eq!(sort_runs(0..3, &draws, &sorts), vec![1, 0, 2]);
        assert_ne!(
            runs_key(&draws[0], sorts[0]).0,
            runs_key(&draws[1], sorts[1]).0
        );
    }

    #[test]
    fn a_packed_key_sorts_as_the_key_and_place_do() {
        let places = [-1.0f32, -0.0, 0.0, 1e-30, 1.0, 512.0, f32::INFINITY];
        for &f in &places {
            assert_eq!(from_total_order(total_order(f)), f.to_bits());
            for &g in &places {
                assert_eq!(total_order(f).cmp(&total_order(g)), f.total_cmp(&g));
            }
        }
        let keys: [RunKey; 8] = [
            (0, 0, 0, 0, 0, 0, false),
            (0, 0, 0, 0, 0, 0, true),
            (0, 0, 0, 0, 0, 1, false),
            (0, 0, 0, 0, u32::MAX, 0, false),
            (0, 0, 0, u32::MAX, 0, 0, false),
            (0, 0, 1, 0, 0, 0, false),
            (0, 1, 0, 0, 0, 0, false),
            (6, 0, 0, 0, 0, 0, false),
        ];
        for w in keys.windows(2) {
            assert!(packed_key(w[0], 512.0) < packed_key(w[1], -1.0));
        }
    }

    /// Near-first order: runs and the instances in them go nearest first, a
    /// run keeps together, and within a bucket the mesh order stands.
    #[test]
    fn opaque_runs_and_their_instances_go_nearest_first() {
        let t = TextureHandle(0);
        let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
        let draws = [d(1), d(2), d(1), d(3), d(3)];
        let near = sorts_at(&[Some(40.0), Some(100.0), Some(10.0), Some(10.5), Some(50.0)]);
        let order = sort_runs(0..draws.len(), &draws, &near);
        // Mesh 1 (10, 40) and mesh 3 (10.5, 50) tie on their bucket, so mesh
        // order decides between them; mesh 2 starts farther.
        assert_eq!(order, vec![2, 0, 3, 4, 1]);
    }

    /// One mesh at the eye and across the map splits, so its far instances
    /// wait for what hides them.
    #[test]
    fn a_run_splits_across_three_distance_doublings() {
        let t = TextureHandle(0);
        let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
        let draws = [d(1), d(1), d(1), d(2)];
        let near = sorts_at(&[Some(2.0), Some(3.0), Some(900.0), Some(30.0)]);
        let order = sort_runs(0..draws.len(), &draws, &near);
        assert_eq!(order, vec![0, 1, 3, 2], "mesh 2 between mesh 1's two runs");
        let keys: Vec<_> = order
            .iter()
            .map(|&i| runs_key(&draws[i], near[i]).0)
            .collect();
        assert_eq!(keys[0], keys[1], "the near pair stays one instanced run");
        assert_ne!(keys[1], keys[3], "the far instance is a run of its own");
    }

    /// A draw that composites keeps its key order, whatever its distance:
    /// the terrain's layers paint over each other in mesh order and the water
    /// over everything, and they stay after every opaque run.
    #[test]
    fn composited_layers_do_not_move_for_depth() {
        let (m, t) = (MeshHandle(1), TextureHandle(0));
        let layer = |mesh: u32| DrawCmd::terrain_layer(MeshHandle(mesh), t, Mat4::IDENTITY);
        let draws = [
            layer(9),
            layer(3),
            DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY),
            DrawCmd::textured(m, t, Mat4::IDENTITY),
        ];
        assert!(!may_move(&draws[0], 5.0, None));
        assert!(!may_move(&draws[2], 5.0, None));
        assert!(may_move(&draws[3], 5.0, None));
        let near = sorts_at(&[None, None, None, Some(900.0)]);
        assert_eq!(sort_runs(0..draws.len(), &draws, &near), vec![3, 1, 0, 2]);
    }

    /// On a fade map only a draw wholly short of the fade moves: past its
    /// start an opaque draw composites over what is already down.
    #[test]
    fn a_draw_reaching_the_far_fade_keeps_its_mesh_order_after_the_rest() {
        let t = TextureHandle(0);
        let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
        assert!(may_move(&d(1), 149.0, Some(150.0)));
        assert!(!may_move(&d(1), 150.0, Some(150.0)));
        let draws = [d(1), d(2), d(9)];
        let near = sorts_at(&[None, None, Some(300.0)]);
        assert_eq!(
            sort_runs(0..draws.len(), &draws, &near),
            vec![2, 0, 1],
            "the movable draw goes first even from farther"
        );
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
