use super::*;
use crate::walk::WalkSim;

#[test]
fn tutorial_followup_class_change_survives_a_disabled_capture_region() {
    let (missions, baked) = baked().expect("tutorial bake");
    let (_, mut walk, _) = setup_baked_campaign(&missions, &baked, "geo1c_c");
    let ci = walk
        .sim
        .cps
        .iter()
        .position(|c| c.def.name == "c_cp3")
        .unwrap();
    let cp = &mut walk.sim.cps[ci];
    cp.killed = false;
    cp.owner = Some(0);
    cp.def.capture = Region::sphere(Vec3::ZERO, 0.0);
    cp.capture_authored = false;
    let pos = cp.def.pos;
    let u = &mut walk.sim.units[walk.player.index as usize];
    u.alive = true;
    u.soldier.pos = pos + Vec3::Z * 2.0;
    u.soldier.yaw = 0.0;
    u.soldier.pitch = 0.0;
    assert_eq!(
        walk.sim.class_change_post(walk.player.index as usize),
        Some(ci)
    );
    walk.sim.cps[ci].killed = true;
    assert_ne!(
        walk.sim.class_change_post(walk.player.index as usize),
        Some(ci)
    );
}

#[test]
fn tutorial_followup_script_accepts_mace_without_a_score_offer() {
    use crate::campaign_script::Host;
    let (missions, baked) = baked().expect("tutorial bake");
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions, &baked, "geo1c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let class = walk.sim.classes[0].len() as u8;
    let hero = walk.sim.classes[0][0].clone();
    walk.sim.classes[0].push(hero);
    rt.w.team_classes[0].push("rep_hero_macewindu".into());
    walk.sim.heroes = Some(core::sim::heroes::Heroes::new([None; 2]));
    let tickets = walk.sim.battle.teams[0].tickets();
    let mut host = Ctx {
        w: &mut rt.w,
        sim: &mut walk.sim,
    };
    assert!(host.known("AcceptHero"));
    host.call(
        &mut rt.interp,
        "SetHeroClass",
        &[Value::Num(1.0), Value::Str("rep_hero_macewindu".into())],
    )
    .unwrap();
    assert_eq!(host.sim.hero_class(0), Some(class));
    host.call(&mut rt.interp, "AcceptHero", &[Value::Num(0.0)])
        .unwrap();
    assert_eq!(host.sim.spawn_choice[0], class);
    assert!(host.sim.heroes.as_ref().unwrap().teams[0].owner.is_some());
    assert_eq!(host.sim.battle.teams[0].tickets(), tickets);
}

#[test]
fn tutorial_objective_zone_does_not_show_a_capture_meter() {
    let (missions, baked) = baked().expect("campaign tutorial bake required");
    let (_, mut walk, _) = setup_baked_campaign(&missions, &baked, "geo1c_c");
    walk.sim.cps.retain(|cp| cp.def.name == "goto");
    assert_eq!(walk.sim.cps.len(), 1);
    assert!(!walk.sim.cps[0].capturable);
    let pos = walk.sim.cps[0].def.pos;
    // Also cover untakeable zones with an authored, nonempty region.
    walk.sim.cps[0].def.capture = core::region::Region::sphere(pos, 8.0);
    let player = &mut walk.sim.units[walk.player.index as usize];
    player.soldier.pos = pos;
    player.alive = true;
    player.capture_posts = true;
    assert!(walk.sim.counts_in_cp(walk.player.index as usize, 0));
    assert!(crate::capture_feed::for_unit(
        &walk.sim,
        walk.player.index as usize,
        crate::capture_feed::Rules::PORT
    )
    .is_none());
    walk.sim.cps[0].capturable = true;
    walk.sim.cps[0].owner = None;
    assert!(crate::capture_feed::for_unit(
        &walk.sim,
        walk.player.index as usize,
        crate::capture_feed::Rules::PORT
    )
    .is_some());
}

