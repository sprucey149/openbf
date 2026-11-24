//! Extension discovery registry, capability conflict checks, and deterministic resolution.

use crate::profile::{ExtensionProfile, StartupOverrides};
use crate::{ExclusiveCapability, ExtensionManifest};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Which display backend drives rendering and window/headset presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayBackendSelection {
    /// Default single-window desktop renderer.
    Desktop,
    /// Exclusive display backend owned by the named extension ID.
    Extension(String),
}

/// Validation failure encountered while resolving active extensions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionError {
    UnknownExtension {
        id: String,
    },
    Unsupported {
        id: String,
        name: String,
        reason: String,
    },
    MissingDependency {
        id: String,
        dependency: String,
    },
    ExplicitConflict {
        id: String,
        conflict: String,
    },
    ExclusiveCapabilityConflict {
        capability: ExclusiveCapability,
        first: String,
        second: String,
    },
    DependencyCycle {
        id: String,
    },
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownExtension { id } => write!(f, "requested unknown extension `{id}`"),
            Self::Unsupported { id, name, reason } => write!(
                f,
                "extension `{id}` ({name}) is not supported on this build or platform: {reason}"
            ),
            Self::MissingDependency { id, dependency } => write!(
                f,
                "extension `{id}` requires missing or disabled dependency `{dependency}`"
            ),
            Self::ExplicitConflict { id, conflict } => {
                write!(
                    f,
                    "extension `{id}` conflicts with active extension `{conflict}`"
                )
            }
            Self::ExclusiveCapabilityConflict {
                capability,
                first,
                second,
            } => write!(
                f,
                "exclusive capability `{}` cannot be provided by both `{first}` and `{second}`",
                capability.label()
            ),
            Self::DependencyCycle { id } => {
                write!(f, "dependency cycle detected involving extension `{id}`")
            }
        }
    }
}

impl std::error::Error for ResolutionError {}

/// Validated startup plan produced by [`ExtensionRegistry::resolve`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationPlan {
    /// Active extension IDs in deterministic activation order (dependencies first).
    pub active: Vec<String>,
    /// Selected display backend provider.
    pub display_backend: DisplayBackendSelection,
    /// Non-fatal diagnostics (for example, unknown stale entries in `extensions.ron`).
    pub warnings: Vec<String>,
}

/// Registry of discovered extensions and their declarative manifests.
#[derive(Clone, Debug, Default)]
pub struct ExtensionRegistry {
    manifests: BTreeMap<String, ExtensionManifest>,
}

impl ExtensionRegistry {
    pub fn new(manifests: impl IntoIterator<Item = ExtensionManifest>) -> Self {
        let mut reg = Self::default();
        for manifest in manifests {
            reg.register(manifest);
        }
        reg
    }

    pub fn register(&mut self, manifest: ExtensionManifest) {
        self.manifests.insert(manifest.id.clone(), manifest);
    }

    pub fn get(&self, id: &str) -> Option<&ExtensionManifest> {
        self.manifests.get(id)
    }

    pub fn manifests(&self) -> impl Iterator<Item = &ExtensionManifest> {
        self.manifests.values()
    }

    /// Resolve which extensions should activate from `profile` + `overrides`.
    ///
    /// Rules:
    /// - Profile flags seed initial enabled state (`false` by default).
    /// - `overrides.enable` enables specific extensions (and validates unknown IDs).
    /// - `overrides.disable` disables specific extensions.
    /// - `overrides.force_desktop` suppresses every extension declaring
    ///   [`ExclusiveCapability::DisplayBackend`] regardless of profile/enable state.
    /// - Unsupported extensions cannot activate.
    /// - Missing dependencies, explicit conflicts, and exclusive capability
    ///   collisions are reported cleanly without using arbitrary list order.
    pub fn resolve(
        &self,
        profile: &ExtensionProfile,
        overrides: &StartupOverrides,
    ) -> Result<ActivationPlan, Vec<ResolutionError>> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut candidate_ids = BTreeSet::new();

        for (id, enabled) in &profile.enabled {
            if !*enabled || id.starts_with("content.") {
                continue;
            }
            if !self.manifests.contains_key(id) {
                warnings.push(format!(
                    "ignoring unknown extension `{id}` recorded in extensions profile"
                ));
                continue;
            }
            candidate_ids.insert(id.clone());
        }

        for id in &overrides.enable {
            if !self.manifests.contains_key(id) {
                errors.push(ResolutionError::UnknownExtension { id: id.clone() });
            } else {
                candidate_ids.insert(id.clone());
            }
        }

