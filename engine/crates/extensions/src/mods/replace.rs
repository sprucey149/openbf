//! Legacy resource replacement: which stock LVL each LVL of a mod replaces,
//! what it changes resource by resource, and whether a change touches
//! gameplay.
//!
//! The original game has no mod system: a mod replaces a stock LVL by sitting
//! at that LVL's path below `_LVL_PC`. Whatever folder a mod was extracted
//! into, its relative path, filename, chunk layout and resource names are the
//! evidence for which stock file it stands in for. [`resolve_replacements`]
//! weighs that evidence against the stock catalog ([`super::catalog`]) and
//! never guesses: a file it cannot place is `Ambiguous` or `Unresolved` until
//! the player picks a target in the launcher (persisted in
//! [`TargetSelections`]) or the mod's optional `openbf_mod.ron` names one.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::catalog::{
    normalize_logical, CatalogLvl, LvlFacts, LvlResource, SourceGame, SourcePlatform,
    StockCatalogs, TargetRef,
};
use crate::profile::write_ron_atomic;

pub const TARGET_SELECTIONS_FILE: &str = "mod_targets.ron";

/// What a mod's content does, independent of how it is packaged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Capability {
    AddonMissions,
    NewWorld,
    WorldReplacement,
    SideReplacement,
    ShellSideReplacement,
    FirstPerson,
    Textures,
    Models,
    Animation,
    Audio,
    ShellHud,
    Localization,
    ClassPresentation,
    GameplayClasses,
    Collision,
    Effects,
    Missions,
    Other,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Self::AddonMissions => "New maps",
            Self::NewWorld => "New world",
            Self::WorldReplacement => "World replacement",
            Self::SideReplacement => "Side replacement",
            Self::ShellSideReplacement => "Menu side replacement",
            Self::FirstPerson => "First person",
            Self::Textures => "Textures",
            Self::Models => "Models",
            Self::Animation => "Animation",
            Self::Audio => "Audio",
            Self::ShellHud => "Shell / HUD",
            Self::Localization => "Localization",
            Self::ClassPresentation => "Class presentation",
            Self::GameplayClasses => "Gameplay classes",
            Self::Collision => "Collision",
            Self::Effects => "Effects",
            Self::Missions => "Mission scripts",
            Self::Other => "Other",
        }
    }
}

/// `Full`: the mod's LVL stands in for the whole stock LVL, as the original
/// game loads it. `Patch`: only the resources the mod's LVL carries are
/// applied over the stock ones (declared in `openbf_mod.ron`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReplacementMode {
    #[default]
    Full,
    Patch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpStatus {
    /// Resolved and applied when the mod is enabled.
    Active,
    /// Byte-identical to its stock target: nothing to do.
    Identical,
    /// Deliberately not applied (backup folder, superseded copy, ignored).
    Inactive(String),
    /// More than one stock LVL fits; the player must pick.
    Ambiguous,
    /// No stock LVL fits.
    Unresolved(String),
    /// Resolved, but nothing OpenBF reads can be produced from it.
    Unsupported(String),
}

impl OpStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Identical => "Same as stock",
            Self::Inactive(_) => "Inactive",
            Self::Ambiguous => "Choose target",
            Self::Unresolved(_) => "No target",
            Self::Unsupported(_) => "Unsupported",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Inactive(r) | Self::Unresolved(r) | Self::Unsupported(r) => Some(r),
            _ => None,
        }
    }

    pub fn needs_user(&self) -> bool {
        matches!(self, Self::Ambiguous)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub target: TargetRef,
    /// How many trailing path segments of the source equal the target's.
    pub path_depth: u32,
    /// Share of the source's `(tag, name)` keys the target also has.
    pub overlap: f32,
}

/// Per chunk tag: how many resources the mod changed, added and removed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDelta {
    pub tag: String,
    pub same: u32,
    pub changed: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// One gameplay-relevant difference, in words a player can check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplayChange {
    pub resource: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceOp {
    /// Path of the LVL inside the mod folder, `/`-separated, case kept.
    pub source: String,
    pub source_sha256: String,
    pub source_size: u64,
    #[serde(default)]
    pub detected_game: Option<SourceGame>,
    #[serde(default)]
    pub detected_platform: Option<SourcePlatform>,
    #[serde(default)]
    pub target: Option<TargetRef>,
    pub mode: ReplacementMode,
    pub status: OpStatus,
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    /// Why this target was chosen (or not), one line per piece of evidence.
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub changes: Vec<TagDelta>,
    #[serde(default)]
    pub gameplay: Vec<GameplayChange>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub selected_by_user: bool,
}

