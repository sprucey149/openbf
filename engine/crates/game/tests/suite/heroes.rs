//! Every shipped conquest assignment must reach the actual roster builder.

#[test]
fn all_bf2_missions_append_their_authored_hero_without_changing_infantry() {
    let root = game::assets::baked_root();
    let missions = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    if !root.join("classes/bf2_rep.ron").is_file() || !missions.is_dir() {
        eprintln!("SKIP: actual BF2 classes and generated missions are required");
        return;
    }
    let mut paths: Vec<_> = std::fs::read_dir(missions)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    paths.sort();
    let mut count = 0;
    let mut names = std::collections::BTreeSet::new();
    let mut spawn_voices = std::collections::BTreeSet::new();
    let mut defeat_voices = std::collections::BTreeSet::new();
    for path in paths {
        let m = game::mission::MissionDef::load(&path).unwrap();
        if m.source_game != "bf2" || m.is_hero_assault() {
            continue;
        }
        let factions = m.factions();
        let (mut rosters, warnings) = game::roster::FactionRoster::pair(
            &root.join("classes"),
            "bf2",
            [&factions[0], &factions[1]],
            Some(&m),
        )
        .unwrap();
        for (team, roster) in rosters.iter_mut().enumerate() {
            let ordinary: Vec<_> = roster.classes.iter().map(|c| c.class.clone()).collect();
            let class = game::heroes::append(roster, &root.join("classes"), Some(&m), team)
                .unwrap_or_else(|| panic!("{} team {team}: hero append failed", path.display()));
            assert_eq!(usize::from(class), ordinary.len());
            assert_eq!(
                roster.classes[..ordinary.len()]
                    .iter()
                    .map(|c| &c.class)
                    .collect::<Vec<_>>(),
                ordinary.iter().collect::<Vec<_>>()
            );
            let authored = &m
                .heroes
                .iter()
                .find(|(t, _)| usize::from(*t) == team + 1)
                .unwrap()
                .1;
            assert!(roster.classes[usize::from(class)]
                .class
                .eq_ignore_ascii_case(authored));
            let built = &roster.classes[usize::from(class)];
            // The disc authors no spawn/defeat VO for Chewbacca. Keep
            // those optional while requiring every authored speaking hero.
            if built.cries.hero.spawned.is_some() {
                spawn_voices.insert(built.class.clone());
            }
            if built.cries.hero.defeated.is_some() {
                defeat_voices.insert(built.class.clone());
            }
            if built.class == "all_hero_chewbacca" {
                assert!(built.cries.hero.spawned.is_none());
                assert!(built.cries.hero.defeated.is_none());
            }
            assert!(built.cries.hero.music.is_some(), "{authored}: hero theme");
            if built.weapon.saber.is_some() {
                assert!(
                    built.cries.hero.saber_ambient.is_some(),
                    "{authored}: saber hum"
                );
            }
            if built.held.iter().any(|s| s.weapon.saber_throw.is_some()) {
                assert_eq!(
                    built.cries.hero.saber_flight.as_deref(),
                    Some("saber_twirl"),
                    "{authored}: authored flight sound"
                );
            }
            names.insert(authored.to_ascii_lowercase());
            count += 1;
        }
        let (_, after_warnings) = game::roster::FactionRoster::pair(
            &root.join("classes"),
            "bf2",
            [&factions[0], &factions[1]],
            Some(&m),
        )
        .unwrap();
        assert_eq!(
            warnings,
            after_warnings,
            "{}: ordinary roster warnings changed",
            path.display()
        );
    }
    assert_eq!(count, 66);
    assert_eq!(names.len(), 18);
    assert_eq!(spawn_voices.len(), 17);
    assert_eq!(defeat_voices.len(), 17);
}

/// The two combo blasts the discs author (Mace's ground crack, Sidious's
/// saber blast) carry `HurtOwner 0`: the hero's own PlayExplosion never
/// hurts or pushes him (B2 FUN_0030c8f8, B2:300612-300633). A force push's
/// blast keeps the class default (1).
#[test]
fn hero_combo_blasts_read_hurt_owner_0_from_the_disc() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_rep.ron").is_file() || !root.join("classes/bf2_imp.ron").is_file() {
        eprintln!("SKIP: actual BF2 hero classes required");
        return;
    }
    for (side, hero) in [("rep", "rep_hero_macewindu"), ("imp", "imp_hero_emperor")] {
        let db = game::roster::load_classdb(&root.join("classes"), "bf2", &[side]).unwrap();
        let roster = game::roster::ClassRoster::build(&db, hero, 0, Some(1)).unwrap();
        let mut sim = core::sim::Sim::empty(1);
        roster.register(&mut sim, &mut Vec::new());
        let blasts: Vec<_> = sim
            .weapon_classes
            .iter()
            .filter(|w| w.saber.is_some())
            .filter_map(|w| w.ordnance.explosion.as_ref())
            .collect();
        assert!(!blasts.is_empty(), "{hero}: its saber has a combo blast");
        assert!(blasts.iter().all(|e| !e.hurt_owner), "{hero}: HurtOwner 0");
    }
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
    let e = core::weapon::ExplosionClass::from_props(&game::roster::ClassSrc {
        db: &db,
        class: db.resolve("com_weap_inf_force_push_exp").unwrap(),
    });
    assert!(e.hurt_owner, "the class default is 1");
}

/// BF2 Kamino's clone-war CIS fields the droideka and its hero in one table: no
/// roll gives a bot the hero, and the droideka keeps its share.
#[test]
fn a_real_cis_table_holds_the_droideka_and_the_hero_together() {
    let root = game::assets::baked_root();
    let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
        .join("bf2_kam1_con_cw.ron");
    if !root.join("classes/bf2_cis.ron").is_file() || !path.is_file() {
        eprintln!("SKIP: actual BF2 classes and generated missions are required");
        return;
    }
    let m = game::mission::MissionDef::load(&path).unwrap();
    let factions = m.factions();
    let (mut rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        "bf2",
        [&factions[0], &factions[1]],
        Some(&m),
    )
    .unwrap();
    let mut sim = core::sim::Sim::empty(42);
    let mut names = Vec::new();
    let mut heroes = [None; 2];
    for (team, roster) in rosters.iter_mut().enumerate() {
        heroes[team] = game::heroes::append(roster, &root.join("classes"), Some(&m), team);
        sim.classes[team] = roster
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
    }
    game::heroes::install(&mut sim, heroes);
    let team = factions.iter().position(|f| f == "cis").unwrap();
    let hero = usize::from(heroes[team].expect("the mission authors a CIS hero"));
    let deka = sim.classes[team]
        .iter()
        .position(|c| c.droideka.is_some())
        .expect("the CIS table fields a droideka");
    assert_ne!(hero, deka);
    assert!(sim.classes[team][hero].droideka.is_none());
    let weights = sim.ai_class_weights(team as u8);
    assert_eq!(weights[hero], 0.0, "a bot never rolls the hero");
    assert!(
        weights[deka] > 0.0,
        "the hero does not cost the droideka its share"
    );
}
