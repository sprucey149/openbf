//! Build the uploaded world draw list and its source census.

use super::{
    anyhow, apply_lighting, clear_color, draw_in, far_cull, foam_uv_scale, get_material_texture,
    get_terrain_texture, get_texture, install_omniproj, install_spec_table, instance_matrix,
    lod_bands, lod_mode, lowres_enabled, lowres_range, spec_cull, speckle_look, star_field,
    terrain_drawn, texture_has_alpha, upload_atmosphere, upload_cpu_texture_scrolling, upload_sky,
    upload_submeshes, upload_terrain_chunks, upload_terrain_layer_chunks, water_look, BoundTexture,
    Census, DoorDraw, DrawCmd, HashMap, Mat4, ModelPart, PlacedModel, Renderer, Result, Scene,
    SceneGpu, SpeckleAnim, TextureHandle, Untextured, Vec3, CLAMP_V_KEY_PREFIX, WATER_TINT,
};

pub fn upload_scene(r: &mut Renderer, scene: &Scene) -> Result<SceneGpu> {
    let mut textures: HashMap<String, BoundTexture> = HashMap::new();
    let mut draws = Vec::new();
    let mut census = Census::default();
    // The far-scene cull, off unless `OPENBF_FAR_CULL=1`. It is decided ONCE
    // here and carried per draw (`render::DrawCmd::cull`), because the two
    // distances belong to different draws: the terrain stops at the near
    // scene's far plane, the models and the water at `FarSceneRange`. The
    // sky's own upload path takes none.
    let cull = far_cull(&scene.manifest.lighting);

    // Terrain at identity: bake emits world-space positions. Its material is
    // LTEX layer 0, tiled by SCAL[0] in the baked UVs (bake::terrain_glb).
    // SPLIT into draw chunks here rather than in the bake — see
    // `upload_terrain_chunks`.
    //
    // A chunk whose layer texture does not resolve is NOT DRAWN. There used
    // to be a `TERRAIN_TINT` fallback (a tan we picked), and exactly one map
    // ever hit it: BF1 Bespin Platforms, whose `tern` names `LTEX("kam1_water")`
    // and `DTEX("yav_decal")` — both textures from OTHER maps' LVLs, in no
    // chunk BES1 ships, so the PS2 cannot have textured that sheet from
    // BES1.LVL either. Painting it tan laid a flat invented floor over the
    // cloud art Bespin DOES ship below its horizon
    // (`bes1_sky_dome_clouds_2`, 60 vertices, every one below the horizon).
    // `Scene::heightfield` still reads `scene.terrain`, so the sim's ground
    // is untouched — this only decides what is DRAWN.
    //
    // A BF1 map authoring `DomeInfo TerrainEnable(0)` (KAM1, BES1) draws no
    // terrain at all — neither the base nor the layers: see `terrain_drawn`.
    let terrain = scene
        .terrain
        .as_ref()
        .filter(|_| terrain_drawn(&scene.manifest.source_game, scene.manifest.sky.as_ref()));
    if let Some(t) = terrain {
        for (mesh, mat, layer, _) in upload_terrain_chunks(r, t)? {
            let tex = match mat {
                Some(m) => get_terrain_texture(r, scene, &mut textures, &m.to_ascii_lowercase())?,
                None => None,
            };
            draws.push(match tex {
                // UNLIT with the GS vertex byte, per game: no ambient, sun
                // or Light2 reaches the PS2's terrain (re-terrain-lighting.md
                // §0, [O]).
                Some(b) => draw_in(layer, mesh, b.handle, Mat4::IDENTITY)
                    .with_cull(cull.map(|c| c.terrain))
                    .with_light(render::LightPath::Terrain),
                None => {
                    // Its own report line: this is a whole SHEET of terrain
                    // whose layer texture lives in another map's LVL, not a
                    // per-segment authoring gap. See `terrain_dropped`.
                    census.terrain_dropped += 1;
                    continue;
                }
            });
        }
    }

    // The terrain's texture LAYERS above LTEX[0] (card TALPH,
    // `analysis/re-terrain-layers.md`): each layer's triangles composited
    // over the base terrain by their `ALPH` alpha, in ascending LTEX order —
    // the order `upload_terrain_layer_chunks` uploads them in, which the
    // renderer's `TerrainLayer` pass keeps. Chunked like the base for the
    // frustum cull; a layer whose texture does not resolve is dropped like a
    // base chunk (none on the disc: every LTEX name ships in its own LVL).
    if let (Some(t), Some(layers)) = (terrain, &scene.terrain_layers) {
        for (mesh, mat) in upload_terrain_layer_chunks(r, t, layers)? {
            match get_terrain_texture(r, scene, &mut textures, &mat.to_ascii_lowercase())? {
                Some(b) => draws.push(
                    DrawCmd::terrain_layer(mesh, b.handle, Mat4::IDENTITY)
                        .with_cull(cull.map(|c| c.terrain)),
                ),
                None => census.terrain_dropped += 1,
            }
        }
    }

    // The `LOWR` far-LOD sheet at identity, like the terrain, and behind
    // `OPENBF_LOWRES=1`: the only terrain either PS2 draws past its near
    // scene (`analysis/re-lowr.md` §0.2 [V]). Its range is
    // [`lowres_range`] — MaxDistance out, and the heightfield's own end in,
    // which is the port's one deviation and is written up there.
    //
    // Drawn on its own pass, `DrawLayer::FarSheet` (queue CLN5): UNLIT, as
    // the disc's base colour 0x80808080 is GS unity; alpha-tested at the
    // disc's AREF 0x10, which cuts real holes in BF1 KAS1's and KAS2's
    // sheets (`bake::lowres_glb`); and fogged like any far-scene draw — the
    // sheet is drawn with the `fogspread` program, never as sky
    // (re-lowr.md §2.2 [V]).
    // `TerrainEnable(0)` switches it off with the rest (`terrain_drawn`).
    if lowres_enabled() && terrain_drawn(&scene.manifest.source_game, scene.manifest.sky.as_ref()) {
        if let (Some(mesh), Some((near, far))) = (
            &scene.lowres,
            lowres_range(cull, &scene.manifest.lighting, scene.manifest.sky.as_ref()),
        ) {
            let name = scene
                .manifest
                .sky
                .as_ref()
                .and_then(|s| s.low_res_terrain.as_deref())
                .unwrap_or_default();
            // WrapV::Repeat because the two maps that author a big
            // `TextureScale` (BES2 20, END1 19) tile their sheet.
            match get_texture(r, scene, &mut textures, name, render::WrapV::Repeat)? {
                Some(b) => {
                    let handle = r
                        .add_mesh(&mesh.mesh_data(&mesh.indices))
                        .map_err(|e| anyhow!("lowres mesh upload: {e}"))?;
                    draws.push(
                        DrawCmd::far_sheet(handle, b.handle, Mat4::IDENTITY)
                            .with_cull(Some(far))
                            .with_cull_near(Some(near)),
                    );
                }
                // Same reasoning as the terrain's dropped sheet: a whole
                // surface with no texture is not worth an invented colour.
                None => census.failed("lowres", Some(name)),
            }
        }
    }

    // Water at identity, like the terrain: `bake::water_glb` emits world-space
    // positions at the plane's own height. Its texture gets a DEDICATED
    // handle so the authored UV scroll cannot leak into a model that happens
    // to share the name.
    //
    // Its LOOK is [`water_look`] — the disc's tint and blend weight, its own
    // fog rule (carried in `render::Lighting`, not per draw) and its own far
    // distance. On FLAT water that distance rides the same `with_cull` switch
    // as everything else and so is OFF unless `OPENBF_FAR_CULL=1`: culling
    // the water while the terrain under it runs on to the horizon would open
    // a hole in the sea and show the seabed, which is a worse picture than
    // the one the flag keeps off by default (`far_cull_enabled`; queue L's
    // landing). An OCEAN always ends there (card KSEA,
    // `analysis/re-kamino.md` §2): the PS2 draws no water past the ocean's
    // far pass and the frame behind it is black, and neither Kamino draws a
    // seabed. A stepped ocean ends on whole cells (`water_anim`); this still
    // plane, which has no eye to pick cells by, is cut per fragment.
    let mut water_motion = None;
    let mut speckle = None;
    if let (Some(info), Some(mesh)) = (&scene.manifest.water, &scene.water) {
        let first = draws.len();
        let look = water_look(&scene.manifest.source_game, &scene.manifest.lighting, info);
        let ocean = info.config.as_ref().is_some_and(|c| c.ocean);
        let far = if ocean { look.far } else { cull.and(look.far) };
        let mut handle = |name: Option<&str>, uv_scale| -> Result<Option<TextureHandle>> {
            let Some(t) = name.and_then(|n| scene.textures.get(&n.to_ascii_lowercase())) else {
                return Ok(None);
            };
            let upload = if ocean {
                upload_cpu_texture_scrolling
            } else {
                super::upload::upload_flat_water_texture
            };
            Ok(Some(upload(
                r,
                t,
                name.unwrap_or("water"),
                info.uv_velocity,
                uv_scale,
            )?))
        };
        let tex = handle(info.texture.as_deref(), [1.0, 1.0])?;
        // The ocean's foam layer: a SECOND draw of the same surface with the
        // map's `FoamTexture`, blended over the opaque base by its sheet's
        // alpha × its vertex alpha (`analysis/re-water.md` §3 cit. 5 [V],
        // `re-water-alpha.md` §0 [O]). Both KAM1s and nothing else. That
        // vertex alpha is the disc's per-point foam byte, which a STEPPED
        // ocean carries (`water_anim::Foam`, queue OCF). The still plane is
        // flat, and on a flat field the disc's rule decays every foam byte to
        // 0, so this still draw carries no foam (alpha 0, set below once
        // `WaterMotion::new` has taken its templates at the look's foam
        // alpha: 1, or the lit ocean's 0.5) — what an
        // unstepped scene shows; `WaterMotion::step` replaces it. Its colour is
        // unity (WCOLB): the baked mesh's placeholder white alpha under a
        // unity foam would paint the whole sea with the sheet.
        //
        // It is drawn only if the sheet carries transparency: blending an
        // opaque layer is a PAINT that would hide `kam1_water_2` and put back
        // the flat grey plane W1 removed. Both KAM1s' do since WA (`19975d0`,
        // 2026-09-23) — `kam1_water` keeps its disc alpha, 0..218
        // (`re-water-alpha.md` §4), where the bake before it forced 255.
        let foam = match look.foam_tint {
            Some(_) => {
                let name = info.config.as_ref().and_then(|c| c.foam_texture.as_deref());
                match name
                    .and_then(|n| scene.textures.get(&n.to_ascii_lowercase()))
                    .is_some_and(texture_has_alpha)
                {
                    true => handle(name, foam_uv_scale(info.config.as_ref()))?,
                    false => None,
                }
            }
            None => None,
        };
        let normal_cfg = info
            .config
            .as_ref()
            .filter(|_| info.texture.is_none())
            .and_then(|c| c.normal_maps.as_ref());
        let mut normals = Vec::new();
        if let Some(n) = normal_cfg {
            for name in &n.textures {
                let Some(t) = scene.textures.get(&name.to_ascii_lowercase()) else {
                    normals.clear();
                    break;
                };
                let mips = t.mips();
                normals.push(
                    r.add_texture(&render::TextureDesc {
                        width: t.width,
                        height: t.height,
                        mips: &mips,
                        sampler: render::SamplerMode::Linear,
                        wrap_v: render::WrapV::Repeat,
                        srgb: false,
                        detail: None,
                        scroll: info.uv_velocity,
                        uv_scale: [1.0, 1.0],
                        label: Some(name),
                    })
                    .map_err(|e| anyhow!("normal water {name}: {e}"))?,
                );
            }
        }
        // The speckle layer ([`speckle_look`], queue W3): its frames get
        // their own handles, carrying the layer's tiling and scroll. The
        // WATR cell side comes from the wet mask, or from the first lattice
        // quad on older bakes. A frame the bake
        // did not ship drops the whole layer — 0 on the shipped maps.
        let cell = info.wet.as_ref().map(|w| w.cell_size).or_else(|| {
            mesh.indices
                .get(..6)
                .and_then(|q| crate::water_anim::lattice_cells(&mesh.positions, q))
                .map(|l| l.cell)
        });
        let look_s = cell.and_then(|cell| speckle_look(&scene.manifest.source_game, info, cell));
        let mut frames = Vec::new();
        if let Some(sp) = &look_s {
            for n in &sp.frames {
                match scene.textures.get(&n.to_ascii_lowercase()) {
                    Some(t) => frames.push(super::upload::upload_flat_water_texture(
                        r,
                        t,
                        n,
                        sp.scroll,
                        sp.uv_scale,
                    )?),
                    None => {
                        frames.clear();
                        break;
                    }
                }
            }
        }
        let mut speckle_first = Vec::new();
        let mut normal_first = Vec::new();
        let mut still_foam = Vec::new();
        for (m, _, _, _) in upload_submeshes(r, mesh)? {
            let tint = if info.vertex_diffuse {
                [1.0, 1.0, 1.0, look.tint[3]]
            } else {
                look.tint
            };
            if let (Some(n), Some(h)) = (normal_cfg, normals.first()) {
                normal_first.push(draws.len());
                draws.push(
                    DrawCmd::water(
                        m,
                        *h,
                        n.refraction.map(|v| (v / 255.0).clamp(0.0, 1.0)),
                        Mat4::IDENTITY,
                    )
                    .with_light(render::LightPath::WaterNormal)
                    .with_cull(far),
                );
                continue;
            }
            draws.push(match tex {
                Some(h) => DrawCmd::water(m, h, tint, Mat4::IDENTITY).with_cull(far),
                None => {
                    // The water plane keeps its own tint rather than being
                    // routed: the manifest always names a texture, so a miss
                    // here IS a lookup failure. 0 on all 32 maps.
                    census.failed("<water>", info.texture.as_deref());
                    DrawCmd::flat(m, WATER_TINT, Mat4::IDENTITY).with_cull(far)
                }
            });
            if let (Some(h), Some(tint)) = (foam, look.foam_tint) {
                still_foam.push(draws.len());
                draws.push(DrawCmd::water(m, h, tint, Mat4::IDENTITY).with_cull(far));
            }
            if let (Some(_), false) = (&look_s, frames.is_empty()) {
                let c = |w: f32| [1.0, 1.0, 1.0, w];
                let next = frames[1 % frames.len()];
                speckle_first.push(draws.len());
                draws.push(DrawCmd::speckle(m, frames[0], c(1.0), Mat4::IDENTITY).with_cull(far));
                draws.push(
                    DrawCmd::speckle(m, next, [0.0, 1.0, 1.0, 0.0], Mat4::IDENTITY).with_cull(far),
                );
            }
        }
        if let (Some(sp), false) = (&look_s, speckle_first.is_empty()) {
            speckle = Some(SpeckleAnim {
                first: speckle_first,
                frames,
                fps: sp.fps,
                normal: false,
            });
        }
        if let (Some(n), false) = (normal_cfg, normal_first.is_empty()) {
            speckle = Some(SpeckleAnim {
                first: normal_first,
                frames: normals,
                fps: n.fps,
                normal: true,
            });
        }
        // What moves it: the bob, or the ocean's FFT patch. Built here, and
        // inert until a caller steps it — these draws stay exactly as pushed.
        let ranges = crate::water_anim::WaterRanges {
            near: scene
                .manifest
                .lighting
                .near_scene_range
                .map_or(0.0, |r| r[1]),
            far: look.far.unwrap_or(f32::INFINITY),
        };
        water_motion = Some(crate::water_anim::WaterMotion::new(
            first..draws.len(),
            &draws,
            mesh,
            info.config.as_ref(),
            ranges,
        ));
        for k in still_foam {
            draws[k].color[3] = 0.0;
        }
    }

    // Models: upload each drawable model's submeshes once, then instance them.
    // Three sources, in this order: the world's instances (`ii` is the
    // manifest index a door rig keys on), the foliage's path-block props
    // (world instances of their class, re-foliage §9) and the foliage's
    // model instances (`super::foliage`), which carry their tint and fade.
    let mut uploaded: HashMap<String, Vec<ModelPart>> = HashMap::new();
    let mut skipped: HashMap<String, usize> = HashMap::new();
    let mut doors: Vec<DoorDraw> = Vec::new();
    let mut turrets: Vec<super::turret_pose::TurretDraw> = Vec::new();
    let mut stations: Vec<super::stations::StationDraw> = Vec::new();
    let mut instance_draws: Vec<(u32, std::ops::Range<usize>)> = Vec::new();
    let wreck_models = crate::stations::wreck_models(scene)?;
    let fallen_models = crate::turret_fall::fallen_models(scene)?;
    let mut fallen: Vec<super::turret_fall::FallenDraw> = Vec::new();
    let path_props = scene
        .manifest
        .foliage
        .as_ref()
        .map_or(&[][..], |f| &f.path_props[..]);
    let mut placed: Vec<PlacedModel<'_>> = Vec::new();
    let mode = lod_mode();
    let bands = (mode != super::look::LodMode::Off)
        .then(|| lod_bands(&scene.manifest.lighting))
        .flatten();
    for (ii, inst) in scene
        .manifest
        .instances
        .iter()
        .chain(path_props)
        .enumerate()
    {
        let Some(model) = scene.resolve_model(&inst.class) else {
            *skipped.entry(inst.class.clone()).or_default() += 1;
            continue;
        };
        let world = ii < scene.manifest.instances.len();
        // Card LOD (`analysis/re-lod.md` §4, §7 [O]): on a map with a far
        // scene the PS2 draws a geometry that has a LOWD as its model to the
        // near scene's end and as its LOWD past the far scene's reach; the
        // two draws below switch whole at one distance ([`render::LodSwitch`],
        // `LodBands::at`). The PS2
        // culls the LOWD at `FarSceneRange` by the OBJECT's sphere; the port
        // gives it the map's model cull like every other draw (per fragment,
        // off by default), since a per-fragment cut at `FarSceneRange` drops
        // BF2 UTA1's far sinkhole wall, a ring whose sphere is near. A geometry with no LOWD is drawn as its
        // model at every distance — the PS2 does not draw it past
        // `NearSceneRange[2]`; the port keeps it, a named deviation (draw
        // distance, re-lod §7.2). Path props are world instances of their
        // class (re-foliage §9) and take the same rule.
        let matrix = instance_matrix(inst.rotation, inst.translation);
        // The switch's sphere: the model's, shared by both sides so the
        // instance switches whole.
        let lowd = bands.and_then(|b| {
            let (anchor, radius) = crate::point_light::model_sphere(&scene.meshes[model], matrix)
                .map_or((Vec3::from(inst.translation), 0.0), |s| {
                    (s.centre, s.radius)
                });
            let reach = super::leaf::attach_reach(&scene.manifest, &inst.class);
            Some((
                scene.resolve_lowd(&inst.class)?,
                b.sides(mode, reach, anchor, radius),
            ))
        });
        // PWST: a droid turns whole, its rig's nodes by its own animation.
        let class = world.then(|| scene.resolve_class(&inst.class)).flatten();
        // TBLD: a building turret, drawn as its fallen look while down.
        let building = (world && fallen_models.contains_key(&ii)).then_some(ii);
        let station = class
            .filter(|c| crate::stations::is_station_base(&c.base))
            .map(|c| {
                (
                    ii,
                    Vec3::from(inst.translation),
                    c.turret_rig.as_ref(),
                    false,
                )
            });
        placed.push(PlacedModel {
            model,
            matrix: instance_matrix(inst.rotation, inst.translation),
            // DRV: the submeshes this instance's door clip moves.
            door: world
                .then(|| {
                    scene
                        .resolve_class(&inst.class)
                        .and_then(|c| c.door.as_ref())
                })
                .flatten()
                .map(|d| (ii, d)),
            // VTURD: the parts this instance's turret turns.
            turret: class
                .filter(|_| station.is_none())
                .and_then(|c| c.turret_rig.as_ref())
                .map(|t| (ii, inst, t)),
            station,
            instance: world.then_some(ii),
            color: None,
            fade: None,
            near: None,
            lod: lowd.map(|(_, s)| s.0),
            building: building.map(|ii| (ii, false)),
        });
        if let Some((ii, Some(model))) = building.map(|ii| (ii, fallen_models.get(&ii))) {
            if let Some(model) = model.as_deref().and_then(|m| scene.meshes.get_key_value(m)) {
                placed.push(PlacedModel {
                    model: model.0,
                    matrix: instance_matrix(inst.rotation, inst.translation),
                    door: None,
                    turret: None,
                    station: None,
                    instance: None,
                    color: None,
                    fade: None,
                    near: None,
                    lod: None,
                    building: Some((ii, true)),
                });
            }
        }
        if let Some(model) = wreck_models.get(&ii).filter(|_| station.is_some()) {
            placed.push(PlacedModel {
                model,
                matrix: instance_matrix(inst.rotation, inst.translation),
                door: None,
                turret: None,
                station: Some((ii, Vec3::from(inst.translation), None, true)),
                instance: None,
                color: None,
                fade: None,
                near: None,
                lod: None,
                building: None,
            });
        }
        if let Some((lowd, sides)) = lowd {
            placed.push(PlacedModel {
                model: lowd,
                matrix: instance_matrix(inst.rotation, inst.translation),
                door: None,
                turret: None,
                station: station.map(|(i, at, _, dead)| (i, at, None, dead)),
                instance: world.then_some(ii),
                color: None,
                fade: None,
                near: None,
                lod: Some(sides.1),
                building: building.map(|ii| (ii, false)),
            });
        }
        // The far scene's LOWD extension (card UTAS, `analysis/re-far-lowd.md`
        // §0/§6 [O]): the LOWD triangles its LOD0 lacks — BF2 UTA1's far
        // sinkhole and canyon walls — with the fragments nearer than
        // `NearSceneRange[1]` discarded. It goes with the MODEL: drawn
        // whenever the model is (the switch's model side), and dropped once
        // the whole LOWD, which holds those triangles, takes over.
        let reach = scene.manifest.lighting.near_scene_range.map(|r| r[1]);
        if let (true, Some(far), Some(near)) = (world, scene.resolve_far(&inst.class), reach) {
            placed.push(PlacedModel {
                model: far,
                matrix: instance_matrix(inst.rotation, inst.translation),
                door: None,
                turret: None,
                station: station.map(|(i, at, _, dead)| (i, at, None, dead)),
                instance: world.then_some(ii),
                color: None,
                fade: None,
                near: Some(near),
                lod: lowd.map(|(_, s)| s.0),
                building: building.map(|ii| (ii, false)),
            });
        }
    }
    let foliage_records = super::foliage::records(scene);
    placed.extend(
        super::foliage::models(scene, &foliage_records)
            .into_iter()
            .map(|f| PlacedModel {
                model: f.model,
                matrix: f.matrix,
                door: None,
                turret: None,
                station: None,
                instance: None,
                color: Some(f.color),
                fade: f.fade,
                near: None,
                lod: None,
                building: None,
            }),
    );
    // Card BPNT: each world model's sphere, in model space, for the local
    // lights' gather (`crate::point_light::model_sphere`).
    let mut spheres: HashMap<String, Option<render::LitSphere>> = HashMap::new();
    for p in &placed {
        let model = p.model;
        // The terrain foliage's models (tinted) take none: PROVISIONAL, their
        // draw path is not the model draw's (re-foliage §6).
        // measure: matched PS2 terrain-foliage lighting with local lights before replacing the
        // no-point-light gather choice.
        let lit = match p.color {
            Some(_) => None,
            None => spheres
                .entry(model.to_owned())
                .or_insert_with(|| {
                    crate::point_light::model_sphere(&scene.meshes[model], Mat4::IDENTITY)
                })
                .map(|s| render::LitSphere {
                    centre: p.matrix.transform_point3(s.centre),
                    radius: s.radius * p.matrix.transform_vector3(Vec3::X).length(),
                }),
        };
        if !uploaded.contains_key(model) {
            let mut parts = Vec::new();
            let cpu = &scene.meshes[model];
            for (k, (mesh, mat, layer, _)) in upload_submeshes(r, cpu)?.into_iter().enumerate() {
                let tex = match mat {
                    Some(m) => get_material_texture(
                        r,
                        scene,
                        &mut textures,
                        m,
                        render::WrapV::Repeat,
                        cpu.uv_scroll(k),
                    )?,
                    None => None,
                };
                parts.push((
                    mesh,
                    tex,
                    mat.map(|m| m.to_string()),
                    layer,
                    cpu.spec_pass(k),
                    cpu.light_path(k),
                ));
            }
            uploaded.insert(model.to_owned(), parts);
        }
        let m = p.matrix;
        let far = cull.map(|c| c.world);
        let lowd = matches!(p.lod, Some(l) if l.side == render::LodSide::Lowd);
        let rig = p.door.map(|(_, d)| d);
        let mut moving = Vec::new();
        let mut turning = Vec::new();
        let whole_start = draws.len();
        for (k, (mesh, tex, mat, layer, spec, light)) in uploaded[model].iter().enumerate() {
            let first = draws.len();
            // The specular second pass rides only a textured draw: its add is
            // weighted by the base texel's alpha. A far-scene draw (`p.near`)
            // takes none: BF2's pass is the near scene's (`spec_cull`). Nor
            // does a LOWD (card LOD), drawn only in the far scene.
            if let (Some(b), Some(sp), None, false) = (tex, spec, p.near, lowd) {
                let rgb = sp.rgb_under(scene.manifest.lighting.sun_color);
                // `light` too: `fs_spec` recomputes its base draw's colour.
                draws.push(
                    DrawCmd::specular(*mesh, b.handle, rgb, sp.power, m)
                        .with_cull(spec_cull(scene, far))
                        .with_light(*light)
                        .with_fade(p.fade)
                        .with_lod(p.lod)
                        .with_lit(lit),
                );
            }
            let mut base = match tex {
                Some(b) => draw_in(*layer, *mesh, b.handle, m).with_cull(far),
                None => match census.route(model, mat.as_deref()) {
                    // Nothing the disc left untextured, and nothing whose
                    // texture is on neither disc, is drawn.
                    Untextured::DiscUntextured | Untextured::Dangling => continue,
                    Untextured::LookupFailed => {
                        DrawCmd::flat(*mesh, [0.6, 0.6, 0.6, 1.0], m).with_cull(far)
                    }
                },
            }
            .with_cull_near(p.near)
            .with_lod(p.lod)
            .with_light(*light)
            .with_fade(p.fade)
            .with_lit(lit);
            if let Some(c) = p.color {
                base.color = std::array::from_fn(|i| base.color[i] * c[i]);
            }
            if *light == render::LightPath::Glow {
                base.color[3] *= super::look::hallway_lamp_bloom(&scene.manifest, mat.as_deref());
            }
            draws.push(base);
            if let Some(part) = rig.and_then(|r| r.parts.iter().find(|q| q.submesh as usize == k)) {
                let frames = part
                    .motion
                    .iter()
                    .map(|(q, t)| (glam::Quat::from_array(*q), Vec3::from(*t)))
                    .collect();
                moving.push((first..draws.len(), frames));
            }
            if p.turret
                .is_some_and(|(_, _, t)| matches!(t.parts.get(k), Some(Some(_))))
            {
                turning.push((first..draws.len(), k));
            }
            if let Some((ii, at, rig, false)) = p.station {
                let at_k = match stations.iter().position(|s| s.instance == ii) {
                    Some(j) => j,
                    None => {
                        stations.push(super::stations::StationDraw::new(ii, at, m, rig));
                        stations.len() - 1
                    }
                };
                stations[at_k]
                    .parts
                    .push((first..draws.len(), rig.map(|_| k)));
            }
        }
        if let Some((ii, _, _, true)) = p.station {
            if let Some(s) = stations.iter_mut().find(|s| s.instance == ii) {
                s.wreck.extend(draws.drain(whole_start..));
            }
        }
        if let Some((ii, look)) = p.building {
            let at = match fallen.iter().position(|f| f.instance == ii) {
                Some(k) => k,
                None => {
                    fallen.push(super::turret_fall::FallenDraw {
                        instance: ii,
                        intact: Vec::new(),
                        fallen: Vec::new(),
                    });
                    fallen.len() - 1
                }
            };
            if look {
                fallen[at].fallen.extend(draws.drain(whole_start..));
            } else {
                fallen[at].intact.push(whole_start..draws.len());
            }
        }
        if let (false, Some((ii, inst, rig))) = (turning.is_empty(), p.turret) {
            turrets.push(super::turret_pose::TurretDraw::new(
                ii,
                inst,
                m,
                rig,
                turning,
                whole_start..draws.len(),
            ));
        }
        if let (false, Some((ii, _))) = (moving.is_empty(), p.door) {
            doors.push(DoorDraw {
                instance: ii,
                model: m,
                parts: moving,
            });
        }
        if let Some(ii) = p.instance.filter(|_| whole_start < draws.len()) {
            instance_draws.push((ii as u32, whole_start..draws.len()));
        }
    }

    let mut skipped: Vec<(String, usize)> = skipped.into_iter().collect();
    skipped.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    apply_lighting(r, scene);
    // Set on every upload, so a map without lights clears the last map's.
    r.set_point_lights(crate::point_light::still_lights(scene));
    if let Some(assets_dir) = scene.dir.parent() {
        install_spec_table(r, assets_dir, &scene.manifest.source_game)?;
        install_omniproj(r, assets_dir, &scene.manifest.source_game)?;
    }
    let (sky, dome_shells) = upload_sky(r, scene, &mut textures, &mut census)?;
    r.set_sky(sky);
    upload_atmosphere(r, scene, &mut textures)?;
    r.set_sky_particles(Vec::new());
    r.set_lightning(None);
    // Set on every upload, so a map without stars clears the last map's.
    r.set_stars(star_field(&scene.manifest), dome_shells);
    // The grass patches (queue FOLGG, `super::grass`): the FOLG records'
    // patches and the world instances of a grass class, drawn by the
    // renderer's own pass every frame. Set on every upload, so a map
    // without grass clears the last map's.
    let (grass_classes, grass_patches) = super::grass::build(scene, &foliage_records, |name| {
        Ok(get_texture(r, scene, &mut textures, name, render::WrapV::Repeat)?.map(|t| t.handle))
    })?;
    r.set_grass(grass_classes, grass_patches);
    // The leaf patches (card LEAF, `super::leaf`): the props' attached
    // clusters, the world and FOLG ones; they ride the grass pass and fade
    // by NearSceneRange (b, c). Set on every upload, as the grass.
    let (leaf_classes, leaf_objects) = super::leaf::build(scene, &foliage_records, |name| {
        Ok(get_texture(r, scene, &mut textures, name, render::WrapV::Repeat)?.map(|t| t.handle))
    })?;
    let near = scene
        .manifest
        .lighting
        .near_scene_range
        .map(|[_, b, c]| (b, c));
    r.set_leaves(leaf_classes, leaf_objects, near);
    let emitters = scene
        .manifest
        .leaf
        .iter()
        .map(|g| render::LeafEmitter {
            count: g.falling_count,
            birds: g.bird_count,
            radius: g.radius,
            height: g.height,
            scale: g.scale,
            distance: g.max_distance,
        })
        .collect();
    let falling_texture = if scene.manifest.leaf.iter().any(|g| g.falling_count > 0) {
        get_texture(r, scene, &mut textures, "leaf", render::WrapV::Repeat)?.map(|t| t.handle)
    } else {
        None
    };
    let ground = scene.heightfield().map(|g| {
        Box::new(move |x, z| g.height_at(x, z)) as Box<dyn Fn(f32, f32) -> f32 + Send + Sync>
    });
    r.set_leaf_fall(emitters, falling_texture, ground);
    if let Some(wind) = &scene.manifest.wind {
        r.set_leaf_wind(Vec3::new(wind.velocity[0], 0.0, wind.velocity[1]));
    }
    crate::forest::configure_default(r, scene)?;
    let mut untextured: Vec<(String, String, usize)> = census
        .failed
        .into_iter()
        .map(|((a, b), n)| (a, b, n))
        .collect();
    untextured.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    Ok(SceneGpu {
        draws,
        skipped,
        untextured,
        disc_untextured: census.disc_untextured,
        dangling: census.dangling,
        terrain_dropped: census.terrain_dropped,
        clear: clear_color(scene),
        sky_clamp_extra_uploads: textures
            .keys()
            .filter_map(|k| k.strip_prefix(CLAMP_V_KEY_PREFIX))
            .filter(|bare| textures.contains_key(*bare))
            .count(),
        sky_clamped: census.sky_clamped,
        water_motion,
        speckle,
        doors,
        turrets,
        stations,
        instance_draws,
        faded: Vec::new(),
        fallen,
        lights: crate::point_light::scene_lights(scene),
        ground_lights: crate::point_light::ground_lights(scene),
        post_lights: scene.manifest.source_game == "bf2",
        holo: crate::holo::HoloArt::load(r, scene),
    })
}
