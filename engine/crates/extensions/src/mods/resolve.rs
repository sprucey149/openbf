//! Deterministic content mod load-order resolution, dependency validation,
//! and override conflict diagnostics.
//!
//! Convention: **higher in the list = higher override priority** (index 0 wins).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::mods::DiscoveredMod;
use crate::profile::ExtensionProfile;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentResolutionError {
    InvalidOrUnsupported {
        id: String,
        name: String,
        reason: String,
    },
    MissingDependency {
        id: String,
        name: String,
        dependency: String,
    },
    DependencyCycle {
        id: String,
    },
    /// When mod A depends on mod B and overrides resources from B, A must sit
    /// above B (lower index in `order`) so A wins.
    DependencyOrderViolation {
        id: String,
        name: String,
        dependency: String,
        dependency_name: String,
    },
    ExplicitConflict {
        id: String,
        name: String,
        conflict: String,
        conflict_name: String,
    },
}

impl fmt::Display for ContentResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOrUnsupported { name, reason, .. } => {
                write!(f, "Enabled mod `{name}` cannot be loaded: {reason}")
            }
            Self::MissingDependency {
                name, dependency, ..
            } => write!(
                f,
                "`{name}` requires `{dependency}`, which is missing or disabled."
            ),
            Self::DependencyCycle { id } => {
                write!(f, "Dependency cycle detected involving content mod `{id}`.")
            }
            Self::DependencyOrderViolation {
                name,
                dependency_name,
                ..
            } => write!(
                f,
                "Load order error: `{name}` depends on `{dependency_name}` and must be placed ABOVE it."
            ),
            Self::ExplicitConflict {
                name, conflict_name, ..
            } => write!(
                f,
                "`{name}` is marked incompatible with enabled mod `{conflict_name}`."
            ),
        }
    }
}

impl std::error::Error for ContentResolutionError {}

/// Non-fatal diagnostic emitted when two enabled mods declare overrides to the
/// same permitted stock or shared resource path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceOverrideConflict {
    pub resource_path: String,
    pub winner_id: String,
    pub winner_name: String,
    pub overridden_ids: Vec<String>,
    pub overridden_names: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentResolutionPlan {
    /// Enabled valid content mod IDs in load order (index 0 = highest override priority).
    pub active_order: Vec<String>,
    pub conflicts: Vec<ResourceOverrideConflict>,
    pub warnings: Vec<String>,
}

