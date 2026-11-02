//! The Session Options page: its groups, rows and values.

use super::*;

/// The Session Options groups (§2.1; Host and Elimination are not reachable
/// in single player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Global,
    Hero,
    Conquest,
    Ctf,
    Hunt,
    Assault,
}

pub const GROUPS: [Group; 6] = [
    Group::Global,
    Group::Hero,
    Group::Conquest,
    Group::Ctf,
    Group::Hunt,
    Group::Assault,
];

impl Group {
    /// The overview button and the page title.
    pub(super) fn text(self) -> ((&'static str, &'static str), (&'static str, &'static str)) {
        match self {
            Group::Global => (
                ("ifs.instantoptions.buttons.global", "Global"),
                ("ifs.instantoptions.titles.global", "Global Options"),
            ),
            Group::Hero => (
                ("ifs.instantoptions.buttons.hero", "Hero"),
                ("ifs.instantoptions.titles.hero", "Hero Options"),
            ),
            Group::Conquest => (
                ("ifs.instantoptions.buttons.conquest", "Conquest"),
                ("ifs.instantoptions.titles.conquest", "Conquest Options"),
            ),
            Group::Ctf => (
                ("ifs.instantoptions.buttons.ctf", "CTF"),
                ("ifs.instantoptions.titles.ctf", "CTF Options"),
            ),
            Group::Hunt => (
                ("ifs.instantoptions.buttons.hunt", "Hunt"),
                ("ifs.instantoptions.titles.hunt", "Hunt Options"),
            ),
            Group::Assault => (
                ("ifs.instantoptions.buttons.assault", "Assault"),
                ("ifs.instantoptions.titles.assault", "Assault Options"),
            ),
        }
    }

    /// Whether the port honours the group's options: Global (AI per team,
    /// AI difficulty) and Conquest (AI per team, reinforcements, timer).
    /// Heroes, CTF, Hunt and Assault have no port mode: dimmed (§7); their
    /// values are still the PS2's.
    pub fn honoured(self) -> bool {
        matches!(self, Group::Global | Group::Conquest)
    }
}

/// One option row (`ifs_instant_options_tags`, §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opt {
    NumBots,
    AiDifficulty,
    HeroesOnOff,
    HeroUnlock,
    HeroUnlockVal,
    HeroAssign,
    HeroRespawn,
    DlcHeroes,
    ConNumBots,
    ConMult,
    ConTimer,
    CtfNumBots,
    CtfScore,
    CtfTimer,
    HunTimer,
    HunScore,
    AssNumBots,
    AssScore,
}

