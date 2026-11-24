//! Path discovery for both release folders and repository checkouts.
//!
//! In a release bundle, `openbf-launcher`, `openbf`, and `openbf_data/` live
//! side by side in the same folder. In a development checkout
//! (`engine/target/{debug,release}/openbf-launcher`), the packed game lives in
//! `dist/openbf(.exe)` at the repository root and `openbf_data/` sits beside it.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The archive entry only a build with Battlefront I carries.
const BF1_MARKER: &str = "assets_baked/common_bf1/common.ron";

/// Readiness of the built game binary/archive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuiltStatus {
    pub ready: bool,
    pub has_bf1: bool,
}

/// Executable filename suffix for the target OS.
pub const EXE_SUFFIX: &str = std::env::consts::EXE_SUFFIX;

/// Discovered layout around the running launcher binary.
#[derive(Clone, Debug)]
pub struct LauncherPaths {
    /// Directory containing `openbf-launcher`.
    pub exe_dir: PathBuf,
    /// Repository checkout root (the folder holding `scripts/pack.py`), when
    /// the launcher runs from one.
    pub checkout_root: Option<PathBuf>,
    /// Target path of the playable game executable (`openbf(.exe)`).
    pub game_out: PathBuf,
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
        let checkout_root =
            find_checkout_root(&exe_dir).or_else(|| cwd.and_then(find_checkout_root));

        // A checkout build packs into the repository's `dist/`; a release
        // folder keeps `openbf` right beside the launcher.
        let openbf_name = format!("openbf{EXE_SUFFIX}");
        let game_out = match &checkout_root {
            Some(root) if in_cargo_target(&exe_dir) => root.join("dist").join(&openbf_name),
            _ => exe_dir.join(&openbf_name),
        };

        let data_dir = game_out
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(&exe_dir)
            .join("openbf_data");
        Self {
            config_path: data_dir.join("launcher.ron"),
            logs_dir: data_dir.join("logs"),
            data_dir,
            exe_dir,
            checkout_root,
            game_out,
        }
    }

    /// Locate the compiled `bake` executable (`openbf_data/tools/bake`,
    /// beside the launcher, or in a checkout's `engine/target/{release,debug}`).
    pub fn bake_bin(&self) -> Option<PathBuf> {
        let name = format!("bake{EXE_SUFFIX}");
        [
            self.data_dir.join("tools").join(&name),
            self.exe_dir.join(&name),
        ]
        .into_iter()
        .chain(self.checkout_targets(&name))
        .find(|p| p.is_file())
    }

    /// Locate the base game executable used by `--write-pack`: `game` beside
    /// the launcher, a release folder's existing `openbf`, or a checkout's
    /// `engine/target/{release,debug}/game`.
    pub fn base_game_bin(&self) -> Option<PathBuf> {
        let name = format!("game{EXE_SUFFIX}");
        let release_openbf = (!in_cargo_target(&self.exe_dir)).then(|| self.game_out.clone());
        std::iter::once(self.exe_dir.join(&name))
            .chain(release_openbf)
            .chain(self.checkout_targets(&name))
            .find(|p| p.is_file())
    }

    /// `engine/target/release/<name>` then `engine/target/debug/<name>` in the checkout.
    fn checkout_targets(&self, name: &str) -> impl Iterator<Item = PathBuf> + '_ {
        let name = name.to_owned();
        self.checkout_root.iter().flat_map(move |root| {
            let target = root.join("engine").join("target");
            ["release", "debug"].map(|profile| target.join(profile).join(&name))
        })
    }

    /// True when the native setup tools (`bake` + base game binary) are present.
    pub fn can_build(&self) -> bool {
        self.bake_bin().is_some() && self.base_game_bin().is_some()
    }

    /// True when the game binary exists and carries baked assets (either
    /// appended to `openbf` or in `openbf.pak` beside it).
    pub fn is_game_ready(&self) -> bool {
        self.built_status().ready
    }

    /// Open the built game's archive ([`pak::find_archive`]: inside `openbf`,
    /// else `openbf.pak` beside it) and report whether it carries BF1.
    pub fn built_status(&self) -> BuiltStatus {
        if !self.game_out.is_file() {
            return BuiltStatus::default();
        }
        match pak::find_archive(&self.game_out) {
            Some(archive) => BuiltStatus {
                ready: true,
                has_bf1: archive.names().any(|n| n == BF1_MARKER),
            },
            None => BuiltStatus::default(),
        }
    }

    /// Location of `extensions.ron` inside `openbf_data`.
    pub fn extensions_profile_path(&self) -> PathBuf {
        extensions::profile::profile_path(&self.data_dir)
    }

    /// Location of `openbf_data/mods/` (filesystem source of truth for legacy mods).
    pub fn mods_dir(&self) -> PathBuf {
        extensions::profile::mods_dir(&self.data_dir)
    }

    /// Location of `openbf_data/mod_cache/` (versioned converted asset cache).
    pub fn mod_cache_dir(&self) -> PathBuf {
        extensions::profile::mod_cache_dir(&self.data_dir)
    }

    /// Location of `last_startup_error.ron` inside `openbf_data`.
    pub fn startup_error_path(&self) -> PathBuf {
        extensions::profile::startup_error_path(&self.data_dir)
    }

    /// Discover bundled engine extensions by querying the built/base game binary
    /// (`--list-extensions`), falling back to the compile-time catalog.
    pub fn probe_bundled_extensions(&self) -> Vec<extensions::ExtensionManifest> {
        let mut candidates = Vec::new();
        if self.game_out.is_file() {
            candidates.push(self.game_out.clone());
        }
        if let Some(base) = self.base_game_bin() {
            if !candidates.contains(&base) {
                candidates.push(base);
            }
        }
        for bin in candidates {
            if let Some(list) = query_extensions_from_bin(&bin) {
                if !list.is_empty() {
                    return list;
                }
            }
        }
        extensions::bundled_extensions_catalog(cfg!(any(windows, target_os = "linux")))
    }
}

