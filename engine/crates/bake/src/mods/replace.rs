//! Stock replacements: a mod LVL that takes the place of a stock LVL
//! ([`extensions::mods::replace`]), translated onto the files the stock bake
//! made from that LVL, written below the cache's `overrides/assets/`.
//!
//! What a replacement becomes, per stock LVL kind:
//!
//! - **Side LVL** (`side/<s>.lvl`): a class patch over `classes/<g>_<s>.ron`
//!   ([`manifest::class_patch`]) holding only the properties the mod changed
//!   relative to the stock PC copy (the catalog's per-property digests), so
//!   every untouched class keeps the stock bake's PS2 gameplay values; every
//!   soldier, weapon and vehicle whose model the mod changed, re-baked from
//!   the mod LVL with the stock bake's commands (`scripts/rebake.sh`): the
//!   same animation banks, the same borrowed clips, the four sides loaded
//!   side by side with the mod's own sides in place of the stock ones; every
//!   changed texture the stock bake wrote from that side; the side's effects
//!   when the mod changed them.
//! - **First-person LVL** (`fpm/...`): the arm model, re-baked with the
//!   stock first-person animation bank.
//! - **Anything else**, and every resource kind none of the above covers, is
//!   reported in [`Replaced::unsupported`], never silently dropped.
//!
//! A mod texture is written at both texture tiers (`<name>.ktx2` and, where
//! the stock bake has one, `<name>.tier2.ktx2`): the mod's art is what the
//! player picked, at either setting. A stock file the mod's version of a
//! resource no longer has (a soldier with no cloth) is recorded as removed,
//! which hides it ([`ReplacementOutput::removed`]).

use anyhow::{anyhow, Context, Result};
use extensions::mods::catalog::{
    CatalogLvl, LvlFacts, LvlResource, SourceGame, SourcePlatform, StockDependency, TargetRef,
};
use extensions::mods::replace::ReplacementMode;
use extensions::mods::{ReplacementOutput, ResourceOp};
use manifest::class_patch::{ClassPatch, ClassPatchFile, CLASS_PATCH_SCHEMA};
use manifest::ClassDef;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use super::ModEnv;
use crate::stock::StockAssets;

/// What the replacements wrote.
#[derive(Debug, Default)]
pub struct Replaced {
    pub outputs: Vec<ReplacementOutput>,
    pub unsupported: Vec<String>,
    pub warnings: Vec<String>,
}

const SIDES: [&str; 4] = ["all", "imp", "rep", "cis"];

/// Convert every active operation of `ops`. `abs` maps a source LVL's path
/// (lowercase, relative to the mod root) to the file; `out` is the cache's
/// `overrides/assets/`, `scratch` a directory for stock copies and
/// intermediate bakes.
pub fn convert_replacements(
    ops: &[ResourceOp],
    abs: &BTreeMap<String, PathBuf>,
    env: &ModEnv,
    out: &Path,
    scratch: &Path,
    pct_range: (u8, u8),
    progress: &mut dyn FnMut(u8, &str),
) -> Result<Replaced> {
    let mut r = Replaced::default();
    let active: Vec<&ResourceOp> = ops.iter().filter(|o| o.is_active()).collect();
    let Some(stock) = env.stock.as_ref() else {
        for op in &active {
            r.unsupported.push(format!(
                "{}: not converted, the stock bake is not available to the converter",
                op.source
            ));
        }
        return Ok(r);
    };
    std::fs::create_dir_all(scratch)?;

    let mut mod_sides: BTreeMap<(SourceGame, String), PathBuf> = BTreeMap::new();
    for op in &active {
        let (Some(t), Some(src)) = (&op.target, abs.get(&op.source_key())) else {
            continue;
        };
        if let ["side", file] = t.path.split('/').collect::<Vec<_>>().as_slice() {
            if let Some(stem) = file.strip_suffix(".lvl") {
                mod_sides.insert((t.game, stem.to_owned()), src.clone());
            }
        }
    }

    let (pct_min, pct_max) = pct_range;
    let pct_span = u64::from(pct_max.saturating_sub(pct_min));
    let total_bytes: u64 = active
        .iter()
        .map(|op| op.source_size.max(1))
        .sum::<u64>()
        .max(1);
    let mut done_bytes = 0u64;

    let mut written: BTreeMap<String, ReplacementOutput> = BTreeMap::new();
    for (i, op) in active.iter().enumerate() {
        let op_bytes = op.source_size.max(1);
        let op_start = pct_min + ((done_bytes * pct_span) / total_bytes) as u8;
        let op_end = pct_min + (((done_bytes + op_bytes) * pct_span) / total_bytes) as u8;
        done_bytes += op_bytes;
        progress(op_start, &format!("Converting {}...", op.source));
        let (Some(target), Some(src)) = (op.target.clone(), abs.get(&op.source_key())) else {
            r.warnings
                .push(format!("{}: source file not found; skipped", op.source));
            continue;
        };
        crate::gameplay::exempt_addon(src);
        let Some(stock_lvl) = env.catalogs.lvl(&target) else {
            r.unsupported
                .push(format!("{}: {} is not in the stock catalog", op.source, target.label()));
            continue;
        };
        let staging = scratch.join(format!("op{i}"));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging)?;
        let mut job = Job {
            op,
            src,
            target: &target,
            stock_lvl,
            env,
            stock,
            staging: &staging,
            scratch,
            mod_sides: &mod_sides,
            removed: BTreeSet::new(),
            handled: BTreeSet::new(),
            addon_models: BTreeSet::new(),
            r: &mut r,
            pct_range: (op_start, op_end.max(op_start)),
            progress,
        };
        let result = match classify(target.game, &target.path) {
            Kind::Side(_) if stock_lvl.outputs.is_empty() => {
                job.r.unsupported.push(format!(
                    "{}: OpenBF builds nothing from the stock {}, so replacing it has no effect",
                    op.source, target.path
                ));
                job.handled = all_changes(op);
                Ok(())
            }
            Kind::Side(stem) => job.side(&stem),
            Kind::Fpm => job.fpm(),
            Kind::World(map_dir) => job.world(&map_dir),
            Kind::Load(map_dir) => job.loadscreen(&map_dir),
            Kind::Other => {
                job.r.unsupported.push(format!(
                    "{}: replacing {} is not supported yet",
                    op.source, target.path
                ));
                Ok(())
            }
        };
        if let Err(e) = result {
            job.r
                .warnings
                .push(format!("{}: conversion failed ({e:#}); the stock version stays", op.source));
            let _ = std::fs::remove_dir_all(&staging);
            continue;
        }
        let removed = std::mem::take(&mut job.removed);
        job.report_unhandled();
        commit(&staging, out, target.game, &removed, &mut written)?;
        let _ = std::fs::remove_dir_all(&staging);
    }
    r.outputs = written.into_values().collect();
    Ok(r)
}

