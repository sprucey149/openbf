//! The map list: eras, both games' maps and the catalog built from them.

use super::*;

use std::collections::BTreeMap;

/// `gMapEras` (§6), extended with mod-declared v1.3 eras ([`Era::Custom`]).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Era {
    Cw,
    Gcw,
    /// A mod-declared era (lowercase `addme.script` suffix after `era_`, e.g. `"k"`, `"n"`, `"y"`).
    Custom(String),
}

pub const ERAS: [Era; 2] = [Era::Cw, Era::Gcw];

/// Built-in v1.3 unofficial patch era defaults `(code, name, icon)`, traced from
/// `shell.lvl` (`ifs_era_handler`, `custom_gEras`) and `v1.3patch_strings.lvl`
/// (`common.era.<code>`). Entries without an explicit `icon2` in `custom_gEras`
/// (`i`, `j`, `u`, `v`, `z`) use `custom_UnknownEraIcon2` (`"imp_icon"`).
const V13_ERA_DEFAULTS: [(&str, &str, &str); 26] = [
    ("a", "BFX Clone Wars", "bfx_cw_icon"),
    ("b", "BFX Galactic Civil War", "bfx_gcw_icon"),
    ("d", "New Sith Wars", "newsithwars_icon"),
    ("e", "Earth", "earth_icon"),
    ("f", "Prime Front", "front_icon"),
    ("h", "Halo", "halo_icon"),
    ("i", "I", "imp_icon"),
    ("j", "J", "imp_icon"),
    ("k", "KotOR", "kotor_icon"),
    ("l", "Lego", "lego_icon"),
    ("m", "Movie CW and GCW", "imp_icon"),
    ("n", "New Jedi Order", "newrep_icon"),
    ("o", "Old Sith War", "oldsith_icon"),
    ("p", "Battlefront Project - Clone War", "rep_icon"),
    ("q", "Battlefront Project - Galactic Civil War", "all_icon"),
    ("r", "Red vs Blue", "rvb_icon"),
    ("s", "Birth of the Rebellion", "rebirth_icon"),
    ("t", "Toys", "toys_icon"),
    ("u", "U", "imp_icon"),
    ("v", "V", "imp_icon"),
    ("w", "Wacky", "wacky_icon"),
    ("x", "Extended Civil War", "exgcw_icon"),
    ("y", "Yuuzhan Vong", "yuz_icon"),
    ("z", "Z", "imp_icon"),
    ("1", "+123 Clone Wars", "cis_icon"),
    ("2", "+123 Galactic Civil War", "imp_icon"),
];

impl Era {
    /// Parse a canonical or `addme.script` era code into a built-in or mod-declared [`Era`].
    pub fn from_code(raw: &str) -> Option<Era> {
        let low = raw.trim().to_ascii_lowercase();
        if low.is_empty() || !low.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return None;
        }
        Some(match low.as_str() {
            "c" | "cw" => Era::Cw,
            "g" | "gcw" => Era::Gcw,
            _ => Era::Custom(low),
        })
    }

    /// Canonical code (`"cw"`, `"gcw"`, or the lowercase custom era code).
    pub fn code(&self) -> &str {
        match self {
            Era::Cw => "cw",
            Era::Gcw => "gcw",
            Era::Custom(s) => s.as_str(),
        }
    }

    /// Built-in v1.3 patch `(name, icon)` default for a custom era code, if any.
    pub fn v13_default(code: &str) -> Option<(&'static str, &'static str)> {
        V13_ERA_DEFAULTS
            .iter()
            .find(|(c, _, _)| c.eq_ignore_ascii_case(code))
            .map(|(_, name, icon)| (*name, *icon))
    }

    /// The era's two armies (its stock mission's faction pair).
    pub fn armies(&self) -> [&'static str; 2] {
        match self {
            Era::Cw => ["rep", "cis"],
            Era::Gcw | Era::Custom(_) => ["all", "imp"],
        }
    }

    pub(super) fn text(&self) -> (&'static str, &'static str) {
        match self {
            Era::Cw => ("common.era.cw", "Clone Wars"),
            Era::Gcw => ("common.era.gcw", "Galactic Civil War"),
            Era::Custom(_) => ("common.era.custom", "Custom Era"),
        }
    }

    /// The era's English fallback name (v1.3 patch default for known codes, else `"Era <CODE>"`).
    pub fn english(&self) -> String {
        match self {
            Era::Cw => "Clone Wars".into(),
            Era::Gcw => "Galactic Civil War".into(),
            Era::Custom(s) => Self::v13_default(s)
                .map(|(name, _)| name.to_owned())
                .unwrap_or_else(|| format!("Era {}", s.to_ascii_uppercase())),
        }
    }

    /// The era row's default icon (`SHELL.LVL` §6; v1.3 `ifs_era_handler` `icon2` / `"imp_icon"`).
    pub fn icon(&self) -> &'static str {
        match self {
            Era::Cw => "cis_icon",
            Era::Gcw => "imp_icon",
            Era::Custom(s) => Self::v13_default(s).map_or("imp_icon", |(_, icon)| icon),
        }
    }
}

