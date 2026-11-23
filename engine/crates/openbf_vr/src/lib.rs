//! OpenBF VR bundled engine extension.
//!
//! Provides OpenXR stereo headset rendering, motion-controller spatial aim and
//! melee/grenade tracking, curved visor HUD compositing, and VR menu cursor
//! interaction via [`extensions::runtime::EngineExtension`].

#[cfg(any(windows, target_os = "linux"))]
mod actions;
#[cfg(any(windows, target_os = "linux"))]
mod gesture;
#[cfg(any(windows, target_os = "linux"))]
mod graphics;
#[cfg(any(windows, target_os = "linux"))]
mod panel;
#[cfg(any(windows, target_os = "linux"))]
mod rig;
#[cfg(any(windows, target_os = "linux"))]
mod session;

use anyhow::Result;
use extensions::runtime::{EarlyDisplayContext, EngineExtension};
use extensions::{bundled_vr_manifest, ExtensionManifest};

/// Bundled OpenBF VR extension descriptor.
#[derive(Debug, Default, Clone, Copy)]
pub struct VrExtension;

impl EngineExtension for VrExtension {
    fn manifest(&self) -> ExtensionManifest {
        bundled_vr_manifest(true)
    }

    fn early_init_display(&self) -> Result<Box<dyn EarlyDisplayContext>> {
        #[cfg(any(windows, target_os = "linux"))]
        {
            Ok(Box::new(session::Runtime::new()?))
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            anyhow::bail!("OpenBF VR is only supported on Windows and Linux")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extensions::{Capability, ExclusiveCapability, ExtensionKind, ExtensionOrigin, OPENBF_VR_ID};

    #[test]
    fn vr_extension_declares_expected_manifest_and_exclusive_display_capability() {
        let ext = VrExtension;
        let m = ext.manifest();
        assert_eq!(m.id, OPENBF_VR_ID);
        assert_eq!(m.name, "OpenBF VR");
        assert_eq!(m.kind, ExtensionKind::EngineExtension);
        assert_eq!(m.origin, ExtensionOrigin::Bundled);
        assert!(m.capabilities.contains(&Capability::DisplayBackend));
        assert!(m.capabilities.contains(&Capability::StereoPresentation));
        assert!(m.capabilities.contains(&Capability::TrackedWeapons));
        assert_eq!(
            m.exclusive_capabilities,
            vec![ExclusiveCapability::DisplayBackend]
        );
        #[cfg(any(windows, target_os = "linux"))]
        assert!(m.supported);
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn invalid_openxr_loader_override_fails_early_init_cleanly() {
        let _prev = std::env::var_os("OPENBF_XR_LOADER");
        // Verify load_entry fails cleanly without panicking when pointed at a missing file.
        // We test the helper directly without mutating process-wide env vars in parallel tests.
        let missing = std::path::Path::new("nonexistent_openxr_loader_12345.dll");
        let res = unsafe { openxr::Entry::load_from(missing, &()) };
        assert!(res.is_err());
    }
}
