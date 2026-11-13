//! The unlockable classes (`[ours]`, cards UNLK1-UNLK3): BF2's eight classes
//! with a `PointsToUnlock` (two a side: the officer at 8 points, the special
//! at 12, `analysis/unit-audit-bf2.md` §3.8), each with one career counter:
//! the player's kills as any of its classes, BF1's counterparts included
//! (`Career::class_kills`, card UNLK2).
//!
//! - BF1 has no unlocks; its four specials (Wookiee smuggler, dark trooper,
//!   jet trooper, droideka) take their BF2 counterpart's cost here
//!   ([`bf1_cost`]). BF1 has no officers.
//! - The counter earns the class down ([`effective_cost`]): under 50 kills
//!   the full cost, 50 half, 100 none - unlocked for good.

use crate::profile::Career;
use std::collections::BTreeSet;

/// One unlockable class and its counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unlockable {
    /// The counter's name (side and class), stable across maps and games.
    pub key: &'static str,
    /// The class whose `entity.<side>.<rest>` text names it on BF2's table.
    pub name_class: &'static str,
    /// The table's English, for a bake without the table.
    pub english: &'static str,
    /// BF2's `PointsToUnlock` (8 officer, 12 special).
    pub cost: u32,
    /// The BF2 classes that resolve to that cost (map variants).
    pub bf2: &'static [&'static str],
    /// The BF1 classes counted with it and locked at its cost.
    pub bf1: &'static [&'static str],
}

/// The eight, a side's officer then its special (Rebels, Empire, Republic,
/// CIS). The BF2 lists are every class of the baked BF2 class files whose
/// resolved `PointsToUnlock` is above 0 (`_hunt` and training variants
/// author 0 and stay out).
pub const UNLOCKABLES: [Unlockable; 8] = [
    Unlockable {
        key: "all_officer",
        name_class: "all_inf_officer",
        english: "Bothan Spy",
        cost: 8,
        bf2: &[
            "all_inf_officer",
            "all_inf_officer_jungle",
            "all_inf_officer_snow",
        ],
        bf1: &[],
    },
    Unlockable {
        key: "all_wookiee",
        name_class: "all_inf_wookiee",
        english: "Wookiee Warrior",
        cost: 12,
        bf2: &["all_inf_wookiee", "all_inf_wookiee_snow"],
        bf1: &["all_inf_smuggler", "all_inf_smugglersnow"],
    },
    Unlockable {
        key: "imp_officer",
        name_class: "imp_inf_officer",
        english: "Imperial Officer",
        cost: 8,
        bf2: &[
            "imp_inf_officer",
            "imp_inf_officer_gray",
            "imp_inf_officer_snow",
        ],
        bf1: &[],
    },
    Unlockable {
        key: "imp_dark_trooper",
        name_class: "imp_inf_dark_trooper",
        english: "Dark Trooper",
        cost: 12,
        bf2: &["imp_inf_dark_trooper"],
        bf1: &["imp_inf_dark_trooper", "imp_inf_dark_troopersnow"],
    },
    Unlockable {
        key: "rep_officer",
        name_class: "rep_inf_ep3_officer",
        english: "Clone Commander",
        cost: 8,
        bf2: &["rep_inf_ep3_officer"],
        bf1: &[],
    },
    Unlockable {
        key: "rep_jet_trooper",
        name_class: "rep_inf_ep3_jettrooper",
        english: "Jet Trooper",
        cost: 12,
        bf2: &[
            "rep_inf_ep3_jettrooper",
            "rep_inf_ep2_jettrooper",
            "rep_inf_ep2_jettrooper_rifleman",
            "rep_inf_ep2_jettrooper_sniper",
        ],
        bf1: &["rep_inf_jet_trooper"],
    },
    Unlockable {
        key: "cis_officer",
        name_class: "cis_inf_officer",
        english: "MagnaGuard",
        cost: 8,
        bf2: &["cis_inf_officer"],
        bf1: &[],
    },
    Unlockable {
        key: "cis_droideka",
        name_class: "cis_inf_droideka",
        english: "Droideka",
        cost: 12,
        bf2: &["cis_inf_droideka"],
        bf1: &["cis_inf_droideka"],
    },
];

