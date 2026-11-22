//! Cross-campaign startup, audio coverage and binding-default regressions.

use super::*;

#[test]
fn every_campaign_starts_and_runs_without_script_errors() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
    let mut failures = Vec::new();
    for m in ui::campaign::RISE_OF_THE_EMPIRE {
        let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, m.script);
        for _ in 0..(20 * core::TICK_HZ) {
            tick(&mut walk, &mut rt, &def);
            rt.close_popup();
        }
        assert!(
            !rt.objectives().is_empty() || walk.sim.outcome.is_some(),
            "{} listed no objective",
            m.script
        );
        for e in rt.errors() {
            failures.push(format!("{}: {e}", m.script));
        }
    }
    assert!(
        failures.is_empty(),
        "script errors:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_campaign_voice_line_resolves_in_its_battle_sound_library() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
    // Silent on the PS2 too: no property with these hashes in any SOUND,
    // SIDE or map LVL of the disc; KAM_boba_01's is in KAM.LVL's kam1cw
    // only (the campaign loads kam1cross) and its segment is in no stream.
    let silent = [
        ("kas2c_c", "KAS_obj_02_TEMP"),
        ("kam1c_c", "KAM_boba_01"),
        ("kam1c_c", "KAM_boba_19"),
        ("kam1c_c", "imp_fly_trooptrans_takeoff"),
        ("kam1c_c", "rep_fly_gunship_takeoff"),
        ("dea1g_c", "DEA_obj_31"),
        ("hot1g_c", "rep_off_com_report_us_overwhelmed"),
    ];
    let mut missing = Vec::new();
    let mut counted = 0;
    for m in ui::campaign::RISE_OF_THE_EMPIRE {
        let def = crate::mission::MissionDef::load_campaign(&missions_dir, m.script)
            .unwrap_or_else(|e| panic!("load campaign {}: {e:#}", m.script));
        let text =
            std::fs::read_to_string(missions_dir.join(format!("campaign/bf2_{}.ron", m.script)))
                .expect("campaign script text");
        let mut lib = crate::sounds::SoundLib::load_for(&baked_dir.join(&def.map), &def)
            .unwrap_or_else(|| panic!("{}: no sound library", m.script));
        for call in ["BroadcastVoiceOver", "ScriptCB_SndPlaySound"] {
            let head = format!("Global(\"{call}\"), [Str(\"");
            for (at, _) in text.match_indices(&head) {
                let rest = &text[at + head.len()..];
                let name = &rest[..rest.find('"').expect("closing quote")];
                counted += 1;
                if silent.contains(&(m.script, name)) {
                    if lib.voice_take(name).is_some() {
                        missing.push(format!("{}: {name} is listed silent but plays", m.script));
                    }
                } else if lib.voice_take(name).is_none() {
                    missing.push(format!("{}: {call}({name}) has no take", m.script));
                } else if call == "ScriptCB_SndPlaySound"
                    && name.to_ascii_lowercase().contains("_obj_")
                    && !lib.streamed(name)
                {
                    missing.push(format!("{}: {name} is not a voice stream line", m.script));
                }
            }
        }
    }
    assert!(counted > 200, "only {counted} voice calls found");
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn every_campaign_in_game_music_resolves_in_its_battle_sound_library() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
    // Silent on the PS2 too: no record with these hashes in any LVL of the
    // disc, so FUN_00440e08 finds nothing and the handle stays empty.
    let silent = [
        ("pol1g_c", "imp_pol_amb_pol_icon_disk_retrieve"),
        ("spa1g_c", "mp_spa1_objComplete_02"),
    ];
    let mut missing = Vec::new();
    let mut counted = 0;
    for m in ui::campaign::RISE_OF_THE_EMPIRE {
        let def = crate::mission::MissionDef::load_campaign(&missions_dir, m.script)
            .unwrap_or_else(|e| panic!("load campaign {}: {e:#}", m.script));
        let text =
            std::fs::read_to_string(missions_dir.join(format!("campaign/bf2_{}.ron", m.script)))
                .expect("campaign script text");
        let lib = crate::sounds::SoundLib::load_for(&baked_dir.join(&def.map), &def)
            .unwrap_or_else(|| panic!("{}: no sound library", m.script));
        let head = "Global(\"ScriptCB_PlayInGameMusic\"), [Str(\"";
        for (at, _) in text.match_indices(head) {
            let rest = &text[at + head.len()..];
            let name = &rest[..rest.find('"').expect("closing quote")];
            counted += 1;
            let silent = silent.contains(&(m.script, name));
            if lib.music(name).is_some() == silent {
                missing.push(format!("{}: {name} (listed silent: {silent})", m.script));
            }
        }
    }
    eprintln!("{counted} in-game music calls");
    assert!(counted > 0);
    assert!(
        missing.is_empty(),
        "{}",
        missing.join(
            "
"
        )
    );
}

