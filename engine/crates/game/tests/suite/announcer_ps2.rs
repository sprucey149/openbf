//! Card ANNC's PS2 pair: the commander's lines of a BF2 Utapau Clone Wars
//! battle heard as CIS, replayed on the port's rules and data and compared
//! with the PS2's own log (`analysis/re-announcer.md` §4 rows 7, 8-9, 11, 13,
//! 14; `lead_tmp/annc/run4_bf2_uta1.csv`). The events are scripted, as the
//! PS2 run's were poked: the post owners, bleed rates and tickets go straight
//! into `Commander::observe`; the names come from the baked posts and the
//! generated mission, the lengths from the baked takes.
//!
//! Skips cleanly when Utapau is not baked or the audio backend is not built.

use game::announce::{battle_lines, Commander};
use game::assets::Manifest;
use game::layout::MapLayout;
use game::mission::MissionDef;
use game::sounds::SoundLib;
use std::path::{Path, PathBuf};

const DT: f32 = 1.0 / 30.0;

/// One PS2 row: what was poked (or happened), the line that played, and the
/// seconds from the cause to the line's first frame.
struct Row {
    ps2: &'static str,
    at: f32,
    line: &'static str,
    onset: f32,
    /// The line waited for the one before it (row 11's second line): the
    /// PS2's onset is that line's length.
    queued: bool,
}

const ROWS: [Row; 7] = [
    Row {
        ps2: "7  post 0 -> us",
        at: 1.0,
        line: "cis_off_com_report_captured_commandpost",
        onset: 0.03,
        queued: false,
    },
    Row {
        ps2: "8  own bleed 1/3",
        at: 10.0,
        line: "cis_off_com_report_us_overwhelmed",
        onset: 0.03,
        queued: false,
    },
    Row {
        ps2: "9  +30.06 s",
        at: 40.06,
        line: "cis_off_com_report_us_overwhelmed",
        onset: 0.03,
        queued: false,
    },
    Row {
        ps2: "11 post 0 -> us",
        at: 50.0,
        line: "cis_off_com_report_captured_commandpost",
        onset: 0.03,
        queued: false,
    },
    Row {
        ps2: "11 + enemy bleed",
        at: 50.0,
        line: "cis_off_com_report_enemy_losing",
        onset: 2.84,
        queued: true,
    },
    Row {
        ps2: "13 tickets 15",
        at: 121.0,
        line: "cis_off_defeat_im",
        onset: 0.03,
        queued: false,
    },
    Row {
        ps2: "14 post 0 -> them",
        at: 130.0,
        line: "cis_off_com_report_enemyCaptured_commandpost",
        onset: 0.03,
        queued: false,
    },
];

fn conquest_manifest(dir: &Path) -> Option<Manifest> {
    let mut m: Manifest =
        ron::from_str(&std::fs::read_to_string(dir.join("manifest.ron")).ok()?).ok()?;
    if let Some(i) = game::assets::mode_layer_index(&m, "con") {
        let l = m.layers.remove(i);
        m.regions.extend(l.regions);
        m.instances.extend(l.instances);
    }
    Some(m)
}

/// The lengths of every take of `name`, in seconds.
fn take_lengths(sl: &SoundLib, name: &str) -> Vec<f32> {
    (0..)
        .map_while(|n| sl.take_path(name, n))
        .filter_map(|p| audio::decode(&p).ok())
        .map(|d| d.duration_secs())
        .collect()
}