/// Career kills that halve a class's cost, and that unlock it for good.
pub const HALF_AT: u32 = 50;
pub const FREE_AT: u32 = 100;

impl Unlockable {
    /// Every class name counted (BF2's and BF1's, once each).
    pub fn classes(&self) -> BTreeSet<&'static str> {
        self.bf2.iter().chain(self.bf1).copied().collect()
    }

    /// The counter: the career's kills as any of its classes.
    pub fn kills(&self, career: &Career) -> u32 {
        self.classes()
            .into_iter()
            .filter_map(|c| career.class_kills.get(c))
            .fold(0u32, |n, k| n.saturating_add(*k))
    }

    /// The kills the next stage needs (25, then 100); `None` once unlocked
    /// for good.
    pub fn next_stage(kills: u32) -> Option<u32> {
        match kills {
            k if k < HALF_AT => Some(HALF_AT),
            k if k < FREE_AT => Some(FREE_AT),
            _ => None,
        }
    }
}

/// The counter a class counts toward, if any (BF2 or BF1 name).
pub fn of_class(class: &str) -> Option<&'static Unlockable> {
    UNLOCKABLES
        .iter()
        .find(|u| u.bf2.contains(&class) || u.bf1.contains(&class))
}

/// `[ours]` (card UNLK1): a BF1 class's cost, its BF2 counterpart's; `None`
/// for a class BF1 does not lock.
pub fn bf1_cost(class: &str) -> Option<u32> {
    UNLOCKABLES
        .iter()
        .find(|u| u.bf1.contains(&class))
        .map(|u| u.cost)
}

/// `[ours]` (card UNLK3): a cost as career kills earn it down: the full
/// cost under 25 kills, half (rounded up) from 25, none from 100.
pub fn effective_cost(cost: u32, kills: u32) -> u32 {
    match kills {
        k if k >= FREE_AT => 0,
        k if k >= HALF_AT => cost.div_ceil(2),
        _ => cost,
    }
}

/// A class's cost for the player with `career`: `authored` (the class's
/// `unlock_points`) earned down by its counter; a class with no counter
/// keeps `authored`.
pub fn class_cost(class: &str, authored: u32, career: &Career) -> u32 {
    match of_class(class) {
        Some(u) => effective_cost(authored, u.kills(career)),
        None => authored,
    }
}

/// The Career Stats page's unlocks (`ui::stats::Career::unlocks`): each
/// counter as a medal row - its name from BF2's table by `text` (key path,
/// English), its career kills, its cost now ("12 PTS", "UNLOCKED") where a
/// medal has its level, and a line on the next stage. `[ours]`: the PS2 has
/// none; the stage texts are English.
pub fn rows(career: &Career, text: &dyn Fn(&str, &str) -> String) -> Vec<ui::stats::Medal> {
    UNLOCKABLES
        .iter()
        .map(|u| {
            let (side, rest) = u.name_class.split_once('_').unwrap_or_default();
            let name = text(&format!("entity.{side}.{rest}"), u.english);
            let kills = u.kills(career);
            let cost = effective_cost(u.cost, kills);
            let level = match cost {
                0 => "UNLOCKED".to_owned(),
                _ => format!("{cost} PTS"),
            };
            // Worded like BF2's award lines ("%s kills with a pistol in one
            // life equips you with a Precision Pistol.", "You have unlocked %s").
            let info = match Unlockable::next_stage(kills) {
                Some(HALF_AT) => format!(
                    "{HALF_AT} career kills with the {name} halves its unlock cost. \
                     {FREE_AT} career kills to unlock it permanently."
                ),
                Some(_) => {
                    format!("{FREE_AT} career kills to unlock it permanently.")
                }
                None => format!("You have unlocked the {name} permanently."),
            };
            ui::stats::Medal {
                name,
                count: kills,
                level,
                info,
            }
        })
        .collect()
}

