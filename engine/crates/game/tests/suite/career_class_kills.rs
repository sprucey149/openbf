use core::collision::Layers;
use core::sim::{buttons, Event, PadState, Sim, SpawnStage};
use core::unit::{Controller, UnitHandle};
use game::event_feed::EventFeed;
use game::profile::Profile;
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;

struct ProfileFile {
    path: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl ProfileFile {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("openbf-class-career-{}.ron", std::process::id()));
        let previous = std::env::var_os("OPENBF_PROFILE");
        unsafe { std::env::set_var("OPENBF_PROFILE", &path) };
        Self { path, previous }
    }
}

impl Drop for ProfileFile {
    fn drop(&mut self) {
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("OPENBF_PROFILE", value),
                None => std::env::remove_var("OPENBF_PROFILE"),
            }
        };
        let _ = std::fs::remove_file(&self.path);
    }
}

fn utapau_match() -> Option<WalkSim> {
    let root = game::assets::baked_root();
    let map = root.join("bf2_uta1");
    if !map.join("manifest.ron").exists() {
        eprintln!("SKIP: career headless match requires baked BF2 Utapau");
        return None;
    }
    let (scene, _) = game::assets::load_battle_scene(&map, "con").unwrap();
    let mission =
        game::mission::MissionDef::load(&root.parent().unwrap().join("missions/bf2_uta1_con.ron"))
            .unwrap();
    let (rosters, warnings) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        "bf2",
        ["all", "imp"],
        Some(&mission),
    )
    .unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::Bf2;
    sim.omni_radius = core::sim::omni_radius_for("bf2");
    for (team, definition) in mission.teams.iter().enumerate() {
        sim.battle.teams[team] = core::battle::TeamState::new(definition.tickets);
    }
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
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut walk = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, templates, nav);
    scene.install_collision(&mut walk.sim);
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    walk.sim.death_regions = layout.apply_mission(&mission).death_regions;
    mission.apply_arc_blocks(walk.sim.nav.as_mut());
    mission.apply_barriers(walk.sim.nav.as_mut());
    walk.setup_conquest(&layout, [0, 0], true);
    assert!(walk.sim.open_spawn_screens(0, false));
    Some(walk)
}

fn step(walk: &mut WalkSim, feed: &mut EventFeed, pad: PadState) {
    walk.step_tick(&[pad]);
    feed.tick(&walk.sim, walk.player);
}

fn tap(walk: &mut WalkSim, feed: &mut EventFeed, buttons: u32) {
    step(
        walk,
        feed,
        PadState {
            buttons,
            ..Default::default()
        },
    );
    step(walk, feed, PadState::default());
}

fn choose_class(walk: &mut WalkSim, feed: &mut EventFeed, name: &str) {
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Map);
    walk.sim.set_spawn_points(0, 12);
    tap(walk, feed, buttons::SPAWN_ACCEPT);
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Class);
    let class = walk.teams[0]
        .classes
        .iter()
        .position(|c| c.name == name)
        .unwrap();
    assert!(walk.sim.classes[0][class].unlock_points > 0);
    for _ in 0..walk.teams[0].classes.len() {
        if usize::from(walk.sim.spawn_choice[0]) == class {
            break;
        }
        tap(walk, feed, buttons::SPAWN_CLASS_NEXT);
        while walk.sim.class_carousel[0].slide > 0 {
            step(walk, feed, PadState::default());
        }
    }
    assert_eq!(usize::from(walk.sim.spawn_choice[0]), class);
    tap(walk, feed, buttons::SPAWN_ACCEPT);
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Accepted);
    assert_eq!(
        usize::from(walk.sim.unit(walk.player).unwrap().spec.class),
        class
    );
}

