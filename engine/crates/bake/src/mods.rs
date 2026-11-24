//! Generic legacy Battlefront II addon discovery, mission translation, and
//! versioned cache conversion.

pub mod convert;
pub mod mission;

use extensions::mods::{
    compute_mod_fingerprint, merge_optional_manifest, read_explicit_manifest, DiscoveredMod,
    ModCompatibility, ModContentType, ModMapSummary, ModMissionSummary,
};
use mungefs::addon::{inspect_addon_dir, AddonCompatibility, AddonInspection, AddonKind};
use std::collections::BTreeSet;
use std::path::Path;

pub use convert::{convert_mod_to_cache, ConvertOptions, ConvertReport};

/// Inspect a single directory inside `openbf_data/mods/` into shared
/// `DiscoveredMod` metadata alongside raw [`AddonInspection`].
pub fn inspect_mod_folder(
    mod_root: &Path,
    mod_id: &str,
    namespace: &str,
) -> (DiscoveredMod, AddonInspection) {
    let folder_name = mod_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mod")
        .to_owned();
    let mut insp = inspect_addon_dir(mod_root);

    let fp = compute_mod_fingerprint(mod_root).unwrap_or_else(|e| {
        insp.errors.push(format!("Failed to fingerprint mod: {e}"));
        extensions::mods::fingerprint::FingerprintResult {
            digest: String::new(),
            warnings: Vec::new(),
        }
    });
    for w in fp.warnings {
        if !insp.warnings.contains(&w) {
            insp.warnings.push(w);
        }
    }

    let mod_type = match insp.kind {
        AddonKind::LegacyMapAddon => ModContentType::LegacyMapAddon,
        AddonKind::LegacySideOverhaul => ModContentType::LegacySideOverhaul,
        AddonKind::LegacyAssetPack => ModContentType::LegacyAssetPack,
        AddonKind::LegacyHybridAddon => ModContentType::LegacyHybridAddon,
        AddonKind::Unsupported => ModContentType::Unsupported,
        AddonKind::Invalid => ModContentType::Invalid,
    };

    let compatibility = match &insp.compatibility {
        AddonCompatibility::Supported => ModCompatibility::Supported,
        AddonCompatibility::PartialWithWarnings => ModCompatibility::PartialWithWarnings,
        AddonCompatibility::Unsupported(s) => ModCompatibility::Unsupported(s.clone()),
        AddonCompatibility::Invalid(s) => ModCompatibility::Invalid(s.clone()),
    };

    let groups = insp.map_groups(namespace);
    let maps: Vec<ModMapSummary> = groups
        .iter()
        .map(|g| ModMapSummary {
            map_key: g.map_key.clone(),
            legacy_code: g.legacy_code.clone(),
            display_name: g.display_name.clone(),
            description: g.description.clone(),
            source_lvl: g.source_lvl_rel.clone(),
            modes_cw: g.modes_cw.clone(),
            modes_gcw: g.modes_gcw.clone(),
            missions: g
                .missions
                .iter()
                .map(|pm| ModMissionSummary {
                    script_name: pm.mission.script_name.clone(),
                    mode: pm.mission.mode.clone(),
                    era: pm.mission.era.clone(),
                    mission_ron: pm.mission_ron.clone(),
                    supported: pm.mission.supported,
                    unsupported_reason: pm.mission.unsupported_reason.clone(),
                })
                .collect(),
            sound_planets: g.sound_planets.clone(),
        })
        .collect();

    let mut custom_sides = BTreeSet::new();
    for m in &insp.missions {
        for (side, _) in &m.custom_side_lvls {
            custom_sides.insert(side.clone());
        }
    }
    let declared_overrides: Vec<String> = insp
        .stock_override_lvls
        .iter()
        .map(|o| o.rel_path.clone())
        .collect();

    let mut discovered = DiscoveredMod {
        id: mod_id.to_owned(),
        namespace: namespace.to_owned(),
        folder_name,
        root_dir: mod_root.to_path_buf(),
        content_root: insp.content_root.clone(),
        legacy_addon_id: insp.legacy_addon_id.clone(),
        display_name: insp.display_name.clone(),
        version: None,
        author: insp.author.clone(),
        description: insp.description.clone(),
        mod_type,
        compatibility,
        source_fingerprint: fp.digest,
        maps,
        dependencies: Vec::new(),
        conflicts: Vec::new(),
        declared_overrides,
        custom_sides: custom_sides.into_iter().collect(),
        warnings: insp.warnings.clone(),
        errors: insp.errors.clone(),
        unused_lvls: insp.unused_lvls.clone(),
    };

    if let Some(manifest) = read_explicit_manifest(mod_root) {
        merge_optional_manifest(&mut discovered, manifest);
    }

    (discovered, insp)
}
