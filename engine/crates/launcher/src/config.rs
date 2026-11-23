//! Persistent launcher configuration (`openbf_data/launcher.ron`).
//!
//! Stores the selected Battlefront I and Battlefront II source paths and
//! platforms, optional DLC heroes folder, and preferred graphics API override.
//! Writes are atomic (`.tmp` then rename) so an interrupted save never leaves a
//! partial config behind.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Which edition of a game the user is supplying.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// One game's selected input source.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub path: Option<PathBuf>,
}

impl SourceConfig {
    /// True when a path is selected and currently exists on disk with the
    /// expected filesystem kind (directory for PC, file for PS2/Xbox).
    pub fn is_present(&self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        if self.platform.picks_folder() {
            path.is_dir()
        } else {
            path.is_file()
        }
    }

    /// Short display label for the read-only path field.
    pub fn display_name(&self) -> Cow<'_, str> {
        path_label(self.path.as_deref())
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
        ron::from_str(&text).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_config() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_cfg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("openbf_data").join("launcher.ron");

        let cfg = LauncherConfig {
            bf1: SourceConfig {
                platform: Platform::Ps2,
                path: Some(PathBuf::from("/games/bf1.iso")),
            },
            bf2: SourceConfig {
                platform: Platform::Pc,
                path: Some(PathBuf::from("/games/SWBF2")),
            },
            dlc_dir: Some(PathBuf::from("/games/DLC_XBOX")),
            graphics_api: GraphicsApi::Vulkan,
        };
        cfg.save(&path).expect("save config");
        let loaded = LauncherConfig::load(&path);
        assert_eq!(loaded, cfg);

        let _ = std::fs::remove_dir_all(&dir);
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
