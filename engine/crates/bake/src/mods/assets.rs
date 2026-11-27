//! The slow steps of a mod conversion (each map's world, the maps' own units,
//! the stock replacements), and their reuse from the mod's previous cache.
//!
//! A step's [`StepKey`] names everything its output is made from. When the
//! previous cache has the same key for a step, its output folder moves into
//! the new cache instead of being baked again. A folder that no longer holds
//! the files and bytes the step left in it is baked again.
//!
//! The key holds `MOD_ASSET_ID`, a hash of the converter's source without the
//! mission and menu logic (`extensions/build.rs`). Every byte below a reused
//! folder must therefore come from code in that hash: nothing in `convert.rs`
//! or `mission.rs`, which are left out of it, may write into these folders.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use extensions::mods::{DiscoveredMod, ReplacementOutput, MOD_ASSET_ID};
use mungefs::addon::AddonWorldLoad;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::replace::Replaced;
use super::world::{link_tree, read_world, SharedWorldFiles};
use super::ModEnv;

/// The record of a cache's asset steps, at its root.
const RECORD_FILE: &str = "asset_steps.ron";

/// Everything a step's output is made from, hashed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct StepKey(String);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StepRecord {
    key: StepKey,
    /// The folder the step wrote, relative to the cache root, `/`-separated.
    dir: String,
    /// [`tally`] of `dir` as the step left it.
    holds: (u64, u64),
    warnings: Vec<String>,
    #[serde(default)]
    unsupported: Vec<String>,
    #[serde(default)]
    outputs: Vec<ReplacementOutput>,
}

/// The previous cache's steps, and the ones this conversion records.
pub(super) struct Reuse {
    old_cache: PathBuf,
    old: BTreeMap<String, StepRecord>,
    new: BTreeMap<String, StepRecord>,
    /// The folders to move from the previous cache, relative to its root.
    adopt: Vec<String>,
    /// What every key starts from; `None` when the stock data cannot be
    /// identified, and then nothing is reused or recorded.
    base: Option<String>,
}

impl Reuse {
    pub(super) fn open(old_cache: &Path, discovered: &DiscoveredMod, env: &ModEnv) -> Self {
        // Beside the code: the mod's files (every file a step reads of the
        // mod's), the stock bake and catalog, the stock copies and the
        // player's target choices. OURS: a stock game copy is taken to stay
        // as it is while its path does. measure: hash the stock files a step
        // reads, were a copy ever edited in place.
        let base = env.stock_identity.as_ref().map(|stock| {
            format!(
                "{MOD_ASSET_ID}\n{}\n{}\n{stock}\n{}\n{:?}\n{:?}",
                discovered.id,
                discovered.source_fingerprint,
                env.catalogs.digest,
                env.sources,
                env.selections.for_mod(&discovered.id),
            )
        });
        let old = std::fs::read_to_string(old_cache.join(RECORD_FILE))
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            old_cache: old_cache.to_owned(),
            old,
            new: BTreeMap::new(),
            adopt: Vec::new(),
            base,
        }
    }

    /// The key of a step with these settings of its own.
    pub(super) fn key(&self, settings: &str) -> Option<StepKey> {
        let base = self.base.as_ref()?;
        let mut h = Sha256::new();
        h.update(base.as_bytes());
        h.update([0]);
        h.update(settings.as_bytes());
        Some(StepKey(
            h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        ))
    }

    /// Take `step` from the previous cache when it was made from the same
    /// things: what it reported, with its folder due to move at
    /// [`Self::commit`].
    pub(super) fn take(&mut self, step: &str, key: Option<&StepKey>) -> Option<Replaced> {
        let record = self.old.get(step)?;
        if Some(&record.key) != key
            || tally(&self.old_cache.join(&record.dir)).ok() != Some(record.holds)
        {
            return None;
        }
        let record = record.clone();
        let report = Replaced {
            outputs: record.outputs.clone(),
            unsupported: record.unsupported.clone(),
            warnings: record.warnings.clone(),
        };
        self.adopt.push(record.dir.clone());
        self.new.insert(step.to_owned(), record);
        Some(report)
    }

    /// Record a step this conversion baked into `staging/dir`.
    pub(super) fn record(
        &mut self,
        step: &str,
        key: Option<StepKey>,
        staging: &Path,
        dir: String,
        report: &Replaced,
    ) {
        let Some(key) = key else { return };
        // A folder that cannot be read through is only not reused.
        if let Ok(holds) = tally(&staging.join(&dir)) {
            self.new.insert(
                step.to_owned(),
                StepRecord {
                    key,
                    dir,
                    holds,
                    warnings: report.warnings.clone(),
                    unsupported: report.unsupported.clone(),
                    outputs: report.outputs.clone(),
                },
            );
        }
    }

    /// Finish `staging` (write the record, move the reused folders in) and put
    /// it in the place of the previous cache. Nothing of the previous cache is
    /// touched before everything else is made, and a failed swap puts its
    /// folders back.
    pub(super) fn commit(self, staging: &Path) -> Result<()> {
        crate::write_ron(&staging.join(RECORD_FILE), &self.new)?;
        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        let done = (|| -> Result<()> {
            for dir in &self.adopt {
                let from = self.old_cache.join(dir);
                let to = staging.join(dir);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                rename_when_free(&from, &to)
                    .with_context(|| format!("keeping {}", from.display()))?;
                moved.push((from, to));
            }
            swap_dir(staging, &self.old_cache)
                .with_context(|| format!("committing cache {}", self.old_cache.display()))
        })();
        if done.is_err() {
            for (from, to) in moved.into_iter().rev() {
                let _ = rename_when_free(&to, &from);
            }
        }
        done
    }
}

