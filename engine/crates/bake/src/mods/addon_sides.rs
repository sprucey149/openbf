//! An addon map's own SIDE LVLs (`ReadDataFile("dc:SIDE\...")`): the units
//! its missions load from them, written as one layer of the asset tree per
//! map, `levels/<map_key>/assets/`, which the game serves only while that map
//! is loaded (`pak::set_level_scope`). Inside the layer the files have the
//! stock bake's own names (`side_bf2/soldiers/<model>.glb`, ...), so a unit
//! loads as a stock one does, and an addon's copy of a stock model or class
//! stands in for it on that map alone, as the level's own LVLs do.
//!
//! What a layer holds: every soldier and hand-held weapon model its missions'
//! classes name that the map's SIDE LVLs carry, their textures, and the
//! classes (`classes/<file>.patch.ron`), and the sides' particle effects and
//! bolt sheets (`side_bf2/effects/addon/`), and the vehicles of each side
//! LVL a mission loads a vehicle from (`side_bf2/vehicles/0addon*/`). Sounds
//! are not converted here.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use extensions::mods::catalog::{LvlFacts, SourceGame, SourcePlatform};
use manifest::class_patch::{ClassPatch, ClassPatchFile, CLASS_PATCH_SCHEMA};
use manifest::ClassDef;

use super::replace::sibling_sides;
use super::ModEnv;
use crate::stock::StockAssets;

/// Legacy addons are Battlefront II's.
const GAME: SourceGame = SourceGame::Bf2;
const GAME_KEY: &str = "bf2";
/// The class file every BF2 round loads (`game::roster::load_classdb`).
const SHARED_CLASSES: &str = "bf2_ingame.ron";

/// One map's own sides and what its missions ask of them.
#[derive(Debug)]
pub(super) struct LevelSides {
    pub map_key: String,
    /// `(stem, LVL)` in the order the scripts first read them: the first
    /// definition of a name wins, as the game stacks the LVLs it reads.
    pub sides: Vec<(String, PathBuf)>,
    /// `(stem, class)`: the classes the missions' `ReadDataFile` calls name.
    pub requests: BTreeSet<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Model {
    Soldier,
    Weapon,
}

struct SideLvl {
    stem: String,
    classes: Vec<ClassDef>,
    facts: LvlFacts,
    models: BTreeSet<String>,
    wanted: BTreeMap<String, Model>,
    out: PathBuf,
    /// The models that were written.
    baked: BTreeMap<String, Model>,
}

fn prop<'a>(chain: &[&'a ClassDef], key: &str) -> Option<&'a str> {
    let hash = mungefs::fnv1a_lowercase(key);
    // The child's value wins over its parents'; a class's later row over an
    // earlier one.
    chain
        .iter()
        .find_map(|c| c.props.iter().rev().find(|p| p.0 == hash))
        .map(|p| p.1.trim().trim_matches('"'))
        .filter(|v| !v.is_empty())
}

/// Every weapon the class or its parents name, in any slot.
fn weapons(chain: &[&ClassDef]) -> BTreeSet<String> {
    let keys: Vec<u32> = std::iter::once("WeaponName".to_owned())
        .chain((1..=4).map(|i| format!("WeaponName{i}")))
        .map(|k| mungefs::fnv1a_lowercase(&k))
        .collect();
    chain
        .iter()
        .flat_map(|c| &c.props)
        .filter(|(k, _)| keys.contains(k))
        .map(|(_, v)| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect()
}

/// `name` and its BASE chain, child first.
fn chain<'a>(classes: &HashMap<String, &'a ClassDef>, name: &str) -> Vec<&'a ClassDef> {
    let mut out: Vec<&ClassDef> = Vec::new();
    let mut cur = name.to_ascii_lowercase();
    while let Some(def) = classes.get(&cur) {
        if out.len() == 16 {
            break;
        }
        out.push(def);
        cur = def.base.to_ascii_lowercase();
    }
    out
}

