//! Card UVO2's referee for BF2's unit voice through the BAKED class db
//! (`analysis/re-unit-chatter.md` 3.1, 3.2, 4): every class the three BF2
//! showcase maps' missions field carries its chain's `VOSound` entries - as
//! many as the RE's census of the disc counts for it
//! (`lead_tmp/usnd/bf2_classes.tsv`, `VOSound_lines`) - and its
//! `VOUnitType`; an AcquiredTarget against each fielded enemy class picks
//! the entry for that class's unit type whenever the speaker has one, and
//! every picked line has a take in the map's bank (data-gated: skips without
//! a bake).
//!
//! The rule's referees live beside the code (`game::vo`'s tests: the
//! speaker, the radius, the 31 in 101, the ranking, the sprees, the
//! slaughter counters; `game::sounds`' BF2 take rule).

use game::mission::MissionDef;
use game::roster::FactionRoster;
use game::sounds::SoundLib;
use game::vo;
use std::path::PathBuf;

/// The three BF2 showcase maps' missions (PLAN §Milestones M-ALPHA).
const SHOWCASE: [&str; 3] = ["bf2_uta1_con", "bf2_myg1_con", "bf2_hot1_con"];

/// The disc census's line count and unit type for the classes these
/// missions field (`bf2_classes.tsv`, read 2026-10-16; the type as the class
/// spells it, `083` = 83).
const CENSUS: &[(&str, usize, u8)] = &[
    ("all_inf_engineer", 162, 1),
    ("all_inf_officer", 0, 3),
    ("all_inf_rifleman", 162, 1),
    ("all_inf_rocketeer", 162, 1),
    // The census reads 162: the version the bake keeps (the `all3` voice)
    // has 161 - no `all3_inf_chatter_NearbyEnemySlaughter_InWalker`; which
    // LVL's copy the census row read is not recorded.
    ("all_inf_sniper", 161, 4),
    ("all_inf_wookiee", 20, 2),
    ("all_inf_engineer_snow", 162, 1),
    ("all_inf_officer_snow", 0, 3),
    ("all_inf_rifleman_snow", 162, 1),
    ("all_inf_rocketeer_snow", 162, 1),
    ("all_inf_sniper_snow", 161, 4),
    ("all_inf_wookiee_snow", 20, 2),
    ("cis_inf_droideka", 0, 81),
    ("cis_inf_engineer", 171, 82),
    // The CIS rifleman and officer author none (their chains start empty).
    ("cis_inf_officer", 0, 93),
    ("cis_inf_rifleman", 0, 83),
    ("cis_inf_rocketeer", 171, 82),
    ("cis_inf_sniper", 171, 84),
    ("imp_inf_dark_trooper", 211, 41),
    ("imp_inf_engineer", 211, 42),
    ("imp_inf_officer", 44, 43),
    ("imp_inf_rifleman", 211, 42),
    ("imp_inf_rocketeer", 211, 42),
    ("imp_inf_sniper", 211, 44),
    ("imp_inf_engineer_snow", 211, 42),
    ("imp_inf_rifleman_snow", 211, 42),
    ("imp_inf_rocketeer_snow", 211, 42),
    ("imp_inf_sniper_snow", 211, 44),
    ("rep_inf_ep3_engineer", 197, 121),
    ("rep_inf_ep3_jettrooper", 197, 121),
    ("rep_inf_ep3_officer", 197, 121),
    ("rep_inf_ep3_rifleman", 197, 121),
    ("rep_inf_ep3_rocketeer", 197, 121),
    ("rep_inf_ep3_sniper", 197, 122),
];

/// Lines whose takes the bake does not carry yet: the Wookiee's `wok2_`
/// chatter (the property is in the bank, its stream segments are not).
const NO_TAKE_YET: &str = "wok2_inf_chatter_";