/// The number of files below `dir`, and their bytes.
fn tally(dir: &Path) -> std::io::Result<(u64, u64)> {
    let mut total = (0, 0);
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let (files, bytes) = if meta.is_dir() {
            tally(&entry.path())?
        } else {
            (1, meta.len())
        };
        total = (total.0 + files, total.1 + bytes);
    }
    Ok(total)
}

/// OURS: how long [`rename_when_free`] waits. replace: a wait on the handle
/// itself.
const RENAME_WAIT: Duration = Duration::from_secs(60);

/// Rename a cache folder, waiting out a refusal: Windows refuses while any
/// process (a virus scanner, the search indexer) holds a file below it open,
/// and giving up throws the whole conversion away.
fn rename_when_free(from: &Path, to: &Path) -> std::io::Result<()> {
    let started = Instant::now();
    loop {
        match std::fs::rename(from, to) {
            Err(e)
                // 32: Windows' ERROR_SHARING_VIOLATION, the same cause.
                if (e.kind() == std::io::ErrorKind::PermissionDenied
                    || cfg!(windows) && e.raw_os_error() == Some(32))
                    && started.elapsed() < RENAME_WAIT =>
            {
                std::thread::sleep(Duration::from_millis(500));
            }
            done => return done,
        }
    }
}