/// The stock bake's BF2 class files: `(file name, classes)`.
fn stock_class_files(stock: &StockAssets) -> Vec<(String, Vec<ClassDef>)> {
    stock
        .list("classes")
        .iter()
        .filter_map(|f| f.strip_prefix("classes/"))
        .filter(|f| {
            !f.contains('/')
                && f.starts_with("bf2_")
                && f.ends_with(".ron")
                && !f.ends_with(".patch.ron")
        })
        .filter_map(|f| {
            let bytes = stock.read(&format!("classes/{f}"))?;
            let defs = ron::from_str(&String::from_utf8(bytes).ok()?).ok()?;
            Some((f.to_owned(), defs))
        })
        .collect()
}

/// Is this class exactly as a stock PC LVL has it? Then the stock bake's own
/// (PS2) values stay, as a side replacement keeps them (`super::replace`).
fn is_stock_copy(env: &ModEnv, facts: &LvlFacts, def: &ClassDef) -> bool {
    let name = def.name.to_ascii_lowercase();
    let Some(own) = facts
        .resources
        .iter()
        .find(|r| r.tag == def.kind && r.name.eq_ignore_ascii_case(&name))
    else {
        return false;
    };
    env.catalogs.get(GAME, SourcePlatform::Pc).is_some_and(|c| {
        c.lvls.iter().flat_map(|l| &l.facts.resources).any(|r| {
            r.tag == own.tag
                && r.name.eq_ignore_ascii_case(&name)
                && r.digest == own.digest
                && r.props == own.props
                && r.base == own.base
        })
    })
}

