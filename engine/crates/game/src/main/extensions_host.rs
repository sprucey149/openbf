//! Engine extension discovery, startup activation, and error recovery host.

use anyhow::{anyhow, bail, Result};
use extensions::profile::{
    discover_data_dir, profile_path, startup_error_path, ExtensionProfile, StartupErrorReport,
    StartupOverrides, LAUNCHED_BY_LAUNCHER_ENV,
};
use extensions::registry::{ActivationPlan, DisplayBackendSelection, ExtensionRegistry};
use extensions::runtime::{EarlyDisplayContext, EngineExtension};
use extensions::{bundled_vr_manifest, ExtensionManifest, OPENBF_VR_ID};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Return all compiled-in bundled engine extensions.
pub(super) fn bundled_extensions() -> Vec<Box<dyn EngineExtension>> {
    #[allow(unused_mut)]
    let mut list: Vec<Box<dyn EngineExtension>> = Vec::new();
    #[cfg(feature = "vr")]
    {
        list.push(Box::new(openbf_vr::VrExtension));
    }
    list
}

/// Build the registry of bundled extensions (including unsupported placeholders
/// when built without optional extension features such as `vr`).
pub(super) fn build_registry() -> ExtensionRegistry {
    let mut reg = ExtensionRegistry::default();
    for ext in bundled_extensions() {
        reg.register(ext.manifest());
    }
    if reg.get(OPENBF_VR_ID).is_none() {
        reg.register(bundled_vr_manifest(false));
    }
    reg
}

/// Ordered list of discovered extension manifests for CLI inspection (`--list-extensions`).
pub(super) fn discovered_manifests() -> Vec<ExtensionManifest> {
    build_registry().manifests().cloned().collect()
}

/// Print discovered extension manifests as RON for the launcher and tooling.
pub(super) fn print_extensions_ron() -> Result<()> {
    let manifests = discovered_manifests();
    let pretty = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .new_line("\n".to_string());
    let text = ron::ser::to_string_pretty(&manifests, pretty)?;
    println!("{text}");
    Ok(())
}

/// Resolved startup plan and optional early-initialized display backend.
pub(super) struct ResolvedStartup {
    pub data_dir: PathBuf,
    pub display_ext_meta: Option<(String, String)>,
    pub early_display: Option<Box<dyn EarlyDisplayContext>>,
}

/// Resolve enabled extensions from `openbf_data/extensions.ron` + CLI overrides
/// and run early display initialization when an exclusive display backend is active.
pub(super) fn resolve_startup(
    overrides: &StartupOverrides,
    interactive: bool,
) -> Result<ResolvedStartup> {
    let data_dir = discover_data_dir();
    resolve_startup_in(data_dir, overrides, interactive)
}

fn resolve_startup_in(
    data_dir: PathBuf,
    overrides: &StartupOverrides,
    interactive: bool,
) -> Result<ResolvedStartup> {
    let profile = ExtensionProfile::load(&profile_path(&data_dir));
    let registry = build_registry();

    let mut effective = overrides.clone();
    if !interactive {
        // Non-interactive commands (`--headless`, `--bench`, `--sim-hash`) must
        // stay unaffected by a saved VR profile unless explicitly forced via CLI.
        if effective.enable.iter().any(|id| {
            registry
                .get(id)
                .is_some_and(|m| !m.exclusive_capabilities.is_empty())
        }) {
            bail!("--vr requires an interactive game");
        }
        effective.force_desktop = true;
    }

    let plan: ActivationPlan = match registry.resolve(&profile, &effective) {
        Ok(plan) => plan,
        Err(errors) => {
            let summary = errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            let (ext_id, ext_name) = failing_extension_meta(&registry, &errors);
            let err = anyhow!(
                "{summary}\n\nLaunch with `--desktop` to start using the desktop renderer, or adjust extensions in the launcher Mods tab."
            );
            if interactive {
                report_startup_error(&data_dir, &ext_id, &ext_name, &err);
            }
            return Err(err);
        }
    };

    for warning in &plan.warnings {
        eprintln!("extensions: {warning}");
    }

    let (display_ext_meta, early_display) = match &plan.display_backend {
        DisplayBackendSelection::Desktop => (None, None),
        DisplayBackendSelection::Extension(ext_id) => {
            let manifest = registry
                .get(ext_id)
                .cloned()
                .unwrap_or_else(|| bundled_vr_manifest(false));
            let ext = bundled_extensions()
                .into_iter()
                .find(|candidate| candidate.manifest().id == *ext_id)
                .ok_or_else(|| {
                    let err = anyhow!("extension `{ext_id}` is not compiled into this binary");
                    if interactive {
                        report_startup_error(&data_dir, ext_id, &manifest.name, &err);
                    }
                    err
                })?;
            match ext.early_init_display() {
                Ok(ctx) => (Some((manifest.id, manifest.name)), Some(ctx)),
                Err(err) => {
                    if interactive {
                        report_startup_error(&data_dir, ext_id, &manifest.name, &err);
                    }
                    return Err(err);
                }
            }
        }
    };

    Ok(ResolvedStartup {
        data_dir,
        display_ext_meta,
        early_display,
    })
}

