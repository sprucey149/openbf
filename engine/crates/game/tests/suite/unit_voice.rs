//! Card UCRY's referees: a unit's hurt and death cries
//! (`analysis/re-unit-chatter.md` 2.7, 3.6).
//!
//! - the rule: a surviving hit that took more than 2 % of the health it
//!   left asks the victim's `HurtSound`; a smaller one, a shield-swallowed
//!   or zero-damage one and the killing hit ask nothing; the kill (a bolt's
//!   or the world's) asks `DeathSound` (`game::event_sounds::cries`);
//! - the voice: every class the six showcase maps' missions field resolves
//!   both keys through the BAKED class db to the property its faction's
//!   sounds hold, and that property has a take in the map's own bank
//!   (data-gated: skips without a bake).
//!
//! The one voice per unit (a cry cuts the one still sounding) is
//! `game::sounds`' own test (`a_unit_has_one_voice_for_its_cries`).

use core::sim::{DeathCause, Event};
use game::event_sounds::{cries, hurts, Cry};
use game::mission::MissionDef;
use game::roster::FactionRoster;
use game::sounds::SoundLib;
use game::walk::{Cries, WalkSim};
use glam::Vec3;
use std::path::PathBuf;

#[test]
fn a_hit_cries_over_two_percent_of_the_health_left() {
    // 2 % of 100 left is 2.
    assert!(hurts(2.5, 100.0));
    assert!(!hurts(1.5, 100.0), "a hit of 2 % or less is tiny");
    assert!(hurts(30.0, 270.0), "a rifle bolt on a fresh trooper");
    // The share is of what is LEFT: 3 points on 200 left is 1.5 % ...
    assert!(!hurts(3.0, 200.0));
    // ... on 100 left 3 %.
    assert!(hurts(3.0, 100.0));
    assert!(!hurts(0.0, 300.0), "a shield-swallowed or zero-damage hit");
    assert!(!hurts(50.0, 0.0), "the killing hit cries death, not hurt");
    assert!(!hurts(50.0, -20.0));
}

/// A walk whose every class cries `hurt_x` / `death_x`.
fn walk() -> WalkSim {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    for t in &mut w.teams {
        for c in &mut t.classes {
            c.cries = Cries {
                hurt: Some("hurt_x".into()),
                death: Some("death_x".into()),
                ..Cries::default()
            };
        }
    }
    w
}

fn asked(w: &WalkSim) -> Vec<(u32, Option<String>, bool)> {
    let mut out = Vec::new();
    cries(w, |c: Cry| {
        out.push((c.unit, c.name.map(str::to_owned), c.death))
    });
    out
}

#[test]
fn the_events_ask_the_victims_cries() {
    let mut w = walk();
    let me = w.player;
    let hit = |damage: f32, health_left: f32| Event::Hit {
        direct_round: true,
        shooter: me,
        victim: me,
        damage,
        age: 0.1,
        health_left,
        dir: glam::Vec3::NEG_Z,
    };
    let hurt = (me.index, Some("hurt_x".to_owned()), false);
    let death = (me.index, Some("death_x".to_owned()), true);

    w.sim.events = vec![hit(40.0, 260.0)];
    assert_eq!(asked(&w), vec![hurt.clone()]);
    w.sim.events = vec![hit(4.0, 260.0), hit(0.0, 300.0)];
    assert_eq!(asked(&w), vec![], "tiny and swallowed hits ask nothing");
    // Two hits in one tick: two asks (the second cuts the first's voice).
    w.sim.events = vec![hit(40.0, 220.0), hit(40.0, 180.0)];
    assert_eq!(asked(&w), vec![hurt.clone(), hurt.clone()]);
    // The killing hit asks death and no hurt.
    w.sim.events = vec![
        hit(40.0, -10.0),
        Event::Kill {
            shooter: me,
            victim: me,
        },
    ];
    assert_eq!(asked(&w), vec![death.clone()]);
    // A hurt then the kill in one tick: hurt, then death (which cuts it).
    w.sim.events = vec![
        hit(40.0, 30.0),
        hit(40.0, -10.0),
        Event::Kill {
            shooter: me,
            victim: me,
        },
    ];
    assert_eq!(asked(&w), vec![hurt.clone(), death.clone()]);
    // The world's kills cry too (the PS2's kill routine, B1 FUN_001bbc98).
    w.sim.events = vec![Event::EnvDeath {
        unit: me,
        cause: DeathCause::Fall,
    }];
    assert_eq!(asked(&w), vec![death]);
    // A class without the keys still asks (its voice is cut), with no name.
    for t in &mut w.teams {
        for c in &mut t.classes {
            c.cries = Cries::default();
        }
    }
    w.sim.events = vec![hit(40.0, 260.0)];
    assert_eq!(asked(&w), vec![(me.index, None, false)]);
}

/// The cry is the victim's own class's, not the shooter's.
#[test]
fn the_cry_is_the_victims_class() {
    let mut w = walk();
    w.teams[0].classes[0].cries.hurt = Some("hurt_team0".into());
    w.teams[1].classes[0].cries.hurt = Some("hurt_team1".into());
    let me = w.player;
    let mine = w.sim.units[me.index as usize].team;
    let enemy = w
        .sim
        .spawn(&core::unit::UnitSpec::trooper(
            1 - mine,
            Vec3::new(5.0, 0.0, 0.0),
        ))
        .expect("enemy");
    for (shooter, victim, team) in [(enemy, me, mine), (me, enemy, 1 - mine)] {
        w.sim.events = vec![Event::Hit {
            direct_round: true,
            shooter,
            victim,
            damage: 40.0,
            age: 0.0,
            health_left: 100.0,
            dir: glam::Vec3::NEG_Z,
        }];
        let got = asked(&w);
        assert_eq!(got[0].0, victim.index);
        assert_eq!(
            got[0].1.as_deref(),
            Some(format!("hurt_team{team}").as_str())
        );
    }
}

