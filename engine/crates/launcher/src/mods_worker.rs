//! Background mod discovery and versioned cache conversion worker for the launcher.
//!
//! Keeps potentially slow filesystem scanning, fingerprinting, and `bake mod-convert`
//! subprocesses off the egui thread while waking the UI cleanly as snapshots change.

use extensions::mods::{
    compute_mod_fingerprint, list_mod_folders, merge_optional_manifest, mod_cache_path,
    read_explicit_manifest, DiscoveredMod, ModCacheMeta, ModCacheStatus, ModCompatibility,
    ModContentType, ModFolderEntry, ModMapSummary, ModMissionSummary,
};
use mungefs::addon::{inspect_addon_dir, AddonCompatibility, AddonKind};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct DiscoveredModState {
    pub discovered: DiscoveredMod,
    pub cache_status: ModCacheStatus,
    pub progress_pct: u8,
    pub progress_msg: String,
}

#[derive(Clone, Debug, Default)]
pub struct ModsSnapshot {
    pub scanning: bool,
    pub generation: u64,
    pub mods: BTreeMap<String, DiscoveredModState>,
    pub folder_ids: Vec<String>,
    pub converting_id: Option<String>,
    pub scan_error: Option<String>,
    failed_fingerprints: BTreeMap<String, (String, String)>,
}

enum WorkerCmd {
    Scan {
        data_dir: PathBuf,
    },
    ConvertQueue {
        bake_bin: Option<PathBuf>,
        data_dir: PathBuf,
        mod_ids: Vec<String>,
        force_retry: bool,
    },
}

pub struct ModsWorker {
    tx: Sender<WorkerCmd>,
    state: Arc<Mutex<ModsSnapshot>>,
}

impl ModsWorker {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, rx) = mpsc::channel::<WorkerCmd>();
        let state = Arc::new(Mutex::new(ModsSnapshot::default()));
        let state_bg = Arc::clone(&state);
        std::thread::Builder::new()
            .name("launcher-mods-worker".into())
            .spawn(move || worker_loop(rx, state_bg, wake))
            .expect("spawn mods worker thread");
        Self { tx, state }
    }

    pub fn snapshot(&self) -> ModsSnapshot {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn trigger_scan(&self, data_dir: PathBuf) {
        if let Ok(mut s) = self.state.lock() {
            s.scanning = true;
        }
        let _ = self.tx.send(WorkerCmd::Scan { data_dir });
    }

    pub fn queue_conversions(
        &self,
        bake_bin: Option<PathBuf>,
        data_dir: PathBuf,
        ordered_enabled_ids: Vec<String>,
    ) {
        if ordered_enabled_ids.is_empty() {
            return;
        }
        let _ = self.tx.send(WorkerCmd::ConvertQueue {
            bake_bin,
            data_dir,
            mod_ids: ordered_enabled_ids,
            force_retry: false,
        });
    }

    pub fn retry_conversion(&self, bake_bin: Option<PathBuf>, data_dir: PathBuf, mod_id: String) {
        if let Ok(mut s) = self.state.lock() {
            s.failed_fingerprints.remove(&mod_id);
        }
        let _ = self.tx.send(WorkerCmd::ConvertQueue {
            bake_bin,
            data_dir,
            mod_ids: vec![mod_id],
            force_retry: true,
        });
    }
}

fn worker_loop(
    rx: mpsc::Receiver<WorkerCmd>,
    state: Arc<Mutex<ModsSnapshot>>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    while let Ok(first) = rx.recv() {
        let mut cmds = vec![first];
        while let Ok(more) = rx.try_recv() {
            cmds.push(more);
        }

        // Coalesce back-to-back scans so focus churn does not repeat work.
        let mut last_scan: Option<PathBuf> = None;
        let mut converts: Vec<(Option<PathBuf>, PathBuf, Vec<String>, bool)> = Vec::new();
        for cmd in cmds {
            match cmd {
                WorkerCmd::Scan { data_dir } => last_scan = Some(data_dir),
                WorkerCmd::ConvertQueue {
                    bake_bin,
                    data_dir,
                    mod_ids,
                    force_retry,
                } => converts.push((bake_bin, data_dir, mod_ids, force_retry)),
            }
        }

        if let Some(data_dir) = last_scan {
            run_scan(&data_dir, &state);
            wake();
        }
        for (bake_bin, data_dir, mod_ids, force_retry) in converts {
            run_convert_queue(
                bake_bin.as_deref(),
                &data_dir,
                &mod_ids,
                force_retry,
                &state,
                &wake,
            );
        }
    }
}

