//! Persistent launcher configuration (`openbf_data/launcher.ron`).
//!
//! Stores the Battlefront I and Battlefront II source paths (one per platform
//! for each game, so a PC install, a PS2 disc and an Xbox disc can all feed
//! one build), the platform each game's picker shows, optional DLC heroes
//! folder, and preferred graphics API override. Writes are atomic (`.tmp`
//! then rename) so an interrupted save never leaves a partial config behind.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which edition of a game the user is supplying.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Platform {
    #[default]
    Pc,
    Ps2,
    Xbox,
}

impl Platform {
    pub const ALL: [Platform; 3] = [Platform::Pc, Platform::Ps2, Platform::Xbox];

    pub fn label(self) -> &'static str {
        match self {
            Platform::Pc => "PC",
            Platform::Ps2 => "PS2",
            Platform::Xbox => "Xbox",
        }
    }

    /// True when this platform is selected as an install folder rather than a
    /// disc image file.
    pub fn picks_folder(self) -> bool {
        matches!(self, Platform::Pc)
    }
}

/// One game's input sources: at most one path per platform, and the
/// platform the picker currently shows (`platform`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub paths: BTreeMap<Platform, PathBuf>,
    /// The single path older launchers saved for `platform`; folded into
    /// `paths` on load and never written back.
    #[serde(default, skip_serializing)]
    path: Option<PathBuf>,
}

impl SourceConfig {
    /// A config whose only source is `path` for `platform`, shown in the picker.
    pub fn single(platform: Platform, path: impl Into<PathBuf>) -> Self {
        let mut source = Self {
            platform,
            ..Self::default()
        };
        source.set_path(platform, path.into());
        source
    }

    fn migrate_legacy(&mut self) {
        if let Some(path) = self.path.take() {
            self.paths.entry(self.platform).or_insert(path);
        }
    }

    /// The path chosen for `platform`, if any.
    pub fn path(&self, platform: Platform) -> Option<&Path> {
        self.paths.get(&platform).map(PathBuf::as_path)
    }

    /// The path chosen for the platform the picker shows.
    pub fn active_path(&self) -> Option<&Path> {
        self.path(self.platform)
    }

    pub fn set_path(&mut self, platform: Platform, path: PathBuf) {
        self.paths.insert(platform, path);
    }

    /// Forget `platform`'s path, keeping the other platforms'.
    pub fn clear(&mut self, platform: Platform) {
        self.paths.remove(&platform);
    }

    /// Every chosen source in [`Platform::ALL`] order.
    pub fn sources(&self) -> impl Iterator<Item = (Platform, &Path)> {
        self.paths.iter().map(|(p, path)| (*p, path.as_path()))
    }

    /// True when at least one platform has a path.
    pub fn has_any(&self) -> bool {
        !self.paths.is_empty()
    }

    /// True when at least one path is chosen and every chosen path exists on
    /// disk with its platform's filesystem kind (directory for PC, file for
    /// PS2/Xbox).
    pub fn is_present(&self) -> bool {
        self.has_any()
            && self.sources().all(|(platform, path)| {
                if platform.picks_folder() {
                    path.is_dir()
                } else {
                    path.is_file()
                }
            })
    }

    /// The picker's title: every platform with a path plus the one shown, in
    /// [`Platform::ALL`] order, each flagged true when it is the one shown.
    pub fn title_parts(&self) -> Vec<(Platform, bool)> {
        Platform::ALL
            .into_iter()
            .filter(|p| *p == self.platform || self.paths.contains_key(p))
            .map(|p| (p, p == self.platform))
            .collect()
    }

    /// Short display label for the shown platform's read-only path field.
    pub fn display_name(&self) -> Cow<'_, str> {
        path_label(self.active_path())
    }
}

/// A picked path's file name for a read-only field (the whole path when it
/// has none, "Not selected" when nothing is picked).
fn path_label(path: Option<&Path>) -> Cow<'_, str> {
    let Some(path) = path else {
        return Cow::Borrowed("Not selected");
    };
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) if !name.is_empty() => Cow::Borrowed(name),
        _ => path.to_string_lossy(),
    }
}

/// Optional `WGPU_BACKEND` override passed when launching `openbf`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsApi {
    #[default]
    Auto,
    Vulkan,
    Dx12,
    Metal,
}

impl GraphicsApi {
    /// Choices that make sense on the host operating system.
    pub fn supported_on_host() -> &'static [GraphicsApi] {
        #[cfg(target_os = "windows")]
        {
            &[GraphicsApi::Auto, GraphicsApi::Dx12, GraphicsApi::Vulkan]
        }
        #[cfg(target_os = "macos")]
        {
            &[GraphicsApi::Auto, GraphicsApi::Metal]
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            &[GraphicsApi::Auto, GraphicsApi::Vulkan]
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GraphicsApi::Auto => "Auto (default)",
            GraphicsApi::Vulkan => "Vulkan",
            GraphicsApi::Dx12 => "DirectX 12",
            GraphicsApi::Metal => "Metal",
        }
    }

    /// Value for `WGPU_BACKEND`, or `None` for automatic selection.
    pub fn wgpu_backend(self) -> Option<&'static str> {
        match self {
            GraphicsApi::Auto => None,
            GraphicsApi::Vulkan => Some("vulkan"),
            GraphicsApi::Dx12 => Some("dx12"),
            GraphicsApi::Metal => Some("metal"),
        }
    }
}

