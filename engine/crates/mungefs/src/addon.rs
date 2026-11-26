//! Generic, read-only legacy Battlefront II addon inspector.
//!
//! Scans an extracted addon folder under `openbf_data/mods/<folder>` without
//! modifying any files or executing untrusted binaries or scripts. Supports the
//! standard BF2 addon hierarchy (`addme.script`, `data/_LVL_PC/...`) as well as
//! single-wrapper extracted archives, while safely rejecting path traversal,
//! escaping symlinks, and native binary/DLL-only mods.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::lua50::{self, LuaVal, UnsupportedScriptFeature};
use crate::{locl, open, UcfbError};

mod world_events;

const MAX_SCRIPT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_LVL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_README_BYTES: u64 = 8 * 1024;
const MAX_WALK_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddonKind {
    LegacyMapAddon,
    LegacySideOverhaul,
    LegacyAssetPack,
    LegacyHybridAddon,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddonCompatibility {
    Supported,
    PartialWithWarnings,
    Unsupported(String),
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTeam {
    pub team_idx: u8,
    pub faction: String,
    pub units: u32,
    pub tickets: u32,
    pub classes: Vec<(String, u32, Option<u32>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCtfFlag {
    pub name: String,
    pub home_region: String,
    pub capture_region: String,
    pub team_hint: Option<u8>,
    pub geometry: Option<String>,
    pub carried_geometry: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCtf {
    pub two_flag: bool,
    pub score_limit: Option<u32>,
    pub hide_cps: bool,
    pub flag: String,
    pub home_region: String,
    pub capture_region_att: String,
    pub capture_region_def: String,
    pub cap_region_dummy_att: Option<String>,
    pub cap_region_dummy_def: Option<String>,
    pub flags: Vec<ParsedCtfFlag>,
}

pub type LocalTeamClasses = Vec<(String, u32, Option<u32>)>;
pub type LocalTeamDef = (u8, u32, LocalTeamClasses);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AddonWorldLoad {
    pub rel_path: String,
    pub path: PathBuf,
    pub selectors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum WorldEffectAnchor {
    Object(String),
    PathPoint(String, usize),
}

#[derive(Debug, Clone)]
pub enum WorldAction {
    PlayAnimation(String),
    PauseAnimation(String),
    KillObject(String),
    Barrier(String, bool),
    Arc(String, bool),
    Effect(String, WorldEffectAnchor),
    After(f32, Vec<WorldAction>),
}

#[derive(Debug, Clone, Default)]
pub struct WorldEvents {
    pub start: Vec<WorldAction>,
    pub object_destroyed: Vec<(String, Vec<WorldAction>)>,
}

#[derive(Debug, Clone, Default)]
pub struct InspectedMission {
    pub script_name: String,
    pub registered_in_addme: bool,
    pub mode: String,
    pub era: String,
    pub map_lvl_rel: Option<String>,
    pub map_lvl_path: Option<PathBuf>,
    /// The script reads one of the game's own world LVLs, so the addon's
    /// world LVLs it also reads are layers on that map.
    pub on_stock_world: bool,
    pub loadscreen_lvl_path: Option<PathBuf>,
    pub layers: Vec<String>,
    pub world_loads: Vec<AddonWorldLoad>,
    pub world_variant: Vec<String>,
    pub single_player_weight: u8,
    pub multiplayer_variant: bool,
    pub shares_variant_entry: bool,
    pub world_events: WorldEvents,
    pub teams: Vec<ParsedTeam>,
    pub local_teams: Vec<LocalTeamDef>,
    pub local_team_relations: Vec<(u8, u8, bool)>,
    pub heroes: Vec<(u8, String)>,
    pub class_load_order: Vec<(String, String)>,
    pub custom_side_lvls: Vec<(String, PathBuf)>,
    pub custom_sound_lvls: Vec<(String, PathBuf)>,
    pub sound_banks: Vec<(String, String)>,
    pub ai_alive_cap: u32,
    pub spawn_delay: Option<(f32, f32)>,
    pub attacking_team: Option<u8>,
    pub cps: Vec<(String, u8)>,
    pub posts: Vec<String>,
    pub killed: Vec<String>,
    pub death_regions: Vec<String>,
    pub landing_regions: Vec<String>,
    pub max_player_fly_height: Option<f32>,
    pub min_player_fly_height: Option<f32>,
    pub max_fly_height: Option<f32>,
    pub min_fly_height: Option<f32>,
    pub ground_flyer_map: bool,
    pub particle_lod_bias: Option<f32>,
    pub arc_blocks: Vec<(String, bool)>,
    pub door_locks: Vec<(String, bool)>,
    pub barriers: Vec<(String, bool)>,
    pub voice_bleeding: Vec<(u8, u8, String, bool)>,
    pub voice_low_reinforcements: Vec<(u8, u8, String, f32, bool)>,
    pub voice_out_of_bounds: Vec<(u8, String)>,
    pub music_ambient: Vec<(u8, f32, String, u8, bool)>,
    pub music_victory: Vec<(u8, String)>,
    pub music_defeat: Vec<(u8, String)>,
    pub camera_shots: Vec<[f32; 7]>,
    pub ai_view_mult: Option<f32>,
    pub ai_aggressiveness: Vec<(u8, f32)>,
    pub ai_snipe_suitability: Option<f32>,
    pub ai_no_capture: Vec<(String, u8)>,
    pub ctf: Option<ParsedCtf>,
    pub supported: bool,
    pub unsupported_reason: Option<String>,
    pub unsupported_features: Vec<UnsupportedScriptFeature>,
}

#[derive(Debug, Clone)]
pub struct OverrideLvl {
    pub rel_path: String,
    pub abs_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AddonInspection {
    pub mod_root: PathBuf,
    pub content_root: Option<PathBuf>,
    pub kind: AddonKind,
    pub compatibility: AddonCompatibility,
    pub legacy_addon_id: Option<String>,
    pub display_name: String,
    pub author: Option<String>,
    pub description: String,
    pub openbf_manifest_ron: Option<String>,
    pub core_lvl: Option<PathBuf>,
    pub mission_lvls: Vec<PathBuf>,
    pub map_titles: BTreeMap<String, (Option<String>, Option<String>)>,
    pub missions: Vec<InspectedMission>,
    pub stock_override_lvls: Vec<OverrideLvl>,
    pub unused_lvls: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AddonPlannedMission<'a> {
    pub mission: &'a InspectedMission,
    pub mission_ron: String,
}

#[derive(Debug, Clone)]
pub struct AddonMapGroup<'a> {
    pub map_key: String,
    pub legacy_code: String,
    pub display_name: String,
    pub description: String,
    pub source_lvl_rel: String,
    pub variant_title: Option<String>,
    pub source_lvl_path: Option<PathBuf>,
    pub loadscreen_lvl_path: Option<PathBuf>,
    pub world_loads: Vec<AddonWorldLoad>,
    pub custom_side_lvls: Vec<(String, PathBuf)>,
    pub custom_sound_lvls: Vec<(String, PathBuf)>,
    pub modes_cw: Vec<String>,
    pub modes_gcw: Vec<String>,
    pub sound_planets: Vec<String>,
    pub missions: Vec<AddonPlannedMission<'a>>,
}

impl AddonInspection {
    /// Group inspected missions by target map LVL under `namespace`, computing
    /// exact mission RON filenames compatible with `game::mission::pick`.
    pub fn map_groups(&self, namespace: &str) -> Vec<AddonMapGroup<'_>> {
        let mut by_lvl = BTreeMap::<_, Vec<&InspectedMission>>::new();
        // A script that reads no world of the addon's is not one of its maps.
        for m in self
            .missions
            .iter()
            .filter(|m| m.map_lvl_rel.is_some() || m.supported)
        {
            let key = m
                .map_lvl_rel
                .clone()
                .unwrap_or_else(|| self.legacy_addon_id.clone().unwrap_or_else(|| "map".into()));
            let companion_loads: Vec<_> = m
                .world_loads
                .iter()
                .filter(|load| Some(&load.path) != m.map_lvl_path.as_ref())
                .cloned()
                .collect();
            by_lvl
                .entry((
                    key,
                    companion_loads,
                    m.world_variant.clone(),
                    m.map_lvl_path.clone(),
                ))
                .or_default()
                .push(m);
        }

        let mut used_map_keys: BTreeSet<String> = BTreeSet::new();
        let mut groups = Vec::new();
        for ((source_lvl_rel, companion_loads, primary_variant, _), list) in by_lvl {
            let legacy_stem = Path::new(&source_lvl_rel)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("map")
                .to_ascii_lowercase();
            let variant = companion_loads
                .iter()
                .flat_map(|load| &load.selectors)
                .chain(&primary_variant)
                .cloned()
                .collect::<Vec<_>>()
                .join("_");
            let slug: String = variant
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .take(48)
                .collect();
            let base_key = if slug.is_empty() {
                format!("{namespace}__{legacy_stem}")
            } else {
                format!("{namespace}__{legacy_stem}_{slug}")
            };
            let mut map_key = base_key.clone();
            let mut n = 2usize;
            while !used_map_keys.insert(map_key.clone()) {
                map_key = format!("{base_key}_{n}");
                n += 1;
            }

            let source_lvl_path = list.iter().find_map(|m| m.map_lvl_path.clone());
            let loadscreen_lvl_path = list.iter().find_map(|m| m.loadscreen_lvl_path.clone());
            let mut world_loads: Vec<AddonWorldLoad> = Vec::new();
            if !companion_loads.is_empty() || !primary_variant.is_empty() {
                for load in list.iter().flat_map(|m| &m.world_loads) {
                    if let Some(previous) = world_loads
                        .iter_mut()
                        .find(|previous| previous.path == load.path)
                    {
                        for selector in &load.selectors {
                            if !previous
                                .selectors
                                .iter()
                                .any(|s| s.eq_ignore_ascii_case(selector))
                            {
                                previous.selectors.push(selector.clone());
                            }
                        }
                    } else {
                        world_loads.push(load.clone());
                    }
                }
            }

            let mut modes_cw_set = BTreeSet::new();
            let mut modes_gcw_set = BTreeSet::new();
            let mut planets_set = BTreeSet::new();
            let mut side_map: BTreeMap<String, PathBuf> = BTreeMap::new();
            let mut sound_map: BTreeMap<String, PathBuf> = BTreeMap::new();

            for m in &list {
                if m.supported {
                    if m.era == "cw" {
                        modes_cw_set.insert(m.mode.clone());
                    } else {
                        modes_gcw_set.insert(m.mode.clone());
                    }
                }
                for (planet, _) in &m.sound_banks {
                    planets_set.insert(planet.clone());
                }
                for (s, p) in &m.custom_side_lvls {
                    side_map.entry(s.clone()).or_insert_with(|| p.clone());
                }
                for (s, p) in &m.custom_sound_lvls {
                    sound_map.entry(s.clone()).or_insert_with(|| p.clone());
                }
            }

            let mut used_filenames = BTreeSet::new();
            let mut planned = Vec::with_capacity(list.len());
            for m in &list {
                let has_non_cw_same_mode = list
                    .iter()
                    .any(|other| other.mode == m.mode && other.era != "cw");
                let era_suffix = if m.era == "cw" && has_non_cw_same_mode {
                    "_cw"
                } else {
                    ""
                };
                let mut fname = format!("{map_key}_{}{era_suffix}.ron", m.mode);
                if !used_filenames.insert(fname.clone()) {
                    fname = format!("{map_key}_{}.ron", m.script_name.to_ascii_lowercase());
                }
                planned.push(AddonPlannedMission {
                    mission: m,
                    mission_ron: fname,
                });
            }

            let (title, desc) = list
                .iter()
                .find_map(|m| self.map_titles.get(&m.script_name.to_ascii_lowercase()))
                .or_else(|| self.map_titles.get(&legacy_stem))
                .map(|(n, d)| {
                    let t = match (n.as_deref(), d.as_deref().and_then(|s| s.lines().next())) {
                        (Some(name), Some(d1))
                            if d1.trim().starts_with(name) && d1.trim().len() > name.len() =>
                        {
                            d1.trim().to_owned()
                        }
                        (Some(name), _) => name.to_owned(),
                        (None, _) => self.display_name.clone(),
                    };
                    (t, d.clone().unwrap_or_else(|| self.description.clone()))
                })
                .unwrap_or_else(|| (self.display_name.clone(), self.description.clone()));

            let variant_title = list
                .iter()
                .any(|m| m.shares_variant_entry)
                .then(|| title.clone());
            let title = if variant.is_empty() {
                title
            } else {
                let mut suffix = variant.replace('_', " ");
                if let Some(first) = suffix.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                format!("{title} ({suffix})")
            };
            groups.push(AddonMapGroup {
                map_key,
                legacy_code: legacy_stem,
                display_name: title,
                variant_title,
                description: desc,
                source_lvl_rel,
                source_lvl_path,
                loadscreen_lvl_path,
                world_loads,
                custom_side_lvls: side_map.into_iter().collect(),
                custom_sound_lvls: sound_map.into_iter().collect(),
                modes_cw: modes_cw_set.into_iter().collect(),
                modes_gcw: modes_gcw_set.into_iter().collect(),
                sound_planets: planets_set.into_iter().collect(),
                missions: planned,
            });
        }
        groups
    }
}

fn canon_norm(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok()
}

fn is_contained_in(root_canon: &Path, candidate: &Path) -> bool {
    let Some(cand_canon) = canon_norm(candidate) else {
        return false;
    };
    cand_canon.starts_with(root_canon)
}

fn find_child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut matches: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let fname = e.file_name();
            let s = fname.to_str()?;
            s.eq_ignore_ascii_case(name).then(|| e.path())
        })
        .collect();
    matches.sort();
    matches.into_iter().next()
}

fn resolve_segments_ci(root_canon: &Path, base: &Path, segs: &[&str]) -> Option<PathBuf> {
    let mut cur = base.to_path_buf();
    for &seg in segs {
        if seg.is_empty() || seg == "." || seg == ".." || seg.contains(['/', '\\', ':']) {
            return None;
        }
        cur = find_child_ci(&cur, seg)?;
        let meta = fs::symlink_metadata(&cur).ok()?;
        if meta.file_type().is_symlink() && !is_contained_in(root_canon, &cur) {
            return None;
        }
    }
    if is_contained_in(root_canon, &cur) {
        Some(cur)
    } else {
        None
    }
}

fn read_bounded(root_canon: &Path, path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("failed to stat {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() && !is_contained_in(root_canon, path) {
        return Err(format!(
            "refused symlink escaping mod folder: {}",
            path.display()
        ));
    }
    if !is_contained_in(root_canon, path) {
        return Err(format!(
            "refused path outside mod folder: {}",
            path.display()
        ));
    }
    let file_meta =
        fs::metadata(path).map_err(|e| format!("failed to inspect {}: {e}", path.display()))?;
    if !file_meta.is_file() {
        return Err(format!(
            "expected file, found directory: {}",
            path.display()
        ));
    }
    if file_meta.len() > max_bytes {
        return Err(format!(
            "file {} exceeds size limit ({} > {} bytes)",
            path.display(),
            file_meta.len(),
            max_bytes
        ));
    }
    fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))
}