#[test]
fn tutorial_markers_keep_repair_targets_and_map_only_regions() {
    let (missions, baked) = baked().expect("campaign tutorial bake required");
    let (_, mut walk, mut rt) = setup_baked_campaign(&missions, &baked, "geo1c_c");
    let station = walk.sim.stations.first_mut().expect("tutorial droids");
    station.destroyed = true;
    station.health.cur = 0.0;
    let pos = station.pos;
    let team = rt.w.player_team + 1;
    rt.w.markers = vec![Marker {
        target: MarkTarget::Obj(Ud::Station(0)),
        icon: "hud_objective_icon_circle".into(),
        scale: 3.0,
        team,
        pulse: true,
    }];
    assert_eq!(rt.objective_markers(&walk.sim)[0].pos, pos + Vec3::Y * 2.0);
    assert_eq!(rt.map_markers(&walk.sim)[0].pos, pos);
    let class = rt.w.station_classes[0].clone();
    rt.w.markers[0].target = MarkTarget::Class(class);
    assert!(rt
        .objective_markers(&walk.sim)
        .iter()
        .any(|m| m.pos == pos + Vec3::Y * 2.0));
    rt.w.markers[0].target = MarkTarget::Region(0);
    assert!(rt.objective_markers(&walk.sim).is_empty());
    assert_eq!(rt.map_markers(&walk.sim)[0].pos, rt.w.regions[0].1.center);
    rt.w.markers[0].target = MarkTarget::Obj(Ud::Station(0));
    rt.w.markers[0].pulse = false;
    assert!(rt.objective_markers(&walk.sim).is_empty());
    assert_eq!(rt.map_markers(&walk.sim).len(), 1);
    rt.w.markers[0].team = team + 1;
    assert!(rt.map_markers(&walk.sim).is_empty());
    rt.w.markers.clear();
    assert!(rt.objective_markers(&walk.sim).is_empty());
}

fn setup_baked_campaign(
    missions_dir: &std::path::Path,
    baked_dir: &std::path::Path,
    script: &str,
) -> (crate::mission::MissionDef, WalkSim, CampaignRuntime) {
    setup_baked_campaign_with(missions_dir, baked_dir, script, None)
}