fn shoot_enemy(walk: &mut WalkSim, feed: &mut EventFeed) -> UnitHandle {
    let pos = walk.soldier().pos;
    let target_pos = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X]
        .into_iter()
        .find_map(|direction| {
            let mut target = pos + direction * 6.0;
            target.y = walk.sim.ground_under(target, false);
            if (target.y - pos.y).abs() > 0.5 {
                return None;
            }
            let ray = target + Vec3::Y * 1.4 - (pos + Vec3::Y * 1.4);
            walk.sim
                .collision
                .ray(
                    pos + Vec3::Y * 1.4,
                    ray.normalize(),
                    ray.length(),
                    Layers::ALL,
                )
                .is_none()
                .then_some(target)
        })
        .expect("a clear shot from the real command-post spawn");
    let spec = walk.sim.classes[1][0].spec(1, target_pos, 0.0, Controller::Idle, 0, 0);
    let target = walk.sim.spawn(&spec).unwrap();
    let kills_before = walk.sim.unit(walk.player).unwrap().kills;
    let mut shots = 0;
    for tick in 0..600 {
        let aim = walk.sim.unit(target).unwrap().aim_point();
        let (origin, _) = walk
            .sim
            .unit(walk.player)
            .unwrap()
            .aim_ray(walk.sim.ground_rules, walk.sim.views());
        let delta = aim - origin;
        let player = walk.sim.unit_mut(walk.player).unwrap();
        player.soldier.yaw = (-delta.x).atan2(-delta.z);
        player.soldier.pitch = delta.y.atan2(delta.with_y(0.0).length());
        let pad = PadState {
            buttons: buttons::MOUSE_AIM | if tick % 30 < 20 { buttons::FIRE } else { 0 },
            ..Default::default()
        };
        let mut killed = false;
        walk.advance_with(core::TICK_DT * 3.0, &[pad], |walk| {
            feed.tick(&walk.sim, walk.player);
            shots += walk
                .sim
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == walk.player))
                .count();
            killed |= walk.sim.events.iter().any(
                |e| matches!(e, Event::Kill { shooter, victim } if *shooter == walk.player && *victim == target),
            );
        });
        if killed {
            assert!(shots > 0);
            assert_eq!(walk.sim.unit(walk.player).unwrap().kills, kills_before + 1);
            walk.sim.units[target.index as usize].respawn_at = None;
            return target;
        }
    }
    let player = walk.sim.unit(walk.player).unwrap();
    panic!(
        "authored weapon {} did not kill its real-roster target ({shots} shots, {:?}, alive {})",
        walk.sim.weapon_classes[player.weapon_class].name, player.weapon, player.alive
    );
}

#[test]
fn battle_stats_filter_non_player_and_non_enemy_damage_and_deduplicate_ticks() {
    use core::unit::UnitSpec;
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};

    let mut walk = WalkSim::new(9, Vec3::ZERO, None);
    let player = walk.player;
    let enemy = walk
        .sim
        .spawn(&UnitSpec::trooper(1, Vec3::X * 10.0))
        .unwrap();
    let friend = walk
        .sim
        .spawn(&UnitSpec::trooper(0, Vec3::X * 20.0))
        .unwrap();
    let shot = |unit| Event::Shot {
        unit,
        weapon_class: 0,
        origin: Vec3::ZERO,
        dir: Vec3::NEG_Z,
    };
    let hit = |shooter, victim, damage| Event::Hit {
        direct_round: true,
        shooter,
        victim,
        damage,
        age: 0.0,
        health_left: 100.0,
        dir: Vec3::NEG_Z,
    };
    walk.sim.events = vec![
        shot(player),
        shot(player),
        shot(enemy),
        hit(player, enemy, 20.0),
        hit(player, enemy, 0.0),
        hit(player, friend, 10.0),
        hit(player, player, 10.0),
        hit(enemy, player, 20.0),
    ];
    let mut feed = EventFeed::default();
    feed.tick(&walk.sim, player);
    let expected = game::profile::BattleStats {
        shots: 2,
        hits: 1,
        captures: 0,
        points: 0,
    };
    assert_eq!(feed.battle_stats(), expected);
    feed.tick(&walk.sim, player);
    assert_eq!(feed.battle_stats(), expected);

    let vehicle = walk.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::NEG_Z * 20.0,
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    walk.sim.vehicles[vehicle.index as usize].team = Some(1);
    walk.sim.vehicles[vehicle.index as usize].alive = false;
    walk.sim.tick += 1;
    walk.sim.events = vec![Event::VehicleHit {
        shooter: Some(player),
        vehicle,
        damage: 100.0,
        dir: Some(Vec3::NEG_Z),
    }];
    feed.tick(&walk.sim, player);
    assert_eq!(feed.battle_stats().hits, 2);
    walk.sim.vehicles[vehicle.index as usize].team = Some(0);
    walk.sim.tick += 1;
    feed.tick(&walk.sim, player);
    assert_eq!(feed.battle_stats().hits, 2);

    let mut profile = Profile::default();
    walk.record_battle(&mut profile, feed.battle_stats());
    assert_eq!(profile.career.rounds, 1);
    assert_eq!(profile.career.wins, 0);
    assert_eq!(profile.career.shots, 2);
    assert_eq!(profile.career.hits, 2);
}