fn archive_has_base_world(root_canon: &Path, path: &Path) -> bool {
    read_bounded(root_canon, path, MAX_LVL_BYTES)
        .ok()
        .and_then(|bytes| {
            let root = open(&bytes).ok()?;
            Some(
                crate::chunks(root)
                    .flatten()
                    .any(|chunk| &chunk.tag == b"wrld"),
            )
        })
        .unwrap_or(false)
}

struct TreeFacts {
    native_binaries: Vec<String>,
    non_pc_dirs: Vec<String>,
    escaping_symlinks: Vec<String>,
    all_lvls: Vec<(String, PathBuf)>,
    readmes: Vec<PathBuf>,
}

fn scan_mod_tree(mod_root: &Path, root_canon: &Path) -> TreeFacts {
    let mut facts = TreeFacts {
        native_binaries: Vec::new(),
        non_pc_dirs: Vec::new(),
        escaping_symlinks: Vec::new(),
        all_lvls: Vec::new(),
        readmes: Vec::new(),
    };
    let mut stack = vec![(mod_root.to_path_buf(), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        if depth > 10 || seen >= MAX_WALK_ENTRIES {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        children.sort();
        for p in children {
            seen += 1;
            if seen > MAX_WALK_ENTRIES {
                break;
            }
            let Ok(meta) = fs::symlink_metadata(&p) else {
                continue;
            };
            let rel = p
                .strip_prefix(mod_root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            if meta.file_type().is_symlink() && !is_contained_in(root_canon, &p) {
                facts.escaping_symlinks.push(rel);
                continue;
            }
            let Ok(real_meta) = fs::metadata(&p) else {
                continue;
            };
            if real_meta.is_dir() {
                let name_lower = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if matches!(name_lower.as_str(), "_lvl_ps2" | "_lvl_xbox") {
                    facts.non_pc_dirs.push(rel);
                }
                stack.push((p, depth + 1));
            } else if real_meta.is_file() {
                let lower = rel.to_ascii_lowercase();
                if lower.ends_with(".dll")
                    || lower.ends_with(".so")
                    || lower.ends_with(".dylib")
                    || lower.ends_with(".exe")
                {
                    facts.native_binaries.push(rel);
                } else if lower.ends_with(".lvl") {
                    facts.all_lvls.push((rel, p));
                } else if lower.ends_with("readme.txt") {
                    facts.readmes.push(p);
                }
            }
        }
    }
    facts
}

fn native_binaries_warning(facts: &TreeFacts) -> String {
    format!(
        "Ignored native binary file(s) ({}) alongside addon data; arbitrary DLL execution is disabled.",
        facts.native_binaries.join(", ")
    )
}

fn has_bf2_layout(dir: &Path, root_canon: &Path) -> bool {
    let addme = find_child_ci(dir, "addme.script").is_some_and(|p| p.is_file());
    let data_lvl_pc = resolve_segments_ci(root_canon, dir, &["data", "_LVL_PC"]).is_some();
    let direct_lvl_pc = resolve_segments_ci(root_canon, dir, &["_LVL_PC"]).is_some();
    addme || data_lvl_pc || direct_lvl_pc
}

fn detect_content_root(mod_root: &Path, root_canon: &Path) -> Option<PathBuf> {
    if has_bf2_layout(mod_root, root_canon) {
        return Some(mod_root.to_path_buf());
    }
    // Common single-wrapper archive extraction: `<mod>/<wrapper>/...`
    let subdirs: Vec<PathBuf> = fs::read_dir(mod_root)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let meta = fs::symlink_metadata(&p).ok()?;
            if meta.file_type().is_symlink() && !is_contained_in(root_canon, &p) {
                return None;
            }
            let name = p.file_name()?.to_str()?;
            if name.starts_with('.') {
                return None;
            }
            p.is_dir().then_some(p)
        })
        .collect();
    if subdirs.len() == 1 {
        let sub = &subdirs[0];
        if has_bf2_layout(sub, root_canon) {
            return Some(sub.clone());
        }
        // `GameData/addon/<ID>` or `addon/<ID>` inside wrapper
        if let Some(addon) = find_child_ci(sub, "addon")
            .or_else(|| find_child_ci(sub, "GameData").and_then(|g| find_child_ci(&g, "addon")))
        {
            let nested: Vec<PathBuf> = fs::read_dir(&addon)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            if nested.len() == 1 && has_bf2_layout(&nested[0], root_canon) {
                return Some(nested[0].clone());
            }
        }
    }
    None
}

/// A download laid out as the game folder it is copied over: addons under
/// `addon/<ID>`, and often replaced stock files under `data/_LVL_PC`.
struct GameFolder {
    dir: PathBuf,
    addons: Vec<PathBuf>,
}

/// `mod_root`, or its only folder, when it holds `addon/<ID>/addme.script`.
/// The game runs every addon it finds there.
fn detect_game_folder(mod_root: &Path, root_canon: &Path) -> Option<GameFolder> {
    let subdirs = |dir: &Path| -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && is_contained_in(root_canon, p))
            .collect();
        found.sort();
        found
    };
    let mut candidates = vec![mod_root.to_path_buf()];
    if let [only] = subdirs(mod_root).as_slice() {
        candidates.push(only.clone());
    }
    candidates.into_iter().find_map(|dir| {
        let addon = find_child_ci(&dir, "addon")?;
        let addons: Vec<PathBuf> = subdirs(&addon)
            .into_iter()
            .filter(|a| find_child_ci(a, "addme.script").is_some_and(|p| p.is_file()))
            .collect();
        (!addons.is_empty()).then_some(GameFolder { dir, addons })
    })
}