impl Opt {
    /// The help line key (`ifs.mp.createopts.helptext.<tag>`) and English.
    pub(super) fn help(self) -> (&'static str, &'static str) {
        match self {
            Opt::NumBots => (
                "ifs.mp.createopts.helptext.numbots",
                "Set the number of AI units to fight on each team for all modes.",
            ),
            Opt::AiDifficulty => (
                "ifs.mp.createopts.helptext.aidifficulty",
                "Determine how tough the AI units will be for all modes.",
            ),
            Opt::HeroesOnOff => (
                "ifs.mp.createopts.helptext.heroes_onoff",
                "Determine whether or not heroes will appear for all maps on your playlist.",
            ),
            Opt::HeroUnlock => (
                "ifs.mp.createopts.helptext.hero_unlock_1",
                "Set the condition that will cause the hero to become available to a player.",
            ),
            Opt::HeroUnlockVal => (
                "ifs.mp.createopts.helptext.hero_unlock_2",
                "Based on the above condition, specify the details that will make the hero available to a player.",
            ),
            Opt::HeroAssign => (
                "ifs.mp.createopts.helptext.hero_assign",
                "Set the condition that will determine which player gets to play as the available hero.",
            ),
            Opt::HeroRespawn => (
                "ifs.mp.createopts.helptext.hero_respawn_val",
                "Determine the amount of time that will elapse before the hero becomes available after being defeated.",
            ),
            Opt::DlcHeroes => (
                "ifs.mp.createopts.helptext.dlc_heroes",
                "Off: each map keeps its own hero. On: Kit Fisto and Asajj Ventress are the Republic and CIS heroes.",
            ),
            Opt::ConNumBots => (
                "ifs.mp.createopts.helptext.con_numbots",
                "Set the number of AI units per team for all Conquest matches in your session.",
            ),
            Opt::ConMult => (
                "ifs.mp.createopts.helptext.con_mult",
                "Adjust the percent of starting reinforcements per team for all Conquest maps on your playlist.",
            ),
            Opt::ConTimer => (
                "ifs.mp.createopts.helptext.con_timer",
                "Set the time limit for all Conquest matches in your session.",
            ),
            Opt::CtfNumBots => (
                "ifs.mp.createopts.helptext.ctf_numbots",
                "Set the number of AI units per team for all 1 and 2-flag CTF matches in your session.",
            ),
            Opt::CtfScore => (
                "ifs.mp.createopts.helptext.ctf_score",
                "Set the number of flags a team must capture to win for all Capture the Flag maps on your playlist.",
            ),
            Opt::CtfTimer => (
                "ifs.mp.createopts.helptext.ctf_timer",
                "Set the time limit for all 1 and 2-flag CTF matches in your session.",
            ),
            Opt::HunTimer => (
                "ifs.mp.createopts.helptext.hun_timer",
                "Set the time limit for all Hunt matches in your session.",
            ),
            Opt::HunScore => (
                "ifs.mp.createopts.helptext.hun_score",
                "Set the score limit for all the Hunt matches in your session.",
            ),
            Opt::AssNumBots => (
                "ifs.mp.createopts.helptext.ass_numbots",
                "Set the number of AI units per team for all space Assault matches in your session.",
            ),
            Opt::AssScore => (
                "ifs.mp.createopts.helptext.ass_score",
                "Set the score limit for all space Assault matches in your session.",
            ),
        }
    }
}

/// The session's option values (§2.3's fields and defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOptions {
    /// Every conquest map/era's defaults (`game::conquest_defaults::TABLE`):
    /// the Team size / Reinforcements help averages over all of them.
    pub population_table: &'static [crate::population::Entry],
    pub num_bots: i32,
    /// The AI tier, 1 Easy, 2 Normal, 3 Hard, 4 Elite (the profile's;
    /// [`DIFFICULTY_LEVELS`]). A named deviation: the PS2's `iDifficulty` is
    /// 1 Easy, 2 Normal, 3 Elite with only 2 <-> 3 reachable (the maintainer,
    /// 2026-10-28: the port's four tiers replace the PS2's three).
    pub difficulty: i32,
    pub heroes: bool,
    /// 3 Points or 5 Timer.
    pub hero_unlock: i32,
    pub hero_unlock_val: i32,
    /// 1 Best, 4 Worst, 7 Random.
    pub hero_assign: i32,
    /// 0 Always, 1..120 s, -1 Never.
    pub hero_respawn: i32,
    /// [ours] The optional DLC heroes' bake is installed, so their row shows.
    pub dlc_heroes_installed: bool,
    /// [ours] Kit Fisto and Asajj Ventress take the Republic's and the CIS's
    /// one hero slot in place of the map's own. Off by default: they are an
    /// option added to the game and no existing hero is lost to them.
    pub dlc_heroes: bool,
    pub con_numbots: i32,
    pub con_mult: i32,
    pub con_timer: i32,
    pub ctf_numbots: i32,
    pub ctf_score: i32,
    pub ctf_timer: i32,
    pub hun_timer: i32,
    pub hun_score: i32,
    pub ass_numbots: i32,
    pub ass_score: i32,
}