/// Convert `levels` below `staging/levels/`. Returns what could not be done.
pub(super) fn convert_addon_sides(
    levels: &[LevelSides],
    script_names: &BTreeSet<String>,
    env: &ModEnv,
    staging: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    if levels.is_empty() {
        return Ok(warnings);
    }
    let Some(stock) = env.stock.as_ref() else {
        warnings.push(
            "The addon's own units were not converted: the stock bake is not available to the converter."
                .into(),
        );
        return Ok(warnings);
    };
    let stock_files = stock_class_files(stock);
    let scratch = staging.join("side_scratch");

    // Each SIDE LVL once, however many maps read it.
    let mut lvls: BTreeMap<PathBuf, SideLvl> = BTreeMap::new();
    for (stem, path) in levels.iter().flat_map(|l| &l.sides) {
        if lvls.contains_key(path) {
            continue;
        }
        progress(&format!("Reading the addon's {stem}.lvl..."));
        crate::gameplay::exempt_addon(path);
        let read = crate::gameplay::read(path).and_then(|buf| {
            let classes = crate::classes::read_classes(
                &buf,
                &crate::classes::CLASS_TAGS,
                crate::classes::MalformedClass::Skip,
            )?;
            let facts = mungefs::inventory::scan_bytes(&buf);
            Ok((classes, facts))
        });
        let (classes, facts) = match read {
            Ok(x) => x,
            Err(e) => {
                warnings.push(format!("The addon's {stem}.lvl could not be read ({e:#})."));
                continue;
            }
        };
        let models = facts
            .resources
            .iter()
            .filter(|r| r.tag == "modl")
            .map(|r| r.name.to_ascii_lowercase())
            .collect();
        let out = scratch.join(format!("out{}_{stem}", lvls.len()));
        lvls.insert(
            path.clone(),
            SideLvl {
                stem: stem.clone(),
                classes,
                facts,
                models,
                wanted: BTreeMap::new(),
                out,
                baked: BTreeMap::new(),
            },
        );
    }

    // Which models each LVL has to give: the requested classes' bodies and
    // weapons, each from the first of the map's sides that carries it.
    // Per map, the positions in its read order of the sides that define a
    // vehicle one of its missions loads.
    let mut vehicle_sides: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); levels.len()];
    let mut unknown = BTreeSet::new();
    for (index, level) in levels.iter().enumerate() {
        let mut wanted: Vec<(PathBuf, String, Model)> = Vec::new();
        {
            let mut classes: HashMap<String, &ClassDef> = HashMap::new();
            for (_, path) in &level.sides {
                for c in lvls.get(path).into_iter().flat_map(|l| &l.classes) {
                    classes.entry(c.name.to_ascii_lowercase()).or_insert(c);
                }
            }
            for c in stock_files.iter().flat_map(|(_, defs)| defs) {
                classes.entry(c.name.to_ascii_lowercase()).or_insert(c);
            }
            let mut want = |model: &str, kind: Model| {
                let model = model.to_ascii_lowercase();
                if let Some((_, path)) = level
                    .sides
                    .iter()
                    .find(|(_, p)| lvls.get(p).is_some_and(|l| l.models.contains(&model)))
                {
                    wanted.push((path.clone(), model, kind));
                }
            };
            // A script may pick a unit while it runs (a random hero) and
            // load it by a variable, so every soldier of the map's sides that
            // a script of the mod names is asked for too.
            let mut requests = level.requests.clone();
            for (side, path) in &level.sides {
                for c in lvls.get(path).into_iter().flat_map(|l| &l.classes) {
                    let name = c.name.to_ascii_lowercase();
                    if script_names.contains(&name)
                        && chain(&classes, &name)
                            .last()
                            .is_some_and(|root| root.base.eq_ignore_ascii_case("soldier"))
                    {
                        requests.insert((side.clone(), name));
                    }
                }
            }
            for (side, request) in &requests {
                let units = chain(&classes, request);
                let Some(root) = units.last() else {
                    unknown.insert(format!("{request} ({side}.lvl)"));
                    continue;
                };
                if !root.base.eq_ignore_ascii_case("soldier") {
                    // A class only the stock files have keeps the stock bake.
                    if let Some(at) = defining_side(level, &lvls, request) {
                        vehicle_sides[index].insert(at);
                    }
                    continue;
                }
                if let Some(body) = prop(&units, "GeometryName") {
                    want(body, Model::Soldier);
                }
                for weapon in weapons(&units) {
                    let weapon = chain(&classes, &weapon);
                    // The held model, and the first-person one where the
                    // weapon names its own.
                    for key in ["GeometryName", "HighResGeometry"] {
                        if let Some(model) = prop(&weapon, key) {
                            want(model, Model::Weapon);
                        }
                    }
                }
            }
        }
        for (path, model, kind) in wanted {
            if let Some(l) = lvls.get_mut(&path) {
                // A model both a body and a held item name is a body.
                l.wanted
                    .entry(model)
                    .and_modify(|k| *k = (*k).min(kind))
                    .or_insert(kind);
            }
        }
    }
    if !unknown.is_empty() {
        warnings.push(summary("addon classes no LVL defines", &unknown));
    }

    // The addon's own `all.lvl` and the like stand beside the other stock
    // sides for the animation bake.
    let total = lvls.values().filter(|l| !l.wanted.is_empty()).count();
    for (done, (path, lvl)) in lvls
        .iter_mut()
        .filter(|(_, l)| !l.wanted.is_empty())
        .enumerate()
    {
        progress(&format!(
            "Converting the addon's {}.lvl units ({}/{total}, {} models)...",
            lvl.stem,
            done + 1,
            lvl.wanted.len()
        ));
        let work = scratch.join(format!("work{done}"));
        if let Err(e) = bake_side(env, stock, path, lvl, &work, &mut warnings) {
            warnings.push(format!(
                "The addon's {}.lvl units could not be converted ({e:#}).",
                lvl.stem
            ));
        }
        // The side copies are as large as the game's own LVLs.
        let _ = std::fs::remove_dir_all(&work);
    }

    // Each side LVL's vehicles once, however many maps load from it.
    let mut vehicle_layers: BTreeMap<&PathBuf, PathBuf> = BTreeMap::new();
    for (level, wanted) in levels.iter().zip(&vehicle_sides) {
        for &at in wanted {
            let (stem, path) = &level.sides[at];
            let next = vehicle_layers.len();
            vehicle_layers.entry(path).or_insert_with(|| {
                let out = scratch.join(format!("veh{next}"));
                progress(&format!("Converting the addon's {stem}.lvl vehicles..."));
                let source = path.to_string_lossy();
                if let Err(e) =
                    crate::vehicle::bake_vehicles(&source, &out.to_string_lossy(), Some(&source))
                {
                    warnings.push(format!(
                        "The addon's {stem}.lvl vehicles could not be converted ({e:#})."
                    ));
                    let _ = std::fs::remove_dir_all(&out);
                }
                out
            });
        }
    }

    // Maps that read the same sides share one effects layer. The layers are
    // independent, so they bake together over one read of each side.
    let orders: Vec<Vec<&PathBuf>> = levels
        .iter()
        .map(|l| l.sides.iter().map(|(_, p)| p).collect())
        .collect();
    let mut unique: Vec<(&[&PathBuf], &str)> = Vec::new();
    let mut layer_of = Vec::with_capacity(levels.len());
    for (level, order) in levels.iter().zip(&orders) {
        let at = unique.iter().position(|(o, _)| *o == order.as_slice());
        layer_of.push(at.unwrap_or(unique.len()));
        if at.is_none() {
            unique.push((order, &level.map_key));
        }
    }
    let bufs: BTreeMap<&PathBuf, _> = lvls
        .keys()
        .map(|p| (p, crate::gameplay::read(p)))
        .collect();
    let jobs: Vec<usize> = (0..unique.len()).collect();
    let layer_dir = |at: usize| scratch.join(format!("fx{at}"));
    let results = crate::soldier::in_parallel(
        &jobs,
        |&at| bake_effects(unique[at].0, &lvls, &bufs, &layer_dir(at)),
        |done, _| {
            progress(&format!(
                "Converting the addon's effects ({done}/{})...",
                jobs.len()
            ))
        },
    );
    for ((_, map_key), result) in unique.iter().zip(results) {
        if let Err(e) = result {
            warnings.push(format!(
                "The addon's side effects for `{map_key}` could not be converted ({e:#})."
            ));
        }
    }
    drop(bufs);

    // Each map writes only its own folder, mostly links to the shared bakes,
    // and the file system takes those far faster from several threads.
    let maps: Vec<usize> = (0..levels.len()).collect();
    let written = crate::soldier::in_parallel(
        &maps,
        |&at| -> Result<()> {
            let level = &levels[at];
            write_level(level, env, stock, &stock_files, &lvls, staging)
                .with_context(|| format!("writing the addon units of {}", level.map_key))?;
            let to = staging.join("levels").join(&level.map_key).join("assets");
            link_files(&layer_dir(layer_of[at]), &to.join(EFFECTS_DIR))?;
            for &side in &vehicle_sides[at] {
                let (stem, path) = &level.sides[side];
                if let Some(layer) = vehicle_layers.get(path) {
                    link_files(layer, &to.join(vehicle_dir(side, stem)))?;
                }
            }
            Ok(())
        },
        |done, _| {
            progress(&format!(
                "Writing each map's units ({done}/{})...",
                maps.len()
            ))
        },
    );
    written.into_iter().collect::<Result<()>>()?;
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(warnings)
}