/// Reads each addon of a game folder on its own and joins the results. The
/// files under the folder's own `data/_LVL_PC` stand in for stock ones.
fn inspect_game_folder(
    mut out: AddonInspection,
    root_canon: &Path,
    facts: &TreeFacts,
    game: GameFolder,
) -> AddonInspection {
    let mod_root = out.mod_root.clone();
    let rel = |path: &Path| {
        path.strip_prefix(&mod_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut parts: Vec<(String, AddonInspection)> = game
        .addons
        .iter()
        .map(|a| (rel(a), inspect_addon_dir(a)))
        .collect();
    // The addon with the most missions names the package's `core.lvl`.
    parts.sort_by_key(|(_, p)| std::cmp::Reverse(p.missions.len()));
    if parts.iter().filter(|(_, p)| !p.missions.is_empty()).count() > 1 {
        out.warnings.push(
            "Several addons in this folder add missions; names are read from the largest one's core.lvl first."
                .into(),
        );
    }
    for (i, (prefix, part)) in parts.into_iter().enumerate() {
        if i == 0 {
            out.core_lvl = part.core_lvl;
            out.legacy_addon_id = part.legacy_addon_id;
        }
        out.mission_lvls.extend(part.mission_lvls);
        out.missions.extend(part.missions);
        for (id, title) in part.map_titles {
            out.map_titles.entry(id).or_insert(title);
        }
        out.warnings.extend(part.warnings);
        out.errors.extend(part.errors);
        out.unused_lvls
            .extend(part.unused_lvls.iter().map(|lvl| format!("{prefix}/{lvl}")));
        out.stock_override_lvls
            .extend(part.stock_override_lvls.into_iter().map(|o| OverrideLvl {
                rel_path: format!("{prefix}/{}", o.rel_path),
                abs_path: o.abs_path,
            }));
    }
    if let Some(pc) = resolve_lvl_pc_dir(&game.dir, root_canon) {
        out.stock_override_lvls.extend(
            facts
                .all_lvls
                .iter()
                .filter(|(_, abs)| abs.starts_with(&pc))
                .map(|(rel_path, abs_path)| OverrideLvl {
                    rel_path: rel_path.clone(),
                    abs_path: abs_path.clone(),
                }),
        );
    }
    out.content_root = Some(game.dir);
    classify(&mut out, true);
    out
}

/// The addon's kind and compatibility, from what the inspection found.
fn classify(out: &mut AddonInspection, has_content: bool) {
    let mut seen = BTreeSet::new();
    out.warnings.retain(|w| seen.insert(w.clone()));

    let has_missions = !out.missions.is_empty();
    let has_custom_sides = out.missions.iter().any(|m| !m.custom_side_lvls.is_empty());
    let has_overrides = !out.stock_override_lvls.is_empty();
    out.kind = match (has_missions, has_custom_sides, has_overrides) {
        (true, true, _) | (true, _, true) => AddonKind::LegacyHybridAddon,
        (true, false, false) => AddonKind::LegacyMapAddon,
        (false, _, true) => {
            if out
                .stock_override_lvls
                .iter()
                .all(|o| o.rel_path.to_ascii_lowercase().starts_with("side/"))
            {
                AddonKind::LegacySideOverhaul
            } else {
                AddonKind::LegacyAssetPack
            }
        }
        (false, _, false) => {
            if has_content {
                AddonKind::LegacyAssetPack
            } else {
                AddonKind::Invalid
            }
        }
    };

    let playable_count = out.missions.iter().filter(|m| m.supported).count();
    out.compatibility = if !out.errors.is_empty() {
        AddonCompatibility::Unsupported(out.errors[0].clone())
    } else if out.kind == AddonKind::Invalid {
        let msg = "Folder does not contain playable BF2 addon content or manifest.".to_owned();
        AddonCompatibility::Invalid(msg)
    } else if has_missions && playable_count == 0 {
        let reason = out
            .missions
            .first()
            .and_then(|m| m.unsupported_reason.clone())
            .unwrap_or_else(|| "No supported missions found in mission.lvl".into());
        AddonCompatibility::Unsupported(reason)
    } else if !out.warnings.is_empty() || out.missions.iter().any(|m| !m.supported) {
        AddonCompatibility::PartialWithWarnings
    } else {
        AddonCompatibility::Supported
    };
}

fn resolve_lvl_pc_dir(content_root: &Path, root_canon: &Path) -> Option<PathBuf> {
    resolve_segments_ci(root_canon, content_root, &["data", "_LVL_PC"])
        .or_else(|| resolve_segments_ci(root_canon, content_root, &["_LVL_PC"]))
}

fn resolve_inside_lvl_pc(
    root_canon: &Path,
    lvl_pc: &Path,
    raw_ref: &str,
) -> Option<(String, PathBuf)> {
    let cleaned = raw_ref
        .split(';')
        .next()
        .unwrap_or(raw_ref)
        .trim()
        .replace('\\', "/");
    let lower = cleaned.to_ascii_lowercase();
    let rel_after_dc = if let Some(rest) = lower.strip_prefix("dc:") {
        &cleaned[cleaned.len() - rest.len()..]
    } else if let Some(pos) = lower.find("/data/_lvl_pc/") {
        &cleaned[pos + "/data/_lvl_pc/".len()..]
    } else if let Some(pos) = lower.find("_lvl_pc/") {
        &cleaned[pos + "_lvl_pc/".len()..]
    } else {
        cleaned.as_str()
    };
    let parts: Vec<&str> = rel_after_dc
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if parts.is_empty() || parts.iter().any(|s| *s == ".." || s.contains(':')) {
        return None;
    }
    let resolved = resolve_segments_ci(root_canon, lvl_pc, &parts)?;
    Some((parts.join("/"), resolved))
}

#[derive(Default)]
struct AddmeSummary {
    legacy_ids: BTreeSet<String>,
    registered_scripts: BTreeSet<String>,
    core_lvl_hint: Option<String>,
    unsupported: Vec<UnsupportedScriptFeature>,
    warnings: Vec<String>,
}

fn expand_mapluafile(template: &str, era: &str, mode: &str) -> String {
    let first = template.replacen("%s", era, 1);
    first.replacen("%s", mode, 1)
}

fn parse_addme_script(bytes: &[u8]) -> Result<AddmeSummary, String> {
    let chunks = lua50::extract_scripts(bytes).map_err(|e| format!("addme.script UCFB: {e}"))?;
    let mut summary = AddmeSummary::default();
    let mut processed = false;

    for sc in chunks {
        if !sc.body.starts_with(b"\x1bLua") {
            continue;
        }
        let proto = lua50::undump(&sc.body).map_err(|e| format!("addme.script Lua 5.0: {e}"))?;
        let env = lua50::evaluate_standalone_registration(proto);
        summary.unsupported.extend(env.unsupported.clone());
        summary.warnings.extend(env.warnings.clone());
        processed = true;

        // Collect listbox registrations from both SP and MP tables.
        for tid in [env.sp_listbox_id, env.mp_listbox_id] {
            let Some(tbl) = env.table(tid) else { continue };
            for item in tbl.array_items() {
                let LuaVal::Table(etid) = item else { continue };
                let Some(entry) = env.table(*etid) else {
                    continue;
                };
                let Some(tmpl) = entry.get_str_ci("mapluafile").and_then(|v| v.as_str()) else {
                    continue;
                };
                if let Some(id) = tmpl.split("%s").next().filter(|s| !s.is_empty()) {
                    let clean_id = id.trim_end_matches('_');
                    if !clean_id.is_empty() {
                        summary.legacy_ids.insert(clean_id.to_owned());
                    }
                }
                for (era_code, era_field) in [("g", "era_g"), ("c", "era_c")] {
                    let era_on = entry
                        .get_str_ci(era_field)
                        .and_then(|v| v.is_truthy())
                        .unwrap_or(false);
                    if !era_on {
                        continue;
                    }
                    for (mode_code, field_prefix) in [
                        ("con", "mode_con_"),
                        ("ctf", "mode_ctf_"),
                        ("1flag", "mode_1flag_"),
                        ("eli", "mode_eli_"),
                        ("hunt", "mode_hunt_"),
                        ("ass", "mode_ass_"),
                        ("tdm", "mode_tdm_"),
                    ] {
                        let key = format!("{field_prefix}{era_code}");
                        if entry
                            .get_str_ci(&key)
                            .and_then(|v| v.is_truthy())
                            .unwrap_or(false)
                        {
                            let script = expand_mapluafile(tmpl, era_code, mode_code);
                            summary
                                .registered_scripts
                                .insert(script.to_ascii_lowercase());
                        }
                    }
                }
            }
        }

        for call in &env.calls {
            if call.func.eq_ignore_ascii_case("AddDownloadableContent") {
                if let Some(id) = call.args.first().and_then(|v| v.as_str()) {
                    if !id.is_empty() {
                        summary.legacy_ids.insert(id.to_owned());
                    }
                }
                if let Some(script) = call.args.get(1).and_then(|v| v.as_str()) {
                    if !script.is_empty() {
                        summary
                            .registered_scripts
                            .insert(script.to_ascii_lowercase());
                    }
                }
            } else if call.func.eq_ignore_ascii_case("ReadDataFile") {
                if let Some(path) = call.args.first().and_then(|v| v.as_str()) {
                    if path.to_ascii_lowercase().contains("core.lvl") {
                        summary.core_lvl_hint = Some(path.to_owned());
                    }
                }
            }
        }
    }

    if !processed {
        return Err("addme.script contained no compiled Lua 5.0 script chunk".into());
    }
    Ok(summary)
}

/// Read the preferred `Locl` table (`english`, `uk_english`, or first present)
/// from a `core.lvl` file.
pub fn read_core_locl(path: &Path) -> Result<Option<locl::Locl>, UcfbError> {
    let bytes = fs::read(path).map_err(|e| UcfbError::Malformed(e.to_string()))?;
    let payload = open(&bytes)?;
    let content = crate::content_chunks(payload)?;
    if let Some(eng) = locl::find(&content, "english")? {
        return Ok(Some(eng));
    }
    if let Some(uk) = locl::find(&content, "uk_english")? {
        return Ok(Some(uk));
    }
    for c in content.iter().filter(|c| &c.tag == b"Locl") {
        if let Ok(table) = locl::parse(c) {
            return Ok(Some(table));
        }
    }
    Ok(None)
}

/// A mission's localized text, found as the game's
/// `missionlist_GetLocalizedMapName` finds it: `prefix` plus the longest
/// leading part of the script name that has a string, starting at its last
/// underscore (`rxc1_con`: `rxc1_`, `rxc1`, `rxc`, ...).
fn mission_text(
    script: &str,
    prefix: &str,
    text: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let last = script.len().saturating_sub(1);
    let end = script
        .get(..last)
        .and_then(|head| head.rfind('_'))
        .map_or(script.len(), |at| at + 1);
    (1..=end)
        .rev()
        .find_map(|len| text(&format!("{prefix}{}", script.get(..len)?)))
}

pub fn is_helper_script_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l == "setup_teams"
        || l == "ambush"
        || l == "playmoviewithtransition"
        || l == "missionlist"
        || l.starts_with("gametype_")
        || l.starts_with("objective")
        || l.starts_with("soundevent_")
        || l.starts_with("linked")
        || l.starts_with("ifs_")
}

mod mission_fold;
use mission_fold::fold_calls;

fn parse_readme_hint(
    mod_root: &Path,
    root_canon: &Path,
    readmes: &[PathBuf],
) -> (Option<String>, String) {
    let preferred = readmes.iter().min_by_key(|p| {
        p.strip_prefix(mod_root)
            .map_or(99, |r| r.components().count())
    });
    let Some(path) = preferred else {
        return (None, String::new());
    };
    let Ok(bytes) = read_bounded(root_canon, path, MAX_README_BYTES) else {
        return (None, String::new());
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut author = None;
    let mut lines = Vec::new();
    for line in text.lines().take(12) {
        let trimmed = line.trim().trim_matches('-').trim();
        if trimmed.is_empty() {
            continue;
        }
        if author.is_none() {
            let lower = trimmed.to_ascii_lowercase();
            if let Some(pos) = lower.find("by ") {
                let candidate = trimmed[pos + 3..]
                    .split(['-', '(', '\r', '\n'])
                    .next()
                    .unwrap_or("")
                    .trim();
                if !candidate.is_empty() && candidate.len() <= 48 {
                    author = Some(candidate.to_owned());
                }
            }
        }
        lines.push(trimmed.to_owned());
    }
    (author, lines.join("\n"))
}

fn sanitize_fallback_title(folder: &str) -> String {
    folder.replace('_', " ").trim().to_owned()
}

/// Inspect a single directory inside `openbf_data/mods/` in read-only mode.
pub fn inspect_addon_dir(mod_root: &Path) -> AddonInspection {
    let folder_name = mod_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_owned();
    let mut out = AddonInspection {
        mod_root: mod_root.to_path_buf(),
        content_root: None,
        kind: AddonKind::Invalid,
        compatibility: AddonCompatibility::Invalid("Folder not found".into()),
        legacy_addon_id: None,
        display_name: sanitize_fallback_title(&folder_name),
        author: None,
        description: String::new(),
        openbf_manifest_ron: None,
        core_lvl: None,
        mission_lvls: Vec::new(),
        map_titles: BTreeMap::new(),
        missions: Vec::new(),
        stock_override_lvls: Vec::new(),
        unused_lvls: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    let Some(root_canon) = canon_norm(mod_root) else {
        out.errors.push(format!(
            "Unable to access mod directory {}",
            mod_root.display()
        ));
        return out;
    };
    if !mod_root.is_dir() {
        return out;
    }

    // Reject path traversal components on mod_root itself just in case.
    if mod_root
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        out.errors
            .push("Refused mod path containing parent traversal ('..')".into());
        return out;
    }

    let facts = scan_mod_tree(mod_root, &root_canon);
    for esc in &facts.escaping_symlinks {
        out.warnings.push(format!(
            "Ignored symlink pointing outside mod directory: {esc}"
        ));
    }

    if let Some(manifest_path) = find_child_ci(mod_root, "openbf_mod.ron") {
        match read_bounded(&root_canon, &manifest_path, MAX_MANIFEST_BYTES) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => out.openbf_manifest_ron = Some(s),
                Err(_) => out
                    .warnings
                    .push("openbf_mod.ron is not valid UTF-8 and was ignored.".into()),
            },
            Err(e) => out.warnings.push(e),
        }
    }

    if let Some(game) = detect_game_folder(mod_root, &root_canon) {
        if !facts.native_binaries.is_empty() {
            out.warnings.push(native_binaries_warning(&facts));
        }
        let (author, description) = parse_readme_hint(mod_root, &root_canon, &facts.readmes);
        out.author = author;
        out.description = description;
        return inspect_game_folder(out, &root_canon, &facts, game);
    }

    let content_root = detect_content_root(mod_root, &root_canon);
    out.content_root = content_root.clone();

    let loose_lvls = content_root.is_none()
        && facts.native_binaries.is_empty()
        && facts.non_pc_dirs.is_empty()
        && !facts.all_lvls.is_empty();
    if content_root.is_none() && out.openbf_manifest_ron.is_none() && !loose_lvls {
        if !facts.native_binaries.is_empty() {
            let reason = format!(
                "Third-party native binary/DLL mods ({}) are not supported.",
                facts.native_binaries[0]
            );
            out.kind = AddonKind::Unsupported;
            out.compatibility = AddonCompatibility::Unsupported(reason.clone());
            out.errors.push(reason);
        } else if !facts.non_pc_dirs.is_empty() {
            let reason = format!(
                "Contains non-PC level data ({}) without _LVL_PC.",
                facts.non_pc_dirs[0]
            );
            out.kind = AddonKind::Unsupported;
            out.compatibility = AddonCompatibility::Unsupported(reason.clone());
            out.errors.push(reason);
        } else {
            let reason =
                "Folder does not contain a recognized legacy Battlefront II addon layout.".into();
            out.kind = AddonKind::Invalid;
            out.compatibility = AddonCompatibility::Invalid(reason);
        }
        return out;
    }

    if !facts.native_binaries.is_empty() {
        out.warnings.push(native_binaries_warning(&facts));
    }

    let (readme_author, readme_desc) = parse_readme_hint(mod_root, &root_canon, &facts.readmes);
    out.author = readme_author;

    let mut addme_summary = AddmeSummary::default();
    let mut has_addme = false;
    if let Some(cr) = &content_root {
        if let Some(addme_path) = find_child_ci(cr, "addme.script") {
            has_addme = true;
            match read_bounded(&root_canon, &addme_path, MAX_SCRIPT_BYTES)
                .and_then(|b| parse_addme_script(&b))
            {
                Ok(s) => {
                    for u in &s.unsupported {
                        out.warnings
                            .push(format!("addme.script uses unsupported API: {}", u.detail));
                    }
                    out.warnings.extend(s.warnings.iter().cloned());
                    addme_summary = s;
                }
                Err(e) => out.errors.push(e),
            }
        }
    }

    let lvl_pc = content_root
        .as_ref()
        .and_then(|cr| resolve_lvl_pc_dir(cr, &root_canon));

    // Locate core.lvl (either from addme.script's ReadDataFile hint or data/_LVL_PC/core.lvl)
    if let Some(pc) = &lvl_pc {
        out.core_lvl = addme_summary
            .core_lvl_hint
            .as_deref()
            .and_then(|h| resolve_inside_lvl_pc(&root_canon, pc, h).map(|(_, p)| p))
            .or_else(|| find_child_ci(pc, "core.lvl").filter(|p| p.is_file()));
    }

    // Choose primary legacy addon ID.
    let mut candidate_ids: Vec<String> = addme_summary.legacy_ids.iter().cloned().collect();
    if candidate_ids.is_empty() {
        if let Some(pc) = &lvl_pc {
            if let Ok(rd) = fs::read_dir(pc) {
                let mut dirs: Vec<String> = rd
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                    .filter(|n| {
                        !matches!(
                            n.to_ascii_lowercase().as_str(),
                            "load" | "side" | "sound" | "movies" | "shell"
                        )
                    })
                    .collect();
                dirs.sort();
                if let Some(first) = dirs.first() {
                    candidate_ids.push(first.clone());
                }
            }
        }
    }
    if candidate_ids.is_empty() && !folder_name.is_empty() {
        candidate_ids.push(folder_name.clone());
    }
    out.legacy_addon_id = addme_summary
        .legacy_ids
        .iter()
        .next()
        .cloned()
        .or_else(|| candidate_ids.first().cloned());

    // Read core.lvl localization if present.
    let mut core_locl = None;
    if let Some(core_path) = &out.core_lvl {
        match read_bounded(&root_canon, core_path, MAX_LVL_BYTES)
            .ok()
            .and_then(|_| read_core_locl(core_path).ok())
            .flatten()
        {
            Some(locl_table) => {
                let find_text = |k: &str| {
                    let hash = locl::key(k);
                    locl_table
                        .entries
                        .iter()
                        .find(|(h, _)| *h == hash)
                        .map(|(_, s)| s.replace('\r', ""))
                };
                for id in &candidate_ids {
                    let low = id.to_ascii_lowercase();
                    let name = find_text(&format!("mapname.name.{low}"));
                    let desc = find_text(&format!("mapname.description.{low}"));
                    if name.is_some() || desc.is_some() {
                        out.map_titles.insert(low, (name, desc));
                    }
                }
                core_locl = Some(locl_table);
            }
            None => {
                out.warnings
                    .push("core.lvl did not yield an English localization table.".into());
            }
        }
    }

    // Locate mission.lvl (and mission*.lvl inside _LVL_PC)
    let mut mission_lvls = Vec::new();
    if let Some(pc) = &lvl_pc {
        if let Some(ml) = find_child_ci(pc, "mission.lvl").filter(|p| p.is_file()) {
            mission_lvls.push(ml);
        }
        if let Ok(rd) = fs::read_dir(pc) {
            let mut extras: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                            let l = n.to_ascii_lowercase();
                            l.starts_with("mission") && l.ends_with(".lvl") && l != "mission.lvl"
                        })
                })
                .collect();
            extras.sort();
            mission_lvls.extend(extras);
        }
    }
    out.mission_lvls = mission_lvls.clone();

    let mut referenced_lvls_canon: BTreeSet<PathBuf> = BTreeSet::new();
    if let Some(p) = &out.core_lvl {
        if let Some(c) = canon_norm(p) {
            referenced_lvls_canon.insert(c);
        }
    }
    for ml in &mission_lvls {
        if let Some(c) = canon_norm(ml) {
            referenced_lvls_canon.insert(c);
        }
    }

    // Inspect mission scripts.
    let mut base_world_archives = BTreeMap::new();
    for ml_path in &mission_lvls {
        let Ok(bytes) = read_bounded(&root_canon, ml_path, MAX_LVL_BYTES) else {
            out.errors
                .push(format!("Failed to read {}", ml_path.display()));
            continue;
        };
        let scripts = match lua50::extract_scripts(&bytes) {
            Ok(s) => s,
            Err(e) => {
                out.errors
                    .push(format!("Failed to parse {}: {e}", ml_path.display()));
                continue;
            }
        };
        let script_protos = std::sync::Arc::new(
            scripts
                .iter()
                .filter_map(|sc| {
                    lua50::undump(&sc.body)
                        .ok()
                        .map(|proto| (sc.name.to_ascii_lowercase(), proto))
                })
                .collect::<BTreeMap<_, _>>(),
        );
        for sc in &scripts {
            if is_helper_script_name(&sc.name) || !sc.body.starts_with(b"\x1bLua") {
                continue;
            }
            let low_name = sc.name.to_ascii_lowercase();
            let registered = if has_addme && !addme_summary.registered_scripts.is_empty() {
                addme_summary.registered_scripts.contains(&low_name)
            } else {
                true
            };
            if !registered {
                out.warnings.push(format!(
                    "Mission script '{}' exists in mission.lvl but is not registered in addme.script (skipped).",
                    sc.name
                ));
                continue;
            }
            if let Some(proto) = script_protos.get(&low_name) {
                let fold = |analysis: &lua50::MissionScriptAnalysis| {
                    fold_calls(
                        analysis,
                        &sc.name,
                        registered,
                        &root_canon,
                        lvl_pc.as_deref(),
                    )
                };
                let (single_player, multiplayer) =
                    lua50::analyze_standalone_variants(proto, &script_protos);
                let mut variants: Vec<InspectedMission> = Vec::new();
                for mut mission in single_player.iter().map(fold) {
                    if let Some(previous) = variants
                        .iter_mut()
                        .find(|m| m.world_loads == mission.world_loads)
                    {
                        previous.single_player_weight += 1;
                    } else {
                        mission.single_player_weight = 1;
                        variants.push(mission);
                    }
                }
                let mut multiplayer = fold(&multiplayer);
                if let Some(m) = variants
                    .iter_mut()
                    .find(|m| m.world_loads == multiplayer.world_loads)
                {
                    m.multiplayer_variant = true;
                } else if !multiplayer.world_loads.is_empty() {
                    multiplayer.multiplayer_variant = true;
                    variants.push(multiplayer);
                }
                if variants.len() == 1 && variants[0].world_loads.len() <= 1 {
                    // Keep the original declarative extraction for single-archive scripts.
                    let mut mission = fold(&lua50::analyze_mission_script(proto.clone()));
                    mission.world_loads.clear();
                    mission.single_player_weight = 1;
                    mission.multiplayer_variant = true;
                    variants = vec![mission];
                }
                if variants.len() > 1 && variants.iter().all(|m| m.world_loads.len() == 1) {
                    let common: Vec<_> = variants[0].world_loads[0]
                        .selectors
                        .iter()
                        .filter(|selector| {
                            variants
                                .iter()
                                .all(|m| m.world_loads[0].selectors.contains(selector))
                        })
                        .cloned()
                        .collect();
                    for mission in &mut variants {
                        mission.world_variant = mission.world_loads[0]
                            .selectors
                            .iter()
                            .filter(|selector| !common.contains(selector))
                            .cloned()
                            .collect();
                    }
                }
                let shares_variant_entry = variants.len() > 1;
                for mut mission in variants {
                    mission.shares_variant_entry = shares_variant_entry;
                    for load in &mission.world_loads {
                        if let Some(c) = canon_norm(&load.path) {
                            referenced_lvls_canon.insert(c);
                        }
                    }
                    let base_world = mission.world_loads.iter().find(|load| {
                        *base_world_archives
                            .entry(load.path.clone())
                            .or_insert_with(|| archive_has_base_world(&root_canon, &load.path))
                    });
                    if mission.on_stock_world {
                        mission.map_lvl_rel = None;
                        mission.map_lvl_path = None;
                        mission.supported = false;
                        mission.unsupported_reason = Some(format!(
                            "Mission '{}' plays on a stock map, which a mod cannot change yet",
                            mission.script_name
                        ));
                    } else if let Some(base) = base_world.or_else(|| mission.world_loads.last()) {
                        mission.map_lvl_rel = Some(base.rel_path.clone());
                        mission.map_lvl_path = Some(base.path.clone());
                        mission.layers = base.selectors.clone();
                    }
                    if let Some(p) = &mission.map_lvl_path {
                        if let Some(c) = canon_norm(p) {
                            referenced_lvls_canon.insert(c);
                        }
                    }
                    if let Some(p) = &mission.loadscreen_lvl_path {
                        if let Some(c) = canon_norm(p) {
                            referenced_lvls_canon.insert(c);
                        }
                    }
                    for (_, p) in &mission.custom_side_lvls {
                        if let Some(c) = canon_norm(p) {
                            referenced_lvls_canon.insert(c);
                        }
                    }
                    for (_, p) in &mission.custom_sound_lvls {
                        if let Some(c) = canon_norm(p) {
                            referenced_lvls_canon.insert(c);
                        }
                    }
                    for feat in &mission.unsupported_features {
                        out.warnings.push(format!(
                            "Mission '{}': dynamic script call {} may differ from original game behavior.",
                            mission.script_name, feat.detail
                        ));
                    }
                    if let Some(reason) = &mission.unsupported_reason {
                        out.warnings
                            .push(format!("Mission '{}': {reason}", mission.script_name));
                    }
                    out.missions.push(mission);
                }
            } else if let Err(e) = lua50::undump(&sc.body) {
                out.warnings.push(format!(
                    "Mission script '{}' failed Lua 5.0 decoding: {e}",
                    sc.name
                ));
            }
        }
    }

    if let Some(table) = &core_locl {
        let text = |key: &str| {
            let hash = locl::key(key);
            table
                .entries
                .iter()
                .find(|(h, _)| *h == hash)
                .map(|(_, s)| s.replace('\r', ""))
        };
        for mission in &out.missions {
            let script = mission.script_name.to_ascii_lowercase();
            let name = mission_text(&script, "mapname.name.", text);
            let desc = mission_text(&script, "mapname.description.", text);
            if name.is_some() || desc.is_some() {
                out.map_titles.insert(script, (name, desc));
            }
        }
    }

    // A folder of LVLs with no addon layout (`side/`, `fpm/`, a loose
    // `end1.lvl`): every LVL is a replacement candidate, placed against the
    // stock catalog by `extensions::mods::replace`, which also keeps backup
    // and alternative folders inactive.
    if lvl_pc.is_none() && !has_addme {
        for (rel_from_mod, abs_path) in &facts.all_lvls {
            out.stock_override_lvls.push(OverrideLvl {
                rel_path: rel_from_mod.clone(),
                abs_path: abs_path.clone(),
            });
        }
    }

    // Classify remaining LVLs. Without addon missions every unreferenced LVL
    // is a replacement candidate, wherever it sits (the resolver decides which
    // stock file, if any, it stands in for). An addon's unreferenced leftovers
    // inside its `_LVL_PC` (`<ID>/kas2.lvl` copied from a template) must NOT
    // shadow stock.
    if let Some(pc) = &lvl_pc {
        let has_map_missions = !out.missions.is_empty();
        for (rel_from_mod, abs_path) in &facts.all_lvls {
            let Some(c) = canon_norm(abs_path) else {
                continue;
            };
            if referenced_lvls_canon.contains(&c) {
                continue;
            }
            let inside_pc = abs_path.starts_with(pc);
            if !has_addme && !has_map_missions {
                out.stock_override_lvls.push(OverrideLvl {
                    rel_path: rel_from_mod.clone(),
                    abs_path: abs_path.clone(),
                });
            } else if !inside_pc {
                continue;
            } else {
                out.unused_lvls.push(rel_from_mod.clone());
            }
        }
        if !out.unused_lvls.is_empty() {
            out.warnings.push(format!(
                "Contains {} unreferenced LVL file(s) ({}); kept inactive so stock maps are not shadowed.",
                out.unused_lvls.len(),
                out.unused_lvls.join(", ")
            ));
        }
    }

    // Pick a clean display name and description.
    if let Some(id) = &out.legacy_addon_id {
        if let Some((loc_name, loc_desc)) = out.map_titles.get(&id.to_ascii_lowercase()) {
            let first_desc_line = loc_desc
                .as_deref()
                .and_then(|d| d.lines().next())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            out.display_name = match (loc_name.as_deref(), first_desc_line) {
                (Some(n), Some(d1)) if d1.starts_with(n) && d1.len() > n.len() => d1.to_owned(),
                (Some(n), _) => n.to_owned(),
                (None, Some(d1)) => d1.to_owned(),
                (None, None) => sanitize_fallback_title(&folder_name),
            };
            if let Some(desc) = loc_desc {
                out.description = desc.clone();
            }
        }
    }
    if out.description.is_empty() {
        out.description = readme_desc;
    }

    let has_content = out.openbf_manifest_ron.is_some() || !facts.all_lvls.is_empty() || has_addme;
    classify(&mut out, has_content);

    out
}

#[cfg(test)]
mod tests;