fn swap_dir(staging: &Path, target: &Path) -> std::io::Result<()> {
    if !target.exists() {
        return rename_when_free(staging, target);
    }
    let backup = target.with_extension(format!("old.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&backup);
    rename_when_free(target, &backup)?;
    match rename_when_free(staging, target) {
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

/// What one map's folder is baked from.
#[derive(Debug)]
pub(super) struct MapJob<'a> {
    pub map_key: &'a str,
    pub map_lvl: &'a Path,
    pub world_loads: &'a [AddonWorldLoad],
    /// Whether the mission reads a world of its own choosing, whose files
    /// other variants may share.
    pub shares_files: bool,
    pub loadscreen: Option<&'a Path>,
    pub sounds: &'a [(String, PathBuf)],
    pub core: Option<&'a Path>,
}

/// What the maps of one conversion share.
#[derive(Default)]
pub(super) struct MapShared {
    files: SharedWorldFiles,
    sounds: BTreeMap<PathBuf, PathBuf>,
}

/// Bake one map into `map_out`: its world, load screen and sound archive.
/// `Err` when the world cannot be built; what else fails is a warning.
pub(super) fn bake_map_assets(
    job: &MapJob<'_>,
    map_out: &Path,
    shared: &mut MapShared,
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let map_lvl_str = job.map_lvl.to_string_lossy();
    let map_out_str = map_out.to_string_lossy();

    if job.world_loads.is_empty() {
        crate::map::bake_map(&map_lvl_str, &map_out_str, None)
            .with_context(|| format!("baking map LVL {}", job.map_lvl.display()))?;
    } else {
        let world = read_world(job.world_loads, job.map_lvl)
            .with_context(|| format!("reading the archive selection for {}", job.map_key))?;
        crate::map::bake_addon_map(
            &map_lvl_str,
            &map_out_str,
            crate::map::AddonMapInput {
                bytes: &world.bytes,
                selectors: &world.selectors,
            },
        )
        .with_context(|| format!("baking selected world {}", job.map_key))?;
    }

    if let Some(load_lvl) = job.loadscreen {
        if let Err(e) =
            crate::loadscreen::bake_loadscreen(&load_lvl.to_string_lossy(), &map_out_str)
        {
            warnings.push(format!(
                "Load screen `{}` could not be converted ({e:#}); using fallback backdrop.",
                load_lvl.display()
            ));
        }
    }

    if job.shares_files {
        shared.files.share(map_out)?;
    }

    if let Some((snd_name, snd_lvl)) = job.sounds.first() {
        let sound_out = map_out.join("sounds");
        if let Some(previous) = shared.sounds.get(snd_lvl) {
            link_tree(previous, &sound_out)?;
        } else {
            let core = job.core.map(Path::to_string_lossy);
            match crate::sounds::bake_addon_sounds(
                &snd_lvl.to_string_lossy(),
                &map_out_str,
                core.as_deref(),
            ) {
                Ok(_) => {
                    shared.sounds.insert(snd_lvl.clone(), sound_out);
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
        for (extra, _) in job.sounds.iter().skip(1) {
            warnings.push(format!(
                "Additional custom sound archive `{extra}` is not supported for this map."
            ));
        }
    }
    Ok(warnings)
}

/// The settings of the units step: what every map asks of its sides, and the
/// names the mod's scripts may load a unit by.
pub(super) fn units_settings(
    levels: &[super::addon_sides::LevelSides],
    script_names: &BTreeSet<String>,
) -> String {
    format!("{levels:?}\n{script_names:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reuse(cache: &Path) -> Reuse {
        let old = std::fs::read_to_string(cache.join(RECORD_FILE))
            .ok()
            .map(|t| ron::from_str(&t).unwrap())
            .unwrap_or_default();
        Reuse {
            old_cache: cache.to_owned(),
            old,
            new: BTreeMap::new(),
            adopt: Vec::new(),
            base: Some("base".into()),
        }
    }

    fn write(path: PathBuf, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A cache holding one recorded step, `map:a` in `assets/a`.
    fn first_cache(root: &Path) -> PathBuf {
        let cache = root.join("cache");
        let staging = root.join("first");
        let mut first = reuse(&cache);
        write(staging.join("assets/a/world"), "baked once");
        let report = Replaced {
            warnings: vec!["said once".into()],
            ..Default::default()
        };
        let key = first.key("a");
        first.record("map:a", key, &staging, "assets/a".into(), &report);
        first.commit(&staging).unwrap();
        cache
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openbf-reuse-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_unchanged_step_moves_into_the_new_cache() {
        let root = temp("kept");
        let cache = first_cache(&root);
        let mut next = reuse(&cache);
        let other = next.key("b");
        assert!(next.take("map:a", other.as_ref()).is_none());
        assert!(next.take("map:a", None).is_none());
        let key = next.key("a");
        let kept = next.take("map:a", key.as_ref()).unwrap();
        assert_eq!(kept.warnings, ["said once"]);
        let staging = root.join("next");
        std::fs::create_dir_all(&staging).unwrap();
        next.commit(&staging).unwrap();
        let world = std::fs::read_to_string(cache.join("assets/a/world")).unwrap();
        assert_eq!(world, "baked once");
        // And again from the cache that kept it.
        let mut third = reuse(&cache);
        assert!(third.take("map:a", key.as_ref()).is_some());
        std::fs::remove_file(cache.join("assets/a/world")).unwrap();
        assert!(reuse(&cache).take("map:a", key.as_ref()).is_none());
        std::fs::remove_dir_all(cache.join("assets/a")).unwrap();
        assert!(reuse(&cache).take("map:a", key.as_ref()).is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failed_commit_leaves_the_old_cache_whole() {
        let root = temp("failed");
        let cache = first_cache(&root);
        write(cache.join("assets/b/world"), "baked too");
        let mut next = reuse(&cache);
        let key = next.key("a");
        next.take("map:a", key.as_ref()).unwrap();
        // The second folder cannot move: something is already in its place.
        let staging = root.join("next");
        write(staging.join("assets/b/world"), "in the way");
        next.adopt.push("assets/b".into());
        assert!(next.commit(&staging).is_err());
        for (file, text) in [("assets/a/world", "baked once"), ("assets/b/world", "baked too")] {
            assert_eq!(std::fs::read_to_string(cache.join(file)).unwrap(), text);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