impl ResourceOp {
    pub fn is_active(&self) -> bool {
        self.status == OpStatus::Active
    }

    pub fn changes_gameplay(&self) -> bool {
        !self.gameplay.is_empty()
    }

    pub fn source_key(&self) -> String {
        self.source.to_ascii_lowercase()
    }
}

/// A player's (or manifest's) answer for one source LVL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetChoice {
    /// A [`TargetRef::label`].
    Target(String),
    Ignore,
}

/// `openbf_data/mod_targets.ron`: target choices per mod, kept outside the mod
/// files so a re-extracted mod keeps them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSelections {
    #[serde(default)]
    pub mods: BTreeMap<String, BTreeMap<String, TargetChoice>>,
}

impl TargetSelections {
    pub fn path(data_dir: &Path) -> std::path::PathBuf {
        data_dir.join(TARGET_SELECTIONS_FILE)
    }

    pub fn load(data_dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(data_dir))
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> anyhow::Result<()> {
        write_ron_atomic(&Self::path(data_dir), self, "mod target selections")
    }

    pub fn for_mod(&self, mod_id: &str) -> BTreeMap<String, TargetChoice> {
        self.mods.get(mod_id).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, mod_id: &str, source: &str, choice: Option<TargetChoice>) {
        let key = source.replace('\\', "/").to_ascii_lowercase();
        let entry = self.mods.entry(mod_id.to_owned()).or_default();
        match choice {
            Some(c) => {
                entry.insert(key, c);
            }
            None => {
                entry.remove(&key);
            }
        }
        if entry.is_empty() {
            self.mods.remove(mod_id);
        }
    }
}

/// A replacement the mod's own `openbf_mod.ron` declares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredReplacement {
    pub source: String,
    /// A [`TargetRef::label`], or `ignore`.
    pub target: String,
    #[serde(default)]
    pub mode: ReplacementMode,
}

/// One LVL found in a mod folder, already scanned.
#[derive(Clone, Debug)]
pub struct SourceLvl {
    pub rel: String,
    pub facts: LvlFacts,
}

/// Folder-name words that mark a copy the player keeps aside: a backup, a
/// disabled set, or one of several alternatives. A file below one is never
/// applied without an explicit choice.
const ASIDE_WORDS: &[&str] = &[
    "backup", "backups", "bak", "old", "orig", "original", "originals", "copy", "copia",
    "kopie", "copie", "unused", "disabled", "alt", "alts", "alternate", "alternative",
    "alternatives", "optional", "options", "variant", "variants", "vanilla", "stock",
];

/// Folder names that are containers of the original game's own layout. A path
/// is anchored after the last of these.
const LAYOUT_ANCHORS: &[&str] = &["_lvl_pc", "_lvl_ps2", "_lvl_xbox"];

fn aside_segment(seg: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| ASIDE_WORDS.contains(&w))
        || lower.ends_with(".bak")
        || lower.ends_with(".old")
}

/// The path the original game would see: everything after the last
/// `_LVL_<platform>` folder, with the platform that folder names.
fn anchored(rel_lower: &[String]) -> Option<(usize, SourcePlatform)> {
    let pos = rel_lower
        .iter()
        .rposition(|s| LAYOUT_ANCHORS.contains(&s.as_str()))?;
    let platform = match rel_lower[pos].as_str() {
        "_lvl_ps2" => SourcePlatform::Ps2,
        "_lvl_xbox" => SourcePlatform::Xbox,
        _ => SourcePlatform::Pc,
    };
    Some((pos + 1, platform))
}

fn suffix_depth(src: &[String], target: &str) -> u32 {
    let t: Vec<&str> = target.split('/').collect();
    let mut n = 0;
    for (a, b) in src.iter().rev().zip(t.iter().rev()) {
        if a != b {
            break;
        }
        n += 1;
    }
    n
}

fn overlap(src: &LvlFacts, stock: &LvlFacts) -> f32 {
    let a = src.keys();
    if a.is_empty() {
        return 0.0;
    }
    let b = stock.keys();
    a.intersection(&b).count() as f32 / a.len() as f32
}

/// Required share of resource keys for a filename-only match, and the lead it
/// needs over the runner-up. OURS: a policy threshold, not a game value.
/// measure: the Improved Sides LVLs overlap their stock files at 0.6 to 1.0
/// and the nearest other stock LVL below 0.2.
const FILENAME_MIN_OVERLAP: f32 = 0.5;
const FILENAME_MIN_LEAD: f32 = 0.25;

