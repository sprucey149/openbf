use super::*;

#[test]
fn space_flag_announcements_resolve_to_audible_takes() {
    let root = crate::assets::baked_root();
    let map = root.join("bf2_spa8");
    if !map.join("sounds/sounds.ron").is_file() {
        assert!(
            std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
            "missing SPA8 sound bank"
        );
        return;
    }
    let mut lib = SoundLib::load(&map).expect("space sound bank");
    for listener in ["imp", "all"] {
        for subject in ["imp", "all"] {
            for name in [
                format!("{listener}_1Flag_take_{subject}"),
                format!("{listener}_1Flag_drop_{subject}"),
                format!("{listener}_{subject}Flag_score"),
            ] {
                let take = lib
                    .prepare_voice(&name)
                    .unwrap_or_else(|| panic!("missing {name}"));
                assert!(
                    take.path.is_file() && take.secs > 0.0 && take.gain > 0.0,
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn both_games_resolve_the_cutters_loop_to_an_audible_take() {
    let root = crate::assets::baked_root();
    for map in ["bf1_bes1", "bf2_cor1"] {
        if !root.join(map).join("sounds/sounds.ron").is_file() {
            eprintln!("SKIP: {map} sounds are not baked");
            continue;
        }
        let mut lib = SoundLib::load(&root.join(map)).expect("baked map sound bank");
        let take = lib
            .prepare_voice("com_weap_inf_fusioncutter_fire")
            .expect(map);
        assert!(take.path.is_file(), "{map}: {}", take.path.display());
        assert!(take.secs > 0.0 && take.gain > 0.0, "{map}");
    }
}

#[test]
fn both_games_resolve_the_empty_clicks_to_an_audible_take() {
    let root = crate::assets::baked_root();
    for map in ["bf1_nab1", "bf2_kam1"] {
        if !root.join(map).join("sounds/sounds.ron").is_file() {
            eprintln!("SKIP: {map} sounds are not baked");
            continue;
        }
        let mut lib = SoundLib::load(&root.join(map)).expect("baked map sound bank");
        for name in ["com_weap_inf_ammo_empty", "com_weap_inf_weapon_empty"] {
            let take = lib.prepare_voice(name).expect(name);
            assert!(take.path.is_file(), "{map}: {}", take.path.display());
            assert!(take.secs > 0.0 && take.gain > 0.0, "{map} {name}");
        }
    }
}

#[test]
fn attenuation_and_pan_shapes() {
    // The rifle fire's CORE template on BF1: 5 - 400 m, rolloff 6.
    let mut p = SoundProp {
        name_hash: 1,
        kind: 0,
        gain: 0.7,
        pitch: 1.0,
        min_distance: 5.0,
        max_distance: 400.0,
        rolloff: 6.0,
        priority: 0.85,
        looping: false,
        samples: vec![],
        layers: vec![],
        stream: None,
        segments: vec![],
        mute_distance: None,
        roll_in_distance: None,
        flat: false,
        layer_delays: vec![],
        pitch_dev: None,
        gain_dev: None,
        play_interval: None,
        play_probability: None,
        level: 0,
        bus: 0,
        take_locks: vec![],
    };
    assert_eq!(attenuation(2.0, &p), 1.0);
    assert_eq!(attenuation(5.0, &p), 1.0);
    // (395 - 45) / (45 * 60 + 395) and (395 - 148) / (148 * 60 + 395).
    assert!((attenuation(50.0, &p) - 350.0 / 3095.0).abs() < 1e-6);
    assert!((attenuation(153.0, &p) - 247.0 / 9275.0).abs() < 1e-6);
    assert_eq!(attenuation(400.0, &p), 0.0);
    assert_eq!(attenuation(900.0, &p), 0.0);
    // Falling all the way: a higher rolloff is quieter at every range.
    let mut last = 1.0;
    for d in 6..400 {
        let a = attenuation(d as f32, &p);
        assert!(a < last && a > 0.0, "{d} m: {a}");
        last = a;
    }
    // The mute distance cuts the tail off; the roll-in quietens the near
    // side ((50 - 25 inside) -> 25 / (25 * 60 + 50)); a 2-D sound is flat.
    p.mute_distance = Some(200.0);
    assert!(attenuation(199.0, &p) > 0.0);
    assert_eq!(attenuation(200.0, &p), 0.0);
    (p.min_distance, p.roll_in_distance) = (50.0, Some(50.0));
    assert!((attenuation(25.0, &p) - 25.0 / 1550.0).abs() < 1e-6);
    assert_eq!(attenuation(50.0, &p), 1.0);
    p.flat = true;
    assert_eq!(attenuation(900.0, &p), 1.0);
    let l = Listener {
        pos: Vec3::ZERO,
        yaw: 0.0,
    };
    assert_eq!(
        side(&l, Vec3::new(10.0, 0.0, 0.0)),
        1.0,
        "right of a -Z-facing listener"
    );
    assert_eq!(side(&l, Vec3::new(-10.0, 0.0, 0.0)), -1.0);
    assert!(side(&l, Vec3::new(0.0, 0.0, -10.0)).abs() < 1e-6);
    assert!((side(&l, Vec3::new(10.0, 0.0, -10.0)) - 0.5f32.sqrt()).abs() < 1e-6);
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
}

/// Card MIXH: the PS2's per-ear split of a 3-D voice (L + R = the
/// amplitude, centred = half in each ear; analysis/re-mix-headroom.md
/// 1.1 claim 4), and kira's own pan law carrying the pair exactly.
#[test]
fn a_3d_voice_splits_linearly_between_the_ears() {
    // kira 0.10.8 `Frame::panned` on a mono frame of 1 at that volume.
    let kira = |(db, pan): (f32, f32)| {
        let a = 10f32.powf(db / 20.0);
        let m = (pan + 1.0) * 0.5;
        let r2 = std::f32::consts::SQRT_2;
        (a * (1.0 - m).sqrt() * r2, a * m.sqrt() * r2)
    };
    for (s, want) in [
        (0.0, (0.4, 0.4)),
        (1.0, (0.0, 0.8)),
        (-1.0, (0.8, 0.0)),
        (0.5, (0.2, 0.6)),
        (-0.25, (0.5, 0.3)),
    ] {
        let got = ears_3d(0.8, s);
        assert!(close(got, want), "side {s}: {got:?}");
        let heard = kira(mixer_pair(got));
        assert!(close(heard, want), "side {s} through kira: {heard:?}");
    }
    // A 2-D pair is the take at its gain, centred.
    let (db, pan) = mixer_pair((0.5, 0.5));
    assert!((db - level_db(0.5)).abs() < 1e-4 && pan.abs() < 1e-6);
    assert!(close(kira((db, pan)), (0.5, 0.5)));
    // Silence stays silent.
    assert!(kira(mixer_pair((0.0, 0.0))).0 < 1e-2);
}

/// Card MIXH: what a start hands each ear - the gain x the level x its
/// BAKED bus's gain for the map's game, split for a 3-D property, full
/// both ears for a 2-D one. The reference is the PS2's own: BF1 Naboo's
/// blaster, gain 0.7012 at level 0.8603 in front of the listener, read
/// 0x134d in both ears' SPU2 registers (`lead_tmp/mixh/ps2/shot1.p2s`;
/// register = (int)(v x 32767) >> 1, 1.1 claim 1).
#[test]
fn a_start_hands_each_ear_the_ps2s_level() {
    let l = Listener {
        pos: Vec3::ZERO,
        yaw: 0.0,
    };
    let bus = fnv1a_lowercase;
    let shot = SoundProp {
        gain: 0.7012 * 0.8603,
        bus: bus("soundfx"),
        ..prop("shot", 1)
    };
    let props = vec![
        shot.clone(),
        SoundProp {
            bus: bus("voiceover"),
            ..prop("line", 2)
        },
        SoundProp {
            bus: bus("voiceover"),
            flat: true,
            ..prop("line_2d", 3)
        },
        SoundProp {
            bus: bus("battlechatter"),
            ..prop("chatter", 4)
        },
        prop("no_bus", 5),
    ];
    let mut sl = lib();
    sl.map.by_level = by_level(&props);
    sl.map.props = merge_props(props);
    let start = |sl: &mut SoundLib, name: &str, pos: Vec3| {
        let mut out = Vec::new();
        sl.start(fnv1a_lowercase(name), pos, l.pos, &mut out);
        out.pop().expect(name).ears(&l)
    };
    let ahead = Vec3::new(0.0, 0.0, -6.0);
    let right = Vec3::new(6.0, 0.0, 0.0);
    let reg = |v: f32| ((v * 32767.0) as i32 & 0xffff) >> 1;
    // BF1: soundfx 1.0, voiceover 0.64.
    let (le, ri) = start(&mut sl, "shot", ahead);
    assert_eq!((reg(le), reg(ri)), (0x134d, 0x134d), "{le} {ri}");
    assert!(close((le, ri), (0.3016, 0.3016)));
    assert!(close(start(&mut sl, "shot", right), (0.0, 0.6032)));
    assert!(close(start(&mut sl, "line", ahead), (0.32, 0.32)));
    assert!(close(start(&mut sl, "line_2d", right), (0.64, 0.64)));
    assert!(close(start(&mut sl, "no_bus", ahead), (0.5, 0.5)));
    // BF2: soundfx 0.63, battlechatter 0.9, voiceover 0.63.
    sl.buses = bus_table("bf2");
    let half = 0.7012 * 0.8603 * 0.63 * 0.5;
    assert!(close(start(&mut sl, "shot", ahead), (half, half)));
    assert!(close(start(&mut sl, "chatter", right), (0.0, 0.9)));
    assert!(close(start(&mut sl, "line_2d", ahead), (0.63, 0.63)));
    // A looping 3-D voice (an engine layer) the same way.
    let voice = |pos| crate::engine_sound::Voice {
        vehicle: 0,
        soldier: false,
        layer: 0,
        record: 0,
        sound: fnv1a_lowercase("shot"),
        pos,
        gain_factor: 1.0,
        pitch_factor: 1.0,
        speed: 0.0,
    };
    let engine = |sl: &SoundLib, pos| sl.engine_level(&voice(pos), &l).expect("in range").0;
    assert!(close(engine(&sl, ahead), (half, half)));
    assert!(close(engine(&sl, -right), (2.0 * half, 0.0)));
}

/// A menu click (bus `shellfx`) plays on the menu bus, which the pause menu
/// leaves running; a battle sound plays on the game bus it pauses.
#[test]
fn a_shellfx_start_plays_on_the_menu_bus() {
    let props = vec![
        SoundProp {
            bus: fnv1a_lowercase("shellfx"),
            ..prop("click", 1)
        },
        SoundProp {
            bus: fnv1a_lowercase("soundfx"),
            ..prop("shot", 2)
        },
    ];
    let mut sl = lib();
    sl.map.by_level = by_level(&props);
    sl.map.props = merge_props(props);
    let menu = |sl: &mut SoundLib, name: &str| {
        let mut out = Vec::new();
        sl.start(fnv1a_lowercase(name), Vec3::ZERO, Vec3::ZERO, &mut out);
        out.pop().expect(name).menu
    };
    assert!(menu(&mut sl, "click"));
    assert!(!menu(&mut sl, "shot"));
}

fn prop(name: &str, sample: u32) -> SoundProp {
    SoundProp {
        name_hash: fnv1a_lowercase(name),
        kind: 0,
        gain: 1.0,
        pitch: 1.0,
        min_distance: 30.0,
        max_distance: 450.0,
        rolloff: 1.0,
        priority: 0.5,
        looping: false,
        samples: if sample == 0 {
            vec![]
        } else {
            vec![(sample, 1.0)]
        },
        layers: vec![],
        stream: None,
        segments: vec![],
        mute_distance: None,
        roll_in_distance: None,
        flat: false,
        layer_delays: vec![],
        pitch_dev: None,
        gain_dev: None,
        play_interval: None,
        play_probability: None,
        level: 0,
        bus: 0,
        take_locks: vec![],
    }
}

fn layered(name: &str, layers: &[(&str, f32, f32)]) -> SoundProp {
    SoundProp {
        layers: layers.iter().map(|l| fnv1a_lowercase(l.0)).collect(),
        layer_delays: layers.iter().map(|l| (l.1, l.2)).collect(),
        ..prop(name, 0)
    }
}

/// The disc's records the card names (BF1 NAB.LVL; `bake::sounds`'
/// `layered_records_keep_what_the_ps2_parser_keeps` pins them there).
fn lib() -> SoundLib {
    let props = vec![
        prop("exp_high", 1),
        prop("exp_low", 2),
        layered(
            "com_weap_inf_ordnance_exp",
            &[
                ("exp_high", 0.0, 0.0),
                ("exp_high", 0.1, 0.0),
                ("exp_high", 0.2, 0.0),
                ("exp_low", 0.0, 0.0),
            ],
        ),
        prop("near", 3),
        prop("distant", 4),
        // The far layer's own gate on the disc.
        SoundProp {
            play_interval: Some((0.5, 0.0)),
            ..prop("far", 5)
        },
        layered(
            "com_weap_obj_med_exp",
            &[
                ("near", 0.0, 0.0),
                ("near", 0.3, 0.1),
                ("distant", 0.3, 0.1),
                ("far", 0.5, 0.1),
            ],
        ),
        SoundProp {
            pitch_dev: Some(0.15),
            gain: 0.5,
            gain_dev: Some(0.2),
            ..prop("com_weap_thermaldetonator_exp", 6)
        },
        SoundProp {
            play_probability: Some(0.25),
            ..prop("rare", 7)
        },
        // Card UCALL's unit lines: a property gate, a roll that always
        // refuses, a take with its own lock.
        SoundProp {
            play_interval: Some((20.0, 0.0)),
            ..prop("call_gated", 8)
        },
        SoundProp {
            play_interval: Some((10.0, 0.0)),
            play_probability: Some(0.0),
            ..prop("call_never", 9)
        },
        SoundProp {
            take_locks: vec![(25.0, 0.0)],
            ..prop("call_locked", 10)
        },
        // Card UVO2: three takes, each locked 10 s once drawn.
        SoundProp {
            samples: vec![(7, 1.0), (8, 1.0), (9, 1.0)],
            take_locks: vec![(10.0, 0.0); 3],
            ..prop("vo_three", 0)
        },
        // Card UTALK's low-health voice: no interval, one take locked 1 s.
        SoundProp {
            take_locks: vec![(1.0, 0.0)],
            stream: Some(1),
            ..prop("low_voice", 10)
        },
    ];
    let samples = (1..=10).map(|i| (i, format!("b/{i}.wav"))).collect();
    SoundLib {
        timers: Vec::new(),
        now: 0.0,
        menu_now: 0.0,
        next_ok: HashMap::new(),
        take_ok: HashMap::new(),
        map: Bank {
            dir: PathBuf::from("sounds"),
            by_level: by_level(&props),
            props: merge_props(props),
            sample_paths: samples,
            engines: HashMap::new(),
            music: HashMap::new(),
            triggers: Vec::new(),
        },
        levels: Vec::new(),
        foreign: Vec::new(),
        core: None,
        global_dir: PathBuf::new(),
        global_files: HashMap::new(),
        map_streams_dir: PathBuf::new(),
        map_stream_files: HashMap::new(),
        indexed: HashMap::new(),
        music_takes: HashMap::new(),
        cache: HashMap::new(),
        loops: HashMap::new(),
        ambient: HashMap::new(),
        cries: HashMap::new(),
        boosts: HashMap::new(),
        weapon_handles: HashMap::new(),
        station_voices: HashMap::new(),
        hero_voices: HashMap::new(),
        hero_blades: Vec::new(),
        seed: 0x1234_5678,
        played: 0,
        handed: None,
        rate_mul: 1.0,
        buses: bus_table("bf1"),
        effects_volume: 1.0,
        dialogue_volume: 1.0,
    }
}

const DT: f32 = 1.0 / 60.0;

/// Start `name` and run the clock for `secs`: (seconds after the start,
/// what started), the times on the tick grid.
fn play(sl: &mut SoundLib, name: &str, secs: f32) -> Vec<(f32, Start)> {
    let mut starts = Vec::new();
    let mut out = Vec::new();
    sl.start(fnv1a_lowercase(name), Vec3::ZERO, Vec3::ZERO, &mut starts);
    out.extend(starts.drain(..).map(|s| (0.0, s)));
    for k in 0..(secs / DT).round() as u32 {
        sl.tick(DT, Vec3::ZERO, &mut starts);
        out.extend(starts.drain(..).map(|s| (k as f32 * DT, s)));
    }
    out
}

/// Card WSND: a layer of delay 0 starts with the record, any other at its
/// delay (the PS2's grenade explosion: the second layer +0.100 s in 11 of
/// 11, re-weapon-sounds.md §2.5).
#[test]
fn a_layered_sound_starts_each_layer_at_its_delay() {
    let mut sl = lib();
    let got = play(&mut sl, "com_weap_inf_ordnance_exp", 1.0);
    let at: Vec<(u32, &Path)> = got
        .iter()
        .map(|(t, s)| ((t * 60.0).round() as u32, s.path.as_path()))
        .collect();
    assert_eq!(
        at,
        [
            (0, Path::new("sounds/b/1.wav")),
            (0, Path::new("sounds/b/2.wav")),
            (6, Path::new("sounds/b/1.wav")),
            (12, Path::new("sounds/b/1.wav"))
        ]
    );
    assert_eq!(sl.waiting(), 0);
}

/// A delay's deviation: delay + deviation x U(-1, 1), drawn per start
/// (PS2 logs: 0.234 - 0.400 s for 0.3 +- 0.1, 0.467 - 0.600 for 0.5 +-
/// 0.1).
#[test]
fn delayed_layers_fall_inside_their_deviation_and_cover_it() {
    let mut sl = lib();
    // near (delay 0.3), distant, far: the seen range of each.
    let mut seen = [(f32::MAX, f32::MIN); 3];
    for _ in 0..1000 {
        let got = play(&mut sl, "com_weap_obj_med_exp", 1.0);
        assert_eq!(got.len(), 4, "{got:?}");
        assert_eq!((got[0].0, got[0].1.hash), (0.0, fnv1a_lowercase("near")));
        for (t, s) in &got[1..] {
            let i = ["near", "distant", "far"]
                .iter()
                .position(|n| fnv1a_lowercase(n) == s.hash)
                .unwrap();
            seen[i] = (seen[i].0.min(*t), seen[i].1.max(*t));
        }
    }
    // On the tick grid: the first update at or after the drawn time.
    for (i, (lo, hi)) in [(0.2, 0.4), (0.2, 0.4), (0.4, 0.6)].into_iter().enumerate() {
        let (a, b) = seen[i];
        assert!(a >= lo - 1e-4 && b <= hi + DT + 1e-4, "{i}: {a}..{b}");
        assert!(a < lo + 0.03 && b > hi - 0.03, "{i}: {a}..{b} covers");
    }
}

/// Every start draws its own pitch and gain: +- `PitchDev` / `GainDev`
/// (the explosions' `PitchDev` 0.15).
#[test]
fn each_start_draws_its_pitch_and_gain() {
    let mut sl = lib();
    let (mut p, mut g) = ((f32::MAX, f32::MIN), (f32::MAX, f32::MIN));
    for _ in 0..1000 {
        let got = play(&mut sl, "com_weap_thermaldetonator_exp", 0.0);
        let s = &got[0].1;
        p = (p.0.min(s.pitch), p.1.max(s.pitch));
        g = (g.0.min(s.gain), g.1.max(s.gain));
    }
    assert!(
        p.0 >= 0.85 && p.0 < 0.86 && p.1 <= 1.15 && p.1 > 1.14,
        "{p:?}"
    );
    assert!(
        g.0 >= 0.3 && g.0 < 0.31 && g.1 <= 0.7 && g.1 > 0.69,
        "{g:?}"
    );
}

/// `PlayInterval` refuses a start inside the interval; `PlayProbability`
/// takes its share of the others.
#[test]
fn a_unit_line_is_gated_by_its_take_then_its_property() {
    let mut sl = lib();
    let near = Vec3::new(0.0, 0.0, 40.0);
    let ask = |sl: &mut SoundLib, n: &str, ears: Vec3| sl.ask_line(n, Vec3::ZERO, ears);
    let wait = |sl: &mut SoundLib, secs: f32| sl.tick(secs, Vec3::ZERO, &mut Vec::new());
    // The property's interval: a second ask inside it is silent.
    let line = ask(&mut sl, "call_gated", near).expect("the first ask");
    assert_eq!(
        (line.take, line.at, line.mode_3d),
        (Some(0), Some(Vec3::ZERO), true)
    );
    assert_eq!(ask(&mut sl, "call_gated", near), Err(Refused::Interval));
    wait(&mut sl, 19.0);
    assert_eq!(ask(&mut sl, "call_gated", near), Err(Refused::Interval));
    wait(&mut sl, 1.5);
    assert!(
        ask(&mut sl, "call_gated", near).is_ok(),
        "the interval is over"
    );
    // A refused roll still spends the interval.
    assert_eq!(ask(&mut sl, "call_never", near), Err(Refused::Probability));
    assert_eq!(ask(&mut sl, "call_never", near), Err(Refused::Interval));
    // A take inside its own lock silences the request.
    assert!(ask(&mut sl, "call_locked", near).is_ok());
    assert_eq!(ask(&mut sl, "call_locked", near), Err(Refused::TakeLocked));
    wait(&mut sl, 25.5);
    assert!(
        ask(&mut sl, "call_locked", near).is_ok(),
        "the lock is over"
    );
    // A speaker past the mute distance is not queued - and has spent
    // the gate (the PS2 rolls it before the level).
    wait(&mut sl, 21.0);
    let far = Vec3::new(0.0, 0.0, 460.0);
    assert_eq!(ask(&mut sl, "call_gated", far), Err(Refused::Far));
    assert_eq!(ask(&mut sl, "call_gated", near), Err(Refused::Interval));
    // A line at full level stays under a commander line's key.
    wait(&mut sl, 21.0);
    let l = ask(&mut sl, "call_gated", Vec3::ZERO).expect("at the ears");
    assert!(
        l.key > 0.9 && l.key < core::announcer::COMMANDER_KEY,
        "{}",
        l.key
    );
    assert_eq!(ask(&mut sl, "no_such_line", near), Err(Refused::Unknown));
}

/// Card UVO2, BF2's take rule (B2 `FUN_0055bf60` 693234-693238): a
/// drawn take inside its lock gives way to the next open one, so three
/// asks in a row say the three takes and only the fourth is silent
/// (BF1's one draw: `a_unit_line_is_gated_by_its_take_then_its_property`).
#[test]
fn a_bf2_line_steps_past_a_locked_take() {
    let mut sl = lib();
    let ask =
        |sl: &mut SoundLib, step: bool| sl.ask_line_with("vo_three", Vec3::ZERO, Vec3::ZERO, step);
    let takes: Vec<_> = (0..3)
        .map(|_| ask(&mut sl, true).expect("an open take").take)
        .collect();
    let mut seen = takes.clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 3, "{takes:?}");
    assert_eq!(ask(&mut sl, true), Err(Refused::TakeLocked));
}

/// Card UTALK (re-unit-chatter.md 2.8): a living unit whose class's
/// `LowHealthThreshold` is 1.1 asks its `LowHealthSound` at full health,
/// never again while its own line waits or plays, and again once it has
/// ended (its take's 1 s lock over); dead, its waiting line is dropped.
/// A class with the default threshold (0.3) is silent above 30 % health
/// and asks under it.
#[test]
fn a_low_health_voice_asks_again_only_when_its_line_has_ended() {
    use crate::announce::Commander;
    use core::sim::Call;
    let mut sl = lib();
    let mut w = crate::walk::WalkSim::new(1, Vec3::ZERO, None);
    let set = |w: &mut crate::walk::WalkSim, threshold: f32| {
        for t in &mut w.teams {
            for c in &mut t.classes {
                c.cries.low_health = Some("low_voice".into());
                c.cries.low_threshold = threshold;
            }
        }
    };
    set(&mut w, 1.1);
    let me = w.player.index;
    let mut v = Commander::new(
        core::announcer::BattleLines::default(),
        ["rep", "cis"],
        0,
        Vec::new(),
        Vec::new(),
    );
    // One tick: the asks, the gates, the queue's frame (the line is 1 s).
    let tick = |v: &mut Commander, sl: &mut SoundLib, w: &crate::walk::WalkSim| {
        v.calls(&w.sim, &w.teams);
        let queued = v
            .gate_calls(sl, Vec3::ZERO)
            .into_iter()
            .filter(|(a, r)| a.unit == me && a.call == Call::LowHealth && r.is_ok())
            .count();
        let started = v.queue.step(1.0 / 30.0, |_| Some(1.0)).is_some();
        sl.tick(1.0 / 30.0, Vec3::ZERO, &mut Vec::new());
        (queued, started)
    };
    assert_eq!(tick(&mut v, &mut sl, &w), (1, true), "asked at full health");
    // While it plays (1 s and the 0.45 s gap): no ask touches a gate.
    for k in 0..40 {
        assert_eq!(tick(&mut v, &mut sl, &w), (0, false), "tick {k}: it sounds");
    }
    let again = (0..10).find(|_| tick(&mut v, &mut sl, &w).0 == 1);
    assert!(again.is_some(), "asked again once the line has ended");
    // A waiting line of a unit that dies is dropped.
    v.queue.playing(5.0);
    let _ = tick(&mut v, &mut sl, &w);
    assert!(v.queue.holds(me));
    w.sim.units[me as usize].alive = false;
    let _ = tick(&mut v, &mut sl, &w);
    assert!(!v.queue.holds(me), "dropped at its death");
    // The default threshold: silent at full health, asked under 30 %.
    let mut sl = lib();
    let mut w = crate::walk::WalkSim::new(1, Vec3::ZERO, None);
    set(&mut w, crate::walk::LOW_HEALTH_THRESHOLD);
    let mut v = Commander::new(
        core::announcer::BattleLines::default(),
        ["rep", "cis"],
        0,
        Vec::new(),
        Vec::new(),
    );
    for _ in 0..60 {
        assert_eq!(tick(&mut v, &mut sl, &w).0, 0, "full health");
    }
    let h = &mut w.sim.units[me as usize].health;
    h.cur = 0.25 * h.max;
    assert_eq!(tick(&mut v, &mut sl, &w).0, 1, "under 30 %");
    // A sample-bank low-health sound (a saber's hum) is not the voice
    // queue's.
    for t in &mut w.teams {
        for c in &mut t.classes {
            c.cries.low_health = Some("rare".into());
        }
    }
    for _ in 0..60 {
        assert_eq!(tick(&mut v, &mut sl, &w).0, 0, "not on a stream");
    }
}

#[test]
fn the_interval_and_the_probability_gate_a_start() {
    let mut sl = lib();
    // Four starts 0.25 s apart: the 0.5 s interval lets every other in.
    let n: usize = (0..4).map(|_| play(&mut sl, "far", 0.25).len()).sum();
    assert_eq!(n, 2);
    let n: usize = (0..4000).map(|_| play(&mut sl, "rare", 0.0).len()).sum();
    assert!((900..1100).contains(&n), "{n} of 4000 at 0.25");
}

/// The pool holds 78 waiting layers; a full pool drops a new one that
/// outranks none.
#[test]
fn the_timer_pool_is_bounded() {
    let mut sl = lib();
    let mut out = Vec::new();
    for _ in 0..40 {
        let h = fnv1a_lowercase("com_weap_obj_med_exp");
        sl.start(h, Vec3::ZERO, Vec3::ZERO, &mut out);
    }
    assert_eq!(sl.waiting(), LAYER_TIMERS);
}

/// Card SNDE: a name's definition is the battle's own sub-level's, in
/// the order of its sub-levels; a name they do not define falls through
/// to the LVL's last audible one.
#[test]
fn a_name_resolves_in_the_battles_sub_level_first() {
    let at = |name: &str, level: u32, sample: u32| SoundProp {
        level,
        ..prop(name, sample)
    };
    let mut sl = lib();
    let records = vec![
        at("post_line", 10, 1),
        at("post_line", 20, 2),
        at("other_army", 20, 3),
        // Silent in the battle's own sub-level: the PS2 holds only that.
        at("quiet_here", 10, 0),
        at("quiet_here", 20, 4),
    ];
    sl.map.by_level = by_level(&records);
    sl.map.props = merge_props(records);
    let sample = |sl: &SoundLib, name: &str| {
        let (_, _, samples, _) = sl.audio_of(fnv1a_lowercase(name)).expect(name);
        samples
    };
    // No battle: the whole LVL's last audible definition (the old rule).
    assert_eq!(sample(&sl, "post_line"), [2]);
    sl.set_levels(vec![10]);
    assert_eq!(sample(&sl, "post_line"), [1]);
    assert_eq!(sl.level_of("post_line"), Some(10));
    assert_eq!(sample(&sl, "other_army"), [3], "falls through");
    assert_eq!(sample(&sl, "quiet_here"), [] as [u32; 0]);
    sl.set_levels(vec![30, 20, 10]);
    assert_eq!(sample(&sl, "post_line"), [2], "the first that defines it");
    // An army of the other era: its own era's sub-level, by its prefix.
    sl.set_levels(vec![10]);
    sl.foreign = vec![("post_".to_owned(), 20)];
    assert_eq!(sl.level_of("Post_line"), Some(20));
    assert_eq!(sl.level_of("quiet_here"), Some(10));
    let kas2 = Path::new("x/bf1_kas2");
    let named = ["kas2gcw".to_owned()];
    let foreign = crate::foley::foreign_armies(kas2, ["rep", "imp"], &named);
    assert_eq!(foreign, [("rep_".to_owned(), fnv1a_lowercase("kas2cw"))]);
    assert!(crate::foley::foreign_armies(kas2, ["all", "imp"], &named).is_empty());
    assert!(crate::foley::foreign_armies(kas2, ["rep", "imp"], &[]).is_empty());
}

/// A duplicated property whose LAST definition is silent must not silence
/// the sound: the discs define one property per sublevel and 63 properties
/// across the 32 baked maps pair an audible definition with an empty one.
/// A streamed definition counts as audible — it names takes.
#[test]
fn duplicate_props_keep_the_audible_definition() {
    let ron = r#"(
        source: "test",
        banks: [],
        samples: [(1, "aa/bb.wav")],
        props: [
            (name_hash: 7, kind: 0, gain: 1.0, pitch: 1.0, min_distance: 5.0,
             max_distance: 50.0, rolloff: 1.0, priority: 0.5, looping: true,
             samples: [(1, 1.0)], layers: []),
            (name_hash: 7, kind: 0, gain: 1.0, pitch: 1.0, min_distance: 5.0,
             max_distance: 50.0, rolloff: 1.0, priority: 0.5, looping: false,
             samples: [], layers: []),
            (name_hash: 8, kind: 0, gain: 1.0, pitch: 1.0, min_distance: 5.0,
             max_distance: 50.0, rolloff: 1.0, priority: 0.5, looping: false,
             samples: [], layers: [], stream: Some(9), segments: [(3, 1.0)]),
            (name_hash: 8, kind: 0, gain: 1.0, pitch: 1.0, min_distance: 5.0,
             max_distance: 50.0, rolloff: 1.0, priority: 0.5, looping: false,
             samples: [], layers: []),
        ],
    )"#;
    let parsed: SoundsRon = ron::from_str(ron).expect("ron");
    // The merge `Bank::load` runs, not a copy of it.
    let props = merge_props(parsed.props);
    assert_eq!(props[&7].samples.len(), 1, "the silent duplicate won");
    assert_eq!(props[&8].segments.len(), 1, "the silent duplicate won");
    // A pre-item-15 RON (no `stream`/`segments` at all) still parses.
    assert_eq!(props[&7].stream, None);
    assert!(props[&7].segments.is_empty());
}

/// A 1 s silent mono WAV at `path` (a take to time a cry by).
fn one_second_wav(path: &Path) {
    let rate = 8000u32;
    let data = vec![0u8; rate as usize * 2];
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&data);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b).unwrap();
}

/// Card UCRY: ONE voice per unit for its cries (B1 entity +0xec, B2
/// +0x25c; `analysis/re-unit-chatter.md` 2.7) - a second hurt cuts the
/// first, death cuts a sounding hurt, another unit's cry cuts nothing, a
/// cry its draw refuses (or a class without the key) still cuts, and a
/// finished cry leaves nothing to cut.
#[test]
fn a_unit_has_one_voice_for_its_cries() {
    let dir = std::env::temp_dir().join(format!("openbf-ucry-{}", std::process::id()));
    let mut l = lib();
    l.map.dir = dir.clone();
    for i in [1, 2, 7] {
        one_second_wav(&dir.join(format!("b/{i}.wav")));
    }
    let (hurt, death, rare) = (
        fnv1a_lowercase("exp_high"),
        fnv1a_lowercase("exp_low"),
        fnv1a_lowercase("rare"),
    );
    let (pos, ears) = (Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO);
    let mut out = Vec::new();
    assert!(!l.cry(3, hurt, pos, ears, &mut out), "nothing to cut yet");
    assert_eq!(out.len(), 1);
    assert!(l.crying(3));
    // Another unit's cry leaves unit 3's sounding.
    assert!(!l.cry(4, hurt, pos, ears, &mut out));
    assert!(l.crying(3) && l.crying(4));
    // Half a second on, a second hurt cuts the first; then death cuts it.
    l.tick(0.5, ears, &mut out);
    assert!(
        l.cry(3, hurt, pos, ears, &mut out),
        "a new hurt cuts the last"
    );
    l.tick(0.5, ears, &mut out);
    assert!(l.cry(3, death, pos, ears, &mut out), "death cuts hurt");
    assert_eq!(out.len(), 4);
    assert!(l.crying(3));
    // The take is 1 s: after it, nothing is cut.
    l.tick(1.01, ears, &mut out);
    assert!(!l.crying(3));
    assert!(!l.cry(3, hurt, pos, ears, &mut out));
    // A class without the key cuts and starts nothing.
    let n = out.len();
    assert!(l.cry(3, 0, pos, ears, &mut out));
    assert_eq!(out.len(), n);
    assert!(!l.crying(3));
    // A refused draw (`rare`: chance 0.25) still cuts the sounding cry.
    let mut refused = 0;
    for _ in 0..40 {
        l.cry(5, hurt, pos, ears, &mut out);
        let n = out.len();
        assert!(
            l.cry(5, rare, pos, ears, &mut out),
            "the cut comes before the draw"
        );
        if out.len() == n {
            refused += 1;
            assert!(!l.crying(5));
        }
    }
    assert!(refused > 10, "{refused}");
    // One tick's asks: per unit only the last one's start (it cut the
    // others as they began); another unit's stands; a last ask with no
    // key leaves its unit silent.
    let asks = [
        (7, hurt, pos),
        (8, hurt, pos),
        (7, hurt, pos),
        (7, death, pos),
        (8, 0, pos),
    ];
    let got = l.cries(&asks, ears);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!((got[0].0, got[0].1.hash), (7, death));
    assert!(l.crying(7) && !l.crying(8));
    let got = l.cries(&[(9, hurt, pos), (10, death, pos)], ears);
    assert_eq!(got.iter().map(|g| g.0).collect::<Vec<_>>(), vec![9, 10]);
    let _ = std::fs::remove_dir_all(dir);
}

/// The windowed path's mixer: each voice on the handle keeps the shot that
/// cuts it, so a click's cut reaches the sounding take.
#[test]
#[ignore = "requires an explicitly configured audio output, e.g. null ALSA"]
fn weapon_voices_hold_their_mixer_shots_through_a_click() {
    let dir = std::env::temp_dir().join(format!("openbf-empty-mix-{}", std::process::id()));
    let mut l = lib();
    l.map.dir = dir.clone();
    for i in [1, 2] {
        one_second_wav(&dir.join(format!("b/{i}.wav")));
    }
    let mut audio = audio::Audio::new().expect("configured output required; never skip");
    let listener = Listener {
        pos: Vec3::ZERO,
        yaw: 0.0,
    };
    let ask = |name: &str, click| WeaponAsk {
        unit: 1,
        weapon_class: 5,
        hash: fnv1a_lowercase(name),
        pos: Vec3::new(10.0, 0.0, 0.0),
        click,
    };
    let held = |l: &SoundLib| {
        let v = &l.weapon_handles[&(1, 5)];
        (v.len(), v.iter().filter(|c| c.shot.is_some()).count())
    };
    l.play_weapon_voices(&mut audio, &[ask("exp_high", false)], &listener);
    assert_eq!(held(&l), (1, 1));
    l.play_weapon_voices(&mut audio, &[ask("exp_high", false)], &listener);
    assert_eq!(held(&l), (2, 2));
    l.play_weapon_voices(&mut audio, &[ask("exp_low", true)], &listener);
    assert_eq!(held(&l), (1, 1), "the click alone is left on the handle");
    assert_eq!(l.played, 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_click_cuts_its_weapons_handle_and_mashing_keeps_one_voice() {
    let dir = std::env::temp_dir().join(format!("openbf-empty-{}", std::process::id()));
    let mut l = lib();
    l.map.dir = dir.clone();
    for i in [1, 2, 7, 8] {
        one_second_wav(&dir.join(format!("b/{i}.wav")));
    }
    let ears = Vec3::ZERO;
    let ask = |unit, weapon_class, name: &str, click| WeaponAsk {
        unit,
        weapon_class,
        hash: fnv1a_lowercase(name),
        pos: Vec3::new(10.0, 0.0, 0.0),
        click,
    };
    let starts = |v: &[(Option<(u32, u32)>, Start)]| v.len();
    let fire = ask(1, 5, "exp_high", false);
    let click = ask(1, 5, "exp_low", true);

    // Shots do not cut each other, and another handle's tail is its own.
    assert_eq!(starts(&l.weapon_voices(&[fire, fire], ears)), 2);
    assert_eq!(l.weapon_handle_voices(1, 5), 2);
    assert_eq!(
        starts(&l.weapon_voices(&[ask(2, 5, "exp_high", false)], ears)),
        1
    );

    // A click stops the handle's tail, and only that handle's.
    assert_eq!(starts(&l.weapon_voices(&[click], ears)), 1);
    assert_eq!(l.weapon_handle_voices(1, 5), 1);
    assert_eq!(l.weapon_handle_voices(2, 5), 1);

    // Mashed for a tick at a time: the handle never holds two clicks.
    let mut sink = Vec::new();
    for _ in 0..30 {
        l.weapon_voices(&[click], ears);
        assert_eq!(l.weapon_handle_voices(1, 5), 1);
        l.tick(1.0 / 30.0, ears, &mut sink);
    }

    // A tick's own earlier start on the handle is never heard.
    let got = l.weapon_voices(&[fire, click], ears);
    assert_eq!(got.len(), 1, "the shot before the click is dropped");
    assert_eq!(l.weapon_handle_voices(1, 5), 1);

    // A click its gate refuses starts nothing and so cuts nothing.
    let refused = ask(1, 5, "call_never", true);
    assert_eq!(starts(&l.weapon_voices(&[refused], ears)), 0);
    assert_eq!(l.weapon_handle_voices(1, 5), 1);

    // A finished take leaves nothing on the handle, and a layered sound
    // holds no handle to cut.
    for _ in 0..40 {
        l.tick(1.0 / 30.0, ears, &mut sink);
    }
    assert!(l.weapon_voices(&[], ears).is_empty());
    assert_eq!(l.weapon_handle_voices(1, 5), 0);
    let got = l.weapon_voices(&[ask(1, 5, "com_weap_obj_med_exp", false)], ears);
    assert!(got.iter().all(|(k, _)| k.is_none()), "layered: no handle");
    assert_eq!(l.weapon_handle_voices(1, 5), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Card PWST: a station's `AmbientSound` holds ONE handle (re-ambience-
/// music §5.2) - asked every tick while it is not sounding, through the
/// property's draw; a sounding take blocks every ask of its station until
/// it ends, another station's handle is its own, a listener out of range
/// starts nothing.
#[test]
fn a_flyers_boost_handle_lets_a_rise_replace_and_a_fall_wait() {
    let dir = std::env::temp_dir().join(format!("openbf-boost-{}", std::process::id()));
    let mut l = lib();
    l.map.dir = dir.clone();
    one_second_wav(&dir.join("b/1.wav"));
    let (pos, ears) = (Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO);
    assert!(l.boost(3, "exp_high", true, pos, ears).is_some());
    assert!(l.boost(3, "exp_high", false, pos, ears).is_none());
    assert!(l.boost(4, "exp_high", false, pos, ears).is_some());
    assert!(l.boost(3, "exp_high", true, pos, ears).is_some());
    l.tick(1.01, ears, &mut Vec::new());
    assert!(l.boost(3, "exp_high", false, pos, ears).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_station_has_one_ambient_handle() {
    let dir = std::env::temp_dir().join(format!("openbf-pwst-{}", std::process::id()));
    let mut l = lib();
    l.map.dir = dir.clone();
    for i in [1, 7] {
        one_second_wav(&dir.join(format!("b/{i}.wav")));
    }
    let (sure, rare) = (fnv1a_lowercase("exp_high"), fnv1a_lowercase("rare"));
    let (pos, ears) = (Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO);
    let dt = 1.0 / 30.0;
    let mut sink = Vec::new();
    // A sure draw starts at once, then holds the handle for its 1 s take.
    let mut starts = Vec::new();
    for _ in 0..60 {
        starts.push(
            l.station_ambient(&[(0, sure, pos), (1, sure, pos)], ears)
                .len(),
        );
        l.tick(dt, ears, &mut sink);
    }
    assert_eq!(starts[0], 2, "both stations start on the first ask");
    assert!(starts[1..30].iter().all(|n| *n == 0), "{starts:?}");
    assert_eq!(starts[30..32].iter().sum::<usize>(), 2, "{starts:?}");
    // A 0.25 draw: once the take ends the asks resume, so the gap past
    // the take is about 1 / 0.25 ticks.
    let (mut n, mut ticks) = (0, 0);
    while n < 200 {
        n += l.station_ambient(&[(2, rare, pos)], ears).len();
        l.tick(dt, ears, &mut sink);
        ticks += 1;
    }
    let gap = ticks as f32 / n as f32 - 30.0;
    assert!(
        (2.5..6.0).contains(&gap),
        "ticks per start past the take {gap}"
    );
    // Out of range (the property mutes from 450 m): no start, no hold.
    let far = Vec3::new(500.0, 0.0, 0.0);
    assert!(l.station_ambient(&[(3, sure, far)], ears).is_empty());
    assert_eq!(l.station_ambient(&[(3, sure, pos)], ears).len(), 1);
    // A slowed positional take keeps its station for the actual mixer
    // duration. The rate is fixed when the take starts.
    l.rate_mul = 0.3;
    assert_eq!(l.station_ambient(&[(4, sure, pos)], ears).len(), 1);
    l.tick(1.1, ears, &mut sink);
    assert!(l.station_ambient(&[(4, sure, pos)], ears).is_empty());
    l.tick(2.3, ears, &mut sink);
    assert_eq!(l.station_ambient(&[(4, sure, pos)], ears).len(), 1);
    // At a low base pitch the mixer floors the product, not its factors.
    l.map.props.get_mut(&sure).unwrap().pitch = 0.01;
    assert_eq!(l.station_ambient(&[(5, sure, pos)], ears).len(), 1);
    l.tick(19.9, ears, &mut sink);
    assert!(l.station_ambient(&[(5, sure, pos)], ears).is_empty());
    l.tick(0.2, ears, &mut sink);
    assert_eq!(l.station_ambient(&[(5, sure, pos)], ears).len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

// Generated banks exercise the runtime parser and decoder without game content.
struct VoiceFixture(PathBuf);
impl VoiceFixture {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir()
            .join(format!("openbf-voicep-{}-{tag}", std::process::id()))
            .join("bf1_kas2");
        let mut foreign = SoundProp {
            level: fnv1a_lowercase("kas2cw"),
            pitch: 1.6,
            gain: 0.5,
            bus: fnv1a_lowercase("voiceover"),
            ..prop("rep_test_voice", 2)
        };
        foreign.samples.push((3, 3.0));
        let props = vec![
            SoundProp {
                level: fnv1a_lowercase("kas2gcw"),
                pitch: 0.8,
                gain: 0.9,
                bus: fnv1a_lowercase("soundfx"),
                ..prop("rep_test_voice", 1)
            },
            foreign,
            SoundProp {
                level: fnv1a_lowercase("kas2gcw"),
                ..prop("rep_fallback", 1)
            },
            prop("missing_file", 4),
        ];
        for i in [1, 2, 3] {
            one_second_wav(&dir.join(format!("sounds/b/{i}.wav")));
        }
        let bank = SoundsRon {
            source: "generated VOICEP fixture".into(),
            banks: vec![],
            samples: (1..=4).map(|i| (i, format!("b/{i}.wav"))).collect(),
            props,
            odd_frequency: 0,
            streamed: 0,
            engines: vec![],
            music: vec![],
            triggers: vec![],
        };
        std::fs::write(
            dir.join("sounds/sounds.ron"),
            ron::to_string(&bank).unwrap(),
        )
        .unwrap();
        Self(dir)
    }
    fn load(&self) -> SoundLib {
        let mut sl = SoundLib::load(&self.0).expect("generated bank");
        sl.for_battle(&self.0, ["rep", "imp"], &["kas2gcw".into()]);
        sl.handed = Some(Vec::new());
        sl
    }
}
impl Drop for VoiceFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

#[test]
fn foreign_voice_uses_selected_pitch() {
    let f = VoiceFixture::new("pitch");
    let h = f.load().prepare_voice("ReP_TeSt_VoIcE").unwrap();
    near(h.pitch, 1.6);
    near(h.secs, 0.625);
}
#[test]
fn foreign_voice_uses_selected_bus() {
    let f = VoiceFixture::new("bus");
    let h = f.load().prepare_voice("rep_test_voice").unwrap();
    near(h.gain, 0.5);
    assert!(close(h.ears, (0.32, 0.32)), "{:?}", h.ears);
}
#[test]
fn commander_draws_once_and_keeps_weighted_take_and_duration() {
    let f = VoiceFixture::new("draw");
    let (mut sl, mut reference) = (f.load(), f.load());
    sl.seed = 1;
    reference.seed = 1;
    let mut counts = [0; 2];
    for _ in 0..64 {
        let expected = reference.voice_take("rep_test_voice").unwrap();
        let h = sl.prepare_voice("rep_test_voice").unwrap();
        assert_eq!(h.path, expected.0);
        assert_eq!(sl.seed, reference.seed, "exactly one weighted draw");
        near(h.gain, expected.1);
        near(h.secs, expected.2);
        near(h.secs, sl.cache[&h.path].duration_secs() / h.pitch);
        assert_eq!(h.dist, 0.0);
        if h.path.ends_with("2.wav") {
            counts[0] += 1;
        } else {
            assert!(h.path.ends_with("3.wav"));
            counts[1] += 1;
        }
    }
    assert!(counts[0] > 0 && counts[1] > counts[0], "{counts:?}");
}
#[test]
fn commander_controls_and_fallbacks() {
    let f = VoiceFixture::new("fallbacks");
    let (mut normal, mut slow) = (f.load(), f.load());
    slow.rate_mul = 0.3;
    assert_eq!(
        normal.prepare_voice("rep_test_voice"),
        slow.prepare_voice("rep_test_voice")
    );
    normal.for_battle(&f.0, ["rep", "cis"], &["kas2cw".into()]);
    near(normal.prepare_voice("rep_test_voice").unwrap().pitch, 1.6);
    normal.for_battle(&f.0, ["all", "imp"], &["kas2gcw".into()]);
    let h = normal.prepare_voice("rep_test_voice").unwrap();
    assert!(h.path.ends_with("1.wav"));
    near(h.pitch, 0.8);
    assert!(close(h.ears, (0.9, 0.9)));
    let mut sl = f.load();
    // No foreign definition for this name: use the loaded era's definition.
    assert!(sl
        .prepare_voice("rep_fallback")
        .unwrap()
        .path
        .ends_with("1.wav"));
    assert!(sl.prepare_voice("unknown").is_none());
    assert!(sl.prepare_voice("missing_file").is_none());
    // Resident CORE fallback uses its own bank-relative paths.
    sl.core = Bank::load(&f.0.join("sounds"));
    sl.map.props.clear();
    sl.map.by_level.clear();
    let h = sl.prepare_voice("rep_fallback").unwrap();
    assert_eq!(h.path, f.0.join("sounds/b/1.wav"));
    near(h.secs, 1.0);
}
#[test]
fn positional_rate_and_product_floor() {
    let f = VoiceFixture::new("position");
    let mut sl = f.load();
    let listener = Listener {
        pos: Vec3::ZERO,
        yaw: 0.0,
    };
    let hash = fnv1a_lowercase("rep_test_voice");
    let seed = sl.seed;
    for mul in [1.0, 0.3] {
        sl.rate_mul = mul;
        let h = sl
            .prepare_take("ReP_TeSt_VoIcE", 0, Vec3::new(60.0, 0.0, 0.0), &listener)
            .unwrap();
        // Hashed positional lookup remains in kas2gcw, even for a foreign army.
        assert_eq!(h.path, f.0.join("sounds/b/1.wav"));
        near(h.pitch, 0.8 * mul);
        near(h.secs, 1.0 / (0.8 * mul));
        near(h.dist, 60.0);
        let expected_gain = 0.9 * (390.0 / 720.0);
        near(h.gain, expected_gain);
        assert!(close(h.ears, (0.0, expected_gain)), "{:?}", h.ears);
        near(h.secs, sl.cache[&h.path].duration_secs() / h.pitch);
    }
    assert_eq!(seed, sl.seed, "fixed nth must not draw");
    sl.map
        .by_level
        .get_mut(&(fnv1a_lowercase("kas2gcw"), hash))
        .unwrap()
        .pitch = 0.01;
    let h = sl
        .prepare_take("rep_test_voice", 0, Vec3::ZERO, &listener)
        .unwrap();
    near(h.pitch, 0.05);
    near(h.secs, 20.0);
    assert!(sl
        .prepare_take("rep_test_voice", 99, Vec3::ZERO, &listener)
        .is_none());
    assert!(sl
        .prepare_take("unknown", 0, Vec3::ZERO, &listener)
        .is_none());
    assert!(sl
        .prepare_take("missing_file", 0, Vec3::ZERO, &listener)
        .is_none());
    let h = sl
        .prepare_take("rep_test_voice", 0, Vec3::splat(1000.0), &listener)
        .unwrap();
    near(h.gain, 0.0);
    assert_eq!(h.ears, (0.0, 0.0));
    near(h.secs, 20.0);
    let p = sl
        .map
        .by_level
        .get_mut(&(fnv1a_lowercase("kas2gcw"), hash))
        .unwrap();
    p.gain = 0.0;
    let h = sl
        .prepare_take("rep_test_voice", 0, Vec3::ZERO, &listener)
        .unwrap();
    near(h.gain, 0.0);
    near(h.secs, 20.0);
}

/// Supplemental actual mixer callers; invoke explicitly with a null ALSA config.
#[test]
#[ignore = "requires an explicitly configured audio output, e.g. null ALSA"]
fn voicep_public_callers() {
    let f = VoiceFixture::new("public");
    let mut audio = audio::Audio::new().expect("configured output required; never skip");
    let (mut sl, mut reference) = (f.load(), f.load());
    for mul in [1.0, 0.3] {
        sl.rate_mul = mul;
        reference.rate_mul = mul;
        let expected = reference.prepare_voice("rep_test_voice").unwrap();
        let secs = sl.play_voice(&mut audio, "rep_test_voice").unwrap();
        assert_eq!(sl.handed.as_mut().unwrap().pop().unwrap(), expected);
        near(secs, expected.secs);
        assert_eq!(sl.seed, reference.seed);
    }
    let listener = Listener {
        pos: Vec3::ZERO,
        yaw: 0.0,
    };
    for mul in [1.0, 0.3] {
        sl.rate_mul = mul;
        reference.rate_mul = mul;
        let pos = Vec3::new(60.0, 0.0, 0.0);
        let expected = reference
            .prepare_take("rep_test_voice", 0, pos, &listener)
            .unwrap();
        let secs = sl
            .play_take(&mut audio, "rep_test_voice", 0, pos, &listener)
            .unwrap();
        assert_eq!(sl.handed.as_mut().unwrap().pop().unwrap(), expected);
        near(secs, expected.secs);
    }
    assert!(sl.play_voice(&mut audio, "unknown").is_none());
    assert!(sl
        .play_take(&mut audio, "rep_test_voice", 99, Vec3::ZERO, &listener)
        .is_none());
    assert!(sl
        .play_take(
            &mut audio,
            "rep_test_voice",
            0,
            Vec3::splat(1000.0),
            &listener
        )
        .is_some());
    assert!(sl.handed.as_ref().unwrap().is_empty());
}

/// Rows supplied by the content census: mission path, faction0, faction1,
/// requested commander property (tab separated). Prints both resolutions and
/// every take; the input inventory must cover all selectable mixed battles.
#[test]
#[ignore = "requires actual mission/bank inputs and a complete commander inventory"]
fn voicep_bank_census() {
    let root = PathBuf::from(std::env::var_os("OPENBF_SOUND_ROOT").expect("baked root"));
    let inventory = std::fs::read_to_string(
        std::env::var_os("VOICEP_CENSUS_TSV").expect("complete battle/name inventory"),
    )
    .unwrap();
    let mut rows = 0;
    for row in inventory
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 4, "mission, faction0, faction1, name: {row}");
        let mission = crate::mission::MissionDef::load(Path::new(fields[0])).unwrap();
        let map_dir = root.join(&mission.map);
        let mut sl = SoundLib::load_for(&map_dir, &mission).expect("actual sound bank");
        sl.for_battle(&map_dir, [fields[1], fields[2]], &mission.sound_levels());
        let name = fields[3];
        let named = sl.find_named(name);
        let hashed = sl.find(fnv1a_lowercase(name));
        println!("RESOLVE\t{row}\tnamed={named:?}\thashed={hashed:?}");
        // Full SoundProp Debug includes level, sample/segment ids and weights,
        // pitch, gain and bus. Preserve absent rows rather than assuming audio.
        if let Some((p, _)) = &named {
            let old_pitch = hashed.as_ref().map_or(1.0, |(p, _)| p.pitch.max(0.05));
            for nth in 0..p.samples.len().max(p.segments.len()) {
                let path = sl.take_path(name, nth).expect("indexed take");
                let decoded = sl.voice_secs(&path).expect("actual decoded take");
                println!(
                    "TAKE\t{row}\t{nth}\t{}\tdecoded={decoded}\told_rate={old_pitch}\told_play_secs={}\told_return_secs={}\tnew_rate={}\tnew_secs={}",
                    path.display(),
                    decoded / old_pitch,
                    decoded / p.pitch.max(0.05),
                    p.pitch.max(0.05),
                    decoded / p.pitch.max(0.05)
                );
            }
        }
        rows += 1;
    }
    assert!(rows > 0, "empty census is not evidence");
    println!("VOICEP census input rows {rows}; inventory completeness requires separate review");
}

#[test]
fn dlc_sounds_join_the_core_bank_only_with_their_bake_and_never_over_retail() {
    let root = std::env::temp_dir().join(format!("openbf_dlc_sounds_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let bank = |dir: &str, sample: &str, names: &[&str]| {
        let dir = root.join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let props: Vec<String> = names
            .iter()
            .map(|name| {
                format!(
                    "(name_hash:{},kind:0,gain:1.0,pitch:1.0,min_distance:1.0,max_distance:2.0,\
                     rolloff:1.0,priority:0.5,looping:false,samples:[(7,1.0)],layers:[])",
                    fnv1a_lowercase(name)
                )
            })
            .collect();
        std::fs::write(
            dir.join("sounds.ron"),
            format!(
                "(source:\"x\",banks:[],samples:[(7,\"{sample}\")],props:[{}])",
                props.join(",")
            ),
        )
        .unwrap();
    };
    bank("core_bf2/sounds", "retail.wav", &["shared_name"]);
    bank(
        "side_bf2/dlc_sounds",
        "dlc.wav",
        &["shared_name", "fisto_bubble"],
    );
    let mut core = Bank::load(&root.join("core_bf2/sounds")).unwrap();
    core.import_dlc_sounds(&root);
    assert!(!core.props.contains_key(&fnv1a_lowercase("fisto_bubble")));
    std::fs::write(root.join("side_bf2/dlc_heroes.ron"), "()").unwrap();
    core.import_dlc_sounds(&root);
    let bubble = &core.props[&fnv1a_lowercase("fisto_bubble")];
    let path = core.sample_path(bubble.samples[0].0).unwrap();
    assert!(path.ends_with("dlc.wav"), "{}", path.display());
    let shared = &core.props[&fnv1a_lowercase("shared_name")];
    let path = core.sample_path(shared.samples[0].0).unwrap();
    assert!(path.ends_with("retail.wav"), "{}", path.display());
    std::fs::remove_dir_all(&root).unwrap();
}

/// Fisto's bubble sounds reach a real battle's library, by the map bank's path
/// through the resident bank, and each resolves to a file on disk.
#[test]
#[ignore = "requires OPENBF_DLC_TEST_ROOT with bf2_myg1 and core_bf2 sounds beside the DLC bake"]
fn real_dlc_bubble_sounds_start_in_a_battle() {
    let root = std::path::PathBuf::from(
        std::env::var_os("OPENBF_DLC_TEST_ROOT").expect("DLC test overlay"),
    );
    let lib = SoundLib::load(&root.join("bf2_myg1")).expect("map bank");
    for name in [
        "hero_weap_energyBall_throw",
        "hero_weap_energyBall_stdy",
        "hero_weap_energyBall_explo",
    ] {
        let (prop, in_core) = lib.find_named(name).expect(name);
        assert!(in_core, "{name}: the resident bank's");
        let core = lib.core.as_ref().expect("resident bank");
        let path = core.sample_path(prop.samples[0].0).expect(name);
        assert!(pak::is_file(&path), "{}", path.display());
    }
}

/// A BF1 map's resident bank carries BF2's aura loops and cast sounds,
/// which BF1 has none of; each resolves to a file on disk.
#[test]
fn a_bf1_core_bank_imports_the_buff_sounds() {
    let root = crate::assets::baked_root();
    if !root.join("bf1_nab1/sounds/sounds.ron").exists()
        || !root.join("bf2_nab2/sounds/sounds.ron").exists()
    {
        eprintln!("no baked bf1_nab1 and bf2_nab2 sounds: skipped");
        return;
    }
    let lib = SoundLib::load(&root.join("bf1_nab1")).expect("map bank");
    // No BF2 bank bakes `buff_rally_fire`'s sample: the Rally cast is
    // silent on BF2's maps too, and nothing is imported for it.
    assert!(lib.find_named("buff_rally_fire").is_none());
    for name in BUFF_SOUNDS.into_iter().filter(|n| *n != "buff_rally_fire") {
        let (prop, in_core) = lib.find_named(name).expect(name);
        assert!(in_core, "{name}: the resident bank's");
        let core = lib.core.as_ref().expect("resident bank");
        let path = core.sample_path(prop.samples[0].0).expect(name);
        assert!(pak::is_file(&path), "{}", path.display());
    }
    let (rage, _) = lib.find_named("buff_rage_effect").unwrap();
    assert!(rage.looping && rage.gain == 0.5 && rage.max_distance == 30.0);
}

#[test]
fn pause_click_interval_advances_without_aging_battle_sounds() {
    let mut sl = lib();
    let select = ui::shell_feel::MenuSound::Select.name();
    let props = [
        SoundProp {
            bus: SHELLFX,
            flat: true,
            pitch: 1.5,
            play_interval: Some((0.1, 0.0)),
            ..prop(select, 1)
        },
        SoundProp {
            play_interval: Some((0.1, 0.0)),
            ..prop("test_battle", 1)
        },
    ];
    sl.map.props.extend(merge_props(props.into()));
    let battle = sl.play_2d(None, "test_battle");
    assert_eq!(battle.len(), 1);
    let first = sl.play_2d(None, select);
    assert_eq!(first.len(), 1);
    assert!(first[0].menu);
    assert!(sl.play_2d(None, select).is_empty());

    sl.rate_mul = 0.3;
    assert_eq!(sl.one_shot_pitch(&first[0]), 1.5);
    assert_eq!(sl.one_shot_pitch(&battle[0]), battle[0].pitch * 0.3);

    // In a paused frame only the menu clock advances.
    sl.advance_menu_clock(0.05);
    assert!(sl.play_2d(None, select).is_empty());
    sl.advance_menu_clock(0.2);
    assert_eq!(sl.play_2d(None, select).len(), 1);
    assert!(sl.play_2d(None, "test_battle").is_empty());
}

#[test]
fn pause_click_intervals_from_the_bf2_battle_bank() {
    let root = crate::assets::baked_root();
    if !root.join("bf2_cor1/sounds/sounds.ron").is_file() {
        eprintln!("no baked BF2 Coruscant bank: skipped");
        return;
    }
    let mut sl = SoundLib::load(&root.join("bf2_cor1")).unwrap();
    let select = ui::shell_feel::MenuSound::Select.name();
    let (prop, _) = sl.find_named(select).unwrap();
    let (interval, _) = prop.play_interval.expect("baked navigation click interval");
    assert_eq!(prop.bus, SHELLFX);
    assert!(interval > 0.0);
    assert_eq!(sl.play_2d(None, select).len(), 1);
    sl.advance_menu_clock(interval * 0.5);
    assert!(sl.play_2d(None, select).is_empty());
    for _ in 0..2 {
        sl.advance_menu_clock(interval + 0.05);
        assert_eq!(sl.play_2d(None, select).len(), 1);
    }
    assert_eq!(sl.now, 0.0);
}
