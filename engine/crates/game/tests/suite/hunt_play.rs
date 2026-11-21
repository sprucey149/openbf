//! Issue #161 on the Hunt bakes: the BF1 Hunt Ewoks' weapons, the class
//! screen's names, class change at your own posts, and the per-map kit
//! census behind the card's audit table.
//!
//! `OPENBF_HUNT_ROOT` names a folder holding `assets_baked` and `missions`
//! with the 14 `*_hunt.ron` (the battle-launch check's fixture); without it
//! every test skips.

use core::sim::{buttons, PadState, SideChoice, Sim, SpawnStage};
use game::mission::MissionDef;
use game::roster::FactionRoster;
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;
use std::path::{Path, PathBuf};

const HUNTS: [&str; 14] = [
    "bf1_end1", "bf1_geo1", "bf1_kas1", "bf1_kas2", "bf1_nab1", "bf1_nab2", "bf1_tat2", "tat1",
    "bf2_end1", "bf2_geo1", "bf2_hot1", "bf2_kas2", "bf2_nab2", "tat2",
];

fn root() -> Option<PathBuf> {
    let Some(root) = std::env::var_os("OPENBF_HUNT_ROOT").map(PathBuf::from) else {
        eprintln!("SKIP: set OPENBF_HUNT_ROOT to a Hunt bake");
        return None;
    };
    Some(root)
}

fn mission(root: &Path, key: &str) -> MissionDef {
    MissionDef::load(&root.join(format!("missions/{key}_hunt.ron"))).expect(key)
}

fn rosters(root: &Path, m: &MissionDef) -> [FactionRoster; 2] {
    let factions = [m.teams[0].faction.as_str(), m.teams[1].faction.as_str()];
    FactionRoster::pair(
        &root.join("assets_baked/classes"),
        &m.source_game,
        factions,
        Some(m),
    )
    .unwrap_or_else(|e| panic!("{}: {e:#}", m.map))
    .0
}

/// A Hunt round on `key`'s map as the battle setup builds one: the Hunt
/// layout with the mission's posts, both rosters, the Hunt rules and the
/// posts' class-change reach. No bots.
fn hunt_walk(root: &Path, key: &str) -> WalkSim {
    let m = mission(root, key);
    let assets = root.join("assets_baked");
    let dir = assets.join(&m.map);
    let (mut scene, _) = game::assets::load_battle_scene(&dir, "hunt").unwrap();
    scene.kill_objects(m.scene_kills());
    let rosters = rosters(root, &m);
    let mut sim = Sim::empty(42);
    if m.source_game == "bf2" {
        sim.ground_rules = core::soldier::GroundRules::Bf2;
    }
    sim.omni_radius = core::sim::omni_radius_for(&m.source_game);
    for (team, definition) in m.teams.iter().enumerate() {
        sim.battle.teams[team] = core::battle::TeamState::new(definition.tickets);
    }
    sim.battle.unlimited_reinforcements = true;
    let mut names = Vec::new();
    let mut team = 0;
    let templates = rosters.map(|roster| {
        let table: Vec<_> = roster
            .classes
            .iter()
            .map(|class| class.register(&mut sim, &mut names))
            .collect();
        let visuals = roster
            .classes
            .iter()
            .map(ClassVisual::from_roster)
            .collect();
        let template = TeamTemplate::from_roster(&roster, &table[0], visuals);
        sim.classes[team] = table;
        team += 1;
        template
    });
    sim.deathmatch = Some(core::sim::deathmatch::Deathmatch::hunt(50, [8, 8]));
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut walk = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, templates, nav);
    scene.install_collision(&mut walk.sim);
    let mut layout = game::layout::MapLayout::from_battle_manifest(&scene.manifest, true);
    layout.apply_mission(&m);
    walk.setup_conquest(&layout, [0, 0], true);
    let posts = game::walk::hunt_post_classes(
        &assets.join("classes"),
        &scene.dir,
        &scene.manifest.source_game,
    )
    .unwrap();
    walk.set_hunt_class_change(&layout, &posts, &scene.manifest.source_game);
    walk
}

fn tap(walk: &mut WalkSim, bits: u32) {
    walk.step_tick(&[PadState {
        buttons: bits,
        ..Default::default()
    }]);
    walk.step_tick(&[PadState::default()]);
}

