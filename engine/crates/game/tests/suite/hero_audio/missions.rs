//! The battle's own bank priorities must retain cross-side hero sounds.

use super::*;

fn repaired_tantive_aliases(
    root: &Path,
    mission: &Path,
    map: &str,
    hero: &str,
    sound: &str,
    lib: &SoundLib,
) -> bool {
    if mission.file_name().and_then(|n| n.to_str()) != Some("bf2_tan1_con.ron")
        || map != "bf2_tan1"
        || hero != "all_hero_leia"
        || !matches!(
            sound,
            "all_inf_com_chatter_wound_female" | "all_inf_com_chatter_death_female"
        )
    {
        return false;
    }
    let aliases = [2_298_015_460, 2_314_793_079, 117_175_253];
    assert_eq!(
        lib.audio_of_name(sound),
        Some((None, vec![], aliases.to_vec(), vec![]))
    );
    let dir = root.join(map).join("sounds");
    let bank: game::sounds::SoundsRon =
        ron::from_str(&std::fs::read_to_string(dir.join("sounds.ron")).unwrap()).unwrap();
    assert_eq!(lib.level_of(sound), Some(0xd4cb_a316), "Tantive GCW bank");
    for (nth, (alias, backing)) in aliases
        .into_iter()
        .zip([2_911_796_988, 2_928_574_607, 2_945_352_226])
        .enumerate()
    {
        let (_, file) = bank.samples.iter().find(|(id, _)| *id == backing).unwrap();
        assert_eq!(
            bank.samples.iter().find(|(id, _)| *id == alias),
            Some(&(alias, file.clone())),
            "the Tantive alias must reuse its original backing WAV"
        );
        assert_eq!(
            file,
            &format!("d4cba316/{backing:08x}.wav"),
            "original same-map backing"
        );
        let expected = dir.join(file);
        assert_eq!(lib.take_path(sound, nth), Some(expected.clone()));
        let decoded = audio::decode(&expected).unwrap();
        assert_eq!(decoded.sample_rate(), [14_038, 14_047, 14_014][nth]);
        assert_eq!(decoded.frames(), [6_580, 7_420, 18_060][nth]);
        let bytes = pak::read(&expected).unwrap();
        // The baker's minimal WAV header: PCM, mono, signed 16-bit samples.
        assert_eq!(&bytes[12..20], b"fmt \x10\0\0\0");
        assert_eq!(&bytes[20..24], &[1, 0, 1, 0]);
        assert_eq!(&bytes[34..36], &16u16.to_le_bytes());
        println!(
            "hero_repaired_alias {}",
            serde_json::json!({
                "mission":mission,"hero":hero,"property":sound,"alias":alias,
                "backing_id":backing,"backing_file":expected,"frames":decoded.frames(),
                "rate":decoded.sample_rate(),"channels":1,"status":"resolved_repair"
            })
        );
    }
    true
}

