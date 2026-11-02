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
    /// The bake's directory (`bf2_hot1`, the legacy `tat2`), else
    /// `<game>_<code>` for a map that is not baked.
    pub key: String,
    pub game: Game,
    pub code: &'static str,
    /// "<name> (bf1)" / "<name> (bf2)" (§7).
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
        let open = PORT_MODES.contains(&mode) || (mode == Mode::Assault && space_assault_open());
        if !self.baked || !open {
            return false;
        }
        let mut pairs = self.missions.iter().filter(|(m, _)| *m == mode);
        match side {
            Side::Era(era) => {
                self.modes[era.index()].contains(&mode)
                    && pairs.any(|(_, p)| {
                        let a = era.armies();
                        (p[0] == a[0] && p[1] == a[1]) || (p[0] == a[1] && p[1] == a[0])
                    })
            }
            Side::Mixed(_) => pairs.next().is_some(),
        }
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
    let find = |game: Game, code: &str| {
        let prefixed = format!("{}_{code}", game.tag());
        bakes
            .iter()
            .find(|(k, g)| *k == prefixed || (k == code && g == game.tag()))
            .map(|(k, _)| k.clone())
    };
    let entry = |game: Game, code: &'static str, name: String, desc: String, modes| {
        let baked = find(game, code);
        let key = baked
            .clone()
            .unwrap_or_else(|| format!("{}_{code}", game.tag()));
        let missions = MODES
            .into_iter()
            .flat_map(|m| missions(&key, m).into_iter().map(move |p| (m, p)))
            .collect();
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
            .map_or_else(|| code.to_uppercase(), |t| t.trim().to_owned());
        let desc = text(Game::Bf2, &format!("mapname.description.{code}"))
            .map_or_else(|| name.clone(), |t| t.trim_end().to_owned());
        out.push(entry(
            Game::Bf2,
            code,
            name,
            desc,
            [cw.to_vec(), gcw.to_vec()],
        ));
    }
    for (code, key, english, cw, gcw) in BF1_MAPS {
        let name = text(Game::Bf1, key).map_or_else(|| english.to_owned(), |t| t.trim().to_owned());
        let modes = [cw, gcw].map(|on| if on { vec![Mode::Con] } else { Vec::new() });
        out.push(entry(Game::Bf1, code, name.clone(), name, modes));
    }
    out.sort_by_key(|m| m.label.to_lowercase());
    out
}