/// Spawn the player on `team` through the side, class and post screens.
fn spawn_on(walk: &mut WalkSim, team: u8) {
    assert!(walk.sim.open_spawn_screens(0, true));
    walk.sim.side_choice[0] = if team == 0 {
        SideChoice::Team1
    } else {
        SideChoice::Team2
    };
    for _ in 0..6 {
        if walk.sim.unit(walk.player).is_some_and(|u| u.alive) {
            break;
        }
        tap(walk, buttons::SPAWN_ACCEPT);
        walk.player = walk.sim.players[0].unwrap();
    }
    let me = walk.sim.unit(walk.player).unwrap();
    assert!(me.alive, "spawned");
    assert_eq!(me.team, team);
}

/// Stand the player near one of his team's posts, facing it, where he may
/// change class; `None` when no spot around any of his posts qualifies.
fn at_own_post(walk: &mut WalkSim) -> Option<usize> {
    let i = walk.player.index as usize;
    let team = walk.sim.units[i].team;
    for (ci, cp) in walk.sim.cps.clone().iter().enumerate() {
        if cp.owner != Some(team) {
            continue;
        }
        for off in [Vec3::Z, Vec3::X, Vec3::NEG_Z, Vec3::NEG_X] {
            for r in [2.0, 3.0, 1.0, 4.0] {
                let mut at = cp.def.pos + off * r;
                at.y = walk.sim.ground_under(at, false);
                let to = cp.def.pos - at;
                let u = &mut walk.sim.units[i];
                u.soldier.pos = at;
                u.soldier.yaw = (-to.x).atan2(-to.z);
                u.soldier.pitch = to.y.atan2(to.with_y(0.0).length());
                if walk.sim.class_change_post(i) == Some(ci) {
                    return Some(ci);
                }
            }
        }
    }
    None
}

/// The LAN playtest's Ewok (2026-11-14): BF1's spear reached about 9 m on a
/// level throw (25 m/s, 1.5 g, no lift) and the rock was a 20 m/s shell with
/// no blast. A BF1 Hunt Ewok now throws BF2's: the spear 50 m/s, 3 s, lifted
/// 0.15, 200 to a soldier on contact; the rock BF2's 40 m/s shell with its
/// blast. Both show BF2's HUD rows. The Imperials keep their own kits.
#[test]
fn bf1_endor_hunt_ewoks_throw_bf2s_spear_and_rock() {
    let Some(root) = root() else { return };
    let m = mission(&root, "bf1_end1");
    let [army, ewoks] = rosters(&root, &m);
    assert_eq!(ewoks.faction, "ewk");
    let mut spears = 0;
    for class in &ewoks.classes {
        for (weapon, hud) in std::iter::once((&class.weapon, &class.weapon_hud))
            .chain(class.held.iter().map(|h| (&h.weapon, &h.hud)))
        {
            match weapon.name.as_str() {
                "ewk_weap_inf_spear" => {
                    spears += 1;
                    let thrown = weapon.thrown.expect("the spear is thrown");
                    assert!((thrown.shot_elevate - 0.15).abs() < 1e-6, "{}", class.class);
                    let shot = weapon.shot_ordnance();
                    assert_eq!(
                        (shot.velocity, shot.life_span),
                        (50.0, 3.0),
                        "{}",
                        class.class
                    );
                    assert_eq!(shot.damage * shot.scales[0], 200.0, "{}", class.class);
                    assert!(hud.bf2, "{}: BF2's HUD row", class.class);
                }
                "ewk_weap_inf_rock" => {
                    let shot = weapon.shot_ordnance();
                    assert_eq!(shot.velocity, 40.0, "{}", class.class);
                    assert!(
                        weapon.ordnance.explosion.is_some(),
                        "{}: the rock's blast",
                        class.class
                    );
                    assert!(hud.bf2, "{}: BF2's HUD row", class.class);
                }
                _ => {}
            }
        }
    }
    assert!(spears > 0, "an Ewok carries the spear");
    for class in &army.classes {
        assert!(!class.weapon.name.starts_with("ewk_"), "{}", class.class);
    }
}

/// The playtest's class screen offered two Ewoks with one name: both
/// games' text tables call every Ewok "Ewok" and BF1's two Ewok classes
/// share `UnitType` `repair`. Every Hunt team's class screen now names each
/// class differently.
#[test]
fn hunt_class_screens_name_every_class_differently() {
    let Some(root) = root() else { return };
    let assets = root.join("assets_baked");
    for key in HUNTS {
        let m = mission(&root, key);
        let walk = hunt_walk(&root, key);
        let feed = game::event_feed::EventFeed::for_walk(&assets, &m.source_game, &walk);
        for team in 0..2 {
            let names = feed.class_names(team);
            println!("{key} team {team}: {names:?}");
            let mut sorted = names.to_vec();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), names.len(), "{key} team {team}: {names:?}");
        }
    }
}

