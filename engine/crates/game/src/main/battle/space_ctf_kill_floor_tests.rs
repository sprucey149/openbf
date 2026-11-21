// One-flag CTF on a space map has the same map's Conquest kill floor
// (maintainer, 2026-11-21), through the constructor CLI and shell battles
// use. Its world layer brings hangar posts of its own (the same hangars,
// some under other names), and the kill floor is walked from them: 15 m
// under the lowest floor they stand on, as in Conquest.
use super::*;

fn walk_for(map: &str, mode: &str) -> WalkSim {
    let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = crate_dir.join("../../assets_baked");
    let mission = crate_dir.join(format!("../../missions/{map}_{mode}.ron"));
    let mission = game::mission::MissionDef::load(&mission).unwrap();
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), mode).unwrap();
    let setup = BattleSetup {
        seed: 42,
        difficulty: Default::default(),
        spawn: (0.0, 0.0),
        spawn_set: false,
        targets: false,
        ai: Some(0),
        ai_cap: None,
        team_size_pct: None,
        xl: false,
        tickets: 250,
        tickets_set: false,
        ticket_pct: 100,
        time_limit: 0,
        score_limit: 180,
        space_score_limit: core::sim::DEFAULT_SCORE_LIMIT,
        cheats: Default::default(),
        teams: Some(mission.factions()),
        classes_dir: root.join("classes"),
        mission: Some(mission),
        vehicle: None,
        mission_file: None,
        player_team: 0,
        heroes: Some(core::sim::heroes::HeroRules::default()),
        dlc_heroes: false,
        netplay_rows: false,
        opening_humans: [0; 2],
    };
    make_walk(&scene, &setup).unwrap()
}

#[test]
fn space_one_flag_ctf_has_the_maps_conquest_kill_floor() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
    for map in [
        "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
    ] {
        if !root.join(map).join("manifest.ron").exists() {
            eprintln!("SKIP {map}: not baked");
            continue;
        }
        let conquest = walk_for(map, "ass").sim.kill_floor;
        let ctf = walk_for(map, "1flag");
        assert!(ctf.sim.ctf.is_some(), "{map}: a one-flag CTF round");
        assert!(
            !ctf.sim.cps.is_empty(),
            "{map}: the CTF layer's hangar posts"
        );
        eprintln!(
            "{map}: Conquest kill floor {conquest:?}, one-flag CTF {:?}",
            ctf.sim.kill_floor
        );
        assert!(conquest.is_some(), "{map}: Conquest has a kill floor");
        assert_eq!(
            ctf.sim.kill_floor.map(f32::to_bits),
            conquest.map(f32::to_bits),
            "{map}: one-flag CTF has the Conquest kill floor"
        );
    }
}

/// The leak census's one-flag runs on BF2 Yavin space that read `float`
/// (off the top of the Imperial side at y 384, still falling at y -100 when
/// the census stopped watching) end at that kill floor.
#[test]
fn a_soldier_who_falls_out_of_a_space_ctf_hangar_dies_at_the_kill_floor() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
    if !root.join("bf2_spa1").join("manifest.ron").exists() {
        eprintln!("SKIP bf2_spa1: not baked");
        return;
    }
    let mut walk = walk_for("bf2_spa1", "1flag");
    let floor = walk.sim.kill_floor.expect("a kill floor");
    let falling = glam::Vec3::new(-2613.88, -100.34, 893.46);
    assert!(falling.y > floor);
    let u = walk
        .sim
        .spawn(&core::unit::UnitSpec::trooper(0, falling))
        .expect("a trooper");
    let mut cause = None;
    for _ in 0..30 * core::TICK_HZ {
        walk.sim.step(&[core::sim::PadState::default()]);
        cause = cause.or(walk.sim.events.iter().find_map(|e| match e {
            core::sim::Event::EnvDeath { unit, cause } if *unit == u => Some(*cause),
            _ => None,
        }));
        if walk.sim.unit(u).is_none() {
            break;
        }
    }
    assert!(walk.sim.unit(u).is_none(), "he fell for 30 s and lived");
    assert_eq!(cause, Some(core::sim::DeathCause::KillFloor));
}
