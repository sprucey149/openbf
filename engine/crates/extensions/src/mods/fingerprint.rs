//! Bounded, traversal-safe source fingerprinting for legacy mod folders.
//!
//! The fingerprint is a SHA-256 over every file's relative path, length and
//! own SHA-256. A [`FileHashCache`] keeps each file's hash by (path, length,
//! modification time), so the launcher re-checking a 3.6 GB side mod on every
//! focus change reads nothing it has read before.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_FILES: usize = 10_000;
/// A stock side LVL reaches 1.03 GB (BF2 PC `side/rep.lvl`); a mod's
/// replacement of it can be larger.
const MAX_SINGLE_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const READ_BUF_BYTES: usize = 1024 * 1024;

pub const FILE_HASH_CACHE: &str = "cache/file_hashes.ron";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct HashEntry {
    len: u64,
    mtime_ns: u128,
    sha256: String,
}

/// Per-file SHA-256s by absolute path, valid while length and modification
/// time stay the same.
#[derive(Debug, Default)]
pub struct FileHashCache {
    path: Option<PathBuf>,
    entries: Mutex<BTreeMap<String, HashEntry>>,
    dirty: Mutex<bool>,
}

pub fn mtime_ns(meta: &fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

impl FileHashCache {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(FILE_HASH_CACHE);
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries: Mutex::new(entries),
            dirty: Mutex::new(false),
        }
    }

    pub fn sha256_of(&self, path: &Path, meta: &fs::Metadata) -> io::Result<String> {
        let key = path.to_string_lossy().into_owned();
        let (len, mt) = (meta.len(), mtime_ns(meta));
        if let Some(e) = self.entries.lock().unwrap().get(&key) {
            if e.len == len && e.mtime_ns == mt && mt != 0 {
                return Ok(e.sha256.clone());
            }
        }
        let sha = sha256_file(path, len)?;
        self.remember(path, meta, &sha);
        Ok(sha)
    }

    /// Record a hash computed elsewhere (an LVL scan reads the bytes anyway).
    pub fn remember(&self, path: &Path, meta: &fs::Metadata, sha256: &str) {
        self.entries.lock().unwrap().insert(
            path.to_string_lossy().into_owned(),
            HashEntry {
                len: meta.len(),
                mtime_ns: mtime_ns(meta),
                sha256: sha256.to_owned(),
            },
        );
        *self.dirty.lock().unwrap() = true;
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        if !std::mem::take(&mut *self.dirty.lock().unwrap()) {
            return;
        }
        let entries = self.entries.lock().unwrap();
        let _ = crate::profile::write_ron_atomic(path, &*entries, "file hash cache");
    }
}

