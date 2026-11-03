//! Every soldier's numbers for the end of the round's stats pages (issue
//! #50, [`ui::stats`]): read from the sim's events by
//! [`crate::event_feed::EventFeed`] once per tick, and turned into the pages
//! by [`screens`].
//!
//! - **Points, kills** by the feed's rules for the viewer (BF2's table,
//!   `event_feed::points`): an AI kill 1, a human 2, a hero 5, a team kill -2 and one
//!   kill less, killing oneself or dying to the world -1; a post's capture 5
//!   to the soldier longest in its region and 2 to the others there.
//!   Deaths are the sim's.
//! - **Posts**: captures credited (both kinds), the Team Stats `stats_cpp`
//!   column `[D]` (all 0 on the captures; re-round-end.md names it "cpp").
//! - **Personal Stats** (statsref `308_after_K.png`, `300_after_K.png`):
//!   the class and the weapon held longest alive, with the time (the PS2's
//!   "HEAVY TROOPER 1:31"); a rider holds his seat's first gun ("LASER
//!   CANNONS"); Nemesis = who killed him most, Bait = whom he killed most,
//!   with the count.
//! - **Awards** `[H]` by their names (the rules were not read): Tank Buster
//!   most vehicles destroyed, Killing Spree most kills in one life, Traitor
//!   most team kills, Bantha Fodder most deaths (each matches the captures'
//!   winners where their rows are shown: UNIT 466 and LOT KROTAN have the
//!   most deaths of their lists); the first unit of the most wins a tie; none
//!   with 0. Camper (a time), Survivalist, Public Enemy and Dead Eye are not
//!   tracked and stay blank.
//! - Not tracked: head shots (0), the
//!   medals (none in a round, every career count 0 "Green"), the rank
//!   ("Private", the first).

use crate::event_feed::points;
use core::sim::Sim;
use core::unit::UnitHandle;
use std::collections::BTreeMap;
use ui::stats::{clock, Award, Career, Medal, StatRow, StatsScreens, StatsTexts, TeamList};

/// One soldier's round.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnitRound {
    pub points: i32,
    /// Subtotal earned while playing the hero (PS2 stats+0x1fe).
    pub hero_points: i32,
    pub kills: i32,
    pub posts: u32,
    pub team_kills: u32,
    /// Kills in the current life, and the most in one life.
    pub spree: u32,
    pub best_spree: u32,
    pub vehicles: u32,
    /// Ticks alive per class index (`UnitSpec::class`) and per weapon class
    /// (`Sim::weapon_classes` index).
    pub class_ticks: BTreeMap<u8, u32>,
    pub weapon_ticks: BTreeMap<usize, u32>,
    /// Who killed him, and whom he killed: unit index → times.
    pub killed_by: BTreeMap<usize, u32>,
    pub killed: BTreeMap<usize, u32>,
}

impl UnitRound {
    pub fn ordinary_points(&self) -> i32 {
        self.points - self.hero_points
    }

    fn score(&mut self, amount: i32, hero: bool) {
        self.points += amount;
        if hero {
            self.hero_points += amount;
        }
    }
}

/// The round so far, per unit (`Sim::units` order).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoundStats {
    pub units: Vec<UnitRound>,
}

impl RoundStats {
    fn unit(&mut self, i: usize) -> &mut UnitRound {
        if self.units.len() <= i {
            self.units.resize(i + 1, UnitRound::default());
        }
        &mut self.units[i]
    }

    /// `Event::Kill`.
    pub fn kill(&mut self, sim: &Sim, shooter: UnitHandle, victim: UnitHandle) {
        let (s, v) = (shooter.index as usize, victim.index as usize);
        let (Some(k), Some(d)) = (sim.units.get(s), sim.units.get(v)) else {
            return;
        };
        let human = d.is_player;
        let hero = sim.hero_class(k.team) == Some(k.spec.class);
        let same_team = sim.allied(k.team, d.team);
        self.died(v);
        let me = self.unit(s);
        if s == v {
            me.kills -= 1;
            me.score(points::SUICIDE, hero);
            return;
        }
        if same_team {
            me.kills -= 1;
            me.score(points::TEAM_KILL, hero);
            me.team_kills += 1;
        } else {
            me.kills += 1;
            me.score(
                points::kill(human, sim.hero_class(d.team) == Some(d.spec.class)),
                hero,
            );
            me.spree += 1;
            me.best_spree = me.best_spree.max(me.spree);
        }
        *me.killed.entry(v).or_default() += 1;
        *self.unit(v).killed_by.entry(s).or_default() += 1;
    }