#[test]
fn uta1_clone_wars_as_cis_plays_the_ps2s_lines() {
    let dir = game::assets::baked_root().join("bf2_uta1");
    let (Some(manifest), Some(mut sl)) = (conquest_manifest(&dir), SoundLib::load(&dir)) else {
        eprintln!("SKIP: bf2_uta1 is not baked");
        return;
    };
    if !audio::Audio::backend_compiled() {
        eprintln!("SKIP: no audio backend, so no take lengths");
        return;
    }
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mission = MissionDef::load(&missions.join("bf2_uta1_con_cw.ron")).expect("mission");
    // CIS is the script's team 1 (re-announcer.md §3), the port's team 0.
    assert_eq!(mission.teams[0].faction, "cis");
    let mut layout = MapLayout::from_manifest(&manifest);
    layout.apply_mission(&mission);
    let posts: Vec<_> = layout.cps.iter().map(|c| c.vo.clone()).collect();
    assert!(posts.len() >= 3 && posts.iter().all(|p| !p.is_empty()));
    let n = posts.len();
    let tickets = [mission.teams[0].tickets, mission.teams[1].tickets];
    assert_eq!(tickets, [150, 150]);
    let factions = [0, 1].map(|t| mission.teams[t].faction.as_str());
    let lines = battle_lines(&mission.voice, factions, factions, tickets);
    assert_eq!(lines.low_threshold, [15, 15], "PS2 RAM: 15 of 150");
    let mut c = Commander::new(lines, factions, 0, posts, vec![None; n]);

    // The scene, by frame: (seconds, what changes).
    let third = 1.0 / 3.0;
    let mut owners: Vec<Option<u8>> = vec![None; n];
    let (mut bleed, mut left) = ([0.0f32; 2], tickets);
    let mut started: Vec<(f32, String, f32)> = Vec::new(); // (t, line, length)
    let mut asked: Vec<(f32, String)> = Vec::new();
    for f in 0..(140.0 / DT) as u32 {
        let t = f as f32 * DT;
        let at = |s: f32| (s / DT).round() as u32 == f;
        if at(1.0) {
            owners[0] = Some(0);
        }
        if at(10.0) {
            bleed[0] = third;
        }
        if at(45.0) {
            bleed[0] = 0.0;
        }
        if at(50.0) {
            owners[1] = Some(0);
            bleed[1] = third;
        }
        if at(118.0) {
            bleed[1] = 0.0;
        }
        if at(120.0) {
            left[0] = 16;
        }
        if at(121.0) {
            left[0] = 15;
        }
        if at(130.0) {
            owners[n - 1] = Some(1);
        }
        for l in c.observe(DT, &owners, bleed, left, true, false) {
            asked.push((t, l.name));
        }
        let mut len = 0.0;
        let line = c.queue.step(DT, |l| {
            let (_, _, secs) = sl.voice_take(&l.name)?;
            len = secs;
            Some(secs)
        });
        if let Some(l) = line {
            started.push((t, l.name, len));
        }
    }

    println!("PS2 row            | line                                          | PS2 onset | port onset");
    assert_eq!(started.len(), ROWS.len(), "lines played: {started:?}");
    assert_eq!(asked.len(), ROWS.len(), "lines asked for: {asked:?}");
    for (i, (row, (t, name, _))) in ROWS.iter().zip(&started).enumerate() {
        let onset = t - row.at;
        println!(
            "{:<18} | {:<45} | {:>6.2} s  | {:>6.2} s",
            row.ps2, name, row.onset, onset
        );
        assert_eq!(name, row.line, "row {}", row.ps2);
        if row.queued {
            // The line before it must end first: the port's onset is its own
            // take's length and the voice's gap. The PS2 played the 2.34 s
            // take (segment 9CB20905) and was busy 2.82 s.
            let before = started[i - 1].2 + core::announcer::VOICE_GAP_SECS;
            assert!((onset - before).abs() <= 2.0 * DT, "{onset} vs {before}");
            let takes = take_lengths(&sl, &started[i - 1].1);
            println!("{:<18} | takes of the line before: {takes:.2?}", "");
            assert!(takes.iter().any(|l| (l - 2.339).abs() < 0.01), "{takes:?}");
            assert!((onset - row.onset).abs() <= 0.5, "row {}: {onset}", row.ps2);
        } else {
            assert!((onset - row.onset).abs() <= 0.5, "row {}: {onset}", row.ps2);
        }
    }
    // Row 11: the enemy's bleed ran 68 s and its line was said once.
    let enemy = asked
        .iter()
        .filter(|a| a.1 == "cis_off_com_report_enemy_losing")
        .count();
    assert_eq!(enemy, 1, "no repeat at +30 s or +60 s");
    // Rows 8-9: the repeat came 30 s after the start (PS2: 30.06 s).
    let own: Vec<f32> = asked
        .iter()
        .filter(|a| a.1 == "cis_off_com_report_us_overwhelmed")
        .map(|a| a.0)
        .collect();
    assert_eq!(own.len(), 2);
    println!(
        "own bleed repeat after {:.2} s (PS2 30.06 s)",
        own[1] - own[0]
    );
    assert!((own[1] - own[0] - 30.06).abs() <= 0.5);
}
/// Every `VO_<a>_<b><Event>` a world file could spell, over the disc's side
/// keys (`lead_tmp/annc/cpvo.py`'s list): the four commander factions and the
/// locals' names.
fn vo_dictionary() -> std::collections::HashMap<u32, String> {
    const SIDES: [&str; 17] = [
        "All", "Imp", "Rep", "CIS", "Loc", "Locals", "Neu", "Neutral", "Gun", "Wok", "Ewk", "Gam",
        "Des", "Tus", "Geo", "Jaw", "Her",
    ];
    let mut out = std::collections::HashMap::new();
    for a in SIDES {
        for b in SIDES {
            for e in core::announcer::PostEvent::ALL {
                let k = format!("VO_{a}_{b}{}", e.key());
                out.insert(game::classdb::fnv1a_lowercase(&k), k);
            }
        }
    }
    out
}