/// The class screen's gate for the player (slot 0): the points scored this
/// round (card SSELB) and each class's cost from `career` ([`apply_costs`]).
pub fn set_player_gate(walk: &mut crate::walk::WalkSim, points: i32, career: &Career) {
    walk.sim.set_spawn_points(0, points);
    apply_costs(walk, career);
}

/// The player's (slot 0) cost of every class on both teams from `career`
/// (`core::sim::Sim::spawn_costs`): the class screen's lock, its text and
/// its silhouette all read it. `[ours]` The `all` cheat
/// (`Cheats::all_classes`, card CHEAT2) makes every class free.
pub fn apply_costs(walk: &mut crate::walk::WalkSim, career: &Career) {
    let free = walk.sim.cheats.all_classes;
    for team in 0..2u8 {
        let t = usize::from(team);
        let costs = walk.teams[t]
            .classes
            .iter()
            .zip(&walk.sim.classes[t])
            .map(|(v, c)| match free {
                true => 0,
                false => class_cost(&v.name, c.unlock_points, career),
            })
            .collect();
        walk.sim.set_spawn_costs(0, team, costs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn career(kills: &[(&str, u32)]) -> Career {
        Career {
            class_kills: kills.iter().map(|(c, k)| ((*c).to_owned(), *k)).collect(),
            ..Default::default()
        }
    }

    /// 0, 49, 50, 99 and 100 kills for an 8- and a 12-point class.
    #[test]
    fn costs_fall_at_50_and_100_kills() {
        for (cost, half) in [(8, 4), (12, 6)] {
            let got = [0, 49, 50, 99, 100, 500].map(|k| effective_cost(cost, k));
            assert_eq!(got, [cost, cost, half, half, 0, 0], "{cost}");
        }
        assert_eq!(effective_cost(7, 50), 4, "odd: rounded up");
        assert_eq!(effective_cost(0, 0), 0);
    }

    /// The counter sums every class of its entry, BF1's included, once each;
    /// a class without a counter keeps its own cost.
    #[test]
    fn counters_sum_their_classes() {
        let c = career(&[
            ("rep_inf_ep3_jettrooper", 10),
            ("rep_inf_ep2_jettrooper_sniper", 5),
            ("rep_inf_jet_trooper", 35),
            ("cis_inf_droideka", 100),
            ("rep_inf_ep3_rifleman", 50),
        ]);
        let jet = of_class("rep_inf_jet_trooper").unwrap();
        assert_eq!(jet.key, "rep_jet_trooper");
        assert_eq!(jet.kills(&c), 50);
        assert_eq!(class_cost("rep_inf_ep2_jettrooper", 12, &c), 6);
        assert_eq!(class_cost("cis_inf_droideka", 12, &c), 0);
        assert_eq!(class_cost("rep_inf_ep3_rifleman", 0, &c), 0);
        assert_eq!(class_cost("rep_inf_ep3_officer", 8, &c), 8);
        assert_eq!(Unlockable::next_stage(49), Some(50));
        assert_eq!(Unlockable::next_stage(50), Some(100));
        assert_eq!(Unlockable::next_stage(100), None);
    }

    /// The page's rows: the counter, the cost now and the next stage.
    #[test]
    fn the_rows_show_kills_cost_and_stage() {
        let c = career(&[("all_inf_smuggler", 14), ("cis_inf_officer", 140)]);
        let rows = rows(&c, &|_, en| en.to_owned());
        assert_eq!(rows.len(), 8);
        let wookiee = &rows[1];
        assert_eq!(
            (wookiee.name.as_str(), wookiee.count),
            ("Wookiee Warrior", 14)
        );
        assert_eq!(wookiee.level, "12 PTS");
        assert_eq!(
            wookiee.info,
            "50 career kills with the Wookiee Warrior halves its unlock cost. 100 career kills \
             to unlock it permanently."
        );
        let magna = &rows[6];
        assert_eq!((magna.count, magna.level.as_str()), (140, "UNLOCKED"));
        assert_eq!(rows[0].level, "8 PTS");
        assert_eq!(magna.info, "You have unlocked the MagnaGuard permanently.");
    }

    /// BF1 locks its four specials at 12 and nothing else.
    #[test]
    fn bf1_locks_its_specials_only() {
        for c in [
            "all_inf_smuggler",
            "all_inf_smugglersnow",
            "imp_inf_dark_trooper",
            "imp_inf_dark_troopersnow",
            "rep_inf_jet_trooper",
            "cis_inf_droideka",
        ] {
            assert_eq!(bf1_cost(c), Some(12), "{c}");
        }
        for c in ["all_inf_soldier", "rep_inf_arc_trooper", "imp_inf_officer"] {
            assert_eq!(bf1_cost(c), None, "{c}");
        }
        let keys: BTreeSet<_> = UNLOCKABLES.iter().map(|u| u.key).collect();
        assert_eq!(keys.len(), 8);
    }

    /// The table is the baked class data's: every BF2 soldier class whose
    /// resolved `PointsToUnlock` is above 0 is in it at that cost, and every
    /// listed BF1 class is a BF1 soldier class with no cost of its own.
    #[test]
    fn the_table_is_the_baked_class_data() {
        let dir = crate::assets::baked_root().join("classes");
        if !dir.join("bf2_all.ron").exists() {
            eprintln!("SKIP: no baked class files");
            return;
        }
        let load = |game: &str| {
            let mut db = crate::classdb::ClassDb::default();
            for side in ["all", "imp", "rep", "cis"] {
                db.load(&dir.join(format!("{game}_{side}.ron"))).unwrap();
            }
            db
        };
        let bf2 = load("bf2");
        let mut found = BTreeSet::new();
        for name in bf2.names() {
            let cost = bf2
                .prop_f32(name, "PointsToUnlock")
                .map_or(0, |p| p.max(0.0) as u32);
            let soldier = bf2.get(name).is_some_and(|d| d.kind == "entc");
            if cost == 0 || !soldier || name.contains("_default_") {
                continue;
            }
            let u = of_class(name).unwrap_or_else(|| panic!("{name} ({cost}) missing"));
            assert_eq!(u.cost, cost, "{name}");
            found.insert(name.to_owned());
        }
        let listed: BTreeSet<_> = UNLOCKABLES
            .iter()
            .flat_map(|u| u.bf2.iter().map(|c| (*c).to_owned()))
            .collect();
        assert_eq!(found, listed);
        let bf1 = load("bf1");
        for u in &UNLOCKABLES {
            for c in u.bf1 {
                assert!(bf1.get(c).is_some(), "{c}");
                assert_eq!(bf1.prop_f32(c, "PointsToUnlock"), None, "{c}");
            }
        }
    }

    /// Each counter's name is BF2's table's, as the career page prints it.
    #[test]
    fn names_are_the_tables() {
        let texts = crate::event_feed::Texts::load(&crate::assets::baked_root(), "bf2");
        if texts.bf2_text("ifs.stats.kills").is_none() {
            eprintln!("SKIP: no baked text table");
            return;
        }
        for u in &UNLOCKABLES {
            let (side, rest) = u.name_class.split_once('_').unwrap();
            let name = texts
                .bf2_text(&format!("entity.{side}.{rest}"))
                .map(str::trim);
            eprintln!("{} {:?}", u.key, name);
            assert_eq!(name, Some(u.english), "{}", u.key);
        }
    }
}