/// The playtest: the use button at his own post did nothing in Hunt. On
/// BF2 Endor (posts without a capture region: BF2's 5 m class radius) and
/// BF1 Endor (its own regions) the player changes class at a post of his
/// team, in place and without dying. Hoth's killed posts are not posts.
#[test]
fn hunt_players_change_class_at_their_own_posts() {
    let Some(root) = root() else { return };
    for key in ["bf2_end1", "bf1_end1"] {
        let mut walk = hunt_walk(&root, key);
        // The Ewoks have two classes on both maps.
        spawn_on(&mut walk, 1);
        let before = walk.player;
        let ci = at_own_post(&mut walk).unwrap_or_else(|| panic!("{key}: no post to change at"));
        println!("{key}: changes class at {}", walk.sim.cps[ci].def.name);
        tap(&mut walk, buttons::ACTION | buttons::SPAWN_BACK);
        assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Alive, "{key}: opened");
        while walk.sim.spawn_choice[0] != 1 {
            tap(&mut walk, buttons::SPAWN_CLASS_NEXT);
            while walk.sim.class_carousel[0].slide > 0 {
                walk.step_tick(&[PadState::default()]);
            }
        }
        tap(&mut walk, buttons::SPAWN_ACCEPT);
        let after = walk.sim.players[0].unwrap();
        let me = walk.sim.unit(after).unwrap();
        assert!(me.alive, "{key}");
        assert_eq!((after.index, me.spec.class), (before.index, 1), "{key}");
    }
    let walk = hunt_walk(&root, "bf2_hot1");
    for gone in ["CP7OBJ", "hangarcp"] {
        assert!(
            !walk
                .sim
                .cps
                .iter()
                .any(|c| c.def.name.eq_ignore_ascii_case(gone)),
            "Hoth Hunt kills {gone}"
        );
    }
}

/// BF1 Kashyyyk Hunt's MagnaGuard is built from BF2's side: the roster
/// records it, so it draws with BF2's assets on a BF1 map, and the Hunt
/// grenade pass reads BF2's class data for its weapons. Its kit matches BF2
/// Kashyyyk Hunt's own MagnaGuard weapon for weapon.
#[test]
fn bf1_hunt_classes_from_bf2s_side_keep_bf2s_kit_and_assets() {
    let Some(root) = root() else { return };
    let kit = |key: &str| {
        let m = mission(&root, key);
        let class = rosters(&root, &m)
            .into_iter()
            .flat_map(|r| r.classes)
            .find(|c| c.class == "cis_inf_officer_hunt")
            .unwrap_or_else(|| panic!("{key}: no MagnaGuard"));
        let weapons: Vec<_> = std::iter::once(&class.weapon)
            .chain(class.held.iter().map(|h| &h.weapon))
            .map(|w| {
                let shot = w.shot_ordnance();
                (
                    w.name.clone(),
                    w.thrown.is_some(),
                    shot.velocity,
                    shot.life_span,
                )
            })
            .collect();
        (class, weapons)
    };
    let (bf2, bf2_kit) = kit("bf2_kas2");
    assert!(!bf2.from_bf2_side, "BF2's own MagnaGuard");
    for key in ["bf1_kas1", "bf1_kas2"] {
        let (class, weapons) = kit(key);
        assert!(class.from_bf2_side, "{key}");
        assert_eq!(
            ClassVisual::from_roster(&class).asset_game("bf1"),
            "bf2",
            "{key}"
        );
        assert_eq!(weapons, bf2_kit, "{key}");
    }
    let m = mission(&root, "bf1_kas1");
    for class in rosters(&root, &m).iter().flat_map(|r| &r.classes) {
        if class.class != "cis_inf_officer_hunt" {
            assert!(!class.from_bf2_side, "{}", class.class);
            assert_eq!(ClassVisual::from_roster(class).asset_game("bf1"), "bf1");
        }
    }
}