fn all_changes(op: &ResourceOp) -> BTreeSet<String> {
    op.changes
        .iter()
        .flat_map(|d| {
            d.changed
                .iter()
                .chain(&d.added)
                .map(move |n| format!("{}:{}", d.tag, n.to_ascii_lowercase()))
        })
        .collect()
}

enum Kind {
    Side(String),
    Fpm,
    World(String),
    Load(String),
    Other,
}

fn classify(game: SourceGame, logical: &str) -> Kind {
    let segs: Vec<&str> = logical.split('/').collect();
    match segs.as_slice() {
        ["side", file] => match file.strip_suffix(".lvl") {
            Some(stem) => Kind::Side(stem.to_owned()),
            None => Kind::Other,
        },
        ["fpm", ..] => Kind::Fpm,
        ["load", _] => match crate::stock::load_output_dir(game, logical) {
            Some(dir) => Kind::Load(dir),
            None => Kind::Other,
        },
        _ => match crate::stock::map_output_dir(game, logical) {
            Some(dir) => Kind::World(dir),
            None => Kind::Other,
        },
    }
}

/// Move every file below `staging` to `out` at the same relative path, and
/// record it (and every removed path) for `game`.
fn commit(
    staging: &Path,
    out: &Path,
    game: SourceGame,
    removed: &BTreeSet<String>,
    written: &mut BTreeMap<String, ReplacementOutput>,
) -> Result<()> {
    for rel in files_below(staging) {
        let dst = out.join(&rel);
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(staging.join(&rel), &dst)
            .with_context(|| format!("writing {}", dst.display()))?;
        written.insert(
            rel.to_ascii_lowercase(),
            ReplacementOutput {
                path: rel,
                game,
                removed: false,
            },
        );
    }
    for rel in removed {
        let key = rel.to_ascii_lowercase();
        if written.get(&key).is_some_and(|o| !o.removed) {
            continue;
        }
        written.insert(
            key,
            ReplacementOutput {
                path: rel.clone(),
                game,
                removed: true,
            },
        );
    }
    Ok(())
}

/// Every file below `root`, relative and `/`-separated, sorted.
fn files_below(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// A directory holding the four sides as the stock soldier bake loads them
/// side by side (`ALL`, `IMP`, `REP`, `CIS`) and `own`: the mod's own where
/// `mod_sides` has one, else the stock PC copy. Also the animation LVL, with
/// the borrowed-clip lender set ([`crate::soldier::set_lender`]).
pub(super) fn sibling_sides(
    env: &ModEnv,
    scratch: &Path,
    game: SourceGame,
    own: &str,
    mod_sides: &BTreeMap<(SourceGame, String), PathBuf>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Result<(PathBuf, PathBuf)> {
    let dir = scratch.join("sides").join(game.key());
    std::fs::create_dir_all(&dir)?;
    let mut names: Vec<String> = SIDES.iter().map(|s| s.to_string()).collect();
    if !names.iter().any(|n| n == own) {
        names.push(own.to_owned());
    }
    for s in &names {
        let dst = dir.join(format!("{}.LVL", s.to_ascii_uppercase()));
        if dst.is_file() {
            continue;
        }
        let src = match mod_sides.get(&(game, s.clone())) {
            Some(p) => p.clone(),
            None => match env.sources.file(
                game,
                SourcePlatform::Pc,
                &format!("side/{s}.lvl"),
                scratch,
            ) {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!(
                        "{label}: soldiers load without the stock {s}.lvl beside them ({e:#}); configure the PC copy in the launcher"
                    ));
                    continue;
                }
            },
        };
        std::fs::copy(&src, &dst)
            .with_context(|| format!("copying {} -> {}", src.display(), dst.display()))?;
        if mod_sides.contains_key(&(game, s.clone())) {
            crate::gameplay::exempt_addon(&dst);
        }
    }
    let (own_anims, lender_game) = match game {
        SourceGame::Bf2 => ("bf2_anims", SourceGame::Bf1),
        SourceGame::Bf1 => ("bf1_anims", SourceGame::Bf2),
    };
    let anims = anim_lvl(env, scratch, game, own_anims, label, warnings)?
        .ok_or_else(|| anyhow!("no {} animation LVL is configured", game.key()))?;
    let stock_has_lender = env.stock.as_ref().is_some_and(|stock| {
        stock.exists(&format!("common_{}/common.ron", lender_game.key()))
    });
    let lender = if stock_has_lender {
        let role = match lender_game {
            SourceGame::Bf1 => "bf1_anims",
            SourceGame::Bf2 => "bf2_anims",
        };
        anim_lvl(env, scratch, lender_game, role, label, warnings)?
    } else {
        None
    };
    let lender_var = match game {
        SourceGame::Bf2 => "BF1_ANIMS",
        SourceGame::Bf1 => "BF2_ANIMS",
    };
    crate::soldier::set_lender(lender_var, lender.as_deref());
    Ok((dir, anims))
}

/// The animation LVL of `game` the stock bake used (`role` in
/// `catalog/stock_bake.ron`, checked byte for byte), else the configured
/// PS2 copy's, else the PC copy's.
fn anim_lvl(
    env: &ModEnv,
    scratch: &Path,
    game: SourceGame,
    role: &str,
    label: &str,
    warnings: &mut Vec<String>,
) -> Result<Option<PathBuf>> {
    if let Some(dep) = env.catalogs.bake.dependency(role) {
        return env
            .sources
            .verified_file(dep, scratch)
            .map(Some)
            .with_context(|| describe(dep));
    }
    let rel = match game {
        SourceGame::Bf2 => "ingame.lvl",
        SourceGame::Bf1 => "common.lvl",
    };
    for platform in [SourcePlatform::Ps2, SourcePlatform::Pc] {
        if let Ok(p) = env.sources.file(game, platform, rel, scratch) {
            warnings.push(format!(
                "{label}: animations from {} {} {rel}, not checked against the stock bake (it recorded none)",
                game.key(),
                platform.key()
            ));
            return Ok(Some(p));
        }
    }
    Ok(None)
}

struct Job<'a> {
    op: &'a ResourceOp,
    src: &'a Path,
    target: &'a TargetRef,
    stock_lvl: &'a CatalogLvl,
    env: &'a ModEnv,
    stock: &'a StockAssets,
    staging: &'a Path,
    scratch: &'a Path,
    mod_sides: &'a BTreeMap<(SourceGame, String), PathBuf>,
    removed: BTreeSet<String>,
    /// `tag:name` of every changed resource a step converted.
    handled: BTreeSet<String>,
    /// Models the mod's classes attach through `GeometryAddon`.
    addon_models: BTreeSet<String>,
    r: &'a mut Replaced,
    pct_range: (u8, u8),
    progress: &'a mut dyn FnMut(u8, &str),
}

