//! The player's stock data, as a mod conversion needs it: the original LVLs
//! (a PC install's `_LVL_PC`, an extracted disc tree, or the PS2 disc image
//! itself), the baked stock assets (a directory or the packed archive), and
//! the stock resource catalog built from those LVLs.
//!
//! Nothing here writes to a stock file: the conversion reads stock LVLs to
//! translate a mod's changes onto the stock bake, and reads baked assets to
//! know which outputs the stock bake made.

use anyhow::{anyhow, bail, Context, Result};
use extensions::mods::catalog::{
    normalize_logical, CatalogLvl, SourceGame, SourcePlatform, StockBakeInfo, StockCatalog,
    StockDependency, CATALOG_DIR, CATALOG_GENERATOR_VERSION, CATALOG_SCHEMA_VERSION,
    STOCK_BAKE_FILE,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The baked stock assets: `assets_baked/` on disk, or the archive a packed
/// build carries (`openbf.pak` or the executable itself).
pub enum StockAssets {
    Dir(PathBuf),
    Archive {
        archive: pak::Archive,
        names: BTreeSet<String>,
    },
}

const ARCHIVE_ROOT: &str = "assets_baked/";

impl StockAssets {
    pub fn open(path: &Path) -> Result<Self> {
        if path.is_dir() {
            return Ok(Self::Dir(path.to_path_buf()));
        }
        if !path.is_file() {
            bail!("{} does not exist", path.display());
        }
        let archive = pak::find_archive(path)
            .ok_or_else(|| anyhow!("{} carries no game archive", path.display()))?;
        let names = archive
            .names()
            .filter_map(|n| n.strip_prefix(ARCHIVE_ROOT))
            .map(str::to_owned)
            .collect();
        Ok(Self::Archive { archive, names })
    }

    /// The bytes of `rel` (relative to the baked asset root, `/`-separated).
    pub fn read(&self, rel: &str) -> Option<Vec<u8>> {
        match self {
            Self::Dir(d) => std::fs::read(d.join(rel)).ok(),
            Self::Archive { archive, names } => names
                .contains(rel)
                .then(|| archive.read_entry(&format!("{ARCHIVE_ROOT}{rel}")).ok())
                .flatten(),
        }
    }

    pub fn exists(&self, rel: &str) -> bool {
        match self {
            Self::Dir(d) => d.join(rel).is_file(),
            Self::Archive { names, .. } => names.contains(rel),
        }
    }

    /// Every file below `prefix` (a directory, no trailing `/`).
    pub fn list(&self, prefix: &str) -> Vec<String> {
        let pre = format!("{}/", prefix.trim_end_matches('/'));
        match self {
            Self::Archive { names, .. } => names
                .range(pre.clone()..)
                .take_while(|n| n.starts_with(&pre))
                .cloned()
                .collect(),
            Self::Dir(d) => {
                let mut out = Vec::new();
                let mut stack = vec![d.join(prefix)];
                while let Some(dir) = stack.pop() {
                    let Ok(rd) = std::fs::read_dir(&dir) else {
                        continue;
                    };
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            stack.push(p);
                        } else if let Ok(rel) = p.strip_prefix(d) {
                            out.push(rel.to_string_lossy().replace('\\', "/"));
                        }
                    }
                }
                out.sort();
                out
            }
        }
    }

    pub fn catalogs(&self) -> extensions::mods::catalog::StockCatalogs {
        extensions::mods::catalog::StockCatalogs::load_with(&|rel| self.read(rel))
    }
}

/// One configured stock copy of a game.
#[derive(Clone, Debug)]
pub struct StockSource {
    pub game: SourceGame,
    pub platform: SourcePlatform,
    pub path: PathBuf,
}

/// Every stock copy the player configured, as `game:platform=path` pairs.
#[derive(Default)]
pub struct StockSources {
    pub sources: Vec<StockSource>,
}

impl StockSources {
    /// The copies the launcher recorded (`openbf_data/stock_sources.ron`).
    pub fn from_list(list: &extensions::mods::catalog::StockSourceList) -> Self {
        Self {
            sources: list
                .sources
                .iter()
                .map(|e| StockSource {
                    game: e.game,
                    platform: e.platform,
                    path: e.path.clone(),
                })
                .collect(),
        }
    }