fn inspect_folder_entry(entry: &ModFolderEntry) -> DiscoveredMod {
    if let Some(err) = &entry.symlink_error {
        return DiscoveredMod {
            id: entry.mod_id.clone(),
            namespace: entry.namespace.clone(),
            folder_name: entry.folder_name.clone(),
            root_dir: entry.path.clone(),
            content_root: None,
            legacy_addon_id: None,
            display_name: entry.folder_name.clone(),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::Invalid,
            compatibility: ModCompatibility::Invalid(err.clone()),
            source_fingerprint: String::new(),
            maps: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            custom_sides: Vec::new(),
            warnings: Vec::new(),
            errors: vec![err.clone()],
            unused_lvls: Vec::new(),
        };
    }

    let mut insp = inspect_addon_dir(&entry.path);
    let fp = compute_mod_fingerprint(&entry.path).unwrap_or_else(|e| {
        insp.errors.push(format!("Failed to fingerprint mod: {e}"));
        extensions::mods::fingerprint::FingerprintResult {
            digest: String::new(),
            warnings: Vec::new(),
        }
    });
    for w in fp.warnings {
        if !insp.warnings.contains(&w) {
            insp.warnings.push(w);
        }
    }

    let mod_type = match insp.kind {
        AddonKind::LegacyMapAddon => ModContentType::LegacyMapAddon,
        AddonKind::LegacySideOverhaul => ModContentType::LegacySideOverhaul,
        AddonKind::LegacyAssetPack => ModContentType::LegacyAssetPack,
        AddonKind::LegacyHybridAddon => ModContentType::LegacyHybridAddon,
        AddonKind::Unsupported => ModContentType::Unsupported,
        AddonKind::Invalid => ModContentType::Invalid,
    };

    let compatibility = match &insp.compatibility {
        AddonCompatibility::Supported => ModCompatibility::Supported,
        AddonCompatibility::PartialWithWarnings => ModCompatibility::PartialWithWarnings,
        AddonCompatibility::Unsupported(s) => ModCompatibility::Unsupported(s.clone()),
        AddonCompatibility::Invalid(s) => ModCompatibility::Invalid(s.clone()),
    };

    let groups = insp.map_groups(&entry.namespace);
    let maps: Vec<ModMapSummary> = groups
        .iter()
        .map(|g| ModMapSummary {
            map_key: g.map_key.clone(),
            legacy_code: g.legacy_code.clone(),
            display_name: g.display_name.clone(),
            description: g.description.clone(),
            source_lvl: g.source_lvl_rel.clone(),
            modes_cw: g.modes_cw.clone(),
            modes_gcw: g.modes_gcw.clone(),
            missions: g
                .missions
                .iter()
                .map(|pm| ModMissionSummary {
                    script_name: pm.mission.script_name.clone(),
                    mode: pm.mission.mode.clone(),
                    era: pm.mission.era.clone(),
                    mission_ron: pm.mission_ron.clone(),
                    supported: pm.mission.supported,
                    unsupported_reason: pm.mission.unsupported_reason.clone(),
                })
                .collect(),
            sound_planets: g.sound_planets.clone(),
        })
        .collect();

    let mut custom_sides = BTreeSet::new();
    for m in &insp.missions {
        for (side, _) in &m.custom_side_lvls {
            custom_sides.insert(side.clone());
        }
    }
    let declared_overrides: Vec<String> = insp
        .stock_override_lvls
        .iter()
        .map(|o| o.rel_path.clone())
        .collect();

    let mut discovered = DiscoveredMod {
        id: entry.mod_id.clone(),
        namespace: entry.namespace.clone(),
        folder_name: entry.folder_name.clone(),
        root_dir: entry.path.clone(),
        content_root: insp.content_root.clone(),
        legacy_addon_id: insp.legacy_addon_id.clone(),
        display_name: insp.display_name,
        version: None,
        author: insp.author,
        description: insp.description,
        mod_type,
        compatibility,
        source_fingerprint: fp.digest,
        maps,
        dependencies: Vec::new(),
        conflicts: Vec::new(),
        declared_overrides,
        custom_sides: custom_sides.into_iter().collect(),
        warnings: insp.warnings,
        errors: insp.errors,
        unused_lvls: insp.unused_lvls,
    };
    if let Some(explicit) = read_explicit_manifest(&entry.path) {
        merge_optional_manifest(&mut discovered, explicit);
    }
    discovered
}