/// `[ours]` The cross-era pairs after "All Eras" (§7; PLAN: era mix and
/// match), each run on the map's default mission.
pub const MIXED: [[&str; 2]; 4] = [
    ["all", "rep"],
    ["all", "cis"],
    ["imp", "rep"],
    ["imp", "cis"],
];

/// The BF2 IA maps (§6 "BF2 tables"): code, the Clone Wars modes, the
/// Galactic Civil War modes.
pub(super) const BF2_MAPS: [(&str, &[Mode], &[Mode]); 24] = {
    use Mode::*;
    [
        ("cor1", &[Con, Ctf], &[Con, Ctf]),
        ("dag1", &[Con, Ctf], &[Con, Ctf]),
        ("dea1", &[Con, OneFlag], &[Con, OneFlag]),
        ("end1", &[], &[Con, Hunt, OneFlag]),
        ("fel1", &[Con, OneFlag], &[Con, OneFlag]),
        ("geo1", &[Con, Ctf, Hunt], &[]),
        ("hot1", &[], &[Con, OneFlag, Hunt]),
        ("tat3", &[Con, OneFlag], &[Con, OneFlag]),
        ("kam1", &[Con, OneFlag], &[Con, OneFlag]),
        ("kas2", &[Con, Hunt, Ctf], &[Con, Ctf]),
        ("tat2", &[Con, Ctf], &[Con, Ctf, Hunt, Eli]),
        ("mus1", &[Con, Ctf], &[Con, Ctf]),
        ("myg1", &[Con, Ctf], &[Con, Ctf]),
        ("nab2", &[Con, Ctf, Hunt], &[Con, Ctf]),
        ("pol1", &[Con, Ctf], &[Con, Ctf]),
        ("spa7", &[Assault, OneFlag], &[]),
        ("spa8", &[], &[Assault, OneFlag]),
        ("spa3", &[Assault, OneFlag], &[]),
        ("spa6", &[Assault, OneFlag], &[]),
        ("spa9", &[], &[Assault, OneFlag]),
        ("spa1", &[], &[Assault, OneFlag]),
        ("tan1", &[Con, OneFlag], &[Con, OneFlag]),
        ("uta1", &[Con, OneFlag], &[Con, OneFlag]),
        ("yav1", &[Con, OneFlag], &[Con, OneFlag]),
    ]
};

const OPTIONAL_ASSAULT: [&str; 1] = ["cor1"];

/// The Battlefront II worlds and eras available for single-player XL.
pub fn xl_eras(code: &str) -> &'static [Era] {
    match code {
        "geo1" => &[Era::Cw],
        "hot1" => &[Era::Gcw],
        "kas2" => &ERAS,
        _ => &[],
    }
}

fn bf1_hunt_era(code: &str) -> Option<Era> {
    match code {
        "end1" | "tat1" | "tat2" => Some(Era::Gcw),
        "geo1" | "kas1" | "kas2" | "nab1" | "nab2" => Some(Era::Cw),
        _ => None,
    }
}