/// [`setup_baked_campaign`] with `hero` appended to the first army's roster
/// as the game's battle setup does (`heroes::append`).
fn setup_baked_campaign_with(
    missions_dir: &std::path::Path,
    baked_dir: &std::path::Path,
    script: &str,
    hero: Option<&str>,
) -> (crate::mission::MissionDef, WalkSim, CampaignRuntime) {
    let def = crate::mission::MissionDef::load_campaign(missions_dir, script)
        .unwrap_or_else(|e| panic!("load campaign {script}: {e:#}"));
    let m_path = baked_dir.join(&def.map).join("manifest.ron");
    let m_text = std::fs::read_to_string(&m_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", m_path.display()));
    let mut manifest: crate::assets::Manifest =
        ron::from_str(&m_text).unwrap_or_else(|e| panic!("parse {}: {e}", m_path.display()));
    manifest.spawn_only_posts = false;
    for req in &def.layers {
        if let Some(idx) = crate::assets::resolve_campaign_layer_index(&manifest, req) {
            let layer = manifest.layers.remove(idx);
            manifest.regions.extend(layer.regions);
            manifest.instances.extend(layer.instances);
        }
    }
    let mut layout = crate::layout::MapLayout::from_mission_manifest(&manifest, false, true);
    layout.apply_mission(&def);
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    walk.add_posts(&layout);
    let post_classes = crate::walk::hunt_post_classes(
        &baked_dir.join("classes"),
        &baked_dir.join(&def.map),
        "bf2",
    )
    .unwrap();
    walk.set_hunt_class_change(&layout, &post_classes, "bf2");
    let factions = [def.teams[0].faction.clone(), def.teams[1].faction.clone()];
    let space = def
        .space_assault
        .as_ref()
        .filter(|_| def.mode == "ass" || (def.is_campaign() && def.map.contains("_spa")))
        .map(|s| (s, 180));
    let _ = crate::vehicle_pads::install_for_mission(
        &mut walk.sim,
        &manifest,
        &baked_dir.join(&def.map),
        &baked_dir.join("classes"),
        &factions,
        space,
        &def.class_load_order,
    );
    let u = &walk.sim.units[walk.player.index as usize];
    let base_uc = core::unit::UnitClass {
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
    };
    for t in 0..2 {
        if !def.teams[t].classes.is_empty() {
            walk.sim.classes[t] = def.teams[t]
                .classes
                .iter()
                .map(|(_, min, max)| {
                    let mut uc = base_uc.clone();
                    uc.min = *min;
                    uc.max = *max;
                    uc
                })
                .collect();
            walk.sim.class_enabled[t] = def.teams[t]
                .classes
                .iter()
                .map(|(name, _, _)| def.teams[t].is_class_initially_enabled(name))
                .collect();
        }
        walk.sim
            .set_ai_spawn_allowed(t as u8, def.teams[t].allows_ai_spawn_initially());
    }
    crate::native_teams::install(
        &mut walk,
        &layout,
        &baked_dir.join("classes"),
        baked_dir,
        &def,
        &manifest,
    );
    if hero.is_some() {
        let kit = walk.sim.classes[0][0].clone();
        walk.sim.classes[0].push(kit);
        walk.sim.class_enabled[0].push(true);
        let index = (walk.sim.classes[0].len() - 1) as u8;
        walk.sim.heroes = Some(core::sim::heroes::Heroes::new([Some(index), None]));
    }
    walk.sim.battle.campaign_rules = true;
    let spec = def
        .campaign
        .clone()
        .unwrap_or_else(|| panic!("{script} missing campaign spec"));
    walk.sim.battle.unlimited_teams = spec.infinite_tickets;
    let saved = CampaignRuntime::kill_startup_posts(&mut walk.sim, &def.killed);
    let mut rt = CampaignRuntime::new(spec, &layout, &manifest, &walk.post_keys, saved, &walk.sim);
    rt.set_team_rosters(team_classes(&def), [def.teams[0].units, def.teams[1].units]);
    rt.set_native_rosters(
        def.local_teams
            .iter()
            .filter_map(|lt| {
                let team = lt.team.checked_sub(1).filter(|t| *t >= 2)?;
                Some((
                    team,
                    (lt.classes.iter().map(|c| c.0.clone()).collect(), lt.units),
                ))
            })
            .collect(),
    );
    rt.start(&mut walk.sim);
    walk.step_tick(&[core::sim::PadState::default()]);
    walk.sim
        .events
        .push(core::sim::Event::Respawn { unit: walk.player });
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &[],
        core::TICK_DT,
    );
    (def, walk, rt)
}

fn team_classes(def: &crate::mission::MissionDef) -> [Vec<String>; 2] {
    std::array::from_fn(|t| {
        def.teams[t]
            .classes
            .iter()
            .map(|(c, _, _)| c.clone())
            .collect()
    })
}

fn tick(walk: &mut WalkSim, rt: &mut CampaignRuntime, def: &crate::mission::MissionDef) {
    walk.step_tick(&[core::sim::PadState::default()]);
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(def),
        &BTreeMap::new(),
        &[],
        core::TICK_DT,
    );
}

/// geo1c's first spawn starts a 12 s timer (`BeginObjectivesTimer`)
/// whose elapse builds the objective sequence.
fn begin_objectives(
    walk: &mut WalkSim,
    rt: &mut CampaignRuntime,
    def: &crate::mission::MissionDef,
) {
    for _ in 0..(13 * core::TICK_HZ) {
        if rt.popup().is_some() {
            return;
        }
        tick(walk, rt, def);
    }
}