/// A BF2 Hunt post reaches as far as its class's `SwitchClassRadius`; a BF2
/// class without one keeps the original's no reach. Every BF1 post takes
/// the `[ours]` 5 m (`HUNT_SWITCH_CLASS_RADIUS`).
#[test]
fn hunt_post_class_reach_follows_each_games_rule() {
    let Some(root) = root() else { return };
    let assets = root.join("assets_baked");
    for key in HUNTS {
        let m = mission(&root, key);
        let walk = hunt_walk(&root, key);
        let dir = assets.join(&m.map);
        let db =
            game::walk::hunt_post_classes(&assets.join("classes"), &dir, &m.source_game).unwrap();
        let mut layout = {
            let (scene, _) = game::assets::load_battle_scene(&dir, "hunt").unwrap();
            game::layout::MapLayout::from_battle_manifest(&scene.manifest, true)
        };
        layout.apply_mission(&m);
        for (post, cp) in walk.sim.cps.iter().zip(&layout.cps) {
            let authored = db
                .resolve(&cp.class)
                .and_then(|c| db.prop_f32(c, "SwitchClassRadius"))
                .filter(|r| *r > 0.0);
            println!(
                "{key} {} ({}): {} m",
                post.def.name, cp.class, post.switch_class_radius
            );
            let want = if m.source_game == "bf2" {
                authored.unwrap_or(0.0)
            } else {
                game::walk::HUNT_SWITCH_CLASS_RADIUS
            };
            assert_eq!(post.switch_class_radius, want, "{key} {}", post.def.name);
        }
    }
}

/// Census (run by hand, `--ignored --nocapture`): per Hunt and side, each
/// class's kit as the round fields it - weapon, fired speed, life, lift,
/// damage to a soldier, ammo, the HUD key - its animation family and root
/// scale, and whether a class change at one of its posts is reachable.
#[test]
#[ignore]
fn census_hunt_kits() {
    let Some(root) = root() else { return };
    for key in HUNTS {
        let m = mission(&root, key);
        let rosters = rosters(&root, &m);
        for (team, roster) in rosters.iter().enumerate() {
            for class in &roster.classes {
                println!(
                    "{key} t{team} {} [{}] {} HP, anim {:?} x{}",
                    class.class,
                    class.label,
                    class.max_health,
                    class.anim.family,
                    class.anim.root_scale
                );
                let kit = std::iter::once((&class.weapon, &class.weapon_hud, class.total_clips))
                    .chain(class.held.iter().map(|h| (&h.weapon, &h.hud, h.clips)));
                for (w, hud, clips) in kit {
                    let shot = w.shot_ordnance();
                    println!(
                        "    {}: {:.1} m/s, life {:.2}, lift {:?}, {:.1} to a soldier, blast {}, clip {} x {clips}, hud {} (bf2 {})",
                        w.name,
                        shot.velocity,
                        shot.life_span,
                        w.thrown.map(|t| t.shot_elevate),
                        shot.damage * shot.scales[0],
                        w.ordnance.explosion.is_some(),
                        w.rounds_per_clip,
                        hud.key,
                        hud.bf2
                    );
                }
            }
        }
        let mut walk = hunt_walk(&root, key);
        for team in 0..2 {
            if walk.sim.classes[team as usize].len() < 2 {
                println!("{key} t{team}: one class, no class change");
                continue;
            }
            spawn_on(&mut walk, team);
            let post = at_own_post(&mut walk).map(|ci| walk.sim.cps[ci].def.name.clone());
            println!("{key} t{team}: class change at {post:?}");
        }
    }
}

