//! The screen's localized text.

use super::*;

/// Every text the screens show, looked up once.
#[derive(Debug, Clone, PartialEq)]
pub struct IaTexts {
    pub planet: String,
    pub mode: String,
    pub era: String,
    pub playlist: String,
    pub remove_all: String,
    pub remove_all_desc: String,
    pub all_maps: String,
    pub all_maps_desc: String,
    pub all_modes: String,
    pub all_modes_desc: String,
    pub all_eras: String,
    pub list_full: String,
    pub back: String,
    pub select: String,
    pub session_options: String,
    pub(super) commands: Vec<(String, String)>,
    pub(super) modes: Vec<(String, String)>,
    pub(super) eras: Vec<String>,
    pub(super) groups: Vec<(String, String)>,
    /// `common.sides.<f>.name` for all, imp, rep, cis.
    pub(super) sides: Vec<(&'static str, String)>,
    pub(super) helps: Vec<(Opt, String)>,
    pub(super) t: Vec<(&'static str, String)>,
}

/// The option rows' text keys and English.
pub(super) const OPTION_TEXTS: [(&str, &str); 19] = [
    (
        "ifs.mp.createopts.numbots",
        "AI Units Per Team: {OptionL} %s {OptionR}",
    ),
    (
        "ifs.mp.createopts.difficulty",
        "AI Difficulty Level: {OptionL} %s {OptionR}",
    ),
    ("ifs.mp.createopts.heroes", "Heroes: {OptionL} %s {OptionR}"),
    (
        "ifs.mp.heroopts.herounlock3",
        "Unlock Hero: {OptionL} Points {OptionR}",
    ),
    (
        "ifs.mp.heroopts.herounlock5",
        "Unlock Hero: {OptionL} Timer {OptionR}",
    ),
    (
        "ifs.mp.heroopts.numpoints",
        "First to {OptionL} %s {OptionR} points",
    ),
    (
        "ifs.mp.heroopts.numseconds",
        "{OptionL} %s {OptionR} seconds",
    ),
    (
        "ifs.mp.heroopts.heroplayer1",
        "Assign To: {OptionL} Best Player {OptionR}",
    ),
    (
        "ifs.mp.heroopts.heroplayer4",
        "Assign To: {OptionL} Worst Player {OptionR}",
    ),
    (
        "ifs.mp.heroopts.heroplayer7",
        "Assign To: {OptionL} Random Player {OptionR}",
    ),
    (
        "ifs.mp.heroopts.respawntimer",
        "Timer: {OptionL} %s {OptionR}%s",
    ),
    (
        "ifs.mp.heroopts.dlcheroes",
        "DLC Heroes: {OptionL} %s {OptionR}",
    ),
    (
        "ifs.mp.create.opt.reinforcements",
        "Reinforcements: {OptionL} %s%% {OptionR}",
    ),
    ("ifs.mp.create.opt.timer", "Timer: {OptionL} %s {OptionR}"),
    ("ifs.mp.create.opt.ctf", "CTF Score: {OptionL} %s {OptionR}"),
    (
        "ifs.mp.create.opt.scorelimit",
        "Score Limit: {OptionL} %s {OptionR}",
    ),
    ("ifs.mp.create.opt.off", "Off"),
    ("ifs.mp.heroopts.seconds", " seconds"),
    ("ifs.mp.heroopts.second", " second"),
];

/// Short texts: (key, English).
pub(super) const SMALL_TEXTS: [(&str, &str); 8] = [
    ("common.on", "On"),
    ("common.off", "Off"),
    ("common.always", "Always"),
    ("common.never", "Never"),
    ("ifs.difficulty.easy", "Easy"),
    ("ifs.difficulty.medium", "Normal"),
    ("ifs.difficulty.hard", "Elite"),
    ("common.mp.options", "Options"),
];

/// The AI Difficulty row's levels, 1..=this: Easy, Normal, Hard, Elite
/// ([`SessionOptions::difficulty`]).
pub const DIFFICULTY_LEVELS: i32 = 4;

impl IaTexts {
    pub fn new(text: &dyn Fn(&str, &str) -> String) -> IaTexts {
        let t = |k: (&str, &str)| text(k.0, k.1);
        IaTexts {
            planet: t(("ifs.missionselect.selectplanet", "Planet")),
            mode: t(("ifs.missionselect.selectmode", "Mode")),
            era: t(("ifs.missionselect.selectera", "Era")),
            playlist: t(("ifs.missionselect.playlist", "Playlist")),
            remove_all: t(("mapname.name._remove_all_", "Remove All")),
            remove_all_desc: t((
                "mapname.description._remove_all_",
                "Remove all maps from the playlist.",
            )),
            all_maps: t(("mapname.name._all_maps_", "All Maps")),
            all_maps_desc: t((
                "mapname.description._all_maps_",
                "Add all maps to the playlist.",
            )),
            all_modes: t(("modename.name._all_modes_", "All Modes")),
            all_modes_desc: t((
                "modename.description._all_modes_",
                "Add all available game modes for this map.",
            )),
            all_eras: t(("common.era._all_eras_", "All Eras")),
            list_full: t(("ifs.missionselect.listfull", "Selected maplist is full.")),
            back: t(("common.back", "Back")),
            select: t(("common.select", "Select")),
            session_options: t(("ifs.instantoptions.title", "Session Options")),
            commands: COMMANDS
                .iter()
                .map(|c| {
                    let (a, b) = c.text();
                    (t(a), t(b))
                })
                .collect(),
            modes: MODES
                .iter()
                .map(|m| {
                    let (a, b) = m.text();
                    (t(a), t(b))
                })
                .collect(),
            eras: ERAS.iter().map(|e| t(e.text())).collect(),
            groups: GROUPS
                .iter()
                .map(|g| {
                    let (a, b) = g.text();
                    (t(a), t(b))
                })
                .collect(),
            sides: [
                ("all", "Rebels"),
                ("imp", "Empire"),
                ("rep", "Republic"),
                ("cis", "CIS"),
            ]
            .map(|(f, e)| (f, text(&format!("common.sides.{f}.name"), e)))
            .to_vec(),
            helps: [
                Opt::NumBots,
                Opt::AiDifficulty,
                Opt::HeroesOnOff,
                Opt::HeroUnlock,
                Opt::HeroUnlockVal,
                Opt::HeroAssign,
                Opt::HeroRespawn,
                Opt::DlcHeroes,
                Opt::ConNumBots,
                Opt::ConMult,
                Opt::ConTimer,
                Opt::CtfNumBots,
                Opt::CtfScore,
                Opt::CtfTimer,
                Opt::HunTimer,
                Opt::HunScore,
                Opt::AssNumBots,
                Opt::AssScore,
            ]
            .map(|o| (o, t(o.help())))
            .to_vec(),
            t: OPTION_TEXTS
                .iter()
                .chain(SMALL_TEXTS.iter())
                .map(|&(k, e)| {
                    let localized = text(k, e);
                    // PC builds these controls beside label-only Locl strings.
                    // Our console-shaped shell needs the complete row template,
                    // independently of which row currently has focus.
                    let row = if e.contains("{OptionL}")
                        && (!localized.contains("{OptionL}") || !localized.contains("{OptionR}"))
                    {
                        e.to_owned()
                    } else {
                        localized
                    };
                    (k, row)
                })
                .collect(),
        }
    }

    pub(super) fn get(&self, key: &str) -> &str {
        self.t
            .iter()
            .find(|(k, _)| *k == key)
            .map_or("", |(_, v)| v.as_str())
    }

    pub fn command(&self, c: Cmd) -> (&str, &str) {
        let i = COMMANDS.iter().position(|x| *x == c).unwrap_or(0);
        let (a, b) = &self.commands[i];
        (a, b)
    }

    pub fn mode(&self, m: Mode) -> (&str, &str) {
        let i = MODES.iter().position(|x| *x == m).unwrap_or(0);
        let (a, b) = &self.modes[i];
        (a, b)
    }

    pub fn era(&self, e: Era) -> &str {
        &self.eras[e.index()]
    }

    pub fn group(&self, g: Group) -> (&str, &str) {
        let i = GROUPS.iter().position(|x| *x == g).unwrap_or(0);
        let (a, b) = &self.groups[i];
        (a, b)
    }

    pub(super) fn side_name<'a>(&'a self, f: &'a str) -> &'a str {
        self.sides
            .iter()
            .find(|(k, _)| *k == f)
            .map_or(f, |(_, v)| v.as_str())
    }

    /// An Era row's text; `[ours]` a cross-era pair reads "<A> vs <B>".
    pub fn side(&self, s: Side) -> String {
        match s {
            Side::Era(e) => self.era(e).to_owned(),
            Side::Mixed(i) => format!(
                "{} vs {}",
                self.side_name(MIXED[i][0]),
                self.side_name(MIXED[i][1])
            ),
        }
    }

    pub fn help(&self, o: Opt) -> &str {
        self.helps
            .iter()
            .find(|(k, _)| *k == o)
            .map_or("", |(_, v)| v.as_str())
    }

    /// An option row as drawn: "Label: < value >" (main.f2).
    pub fn option(&self, o: Opt, v: &SessionOptions) -> Vec<Piece> {
        let n = |x: i32| x.to_string();
        let off = |x: i32| {
            if x == 0 {
                self.get("ifs.mp.create.opt.off").to_owned()
            } else {
                n(x)
            }
        };
        let num = self.get("ifs.mp.createopts.numbots");
        match o {
            Opt::NumBots | Opt::ConNumBots => {
                let pct = if o == Opt::NumBots {
                    v.num_bots
                } else {
                    v.con_numbots
                } as u32;
                vec![
                    Piece::Text("Team size: ".into()),
                    Piece::Left,
                    Piece::Text(format!(" {pct}% ")),
                    Piece::Right,
                ]
            }
            Opt::CtfNumBots => pieces(num, &[&n(v.ctf_numbots)]),
            Opt::AssNumBots => pieces(num, &[&n(v.ass_numbots)]),
            Opt::AiDifficulty => {
                // BF2's words where it has them (`ifs.difficulty.easy`
                // "Easy", `.medium` "Normal", `.hard` "Elite"); `[ours]` Hard,
                // English (BF2 has no "Hard": its Hard level shows "Elite").
                let name = match v.difficulty {
                    1 => self.get("ifs.difficulty.easy"),
                    2 => self.get("ifs.difficulty.medium"),
                    3 => "Hard",
                    _ => self.get("ifs.difficulty.hard"),
                };
                pieces(self.get("ifs.mp.createopts.difficulty"), &[name])
            }
            Opt::HeroesOnOff => pieces(
                self.get("ifs.mp.createopts.heroes"),
                &[self.get(if v.heroes { "common.on" } else { "common.off" })],
            ),
            Opt::HeroUnlock => pieces(
                self.get(if v.hero_unlock == 5 {
                    "ifs.mp.heroopts.herounlock5"
                } else {
                    "ifs.mp.heroopts.herounlock3"
                }),
                &[],
            ),
            Opt::HeroUnlockVal => pieces(
                self.get(if v.hero_unlock == 5 {
                    "ifs.mp.heroopts.numseconds"
                } else {
                    "ifs.mp.heroopts.numpoints"
                }),
                &[&n(v.hero_unlock_val)],
            ),
            Opt::HeroAssign => pieces(
                self.get(match v.hero_assign {
                    4 => "ifs.mp.heroopts.heroplayer4",
                    7 => "ifs.mp.heroopts.heroplayer7",
                    _ => "ifs.mp.heroopts.heroplayer1",
                }),
                &[],
            ),
            Opt::HeroRespawn => {
                let (value, unit) = match v.hero_respawn {
                    0 => (self.get("common.always").to_owned(), ""),
                    -1 => (self.get("common.never").to_owned(), ""),
                    1 => ("1".to_owned(), self.get("ifs.mp.heroopts.second")),
                    s => (n(s), self.get("ifs.mp.heroopts.seconds")),
                };
                pieces(self.get("ifs.mp.heroopts.respawntimer"), &[&value, unit])
            }
            Opt::DlcHeroes => pieces(
                self.get("ifs.mp.heroopts.dlcheroes"),
                &[self.get(if v.dlc_heroes {
                    "common.on"
                } else {
                    "common.off"
                })],
            ),
            Opt::ConMult => {
                vec![
                    Piece::Text("Reinforcements: ".into()),
                    Piece::Left,
                    Piece::Text(format!(" {}% ", v.con_mult)),
                    Piece::Right,
                ]
            }
            Opt::ConTimer => pieces(self.get("ifs.mp.create.opt.timer"), &[&off(v.con_timer)]),
            Opt::CtfTimer => pieces(self.get("ifs.mp.create.opt.timer"), &[&off(v.ctf_timer)]),
            Opt::HunTimer => pieces(self.get("ifs.mp.create.opt.timer"), &[&n(v.hun_timer)]),
            Opt::CtfScore => pieces(self.get("ifs.mp.create.opt.ctf"), &[&n(v.ctf_score)]),
            Opt::HunScore => pieces(self.get("ifs.mp.create.opt.scorelimit"), &[&n(v.hun_score)]),
            Opt::AssScore => pieces(self.get("ifs.mp.create.opt.scorelimit"), &[&n(v.ass_score)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pc_label_only_strings_still_build_complete_console_controls() {
        let texts = IaTexts::new(&|key, english| match key {
            "ifs.mp.createopts.difficulty" => "AI Difficulty Level:".into(),
            "ifs.mp.create.opt.timer" => "Timer:".into(),
            _ => english.to_owned(),
        });
        let options = SessionOptions::default();
        for (opt, expected) in [(Opt::AiDifficulty, " Normal "), (Opt::ConTimer, " Off ")] {
            let row = texts.option(opt, &options);
            assert!(row.contains(&Piece::Left));
            assert!(row.contains(&Piece::Right));
            assert!(row.contains(&Piece::Text(expected.into())), "{row:?}");
        }
        // Complete console templates and their translated value are retained.
        let console = IaTexts::new(&|key, english| {
            if key == "ifs.difficulty.medium" {
                "Translated".into()
            } else {
                english.into()
            }
        });
        assert!(console
            .option(Opt::AiDifficulty, &options)
            .contains(&Piece::Text(" Translated ".into())));
    }
}
