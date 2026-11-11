//! The PS2 spawn screens' content (card SSELB, `analysis/re-spawn-flow.md`
//! §2-§4): what `ui::spawn_draw` draws for the player's slot - the side
//! select's rows and highlight, the spawn map's objective and legend, the class panel's
//! lines as `FUN_00396430` assembles them - with the disc's texts through
//! [`crate::event_feed::EventFeed`].

/// The PS2 spawn screen for `slot` as the HUD pass draws it (card SSELB,
/// `ui::spawn_draw`): its texts from the disc's table through `feed`, the
/// class panel's lines assembled as `FUN_00396430` does
/// (`analysis/re-spawn-flow.md` §4). `None` when the battle runs no spawn
/// screens or the slot is only waiting.
pub fn page(
    walk: &crate::walk::WalkSim,
    feed: &crate::event_feed::EventFeed,
    slot: usize,
) -> Option<ui::spawn_draw::SpawnPage> {
    use core::sim::{ClassRefusal, SideChoice, SpawnStage};
    use ui::spawn_draw::SpawnPage;
    let stage = walk.sim.spawn_stage.get(slot).copied().unwrap_or_default();
    // Issue #28: the live class screen shows with or without the screens.
    if !walk.sim.spawn_screens && stage != SpawnStage::Alive {
        return None;
    }
    let actor = walk.sim.players.get(slot).copied().flatten()?;
    let me = walk
        .sim
        .units
        .get(actor.index as usize)
        .filter(|u| u.gen == actor.gen)?;
    match stage {
        SpawnStage::Side => {
            let [a, b] = feed.team_names();
            Some(SpawnPage::Side {
                title: feed.screen_text("ifs.sideselect.chooseteam", "Select a team"),
                rows: vec![
                    a.to_owned(),
                    b.to_owned(),
                    feed.screen_text("ifs.sideselect.autoassign", "Auto Assign"),
                ],
                selected: match walk.sim.side_choice.get(slot).copied().unwrap_or_default() {
                    SideChoice::Team1 => 0,
                    SideChoice::Team2 => 1,
                    SideChoice::Auto => 2,
                },
                select: feed.screen_text("common.select", "Select"),
            })
        }
        // The objective is the conquest mission's `AddMissionObjective`
        // text (`game.modes.con`; the defenders' `con2` reads the same);
        // the port's battles are all conquest.
        SpawnStage::Map => Some(SpawnPage::Map {
            objective: feed.space_objective(me.team).unwrap_or_else(|| {
                feed.screen_text("game.modes.con", "Capture all the Command Posts")
            }),
            legend: [
                feed.screen_text("game.spawndisplay.commandpost", "Command Post"),
                feed.screen_text("game.spawndisplay.cpchange", "Change Command Post"),
                feed.screen_text(
                    "game.spawndisplay.spawndisplay.promptNext",
                    "{btna} Select Command Post",
                ),
            ],
        }),
        SpawnStage::Class | SpawnStage::Alive => {
            let team = me.team.min(1);
            let t = &walk.teams[usize::from(team)];
            let classes = walk.sim.spawn_class_indices(slot, team);
            let pick = walk.sim.spawn_class_choice(slot, team)?;
            let row = classes.iter().position(|&c| c == pick)?;
            let class = t.classes.get(pick)?;
            let title = feed
                .class_names(team)
                .get(pick)
                .cloned()
                .unwrap_or_else(|| class.label.clone());
            // The refusal line first, then "Unit Count: %d" (`FUN_00396430`,
            // B2:392035-392058, re-spawn-flow §4.3); the count is the team's
            // members on the field in this class (`FUN_0036f538`). `[ours]`:
            // no "Max: %d" - no class has a max (2026-10-29).
            let mut text = match walk.sim.class_refusal(slot, team, pick) {
                Some(ClassRefusal::Points(n)) => feed
                    .text_by_hash(0x155a_b074, "Get %d more points to unlock!\r\n")
                    .replacen("%d", &n.to_string(), 1),
                Some(ClassRefusal::HeroUnavailable) => String::from("Hero unavailable\n"),
                None => String::new(),
            };
            let count = walk.sim.class_count(team, pick);
            text.push_str(
                &feed
                    .text_by_hash(0xecf8_496f, "Unit Count: %d\r\n\r\n")
                    .replacen("%d", &count.to_string(), 1),
            );
            text.push_str(&feed.text_by_hash(0xfe98_f5cf, "Equipment:"));
            text.push('\n');
            // The primary first, then the other slots in slot order. A
            // weapon out of the player's cycle is not listed (`FUN_00396430`,
            // B2:392007-392128: an award until its medal is held, then the
            // weapon it replaces), so an award shows in its own slot in
            // place of that weapon; a guided launcher, which replaces
            // nothing, shows beside the launcher. Without the sim's class
            // the award slots are left out.
            let kit = walk
                .sim
                .classes
                .get(usize::from(team))
                .and_then(|c| c.get(pick));
            // The sim weapon in a slot (`None`: the primary).
            let weapon = |slot: Option<u8>| {
                kit.and_then(|kit| match slot {
                    None => Some(kit.weapon_class),
                    Some(slot) => kit
                        .slots
                        .iter()
                        .find(|s| s.slot == slot)
                        .map(|s| s.weapon_class),
                })
            };
            let listed = |slot: Option<u8>| match (kit, weapon(slot)) {
                (Some(kit), Some(c)) => !walk.sim.class_weapon_locked(me, kit, c),
                _ => slot.is_none_or(|slot| !class.award_slots.contains(&slot)),
            };
            let mut held: Vec<(Option<u8>, &crate::walk::WeaponHud)> = class
                .held_hud
                .iter()
                .filter(|(slot, _)| listed(Some(*slot)))
                .map(|(slot, w)| (Some(*slot), w))
                .collect();
            held.sort_by_key(|(slot, _)| *slot);
            let primary = listed(None).then_some((None, &class.weapon_hud));
            for (slot, w) in primary.into_iter().chain(held) {
                if w.class.is_empty() {
                    continue;
                }
                // An award is named by its own class. A BF1 class's
                // replacing award keeps the HUD row of the weapon it
                // replaces (its mesh), so the row's class would name that
                // weapon instead.
                let award = weapon(slot)
                    .and_then(|c| walk.sim.weapon_classes.get(c))
                    .filter(|c| c.unlock_medal().is_some())
                    .map(|c| c.name.to_ascii_lowercase());
                let named = award.as_ref().unwrap_or(&w.class);
                let name = feed.weapon_name(named).unwrap_or_else(|| named.clone());
                text.push_str(&name);
                text.push('\n');
            }
            let lines = text
                .trim_end()
                .split('\n')
                .map(|l| l.trim_end_matches('\r').to_owned())
                .collect();
            Some(SpawnPage::Class {
                title,
                lines,
                back: feed
                    .screen_text("game.spawndisplay.characterdisplay.promptprev", "Back")
                    .trim()
                    .to_owned(),
                accept: feed
                    .screen_text(
                        "game.spawndisplay.characterdisplay.promptnext",
                        "Select Character",
                    )
                    .trim()
                    .to_owned(),
                team,
                class: row,
                refused: classes
                    .iter()
                    .take(32)
                    .enumerate()
                    .filter(|&(_, &class)| walk.sim.class_refusal(slot, team, class).is_some())
                    .fold(0, |m, (j, _)| m | 1 << j),
                classes: classes.into_iter().map(|c| c as u8).collect(),
            })
        }
        SpawnStage::Accepted => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod heroes;
    use crate::event_feed::EventFeed;
    use crate::walk::{ClassVisual, WalkSim, WeaponHud};
    use core::sim::{buttons, PadState};
    use glam::Vec3;
    use ui::spawn_draw::SpawnPage;

    fn pad(b: u32) -> PadState {
        PadState {
            buttons: b,
            ..PadState::default()
        }
    }

    /// The side select follows the sim's highlight; the class panel lists
    /// the count of the class's live team-mates, a blank line, Equipment:
    /// and the primary, then the other slots in slot order; a battle
    /// without screens has no page.
    #[test]
    fn the_pages_follow_the_sim() {
        let mut w = WalkSim::new(3, Vec3::ZERO, None);
        let feed = EventFeed::default();
        assert_eq!(page(&w, &feed, 0), None, "no screens");
        assert!(w.sim.open_spawn_screens(0, true));
        let side = |w: &WalkSim| match page(w, &feed, 0) {
            Some(SpawnPage::Side {
                rows,
                selected,
                title,
                select,
            }) => {
                assert_eq!(
                    (title.as_str(), select.as_str()),
                    ("Select a team", "Select")
                );
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[2], "Auto Assign");
                selected
            }
            p => panic!("{p:?}"),
        };
        assert_eq!(side(&w), 0);
        w.step_tick(&[pad(buttons::SPAWN_POST_DOWN)]);
        w.step_tick(&[pad(0)]);
        assert_eq!(side(&w), 1);
        w.step_tick(&[pad(buttons::SPAWN_ACCEPT)]);
        w.step_tick(&[pad(0)]);
        assert_eq!(
            page(&w, &feed, 0),
            Some(SpawnPage::Map {
                objective: "Capture all the Command Posts".into(),
                legend: [
                    "Command Post".into(),
                    "Change Command Post".into(),
                    "{btna} Select Command Post".into(),
                ],
            })
        );
        w.sim.spawn_stage[0] = core::sim::SpawnStage::Class;
        let me = w.player.index as usize;
        let u = &w.sim.units[me];
        w.sim.classes[usize::from(u.team)] = vec![core::unit::UnitClass {
            consts: u.spec.consts,
            max_health: u.health.max,
            health_type: u.spec.health_type,
            weapon_class: u.weapon_class,
            total_clips: 6,
            weapon_slot: u.weapon_slot,
            slots: u.spec.slots.clone(),
            jet: false,
            capture_posts: true,
            drops: Default::default(),
            chunk: None,
            droideka: None,
            min: 1,
            max: None,
            unlock_points: 0,
        }];
        let team = usize::from(w.sim.units[me].team.min(1));
        let hud = |class: &str| WeaponHud {
            class: class.into(),
            ..WeaponHud::default()
        };
        let c: &mut ClassVisual = &mut w.teams[team].classes[0];
        c.weapon_hud = hud("imp_weap_inf_rifle");
        c.held_hud = vec![
            (4, hud("imp_weap_inf_thermaldetonator")),
            (1, hud("imp_weap_inf_pistol")),
            (5, hud("imp_weap_award_rifle")),
        ];
        // An award slot is held but not listed.
        c.award_slots = vec![5];
        let mates = w
            .sim
            .units
            .iter()
            .filter(|u| u.alive && usize::from(u.team) == team && u.spec.class == 0)
            .count();
        match page(&w, &feed, 0) {
            Some(SpawnPage::Class {
                lines,
                back,
                accept,
                ..
            }) => {
                assert_eq!(
                    lines,
                    [
                        format!("Unit Count: {mates}"),
                        String::new(),
                        "Equipment:".into(),
                        "imp_weap_inf_rifle".into(),
                        "imp_weap_inf_pistol".into(),
                        "imp_weap_inf_thermaldetonator".into(),
                    ]
                );
                assert_eq!(
                    (back.as_str(), accept.as_str()),
                    ("Back", "Select Character")
                );
            }
            p => panic!("{p:?}"),
        }
        // A class with a cost (re-spawn-flow §4.3): the refusal line
        // first, the count with its max, and the class drawn black.
        w.sim.classes[team] = vec![core::unit::UnitClass {
            consts: core::soldier::SoldierConstants::default(),
            max_health: 300.0,
            health_type: core::weapon::HealthType::Person,
            weapon_class: 0,
            total_clips: 6,
            weapon_slot: 0,
            slots: std::sync::Arc::new([]),
            jet: false,
            capture_posts: true,
            drops: core::sim::DropTable::default(),
            chunk: None,
            droideka: None,
            min: 1,
            max: Some(4),
            unlock_points: 12,
        }];
        w.sim.set_spawn_points(0, 5);
        match page(&w, &feed, 0) {
            Some(SpawnPage::Class { lines, refused, .. }) => {
                assert_eq!(
                    lines[..2],
                    [
                        "Get 7 more points to unlock!".to_owned(),
                        format!("Unit Count: {mates}"),
                    ]
                );
                assert_eq!(refused & 1, 1);
            }
            p => panic!("{p:?}"),
        }
        w.sim.set_spawn_points(0, 12);
        match page(&w, &feed, 0) {
            Some(SpawnPage::Class { lines, refused, .. }) => {
                assert_eq!(lines[0], format!("Unit Count: {mates}"));
                assert_eq!(refused & 1, 0);
            }
            p => panic!("{p:?}"),
        }
        // Card UNLK3: the career's cost (half at 50 kills) sets the line and
        // the silhouette.
        w.sim.set_spawn_costs(0, team as u8, vec![6]);
        w.sim.set_spawn_points(0, 4);
        match page(&w, &feed, 0) {
            Some(SpawnPage::Class { lines, refused, .. }) => {
                assert_eq!(lines[0], "Get 2 more points to unlock!");
                assert_eq!(lines[1], format!("Unit Count: {mates}"));
                assert_eq!(refused & 1, 1);
            }
            p => panic!("{p:?}"),
        }
        w.sim.set_spawn_costs(0, team as u8, vec![0]);
        w.sim.set_spawn_points(0, 0);
        match page(&w, &feed, 0) {
            Some(SpawnPage::Class { lines, refused, .. }) => {
                assert_eq!(lines[0], format!("Unit Count: {mates}"));
                assert_eq!(refused & 1, 0, "unlocked for good");
            }
            p => panic!("{p:?}"),
        }
    }

    /// The unit screen lists what the player would carry, as BF2's
    /// `FUN_00396430` does: an award only once its medal is held, and then
    /// not the weapon it replaces, every slot in its order; a guided
    /// launcher, which replaces nothing, beside the launcher.
    #[test]
    fn the_unit_screen_lists_the_players_awards() {
        use core::unit::SlotSpec;
        use core::weapon::WeaponClass;
        let mut w = WalkSim::new(3, Vec3::ZERO, None);
        let feed = EventFeed::default();
        assert!(w.sim.open_spawn_screens(0, true));
        w.sim.spawn_stage[0] = core::sim::SpawnStage::Class;
        let me = w.player.index as usize;
        let team = usize::from(w.sim.units[me].team.min(1));
        let mut add = |name: &str, unlock: Option<i8>, lock: Option<i8>| {
            w.sim.add_weapon_class(WeaponClass {
                name: name.into(),
                medal_unlock: unlock,
                medal_lock: lock,
                ..WeaponClass::default()
            })
        };
        let kit = [
            add("imp_weap_inf_shotgun", None, Some(5)),
            add("imp_weap_inf_pistol", None, Some(0)),
            add("imp_weap_inf_rocket_launcher", None, None),
            add("imp_weap_award_blast_cannon", Some(5), None),
            add("imp_weap_award_pistol", Some(0), None),
            add("imp_weap_award_rocket_launcher", Some(2), None),
        ];
        let u = &w.sim.units[me];
        w.sim.classes[team] = vec![core::unit::UnitClass {
            consts: u.spec.consts,
            max_health: u.health.max,
            health_type: u.spec.health_type,
            weapon_class: kit[0],
            total_clips: 6,
            weapon_slot: 0,
            slots: kit[1..]
                .iter()
                .zip(1u8..)
                .map(|(&weapon_class, slot)| SlotSpec {
                    slot,
                    channel: 0,
                    weapon_class,
                    clips: 4,
                })
                .collect(),
            jet: false,
            capture_posts: true,
            drops: Default::default(),
            chunk: None,
            droideka: None,
            min: 1,
            max: None,
            unlock_points: 0,
        }];
        let hud = |class: &str| WeaponHud {
            class: class.into(),
            ..WeaponHud::default()
        };
        let names: Vec<String> = kit
            .iter()
            .map(|&c| w.sim.weapon_classes[c].name.clone())
            .collect();
        let c: &mut ClassVisual = &mut w.teams[team].classes[0];
        c.weapon_hud = hud(&names[0]);
        // Held out of slot order, as a class may list them.
        c.held_hud = vec![
            (5, hud(&names[5])),
            (1, hud(&names[1])),
            (3, hud(&names[3])),
            (2, hud(&names[2])),
            (4, hud(&names[4])),
        ];
        c.award_slots = vec![3, 4, 5];
        let equipment = |w: &WalkSim| match page(w, &feed, 0) {
            Some(SpawnPage::Class { lines, .. }) => {
                let at = lines.iter().position(|l| l == "Equipment:").unwrap();
                lines[at + 1..].to_vec()
            }
            p => panic!("{p:?}"),
        };
        assert_eq!(
            equipment(&w),
            [
                "imp_weap_inf_shotgun",
                "imp_weap_inf_pistol",
                "imp_weap_inf_rocket_launcher"
            ],
            "no medal: the class's own kit"
        );
        w.sim.cheats.all_awards = true;
        assert_eq!(
            equipment(&w),
            [
                "imp_weap_inf_rocket_launcher",
                // Our own award's own name (no table has one).
                "AWARD Blast Cannon",
                "imp_weap_award_pistol",
                "imp_weap_award_rocket_launcher"
            ],
            "every weapon medal: the awards in place of what they replace"
        );
        // A bot holds no medal, so a bot's view would be the class's own
        // kit: the medals are the player's.
        let mut bot = w.sim.units[me].clone();
        bot.is_player = false;
        let class = &w.sim.classes[team][0];
        assert!(w.sim.class_weapon_locked(&bot, class, kit[3]));
        assert!(!w.sim.class_weapon_locked(&bot, class, kit[0]));
        w.sim.cheats.all_awards = false;
        assert_eq!(equipment(&w).len(), 3, "the cheat off gives the kit back");
    }

    /// A real BF1 dark trooper: its awards keep the HUD rows of the
    /// weapons they replace, and the screen still names the awards, not
    /// those weapons. Skips without the class bakes.
    #[test]
    fn a_bf1_class_lists_its_awards_by_their_own_names() {
        let dir = crate::assets::baked_root().join("classes");
        if !dir.join("bf1_all.ron").is_file() || !dir.join("bf2_all.ron").is_file() {
            eprintln!("SKIP: class bakes absent");
            return;
        }
        let db = crate::roster::load_classdb(&dir, "bf1", &crate::roster::FACTION_ORDER)
            .expect("class db");
        let roster =
            crate::roster::ClassRoster::build(&db, "imp_inf_dark_trooper", 1, None).expect("class");
        let mut w = WalkSim::new(3, Vec3::ZERO, None);
        let feed = EventFeed::default();
        assert!(w.sim.open_spawn_screens(0, true));
        w.sim.spawn_stage[0] = core::sim::SpawnStage::Class;
        let me = w.player.index as usize;
        let team = usize::from(w.sim.units[me].team.min(1));
        let mut names = Vec::new();
        let uc = roster.register(&mut w.sim, &mut names);
        w.sim.classes[team] = vec![uc];
        w.teams[team].classes[0] = ClassVisual::from_roster_with_held(&roster);
        let equipment = |w: &WalkSim| match page(w, &feed, 0) {
            Some(SpawnPage::Class { lines, .. }) => {
                let at = lines.iter().position(|l| l == "Equipment:").unwrap();
                lines[at + 1..].to_vec()
            }
            p => panic!("{p:?}"),
        };
        let own = equipment(&w);
        assert_eq!(own[0], "imp_weap_inf_shotgun", "{own:?}");
        assert!(own.iter().any(|l| l == "imp_weap_inf_pistol"), "{own:?}");
        assert!(!own.iter().any(|l| l.contains("award")), "{own:?}");
        w.sim.cheats.all_awards = true;
        let awarded = equipment(&w);
        assert!(
            awarded.iter().any(|l| l == "AWARD Blast Cannon"),
            "{awarded:?}"
        );
        assert!(
            awarded.iter().any(|l| l == "imp_weap_award_pistol"),
            "{awarded:?}"
        );
        assert!(
            !awarded
                .iter()
                .any(|l| l == "imp_weap_inf_shotgun" || l == "imp_weap_inf_pistol"),
            "{awarded:?}"
        );
        assert_eq!(awarded.len(), own.len(), "one for one: {own:?} {awarded:?}");
    }
}
