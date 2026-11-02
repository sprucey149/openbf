//! The renderer's construction: `finish_init`, the shared tail of both
//! constructors (`device.rs`), builds the bind group layouts, the globals
//! buffer, the shader modules and every pipeline — opaque, mask, blend,
//! water, the water's speckle, the five `_fade` twins, the two sky modes, the skinned opaque and
//! blend passes, the additive specular pass, the grass, the bolts, BF2's
//! terrain light pass — each a `PipelineDesc` with the
//! fields its pass moves; then the depth target, the white texture and the
//! first instance and palette buffers.

use crate::bindings;
use crate::device::create_depth_texture;
use crate::frame::{create_instance_buffer, create_palette_buffer};
use crate::mesh::Instance;
use crate::texture::GpuTexture;
use crate::{
    FrameStats, Globals, Lighting, RenderError, Renderer, SamplerMode, SkinnedVertex, Target,
    TextureDesc, Vertex, WrapV, PALETTE_STRIDE,
};
use std::sync::Arc;

impl Renderer {
    /// Shared tail of both constructors: layouts, pipeline, globals, white texture.
    pub(super) fn finish_init(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        format: wgpu::TextureFormat,
        adapter_info: wgpu::AdapterInfo,
        gpu_turn: crate::device::GpuTurn,
    ) -> Result<Self, RenderError> {
        // Bind group layouts: group 0 = globals uniform, group 1 = texture+sampler.
        // Shape from Hangar pipeline_m.rs `BindGroupLayouts`.
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals bgl"),
            entries: &[
                bindings::uniform_layout(
                    0,
                    wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    false,
                    None,
                ),
                // BF2's local lights (`point_light.rs`): the view's table,
                // read by the world and skinned vertex shaders.
                bindings::uniform_layout(1, wgpu::ShaderStages::VERTEX, false, None),
            ],
        });
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture bgl"),
            entries: &[
                bindings::texture_layout(0, true),
                bindings::sampler_layout(1),
                // Detail map (PS2 `<name>_dtl` companion). Materials without
                // one bind the base view here and switch it off in params.
                bindings::texture_layout(2, true),
                bindings::uniform_layout(3, wgpu::ShaderStages::FRAGMENT, false, None),
            ],
        });

        let palette_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("palette bgl"),
            entries: &[bindings::uniform_layout(
                0,
                wgpu::ShaderStages::VERTEX,
                true,
                wgpu::BufferSize::new(PALETTE_STRIDE as u64),
            )],
        });

        let globals_buf = crate::resources::buffer(
            &device,
            "globals",
            std::mem::size_of::<Globals>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        // Zero-filled: no light until a frame files one.
        let points_buf = crate::resources::buffer(
            &device,
            "point lights",
            crate::point_light::POINT_TABLE_BYTES,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let globals_bg = crate::made::bind_group(
            &device,
            &wgpu::BindGroupDescriptor {
                label: Some("globals bg"),
                layout: &globals_bgl,
                entries: &[
                    bindings::entire_buffer(0, &globals_buf),
                    bindings::entire_buffer(1, &points_buf),
                ],
            },
        );

        // `shader.wgsl` is the world's module; the pipelines below pick entry
        // points out of it. The grass, the bolts and the terrain light pass
        // each build their own from it plus their file. Attribution and the
        // shape they share: [`PipelineDesc`].
        let shader = crate::made::shader_module(
            &device,
            wgpu::ShaderModuleDescriptor {
                label: Some("map shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            },
        );
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map pipeline layout"),
            bind_group_layouts: &[&globals_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });
        // Every pipeline below is [`PipelineDesc::world`] with one or two fields
        // moved; see [`PipelineDesc`] for the shape they share and why sharing
        // it is safe.
        let vb = [Vertex::layout(), Instance::layout()];
        let pipeline = PipelineDesc::world("map pipeline", &pipeline_layout, &vb)
            .build(&device, &shader, format);

        // Mask pipeline: the opaque pipeline exactly, through `fs_mask` — the
        // alpha test (`MASK_CUTOFF`) is the ONLY difference, so masked draws keep depth
        // writes, keep the instanced (mesh, texture) sort and need no
        // ordering. The disc's cutouts: leaves, bamboo, banners, grates.
        let mask_pipeline = PipelineDesc {
            fs: "fs_mask",
            ..PipelineDesc::world("map mask pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // Blend pipeline: the opaque pipeline with standard src-alpha
        // compositing and depth WRITES OFF (a transparent surface must not
        // occlude what is behind it), depth TEST still on so it is hidden by
        // solid geometry in front. `render_frame` sorts these draws far to near.
        let blend_pipeline = PipelineDesc {
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            ..PipelineDesc::world("map blend pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // Water pipeline: the blend pipeline with the depth write BACK ON and
        // its own fragment entry point. The PS2's near main water layer is
        // ALPHA 0x44 over a depth-tested, depth-WRITTEN target (TEST 0x50000,
        // `analysis/re-water.md` §1 [V]) — a translucent surface that still
        // occludes, which neither the opaque nor the blend pipeline spells.
        //
        // Depth compare is `LessEqual`, not `Less`, and that is the ocean's
        // doing: an `OceanEnable` map draws its foam layer as a SECOND draw of
        // the same surface at the same height, and the RE has both records
        // depth-tested (§3 cit. 5, base 0x69c3c0 / foam 0x69c3e0) — so a
        // fragment at exactly the stored depth must pass, or the foam could
        // never appear. The water is the only pass that draws one surface
        // twice, and it is the only pipeline that relaxes the test.
        //
        // There is no `_fade` twin: this target is ALREADY blended, so
        // `fs_water` can fold the far fade into alpha unconditionally
        // (`far_fade` returns exactly 1.0 on a map with no fade, and 1.0
        // src-alpha blending over an opaque frame is a paint).
        let water_pipeline = PipelineDesc {
            fs: "fs_water",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            ..PipelineDesc::world("map water pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // Speckle pipeline: the water's additive second layer (queue W3) —
        // the PS2's record 0x69c3b0, ALPHA 0x48 = `Cs·As + Cd`, TEST 0x51001,
        // depth-tested with NO depth write (`analysis/re-water.md` §1 [V]).
        // `fs_speckle` writes `Cs·As` premultiplied, so the colour blend is
        // (One, One); the destination's alpha is kept. `LessEqual` for the
        // water's reason: the layer lies on the depth the water just wrote.
        // It carries its fade in its own entry point, as the water does, so
        // there is no `_fade` twin. The specular pass shares the blend.
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let speckle_pipeline = PipelineDesc {
            fs: "fs_speckle",
            blend: Some(additive),
            depth_write: false,
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            ..PipelineDesc::world("map speckle pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // The `LOWR` far sheet (queue CLN5): the world pass's opaque state —
        // depth tested and written, no blend — with its own entry point,
        // `fs_far_sheet` (unlit, alpha-tested at AREF 0x10, fogged).
        let far_sheet_pipeline = PipelineDesc {
            fs: "fs_far_sheet",
            ..PipelineDesc::world("map far sheet pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // The terrain LAYERS (card TALPH): GS ALPHA 0x44 = src-alpha "over"
        // (`analysis/re-terrain-layers.md` §2.1), each layer drawn on the
        // triangles of the base terrain under it — hence `LessEqual`, the
        // water's reason — and writing no depth (the base already has).
        // `fs_terrain_layer` folds the far fade into alpha itself, so there
        // is no `_fade` twin.
        let terrain_layer_pipeline = PipelineDesc {
            fs: "fs_terrain_layer",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            ..PipelineDesc::world("map terrain layer pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // The FADE pipelines — the world pass on a map that authors
        // `FarSceneRange(0)` (`render::FarFade`, `game::viewer::far_fade_from`).
        // Each is the twin of an opaque, blend or skinned pipeline (the two
        // skinned twins are built below, beside their bases) with the colour
        // target BLENDED — the blend bases already are, so their twins move
        // only the entry point — and the fragment entry point moved to the
        // `_fade` variant that writes the depth fade into alpha:
        //
        //   * `fade_pipeline` and `fade_mask_pipeline` keep depth WRITES ON.
        //     The geometry is opaque geometry — it still occludes what is
        //     behind it — and inside b it writes alpha 1, where src-alpha
        //     blending is exactly a paint. Only the b..c band composites.
        //   * `fade_blend_pipeline` is the blend pipeline with the fade folded
        //     into the material's own alpha (depth writes stay OFF).
        //
        // They are built here rather than lazily so a map switch never
        // compiles a pipeline mid-frame; the cost is one pipeline build each
        // at renderer construction (the item 6 report measured the first
        // four; the skinned blend twin came with queue SKA).
        let fade_pipeline = PipelineDesc {
            fs: "fs_fade",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            ..PipelineDesc::world("map fade pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);
        let fade_mask_pipeline = PipelineDesc {
            fs: "fs_mask_fade",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            ..PipelineDesc::world("map fade mask pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);
        let fade_blend_pipeline = PipelineDesc {
            fs: "fs_blend_fade",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            ..PipelineDesc::world("map fade blend pipeline", &pipeline_layout, &vb)
        }
        .build(&device, &shader, format);

        // TWO sky pipelines: OPAQUE and BLEND.
        //
        // The MODE is the shell's own material's, not the pass's: the ELF
        // draws every model — sky shells included — with the GS state its
        // MTRL word asks for (`analysis/re-sky-dome-placement.md` §5 [V],
        // `FUN_0058ed70` 723411–723480). Under the SKY draw flag 0x20, which
        // the sky tick always passes, bit 0x04 OR bit 0x02 takes the
        // translucent path (GS ALPHA 0x44, no Z write) and a word with
        // neither is drawn OPAQUE — there is no alpha-TEST state on the sky
        // path at all (LEAD CORRECTION 2, session 12). See [`SkyMode`].
        //
        // There is no fogged variant: both ELFs set the sky draw flag 0x20
        // and both fog gates exclude it (`analysis/re-fog.md` §2.4 [V]), so
        // the sky is never fogged and one fragment entry point serves both
        // modes. See [`SkyShell`] for the port deviation this replaced.
        //
        // Depth is the same in both and unchanged: NO test and NO write,
        // so the shells paint the background first and every other draw lands
        // on top of them wherever the ~20 m dome mesh physically sits. The
        // ELF Z-writes an opaque shell at its far-scene range (RE §6 [H]) but
        // draws the sky first (task priority −9999, §1 [V]), so painting
        // first with depth off is the same picture for everything after.
        let sky_pipeline = |label: &'static str, blend| {
            PipelineDesc {
                fs: "fs_sky",
                blend,
                depth_write: false,
                depth_compare: Some(wgpu::CompareFunction::Always),
                ..PipelineDesc::world(label, &pipeline_layout, &vb)
            }
            .build(&device, &shader, format)
        };
        // Indexed [SkyMode::index()].
        let sky_pipelines = [
            sky_pipeline("sky pipeline (opaque)", None),
            sky_pipeline(
                "sky pipeline (blended)",
                Some(wgpu::BlendState::ALPHA_BLENDING),
            ),
        ];

        // Skinned pipelines: the world's OPAQUE and BLEND passes exactly —
        // same fragment entry points, same state — through `vs_skinned`, with
        // the joints/weights vertex layout and the palette bind group at 2.
        //
        // No skinned pipeline alpha-TESTS, because the disc never tests a side
        // model: it draws each segment with the GS state its own `MTRL` word
        // asks for — the same word, the same state as a rigid draw — and no
        // soldier segment on either disc has the cutout bit 0x02
        // (`analysis/re-side-alpha.md` §0–§4, `[O]`). An unflagged segment is
        // OPAQUE with its sheet's alpha ignored (`TEST 0x50000`), and the one
        // 0x04 segment per game (`all_inf_wookie`'s fur) is BLENDED:
        // `ALPHA 0x44` by the sheet's alpha, `TEST 0x51001` — Z-tested, no Z
        // write — which is `blend_pipeline`'s state. Until queue SKA every
        // skinned draw was tested at the port's own 0.5, which deleted most of
        // the wookie's fur.
        let skinned_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skinned pipeline layout"),
            bind_group_layouts: &[&globals_bgl, &texture_bgl, &palette_bgl],
            push_constant_ranges: &[],
        });
        let skinned_vb = [SkinnedVertex::layout(), Instance::layout()];
        let skinned = |label, fs, blend, depth_write| {
            PipelineDesc {
                vs: "vs_skinned",
                fs,
                blend,
                depth_write,
                ..PipelineDesc::world(label, &skinned_layout, &skinned_vb)
            }
            .build(&device, &shader, format)
        };
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let skinned_pipeline = skinned("skinned pipeline", "fs_main", None, true);
        let skinned_blend_pipeline = skinned("skinned blend pipeline", "fs_main", alpha, false);
        // The fade twins, as the world's: a soldier fades with the ground he
        // stands on instead of hanging unfaded in the sky at the far end of
        // the band.
        let fade_skinned_pipeline = skinned("skinned fade pipeline", "fs_fade", alpha, true);
        let fade_skinned_blend_pipeline =
            skinned("skinned fade blend pipeline", "fs_blend_fade", alpha, false);

        let world_scale = crate::world_scale::WorldScale::new(&device, format);
        // The same depth-writing passes without `far_cull`'s discards.
        let no_cull: &[(&str, f64)] = &[("CULLS", 0.0)];
        let clean = |desc: PipelineDesc<'_>| desc.build_with(&device, &shader, format, no_cull);
        let no_discard = crate::NoDiscard {
            opaque: clean(PipelineDesc::world(
                "map pipeline, no discard",
                &pipeline_layout,
                &vb,
            )),
            mask: clean(PipelineDesc {
                fs: "fs_mask",
                ..PipelineDesc::world("map mask pipeline, no discard", &pipeline_layout, &vb)
            }),
            fade: clean(PipelineDesc {
                fs: "fs_fade",
                blend: alpha,
                ..PipelineDesc::world("map fade pipeline, no discard", &pipeline_layout, &vb)
            }),
            fade_mask: clean(PipelineDesc {
                fs: "fs_mask_fade",
                blend: alpha,
                ..PipelineDesc::world("map fade mask pipeline, no discard", &pipeline_layout, &vb)
            }),
            terrain_layer: clean(PipelineDesc {
                fs: "fs_terrain_layer",
                blend: alpha,
                depth_write: false,
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                ..PipelineDesc::world(
                    "map terrain layer pipeline, no discard",
                    &pipeline_layout,
                    &vb,
                )
            }),
            water: clean(PipelineDesc {
                fs: "fs_water",
                blend: alpha,
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                ..PipelineDesc::world("map water pipeline, no discard", &pipeline_layout, &vb)
            }),
            skinned: clean(PipelineDesc {
                vs: "vs_skinned",
                ..PipelineDesc::world("skinned pipeline, no discard", &skinned_layout, &skinned_vb)
            }),
            fade_skinned: clean(PipelineDesc {
                vs: "vs_skinned",
                fs: "fs_fade",
                blend: alpha,
                ..PipelineDesc::world(
                    "skinned fade pipeline, no discard",
                    &skinned_layout,
                    &skinned_vb,
                )
            }),
        };

        // The SPECULAR pass (`specular.rs`): additive — One, One on colour,
        // the destination alpha kept — depth-tested `LessEqual` on its base
        // draw's own depth with no depth write
        // (`analysis/re-specular-pass.md` §2, §7, `[O]`). Its layout is the
        // skinned one plus the table at group 3, which the skinned twin
        // (`vs_spec_skinned`, a side model's pass, queue SPEC2) shares; the
        // world pass binds the palette at offset 0 and never reads it. No
        // `_fade` twin: `fs_spec` folds the fade in.
        let spec_bgl = crate::specular::spec_bind_group_layout(&device);
        let spec_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("specular pipeline layout"),
            bind_group_layouts: &[&globals_bgl, &texture_bgl, &palette_bgl, &spec_bgl],
            push_constant_ranges: &[],
        });
        let spec = |label, vs, vb| {
            PipelineDesc {
                vs,
                fs: "fs_spec",
                blend: Some(additive),
                depth_write: false,
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                ..PipelineDesc::world(label, &spec_layout, vb)
            }
            .build(&device, &shader, format)
        };
        let spec_pipeline = spec("specular pipeline", "vs_spec", &vb[..]);
        let spec_skinned_pipeline = spec(
            "skinned specular pipeline",
            "vs_spec_skinned",
            &skinned_vb[..],
        );
        // Grass pipeline (queue FOLGG, `grass.rs`): the world shader with
        // `grass.wgsl` appended, its own vertex and instance layouts, source-
        // over compositing with the depth test on and the depth write OFF —
        // the sprites and the patches arrive far to near. PROVISIONAL: the
        // PS2's GS state for them is not read (re-foliage §7 [H]).
        let grass_shader = crate::made::shader_module(
            &device,
            wgpu::ShaderModuleDescriptor {
                label: Some("grass shader"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(include_str!("shader.wgsl"), include_str!("grass.wgsl")).into(),
                ),
            },
        );
        let grass_vb = [
            crate::grass::GrassVertex::layout(),
            crate::grass::GrassInstance::layout(),
        ];
        let grass_pipeline = PipelineDesc {
            vs: "vs_grass",
            fs: "fs_grass",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            ..PipelineDesc::world("grass pipeline", &pipeline_layout, &grass_vb)
        }
        .build(&device, &grass_shader, format);
        // The laser bolts (card FXB, `bolt.rs`): `vs_grass` over world-space
        // ribbons; GS TEST 0x51001 = depth tested, no depth write
        // (re-bolt §3.3). The core is ALPHA 0x44 (source-over), the glow 0x48
        // (`fs_bolt_glow` writes Cs·As for the shared additive blend).
        let bolt_shader = crate::made::shader_module(
            &device,
            wgpu::ShaderModuleDescriptor {
                label: Some("bolt shader"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("shader.wgsl"),
                        include_str!("grass.wgsl"),
                        include_str!("bolt.wgsl")
                    )
                    .into(),
                ),
            },
        );
        let bolt_core_pipeline = PipelineDesc {
            vs: "vs_grass",
            fs: "fs_bolt_core",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            ..PipelineDesc::world("bolt core pipeline", &pipeline_layout, &grass_vb)
        }
        .build(&device, &bolt_shader, format);
        // The star field (`stars.rs`): the bolt glow's ADDITIVE shader
        // (the draw's GS ALPHA is 0x48, `analysis/re-stars.md` §3) over the
        // sky pass's state — no depth test or write.
        let star_pipeline = PipelineDesc {
            vs: "vs_grass",
            fs: "fs_bolt_glow",
            blend: Some(additive),
            depth_write: false,
            depth_compare: Some(wgpu::CompareFunction::Always),
            ..PipelineDesc::world("star pipeline", &pipeline_layout, &grass_vb)
        }
        .build(&device, &bolt_shader, format);
        let bolt_glow_pipeline = PipelineDesc {
            vs: "vs_grass",
            fs: "fs_bolt_glow",
            blend: Some(additive),
            depth_write: false,
            ..PipelineDesc::world("bolt glow pipeline", &pipeline_layout, &grass_vb)
        }
        .build(&device, &bolt_shader, format);
        let bolt_solid_pipeline = PipelineDesc {
            vs: "vs_grass",
            fs: "fs_bolt_solid",
            ..PipelineDesc::world("bolt solid pipeline", &pipeline_layout, &grass_vb)
        }
        .build(&device, &bolt_shader, format);
        let leaf_cutout_pipeline = PipelineDesc {
            vs: "vs_grass",
            fs: "fs_leaf_particle",
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_write: false,
            ..PipelineDesc::world("leaf cutout pipeline", &pipeline_layout, &grass_vb)
        }
        .build(&device, &bolt_shader, format);
        let display_format = match &target {
            Target::Window { config, .. } => config.view_formats.first().copied(),
            Target::Headless { .. } => Some(crate::device::HEADLESS_FORMAT.remove_srgb_suffix()),
        };
        let particle_display = display_format.map(|display| {
            let twin = |label: &str, fs: &str, blend: Option<wgpu::BlendState>, depth_write| {
                PipelineDesc {
                    vs: "vs_grass",
                    fs,
                    blend,
                    depth_write,
                    ..PipelineDesc::world(label, &pipeline_layout, &grass_vb)
                }
                .build(&device, &bolt_shader, display)
            };
            crate::bolt::DisplayPipelines {
                core: twin(
                    "bolt core display pipeline",
                    "fs_bolt_core_display",
                    Some(wgpu::BlendState::ALPHA_BLENDING),
                    false,
                ),
                glow: twin(
                    "bolt glow display pipeline",
                    "fs_bolt_glow_display",
                    Some(additive),
                    false,
                ),
                solid: twin(
                    "bolt solid display pipeline",
                    "fs_bolt_solid_display",
                    None,
                    true,
                ),
                cutout: twin(
                    "leaf cutout display pipeline",
                    "fs_leaf_particle_display",
                    Some(wgpu::BlendState::ALPHA_BLENDING),
                    false,
                ),
                format: display,
            }
        });
        // BF2's terrain light pass (card FXBL, `terrain_light.rs`): the
        // world vertex layout through `vs_terrain_light`, GS ALPHA 0x148 and
        // TEST 0x53001 = an add, depth tested on the terrain's own depth, no
        // depth write (`analysis/re-bolt.md` §5.3). `fs_terrain_light` writes
        // the linear difference of a gamma-space add, so the blend is the
        // shared additive one.
        let terrain_light_bgl = crate::terrain_light::terrain_light_bind_group_layout(&device);
        let terrain_light_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain light pipeline layout"),
            bind_group_layouts: &[&globals_bgl, &terrain_light_bgl],
            push_constant_ranges: &[],
        });
        let terrain_light_shader = crate::made::shader_module(
            &device,
            wgpu::ShaderModuleDescriptor {
                label: Some("terrain light shader"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("shader.wgsl"),
                        include_str!("terrain_light.wgsl")
                    )
                    .into(),
                ),
            },
        );
        let terrain_light_pipeline = PipelineDesc {
            vs: "vs_terrain_light",
            fs: "fs_terrain_light",
            blend: Some(additive),
            depth_write: false,
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            ..PipelineDesc::world("terrain light pipeline", &terrain_light_layout, &vb)
        }
        .build(&device, &terrain_light_shader, format);
        let spec_buf = crate::resources::buffer(
            &device,
            "specular params",
            std::mem::size_of::<crate::specular::SpecParams>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let copyable = match &target {
            Target::Window { config, .. } => config.usage.contains(wgpu::TextureUsages::COPY_SRC),
            Target::Headless { .. } => true,
        };
        // The DomeInfo frame filter (card BFLT, `filter.rs`): its own
        // full-screen pipeline over a copy of the finished 3D frame.
        let filter = crate::filter::FilterState::new(&device, format, copyable);
        let gather_world = |fs, blend| {
            crate::bloom::gather(
                &device,
                &shader,
                PipelineDesc {
                    blend,
                    ..PipelineDesc::world("bloom surface", &pipeline_layout, &vb)
                },
                fs,
            )
        };
        let gather = crate::bloom::Gather {
            world: gather_world("fs_bloom", None),
            mask: gather_world("fs_bloom_mask", None),
            blend: gather_world("fs_bloom", Some(additive)),
            particle: crate::bloom::gather(
                &device,
                &bolt_shader,
                PipelineDesc {
                    vs: "vs_grass",
                    blend: Some(additive),
                    ..PipelineDesc::world("bloom particle", &pipeline_layout, &grass_vb)
                },
                "fs_bloom_particle",
            ),
        };
        let bloom = crate::bloom::BloomState::new(&device, format, copyable, gather);

        let (w, h) = match &target {
            Target::Window { config, .. } => (config.width, config.height),
            Target::Headless { width, height, .. } => (*width, *height),
        };
        let depth_view = create_depth_texture(&device, w, h);

        let white = GpuTexture::upload(
            &device,
            &queue,
            &texture_bgl,
            &TextureDesc {
                width: 1,
                height: 1,
                mips: &[&[255, 255, 255, 255]],
                sampler: SamplerMode::Nearest,
                wrap_v: WrapV::Repeat,
                srgb: false,
                detail: None,
                scroll: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
                label: Some("white"),
            },
        )?;

        let instance_capacity = 256;
        let instance_buf = create_instance_buffer(&device, instance_capacity);
        let palette_capacity = 16;
        let (palette_buf, palette_bg) =
            create_palette_buffer(&device, &palette_bgl, palette_capacity);
        let skinned_instance_buf = create_instance_buffer(&device, palette_capacity);
        let gpu_timer = crate::gpu_timer::GpuTimer::new(&device, &queue);

        Ok(Self {
            shadows: crate::shadow::ShadowState::new(
                &device,
                format,
                match &target {
                    Target::Window { config, .. } => config.view_formats.first().copied(),
                    Target::Headless { .. } => Some(format.remove_srgb_suffix()),
                },
            ),

            format,
            device: Arc::new(device),
            queue: Arc::new(queue),
            target,
            depth_view,
            pipeline,
            globals_buf,
            globals_bg,
            points_buf,
            point_lights: Vec::new(),
            point_table_clear: true,
            texture_bgl,
            meshes: Vec::new(),
            textures: Vec::new(),
            white,
            instance_buf,
            instance_capacity,
            mask_pipeline,
            blend_pipeline,
            water_pipeline,
            speckle_pipeline,
            far_sheet_pipeline,
            terrain_layer_pipeline,
            no_discard,
            fade_pipeline,
            fade_mask_pipeline,
            fade_blend_pipeline,
            fade_skinned_pipeline,
            fade_skinned_blend_pipeline,
            sky_pipelines,
            sky: Vec::new(),
            frame_stats: FrameStats::default(),
            gpu_timer,
            acquire_ms: 0.0,
            cull: true,
            frame_ab: crate::FrameAb::default(),
            world_scale_now: 1.0,
            world_scale,
            skinned_pipeline,
            skinned_blend_pipeline,
            skinned_meshes: Vec::new(),
            palette_bgl,
            palette_buf,
            palette_bg,
            palette_capacity,
            skinned_instance_buf,
            spec_pipeline,
            spec_skinned_pipeline,
            spec_bgl,
            grass_pipeline,
            grass: Default::default(),
            leaves: Default::default(),
            falling: Default::default(),
            bolt_core_pipeline,
            bolt_glow_pipeline,
            bolt_solid_pipeline,
            leaf_cutout_pipeline,
            particle_display,
            bolts: Default::default(),
            star_pipeline,
            stars: Default::default(),
            particles: Default::default(),
            terrain_light_pipeline,
            terrain_light_bgl,
            terrain_lights: crate::terrain_light::TerrainLightState {
                copyable,
                ..Default::default()
            },
            filter,
            bloom,
            spec_buf,
            spec_bg: None,
            spec_mirror: true,
            lighting: Lighting::default(),
            time: 0.0,
            water_time: 0.0,
            adapter_info,
            _gpu_turn: gpu_turn,
        })
    }
}

// ── Internals ────────────────────────────────────────────────────────────────

/// Every pipeline writes the frame's COLOUR only: its alpha plane stays at
/// the pass's clear value, 1.0 (`frame.rs`), so the frame is opaque.
///
/// A draw's alpha is a blend weight, not coverage: since card DSHN a pass
/// sheet keeps its disc gloss alpha (SBD mean 34/255), and the opaque base
/// pass used to write it into the frame — the droid goldens carried alpha
/// < 200 on 7.6 % of their pixels, and a compositing surface could show the
/// desktop through them (card FALPH). No blend here reads the destination's
/// alpha (the factors are `SrcAlpha`, `OneMinusSrcAlpha`, `One` and `Zero`
/// only), so masking it moves no colour. The egui overlay (`ui::overlay`)
/// blends alpha as `(1 − Ad)·As + Ad`, which is 1 over an opaque frame.
pub(crate) const FRAME_WRITES: wgpu::ColorWrites = wgpu::ColorWrites::COLOR;

/// The one render-pipeline shape this crate builds, with the fields that
/// actually differ between passes lifted out.
///
/// Every pipeline here — opaque, mask, blend, the two sky shells, skinned —
/// was a separate 40-line `RenderPipelineDescriptor` differing in at most four
/// of those fields; the rest (one color target at the target format with
/// [`FRAME_WRITES`], a `TriangleList` with **no culling** because baked map
/// geometry has mixed winding, optional `Depth24PlusStencil8` with default stencil
/// and bias, no multisampling) is identical on purpose and is written
/// once here. A new pass is [`PipelineDesc::world`] plus the fields it moves.
///
/// Base shape adapted from Hangar `pipeline_m.rs` `MeshPipeline::new`, plus the
/// instance-rate buffer for per-draw model/color.
pub(crate) struct PipelineDesc<'a> {
    pub(crate) label: &'a str,
    pub(crate) layout: &'a wgpu::PipelineLayout,
    /// Vertex entry point in the pipeline's module.
    pub(crate) vs: &'a str,
    /// Vertex + instance buffer layouts, in bind order.
    pub(crate) buffers: &'a [wgpu::VertexBufferLayout<'a>],
    /// Fragment entry point in the pipeline's module.
    pub(crate) fs: &'a str,
    /// `None` = opaque write; `Some(..)` = composited.
    pub(crate) blend: Option<wgpu::BlendState>,
    pub(crate) depth_write: bool,
    /// `None` disables the depth attachment; otherwise use Depth24PlusStencil8.
    pub(crate) depth_compare: Option<wgpu::CompareFunction>,
}

impl<'a> PipelineDesc<'a> {
    /// The world pass: `vs_main` -> `fs_main`, opaque, depth test `Less` with
    /// writes ON. An opaque draw composites nothing, so the order its draws
    /// arrive in is free; every other pipeline starts from this one and moves
    /// the entry point, the blend or the depth state.
    fn world(
        label: &'a str,
        layout: &'a wgpu::PipelineLayout,
        buffers: &'a [wgpu::VertexBufferLayout<'a>],
    ) -> Self {
        Self {
            label,
            layout,
            vs: "vs_main",
            buffers,
            fs: "fs_main",
            blend: None,
            depth_write: true,
            depth_compare: Some(wgpu::CompareFunction::Less),
        }
    }

    pub(crate) fn build(
        &self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        self.build_with(device, shader, format, &[])
    }

    /// This pipeline with the fragment stage's override `constants`.
    fn build_with(
        &self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        constants: &[(&str, f64)],
    ) -> wgpu::RenderPipeline {
        crate::made::render_pipeline(
            device,
            &wgpu::RenderPipelineDescriptor {
                label: Some(self.label),
                layout: Some(self.layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some(self.vs),
                    buffers: self.buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(self.fs),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: self.blend,
                        write_mask: FRAME_WRITES,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants,
                        ..Default::default()
                    },
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // No culling for v0: baked map geometry has mixed winding.
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: self
                    .depth_compare
                    .map(|depth_compare| wgpu::DepthStencilState {
                        format: crate::device::DEPTH_FORMAT,
                        depth_write_enabled: self.depth_write,
                        depth_compare,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            },
        )
    }
}
