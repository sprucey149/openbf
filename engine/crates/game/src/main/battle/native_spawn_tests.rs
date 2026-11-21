// Owned-content checks through the same constructor as CLI and shell battles.
use super::*;
use core::sim::PadState;
use std::path::PathBuf;

fn load(key: &str, mission: &str) -> WalkSim {
    let root = PathBuf::from(std::env::var_os("TUSKEN_ASSETS").expect("TUSKEN_ASSETS owned bake"));
    let missions = PathBuf::from(std::env::var_os("TUSKEN_MISSIONS").expect("TUSKEN_MISSIONS"));
    load_from(&root, &missions, key, mission)
}

fn load_from(
    root: &std::path::Path,
    missions: &std::path::Path,
    key: &str,
    mission: &str,
) -> WalkSim {
    let mission =
        game::mission::MissionDef::load(&missions.join(format!("{mission}.ron"))).unwrap();
    let (scene, report) = game::assets::load_battle_scene(&root.join(key), "con").unwrap();
    assert!(!matches!(report.common, Some((_, Err(_)))));
    let setup = BattleSetup {
        seed: 42,
        difficulty: Default::default(),
        spawn: (-140.0, 230.0),
        spawn_set: false,
        targets: false,
        ai: Some(16),
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
    make_walk(&scene, &setup).expect("ordinary Conquest fixture")
}

#[test]
fn gamorrean_resolver_requires_matching_game_and_class() {
    let dir = std::env::temp_dir().join(format!("openbf-gamorrean-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let class = "gam_inf_gamorreanguard";
    let file = dir.join("bf2_gam.ron");
    std::fs::write(
        &file,
        r#"[(name: "gam_inf_gamorreanguard", kind: "entc", base: "soldier", props: [])]"#,
    )
    .unwrap();
    assert_eq!(
        game::native_teams::native_side(&dir, "bf2", class),
        Some(("gam", file))
    );
    assert!(game::native_teams::native_side(&dir, "bf1", class).is_none());
    assert!(game::native_teams::native_side(&dir, "bf2", "absent_guard").is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires GAMORREAN_ASSETS owned overlay and GAMORREAN_MISSIONS"]
fn gamorrean_spawn_posts_are_separate_from_conquest_objectives() {
    let root = PathBuf::from(std::env::var_os("GAMORREAN_ASSETS").expect("owned Gamorrean bake"));
    let missions = PathBuf::from(std::env::var_os("GAMORREAN_MISSIONS").expect("mission input"));
    let (scene, _) = game::assets::load_battle_scene(&root.join("bf2_tat3"), "con").unwrap();
    assert_eq!(
        scene
            .manifest
            .instances
            .iter()
            .filter(|i| i.class == "com_bldg_invisible_controlzone" && i.name.starts_with("npccp"))
            .count(),
        2
    );
    for era in ["bf2_tat3_con", "bf2_tat3_con_cw"] {
        let mission =
            game::mission::MissionDef::load(&missions.join(format!("{era}.ron"))).unwrap();
        assert_eq!(mission.posts.len(), 5);
        let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
        layout.apply_mission(&mission);
        assert_eq!(layout.cps.len(), 5, "only the five conquest objectives");
        assert!(layout.cps.iter().all(|cp| cp.team != 3));
        for path in ["NPC_PATH", "npc_spawn_small"] {
            assert!(!layout.path(path).unwrap().nodes.is_empty());
        }
    }
}

#[test]
#[ignore = "requires GAMORREAN_ASSETS owned overlay and GAMORREAN_MISSIONS"]
fn gamorreans_gcw_spawn_with_stock_melee_audio_and_hostility() {
    gamorreans("bf2_tat3_con");
}

#[test]
#[ignore = "requires GAMORREAN_ASSETS owned overlay and GAMORREAN_MISSIONS"]
fn gamorreans_cw_spawn_with_stock_melee_audio_and_hostility() {
    gamorreans("bf2_tat3_con_cw");
}

fn gamorreans(era: &str) {
    let root = PathBuf::from(std::env::var_os("GAMORREAN_ASSETS").expect("owned Gamorrean bake"));
    let missions = PathBuf::from(std::env::var_os("GAMORREAN_MISSIONS").expect("mission input"));
    let body = root.join("side_bf2/locals/soldiers/gam_inf_gamorreanguard.glb");
    let axe = root.join("side_bf2/locals/weapons/gam_inf_weap_axe.glb");
    assert!(root.join("classes/bf2_gam.ron").is_file());
    assert!(body.is_file() && axe.is_file(), "fresh body and axe bakes");
    game::assets::load_glb(&axe).expect("actual held axe model consumption");
    let asset = game::assets::load_visual_skinned_glb(&body).unwrap();
    assert!(!asset.clips.is_empty(), "actual animated model consumption");
    assert!(asset.skeleton.index_of("hp_weapons").is_some());
    assert!(game::soldier_vis::SoldierVisuals::load_rig(
        &root.join("side_bf2/locals"),
        "gam_inf_gamorreanguard",
        &asset
    )
    .is_some());
    let mut walk = load_from(&root, &missions, "bf2_tat3", era);
    let native: Vec<_> = walk
        .sim
        .alive_units()
        .filter(|(_, u)| u.team == 2)
        .collect();
    assert_eq!(native.len(), 2, "{era}: actual guards at launch");
    assert!(walk.sim.hostile(2, 0) && walk.sim.hostile(0, 2));
    assert!(walk.sim.hostile(2, 1) && walk.sim.hostile(1, 2));
    assert_eq!(
        walk.sim.cps.len(),
        5,
        "native posts do not enter conquest objectives"
    );
    let template = walk.native_teams.get(&2).unwrap();
    assert_eq!(template.faction, "gam");
    let class = &template.classes[0];
    assert_eq!(class.name, "gam_inf_gamorreanguard");
    assert_eq!(class.geometry.as_deref(), Some("gam_inf_gamorreanguard"));
    assert_eq!(class.weapon_geometry.as_deref(), Some("gam_inf_weap_axe"));
    assert_eq!(class.anim.bank_of("gam_weap_inf_axe"), "human_sabre");
    assert_eq!(class.cries.hurt.as_deref(), Some("gam_hurt"));
    assert_eq!(class.cries.death.as_deref(), Some("gam_die"));
    let mission = game::mission::MissionDef::load(&missions.join(format!("{era}.ron"))).unwrap();
    let sounds = game::sounds::SoundLib::load_for(&root.join("bf2_tat3"), &mission).unwrap();
    for name in ["gam_hurt", "gam_die", "gam_chatter"] {
        assert!(sounds.has(name), "{era}: sound {name}");
        let path = sounds.take_path(name, 0).expect("guard audio sample");
        assert!(pak::is_file(path), "guard audio sample exists");
    }
    assert!(
        walk.sim.native_deathmatch(2),
        "{era}: the palace's Deathmatch goal"
    );
    // `[ours]` (2026-11-19): the guards never count for a post.
    assert!(
        native
            .iter()
            .all(|(_, u)| !u.capture_posts && !u.spec.capture_posts),
        "{era}: guards spawn without CapturePosts"
    );
    for (_, guard) in &native {
        let weapon = &walk.sim.weapon_classes[guard.weapon_class];
        assert!(weapon.saber.as_ref().is_some_and(|s| !s.clips.is_empty()));
        let (scene, _) = game::assets::load_battle_scene(&root.join("bf2_tat3"), "con").unwrap();
        let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
        assert!(
            ["NPC_PATH", "npc_spawn_small"].iter().any(|path| layout
                .path(path)
                .unwrap()
                .nodes
                .iter()
                .any(|node| node.pos == guard.spec.pos && node.yaw == guard.spec.yaw)),
            "authored invisible native post path"
        );
    }
    println!(
        "{era}: 2 guards, stock melee, mutual hostility; {} body clips",
        asset.clips.len()
    );
    let mut replay = load_from(&root, &missions, "bf2_tat3", era);
    for tick in 1..=300 {
        walk.step_tick(&[PadState::default()]);
        replay.step_tick(&[PadState::default()]);
        assert_eq!(walk.sim.hash(), replay.sim.hash(), "{era} tick {tick}");
        println!("guard parity {era} {tick} {:016x}", walk.sim.hash());
    }
    assert!(
        !walk.native_teams.is_empty(),
        "native roster survives stepping"
    );
}

/// `[ours]` (maintainer, 2026-11-19): a Gamorrean guard held inside each of
/// the palace's five posts, with its own (spawned) CapturePosts flag, never
/// counts there: the post's owner and capture state stay as they were.
fn gamorreans_hold_no_post(era: &str) {
    let root = PathBuf::from(std::env::var_os("GAMORREAN_ASSETS").expect("owned Gamorrean bake"));
    let missions = PathBuf::from(std::env::var_os("GAMORREAN_MISSIONS").expect("mission input"));
    let mut w = load_from(&root, &missions, "bf2_tat3", era);
    let guards: Vec<usize> = w
        .sim
        .alive_units()
        .filter(|(_, u)| u.team == 2)
        .map(|(h, _)| h.index as usize)
        .collect();
    assert_eq!(guards.len(), 2, "{era}: the guards at launch");
    let guard = guards[0];
    // Everyone else out of the count, every tick (a respawn takes its
    // class's flag again); the guards keep their own.
    let others_out = |w: &mut WalkSim| {
        for (i, u) in w.sim.units.iter_mut().enumerate() {
            if !guards.contains(&i) {
                u.capture_posts = false;
            }
        }
    };
    for ci in 0..w.sim.cps.len() {
        let owner = w.sim.cps[ci].owner;
        let at = w.sim.cps[ci].def.capture.center;
        for _ in 0..(20 * core::TICK_HZ) {
            // Position only: unlike `stand`, the guard keeps its own flag.
            let y = w.sim.ground_under(at, false);
            let u = &mut w.sim.units[guard];
            u.soldier.pos = glam::Vec3::new(at.x, y, at.z);
            u.soldier.vel = glam::Vec3::ZERO;
            assert!(u.alive, "{era} post {ci}: the guard is alive");
            assert!(w.sim.cps[ci]
                .def
                .capture
                .contains(w.sim.units[guard].soldier.pos));
            assert_eq!(
                w.sim.cp_inside(ci)[2],
                0,
                "{era} post {ci}: the guard counts"
            );
            others_out(&mut w);
            controlled_tick(&mut w);
        }
        assert_eq!(w.sim.cps[ci].owner, owner, "{era} post {ci}: owner changed");
    }
    println!("{era}: a guard held 20 s in each of 5 posts; no post counted it");
}

#[test]
#[ignore = "requires GAMORREAN_ASSETS owned overlay and GAMORREAN_MISSIONS"]
fn gamorreans_gcw_hold_no_post() {
    gamorreans_hold_no_post("bf2_tat3_con");
}

#[test]
#[ignore = "requires GAMORREAN_ASSETS owned overlay and GAMORREAN_MISSIONS"]
fn gamorreans_cw_hold_no_post() {
    gamorreans_hold_no_post("bf2_tat3_con_cw");
}

fn dune(era: &str) -> WalkSim {
    let w = load("tat1", era);
    let root = PathBuf::from(std::env::var_os("TUSKEN_ASSETS").unwrap());
    // These input checks must pass even on the broken production constructor.
    assert!(root.join("classes/bf1_des.ron").is_file());
    assert!(root
        .join("side_bf1/locals/soldiers/tat_inf_tusken.glb")
        .is_file());
    let manifest: game::assets::Manifest =
        ron::from_str(&pak::read_to_string(root.join("tat1/manifest.ron")).unwrap()).unwrap();
    let layout = game::layout::MapLayout::from_manifest(&manifest);
    for name in ["cp1", "cp3"] {
        let post = layout.cps.iter().find(|cp| cp.name == name).unwrap();
        assert_eq!(post.team, 3);
        assert_eq!(post.ally_count, Some(65536));
        assert!(post.ally_path.is_none());
        assert!(post.spawn_path.is_some());
    }
    let native: Vec<_> = w.sim.alive_units().filter(|(_, u)| u.team == 2).collect();
    assert_eq!(native.len(), 14, "{era}: actual Tusken actors at launch");
    let cp = w
        .sim
        .cps
        .iter()
        .find(|cp| cp.def.name.eq_ignore_ascii_case("cp3"))
        .unwrap();
    assert_eq!(cp.owner, Some(2));
    assert_eq!(cp.def.spawn_nodes.len(), 13);
    assert!(w.sim.hostile(2, 0) && w.sim.hostile(0, 2));
    assert!(w.sim.hostile(2, 1) && w.sim.hostile(1, 2));
    let own: Vec<_> = w.sim.cps.iter().filter(|cp| cp.owner == Some(2)).collect();
    assert_eq!(own.len(), 2, "CP1 and CP3 are authored native posts");
    let mut cp3_spawns = 0;
    let mut classes = [0; 2];
    for (_, u) in native {
        assert!(
            own.iter()
                .any(|post| post.def.spawn_nodes.contains(&(u.spec.pos, u.spec.yaw))),
            "authored native-owned SpawnPath node"
        );
        cp3_spawns += usize::from(cp.def.spawn_nodes.contains(&(u.spec.pos, u.spec.yaw)));
        classes[usize::from(u.spec.class)] += 1;
        println!(
            "Tusken class={} node={:?} yaw={}",
            u.spec.class, u.spec.pos, u.spec.yaw
        );
    }
    assert_eq!(classes, [7, 7], "{era}: the class selector mix (PR #183)");
    assert!(!w.sim.native_deathmatch(2), "{era}: Tuskens keep the hunt");
    assert!(cp3_spawns > 0);
    println!("{era}: {cp3_spawns} of 14 opening members use CP3's 13 nodes; the rest use CP1");
    w
}

#[test]
#[ignore = "owned retail bakes; set TUSKEN_ASSETS and TUSKEN_MISSIONS"]
fn tusken_launch_gcw() {
    dune("tat1_con");
}

#[test]
#[ignore = "owned retail bakes; set TUSKEN_ASSETS and TUSKEN_MISSIONS"]
fn tusken_launch_cw() {
    dune("tat1_con_cw");
}

#[test]
#[ignore = "owned retail bakes; set TUSKEN_ASSETS and TUSKEN_MISSIONS"]
fn tusken_deterministic() {
    for era in ["tat1_con", "tat1_con_cw"] {
        let mut a = dune(era);
        let mut b = dune(era);
        for tick in 1..=600 {
            a.sim.step(&[PadState::default()]);
            b.sim.step(&[PadState::default()]);
            assert_eq!(a.sim.hash(), b.sim.hash(), "{era} tick {tick}");
            println!("parity {era} {tick} {:016x}", a.sim.hash());
        }
    }
}

// Isolate capture/respawn from combat while retaining actual authored floors,
// posts, capture regions, wave clocks, death and spawn code. Natural AI runs
// separately in the deterministic tests.
fn park(w: &mut WalkSim) {
    let handles: Vec<_> = w.sim.alive_units().map(|(h, _)| h).collect();
    for h in handles {
        w.sim.unseat_unit(h);
        let u = &mut w.sim.units[h.index as usize];
        u.controller = core::unit::Controller::Idle;
        u.capture_posts = false;
        u.soldier.vel = glam::Vec3::ZERO;
    }
}

fn controlled_tick(w: &mut WalkSim) {
    for u in &mut w.sim.units {
        u.controller = core::unit::Controller::Idle;
    }
    w.sim.step(&[PadState::default()]);
    for u in &mut w.sim.units {
        u.controller = core::unit::Controller::Idle;
    }
}

fn stand(w: &mut WalkSim, unit: usize, pos: glam::Vec3) {
    let y = w.sim.ground_under(pos, false);
    w.sim.units[unit].capture_posts = true;
    w.sim.units[unit].soldier.pos = glam::Vec3::new(pos.x, y, pos.z);
    w.sim.units[unit].soldier.vel = glam::Vec3::ZERO;
}

#[test]
#[ignore = "owned retail bakes; set TUSKEN_ASSETS and TUSKEN_MISSIONS"]
fn tusken_post_loss_recapture_and_respawn() {
    for era in ["tat1_con", "tat1_con_cw"] {
        let mut w = dune(era);
        park(&mut w);
        let ci = w
            .sim
            .cps
            .iter()
            .position(|c| c.def.name.eq_ignore_ascii_case("cp3"))
            .unwrap();
        let at = w.sim.cps[ci].def.capture.center;
        let victim = w.sim.alive_units().find(|(_, u)| u.team == 2).unwrap().0;
        let survivor = w
            .sim
            .alive_units()
            .find(|(h, u)| u.team == 2 && *h != victim)
            .unwrap()
            .0;
        let tickets = w.sim.battle.teams.map(|t| t.tickets());
        w.sim
            .kill_environment(victim, core::sim::DeathCause::DeathRegion);
        assert_eq!(w.sim.battle.teams.map(|t| t.tickets()), tickets);
        for _ in 0..450 {
            controlled_tick(&mut w);
        }
        let reborn = w.sim.handle(victim.index as usize);
        assert_ne!(reborn.gen, victim.gen, "own-post respawn");
        let u = w.sim.unit(reborn).unwrap();
        assert!(w
            .sim
            .cps
            .iter()
            .filter(|cp| cp.owner == Some(2))
            .any(|cp| cp.def.spawn_nodes.contains(&(u.spec.pos, u.spec.yaw))));
        park(&mut w);
        let attacker = w.player.index as usize;
        stand(&mut w, attacker, at);
        assert!(w.sim.counts_in_cp(attacker, ci));
        for _ in 0..320 {
            controlled_tick(&mut w);
        }
        assert_eq!(
            w.sim.cps[ci].owner,
            Some(2),
            "spawning remains legal until the owner flips"
        );
        w.sim
            .kill_environment(reborn, core::sim::DeathCause::DeathRegion);
        for _ in 0..50 {
            controlled_tick(&mut w);
        }
        println!("neutralization witness: owner {:?} meter {} of {} inside {:?} actor {:?} alive {} controller {:?}", w.sim.cps[ci].owner, w.sim.cps[ci].neutral, w.sim.cps[ci].def.neutralize_time, w.sim.cp_inside(ci), w.sim.units[attacker].soldier.pos, w.sim.units[attacker].alive, w.sim.units[attacker].controller);
        assert_eq!(w.sim.cps[ci].owner, None, "actual neutralization");
        for _ in 0..370 {
            controlled_tick(&mut w);
        }
        assert_eq!(
            w.sim.cps[ci].owner,
            Some(0),
            "army captures the native post"
        );
        assert!(w.sim.cps[ci].spawnable_by(0));
        assert!(!w.sim.cps[ci].spawnable_by(2));
        let cp1 = w
            .sim
            .cps
            .iter()
            .position(|c| c.def.name.eq_ignore_ascii_case("cp1"))
            .unwrap();
        for _ in 0..450 {
            controlled_tick(&mut w);
        }
        let redirected = w.sim.handle(victim.index as usize);
        let u = w.sim.unit(redirected).unwrap();
        assert!(u.alive);
        assert!(
            w.sim.cps[cp1]
                .def
                .spawn_nodes
                .contains(&(u.spec.pos, u.spec.yaw)),
            "CP3 loss redirects to the remaining owned CP1"
        );
        park(&mut w);
        let at1 = w.sim.cps[cp1].def.capture.center;
        stand(&mut w, attacker, at1);
        for _ in 0..320 {
            controlled_tick(&mut w);
        }
        assert_eq!(w.sim.cps[cp1].owner, Some(2));
        w.sim
            .kill_environment(redirected, core::sim::DeathCause::DeathRegion);
        for _ in 0..50 {
            controlled_tick(&mut w);
        }
        assert_eq!(w.sim.cps[cp1].owner, None);
        for _ in 0..820 {
            controlled_tick(&mut w);
        }
        assert_eq!(w.sim.cps[cp1].owner, Some(0));
        assert!(
            !w.sim.units[victim.index as usize].alive,
            "both posts lost: no native wave"
        );
        park(&mut w);
        stand(&mut w, survivor.index as usize, at);
        assert!(w.sim.counts_in_cp(survivor.index as usize, ci));
        for _ in 0..370 {
            controlled_tick(&mut w);
        }
        assert_eq!(w.sim.cps[ci].owner, None, "native neutralization");
        for _ in 0..370 {
            controlled_tick(&mut w);
        }
        assert_eq!(w.sim.cps[ci].owner, Some(2), "native recapture");
        for _ in 0..450 {
            controlled_tick(&mut w);
        }
        let again = w.sim.unit(w.sim.handle(victim.index as usize)).unwrap();
        assert!(again.alive);
        assert_ne!(again.gen, reborn.gen);
        assert!(w.sim.cps[ci]
            .def
            .spawn_nodes
            .contains(&(again.spec.pos, again.spec.yaw)));
        println!("lifecycle {era}: native -> neutral -> army -> neutral -> native; victim generation {} -> {} -> {}", victim.gen, reborn.gen, again.gen);
    }
}

#[test]
#[ignore = "owned retail bakes; set TUSKEN_ASSETS and TUSKEN_MISSIONS"]
fn tusken_controls_friendly_natives_and_two_armies() {
    for (map, mission, natives) in [
        ("bf2_end1", "bf2_end1_con", 6),
        ("bf2_cor1", "bf2_cor1_con", 0),
    ] {
        let mut a = load(map, mission);
        let mut b = load(map, mission);
        assert_eq!(
            a.sim.alive_units().filter(|(_, u)| u.team >= 2).count(),
            natives
        );
        if natives > 0 {
            assert!(a.sim.allied(2, 0));
            assert!(a.sim.allied(0, 2));
            assert!(a.sim.hostile(2, 1));
            for (_, u) in a.sim.alive_units().filter(|(_, u)| u.team == 2) {
                let root = PathBuf::from(std::env::var_os("TUSKEN_ASSETS").unwrap());
                let (scene, _) = game::assets::load_battle_scene(&root.join(map), "con").unwrap();
                let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
                assert!(layout.cps.iter().any(|cp| cp
                    .ally_path
                    .as_deref()
                    .and_then(|p| layout.path(p))
                    .is_some_and(|p| p
                        .nodes
                        .iter()
                        .any(|n| (n.pos, n.yaw) == (u.spec.pos, u.spec.yaw)))));
            }
        }
        for tick in 1..=600 {
            a.sim.step(&[PadState::default()]);
            b.sim.step(&[PadState::default()]);
            assert_eq!(a.sim.hash(), b.sim.hash(), "{map} tick {tick}");
            println!("control {map} {tick} {:016x}", a.sim.hash());
        }
    }
}

/// The Jawa rule on the four Mos Eisley Conquest rows (maintainer,
/// 2026-11-20, `game::jawa`): fielded, armed with the arc caster (at its
/// native share) and the fusion cutter, hostile to both armies, never in a
/// post's count; a Jawa's death costs no ticket and is no team kill, and a
/// soldier a Jawa kills costs his side one ticket.
#[test]
#[ignore = "owned retail bakes; set TUSKEN_ASSETS and TUSKEN_MISSIONS"]
fn mos_eisley_jawas_fight_both_armies_with_a_toned_down_arc_caster() {
    let root = PathBuf::from(std::env::var_os("TUSKEN_ASSETS").unwrap());
    let donor = game::jawa::donor(&root.join("classes")).unwrap();
    let full = game::roster::weapon_from_db(&donor, game::jawa::ARC_CASTER).unwrap();
    for (map, mission) in [
        ("tat2", "tat2_con"),
        ("tat2", "tat2_con_cw"),
        ("bf1_tat2", "bf1_tat2_con"),
        ("bf1_tat2", "bf1_tat2_con_cw"),
    ] {
        let mut w = load(map, mission);
        let jawas: Vec<_> = w
            .sim
            .alive_units()
            .filter(|(_, u)| u.team == 2)
            .map(|(h, _)| h)
            .collect();
        assert!(!jawas.is_empty(), "{mission}: Jawas on the field");
        assert!(w.sim.native_neutral(2), "{mission}");
        for army in [0, 1] {
            assert!(
                w.sim.hostile(2, army) && w.sim.hostile(army, 2),
                "{mission} {army}"
            );
        }
        let names = |u: &core::unit::Unit| -> Vec<String> {
            std::iter::once(u.weapon_class)
                .chain(u.held.iter().map(|h| h.class))
                .map(|i| w.sim.weapon_classes[i].name.clone())
                .collect()
        };
        for &h in &jawas {
            let u = w.sim.unit(h).unwrap();
            let kit = names(u);
            assert!(
                kit.iter().any(|n| n == game::jawa::ARC_CASTER),
                "{mission} {kit:?}"
            );
            assert!(
                kit.iter().any(|n| n == "tat_weap_inf_jawa_fusioncutter"),
                "{mission} {kit:?}"
            );
            assert!(!u.capture_posts, "{mission}: Jawas take no post");
            for ci in 0..w.sim.cps.len() {
                assert!(!w.sim.counts_in_cp(h.index as usize, ci), "{mission}");
            }
        }
        let arc = w
            .sim
            .weapon_classes
            .iter()
            .find(|c| c.name == game::jawa::ARC_CASTER)
            .unwrap();
        assert_eq!(
            arc.ordnance.max_damage,
            full.ordnance.max_damage * game::jawa::NATIVE_ARC_CASTER_SCALE,
            "{mission}"
        );
        let army = w
            .sim
            .alive_units()
            .find(|(_, u)| u.team == 0 && !u.is_player)
            .map(|(h, _)| h)
            .unwrap();
        let tickets = w.sim.battle.teams.map(|t| t.tickets());
        let team_kills = w.sim.nav_stats.team_kills;
        w.sim.kill_unit_by(jawas[0], Some(army));
        assert_eq!(
            w.sim.battle.teams.map(|t| t.tickets()),
            tickets,
            "{mission}"
        );
        assert_eq!(
            w.sim.nav_stats.team_kills, team_kills,
            "{mission}: no team kill"
        );
        if let Some(&killer) = jawas.get(1) {
            w.sim.kill_unit_by(army, Some(killer));
            assert_eq!(
                w.sim.battle.teams.map(|t| t.tickets()),
                [tickets[0] - 1, tickets[1]],
                "{mission}: one ticket for a soldier a Jawa kills"
            );
            assert_eq!(w.sim.nav_stats.team_kills, team_kills, "{mission}");
        }
    }
}