fn baked() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let missions_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions");
    let baked_dir = crate::assets::baked_root();
    (missions_dir.join("campaign/bf2_geo1c_c.ron").is_file()
        && baked_dir.join("bf2_geo1/manifest.ron").is_file())
    .then_some((missions_dir, baked_dir))
}

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
fn geo1c_opens_with_its_first_objective_popup_marker_and_voice() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    assert!(rt.take_voice_overs().iter().any(|v| v == "GEO_obj_18"));
    assert!(rt.popup().is_none());
    begin_objectives(&mut walk, &mut rt, &def);
    assert_eq!(
        rt.popup(),
        Some(&Popup::Objective(
            "level.geo1.objectives.campaign.1_popup".into()
        ))
    );
    assert!(rt
        .objectives()
        .iter()
        .any(|o| o.text == "level.geo1.objectives.campaign.1" && o.state == 2));
    assert!(rt.take_voice_overs().iter().any(|v| v == "GEO_obj_54"));
    assert!(!rt.objective_markers(&walk.sim).is_empty());
    for _ in 0..core::TICK_HZ {
        tick(&mut walk, &mut rt, &def);
    }
    assert!(rt.objectives_updated());
    assert_eq!(
        rt.errors().count(),
        0,
        "{:?}",
        rt.errors().collect::<Vec<_>>()
    );
}

#[test]
fn geo1c_goto_spawns_the_scout_ambush_and_marks_it() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let goto = rt.w.regions[rt.w.region_id("goto1").expect("goto1 region") as usize]
        .1
        .center;
    let natives_before = walk
        .sim
        .units
        .iter()
        .filter(|u| u.alive && u.team == 3)
        .count();
    let mut completed = false;
    for _ in 0..(8 * core::TICK_HZ) {
        let p = walk.player.index as usize;
        walk.sim.units[p].soldier.pos = goto;
        tick(&mut walk, &mut rt, &def);
        completed |= rt
            .take_messages()
            .iter()
            .any(|m| m == "game.objectives.complete");
        rt.close_popup();
    }
    assert!(completed, "Objective1 did not complete in goto1");
    assert!(rt
        .objectives()
        .iter()
        .any(|o| o.text == "level.geo1.objectives.campaign.2a" && o.state == 2));
    let natives = walk
        .sim
        .units
        .iter()
        .filter(|u| u.alive && u.team == 3)
        .count();
    assert!(
        natives >= natives_before + 3,
        "ambush spawned {} of 3",
        natives - natives_before
    );
    assert!(rt.objective_markers(&walk.sim).len() >= 3);
    assert_eq!(
        rt.errors().count(),
        0,
        "{:?}",
        rt.errors().collect::<Vec<_>>()
    );
}

#[test]
fn geo1c_scout_kills_complete_objective_2a() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let goto = rt.w.regions[rt.w.region_id("goto1").expect("goto1 region") as usize]
        .1
        .center;
    let active = |rt: &CampaignRuntime, text: &str| {
        rt.objectives()
            .iter()
            .any(|o| o.text == text && o.state == 2)
    };
    for _ in 0..(8 * core::TICK_HZ) {
        walk.sim.units[walk.player.index as usize].soldier.pos = goto;
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    assert!(active(&rt, "level.geo1.objectives.campaign.2a"));
    let player = walk.player;
    for _ in 0..(8 * core::TICK_HZ) {
        let scouts: Vec<UnitHandle> = walk
            .sim
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.alive && u.team == 3)
            .map(|(i, u)| UnitHandle {
                index: i as u32,
                gen: u.gen,
            })
            .collect();
        walk.step_tick(&[core::sim::PadState::default()]);
        for h in scouts {
            if walk.sim.unit(h).is_some_and(|u| u.alive) {
                walk.sim.units[h.index as usize].health.cur = 0.0;
                walk.sim.units[h.index as usize].alive = false;
                walk.sim.events.push(core::sim::Event::Kill {
                    shooter: player,
                    victim: h,
                });
            }
        }
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &team_classes(&def),
            &BTreeMap::new(),
            &[],
            core::TICK_DT,
        );
        rt.close_popup();
        if active(&rt, "level.geo1.objectives.campaign.2b") {
            break;
        }
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text == "level.geo1.objectives.campaign.2a" && o.state == 3),
        "{:?}",
        rt.objectives()
    );
    assert!(active(&rt, "level.geo1.objectives.campaign.2b"));
    for _ in 0..(20 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    assert!(
        !walk.sim.units.iter().any(|u| u.alive && u.team == 3),
        "a SpawnCharacter scout left a post and respawned"
    );
    assert_eq!(
        rt.errors().count(),
        0,
        "{:?}",
        rt.errors().collect::<Vec<_>>()
    );
}

