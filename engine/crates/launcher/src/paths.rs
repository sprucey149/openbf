//! Path discovery for both release folders and repository checkouts.
//!
//! In a release bundle, `openbf-launcher`, `openbf`, and `openbf_data/` live
//! side by side in the same folder. In a development checkout
//! (`engine/target/{debug,release}/openbf-launcher`), the packed game lives in
//! `dist/openbf(.exe)` at the repository root and `openbf_data/` sits beside it.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const PACK_MAGIC: &[u8; 8] = b"OPENBFPK";
const PACK_FOOTER_LEN: u64 = 32;

/// Executable filename suffix for the target OS.
pub const EXE_SUFFIX: &str = std::env::consts::EXE_SUFFIX;

/// Discovered layout around the running launcher binary.
#[derive(Clone, Debug)]
pub struct LauncherPaths {
    /// Directory containing `openbf-launcher`.
    pub exe_dir: PathBuf,
    /// Directory containing `scripts/pack.py`, when available.
    pub pack_root: Option<PathBuf>,
    /// Target path of the playable game executable (`openbf(.exe)`).
    pub game_out: PathBuf,
    /// Optional pre-built game binary to hand to `pack.py --game-bin` so a
    /// release bundle never needs Rust/Cargo.
    pub prebuilt_game_bin: Option<PathBuf>,
    /// Portable data folder (`openbf_data/`) beside the game executable.
    pub data_dir: PathBuf,
    /// Path to `openbf_data/launcher.ron`.
    pub config_path: PathBuf,
    /// Path to `openbf_data/logs/`.
    pub logs_dir: PathBuf,
}

impl LauncherPaths {
    /// Discover paths from the current executable and working directory.
    pub fn discover() -> Self {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("openbf-launcher"));
        let cwd = std::env::current_dir().ok();
        Self::from_exe(&exe, cwd.as_deref())
    }

    /// Pure constructor for deterministic testing.
    pub fn from_exe(exe_path: &Path, cwd: Option<&Path>) -> Self {
        let exe_dir = exe_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let pack_root = find_pack_root(&exe_dir).or_else(|| cwd.and_then(find_pack_root));

        let openbf_name = format!("openbf{EXE_SUFFIX}");
        let game_name = format!("game{EXE_SUFFIX}");

        // Release layout: `openbf(.exe)` sits right beside `openbf-launcher(.exe)`
        // (or the launcher is not inside `engine/target/...`).
        let in_cargo_target = exe_dir
            .ancestors()
            .any(|a| a.file_name().is_some_and(|n| n == "target"));

        let (game_out, prebuilt_game_bin) = if in_cargo_target {
            let out = pack_root
                .as_ref()
                .map(|r| r.join("dist").join(&openbf_name))
                .unwrap_or_else(|| exe_dir.join(&openbf_name));
            // In a checkout target dir, if `game(.exe)` is already built right
            // beside the launcher in release mode, offer it as `--game-bin`.
            let sibling_game = exe_dir.join(&game_name);
            let is_release = exe_dir.file_name().is_some_and(|n| n == "release");
            let prebuilt = (is_release && sibling_game.is_file()).then_some(sibling_game);
            (out, prebuilt)
        } else {
            let out = exe_dir.join(&openbf_name);
            let sibling_game = exe_dir.join(&game_name);
            let prebuilt = if sibling_game.is_file() {
                Some(sibling_game)
            } else if out.is_file() {
                Some(out.clone())
            } else {
                None
            };
            (out, prebuilt)
        };

        let game_dir = game_out
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(&exe_dir);
        let data_dir = game_dir.join("openbf_data");
        let config_path = data_dir.join("launcher.ron");
        let logs_dir = data_dir.join("logs");

        Self {
            exe_dir,
            pack_root,
            game_out,
            prebuilt_game_bin,
            data_dir,
            config_path,
            logs_dir,
        }
    }

    /// Path to `scripts/pack.py` when a pack root was found.
    pub fn pack_script(&self) -> Option<PathBuf> {
        self.pack_root
            .as_ref()
            .map(|r| r.join("scripts").join("pack.py"))
            .filter(|p| p.is_file())
    }

    /// True when the game binary exists and carries baked assets (either
    /// appended to `openbf` or in `openbf.pak` beside it).
    ///
    /// Reads only the 32-byte `OPENBFPK` footer (zero heap allocations) instead
    /// of parsing the full archive index into memory.
    pub fn is_game_ready(&self) -> bool {
        if !self.game_out.is_file() {
            return false;
        }
        if has_valid_pak_footer(&self.game_out) {
            return true;
        }
        if let Some(parent) = self.game_out.parent() {
            let sidecar = parent.join(pak::PAK_NAME);
            if sidecar.is_file() && has_valid_pak_footer(&sidecar) {
                return true;
            }
        }
        false
    }
}

