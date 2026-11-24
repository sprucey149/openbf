//! Bounded, traversal-safe source fingerprinting for legacy mod folders.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_FILES: usize = 10_000;
const MAX_SINGLE_FILE_BYTES: u64 = 512 * 1024 * 1024;
const READ_BUF_BYTES: usize = 64 * 1024;

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
    let root_canon = fs::canonicalize(mod_root)?;
    let mut files: Vec<(String, PathBuf, u64)> = Vec::new();
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
                stack.push((p, depth + 1));
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
                files.push((rel, p, meta.len()));
            }
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    hasher.update(b"openbf-mod-fp-v1\0");
    let mut buf = vec![0u8; READ_BUF_BYTES];

    for (rel, path, len) in files {
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update(len.to_le_bytes());
        let mut f = File::open(&path)?;
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
    }

    let digest_bytes = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest_bytes {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
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
}