pub(crate) fn scan_mods_sync(
    data_dir: &Path,
) -> std::io::Result<(BTreeMap<String, DiscoveredMod>, Vec<String>)> {
    let mdir = extensions::profile::mods_dir(data_dir);
    let entries = list_mod_folders(&mdir)?;
    let mut mods = BTreeMap::new();
    let mut ids = Vec::with_capacity(entries.len());
    for entry in &entries {
        let info = inspect_folder_entry(entry);
        ids.push(info.id.clone());
        mods.insert(info.id.clone(), info);
    }
    Ok((mods, ids))
}

fn cache_status_for(
    info: &DiscoveredMod,
    data_dir: &Path,
    failed_fingerprints: &BTreeMap<String, (String, String)>,
) -> ModCacheStatus {
    if !info.compatibility.is_playable() {
        return ModCacheStatus::NotApplicable;
    }
    if let Some((fp, err)) = failed_fingerprints.get(&info.id) {
        if fp == &info.source_fingerprint {
            return ModCacheStatus::Failed(err.clone());
        }
    }
    let cache_dir = mod_cache_path(data_dir, &info.namespace);
    match ModCacheMeta::load(&cache_dir) {
        Some(meta) if meta.mod_id == info.id => meta.validate(&cache_dir, &info.source_fingerprint),
        Some(_) => ModCacheStatus::Stale {
            reason: "mod identity changed".into(),
        },
        None => ModCacheStatus::NeedsConversion,
    }
}

fn run_scan(data_dir: &Path, state: &Arc<Mutex<ModsSnapshot>>) {
    let res = scan_mods_sync(data_dir);
    let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
    snap.scanning = false;
    snap.generation = snap.generation.saturating_add(1);
    match res {
        Ok((discovered_map, folder_ids)) => {
            snap.scan_error = None;
            snap.folder_ids = folder_ids;
            let mut next_mods = BTreeMap::new();
            for (id, info) in discovered_map {
                let prev_converting = snap.converting_id.as_deref() == Some(id.as_str());
                let cache_status = if prev_converting {
                    ModCacheStatus::Converting
                } else {
                    cache_status_for(&info, data_dir, &snap.failed_fingerprints)
                };
                let (progress_pct, progress_msg) = snap
                    .mods
                    .get(&id)
                    .filter(|_| prev_converting)
                    .map_or((0, String::new()), |m| {
                        (m.progress_pct, m.progress_msg.clone())
                    });
                next_mods.insert(
                    id,
                    DiscoveredModState {
                        discovered: info,
                        cache_status,
                        progress_pct,
                        progress_msg,
                    },
                );
            }
            snap.mods = next_mods;
        }
        Err(e) => {
            snap.scan_error = Some(e.to_string());
        }
    }
}