/// The BF1 IA maps (§6 "BF1 table"): code, name key, English, has a Clone
/// Wars mission, has a Galactic Civil War one. Conquest only.
pub(super) const BF1_MAPS: [(&str, &str, &str, bool, bool); 16] = [
    (
        "bes1",
        "planets.bespin.mapname1",
        "BESPIN: PLATFORMS",
        true,
        true,
    ),
    (
        "bes2",
        "planets.bespin.mapname2",
        "BESPIN: CLOUD CITY",
        true,
        true,
    ),
    (
        "end1",
        "planets.endor.mapname1",
        "ENDOR: BUNKER",
        false,
        true,
    ),
    (
        "geo1",
        "planets.geonosis.mapname1",
        "GEONOSIS: SPIRE",
        true,
        false,
    ),
    (
        "hot1",
        "planets.hoth.mapname1",
        "HOTH: ECHO BASE",
        false,
        true,
    ),
    (
        "kam1",
        "planets.kamino.mapname1",
        "KAMINO: TIPOCA CITY",
        true,
        false,
    ),
    (
        "kas1",
        "planets.kashyyyk.mapname1",
        "KASHYYYK: ISLANDS",
        true,
        true,
    ),
    (
        "kas2",
        "planets.kashyyyk.mapname2",
        "KASHYYYK: DOCKS",
        true,
        true,
    ),
    (
        "nab1",
        "planets.naboo.mapname1",
        "NABOO: PLAINS",
        true,
        true,
    ),
    ("nab2", "planets.naboo.mapname2", "NABOO: THEED", true, true),
    (
        "rhn1",
        "planets.rhenvar.mapname1",
        "RHEN VAR: HARBOR",
        true,
        true,
    ),
    (
        "rhn2",
        "planets.rhenvar.mapname2",
        "RHEN VAR: CITADEL",
        true,
        true,
    ),
    (
        "tat1",
        "planets.tatooine.mapname1",
        "TATOOINE: DUNE SEA",
        true,
        true,
    ),
    (
        "tat2",
        "planets.tatooine.mapname2",
        "TATOOINE: MOS EISLEY",
        true,
        true,
    ),
    (
        "yav1",
        "planets.yavin.mapname1",
        "YAVIN 4: TEMPLE",
        true,
        true,
    ),
    (
        "yav2",
        "planets.yavin.mapname2",
        "YAVIN 4: ARENA",
        true,
        true,
    ),
];

/// Optional per-era name and icon override declared by a mod's `addme.script`
/// (`change.era_<id>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EraOverride {
    pub name: Option<String>,
    pub icon: Option<String>,
}

/// One map of the merged list.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    /// The bake's directory (`bf2_hot1`, `mod_abr__abr`, the legacy `tat2`), else
    /// `<game>_<code>` for a map that is not baked.
    pub key: String,
    pub game: Game,
    pub code: String,
    /// "<name> (bf1)" / "<name> (bf2)" / "<name> (mod)" (§7).
    pub label: String,
    /// The label without its source suffix, and the folder that holds the
    /// map when the list is grouped ([`InstantAction::group_by_source`]).
    pub name: String,
    pub source: String,
    /// The info box's text: BF2's `mapname.description.<code>`; `[ours]`
    /// BF1 has none on its disc (§6), so its name stands alone.
    pub description: String,
    /// The modes per era (Clone Wars, Galactic Civil War, and any mod-declared eras).
    pub modes: BTreeMap<Era, Vec<Mode>>,
    /// Optional custom era display metadata per era.
    pub era_overrides: BTreeMap<Era, EraOverride>,
    pub baked: bool,
    /// The faction pairs of the port's stock/CW/GCW mission files, per mode.
    pub missions: Vec<(Mode, [String; 2])>,
    /// The faction pairs of mod-declared custom-era mission files, per `(mode, era)`.
    pub custom_era_missions: Vec<(Mode, Era, [String; 2])>,
}

/// Active mod map metadata passed into [`catalog_with_mods`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMapEntry {
    pub key: String,
    pub variant_keys: Vec<String>,
    pub legacy_code: String,
    /// The Battlefront II map (`cor1`) whose row takes these modes and eras,
    /// for what a mod adds to one of the game's maps rather than a map of its
    /// own.
    pub stock_row: Option<String>,
    pub name: String,
    pub disambiguator: String,
    pub description: String,
    pub modes: BTreeMap<Era, Vec<Mode>>,
    /// Optional custom era display metadata per era.
    pub era_overrides: BTreeMap<Era, EraOverride>,
    /// The display name of the mod that adds the map.
    pub source: String,
}