#[test]
fn a_player_killed_before_their_bolt_lands_still_gets_class_credit() {
    use core::projectile::{Bolt, Hit, Target};
    use core::unit::UnitSpec;

    let mut walk = WalkSim::new(7, Vec3::ZERO, None);
    let player = walk.player;
    let enemy = walk
        .sim
        .spawn(&UnitSpec::trooper(1, Vec3::NEG_Z * 10.0))
        .unwrap();
    for (owner, victim, team) in [(enemy, player, 1), (player, enemy, 0)] {
        walk.sim.units[victim.index as usize].health.cur = 1.0;
        let end = walk.sim.units[victim.index as usize].aim_point();
        walk.sim.bolts.push(Bolt {
            visual_shot: None,
            pos: end,
            vel: Vec3::ZERO,
            remaining: core::TICK_DT * 0.5,
            age: 0.0,
            owner,
            team,
            weapon_class: 0,
            hit: Hit::Target(Target::Unit(victim)),
            end,
            shooter_is_player: owner == player,
            hulls: core::collision::HullFilter::All,
            hit_mult: 1.0,
            gravity: 0.0,
            charge: 1.0,
            missile: None,
            fat: None,
        });
    }
    walk.step_tick(&[PadState::default()]);
    let me = &walk.sim.units[player.index as usize];
    assert!(!me.alive);
    assert_eq!(me.kills, 1);
    let mut profile = Profile::default();
    let mut feed = EventFeed::default();
    feed.tick(&walk.sim, walk.player);
    walk.record_battle(&mut profile, feed.battle_stats());
    assert_eq!(profile.career.class_kills["trooper"], 1);
    assert_eq!(profile.career.kills, 1);
    assert_eq!(profile.career.deaths, 1);
    assert_eq!(profile.career.hits, 1);
    assert!(!walk.flush_class_kills(&mut profile));
}

#[test]
fn real_match_class_switching_autosaves_and_profile_reload_keep_each_kill_once() {
    let Some(mut walk) = utapau_match() else {
        return;
    };
    let _file = ProfileFile::new();
    let mut profile = Profile::default();
    let mut feed = EventFeed::default();
    profile.record_round(Some(true), 10, 2, 100, 50, 2);
    profile
        .career
        .class_kills
        .insert("all_inf_officer".into(), 3);
    choose_class(&mut walk, &mut feed, "all_inf_officer");
    let post = walk
        .sim
        .cps
        .iter()
        .position(|cp| cp.owner != Some(0))
        .unwrap();
    let mut at = walk.sim.cps[post].def.pos;
    at.y = walk.sim.ground_under(at, false);
    walk.sim.unit_mut(walk.player).unwrap().soldier.pos = at;
    for _ in 0..2000 {
        step(&mut walk, &mut feed, PadState::default());
        if walk.sim.cps[post].owner == Some(0) {
            break;
        }
    }
    assert_eq!(walk.sim.cps[post].owner, Some(0));
    assert_eq!(feed.battle_stats().captures, 1);
    assert_eq!(feed.stats(&walk.sim, walk.player).captures, 1);
    shoot_enemy(&mut walk, &mut feed);
    let hash = walk.sim.hash();
    assert!(walk.flush_class_kills(&mut profile));
    assert_eq!(walk.sim.hash(), hash);
    assert_eq!(profile.career.class_kills["all_inf_officer"], 4);
    assert!(!walk.flush_class_kills(&mut profile));
    assert_eq!(profile.career.kills, 10);
    profile.save().unwrap();
    assert_eq!(Profile::load_or_default(), profile);
    profile = Profile::load_or_default();
    shoot_enemy(&mut walk, &mut feed);

    walk.request_respawn();
    for _ in 0..600 {
        step(&mut walk, &mut feed, PadState::default());
        if walk.sim.spawn_stage[0] == SpawnStage::Map {
            break;
        }
    }
    choose_class(&mut walk, &mut feed, "all_inf_wookiee");
    walk.sim.battle.teams[1].reinforcements = 1.0;
    shoot_enemy(&mut walk, &mut feed);
    assert!(walk.flush_class_kills(&mut profile));
    assert!(!walk.flush_class_kills(&mut profile));
    assert_eq!(profile.career.class_kills["all_inf_officer"], 5);
    assert_eq!(profile.career.class_kills["all_inf_wookiee"], 1);
    assert_eq!(profile.career.class_kills.len(), 2);

    let me = walk.sim.unit(walk.player).unwrap();
    let stats = feed.battle_stats();
    assert!(stats.shots > me.weapon.shots_fired);
    assert!(stats.hits >= 3);
    assert_eq!(stats.captures, 1);
    feed.tick(&walk.sim, walk.player);
    assert_eq!(feed.battle_stats(), stats);
    assert_eq!(walk.sim.outcome, Some(0));
    walk.record_battle(&mut profile, stats);
    assert!(!walk.flush_class_kills(&mut profile));
    assert_eq!(profile.career.rounds, 2);
    assert_eq!(profile.career.wins, 2);
    assert_eq!(profile.career.kills, 13);
    assert_eq!(profile.career.deaths, 3);
    assert_eq!(profile.career.shots, 100 + stats.shots);
    assert_eq!(profile.career.hits, 50 + stats.hits);
    assert_eq!(profile.career.captures, 3);

    profile.save().unwrap();
    let restored = Profile::load_or_default();
    profile.save().unwrap();
    let saved_twice = Profile::load_or_default();
    assert_eq!(restored, profile);
    assert_eq!(saved_twice, profile);
    println!("bf2_uta1: officer +2, respawn, Wookiee +1, victory; shots +{}, hits +{}, captures +{}; repeated tick/save and disk reload preserve counts", stats.shots, stats.hits, stats.captures);
}