/// Fast O(1) validation of a `pak` archive's 32-byte trailing footer.
pub fn has_valid_pak_footer(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    let size = meta.len();
    if size < PACK_FOOTER_LEN {
        return false;
    }
    if file.seek(SeekFrom::Start(size - PACK_FOOTER_LEN)).is_err() {
        return false;
    }
    let mut footer = [0u8; PACK_FOOTER_LEN as usize];
    if file.read_exact(&mut footer).is_err() {
        return false;
    }
    if &footer[..8] != PACK_MAGIC {
        return false;
    }
    let start = u64::from_le_bytes(footer[8..16].try_into().unwrap());
    let at = u64::from_le_bytes(footer[16..24].try_into().unwrap());
    let len = u64::from_le_bytes(footer[24..32].try_into().unwrap());
    start <= at
        && at
            .checked_add(len)
            .and_then(|end| end.checked_add(PACK_FOOTER_LEN))
            == Some(size)
}

fn find_pack_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if dir.join("scripts").join("pack.py").is_file() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// Open a folder in the OS file manager, creating it first if needed.
pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .with_context(|| format!("opening {}", path.display()))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .with_context(|| format!("opening {}", path.display()))?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .with_context(|| format!("opening {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_layout_uses_sibling_openbf() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_paths_rel_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts").join("pack.py"), "# stub\n").unwrap();
        let exe = dir.join(format!("openbf-launcher{EXE_SUFFIX}"));
        let openbf = dir.join(format!("openbf{EXE_SUFFIX}"));
        std::fs::write(&openbf, b"stub").unwrap();

        let paths = LauncherPaths::from_exe(&exe, None);
        assert_eq!(paths.pack_root.as_deref(), Some(dir.as_path()));
        assert_eq!(paths.game_out, openbf);
        assert_eq!(paths.prebuilt_game_bin.as_deref(), Some(openbf.as_path()));
        assert_eq!(paths.logs_dir, dir.join("openbf_data").join("logs"));
        assert!(!paths.is_game_ready());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cargo_target_layout_points_to_dist_openbf() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_paths_dev_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts").join("pack.py"), "# stub\n").unwrap();
        let exe = dir
            .join("engine")
            .join("target")
            .join("debug")
            .join(format!("openbf-launcher{EXE_SUFFIX}"));

        let paths = LauncherPaths::from_exe(&exe, Some(&dir));
        assert_eq!(paths.pack_root.as_deref(), Some(dir.as_path()));
        assert_eq!(
            paths.game_out,
            dir.join("dist").join(format!("openbf{EXE_SUFFIX}"))
        );
        assert_eq!(paths.prebuilt_game_bin, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn valid_and_invalid_pak_footers() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_footer_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let bad = dir.join("bad.pak");
        std::fs::write(&bad, b"too short").unwrap();
        assert!(!has_valid_pak_footer(&bad));

        // Build a minimal valid footer: 16 bytes payload + 4 bytes index + 32 bytes footer = 52 bytes.
        let good = dir.join("good.pak");
        let mut bytes = vec![0u8; 20];
        bytes.extend_from_slice(PACK_MAGIC);
        bytes.extend_from_slice(&0u64.to_le_bytes()); // start = 0
        bytes.extend_from_slice(&16u64.to_le_bytes()); // index offset = 16
        bytes.extend_from_slice(&4u64.to_le_bytes()); // index len = 4
        std::fs::write(&good, &bytes).unwrap();
        assert!(has_valid_pak_footer(&good));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
