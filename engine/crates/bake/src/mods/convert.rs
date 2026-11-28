//! Atomic legacy BF2 addon converter -> versioned `openbf_data/mod_cache/<ns>/`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use extensions::mods::{
    ModCacheMeta, ModMapSummary, ModMissionSummary, MOD_CACHE_SCHEMA_VERSION, MOD_CONVERTER_ID,
};
use extensions::mods::catalog::{SourceGame, SourcePlatform};
use mungefs::addon::{read_core_locl, stock_world_rel, AddonWorldLoad};

use super::addon_sides::{convert_addon_sides, LevelSides};
use super::assets::{bake_map_assets, units_settings, MapJob, MapShared, Reuse};
use super::mission::mission_to_ron;
use super::replace::Replaced;
use super::{inspect_mod_folder, ModEnv};

pub struct ConvertOptions<'a> {
    pub mod_dir: &'a Path,
    pub cache_dir: &'a Path,
    pub mod_id: String,
    pub namespace: String,
    pub emit_progress: bool,
    pub env: &'a ModEnv,
}

#[derive(Debug, Clone)]
pub struct ConvertReport {
    pub meta: ModCacheMeta,
    pub maps_baked: usize,
    pub missions_written: usize,
}

struct StagingGuard(Option<PathBuf>);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn progress(emit: bool, pct: u8, msg: &str) {
    if emit {
        println!("PROGRESS {pct} {msg}");
    }
}

/// Whether `rel` is the mod's copy of the game's own INGAME.LVL, which holds
/// the whole HUD (an addon map's `dc:ingame.lvl` is not).
fn is_global_ingame(rel: &str) -> bool {
    let rel = rel.replace('\\', "/").to_ascii_lowercase();
    rel.ends_with("data/_lvl_pc/ingame.lvl") && !rel.contains("addon/")
}

/// Where a mod's INGAME.LVL places weapon models in the one-player HUD's two
/// weapon rows: its `1playerhud` elements of those names with only their
/// `MeshInfo` records. The game adds the models its own layout does not
/// place; without a row's record a model draws at the row's default pose, a
/// third of the size and beside the row. `None` when the LVL has no such
/// layout.
fn weapon_icon_places(lvl: &[u8]) -> Result<Option<manifest::hud::HudLayout>> {
    use manifest::hud::{HudArg, HudLayout, HudNode};
    use mungefs::config::ConfigNode;
    fn node(n: &ConfigNode) -> HudNode {
        HudNode {
            name: mungefs::hud::name(n.hash)
                .map_or_else(|| format!("#{:08x}", n.hash), str::to_owned),
            args: (0..n.values.len())
                .map(|i| match n.str_arg(i) {
                    Some(s) => HudArg::Str(s.to_owned()),
                    None => HudArg::Num(f32::from_bits(n.values[i])),
                })
                .collect(),
            children: n.children.iter().map(node).collect(),
        }
    }
    fn rows(nodes: Vec<HudNode>, out: &mut Vec<HudNode>) {
        for mut n in nodes {
            let row = matches!(n.args.first(), Some(HudArg::Str(s))
                if s.eq_ignore_ascii_case("player1weapon1mesh")
                    || s.eq_ignore_ascii_case("player1weapon2mesh"));
            if row && n.name.eq_ignore_ascii_case("model3d") {
                n.children
                    .retain(|c| c.name.eq_ignore_ascii_case("meshinfo"));
                out.push(n);
            } else {
                rows(n.children, out);
            }
        }
    }
    let inner = mungefs::open(lvl).map_err(|e| anyhow::anyhow!("{e}"))?;
    let content = mungefs::content_chunks(inner).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut found = Vec::new();
    for c in content.iter().filter(|c| &c.tag == b"hud_") {
        let layout = mungefs::hud::parse(c).map_err(|e| anyhow::anyhow!("hud_: {e}"))?;
        if layout.file_name() == Some("1playerhud") {
            rows(layout.nodes.iter().map(node).collect(), &mut found);
        }
    }
    Ok((!found.is_empty()).then(|| HudLayout {
        name: "1playerhud".into(),
        nodes: found,
    }))
}

