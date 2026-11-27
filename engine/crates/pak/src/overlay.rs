//! Virtual overlay stack for namespaced content mods and priority-ordered
//! explicit resource overrides over immutable stock mounts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlayRule {
    /// Dedicated directory namespace (e.g. `assets_baked/mod_abr__abr/...`).
    NamespaceDir { prefix: String, disk_dir: PathBuf },
    /// Dedicated filename prefix directly inside a mount (e.g. `missions/mod_abr__*.ron`).
    NamespacePrefix { prefix: String, disk_dir: PathBuf },
    /// Explicit stock/shared resource override: maps canonical lowercase
    /// relative paths to `Some(actual_disk_path)` (preserving the real
    /// filesystem casing on case-sensitive OSes) or `None` when the override
    /// explicitly removes/hides the stock file. A layer with a `scope`
    /// (`"bf1"` / `"bf2"`) serves only while [`set_overlay_scope`] names that
    /// game: a BF2 side replacement must not reach a BF1 battle that borrows
    /// BF2's weapon kit.
    Overrides {
        entries: BTreeMap<String, Option<PathBuf>>,
        scope: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayLayer {
    pub id: String,
    pub mount_name: String,
    pub rule: OverlayRule,
}

static OVERLAYS: RwLock<Vec<OverlayLayer>> = RwLock::new(Vec::new());
static SCOPE: RwLock<Option<String>> = RwLock::new(None);

/// The game whose battle is loading (`"bf1"` / `"bf2"`), or `None` outside a
/// battle. Scoped override layers serve only their own game; with no scope
/// set they serve everything, which is what the menus see.
pub fn set_overlay_scope(game: Option<&str>) {
    *SCOPE.write().unwrap_or_else(|e| e.into_inner()) = game.map(str::to_ascii_lowercase);
}

pub fn overlay_scope() -> Option<String> {
    SCOPE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

fn in_scope(scope: &Option<String>) -> bool {
    match (scope, &*SCOPE.read().unwrap_or_else(|e| e.into_inner())) {
        (Some(layer), Some(active)) => layer == active,
        _ => true,
    }
}

/// Replace active overlay layers atomically (index 0 = highest priority).
pub fn set_overlays(layers: Vec<OverlayLayer>) {
    let mut guard = OVERLAYS.write().unwrap_or_else(|e| e.into_inner());
    *guard = layers;
}

/// Remove all active overlay layers, restoring stock-only reads.
pub fn clear_overlays() {
    set_overlays(Vec::new());
}

/// Snapshot current active overlay layers.
pub fn current_overlays() -> Vec<OverlayLayer> {
    OVERLAYS.read().unwrap_or_else(|e| e.into_inner()).clone()
}

fn is_safe_rel(rel: &str) -> bool {
    !rel.contains(':')
        && !rel.starts_with('/')
        && !rel.starts_with('\\')
        && !Path::new(rel).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn join_rel(base: &Path, rel: &str) -> Option<PathBuf> {
    if !is_safe_rel(rel) {
        return None;
    }
    if rel.is_empty() {
        return Some(base.to_path_buf());
    }
    let mut out = base.to_path_buf();
    for seg in rel.split('/') {
        if !seg.is_empty() && seg != "." {
            out.push(seg);
        }
    }
    Some(out)
}

/// What the overlays make of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resolved {
    /// Served from this file on disk.
    File(PathBuf),
    /// An override layer claims the path but has no file for it: the stock
    /// file is hidden (a replaced soldier whose mod model has no cloth must
    /// not pick up the stock model's cloth).
    Removed,
}

/// Resolve an overlay file path for `(mount_name, rel_key)`.
/// Order: private namespaces first (never shadowed by unrelated mods), then
/// explicit overrides in load-order priority (`index 0` wins).
pub(crate) fn resolve_file(mount_name: &str, rel_key: &str) -> Option<Resolved> {
    if rel_key.is_empty() || !is_safe_rel(rel_key) {
        return None;
    }
    let layers = OVERLAYS.read().unwrap_or_else(|e| e.into_inner());

    // 1. Namespaced files never compete across unrelated mods.
    for layer in layers.iter() {
        if layer.mount_name != mount_name {
            continue;
        }
        match &layer.rule {
            OverlayRule::NamespaceDir { prefix, disk_dir } => {
                let p = prefix.trim_end_matches('/');
                if let Some(rest) = rel_key.strip_prefix(p).and_then(|s| s.strip_prefix('/')) {
                    if let Some(cand) = join_rel(disk_dir, rest) {
                        if cand.is_file() {
                            return Some(Resolved::File(cand));
                        }
                    }
                }
            }
            OverlayRule::NamespacePrefix { prefix, disk_dir } => {
                if !rel_key.contains('/') && rel_key.starts_with(prefix) {
                    if let Some(cand) = join_rel(disk_dir, rel_key) {
                        if cand.is_file() {
                            return Some(Resolved::File(cand));
                        }
                    }
                }
            }
            OverlayRule::Overrides { .. } => {}
        }
    }

    // 2. Explicit overrides follow priority order (index 0 first).
    let lower_key = rel_key.to_ascii_lowercase();
    for layer in layers.iter() {
        if layer.mount_name != mount_name {
            continue;
        }
        if let OverlayRule::Overrides { entries, scope } = &layer.rule {
            if in_scope(scope) {
                if let Some(entry) = entries.get(&lower_key) {
                    return match entry {
                        Some(path) if path.is_file() => Some(Resolved::File(path.clone())),
                        Some(_) => None,
                        None => Some(Resolved::Removed),
                    };
                }
            }
        }
    }

    None
}

/// All in-scope override files for `(mount_name, rel_key)` in priority order
/// (index 0 = highest priority). Stops at any layer that explicitly removes
/// the path (`None`), so lower-priority layers cannot resurface removed files.
pub(crate) fn resolve_override_stack(mount_name: &str, rel_key: &str) -> Vec<PathBuf> {
    if rel_key.is_empty() || !is_safe_rel(rel_key) {
        return Vec::new();
    }
    let layers = OVERLAYS.read().unwrap_or_else(|e| e.into_inner());
    let lower_key = rel_key.to_ascii_lowercase();
    let mut out = Vec::new();
    for layer in layers.iter() {
        if layer.mount_name != mount_name {
            continue;
        }
        if let OverlayRule::Overrides { entries, scope } = &layer.rule {
            if in_scope(scope) {
                if let Some(entry) = entries.get(&lower_key) {
                    match entry {
                        Some(path) if path.is_file() => out.push(path.clone()),
                        Some(_) => {}
                        None => break,
                    }
                }
            }
        }
    }
    out
}

/// True if an active overlay layer provides directory `(mount_name, rel_key)`.
pub(crate) fn has_dir(mount_name: &str, rel_key: &str) -> bool {
    if !is_safe_rel(rel_key) {
        return false;
    }
    let layers = OVERLAYS.read().unwrap_or_else(|e| e.into_inner());
    let lower_key = rel_key.trim_end_matches('/').to_ascii_lowercase();
    let lower_prefix = if lower_key.is_empty() {
        String::new()
    } else {
        format!("{lower_key}/")
    };

    for layer in layers.iter() {
        if layer.mount_name != mount_name {
            continue;
        }
        match &layer.rule {
            OverlayRule::NamespaceDir { prefix, disk_dir } => {
                let p = prefix.trim_end_matches('/');
                if rel_key.is_empty() && disk_dir.is_dir() {
                    return true;
                }
                if rel_key == p && disk_dir.is_dir() {
                    return true;
                }
                if let Some(rest) = rel_key.strip_prefix(p).and_then(|s| s.strip_prefix('/')) {
                    if let Some(cand) = join_rel(disk_dir, rest) {
                        if cand.is_dir() {
                            return true;
                        }
                    }
                }
            }
            OverlayRule::NamespacePrefix { disk_dir, .. } => {
                if rel_key.is_empty() && disk_dir.is_dir() {
                    return true;
                }
            }
            OverlayRule::Overrides { entries, scope } => {
                if in_scope(scope)
                    && !lower_key.is_empty()
                    && entries
                        .iter()
                        .any(|(ap, p)| p.is_some() && ap.starts_with(&lower_prefix))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Direct child names contributed by matching overlays for directory `(mount_name, rel_key)`.
pub(crate) fn children_for(mount_name: &str, rel_key: &str) -> Vec<String> {
    if !is_safe_rel(rel_key) {
        return Vec::new();
    }
    let mut names = BTreeSet::new();
    let layers = OVERLAYS.read().unwrap_or_else(|e| e.into_inner());
    let key = rel_key.trim_end_matches('/');
    let lower_prefix = if key.is_empty() {
        String::new()
    } else {
        format!("{}/", key.to_ascii_lowercase())
    };

    for layer in layers.iter() {
        if layer.mount_name != mount_name {
            continue;
        }
        match &layer.rule {
            OverlayRule::NamespaceDir { prefix, disk_dir } => {
                let p = prefix.trim_end_matches('/');
                if key.is_empty() {
                    if disk_dir.is_dir() {
                        names.insert(p.to_owned());
                    }
                } else if key == p {
                    if let Ok(rd) = std::fs::read_dir(disk_dir) {
                        for e in rd.flatten() {
                            if let Some(s) = e.file_name().to_str() {
                                names.insert(s.to_owned());
                            }
                        }
                    }
                } else if let Some(rest) = key.strip_prefix(p).and_then(|s| s.strip_prefix('/')) {
                    if let Some(cand) = join_rel(disk_dir, rest) {
                        if let Ok(rd) = std::fs::read_dir(cand) {
                            for e in rd.flatten() {
                                if let Some(s) = e.file_name().to_str() {
                                    names.insert(s.to_owned());
                                }
                            }
                        }
                    }
                }
            }
            OverlayRule::NamespacePrefix { prefix, disk_dir } => {
                if key.is_empty() {
                    if let Ok(rd) = std::fs::read_dir(disk_dir) {
                        for e in rd.flatten() {
                            if let Some(s) = e.file_name().to_str() {
                                if s.starts_with(prefix) {
                                    names.insert(s.to_owned());
                                }
                            }
                        }
                    }
                }
            }
            OverlayRule::Overrides { entries, scope } => {
                if !in_scope(scope) {
                    continue;
                }
                for (ap, path) in entries {
                    if path.is_none() {
                        continue;
                    }
                    let tail = if lower_prefix.is_empty() {
                        ap.as_str()
                    } else if let Some(rest) = ap.strip_prefix(&lower_prefix) {
                        rest
                    } else {
                        continue;
                    };
                    if let Some(child) = tail.split('/').next().filter(|s| !s.is_empty()) {
                        names.insert(child.to_owned());
                    }
                }
            }
        }
    }

    names.into_iter().collect()
}