fn run_convert_queue(
    bake_bin: Option<&Path>,
    data_dir: &Path,
    mod_ids: &[String],
    force_retry: bool,
    state: &Arc<Mutex<ModsSnapshot>>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    for mod_id in mod_ids {
        let task = {
            let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
            let ModsSnapshot {
                mods,
                converting_id,
                generation,
                failed_fingerprints,
                ..
            } = &mut *snap;
            if force_retry {
                failed_fingerprints.remove(mod_id);
            }
            let Some(item) = mods.get_mut(mod_id) else {
                continue;
            };
            if !item.discovered.compatibility.is_playable() {
                continue;
            }
            let status = cache_status_for(&item.discovered, data_dir, failed_fingerprints);
            let needs = matches!(
                status,
                ModCacheStatus::NeedsConversion | ModCacheStatus::Stale { .. }
            );
            if !needs {
                item.cache_status = status;
                continue;
            }
            let Some(bake) = bake_bin else {
                let err = "`bake` tool executable not found".to_owned();
                item.cache_status = ModCacheStatus::Failed(err.clone());
                let fp = item.discovered.source_fingerprint.clone();
                failed_fingerprints.insert(mod_id.clone(), (fp, err));
                *generation = generation.saturating_add(1);
                drop(snap);
                wake();
                continue;
            };
            item.cache_status = ModCacheStatus::Converting;
            item.progress_pct = 0;
            item.progress_msg = "Starting conversion...".into();
            let mod_dir = item.discovered.root_dir.clone();
            let ns = item.discovered.namespace.clone();
            let fp = item.discovered.source_fingerprint.clone();
            let cache_dir = mod_cache_path(data_dir, &ns);
            *converting_id = Some(mod_id.clone());
            *generation = generation.saturating_add(1);
            Some((bake.to_path_buf(), mod_dir, cache_dir, ns, fp))
        };

        let Some((bake, mod_dir, cache_dir, ns, fp)) = task else {
            continue;
        };
        wake();

        let result = run_bake_convert(&bake, &mod_dir, &cache_dir, mod_id, &ns, |pct, msg| {
            let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(item) = snap.mods.get_mut(mod_id) {
                item.progress_pct = pct;
                item.progress_msg = msg.to_owned();
            }
            wake();
        });

        let mut snap = state.lock().unwrap_or_else(|e| e.into_inner());
        snap.converting_id = None;
        snap.generation = snap.generation.saturating_add(1);
        match result {
            Ok(()) => {
                snap.failed_fingerprints.remove(mod_id);
                if let Some(item) = snap.mods.get_mut(mod_id) {
                    item.progress_pct = 100;
                    item.progress_msg = "Ready".into();
                    item.cache_status = match ModCacheMeta::load(&cache_dir) {
                        Some(meta) => meta.validate(&cache_dir, &fp),
                        None => ModCacheStatus::Failed("missing cache metadata".into()),
                    };
                }
            }
            Err(err) => {
                let err_str = err.to_string();
                snap.failed_fingerprints
                    .insert(mod_id.clone(), (fp, err_str.clone()));
                if let Some(item) = snap.mods.get_mut(mod_id) {
                    item.cache_status = ModCacheStatus::Failed(err_str);
                }
            }
        }
        drop(snap);
        wake();
    }
}

fn run_bake_convert(
    bake: &Path,
    mod_dir: &Path,
    cache_dir: &Path,
    mod_id: &str,
    namespace: &str,
    mut on_progress: impl FnMut(u8, &str),
) -> anyhow::Result<()> {
    let mut cmd = Command::new(bake);
    cmd.arg("mod-convert")
        .arg(mod_dir)
        .arg(cache_dir)
        .arg("--mod-id")
        .arg(mod_id)
        .arg("--namespace")
        .arg(namespace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("launching `{}`: {e}", bake.display()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stderr_handle = std::thread::spawn(move || {
        stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    });

    if let Some(out) = stdout {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix("PROGRESS ") {
                if let Some((pct_str, msg)) = rest.split_once(' ') {
                    if let Ok(pct) = pct_str.parse::<u8>() {
                        on_progress(pct, msg.trim());
                    }
                }
            }
        }
    }

    let status = child.wait()?;
    let err_txt = stderr_handle.join().unwrap_or_default();
    if !status.success() {
        anyhow::bail!(
            "exit {:?}{}",
            status.code(),
            if err_txt.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", err_txt.trim())
            }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_duplicate_legacy_ids_and_detects_added_and_removed_folders() {
        let root = std::env::temp_dir().join(format!("openbf-worker-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mods = extensions::profile::mods_dir(&root);
        std::fs::create_dir_all(mods.join("BadMod")).unwrap();
        std::fs::write(mods.join("BadMod/mod.dll"), b"MZ").unwrap();

        let (map1, ids1) = scan_mods_sync(&root).unwrap();
        assert_eq!(ids1, ["content.badmod"]);
        assert_eq!(map1["content.badmod"].mod_type, ModContentType::Unsupported);

        // Two folders sharing legacy layout id get independent stable mod IDs/namespaces.
        for folder in ["ABR_One", "ABR_Two"] {
            let side = mods.join(folder).join("data/_LVL_PC/SIDE");
            std::fs::create_dir_all(&side).unwrap();
            std::fs::write(side.join("all.lvl"), b"ucfb_stub").unwrap();
        }
        let (map2, ids2) = scan_mods_sync(&root).unwrap();
        assert_eq!(ids2.len(), 3);
        assert_ne!(
            map2["content.abr_one"].namespace,
            map2["content.abr_two"].namespace
        );

        std::fs::remove_dir_all(mods.join("ABR_One")).unwrap();
        let (_map3, ids3) = scan_mods_sync(&root).unwrap();
        assert_eq!(ids3.len(), 2);
        assert!(!ids3.contains(&"content.abr_one".to_owned()));

        let _ = std::fs::remove_dir_all(&root);
    }
}