/// Compare a mod LVL with the stock LVL it replaces.
pub fn diff_resources(
    src: &LvlFacts,
    stock: &LvlFacts,
    presentation: &BTreeSet<u32>,
) -> (Vec<TagDelta>, Vec<GameplayChange>) {
    let a = src.digests_by_key();
    let b = stock.digests_by_key();
    let mut by_tag: BTreeMap<String, TagDelta> = BTreeMap::new();
    fn delta<'a>(by_tag: &'a mut BTreeMap<String, TagDelta>, tag: &str) -> &'a mut TagDelta {
        by_tag.entry(tag.to_owned()).or_insert_with(|| TagDelta {
            tag: tag.to_owned(),
            ..Default::default()
        })
    }
    for (key, digests) in &a {
        match b.get(key) {
            Some(d) if d == digests => delta(&mut by_tag, &key.0).same += 1,
            Some(_) => delta(&mut by_tag, &key.0).changed.push(key.1.clone()),
            None => delta(&mut by_tag, &key.0).added.push(key.1.clone()),
        }
    }
    for key in b.keys() {
        if !a.contains_key(key) {
            delta(&mut by_tag, &key.0).removed.push(key.1.clone());
        }
    }

    let classes = |f: &LvlFacts| -> BTreeMap<(String, String), LvlResource> {
        f.resources
            .iter()
            .filter(|r| r.is_class())
            .map(|r| ((r.tag.clone(), r.name.clone()), r.clone()))
            .collect()
    };
    let (ca, cb) = (classes(src), classes(stock));
    let mut gameplay = Vec::new();
    for (key, ra) in &ca {
        let label = format!("{} {}", key.0, key.1);
        let Some(rb) = cb.get(key) else {
            gameplay.push(GameplayChange {
                resource: label,
                detail: "class added".into(),
            });
            continue;
        };
        if ra.base != rb.base {
            gameplay.push(GameplayChange {
                resource: label.clone(),
                detail: format!(
                    "parent class {} -> {}",
                    rb.base.as_deref().unwrap_or("-"),
                    ra.base.as_deref().unwrap_or("-")
                ),
            });
        }
        let pa: BTreeMap<u32, u64> = ra.props.iter().copied().collect();
        let pb: BTreeMap<u32, u64> = rb.props.iter().copied().collect();
        let keys: BTreeSet<u32> = pa.keys().chain(pb.keys()).copied().collect();
        let changed: Vec<u32> = keys
            .into_iter()
            .filter(|k| pa.get(k) != pb.get(k) && !presentation.contains(k))
            .collect();
        if !changed.is_empty() {
            gameplay.push(GameplayChange {
                resource: label,
                detail: format!(
                    "{} gameplay propert{} changed (hash {})",
                    changed.len(),
                    if changed.len() == 1 { "y" } else { "ies" },
                    changed
                        .iter()
                        .take(6)
                        .map(|h| format!("{h:#010x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
    }
    for key in cb.keys() {
        if !ca.contains_key(key) {
            gameplay.push(GameplayChange {
                resource: format!("{} {}", key.0, key.1),
                detail: "class removed".into(),
            });
        }
    }
    for d in by_tag.values() {
        let touched = d.changed.len() + d.added.len() + d.removed.len();
        if touched == 0 {
            continue;
        }
        let what = match d.tag.as_str() {
            "coll" | "prim" => "collision",
            "wrld" => "world layout",
            "tern" => "terrain",
            "plan" | "PATH" | "path" => "AI planning",
            "scr_" => "mission script",
            _ => continue,
        };
        gameplay.push(GameplayChange {
            resource: d.tag.clone(),
            detail: format!(
                "{what}: {} changed, {} added, {} removed",
                d.changed.len(),
                d.added.len(),
                d.removed.len()
            ),
        });
    }
    (by_tag.into_values().collect(), gameplay)
}

fn capabilities_of(target: &TargetRef, changes: &[TagDelta], gameplay: bool) -> Vec<Capability> {
    let mut caps = BTreeSet::new();
    let file = target.path.rsplit('/').next().unwrap_or("");
    let stem = file.strip_suffix(".lvl").unwrap_or(file);
    let dir = target.path.split('/').next().unwrap_or("");
    if dir == "side" {
        if stem.ends_with("shell") {
            caps.insert(Capability::ShellSideReplacement);
        } else {
            caps.insert(Capability::SideReplacement);
        }
    } else if dir == "fpm" {
        caps.insert(Capability::FirstPerson);
    } else if matches!(stem, "shell" | "common" | "ingame" | "mission" | "core") {
        caps.insert(Capability::ShellHud);
    } else if dir == "sound" {
        caps.insert(Capability::Audio);
    }
    for d in changes {
        if d.changed.is_empty() && d.added.is_empty() && d.removed.is_empty() {
            continue;
        }
        let cap = match d.tag.as_str() {
            "tex_" => Capability::Textures,
            "modl" | "skel" | "gmod" | "CLTH" => Capability::Models,
            "zaa_" | "zaf_" | "anim" | "anmg" | "sanm" | "comb" => Capability::Animation,
            "coll" | "prim" => Capability::Collision,
            "fx__" | "ffx_" => Capability::Effects,
            "snd_" | "bnk_" | "Wbnk" => Capability::Audio,
            "Locl" | "loc_" => Capability::Localization,
            "hud_" | "font" => Capability::ShellHud,
            "scr_" => Capability::Missions,
            "wrld" | "tern" | "sky_" | "lght" | "plan" | "PATH" => Capability::WorldReplacement,
            "entc" | "ordc" | "wpnc" | "expc" => {
                if gameplay {
                    Capability::GameplayClasses
                } else {
                    Capability::ClassPresentation
                }
            }
            _ => Capability::Other,
        };
        caps.insert(cap);
    }
    if gameplay {
        caps.insert(Capability::GameplayClasses);
    }
    caps.into_iter().collect()
}

/// Resolve every LVL of one mod. `choices` are the player's selections for
/// this mod, `declared` the manifest's. A class property counts as gameplay
/// unless the target game's correction table lists it as presentation
/// ([`super::catalog::StockBakeInfo::presentation`]).
pub fn resolve_replacements(
    sources: &[SourceLvl],
    catalogs: &StockCatalogs,
    choices: &BTreeMap<String, TargetChoice>,
    declared: &[DeclaredReplacement],
) -> Vec<ResourceOp> {
    let mut ops: Vec<ResourceOp> = sources
        .iter()
        .map(|s| resolve_one(s, catalogs, choices, declared))
        .collect();
    settle_duplicates(&mut ops);
    ops.sort_by(|a, b| a.source_key().cmp(&b.source_key()));
    ops
}

fn declared_for<'a>(declared: &'a [DeclaredReplacement], rel: &str) -> Option<&'a DeclaredReplacement> {
    let key = normalize_logical(rel)?;
    declared
        .iter()
        .find(|d| normalize_logical(&d.source).as_deref() == Some(key.as_str()))
}

fn resolve_one(
    src: &SourceLvl,
    catalogs: &StockCatalogs,
    choices: &BTreeMap<String, TargetChoice>,
    declared: &[DeclaredReplacement],
) -> ResourceOp {
    let rel = src.rel.replace('\\', "/");
    let key = rel.to_ascii_lowercase();
    let mut op = ResourceOp {
        source: rel.clone(),
        source_sha256: src.facts.sha256.clone(),
        source_size: src.facts.size,
        detected_game: src.facts.game,
        detected_platform: src.facts.platform,
        target: None,
        mode: ReplacementMode::Full,
        status: OpStatus::Ambiguous,
        candidates: Vec::new(),
        evidence: Vec::new(),
        changes: Vec::new(),
        gameplay: Vec::new(),
        capabilities: Vec::new(),
        selected_by_user: false,
    };
    if let Some(e) = &src.facts.error {
        op.status = OpStatus::Unsupported(format!("not a readable LVL container: {e}"));
        return op;
    }
    if let Some(g) = src.facts.game {
        op.evidence.push(format!("model chunks are {} format", g.key()));
    }
    if let Some(p) = src.facts.platform {
        op.evidence.push(format!("texture and geometry chunks are {} format", p.key()));
    }
    let segs: Vec<String> = key.split('/').map(str::to_owned).collect();

    let explicit = match choices.get(&key) {
        Some(c) => Some((c.clone(), true, ReplacementMode::Full)),
        None => declared_for(declared, &rel).map(|d| {
            let c = if d.target.trim().eq_ignore_ascii_case("ignore") {
                TargetChoice::Ignore
            } else {
                TargetChoice::Target(d.target.clone())
            };
            (c, false, d.mode)
        }),
    };
    if let Some((choice, by_user, mode)) = explicit {
        let who = if by_user { "your selection" } else { "openbf_mod.ron" };
        op.selected_by_user = by_user;
        op.mode = mode;
        match choice {
            TargetChoice::Ignore => {
                op.status = OpStatus::Inactive(format!("ignored by {who}"));
                return op;
            }
            TargetChoice::Target(label) => {
                let Some(t) = TargetRef::parse(&label) else {
                    op.status = OpStatus::Unresolved(format!("{who} names `{label}`, not a target"));
                    return op;
                };
                op.evidence.push(format!("target set by {who}"));
                return finish(op, &src.facts, t, catalogs);
            }
        }
    }

    let dirs = &segs[..segs.len().saturating_sub(1)];
    let anchor = anchored(&segs);
    if let Some(seg) = dirs.iter().find(|s| aside_segment(s)) {
        op.status = OpStatus::Inactive(format!(
            "inside `{seg}`, which reads as a backup or alternative folder; choose a target to use it"
        ));
        return op;
    }

    let mut cands: Vec<(Candidate, &CatalogLvl)> = Vec::new();
    for cat in &catalogs.catalogs {
        for lvl in &cat.lvls {
            let depth = suffix_depth(&segs, &lvl.path);
            if depth == 0 {
                continue;
            }
            let target = TargetRef {
                game: cat.game,
                platform: cat.platform,
                path: lvl.path.clone(),
            };
            let ov = overlap(&src.facts, &lvl.facts);
            cands.push((
                Candidate {
                    target,
                    path_depth: depth,
                    overlap: ov,
                },
                lvl,
            ));
        }
    }
    if cands.is_empty() {
        op.status = OpStatus::Unresolved(format!(
            "no stock LVL is named `{}`",
            segs.last().map(String::as_str).unwrap_or("")
        ));
        return op;
    }

    if let Some(g) = src.facts.game {
        if cands.iter().any(|(c, _)| c.target.game == g) {
            cands.retain(|(c, _)| c.target.game == g);
        }
    }
    let platform_hint = src.facts.platform.or(anchor.map(|(_, p)| p));
    if let Some(p) = platform_hint {
        if cands.iter().any(|(c, _)| c.target.platform == p) {
            cands.retain(|(c, _)| c.target.platform == p);
        }
    }

    if let Some((c, _)) = cands
        .iter()
        .find(|(_, l)| !l.facts.sha256.is_empty() && l.facts.sha256 == src.facts.sha256)
    {
        op.target = Some(c.target.clone());
        op.candidates = vec![c.clone()];
        op.status = OpStatus::Identical;
        op.evidence
            .push(format!("byte-identical to stock {}", c.target.label()));
        return op;
    }

    let max_depth = cands.iter().map(|(c, _)| c.path_depth).max().unwrap_or(0);
    let mut best: Vec<Candidate> = cands
        .iter()
        .filter(|(c, _)| c.path_depth == max_depth)
        .map(|(c, _)| c.clone())
        .collect();
    best.sort_by(|a, b| b.overlap.total_cmp(&a.overlap));
    op.candidates = best.clone();

    let file_only = max_depth == 1;
    let lead = if best.len() > 1 {
        best[0].overlap - best[1].overlap
    } else {
        best[0].overlap
    };
    let chosen = if file_only {
        let ok = best[0].overlap >= FILENAME_MIN_OVERLAP && lead >= FILENAME_MIN_LEAD;
        if ok {
            op.evidence.push(format!(
                "filename matches and {:.0}% of its resources are in stock {}",
                best[0].overlap * 100.0,
                best[0].target.label()
            ));
        }
        ok.then(|| best[0].target.clone())
    } else if best.len() == 1 {
        op.evidence.push(format!(
            "folder path matches stock {} ({} segments)",
            best[0].target.label(),
            max_depth
        ));
        Some(best[0].target.clone())
    } else if best[0].overlap >= FILENAME_MIN_OVERLAP && lead >= FILENAME_MIN_LEAD {
        op.evidence.push(format!(
            "folder path matches {} stock LVLs; {:.0}% of its resources are in {}",
            best.len(),
            best[0].overlap * 100.0,
            best[0].target.label()
        ));
        Some(best[0].target.clone())
    } else {
        None
    };

    if file_only {
        if let Some((i, _)) = anchor {
            let seen = segs[i..].join("/");
            op.evidence.push(format!(
                "the original game would read it as `{seen}`, which is not a stock path"
            ));
        }
    }

    match chosen {
        Some(t) => finish(op, &src.facts, t, catalogs),
        None if src.facts.stub
            && cands
                .iter()
                .all(|(c, l)| c.path_depth < max_depth || !l.facts.stub) =>
        {
            op.status = OpStatus::Unsupported(BLANKS_STOCK.into());
            op
        }
        None => {
            op.status = OpStatus::Ambiguous;
            op.evidence.push(format!(
                "{} stock LVLs fit equally well; choose one",
                best.len()
            ));
            op
        }
    }
}

/// Why an empty LVL over a stocked one produces nothing, whichever of its
/// targets it has: a file that matches several is refused here too, so the
/// player is never asked for a choice that cannot change the output.
const BLANKS_STOCK: &str =
    "an empty LVL blanks the stock file; removing a whole stock LVL is not supported";

fn finish(
    mut op: ResourceOp,
    facts: &LvlFacts,
    target: TargetRef,
    catalogs: &StockCatalogs,
) -> ResourceOp {
    let Some(stock) = catalogs.lvl(&target) else {
        op.status = OpStatus::Unresolved(format!("{} is not in the stock catalog", target.label()));
        op.target = Some(target);
        return op;
    };
    if facts.sha256 == stock.facts.sha256 {
        op.status = OpStatus::Identical;
        op.target = Some(target);
        return op;
    }
    if let (Some(a), b) = (facts.game, target.game) {
        if a != b {
            op.evidence.push(format!(
                "warning: its models are {} format but the target is {}",
                a.key(),
                b.key()
            ));
        }
    }
    let presentation = catalogs.bake.presentation_of(target.game);
    let (changes, gameplay) = diff_resources(facts, &stock.facts, &presentation);
    op.capabilities = capabilities_of(&target, &changes, !gameplay.is_empty());
    op.changes = changes;
    op.gameplay = gameplay;
    op.status = if facts.stub && !stock.facts.stub {
        OpStatus::Unsupported(BLANKS_STOCK.into())
    } else {
        OpStatus::Active
    };
    op.target = Some(target);
    op
}

/// Two LVLs of one mod resolving to one stock target: the one whose own path
/// matches more of the target wins, the others are marked superseded; a tie
/// needs the player.
fn settle_duplicates(ops: &mut [ResourceOp]) {
    let mut by_target: BTreeMap<TargetRef, Vec<usize>> = BTreeMap::new();
    for (i, op) in ops.iter().enumerate() {
        if let (Some(t), OpStatus::Active) = (&op.target, &op.status) {
            by_target.entry(t.clone()).or_default().push(i);
        }
    }
    for (target, idx) in by_target {
        if idx.len() < 2 {
            continue;
        }
        if let Some(&u) = idx.iter().find(|&&i| ops[i].selected_by_user) {
            for &i in &idx {
                if i != u {
                    ops[i].status = OpStatus::Inactive(format!(
                        "{} is used for {} by your selection",
                        ops[u].source,
                        target.label()
                    ));
                }
            }
            continue;
        }
        let depth = |i: usize| {
            ops[i]
                .candidates
                .iter()
                .find(|c| c.target == target)
                .map_or(0, |c| c.path_depth)
        };
        let max = idx.iter().map(|&i| depth(i)).max().unwrap_or(0);
        let top: Vec<usize> = idx.iter().copied().filter(|&i| depth(i) == max).collect();
        if top.len() == 1 {
            let w = top[0];
            for &i in &idx {
                if i != w {
                    ops[i].status = OpStatus::Inactive(format!(
                        "{} in this mod matches {} more closely",
                        ops[w].source,
                        target.label()
                    ));
                }
            }
        } else {
            for &i in &idx {
                ops[i].status = OpStatus::Ambiguous;
                ops[i].evidence.push(format!(
                    "{} files in this mod target {}; choose which one to use",
                    idx.len(),
                    target.label()
                ));
            }
        }
    }
}

/// Every capability the active operations provide.
pub fn mod_capabilities(ops: &[ResourceOp]) -> Vec<Capability> {
    let set: BTreeSet<Capability> = ops
        .iter()
        .filter(|o| o.is_active())
        .flat_map(|o| o.capabilities.iter().copied())
        .collect();
    set.into_iter().collect()
}

/// Cache key of a mod's replacement plan: which sources go where, against
/// which stock catalog. A change re-runs conversion. A mod with no
/// replacement operations (a pure addon map) replaces nothing, so its key
/// leaves the stock catalog out and stays stable across stock rebakes, as
/// `ModCacheMeta::expected_replacement_key` already treats it.
pub fn replacement_key(ops: &[ResourceOp], catalogs: &StockCatalogs) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"openbf-replacements-v1\0");
    if !ops.is_empty() {
        h.update(catalogs.digest.as_bytes());
    }
    for op in ops {
        h.update(op.source_key().as_bytes());
        h.update([0]);
        h.update(op.source_sha256.as_bytes());
        h.update([0]);
        h.update(op.target.as_ref().map(TargetRef::label).unwrap_or_default().as_bytes());
        h.update([0]);
        h.update(format!("{:?}{:?}", op.mode, op.status).as_bytes());
        h.update([0xff]);
    }
    let mut hex = String::with_capacity(64);
    for b in h.finalize() {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::super::catalog::{StockCatalog, CATALOG_GENERATOR_VERSION, CATALOG_SCHEMA_VERSION};
    use super::*;

    #[test]
    fn an_addon_only_key_ignores_the_stock_catalog() {
        let old = StockCatalogs {
            digest: "old".into(),
            ..StockCatalogs::default()
        };
        let new = StockCatalogs {
            digest: "new".into(),
            ..StockCatalogs::default()
        };
        assert_eq!(replacement_key(&[], &old), replacement_key(&[], &new));
    }

    fn res(tag: &str, name: &str, digest: u64) -> LvlResource {
        LvlResource {
            tag: tag.into(),
            name: name.into(),
            digest,
            size: 0,
            props: Vec::new(),
            base: None,
        }
    }

    fn class(name: &str, props: &[(u32, u64)]) -> LvlResource {
        LvlResource {
            props: props.to_vec(),
            base: Some("soldier".into()),
            ..res("entc", name, 1)
        }
    }

    fn facts(sha: &str, game: SourceGame, resources: Vec<LvlResource>) -> LvlFacts {
        LvlFacts {
            size: 100,
            sha256: sha.into(),
            game: Some(game),
            platform: Some(SourcePlatform::Pc),
            resources,
            ..Default::default()
        }
    }

    fn catalogs() -> StockCatalogs {
        let lvl = |path: &str, f: LvlFacts| CatalogLvl {
            path: path.into(),
            facts: f,
            outputs: Vec::new(),
        };
        let bf2 = StockCatalog {
            schema_version: CATALOG_SCHEMA_VERSION,
            generator_version: CATALOG_GENERATOR_VERSION,
            game: SourceGame::Bf2,
            platform: SourcePlatform::Pc,
            lvls: vec![
                lvl(
                    "side/imp.lvl",
                    facts(
                        "s-imp",
                        SourceGame::Bf2,
                        vec![
                            res("tex_", "imp_trooper", 1),
                            res("modl", "imp_trooper", 2),
                            class("imp_inf_rifleman", &[(10, 1), (20, 2)]),
                        ],
                    ),
                ),
                lvl(
                    "fpm/imp/allxwing.lvl",
                    facts("s-imp-x", SourceGame::Bf2, vec![res("tex_", "cockpit_a", 1)]),
                ),
                lvl(
                    "fpm/rep/allxwing.lvl",
                    facts("s-rep-x", SourceGame::Bf2, vec![res("tex_", "cockpit_a", 1)]),
                ),
                lvl(
                    "end/end1.lvl",
                    facts("s-end2", SourceGame::Bf2, vec![res("tex_", "end_tree", 1)]),
                ),
            ],
        };
        let bf1 = StockCatalog {
            game: SourceGame::Bf1,
            lvls: vec![lvl(
                "end/end1.lvl",
                facts("s-end1", SourceGame::Bf1, vec![res("tex_", "end_rock", 1)]),
            )],
            ..bf2.clone()
        };
        let mut cats = StockCatalogs {
            catalogs: vec![bf2, bf1],
            ..Default::default()
        };
        cats.bake.presentation.insert(SourceGame::Bf2, vec![20]);
        cats
    }

    fn resolve(srcs: Vec<(&str, LvlFacts)>, choices: &BTreeMap<String, TargetChoice>) -> Vec<ResourceOp> {
        let srcs: Vec<SourceLvl> = srcs
            .into_iter()
            .map(|(r, f)| SourceLvl { rel: r.into(), facts: f })
            .collect();
        resolve_replacements(&srcs, &catalogs(), choices, &[])
    }

    fn modded_imp() -> LvlFacts {
        facts(
            "m-imp",
            SourceGame::Bf2,
            vec![
                res("tex_", "imp_trooper", 9),
                res("tex_", "imp_trooper_normal", 9),
                res("modl", "imp_trooper", 2),
                LvlResource {
                    digest: 3,
                    ..class("imp_inf_rifleman", &[(10, 1), (20, 7)])
                },
            ],
        )
    }

    #[test]
    fn side_lvl_resolves_by_path_under_any_wrapper_and_any_case() {
        let ops = resolve(vec![("Improved Sides 1.0/SIDE/Imp.LVL", modded_imp())], &BTreeMap::new());
        let op = &ops[0];
        assert_eq!(op.status, OpStatus::Active);
        assert_eq!(op.target.as_ref().unwrap().label(), "bf2 pc:side/imp.lvl");
        let tex = op.changes.iter().find(|d| d.tag == "tex_").unwrap();
        assert_eq!(tex.changed, vec!["imp_trooper"]);
        assert_eq!(tex.added, vec!["imp_trooper_normal"]);
        assert!(op.gameplay.is_empty(), "presentation-only property change");
        assert!(op.capabilities.contains(&Capability::SideReplacement));
        assert!(op.capabilities.contains(&Capability::Textures));
        assert!(op.capabilities.contains(&Capability::ClassPresentation));
    }

    #[test]
    fn gameplay_property_change_is_reported() {
        let mut f = modded_imp();
        f.resources[3].props[0].1 = 99;
        let ops = resolve(vec![("side/imp.lvl", f)], &BTreeMap::new());
        assert!(ops[0].changes_gameplay());
        assert!(ops[0].capabilities.contains(&Capability::GameplayClasses));
    }

    #[test]
    fn loose_end1_resolves_by_game_and_content() {
        let f = facts("m-end", SourceGame::Bf1, vec![res("tex_", "end_rock", 5)]);
        let ops = resolve(vec![("end1.lvl", f)], &BTreeMap::new());
        assert_eq!(ops[0].status, OpStatus::Active);
        assert_eq!(ops[0].target.as_ref().unwrap().label(), "bf1 pc:end/end1.lvl");
    }

    #[test]
    fn duplicate_filenames_need_the_folder_or_a_choice() {
        let f = facts("m-x", SourceGame::Bf2, vec![res("tex_", "cockpit_a", 7)]);
        let ops = resolve(vec![("fpm/rep/allxwing.lvl", f.clone())], &BTreeMap::new());
        assert_eq!(ops[0].target.as_ref().unwrap().path, "fpm/rep/allxwing.lvl");
        let ops = resolve(vec![("allxwing.lvl", f.clone())], &BTreeMap::new());
        assert_eq!(ops[0].status, OpStatus::Ambiguous);
        assert_eq!(ops[0].candidates.len(), 2);
        let mut choice = BTreeMap::new();
        choice.insert(
            "allxwing.lvl".to_owned(),
            TargetChoice::Target("bf2 pc:fpm/imp/allxwing.lvl".into()),
        );
        let ops = resolve(vec![("allxwing.lvl", f)], &choice);
        assert_eq!(ops[0].status, OpStatus::Active);
        assert!(ops[0].selected_by_user);
    }

    #[test]
    fn an_empty_file_over_several_stocked_targets_asks_for_no_choice() {
        let mut f = facts("m-stub", SourceGame::Bf2, Vec::new());
        f.stub = true;
        let ops = resolve(vec![("allxwing.lvl", f)], &BTreeMap::new());
        assert!(matches!(&ops[0].status, OpStatus::Unsupported(r) if r == BLANKS_STOCK));
        assert!(!ops[0].status.needs_user());
        assert_eq!(ops[0].candidates.len(), 2);
    }

    #[test]
    fn backups_identical_files_and_superseded_copies_stay_inactive() {
        let ops = resolve(
            vec![
                ("_lvl_pc - Copia/side/imp.lvl", modded_imp()),
                ("data/_lvl_pc/side/imp.lvl", modded_imp()),
                ("imp.lvl", modded_imp()),
                (
                    "side/end1.lvl",
                    facts("s-end2", SourceGame::Bf2, vec![res("tex_", "end_tree", 1)]),
                ),
            ],
            &BTreeMap::new(),
        );
        let by = |s: &str| ops.iter().find(|o| o.source == s).unwrap();
        assert!(matches!(by("_lvl_pc - Copia/side/imp.lvl").status, OpStatus::Inactive(_)));
        assert_eq!(by("data/_lvl_pc/side/imp.lvl").status, OpStatus::Active);
        assert!(matches!(by("imp.lvl").status, OpStatus::Inactive(_)));
        assert_eq!(by("side/end1.lvl").status, OpStatus::Identical);
    }

    #[test]
    fn selections_round_trip_and_ignore() {
        let dir = std::env::temp_dir().join(format!("openbf-sel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = TargetSelections::default();
        s.set("content.a", "Side\\Imp.lvl", Some(TargetChoice::Ignore));
        s.save(&dir).unwrap();
        let back = TargetSelections::load(&dir);
        assert_eq!(back, s);
        let ops = resolve(vec![("side/imp.lvl", modded_imp())], &back.for_mod("content.a"));
        assert!(matches!(ops[0].status, OpStatus::Inactive(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
