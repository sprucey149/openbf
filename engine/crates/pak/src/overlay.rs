//! Virtual overlay stack for namespaced content mods and priority-ordered
//! explicit resource overrides over immutable stock mounts.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlayRule {
    /// Dedicated directory namespace (e.g. `assets_baked/mod_abr__abr/...`).
    NamespaceDir { prefix: String, disk_dir: PathBuf },
    /// Dedicated filename prefix directly inside a mount (e.g. `missions/mod_abr__*.ron`).
    NamespacePrefix { prefix: String, disk_dir: PathBuf },
    /// Explicit stock/shared resource override: only relative paths in
    /// `allowed_paths` participate in load-order precedence.
    Overrides {
        allowed_paths: BTreeSet<String>,
        disk_dir: PathBuf,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayLayer {
    pub id: String,
    pub mount_name: String,
    pub rule: OverlayRule,
}

static OVERLAYS: RwLock<Vec<OverlayLayer>> = RwLock::new(Vec::new());

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

/// Resolve an overlay file path for `(mount_name, rel_key)`.
/// Order: private namespaces first (never shadowed by unrelated mods), then
/// explicit overrides in load-order priority (`index 0` wins).
pub(crate) fn resolve_file(mount_name: &str, rel_key: &str) -> Option<PathBuf> {
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
                            return Some(cand);
                        }
                    }
                }
            }
            OverlayRule::NamespacePrefix { prefix, disk_dir } => {
                if !rel_key.contains('/') && rel_key.starts_with(prefix) {
                    if let Some(cand) = join_rel(disk_dir, rel_key) {
                        if cand.is_file() {
                            return Some(cand);
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
        if let OverlayRule::Overrides {
            allowed_paths,
            disk_dir,
        } = &layer.rule
        {
            if allowed_paths.contains(&lower_key) {
                if let Some(cand) = join_rel(disk_dir, rel_key) {
                    if cand.is_file() {
                        return Some(cand);
                    }
                }
            }
        }
    }

    None
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
            OverlayRule::Overrides { allowed_paths, .. } => {
                if !lower_key.is_empty()
                    && allowed_paths.iter().any(|ap| ap.starts_with(&lower_prefix))
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
            OverlayRule::Overrides { allowed_paths, .. } => {
                for ap in allowed_paths {
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
