//! Bundled engine extension registry, persistent profiles, and runtime traits.
//!
//! Provides a shared data model used by both the native `openbf-launcher` (for
//! discovering extensions, toggling enabled state in `openbf_data/extensions.ron`,
//! validating dependencies/conflicts, and surfacing startup failures) and the
//! `game` runtime (with `--features runtime` enabled).

pub mod profile;
pub mod registry;
#[cfg(feature = "runtime")]
pub mod runtime;

use serde::{Deserialize, Serialize};

/// Stable identifier of the bundled OpenBF VR extension.
pub const OPENBF_VR_ID: &str = "openbf.vr";

/// What kind of module an entry in the Mods catalog represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExtensionKind {
    EngineExtension,
    GameplayMod,
    ContentPack,
}

impl ExtensionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::EngineExtension => "Engine Extension",
            Self::GameplayMod => "Gameplay Mod",
            Self::ContentPack => "Content Pack",
        }
    }
}

/// Whether an extension ships compiled into the engine or comes from outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExtensionOrigin {
    Bundled,
    ThirdParty,
}

impl ExtensionOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bundled => "Bundled",
            Self::ThirdParty => "Third-Party",
        }
    }
}

/// Non-exclusive capabilities an extension provides to the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Capability {
    DisplayBackend,
    SpatialInput,
    TrackedWeapons,
    StereoPresentation,
    CustomHud,
    AudioListener,
    Haptics,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Self::DisplayBackend => "Display Backend",
            Self::SpatialInput => "Spatial Input",
            Self::TrackedWeapons => "Tracked Weapons",
            Self::StereoPresentation => "Stereo Presentation",
            Self::CustomHud => "Custom HUD",
            Self::AudioListener => "Audio Listener",
            Self::Haptics => "Haptics",
        }
    }
}

/// Capabilities of which at most one provider may be active at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExclusiveCapability {
    DisplayBackend,
}

impl ExclusiveCapability {
    pub fn label(self) -> &'static str {
        match self {
            Self::DisplayBackend => "Display Backend",
        }
    }
}

/// Declarative metadata and capability specification for an extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: ExtensionKind,
    pub origin: ExtensionOrigin,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub exclusive_capabilities: Vec<ExclusiveCapability>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default = "default_true")]
    pub supported: bool,
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Canonical manifest for the bundled OpenBF VR extension.
pub fn bundled_vr_manifest(vr_compiled: bool) -> ExtensionManifest {
    let platform_supported = cfg!(any(windows, target_os = "linux"));
    let (supported, unsupported_reason) = if !platform_supported {
        (
            false,
            Some("OpenXR VR is only supported on Windows and Linux.".to_string()),
        )
    } else if !vr_compiled {
        (
            false,
            Some("This game binary was built without OpenBF VR support.".to_string()),
        )
    } else {
        (true, None)
    };

    ExtensionManifest {
        id: OPENBF_VR_ID.to_string(),
        name: "OpenBF VR".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "OpenXR stereo headset rendering, tracked motion controllers, physical weapon aiming, two-handed grips, tracked lightsaber swings, grenade throwing, and curved visor HUD.".to_string(),
        kind: ExtensionKind::EngineExtension,
        origin: ExtensionOrigin::Bundled,
        capabilities: vec![
            Capability::DisplayBackend,
            Capability::SpatialInput,
            Capability::TrackedWeapons,
            Capability::StereoPresentation,
            Capability::CustomHud,
            Capability::AudioListener,
            Capability::Haptics,
        ],
        exclusive_capabilities: vec![ExclusiveCapability::DisplayBackend],
        dependencies: Vec::new(),
        conflicts: Vec::new(),
        supported,
        unsupported_reason,
    }
}

/// Default bundled extension catalog (used as fallback when probing the game binary is unavailable).
pub fn bundled_extensions_catalog(vr_compiled: bool) -> Vec<ExtensionManifest> {
    vec![bundled_vr_manifest(vr_compiled)]
}