impl Default for SessionOptions {
    /// Conquest population is map-relative (maintainer, 2026-10-29): 100%.
    /// Unsupported modes retain the PS2's values (§2.3): 16 AI,
    /// Normal (ours: 2 = Normal, the port's default tier), 100 %, timers Off,
    /// CTF 5, Hunt 5 min / 50, Assault 180; Unlock Points, first to 10
    /// points, Best Player, Timer 90 s (`so_hero`,
    /// `sheets/hero_all_steps.png`). [ours] Heroes On (the PS2's is Off;
    /// maintainer 2026-10-30: "Make heroes on by default in instant
    /// action").
    fn default() -> Self {
        SessionOptions {
            population_table: &[],
            num_bots: 100,
            difficulty: 2,
            heroes: true,
            hero_unlock: 3,
            hero_unlock_val: 10,
            hero_assign: 1,
            hero_respawn: 90,
            dlc_heroes_installed: false,
            dlc_heroes: false,
            con_numbots: 100,
            con_mult: 100,
            con_timer: 0,
            ctf_numbots: 16,
            ctf_score: 5,
            ctf_timer: 0,
            hun_timer: 5,
            hun_score: 50,
            ass_numbots: 16,
            ass_score: 180,
        }
    }
}

impl SessionOptions {
    /// The Team size / Reinforcements help: what an average match over every
    /// map looks like at the current percentages (two lines).
    pub fn population_help(&self, option: Opt) -> Option<String> {
        let team = match option {
            Opt::NumBots => self.num_bots,
            Opt::ConNumBots | Opt::ConMult => self.con_numbots,
            _ => return None,
        } as u32;
        let summary = crate::population::summary(self.population_table, team, self.con_mult as u32);
        let [team_note, ticket_note] = crate::population::notes(summary);
        Some(if option == Opt::ConMult {
            ticket_note
        } else {
            team_note
        })
    }

    /// The group's rows; Heroes Off hides the other hero rows (main.f8).
    pub fn rows(&self, g: Group) -> Vec<Opt> {
        match g {
            Group::Global => vec![Opt::NumBots, Opt::AiDifficulty],
            Group::Hero if !self.heroes => vec![Opt::HeroesOnOff],
            Group::Hero => {
                let mut rows = vec![
                    Opt::HeroesOnOff,
                    Opt::HeroUnlock,
                    Opt::HeroUnlockVal,
                    Opt::HeroAssign,
                    Opt::HeroRespawn,
                ];
                // Beside the switch: a sixth row would start out of sight.
                if self.dlc_heroes_installed {
                    rows.insert(1, Opt::DlcHeroes);
                }
                rows
            }
            Group::Conquest => vec![Opt::ConNumBots, Opt::ConMult, Opt::ConTimer],
            Group::Ctf => vec![Opt::CtfNumBots, Opt::CtfScore, Opt::CtfTimer],
            Group::Hunt => vec![Opt::HunTimer, Opt::HunScore],
            Group::Assault => vec![Opt::AssNumBots, Opt::AssScore],
        }
    }