        for id in &overrides.disable {
            candidate_ids.remove(id);
        }

        if overrides.force_desktop {
            candidate_ids.retain(|id| {
                self.manifests
                    .get(id)
                    .is_some_and(|m| m.exclusive_capabilities.is_empty())
            });
        }

        for id in &candidate_ids {
            let manifest = &self.manifests[id];
            if !manifest.supported {
                errors.push(ResolutionError::Unsupported {
                    id: id.clone(),
                    name: manifest.name.clone(),
                    reason: manifest
                        .unsupported_reason
                        .clone()
                        .unwrap_or_else(|| "unsupported on this build or platform".to_string()),
                });
            }
            for dep in &manifest.dependencies {
                if !candidate_ids.contains(dep) {
                    errors.push(ResolutionError::MissingDependency {
                        id: id.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
            for conflict in &manifest.conflicts {
                if candidate_ids.contains(conflict) && id < conflict {
                    errors.push(ResolutionError::ExplicitConflict {
                        id: id.clone(),
                        conflict: conflict.clone(),
                    });
                } else if candidate_ids.contains(conflict)
                    && !self.manifests[conflict].conflicts.contains(id)
                {
                    errors.push(ResolutionError::ExplicitConflict {
                        id: id.clone(),
                        conflict: conflict.clone(),
                    });
                }
            }
        }

        // Check exclusive capabilities without letting mod-list order silently pick a winner.
        let mut display_providers: Vec<String> = Vec::new();
        for id in &candidate_ids {
            let manifest = &self.manifests[id];
            if manifest
                .exclusive_capabilities
                .contains(&ExclusiveCapability::DisplayBackend)
            {
                display_providers.push(id.clone());
            }
        }
        if display_providers.len() > 1 {
            let first = display_providers[0].clone();
            for second in display_providers.iter().skip(1) {
                errors.push(ResolutionError::ExclusiveCapabilityConflict {
                    capability: ExclusiveCapability::DisplayBackend,
                    first: first.clone(),
                    second: second.clone(),
                });
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let active = match topological_sort(&candidate_ids, &self.manifests) {
            Ok(sorted) => sorted,
            Err(cycle_id) => {
                return Err(vec![ResolutionError::DependencyCycle { id: cycle_id }]);
            }
        };

        let display_backend = match display_providers.into_iter().next() {
            Some(id) => DisplayBackendSelection::Extension(id),
            None => DisplayBackendSelection::Desktop,
        };

        Ok(ActivationPlan {
            active,
            display_backend,
            warnings,
        })
    }
}

/// Deterministic topological sort: alphabetically smallest ready ID first.
fn topological_sort(
    active: &BTreeSet<String>,
    manifests: &BTreeMap<String, ExtensionManifest>,
) -> Result<Vec<String>, String> {
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in active {
        in_degree.insert(id.clone(), 0);
    }
    for id in active {
        for dep in &manifests[id].dependencies {
            if active.contains(dep) {
                *in_degree.get_mut(id).unwrap() += 1;
                dependents.entry(dep.clone()).or_default().push(id.clone());
            }
        }
    }

    let mut ready: BTreeSet<String> = in_degree
        .iter()
        .filter_map(|(id, deg)| (*deg == 0).then_some(id.clone()))
        .collect();
    let mut ordered = Vec::with_capacity(active.len());

    while let Some(next) = ready.pop_first() {
        ordered.push(next.clone());
        if let Some(children) = dependents.get(&next) {
            for child in children {
                let deg = in_degree.get_mut(child).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }

    if ordered.len() != active.len() {
        let stuck = active
            .iter()
            .find(|id| !ordered.contains(*id))
            .cloned()
            .unwrap_or_default();
        return Err(stuck);
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bundled_vr_manifest, Capability, ExtensionKind, ExtensionOrigin, OPENBF_VR_ID};

    fn sample_ext(id: &str) -> ExtensionManifest {
        ExtensionManifest {
            id: id.to_string(),
            name: id.to_string(),
            version: "0.1.0".to_string(),
            description: "test extension".to_string(),
            kind: ExtensionKind::EngineExtension,
            origin: ExtensionOrigin::ThirdParty,
            capabilities: vec![],
            exclusive_capabilities: vec![],
            dependencies: vec![],
            conflicts: vec![],
            supported: true,
            unsupported_reason: None,
            author: None,
            order_sensitive: false,
            legacy_addon_id: None,
        }
    }

    #[test]
    fn bundled_vr_is_disabled_by_default_and_desktop_is_selected() {
        let reg = ExtensionRegistry::new([bundled_vr_manifest(true)]);
        let plan = reg
            .resolve(&ExtensionProfile::default(), &StartupOverrides::default())
            .unwrap();
        assert!(plan.active.is_empty());
        assert_eq!(plan.display_backend, DisplayBackendSelection::Desktop);
    }

    #[test]
    fn cli_overrides_enable_vr_and_force_desktop_recovers() {
        let reg = ExtensionRegistry::new([bundled_vr_manifest(true)]);
        let mut profile = ExtensionProfile::default();

        // --vr enables VR even when disabled in profile.
        let vr_plan = reg
            .resolve(
                &profile,
                &StartupOverrides {
                    enable: vec![OPENBF_VR_ID.to_string()],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(vr_plan.active, vec![OPENBF_VR_ID]);
        assert_eq!(
            vr_plan.display_backend,
            DisplayBackendSelection::Extension(OPENBF_VR_ID.to_string())
        );

        // Saved VR enabled in profile -> --desktop recovers to Desktop.
        profile.set_enabled(OPENBF_VR_ID, true);
        let desktop_plan = reg
            .resolve(
                &profile,
                &StartupOverrides {
                    force_desktop: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(desktop_plan.active.is_empty());
        assert_eq!(
            desktop_plan.display_backend,
            DisplayBackendSelection::Desktop
        );
    }

    #[test]
    fn unsupported_extensions_cannot_be_activated() {
        let reg = ExtensionRegistry::new([bundled_vr_manifest(false)]);
        let mut profile = ExtensionProfile::default();
        profile.set_enabled(OPENBF_VR_ID, true);
        let errs = reg
            .resolve(&profile, &StartupOverrides::default())
            .unwrap_err();
        assert!(matches!(errs[0], ResolutionError::Unsupported { .. }));
    }

    #[test]
    fn exclusive_display_backend_conflicts_are_detected_regardless_of_order() {
        let mut alt_display = sample_ext("alt.display");
        alt_display.capabilities.push(Capability::DisplayBackend);
        alt_display
            .exclusive_capabilities
            .push(ExclusiveCapability::DisplayBackend);

        let reg = ExtensionRegistry::new([bundled_vr_manifest(true), alt_display]);
        let mut profile = ExtensionProfile::default();
        profile.set_enabled(OPENBF_VR_ID, true);
        profile.set_enabled("alt.display", true);
        profile.order = vec!["alt.display".into(), OPENBF_VR_ID.into()];

        let errs = reg
            .resolve(&profile, &StartupOverrides::default())
            .unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ResolutionError::ExclusiveCapabilityConflict { .. })));
    }

    #[test]
    fn dependencies_and_conflicts_resolve_deterministically() {
        let base = sample_ext("a.base");
        let mut addon = sample_ext("b.addon");
        addon.dependencies.push("a.base".into());
        let mut rival = sample_ext("c.rival");
        rival.conflicts.push("b.addon".into());

        let reg = ExtensionRegistry::new([addon, base, rival]);
        let mut profile = ExtensionProfile::default();
        profile.set_enabled("b.addon", true);

        let missing = reg
            .resolve(&profile, &StartupOverrides::default())
            .unwrap_err();
        assert!(matches!(
            missing[0],
            ResolutionError::MissingDependency { .. }
        ));

        profile.set_enabled("a.base", true);
        let ok = reg.resolve(&profile, &StartupOverrides::default()).unwrap();
        assert_eq!(ok.active, vec!["a.base", "b.addon"]);

        profile.set_enabled("c.rival", true);
        let conflict = reg
            .resolve(&profile, &StartupOverrides::default())
            .unwrap_err();
        assert!(matches!(
            conflict[0],
            ResolutionError::ExplicitConflict { .. }
        ));
    }

    #[test]
    fn enabled_content_mods_do_not_warn_or_affect_display_backend() {
        let reg = ExtensionRegistry::new([bundled_vr_manifest(true)]);
        let mut profile = ExtensionProfile::default();
        profile.set_enabled("content.abr", true);
        profile.set_enabled("content.overhaul", true);
        profile.order = vec!["content.overhaul".into(), "content.abr".into()];
        let plan = reg.resolve(&profile, &StartupOverrides::default()).unwrap();
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.display_backend, DisplayBackendSelection::Desktop);
    }
}