/// geo1c's opening (fn#1 ScriptPostLoad, fn#6 Objective1.OnStart): no
/// post spawns either army (cis_spawn1 and the c_cp posts start dead or
/// neutral, obj3_wheel has no SpawnPath), so the player stands alone; and
/// Objective1 locks the pad's hailfire at 5e6 of 8e7 health with a
/// `DisableTime` of 1e37, so no soldier can board it.
#[test]
fn geo1c_opens_with_no_army_on_the_field_and_the_hailfire_shut() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    let others = |walk: &WalkSim| {
        walk.sim
            .units
            .iter()
            .enumerate()
            .filter(|(i, u)| u.alive && *i != walk.player.index as usize)
            .count()
    };
    begin_objectives(&mut walk, &mut rt, &def);
    assert_eq!(others(&walk), 0);
    rt.close_popup();
    for _ in 0..(2 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    assert_eq!(others(&walk), 0);
    let pad = walk
        .sim
        .vehicle_pads
        .iter()
        .find(|p| p.def.name == "obj3_wheeltank")
        .expect("obj3_wheeltank pad");
    let v = &walk.sim.vehicles[pad.vehicles.first().expect("hailfire").handle.index as usize];
    assert_eq!((v.health.cur, v.health.max), (5e6, 8e7));
    assert!(v.disable > 1e36);
    assert!(v.held_against(walk.player, walk.sim.tick));
}

