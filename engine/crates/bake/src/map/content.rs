use super::{
    alpha_report, bail_no_world, class_from_entc, collision_slot, instance_entry, layer_loses,
    manifest_prims, nav_block, nav_plan_entry, pull_side_models, region_entry, unique_nav_key,
    write_coll, Content, Manifest, ManifestClass, ManifestCollision, ManifestCutter,
    ManifestGeometry, ManifestInstance, ManifestLayer, ManifestPath, ManifestRegion, ManifestWater,
    ModeLayer,
};
use anyhow::{anyhow, Context, Result};
use std::path::Path;

pub(super) fn bake_content(
    lvl_path: &str,
    outdir: &str,
    kind: Content,
    pc_lvl: Option<&str>,
) -> Result<Manifest> {
    let pc = pc_lvl
        .map(crate::tier::PcTextures::load)
        .transpose()
        .with_context(|| format!("PC override LVL for {lvl_path}"))?;
    let (buf, original) =
        crate::gameplay::read_with_original(lvl_path).with_context(|| lvl_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let out = Path::new(outdir);
    std::fs::create_dir_all(out.join("models"))?;
    std::fs::create_dir_all(out.join("textures"))?;
    std::fs::create_dir_all(out.join("collision"))?;

    let mut models: Vec<String> = Vec::new();
    // Every baked `modl`, by lowercase name, for the far extension
    // (`crate::far_lowd`) once the gmods are known.
    let mut parsed_models: std::collections::HashMap<String, mungefs::model::Model> =
        std::collections::HashMap::new();
    let mut bf2_models = 0usize;
    let mut all_textures: Vec<mungefs::texture::Texture> = Vec::new();
    // ALPHA RULE v2 (crate::alpha): what the DRAWN segments of this bake ask
    // of each texture's alpha. Replaces the old per-texture OR of the
    // material flag word, which was lossy — see the module doc there.
    let mut alpha_usage = crate::alpha::AlphaUsage::default();
    // The PRE-v2 per-texture rule, kept for ONE purpose: design D7's sky
    // carve-out (`crate::alpha::keep_alpha`). The OR of the material flag word
    // over every segment of every TOP-LEVEL `modl` in this LVL — deliberately
    // NOT including the sky-object models pulled from SIDE LVLs, because the
    // old rule did not either, and D7 has to reproduce it exactly.
    let mut tex_flags: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    // The same OR over the models a BF2 `lvl_` MODE LAYER contributes (design
    // D1). Design D3 folds it into the D7 answer — a layer draw is a draw —
    // but keeps it apart so the bake can say when doing so would have moved a
    // sky texture's alpha away from what D7 reproduced.
    let mut layer_tex_flags: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    // Queue DSHN: the sheets a specular-pass segment of any model here draws
    // (`crate::spec::pass_textures`) keep their disc alpha — the pass's gloss
    // weight (the BF1/BF2 KAM1 decks, DEA1's rooms, common's probe droid…).
    let mut pass_named: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Card FXMF: the muzzle flash model's sheet keeps its disc alpha — it is
    // the flame's whole shape (a white CLUT, mean alpha 38/255), and the
    // flash draws it ADDITIVELY by that alpha (`analysis/re-muzzle-flash.md`
    // §4.3; BF1 MTRL 0x84 and BF2 0x10044 would otherwise be rule 2's
    // OPAQUE).
    let mut flash_named: std::collections::HashSet<String> = std::collections::HashSet::new();
    // A `hud_*` mesh's sheets keep their disc alpha: the HUD composites its own
    // art by that alpha whatever the material says (`analysis/re-hud-icons.md`
    // §2; `game::hud_feed::load_mesh`). `hud_herobar_saberhilt` (0x10001) only
    // kept its shape through a census MASK, which BF2's ELF rule now overrules.
    let mut hud_named: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Contact-surface fallbacks ignore alpha for solid coverage, but retain
    // every native texture alpha byte for any other consumer of their art.
    let mut contact_named: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut contact_models: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut textures = Vec::new();
    let mut transparent: Vec<String> = Vec::new();
    // One entry per texture that HAS a classic-plus override: which source
    // filled tier 2, and the file it went to (`<name>.tier2.ktx2`, beside the
    // PS2 base). See `crate::tier`.
    let mut texture_tiers: Vec<(String, String)> = Vec::new();
    let mut texture_tier2: Vec<(String, String)> = Vec::new();
    let mut classes: Vec<ManifestClass> = Vec::new();
    let mut geometries: Vec<ManifestGeometry> = Vec::new();
    // Worlds: unwrapped (base + always-on design/sound/collision layers) and
    // lvl_-wrapped per-mode layers (BF2), grouped by wrapper.
    let mut worlds: Vec<mungefs::world::World> = Vec::new();
    let mut mode_layers: Vec<ModeLayer> = Vec::new();
    // The base world's AI graph: the LAST top-level `plan`, with the `plnp`
    // that follows it. BF2 frees the previous graph on every 'plan' chunk, so
    // last-loaded wins (KAM1 and MUS1 ship three top-level chunks each);
    // BF1 ships exactly one.
    let mut base_plan: Option<mungefs::plan::Plan> = None;
    let mut base_plnp: Option<mungefs::plan::Plnp> = None;
    let mut terrain_glb = None;
    let mut terrain_query_size = None;
    let mut terrain_render_heights = None;
    let mut terrain_parsed: Option<mungefs::terrain::Terrain> = None;
    // The far-LOD sheet is written AFTER the walk, not here: its texture and
    // its UV scale come from `sky_`, and `tern` precedes `sky_` in every one
    // of the 32 shipped LVLs (re-lowr.md §4).
    let mut lowres: Option<(String, mungefs::terrain::LowResSheet, f32)> = None;
    let mut water: Option<mungefs::water::Water> = None;
    let mut water_cfg: Option<mungefs::fx::WaterConfig> = None;
    let mut paths = Vec::new();
    let mut sky_cfg: Option<mungefs::sky::Sky> = None;
    let mut lights_cfg: Option<mungefs::light::Lights> = None;
    // Collision, merged by LOWERCASE geometry name. The Vec keeps first-seen
    // chunk order (the sim visits bodies in manifest order and must not depend
    // on a hash); the map is a lookup index only, never iterated.
    let mut collision: Vec<ManifestCollision> = Vec::new();
    let mut collision_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut terrain_cutters: Vec<ManifestCutter> = Vec::new();
    // The terrain foliage (`crate::foliage`): the map's `prp_` file and its
    // terrain's FOLG layer mask, placed once the game is known.
    let mut foliage_file: Option<mungefs::foliage::FoliageFile> = None;
    let mut folg_mask: Option<mungefs::foliage::FolgMask> = None;
    let mut grass_props: Vec<(String, Vec<(u32, String)>)> = Vec::new();
    // Card LEAF (`crate::leafpatch`): every `leafpatch` class's PROPs, and
    // every class's attach PROPs in walk order for the attach replay.
    let mut leaf_props: Vec<(String, Vec<(u32, String)>)> = Vec::new();
    // Card BPNT (`crate::entity_light`): every `Light` class, and the
    // authored omnis and spots of every non-stub `lght`.
    let mut entity_lights: Vec<crate::entity_light::LightClass> = Vec::new();
    let mut point_lights: Vec<manifest::ManifestPointLight> = Vec::new();
    let mut attach_props: Vec<(String, Vec<(u32, String)>)> = Vec::new();

    // PRE-EXPANSION (design D1). BF2 map LVLs wrap each game mode's layer in a
    // `lvl_` whose single child is a hashed sub-level container (A2 §1), and
    // that container holds not only the layer's `wrld`/`plan`/`plnp` but the
    // layer's OWN content: its `entc`, `gmod`, `modl`, `skel`, `coll`, `prim`,
    // `tex_`, `expc`. Walking only the top level therefore missed every prop a
    // mode layer introduces — the Hoth bridges, Polis Massa's 15 classes,
    // Coruscant's 11 — so the manifest had instances naming classes that had no
    // row, no GLB and no collision. The walk below runs over the top-level
    // chunks in file order followed by every wrapper's content in wrapper
    // order, which is exactly the flattening `mungefs::content_chunks` does for
    // the SIDE bake, with the layer bookkeeping kept.
    //
    // A layer's `wrld`/`plan`/`plnp` are consumed HERE, into `ModeLayer`, and
    // never reach the walk (a layer `plan` must not become the base graph). A
    // layer `lght` is dropped: BF2 ships one per sublevel, only the first
    // carries content, and the walk's `lght` arm has already taken it from the
    // top level.
    let side_source = match kind {
        Content::Map => crate::side_classes::side_classes_lvl(lvl_path)
            .map(|p| {
                crate::gameplay::read_with_original(&p).with_context(|| p.display().to_string())
            })
            .transpose()?,
        Content::Common => None,
    };
    let side_buf = side_source.as_ref().map(|(bytes, _)| bytes.as_slice());
    let mut expanded: Vec<mungefs::Chunk<'_>> = Vec::new();
    let mut layer_chunks: Vec<mungefs::Chunk<'_>> = Vec::new();
    let mut wrappers = 0usize;
    for c in mungefs::chunks(inner) {
        let c = c.map_err(|e| anyhow!("{e}"))?;
        if &c.tag != b"lvl_" {
            expanded.push(c);
            continue;
        }
        wrappers += 1;
        // Sub-level wrappers carry hashed tags; a hash can happen to be
        // printable ASCII (TAT2's hunt layer = 0x2A635340 "@Sc*"), so never
        // use tag_is_ascii here — just look at what is inside.
        for sub in c.children().flatten() {
            if &sub.tag == b"wrld" {
                // Content directly under `lvl_` rather than under a hashed
                // container: an unwrapped world, handled by the walk's own arm
                // (no map on either disc ships one, but the shape is legal).
                layer_chunks.push(sub);
                continue;
            }
            let mut group = Vec::new();
            let mut plan = None;
            let mut plnp = None;
            for item in sub.children().flatten() {
                match &item.tag {
                    b"wrld" => group.push(
                        mungefs::world::World::parse(&item)
                            .map_err(|e| anyhow!("wrld 0x{:X}: {e}", item.offset))?,
                    ),
                    b"plan" => {
                        plan = Some(
                            mungefs::plan::Plan::parse(&item)
                                .map_err(|e| anyhow!("plan 0x{:X}: {e}", item.offset))?,
                        );
                        plnp = None;
                    }
                    b"plnp" => {
                        if plan.is_some() {
                            plnp = Some(
                                mungefs::plan::Plnp::parse(&item)
                                    .map_err(|e| anyhow!("plnp 0x{:X}: {e}", item.offset))?,
                            );
                        }
                    }
                    b"lght" => {}
                    _ => layer_chunks.push(item),
                }
            }
            if !group.is_empty() {
                mode_layers.push(ModeLayer { group, plan, plnp });
            }
        }
    }
    // Card VTUR2: the building-turret classes a world places from BF2's
    // SIDE/TUR.LVL join the walk as layer content (losing every by-name
    // contest to the map's own), so their models, collision, skel nodes and
    // manifest rows bake like the map's. Their PROPs go to the side-classes
    // file `bf2_tur.ron` (`bake classes`), not to this map's classes.ron.
    if let Some(side) = side_buf {
        let mut placed = std::collections::BTreeSet::new();
        let mut defined = std::collections::HashSet::new();
        for c in expanded.iter().chain(&layer_chunks) {
            match &c.tag {
                b"wrld" => {
                    if let Ok(w) = mungefs::world::World::parse(c) {
                        placed.extend(w.instances.iter().map(|i| i.class.to_ascii_lowercase()));
                    }
                }
                b"entc" => {
                    if let Ok(e) = mungefs::entity::EntityClass::parse(c) {
                        defined.insert(e.name.to_ascii_lowercase());
                    }
                }
                _ => {}
            }
        }
        for w in mode_layers.iter().flat_map(|l| &l.group) {
            placed.extend(w.instances.iter().map(|i| i.class.to_ascii_lowercase()));
        }
        let (pulled, taken) = crate::side_classes::pull_side_classes(side, &placed, &defined)?;
        if !taken.is_empty() {
            eprintln!(
                "side classes: {} chunk(s) from SIDE/TUR.LVL for {}",
                pulled.len(),
                taken.join(" ")
            );
        }
        layer_chunks.extend(pulled);
    }
    let top_level = expanded.len();
    expanded.extend(layer_chunks);
    let shared_render = if original.is_some() && kind == Content::Map {
        super::render_recovery::shared(lvl_path, &expanded)?
    } else {
        Vec::new()
    };
    expanded.extend(mungefs::content_chunks(&shared_render).map_err(|e| anyhow!("{e}"))?);
    if expanded.len() > top_level {
        eprintln!(
            "lvl_ layers: {wrappers} wrapper(s) contributed {} content chunk(s)",
            expanded.len() - top_level
        );
    }
    // Everything at index >= `top_level` came out of a mode layer, and loses
    // every by-name contest against content the top level already baked
    // (design D2: first wins, and the top level goes first).
    let from_layer = |i: usize| i >= top_level;
    // Per-tag counts of layer content that lost such a contest, for the report.
    let mut layer_skips: std::collections::BTreeMap<&'static str, usize> = Default::default();

    // Textures FIRST, in their own pass: ALPHA RULE v2 judges each drawn
    // segment against the alpha of the texture it names, so every `tex_` must
    // be decoded before the first `modl` is written. Detail companions are
    // paired with their base after the walk (crate::detail), so nothing is
    // written here.
    for (i, c) in expanded.iter().enumerate() {
        if &c.tag != b"tex_" {
            continue;
        }
        match mungefs::texture::Texture::parse(c) {
            Ok(t) => {
                let dup = from_layer(i)
                    && all_textures
                        .iter()
                        .any(|e| e.name.eq_ignore_ascii_case(&t.name));
                if layer_loses("tex_", c.offset, &t.name, dup, &mut layer_skips) {
                    continue;
                }
                all_textures.push(t);
            }
            Err(e) => eprintln!("skipping tex_ 0x{:X}: {e}", c.offset),
        }
    }
    let alpha_table = crate::alpha::AlphaTable::new(&all_textures);
    // Card TBIND (`crate::bind_pose`): a model with a `skel` is written in
    // its bind pose, so a turret's head and gun sit on their bones.
    let shadow_skels = crate::hardpoints::Skeletons::of(&expanded);
    let mut bind_pose = crate::bind_pose::BindPose::of(&expanded);
    let render_alignment = super::render_alignment::Alignment::of(original.as_deref())?;
    let render_conformation = super::render_conform::Conformation::of(original.as_deref())?;
    let render_entrance = super::render_entrance::Entrance::of(original.as_deref())?;
    for original in original.as_deref().into_iter().chain(
        side_source
            .as_ref()
            .and_then(|(_, source)| source.as_deref()),
    ) {
        bind_pose
            .add_render_source(original)
            .map_err(|e| anyhow!(e))?;
    }
    let mut posed: Vec<String> = Vec::new();
    let restore_door_halves = super::render_symmetry::enabled(original.as_deref())?;
    let correct_column = super::render_column::enabled(original.as_deref())?;
    let render_faces = super::render_faces::Faces::of(original.as_deref())?;
    let mut reflected_door_groups = std::collections::HashMap::new();
    // Card VTURD (`crate::turret_rig`): an armed building's model is written
    // as one primitive per (segment, node), so its gun can turn.
    let turret_rigs = crate::turret_rig::TurretRigs::of(&expanded);
    // Card FXP1 (`crate::effects`): this layer's `fx__` particle effects,
    // collected before the texture write because the sheets they name are
    // drawn blended by their alpha (re-fx §6.7) and so keep it.
    let fx = crate::effects::Collected::from_chunks(expanded.iter())?;
    let fx_named = fx.textures();
    // Card GLOW: BF2's `Effect("HDR")`, read before the models because it
    // decides how their self-lit segments are written (`crate::glb::
    // glow_path`); the first `fx__` that carries one. Only an enabled one
    // is kept — a disabled HDR draws nothing differently.
    let mut hdr = None;
    for c in expanded.iter().filter(|c| &c.tag == b"fx__") {
        if let Some(h) = mungefs::fx::HdrConfig::parse(c)
            .map_err(|e| anyhow!("fx__ 0x{:X} Effect(\"HDR\"): {e}", c.offset))?
        {
            hdr = Some(h);
            break;
        }
    }
    let hdr = hdr.filter(|h| h.enable && kind == Content::Map);
    if let Some(h) = hdr {
        eprintln!(
            "hdr: on ({} bloom passes, weight {}, threshold {})",
            h.num_bloom_passes, h.max_total_weight, h.glow_threshold
        );
    }
    // Card KSKY: `Effect("Lightning")`, the first `fx__` that carries one.
    // Only an enabled one is kept (BF1 KAM1, BF1 YAV2, BF2 KAM1) — a disabled
    // effect has no tick and darkens nothing, so the other maps' manifests
    // do not move. Its `LightningBolt` is parsed by `mungefs::fx` and not
    // carried: nothing draws a bolt yet (card WTHL).
    let mut lightning = None;
    for c in expanded.iter().filter(|c| &c.tag == b"fx__") {
        if let Some(l) = mungefs::fx::LightningConfig::parse(c)
            .map_err(|e| anyhow!("fx__ 0x{:X} Effect(\"Lightning\"): {e}", c.offset))?
        {
            lightning = Some(l);
            break;
        }
    }
    let lightning = lightning
        .filter(|l| l.enable && kind == Content::Map)
        .map(|l| {
            eprintln!(
                "lightning: on (sky dome darken {}, dome byte {:?})",
                l.sky_dome_darken_factor,
                l.resting_dome_colour()
            );
            manifest::ManifestLightning {
                sky_dome_darken_factor: l.sky_dome_darken_factor,
                sunlight_fade_factor: l.sunlight_fade_factor,
                color: l.color,
                brightness_min: l.brightness_min,
                fade_time: l.fade_time,
                time_between_flashes: l.time_between_flashes,
                time_between_sub_flashes: l.time_between_sub_flashes,
                num_sub_flashes: l.num_sub_flashes,
                horizon_angle: l.horizon_angle,
                sound_crack: l.sound_crack,
                sound_sub_crack: l.sound_sub_crack,
            }
        });
    // Card WTHR: `Effect("Precipitation")` and `Effect("Wind")`, the first
    // `fx__` that carries each; kept only when enabled (re-weather.md §1:
    // nine maps and four), so every other map's manifest does not move.
    let mut precipitation = None;
    let mut wind = None;
    let mut fog_cloud = None;
    for c in expanded.iter().filter(|c| &c.tag == b"fx__") {
        if precipitation.is_none() {
            precipitation = mungefs::fx::PrecipitationConfig::parse(c)
                .map_err(|e| anyhow!("fx__ 0x{:X} Effect(\"Precipitation\"): {e}", c.offset))?;
        }
        if wind.is_none() {
            wind = mungefs::fx::WindConfig::parse(c)
                .map_err(|e| anyhow!("fx__ 0x{:X} Effect(\"Wind\"): {e}", c.offset))?;
        }
        if fog_cloud.is_none() {
            fog_cloud = mungefs::fx::FogCloudConfig::parse(c)
                .map_err(|e| anyhow!("fx__ 0x{:X} Effect(\"FogCloud\"): {e}", c.offset))?;
        }
    }
    let precipitation = precipitation
        .filter(|p| p.enable && kind == Content::Map)
        .map(|p| {
            eprintln!(
                "precipitation: on ({}, {} particles, range {} m)",
                if p.streaks { "streaks" } else { "quads" },
                p.particle_density,
                p.range
            );
            manifest::ManifestPrecipitation {
                streaks: p.streaks,
                texture: p.texture,
                particle_size: p.particle_size,
                range: p.range,
                color: p.color,
                alpha_min_max: p.alpha_min_max,
                particle_density: p.particle_density,
                particle_density_range: p.particle_density_range,
                velocity: p.velocity,
                velocity_range: p.velocity_range,
                streak_length: p.streak_length,
                camera_cross_velocity_scale: p.camera_cross_velocity_scale,
                camera_axial_velocity_scale: p.camera_axial_velocity_scale,
                rotation: p.rotation,
                rotation_range: p.rotation_range,
                ground_effect: p.ground_effect,
                ground_effects_per_sec: p.ground_effects_per_sec,
                ground_effect_spread: p.ground_effect_spread,
            }
        });
    // A flake's sheet is drawn blended by its alpha (re-weather.md §3.4:
    // MODULATE with texture alpha), so it keeps it.
    let precip_named: Option<String> = precipitation
        .as_ref()
        .and_then(|p| p.texture.as_ref().map(|t| t.to_ascii_lowercase()));
    let wind = wind
        .filter(|w| w.enable && kind == Content::Map)
        .map(|w| manifest::ManifestWind {
            velocity: w.velocity,
            velocity_range: w.velocity_range,
            velocity_change_rate: w.velocity_change_rate,
        });
    let fog_cloud = fog_cloud
        .filter(|f| f.enable && kind == Content::Map)
        .map(|f| manifest::ManifestFogCloud {
            texture: f.texture,
            range: f.range,
            color: f.color,
            velocity: f.velocity,
            rotation: f.rotation,
            height: f.height,
            particle_size: f.particle_size,
            particle_density: f.particle_density,
        });
    let fog_cloud_named = fog_cloud
        .as_ref()
        .and_then(|f| f.texture.as_ref())
        .map(|s| s.to_ascii_lowercase());
    // The sheets a self-lit segment draws keep their alpha: the glow mask.
    let mut glow_named: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, c) in expanded.iter().enumerate() {
        match &c.tag {
            b"sky_" if sky_cfg.is_none() => {
                sky_cfg = Some(
                    mungefs::sky::Sky::parse(c)
                        .map_err(|e| anyhow!("sky_ 0x{:X}: {e}", c.offset))?,
                );
            }
            // The water's real config: a map carries 3-9 `fx__` chunks and
            // at most one of them holds the `Effect("Water")` (only the 12
            // water maps hold any), so this scans until one parses.
            b"fx__" if water_cfg.is_none() => {
                water_cfg = mungefs::fx::WaterConfig::parse(c)
                    .map_err(|e| anyhow!("fx__ 0x{:X} Effect(\"Water\"): {e}", c.offset))?;
            }
            // BF2 ships one `lght` per mode sublevel and only the first
            // carries content; the stubs are 12 bytes (NAME only).
            b"lght" if c.payload.len() > 12 => {
                let l = mungefs::light::Lights::parse(c)
                    .map_err(|e| anyhow!("lght 0x{:X}: {e}", c.offset))?;
                // Card BPNT: every non-stub chunk's omnis and spots are
                // local lights (DEA1, FEL1 and UTA1 ship three chunks).
                point_lights.extend(crate::lighting::point_lights_from(&l));
                if lights_cfg.is_none() {
                    lights_cfg = Some(l);
                }
            }
            b"tex_" => {}
            b"modl" => {
                let mut m = mungefs::model::Model::parse(c)
                    .map_err(|e| anyhow!("modl 0x{:X}: {e}", c.offset))?;
                let dup = from_layer(i) && models.iter().any(|n| n.eq_ignore_ascii_case(&m.name));
                if layer_loses("modl", c.offset, &m.name, dup, &mut layer_skips) {
                    continue;
                }
                if crate::shadow::write_model(c, out, &shadow_skels)? > 0 {
                    render_alignment.shadow(&m.name, out)?;
                }
                match bind_pose.pose(&m) {
                    Ok(Some(p)) => {
                        posed.push(p.name.clone());
                        m = p;
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("bind pose: {e} — {} stays as stored", m.name),
                }
                render_alignment.apply(&mut m)?;
                if restore_door_halves {
                    if let Some(count) = super::render_symmetry::model(&mut m)? {
                        reflected_door_groups.insert(m.name.to_ascii_lowercase(), count);
                    }
                }
                if correct_column {
                    super::render_column::model(&mut m)?;
                }
                render_faces.apply(&mut m)?;
                let entrance_contact = render_entrance.apply(&mut m, &expanded)?;
                let original_alpha = alpha_table.model(&m);
                let contact_surface =
                    render_conformation.apply(&mut m, &expanded, &original_alpha)?
                        || entrance_contact;
                if contact_surface {
                    anyhow::ensure!(kind == Content::Map, "contact surfaces require a map bake");
                    if m.is_bf2() {
                        // Leaf-card fallbacks retain their native alpha for
                        // other consumers. The guarded BF1 donor was already
                        // opaque: preserve its existing shared-sheet policy.
                        contact_named.extend(
                            m.segments
                                .iter()
                                .filter_map(crate::alpha::segment_texture)
                                .map(str::to_ascii_lowercase),
                        );
                    }
                }
                let keep_contact_visible = contact_surface
                    || render_alignment.contact_surface(&m.name)
                    || render_faces.contact_surface(&m.name)
                    || (correct_column && m.name.eq_ignore_ascii_case("uta1_prop_col"));
                anyhow::ensure!(
                    !keep_contact_visible || kind == Content::Map,
                    "contact visibility requires a map bake"
                );
                if keep_contact_visible {
                    contact_models.insert(m.name.to_ascii_lowercase());
                }
                if turret_rigs.has_model(&m.name) {
                    bind_pose
                        .canonical_model_nodes(&mut m)
                        .map_err(|e| anyhow!(e))?;
                }
                match turret_rigs.split(&m) {
                    Ok(Some(p)) => m = p,
                    Ok(None) => {}
                    Err(e) => eprintln!("turret rig: {e} — written whole"),
                }
                let flags = if from_layer(i) {
                    &mut layer_tex_flags
                } else {
                    &mut tex_flags
                };
                for seg in &m.segments {
                    for (_, t) in &seg.textures {
                        *flags.entry(t.to_ascii_lowercase()).or_default() |= seg.material_flags;
                    }
                }
                pass_named.extend(crate::spec::pass_textures([&m]));
                flash_named.extend(crate::alpha::flash_textures(&m));
                if m.name.to_ascii_lowercase().starts_with("hud_") {
                    hud_named.extend(
                        m.segments
                            .iter()
                            .filter_map(crate::alpha::segment_texture)
                            .map(str::to_ascii_lowercase),
                    );
                }
                glow_named.extend(crate::alpha::glow_textures(&m, hdr.is_some()));
                let alphas = alpha_table.model(&m);
                alpha_usage.record_model(&m, &alphas);
                let glb = if keep_contact_visible {
                    crate::glb::world_contact_surface_to_glb(&m, &alphas, hdr.is_some())?
                } else if kind == Content::Map {
                    crate::glb::world_model_to_glb(&m, &alphas, hdr.is_some())?
                } else if kind == Content::Common && turret_rigs.station_model(&m.name) {
                    crate::glb::station_model_to_glb(&m, &alphas)?
                } else if m.name.to_ascii_lowercase().starts_with("hud_") {
                    // A HUD mesh keeps its vertex colours (card BHUD2).
                    crate::glb::hud_model_to_glb(&m, &alphas)?
                } else {
                    crate::glb::model_to_glb(&m, &alphas, false)?
                };
                std::fs::write(out.join("models").join(format!("{}.glb", m.name)), glb)?;
                if m.is_bf2() {
                    bf2_models += 1;
                }
                models.push(m.name.clone());
                parsed_models.insert(m.name.to_ascii_lowercase(), m);
            }
            b"tern" => {
                // WATR first, and independently of the heightfield: BF2 DEA1
                // and BF2 KAM1 carry a STUB tern whose only content is the
                // water, so it is the one ground plane those maps have.
                if water.is_none() {
                    water = mungefs::water::Water::parse(c)
                        .map_err(|e| anyhow!("tern 0x{:X} WATR: {e}", c.offset))?;
                }
                if mungefs::terrain::Terrain::is_stub(c) {
                    // BF2 model-built maps (COR1/DEA1/MYG1/TAN1/UTA1) carry a
                    // NAME-only tern: no heightfield to bake.
                    eprintln!("tern 0x{:X}: stub (no heightfield)", c.offset);
                } else {
                    let mut t = mungefs::terrain::Terrain::parse(c)
                        .map_err(|e| anyhow!("tern 0x{:X}: {e}", c.offset))?;
                    terrain_query_size = crate::gameplay::apply_terrain(lvl_path, &mut t)?;
                    terrain_render_heights = crate::gameplay::apply_render_heights(lvl_path, &t)?;
                    if terrain_cutters.is_empty() {
                        terrain_cutters = t
                            .cutters
                            .iter()
                            .map(|k| ManifestCutter {
                                sphere: k.sphere,
                                planes: k.planes.clone(),
                            })
                            .collect();
                    }
                    if let Some(sheet) = t.lowres.take() {
                        let half = t.info.grid_unit * t.info.grid_size as f32 / 2.0;
                        lowres = Some((t.name.clone(), sheet, half));
                    }
                    // Written after the loop: its vertex colour is per game
                    // (`terrain_glb::gs_vertex_byte`), and the game is known
                    // only once every model has been seen.
                    terrain_glb = Some(format!("terrain_{}.glb", t.name));
                    // At the payload's own size here; `crate::foliage`
                    // checks it against the game's grid.
                    folg_mask = mungefs::foliage::FolgMask::payload_dim(c)
                        .and_then(|d| mungefs::foliage::FolgMask::parse(c, d));
                    terrain_parsed = Some(t);
                }
            }
            b"coll" => {
                let m = mungefs::coll::CollisionMesh::parse(c)
                    .map_err(|e| anyhow!("coll 0x{:X}: {e}", c.offset))?;
                let slot = collision_slot(&m.name, &mut collision, &mut collision_index);
                if collision[slot].glb.is_some() {
                    eprintln!(
                        "coll 0x{:X}: duplicate collision mesh for {} — first occurrence wins",
                        c.offset, m.name
                    );
                    continue;
                }
                collision[slot].node = m.node.clone();
                collision[slot].glb = write_coll(&m, out, c.offset)?;
            }
            b"prim" => {
                let p = mungefs::coll::Primitives::parse(c)
                    .map_err(|e| anyhow!("prim 0x{:X}: {e}", c.offset))?;
                let slot = collision_slot(&p.model, &mut collision, &mut collision_index);
                if !collision[slot].prims.is_empty() {
                    eprintln!(
                        "prim 0x{:X}: duplicate primitives for {} — first occurrence wins",
                        c.offset, p.model
                    );
                    continue;
                }
                let (mut prims, unknown) = manifest_prims(&p);
                // STDOR / RKHULT: door and map-local vehicle prims rest at bind.
                bind_pose.rest_prims(&p.model, &mut prims);
                collision[slot].prims = prims;
                if unknown > 0 {
                    eprintln!(
                        "prim 0x{:X}: {} dropped {unknown} primitive(s) of an undecoded kind",
                        c.offset, p.model
                    );
                }
            }
            b"entc" => {
                let e = mungefs::entity::EntityClass::parse(c)
                    .map_err(|e| anyhow!("entc 0x{:X}: {e}", c.offset))?;
                // `resolve_class` takes the FIRST row of a name anyway, so a
                // layer class that repeats a top-level one is dropped rather
                // than left as an unreachable duplicate.
                let dup =
                    from_layer(i) && classes.iter().any(|k| k.name.eq_ignore_ascii_case(&e.name));
                if layer_loses("entc", c.offset, &e.name, dup, &mut layer_skips) {
                    continue;
                }
                // A grass patch class keeps its PROPs for the foliage bake
                // (`crate::foliage::grass_class`); BF2 FEL1 spells the BASE
                // `Grasspatch`.
                if e.base.eq_ignore_ascii_case("grasspatch") {
                    grass_props.push((
                        e.name.clone(),
                        e.props
                            .iter()
                            .map(|p| (p.key_hash, p.value.clone()))
                            .collect(),
                    ));
                }
                if crate::entity_light::is_light(&e.base) {
                    let p: Vec<(u32, String)> = e
                        .props
                        .iter()
                        .filter(|p| crate::entity_light::is_light_prop(p.key_hash))
                        .map(|p| (p.key_hash, p.value.clone()))
                        .collect();
                    entity_lights.push(crate::entity_light::light_class(&e.name, &p));
                }
                if crate::leafpatch::is_leafpatch(&e.base) {
                    leaf_props.push((
                        e.name.clone(),
                        e.props
                            .iter()
                            .map(|p| (p.key_hash, p.value.clone()))
                            .collect(),
                    ));
                }
                attach_props.push((
                    e.name.clone(),
                    e.props
                        .iter()
                        .filter(|p| crate::attached_effects::is_prop(p.key_hash))
                        .map(|p| (p.key_hash, p.value.clone()))
                        .collect(),
                ));
                classes.push(class_from_entc(&e));
            }
            b"gmod" => {
                let g = mungefs::entity::GeomModel::parse(c)
                    .map_err(|e| anyhow!("gmod 0x{:X}: {e}", c.offset))?;
                // Keyed by `name`: that is what `Scene::resolve_model` looks a
                // class's GeometryName up by, so first-by-name is exactly the
                // row the runtime would have used.
                let dup = from_layer(i)
                    && geometries
                        .iter()
                        .any(|k| k.name.eq_ignore_ascii_case(&g.name));
                if layer_loses("gmod", c.offset, &g.name, dup, &mut layer_skips) {
                    continue;
                }
                let Some(best) = g.best_model() else {
                    continue; // no LOD entries — nothing drawable
                };
                geometries.push(ManifestGeometry {
                    model: best.to_owned(),
                    lods: g
                        .lods
                        .iter()
                        .map(|l| (l.tier.clone(), l.model.clone()))
                        .collect(),
                    name: g.name,
                    far: None,
                    hardpoints: Vec::new(),
                });
            }
            b"prp_" if foliage_file.is_none() => {
                foliage_file = Some(
                    mungefs::foliage::FoliageFile::parse(c)
                        .map_err(|e| anyhow!("prp_ 0x{:X}: {e}", c.offset))?,
                );
            }
            b"PATH" => {
                for p in mungefs::path::parse_all(c)
                    .map_err(|e| anyhow!("PATH 0x{:X}: {e}", c.offset))?
                {
                    paths.push(ManifestPath {
                        name: p.name,
                        nodes: p.nodes.iter().map(|n| (n.pos, n.rot)).collect(),
                        props: p.props,
                    });
                }
            }
            b"path" => {
                // BF2: top-level config-format path files.
                if mungefs::path::is_config_path(c) {
                    for p in mungefs::path::parse_config_paths(c)
                        .map_err(|e| anyhow!("path 0x{:X}: {e}", c.offset))?
                    {
                        paths.push(ManifestPath {
                            name: p.name,
                            nodes: p.nodes.iter().map(|n| (n.pos, n.rot)).collect(),
                            props: p.props,
                        });
                    }
                }
            }
            b"plan" => {
                base_plan = Some(
                    mungefs::plan::Plan::parse(c)
                        .map_err(|e| anyhow!("plan 0x{:X}: {e}", c.offset))?,
                );
                base_plnp = None; // a plnp belongs to the plan BEFORE it
            }
            b"plnp" if base_plan.is_some() => {
                base_plnp = Some(
                    mungefs::plan::Plnp::parse(c)
                        .map_err(|e| anyhow!("plnp 0x{:X}: {e}", c.offset))?,
                );
            }
            b"wrld" => {
                worlds.push(
                    mungefs::world::World::parse(c)
                        .map_err(|e| anyhow!("wrld 0x{:X}: {e}", c.offset))?,
                );
            }
            _ => {}
        }
    }
    if !posed.is_empty() {
        eprintln!(
            "bind pose: {} model(s) posed ({})",
            posed.len(),
            posed.join(" ")
        );
    }
    if kind == Content::Map {
        if let Some(original) = original.as_deref() {
            let render_worlds: Vec<_> = worlds
                .iter()
                .chain(mode_layers.iter().flat_map(|layer| &layer.group))
                .collect();
            for assembled in
                super::render_reassembly::models(original, &render_worlds, &parsed_models)?
            {
                let model = assembled.model;
                let alphas = alpha_table.model(&model);
                alpha_usage.record_model(&model, &alphas);
                let glb = crate::glb::world_model_to_glb_programs(
                    &model,
                    &alphas,
                    hdr.is_some(),
                    &assembled.lighting,
                )?;
                std::fs::write(out.join("models").join(format!("{}.glb", model.name)), glb)?;
                parsed_models.insert(model.name.to_ascii_lowercase(), model);
            }
            for model in
                super::render_recovery::pieces(original, &expanded, &worlds, &parsed_models)?
            {
                let alphas = alpha_table.model(&model);
                alpha_usage.record_model(&model, &alphas);
                // Recovery can trim a contributor and rewrite its GLB after
                // the model loop. Retain its reviewed visibility policy.
                let glb = if contact_models.contains(&model.name.to_ascii_lowercase()) {
                    crate::glb::world_contact_surface_to_glb(&model, &alphas, hdr.is_some())?
                } else {
                    crate::glb::world_model_to_glb(&model, &alphas, hdr.is_some())?
                };
                std::fs::write(out.join("models").join(format!("{}.glb", model.name)), glb)?;
                if !models
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&model.name))
                {
                    models.push(model.name.clone());
                }
                parsed_models.insert(model.name.to_ascii_lowercase(), model);
            }
        }
    }
    // The far scene's LOWD extension (`crate::far_lowd`, card UTAS): what a
    // geometry's LOWD draws beyond its LOD0, as one more model.
    for g in &mut geometries {
        let tier = |t: &str| {
            g.lods
                .iter()
                .find(|(k, _)| k == t)
                .and_then(|(_, m)| parsed_models.get(&m.to_ascii_lowercase()))
        };
        let (Some(lod0), Some(lowd)) = (tier("LOD0"), tier("LOWD")) else {
            continue;
        };
        if let Some(ext) = crate::far_lowd::extension(lod0, lowd) {
            let alphas = alpha_table.model(&ext);
            let glb = if kind == Content::Map {
                crate::glb::world_model_to_glb(&ext, &alphas, hdr.is_some())?
            } else {
                crate::glb::model_to_glb(&ext, &alphas, false)?
            };
            std::fs::write(out.join("models").join(format!("{}.glb", ext.name)), glb)?;
            eprintln!(
                "far_lowd: {} draws {} of {}'s triangles past NearSceneRange[1] as {}",
                g.name, ext.face_count, lowd.name, ext.name
            );
            models.push(ext.name.clone());
            g.far = Some(ext.name);
        }
    }
    if !layer_skips.is_empty() {
        eprintln!(
            "lvl_ layers: {} chunk(s) skipped for a name the top level already baked ({})",
            layer_skips.values().sum::<usize>(),
            layer_skips
                .iter()
                .map(|(t, n)| format!("{t} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // BF2 doors (queue DR2): each `door`-based class gets its rig — the
    // trigger spheres and the `open` clip's motion per prim (crate::door).
    for (name, rig) in crate::door::rigs(&expanded) {
        match rig {
            Ok(mut d) => {
                if let Some(k) = classes
                    .iter_mut()
                    .find(|k| k.name.eq_ignore_ascii_case(&name))
                {
                    if let Some(count) = k.geometry.as_ref().and_then(|geometry| {
                        reflected_door_groups.get(&geometry.to_ascii_lowercase())
                    }) {
                        super::render_symmetry::door_parts(&mut d, *count)?;
                    }
                    k.door = Some(d);
                }
            }
            Err(e) => eprintln!("door {name}: no rig ({e}) — it stays shut"),
        }
    }

    // Card VTUR: a vehicle's or armed building's skel nodes (fire points).
    crate::hardpoints::attach(&expanded, classes.iter_mut());
    // Card VTURD: an armed building's node rig over its written model.
    for line in turret_rigs.attach(&parsed_models, classes.iter_mut()) {
        eprintln!("turret rig: {line}");
    }
    drop(parsed_models);

    // Textures: write each one, plus a `<base>.dtl.ktx2` companion for every
    // detailCompressed base (crate::detail). A companion consumed that way is
    // dropped from the plain texture list — it is a modulation layer, not a
    // material of its own — but an orphan `_dtl` (no flagged base) still bakes
    // normally.
    let tex_dir = out.join("textures");
    // `sky_ SkyObject` names capital ships that do NOT live in the map's own
    // LVL — VERIFIED by searching every LVL on both discs for all 17 shipped
    // names: on every GROUND map they are in a SIDE LVL (ALL/CIS/IMP/REP),
    // and only the space maps carry theirs locally. Pull them in so the
    // manifest's `have_model` check below can see them.
    if let Some(sky) = sky_cfg.as_ref() {
        let wanted: Vec<String> = sky
            .objects
            .iter()
            .map(|o| o.geometry.clone())
            .filter(|g| !g.is_empty() && !models.iter().any(|m| m.eq_ignore_ascii_case(g)))
            .collect();
        if !wanted.is_empty() {
            let pulled =
                pull_side_models(lvl_path, &wanted, out, &mut all_textures, &mut alpha_usage)?;
            models.extend(pulled);
        }
    }

    // The manifest's sky, resolved HERE rather than beside the rest of the
    // manifest: design D7 needs to know which models and which texture the sky
    // path uses before the KTX2s are written, and `models` is final by now
    // (the walk is over and `pull_side_models` has run).
    let mut sky = crate::lighting::sky_from(sky_cfg.as_ref(), |g| {
        models.iter().find(|m| m.eq_ignore_ascii_case(g)).cloned()
    });
    if let (Some(sky), Some(dome)) = (sky.as_mut(), sky_cfg.as_ref().and_then(|s| s.dome.as_ref()))
    {
        let skeletons = crate::hardpoints::Skeletons::of(&expanded);
        let authored_models = dome.models.iter().filter(|m| {
            models
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&m.geometry))
        });
        for (model, authored) in sky.models.iter_mut().zip(authored_models) {
            let Some(skel) = skeletons.find(&authored.geometry) else {
                continue;
            };
            let Ok(matrices) = skel.bind_matrices() else {
                continue;
            };
            for effect in &authored.effects {
                let Some(index) = skel.index_of(&effect.hardpoint) else {
                    continue;
                };
                let matrix = matrices[index];
                model.effects.push(manifest::ManifestSkyEffect {
                    effect: effect.effect.clone(),
                    hardpoint: effect.hardpoint.clone(),
                    parameter: effect.parameter,
                    max_delay: effect.max_delay,
                    pos: [matrix[3][0], matrix[3][1], matrix[3][2]],
                    axes: [0, 1, 2].map(|i| [matrix[i][0], matrix[i][1], matrix[i][2]]),
                });
            }
        }
    }

    // The far-LOD sheet, now that `sky_` has been walked: its texture is
    // `DomeInfo.LowResTerrain.Texture` and its UV scale that scope's
    // `TextureScale` (`bake::lowres_glb`). Written whenever the `tern` SHIPS
    // one, including on BF2 DAG1/NAB2, whose sheet the disc never draws
    // because they author no `LowResTerrain` scope at all — the manifest says
    // that with `sky.low_res_terrain: None`, and the bake stays a
    // transcription of what the disc holds.
    let lrt = sky_cfg
        .as_ref()
        .and_then(|s| s.dome.as_ref())
        .and_then(|d| d.low_res_terrain.as_ref());
    // D7: the sky path is out of scope for ALPHA RULE v2. `sky_models` are the
    // DOME shells (`ManifestSky.models`); `sky_textures` is every texture their
    // drawn segments name, plus the dome's own `DomeInfo.Texture`.
    let sky_models: std::collections::HashSet<String> = sky
        .as_ref()
        .map(|s| {
            s.models
                .iter()
                .map(|m| m.geometry.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let fx_model_named: std::collections::HashSet<String> = fx
        .models()
        .iter()
        .flat_map(|m| alpha_usage.textures_of(m).map(str::to_owned))
        .collect();
    let mut sky_textures: std::collections::HashSet<String> = sky_models
        .iter()
        .flat_map(|m| alpha_usage.textures_of(m).map(str::to_owned))
        .collect();
    if let Some(t) = sky.as_ref().and_then(|s| s.texture.as_ref()) {
        sky_textures.insert(t.to_ascii_lowercase());
    }

    // Textures whose tier-2 override carries the PS2 alpha channel instead of
    // its own, because a draw depends on that alpha (design D3). Reported by
    // the bake so the count stays visible.
    let mut pc_alpha_substitutions: Vec<String> = Vec::new();
    // D7 report: sky textures whose KTX2 alpha state the carve-out changes
    // relative to D3 alone, and textures BOTH a sky shell and a world model
    // draw (where the two rules can disagree).
    let mut sky_overrides: Vec<String> = Vec::new();
    let mut sky_and_world: Vec<String> = Vec::new();
    // D3's carve-out check: sky textures whose KTX2 alpha state moved because
    // the mode layers' models joined the pre-v2 OR. Expected EMPTY on every
    // map — if it is not, design D3 says to take the layer models back out.
    let mut layer_flag_moves: Vec<String> = Vec::new();
    // Queue item SM: textures NO segment names whose disc alpha is not all
    // 255 — the ones `keep_alpha`'s `unnamed` clause keeps an alpha for that
    // D3 alone would have forced opaque. Reported so the count stays visible.
    let mut unnamed_kept: Vec<String> = Vec::new();
    // Queue item WA: the lower-cased names THIS map's `fx__ Effect("Water")`
    // gives its water layers — `MainTexture` (all twelve wet maps author one)
    // and `FoamTexture` (both KAM1s, nobody else). Those sheets keep their
    // disc alpha; every other sheet in the bake is unaffected. Empty on the
    // 22 dry maps, which author no `Effect("Water")` at all, and asked only
    // of a MAP bake so the clause names the bake it applies to, exactly as
    // `unnamed` does for the common ones.
    //
    // The config's OWN names, not `water_glb::resolve_texture`'s answer: the
    // fallbacks that function carries (`LAYR`, then the map's single
    // `*_water`) are guesses for data neither disc ships, and a guess must
    // not decide what a baked file contains. All twelve shipped maps author a
    // `MainTexture`, so the two agree on every map that exists.
    let water_named_textures: std::collections::BTreeSet<String> = match (kind, &water_cfg) {
        (Content::Map, Some(c)) => [c.main_texture.as_deref(), c.foam_texture.as_deref()]
            .into_iter()
            .flatten()
            .map(|n| n.to_ascii_lowercase())
            .collect(),
        _ => Default::default(),
    };
    // Queue CLN5 (2026-09-25): the drawn `LOWR` sheet's texture keeps its
    // disc alpha too — the PS2 alpha-tests that draw at AREF 0x10
    // (`analysis/re-lowr.md` §2.2 [V]) and BF1 KAS1's and KAS2's sheets carry
    // real alpha (`crate::alpha::keep_alpha`, `consumer_named`). Only a sheet
    // the map both SHIPS and names in a `LowResTerrain` scope: BF2 DAG1/NAB2's
    // undrawn ones name nothing.
    let sheet_named: Option<String> = match (kind, &lowres) {
        (Content::Map, Some(_)) => lrt
            .and_then(|l| l.texture.as_deref())
            .map(|n| n.to_ascii_lowercase()),
        _ => None,
    };
    // A public sky correction keeps the canonical name, while the player's
    // donor may call that same far artwork something else. Preserve its native
    // binding under the corrected name without changing gameplay metadata.
    let far_texture_alias = super::texture_alias::far_alias(
        original.as_deref(),
        sheet_named.as_deref(),
        &all_textures
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
    )?;
    // Queue FOLGG: the sheets this map's `grasspatch` classes name
    // (`Texture`, default "grass" — `crate::foliage::grass_class`) keep their
    // disc alpha too: every grass sprite is a cutout composited by it
    // (analysis/re-foliage.md §7; the sprites are rectangles without it). No
    // model names these sheets, so without the clause they bake opaque.
    // Card LEAF: so do the `leafpatch` classes' sheets — the PS2 blends every
    // leaf sprite by its texel alpha (analysis/re-leafpatch.md §6.5).
    let grass_named: std::collections::BTreeSet<String> = match kind {
        Content::Map => grass_props
            .iter()
            .map(|(name, props)| {
                crate::foliage::grass_class(name, props)
                    .texture
                    .to_ascii_lowercase()
            })
            .chain(leaf_props.iter().map(|(name, props)| {
                crate::leafpatch::leaf_class(name, props)
                    .texture
                    .to_ascii_lowercase()
            }))
            .collect(),
        _ => Default::default(),
    };
    let mut water_alpha_kept: Vec<String> = Vec::new();
    let mut consumed: Vec<String> = Vec::new();
    for t in &all_textures {
        // Keep the existing companion writes/consumption order. Absent
        // companions are removed only after their base is successfully emitted.
        if crate::detail::companion(t, &all_textures).is_none() {
            continue;
        }
        if let Some(name) = crate::detail::write_detail_for(&tex_dir, t, &all_textures)? {
            consumed.push(name);
        }
    }
    for t in &all_textures {
        if consumed.contains(&t.name.to_ascii_lowercase()) {
            continue;
        }
        let lower = t.name.to_ascii_lowercase();
        // ALPHA RULE v2, design D3: the baked file keeps its alpha iff at
        // least one DRAW in this bake was judged MASK or BLEND. A texture a
        // segment names but no such draw needs bakes opaque.
        //
        // A texture NO segment names at all keeps its disc alpha (queue item
        // SM): it is not a world texture, and the PS2 composites the HUD
        // sheets among them by exactly that alpha (`analysis/re-hud-icons.md`
        // §2 [V]) — see `crate::alpha::keep_alpha`.
        //
        // Design D7: unless the SKY path uses it, in which case the pre-v2
        // answer stands — the sky pipeline composites by alpha and never reads
        // the material, so the rule must not reach it.
        let is_sky = sky_textures.contains(&lower);
        let top_or_flags = tex_flags.get(&lower).copied();
        let layer_or_flags = layer_tex_flags.get(&lower).copied();
        // No segment of any top-level or mode-layer model names it — and, the
        // lead's scope decision for item SM (2026-09-17), only in a COMMON
        // bake. The evidence for the clause is the HUD's (every HUD bitmap is
        // composited by exactly this alpha, `analysis/re-hud-icons.md` §2
        // [V]) and the HUD sheets live in COMMON.LVL / INGAME.LVL. A map's
        // water, terrain and effect sheets are unnamed too, but the water
        // pass composites by texture alpha and what the PS2's water layer
        // does with it was unread VU1 (`analysis/re-water.md` §8, HOLD 21), so
        // those kept design D3's answer until WA measured the WATER half (the
        // clause below); terrain sheets still keep it, and the particle
        // effects' sheets keep their alpha by `fx_named` (card FXP1).
        let unnamed = kind == Content::Common && top_or_flags.is_none() && layer_or_flags.is_none();
        // Queue item WA (2026-09-17): the sheets THIS map's own `fx__
        // Effect("Water")` names keep their disc alpha, because every blended
        // water layer on both discs composites by it — TEX0 TCC=1 / TFX=0
        // MODULATE through the engine's one texture path, then ALPHA 0x44
        // source-over, so `As = At x Av >> 7`
        // (`analysis/re-water-alpha.md` §0, [V] but `[O]` UNAUDITED). This is
        // the call site that carries the name set; it is deliberately NARROW
        // (the lead's scope decision, same doc) — `MainTexture` and
        // `FoamTexture` and nothing else, so a map's terrain, foliage and
        // mist sheets keep design D3's answer.
        let water_named = water_named_textures.contains(&lower);
        let consumer_named = water_named
            || sheet_named.as_deref() == Some(lower.as_str())
            || far_texture_alias
                .as_ref()
                .is_some_and(|(_, native)| native.eq_ignore_ascii_case(&lower))
            || grass_named.contains(&lower)
            || pass_named.contains(&lower)
            || flash_named.contains(&lower)
            || hud_named.contains(&lower)
            || contact_named.contains(&lower)
            || glow_named.contains(&lower)
            || fx_named.contains(&lower)
            || fx_model_named.contains(&lower)
            || precip_named.as_deref() == Some(lower.as_str())
            || fog_cloud_named.as_deref() == Some(lower.as_str());
        // The pre-v2 OR of the material flag word over every model that draws
        // this texture: top-level ones, and since design D3 the mode layers'
        // too. A texture NO model names keeps the old `unwrap_or` default.
        let old_or_flags = match (top_or_flags, layer_or_flags) {
            (None, None) => crate::detail::ALPHA_IS_MEANINGFUL,
            (a, b) => a.unwrap_or(0) | b.unwrap_or(0),
        };
        let world_needs = alpha_usage.needs_alpha_outside(&lower, &sky_models);
        let needs_alpha = glow_named.contains(&lower)
            || crate::alpha::keep_alpha(is_sky, old_or_flags, world_needs, unnamed, consumer_named);
        if is_sky {
            // Design D3's carve-out check: would leaving the layer models out
            // of the pre-v2 OR — D7's exact input — have answered differently?
            let top_only = top_or_flags.unwrap_or(crate::detail::ALPHA_IS_MEANINGFUL);
            if (glow_named.contains(&lower)
                || crate::alpha::keep_alpha(true, top_only, world_needs, unnamed, consumer_named))
                != needs_alpha
            {
                layer_flag_moves.push(t.name.clone());
            }
            if needs_alpha != alpha_usage.needs_alpha(&lower) {
                sky_overrides.push(format!(
                    "{}({})",
                    t.name,
                    if needs_alpha { "keeps alpha" } else { "opaque" }
                ));
            }
            if alpha_usage
                .models_using(&lower)
                .any(|m| !sky_models.contains(m))
            {
                sky_and_world.push(t.name.clone());
            }
        }
        // Both tiers, side by side (PLAN §Classic-plus assets, the maintainer
        // 2026-09-03): the base file is ALWAYS the PS2 disc's pixels — the sim
        // reads only that — and the classic-plus override, where the chain has
        // one, is written beside it as `<name>.tier2.ktx2`. The renderer picks
        // between them at runtime.
        //
        // The alpha state is the PS2 texture's on BOTH files: force it opaque
        // where no draw wants alpha, and where a draw does want it, give the
        // override the PS2 mask (the PC file's own alpha is a different
        // channel — gloss/spec — on several of the shipped sheets, so a MASK
        // draw would cut out along a mask nobody authored and the tier flip
        // would change the silhouette).
        let mut r = crate::tier::Resolved::from_ps2(t);
        let usage = crate::tier::Usage {
            sky: is_sky,
            alpha_carried: needs_alpha,
        };
        let mut tier2 = crate::tier::tier2(t, pc.as_ref(), usage);
        let carries_alpha = || {
            r.mips
                .iter()
                .any(|m| m.as_chunks::<4>().0.iter().any(|px| px[3] < 255))
        };
        if !is_sky && unnamed && carries_alpha() {
            unnamed_kept.push(t.name.clone());
        }
        // Queue item WA, reported so the count stays visible exactly as SM's
        // does: the map's own water sheets that carry real alpha on the disc.
        // Expected 1 per water map, the KAM1s included: they name 2, but the
        // BASE sheet `kam1_water_2` is alpha-255 everywhere and so never
        // appears (the build's `maps` bake printed 12 lines, one per map).
        if water_named && carries_alpha() {
            water_alpha_kept.push(t.name.clone());
        }
        if !needs_alpha {
            r.make_opaque();
            if let Some(o) = tier2.as_mut() {
                o.make_opaque();
            }
        } else if let Some(o) = tier2.as_mut() {
            o.take_alpha_from(&r);
            pc_alpha_substitutions.push(t.name.clone());
        }
        r.write_pair(&tex_dir, &t.name, tier2.as_ref())?;
        if crate::detail::companion(t, &all_textures).is_none() {
            crate::remove_optional_file(&tex_dir.join(crate::detail::detail_file(&lower)))?;
        }
        if let Some(o) = &tier2 {
            let file = format!("{}.tier2.ktx2", t.name);
            texture_tiers.push((lower.clone(), o.tier.as_str().to_owned()));
            texture_tier2.push((lower.clone(), file));
        }
        // DERIVED, and read by nothing at runtime (design D4): the textures at
        // least one draw composites.
        if alpha_usage.blended(&lower) {
            transparent.push(t.name.clone());
        }
        textures.push(t.name.clone());
    }
    if let Some(alias) = &far_texture_alias {
        super::texture_alias::install(
            alias,
            &tex_dir,
            &mut textures,
            &mut texture_tiers,
            &mut texture_tier2,
            &mut transparent,
        )?;
    }
    alpha_report(layer_flag_moves, |n| {
        format!(
            "D3 CARVE-OUT MOVED — {n} sky texture(s) change alpha state because the \
             lvl_ layers' models joined the pre-v2 OR"
        )
    });
    alpha_report(sky_overrides, |n| {
        format!("D7 sky carve-out changed the KTX2 alpha state of {n} texture(s)")
    });
    alpha_report(sky_and_world, |n| {
        format!("{n} texture(s) drawn by BOTH a sky shell and a world model")
    });
    alpha_report(pc_alpha_substitutions, |n| {
        format!("tier 2 took the PS2 alpha on {n} texture(s) whose alpha the base keeps")
    });
    alpha_report(unnamed_kept, |n| {
        format!("{n} alpha-bearing texture(s) no segment names keep their disc alpha")
    });
    alpha_report(water_alpha_kept, |n| {
        format!("{n} water sheet(s) this map's Effect(\"Water\") names keep their disc alpha")
    });

    // Water, after the texture list is final: resolving which texture paints
    // it needs the baked names (4 of the 12 maps name none in `LAYR`).
    let water = match water {
        Some(w) if crate::water_glb::triangle_count(&w) > 0 => {
            let cfg = water_cfg.as_ref();
            let texture = crate::water_glb::resolve_texture(&w, cfg, &textures);
            let glb = crate::water_glb::water_to_glb(&w, cfg, "water", texture.as_deref())?
                .expect("triangle_count > 0");
            std::fs::write(out.join("water.glb"), glb)?;
            Some(ManifestWater {
                glb: "water.glb".into(),
                texture,
                height: w.layer.height,
                // The PS2's scroll where the map authors a `Velocity` (all
                // twelve do); `LAYR`'s dead copy only for data neither disc
                // ships.
                uv_velocity: cfg
                    .and_then(|c| c.scroll_per_second(w.info.unknown_5))
                    .unwrap_or(w.layer.uv_velocity),
                layer_color: w.layer.color,
                wet_cells: w.wet_cells(),
                patches: w.patches.len(),
                triangles: crate::water_glb::triangle_count(&w),
                config: cfg.map(|c| crate::water_glb::manifest_config(c, &w)),
                wet: Some(manifest::ManifestWetMask::from_nibbles(
                    w.info.grid,
                    w.info.cell_size,
                    w.info.half_extent,
                    &w.cells,
                )),
            })
        }
        _ => None,
    };

    // Base world = the one carrying the terrain name (else the most
    // populated); every other unwrapped world merges into it.
    if worlds.is_empty() {
        if kind == Content::Map {
            bail_no_world(lvl_path)?;
        }
        let stem = Path::new(lvl_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| "common".into());
        worlds.push(mungefs::world::World {
            name: stem,
            terrain_name: None,
            sky_name: None,
            regions: Vec::new(),
            instances: Vec::new(),
            hints: Vec::new(),
            barriers: Vec::new(),
        });
    }
    let base_idx = worlds
        .iter()
        .position(|w| w.terrain_name.is_some())
        .unwrap_or_else(|| {
            worlds
                .iter()
                .enumerate()
                .max_by_key(|(_, w)| w.instances.len())
                .map(|(i, _)| i)
                .unwrap()
        });
    let w = worlds.remove(base_idx);
    let mut regions: Vec<ManifestRegion> = w.regions.iter().map(region_entry).collect();
    let mut instances: Vec<ManifestInstance> = w.instances.iter().map(instance_entry).collect();
    for extra in &worlds {
        regions.extend(extra.regions.iter().map(region_entry));
        instances.extend(extra.instances.iter().map(instance_entry));
    }
    // The AI data: the base world's own block, then one per mode layer that
    // ships a `plan` of its own. A layer without one gets None and the
    // runtime falls back to the base.
    // Sidecar keys are claimed only by a world that HAS a plan, so a layer
    // without one neither writes nor reserves a file name.
    let mut nav_keys: Vec<String> = vec!["base".to_string()];
    let base_worlds: Vec<&mungefs::world::World> =
        std::iter::once(&w).chain(worlds.iter()).collect();
    let base_graph = base_plan
        .as_ref()
        .map(|p| nav_plan_entry(out, "base", p, base_plnp.as_ref()))
        .transpose()?;
    let nav = nav_block(base_graph, &base_worlds);
    let mut layers: Vec<ManifestLayer> = Vec::new();
    for ml in &mode_layers {
        let group = &ml.group;
        let graph = match ml.plan.as_ref() {
            Some(plan) => {
                let key = unique_nav_key(&group[0].name, &mut nav_keys);
                Some(nav_plan_entry(out, &key, plan, ml.plnp.as_ref())?)
            }
            None => None,
        };
        let refs: Vec<&mungefs::world::World> = group.iter().collect();
        let nav = nav_block(graph, &refs);
        layers.push(ManifestLayer {
            name: group[0].name.clone(),
            worlds: group.iter().map(|g| g.name.clone()).collect(),
            regions: group
                .iter()
                .flat_map(|g| g.regions.iter().map(region_entry))
                .collect(),
            instances: group
                .iter()
                .flat_map(|g| g.instances.iter().map(instance_entry))
                .collect(),
            nav,
        });
    }
    // PC BF1 uses the later model encoding too. The mission Lua generation
    // identifies the game; a model's vertex encoding only identifies its codec.
    let source_game = crate::gameplay::source_game(Path::new(lvl_path))?
        .unwrap_or(
            if bf2_models > 0 || (models.is_empty() && !layers.is_empty()) {
                "bf2"
            } else {
                "bf1"
            },
        )
        .to_string();
    let aligned_surface = terrain_parsed
        .as_ref()
        .zip(terrain_render_heights.as_deref())
        .map(|(t, heights)| {
            crate::terrain_glb::AlignedSurface::build(t, source_game == "bf2", heights)
        })
        .transpose()?;
    let lowres_glb = match lowres {
        Some((ref name, ref sheet, half)) => crate::lowres_glb::lowres_to_glb_with_surface(
            name,
            sheet,
            half,
            lrt.and_then(|l| l.texture.as_deref()),
            lrt.and_then(|l| l.texture_scale)
                .unwrap_or(crate::lowres_glb::DEFAULT_TEXTURE_SCALE),
            aligned_surface.as_ref(),
        )?
        .map(|glb| -> Result<String> {
            let fname = format!("lowres_{name}.glb");
            std::fs::write(out.join(&fname), glb)?;
            Ok(fname)
        })
        .transpose()?,
        None => None,
    };
    let mut terrain_layers_glb = None;
    if let (Some(t), Some(fname)) = (&terrain_parsed, &terrain_glb) {
        let bf2 = source_game == "bf2";
        let glb = match &aligned_surface {
            Some(surface) => surface.base_glb(t)?,
            None => crate::terrain_glb::terrain_to_glb(t, bf2)?,
        };
        std::fs::write(out.join(fname), glb)?;
        let layers = match &aligned_surface {
            Some(surface) => surface.layers_glb(t, bf2)?,
            None => crate::terrain_glb::terrain_layers_to_glb(t, bf2)?,
        };
        if let Some(glb) = layers {
            let fname = format!("terrain_layers_{}.glb", t.name);
            std::fs::write(out.join(&fname), glb)?;
            terrain_layers_glb = Some(fname);
        }
    }
    let lighting =
        crate::lighting::lighting_from(sky_cfg.as_ref(), lights_cfg.as_ref(), source_game == "bf2");
    let foliage = if foliage_file.is_none() && grass_props.is_empty() {
        None
    } else {
        let f = crate::foliage::bake(
            out,
            &crate::foliage::Inputs {
                lvl_path,
                game: &source_game,
                terrain: terrain_parsed.as_ref(),
                mask: folg_mask.as_ref(),
                file: foliage_file.as_ref(),
                models: &models,
                classes: &classes,
                paths: &paths,
                grass: &grass_props,
            },
        )?;
        if let Some(f) = &f {
            eprintln!(
                "foliage: {} instances over {} meshes, {} path props, {} grass classes",
                f.count,
                f.meshes.len(),
                f.path_props.len(),
                f.grass.len()
            );
        }
        f
    };
    // Card LEAF: the leaf classes, and what each prop class hangs on its
    // model's hardpoints (cap 30 on BF1, 16 on BF2).
    let leaf: Vec<manifest::ManifestLeafClass> = leaf_props
        .iter()
        .map(|(name, props)| crate::leafpatch::leaf_class(name, props))
        .collect();
    if !leaf.is_empty() {
        let cap = if source_game == "bf2" { 16 } else { 30 };
        crate::leafpatch::attach(
            &attach_props,
            &leaf,
            cap,
            &crate::hardpoints::Skeletons::of(&expanded),
            &mut classes,
        );
        eprintln!(
            "leaf: {} classes, {} attachments on {} classes",
            leaf.len(),
            classes.iter().map(|k| k.attach.len()).sum::<usize>(),
            classes.iter().filter(|k| !k.attach.is_empty()).count()
        );
    }
    crate::attached_effects::attach(
        &attach_props,
        &crate::hardpoints::Skeletons::of(&expanded),
        &mut classes,
    );
    // Card BPNT: the EntityLights each prop class hangs on its hardpoints.
    // BF2 only: BF1's sole point source is its explosions' flash (§11).
    if !entity_lights.is_empty() && source_game == "bf2" {
        crate::entity_light::attach(
            &attach_props,
            &entity_lights,
            &crate::hardpoints::Skeletons::of(&expanded),
            crate::entity_light::Miss::of(&source_game),
            &mut classes,
        );
    }
    if !(point_lights.is_empty() && entity_lights.is_empty()) {
        eprintln!(
            "lights: {} authored, {} on {} classes",
            point_lights.len(),
            classes.iter().map(|k| k.lights.len()).sum::<usize>(),
            classes.iter().filter(|k| !k.lights.is_empty()).count()
        );
    }
    {
        let lower = |v: &[String]| v.iter().map(|n| n.to_ascii_lowercase()).collect::<Vec<_>>();
        fx.write(out, &source_game, &lower(&textures), &lower(&models))?;
    }
    let manifest = Manifest {
        map: w.name.clone(),
        source_game,
        terrain_name: w.terrain_name.clone(),
        terrain_glb,
        sky_name: w.sky_name.clone(),
        models,
        textures,
        transparent,
        texture_tiers,
        classes,
        geometries,
        regions,
        instances,
        paths,
        layers,
        lighting,
        sky,
        water,
        collision,
        terrain_cutters,
        nav,
        texture_tier2,
        lowres_glb,
        foliage,
        terrain_layers_glb,
        leaf,
        hdr: hdr.map(|h| manifest::ManifestHdr {
            num_bloom_passes: h.num_bloom_passes,
            max_total_weight: h.max_total_weight,
            glow_threshold: h.glow_threshold,
        }),
        point_lights,
        lightning,
        precipitation,
        wind,
        terrain_query_size,
        fog_cloud,
    };
    Ok(manifest)
}