/// Full launcher state persisted in `openbf_data/launcher.ron`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default)]
    pub bf1: SourceConfig,
    #[serde(default)]
    pub bf2: SourceConfig,
    #[serde(default)]
    pub dlc_dir: Option<PathBuf>,
    #[serde(default)]
    pub graphics_api: GraphicsApi,
}

impl LauncherConfig {
    /// Load config from `path`, returning defaults when missing or unreadable.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let mut config: Self = ron::from_str(&text).unwrap_or_default();
        config.bf1.migrate_legacy();
        config.bf2.migrate_legacy();
        config
    }

    /// Save config atomically to `path`.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let pretty = ron::ser::PrettyConfig::new()
            .depth_limit(4)
            .new_line("\n".to_string());
        let text =
            ron::ser::to_string_pretty(self, pretty).context("serializing launcher config")?;
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }

    /// Short display label for the optional DLC Heroes directory.
    pub fn dlc_display_name(&self) -> Cow<'_, str> {
        path_label(self.dlc_dir.as_deref())
    }
}

impl LauncherConfig {
    /// The original games' files as the mod converter reads them: soldier
    /// replacements bake against the stock sibling sides and animation banks
    /// of the install or disc this lists.
    pub fn stock_sources(&self) -> extensions::mods::catalog::StockSourceList {
        use extensions::mods::catalog::{SourceGame, SourcePlatform, StockSourceEntry};
        let mut sources = Vec::new();
        for (game, cfg) in [(SourceGame::Bf1, &self.bf1), (SourceGame::Bf2, &self.bf2)] {
            for (platform, path) in cfg.sources() {
                sources.push(StockSourceEntry {
                    game,
                    platform: match platform {
                        Platform::Pc => SourcePlatform::Pc,
                        Platform::Ps2 => SourcePlatform::Ps2,
                        Platform::Xbox => SourcePlatform::Xbox,
                    },
                    path: path.to_path_buf(),
                });
            }
        }
        extensions::mods::catalog::StockSourceList { sources }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_config() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_cfg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("openbf_data").join("launcher.ron");

        let mut bf2 = SourceConfig::single(Platform::Pc, "/games/SWBF2");
        bf2.set_path(Platform::Ps2, PathBuf::from("/games/bf2.iso"));
        let cfg = LauncherConfig {
            bf1: SourceConfig::single(Platform::Ps2, "/games/bf1.iso"),
            bf2,
            dlc_dir: Some(PathBuf::from("/games/DLC_XBOX")),
            graphics_api: GraphicsApi::Vulkan,
        };
        cfg.save(&path).expect("save config");
        let loaded = LauncherConfig::load(&path);
        assert_eq!(loaded, cfg);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_path_configs_from_older_launchers_still_load() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_old_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("launcher.ron");
        std::fs::write(
            &path,
            r#"(bf1: (platform: Ps2, path: Some("/games/bf1.iso")), bf2: (platform: Pc, path: Some("/games/SWBF2")), dlc_dir: None, graphics_api: Auto)"#,
        )
        .unwrap();
        let loaded = LauncherConfig::load(&path);
        assert_eq!(loaded.bf1, SourceConfig::single(Platform::Ps2, "/games/bf1.iso"));
        assert_eq!(loaded.bf2, SourceConfig::single(Platform::Pc, "/games/SWBF2"));
        loaded.save(&path).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("path:"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switching_the_shown_platform_keeps_the_other_paths() {
        let mut source = SourceConfig::single(Platform::Pc, "/games/SWBF2");
        source.platform = Platform::Ps2;
        assert_eq!(source.active_path(), None);
        assert_eq!(source.path(Platform::Pc), Some(Path::new("/games/SWBF2")));
        assert_eq!(
            source.title_parts(),
            [(Platform::Pc, false), (Platform::Ps2, true)]
        );
        source.set_path(Platform::Ps2, PathBuf::from("/games/bf2.iso"));
        source.set_path(Platform::Xbox, PathBuf::from("/games/bf2x.iso"));
        assert_eq!(source.sources().count(), 3);
        source.clear(Platform::Ps2);
        assert_eq!(
            source.sources().map(|(p, _)| p).collect::<Vec<_>>(),
            [Platform::Pc, Platform::Xbox]
        );
        assert_eq!(
            source.title_parts(),
            [(Platform::Pc, false), (Platform::Ps2, true), (Platform::Xbox, false)]
        );
    }

    #[test]
    fn missing_or_corrupt_config_falls_back_to_default() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_bad_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("launcher.ron");
        assert_eq!(LauncherConfig::load(&path), LauncherConfig::default());

        std::fs::write(&path, "not valid ron {{{").unwrap();
        assert_eq!(LauncherConfig::load(&path), LauncherConfig::default());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