#[test]
fn in_game_music_replays_after_its_delay() {
    use crate::music::{BattleMusic, InGameMusic};
    let def = core::music::MusicDef {
        name: "rep_myg_act_01".into(),
        fade_in: 0.0,
        fade_out: 1.5,
        min_playback: 20.0,
        min_inactive: 15.0,
        max_playback: Some(180.0),
        priority: 1.0,
        looping: false,
        takes: vec![30.0],
    };
    let track = crate::sounds::MusicTrack {
        def,
        gain: 1.0,
        takes: Vec::new(),
        ids: Vec::new(),
    };
    let mut m = BattleMusic::new(&Default::default(), true, [100, 100]);
    m.bind_with(|_| None);
    assert_eq!(
        m.script_call(
            InGameMusic::Play {
                name: "rep_myg_act_01".into(),
                replay: 15.0
            },
            |_| Some(track.clone()),
        ),
        None
    );
    let mut starts = Vec::new();
    let dt = 0.1;
    for f in 0..1200 {
        for c in m.observe(dt, 0, [100, 100], true, None) {
            if let core::music::Change::Started { .. } = c {
                starts.push(f as f32 * dt);
            }
        }
    }
    assert!(starts.len() >= 2, "{starts:?}");
    assert!(starts[0] < 0.2, "{starts:?}");
    m.script_call(InGameMusic::Stop, |_| None);
    assert!(!m.player.musics[0].wanted);
}

/// The Naboo campaign opens its slow voice slot with `nab_objective_vo_slow`
/// and appends both sides' `*_unit_vo_slow` and `global_vo_slow` to it: an
/// objective line asked while a commander line sounds waits for it on that
/// slot (re-announcer.md §1 steps 2-4), and the quick slot stays its own.
#[test]
fn nab2g_c_objective_line_waits_for_the_commander_line_on_its_slot() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
    if !baked_dir.join("bf2_nab2/manifest.ron").is_file() {
        eprintln!("SKIP: bf2_nab2 baked assets required");
        return;
    }
    let (def, walk, rt) = setup_baked_campaign(&missions_dir, &baked_dir, "nab2g_c");
    let lib = crate::sounds::SoundLib::load_for(&baked_dir.join(&def.map), &def)
        .expect("nab2 sound library");
    let h = crate::classdb::fnv1a_lowercase;
    let slots = rt.stream_slots();
    let slow = slots[&h("nab_objective_vo_slow")];
    for s in ["global_vo_slow", "imp_unit_vo_slow", "all_unit_vo_slow"] {
        assert_eq!(slots.get(&h(s)), Some(&slow), "{s} on the slow slot");
    }
    let quick = slots[&h("imp_unit_vo_quick")];
    assert_ne!(quick, slow);
    assert_eq!(slots.get(&h("global_vo_quick")), Some(&quick));
    assert!(
        lib.stream_id("NAB_obj_22").is_some(),
        "NAB_obj_22 is a stream line"
    );

    let mut v =
        crate::announce::Commander::for_battle(&def, &crate::layout::MapLayout::default(), &walk);
    v.route_streams(slots);
    let _ = v.gate_script_sounds(&lib);
    let lines = ["imp_off_com_report_us_overwhelmed", "NAB_obj_22"];
    let mut played = Vec::new();
    let frame = |v: &mut crate::announce::Commander, played: &mut Vec<String>| {
        let _ = v.frame(&walk.sim, walk.player, core::TICK_DT, |line| {
            if lines.contains(&line.name.as_str()) {
                played.push(line.name.clone());
            }
            Some(5.0)
        });
    };
    v.queue.ask(core::announcer::Line::commander(
        "imp_off_com_report_us_overwhelmed",
    ));
    frame(&mut v, &mut played);
    assert_eq!(played, ["imp_off_com_report_us_overwhelmed"]);
    v.ask_script_sounds(vec!["NAB_obj_22".into()]);
    assert!(v.gate_script_sounds(&lib).is_empty());
    for _ in 0..(4 * core::TICK_HZ) {
        frame(&mut v, &mut played);
    }
    assert_eq!(
        played,
        ["imp_off_com_report_us_overwhelmed"],
        "nothing over the line"
    );
    for _ in 0..(2 * core::TICK_HZ) {
        frame(&mut v, &mut played);
    }
    assert_eq!(played, lines, "the objective line next");
}