    /// Parse `bf2:pc=E:\Games\SWBF2` (the launcher passes one per configured
    /// source).
    pub fn parse_arg(&mut self, arg: &str) -> Result<()> {
        let (head, path) = arg
            .split_once('=')
            .ok_or_else(|| anyhow!("--stock-source wants game:platform=path, got {arg}"))?;
        let (g, p) = head
            .split_once(':')
            .ok_or_else(|| anyhow!("--stock-source wants game:platform=path, got {arg}"))?;
        let game = SourceGame::parse(g).ok_or_else(|| anyhow!("unknown game {g}"))?;
        let platform = SourcePlatform::parse(p).ok_or_else(|| anyhow!("unknown platform {p}"))?;
        self.sources.push(StockSource {
            game,
            platform,
            path: PathBuf::from(path),
        });
        Ok(())
    }

    pub fn has(&self, game: SourceGame, platform: SourcePlatform) -> bool {
        self.sources
            .iter()
            .any(|s| s.game == game && s.platform == platform)
    }

    fn source(&self, game: SourceGame, platform: SourcePlatform) -> Option<&StockSource> {
        self.sources
            .iter()
            .find(|s| s.game == game && s.platform == platform)
    }

    /// The LVL root of a directory source, the way the launcher finds it.
    fn lvl_root(src: &StockSource) -> Option<PathBuf> {
        let dir = &src.path;
        let lvl = src.platform.lvl_dir();
        let tries: Vec<String> = vec![
            format!("GameData/Data/{lvl}"),
            format!("Data/{lvl}"),
            format!("DATA/{lvl}"),
            lvl.to_owned(),
        ];
        let base = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if base.eq_ignore_ascii_case(lvl) {
            return Some(dir.clone());
        }
        tries
            .iter()
            .map(|t| crate::join_ci(dir, t))
            .find(|p| p.is_dir())
    }

    /// The stock LVL at logical path `rel` (below `_LVL_<platform>`), as a
    /// file on disk: the source's own file, or a copy written into `scratch`
    /// for a disc image.
    pub fn file(
        &self,
        game: SourceGame,
        platform: SourcePlatform,
        rel: &str,
        scratch: &Path,
    ) -> Result<PathBuf> {
        let src = self.source(game, platform).ok_or_else(|| {
            anyhow!(
                "no {} {} copy is configured",
                game.key(),
                platform.key()
            )
        })?;
        let rel = normalize_logical(rel).ok_or_else(|| anyhow!("bad stock path {rel}"))?;
        if src.path.is_dir() {
            let root = Self::lvl_root(src).ok_or_else(|| {
                anyhow!(
                    "{}: no {} folder",
                    src.path.display(),
                    platform.lvl_dir()
                )
            })?;
            let p = crate::join_ci(&root, &rel);
            if !p.is_file() {
                bail!("{} has no {rel}", root.display());
            }
            return Ok(p);
        }
        let out = scratch
            .join("stock")
            .join(game.key())
            .join(platform.key())
            .join(&rel);
        if out.is_file() {
            return Ok(out);
        }
        let disc_path = format!("DATA/{}/{}", platform.lvl_dir(), rel);
        let f = std::fs::File::open(&src.path).with_context(|| src.path.display().to_string())?;
        let mut iso = mungefs::iso::Iso::open(std::io::BufReader::new(f))
            .map_err(|e| anyhow!("{}: {e}", src.path.display()))?;
        let i = iso
            .find(&disc_path)
            .ok_or_else(|| anyhow!("{} has no {disc_path}", src.path.display()))?;
        let entry = mungefs::iso::IsoFile {
            path: iso.files[i].path.clone(),
            lba: iso.files[i].lba,
            size: iso.files[i].size,
        };
        let bytes = iso.read_file(&entry).map_err(|e| anyhow!("{e}"))?;
        std::fs::create_dir_all(out.parent().unwrap())?;
        std::fs::write(&out, bytes)?;
        Ok(out)
    }

    /// As [`Self::file`], refusing a file whose SHA-256 is not `sha256`:
    /// a conversion translates against the exact stock bytes it was told of.
    pub fn verified_file(
        &self,
        dep: &StockDependency,
        scratch: &Path,
    ) -> Result<PathBuf> {
        let p = self.file(dep.game, dep.platform, &dep.path, scratch)?;
        let got = mungefs::inventory::sha256_hex(&std::fs::read(&p)?);
        if got != dep.sha256 {
            bail!(
                "{} is not the copy the stock bake used ({} expected, {} found)",
                p.display(),
                dep.sha256,
                got
            );
        }
        Ok(p)
    }
}