#[test]
fn all_authored_heroes_resolve_from_their_actual_mission_walk_sound_levels() {
    let Some((root, _)) = content() else { return };
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    paths.sort();
    let mut assignments = 0;
    let mut cross_side = 0;
    let mut cuts = Vec::new();
    let mut repaired_aliases = Vec::new();
    let mut missing = Vec::new();
    for path in paths {
        let mission = game::mission::MissionDef::load(&path).unwrap();
        if mission.source_game != "bf2" || mission.is_hero_assault() {
            continue;
        }
        let factions = mission.factions();
        let (mut rosters, _) = FactionRoster::pair(
            &root.join("classes"),
            "bf2",
            [&factions[0], &factions[1]],
            Some(&mission),
        )
        .unwrap();
        let hero_classes = std::array::from_fn(|t| {
            game::heroes::append(&mut rosters[t], &root.join("classes"), Some(&mission), t)
        });
        let mut sim = Sim::empty(53);
        sim.ground_rules = core::soldier::GroundRules::Bf2;
        let mut names = Vec::new();
        let teams = std::array::from_fn(|t| {
            let registered: Vec<_> = rosters[t]
                .classes
                .iter()
                .map(|c| c.register(&mut sim, &mut names))
                .collect();
            sim.classes[t] = registered.clone();
            let visuals = rosters[t]
                .classes
                .iter()
                .map(ClassVisual::from_roster)
                .collect();
            TeamTemplate::from_roster(&rosters[t], &registered[0], visuals)
        });
        game::heroes::install(&mut sim, hero_classes);
        let mut walk = WalkSim::with_teams(sim, None, Vec3::ZERO, teams, None);
        walk.sound_levels = mission.sound_levels();
        let map = root.join(&mission.map);
        let mut lib =
            SoundLib::load(&map).unwrap_or_else(|| panic!("{}: missing map bank", mission.map));
        lib.for_walk(&map, &walk);
        for (team, class) in hero_classes.into_iter().enumerate() {
            let class = class.unwrap();
            let cries = &walk.teams[team].class(class).cries;
            let hero = &cries.hero;
            let name = &rosters[team].classes[usize::from(class)].class;
            cross_side += usize::from(name.split('_').next() != Some(factions[team].as_str()));
            for sound in [
                &hero.offer,
                &hero.spawned,
                &hero.defeated,
                &hero.saber_ambient,
                &hero.saber_on,
                &hero.saber_off,
                &hero.saber_flight,
                &cries.hurt,
                &cries.death,
                &cries.low_health,
            ]
            .into_iter()
            .flatten()
            {
                assert!(
                    lib.has(sound),
                    "{} {name}: sound {sound} absent from walk levels {:?}",
                    mission.map,
                    walk.sound_levels
                );
                if repaired_tantive_aliases(&root, &path, &mission.map, name, sound, &lib) {
                    repaired_aliases.push((mission.map.clone(), name.clone(), sound.clone()));
                }
                if original_cut(&lib, sound) {
                    cuts.push(sound.clone());
                } else {
                    let (_, segments, samples, layers) = lib.audio_of_name(sound).unwrap();
                    assert!(
                        layers.is_empty(),
                        "{name}: inspect layered hero property {sound}"
                    );
                    let ids = if samples.is_empty() {
                        &segments
                    } else {
                        &samples
                    };
                    assert!(!ids.is_empty(), "{name}: empty hero property {sound}");
                    for (nth, id) in ids.iter().enumerate() {
                        if let Some(path) = lib.take_path(sound, nth) {
                            assert!(audio::decode(&path).unwrap().frames() > 0);
                        } else {
                            let row = serde_json::json!({"mission":mission.map,"hero":name,"sound":sound,"take":nth,"id":id,"audio":lib.audio_of_name(sound)});
                            println!("hero_mission_missing {row}");
                            missing.push(row);
                        }
                    }
                }
            }
            let music = hero.music.as_ref().unwrap();
            let track = lib
                .music(music)
                .unwrap_or_else(|| panic!("{} {name}: no theme {music}", mission.map));
            assert!(!track.takes.is_empty());
            println!(
                "hero_mission_audio {}",
                serde_json::json!({"mission":path,"team":team,"hero":name,"levels":walk.sound_levels,"theme":music,"theme_takes":track.takes.len()})
            );
            assignments += 1;
        }
    }
    assert_eq!(assignments, 66);
    assert_eq!(cross_side, 4);
    cuts.sort();
    assert_eq!(cuts, ["hero_mundi_exhausted", "hero_mundi_spawn"]);
    repaired_aliases.sort();
    assert_eq!(
        repaired_aliases,
        [
            (
                "bf2_tan1".to_owned(),
                "all_hero_leia".to_owned(),
                "all_inf_com_chatter_death_female".to_owned()
            ),
            (
                "bf2_tan1".to_owned(),
                "all_hero_leia".to_owned(),
                "all_inf_com_chatter_wound_female".to_owned()
            ),
        ]
    );
    assert!(missing.is_empty(), "missing hero takes: {missing:?}");
}
