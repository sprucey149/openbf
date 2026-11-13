//! BF2 mission heroes: append their authored class beside the ordinary mix,
//! then send the existing round-score feed to the sim's hero manager.

use crate::{
    event_feed::EventFeed,
    mission::MissionDef,
    roster::{ClassRoster, FactionRoster},
};
use core::sim::{
    heroes::{HeroAssign, HeroRules, HeroUnlock, Heroes},
    Sim,
};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeroAudio {
    pub offer: Option<String>,
    pub spawned: Option<String>,
    pub defeated: Option<String>,
    pub music: Option<String>,
    pub saber_ambient: Option<String>,
    pub saber_on: Option<String>,
    pub saber_off: Option<String>,
    pub saber_flight: Option<String>,
}

impl HeroAudio {
    pub fn from_db(db: &crate::classdb::ClassDb, class: &str) -> Self {
        Self {
            // PS2 soldier class+0x1254; hero HUD's offer table (0x383208).
            offer: db
                .prop_hash(class, 0x4ac2_b220)
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
                .map(str::to_owned),
            spawned: crate::roster::sound_key(db, class, "SndHeroSpawned"),
            defeated: crate::roster::sound_key(db, class, "SndHeroDefeated"),
            music: crate::roster::sound_key(db, class, "SoldierMusic"),
            saber_ambient: crate::roster::primary_weapon(db, class).and_then(|(weapon, _)| {
                (db.engine_base(&weapon) == Some("melee"))
                    // PS2 melee parser: key -> class+0x278 sound handle.
                    .then(|| db.prop_hash(&weapon, 0xde71_b19f).map(str::to_owned))
                    .flatten()
            }),
            saber_on: crate::roster::primary_weapon(db, class)
                .and_then(|(weapon, _)| crate::roster::sound_key(db, &weapon, "TurnOnSound")),
            saber_off: crate::roster::primary_weapon(db, class)
                .and_then(|(weapon, _)| crate::roster::sound_key(db, &weapon, "TurnOffSound")),
            saber_flight: db.weapon_slots(class).iter().find_map(|slot| {
                (db.engine_base(&slot.weapon) == Some("meleethrow"))
                    .then(|| db.prop(&slot.weapon, "OrdnanceName"))
                    .flatten()
                    .and_then(|ord| crate::roster::sound_key(db, ord.trim(), "OrdnanceSound"))
            }),
        }
    }
}

/// The hero matching this mission team; cross-era factions keep their own
/// infantry roster rather than borrowing an opponent's hero.
pub fn append(
    roster: &mut FactionRoster,
    classes_dir: &Path,
    mission: Option<&MissionDef>,
    team: usize,
) -> Option<u8> {
    let m =
        mission.filter(|m| m.source_game.eq_ignore_ascii_case("bf2") && !m.is_hero_assault())?;
    let source_team = if m
        .teams
        .get(team)
        .is_some_and(|t| t.faction == roster.faction)
    {
        team + 1
    } else {
        m.teams.iter().position(|t| t.faction == roster.faction)? + 1
    };
    let name = &m
        .heroes
        .iter()
        .find(|(t, _)| usize::from(*t) == source_team)?
        .1;
    if roster
        .classes
        .iter()
        .any(|c| c.class.eq_ignore_ascii_case(name))
    {
        return None;
    }
    let source_faction = name.split('_').next().unwrap_or(&roster.faction);
    let db =
        match crate::roster::load_classdb(classes_dir, "bf2", &[&roster.faction, source_faction]) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("hero {name}: {e:#}");
                return None;
            }
        };
    let Some(name) = db.resolve(name) else {
        eprintln!("hero {name}: class not baked");
        return None;
    };
    match ClassRoster::build(&db, name, 0, Some(1)) {
        Ok(mut class) => {
            class.anim.custom = match crate::animator::custom::for_hero(classes_dir, m, name) {
                Ok(descriptors) => descriptors,
                Err(error) => {
                    eprintln!("hero {name}: custom animations: {error:#}");
                    return None;
                }
            };
            let index = u8::try_from(roster.classes.len()).ok()?;
            roster.classes.push(class);
            Some(index)
        }
        Err(e) => {
            eprintln!("hero {name}: {e:#}");
            None
        }
    }
}

pub fn prepare_assault(
    roster: &mut FactionRoster,
    classes_dir: &Path,
    mission: &MissionDef,
    team: usize,
    dlc_heroes: bool,
) -> anyhow::Result<()> {
    let mut first_error = None;
    roster.classes.retain_mut(|class| {
        match crate::animator::custom::for_hero(classes_dir, mission, &class.class) {
            Ok(descriptors) => {
                class.anim.custom = descriptors;
                true
            }
            Err(error) => {
                eprintln!("hero {}: custom animations: {error:#}", class.class);
                if first_error.is_none() {
                    first_error = Some(error);
                }
                false
            }
        }
    });
    if dlc_heroes {
        match assault_dlc_class(classes_dir, mission, team) {
            Ok(Some(class)) => roster.classes.push(class),
            Ok(None) => {}
            Err(error) => eprintln!("DLC heroes: {error:#}"),
        }
    }
    if roster.classes.is_empty() {
        let cause = first_error.unwrap_or_else(|| anyhow::anyhow!("no authored hero classes"));
        return Err(cause.context(format!(
            "Hero Assault team {} ({}): no usable heroes after loading custom animations from {}",
            team + 1,
            roster.faction,
            classes_dir.display()
        )));
    }
    Ok(())
}