/// Every `.lvl` below `root`, with its logical path.
fn lvl_files(root: &Path) -> Vec<(String, PathBuf)> {
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
            } else if p
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("lvl"))
            {
                if let Some(rel) = p
                    .strip_prefix(root)
                    .ok()
                    .and_then(|r| normalize_logical(&r.to_string_lossy()))
                {
                    out.push((rel, p));
                }
            }
        }
    }
    out.sort();
    out
}

/// The baked map directory (`assets_baked/<dir>`) that a stock world LVL
/// feeds (`scripts/rebake.sh`, `launcher::pipeline::rebake`).
pub fn map_output_dir(game: SourceGame, logical: &str) -> Option<String> {
    let segs: Vec<&str> = logical.split('/').collect();
    let [dir, file] = segs.as_slice() else {
        return None;
    };
    let stem = file.strip_suffix(".lvl")?;
    if !stem.starts_with(dir) {
        return None;
    }
    match (game, stem) {
        (SourceGame::Bf1, "tat1") => Some("tat1".into()),
        (SourceGame::Bf2, "tat2") => Some("tat2".into()),
        _ => Some(format!("{}_{stem}", game.key())),
    }
}

/// The baked asset directory whose `load/` folder a stock `load/<name>.lvl`
/// feeds.
pub fn load_output_dir(game: SourceGame, logical: &str) -> Option<String> {
    let segs: Vec<&str> = logical.split('/').collect();
    let ["load", file] = segs.as_slice() else {
        return None;
    };
    let stem = file.strip_suffix(".lvl")?;
    match (game, stem) {
        (_, "common") => Some(format!("common_{}", game.key())),
        (SourceGame::Bf1, "tat1") => Some("tat1".into()),
        (SourceGame::Bf2, "tat2") => Some("tat2".into()),
        _ => Some(format!("{}_{stem}", game.key())),
    }
}

/// The stock outputs a stock LVL fed, by the stock bake's layout
/// (`scripts/rebake.sh`, `launcher::pipeline::rebake`): a side LVL's classes,
/// vehicles, combos and effects; a world LVL's map folder; load-screen
/// folders; common/core/shell assets; and the first-person folder for every
/// FPM LVL. Only outputs that exist are recorded.
pub fn stock_outputs(
    game: SourceGame,
    logical: &str,
    assets: Option<&StockAssets>,
) -> Vec<String> {
    let Some(assets) = assets else {
        return Vec::new();
    };
    let g = game.key();
    let segs: Vec<&str> = logical.split('/').collect();
    let stem = segs
        .last()
        .and_then(|f| f.strip_suffix(".lvl"))
        .unwrap_or("");
    let mut out = Vec::new();
    let mut file = |rel: String| {
        if assets.exists(&rel) {
            out.push(rel);
        }
    };
    match segs.as_slice() {
        ["side", _] => {
            file(format!("classes/{g}_{stem}.ron"));
            file(format!("side_{g}/combos/{stem}.ron"));
            for dir in ["vehicles", "effects"] {
                let d = format!("side_{g}/{dir}/{stem}");
                if !assets.list(&d).is_empty() {
                    out.push(format!("{d}/"));
                }
            }
        }
        ["fpm", ..] => {
            let d = format!("side_{g}/fpm");
            if !assets.list(&d).is_empty() {
                out.push(format!("{d}/"));
            }
        }
        ["load", _] => {
            if let Some(d) = load_output_dir(game, logical) {
                let load_dir = format!("{d}/load");
                if !assets.list(&load_dir).is_empty() {
                    out.push(format!("{load_dir}/"));
                }
            }
        }
        ["ingame.lvl"] if game == SourceGame::Bf2 => {
            let d = format!("common_{g}");
            if assets.exists(&format!("{d}/common.ron")) {
                out.push(format!("{d}/"));
            }
        }
        ["common", "eng", "common.lvl"] if game == SourceGame::Bf1 => {
            let d = format!("common_{g}");
            if assets.exists(&format!("{d}/common.ron")) {
                out.push(format!("{d}/"));
            }
        }
        ["core.lvl"] => {
            let d = format!("common_{g}");
            if assets.exists(&format!("{d}/locl/english.ron")) {
                out.push(format!("{d}/locl/english.ron"));
            }
        }
        ["shell.lvl"] if game == SourceGame::Bf2 => {
            let d = format!("common_{g}/shell");
            if !assets.list(&d).is_empty() {
                out.push(format!("{d}/"));
            }
        }
        _ => {
            if let Some(d) = map_output_dir(game, logical) {
                if assets.exists(&format!("{d}/manifest.ron")) {
                    out.push(format!("{d}/"));
                }
            }
        }
    }
    out
}