/// `ScriptCB_EnableCommandPostVO(n)` sets the post-line flag to `n == 1`
/// (B2 `FUN_001a0580`), on by default.
#[test]
fn enable_command_post_vo_sets_the_flag_to_n_equals_one() {
    let Some((missions_dir, baked_dir)) = baked() else {
        return;
    };
    let (_def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "dea1g_c");
    assert!(rt.command_post_vo());
    for (n, on) in [(0.0, false), (1.0, true), (2.0, false), (1.0, true)] {
        call_script(
            &mut rt,
            &mut walk.sim,
            "ScriptCB_EnableCommandPostVO",
            vec![Value::Num(n)],
        );
        assert_eq!(rt.command_post_vo(), on, "n = {n}");
    }
}

/// Every Rise of the Empire map is in a normal pack: the ground maps in
/// rebake.sh's BF2 list and the space maps in its space stage. A pack that
/// left SPA2 out sent "Attack Coruscant" back to its own screen after the
/// movie, every time.
#[test]
fn rebake_bakes_every_campaign_map() {
    let script = include_str!("../../../../../../scripts/rebake.sh");
    let ground = script
        .lines()
        .find_map(|l| l.strip_prefix("BF2_MAPS=\""))
        .expect("rebake.sh has a default BF2_MAPS line")
        .trim_end_matches('"')
        .to_ascii_lowercase();
    let stage = script
        .split("stage \"space maps\"")
        .nth(1)
        .expect("rebake.sh has a space maps stage");
    let space = stage
        .lines()
        .find_map(|l| l.trim().strip_prefix("for n in "))
        .expect("the space stage loops over its maps")
        .to_ascii_lowercase();
    let mut missing = Vec::new();
    for m in ui::campaign::RISE_OF_THE_EMPIRE {
        let lvl = m.map.strip_prefix("bf2_").expect("campaign maps are BF2's");
        let baked = if lvl.starts_with("spa") {
            space.split_whitespace().any(|n| n == lvl)
        } else {
            ground
                .split_whitespace()
                .any(|p| p.rsplit('/').next() == Some(lvl))
        };
        if !baked {
            missing.push(m.map);
        }
    }
    assert!(missing.is_empty(), "rebake.sh does not bake {missing:?}");
}

/// A build without a mission's map says so on the battle screen before any
/// movie plays, instead of a silent return.
#[test]
fn launch_problem_names_a_map_missing_from_the_build() {
    let dir = std::env::temp_dir().join(format!("openbf-launch-problem-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let coruscant = &ui::campaign::RISE_OF_THE_EMPIRE[2];
    assert_eq!(coruscant.map, "bf2_spa2");
    let problem = crate::campaign_runtime::launch_problem(&dir, &dir, coruscant)
        .expect("no map baked is a problem");
    assert!(problem.contains("bf2_spa2"), "{problem}");
    // Map present, script absent: a script problem.
    std::fs::create_dir_all(dir.join("bf2_spa2")).unwrap();
    std::fs::write(dir.join("bf2_spa2/manifest.ron"), "()").unwrap();
    let problem = crate::campaign_runtime::launch_problem(&dir, &dir, coruscant)
        .expect("no mission script is a problem");
    assert!(problem.contains("spa2c_c"), "{problem}");
    let _ = std::fs::remove_dir_all(&dir);
}