/// The maintainer (2026-11-20): each BF2 Hunt plays the original's Hunt world.
/// The scene merges the sublevel that holds the mission's `<map>_hunt` world,
/// every object the script's `KillObject` names is gone from the scene (model
/// and collision), the round's posts are that sublevel's and none is killed,
/// the script's AI barriers switch and its death regions are found. BF2 Hoth's
/// Hunt opens the Echo Base back shields and removes the shield generator. The
/// BF1 Hunts (ours: BF1 maps have no Hunt world) play the base world's posts.
/// Kashyyyk's flag bases (`ctfbase1`, `ctfbase2`) are not in its Hunt world,
/// so its KillObject removes nothing there, as on the disc.
#[test]
fn every_hunt_plays_its_hunt_world_without_the_objects_its_script_kills() {
    let Some(root) = root() else { return };
    for key in HUNTS {
        let m = mission(&root, key);
        let dir = root.join("assets_baked").join(&m.map);
        let bare = game::assets::load_scene(&dir).unwrap();
        let (mut scene, load) = game::assets::load_battle_scene(&dir, "hunt").unwrap();
        let removed = scene.kill_objects(m.scene_kills());
        let layer = game::assets::authored_mode_layer_index(&bare.manifest, "hunt")
            .map(|i| &bare.manifest.layers[i]);
        let mut layer_posts: Vec<String> = Vec::new();
        if m.source_game == "bf2" {
            let layer = layer.unwrap_or_else(|| panic!("{key}: no Hunt world"));
            assert!(
                layer
                    .worlds
                    .iter()
                    .any(|w| w.to_ascii_lowercase().ends_with("_hunt")),
                "{key}: layer {} holds no _hunt world: {:?}",
                layer.name,
                layer.worlds
            );
            assert_eq!(load.layer.as_deref(), Some(layer.name.as_str()), "{key}");
            layer_posts = layer
                .instances
                .iter()
                .map(|i| i.name.to_ascii_lowercase())
                .collect();
        } else {
            assert!(layer.is_none(), "{key}: BF1 has no mode worlds");
        }
        for name in m.scene_kills() {
            assert!(
                !scene
                    .manifest
                    .instances
                    .iter()
                    .any(|i| i.name.eq_ignore_ascii_case(name)),
                "{key}: {name} is killed but still in the scene"
            );
        }
        let walk = hunt_walk(&root, key);
        let posts: Vec<String> = walk.sim.cps.iter().map(|c| c.def.name.clone()).collect();
        assert!(!posts.is_empty(), "{key}: no Hunt posts");
        for post in &posts {
            assert!(
                !m.killed.iter().any(|k| k.eq_ignore_ascii_case(post)),
                "{key}: killed post {post} is in the round"
            );
            if m.source_game == "bf2" {
                assert!(
                    layer_posts.contains(&post.to_ascii_lowercase()),
                    "{key}: post {post} is not the Hunt world's"
                );
            }
        }
        for team in 0..2u8 {
            assert!(
                !walk.sim.spawn_points(team).is_empty(),
                "{key}: team {team} cannot spawn"
            );
        }
        let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
        let switched = m.apply_barriers(nav.as_mut());
        let mut layout =
            game::layout::MapLayout::from_mission_manifest(&scene.manifest, true, false);
        let applied = layout.apply_mission(&m);
        println!(
            "{key}: layer {:?}, {removed} objects killed {:?}, posts {posts:?}, \
             {switched} barriers switched of {}, death regions {} (missing {:?})",
            load.layer,
            m.scene_kills(),
            m.barriers.len(),
            applied.death_regions.len(),
            applied.missing_death_regions,
        );
        if key == "bf2_hot1" {
            // CP7OBJ twice: one instance each of the eight names.
            assert_eq!(removed, 8, "{key}");
        }
    }
    // Hoth: the two back shields and the shield generator are gone.
    let m = mission(&root, "bf2_hot1");
    for name in ["echoback1", "echoback2", "shield"] {
        assert!(m.scene_kills().iter().any(|k| k == name), "{name}");
    }
}

/// The maintainer's big Hunt sides (2026-11-20 late): 32 a side open on
/// every Hunt map, more than most maps' spawn nodes, and spread out around
/// the nodes instead of stacking. Prints each team's posts and nodes (the
/// fitter's limits).
#[test]
fn big_hunt_sides_open_spread_out_on_every_map() {
    let Some(root) = root() else { return };
    const SIDE: u32 = 32;
    for key in HUNTS {
        let mut walk = hunt_walk(&root, key);
        walk.sim.team_combatants = Some([SIDE; 2]);
        walk.sim.ai_alive_cap = SIDE;
        let nodes = [0u8, 1].map(|team| {
            let mut n: Vec<[u32; 3]> = walk
                .sim
                .spawn_points(team)
                .iter()
                .map(|(p, _)| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
                .collect();
            n.sort_unstable();
            n.dedup();
            (walk.sim.spawn_posts(team).len(), n.len())
        });
        walk.opening_wave([SIDE; 2], false);
        let alive: Vec<(u8, Vec3)> = walk
            .sim
            .alive_units()
            .filter(|(_, u)| u.team < 2 && !u.is_player)
            .map(|(_, u)| (u.team, u.soldier.pos))
            .collect();
        let per_team = [0u8, 1].map(|t| alive.iter().filter(|(team, _)| *team == t).count());
        let stacked = alive
            .iter()
            .enumerate()
            .filter(|(i, (_, a))| {
                alive.iter().enumerate().any(|(j, (_, b))| {
                    j != *i
                        && (a.y - b.y).abs() < 2.0
                        && (Vec3::new(a.x - b.x, 0.0, a.z - b.z)).length() < 0.5
                })
            })
            .count();
        println!(
            "{key}: team 0 {} posts {} nodes, team 1 {} posts {} nodes; opened {per_team:?}, \
             {stacked} within 0.5 m of another",
            nodes[0].0, nodes[0].1, nodes[1].0, nodes[1].1
        );
        if std::env::var_os("HUNT_SPREAD_DEBUG").is_some() {
            for (t, p) in &alive {
                println!("  {key} t{t} {p}");
            }
        }
        assert_eq!(per_team, [SIDE as usize; 2], "{key}: every soldier opened");
        assert!(
            stacked * 10 <= alive.len(),
            "{key}: {stacked} of {} soldiers stand on another",
            alive.len()
        );
    }
}