/// `bake stock-catalog <game> <platform> <lvl root> <assets_baked>`: write
/// `<assets_baked>/catalog/<game>_<platform>.ron`.
pub fn write_catalog(
    game: SourceGame,
    platform: SourcePlatform,
    lvl_root: &Path,
    assets_dir: &Path,
) -> Result<usize> {
    let files = lvl_files(lvl_root);
    if files.is_empty() {
        bail!("{}: no LVL files", lvl_root.display());
    }
    let assets = StockAssets::open(assets_dir).ok();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Mutex<Vec<CatalogLvl>> = Mutex::new(Vec::with_capacity(files.len()));
    let threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .min(4);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((rel, path)) = files.get(i) else {
                    break;
                };
                let facts = match mungefs::inventory::scan_file(path) {
                    Ok(f) => f,
                    Err(e) => extensions::mods::catalog::LvlFacts {
                        error: Some(e.to_string()),
                        ..Default::default()
                    },
                };
                let outputs = stock_outputs(game, rel, assets.as_ref());
                results.lock().unwrap().push(CatalogLvl {
                    path: rel.clone(),
                    facts,
                    outputs,
                });
            });
        }
    });
    let mut lvls = results.into_inner().unwrap();
    lvls.sort_by(|a, b| a.path.cmp(&b.path));
    let n = lvls.len();
    let cat = StockCatalog {
        schema_version: CATALOG_SCHEMA_VERSION,
        generator_version: CATALOG_GENERATOR_VERSION,
        game,
        platform,
        lvls,
    };
    let dir = assets_dir.join(CATALOG_DIR);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(StockCatalog::file_name(game, platform)),
        ron::to_string(&cat)?,
    )?;
    Ok(n)
}

/// Record, in `<assets_baked>/catalog/stock_bake.ron`, that `game`'s gameplay
/// was baked from `platform`, and the class property hashes the gameplay
/// correction tables treat as presentation.
pub fn record_gameplay(assets_dir: &Path, game: SourceGame, platform: SourcePlatform) -> Result<()> {
    let presentation: Vec<u32> = presentation_hashes(game).into_iter().collect();
    update_bake_info(assets_dir, |info| {
        info.gameplay.insert(game, platform);
        info.presentation.insert(game, presentation);
    })
}

/// Record a stock LVL a whole bake group read (e.g. the animation bank every
/// soldier bake takes), by role and SHA-256.
pub fn record_dependency(
    assets_dir: &Path,
    role: &str,
    game: SourceGame,
    platform: SourcePlatform,
    lvl_root: &Path,
    rel: &str,
) -> Result<()> {
    let logical = normalize_logical(rel).ok_or_else(|| anyhow!("bad path {rel}"))?;
    let p = crate::join_ci(lvl_root, &logical);
    let sha = mungefs::inventory::sha256_hex(
        &std::fs::read(&p).with_context(|| p.display().to_string())?,
    );
    update_bake_info(assets_dir, |info| {
        info.dependencies.retain(|d| d.role != role);
        info.dependencies.push(StockDependency {
            role: role.to_owned(),
            game,
            platform,
            path: logical.clone(),
            sha256: sha.clone(),
        });
        info.dependencies.sort_by(|a, b| a.role.cmp(&b.role));
    })
}

fn update_bake_info(assets_dir: &Path, f: impl FnOnce(&mut StockBakeInfo)) -> Result<()> {
    let dir = assets_dir.join(CATALOG_DIR);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(STOCK_BAKE_FILE);
    let mut info: StockBakeInfo = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| ron::from_str(&t).ok())
        .unwrap_or_default();
    info.schema_version = 1;
    f(&mut info);
    std::fs::write(&path, ron::ser::to_string_pretty(&info, crate::pretty())?)?;
    Ok(())
}

/// The class property hashes `game`'s gameplay correction tables keep from
/// the PC copy because they only change presentation (`crate::gameplay`).
pub fn presentation_hashes(game: SourceGame) -> BTreeSet<u32> {
    crate::gameplay::presentation_hashes(game.key())
}