    /// `Event::EnvDeath`: the world killed him.
    pub fn env_death(&mut self, sim: &Sim, unit: UnitHandle) {
        let i = unit.index as usize;
        self.died(i);
        let u = &sim.units[i];
        self.unit(i).score(
            points::SUICIDE,
            sim.hero_class(u.team) == Some(u.spec.class),
        );
    }

    fn died(&mut self, i: usize) {
        self.unit(i).spree = 0;
    }

    /// A capture credited to unit `i`: `first` when he was longest in the
    /// region.
    pub fn capture(&mut self, i: usize, first: bool, hero: bool) {
        let u = self.unit(i);
        u.posts += 1;
        u.score(
            if first {
                points::CAPTURE
            } else {
                points::CAPTURE_ASSIST
            },
            hero,
        );
    }

    /// `Event::VehicleDestroyed` by `killer`, of a vehicle of `team`.
    pub fn vehicle(&mut self, sim: &Sim, killer: UnitHandle, team: Option<u8>) {
        let i = killer.index as usize;
        if sim.units.get(i).is_some_and(|k| team != Some(k.team)) {
            self.unit(i).vehicles += 1;
        }
    }

    /// One tick's time for every living soldier: his class, and his weapon
    /// (his seat's first gun when riding one).
    pub fn tick(&mut self, sim: &Sim) {
        // The round's clock stops with the round.
        if sim.outcome.is_some() {
            return;
        }
        for (i, u) in sim.units.iter().enumerate() {
            if !u.alive || u.gen == 0 {
                continue;
            }
            let seat_gun = u.mount.and_then(|m| {
                let v = sim.vehicle(m.vehicle)?;
                let setup = sim.seat_setup.get(v.class as usize)?.as_ref()?;
                let gun = setup.guns.get(usize::from(m.seat))?.as_ref()?;
                Some(gun.weapons.first()?.class)
            });
            let weapon = seat_gun.unwrap_or(u.weapon_class);
            let class = u.spec.class;
            let me = self.unit(i);
            *me.class_ticks.entry(class).or_default() += 1;
            *me.weapon_ticks.entry(weapon).or_default() += 1;
        }
    }
}

/// Ticks as the pages print a time.
fn ticks_clock(ticks: u32) -> String {
    clock(ticks as f32 / core::TICK_HZ as f32)
}

/// The key with the most, the lowest key on a tie.
fn most<K: Copy + Ord, V: Copy + PartialOrd>(m: &BTreeMap<K, V>) -> Option<(K, V)> {
    m.iter().fold(None, |best, (&k, &v)| match best {
        Some((_, b)) if b >= v => best,
        _ => Some((k, v)),
    })
}

/// The names the pages need, by unit.
pub struct Names<'a> {
    /// Unit index → its name ("Player 1", "Unit 538").
    pub unit: &'a dyn Fn(usize) -> String,
    /// (team, class index) → the class's name.
    pub class: &'a dyn Fn(u8, u8) -> String,
    /// Weapon class index → its name.
    pub weapon: &'a dyn Fn(usize) -> String,
    /// A text by its key path, else the English given.
    pub text: &'a dyn Fn(&str, &str) -> String,
    /// The two teams' sides ("rep", "cis", ...).
    pub sides: [&'a str; 2],
}

/// The nine medals in the career page's order (left column, then right),
/// their text keys and the counts that earn one (B2 table 0x62c560, 12 bytes
/// each, read from the ELF: gunslinger 6, frenzy 12, demolition 4,
/// technician 1, marksman 6, regulator 8, endurance 12, guardian 24, war
/// hero 36 [V]).
pub const MEDALS: [(&str, &str, u32); 9] = [
    ("gunslinger", "Gunslinger", 6),
    ("demolition", "Demolition", 4),
    ("regulator", "Regulator", 8),
    ("guardian", "Guardian", 24),
    ("frenzy", "Frenzy", 12),
    ("technician", "Technician", 1),
    ("marksman", "Marksman", 6),
    ("endurance", "Endurance", 12),
    ("warhero", "War Hero", 36),
];

/// A medal count's level (B2 `FUN_0035fc98` [V]): 0-3 Green, 4-31 Veteran,
/// 32-63 Elite, 64 and more Legendary.
pub fn medal_level(count: u32) -> usize {
    match count {
        0..=3 => 0,
        4..=31 => 1,
        32..=63 => 2,
        _ => 3,
    }
}