    /// One press on `o`: `dir` -1 / +1 (Left / Right), `mult` 1 or 10 (L1 /
    /// R1); `Clamp`, no wrap (main.f5, f6). The ranges are the PS2's
    /// (menuref "Session Options" table, `so_*` clips).
    pub fn adjust(&mut self, o: Opt, dir: i32, mult: i32) {
        let d = dir * mult;
        let clamp = |v: i32, lo: i32, hi: i32| v.clamp(lo, hi);
        match o {
            Opt::NumBots => {
                self.num_bots = clamp(
                    self.num_bots + 10 * d,
                    10,
                    crate::population::MAX_TEAM_PCT as i32,
                );
                // The supported mode shares the global population percentage.
                self.con_numbots = self.num_bots;
            }
            // `[ours]` the four tiers, a step per press, clamped like the
            // other rows (L1/R1 go to the ends). The PS2's row toggles
            // `5 - x`, Normal <-> Elite (a named deviation, the maintainer 2026-10-28).
            Opt::AiDifficulty => self.difficulty = clamp(self.difficulty + d, 1, DIFFICULTY_LEVELS),
            Opt::HeroesOnOff => self.heroes = !self.heroes,
            Opt::HeroUnlock => self.hero_unlock = if self.hero_unlock == 3 { 5 } else { 3 },
            Opt::HeroUnlockVal => self.hero_unlock_val = clamp(self.hero_unlock_val + d, 1, 120),
            // Best -> Worst -> Random, no wrap (menuref `so_hero_ranges`).
            Opt::HeroAssign => self.hero_assign = clamp(self.hero_assign + 3 * dir, 1, 7),
            // Always (0) .. 120, Right at 120 -> -1 Never, Right at Never
            // stays, Left from Never -> 120, Left at Always stays (menuref
            // `so_hero_ranges`).
            Opt::HeroRespawn => {
                self.hero_respawn = match (self.hero_respawn, dir > 0) {
                    (-1, true) => -1,
                    (-1, false) => 120,
                    (v, true) if v + d > 120 => -1,
                    (v, _) => (v + d).max(0),
                }
            }
            Opt::DlcHeroes => self.dlc_heroes = !self.dlc_heroes,
            Opt::ConNumBots => {
                self.con_numbots = clamp(
                    self.con_numbots + 10 * d,
                    10,
                    crate::population::MAX_TEAM_PCT as i32,
                );
                self.num_bots = self.con_numbots;
            }
            Opt::ConMult => self.con_mult = clamp(self.con_mult + 10 * d, 10, 500),
            Opt::ConTimer => self.con_timer = clamp(self.con_timer + 5 * d, 0, 60),
            Opt::CtfNumBots => self.ctf_numbots = clamp(self.ctf_numbots + d, 0, MAX_BOTS),
            Opt::CtfScore => self.ctf_score = clamp(self.ctf_score + d, 1, 15),
            Opt::CtfTimer => self.ctf_timer = clamp(self.ctf_timer + 5 * d, 0, 60),
            Opt::HunTimer => self.hun_timer = clamp(self.hun_timer + 5 * d, 5, 60),
            Opt::HunScore => self.hun_score = clamp(self.hun_score + 10 * d, 10, 1000),
            Opt::AssNumBots => self.ass_numbots = clamp(self.ass_numbots + d, 0, MAX_BOTS),
            Opt::AssScore => self.ass_score = clamp(self.ass_score + 10 * d, 50, 1000),
        }
    }
}

/// A piece of an option row: text, or the `{OptionL}` / `{OptionR}` arrows
/// (drawn as COMMON's `optionl` / `optionr`, §2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Left,
    Right,
}

/// `template`'s `%s` filled left to right, `%%` as `%`, cut at the arrow
/// tokens.
pub(super) fn pieces(template: &str, args: &[&str]) -> Vec<Piece> {
    let mut filled = String::new();
    let mut args = args.iter();
    let mut rest = template;
    while let Some(at) = rest.find('%') {
        filled.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(after) = tail.strip_prefix("%s") {
            filled.push_str(args.next().copied().unwrap_or(""));
            rest = after;
        } else if let Some(after) = tail.strip_prefix("%%") {
            filled.push('%');
            rest = after;
        } else {
            filled.push('%');
            rest = &tail[1..];
        }
    }
    filled.push_str(rest);
    let mut out = Vec::new();
    let mut rest = filled.as_str();
    loop {
        let l = rest.find("{OptionL}");
        let r = rest.find("{OptionR}");
        let next = match (l, r) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let Some(at) = next else {
            if !rest.is_empty() {
                out.push(Piece::Text(rest.to_owned()));
            }
            return out;
        };
        if at > 0 {
            out.push(Piece::Text(rest[..at].to_owned()));
        }
        out.push(if Some(at) == l {
            Piece::Left
        } else {
            Piece::Right
        });
        rest = &rest[at + 9..];
    }
}
