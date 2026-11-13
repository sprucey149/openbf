//! Frame validation, buffer upload and ordered render passes.
use super::{
    blend_order, continue_pass, create_instance_buffer, create_palette_buffer,
    display_view_allowed, draw_key, draw_key_layer, draw_mesh, iface_dir, mark, may_move,
    run_section, runs_key, skinned_go_before, sort_distance, sort_runs, Blended, DrawSort,
    FrameStats, OverlayFn, WorldBox,
};
use crate::frustum;
use crate::mesh::{self, GpuMesh, Instance};
use crate::{
    Camera, DrawCmd, DrawLayer, DrawOrder, Globals, LightPath, RenderError, Renderer,
    SkinnedDrawCmd, Target, MAX_JOINTS, PALETTE_STRIDE,
};

impl Renderer {
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
        self.shadows
            .prepare(&self.device, &self.queue, camera.view_proj(aspect));
        let scene_camera = camera.reaching(self.far_range);
        let view_proj = scene_camera.view_proj(aspect);
        let iface_view_proj = Camera {
            position: glam::Vec3::ZERO,
            ..scene_camera
        }
        .view_proj(aspect);
        let frustum = frustum::Frustum::from_view_proj(&view_proj);
        let (lighting, dome_gain) = self.lightning.unwrap_or((self.lighting, 1.0));
        let water_fog = lighting.water_fog.or(lighting.fog);
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: camera.position.to_array(),
            guard_x: guard[0],
            sun_dir: self.lighting.sun_dir.normalize_or_zero().to_array(),
            sun_d4: f32::from(u8::from(self.lighting.sun_d4)),
            sun_color: lighting.sun_color,
            guard_y: guard[1],
            ambient_sky: lighting.ambient_sky,
            material_time: crate::material_scroll_time(self.water_time),
            ambient_ground: lighting.ambient_ground,
            fog_on: if lighting.fog.is_some() { 1.0 } else { 0.0 },
            fog_color: lighting.fog.map(|f| f.color).unwrap_or([0.0; 3]),
            fog_near: lighting.fog.map(|f| f.near).unwrap_or(0.0),
            fog_far: lighting.fog.map(|f| f.far).unwrap_or(1.0),
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
            water_gain: self.lighting.water_gain,
            speckle_light: self.lighting.water.speckle_light.to_array(),
            speckle_far_begin: self.lighting.water.far_range[0],
            speckle_shift: self.lighting.water.speckle_shift.to_array(),
            speckle_far_end: self.lighting.water.far_range[1],
            water_grid_origin: self.lighting.water.far_diffuse.origin,
            water_grid_step: self.lighting.water.far_diffuse.step,
            water_grid_dims: self.lighting.water.far_diffuse.dims,
            water_grid_range: self.lighting.water.far_diffuse.range,
            water_grid_colors: self.lighting.water.far_diffuse.colors,
            water_fog_color: water_fog.map_or([0.0; 3], |f| f.color),
            water_fog_on: f32::from(u8::from(water_fog.is_some())),
            water_fog_range: water_fog.map_or([0.0, 1.0], |f| [f.near, f.far]),
            _pad_water_fog: [0.0; 2],
            character_ambient: self.lighting.character_ambient.unwrap_or([0.0; 3]),
            character_ambient_on: f32::from(u8::from(self.lighting.character_ambient.is_some())),
            iface_dir: iface_dir(camera).to_array(),
            _pad11: 0.0,
            iface_view_proj: iface_view_proj.to_cols_array_2d(),
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
        let boxed = |d: &DrawCmd, meshes: &[GpuMesh], eye| {
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
        let mut iface_frustum = None;
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
            let iface = d.light == LightPath::Iface;
            let eye = if iface { glam::Vec3::ZERO } else { eye };
            let b = boxed(d, &self.meshes, eye);
            let mut v = switched(d);
            if self.cull {
                let view = if iface {
                    iface_frustum
                        .get_or_insert_with(|| frustum::Frustum::from_view_proj(&iface_view_proj))
                } else {
                    &frustum
                };
                v = v && visible(d, b, view);
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
        let atmosphere_time = self.atmosphere_time();
        let mut instances: Vec<Instance> = self
            .sky
            .iter()
            .map(|s| Instance {
                model: Self::sky_model(s, camera.position, atmosphere_time, self.water_time)
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
                region_top: [0.0; 4],
                region_bottom: [0.0; 4],
            })
            .collect();
        if dome_gain != 1.0 {
            for dome in instances.iter_mut().take(self.stars.slot) {
                for channel in &mut dome.color[..3] {
                    *channel *= dome_gain;
                }
            }
        }
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
            let region = draws[i].region_lit.and_then(|sphere| {
                crate::lighting::character_region_light(
                    &self.character_light_regions,
                    &lighting,
                    sphere,
                )
            });
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
                region_top: region.map_or([0.0; 4], |r| [r[0][0], r[0][1], r[0][2], r[0][3] + 1.0]),
                region_bottom: region.map_or([0.0; 4], |r| r[1]),
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
                .map(|d| {
                    let mut inst = Instance {
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
                        region_top: [0.0; 4],
                        region_bottom: [0.0; 4],
                    };
                    if d.light == LightPath::Character {
                        let sphere = d.lit.unwrap_or(crate::LitSphere {
                            centre: d.model.w_axis.truncate(),
                            radius: 0.0,
                        });
                        if let Some([top, bottom]) = crate::lighting::character_region_light(
                            &self.character_light_regions,
                            &lighting,
                            sphere,
                        ) {
                            inst.cull = [top[0], top[1]];
                            inst.cull_near = [top[2], top[3]];
                            inst.fade = bottom;
                            inst.light |= crate::lighting::CHARACTER_LIGHT_REGION;
                        }
                    }
                    inst
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
                .extend(cloud.draws(camera, atmosphere_time));
        }
        for dust in &mut self.particles.dust {
            particles
                .to_mut()
                .extend(dust.draws(camera, atmosphere_time));
        }
        let mut bolts = crate::particles::build(&[], &self.particles.sky, camera);
        let sky_particle_runs = bolts.runs.len();
        bolts.append(crate::particles::build(
            &self.bolts.bolts,
            &particles,
            camera,
        ));
        if !bolts.vertices.is_empty() {
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
        let particle_blur = !bolts.blur_runs.is_empty() && !ab.skips("bolts+particles");
        if particle_blur {
            self.particle_blur
                .prepare(&self.device, self.format, [width, height]);
        }
        let filter = self.filter_wanted();
        if filter {
            self.prepare_filter(width, height);
        }
        let softness = self.softness_wanted();
        if softness {
            self.prepare_softness(width, height);
        }

        // Acquire the frame's color view.
        let acquire = std::time::Instant::now();
        let (frame, view) = match &self.target {
            Target::Window { surface, .. } => match surface.get_current_texture() {
                Ok(frame) => {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    (Some(frame), view)
                }
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    // Rebuilt at the window's size, not the stale one; left
                    // alone while minimized (#142).
                    let window = self.window_size().unwrap_or((0, 0));
                    if let Some((w, h)) = crate::device::outdated_surface_size(window) {
                        self.reconfigure_window(w, h);
                    }
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
        let world_target = ((width, height) != (frame_width, frame_height)
            || (particle_blur
                && !frame_texture
                    .usage()
                    .contains(wgpu::TextureUsages::COPY_SRC)))
        .then(|| {
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
        let water_view = self.water_display_format.map(|format| {
            frame_texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(format),
                ..Default::default()
            })
        });
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
            let mut water_display_active = false;
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
                    water_display_active = false;
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
                    if water_display_active {
                        drop(pass);
                        pass = continue_pass(
                            &mut encoder,
                            &view,
                            &self.depth_view,
                            "after terrain layers",
                        );
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                        water_display_active = false;
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
                let water_display = water_view.is_some()
                    && matches!(
                        d.layer,
                        DrawLayer::TerrainLayer | DrawLayer::Water | DrawLayer::Speckle
                    );
                if water_display != water_display_active {
                    if let Some(l) = section.take() {
                        mark(&mut timer, &mut pass, l);
                    }
                    drop(pass);
                    pass = continue_pass(
                        &mut encoder,
                        if water_display {
                            water_view.as_ref().unwrap()
                        } else {
                            &view
                        },
                        &self.depth_view,
                        "water colour transition",
                    );
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                    pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                    bound_layer = None;
                    water_display_active = water_display;
                }
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
            if water_display_active {
                drop(pass);
                pass = continue_pass(&mut encoder, &view, &self.depth_view, "after water");
            }

            // The grass patches (queue FOLGG) and then the leaf patches (card
            // LEAF): after every opaque draw, before the blend pass, each run
            // one instanced draw of its class's sprite buffer.
            if let (false, Some((vbuf, _)), Some((ibuf, _))) = (
                grass.runs.is_empty() || ab.skips("grass"),
                &self.grass.vertex_buf,
                &self.grass.instance_buf,
            ) {
                if let Some(display_view) = &water_view {
                    drop(pass);
                    pass = continue_pass(
                        &mut encoder,
                        display_view,
                        &self.depth_view,
                        "grass display colour",
                    );
                }
                pass.set_pipeline(&self.grass_pipeline);
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.set_vertex_buffer(1, ibuf.slice(..));
                for run in &grass.runs {
                    pass.set_bind_group(1, self.tex_bg(run.texture), &[]);
                    pass.draw(run.vertices.clone(), run.instances.clone());
                    draw_calls += 1;
                }
                if water_view.is_some() {
                    drop(pass);
                    pass = continue_pass(
                        &mut encoder,
                        &view,
                        &self.depth_view,
                        "after grass display colour",
                    );
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                }
            }
            mark(&mut timer, &mut pass, "grass");

            // The blend pass: after every opaque draw, far to near, one at a
            // time — the world's blended draws (their instances sit at
            // blend_base.. of the instance buffer, in this order) and the
            // skinned ones where their distance puts them. Passing from one
            // kind to the other rebinds the pipeline and the instance buffer.
            let mut world_slot = blend_base;
            let mut bound_skinned: Option<(bool, bool, bool)> = None;
            if !ab.skips("sky") {
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                for &slot in &sky_order[sky_solid_count..] {
                    let pipeline = &self.sky_pipelines[1][usize::from(self.sky[slot].lit)][1];
                    draw_calls += self.draw_sky_slot(&mut pass, slot, pipeline);
                }
            }
            for &b in blended.iter().filter(|_| !ab.skips("blend")) {
                let skin = matches!(b, Blended::Skinned(_));
                let additive = matches!(b, Blended::World(i) if draws[i].additive);
                let fading = fade || matches!(b, Blended::World(i) if draws[i].fade.is_some());
                if bound_skinned != Some((skin, fading, additive)) {
                    pass.set_pipeline(match (skin, fading, additive) {
                        (false, false, true) => &self.additive_blend_pipeline,
                        (false, true, true) => &self.fade_additive_blend_pipeline,
                        (false, false, false) => &self.blend_pipeline,
                        (false, true, false) => &self.fade_blend_pipeline,
                        (true, false, _) => &self.skinned_blend_pipeline,
                        (true, true, _) => &self.fade_skinned_blend_pipeline,
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
                    bound_skinned = Some((skin, fading, additive));
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
        // The particle blur and the sun flare draw in display values. The
        // world target always carries the non-sRGB view; a surface carries
        // it only where the backend allows, and without it both are left out
        // rather than making an illegal view.
        let frame_display_view = (world_target.is_some()
            || display_view_allowed(self.format, self.display_view_format()))
        .then(|| {
            frame_texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.format.remove_srgb_suffix()),
                ..Default::default()
            })
        });
        if let (true, Some(frame_display_view)) = (particle_blur, &frame_display_view) {
            let mut pass = self.particle_blur.pass(
                &mut encoder,
                frame_texture,
                frame_display_view,
                &self.depth_view,
            );
            draw_calls += 1;
            pass.set_bind_group(0, &self.globals_bg, &[]);
            let characters = !skin_opaque.is_empty() && !ab.skips("soldiers");
            if characters {
                pass.set_stencil_reference(1);
                pass.set_pipeline(&self.heat_character_mask);
                pass.set_vertex_buffer(1, self.skinned_instance_buf.slice(..));
                for &i in &skin_opaque {
                    self.draw_skinned(&mut pass, &skinned[i], i);
                    draw_calls += 1;
                }
            }
            pass.set_vertex_buffer(0, self.bolts.vertex_buf.as_ref().unwrap().0.slice(..));
            for over_character in [false, true].into_iter().take(1 + usize::from(characters)) {
                self.particle_blur.bind_composite(&mut pass, over_character);
                for (texture, vertices) in &bolts.blur_runs {
                    pass.set_bind_group(1, self.tex_bg(Some(*texture)), &[]);
                    pass.draw(vertices.clone(), 0..1);
                    draw_calls += 1;
                }
            }
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
                    let pipeline = &self.bloom.gather.sky[Self::sky_mode(&self.sky[slot]).index()];
                    draw_calls += self.draw_sky_slot(&mut pass, slot, pipeline);
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
            draw_calls += self.sun_flare_pass(
                &mut encoder,
                self.bloom_gather_view(),
                camera,
                width,
                height,
                true,
            );
            draw_calls += self.bloom_pass(&mut encoder, frame_texture, &view);
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "bloom");
            }
        }
        if let Some(frame_display_view) = &frame_display_view {
            draw_calls += self.sun_flare_pass(
                &mut encoder,
                frame_display_view,
                camera,
                width,
                height,
                false,
            );
        }
        // The frame filter lifts the finished 3D frame, before the overlay
        // (the PS2 draws its HUD after it, `analysis/re-glow.md` §3.3).
        if filter && !ab.skips("filter") {
            draw_calls += self.filter_pass(&mut encoder, frame_texture, &view);
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "filter");
            }
        }
        if softness && !ab.skips("softness") {
            draw_calls += self.softness_pass(&mut encoder, frame_texture, &view);
            if let Some(t) = &mut timer {
                t.mark(&mut encoder, "softness");
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
            let suboptimal = frame.suboptimal;
            frame.present();
            // A frame presented at a stale size: rebuild the surface at the
            // window's, once the frame is gone (#142). The window is asked
            // only then: on X11 its size is a round trip to the server.
            if suboptimal {
                let window = self.window_size().unwrap_or((0, 0));
                let config = self.size();
                if let Some((w, h)) =
                    crate::device::suboptimal_rebuild_size(suboptimal, window, config)
                {
                    self.reconfigure_window(w, h);
                }
            }
        }
        Ok(())
    }
}