fn sha256_file(path: &Path, len: u64) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; READ_BUF_BYTES];
    let mut remaining = len;
    while remaining > 0 {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let take = (n as u64).min(remaining) as usize;
        hasher.update(&buf[..take]);
        remaining -= take as u64;
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FingerprintResult {
    pub digest: String,
    pub warnings: Vec<String>,
}

/// Compute a deterministic SHA-256 fingerprint over all regular files inside
/// `mod_root`, sorted by normalized relative path. Refuses any symlink whose
/// target resolves outside `mod_root` and caps file counts and individual file
/// sizes to protect against bombs/cycles.
pub fn compute_mod_fingerprint(mod_root: &Path) -> io::Result<FingerprintResult> {
    compute_mod_fingerprint_cached(mod_root, &FileHashCache::default())
}

/// [`compute_mod_fingerprint`] reusing `cache` for unchanged files.
pub fn compute_mod_fingerprint_cached(
    mod_root: &Path,
    cache: &FileHashCache,
) -> io::Result<FingerprintResult> {
    let root_canon = fs::canonicalize(mod_root)?;
    let mut visited_dirs: std::collections::BTreeSet<PathBuf> =
        std::collections::BTreeSet::from([root_canon.clone()]);
    let mut files: Vec<(String, PathBuf, fs::Metadata)> = Vec::new();
    let mut warnings = Vec::new();
    let mut stack = vec![(mod_root.to_path_buf(), 0usize)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > 16 {
            warnings.push(format!(
                "Stopped fingerprinting beyond directory depth 16 at {}",
                dir.display()
            ));
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for p in children {
            let Ok(smeta) = fs::symlink_metadata(&p) else {
                continue;
            };
            let rel = p
                .strip_prefix(mod_root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            if smeta.file_type().is_symlink() {
                match fs::canonicalize(&p) {
                    Ok(target) if target.starts_with(&root_canon) => {}
                    _ => {
                        warnings.push(format!("Fingerprint skipped escaping symlink `{rel}`"));
                        continue;
                    }
                }
            }
            let Ok(meta) = fs::metadata(&p) else {
                continue;
            };
            if meta.is_dir() {
                match fs::canonicalize(&p) {
                    Ok(canon) if canon.starts_with(&root_canon) => {
                        if visited_dirs.insert(canon) {
                            stack.push((p, depth + 1));
                        } else {
                            warnings.push(format!("Fingerprint skipped cyclic directory `{rel}`"));
                        }
                    }
                    _ => {
                        warnings.push(format!("Fingerprint skipped escaping directory `{rel}`"));
                    }
                }
            } else if meta.is_file() {
                if files.len() >= MAX_FILES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("mod exceeds maximum fingerprinted file count ({MAX_FILES})"),
                    ));
                }
                if meta.len() > MAX_SINGLE_FILE_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "file `{rel}` exceeds fingerprint size cap ({MAX_SINGLE_FILE_BYTES} B)"
                        ),
                    ));
                }
                files.push((rel, p, meta));
            }
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    hasher.update(b"openbf-mod-fp-v2");
    for (rel, path, meta) in files {
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update(meta.len().to_le_bytes());
        hasher.update(cache.sha256_of(&path, &meta)?.as_bytes());
    }
    let hex = hex(&hasher.finalize());
    Ok(FingerprintResult {
        digest: hex,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openbf_fp_test_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fingerprint_is_deterministic_and_sensitive_to_changes() {
        let dir = tmp_dir("det");
        fs::create_dir_all(dir.join("data/_LVL_PC")).unwrap();
        fs::write(dir.join("addme.script"), b"abc").unwrap();
        fs::write(dir.join("data/_LVL_PC/core.lvl"), b"xyz").unwrap();

        let fp1 = compute_mod_fingerprint(&dir).unwrap();
        let fp2 = compute_mod_fingerprint(&dir).unwrap();
        assert_eq!(fp1.digest, fp2.digest);
        assert_eq!(fp1.digest.len(), 64);

        fs::write(dir.join("data/_LVL_PC/core.lvl"), b"xyZ").unwrap();
        let fp3 = compute_mod_fingerprint(&dir).unwrap();
        assert_ne!(fp1.digest, fp3.digest);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repeated_file_hash_cache_saves_leave_no_temp_files() {
        let dir = tmp_dir("cache_save");
        let sample = dir.join("sample.lvl");
        fs::write(&sample, b"hello").unwrap();

        let cache = FileHashCache::load(&dir);
        let meta = fs::metadata(&sample).unwrap();
        let h1 = cache.sha256_of(&sample, &meta).unwrap();
        cache.save();

        fs::write(&sample, b"hello-updated").unwrap();
        let meta2 = fs::metadata(&sample).unwrap();
        let h2 = cache.sha256_of(&sample, &meta2).unwrap();
        assert_ne!(h1, h2);
        cache.save();

        let reloaded = FileHashCache::load(&dir);
        assert_eq!(reloaded.sha256_of(&sample, &meta2).unwrap(), h2);

        let cache_dir = dir.join("cache");
        for entry in fs::read_dir(&cache_dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.contains(".tmp"), "stale temp file left behind: {name}");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