fn query_extensions_from_bin(bin: &Path) -> Option<Vec<extensions::ExtensionManifest>> {
    let mut cmd = std::process::Command::new(bin);
    cmd.arg("--list-extensions");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    ron::from_str(&text).ok()
}

fn in_cargo_target(dir: &Path) -> bool {
    dir.ancestors()
        .any(|a| a.file_name().is_some_and(|n| n == "target"))
}

fn find_checkout_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join("scripts").join("pack.py").is_file())
        .map(Path::to_path_buf)
}

/// Open a folder in the OS file manager, creating it first if needed.
pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))?;
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .with_context(|| format!("opening {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts").join("pack.py"), "# stub\n").unwrap();
        dir
    }

    #[test]
    fn release_layout_uses_sibling_openbf() {
        let dir = scratch("paths_rel");
        let exe = dir.join(format!("openbf-launcher{EXE_SUFFIX}"));
        let openbf = dir.join(format!("openbf{EXE_SUFFIX}"));
        std::fs::write(&openbf, b"stub").unwrap();

        let paths = LauncherPaths::from_exe(&exe, None);
        assert_eq!(paths.checkout_root.as_deref(), Some(dir.as_path()));
        assert_eq!(paths.game_out, openbf);
        assert_eq!(paths.base_game_bin().as_deref(), Some(openbf.as_path()));
        assert_eq!(paths.logs_dir, dir.join("openbf_data").join("logs"));
        assert!(!paths.is_game_ready());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cargo_target_layout_points_to_dist_openbf() {
        let dir = scratch("paths_dev");
        let exe = dir
            .join("engine")
            .join("target")
            .join("debug")
            .join(format!("openbf-launcher{EXE_SUFFIX}"));

        let paths = LauncherPaths::from_exe(&exe, Some(&dir));
        assert_eq!(paths.checkout_root.as_deref(), Some(dir.as_path()));
        assert_eq!(
            paths.game_out,
            dir.join("dist").join(format!("openbf{EXE_SUFFIX}"))
        );
        assert_eq!(paths.base_game_bin(), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn built_status_reads_the_archive_index() {
        let dir = scratch("built_status");
        let exe = dir.join(format!("openbf-launcher{EXE_SUFFIX}"));
        let paths = LauncherPaths::from_exe(&exe, None);
        let stub = dir.join("stub.exe");
        std::fs::write(&stub, b"not an archive").unwrap();
        std::fs::copy(&stub, &paths.game_out).unwrap();
        assert_eq!(paths.built_status(), BuiltStatus::default());

        let asset = dir.join("common.ron");
        std::fs::write(&asset, b"()").unwrap();
        for (name, has_bf1) in [
            ("assets_baked/common_bf2/common.ron", false),
            (BF1_MARKER, true),
        ] {
            pak::write_pack(&stub, &paths.game_out, &[(name.to_owned(), asset.clone())]).unwrap();
            assert_eq!(
                paths.built_status(),
                BuiltStatus {
                    ready: true,
                    has_bf1
                }
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extension_paths_and_fallback_catalog_work() {
        let dir = scratch("ext_paths");
        let exe = dir.join(format!("openbf-launcher{EXE_SUFFIX}"));
        let paths = LauncherPaths::from_exe(&exe, None);
        assert_eq!(
            paths.extensions_profile_path(),
            paths.data_dir.join(extensions::profile::PROFILE_FILE)
        );
        assert_eq!(
            paths.startup_error_path(),
            paths.data_dir.join(extensions::profile::STARTUP_ERROR_FILE)
        );
        let catalog = paths.probe_bundled_extensions();
        assert!(catalog.iter().any(|m| m.id == extensions::OPENBF_VR_ID));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