/// The layout reads every VO property the bake holds: on every baked map,
/// with every layer merged, the posts' tables hold as many lines as the
/// capture and spawn-only CTF posts carry `VO_*` properties, and none names a side outside the
/// four commander factions; the ones on other instances are listed. `ANNC_DUMP=<file>` writes one
/// `map<TAB>instance<TAB>property<TAB>line` row per entry, for the comparison
/// with the disc (`lead_tmp/annc/check_posts.py`).
#[test]
fn every_baked_post_reads_its_whole_vo_table() {
    let root = game::assets::baked_root();
    let maps = game::assets::discover_maps(&root);
    if maps.is_empty() {
        eprintln!("SKIP: no baked maps under {}", root.display());
        return;
    }
    let dict = vo_dictionary();
    let four: std::collections::HashSet<u32> = (0..4)
        .flat_map(|l| (0..4).map(move |s| (l, s)))
        .flat_map(|(l, s)| core::announcer::PostEvent::ALL.map(|e| (l, s, e)))
        .map(|(l, s, e)| game::classdb::fnv1a_lowercase(&core::announcer::post_property(l, s, e)))
        .collect();
    let is_vo = |p: &&(u32, String)| dict.contains_key(&p.0) && !p.1.is_empty();
    let mut dump = String::new();
    let (mut posts, mut lines) = (0, 0);
    let mut strays = std::collections::BTreeMap::<String, usize>::new();
    for (key, _, dir) in &maps {
        let txt = std::fs::read_to_string(dir.join("manifest.ron")).expect("manifest");
        let mut m: Manifest = ron::from_str(&txt).expect("manifest parses");
        for l in std::mem::take(&mut m.layers) {
            m.regions.extend(l.regions);
            m.instances.extend(l.instances);
        }
        let mut on_posts = 0;
        for i in &m.instances {
            let n = i.props.iter().filter(is_vo).count();
            for p in i.props.iter().filter(is_vo) {
                assert!(
                    four.contains(&p.0),
                    "{key} {}: {} names another side",
                    i.name,
                    dict[&p.0]
                );
            }
            // Spawn-only CTF posts also become layout posts. KAS2's two
            // TDM spawn posts carry 24 lines without a capture region.
            let spawn_post = i.class.eq_ignore_ascii_case("com_bldg_controlzone_ctf")
                && [
                    game::layout::PROP_SPAWN_PATH,
                    game::layout::PROP_CONTROL_REGION,
                ]
                .iter()
                .all(|key| i.props.iter().any(|p| p.0 == *key));
            if spawn_post
                || i.props
                    .iter()
                    .any(|p| p.0 == game::layout::PROP_CAPTURE_REGION)
            {
                on_posts += n;
            } else if n > 0 {
                *strays
                    .entry(format!("{key} {} ({})", i.name, i.class))
                    .or_default() += n;
            }
        }
        let layout = MapLayout::from_manifest(&m);
        let read: usize = layout.cps.iter().map(|c| c.vo.iter().count()).sum();
        assert_eq!(read, on_posts, "{key}: VO lines read vs VO props baked");
        posts += layout.cps.iter().filter(|c| !c.vo.is_empty()).count();
        lines += read;
        for c in &layout.cps {
            for (l, s, e, line) in c.vo.iter() {
                let prop = core::announcer::post_property(l, s, e);
                dump += &format!("{key}\t{}\t{prop}\t{line}\n", c.name);
            }
        }
    }
    println!(
        "{} maps: {posts} posts with a VO table, {lines} lines read",
        maps.len()
    );
    // VO properties on an instance with no capture region: a post that is
    // destroyed, not captured (BF1 Endor's bunker panel), or an instance
    // without the paths/regions needed for a layout post.
    // BF1 has no modes: its strays are the destroyable posts, listed.
    println!(
        "{} more lines on {} instances that are no capture post",
        strays.values().sum::<usize>(),
        strays.len()
    );
    for (what, n) in strays
        .iter()
        .filter(|s| !s.0.contains("bf2_") && !s.0.contains("tat2"))
    {
        println!("  {what}: {n} lines");
    }
    if let Ok(path) = std::env::var("ANNC_DUMP") {
        std::fs::write(path, dump).expect("dump");
    }
}