/// How a changed model is baked, by the stock output it replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelKind {
    Soldier,
    Weapon,
    Vehicle,
}

/// A model baked off the job thread, with what it asks of the sheets.
enum Baked {
    Soldier((crate::soldier::BakedSoldier, Vec<crate::soldier::Request>)),
    Weapon((crate::soldier::BakedWeapon, crate::soldier::Request)),
}

impl Job<'_> {
    fn sub_progress(&mut self, done: usize, total: usize, detail: &str) {
        let (lo, hi) = self.pct_range;
        let span = usize::from(hi.saturating_sub(lo));
        let pct = lo + ((done.min(total) * span) / total.max(1)) as u8;
        (self.progress)(pct, &format!("Converting {} ({detail})...", self.op.source));
    }

    fn g(&self) -> &'static str {
        self.target.game.key()
    }

    fn changed(&self, tag: &str) -> Vec<String> {
        self.op
            .changes
            .iter()
            .filter(|d| d.tag == tag)
            .flat_map(|d| d.changed.iter().chain(&d.added).cloned())
            .collect()
    }

    fn mark(&mut self, tag: &str, name: &str) {
        self.handled
            .insert(format!("{tag}:{}", name.to_ascii_lowercase()));
    }

    /// Every changed or added resource no step converted, by kind.
    fn report_unhandled(&mut self) {
        let mut by_kind: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for d in &self.op.changes {
            for name in d.changed.iter().chain(&d.added) {
                let lower = name.to_ascii_lowercase();
                let handled = match d.tag.as_str() {
                    "modl" | "skel" | "gmod" => ["modl", "skel", "gmod"]
                        .iter()
                        .any(|t| self.handled.contains(&format!("{t}:{lower}"))),
                    "coll" | "prim" => ["coll", "prim"]
                        .iter()
                        .any(|t| self.handled.contains(&format!("{t}:{lower}"))),
                    tag => self.handled.contains(&format!("{tag}:{lower}")),
                };
                if handled {
                    continue;
                }
                let what = match d.tag.as_str() {
                    "modl" | "skel" | "gmod" if self.addon_models.contains(&lower) => {
                        "attachment models (`GeometryAddon`, which OpenBF does not draw yet)"
                            .to_owned()
                    }
                    "modl" | "skel" | "gmod" => "models no stock OpenBF output uses".to_owned(),
                    "tex_" => "textures no converted model or stock output uses".to_owned(),
                    "coll" | "prim" => "collision outside a converted vehicle".to_owned(),
                    "CLTH" => "cloth outside a converted soldier".to_owned(),
                    "entc" | "ordc" | "wpnc" | "expc" => "classes".to_owned(),
                    "fx__" => "effects".to_owned(),
                    "comb" => "combos".to_owned(),
                    "snd_" | "SndB" | "SBnk" | "_pad" | "emo_" => "sounds".to_owned(),
                    "scr_" => "scripts".to_owned(),
                    "Locl" => "localized text".to_owned(),
                    tag => format!("`{tag}` resources"),
                };
                by_kind.entry(what).or_default().insert(lower);
            }
        }
        for (what, names) in by_kind {
            let shown: Vec<&str> = names.iter().take(6).map(String::as_str).collect();
            let more = names.len().saturating_sub(shown.len());
            self.r.unsupported.push(format!(
                "{}: {} {what} not converted ({}{})",
                self.op.source,
                names.len(),
                shown.join(", "),
                if more > 0 {
                    format!(", +{more} more")
                } else {
                    String::new()
                }
            ));
        }
    }

    fn side(&mut self, stem: &str) -> Result<()> {
        let mod_buf = std::fs::read(self.src).with_context(|| self.src.display().to_string())?;
        let mod_classes = crate::classes::read_classes(
            &mod_buf,
            &crate::classes::CLASS_TAGS,
            crate::classes::MalformedClass::Strict,
        )?;
        self.class_patch(stem, &mod_buf, &mod_classes)?;
        self.models(stem, &mod_buf, &mod_classes)?;
        self.effects(stem)?;
        Ok(())
    }

    /// The class patch over `classes/<g>_<stem>.ron`.
    fn class_patch(&mut self, stem: &str, mod_buf: &[u8], mod_classes: &[ClassDef]) -> Result<()> {
        let class_file = format!("classes/{}_{stem}.ron", self.g());
        let mod_facts = mungefs::inventory::scan_bytes(mod_buf);
        let patch = class_patch_with_mode(
            &mod_facts,
            &self.stock_lvl.facts,
            mod_classes,
            &format!("{} over {}", self.op.source, self.target.label()),
            self.op.mode == ReplacementMode::Full,
        );
        for p in &patch.classes {
            let name = p.name().to_owned();
            for tag in crate::classes::CLASS_TAGS {
                self.mark(std::str::from_utf8(tag).unwrap_or(""), &name);
            }
        }
        if patch.classes.is_empty() {
            return Ok(());
        }
        if !self.stock.exists(&class_file) {
            self.r.unsupported.push(format!(
                "{}: {} class change(s), but the stock build has no {class_file}",
                self.op.source,
                patch.classes.len()
            ));
            return Ok(());
        }
        let path = manifest::class_patch::patch_path(&self.staging.join(&class_file));
        crate::write_ron(&path, &patch)?;
        Ok(())
    }

    /// The stock bake's classes of this game (every `classes/<g>_*.ron`),
    /// for class chains that end outside the mod's LVL (BF2's side classes
    /// derive from INGAME's `com_inf_default`).
    fn stock_classes(&self) -> Vec<ClassDef> {
        let prefix = format!("{}_", self.g());
        let mut out = Vec::new();
        for f in self.stock.list("classes") {
            let Some(file) = f.strip_prefix("classes/") else {
                continue;
            };
            if file.contains('/') || !file.starts_with(&prefix) || !file.ends_with(".ron") || file.ends_with(".patch.ron") {
                continue;
            }
            if let Some(defs) = self
                .stock
                .read(&f)
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|t| ron::from_str::<Vec<ClassDef>>(&t).ok())
            {
                out.extend(defs);
            }
        }
        out
    }

    fn models(&mut self, stem: &str, mod_buf: &[u8], mod_classes: &[ClassDef]) -> Result<()> {
        let g = self.g();
        let mut lookup: Vec<ClassDef> = mod_classes.to_vec();
        lookup.extend(self.stock_classes());
        let low_res = mungefs::fnv1a_lowercase("GeometryLowRes");
        let addon = mungefs::fnv1a_lowercase("GeometryAddon");
        for c in mod_classes {
            for (k, v) in &c.props {
                if *k == addon {
                    self.addon_models.insert(v.trim().to_ascii_lowercase());
                }
            }
        }
        let vehicle_dir = format!("side_{g}/vehicles/{stem}");
        let vehicle_models: BTreeSet<String> = self
            .stock
            .list(&format!("{vehicle_dir}/models"))
            .iter()
            .filter_map(|f| f.rsplit('/').next())
            .filter_map(|f| f.strip_suffix(".glb"))
            .map(str::to_ascii_lowercase)
            .collect();
        let has_vehicles = !vehicle_models.is_empty();
        let geometry = mungefs::fnv1a_lowercase("GeometryName");

        let mut wanted: BTreeMap<String, ModelKind> = BTreeMap::new();
        let mut names: Vec<String> = self.changed("modl");
        names.extend(self.changed("skel"));
        names.extend(self.changed("gmod"));
        names.sort();
        names.dedup();
        for m in &names {
            let kind = if self.stock.exists(&format!("side_{g}/soldiers/{m}.glb")) {
                Some(ModelKind::Soldier)
            } else if self.stock.exists(&format!("side_{g}/weapons/{m}.glb")) {
                Some(ModelKind::Weapon)
            } else if vehicle_models.contains(m) {
                Some(ModelKind::Vehicle)
            } else {
                mod_classes
                    .iter()
                    .find(|c| {
                        c.props
                            .iter()
                            .any(|(k, v)| *k == geometry && v.eq_ignore_ascii_case(m))
                    })
                    .and_then(|c| match chain_root(&lookup, &c.name).as_str() {
                        "soldier" => Some(ModelKind::Soldier),
                        _ if c.kind == "wpnc" => Some(ModelKind::Weapon),
                        "hover" | "flyer" | "walker" | "commandwalker" | "commandflyer"
                        | "commandhover" | "armedbuilding" | "vehicle"
                            if has_vehicles =>
                        {
                            Some(ModelKind::Vehicle)
                        }
                        _ => None,
                    })
            };
            if let Some(k) = kind {
                wanted.insert(m.clone(), k);
            }
        }

        let side_out = self.staging.join(format!("side_{g}"));
        let side_out_s = side_out.to_string_lossy().into_owned();
        let src_s = self.src.to_string_lossy().into_owned();
        let has_soldiers = wanted.values().any(|k| *k == ModelKind::Soldier);
        let has_weapons = wanted.values().any(|k| *k == ModelKind::Weapon);
        let total_models = wanted.len().max(1);
        let total_units = total_models + 2;
        let side_cache = if has_soldiers {
            let (dir, anims) = self.sibling_dir()?;
            let lvl = crate::join_ci(&dir, &format!("{}.LVL", self.target_stem_upper()));
            self.sub_progress(0, total_units, "loading side & animation banks");
            Some(
                crate::soldier::SideBakeCache::load(
                    &lvl.to_string_lossy(),
                    Some(&anims.to_string_lossy()),
                    Some(&src_s),
                )?
                .low_needs_same_sheets(),
            )
        } else if has_weapons {
            Some(crate::soldier::SideBakeCache::load(&src_s, None, Some(&src_s))?)
        } else {
            None
        };
        let mut flags: HashMap<String, u32> = HashMap::new();
        let mut produced_textures: BTreeSet<String> = BTreeSet::new();

        // Models bake side by side; what each asks of the shared sheets is
        // then applied in model order, which decides the flags a weapon sees
        // and the last word on each sheet.
        let jobs: Vec<(&String, &ModelKind)> = wanted
            .iter()
            .filter(|(_, k)| **k != ModelKind::Vehicle)
            .collect();
        if let Some(cache) = &side_cache {
            if !jobs.is_empty() {
                self.sub_progress(0, total_units, &format!("{} models", jobs.len()));
            }
            let baked = crate::soldier::in_parallel(
                &jobs,
                |(m, kind)| match kind {
                    ModelKind::Soldier => cache.soldier_model(m, &side_out_s).map(Baked::Soldier),
                    _ => cache.weapon_model(m, &side_out_s).map(Baked::Weapon),
                },
                |done, (m, _)| {
                    self.sub_progress(
                        done,
                        total_units,
                        &format!("model {done}/{}: {m}", jobs.len()),
                    );
                },
            );
            let mut plan = crate::soldier::Plan::default();
            for ((m, kind), res) in jobs.iter().zip(baked) {
                match res {
                    Ok(Baked::Soldier((b, requests))) => {
                        let sheets = cache.sheets()?;
                        for request in &requests {
                            plan.add(sheets, request);
                        }
                        for (name, f) in b.texture_flags {
                            *flags.entry(name).or_default() |= f;
                        }
                        produced_textures.extend(b.textures.iter().map(|t| t.to_ascii_lowercase()));
                        self.mark_model(m);
                        for c in mod_classes {
                            if c.props
                                .iter()
                                .any(|(k, v)| *k == geometry && v.trim().eq_ignore_ascii_case(m))
                            {
                                for (k, v) in &c.props {
                                    if *k == low_res {
                                        self.mark_model(&v.trim().to_ascii_lowercase());
                                    }
                                }
                            }
                        }
                        self.hide_missing(&format!("side_{g}/soldiers"), m);
                    }
                    Ok(Baked::Weapon((w, mut request))) => {
                        request.flags = crate::soldier::merge_prior(request.flags, &flags);
                        plan.add(cache.sheets()?, &request);
                        produced_textures.extend(w.textures.iter().map(|t| t.to_ascii_lowercase()));
                        self.mark_model(m);
                        self.hide_missing(&format!("side_{g}/weapons"), m);
                        let pc_fp = format!("side_{g}/pc_first_person/weapons/{m}.glb");
                        if self.stock.exists(&pc_fp) {
                            self.removed.insert(pc_fp);
                        }
                    }
                    Err(e) => {
                        let what = if **kind == ModelKind::Soldier {
                            "soldier"
                        } else {
                            "weapon"
                        };
                        self.r
                            .warnings
                            .push(format!("{}: {what} {m} kept stock ({e:#})", self.op.source));
                    }
                }
            }
            if !plan.is_empty() {
                self.sub_progress(
                    jobs.len(),
                    total_units,
                    &format!("writing {} textures", plan.len()),
                );
                cache
                    .sheets()?
                    .write(&plan, &side_out.join("textures"), cache.pc_textures())?;
            }
        }
        // The vehicle bake loads its own copies, so the side's go first.
        let side_pc = side_cache.and_then(crate::soldier::SideBakeCache::into_pc_textures);

        if wanted.values().any(|k| *k == ModelKind::Vehicle)
            || self.changed("coll").iter().any(|c| vehicle_models.contains(c))
        {
            self.sub_progress(total_models, total_units, "vehicles");
            let vout = self.staging.join(&vehicle_dir);
            match crate::vehicle::bake_vehicles(&src_s, &vout.to_string_lossy(), Some(&src_s)) {
                Ok(v) => {
                    for name in v.models.iter().chain(v.geometries.iter().map(|g| &g.model)) {
                        self.mark_model(&name.to_ascii_lowercase());
                    }
                    for geo in &v.geometries {
                        self.mark_model(&geo.name.to_ascii_lowercase());
                        for (_, lod) in &geo.lods {
                            self.mark_model(&lod.to_ascii_lowercase());
                        }
                    }
                    for name in &v.textures {
                        self.mark("tex_", name);
                    }
                    for c in self.changed("coll").into_iter().chain(self.changed("prim")) {
                        self.mark("coll", &c);
                        self.mark("prim", &c);
                    }
                    if self.op.mode == ReplacementMode::Full {
                        let produced: BTreeSet<String> = files_below(&vout)
                            .into_iter()
                            .map(|f| f.to_ascii_lowercase())
                            .collect();
                        for f in self.stock.list(&vehicle_dir) {
                            let rel = f
                                .strip_prefix(&format!("{vehicle_dir}/"))
                                .unwrap_or(&f)
                                .to_ascii_lowercase();
                            if !produced.contains(&rel) && !rel.ends_with(".tier2.ktx2") {
                                self.removed.insert(f.clone());
                            }
                        }
                    }
                }
                Err(e) => self.r.warnings.push(format!(
                    "{}: vehicles kept stock ({e:#})",
                    self.op.source
                )),
            }
        }

        // Textures the stock bake wrote from this side whose art the mod
        // changed, where no model above already wrote them.
        let mut tex_only = Vec::new();
        for t in self.changed("tex_") {
            let lower = t.to_ascii_lowercase();
            if produced_textures.contains(&lower) {
                self.mark("tex_", &t);
                continue;
            }
            if self.stock.exists(&format!("side_{g}/textures/{lower}.ktx2")) {
                tex_only.push(lower);
            }
        }
        if !tex_only.is_empty() {
            self.sub_progress(total_models + 1, total_units, &format!("{} textures", tex_only.len()));
            let inner = mungefs::open(mod_buf).map_err(|e| anyhow!("{e}"))?;
            for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
                if &c.tag != b"modl" {
                    continue;
                }
                if let Ok(model) = mungefs::model::Model::parse(&c) {
                    for (name, f) in crate::soldier::bake_flags(&model) {
                        *flags.entry(name).or_default() |= f;
                    }
                }
            }
            let loaded_pc;
            let pc_ref = match side_pc.as_ref() {
                Some(pc) => pc,
                None => {
                    loaded_pc = crate::tier::PcTextures::load(&src_s)?;
                    &loaded_pc
                }
            };
            let done = crate::soldier::bake_textures_flagged(inner, &tex_only, &flags, &side_out, Some(pc_ref))?;
            for t in done {
                self.mark("tex_", &t);
            }
        }
        for t in &produced_textures {
            self.mark("tex_", t);
        }
        self.both_tiers()?;
        Ok(())
    }

    fn mark_model(&mut self, m: &str) {
        for tag in ["modl", "skel", "gmod", "CLTH"] {
            self.mark(tag, m);
        }
        for d in &self.op.changes {
            if d.tag == "CLTH" {
                for c in d.changed.iter().chain(&d.added) {
                    if c.to_ascii_lowercase().starts_with(m) {
                        self.handled.insert(format!("CLTH:{}", c.to_ascii_lowercase()));
                    }
                }
            }
        }
    }

    /// Hide the stock companions of `<dir>/<model>.*` the re-bake did not
    /// write (a stock cloth, combo or low-detail model the mod's version
    /// lacks). In `Patch` mode stock companions are preserved.
    fn hide_missing(&mut self, dir: &str, model: &str) {
        if self.op.mode != ReplacementMode::Full {
            return;
        }
        let produced: BTreeSet<String> = files_below(&self.staging.join(dir))
            .into_iter()
            .map(|f| f.to_ascii_lowercase())
            .collect();
        let prefix = format!("{dir}/{model}.");
        for f in self.stock.list(dir) {
            let lower = f.to_ascii_lowercase();
            if !lower.starts_with(&prefix) {
                continue;
            }
            let file = &lower[dir.len() + 1..];
            if !produced.contains(file) {
                self.removed.insert(f);
            }
        }
    }

    /// Every texture written below the staging tree, also at the second
    /// tier wherever the stock bake has one.
    fn both_tiers(&mut self) -> Result<()> {
        for rel in files_below(self.staging) {
            let Some(base) = rel.strip_suffix(".ktx2") else {
                continue;
            };
            if base.ends_with(".tier2") {
                continue;
            }
            let tier2 = format!("{base}.tier2.ktx2");
            if self.staging.join(&tier2).is_file() {
                continue;
            }
            if self.stock.exists(&tier2) {
                std::fs::copy(self.staging.join(&rel), self.staging.join(&tier2))?;
            }
        }
        Ok(())
    }

    fn target_stem_upper(&self) -> String {
        self.target
            .path
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".lvl"))
            .unwrap_or("")
            .to_ascii_uppercase()
    }

    /// [`sibling_sides`] for this replacement.
    fn sibling_dir(&mut self) -> Result<(PathBuf, PathBuf)> {
        let own = self.target_stem_upper().to_ascii_lowercase();
        sibling_sides(
            self.env,
            self.scratch,
            self.target.game,
            &own,
            self.mod_sides,
            &self.op.source,
            &mut self.r.warnings,
        )
    }

    fn anim_lvl(&mut self, game: SourceGame, role: &str) -> Result<Option<PathBuf>> {
        anim_lvl(
            self.env,
            self.scratch,
            game,
            role,
            &self.op.source,
            &mut self.r.warnings,
        )
    }

    fn effects(&mut self, stem: &str) -> Result<()> {
        let fx = self.changed("fx__");
        if fx.is_empty() {
            return Ok(());
        }
        let g = self.g();
        let dir = format!("side_{g}/effects/{stem}");
        if self.stock.list(&dir).is_empty() {
            return Ok(());
        }
        let out = self.staging.join(&dir);
        match crate::effects::bake_side(&self.src.to_string_lossy(), &out.to_string_lossy(), g) {
            Ok(_) => {
                for f in fx {
                    self.mark("fx__", &f);
                }
            }
            Err(e) => self
                .r
                .warnings
                .push(format!("{}: effects kept stock ({e:#})", self.op.source)),
        }
        Ok(())
    }

    fn fpm(&mut self) -> Result<()> {
        let g = self.g();
        let changed = self.changed("modl");
        let fpm_dir = format!("side_{g}/fpm");
        let arms: Vec<String> = changed
            .iter()
            .filter(|m| self.stock.exists(&format!("{fpm_dir}/{m}.glb")))
            .cloned()
            .collect();
        if arms.is_empty() {
            let tex = self.changed("tex_");
            let out = self.staging.join(format!("side_{g}"));
            let wanted: Vec<String> = tex
                .iter()
                .map(|t| t.to_ascii_lowercase())
                .filter(|t| self.stock.exists(&format!("side_{g}/textures/{t}.ktx2")))
                .collect();
            if !wanted.is_empty() {
                let buf = std::fs::read(self.src)?;
                let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
                let mut flags = HashMap::new();
                for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
                    if &c.tag == b"modl" {
                        if let Ok(model) = mungefs::model::Model::parse(&c) {
                            for (name, f) in crate::soldier::bake_flags(&model) {
                                *flags.entry(name).or_default() |= f;
                            }
                        }
                    }
                }
                let pc = crate::tier::PcTextures::load(&self.src.to_string_lossy())?;
                for t in crate::soldier::bake_textures_flagged(inner, &wanted, &flags, &out, Some(&pc))? {
                    self.mark("tex_", &t);
                }
                self.both_tiers()?;
            }
            return Ok(());
        }
        let role = match self.target.game {
            SourceGame::Bf2 => "bf2_anims",
            SourceGame::Bf1 => "bf1_anims",
        };
        let anims = self
            .anim_lvl(self.target.game, role)?
            .ok_or_else(|| anyhow!("no {g} animation LVL is configured"))?;
        let abuf = std::fs::read(&anims).with_context(|| anims.display().to_string())?;
        let ainner = mungefs::open(&abuf).map_err(|e| anyhow!("{e}"))?;
        let banks = mungefs::anim::find_banks(ainner).map_err(|e| anyhow!("{e}"))?;
        let bank = banks
            .iter()
            .find(|b| {
                b.name.eq_ignore_ascii_case(manifest::fpm::BF1_BANK)
                    || b.name.eq_ignore_ascii_case(manifest::fpm::BF2_BANK)
            })
            .ok_or_else(|| anyhow!("{}: no first-person animation bank", anims.display()))?;
        let out = self.staging.join(format!("side_{g}"));
        let pc = crate::tier::PcTextures::load(&self.src.to_string_lossy())?;
        if let Some(b) = crate::fpm::bake_fpm_with_bank(
            &self.src.to_string_lossy(),
            bank,
            &out.to_string_lossy(),
            Some(&pc),
        )? {
            self.mark_model(&b.name.to_ascii_lowercase());
            for t in &b.textures {
                self.mark("tex_", t);
            }
        }
        self.both_tiers()?;
        Ok(())
    }

    fn world(&mut self, map_dir: &str) -> Result<()> {
        if !self.stock.exists(&format!("{map_dir}/manifest.ron")) {
            self.r.unsupported.push(format!(
                "{}: OpenBF has no stock map at {map_dir}, so replacing {} has no effect",
                self.op.source, self.target.path
            ));
            self.handled = all_changes(self.op);
            return Ok(());
        }
        let non_tex_changes = self.op.changes.iter().any(|d| {
            d.tag != "tex_"
                && (!d.changed.is_empty() || !d.added.is_empty() || !d.removed.is_empty())
        });
        if !non_tex_changes {
            return self.world_textures_only(map_dir);
        }

        // Full world replacement: stage the LVL under `<dir>/<file>.lvl` with
        // `SIDE/TUR.LVL` alongside so `bake_map` resolves stock building turrets
        // even when the mod ships a single loose world LVL.
        let world_stage = self.scratch.join("world");
        let _ = std::fs::remove_dir_all(&world_stage);
        let staged_lvl = world_stage.join(&self.target.path);
        if let Some(parent) = staged_lvl.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(self.src, &staged_lvl)
            .with_context(|| format!("staging world LVL {}", self.src.display()))?;
        crate::gameplay::exempt_addon(&staged_lvl);
        if self.target.game == SourceGame::Bf2 {
            let tur_dst = world_stage.join("SIDE").join("TUR.LVL");
            let tur_src = self
                .mod_sides
                .get(&(SourceGame::Bf2, "tur".to_owned()))
                .cloned()
                .or_else(|| {
                    self.env
                        .sources
                        .file(SourceGame::Bf2, SourcePlatform::Pc, "side/tur.lvl", self.scratch)
                        .ok()
                });
            if let Some(tur) = tur_src {
                std::fs::create_dir_all(tur_dst.parent().unwrap())?;
                if std::fs::copy(&tur, &tur_dst).is_ok() {
                    crate::gameplay::exempt_addon(&tur_dst);
                }
            }
        }

        let map_out = self.staging.join(map_dir);
        std::fs::create_dir_all(&map_out)?;
        crate::map::bake_map(
            &staged_lvl.to_string_lossy(),
            &map_out.to_string_lossy(),
            None,
        )
        .with_context(|| format!("baking world replacement {}", self.op.source))?;
        self.both_tiers()?;

        if self.op.mode == ReplacementMode::Full {
            let produced: BTreeSet<String> = files_below(&map_out)
                .into_iter()
                .map(|f| f.to_ascii_lowercase())
                .collect();
            for f in self.stock.list(map_dir) {
                let rel = f
                    .strip_prefix(&format!("{map_dir}/"))
                    .unwrap_or(&f)
                    .to_ascii_lowercase();
                if rel.starts_with("sounds/")
                    || rel.starts_with("load/")
                    || rel.starts_with("load_order/")
                {
                    continue;
                }
                if !produced.contains(&rel) {
                    self.removed.insert(f);
                }
            }
        }

        const WORLD_TAGS: &[&str] = &[
            "tex_", "modl", "skel", "gmod", "tern", "wrld", "sky_", "lght", "coll", "prim",
            "plan", "plnp", "PATH", "path", "WATR", "LOWR", "fx__", "prp_", "entc", "ordc",
            "wpnc", "expc", "anim", "anmg", "CLTH", "FOLG",
        ];
        for d in &self.op.changes {
            if WORLD_TAGS.contains(&d.tag.as_str()) {
                for name in d.changed.iter().chain(&d.added) {
                    self.mark(&d.tag, name);
                }
            }
        }
        Ok(())
    }

    /// Texture-only world replacement: re-encode only the modified textures
    /// into `<map_dir>/textures/` (and `.tier2.ktx2`), preserving stock
    /// PS2-canonical geometry, terrain, collision, nav and gameplay untouched.
    fn world_textures_only(&mut self, map_dir: &str) -> Result<()> {
        let buf = std::fs::read(self.src).with_context(|| self.src.display().to_string())?;
        let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
        let expanded = mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))?;
        let mut decoded: Vec<mungefs::texture::Texture> = Vec::new();
        let mut by_lower: HashMap<String, usize> = HashMap::new();
        for c in &expanded {
            if &c.tag != b"tex_" {
                continue;
            }
            if let Ok(t) = mungefs::texture::Texture::parse(c) {
                let lower = t.name.to_ascii_lowercase();
                if let std::collections::hash_map::Entry::Vacant(e) = by_lower.entry(lower) {
                    e.insert(decoded.len());
                    decoded.push(t);
                }
            }
        }

        let manifest: Option<manifest::Manifest> = self
            .stock
            .read(&format!("{map_dir}/manifest.ron"))
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| ron::from_str(&s).ok());
        let terrain_detail = manifest
            .as_ref()
            .and_then(|m| m.terrain_detail.as_ref().map(|(n, _)| n.to_ascii_lowercase()));
        let transparent: BTreeSet<String> = manifest
            .as_ref()
            .map(|m| {
                m.transparent
                    .iter()
                    .map(|n| n.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();

        let tex_prefix = format!("{map_dir}/textures");
        let mut stock_files: HashMap<String, String> = HashMap::new();
        for rel in self.stock.list(&tex_prefix) {
            if let Some(fname) = rel.strip_prefix(&format!("{tex_prefix}/")) {
                stock_files.insert(fname.to_ascii_lowercase(), fname.to_owned());
            }
        }

        let tex_out = self.staging.join(&tex_prefix);
        std::fs::create_dir_all(&tex_out)?;
        for name in self.changed("tex_") {
            let lower = name.to_ascii_lowercase();
            let Some(&idx) = by_lower.get(&lower) else {
                continue;
            };
            if let Some(base) = lower.strip_suffix("_dtl") {
                let dtl_key = crate::detail::detail_file(base);
                if let Some(stock_name) = stock_files.get(&dtl_key) {
                    crate::detail::write_ktx2(&tex_out, stock_name, &decoded[idx])?;
                    self.mark("tex_", &lower);
                    continue;
                }
            }
            let ktx_key = format!("{lower}.ktx2");
            let Some(stock_name) = stock_files.get(&ktx_key) else {
                continue;
            };
            let tex = &decoded[idx];
            let mut resolved = crate::tier::Resolved::from_ps2(tex);
            if terrain_detail.as_deref() == Some(lower.as_str()) {
                resolved.promote_rgb_to_alpha_if_opaque();
            }
            let stock_rel = format!("{tex_prefix}/{stock_name}");
            let needs_alpha = transparent.contains(&lower)
                || self
                    .stock
                    .read(&stock_rel)
                    .is_some_and(|bytes| ktx2_has_non_opaque_alpha(&bytes));
            if !needs_alpha {
                resolved.make_opaque();
            }
            let stem = stock_name.strip_suffix(".ktx2").unwrap_or(&lower);
            resolved.write_pair(&tex_out, stem, None)?;
            if let Some(dtl) = crate::detail::write_detail_for(&tex_out, tex, &decoded)? {
                self.mark("tex_", &dtl);
            }
            self.mark("tex_", &lower);
        }
        self.both_tiers()?;
        Ok(())
    }

    fn loadscreen(&mut self, map_dir: &str) -> Result<()> {
        let load_dir = format!("{map_dir}/load");
        if self.stock.list(&load_dir).is_empty() {
            self.r.unsupported.push(format!(
                "{}: OpenBF has no stock load screen at {load_dir}, so replacing {} has no effect",
                self.op.source, self.target.path
            ));
            self.handled = all_changes(self.op);
            return Ok(());
        }
        let out = self.staging.join(map_dir);
        std::fs::create_dir_all(&out)?;
        crate::loadscreen::bake_loadscreen(
            &self.src.to_string_lossy(),
            &out.to_string_lossy(),
        )
        .with_context(|| format!("baking load screen {}", self.op.source))?;
        self.both_tiers()?;
        for tag in ["tex_", "modl", "skel", "gmod", "load"] {
            for name in self.changed(tag) {
                self.mark(tag, &name);
            }
        }
        Ok(())
    }
}

fn ktx2_has_non_opaque_alpha(bytes: &[u8]) -> bool {
    let Ok(reader) = ktx2::Reader::new(bytes) else {
        return false;
    };
    let Some(level) = reader.levels().next() else {
        return false;
    };
    level
        .data
        .chunks_exact(4)
        .any(|px| px[3] < 255)
}

fn describe(dep: &StockDependency) -> String {
    format!(
        "the stock bake's {} ({} {} {})",
        dep.role,
        dep.game.key(),
        dep.platform.key(),
        dep.path
    )
}

/// The BASE chain's root class name of `name` among `classes` (`soldier`,
/// `hover`, ...): the first BASE that names no class of `classes`.
fn chain_root(classes: &[ClassDef], name: &str) -> String {
    let mut cur = name.to_owned();
    for _ in 0..16 {
        match classes.iter().find(|c| c.name.eq_ignore_ascii_case(&cur)) {
            Some(c) if !c.base.is_empty() => cur = c.base.clone(),
            _ => return cur.to_ascii_lowercase(),
        }
    }
    cur.to_ascii_lowercase()
}

/// The patch that carries what the mod's classes changed relative to the
/// stock PC copy: per class, every property key whose value digest
/// differs from the catalog's ([`mungefs::inventory`]), with the mod's
/// values; every key the mod dropped; a changed BASE; every class only the
/// mod has (whole) and (in `Full` mode) every stock class the mod dropped.
pub fn class_patch(
    mod_facts: &LvlFacts,
    stock_facts: &LvlFacts,
    mod_classes: &[ClassDef],
    source: &str,
) -> ClassPatchFile {
    class_patch_with_mode(mod_facts, stock_facts, mod_classes, source, true)
}

pub fn class_patch_with_mode(
    mod_facts: &LvlFacts,
    stock_facts: &LvlFacts,
    mod_classes: &[ClassDef],
    source: &str,
    full: bool,
) -> ClassPatchFile {
    let stock: BTreeMap<String, &LvlResource> = stock_facts
        .resources
        .iter()
        .filter(|r| r.is_class())
        .map(|r| (format!("{}:{}", r.tag, r.name.to_ascii_lowercase()), r))
        .collect();
    let modded: BTreeMap<String, &LvlResource> = mod_facts
        .resources
        .iter()
        .filter(|r| r.is_class())
        .map(|r| (format!("{}:{}", r.tag, r.name.to_ascii_lowercase()), r))
        .collect();
    let mut classes = Vec::new();
    let mut seen = BTreeSet::new();
    for def in mod_classes {
        let key = format!("{}:{}", def.kind, def.name.to_ascii_lowercase());
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(m) = modded.get(&key) else { continue };
        let Some(s) = stock.get(&key) else {
            classes.push(ClassPatch::Add(def.clone()));
            continue;
        };
        if m.digest == s.digest && m.props == s.props && m.base == s.base {
            continue;
        }
        let stock_keys: BTreeMap<u32, u64> = s.props.iter().copied().collect();
        let mod_keys: BTreeMap<u32, u64> = m.props.iter().copied().collect();
        let mut set = Vec::new();
        let mut order = Vec::new();
        for (k, _) in &def.props {
            if !order.contains(k) {
                order.push(*k);
            }
        }
        for k in order {
            if stock_keys.get(&k) != mod_keys.get(&k) {
                let values = def
                    .props
                    .iter()
                    .filter(|(h, _)| *h == k)
                    .map(|(_, v)| v.clone())
                    .collect();
                set.push((k, values));
            }
        }
        let remove: Vec<u32> = stock_keys
            .keys()
            .filter(|k| !mod_keys.contains_key(k))
            .copied()
            .collect();
        let base = (m.base != s.base).then(|| def.base.clone());
        if set.is_empty() && remove.is_empty() && base.is_none() {
            continue;
        }
        classes.push(ClassPatch::Change {
            name: def.name.clone(),
            base,
            set,
            remove,
        });
    }
    if full {
        for (key, s) in &stock {
            if !modded.contains_key(key) {
                classes.push(ClassPatch::Remove {
                    name: s.name.clone(),
                });
            }
        }
    }
    ClassPatchFile {
        schema: CLASS_PATCH_SCHEMA,
        source: source.to_owned(),
        full,
        classes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(tag: &str, name: &str, props: &[(u32, u64)], base: &str) -> LvlResource {
        LvlResource {
            tag: tag.into(),
            name: name.into(),
            digest: props.iter().map(|p| p.1).sum(),
            size: 0,
            props: props.to_vec(),
            base: Some(base.into()),
        }
    }

    fn facts(resources: Vec<LvlResource>) -> LvlFacts {
        LvlFacts {
            resources,
            ..Default::default()
        }
    }

    fn def(name: &str, base: &str, props: &[(u32, &str)]) -> ClassDef {
        ClassDef {
            name: name.into(),
            kind: "entc".into(),
            base: base.into(),
            props: props.iter().map(|(k, v)| (*k, v.to_string())).collect(),
        }
    }

    #[test]
    fn a_patch_carries_only_the_keys_the_mod_changed() {
        let stock = facts(vec![
            res("entc", "imp_inf_rifleman", &[(1, 10), (2, 20), (3, 30)], "imp_inf_default"),
            res("entc", "imp_inf_gone", &[(1, 10)], "soldier"),
        ]);
        let modded = facts(vec![
            res("entc", "imp_inf_rifleman", &[(1, 10), (2, 21)], "imp_inf_default"),
            res("entc", "imp_inf_new", &[(1, 10)], "soldier"),
        ]);
        let classes = vec![
            def("imp_inf_rifleman", "imp_inf_default", &[(1, "a"), (2, "x"), (2, "y")]),
            def("imp_inf_new", "soldier", &[(1, "a")]),
        ];
        let p = class_patch(&modded, &stock, &classes, "test");
        assert_eq!(
            p.classes[0],
            ClassPatch::Change {
                name: "imp_inf_rifleman".into(),
                base: None,
                set: vec![(2, vec!["x".into(), "y".into()])],
                remove: vec![3],
            }
        );
        assert_eq!(p.classes[1], ClassPatch::Add(classes[1].clone()));
        assert_eq!(
            p.classes[2],
            ClassPatch::Remove {
                name: "imp_inf_gone".into()
            }
        );
    }

    #[test]
    fn an_unchanged_class_is_left_out() {
        let r = res("entc", "imp_inf_rifleman", &[(1, 10)], "soldier");
        let p = class_patch(
            &facts(vec![r.clone()]),
            &facts(vec![r]),
            &[def("imp_inf_rifleman", "soldier", &[(1, "a")])],
            "test",
        );
        assert!(p.classes.is_empty());
    }

    #[test]
    fn a_class_chain_ends_at_its_engine_base() {
        let classes = vec![
            def("imp_hero_vader", "imp_inf_default", &[]),
            def("imp_inf_default", "soldier", &[]),
        ];
        assert_eq!(chain_root(&classes, "imp_hero_vader"), "soldier");
    }

    #[test]
    fn patch_mode_does_not_remove_unlisted_stock_classes() {
        let stock = facts(vec![
            res("entc", "imp_inf_rifleman", &[(1, 10)], "soldier"),
            res("entc", "imp_inf_kept", &[(1, 10)], "soldier"),
        ]);
        let modded = facts(vec![res("entc", "imp_inf_rifleman", &[(1, 20)], "soldier")]);
        let classes = vec![def("imp_inf_rifleman", "soldier", &[(1, "mod")])];
        let full = class_patch_with_mode(&modded, &stock, &classes, "full", true);
        assert_eq!(full.classes.len(), 2);
        assert!(matches!(full.classes[1], ClassPatch::Remove { .. }));
        let patch = class_patch_with_mode(&modded, &stock, &classes, "patch", false);
        assert_eq!(patch.classes.len(), 1);
        assert!(matches!(patch.classes[0], ClassPatch::Change { .. }));
    }

    #[test]
    fn classify_recognizes_sides_fpms_worlds_and_loadscreens() {
        assert!(matches!(
            classify(SourceGame::Bf2, "side/imp.lvl"),
            Kind::Side(s) if s == "imp"
        ));
        assert!(matches!(
            classify(SourceGame::Bf2, "fpm/imp/allxwing.lvl"),
            Kind::Fpm
        ));
        assert!(matches!(
            classify(SourceGame::Bf2, "end/end1.lvl"),
            Kind::World(d) if d == "bf2_end1"
        ));
        assert!(matches!(
            classify(SourceGame::Bf1, "tat/tat1.lvl"),
            Kind::World(d) if d == "tat1"
        ));
        assert!(matches!(
            classify(SourceGame::Bf2, "tat/tat2.lvl"),
            Kind::World(d) if d == "tat2"
        ));
        assert!(matches!(
            classify(SourceGame::Bf2, "load/end1.lvl"),
            Kind::Load(d) if d == "bf2_end1"
        ));
    }
}