/// Issue #28: put the player inside a friendly post he can change class at,
/// facing it.
fn at_friendly_post(walk: &mut WalkSim) {
    let i = walk.player.index as usize;
    let team = walk.sim.units[i].team;
    for (ci, cp) in walk.sim.cps.clone().iter().enumerate() {
        if !cp.spawnable_by(team) {
            continue;
        }
        for off in [Vec3::Z, Vec3::X, Vec3::NEG_Z, Vec3::NEG_X] {
            for r in [2.0, 3.0, 1.0] {
                let mut at = cp.def.pos + off * r;
                at.y = walk.sim.ground_under(at, false);
                let to = cp.def.pos - at;
                let u = &mut walk.sim.units[i];
                u.soldier.pos = at;
                u.soldier.yaw = (-to.x).atan2(-to.z);
                u.soldier.pitch = to.y.atan2(to.with_y(0.0).length());
                if walk.sim.class_change_post(i) == Some(ci) {
                    return;
                }
            }
        }
    }
    panic!("no friendly post to change class at");
}

#[test]
fn kills_after_a_class_change_at_a_post_count_for_the_new_class() {
    let Some(mut walk) = utapau_match() else {
        return;
    };
    let mut profile = Profile::default();
    let mut feed = EventFeed::default();
    choose_class(&mut walk, &mut feed, "all_inf_officer");
    at_friendly_post(&mut walk);
    tap(&mut walk, &mut feed, buttons::ACTION | buttons::SPAWN_BACK);
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Alive);
    while walk.sim.spawn_choice[0] != 0 {
        tap(&mut walk, &mut feed, buttons::SPAWN_CLASS_NEXT);
        while walk.sim.class_carousel[0].slide > 0 {
            step(&mut walk, &mut feed, PadState::default());
        }
    }
    tap(&mut walk, &mut feed, buttons::SPAWN_ACCEPT);
    assert_eq!(walk.sim.spawn_stage[0], SpawnStage::Accepted);
    assert_eq!(walk.sim.unit(walk.player).unwrap().spec.class, 0);
    shoot_enemy(&mut walk, &mut feed);
    assert!(walk.flush_class_kills(&mut profile));
    let rifleman = walk.teams[0].classes[0].name.clone();
    assert_eq!(profile.career.class_kills.get(&rifleman), Some(&1));
    assert_eq!(profile.career.class_kills.get("all_inf_officer"), None);
}