fn baked_root() -> PathBuf {
    game::assets::baked_root()
}

/// The six showcase maps' missions (PLAN §Milestones M-ALPHA).
const SHOWCASE: [&str; 6] = [
    "bf1_nab1_con",
    "tat1_con",
    "bf1_yav2_con",
    "bf2_uta1_con",
    "bf2_myg1_con",
    "bf2_hot1_con",
];

/// Classes whose voice is not their side's trooper's, as the RE's own
/// census of the disc reads them (`lead_tmp/usnd/bf{1,2}_classes.tsv`, card
/// USND: every `entc` chunk, the BASE chain followed).
const PINNED: [(&str, &str, &str, &str); 6] = [
    (
        "bf1",
        "all_inf_marksman",
        "all_inf_com_chatter_wound_female",
        "all_inf_com_chatter_death_female",
    ),
    (
        "bf1",
        "all_inf_smuggler",
        "all_inf_com_chatter_wound_wookie",
        "all_inf_com_chatter_death_wookie",
    ),
    (
        "bf2",
        "all_inf_sniper",
        "all_inf_com_chatter_wound_female",
        "all_inf_com_chatter_death_female",
    ),
    (
        "bf2",
        "all_inf_sniper_snow",
        "all_inf_com_chatter_wound_female",
        "all_inf_com_chatter_death_female",
    ),
    (
        "bf2",
        "cis_inf_rifleman",
        "cis_inf_com_chatter_wound",
        "droid_death",
    ),
    (
        "bf1",
        "cis_inf_battledroid",
        "cis_inf_com_chatter_wound",
        "droid_death",
    ),
];

#[test]
fn every_fielded_class_has_its_own_cries_in_the_map_bank() {
    let root = baked_root();
    let classes = root.join("classes");
    if !classes.join("bf1_all.ron").exists() {
        eprintln!("SKIP: no baked classes at {}", classes.display());
        return;
    }
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let (mut checked, mut pinned) = (0, 0);
    let mut wrong = Vec::new();
    for name in SHOWCASE {
        for era in ["", "_cw"] {
            let path = missions.join(format!("{name}{era}.ron"));
            if !path.exists() {
                continue;
            }
            let m = MissionDef::load(&path).expect("mission");
            let map_dir = root.join(&m.map);
            let Some(lib) = SoundLib::load_for(&map_dir, &m) else {
                eprintln!("SKIP {}: no sound bank", m.map);
                continue;
            };
            let f = m.factions();
            let (rosters, _) = FactionRoster::pair(
                &classes,
                &m.source_game,
                [f[0].as_str(), f[1].as_str()],
                Some(&m),
            )
            .expect("rosters");
            for r in &rosters {
                for c in r.classes.iter().filter(|c| c.droideka.is_none()) {
                    checked += 1;
                    let (Some(hurt), Some(death)) = (&c.cries.hurt, &c.cries.death) else {
                        wrong.push(format!(
                            "{}{era} {}: no cry keys {:?}",
                            name, c.class, c.cries
                        ));
                        continue;
                    };
                    // The class's own voice: its faction's prefix (the CIS
                    // die as droids), hurt and death in ONE voice (the
                    // women's `_female`, the Wookiees' `_wookie`).
                    let side = &r.faction;
                    let voice = hurt.strip_prefix(&format!("{side}_inf_com_chatter_wound"));
                    let ok = match voice {
                        None => false,
                        Some(_) if side == "cis" => death == "droid_death",
                        Some(v) => *death == format!("{side}_inf_com_chatter_death{v}"),
                    };
                    if !ok {
                        wrong.push(format!("{}{era} {}: {hurt} / {death}", name, c.class));
                    }
                    if let Some((_, _, h, d)) = PINNED
                        .iter()
                        .find(|p| p.0 == m.source_game && p.1 == c.class)
                    {
                        if (h, d) != (&hurt.as_str(), &death.as_str()) {
                            wrong.push(format!(
                                "{} {}: {hurt} / {death}, the disc {h} / {d}",
                                m.source_game, c.class
                            ));
                        }
                        pinned += 1;
                    }
                    println!("CRY {} {} {} {hurt} {death}", m.source_game, m.map, c.class);
                    for n in [hurt, death] {
                        let take = lib.take_path(n, 0);
                        if !take.as_ref().is_some_and(|p| p.exists()) {
                            wrong.push(format!(
                                "{}{era} {}: {n} has no take ({take:?})",
                                name, c.class
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {checked}:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    assert!(checked >= 60, "only {checked} classes checked");
    assert!(pinned >= 8, "only {pinned} pinned classes fielded");
    println!("{checked} fielded classes cry in their own voice");
}

/// NAB1's natives: their cries are in the map's bank under the names the
/// Gungan class gives.
#[test]
fn naboo_gungans_cries_are_in_the_bank() {
    let root = baked_root();
    let m = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../missions/bf1_nab1_con.ron"
    ));
    if !m.is_file() {
        eprintln!("SKIP: no engine/missions (scripts/pack.py generates them)");
        return;
    }
    let m = MissionDef::load(&m).expect("mission");
    let Some(lib) = SoundLib::load_for(&root.join(&m.map), &m) else {
        eprintln!("SKIP: no NAB1 sound bank");
        return;
    };
    for n in ["gungan_hurt", "gungan_die"] {
        assert!(lib.take_path(n, 0).is_some_and(|p| p.exists()), "{n}");
    }
}