impl MapEntry {
    pub fn modes_for(&self, era: &Era) -> &[Mode] {
        self.modes.get(era).map_or(&[], Vec::as_slice)
    }

    pub(super) fn has_era(&self, era: &Era) -> bool {
        !self.modes_for(era).is_empty()
    }

    pub(super) fn eras(&self) -> Vec<Era> {
        self.modes
            .iter()
            .filter(|(_, m)| !m.is_empty())
            .map(|(e, _)| e.clone())
            .collect()
    }

    /// The map's modes in `gMapModes` order followed by any mod-declared modes (any era).
    pub(super) fn all_modes(&self) -> Vec<Mode> {
        let mut out: Vec<Mode> = MODES
            .into_iter()
            .filter(|m| self.modes.values().any(|e| e.contains(m)))
            .collect();
        let mut custom: Vec<Mode> = Vec::new();
        for era_modes in self.modes.values() {
            for m in era_modes {
                if matches!(m, Mode::Custom(_)) && !custom.contains(m) {
                    custom.push(m.clone());
                }
            }
        }
        custom.sort_by(|a, b| a.subst().cmp(b.subst()));
        out.extend(custom);
        out
    }

    /// Whether the port can run `mode` with `side` here: a baked map, a
    /// mode the sim runs, and a mission file - of that pair (either order)
    /// for a stock era, of that `(mode, era)` for a custom era, any of the
    /// mode for a cross-era pair (§7).
    pub fn runnable(&self, mode: &Mode, side: &Side) -> bool {
        if !self.baked || !mode.is_supported() {
            return false;
        }
        if let Side::Era(era @ Era::Custom(_)) = side {
            return self.modes_for(era).contains(mode)
                && self
                    .custom_era_missions
                    .iter()
                    .any(|(m, e, _)| m == mode && e == era);
        }
        let mut pairs = self.missions.iter().filter(|(m, _)| m == mode);
        if *mode == Mode::Hunt {
            return matches!(side, Side::Era(era) if self.modes_for(era).contains(mode))
                && pairs.next().is_some();
        }
        match side {
            Side::Era(era) => {
                self.modes_for(era).contains(mode)
                    && pairs.any(|(_, p)| {
                        let a = era.armies();
                        (p[0] == a[0] && p[1] == a[1]) || (p[0] == a[1] && p[1] == a[0])
                    })
            }
            Side::Mixed(_) => {
                !matches!(mode, Mode::Eli | Mode::Xl | Mode::Custom(_)) && pairs.next().is_some()
            }
        }
    }

    pub(super) fn launch_armies(&self, mode: &Mode, side: &Side) -> [String; 2] {
        if let Side::Era(era @ Era::Custom(_)) = side {
            if let Some((_, _, pair)) = self
                .custom_era_missions
                .iter()
                .find(|(m, e, _)| m == mode && e == era)
            {
                return pair.clone();
            }
        }
        let armies = side.armies();
        if *mode == Mode::Hunt {
            let mut pairs = self.missions.iter().filter(|(m, _)| m == mode);
            // Addons can register Hunt in both eras. Pick the era's
            // authored pairing before using the retail native-faction row.
            let exact = pairs.clone().find(|(_, pair)| {
                let pair = pair.each_ref().map(String::as_str);
                pair == armies || [pair[1], pair[0]] == armies
            });
            if let Some((_, pair)) = exact.or_else(|| pairs.next()) {
                return pair.clone();
            }
        }
        armies.map(str::to_owned)
    }
}

/// Build the merged list (§6 "Merged list", §7): every map of both games,
/// sorted by the case-folded label. `text(game, key)` is the game's English
/// table; `bakes` the baked maps as (directory, source game); `missions(key,
/// mode)` the faction pairs of that map's mission files for `mode`.
pub fn catalog(
    text: &dyn Fn(Game, &str) -> Option<String>,
    bakes: &[(String, String)],
    missions: &dyn Fn(&str, Mode) -> Vec<[String; 2]>,
) -> Vec<MapEntry> {
    catalog_with_mods(text, bakes, missions, &[])
}

