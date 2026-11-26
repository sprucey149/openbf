//! Atomic legacy BF2 addon converter -> versioned `openbf_data/mod_cache/<ns>/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use extensions::mods::{
    ModCacheMeta, ModMapSummary, ModMissionSummary, MOD_CACHE_SCHEMA_VERSION, MOD_CONVERTER_ID,
};
use extensions::mods::catalog::{SourceGame, SourcePlatform};
use mungefs::addon::read_core_locl;

use super::mission::mission_to_ron;
use super::{inspect_mod_folder, ModEnv};
use super::world::{link_tree, read_world, SharedWorldFiles};

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
    let mut shared_world_files = SharedWorldFiles::default();
    let mut sound_bakes: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();

    let total_steps = groups.len().max(1);
    for (gi, g) in groups.iter().enumerate() {
        let base_pct = 15 + ((gi * 65) / total_steps) as u8;
        progress(
            opts.emit_progress,
            base_pct,
            &format!("Converting map `{}`...", g.display_name),
        );

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

        let map_out = staging.join("assets").join(&g.map_key);
        std::fs::create_dir_all(&map_out)?;
        crate::gameplay::exempt_addon(map_lvl);
        let map_lvl_str = map_lvl.to_string_lossy();
        let map_out_str = map_out.to_string_lossy();

        let baked = if g.world_loads.is_empty() {
            crate::map::bake_map(&map_lvl_str, &map_out_str, None)
                .with_context(|| format!("baking map LVL {}", map_lvl.display()))
        } else {
            read_world(&g.world_loads, map_lvl)
                .with_context(|| format!("reading the archive selection for {}", g.display_name))
                .and_then(|world| {
                    crate::map::bake_addon_map(
                        &map_lvl_str,
                        &map_out_str,
                        crate::map::AddonMapInput {
                            bytes: &world.bytes,
                            selectors: &world.selectors,
                        },
                    )
                    .with_context(|| format!("baking selected world {}", g.display_name))
                })
        };
        // One map that cannot be built leaves the others playable.
        if let Err(e) = baked {
            std::fs::remove_dir_all(&map_out)?;
            warnings.push(format!(
                "Map `{}` could not be converted ({e:#}).",
                g.display_name
            ));
            first_failed_map.get_or_insert(e);
            continue;
        }
        maps_baked += 1;

        if let Some(load_lvl) = &g.loadscreen_lvl_path {
            crate::gameplay::exempt_addon(load_lvl);
            if let Err(e) =
                crate::loadscreen::bake_loadscreen(&load_lvl.to_string_lossy(), &map_out_str)
            {
                warnings.push(format!(
                    "Load screen `{}` could not be converted ({e:#}); using fallback backdrop.",
                    load_lvl.display()
                ));
            }
        }

        if !g.world_loads.is_empty() {
            shared_world_files.share(&map_out)?;
        }

        for (side, side_lvl) in &g.custom_side_lvls {
            crate::gameplay::exempt_addon(side_lvl);
            let side_dir = staging
                .join("assets")
                .join(format!("{}_side_{side}", opts.namespace));
            std::fs::create_dir_all(&side_dir)?;
            if let Err(e) = crate::classes::bake_classes(
                &side_lvl.to_string_lossy(),
                &side_dir.join("classes.ron").to_string_lossy(),
            ) {
                warnings.push(format!("Custom SIDE `{side}.lvl` partial skip: {e:#}"));
            }
        }

        if let Some((snd_name, snd_lvl)) = g.custom_sound_lvls.first() {
            let sound_out = map_out.join("sounds");
            if let Some(previous) = sound_bakes.get(snd_lvl) {
                link_tree(previous, &sound_out)?;
            } else {
                let core = insp.core_lvl.as_deref().map(Path::to_string_lossy);
                match crate::sounds::bake_addon_sounds(
                    &snd_lvl.to_string_lossy(),
                    &map_out_str,
                    core.as_deref(),
                ) {
                    Ok(_) => {
                        sound_bakes.insert(snd_lvl.clone(), sound_out);
                    }
                    Err(e) => {
                        if sound_out.is_dir() {
                            std::fs::remove_dir_all(&sound_out)?;
                        }
                        warnings.push(format!(
                            "Custom sound archive `{snd_name}` could not be converted ({e:#})."
                        ));
                    }
                }
            }
            for (extra, _) in g.custom_sound_lvls.iter().skip(1) {
                warnings.push(format!(
                    "Additional custom sound archive `{extra}` is not supported for this map."
                ));
            }
        }

        let mut planned_missions = Vec::with_capacity(g.missions.len());
        let missions_dir = staging.join("missions");
        std::fs::create_dir_all(&missions_dir)?;

        for pm in &g.missions {
            let mut supported = pm.mission.supported;
            let mut reason = pm.mission.unsupported_reason.clone();
            if supported {
                match mission_to_ron(&g.map_key, pm.mission) {
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
            modes_cw: g.modes_cw.clone(),
            modes_gcw: g.modes_gcw.clone(),
            missions: planned_missions,
            sound_planets: g.sound_planets.clone(),
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

    // Stock replacements: every active resource operation, translated onto
    // the stock bake's own outputs (`super::replace`).
    let replaced = if discovered.active_ops().next().is_some() {
        let (pct_start, pct_end) = if groups.is_empty() { (10u8, 95u8) } else { (82u8, 95u8) };
        progress(opts.emit_progress, pct_start, "Converting stock replacements...");
        let abs: BTreeMap<String, PathBuf> = insp
            .stock_override_lvls
            .iter()
            .map(|o| (o.rel_path.to_ascii_lowercase().replace('\\', "/"), o.abs_path.clone()))
            .collect();
        super::replace::convert_replacements(
            &discovered.resource_ops,
            &abs,
            opts.env,
            &staging.join("overrides").join("assets"),
            &staging.join("scratch"),
            (pct_start, pct_end),
            &mut |pct, msg| progress(opts.emit_progress, pct, msg),
        )?
    } else {
        super::replace::Replaced::default()
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
    };
    meta.save_atomic(&staging)?;

    // Atomic cache directory commit (with backup swap on Windows).
    swap_dir(&staging, opts.cache_dir)
        .with_context(|| format!("committing cache {}", opts.cache_dir.display()))?;
    guard.0 = None;
    progress(opts.emit_progress, 100, "Done");

    Ok(ConvertReport {
        meta,
        maps_baked,
        missions_written,
    })
}

fn swap_dir(staging: &Path, target: &Path) -> std::io::Result<()> {
    if !target.exists() {
        return std::fs::rename(staging, target);
    }
    let backup = target.with_extension(format!("old.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::rename(target, &backup)?;
    match std::fs::rename(staging, target) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&backup);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::rename(&backup, target);
            Err(e)
        }
    }
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