fn assault_dlc_class(
    classes_dir: &Path,
    mission: &MissionDef,
    team: usize,
) -> anyhow::Result<Option<ClassRoster>> {
    let side = ["rep", "cis"][team.min(1)];
    let Some(name) = crate::dlc::hero(classes_dir, mission, side)? else {
        return Ok(None);
    };
    let mut db = crate::roster::load_classdb(classes_dir, "bf2", &[side])?;
    db.load(&classes_dir.join(crate::dlc::CLASSES))?;
    let name = db
        .resolve(name)
        .ok_or_else(|| anyhow::anyhow!("hero {name}: class not baked"))?;
    let mut class = ClassRoster::build(&db, name, 1, Some(2))?;
    if let Some(label) = crate::dlc::label(name) {
        class.label = label.to_owned();
    }
    let geometry = class
        .geometry
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("{name}: no DLC geometry"))?;
    class.anim.custom = crate::dlc::animations(classes_dir, geometry)?;
    Ok(Some(class))
}

pub fn install(sim: &mut Sim, classes: [Option<u8>; 2]) {
    install_with(sim, classes, HeroRules::default());
}

/// [`install`] with the session's rules ([`rules`]).
pub fn install_with(sim: &mut Sim, classes: [Option<u8>; 2], rules: HeroRules) {
    sim.heroes = classes
        .iter()
        .any(Option::is_some)
        .then(|| Heroes::with_rules(classes, rules).expect("session hero rules are valid"));
}

/// The shell's Hero Options as the battle's hero rules, as the PS2's
/// `ScriptCB_SetNetHeroDefaults` (B2:44471-44497) copies `HeroPrefs` into
/// the manager: Unlock Hero 3 Points / 5 Timer with its value, Assign To
/// 1 Best / 4 Worst / 7 Random, the respawn Timer 0 Always / n s / -1
/// Never (`analysis/re-bf2-menus.md` §2.3). `None` when Heroes is Off: no
/// hero is installed at all. The separate AI switch opts bots into eligibility.
pub fn rules(session: &ui::shell::SessionOptions) -> Option<HeroRules> {
    if !session.heroes {
        return None;
    }
    let value = session.hero_unlock_val.clamp(1, 120);
    Some(HeroRules {
        unlock: if session.hero_unlock == 5 {
            HeroUnlock::Timer(value as u32)
        } else {
            HeroUnlock::Points(value)
        },
        assign: match session.hero_assign {
            4 => HeroAssign::Worst,
            7 => HeroAssign::Random,
            _ => HeroAssign::Best,
        },
        respawn_ticks: (session.hero_respawn >= 0)
            .then(|| session.hero_respawn.min(120) as u64 * u64::from(core::TICK_HZ)),
        ai_candidates: session.ai_heroes,
        ..HeroRules::default()
    })
}

/// One scoring authority for class unlocks, hero selection and round results.
/// Runs before every tick of a battle that keeps a feed (the window, the
/// headless replay). `WalkSim` owns no feed, so `--bench` steps without one:
/// its points stay 0 and no Points unlock happens.
pub fn scores(sim: &mut Sim, feed: &EventFeed) {
    if let Some(heroes) = &mut sim.heroes {
        heroes.points.clear();
        heroes
            .points
            .extend(feed.round().units.iter().map(|u| u.ordinary_points()));
    }
}

/// Read one tick's events and publish its score before the next fixed tick,
/// including when several ticks run within one rendered frame.
pub fn tick(walk: &mut crate::walk::WalkSim, feed: &mut EventFeed) {
    let positive_points = feed.tick(&walk.sim, walk.player);
    walk.sim.score_medal_points(walk.player, positive_points);
    scores(&mut walk.sim, feed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Hero rows' values (`re-bf2-menus.md` §2.3) as the manager's rules.
    #[test]
    fn session_hero_rows_become_the_battle_rules() {
        let mut s = ui::shell::SessionOptions::default();
        // [ours] On by default; the PS2's values otherwise.
        assert_eq!(rules(&s), Some(HeroRules::default()));
        s.heroes = false;
        assert_eq!(rules(&s), None, "Off: no heroes at all");
        s.heroes = true;
        s.hero_unlock_val = 37;
        assert_eq!(rules(&s).unwrap().unlock, HeroUnlock::Points(37));
        s.hero_unlock = 5;
        assert_eq!(rules(&s).unwrap().unlock, HeroUnlock::Timer(37));
        for (row, want) in [
            (1, HeroAssign::Best),
            (4, HeroAssign::Worst),
            (7, HeroAssign::Random),
        ] {
            s.hero_assign = row;
            assert_eq!(rules(&s).unwrap().assign, want);
        }
        let hz = u64::from(core::TICK_HZ);
        for (row, want) in [
            (0, Some(0)),
            (45, Some(45 * hz)),
            (120, Some(120 * hz)),
            (-1, None),
        ] {
            s.hero_respawn = row;
            assert_eq!(rules(&s).unwrap().respawn_ticks, want, "{row}");
        }
        assert!(!rules(&s).unwrap().ai_candidates, "human-only by default");
        s.ai_heroes = true;
        assert!(rules(&s).unwrap().ai_candidates, "the switch admits bots");
    }
}