/// Resolve enabled content mods using `profile.enabled` and `profile.order`
/// without involving engine-extension exclusive display backends.
pub fn resolve_content_mods(
    mods: &[DiscoveredMod],
    profile: &ExtensionProfile,
) -> Result<ContentResolutionPlan, Vec<ContentResolutionError>> {
    let by_id: BTreeMap<&str, &DiscoveredMod> = mods.iter().map(|m| (m.id.as_str(), m)).collect();
    let discovered_ids: Vec<String> = mods.iter().map(|m| m.id.clone()).collect();
    let ordered_ids = profile.ordered_visible(&discovered_ids);

    let mut enabled_ordered: Vec<&DiscoveredMod> = Vec::new();
    for id in ordered_ids {
        if profile.is_enabled(id) {
            if let Some(&m) = by_id.get(id) {
                enabled_ordered.push(m);
            }
        }
    }

    let mut errors = Vec::new();
    let active_pos: BTreeMap<&str, usize> = enabled_ordered
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();

    for m in &enabled_ordered {
        if !m.compatibility.is_playable() {
            errors.push(ContentResolutionError::InvalidOrUnsupported {
                id: m.id.clone(),
                name: m.display_name.clone(),
                reason: m
                    .compatibility
                    .reason()
                    .unwrap_or("incompatible mod")
                    .to_owned(),
            });
            continue;
        }
        let my_pos = active_pos[m.id.as_str()];
        for dep in &m.dependencies {
            match active_pos.get(dep.as_str()) {
                None => {
                    errors.push(ContentResolutionError::MissingDependency {
                        id: m.id.clone(),
                        name: m.display_name.clone(),
                        dependency: dep.clone(),
                    });
                }
                Some(&dep_pos) if dep_pos < my_pos => {
                    let dep_name = by_id
                        .get(dep.as_str())
                        .map_or(dep.as_str(), |d| d.display_name.as_str())
                        .to_owned();
                    errors.push(ContentResolutionError::DependencyOrderViolation {
                        id: m.id.clone(),
                        name: m.display_name.clone(),
                        dependency: dep.clone(),
                        dependency_name: dep_name,
                    });
                }
                Some(_) => {}
            }
        }
        for conf in &m.conflicts {
            if let Some(&other) = by_id.get(conf.as_str()) {
                if active_pos.contains_key(conf.as_str())
                    && (m.id < other.id || !other.conflicts.contains(&m.id))
                {
                    errors.push(ContentResolutionError::ExplicitConflict {
                        id: m.id.clone(),
                        name: m.display_name.clone(),
                        conflict: other.id.clone(),
                        conflict_name: other.display_name.clone(),
                    });
                }
            }
        }
    }

    // Cycle detection across enabled mods using declared dependencies.
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for m in &enabled_ordered {
        if !visited.contains(m.id.as_str()) {
            if let Some(cycle_id) =
                detect_cycle(&m.id, &by_id, &active_pos, &mut visiting, &mut visited)
            {
                if !errors
                    .iter()
                    .any(|e| matches!(e, ContentResolutionError::DependencyCycle { id } if id == &cycle_id))
                {
                    errors.push(ContentResolutionError::DependencyCycle { id: cycle_id });
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // Compute override conflicts: both explicit declared_overrides and active
    // stock-replacement operations targeting the same logical stock LVL.
    let mut by_resource: BTreeMap<String, Vec<&DiscoveredMod>> = BTreeMap::new();
    for m in &enabled_ordered {
        let mut seen_for_mod: BTreeSet<String> = BTreeSet::new();
        for rel in &m.declared_overrides {
            let key = rel.replace('\\', "/").to_ascii_lowercase();
            if seen_for_mod.insert(key.clone()) {
                by_resource.entry(key).or_default().push(m);
            }
        }
        for op in m.active_ops() {
            if let Some(target) = &op.target {
                let key = target.label();
                if seen_for_mod.insert(key.clone()) {
                    by_resource.entry(key).or_default().push(m);
                }
            }
        }
    }

    let mut conflicts = Vec::new();
    for (resource_path, owners) in by_resource {
        if owners.len() > 1 {
            conflicts.push(ResourceOverrideConflict {
                resource_path,
                winner_id: owners[0].id.clone(),
                winner_name: owners[0].display_name.clone(),
                overridden_ids: owners[1..].iter().map(|m| m.id.clone()).collect(),
                overridden_names: owners[1..].iter().map(|m| m.display_name.clone()).collect(),
            });
        }
    }

    let mut warnings = Vec::new();
    for m in &enabled_ordered {
        for w in &m.warnings {
            warnings.push(format!("{}: {w}", m.display_name));
        }
    }

    Ok(ContentResolutionPlan {
        active_order: enabled_ordered.iter().map(|m| m.id.clone()).collect(),
        conflicts,
        warnings,
    })
}

fn detect_cycle<'a>(
    id: &'a str,
    by_id: &BTreeMap<&'a str, &'a DiscoveredMod>,
    active: &BTreeMap<&str, usize>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> Option<String> {
    if !visiting.insert(id) {
        return Some(id.to_owned());
    }
    if let Some(m) = by_id.get(id) {
        for dep in &m.dependencies {
            if active.contains_key(dep.as_str()) && !visited.contains(dep.as_str()) {
                if let Some((k, _)) = by_id.get_key_value(dep.as_str()) {
                    if let Some(cyc) = detect_cycle(k, by_id, active, visiting, visited) {
                        return Some(cyc);
                    }
                }
            }
        }
    }
    visiting.remove(id);
    visited.insert(id);
    None
}

/// Deterministic FNV-1a-64 digest representing active shared/override mods so
/// multiplayer peers detect incompatible override stacks before launching.
pub fn content_mod_multiplayer_digest(
    ordered_override_mods: &[(&str, &str)],
    overrides_active: bool,
) -> u64 {
    if !overrides_active || ordered_override_mods.is_empty() {
        return 0;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &(mod_id, fp) in ordered_override_mods {
        for b in mod_id
            .as_bytes()
            .iter()
            .chain([0u8].iter())
            .chain(fp.as_bytes().iter())
            .chain([0xffu8].iter())
        {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{stable_mod_id, DiscoveredMod, ModCompatibility, ModContentType};
    use std::path::PathBuf;

    fn sample_mod(id: &str, name: &str, legacy_id: Option<&str>) -> DiscoveredMod {
        DiscoveredMod {
            id: id.into(),
            namespace: crate::mods::namespace_prefix(id),
            folder_name: id.into(),
            root_dir: PathBuf::from(id),
            content_root: None,
            legacy_addon_id: legacy_id.map(str::to_owned),
            display_name: name.into(),
            version: None,
            author: None,
            description: String::new(),
            mod_type: ModContentType::LegacyMapAddon,
            compatibility: ModCompatibility::Supported,
            source_fingerprint: "deadbeef".into(),
            maps: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            custom_sides: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            unused_lvls: Vec::new(),
            resource_ops: Vec::new(),
            capabilities: Vec::new(),
            replacement_key: String::new(),
        }
    }

    #[test]
    fn duplicate_legacy_ids_produce_distinct_stable_mod_ids_and_namespaces() {
        let folders = vec!["ABR".to_string(), "ABR_remaster".to_string()];
        let id1 = stable_mod_id(&folders[0], &folders, None);
        let id2 = stable_mod_id(&folders[1], &folders, None);
        assert_ne!(id1, id2);
        assert_ne!(
            crate::mods::namespace_prefix(&id1),
            crate::mods::namespace_prefix(&id2)
        );
    }

    #[test]
    fn enforces_dependency_order_conflicts_and_override_winner() {
        let mut base = sample_mod("content.base", "Base Pack", None);
        base.declared_overrides = vec![
            "common_bf2/shared.ron".into(),
            "common_bf2/base_only.ron".into(),
        ];
        let mut addon = sample_mod("content.patch", "Patch Pack", None);
        addon.dependencies = vec!["content.base".into()];
        addon.declared_overrides = vec!["common_bf2/shared.ron".into()];

        let mods = vec![base.clone(), addon.clone()];
        let mut profile = ExtensionProfile::default();
        profile.set_enabled("content.base", true);
        profile.set_enabled("content.patch", true);
        // Wrong order: base above patch
        profile.order = vec!["content.base".into(), "content.patch".into()];

        let errs = resolve_content_mods(&mods, &profile).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ContentResolutionError::DependencyOrderViolation { .. })));

        // Move patch above base (higher priority = lower index) -> valid!
        let visible = vec!["content.base".to_string(), "content.patch".to_string()];
        assert!(profile.move_up("content.patch", &visible));
        let plan = resolve_content_mods(&mods, &profile).unwrap();
        assert_eq!(plan.active_order, ["content.patch", "content.base"]);
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].winner_id, "content.patch");
        assert_eq!(plan.conflicts[0].overridden_ids, ["content.base"]);
    }

    #[test]
    fn explicit_conflicts_and_cycles_fail_cleanly() {
        let mut a = sample_mod("content.a", "Mod A", None);
        let mut b = sample_mod("content.b", "Mod B", None);
        a.dependencies.push("content.b".into());
        b.dependencies.push("content.a".into());

        let mut profile = ExtensionProfile::default();
        profile.set_enabled("content.a", true);
        profile.set_enabled("content.b", true);
        profile.order = vec!["content.a".into(), "content.b".into()];

        let errs = resolve_content_mods(&[a.clone(), b.clone()], &profile).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ContentResolutionError::DependencyCycle { .. })));

        a.dependencies.clear();
        b.dependencies.clear();
        a.conflicts.push("content.b".into());
        let errs2 = resolve_content_mods(&[a, b], &profile).unwrap_err();
        assert!(errs2
            .iter()
            .any(|e| matches!(e, ContentResolutionError::ExplicitConflict { .. })));
    }

    #[test]
    fn active_replacement_ops_report_override_conflicts_by_load_order() {
        use crate::mods::catalog::{SourceGame, SourcePlatform, TargetRef};
        use crate::mods::replace::{OpStatus, ReplacementMode, ResourceOp};

        let make_op = |path: &str| ResourceOp {
            source: path.into(),
            source_sha256: "abc".into(),
            source_size: 10,
            detected_game: Some(SourceGame::Bf2),
            detected_platform: Some(SourcePlatform::Pc),
            target: Some(TargetRef {
                game: SourceGame::Bf2,
                platform: SourcePlatform::Pc,
                path: path.into(),
            }),
            mode: ReplacementMode::Full,
            status: OpStatus::Active,
            candidates: Vec::new(),
            evidence: Vec::new(),
            changes: Vec::new(),
            gameplay: Vec::new(),
            capabilities: Vec::new(),
            selected_by_user: false,
        };
        let mut top = sample_mod("content.top", "Top Mod", None);
        top.resource_ops.push(make_op("side/imp.lvl"));
        let mut bottom = sample_mod("content.bottom", "Bottom Mod", None);
        bottom.resource_ops.push(make_op("side/imp.lvl"));

        let mut profile = ExtensionProfile::default();
        profile.set_enabled("content.top", true);
        profile.set_enabled("content.bottom", true);
        profile.order = vec!["content.top".into(), "content.bottom".into()];

        let plan = resolve_content_mods(&[bottom, top], &profile).unwrap();
        assert_eq!(plan.active_order, ["content.top", "content.bottom"]);
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].resource_path, "bf2 pc:side/imp.lvl");
        assert_eq!(plan.conflicts[0].winner_id, "content.top");
        assert_eq!(plan.conflicts[0].overridden_ids, ["content.bottom"]);
    }
}