#[test]
fn every_fielded_bf2_class_talks_with_its_own_lines() {
    let root = game::assets::baked_root();
    let classes = root.join("classes");
    if !classes.join("bf2_rep.ron").exists() {
        eprintln!("SKIP: no baked classes at {}", classes.display());
        return;
    }
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mut wrong = Vec::new();
    let (mut checked, mut typed, mut picks) = (0, 0, 0);
    for name in SHOWCASE {
        for era in ["", "_cw"] {
            let path = missions.join(format!("{name}{era}.ron"));
            if !path.exists() {
                continue;
            }
            let m = MissionDef::load(&path).expect("mission");
            let Some(mut lib) = SoundLib::load_for(&root.join(&m.map), &m) else {
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
            for (side, r) in rosters.iter().enumerate() {
                for c in &r.classes {
                    checked += 1;
                    let v = &c.cries.vo;
                    match CENSUS.iter().find(|k| k.0 == c.class) {
                        Some(&(_, n, t)) if (v.entries.len(), v.unit_type) != (n, t) => {
                            wrong.push(format!(
                                "{}: {} lines type {}, the census {n} type {t}",
                                c.class,
                                v.entries.len(),
                                v.unit_type
                            ))
                        }
                        None => wrong.push(format!("{}: not in the census table", c.class)),
                        _ => {}
                    }
                    if v.radius != vo::VO_RADIUS {
                        wrong.push(format!("{}: radius {}", c.class, v.radius));
                    }
                    // AcquiredTarget against each enemy class, ahead at 50 m.
                    for e in &rosters[1 - side].classes {
                        let ut = e.cries.vo.unit_type;
                        let ctx = u32::from(ut) << 16;
                        let Some(pick) = vo::best(v, vo::ACQUIRED_TARGET, ctx) else {
                            continue;
                        };
                        picks += 1;
                        let own = v
                            .entries
                            .iter()
                            .find(|x| x.event == vo::ACQUIRED_TARGET && x.cond == ctx);
                        if let Some(own) = own {
                            typed += 1;
                            if pick.sound != own.sound {
                                wrong.push(format!(
                                    "{} at {}: {} not {}",
                                    c.class, e.class, pick.sound, own.sound
                                ));
                            }
                        }
                        if lib.voice_take(&pick.sound).is_none()
                            && !pick.sound.starts_with(NO_TAKE_YET)
                        {
                            wrong.push(format!(
                                "{} at {}: {} has no take",
                                c.class, e.class, pick.sound
                            ));
                        }
                    }
                }
            }
        }
    }
    eprintln!("{checked} classes, {picks} picks, {typed} by unit type");
    assert!(checked > 0 && typed > 0, "no mission checked");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Playtest C2: an enemy trooper who acquires the player's hero says the
/// line typed for that hero ("It's Darth Maul!"), not the side line. The
/// subject and the object are both the player, so the context is LEFT and
/// under 10 m with the hero's type; the PS2's sort lets the typed line,
/// parsed after `left`, win the tie.
#[test]
fn enemy_troopers_name_each_hero_they_have_a_line_for() {
    let root = game::assets::baked_root();
    let classes = root.join("classes");
    if !classes.join("bf2_rep.ron").exists() {
        eprintln!("SKIP: no baked classes at {}", classes.display());
        return;
    }
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mut wrong = Vec::new();
    let mut named = 0;
    for name in SHOWCASE.iter().copied().chain(["bf2_cor1_con"]) {
        for era in ["", "_cw"] {
            let path = missions.join(format!("{name}{era}.ron"));
            if !path.exists() {
                continue;
            }
            let m = MissionDef::load(&path).expect("mission");
            let Some(mut lib) = SoundLib::load_for(&root.join(&m.map), &m) else {
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
                for c in &r.classes {
                    let v = &c.cries.vo;
                    for e in v.entries.iter().filter(|e| {
                        e.event == vo::ACQUIRED_TARGET
                            && e.cond & 0xffff == 0
                            && (181..=199).contains(&(e.cond >> 16))
                    }) {
                        let hero = vo::Body {
                            unit_type: (e.cond >> 16) as u8,
                            ..vo::Body::default()
                        };
                        let ctx = vo::context(&hero, Some(&hero));
                        let pick = vo::best(v, vo::ACQUIRED_TARGET, ctx).map(|p| &p.sound);
                        named += 1;
                        if pick != Some(&e.sound) {
                            wrong.push(format!("{} {}: {pick:?} not {}", m.map, c.class, e.sound));
                        } else if lib.voice_take(&e.sound).is_none() {
                            wrong.push(format!("{} {}: {} has no take", m.map, c.class, e.sound));
                        }
                    }
                }
            }
        }
    }
    eprintln!("{named} hero lines checked");
    assert!(named > 0, "no hero line checked");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