/// The lines of a mod's CORE.LVL table the mod itself wrote. OURS: a map
/// mod ships the whole stock PC table beside its own `level.<map>.*` lines
/// (ABR and KTH: key 1202146172, "INSTANT ACTION"), and laid over the
/// port's table its copies re-cased every shell label. A line is kept when
/// its key is one the port's own BF2 table lacks, or its text differs from
/// stock BF2 PC's CORE.LVL and from the port's line by more than case.
/// replace: the mod's own list of changed keys, were one shipped.
fn mod_authored_locl(
    mut table: manifest::hud::HudLocl,
    env: &ModEnv,
    scratch: &Path,
) -> manifest::hud::HudLocl {
    let pc = env
        .sources
        .file(SourceGame::Bf2, SourcePlatform::Pc, "core.lvl", scratch)
        .ok()
        .and_then(|p| crate::hud::read_locl(&p.to_string_lossy()).ok());
    let ours = env
        .stock
        .as_ref()
        .and_then(|s| s.read("common_bf2/locl/english.ron"))
        .and_then(|b| ron::from_str::<manifest::hud::HudLocl>(&String::from_utf8_lossy(&b)).ok());
    table.entries.retain(|(h, text)| {
        let Some(own) = ours.as_ref().and_then(|t| t.get(*h)) else {
            return true;
        };
        let pc_copy = pc.as_ref().and_then(|t| t.get(*h)) == Some(text.as_str());
        !pc_copy && !own.eq_ignore_ascii_case(text)
    });
    table
}