const LEVELS: [(&str, &str); 4] = [
    ("ifs.stats.medallevel.green", "Green"),
    ("ifs.stats.medallevel.veteran", "Veteran"),
    ("ifs.stats.medallevel.elite", "Elite"),
    ("ifs.stats.medallevel.legendary", "Legendary"),
];

/// The emblems the PS2 heads the lists with whatever the sides.
const EMBLEMS: [&str; 2] = ["cis_icon", "rep_icon"];

/// Team `team`'s emblem: its side's own (`[ours]`, a deviation: the PS2
/// draws CIS left and Republic right on every map, statsref
/// `*/20_teamstats.png`), the PS2's pair for a side without one.
fn emblem(side: &str, team: u8) -> String {
    match side {
        "all" | "imp" | "rep" | "cis" => format!("{side}_icon"),
        _ => EMBLEMS[usize::from(team.min(1))].to_owned(),
    }
}

/// The stats pages of a battle: the round's numbers `round`, the sim's
/// units and deaths, the viewer, the profile's career (this round already
/// in it).
pub fn screens(
    sim: &Sim,
    round: &RoundStats,
    viewer: UnitHandle,
    career: &crate::profile::Career,
    names: &Names<'_>,
) -> StatsScreens {
    let t = names.text;
    let none = UnitRound::default();
    let of = |i: usize| round.units.get(i).unwrap_or(&none);
    let label = |key: &str, en: &str| t(key, en);
    let with_count = |i: usize, n: u32| format!("{} ({n})", (names.unit)(i));
    let lists = [0u8, 1].map(|team| TeamList {
        icon: emblem(names.sides[usize::from(team)], team),
        rows: sim
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.team == team && u.gen > 0)
            .map(|(i, u)| {
                let r = of(i);
                let favourite_class = most(&r.class_ticks)
                    .map(|(c, t)| format!("{} {}", (names.class)(team, c), ticks_clock(t)))
                    .unwrap_or_default();
                let favourite_weapon = most(&r.weapon_ticks)
                    .map(|(w, t)| format!("{} {}", (names.weapon)(w), ticks_clock(t)))
                    .unwrap_or_default();
                let nemesis = most(&r.killed_by)
                    .map(|(k, n)| with_count(k, n))
                    .unwrap_or_default();
                let bait = most(&r.killed)
                    .map(|(k, n)| with_count(k, n))
                    .unwrap_or_default();
                StatRow {
                    name: (names.unit)(i),
                    points: r.points,
                    kills: r.kills,
                    deaths: u.deaths as i32,
                    posts: r.posts as i32,
                    me: i == viewer.index as usize,
                    personal: vec![
                        (
                            label("ifs.stats.points", "Player Points"),
                            r.points.to_string(),
                        ),
                        (
                            label("ifs.stats.hero_points", "Hero Points"),
                            r.hero_points.to_string(),
                        ),
                        (label("ifs.stats.kills", "Kills"), r.kills.to_string()),
                        (label("ifs.stats.deaths", "Deaths"), u.deaths.to_string()),
                        (String::new(), String::new()),
                        (
                            label("ifs.stats.favclass", "Favorite Class"),
                            favourite_class,
                        ),
                        (
                            label("ifs.stats.favweapon", "Favorite Weapon"),
                            favourite_weapon,
                        ),
                        (label("ifs.stats.headshots", "Head Shots"), "0".into()),
                        (label("ifs.stats.nemesis", "Nemesis"), nemesis),
                        (label("ifs.stats.bait", "Bait"), bait),
                    ],
                }
            })
            .collect(),
    });
    // An award's winner: the unit with the most of `pick`, none at 0.
    let award = |key: &str, en: &str, pick: &dyn Fn(usize) -> u32| {
        let best = (0..sim.units.len())
            .filter(|&i| sim.units[i].gen > 0)
            .map(|i| (i, pick(i)))
            .fold(None, |b: Option<(usize, u32)>, (i, n)| match b {
                Some((_, m)) if m >= n => b,
                _ => Some((i, n)),
            })
            .filter(|(_, n)| *n > 0);
        Award {
            label: label(key, en),
            winner: best.map(|(i, n)| with_count(i, n)).unwrap_or_default(),
        }
    };
    let blank = |key: &str, en: &str| Award {
        label: label(key, en),
        winner: String::new(),
    };
    let awards = [
        vec![
            award("ifs.stats.tankbuster", "Tank Buster", &|i| of(i).vehicles),
            award("ifs.stats.killingspree", "Killing Spree", &|i| {
                of(i).best_spree
            }),
            blank("ifs.stats.camper", "Camper"),
            blank("ifs.stats.survivalist", "Survivalist"),
        ],
        vec![
            award("ifs.stats.traitor", "Traitor", &|i| of(i).team_kills),
            blank("ifs.stats.publicenemy", "Public Enemy"),
            Award {
                // Bantha Fodder's key path is not known: its text by hash.
                label: t("#1f913713", "Bantha Fodder"),
                ..award("", "", &|i| sim.units[i].deaths)
            },
            blank("ifs.stats.deadeye", "Dead Eye"),
        ],
    ];
    let lives = career.deaths.max(1);
    let level = t(LEVELS[medal_level(0)].0, LEVELS[medal_level(0)].1);
    let career = Career {
        values: vec![
            (
                label("ifs.stats.points", "Player Points"),
                career.points.to_string(),
            ),
            (
                label("ifs.stats.rank", "Rank"),
                t("ifs.stats.playertitle.private", "Private"),
            ),
            (label("ifs.stats.kills", "Kills"), career.kills.to_string()),
            (
                label("ifs.stats.killdeathratio", "Kill/Death Ratio"),
                format!("{:.2}", career.kills as f32 / lives as f32),
            ),
            (
                label("ifs.stats.deaths", "Deaths"),
                career.deaths.to_string(),
            ),
            (
                label("ifs.stats.ptsperlife", "Points Per Life"),
                (career.points / i64::from(lives)).to_string(),
            ),
        ],
        medals: MEDALS
            .iter()
            .map(|(key, en, needed)| Medal {
                name: t(&format!("game.awards.medal.{key}"), en),
                count: 0,
                level: level.clone(),
                info: t(&format!("game.awards.medalinfo.{key}"), "%s")
                    .replacen("%s", &needed.to_string(), 1)
                    .replace("\\'", "'"),
            })
            .collect(),
        unlocks: crate::unlocks::rows(career, t),
    };
    let texts = StatsTexts {
        team_title: t("ifs.stats.teamstatstitle", "Team Stats"),
        awards_title: t("ifs.stats.awards", "Awards"),
        personal_title: t("ifs.stats.personalstatstitle", "Personal Stats"),
        medals_title: t("ifs.stats.medals", "Medals"),
        career_title: t("ifs.stats.careerstatstitle", "Career Stats"),
        career_medals: t("ifs.stats.CareerMedals", "Career Medals"),
        // `[ours]` (card UNLK3): BF2's table has no such heading.
        career_unlocks: "Career Unlocks".into(),
        done: t("ifs.stats.done", "Done"),
        select: t("common.select", "Select"),
        back: t("ifs.stats.back", "Back"),
    };
    StatsScreens::new(texts, lists, awards, career)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walk::WalkSim;
    use core::unit::{Controller, UnitSpec};

    /// The player (team 0) and three bots: one beside him, two against.
    fn walk() -> WalkSim {
        let mut w = WalkSim::new(7, glam::Vec3::ZERO, None);
        for (team, x) in [(0, 5.0), (1, 40.0), (1, 42.0)] {
            w.sim
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(team, glam::Vec3::new(x, 0.0, 0.0))
                })
                .expect("bot");
        }
        w
    }

    fn names_for<'a>(
        unit: &'a dyn Fn(usize) -> String,
        text: &'a dyn Fn(&str, &str) -> String,
    ) -> Names<'a> {
        Names {
            unit,
            class: &|_, c| format!("C{c}"),
            weapon: &|w| format!("W{w}"),
            text,
            sides: ["imp", "all"],
        }
    }

    /// Kills, team kills, a suicide and a capture score as the viewer's feed
    /// scores them; Nemesis and Bait name the other with the count; the
    /// awards name the most; a tie goes to the first unit.
    #[test]
    fn the_round_scores_every_soldier() {
        let w = walk();
        let sim = &w.sim;
        let team = |t: u8| {
            (0..sim.units.len())
                .filter(|&i| sim.units[i].team == t)
                .map(|i| sim.handle(i))
                .collect::<Vec<_>>()
        };
        let (a, b) = (team(0), team(1));
        assert!(a.len() >= 2 && b.len() >= 2);
        let mut r = RoundStats::default();
        r.kill(sim, a[0], b[0]);
        r.kill(sim, a[0], b[0]);
        r.kill(sim, a[0], b[1]);
        r.kill(sim, b[1], a[1]);
        r.kill(sim, a[1], a[0]);
        r.kill(sim, b[0], b[0]);
        r.env_death(sim, b[1]);
        r.capture(b[1].index as usize, true, false);
        let ru = |h: UnitHandle| r.units[h.index as usize].clone();
        let human = |h: UnitHandle| sim.units[h.index as usize].is_player;
        let kill_pts = |h: UnitHandle| {
            if human(h) {
                points::KILL_HUMAN
            } else {
                points::KILL_AI
            }
        };
        assert_eq!(ru(a[0]).kills, 3);
        assert_eq!(ru(a[0]).points, 2 * kill_pts(b[0]) + kill_pts(b[1]));
        assert_eq!(ru(a[0]).best_spree, 3);
        assert_eq!(ru(a[0]).spree, 0, "a team mate killed him");
        assert_eq!(
            (ru(a[1]).kills, ru(a[1]).points, ru(a[1]).team_kills),
            (-1, -2, 1)
        );
        assert_eq!((ru(b[0]).kills, ru(b[0]).points), (-1, -1));
        assert_eq!(ru(b[1]).points, kill_pts(a[1]) - 1 + points::CAPTURE);
        assert_eq!(ru(b[1]).posts, 1);
        assert_eq!(most(&ru(b[0]).killed_by), Some((a[0].index as usize, 2)));
        assert_eq!(most(&ru(a[0]).killed), Some((b[0].index as usize, 2)));

        let unit = |i: usize| format!("U{i}");
        let text = |_: &str, en: &str| en.to_owned();
        let career = crate::profile::Career {
            kills: 3,
            deaths: 2,
            points: 7,
            ..Default::default()
        };
        let s = screens(sim, &r, w.player, &career, &names_for(&unit, &text));
        assert_eq!(
            [&s.lists[0].icon, &s.lists[1].icon],
            ["imp_icon", "all_icon"],
            "each team its own side's emblem"
        );
        let spree = &s.awards[0][1];
        assert_eq!(spree.winner, format!("U{} (3)", a[0].index));
        assert_eq!(s.awards[1][0].winner, format!("U{} (1)", a[1].index));
        assert_eq!(s.awards[0][0].winner, "", "no vehicles: blank");
        assert!(
            s.selected().is_some_and(|r| r.me),
            "the cursor on the viewer"
        );
        let row = s.lists[0]
            .rows
            .iter()
            .find(|r| r.name == format!("U{}", a[0].index))
            .unwrap();
        assert_eq!(row.personal[0].1, row.points.to_string());
        assert_eq!(row.personal[9].1, format!("U{} (2)", b[0].index), "Bait");
        assert_eq!(s.career.values[3].1, "1.50");
        assert_eq!(s.career.values[5].1, "3");
        assert_eq!(s.career.medals[0].info, "6");
        assert_eq!(s.career.medals[0].level, "Green");
    }

    /// Time alive goes to the class and the weapon held, and prints as the
    /// favourite with m:ss.
    #[test]
    fn time_alive_makes_the_favourites() {
        let w = walk();
        let mut r = RoundStats::default();
        for _ in 0..(core::TICK_HZ as usize * 36) {
            r.tick(&w.sim);
        }
        let me = w.player.index as usize;
        let ticks = *r.units[me].class_ticks.values().next().unwrap();
        assert_eq!(ticks, core::TICK_HZ * 36);
        let unit = |i: usize| format!("U{i}");
        let text = |_: &str, en: &str| en.to_owned();
        let s = screens(
            &w.sim,
            &r,
            w.player,
            &Default::default(),
            &names_for(&unit, &text),
        );
        let row = s.selected().unwrap();
        assert!(
            row.personal[5].1.ends_with(" 0:36"),
            "{:?}",
            row.personal[5]
        );
        assert!(row.personal[6].1.starts_with('W'), "{:?}", row.personal[6]);
    }

    #[test]
    fn medal_levels_follow_the_counts() {
        assert_eq!(
            [0, 3, 4, 31, 32, 63, 64].map(medal_level),
            [0, 0, 1, 1, 2, 2, 3]
        );
    }
}