/// The position in `level`'s read order of the first side that defines `class`.
fn defining_side(
    level: &LevelSides,
    lvls: &BTreeMap<PathBuf, SideLvl>,
    class: &str,
) -> Option<usize> {
    level.sides.iter().position(|(_, p)| {
        lvls.get(p)
            .is_some_and(|l| l.classes.iter().any(|c| c.name.eq_ignore_ascii_case(class)))
    })
}

/// The level layer's directory for the vehicles of the side read `at`th. The
/// game takes a vehicle from the first of `side_bf2/vehicles/*` that has it,
/// in path order: these sort before the stock sides, in the map's read order.
fn vehicle_dir(at: usize, stem: &str) -> String {
    format!(
        "side_{GAME_KEY}/vehicles/0addon{at:02}_{}",
        stem.to_ascii_lowercase()
    )
}

/// The level layer's directory for the addon's own side effects and bolt
/// sheets, which the game reads before the stock effect layers.
const EFFECTS_DIR: &str = "side_bf2/effects/addon";

/// The particle effects of a map's SIDE LVLs, and the sheets their ordnance
/// classes draw bolts with.
fn bake_effects<B: std::ops::Deref<Target = [u8]>>(
    order: &[&PathBuf],
    lvls: &BTreeMap<PathBuf, SideLvl>,
    bufs: &BTreeMap<&PathBuf, Result<B>>,
    out: &Path,
) -> Result<()> {
    let laser = mungefs::fnv1a_lowercase("LaserTexture");
    let sheets: BTreeSet<String> = order
        .iter()
        .filter_map(|p| lvls.get(*p))
        .flat_map(|l| &l.classes)
        .flat_map(|c| &c.props)
        .filter(|(k, _)| *k == laser)
        .map(|(_, v)| v.trim().trim_matches('"').to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    let slices = order
        .iter()
        .filter_map(|p| bufs.get(*p))
        .map(|b| match b {
            Ok(b) => Ok(&**b),
            Err(e) => Err(anyhow::anyhow!("{e:#}")),
        })
        .collect::<Result<Vec<&[u8]>>>()?;
    let sheets: Vec<String> = sheets.into_iter().collect();
    crate::effects::bake_layers(
        &slices,
        "the addon's sides",
        &out.to_string_lossy(),
        GAME_KEY,
        &sheets,
    )?;
    Ok(())
}

/// Every file below `from`, hard-linked (else copied) below `to`.
fn link_files(from: &Path, to: &Path) -> Result<()> {
    for rel in files_below(from) {
        let target = to.join(&rel);
        if target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let source = from.join(&rel);
        if std::fs::hard_link(&source, &target).is_err() {
            std::fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

fn summary(what: &str, names: &BTreeSet<String>) -> String {
    let shown: Vec<&str> = names.iter().take(6).map(String::as_str).collect();
    let more = names.len() - shown.len();
    format!(
        "{} {what} ({}{})",
        names.len(),
        shown.join(", "),
        if more > 0 {
            format!(", +{more} more")
        } else {
            String::new()
        }
    )
}

/// Bake the wanted models of one SIDE LVL into `lvl.out`.
fn bake_side(
    env: &ModEnv,
    stock: &StockAssets,
    path: &Path,
    lvl: &mut SideLvl,
    work: &Path,
    warnings: &mut Vec<String>,
) -> Result<()> {
    std::fs::create_dir_all(work)?;
    let own = lvl.stem.to_ascii_lowercase();
    let mod_sides = BTreeMap::from([((GAME, own.clone()), path.to_path_buf())]);
    let label = format!("the addon's {own}.lvl");
    let (dir, anims) = sibling_sides(env, work, GAME, &own, &mod_sides, &label, warnings)?;
    let own_lvl = crate::join_ci(&dir, &format!("{}.LVL", own.to_ascii_uppercase()));
    let source = path.to_string_lossy().into_owned();
    let cache = crate::soldier::SideBakeCache::load(
        &own_lvl.to_string_lossy(),
        Some(&anims.to_string_lossy()),
        Some(&source),
    )?
    .low_needs_same_sheets();

    let side_out = lvl.out.join(format!("side_{GAME_KEY}"));
    let side_out_s = side_out.to_string_lossy().into_owned();
    enum Baked {
        Soldier(crate::soldier::BakedSoldier, Vec<crate::soldier::Request>),
        Weapon(crate::soldier::Request),
    }
    let jobs: Vec<(&String, &Model)> = lvl.wanted.iter().collect();
    let results = crate::soldier::in_parallel(
        &jobs,
        |(m, kind)| match kind {
            Model::Soldier => cache
                .soldier_model(m, &side_out_s)
                .map(|(b, r)| Baked::Soldier(b, r)),
            Model::Weapon => cache
                .weapon_model(m, &side_out_s)
                .map(|(_, r)| Baked::Weapon(r)),
        },
        |_, _| {},
    );
    // What each model asks of the shared sheets applies in model order.
    let mut plan = crate::soldier::Plan::default();
    let mut flags: HashMap<String, u32> = HashMap::new();
    let mut failed = BTreeMap::new();
    for ((model, kind), result) in jobs.iter().zip(results) {
        match result {
            Ok(Baked::Soldier(b, requests)) => {
                let sheets = cache.sheets()?;
                for request in &requests {
                    plan.add(sheets, request);
                }
                for (name, f) in b.texture_flags {
                    *flags.entry(name).or_default() |= f;
                }
                lvl.baked.insert((*model).clone(), **kind);
            }
            Ok(Baked::Weapon(mut request)) => {
                request.flags = crate::soldier::merge_prior(request.flags, &flags);
                plan.add(cache.sheets()?, &request);
                lvl.baked.insert((*model).clone(), **kind);
            }
            Err(e) => {
                failed.insert((*model).clone(), format!("{e:#}"));
            }
        }
    }
    if let Some((first, why)) = failed.iter().next() {
        let names: BTreeSet<String> = failed.keys().cloned().collect();
        warnings.push(format!(
            "{label}: {}; {first}: {why}",
            summary("models could not be converted", &names)
        ));
    }
    if !plan.is_empty() {
        cache
            .sheets()?
            .write(&plan, &side_out.join("textures"), cache.pc_textures())?;
    }
    // A sheet the stock bake has at both texture tiers is the addon's at both.
    for rel in files_below(&lvl.out) {
        let Some(base) = rel.strip_suffix(".ktx2") else {
            continue;
        };
        let tier2 = format!("{base}.tier2.ktx2");
        if !base.ends_with(".tier2") && !lvl.out.join(&tier2).is_file() && stock.exists(&tier2) {
            std::fs::copy(lvl.out.join(&rel), lvl.out.join(&tier2))?;
        }
    }
    Ok(())
}

/// Every file below `root`, relative to it, with `/`.
fn files_below(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// One map's layer: its sides' files, the first side's winning, the stock
/// companions the addon's versions of stock models lack, and its classes.
fn write_level(
    level: &LevelSides,
    env: &ModEnv,
    stock: &StockAssets,
    stock_files: &[(String, Vec<ClassDef>)],
    lvls: &BTreeMap<PathBuf, SideLvl>,
    staging: &Path,
) -> Result<()> {
    let root = staging.join("levels").join(&level.map_key);
    let assets = root.join("assets");
    let sides: Vec<&SideLvl> = level.sides.iter().filter_map(|(_, p)| lvls.get(p)).collect();

    for side in &sides {
        for rel in files_below(&side.out) {
            let to = assets.join(&rel);
            if to.exists() {
                continue;
            }
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let from = side.out.join(&rel);
            if std::fs::hard_link(&from, &to).is_err() {
                std::fs::copy(&from, &to)?;
            }
        }
    }

    // A stock cloth, combo or low-detail model the addon's own version of
    // the model lacks must not be worn by it.
    let mut removed = BTreeSet::new();
    for (model, kind) in sides.iter().flat_map(|s| &s.baked) {
        let dir = match kind {
            Model::Soldier => format!("side_{GAME_KEY}/soldiers"),
            Model::Weapon => format!("side_{GAME_KEY}/weapons"),
        };
        let prefix = format!("{dir}/{model}.");
        for file in stock.list(&dir) {
            if file.to_ascii_lowercase().starts_with(&prefix) && !assets.join(&file).is_file() {
                removed.insert(file);
            }
        }
    }
    if !removed.is_empty() {
        std::fs::create_dir_all(&root)?;
        crate::write_ron(&root.join("removed.ron"), &removed)?;
    }

    // The classes. An addon class goes into the shared file every round
    // loads, and also over each stock file that already has its name, since
    // the first file loaded wins.
    let mut patches: BTreeMap<&str, Vec<ClassPatch>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for side in &sides {
        for def in &side.classes {
            if !seen.insert(def.name.to_ascii_lowercase()) {
                continue;
            }
            let owners: Vec<&str> = stock_files
                .iter()
                .filter(|(_, defs)| defs.iter().any(|c| c.name.eq_ignore_ascii_case(&def.name)))
                .map(|(file, _)| file.as_str())
                .collect();
            if !owners.is_empty() && is_stock_copy(env, &side.facts, def) {
                continue;
            }
            for file in owners.iter().copied().chain([SHARED_CLASSES]) {
                let rows = patches.entry(file).or_default();
                if !rows.iter().any(|p| p.name() == def.name) {
                    rows.push(ClassPatch::Add(def.clone()));
                }
            }
        }
    }
    let classes_dir = assets.join("classes");
    for (file, classes) in patches {
        if !stock.exists(&format!("classes/{file}")) {
            return Err(anyhow!("the stock build has no classes/{file}"));
        }
        std::fs::create_dir_all(&classes_dir)?;
        crate::write_ron(
            &manifest::class_patch::patch_path(&classes_dir.join(file)),
            &ClassPatchFile {
                schema: CLASS_PATCH_SCHEMA,
                source: format!("the addon's sides of {}", level.map_key),
                full: false,
                classes,
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str, base: &str, rows: &[(&str, &str)]) -> ClassDef {
        ClassDef {
            name: name.into(),
            kind: "entc".into(),
            base: base.into(),
            props: rows
                .iter()
                .map(|(k, v)| (mungefs::fnv1a_lowercase(k), (*v).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn a_vehicle_comes_from_the_first_side_read_that_defines_it() {
        let side = |classes: &[&str]| SideLvl {
            stem: String::new(),
            classes: classes.iter().map(|n| def(n, "hover", &[])).collect(),
            facts: LvlFacts::default(),
            models: BTreeSet::new(),
            wanted: BTreeMap::new(),
            out: PathBuf::new(),
            baked: BTreeMap::new(),
        };
        let lvls = BTreeMap::from([
            (PathBuf::from("a.lvl"), side(&["x_tank"])),
            (PathBuf::from("b.lvl"), side(&["x_tank", "x_walker"])),
        ]);
        let level = LevelSides {
            map_key: "m".into(),
            sides: vec![("A".into(), "a.lvl".into()), ("B".into(), "b.lvl".into())],
            requests: BTreeSet::new(),
        };
        assert_eq!(defining_side(&level, &lvls, "X_TANK"), Some(0));
        assert_eq!(defining_side(&level, &lvls, "x_walker"), Some(1));
        assert_eq!(defining_side(&level, &lvls, "x_none"), None);
        // The game reads `vehicles/*` in path order: read order, then stock.
        let mut dirs = [
            "side_bf2/vehicles/all".to_owned(),
            vehicle_dir(1, "B"),
            vehicle_dir(0, "Z"),
        ];
        dirs.sort();
        assert_eq!(dirs[0], "side_bf2/vehicles/0addon00_z");
        assert_eq!(dirs[1], "side_bf2/vehicles/0addon01_b");
    }

    #[test]
    fn a_unit_takes_its_body_and_every_weapon_through_its_parents() {
        let defs = [
            def(
                "side_inf_default",
                "soldier",
                &[("GeometryName", "parent_body"), ("WeaponName", "pistol")],
            ),
            def(
                "side_inf_trooper",
                "side_inf_default",
                &[("GeometryName", "\"trooper_body\""), ("WeaponName1", "Rifle")],
            ),
        ];
        let classes: HashMap<String, &ClassDef> =
            defs.iter().map(|d| (d.name.clone(), d)).collect();
        let unit = chain(&classes, "Side_Inf_Trooper");
        assert_eq!(unit.last().unwrap().base, "soldier");
        assert_eq!(prop(&unit, "GeometryName"), Some("trooper_body"));
        assert_eq!(
            weapons(&unit),
            BTreeSet::from(["pistol".to_owned(), "rifle".to_owned()])
        );
    }
}
