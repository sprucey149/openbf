//! The map list: eras, both games' maps and the catalog built from them.

use super::*;

/// `gMapEras` (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Era {
    Cw,
    Gcw,
}

pub const ERAS: [Era; 2] = [Era::Cw, Era::Gcw];

impl Era {
    pub(super) fn index(self) -> usize {
        match self {
            Era::Cw => 0,
            Era::Gcw => 1,
        }
    }

    /// The era's two armies (its mission's faction pair).
    pub fn armies(self) -> [&'static str; 2] {
        match self {
            Era::Cw => ["rep", "cis"],
            Era::Gcw => ["all", "imp"],
        }
    }

    pub(super) fn text(self) -> (&'static str, &'static str) {
        match self {
            Era::Cw => ("common.era.cw", "Clone Wars"),
            Era::Gcw => ("common.era.gcw", "Galactic Civil War"),
        }
    }

    /// The era row's `icon1` (§6).
    pub fn icon(self) -> &'static str {
        match self {
            Era::Cw => "cis_icon",
            Era::Gcw => "imp_icon",
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
    /// The info box's text: BF2's `mapname.description.<code>`; `[ours]`
    /// BF1 has none on its disc (§6), so its name stands alone.
    pub description: String,
    /// The modes per era (Clone Wars, Galactic Civil War), from the disc's
    /// tables.
    pub modes: [Vec<Mode>; 2],
    pub baked: bool,
    /// The faction pairs of the port's mission files, per mode.
    pub missions: Vec<(Mode, [String; 2])>,
}

/// Active mod map metadata passed into [`catalog_with_mods`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMapEntry {
    pub key: String,
    pub variant_keys: Vec<String>,
    pub legacy_code: String,
    pub name: String,
    pub disambiguator: String,
    pub description: String,
    pub modes: [Vec<Mode>; 2],
}

impl MapEntry {
    pub(super) fn has_era(&self, era: Era) -> bool {
        !self.modes[era.index()].is_empty()
    }

    /// The map's modes in `gMapModes` order (any era).
    pub(super) fn all_modes(&self) -> Vec<Mode> {
        MODES
            .into_iter()
            .filter(|m| self.modes.iter().any(|e| e.contains(m)))
            .collect()
    }

    /// Whether the port can run `mode` with `armies` here: a baked map, a
    /// mode the sim runs, and a mission file - of that pair (either order)
    /// for an era, any of the mode for a cross-era pair (§7).
    pub fn runnable(&self, mode: Mode, side: Side) -> bool {
        if !self.baked || !PORT_MODES.contains(&mode) {
            return false;
        }
        let mut pairs = self.missions.iter().filter(|(m, _)| *m == mode);
        if mode == Mode::Hunt {
            return matches!(side, Side::Era(era) if self.modes[era.index()].contains(&mode))
                && pairs.next().is_some();
        }
        match side {
            Side::Era(era) => {
                self.modes[era.index()].contains(&mode)
                    && pairs.any(|(_, p)| {
                        let a = era.armies();
                        (p[0] == a[0] && p[1] == a[1]) || (p[0] == a[1] && p[1] == a[0])
                    })
            }
            Side::Mixed(_) => !matches!(mode, Mode::Eli | Mode::Xl) && pairs.next().is_some(),
        }
    }

    pub(super) fn launch_armies(&self, mode: Mode, side: Side) -> [String; 2] {
        let armies = side.armies();
        if mode == Mode::Hunt {
            let mut pairs = self.missions.iter().filter(|(m, _)| *m == mode);
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
    let find = |game: Game, code: &str| {
        let prefixed = format!("{}_{code}", game.tag());
        bakes
            .iter()
            .find(|(k, g)| *k == prefixed || (k == code && g == game.tag()))
            .map(|(k, _)| k.clone())
    };
    let entry = |game: Game, code: String, name: String, desc: String, modes| {
        let baked = find(game, &code);
        let key = baked
            .clone()
            .unwrap_or_else(|| format!("{}_{code}", game.tag()));
        let missions: Vec<_> = MODES
            .into_iter()
            .filter(|&mode| mode != Mode::Xl || (game == Game::Bf2 && !xl_eras(&code).is_empty()))
            .flat_map(|m| missions_fn(&key, m).into_iter().map(move |p| (m, p)))
            .collect();
        let mut modes: [Vec<Mode>; 2] = modes;
        if game == Game::Bf2 {
            for &era in xl_eras(&code) {
                let armies = era.armies();
                if missions.iter().any(|(mode, pair)| {
                    let pair = pair.each_ref().map(String::as_str);
                    *mode == Mode::Xl && (pair == armies || [pair[1], pair[0]] == armies)
                }) {
                    modes[era.index()].push(Mode::Xl);
                }
            }
        }
        MapEntry {
            label: format!("{name} ({})", game.tag()),
            key,
            game,
            code,
            description: desc,
            modes,
            baked: baked.is_some(),
            missions,
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
                modes[era.index()].push(Mode::Hunt);
            }
        }
        out.push(entry(Game::Bf1, code.to_owned(), name.clone(), name, modes));
    }

    let mut name_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for m in mod_maps {
        *name_counts.entry(m.name.to_ascii_lowercase()).or_insert(0) += 1;
    }
    for m in mod_maps {
        let mut missions = Vec::new();
        for mode in MODES
            .into_iter()
            .filter(|mode| m.modes.iter().any(|era_modes| era_modes.contains(mode)))
        {
            for key in &m.variant_keys {
                for pair in missions_fn(key, mode) {
                    let mission = (mode, pair);
                    if !missions.contains(&mission) {
                        missions.push(mission);
                    }
                }
            }
        }
        let duplicate = name_counts
            .get(&m.name.to_ascii_lowercase())
            .is_some_and(|&c| c > 1);
        let label = if duplicate && !m.disambiguator.is_empty() {
            format!("{} [{}] (mod)", m.name, m.disambiguator)
        } else {
            format!("{} (mod)", m.name)
        };
        let is_baked =
            bakes.is_empty() || bakes.iter().any(|(k, _)| k == &m.key) || !missions.is_empty();
        out.push(MapEntry {
            key: m.key.clone(),
            game: Game::Bf2,
            code: m.legacy_code.clone(),
            label,
            description: m.description.clone(),
            modes: m.modes.clone(),
            baked: is_baked,
            missions,
        });
    }

    out.sort_by_key(|m| m.label.to_lowercase());
    out
}
