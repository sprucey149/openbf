//! Path discovery for both release folders and repository checkouts.
//!
//! In a release bundle, `openbf-launcher`, `openbf`, and `openbf_data/` live
//! side by side in the same folder. In a development checkout
//! (`engine/target/{debug,release}/openbf-launcher`), the packed game lives in
//! `dist/openbf(.exe)` at the repository root and `openbf_data/` sits beside it.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/embedded_ffmpeg.rs"));

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
    /// Discover paths from the current executable, working directory, and `OPENBF_DATA_DIR`.
    pub fn discover() -> Self {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("openbf-launcher"));
        let cwd = std::env::current_dir().ok();
        let env = std::env::var_os(extensions::profile::DATA_DIR_ENV);
        Self::from_exe_with_env(&exe, cwd.as_deref(), env.as_deref())
    }

    /// Pure constructor for deterministic testing (without `OPENBF_DATA_DIR`).
    pub fn from_exe(exe_path: &Path, cwd: Option<&Path>) -> Self {
        Self::from_exe_with_env(exe_path, cwd, None)
    }

    /// Pure constructor honoring an explicit `OPENBF_DATA_DIR` override while
    /// keeping executable and packaged asset discovery tied to `exe_path`.
    pub fn from_exe_with_env(
        exe_path: &Path,
        cwd: Option<&Path>,
        data_dir_env: Option<&std::ffi::OsStr>,
    ) -> Self {
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

        let data_dir = extensions::profile::resolve_data_dir(exe_path, cwd, data_dir_env);
        Self {
            config_path: data_dir.join("launcher.ron"),
            logs_dir: data_dir.join("logs"),
            data_dir,
            exe_dir,
            checkout_root,
            game_out,
        }
    }

    /// Locate the compiled `bake` executable (beside the launcher,
    /// `openbf_data/tools/bake`, or in a checkout's
    /// `engine/target/{release,debug}`), preferring one that writes this
    /// build's mod converter version (`extensions::mods::pick_bake_bin`).
    pub fn bake_bin(&self) -> Option<PathBuf> {
        let name = format!("bake{EXE_SUFFIX}");
        extensions::mods::pick_bake_bin(
            [
                self.exe_dir.join(&name),
                self.data_dir.join("tools").join(&name),
            ]
            .into_iter()
            .chain(self.checkout_targets(&name)),
        )
    }

    /// If this launcher was built with `OPENBF_EMBED_FFMPEG`, materialize the
    /// embedded `ffmpeg` binary into `openbf_data/tools/ffmpeg(.exe)` when absent
    /// (or different size) so the user never has to install `ffmpeg`.
    pub fn ensure_embedded_ffmpeg(&self) -> Result<Option<PathBuf>> {
        let Some(bytes) = EMBEDDED_FFMPEG else {
            return Ok(None);
        };
        let tools_dir = self.data_dir.join("tools");
        let target = tools_dir.join(format!("ffmpeg{EXE_SUFFIX}"));
        let up_to_date = std::fs::metadata(&target)
            .ok()
            .is_some_and(|m| m.is_file() && m.len() == bytes.len() as u64);
        if !up_to_date {
            std::fs::create_dir_all(&tools_dir)
                .with_context(|| format!("creating {}", tools_dir.display()))?;
            let tmp = tools_dir.join(format!("ffmpeg.tmp.{}{EXE_SUFFIX}", std::process::id()));
            std::fs::write(&tmp, bytes)
                .with_context(|| format!("extracting embedded ffmpeg to {}", tmp.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
            }
            if target.exists() {
                let _ = std::fs::remove_file(&target);
            }
            std::fs::rename(&tmp, &target).with_context(|| {
                format!("installing embedded ffmpeg at {}", target.display())
            })?;
        }
        Ok(Some(target))
    }

    /// Locate a bundled or explicitly configured `ffmpeg` executable:
    /// `OPENBF_FFMPEG`, `openbf_data/tools/ffmpeg(.exe)`, beside the launcher,
    /// `<exe_dir>/tools/ffmpeg(.exe)`, or `<exe_dir>/openbf_data/tools/ffmpeg(.exe)`.
    /// Returns `None` when none of those exist (callers may then fall back to `PATH`).
    pub fn ffmpeg_bin(&self) -> Option<PathBuf> {
        if let Some(ov) = std::env::var_os("OPENBF_FFMPEG").filter(|s| !s.is_empty()) {
            return Some(PathBuf::from(ov));
        }
        let name = format!("ffmpeg{EXE_SUFFIX}");
        [
            self.data_dir.join("tools").join(&name),
            self.exe_dir.join(&name),
            self.exe_dir.join("tools").join(&name),
            self.exe_dir.join("openbf_data").join("tools").join(&name),
        ]
        .into_iter()
        .find(|p| p.is_file())
    }

    /// The stock bake a mod scan resolves replacements against and a
    /// conversion builds on: the built game, whose archive is the one the
    /// game reads at play time, else a checkout's `engine/assets_baked`.
    pub fn stock_assets(&self) -> Option<PathBuf> {
        if self.game_out.is_file() && pak::find_archive(&self.game_out).is_some() {
            return Some(self.game_out.clone());
        }
        self.checkout_root
            .as_ref()
            .map(|root| root.join("engine").join("assets_baked"))
            .filter(|p| p.is_dir())
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

    #[test]
    fn data_dir_env_override_keeps_exe_and_game_out_intact() {
        let dir = scratch("env_override");
        let exe = dir.join(format!("openbf-launcher{EXE_SUFFIX}"));
        let custom = dir.join("isolated_user_data");

        let paths = LauncherPaths::from_exe_with_env(&exe, None, Some(custom.as_os_str()));
        assert_eq!(paths.exe_dir, dir);
        assert_eq!(paths.game_out, dir.join(format!("openbf{EXE_SUFFIX}")));
        assert_eq!(paths.data_dir, custom);
        assert_eq!(paths.config_path, custom.join("launcher.ron"));
        assert_eq!(paths.mods_dir(), custom.join("mods"));
        assert_eq!(paths.mod_cache_dir(), custom.join("mod_cache"));
        assert_eq!(paths.extensions_profile_path(), custom.join("extensions.ron"));
        assert_eq!(
            paths.startup_error_path(),
            custom.join("last_startup_error.ron")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ffmpeg_bin_prefers_data_tools_then_sibling() {
        let dir = scratch("ffmpeg_bin");
        let exe = dir.join(format!("openbf-launcher{EXE_SUFFIX}"));
        let paths = LauncherPaths::from_exe(&exe, None);
        let name = format!("ffmpeg{EXE_SUFFIX}");

        // Without OPENBF_FFMPEG or bundled files, returns None (callers fall back to PATH).
        if std::env::var_os("OPENBF_FFMPEG").is_none() {
            assert_eq!(paths.ffmpeg_bin(), None);

            let sibling = dir.join(&name);
            std::fs::write(&sibling, b"stub").unwrap();
            assert_eq!(paths.ffmpeg_bin(), Some(sibling));

            let in_tools = paths.data_dir.join("tools").join(&name);
            std::fs::create_dir_all(in_tools.parent().unwrap()).unwrap();
            std::fs::write(&in_tools, b"stub").unwrap();
            assert_eq!(paths.ffmpeg_bin(), Some(in_tools));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