/// Every line a battle can ask for has a take to play: for each generated
/// mission, the script's bleeding and low-reinforcement lines and the lines
/// its two factions can hear from the map's posts, looked up in the map's
/// sound library.
#[test]
fn every_mission_line_has_a_take() {
    let root = game::assets::baked_root();
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let Ok(dir) = std::fs::read_dir(&missions) else {
        eprintln!("SKIP: no engine/missions (scripts/pack.py generates them)");
        return;
    };
    let mut paths: Vec<PathBuf> = dir
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    paths.sort();
    let (mut checked, mut lines) = (0, 0);
    let mut silent = Vec::new();
    for path in &paths {
        let mission = MissionDef::load(path).expect("mission");
        let dir = root.join(&mission.map);
        let (Some(manifest), Some(sl)) =
            (conquest_manifest(&dir), SoundLib::load_for(&dir, &mission))
        else {
            continue;
        };
        let mut layout = MapLayout::from_manifest(&manifest);
        layout.apply_mission(&mission);
        let factions: Vec<usize> = mission
            .teams
            .iter()
            .filter_map(|t| core::announcer::faction_index(&t.faction))
            .collect();
        let mut names = std::collections::BTreeSet::new();
        names.extend(mission.voice.bleeding.iter().map(|b| b.2.clone()));
        names.extend(mission.voice.low_reinforcements.iter().map(|b| b.2.clone()));
        names.extend(mission.voice.out_of_bounds.iter().map(|b| b.1.clone()));
        for c in &layout.cps {
            for (l, s, e, line) in c.vo.iter() {
                let heard = matches!(
                    e,
                    core::announcer::PostEvent::Capture | core::announcer::PostEvent::Lost
                );
                if heard && factions.contains(&l) && factions.contains(&s) {
                    names.insert(line.to_owned());
                }
            }
        }
        checked += 1;
        for n in names {
            lines += 1;
            if sl.take_path(&n, 0).is_none_or(|p| !p.exists()) {
                silent.push(format!(
                    "{}: {n}",
                    path.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    if checked == 0 {
        eprintln!("SKIP: no baked maps under {}", root.display());
        return;
    }
    println!(
        "{checked} missions, {lines} lines, {} without a take",
        silent.len()
    );
    for s in &silent {
        println!("  {s}");
    }
    // The one name on the discs that is no sound: BF1 Yavin's temple post
    // names `jungle2`.
    assert!(
        silent.iter().all(|s| s == "bf1_yav1_con.ron: jungle2"),
        "{silent:?}"
    );
}

/// The game's own path, end to end: an AI battle on Utapau (the mission's
/// factions and tickets, the built-in troopers) stepped tick by tick through
/// `Commander::frame`, as `main/windowed.rs` does. Every capture and every
/// neutralisation the sim raises is answered with the post's line for the
/// listener (CIS, team 0) — or with silence where the table has none — and
/// the voice never starts a line while another is playing.
#[test]
fn a_sim_battle_speaks_through_the_frame_hook() {
    use core::sim::Event;
    let dir = game::assets::baked_root().join("bf2_uta1");
    let Ok(mut scene) = game::assets::load_scene(&dir) else {
        eprintln!("SKIP: bf2_uta1 is not baked");
        return;
    };
    scene.apply_mode_layer("con");
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mission = MissionDef::load(&missions.join("bf2_uta1_con_cw.ron")).expect("mission");
    let mut layout = MapLayout::from_manifest(&scene.manifest);
    layout.apply_mission(&mission);

    let mut sim = core::sim::Sim::empty(7);
    sim.battle = core::battle::Battle::new(mission.tickets());
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.ai_alive_cap = mission.ai_alive_cap;
    let mut teams = [0, 1].map(|_| game::walk::TeamTemplate::default_trooper());
    for (t, def) in teams.iter_mut().zip(&mission.teams) {
        t.faction = def.faction.clone();
    }
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut walk =
        game::walk::WalkSim::with_teams(sim, scene.heightfield(), glam::Vec3::ZERO, teams, nav);
    scene.install_collision(&mut walk.sim);
    walk.setup_conquest(&layout, [12, 12], true);
    let mut voice = Commander::for_battle(&mission, &layout, &walk);
    assert_eq!(voice.announcer.listener(), 0);

    let pad = core::sim::PadState::default();
    let (mut want, mut asked, mut started) = (Vec::new(), Vec::new(), Vec::new());
    let mut busy_until = 0.0f32;
    for tick in 0..30 * 1200 {
        walk.step_tick(std::slice::from_ref(&pad));
        let t = tick as f32 * DT;
        for e in &walk.sim.events {
            // What the PS2's table says a CIS listener hears.
            match e {
                Event::Captured { team: 0, .. } => {
                    want.push("cis_off_com_report_captured_commandpost");
                }
                Event::Captured { .. } => {
                    want.push("cis_off_com_report_enemyCaptured_commandpost");
                }
                Event::Neutralized { by: 1, .. } => {
                    want.push("cis_off_com_report_lost_commandpost");
                }
                _ => {}
            }
        }
        let line = voice.frame(&walk.sim, walk.player, DT, |_| Some(2.0));
        asked.extend(voice.asked.iter().map(|l| l.name.clone()));
        if let Some(l) = line {
            assert!(t + 1e-3 >= busy_until, "{} cut a line off at {t}", l.name);
            busy_until = t + 2.0;
            started.push(l.name);
        }
        // Four post events are enough; a whole round costs a minute and a half.
        if walk.sim.outcome.is_some() || want.len() >= 4 {
            break;
        }
    }
    println!(
        "tick {} outcome {:?} owners {:?} alive {:?}",
        walk.sim.tick,
        walk.sim.outcome,
        walk.sim.cps.iter().map(|c| c.owner).collect::<Vec<_>>(),
        [0, 1].map(|t| walk.sim.battle.teams[t].alive()),
    );
    let post = |n: &&String| n.contains("commandpost");
    let asked_posts: Vec<&str> = asked.iter().filter(post).map(String::as_str).collect();
    println!(
        "{} post events, {} lines asked for ({} post lines), {} started",
        want.len(),
        asked.len(),
        asked_posts.len(),
        started.len()
    );
    assert!(
        want.len() >= 3,
        "the battle moved fewer than 3 posts: {want:?}"
    );
    assert_eq!(asked_posts, want);
    assert!(started.len() <= asked.len());
}

/// Card ANNC2's PS2 pair: the last stand of `lead_tmp/audit-annc/live1.csv`
/// (BF2 Utapau Clone Wars heard as CIS, the Republic's reinforcements held at
/// 3 with more than 3 alive). Eight post events at the PS2's game seconds:
/// the five that leave CIS on 4+ of the 6 posts set the Republic's script
/// rate (0 -> 0.3333 in the owner change's own sample) and ask
/// `cis_off_com_report_enemy_losing`, and the rate reads 0 again 3.003 s
/// later, four times of four (at the fifth the Republic had 2 alive for its 3
/// tickets, so the rate stays, and the log ends); the three with CIS on 2-3
/// posts ask no bleeding line. The port: the sim's posts are set
/// to the PS2's owners at the PS2's times and `Commander::frame` runs every
/// tick, as `main/windowed.rs` runs it.
#[test]
fn uta1_last_stand_bleeds_three_seconds_at_every_post_event() {
    let dir = game::assets::baked_root().join("bf2_uta1");
    let Ok(mut scene) = game::assets::load_scene(&dir) else {
        eprintln!("SKIP: bf2_uta1 is not baked");
        return;
    };
    scene.apply_mode_layer("con");
    let missions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mission = MissionDef::load(&missions.join("bf2_uta1_con_cw.ron")).expect("mission");
    assert_eq!(mission.factions(), ["cis".to_string(), "rep".to_string()]);
    let mut layout = MapLayout::from_manifest(&scene.manifest);
    layout.apply_mission(&mission);
    let mut sim = core::sim::Sim::empty(7);
    sim.battle = core::battle::Battle::new(mission.tickets());
    let mut teams = [0, 1].map(|_| game::walk::TeamTemplate::default_trooper());
    for (t, def) in teams.iter_mut().zip(&mission.teams) {
        t.faction = def.faction.clone();
    }
    let mut walk =
        game::walk::WalkSim::with_teams(sim, scene.heightfield(), glam::Vec3::ZERO, teams, None);
    walk.setup_conquest(&layout, [0, 0], true);
    assert_eq!(walk.sim.cps.len(), 6);
    // The PS2's posts at the start of the log (its team 1 = CIS = port 0).
    let start = [None, Some(1), None, Some(0), Some(0), Some(1)];
    for (c, o) in walk.sim.cps.iter_mut().zip(start) {
        c.owner = o;
    }
    let mut voice = Commander::for_battle(&mission, &layout, &walk);
    assert!(voice.script_bleed.is_some(), "a BF2 map");
    assert_eq!(voice.announcer.listener(), 0);
    voice.announcer.player_spawned();
    // (PS2 game s, post, new owner, a bleeding line on the PS2).
    let events: [(f32, usize, Option<u8>, bool); 8] = [
        (21.55, 2, Some(1), false),
        (26.76, 0, Some(0), false),
        (61.56, 1, None, false),
        (71.60, 1, Some(0), true),
        (85.42, 5, None, true),
        (91.69, 5, Some(0), true),
        (135.47, 2, None, true),
        (158.56, 2, Some(0), true),
    ];
    // The PS2's sample at each event: (Republic tickets, Republic alive).
    let rep = [
        (3, 12),
        (3, 12),
        (3, 7),
        (3, 7),
        (3, 5),
        (3, 4),
        (3, 3),
        (3, 2),
    ];
    let mut said: Vec<(f32, String)> = Vec::new();
    let mut zero_after: Vec<f32> = Vec::new();
    let (mut last_event, mut k) = (0.0f32, 0usize);
    let mut was = 0.0f32;
    println!(
        "PS2 event            | PS2 line     | port line    | port rate 0 after (PS2 3.003 s)"
    );
    for f in 0..(165.0 / DT) as u32 {
        let t = f as f32 * DT;
        if k < events.len() && t >= events[k].0 {
            walk.sim.cps[events[k].1].owner = events[k].2;
            let team = &mut walk.sim.battle.teams[1];
            team.reinforcements = rep[k].0 as f32;
            team.alive_ai = rep[k].1;
            last_event = t;
            k += 1;
        }
        voice.frame(&walk.sim, walk.player, DT, |_| Some(2.0));
        let rate = voice.script_bleed.as_ref().unwrap().rates();
        assert_eq!(rate[0], 0.0, "CIS never bleeds in this scene");
        for l in &voice.asked {
            if !l.name.contains("commandpost") {
                said.push((t, l.name.clone()));
                assert_eq!(t, last_event, "{} is asked in the event's frame", l.name);
            }
        }
        if was > 0.0 && rate[1] == 0.0 {
            zero_after.push(t - last_event);
        }
        was = rate[1];
    }
    let bleeding: Vec<&(f32, String)> = said.iter().filter(|s| s.1.contains("report")).collect();
    for (i, e) in events.iter().enumerate() {
        let port = bleeding.iter().find(|s| (s.0 - e.0).abs() <= DT);
        println!(
            "{:7.2} s post {} -> {:<4} | {:<12} | {:<12} | {}",
            e.0,
            e.1,
            e.2.map_or("none".into(), |o| o.to_string()),
            if e.3 { "enemy_losing" } else { "-" },
            port.map_or("-", |_| "enemy_losing"),
            match zero_after.get(events[..i].iter().filter(|x| x.3).count()) {
                Some(z) if e.3 => format!("{z:.3} s"),
                None if e.3 => "stays (alive 2 < tickets 3, as on the PS2)".into(),
                _ => String::new(),
            }
        );
        assert_eq!(port.is_some(), e.3, "event at {} s", e.0);
    }
    assert_eq!(bleeding.len(), 5, "{said:?}");
    assert!(bleeding
        .iter()
        .all(|s| s.1 == "cis_off_com_report_enemy_losing"));
    assert_eq!(zero_after.len(), 4);
    for z in &zero_after {
        assert!((z - 3.0).abs() <= 0.1, "{zero_after:?}");
    }
}