fn failing_extension_meta(
    registry: &ExtensionRegistry,
    errors: &[extensions::registry::ResolutionError],
) -> (String, String) {
    use extensions::registry::ResolutionError;
    let id = errors.first().map_or_else(
        || OPENBF_VR_ID.to_string(),
        |err| match err {
            ResolutionError::UnknownExtension { id }
            | ResolutionError::Unsupported { id, .. }
            | ResolutionError::MissingDependency { id, .. }
            | ResolutionError::ExplicitConflict { id, .. }
            | ResolutionError::DependencyCycle { id } => id.clone(),
            ResolutionError::ExclusiveCapabilityConflict { first, .. } => first.clone(),
        },
    );
    let name = registry
        .get(&id)
        .map_or_else(|| id.clone(), |m| m.name.clone());
    (id, name)
}

/// Record an extension startup failure to `openbf_data/last_startup_error.ron` and
/// `openbf_data/logs/startup_error.log`, and reopen `openbf-launcher` when launched from it.
pub(super) fn report_startup_error(
    data_dir: &Path,
    ext_id: &str,
    ext_name: &str,
    err: &anyhow::Error,
) {
    let summary = format!("{err}");
    let details = format!(
        "{err:#}\n\nTo recover, connect your VR headset and start its active OpenXR runtime \
         (SteamVR, Oculus, Virtual Desktop, or Monado), or launch OpenBF in Desktop mode (`--desktop`)."
    );
    let report = StartupErrorReport::new(ext_id, ext_name, &summary, &details, true);
    if let Err(write_err) = report.save(&startup_error_path(data_dir)) {
        eprintln!("extensions: failed to write startup error report: {write_err:#}");
    }
    let log_dir = data_dir.join("logs");
    if std::fs::create_dir_all(&log_dir).is_ok() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("startup_error.log"))
        {
            let _ = writeln!(file, "[{}] {}: {err:#}", ext_id, report.timestamp_secs);
        }
    }
    eprintln!("Error: {ext_name} ({ext_id}) failed to initialize: {err:#}");
    eprintln!("Hint: Run with `--desktop` or disable `{ext_id}` in the launcher Mods tab.");

    if std::env::var_os(LAUNCHED_BY_LAUNCHER_ENV).is_some() {
        relaunch_launcher();
    }
}

/// Clear any prior startup error report once display initialization succeeds.
pub(super) fn clear_startup_error(data_dir: &Path) {
    StartupErrorReport::clear(&startup_error_path(data_dir));
}

fn relaunch_launcher() {
    let suffix = std::env::consts::EXE_SUFFIX;
    let launcher_name = format!("openbf-launcher{suffix}");
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut candidates = Vec::new();
    if let Some(dir) = exe.parent() {
        candidates.push(dir.join(&launcher_name));
        // Checkout `dist/openbf.exe` -> `engine/target/{release,debug}/openbf-launcher.exe`.
        if let Some(root) = dir.parent() {
            let target = root.join("engine").join("target");
            candidates.push(target.join("release").join(&launcher_name));
            candidates.push(target.join("debug").join(&launcher_name));
        }
    }
    if let Some(bin) = candidates.into_iter().find(|p| p.is_file()) {
        let mut cmd = std::process::Command::new(bin);
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        }
        let _ = cmd.spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openbf_game_ext_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn registry_includes_vr_and_defaults_to_desktop() {
        let reg = build_registry();
        let vr = reg.get(OPENBF_VR_ID).expect("openbf.vr registered");
        assert_eq!(vr.name, "OpenBF VR");
        let dir = scratch("default");
        let resolved =
            resolve_startup_in(dir.clone(), &StartupOverrides::default(), false).unwrap();
        assert!(resolved.display_ext_meta.is_none());
        assert!(resolved.early_display.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_vr_profile_never_blocks_headless_and_desktop_override_recovers() {
        let dir = scratch("headless_override");
        let mut profile = ExtensionProfile::default();
        profile.set_enabled(OPENBF_VR_ID, true);
        profile.save(&profile_path(&dir)).unwrap();

        // Non-interactive mode stays on Desktop even with VR saved in profile.
        let non_interactive =
            resolve_startup_in(dir.clone(), &StartupOverrides::default(), false).unwrap();
        assert!(non_interactive.display_ext_meta.is_none());

        // Interactive mode with --desktop override also stays on Desktop.
        let forced_desktop = resolve_startup_in(
            dir.clone(),
            &StartupOverrides {
                force_desktop: true,
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert!(forced_desktop.display_ext_meta.is_none());

        // Explicit --vr in non-interactive mode is rejected.
        let vr_headless = resolve_startup_in(
            dir.clone(),
            &StartupOverrides {
                enable: vec![OPENBF_VR_ID.to_string()],
                ..Default::default()
            },
            false,
        );
        assert!(vr_headless.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_error_report_writes_and_clears_cleanly() {
        let dir = scratch("err_report");
        let err = anyhow!("missing OpenXR runtime");
        report_startup_error(&dir, OPENBF_VR_ID, "OpenBF VR", &err);
        let report = StartupErrorReport::load(&startup_error_path(&dir)).expect("report saved");
        assert_eq!(report.extension_id, OPENBF_VR_ID);
        assert!(report.summary.contains("missing OpenXR runtime"));
        assert!(dir.join("logs").join("startup_error.log").is_file());

        clear_startup_error(&dir);
        assert!(StartupErrorReport::load(&startup_error_path(&dir)).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