/// geo1c's Objective6a (fn#50) completes on `GetCharacterClass == 1`, the
/// engineer `AddUnitClass` adds second. The game appends the scripted hero
/// to the roster before the script starts (`heroes::append`); he must not
/// take a place in that numbering.
#[test]
fn geo1c_engineer_completes_objective_6a_with_the_hero_in_the_roster() {
    let (missions, baked) = baked().expect("tutorial bake");
    let (def, mut walk, mut rt) =
        setup_baked_campaign_with(&missions, &baked, "geo1c_c", Some("rep_hero_macewindu"));
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let call = |rt: &mut CampaignRuntime, walk: &mut WalkSim, obj: &str, method: &str, arg: f32| {
        let this = rt.interp.global(obj);
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        let f = rt
            .interp
            .index(&mut host, &this, &Value::str(method))
            .unwrap();
        rt.interp
            .call_value(&mut host, &f, vec![this.clone(), Value::Num(arg)])
            .unwrap();
    };
    call(
        &mut rt,
        &mut walk,
        "objectiveSequence",
        "ActivateObjectiveSet",
        4.0,
    );
    for _ in 0..30 {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    call(&mut rt, &mut walk, "Objective2", "Complete", 1.0);
    let engineer = team_classes(&def)[0]
        .iter()
        .position(|c| c == "rep_inf_ep2_engineer")
        .unwrap();
    walk.sim.players[0] = Some(walk.player);
    let mut changed = false;
    for _ in 0..(12 * core::TICK_HZ) {
        if !changed && walk.sim.is_class_enabled(0, engineer) {
            let u = &mut walk.sim.units[walk.player.index as usize];
            u.alive = true;
            u.health.cur = u.health.max;
            assert!(walk.sim.test_change_class(0, engineer as u8, false));
            walk.player = walk.sim.players[0].unwrap();
            changed = true;
        }
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    assert!(changed, "Objective6a never added the engineer");
    let state = |text: &str| {
        rt.objectives()
            .iter()
            .find(|o| o.text == text)
            .map(|o| o.state)
    };
    assert_eq!(state("level.geo1.objectives.campaign.6a"), Some(3));
    assert_eq!(state("level.geo1.objectives.campaign.6"), Some(2));
    assert_eq!(
        rt.errors().count(),
        0,
        "{:?}",
        rt.errors().collect::<Vec<_>>()
    );
}

/// The user's spider that had to die twice: a walker's `VehicleDestroyed`
/// comes at the end of its death clip, after the player may have switched
/// weapon, so the last hit's weapon is kept at the hit (the round's
/// `Impact` on that hull), as `GetObjectLastHitWeaponClass` reads it.
#[test]
fn a_walker_kill_keeps_the_weapon_of_its_last_hit() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    let (vi, v) = walk
        .sim
        .vehicles
        .iter()
        .enumerate()
        .find(|(_, v)| v.alive)
        .expect("a vehicle on geo1c");
    let vehicle = core::vehicle::VehicleHandle {
        index: vi as u32,
        gen: v.gen,
    };
    let (pos, class) = (v.pos, v.class);
    assert!(walk.sim.weapon_classes.len() >= 2);
    let (rocket, rifle) = (0, 1);
    let rocket_name = walk.sim.weapon_classes[rocket].name.to_ascii_lowercase();
    let player = walk.player;
    let step = |walk: &mut WalkSim, rt: &mut CampaignRuntime| {
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &team_classes(&def),
            &BTreeMap::new(),
            &[],
            core::TICK_DT,
        );
    };
    walk.sim.units[player.index as usize].weapon_class = rocket;
    walk.sim.events.clear();
    walk.sim.events.push(core::sim::Event::VehicleHit {
        shooter: Some(player),
        vehicle,
        damage: 500.0,
        dir: None,
    });
    walk.sim.events.push(core::sim::Event::Impact {
        pos,
        dir: Vec3::Z,
        weapon_class: rocket as u32,
        surface: core::impact::Surface::Vehicle,
        vehicle: Some(vehicle),
    });
    step(&mut walk, &mut rt);
    walk.sim.units[player.index as usize].weapon_class = rifle;
    walk.sim.events.clear();
    walk.sim.events.push(core::sim::Event::VehicleDestroyed {
        vehicle,
        class,
        pos,
        vel: Vec3::ZERO,
        killer: Some(player),
    });
    step(&mut walk, &mut rt);
    let obj = Ud::Vehicle(vehicle.index, vehicle.gen);
    let last =
        rt.w.last_hit_weapon
            .iter()
            .find(|(o, _)| *o == obj)
            .map(|(_, w)| w.as_str());
    assert_eq!(last, Some(rocket_name.as_str()));
}

/// geo1c's script fills the goal manager (`core::goals`): its first
/// objective adds a Deathmatch goal of weight 1 for each army beside the
/// locals' Deathmatch 100 from `ScriptPostLoad`, and completing it deletes
/// the two.
#[test]
fn geo1c_objective_goals_reach_the_goal_manager() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    let goals = |walk: &WalkSim| -> Vec<(u8, core::goals::GoalKind, f32)> {
        walk.sim
            .ai_goals
            .slots
            .iter()
            .flatten()
            .map(|g| (g.team, g.kind, g.weight))
            .collect()
    };
    use core::goals::GoalKind::Deathmatch;
    assert_eq!(
        goals(&walk),
        vec![
            (2, Deathmatch, 100.0),
            (0, Deathmatch, 1.0),
            (1, Deathmatch, 1.0)
        ]
    );
    assert!(rt.complete_objective(&mut walk.sim, "Objective1"));
    // `Objective:Complete` runs OnComplete from its 1 s ditty timer.
    for _ in 0..2 * core::TICK_HZ {
        tick(&mut walk, &mut rt, &def);
    }
    assert_eq!(goals(&walk)[0], (2, Deathmatch, 100.0));
    assert!(!goals(&walk).contains(&(1, Deathmatch, 1.0)));
}