/// Convert a legacy BF2 addon folder into `cache_dir` atomically via a temporary
/// staging directory, never modifying `mod_dir`.
pub fn convert_mod_to_cache(opts: &ConvertOptions<'_>) -> Result<ConvertReport> {
    if !opts.mod_dir.is_dir() {
        bail!("mod directory not found: {}", opts.mod_dir.display());
    }

    progress(opts.emit_progress, 5, "Inspecting mod structure...");
    let (discovered, insp) =
        inspect_mod_folder(opts.mod_dir, &opts.mod_id, &opts.namespace, opts.env);
    if !discovered.compatibility.is_playable() {
        bail!(
            "{}: {}",
            discovered.display_name,
            discovered
                .compatibility
                .reason()
                .unwrap_or("mod is invalid or unsupported")
        );
    }

    if let Some(parent) = opts.cache_dir.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating cache parent {}", parent.display()))?;
    }

    let staging = opts
        .cache_dir
        .with_extension(format!("staging.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .with_context(|| format!("creating staging {}", staging.display()))?;
    let mut guard = StagingGuard(Some(staging.clone()));

    let groups = insp.map_groups(&opts.namespace);
    let mut baked_maps: Vec<ModMapSummary> = Vec::with_capacity(groups.len());
    let mut warnings = discovered.warnings.clone();
    let mut maps_baked = 0usize;
    let mut first_failed_map = None;
    let mut missions_written = 0usize;
    let mut shared = MapShared::default();
    let mut reuse = Reuse::open(opts.cache_dir, &discovered, opts.env);
    let mut mission_scripts: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for ml_path in &insp.mission_lvls {
        if let Ok(bytes) = std::fs::read(ml_path) {
            if let Ok(chunks) = mungefs::lua50::extract_scripts(&bytes) {
                for sc in chunks {
                    if sc.body.starts_with(b"\x1bLua") {
                        mission_scripts
                            .entry(sc.name.to_ascii_lowercase())
                            .or_insert(sc.body);
                    }
                }
            }
        }
    }
    let mut written_libs: BTreeMap<String, String> = BTreeMap::new();
    let mut levels: Vec<LevelSides> = Vec::new();

    let total_steps = groups.len().max(1);
    for (gi, g) in groups.iter().enumerate() {
        let base_pct = 15 + ((gi * 65) / total_steps) as u8;

        // Nothing can be played on it, and its world must not fail the mod.
        if !g.missions.iter().any(|m| m.mission.supported) {
            warnings.push(format!(
                "Map `{}` has no mission openbf can run yet; not converted.",
                g.display_name
            ));
            continue;
        }

        let Some(map_lvl) = &g.source_lvl_path else {
            warnings.push(format!(
                "Map `{}` references missing level file `{}`; skipped.",
                g.display_name, g.source_lvl_rel
            ));
            continue;
        };

        // A mission on one of the game's own maps takes that world from the
        // player's PC copy, under the addon's layers.
        let stock_scratch = staging.join(STOCK_SCRATCH);
        let (map_lvl, world_loads) =
            match stock_worlds(map_lvl, &g.world_loads, opts.env, &stock_scratch) {
                Ok(found) => found,
                Err(e) => {
                    warnings.push(format!(
                        "Map `{}` plays on one of the game's own maps, which needs the PC copy of Battlefront II set in the launcher ({e:#}); not converted.",
                        g.display_name
                    ));
                    continue;
                }
            };

        let map_out = staging.join("assets").join(&g.map_key);
        let job = MapJob {
            map_key: &g.map_key,
            map_lvl: &map_lvl,
            world_loads: &world_loads,
            shares_files: !g.world_loads.is_empty(),
            loadscreen: g.loadscreen_lvl_path.as_deref(),
            sounds: &g.custom_sound_lvls,
            core: insp.core_lvl.as_deref(),
        };
        // The LVLs read as the mod's own, whether or not they are baked now.
        crate::gameplay::exempt_addon(&map_lvl);
        if let Some(load_lvl) = job.loadscreen {
            crate::gameplay::exempt_addon(load_lvl);
        }
        // A stock world may be a copy in this run's own scratch folder, so
        // that folder's name is left out of the key.
        let scratch_name = format!("{:?}", stock_scratch.to_string_lossy());
        let settings = format!("{job:?}").replace(scratch_name.trim_matches('"'), "");
        let key = reuse.key(&settings);
        let step = format!("map:{}", job.map_key);
        let kept = reuse.take(&step, key.as_ref());
        let verb = if kept.is_some() { "Keeping unchanged" } else { "Converting" };
        progress(
            opts.emit_progress,
            base_pct,
            &format!("{verb} map `{}`...", g.display_name),
        );
        if let Some(report) = kept {
            warnings.extend(report.warnings);
        } else {
            std::fs::create_dir_all(&map_out)?;
            // One map that cannot be built leaves the others playable.
            match bake_map_assets(&job, &map_out, &mut shared) {
                Ok(found) => {
                    let report = Replaced {
                        warnings: found,
                        ..Default::default()
                    };
                    reuse.record(&step, key, &staging, format!("assets/{}", g.map_key), &report);
                    warnings.extend(report.warnings);
                }
                Err(e) => {
                    std::fs::remove_dir_all(&map_out)?;
                    warnings.push(format!(
                        "Map `{}` could not be converted ({e:#}).",
                        g.display_name
                    ));
                    first_failed_map.get_or_insert(e);
                    continue;
                }
            }
        }
        maps_baked += 1;

        let level = level_sides(g);
        if !level.sides.is_empty() {
            levels.push(level);
        }

        let mut planned_missions = Vec::with_capacity(g.missions.len());
        let missions_dir = staging.join("missions");
        std::fs::create_dir_all(&missions_dir)?;

        for pm in &g.missions {
            let mut supported = pm.mission.supported;
            let mut reason = pm.mission.unsupported_reason.clone();
            if supported {
                let mut script_ir = None;
                let mut script_libs = Vec::new();
                if pm.mission.scripted_mode {
                    let key = pm.mission.script_name.to_ascii_lowercase();
                    match mission_scripts
                        .get(&key)
                        .ok_or_else(|| anyhow::anyhow!("script chunk `{}` missing", pm.mission.script_name))
                        .and_then(|body| Ok(mungefs::lua50::ir::translate_bytes(body)?))
                    {
                        Ok(def) => {
                            // Transitively translate any `ScriptCB_DoFile` helpers shipped in the mod's mission.lvl.
                            let mut queue: std::collections::VecDeque<String> =
                                def.do_files().into_iter().collect();
                            let mut seen = std::collections::BTreeSet::new();
                            while let Some(lib_name) = queue.pop_front() {
                                let low = lib_name.to_ascii_lowercase();
                                if !seen.insert(low.clone()) {
                                    continue;
                                }
                                let Some(lib_body) = mission_scripts.get(&low) else {
                                    continue;
                                };
                                if let Some(ron_name) = written_libs.get(&low) {
                                    script_libs.push((low, ron_name.clone()));
                                    continue;
                                }
                                match mungefs::lua50::ir::translate_bytes(lib_body) {
                                    Ok(lib_def) => {
                                        for dep in lib_def.do_files() {
                                            queue.push_back(dep);
                                        }
                                        let ron_name = format!("{}__lib_{low}.ron", opts.namespace);
                                        let lib_text =
                                            ron::ser::to_string_pretty(&lib_def, crate::pretty())?;
                                        std::fs::write(missions_dir.join(&ron_name), lib_text)?;
                                        written_libs.insert(low.clone(), ron_name.clone());
                                        script_libs.push((low, ron_name));
                                    }
                                    Err(e) => {
                                        warnings.push(format!(
                                            "Helper script `{lib_name}` for `{}` could not be translated ({e}).",
                                            pm.mission.script_name
                                        ));
                                    }
                                }
                            }
                            script_ir = Some(def);
                        }
                        Err(e) => {
                            supported = false;
                            reason = Some(format!("Script translation failed: {e}"));
                            warnings.push(format!(
                                "Mission `{}` skipped: {e}",
                                pm.mission.script_name
                            ));
                        }
                    }
                }
                if supported {
                    match mission_to_ron(&g.map_key, pm.mission, script_ir, script_libs) {
                        Ok(ron_text) => {
                            std::fs::write(missions_dir.join(&pm.mission_ron), ron_text)?;
                            missions_written += 1;
                        }
                        Err(e) => {
                            supported = false;
                            reason = Some(e.to_string());
                            warnings.push(format!("Mission `{}` skipped: {e}", pm.mission.script_name));
                        }
                    }
                }
            }
            planned_missions.push(ModMissionSummary {
                script_name: pm.mission.script_name.clone(),
                mode: pm.mission.mode.clone(),
                era: pm.mission.era.clone(),
                mission_ron: pm.mission_ron.clone(),
                single_player_weight: pm.mission.single_player_weight,
                multiplayer_variant: pm.mission.multiplayer_variant,
                supported,
                unsupported_reason: reason,
            });
        }

        baked_maps.push(ModMapSummary {
            map_key: g.map_key.clone(),
            legacy_code: g.legacy_code.clone(),
            display_name: g.display_name.clone(),
            description: g.description.clone(),
            source_lvl: g.source_lvl_rel.clone(),
            variant_title: g.variant_title.clone(),
            stock_row: g.stock_row.clone(),
            modes_cw: g.modes_cw.clone(),
            modes_gcw: g.modes_gcw.clone(),
            modes_custom_eras: g.modes_custom_eras.clone(),
            missions: planned_missions,
            sound_planets: g.sound_planets.clone(),
            era_overrides: g.era_overrides.clone(),
        });
    }

    if let Some(e) = first_failed_map.filter(|_| maps_baked == 0) {
        return Err(e);
    }

    // Bake core.lvl localization if present.
    let mut has_locl = false;
    if let Some(core_path) = &insp.core_lvl {
        progress(opts.emit_progress, 85, "Converting localization table...");
        crate::gameplay::exempt_addon(core_path);
        let table = match crate::hud::read_locl(&core_path.to_string_lossy()) {
            Ok(t) => Some(t),
            Err(_) => read_core_locl(core_path).ok().flatten().map(|tbl| {
                let mut entries = tbl.entries;
                entries.sort_by_key(|(h, _)| *h);
                entries.dedup_by_key(|(h, _)| *h);
                manifest::hud::HudLocl {
                    language: "english".into(),
                    entries,
                }
            }),
        };
        if let Some(table) = table {
            let scratch = staging.join("scratch");
            let table = mod_authored_locl(table, opts.env, &scratch);
            let _ = std::fs::remove_dir_all(&scratch);
            if !table.entries.is_empty() {
                crate::write_ron(&staging.join("locl").join("english.ron"), &table)?;
                has_locl = true;
            }
        }
    }

    let _ = std::fs::remove_dir_all(staging.join(STOCK_SCRATCH));

    let ingame = insp
        .stock_override_lvls
        .iter()
        .find(|o| is_global_ingame(&o.rel_path));
    if let Some(ingame) = ingame {
        match std::fs::read(&ingame.abs_path)
            .map_err(anyhow::Error::from)
            .and_then(|lvl| weapon_icon_places(&lvl))
        {
            Ok(Some(places)) => {
                crate::write_ron(&staging.join("hud").join("weapon_icons.ron"), &places)?;
            }
            Ok(None) => {}
            Err(e) => warnings.push(format!("{}: weapon icon places: {e:#}", ingame.rel_path)),
        }
    }

    // The units of the maps' own SIDE LVLs, once every map is known, so that
    // a side many maps read is converted once.
    let script_names: BTreeSet<String> = mission_scripts
        .values()
        .filter_map(|body| mungefs::lua50::undump(body).ok())
        .flat_map(|proto| {
            proto
                .string_constants()
                .into_iter()
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
        })
        .collect();
    let key = reuse.key(&units_settings(&levels, &script_names));
    if let Some(report) = reuse.take("units", key.as_ref()) {
        progress(opts.emit_progress, 81, "Keeping the maps' unchanged units...");
        warnings.extend(report.warnings);
    } else {
        let report = Replaced {
            warnings: convert_addon_sides(
                &levels,
                &script_names,
                opts.mod_dir,
                opts.env,
                &staging,
                &mut |msg| progress(opts.emit_progress, 81, msg),
            )?,
            ..Default::default()
        };
        if staging.join("levels").is_dir() {
            reuse.record("units", key, &staging, "levels".into(), &report);
        }
        warnings.extend(report.warnings);
    }

    // Stock replacements: every active resource operation, translated onto
    // the stock bake's own outputs (`super::replace`).
    let replaced = if discovered.active_ops().next().is_some() {
        let (pct_start, pct_end) = if groups.is_empty() { (10u8, 95u8) } else { (82u8, 95u8) };
        let key = reuse.key(&ron::to_string(&discovered.resource_ops)?);
        let kept = reuse.take("replacements", key.as_ref());
        let verb = if kept.is_some() { "Keeping unchanged" } else { "Converting" };
        progress(opts.emit_progress, pct_start, &format!("{verb} stock replacements..."));
        match kept {
            Some(report) => report,
            None => {
                let abs: BTreeMap<String, PathBuf> = insp
                    .stock_override_lvls
                    .iter()
                    .map(|o| (o.rel_path.to_ascii_lowercase().replace('\\', "/"), o.abs_path.clone()))
                    .collect();
                let report = super::replace::convert_replacements(
                    &discovered.resource_ops,
                    &abs,
                    opts.env,
                    &staging.join("overrides").join("assets"),
                    &staging.join("scratch"),
                    (pct_start, pct_end),
                    &mut |pct, msg| progress(opts.emit_progress, pct, msg),
                )?;
                if staging.join("overrides").is_dir() {
                    reuse.record("replacements", key, &staging, "overrides".into(), &report);
                }
                report
            }
        }
    } else {
        Replaced::default()
    };
    let _ = std::fs::remove_dir_all(staging.join("scratch"));
    warnings.extend(replaced.warnings.iter().cloned());
    if !discovered.declared_overrides.is_empty() {
        warnings.push(
            "Manifest `overrides` entries are not supported by automatic conversion; use legacy LVL replacement files instead."
                .into(),
        );
    }
    let declared_overrides = Vec::new();

    // Bake custom shell / mode icons from addme.script shell LVLs (e.g. dtshell.lvl)
    // and any mod-shipped shell.lvl.
    let mut has_shell_icons = false;
    if !insp.shell_icon_lvls.is_empty() {
        let icons_dir = staging
            .join("assets")
            .join(format!("{}__shell_icons", opts.namespace));
        let mut baked_names = std::collections::BTreeSet::new();
        for lvl_path in &insp.shell_icon_lvls {
            let Ok(bytes) = std::fs::read(lvl_path) else {
                continue;
            };
            let Ok(root) = mungefs::open(&bytes) else {
                continue;
            };
            for tex in mungefs::texture_pc::find_textures(root) {
                let name = tex.name.to_ascii_lowercase();
                if !name.contains("icon") || !baked_names.insert(name.clone()) {
                    continue;
                }
                let Some(surf) = tex.best() else { continue };
                let Some(face) = surf.faces.first() else { continue };
                let Some(mip0) = face.mips.first() else { continue };
                if mip0.rgba.len() != (surf.width * surf.height * 4) as usize {
                    continue;
                }
                let _ = std::fs::create_dir_all(&icons_dir);
                let ktx = crate::ktx2w::write_rgba8(surf.width, surf.height, &[mip0.rgba.clone()]);
                if std::fs::write(icons_dir.join(format!("{name}.ktx2")), ktx).is_ok() {
                    has_shell_icons = true;
                }
            }
        }
    }

    progress(opts.emit_progress, 95, "Finalizing versioned cache...");
    let meta = ModCacheMeta {
        schema_version: MOD_CACHE_SCHEMA_VERSION,
        converter: MOD_CONVERTER_ID.into(),
        mod_id: discovered.id,
        namespace: discovered.namespace,
        source_fingerprint: discovered.source_fingerprint,
        legacy_addon_id: discovered.legacy_addon_id,
        display_name: discovered.display_name,
        version: discovered.version,
        author: discovered.author,
        description: discovered.description,
        mod_type: discovered.mod_type,
        maps: baked_maps,
        dependencies: discovered.dependencies,
        conflicts: discovered.conflicts,
        declared_overrides,
        has_locl,
        warnings,
        resource_ops: discovered.resource_ops,
        replacement_key: discovered.replacement_key,
        replacement_outputs: replaced.outputs,
        unsupported: replaced.unsupported,
        catalog_digest: opts.env.catalogs.digest.clone(),
        target_choices: opts.env.selections.for_mod(&opts.mod_id),
        custom_modes: insp.custom_mode_meta.clone(),
        has_shell_icons,
    };
    meta.save_atomic(&staging)?;

    // Atomic cache directory commit (with backup swap on Windows).
    reuse.commit(&staging)?;
    guard.0 = None;
    progress(opts.emit_progress, 100, "Done");

    Ok(ConvertReport {
        meta,
        maps_baked,
        missions_written,
    })
}

/// Where [`stock_worlds`] may put files it has to extract; removed before
/// the cache is committed.
const STOCK_SCRATCH: &str = ".stock_worlds";

/// A map group's primary world and layers, with each of the game's own worlds
/// (`mungefs::addon::stock_world_rel`) found in the player's PC copy.
fn stock_worlds(
    primary: &Path,
    loads: &[AddonWorldLoad],
    env: &ModEnv,
    scratch: &Path,
) -> Result<(PathBuf, Vec<AddonWorldLoad>)> {
    let find = |path: &Path| -> Result<PathBuf> {
        let Some(rel) = stock_world_rel(path) else {
            return Ok(path.to_owned());
        };
        std::fs::create_dir_all(scratch)?;
        env.sources
            .file(SourceGame::Bf2, SourcePlatform::Pc, rel, scratch)
            .with_context(|| rel.to_owned())
    };
    let loads = loads
        .iter()
        .map(|load| {
            Ok(AddonWorldLoad {
                path: find(&load.path)?,
                ..load.clone()
            })
        })
        .collect::<Result<_>>()?;
    Ok((find(primary)?, loads))
}

/// The map's own SIDE LVLs, in the order its missions first read them, and
/// the classes they ask of them.
fn level_sides(group: &mungefs::addon::AddonMapGroup<'_>) -> LevelSides {
    let mut level = LevelSides {
        map_key: group.map_key.clone(),
        sides: Vec::new(),
        requests: Default::default(),
        reads: Default::default(),
    };
    for planned in group.missions.iter().filter(|m| m.mission.supported) {
        let mission = planned.mission;
        for (side, request) in &mission.class_load_order {
            level
                .reads
                .insert((side.to_ascii_lowercase(), request.to_ascii_lowercase()));
            let Some((_, path)) = mission.custom_side_lvls.iter().find(|(s, _)| s == side) else {
                continue;
            };
            if !level.sides.iter().any(|(_, p)| p == path) {
                level.sides.push((side.clone(), path.clone()));
            }
            level.requests.insert((side.clone(), request.clone()));
        }
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_real_abr_addon_cleanly_without_touching_source() {
        // `OPENBF_ABR_ADDON` names a local copy of the ABR addon (`GameData/addon/ABR`).
        let Some(src_abr) = std::env::var_os("OPENBF_ABR_ADDON") else {
            return;
        };
        let src_abr = Path::new(&src_abr);
        if !src_abr.is_dir() {
            return;
        }
        let cache =
            std::env::temp_dir().join(format!("openbf_abr_cache_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);

        let before = extensions::mods::compute_mod_fingerprint(src_abr)
            .unwrap()
            .digest;
        let report = convert_mod_to_cache(&ConvertOptions {
            mod_dir: src_abr,
            cache_dir: &cache,
            mod_id: "content.abr".into(),
            namespace: "mod_abr".into(),
            emit_progress: false,
            env: &ModEnv::default(),
        })
        .expect("ABR conversion should succeed");
        let after = extensions::mods::compute_mod_fingerprint(src_abr)
            .unwrap()
            .digest;
        assert_eq!(before, after, "source mod files must remain untouched");

        assert_eq!(report.maps_baked, 1);
        assert_eq!(report.missions_written, 4);
        assert!(cache.join("assets/mod_abr__abr/manifest.ron").is_file());
        assert!(cache.join("assets/mod_abr__abr/classes.ron").is_file());
        assert!(cache
            .join("assets/mod_abr__abr/terrain_layers_abr.glb")
            .is_file());
        assert!(cache.join("assets/mod_abr__abr/load/load.ron").is_file());
        assert!(cache.join("locl/english.ron").is_file());
        for ron in [
            "mod_abr__abr_con.ron",
            "mod_abr__abr_con_cw.ron",
            "mod_abr__abr_ctf.ron",
            "mod_abr__abr_ctf_cw.ron",
        ] {
            assert!(cache.join("missions").join(ron).is_file(), "missing {ron}");
        }
        assert_eq!(
            report.meta.validate(&cache, &before, &report.meta.replacement_key),
            extensions::mods::ModCacheStatus::Ready
        );

        let _ = std::fs::remove_dir_all(&cache);
    }
}