/// Build the merged map list including active content mod maps (`mod_maps`).
pub fn catalog_with_mods(
    text: &dyn Fn(Game, &str) -> Option<String>,
    bakes: &[(String, String)],
    missions_fn: &dyn Fn(&str, Mode) -> Vec<[String; 2]>,
    mod_maps: &[ModMapEntry],
) -> Vec<MapEntry> {
    catalog_with_mods_and_eras(text, bakes, missions_fn, &|_, _, _| Vec::new(), mod_maps)
}

/// Build the merged map list including active content mod maps and mod-declared custom-era missions.
pub fn catalog_with_mods_and_eras(
    text: &dyn Fn(Game, &str) -> Option<String>,
    bakes: &[(String, String)],
    missions_fn: &dyn Fn(&str, Mode) -> Vec<[String; 2]>,
    custom_era_missions_fn: &dyn Fn(&str, Mode, &str) -> Vec<[String; 2]>,
    mod_maps: &[ModMapEntry],
) -> Vec<MapEntry> {
    let find = |game: Game, code: &str| {
        let prefixed = format!("{}_{code}", game.tag());
        bakes
            .iter()
            .find(|(k, g)| *k == prefixed || (k == code && g == game.tag()))
            .map(|(k, _)| k.clone())
    };
    let entry = |game: Game, code: String, name: String, desc: String, stock_modes: [Vec<Mode>; 2]| {
        let baked = find(game, &code);
        let key = baked
            .clone()
            .unwrap_or_else(|| format!("{}_{code}", game.tag()));
        let missions: Vec<_> = MODES
            .into_iter()
            .filter(|mode| *mode != Mode::Xl || (game == Game::Bf2 && !xl_eras(&code).is_empty()))
            .flat_map(|m| {
                let pairs = missions_fn(&key, m.clone());
                pairs.into_iter().map(move |p| (m.clone(), p))
            })
            .collect();
        let mut modes: BTreeMap<Era, Vec<Mode>> = BTreeMap::new();
        let [cw_modes, gcw_modes] = stock_modes;
        if !cw_modes.is_empty() {
            modes.insert(Era::Cw, cw_modes);
        }
        if !gcw_modes.is_empty() {
            modes.insert(Era::Gcw, gcw_modes);
        }
        if game == Game::Bf2 {
            for era in xl_eras(&code) {
                let armies = era.armies();
                if missions.iter().any(|(mode, pair)| {
                    let pair = pair.each_ref().map(String::as_str);
                    *mode == Mode::Xl && (pair == armies || [pair[1], pair[0]] == armies)
                }) {
                    modes.entry(era.clone()).or_default().push(Mode::Xl);
                }
            }
        }
        MapEntry {
            label: format!("{name} ({})", game.tag()),
            name,
            source: game.source_name().to_owned(),
            key,
            game,
            code,
            description: desc,
            modes,
            era_overrides: BTreeMap::new(),
            baked: baked.is_some(),
            missions,
            custom_era_missions: Vec::new(),
        }
    };
    let mut out = Vec::new();
    for (code, cw, gcw) in BF2_MAPS {
        let name = text(Game::Bf2, &format!("mapname.name.{code}"))
            .map_or_else(|| code.to_uppercase(), |t| t.trim().to_owned()); // typography: name data
        let desc = text(Game::Bf2, &format!("mapname.description.{code}"))
            .map_or_else(|| name.clone(), |t| t.trim_end().to_owned());
        let mut gcw = gcw.to_vec();
        let key = find(Game::Bf2, code).unwrap_or_else(|| format!("bf2_{code}"));
        if OPTIONAL_ASSAULT.contains(&code) && !missions_fn(&key, Mode::Eli).is_empty() {
            gcw.push(Mode::Eli);
        }
        out.push(entry(
            Game::Bf2,
            code.to_owned(),
            name,
            desc,
            [cw.to_vec(), gcw],
        ));
    }
    // A BF2-only build bakes no BF1 map, so Instant Action lists none.
    let include_bf1 = bakes.is_empty() || bakes.iter().any(|(_, g)| g == "bf1");
    for (code, key, english, cw, gcw) in BF1_MAPS.into_iter().filter(|_| include_bf1) {
        let localized = text(Game::Bf1, key);
        let name = localized.as_deref().unwrap_or(english).trim();
        // The English BF1 table uses all caps. Keep any existing mixed case
        // and the shared (bf1)/(bf2) suffix used by both menus.
        let name = if name.chars().any(char::is_lowercase) {
            name.to_owned()
        } else {
            name.split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    let first = chars
                        .next()
                        .expect("split_whitespace yields nonempty words");
                    // typography: name data (BF1's capitals to Title Case)
                    first
                        .to_uppercase() // typography: name data
                        .chain(chars.flat_map(char::to_lowercase))
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut modes = [cw, gcw].map(|on| if on { vec![Mode::Con] } else { Vec::new() });
        if let Some(era) = bf1_hunt_era(code) {
            let key = find(Game::Bf1, code).unwrap_or_else(|| format!("bf1_{code}"));
            if !missions_fn(&key, Mode::Hunt).is_empty() {
                let idx = usize::from(era == Era::Gcw);
                modes[idx].push(Mode::Hunt);
            }
        }
        out.push(entry(Game::Bf1, code.to_owned(), name.clone(), name, modes));
    }

    let mut name_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for m in mod_maps.iter().filter(|m| m.stock_row.is_none()) {
        *name_counts.entry(m.name.to_ascii_lowercase()).or_insert(0) += 1;
    }
    for m in mod_maps {
        let mut missions = Vec::new();
        let mut custom_era_missions = Vec::new();
        let mut stock_mod_modes: Vec<Mode> = Vec::new();
        for (era, era_modes) in &m.modes {
            match era {
                Era::Cw | Era::Gcw => {
                    for mode in era_modes {
                        if !stock_mod_modes.contains(mode) {
                            stock_mod_modes.push(mode.clone());
                        }
                    }
                }
                Era::Custom(code) => {
                    for mode in era_modes {
                        for key in &m.variant_keys {
                            for pair in custom_era_missions_fn(key, mode.clone(), code) {
                                let row = (mode.clone(), era.clone(), pair);
                                if !custom_era_missions.contains(&row) {
                                    custom_era_missions.push(row);
                                }
                            }
                        }
                    }
                }
            }
        }
        for mode in stock_mod_modes {
            for key in &m.variant_keys {
                for pair in missions_fn(key, mode.clone()) {
                    let mission = (mode.clone(), pair);
                    if !missions.contains(&mission) {
                        missions.push(mission);
                    }
                }
            }
        }
        if let Some(row) = m.stock_row.as_ref().and_then(|code| {
            out.iter_mut()
                .find(|row| row.game == Game::Bf2 && row.code == *code)
        }) {
            for (era, added) in &m.modes {
                let modes = row.modes.entry(era.clone()).or_default();
                for mode in added {
                    if !modes.contains(mode) {
                        modes.push(mode.clone());
                    }
                }
            }
            for (era, meta) in &m.era_overrides {
                row.era_overrides
                    .entry(era.clone())
                    .or_insert_with(|| meta.clone());
            }
            for mission in missions {
                if !row.missions.contains(&mission) {
                    row.missions.push(mission);
                }
            }
            for mission in custom_era_missions {
                if !row.custom_era_missions.contains(&mission) {
                    row.custom_era_missions.push(mission);
                }
            }
            continue;
        }
        let duplicate = name_counts
            .get(&m.name.to_ascii_lowercase())
            .is_some_and(|&c| c > 1);
        let name = if duplicate && !m.disambiguator.is_empty() {
            format!("{} [{}]", m.name, m.disambiguator)
        } else {
            m.name.clone()
        };
        let label = format!("{name} (mod)");
        let is_baked = bakes.is_empty()
            || bakes.iter().any(|(k, _)| k == &m.key)
            || !missions.is_empty()
            || !custom_era_missions.is_empty();
        out.push(MapEntry {
            key: m.key.clone(),
            game: Game::Bf2,
            code: m.legacy_code.clone(),
            label,
            name,
            source: m.source.clone(),
            description: m.description.clone(),
            modes: m.modes.clone(),
            era_overrides: m.era_overrides.clone(),
            baked: is_baked,
            missions,
            custom_era_missions,
        });
    }

    out.sort_by_key(|m| m.label.to_lowercase());
    out
}
