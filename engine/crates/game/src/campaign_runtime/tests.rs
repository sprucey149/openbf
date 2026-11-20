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
    let space_def = def.space_round();
    let space = space_def.as_ref().map(|s| (s, 180));
    if let Ok(pads) = crate::vehicle_pads::install_for_mission(
        &mut walk.sim,
        &manifest,
        &baked_dir.join(&def.map),
        &baked_dir.join("classes"),
        &factions,
        space,
        &def.class_load_order,
    ) {
        walk.vehicle_classes = pads.classes;
    }
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
    rt.set_team_factions([def.teams[0].faction.as_str(), def.teams[1].faction.as_str()]);
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
    rt.load_world_anims(&baked_dir.join(&def.map), &manifest, &walk.sim);
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
        &walk.vehicle_classes,
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
/// myg1c's autoturrets fire unmanned: their `Shot` names no unit (index
/// u32::MAX). The killer table once grew to that index, a 4-billion-entry
/// allocation that froze the game for seconds at the turret's first shot.
#[test]
fn an_unmanned_turret_shot_leaves_the_killer_table_alone() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: geo1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
    walk.sim.events.clear();
    walk.sim.events.push(core::sim::Event::Shot {
        unit: core::unit::UnitHandle { index: u32::MAX, gen: 2 },
        weapon_class: 0,
        origin: Vec3::ZERO,
        dir: Vec3::Z,
    });
    let t = std::time::Instant::now();
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &[],
        core::TICK_DT,
    );
    assert!(t.elapsed().as_millis() < 200, "{:?}", t.elapsed());
    assert!(rt.w.last_shot.len() <= walk.sim.units.len());
}

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
        let text = std::fs::read_to_string(
            missions_dir.join(format!("campaign/bf2_{}.ron", m.script)),
        )
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
        let text = std::fs::read_to_string(
            missions_dir.join(format!("campaign/bf2_{}.ron", m.script)),
        )
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
    assert!(missing.is_empty(), "{}", missing.join("
"));
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
    let track = crate::sounds::MusicTrack { def, gain: 1.0, takes: Vec::new(), ids: Vec::new() };
    let mut m = BattleMusic::new(&Default::default(), true, [100, 100]);
    m.bind_with(|_| None);
    assert_eq!(
        m.script_call(
            InGameMusic::Play { name: "rep_myg_act_01".into(), replay: 15.0 },
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

#[test]
fn myg1c_script_voice_lines_reach_the_commander_queue() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "myg1c_c");
    let lib = crate::sounds::SoundLib::load_for(&baked_dir.join(&def.map), &def)
        .expect("myg1 sound library");
    let mut v = crate::announce::Commander::for_battle(
        &def,
        &crate::layout::MapLayout::default(),
        &walk,
    );
    begin_objectives(&mut walk, &mut rt, &def);
    let mut heard = Vec::new();
    for _ in 0..(10 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        v.ask_script_sounds(rt.take_sounds());
        assert!(v.gate_script_sounds(&lib).is_empty());
        heard.extend(v.queue.pending().iter().map(|l| l.name.clone()));
    }
    assert!(
        heard.iter().any(|n| n.starts_with("MYG_obj_")),
        "no MYG objective line queued: {heard:?}"
    );
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
    assert!(lib.stream_id("NAB_obj_22").is_some(), "NAB_obj_22 is a stream line");

    let mut v = crate::announce::Commander::for_battle(
        &def,
        &crate::layout::MapLayout::default(),
        &walk,
    );
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
    v.queue
        .ask(core::announcer::Line::commander("imp_off_com_report_us_overwhelmed"));
    frame(&mut v, &mut played);
    assert_eq!(played, ["imp_off_com_report_us_overwhelmed"]);
    v.ask_script_sounds(vec!["NAB_obj_22".into()]);
    assert!(v.gate_script_sounds(&lib).is_empty());
    for _ in 0..(4 * core::TICK_HZ) {
        frame(&mut v, &mut played);
    }
    assert_eq!(played, ["imp_off_com_report_us_overwhelmed"], "nothing over the line");
    for _ in 0..(2 * core::TICK_HZ) {
        frame(&mut v, &mut played);
    }
    assert_eq!(played, lines, "the objective line next");
}

#[test]
fn myg1c_walk_from_spawn_to_the_objective_has_no_long_tick() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: myg1c_c mission and baked assets required");
        return;
    };
    if !baked_dir.join("bf2_myg1/manifest.ron").is_file() {
        eprintln!("SKIP: bf2_myg1 baked assets required");
        return;
    }
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "myg1c_c");
    let route = [
        Vec3::new(-123.0, 0.0, 299.0),
        Vec3::new(-80.0, 0.0, 231.0),
        Vec3::new(-64.0, 0.0, 198.0),
        Vec3::new(-62.0, 0.0, 140.0),
        Vec3::new(-64.0, 5.0, 60.0),
    ];
    let mut worst = (0.0f64, 0usize);
    let total = 40 * core::TICK_HZ as usize;
    for i in 0..total {
        let f = i as f32 / total as f32 * (route.len() - 1) as f32;
        let k = (f as usize).min(route.len() - 2);
        let pos = route[k].lerp(route[k + 1], f - k as f32);
        let p = walk.player.index as usize;
        walk.sim.units[p].soldier.pos = pos;
        let t0 = std::time::Instant::now();
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        if ms > worst.0 {
            worst = (ms, i);
        }
    }
    eprintln!("worst tick {:.1} ms at {}", worst.0, worst.1);
    assert!(worst.0 < 500.0, "tick {} took {:.0} ms", worst.1, worst.0);
}

#[test]
fn spa2c_comm_array_is_a_destructible_station() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (_def, walk, rt) = setup_baked_campaign(&missions_dir, &baked_dir, "spa2c_c");
    for name in ["comms_cis", "cisbridge01", "ciseng01", "ciseng02"] {
        let Some(Ud::Station(si)) = rt.w.resolve_name(&walk.sim, name) else {
            panic!("{name} is not a station: {:?}", rt.w.resolve_name(&walk.sim, name));
        };
        let st = &walk.sim.stations[usize::from(si)];
        assert!(st.health.max > 0.0 && st.health.max.is_finite() && !st.destroyed, "{name}");
    }
}

#[test]
fn myg1c_core_shield_animation_drops_the_shield_station() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: myg1c_c mission and baked assets required");
        return;
    };
    if !baked_dir
        .join("bf2_myg1")
        .join(manifest::world_anim::FILE)
        .exists()
    {
        eprintln!("SKIP: bf2_myg1 world animations not baked");
        return;
    }
    let (_def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "myg1c_c");
    let Some(Ud::Station(si)) = rt.w.resolve_name(&walk.sim, "cforce_shield_04") else {
        panic!("cforce_shield_04 is not a station");
    };
    let si = usize::from(si);
    assert_eq!(rt.w.anims.stations("cshield_down_04"), vec![Some(si)]);
    let before = walk.sim.stations[si].transform.w_axis.truncate();
    rt.w.anims.play("cshield_down_04");
    for _ in 0..3 {
        rt.w.anims.step(&mut walk.sim, 1.0 / core::TICK_HZ as f32);
    }
    let after = walk.sim.stations[si].transform.w_axis.truncate();
    assert!((before.y - 9000.0 - after.y).abs() < 1.0, "{before} -> {after}");
    assert!((before.x - after.x).abs() < 1e-3 && (before.z - after.z).abs() < 1e-3);
}

/// Coruscant's "Go to library" objective plays `DoorOpen01`, a world
/// animation whose entries are placed prop instances (the library doors),
/// not sim stations. The port must pose those instances so the doors open.
#[test]
fn coruscant_library_door_animation_poses_its_instances() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: cor1c_c mission and baked assets required");
        return;
    };
    if !baked_dir
        .join("bf2_cor1")
        .join(manifest::world_anim::FILE)
        .exists()
    {
        eprintln!("SKIP: bf2_cor1 world animations not baked");
        return;
    }
    let (_def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "cor1c_c");
    let door =
        rt.w.instances
            .iter()
            .position(|(n, _, _)| n == "leftlibrarydoor")
            .expect("leftlibrarydoor is a placed instance") as u32;
    let base_y = rt.w.instances[door as usize].2.y;
    let bodies_before: Vec<(usize, glam::Vec3)> = walk
        .sim
        .collision
        .instance_bodies(door)
        .into_iter()
        .map(|(i, m)| (i, m.w_axis.truncate()))
        .collect();
    assert!(!bodies_before.is_empty(), "the door has collision bodies");
    assert!(
        rt.instance_poses().is_empty(),
        "no door pose before playing"
    );

    rt.w.anims.play("DoorOpen01");
    // The doors' position keys run 0..4 s; step past the end.
    for _ in 0..(core::TICK_HZ * 5) {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let m = rt
        .instance_poses()
        .get(&door)
        .unwrap_or_else(|| panic!("DoorOpen01 did not pose leftlibrarydoor"));
    assert!(
        m.w_axis.y > base_y + 5.0,
        "the door slid up: base y {base_y}, posed y {}",
        m.w_axis.y
    );
    for (i, before) in bodies_before {
        let after = walk.sim.collision.bodies()[i].to_world.w_axis.truncate();
        assert!(
            after.y > before.y + 5.0,
            "door body {i} collision must rise with the draw: {before} -> {after}"
        );
    }
}

/// Regression test for Mustafar (`mus1c_c`) lava bridge: `lava_bridge_drop`
/// lowers `lavatrap` by 7 m at mission start, and `lava_bridge_raise` raises it
/// back to its rest height for the "Proceed forward across the bridge"
/// objective. Both groups drive the same world instance, so the second group
/// must move collision from the instance's rest-pose offset rather than baking
/// in the first group's -7 m displacement.
#[test]
fn mus1c_c_lava_bridge_collision_rises_back_with_visual_pose() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: mus1c_c mission and baked assets required");
        return;
    };
    if !baked_dir
        .join("bf2_mus1")
        .join(manifest::world_anim::FILE)
        .exists()
    {
        eprintln!("SKIP: bf2_mus1 world animations not baked");
        return;
    }
    let (_def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "mus1c_c");
    let bridge =
        rt.w.instances
            .iter()
            .position(|(n, _, _)| n == "lavatrap")
            .expect("lavatrap is a placed instance") as u32;
    let base_y = rt.w.instances[bridge as usize].2.y;
    let body_indices: Vec<usize> = walk
        .sim
        .collision
        .instance_bodies(bridge)
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    assert!(!body_indices.is_empty(), "lavatrap has collision bodies");

    // Step past the 3 s drop started by ScriptPostLoad's PlayAnimDrop().
    for _ in 0..(core::TICK_HZ * 4) {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let dropped_pose = rt
        .instance_poses()
        .get(&bridge)
        .expect("lava_bridge_drop posed lavatrap");
    assert!(
        (dropped_pose.w_axis.y - (base_y - 7.0)).abs() < 0.05,
        "bridge dropped 7 m: base {base_y}, pose {}",
        dropped_pose.w_axis.y
    );
    for &i in &body_indices {
        let y = walk.sim.collision.bodies()[i].to_world.w_axis.y;
        assert!(
            (y - (base_y - 7.0)).abs() < 0.05,
            "bridge body {i} dropped 7 m: base {base_y}, body {y}"
        );
    }

    // Now play PlayAnimRise() sequence: pause drop, rewind & play raise (4 s).
    rt.w.anims.pause("lava_bridge_drop");
    rt.w.anims.rewind("lava_bridge_raise");
    rt.w.anims.play("lava_bridge_raise");
    for _ in 0..(core::TICK_HZ * 5) {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let raised_pose = rt
        .instance_poses()
        .get(&bridge)
        .expect("lava_bridge_raise posed lavatrap");
    assert!(
        (raised_pose.w_axis.y - base_y).abs() < 0.05,
        "bridge visual returned to base {base_y}, got {}",
        raised_pose.w_axis.y
    );
    for &i in &body_indices {
        let y = walk.sim.collision.bodies()[i].to_world.w_axis.y;
        assert!(
            (y - base_y).abs() < 0.05,
            "bridge body {i} collision must return to base {base_y}, got {y}"
        );
    }
}

/// Regression test for Mustafar (`mus1c_c`) orbital strike beacon (`bomb2`):
/// `bomb_spawn2` sits above the conference room floor, so `mus_flag_bomb` must
/// settle onto the floor under gravity (`SWBFRNT2.ELF` `FUN_0035e058` ->
/// `FUN_00225578`) instead of floating in midair.
#[test]
fn mus1c_c_beacon_settles_onto_conference_room_floor() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: mus1c_c mission and baked assets required");
        return;
    };
    let (_def, mut walk, rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "mus1c_c");
    let spawn = rt
        .w
        .paths
        .iter()
        .find(|(n, _)| n == "bomb_spawn2")
        .and_then(|(_, pts)| pts.first().copied())
        .map(|(p, _)| p)
        .expect("bomb_spawn2 path point");
    let mut ctf = core::sim::ctf::CtfState::new_ground_one_flag(
        u32::MAX,
        spawn,
        glam::Vec3::new(0.0, -10_000.0, 0.0),
        glam::Vec3::new(0.0, -10_000.0, 0.0),
    );
    ctf.flags[0].allow_ai_pickup = false;
    walk.sim.ctf = Some(ctf);
    for _ in 0..(core::TICK_HZ * 2) {
        walk.step_tick(&[core::sim::PadState::default()]);
    }
    let flag = &walk.sim.ctf.as_ref().unwrap().flags[0];
    assert!(flag.landed, "beacon should land on the conference room floor");
    assert!(
        flag.current_pos.y < spawn.y - 1.0 && flag.current_pos.y > spawn.y - 5.0,
        "beacon should drop from bomb_spawn2 y={} onto the floor, got y={}",
        spawn.y,
        flag.current_pos.y
    );
}

/// Regression test for Kamino (`kam1c_c`):
/// 1. Imperial AI teammates spawn at `cp5` after `onfirstspawn`, not piled on
///    top of the player at `boba`.
/// 2. `ambushTeam1` (Team 4) distributes classes up to each class's `max`
///    instead of putting 11/12 into class 0.
/// 3. `transport1` (`uta_fly_ride_gunship`) moves along its `local: true`
///    world animation `anim1` (`PlayAnimation("test")`) and carries
///    `PilotType=self` gunners (`self_pilots`) so it shoots enemies in flight.
#[test]
fn kam1c_c_opening_spawn_class_mix_and_flying_gunship() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: kam1c_c mission and baked assets required");
        return;
    };
    if !baked_dir
        .join("bf2_kam1")
        .join(manifest::world_anim::FILE)
        .exists()
    {
        eprintln!("SKIP: bf2_kam1 world animations not baked");
        return;
    }
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "kam1c_c");
    let team_classes: [Vec<String>; 2] = [0, 1].map(|t| {
        walk.teams[t]
            .classes
            .iter()
            .map(|c| c.name.clone())
            .collect()
    });
    let native_classes: std::collections::BTreeMap<u8, Vec<String>> = def
        .local_teams
        .iter()
        .filter_map(|lt| {
            let team = lt.team.checked_sub(1).filter(|t| *t >= 2)?;
            Some((team, lt.classes.iter().map(|c| c.0.clone()).collect()))
        })
        .collect();
    let vehicle_classes = walk.vehicle_classes.clone();

    // 1. Simulate the real battle.rs + spawn-screen startup: no AI spawned
    //    before `onfirstspawn`, then after `onfirstspawn` switches `boba` to
    //    neutral and `cp5` to Team 1, `defer_opening_wave` spawns all Imperial
    //    AI near `cp5` instead of piling them on `boba`.
    walk.sim.units.retain(|u| u.is_player);
    rt.set_team_rosters(team_classes.clone(), [11, 12]);
    rt.defer_opening_wave();
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes,
        &native_classes,
        &vehicle_classes,
        core::TICK_DT,
    );
    let boba_pos = glam::Vec3::new(-69.235, 73.028, 82.685);
    let cp5_pos = glam::Vec3::new(22.0, 78.428, -76.0);
    let imp_ai: Vec<glam::Vec3> = walk
        .sim
        .units
        .iter()
        .filter(|u| u.alive && !u.is_player && u.team == 0)
        .map(|u| u.soldier.pos)
        .collect();
    assert!(
        imp_ai.len() >= 11,
        "Imperial AI teammates spawned (got {})",
        imp_ai.len()
    );
    let near_cp5 = imp_ai
        .iter()
        .filter(|p| p.distance(cp5_pos) < p.distance(boba_pos))
        .count();
    assert_eq!(
        near_cp5,
        imp_ai.len(),
        "all Imperial AI should spawn near cp5, not boba ({near_cp5}/{})",
        imp_ai.len()
    );

    // 2. Team 4 (`ambushTeam1`, sim team 3): its members draw with
    //    `FUN_0036f198` under the first minute's reserve, where a class
    //    needs `count < max - 1`: 3 snipers (max 4) and 9 riflemen, not
    //    1 sniper + 11 riflemen.
    let mut sniper_count = 0usize;
    let mut rifle_count = 0usize;
    let ambush_node = (glam::Vec3::new(-135.0, 70.0, 35.0), 0.0);
    for _ in 0..12 {
        if let Some(h) = walk.sim.spawn_native_character(3, ambush_node) {
            match walk.sim.units[h.index as usize].spec.class {
                0 => rifle_count += 1,
                1 => sniper_count += 1,
                _ => {}
            }
        }
    }
    assert_eq!(
        (rifle_count, sniper_count),
        (9, 3),
        "ambushTeam1 should hold snipers to max - 1 = 3 under the reserve"
    );

    // 3. `transport1` gunship moves on `PlayAnimation("test")` (`local: true`
    //    `anim1`) and has `PilotType=self` gunners (`self_pilots`).
    let pad_idx = walk
        .sim
        .vehicle_pads
        .iter()
        .position(|p| p.def.name.eq_ignore_ascii_case("transport1"))
        .expect("transport1 pad exists");
    let gunship = walk.sim.vehicle_pads[pad_idx]
        .vehicles
        .first()
        .map(|pv| pv.handle)
        .or_else(|| walk.sim.spawn_from_pad(pad_idx, Some(1)))
        .expect("transport1 gunship spawned");
    let v0 = walk.sim.vehicle(gunship).expect("live transport1");
    assert!(
        !v0.self_pilots.is_empty(),
        "uta_fly_ride_gunship must have PilotType=self turrets"
    );
    let pos0 = v0.pos;
    rt.w.anims.pause("sit");
    rt.w.anims.rewind("test");
    rt.w.anims.play("test");
    for _ in 0..(core::TICK_HZ * 5) {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let pos1 = walk.sim.vehicle(gunship).expect("live transport1").pos;
    assert!(
        pos1.distance(pos0) > 20.0,
        "transport1 must fly along anim1: moved only {} m ({pos0} -> {pos1})",
        pos1.distance(pos0)
    );

    // 4. Bringing a world-animated gunship's health to 0 must immediately
    //    destroy it instead of leaving it flying/shooting at 0 HP.
    if let Some(v) = walk.sim.vehicle_mut(gunship) {
        v.health.max = 2500.0;
        v.health.cur = 2500.0;
    }
    let lethal = core::weapon::DamageDesc {
        damage: 50_000.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    walk.sim
        .damage_vehicle(gunship, &lethal, 1.0, Some((walk.player, 0)));
    assert!(
        walk.sim.vehicle(gunship).is_none(),
        "0-HP world-animated transport1 must be destroyed immediately"
    );

    // 5. `SetSpawnDelay(10.0, 0.25)` in `ScriptInit` overwrites the earlier
    //    `SetSpawnDelayTeam(5.0, 3.0, DEF)` on main teams 1 and 2
    //    (SWBFRNT2.ELF `FUN_0018d348` lines 32695-32722).
    assert_eq!(walk.sim.waves[0].delay, 10.0);
    assert_eq!(walk.sim.waves[0].interval, 0.25);
    assert_eq!(walk.sim.waves[1].delay, 10.0);
    assert_eq!(walk.sim.waves[1].interval, 0.25);

    // 6. When `cp1` and `cp4` both open for `DEF` mid-mission (as in
    //    Objective1a.OnComplete + Objective2.OnStart), `DEF` bots enter through
    //    `sim.waves[1]` (`FUN_0036c750`) and split across `cp1` and `cp4` by
    //    `AISpawnWeight` (`FUN_002e7240`), not 12/12 into `cp1` on one tick.
    walk.sim.tick = u64::from(core::TICK_HZ) * 15;
    for cp in &mut walk.sim.cps {
        if cp.def.name.eq_ignore_ascii_case("cp1") || cp.def.name.eq_ignore_ascii_case("cp4") {
            cp.killed = false;
            cp.owner = Some(1);
        }
    }
    rt.w.fill_enabled_ai(&mut walk.sim);
    let immediate_def = walk
        .sim
        .units
        .iter()
        .filter(|u| u.alive && !u.is_player && u.team == 1)
        .count();
    assert_eq!(
        immediate_def, 0,
        "mid-mission DEF bots should queue on sim.waves[1] instead of materializing all 12 on one tick"
    );
    let queued_def = walk
        .sim
        .units
        .iter()
        .filter(|u| !u.alive && !u.is_player && u.team == 1 && u.spawn_slot.is_some())
        .count();
    assert_eq!(queued_def, 12, "all 12 DEF slots should be queued on waves[1]");
    for _ in 0..(core::TICK_HZ * 15) {
        walk.step_tick(&[core::sim::PadState::default()]);
    }
    let cp1_pos = glam::Vec3::new(-185.5, 71.3, 32.2);
    let cp4_pos = glam::Vec3::new(-15.0, 61.651, 102.0);
    let def_alive: Vec<glam::Vec3> = walk
        .sim
        .units
        .iter()
        .filter(|u| u.alive && !u.is_player && u.team == 1)
        .map(|u| u.soldier.pos)
        .collect();
    assert!(
        !def_alive.is_empty(),
        "DEF bots should spawn once their wave opens"
    );
    let at_cp4 = def_alive
        .iter()
        .filter(|p| p.distance(cp4_pos) < p.distance(cp1_pos))
        .count();
    assert!(
        at_cp4 > 0,
        "DEF spawns should split across cp4 and cp1 by AISpawnWeight (got {at_cp4}/{} near cp4)",
        def_alive.len()
    );
}

fn setup_baked_campaign_full(
    missions_dir: &std::path::Path,
    baked_dir: &std::path::Path,
    script: &str,
) -> (crate::mission::MissionDef, WalkSim, CampaignRuntime) {
    let def = crate::mission::MissionDef::load_campaign(missions_dir, script)
        .unwrap_or_else(|e| panic!("load campaign {script}: {e:#}"));
    let (mut scene, _) = crate::assets::load_battle_scene_with_layers(
        &baked_dir.join(&def.map),
        "campaign",
        &def.layers,
    )
    .expect("scene loads");
    scene.manifest.spawn_only_posts = false;
    let mut layout = crate::layout::MapLayout::from_mission_manifest(&scene.manifest, false, true);
    let a = layout.apply_mission(&def);
    let nav = crate::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    walk.sim.battle.campaign_rules = true;
    walk.sim.nav = nav;
    scene.install_collision(&mut walk.sim);
    walk.sim.death_regions = a.death_regions;
    walk.sim.fly_ceiling = a.fly_ceiling;
    walk.sim.flight_limits = a.flight_limits;
    walk.sim.landing_regions = a.landing_regions;
    walk.sim.ground_flyer_map = def.ground_flyer_map;
    def.apply_door_locks(&mut walk.sim.doors, &scene.manifest);
    def.apply_arc_blocks(walk.sim.nav.as_mut());
    def.apply_barriers(walk.sim.nav.as_mut());
    walk.add_posts(&layout);
    let post_classes = crate::walk::hunt_post_classes(
        &baked_dir.join("classes"),
        &baked_dir.join(&def.map),
        "bf2",
    )
    .unwrap();
    walk.set_hunt_class_change(&layout, &post_classes, "bf2");
    let factions = [def.teams[0].faction.clone(), def.teams[1].faction.clone()];
    let space_def = def.space_round();
    let space = space_def.as_ref().map(|s| (s, 180));
    if let Ok(pads) = crate::vehicle_pads::install_for_mission(
        &mut walk.sim,
        &scene.manifest,
        &baked_dir.join(&def.map),
        &baked_dir.join("classes"),
        &factions,
        space,
        &def.class_load_order,
    ) {
        walk.vehicle_classes = pads.classes;
    }
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
        &scene.manifest,
    );
    let spec = def.campaign.clone().unwrap();
    walk.sim.battle.unlimited_teams = spec.infinite_tickets;
    let saved = CampaignRuntime::kill_startup_posts(&mut walk.sim, &def.killed);
    let mut rt = CampaignRuntime::new(spec, &layout, &scene.manifest, &walk.post_keys, saved, &walk.sim);
    rt.set_team_rosters(team_classes(&def), [def.teams[0].units, def.teams[1].units]);
    rt.set_team_factions([def.teams[0].faction.as_str(), def.teams[1].faction.as_str()]);
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
    rt.load_world_anims(&baked_dir.join(&def.map), &scene.manifest, &walk.sim);
    rt.start(&mut walk.sim);
    walk.step_tick(&[core::sim::PadState::default()]);
    walk.sim.events.push(core::sim::Event::Respawn { unit: walk.player });
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &walk.vehicle_classes,
        core::TICK_DT,
    );
    (def, walk, rt)
}

#[test]
fn spa2c_c_linked_shields_fx_and_target_hud() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa2c_c");
    let step = |walk: &mut WalkSim, rt: &mut CampaignRuntime, pad: core::sim::PadState| {
        walk.step_tick(&[pad]);
        let events = walk.sim.events.clone();
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &team_classes(&def),
            &BTreeMap::new(),
            &walk.vehicle_classes,
            core::TICK_DT,
        );
        events
    };
    let station_of = |sim: &core::sim::Sim, name: &str| -> u16 {
        match rt.w.resolve_name(sim, name) {
            Some(Ud::Station(si)) => si,
            other => panic!("{name} resolved to {other:?}"),
        }
    };
    let comms = station_of(&walk.sim, "comms_cis");
    let bridge = station_of(&walk.sim, "cisbridge01");
    let eng1 = station_of(&walk.sim, "ciseng01");
    let hidden = station_of(&walk.sim, "hangarobj");

    // LinkedShields sets shared shield pool & regen rate on CIS capital systems.
    let shield_max_comms = {
        let st_comms = &walk.sim.stations[usize::from(comms)];
        assert!(st_comms.health.shield_max >= 50_000.0, "{st_comms:?}");
        assert_eq!(st_comms.health.shield, st_comms.health.shield_max);
        assert!(st_comms.add_shield > 0.0);
        st_comms.health.shield_max
    };
    assert!(
        walk.sim.station_classes[usize::from(walk.sim.stations[usize::from(hidden)].class)]
            .hide_health_bar
    );

    // Mount the player in a Republic ARC-170 220 m off comms_cis and aim straight at it.
    let arc_vi = walk
        .sim
        .vehicles
        .iter()
        .position(|v| {
            v.alive
                && walk
                    .vehicle_classes
                    .get(v.class as usize)
                    .is_some_and(|c| c == "rep_fly_arc170fighter_sc")
        })
        .expect("ARC-170 present");
    let arc_h = core::vehicle::VehicleHandle {
        index: arc_vi as u32,
        gen: walk.sim.vehicles[arc_vi].gen,
    };
    let tgt_pos = walk.sim.stations[usize::from(comms)].sphere_centre;
    let ship_pos = tgt_pos + Vec3::new(0.0, 40.0, 220.0);
    let aim_rot = glam::Quat::from_rotation_arc(Vec3::Z, (tgt_pos - ship_pos).normalize());
    {
        let v = &mut walk.sim.vehicles[arc_vi];
        v.pos = ship_pos;
        v.rot = aim_rot;
        v.vel = Vec3::ZERO;
        v.team = Some(0);
        v.flyer.phase = core::vehicle::flyer::FlightPhase::Flying;
        v.seats[0].occupant = Some(walk.player);
    }
    {
        let u = &mut walk.sim.units[walk.player.index as usize];
        u.alive = true;
        u.team = 0;
        u.health.cur = u.health.max;
        u.mount = Some(core::vehicle::Mount {
            vehicle: arc_h,
            seat: 0,
        });
    }

    let feed = crate::event_feed::EventFeed::new(
        crate::event_feed::Texts::builtin(),
        ["rep", "cis"],
        &walk.post_keys,
    );
    let idle = core::sim::PadState::default();
    step(&mut walk, &mut rt, idle);
    let pick = walk.sim.reticle_pick[0];
    assert_eq!(
        pick,
        Some((core::sim::AimTarget::Station(comms), -1)),
        "reticle picks comms_cis"
    );
    let mut hud = feed
        .target(&walk.sim, 0)
        .expect("HUD target shown for comms_cis");
    assert_eq!(hud.relation, -1);
    assert!(
        hud.shield.as_ref().is_some_and(|(label, f)| label == "Shield" && *f > 0.99),
        "expected full Shield bar, got {hud:?}"
    );
    let mut shield_impact = None;
    for _ in 0..30 {
        {
            let v = &mut walk.sim.vehicles[arc_vi];
            v.pos = ship_pos;
            v.rot = aim_rot;
            v.vel = Vec3::ZERO;
        }
        let events = step(
            &mut walk,
            &mut rt,
            core::sim::PadState {
                buttons: core::sim::buttons::FIRE,
                ..Default::default()
            },
        );
        for ev in events {
            if let core::sim::Event::Impact {
                surface,
                weapon_class,
                ..
            } = ev
            {
                if matches!(
                    surface,
                    core::impact::Surface::Shield | core::impact::Surface::ShieldLarge
                ) {
                    shield_impact = Some((surface, weapon_class));
                }
            }
        }
        if shield_impact.is_some() {
            break;
        }
    }
    let (surface, wc) = shield_impact.expect("laser bolt produced a shield impact event");
    let fx_name = walk.sim.weapon_classes[wc as usize]
        .ordnance
        .impact
        .as_ref()
        .and_then(|l| l.effect(surface));
    assert!(
        fx_name.is_some_and(|n| n.contains("shieldimpact")),
        "expected shield impact FX, got {fx_name:?}"
    );

    // Apply a heavy strike exceeding 1 tick of 2000/s regen, then step so
    // OnObjectDamage + GetObjectShield sync LinkedShields siblings.
    let torpedo = core::weapon::DamageDesc {
        damage: 5_000.0,
        shield_scale: 1.0,
        scales: [1.0; 7],
    };
    walk.sim
        .damage_station(comms, &torpedo, 1.0, Some(walk.player), Some(0));
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &walk.vehicle_classes,
        core::TICK_DT,
    );
    let cur_comms = walk.sim.stations[usize::from(comms)].health.shield;
    let cur_bridge = walk.sim.stations[usize::from(bridge)].health.shield;
    let cur_eng1 = walk.sim.stations[usize::from(eng1)].health.shield;
    assert!(
        cur_comms < shield_max_comms,
        "comms shield drained: cur={cur_comms} max={shield_max_comms}"
    );
    assert!(
        (cur_comms - cur_bridge).abs() < 1.0 && (cur_comms - cur_eng1).abs() < 1.0,
        "LinkedShields synced shields: comms={cur_comms} bridge={cur_bridge} eng1={cur_eng1}"
    );

    // Shield regenerates while controller survives.
    for _ in 0..30 {
        let v = &mut walk.sim.vehicles[arc_vi];
        v.pos = ship_pos;
        v.rot = aim_rot;
        v.vel = Vec3::ZERO;
        step(&mut walk, &mut rt, idle);
    }
    assert!(
        walk.sim.stations[usize::from(comms)].health.shield > cur_comms,
        "shield should regenerate via add_shield"
    );

    // Drain remaining shield via live bolt fire inside Sim::step: shield regen
    // before combat must not mask cur == 0.0 from LinkedShields or HUD switch.
    walk.sim.stations[usize::from(comms)].health.shield = 5.0;
    walk.sim.stations[usize::from(comms)].add_shield = 100.0;
    for _ in 0..30 {
        {
            let v = &mut walk.sim.vehicles[arc_vi];
            v.pos = ship_pos;
            v.rot = aim_rot;
            v.vel = Vec3::ZERO;
        }
        step(
            &mut walk,
            &mut rt,
            core::sim::PadState {
                buttons: core::sim::buttons::FIRE,
                ..Default::default()
            },
        );
        if walk.sim.stations[usize::from(comms)].add_shield == 0.0 {
            break;
        }
    }
    assert_eq!(
        walk.sim.stations[usize::from(comms)].health.shield, 0.0,
        "shield must stay 0.0 once drained on a Sim::step tick"
    );
    assert_eq!(
        walk.sim.stations[usize::from(comms)].add_shield, 0.0,
        "LinkedShields must see cur == 0.0 and zero AddShield"
    );
    step(&mut walk, &mut rt, idle);
    assert_eq!(walk.sim.reticle_pick[0], Some((core::sim::AimTarget::Station(comms), -1)));
    assert!(walk.sim.stations[usize::from(comms)].health.shield <= 0.0);
    hud = feed.target(&walk.sim, 0).expect("HUD remains for unshielded station");
    assert_eq!(hud.shield, None);
    assert!(hud.health_fraction > 0.0 && hud.health_fraction < 1.0);
}

#[test]
fn spa2c_c_landing_in_enemy_hangar_completes_objective6() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa2c_c");
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
        6.0,
    );
    for _ in 0..30 {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if rt.has_handlers(EventKind::CharacterLandedFlyer) {
            break;
        }
    }
    assert!(
        rt.has_handlers(EventKind::CharacterLandedFlyer),
        "Objective6 must register OnCharacterLandedFlyer"
    );

    let cp1 = rt
        .w
        .region_id("CP1Control")
        .expect("CP1Control region present");
    let v_pos = rt.w.regions[cp1 as usize].1.center;
    let arc_vi = walk
        .sim
        .vehicles
        .iter()
        .position(|v| {
            v.alive
                && walk
                    .vehicle_classes
                    .get(v.class as usize)
                    .is_some_and(|c| c == "rep_fly_arc170fighter_sc")
        })
        .expect("ARC-170 present");
    let arc_h = core::vehicle::VehicleHandle {
        index: arc_vi as u32,
        gen: walk.sim.vehicles[arc_vi].gen,
    };
    {
        let v = &mut walk.sim.vehicles[arc_vi];
        v.pos = v_pos;
        v.vel = Vec3::ZERO;
        v.team = Some(0);
        v.flyer.phase = core::vehicle::flyer::FlightPhase::Landing;
        v.seats[0].occupant = Some(walk.player);
    }
    {
        let u = &mut walk.sim.units[walk.player.index as usize];
        u.alive = true;
        u.team = 0;
        u.mount = Some(core::vehicle::Mount {
            vehicle: arc_h,
            seat: 0,
        });
    }
    // Seed CharSeen while mounted in non-Landed phase, then transition to Landed.
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &walk.vehicle_classes,
        core::TICK_DT,
    );
    walk.sim.vehicles[arc_vi].flyer.phase = core::vehicle::flyer::FlightPhase::Landed;
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &walk.vehicle_classes,
        core::TICK_DT,
    );
    let obj6 = rt.interp.global("Objective6");
    let mut host = Ctx {
        w: &mut rt.w,
        sim: &mut walk.sim,
    };
    let done = rt
        .interp
        .index(&mut host, &obj6, &Value::str("isComplete"))
        .unwrap();
    assert_eq!(
        done,
        Value::Bool(true),
        "landing inside CP1Control must fire OnCharacterLandedFlyer and complete Objective6"
    );
}

#[test]
fn campaign_space_missions_preserve_hangars_takeoffs_and_tickets() {
    let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let missions_dir = crate_dir.join("../../missions");
    let baked_dir = crate_dir.join("../../assets_baked");
    if !baked_dir.join("bf2_spa2").join("manifest.ron").exists() {
        eprintln!("SKIP: baked space assets required");
        return;
    }
    for (script, max_impact_deaths, want_unlimited_teams, want_kill_floor_below) in [
        ("spa2c_c", 0usize, [true, true], None),
        ("spa3c_c", 1usize, [false, true], None),
        ("spa4g_c", 3usize, [false, true], Some(-330.0)),
        ("spa1g_c", 2usize, [false, true], None),
    ] {
        let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, script);
        assert!(
            !walk.sim.battle.unlimited_reinforcements,
            "{script}: install_space_assault must not overwrite campaign ticket rules"
        );
        assert_eq!(walk.sim.battle.unlimited_teams, want_unlimited_teams, "{script}");
        if let Some(ceiling) = want_kill_floor_below {
            let kf = walk.sim.kill_floor.expect("kill_floor computed");
            assert!(
                kf < ceiling,
                "{script} kill_floor {kf} must stay below CIS hangar {ceiling}"
            );
        }
        let mut impacts = Vec::new();
        for t in 0..900 {
            walk.step_tick(&[core::sim::PadState::default()]);
            for ev in &walk.sim.events {
                if let core::sim::Event::FlyerImpact {
                    vehicle,
                    pos,
                    normal,
                    speed,
                    lethal: true,
                    ..
                } = ev
                {
                    let cls = walk
                        .sim
                        .vehicles
                        .get(vehicle.index as usize)
                        .and_then(|v| walk.vehicle_classes.get(v.class as usize))
                        .map_or("?", String::as_str)
                        .to_string();
                    impacts.push((t, cls, *pos, *normal, *speed));
                }
            }
            rt.step(
                &mut walk.sim,
                walk.player,
                None,
                &team_classes(&def),
                &BTreeMap::new(),
                &walk.vehicle_classes,
                core::TICK_DT,
            );
        }
        let mounted_t1 = walk
            .sim
            .units
            .iter()
            .filter(|u| u.alive && u.team == 1 && u.mount.is_some())
            .count();
        assert!(
            mounted_t1 > 0,
            "{script}: defending AI pilots must take off cleanly (mounted_t1={mounted_t1})"
        );
        assert!(
            impacts.len() <= max_impact_deaths,
            "{script}: impact_deaths={} exceeded budget {max_impact_deaths}: {impacts:?}",
            impacts.len()
        );
    }
}

#[test]
fn spa2c_c_minimap_omits_space_armed_buildings_and_hidden_mini_ships() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (def, walk, _rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa2c_c");
    let mut scene = crate::assets::load_scene(&baked_dir.join(&def.map)).unwrap();
    let common_dir = baked_dir.join("common_bf2");
    if common_dir.join("common.ron").exists() {
        scene.merge_common(crate::assets::load_common(&common_dir).unwrap());
    }
    let map = crate::minimap::MapStatic::load(
        &scene,
        &baked_dir.join("classes"),
        &walk,
        &walk.vehicle_classes,
        def.era.as_deref(),
    );
    let model = map.model(&walk);
    for icon in model.units.iter().chain(model.posts.iter()) {
        assert!(
            !matches!(
                icon.texture.as_str(),
                "icon_cis_minifedcruiser" | "icon_rep_minirepassaultship" | "shipturret_icon"
            ),
            "unexpected minimap icon {:?} at {:?}",
            icon.texture,
            icon.pos
        );
    }
}

#[test]
fn spa2c_c_cis_hangar_fighters_exit_cleanly() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa2c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let mut total_exited = std::collections::BTreeSet::new();
    let mut hangar_ticks: BTreeMap<(usize, u32), usize> = BTreeMap::new();
    let mut impact_deaths_in_hangar = 0usize;
    for _ in 0..2400 {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        for ev in &walk.sim.events {
            if let core::sim::Event::FlyerImpact { pos, lethal: true, .. } = ev {
                if pos.x > -470.0 && pos.x < -300.0 && pos.z > -1060.0 && pos.z < -940.0 {
                    impact_deaths_in_hangar += 1;
                }
            }
        }
        for (vi, v) in walk.sim.vehicles.iter().enumerate() {
            if v.alive
                && v.kind == core::vehicle::VehicleKind::Flyer
                && v.home_team == Some(1)
                && v.flyer.phase == core::vehicle::flyer::FlightPhase::Flying
            {
                if v.pos.x < -500.0 {
                    total_exited.insert((vi, v.gen));
                } else if v.pos.x > -470.0
                    && v.pos.x < -300.0
                    && v.pos.y < 700.0
                    && v.pos.z > -1060.0
                    && v.pos.z < -940.0
                {
                    *hangar_ticks.entry((vi, v.gen)).or_insert(0) += 1;
                }
            }
        }
    }
    let stuck_in_hangar: Vec<_> = hangar_ticks
        .into_iter()
        .filter(|&(_, ticks)| ticks > 300)
        .collect();
    assert!(
        total_exited.len() >= 6,
        "expected >=6 CIS hangar sorties to exit past x=-500, got {}",
        total_exited.len()
    );
    assert!(
        stuck_in_hangar.is_empty(),
        "no CIS flyers should stay trapped >5s inside CP1 hangar (got {}: {stuck_in_hangar:?})",
        stuck_in_hangar.len()
    );
    assert_eq!(
        impact_deaths_in_hangar, 0,
        "CIS flyers must not explode bouncing inside CP1 hangar"
    );
}

#[test]
fn spa2c_c_draining_linked_shields_completes_objective4_and_protects_turrets() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa2c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let teams = team_classes(&def);
    let call_obj = |rt: &mut CampaignRuntime, walk: &mut WalkSim, obj: &str, method: &str, arg: f32| {
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
            .call_value(&mut host, &f, vec![this, Value::Num(arg)])
            .unwrap();
    };
    call_obj(&mut rt, &mut walk, "objectiveSequence", "ActivateObjectiveSet", 4.0);
    for _ in 0..30 {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }

    let station_of = |name: &str| match rt.w.resolve_name(&walk.sim, name) {
        Some(Ud::Station(si)) => si,
        other => panic!("{name} resolved to {other:?}"),
    };
    let vehicle_of = |name: &str| match rt.w.resolve_name(&walk.sim, name) {
        Some(Ud::Vehicle(i, g)) => core::vehicle::VehicleHandle { index: i, gen: g },
        other => panic!("{name} resolved to {other:?}"),
    };
    let shield_cis = station_of("shield_cis");
    let cruiser1 = station_of("cis_fly_fedcruiser1");
    let comms = station_of("comms_cis");
    let turret1 = vehicle_of("cis_prop_shipturret1");
    let turret3 = vehicle_of("cis_prop_shipturret3");

    for vh in [turret1, turret3] {
        let v = walk.sim.vehicle(vh).expect("ship turret alive");
        assert_eq!(v.health.shield_max, 70_000.0);
        assert_eq!(v.health.shield, 70_000.0);
        assert!(v.add_shield > 0.0, "LinkedShields sets AddShield on turrets");
    }

    // Shooting a shielded building turret shows a Shield bar, emits shield impacts,
    // and syncs LinkedShields across CIS capital siblings without damaging turret hull.
    let feed = crate::event_feed::EventFeed::new(
        crate::event_feed::Texts::builtin(),
        ["rep", "cis"],
        &walk.post_keys,
    );
    let arc_vi = walk
        .sim
        .vehicles
        .iter()
        .position(|v| {
            v.alive
                && walk
                    .vehicle_classes
                    .get(v.class as usize)
                    .is_some_and(|c| c == "rep_fly_arc170fighter_sc")
        })
        .expect("ARC-170 present");
    let arc_h = core::vehicle::VehicleHandle {
        index: arc_vi as u32,
        gen: walk.sim.vehicles[arc_vi].gen,
    };
    if walk.sim.units[walk.player.index as usize].mount.is_some() {
        walk.sim.unseat_unit(walk.player);
    }
    let turret_pos = {
        let tv = walk.sim.vehicle(turret1).unwrap();
        tv.aim_sphere.map_or(tv.pos, |(c, _)| tv.pos + tv.rot * c)
    };
    let ship_pos = turret_pos - Vec3::new(140.0, 0.0, 0.0);
    let aim_rot = glam::Quat::from_rotation_arc(Vec3::Z, (turret_pos - ship_pos).normalize());
    {
        let v = &mut walk.sim.vehicles[arc_vi];
        v.pos = ship_pos;
        v.rot = aim_rot;
        v.vel = Vec3::ZERO;
        v.team = Some(0);
        v.flyer.phase = core::vehicle::flyer::FlightPhase::Flying;
        v.seats[0].occupant = Some(walk.player);
    }
    {
        let u = &mut walk.sim.units[walk.player.index as usize];
        u.alive = true;
        u.team = 0;
        u.health.cur = u.health.max;
        u.mount = Some(core::vehicle::Mount {
            vehicle: arc_h,
            seat: 0,
        });
    }
    let mut turret_shield_hit = false;
    for _ in 0..20 {
        {
            let v = &mut walk.sim.vehicles[arc_vi];
            v.pos = ship_pos;
            v.rot = aim_rot;
            v.vel = Vec3::ZERO;
        }
        walk.step_tick(&[core::sim::PadState {
            buttons: core::sim::buttons::FIRE,
            ..Default::default()
        }]);
        if matches!(
            walk.sim.reticle_pick[0],
            Some((core::sim::AimTarget::Vehicle(h), _)) if h == turret1
        ) {
            let hud = feed.target(&walk.sim, 0).expect("HUD target on ship turret");
            assert!(
                hud.shield.as_ref().is_some_and(|(label, _)| label == "Shield"),
                "shielded ship turret should show Shield HUD bar, got {hud:?}"
            );
        }
        for ev in &walk.sim.events {
            if let core::sim::Event::Impact {
                vehicle: Some(vh),
                surface,
                ..
            } = *ev
            {
                if vh == turret1
                    && matches!(
                        surface,
                        core::impact::Surface::Shield | core::impact::Surface::ShieldLarge
                    )
                {
                    turret_shield_hit = true;
                }
            }
        }
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &teams,
            &BTreeMap::new(),
            &walk.vehicle_classes,
            core::TICK_DT,
        );
        rt.close_popup();
        if turret_shield_hit {
            break;
        }
    }
    assert!(
        turret_shield_hit,
        "bolt impact on shielded cis_prop_shipturret1 must produce Shield surface FX"
    );
    let tv = walk.sim.vehicle(turret1).unwrap();
    assert_eq!(tv.health.cur, tv.health.max, "hull untouched while shield holds");

    let strike = core::weapon::DamageDesc {
        damage: 5_000.0,
        shield_scale: 1.0,
        scales: [1.0; 7],
    };
    walk.sim
        .damage_vehicle(turret1, &strike, 1.0, Some((walk.player, 0)));
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &teams,
        &BTreeMap::new(),
        &walk.vehicle_classes,
        core::TICK_DT,
    );
    assert!(
        walk.sim.stations[usize::from(cruiser1)].health.shield < 70_000.0,
        "damaging ship turret shield must sync LinkedShields across capital ship"
    );

    // Draining CIS shield to 0 must KillObject("shield_cis"), emit StationDestroyed,
    // complete Objective4 in the same dispatch, and advance to Objective5 (comms_cis).
    // Sim::step clears events at the start of each tick; clear manual events before next strike.
    walk.sim.events.clear();

    // Draining CIS shield to 0 must KillObject("shield_cis"), emit StationDestroyed,
    // complete Objective4 in the same dispatch, and advance to Objective5 (comms_cis).
    walk.sim.stations[usize::from(cruiser1)].health.shield = 5.0;
    walk.sim.stations[usize::from(cruiser1)].add_shield = 0.0;
    walk.sim
        .damage_station(cruiser1, &strike, 1.0, Some(walk.player), Some(0));
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &teams,
        &BTreeMap::new(),
        &walk.vehicle_classes,
        core::TICK_DT,
    );
    rt.close_popup();
    assert!(
        walk.sim.stations[usize::from(shield_cis)].destroyed,
        "LinkedShields must destroy controllerObject shield_cis when shield reaches 0"
    );
    assert_eq!(
        walk.sim.stations[usize::from(cruiser1)].health.shield, 0.0,
        "cruiser shield must remain at 0"
    );
    let obj4 = rt.interp.global("Objective4");
    let obj4_done = {
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        rt.interp
            .index(&mut host, &obj4, &Value::str("isComplete"))
            .unwrap()
    };
    assert_eq!(
        obj4_done,
        Value::Bool(true),
        "Objective4 ('Disable the Enemy Shields') must complete once shield_cis is killed"
    );
    for _ in 0..300 {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if walk.sim.stations[usize::from(comms)].health.max == 20_000.0 {
            break;
        }
    }
    assert_eq!(
        walk.sim.stations[usize::from(comms)].health.max, 20_000.0,
        "Objective5.OnStart must activate after Objective4 completes"
    );
    assert_eq!(
        walk.sim.stations[usize::from(comms)].health.cur, 20_000.0,
        "Objective5.OnStart must reset comms_cis health to 20,000"
    );
}

#[test]
fn spa2c_c_objective6_hangar_voice_plays_immediately_over_background_streams() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa2c_c");
    let Some(mut lib) = crate::sounds::SoundLib::load_for(&baked_dir.join(&def.map), &def) else {
        eprintln!("SKIP: bf2_spa2 sound bank required");
        return;
    };
    assert!(
        lib.voice_take("SPA2_obj_62").is_some_and(|(_, _, secs)| secs > 1.0),
        "SPA2_obj_62 must decode with non-zero length"
    );
    let mut v = crate::announce::Commander::for_battle(
        &def,
        &crate::layout::MapLayout::default(),
        &walk,
    );
    v.route_streams(rt.stream_slots());
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let _ = rt.take_voice_overs();
    let _ = rt.take_sounds();
    let call_obj =|rt: &mut CampaignRuntime, walk: &mut WalkSim, obj: &str, method: &str, arg: f32| {
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
            .call_value(&mut host, &f, vec![this, Value::Num(arg)])
            .unwrap();
    };
    // Prime Commander with SoundLib gating and start a 10 s common_objComplete on its own stream.
    let _ = v.gate_script_sounds(&lib);
    v.ask_script_sounds(vec!["common_objComplete".into()]);
    let _ = v.gate_script_sounds(&lib);
    let mut played = Vec::new();
    let _ = v.frame(&walk.sim, walk.player, core::TICK_DT, |line| {
        played.push(line.name.clone());
        if line.name == "common_objComplete" {
            Some(10.0)
        } else {
            lib.voice_take(&line.name).map(|(_, _, s)| s)
        }
    });
    assert!(
        played.iter().any(|n| n == "common_objComplete"),
        "expected common_objComplete playing on its stream: {played:?}"
    );
    assert!(v.voice_playing());

    // Now start Objective6 ("Land in the Enemy Hangar") while common_objComplete is still playing.
    played.clear();
    call_obj(&mut rt, &mut walk, "objectiveSequence", "ActivateObjectiveSet", 6.0);
    for _ in 0..30 {
        tick(&mut walk, &mut rt, &def);
        let mut script_lines = rt.take_voice_overs();
        script_lines.extend(rt.take_sounds());
        v.ask_script_sounds(script_lines);
        assert!(v.gate_script_sounds(&lib).is_empty());
        let _ = v.frame(&walk.sim, walk.player, core::TICK_DT, |line| {
            played.push(line.name.clone());
            lib.voice_take(&line.name).map(|(_, _, s)| s)
        });
        if played.iter().any(|n| n == "SPA2_obj_62") {
            break;
        }
    }
    assert!(
        played.iter().any(|n| n == "SPA2_obj_62"),
        "SPA2_obj_62 must start immediately on its own stream without waiting on background voices: {played:?}"
    );
}

#[test]
fn fel1c_c_acklay_objective_counts_ally_kills_and_spawns_both_waves() {
    let Some((missions_dir, baked_dir)) = baked() else {
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "fel1c_c");
    // The real battle gives the runtime each local team's roster (battle.rs's
    // `set_native_rosters`), which the Ambush spawn reads.
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
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let _ = rt.complete_objective(&mut walk.sim, "Objective1");
    for _ in 0..(5 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    let slots: Vec<_> = walk
        .sim
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.team == 4)
        .map(|(i, u)| core::unit::UnitHandle {
            index: i as u32,
            gen: u.gen,
        })
        .collect();
    assert!(
        slots.len() >= 3,
        "first Ambush must spawn 3 acklays: {} slots",
        slots.len()
    );

    // The original script only counts kills credited to a human; the port's
    // allies reliably get the acklay kills, which is the bug documented in
    // analysis/re-bf2-campaign-kill-credit.md. Use an AI ally as the shooter
    // here so this test covers the workaround.
    let step_with_kills =
        |walk: &mut WalkSim, rt: &mut CampaignRuntime, victims: &[core::unit::UnitHandle]| {
            walk.step_tick(&[core::sim::PadState::default()]);
            // The headless battle has no living allies left, so seat a fresh
            // REP bot as the killer: the script's bug is that an ally's kill
            // does not count as a human kill.
            let ally = walk
                .sim
                .spawn_ai(0, Vec3::ZERO, 0.0)
                .expect("a REP bot to stand in for the allied AI killer");
            for &v in victims {
                if let Some(u) = walk.sim.units.get_mut(v.index as usize) {
                    u.health.cur = 0.0;
                    u.alive = false;
                }
                walk.sim.events.push(core::sim::Event::Kill {
                    shooter: ally,
                    victim: v,
                });
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
        };
    step_with_kills(&mut walk, &mut rt, &slots[..3]);
    assert_eq!(
        rt.interp.global("acklay_count").num(),
        Some(3.0),
        "three ally-credited acklay kills must decrement 6 to 3"
    );
    let second: Vec<_> = walk
        .sim
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.team == 4 && u.alive)
        .map(|(i, u)| core::unit::UnitHandle {
            index: i as u32,
            gen: u.gen,
        })
        .collect();
    assert_eq!(
        second.len(),
        3,
        "the second Ambush must spawn 3 live acklays: {} alive",
        second.len()
    );

    step_with_kills(&mut walk, &mut rt, &second);
    assert_eq!(rt.interp.global("acklay_count").num(), Some(0.0));
    for _ in 0..(2 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text == "level.fel1.objectives.campaign.2" && o.state == 3),
        "Objective2 must complete after all 6 acklays are killed by allies"
    );
}

/// spa3's LAAT is authored neutral and disabled until the player reaches the
/// hangar. The pad spawns it 15 s into the round, long after ScriptPostLoad
/// sets those properties, so a pad vehicle the script names must be created
/// when the property is written (otherwise an AI pilots it out of the hangar).
#[test]
fn spa3c_c_laat_is_disabled_before_the_pad_would_spawn_it() {
    let Some((missions_dir, baked_dir)) = baked() else {
        return;
    };
    let (_, walk, _) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa3c_c");
    let pad = walk
        .sim
        .vehicle_pads
        .iter()
        .find(|p| p.def.name.eq_ignore_ascii_case("rep04"))
        .expect("REP04 pad");
    let v = pad
        .vehicles
        .first()
        .and_then(|pv| walk.sim.vehicle(pv.handle))
        .expect("the script's SetProperty must have spawned REP04");
    assert!(v.disable > 0.0, "the LAAT must be disabled until the player arrives");
    assert!(
        walk.sim.units.iter().all(|u| u.mount.map(|m| m.vehicle) != Some(pad.vehicles[0].handle)),
        "no AI may pilot the LAAT out of the hangar"
    );
}

/// The life-support console's geometry is baked ~49 m from its instance
/// origin; the objective arrow must sit on the console the player can shoot,
/// not on the empty origin (where a different structure stands).
#[test]
fn spa3c_c_life_support_marker_sits_on_the_console() {
    let Some((missions_dir, baked_dir)) = baked() else {
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa3c_c");
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    {
        let this = rt.interp.global("objectiveSequence");
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        let f = rt
            .interp
            .index(&mut host, &this, &Value::str("ActivateObjectiveSet"))
            .unwrap();
        rt.interp
            .call_value(&mut host, &f, vec![this, Value::Num(4.0)])
            .unwrap();
    }
    for _ in 0..(2 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    let si = rt
        .w
        .station_names
        .iter()
        .position(|n| n.eq_ignore_ascii_case("life_int_cis"))
        .unwrap();
    let origin = walk.sim.stations[si].pos;
    let centre = walk.sim.collision.instance_centre(walk.sim.stations[si].instance.unwrap()).unwrap();
    assert!(
        origin.distance(centre) > 10.0,
        "this console's geometry is offset from its origin"
    );
    let markers = rt.objective_markers(&walk.sim);
    assert!(
        markers.iter().any(|m| m.pos.distance(centre) < 5.0),
        "no objective marker on the console at {centre}: {markers:?}"
    );
}

#[test]
fn spa3c_c_placed_holocron_is_a_carryable_flag() {
    let Some((missions_dir, baked_dir)) = baked() else { return; };
    let (def, walk, rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa3c_c");
    assert!(
        rt.w.flags.iter().any(|f| f.name.eq_ignore_ascii_case("plans")),
        "the placed plans holocron must be registered as a flag"
    );
    assert!(
        walk.sim.ctf.as_ref().is_some_and(|c| !c.flags.is_empty()),
        "the sim must hold a live flag for the holocron"
    );
    assert!(
        matches!(rt.w.resolve_name(&walk.sim, "plans"), Some(Ud::Flag(_))),
        "the script must resolve plans to a carryable flag"
    );
    // A flag is a carryable item, not solid geometry: its baked coll would
    // stand an invisible 0.6 m box at the floating holocron (the playtest's
    // invisible wall in the plans room).
    let (scene, _) = crate::assets::load_battle_scene_with_layers(
        &baked_dir.join(&def.map),
        "campaign",
        &def.layers,
    )
    .expect("scene loads");
    let plans_instance = scene
        .manifest
        .instances
        .iter()
        .position(|i| i.name.eq_ignore_ascii_case("plans"))
        .expect("the plans instance") as u32;
    assert!(
        walk.sim
            .collision
            .bodies()
            .iter()
            .all(|b| b.instance != Some(plans_instance)),
        "a carryable flag must not leave a solid collision body"
    );
    // The tutorial has no placed holocron: its flag is created later by
    // CreateEntity, so setup must not add a placed one there.
    let (_, _, rt2) = setup_baked_campaign_full(&missions_dir, &baked_dir, "geo1c_c");
    assert!(
        rt2.w.flags.iter().all(|f| !f.name.eq_ignore_ascii_case("holocron")),
        "the tutorial holocron is not a placed instance"
    );
}


/// The maintainer's Utapau black box (2026-11-17, `dist/.../last.json`): the
/// player stands on cam_cp4's platform at (-190.95, 18.4, 204.84), sees no
/// capture meter and cannot take the first objective post. The authored box
/// starts 0.19 m above that floor; the layout's campaign-only correction must
/// bring it down so the post can be taken from its own platform.
#[test]
fn uta1c_first_post_counts_and_captures_from_its_platform() {
    let (missions, baked) = baked().expect("bake");
    let (_, mut walk, _) = setup_baked_campaign(&missions, &baked, "uta1c_c");
    let ci = walk
        .sim
        .cps
        .iter()
        .position(|c| c.def.name.eq_ignore_ascii_case("cam_cp4"))
        .expect("cam_cp4");
    let player = walk.player.index as usize;
    {
        let u = &mut walk.sim.units[player];
        u.alive = true;
        u.capture_posts = true;
        u.mount = None;
        u.soldier.pos = Vec3::new(-190.95, 18.4, 204.84);
    }
    assert!(
        walk.sim.counts_in_cp(player, ci),
        "the platform spot must count toward cam_cp4"
    );
    assert_eq!(
        crate::capture_feed::post_of(&walk.sim, player).map(|x| x.0),
        Some(ci),
        "the HUD capture meter must show cam_cp4"
    );
    // Drive the post's own state machine, as the sim's step 6 does.
    let mut owned_by_player = false;
    let team = walk.sim.units[player].team;
    for _ in 0..(30 * core::TICK_HZ * 3) {
        let inside = walk.sim.cp_inside(ci);
        if matches!(
            walk.sim.cps[ci].update(core::TICK_DT, &inside),
            core::cp::CpChange::Captured { by } if by == team
        ) {
            owned_by_player = true;
            break;
        }
    }
    assert!(owned_by_player, "cam_cp4 must be capturable from the platform");
}

/// Utapau's Objective 3a.OnComplete turns native team 4's member into the CIS
/// hero Grievous with `BatchChangeTeams` + `SelectCharacterClass`, then calls
/// `SpawnCharacter` at the `grievious_spawn` path. The window's roster has the
/// hero appended by `heroes::append`; the fixture builds rosters from the
/// mission file alone, so mirror that here.
#[test]
fn uta1c_scripted_hero_bindings_spawn_grievous_for_the_final_objective() {
    use crate::campaign_bindings::PLACEHOLDER;
    use crate::campaign_script::Host;

    let (missions, baked) = baked().expect("bake");
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions, &baked, "uta1c_c");
    let hero_class = walk.sim.classes[1][0].clone();
    walk.sim.classes[1].push(hero_class);
    let mut tc = team_classes(&def);
    tc[1].push("cis_hero_grievous".into());
    rt.set_team_rosters(tc, [def.teams[0].units, def.teams[1].units]);

    // Native team 4 is sim team 3; its member 0 is the placeholder char id.
    let member = PLACEHOLDER + 3 * 1000;
    let point = {
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        host.call(
            &mut rt.interp,
            "GetPathPoint",
            &[Value::str("grievious_spawn"), Value::Num(0.0)],
        )
        .expect("grievious_spawn path point")
        .into_iter()
        .next()
        .expect("one point")
    };
    assert_ne!(point, Value::Nil);
    {
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        host.call(
            &mut rt.interp,
            "BatchChangeTeams",
            &[
                Value::Num(4.0),
                Value::Num(2.0),
                Value::Num(1.0),
            ],
        )
        .expect("BatchChangeTeams");
        host.call(
            &mut rt.interp,
            "SelectCharacterClass",
            &[Value::Num(member as f32), Value::str("cis_hero_grievous")],
        )
        .expect("SelectCharacterClass");
        host.call(
            &mut rt.interp,
            "SpawnCharacter",
            &[Value::Num(member as f32), point],
        )
        .expect("SpawnCharacter");
    }
    let grievous = walk
        .sim
        .units
        .iter()
        .enumerate()
        .find(|(_, u)| {
            u.alive
                && u.team == 1
                && rt.w
                    .team_classes
                    .get(usize::from(u.team))
                    .and_then(|cs| cs.get(usize::from(u.spec.class)))
                    .is_some_and(|n| n.eq_ignore_ascii_case("cis_hero_grievous"))
        })
        .map(|(i, _)| i);
    assert!(grievous.is_some(), "Grievous must be fielded for Objective 3b");
    assert_eq!(rt.errors().count(), 0, "no script errors");
}

#[test]
fn cor1c_c_fixes_hero_spawns_door_camera_holocron_flagrunner_and_library_defense() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: cor1c_c mission and baked assets required");
        return;
    };
    if !baked_dir
        .join("bf2_cor1")
        .join(manifest::world_anim::FILE)
        .exists()
    {
        eprintln!("SKIP: bf2_cor1 world animations not baked");
        return;
    }

    // Bug 1: PS2-regenerated mission mission heroes field + EnableSPScriptedHeroes disables AI Anakins.
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "cor1c_c");
    assert_eq!(
        def.heroes,
        vec![(1, "rep_hero_cloakedanakin".to_owned())],
        "cor1c_c must declare Anakin as team 1 hero"
    );
    // Re-invoke EnableSPScriptedHeroes after appending the real hero class (matches battle.rs order).
    let (mut rosters, _) = crate::roster::FactionRoster::pair(
        &baked_dir.join("classes"),
        "bf2",
        ["rep", "all"],
        Some(&def),
    )
    .expect("cor1c_c faction rosters");
    let appended_idx = crate::heroes::append(
        &mut rosters[0],
        &baked_dir.join("classes"),
        Some(&def),
        0,
    )
    .expect("cor1c_c must append rep_hero_cloakedanakin including gar combo load index");
    assert_eq!(
        rosters[0].classes[appended_idx as usize].class,
        "rep_hero_cloakedanakin"
    );
    let hero_class_idx = walk.sim.classes[0].len() as u8;
    let hero_uc = walk.sim.classes[0][0].clone();
    walk.sim.classes[0].push(hero_uc);
    walk.sim.class_enabled[0].push(true);
    walk.sim.heroes = Some(core::sim::heroes::Heroes::new([Some(hero_class_idx), None]));
    rt.w.team_classes[0].push("rep_hero_cloakedanakin".into());
    {
        use crate::campaign_script::Host;
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        host.call(&mut rt.interp, "EnableSPScriptedHeroes", &[])
            .unwrap();
    }
    let heroes = walk.sim.heroes.as_ref().unwrap();
    assert!(!heroes.rules.ai_candidates);
    assert_eq!(heroes.rules.unlock, core::sim::heroes::HeroUnlock::Points(i32::MAX));
    assert_eq!(heroes.rules.respawn_ticks, None);
    assert!(
        !heroes.teams[0].unlocked,
        "Anakin must not be unlocked at mission start"
    );
    // Even if team 0 has high AI scores, Points(i32::MAX) keeps it locked and AI never picks Anakin.
    {
        let heroes = walk.sim.heroes.as_mut().unwrap();
        for p in &mut heroes.points {
            *p = 500;
        }
    }
    for _ in 0..(4 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if rt
            .objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.Campaign.1") && o.state == 2)
        {
            break;
        }
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.Campaign.1") && o.state == 2),
        "Objective1 should be active after startup timer, got {:?}",
        rt.objectives()
    );
    assert!(
        walk.sim.hero_offer(0).is_none(),
        "Player must not receive Anakin offer before Objective 4 completes"
    );
    assert!(
        walk.sim
            .units
            .iter()
            .all(|u| !u.alive || u.is_human() || u.team != 0 || u.spec.class != hero_class_idx),
        "AI teammates must never spawn as Anakin"
    );

    // Bug 2: Opening DoorOpen01 moves animated door instances and updates CameraBlockers::mark_moving.
    let door = rt
        .w
        .instances
        .iter()
        .position(|(n, _, _)| n == "leftlibrarydoor")
        .expect("leftlibrarydoor instance") as u32;
    let (scene, _) = crate::assets::load_battle_scene_with_layers(
        &baked_dir.join(&def.map),
        "campaign",
        &def.layers,
    )
    .expect("scene loads");
    let inst = &scene.manifest.instances[door as usize];
    let model_name = scene.resolve_model(&inst.class).expect("door model");
    let cpu_mesh = &scene.meshes[model_name];
    let mut local_min = Vec3::splat(f32::MAX);
    let mut local_max = Vec3::splat(f32::MIN);
    for p in &cpu_mesh.positions {
        let v = Vec3::from(*p);
        local_min = local_min.min(v);
        local_max = local_max.max(v);
    }
    let door_mat = crate::viewer::instance_matrix(inst.rotation, inst.translation);
    let door_center = door_mat.transform_point3((local_min + local_max) * 0.5);
    let local_ext = local_max - local_min;
    let local_thin_axis = if local_ext.x <= local_ext.z {
        Vec3::X
    } else {
        Vec3::Z
    };
    let door_normal = door_mat.transform_vector3(local_thin_axis).normalize();
    let mut blockers = crate::camera_collide::CameraBlockers::from_scene(&scene);
    blockers.mark_moving(rt.animated_instances());
    let cam_from = door_center + door_normal * 1.2;
    let cam_to = door_center - door_normal * 1.2;
    let is_door_prop = blockers.is_prop(door).expect("leftlibrarydoor must be a blocker");
    if is_door_prop {
        assert!(
            blockers
                .see_through(cam_from, &[cam_to], None, |_| None, &walk.sim.doors)
                .iter()
                .any(|(ii, _)| *ii == door),
            "closed library door prop must obstruct sight line before opening"
        );
    } else {
        assert!(
            blockers.fraction(cam_from, cam_to, None, |_| None, &walk.sim.doors) < 1.0,
            "closed library door must obstruct camera ray before opening (hits: {:?})",
            blockers.track_hits(cam_from, cam_to, &walk.sim.doors)
        );
    }

    walk.sim.cheats.invincible = true;

    // Advance from Objective 1 -> Objective 2a (opens DoorOpen01 + UnblockPlanningGraphArcs(1)).
    if let Some(ci) = rt.w.find_cp(&walk.sim, "Consul_CP") {
        walk.sim.cps[ci].owner = Some(0);
    }
    assert!(rt.complete_objective(&mut walk.sim, "Objective1"));
    for _ in 0..(8 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if rt
            .objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.campaign.2.a") && o.state == 2)
        {
            break;
        }
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.campaign.2.a") && o.state == 2),
        "Objective2a should be active, got {:?}",
        rt.objectives()
    );
    for _ in 0..(5 * core::TICK_HZ) {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let poses = rt.instance_poses();
    assert!(poses.contains_key(&door), "DoorOpen01 must pose leftlibrarydoor");
    if is_door_prop {
        assert!(
            !blockers
                .see_through(
                    cam_from,
                    &[cam_to],
                    None,
                    |inst| poses.get(&inst).copied(),
                    &walk.sim.doors,
                )
                .iter()
                .any(|(ii, _)| *ii == door),
            "opened DoorOpen01 must clear see-through obstruction"
        );
    } else {
        let frac = blockers.fraction(
            cam_from,
            cam_to,
            None,
            |inst| poses.get(&inst).copied(),
            &walk.sim.doors,
        );
        assert_eq!(
            frac,
            1.0,
            "opened DoorOpen01 must let camera ray pass cleanly (hits: {:?}, pose: {:?})",
            blockers.track_hits(cam_from, cam_to, &walk.sim.doors),
            poses.get(&door)
        );
    }

    // Bug 4: Objective 2a locks LibCase AI damage threshold to 1.0; Objective 2c unlocks (0.0),
    // spawns JediLibraryDefenders (AMB team 3), issues Destroy goals on LibCase1..4, and AI deals damage.
    let libcase_indices: Vec<usize> = rt
        .w
        .station_names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.starts_with("libcase").then_some(i))
        .collect();
    assert_eq!(libcase_indices.len(), 4, "4 library bookcases");
    for &si in &libcase_indices {
        assert_eq!(walk.sim.stations[si].ai_damage_threshold, 1.0);
    }
    if let Some(ci) = rt.w.find_cp(&walk.sim, "Library_CP") {
        walk.sim.cps[ci].owner = Some(0);
    }
    assert!(rt.complete_objective(&mut walk.sim, "Objective2a"));
    for _ in 0..(8 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if rt
            .objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.Campaign.2.c") && o.state == 2)
        {
            break;
        }
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.Campaign.2.c") && o.state == 2),
        "Objective2c should be active, got {:?}",
        rt.objectives()
    );
    assert_eq!(walk.sim.stations[libcase_indices[0]].ai_damage_threshold, 0.0);
    assert!(
        walk.sim
            .units
            .iter()
            .any(|u| u.alive && u.team == 2),
        "AMB (Lua team 3 -> sim team 2) JediLibraryDefenders must spawn"
    );
    let hp_before: Vec<f32> = libcase_indices
        .iter()
        .map(|&si| walk.sim.stations[si].health.cur)
        .collect();
    let mut any_damaged = false;
    for _ in 0..(15 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if libcase_indices
            .iter()
            .zip(hp_before.iter())
            .any(|(&si, &before)| walk.sim.stations[si].health.cur < before || walk.sim.stations[si].destroyed)
        {
            any_damaged = true;
            break;
        }
    }
    assert!(any_damaged, "Objective 2c AMB/GAR bots must attack and damage LibCase stations");

    // Bug 3: Advance to Objective 5 -> spawns flagrunner (RUN, Lua team 5 -> sim team 4) who picks up holocron2,
    // and holocron2's placed class has base == "flag" so the static ground draw fades on pickup.
    let holo2_inst = scene
        .manifest
        .instances
        .iter()
        .find(|inst| inst.name.eq_ignore_ascii_case("holocron2"))
        .expect("holocron2 placed instance in bf2_cor1");
    assert!(
        scene
            .resolve_class(&holo2_inst.class)
            .is_some_and(|c| c.base.eq_ignore_ascii_case("flag")),
        "holocron2 class ({}) must have base == flag for flag_instances fading",
        holo2_inst.class
    );
    assert!(rt.complete_objective(&mut walk.sim, "Objective2c"));
    for _ in 0..(6 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if rt
            .objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.campaign.4") && o.state == 2)
        {
            break;
        }
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.campaign.4") && o.state == 2),
        "Objective4 should be active, got {:?}",
        rt.objectives()
    );
    // Complete Objective4 -> Objective4.OnComplete calls UnlockHeroForTeam(ATT).
    walk.sim.tick = walk.sim.tick.max(60 * u64::from(core::TICK_HZ));
    assert!(rt.complete_objective(&mut walk.sim, "Objective4"));
    let mut saw_anakin_offer = false;
    for _ in 0..(6 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        if walk.sim.hero_offer(0).map(|(c, _)| c) == Some(hero_class_idx) {
            saw_anakin_offer = true;
        }
        if rt
            .objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.campaign.5") && o.state == 2)
        {
            break;
        }
    }
    if walk.sim.hero_offer(0).map(|(c, _)| c) == Some(hero_class_idx) {
        saw_anakin_offer = true;
    }
    assert!(
        rt.objectives()
            .iter()
            .any(|o| o.text.eq_ignore_ascii_case("level.cor1.objectives.campaign.5") && o.state == 2),
        "Objective5 should be active, got {:?}",
        rt.objectives()
    );
    assert!(
        saw_anakin_offer,
        "Completing Objective 4 (UnlockHeroForTeam) must offer Anakin (F1/F2 prompt) to player 0, heroes state: {:?}",
        walk.sim.heroes
    );
    let holo2_idx = rt
        .w
        .flags
        .iter()
        .position(|f| f.name == "holocron2")
        .expect("holocron2 must be registered as a flag");
    let flag = &walk.sim.ctf.as_ref().unwrap().flags[rt.w.flags[holo2_idx].ctf];
    assert!(flag.allow_ai_pickup, "holocron2 must allow AI pickup for flagrunner");
    assert_eq!(flag.flyer_pickup_radius, 0.0);

    let mut runner_carried = false;
    for _ in 0..(10 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
        let f = &walk.sim.ctf.as_ref().unwrap().flags[rt.w.flags[holo2_idx].ctf];
        if let Some(carrier) = f.carrier() {
            if walk.sim.unit(carrier).is_some_and(|u| u.team == 4) {
                runner_carried = true;
                break;
            }
        }
    }
    assert!(runner_carried, "flagrunner (RUN team) must spawn and pick up holocron2 in Objective 5");
    // The pickup handler was registered as OnFlagPickUp(fn, "holocron"); BF2
    // ignores that argument, so the runner's pickup of `holocron2` must reach
    // it and hang the objective marker over the carrying Jedi.
    for _ in 0..core::TICK_HZ {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    let carrier = walk.sim.ctf.as_ref().unwrap().flags[rt.w.flags[holo2_idx].ctf].carrier();
    if let Some(carrier) = carrier.filter(|c| walk.sim.unit(*c).is_some_and(|u| u.team == 4)) {
        assert!(
            rt.w.markers.iter().any(|m| m.icon == "hud_objective_icon"
                && m.target == MarkTarget::Obj(Ud::Unit(carrier.index, carrier.gen))),
            "Objective 5 must mark the Jedi carrying holocron2, markers: {:?}",
            rt.w.markers
        );
        assert!(
            rt.objective_markers(&walk.sim)
                .iter()
                .any(|m| m.icon == "hud_objective_icon"),
            "the carrier marker must be drawn in the world"
        );
        // Killing the running Jedi on the ground stops it in place (no corpse
        // slide across the floor) and drops holocron2.
        let (death_pos, airborne_jedi) = {
            let u = &mut walk.sim.units[carrier.index as usize];
            assert!(
                u.consts.flee_like_a_hero,
                "Jedi classes must set FleeLikeAHero so they use BF2 HeroDead transitions"
            );
            u.soldier.grounded = true;
            u.soldier.stance = core::soldier::Stance::Sprint;
            u.soldier.vel = glam::Vec3::new(9.0, 0.0, 0.0);
            let pos = u.soldier.pos;
            (pos, walk.sim.units.iter().position(|o| o.alive && o.team == 1 && o.spec.class == 0))
        };
        walk.sim.kill_unit_by(carrier, None);
        assert_eq!(
            walk.sim.units[carrier.index as usize].soldier.vel,
            glam::Vec3::ZERO,
            "grounded Jedi death must immediately zero horizontal velocity instead of sliding"
        );
        tick(&mut walk, &mut rt, &def);
        assert_eq!(
            walk.sim.units[carrier.index as usize].soldier.stance,
            core::soldier::Stance::Dead
        );
        assert!(
            (walk.sim.units[carrier.index as usize].soldier.pos - death_pos).length() < 0.05,
            "grounded Jedi corpse must not slide across the floor during HeroDead"
        );
        if let Some(j_idx) = airborne_jedi {
            let jh = walk.sim.handle(j_idx);
            walk.sim.units[j_idx].consts.flee_like_a_hero = true;
            walk.sim.units[j_idx].soldier.grounded = false;
            walk.sim.units[j_idx].soldier.stance = core::soldier::Stance::Jump;
            walk.sim.units[j_idx].soldier.vel = glam::Vec3::new(4.0, 6.0, 0.0);
            walk.sim.kill_unit_by(jh, None);
            assert_eq!(
                walk.sim.units[j_idx].soldier.stance,
                core::soldier::Stance::Thrown,
                "airborne Jedi death must transition to Thrown instead of locking into HeroDead mid-air"
            );
        }
    }
    assert_eq!(rt.errors().count(), 0, "no Lua script errors during cor1c_c run");
}

fn call_script(
    rt: &mut CampaignRuntime,
    sim: &mut core::sim::Sim,
    name: &str,
    args: Vec<Value>,
) {
    use crate::campaign_script::Host;
    let mut host = crate::campaign_runtime::Ctx { w: &mut rt.w, sim };
    if host.known(name) {
        host.call(&mut rt.interp, name, &args).unwrap();
    } else {
        let f = rt.interp.global(name);
        rt.interp.call_value(&mut host, &f, args).unwrap();
    }
}

#[test]
fn myg1c_destroying_one_generator_clears_only_its_marker() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: myg1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "myg1c_c");
    let resolve = |name: &str| match rt.w.resolve_name(&walk.sim, name) {
        Some(Ud::Station(si)) => si,
        other => panic!("{name} is not a station: {other:?}"),
    };
    let s1 = resolve("backgen1");
    let s2 = resolve("backgen2");
    let s3 = resolve("generator_01");
    let team = rt.w.player_team + 1;
    rt.w.markers = vec![s1, s2, s3]
        .into_iter()
        .map(|si| Marker {
            target: MarkTarget::Obj(Ud::Station(si)),
            icon: "hud_objective_icon".into(),
            scale: 4.0,
            team,
            pulse: true,
        })
        .collect();
    assert_eq!(rt.objective_markers(&walk.sim).len(), 3);
    assert_eq!(rt.map_markers(&walk.sim).len(), 3);

    walk.step_tick(&[core::sim::PadState::default()]);
    walk.sim.kill_station(s1, None);
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &[],
        core::TICK_DT,
    );
    let has_marker = |rt: &CampaignRuntime, si: u16| {
        rt.w.markers
            .iter()
            .any(|m| m.target == MarkTarget::Obj(Ud::Station(si)))
    };
    assert!(!has_marker(&rt, s1), "destroyed backgen1 marker must clear");
    assert!(has_marker(&rt, s2), "living backgen2 marker must remain");
    assert!(has_marker(&rt, s3), "living generator_01 marker must remain");
    assert_eq!(rt.objective_markers(&walk.sim).len(), 2);
    assert_eq!(rt.map_markers(&walk.sim).len(), 2);

    call_script(
        &mut rt,
        &mut walk.sim,
        "SetProperty",
        vec![Value::str("backgen2"), Value::str("CurHealth"), Value::Num(0.0)],
    );
    assert!(
        !has_marker(&rt, s2),
        "CurHealth <= 0 on backgen2 must clear its marker"
    );
    call_script(
        &mut rt,
        &mut walk.sim,
        "KillObject",
        vec![Value::str("generator_01")],
    );
    assert!(
        !has_marker(&rt, s3),
        "KillObject on generator_01 must clear its marker"
    );
    assert!(rt.objective_markers(&walk.sim).is_empty());
    assert!(rt.map_markers(&walk.sim).is_empty());
}

#[test]
fn myg1c_ambush_spawns_native_team_5_droidekas() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: myg1c_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "myg1c_c");
    let native_template = walk
        .native_teams
        .get(&4)
        .expect("BF2 team 5 (cis_inf_droideka) must install on myg1c_c");
    assert!(native_template
        .classes
        .iter()
        .any(|c| c.name.eq_ignore_ascii_case("cis_inf_droideka")));
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
    call_script(
        &mut rt,
        &mut walk.sim,
        "Ambush",
        vec![Value::str("droideka"), Value::Num(3.0), Value::Num(5.0)],
    );
    let droidekas: Vec<_> = walk
        .sim
        .units
        .iter()
        .filter(|u| u.team == 4 && u.droideka.is_some())
        .collect();
    assert_eq!(
        droidekas.len(),
        3,
        "Ambush(\"droideka\", 3, 5) must spawn 3 live team-5 droidekas"
    );
    assert!(droidekas.iter().all(|u| u.alive));
    let deka_class = droidekas[0]
        .droideka_class()
        .expect("spawned team 5 unit must carry DroidekaClass");
    assert!(
        !deka_class.walk_ordnance.is_empty() && !deka_class.guns.is_empty(),
        "team 5 droideka must receive side bake ordnance and guns"
    );
}

#[test]
fn spa4g_c_gunship_spawns_cis_droids_in_imperial_hangar() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa4g_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa4g_c");
    // Step a couple ticks so pads build vehicles (including `cis_gunship`).
    for _ in 0..5 {
        walk.step_tick(&[core::sim::PadState::default()]);
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &team_classes(&def),
            &BTreeMap::new(),
            &walk.vehicle_classes,
            core::TICK_DT,
        );
    }
    let gunship_class = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case("cis_fly_droidgunship"))
        .expect("cis_fly_droidgunship class loaded") as u32;
    let fp = walk
        .sim
        .flyer_params
        .get(gunship_class as usize)
        .and_then(Option::as_ref)
        .expect("cis_fly_droidgunship FlyerParams");
    assert!(fp.command_flyer, "cis_fly_droidgunship must be a commandflyer");
    assert_eq!(
        fp.spawn_points.len(),
        6,
        "cis_fly_droidgunship must carry its 6 authored SpawnPointLocations"
    );
    let (gunship_handle, gunship) = walk
        .sim
        .command_walkers(1)
        .find(|(_, v)| v.class == gunship_class)
        .expect("landed cis_gunship must be an active CIS command spawn post");
    let gunship_pos = gunship.pos;
    // Verify the gunship's missile homing aim point lands inside its solid
    // ordnance hull from the player's hangar floor position (not the gap under
    // its saucer at visual sphere_centre y=0.18).
    let target_pts = &walk.sim.missile_target_points[gunship_class as usize];
    assert!(
        !target_pts.is_empty(),
        "cis_fly_droidgunship must have a lifted solid ordnance missile target point"
    );
    let world_aim = gunship.pos + gunship.rot * target_pts[0];
    let shooter_eye = Vec3::new(-928.77, -83.54, -391.54);
    let aim_dir = (world_aim - shooter_eye).normalize();
    let hit = walk.sim.collision.ray_with(
        shooter_eye,
        aim_dir,
        100.0,
        core::collision::Layers::ORDNANCE,
        core::collision::HullFilter::All,
    );
    assert_eq!(
        hit.and_then(|h| h.owner),
        Some(gunship_handle),
        "locked rocket ray from hangar floor toward cis_gunship aim point must hit its ordnance hull"
    );
    let mut near_gunship_initial = 0usize;
    for u in &walk.sim.units {
        if u.alive && u.team == 1 && (u.soldier.pos - gunship_pos).length() < 30.0 {
            near_gunship_initial += 1;
        }
    }
    // `setup_baked_campaign_full` spawns all 22 authored CIS units while
    // `battle.rs` caps opening AI at `def.initial_ai(1, false)` (= 16).
    // Match `battle.rs`'s live cap so killing hangar droids drops below
    // `ai_limit(1)` and triggers immediate respawns, just like real play.
    walk.sim.ai_alive_cap = def.teams[1].units;
    let mut total_cis_respawns = 0usize;
    let mut respawns_near_gunship = 0usize;
    let mut gunship_phase_after = core::vehicle::flyer::FlightPhase::Landed;
    let mut gunship_pos_after = gunship_pos;
    for tick in 0..900 {
        if tick == 10 || tick == 450 {
            let victims: Vec<_> = walk
                .sim
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| u.alive && u.team == 1 && (u.soldier.pos - gunship_pos).length() < 40.0)
                .map(|(i, u)| core::unit::UnitHandle { index: i as u32, gen: u.gen })
                .collect();
            for h in victims {
                walk.sim.kill_environment(h, core::sim::DeathCause::Suicide);
            }
        }
        walk.step_tick(&[core::sim::PadState::default()]);
        for ev in &walk.sim.events {
            if let core::sim::Event::Respawn { unit } = *ev {
                let u = &walk.sim.units[unit.index as usize];
                if u.team == 1 {
                    total_cis_respawns += 1;
                    if (u.soldier.pos - gunship_pos).length() < 25.0 {
                        respawns_near_gunship += 1;
                    }
                }
            }
        }
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &team_classes(&def),
            &BTreeMap::new(),
            &walk.vehicle_classes,
            core::TICK_DT,
        );
        if let Some(v) = walk.sim.vehicle(gunship_handle) {
            gunship_phase_after = v.flyer.phase;
            gunship_pos_after = v.pos;
        }
    }
    assert!(
        near_gunship_initial > 0,
        "opening wave must spawn CIS droids near cis_gunship"
    );
    assert!(
        respawns_near_gunship > 0,
        "CIS droids must keep respawning near cis_gunship while it is alive and landed (initial={near_gunship_initial}, total_cis_respawns={total_cis_respawns}, respawns={respawns_near_gunship}, phase={gunship_phase_after:?}, pos={gunship_pos_after:?})"
    );
    assert!(
        walk.sim.vehicle(gunship_handle).is_some(),
        "cis_gunship must still be alive in the hangar (last phase={gunship_phase_after:?}, last pos={gunship_pos_after:?}, start pos={gunship_pos:?})"
    );
    // Taking off disables the CommandFlyer's spawn post (BF2 FUN_00352cf8 / FUN_00352ee0).
    walk.sim.vehicle_mut(gunship_handle).unwrap().flyer.phase =
        core::vehicle::flyer::FlightPhase::Flying;
    assert!(
        walk.sim.command_walkers(1).all(|(h, _)| h != gunship_handle),
        "a flying CommandFlyer must not be an eligible spawn post"
    );
    // Landing re-enables it.
    walk.sim.vehicle_mut(gunship_handle).unwrap().flyer.phase =
        core::vehicle::flyer::FlightPhase::Landed;
    assert!(
        walk.sim.command_walkers(1).any(|(h, _)| h == gunship_handle),
        "a landed CommandFlyer must be an eligible spawn post again"
    );
    // Destroying it removes it as a spawn post.
    walk.sim.vehicle_mut(gunship_handle).unwrap().health.cur = 0.0;
    assert!(
        walk.sim.command_walkers(1).all(|(h, _)| h != gunship_handle),
        "a destroyed CommandFlyer must not be an eligible spawn post"
    );
}


/// spa4g's ScriptPostLoad calls `SetAIDifficulty(2, -9, "medium")`: the
/// pair lands on the medium level and leaves easy and hard alone, before
/// the opening wave spawns.
#[test]
fn spa4g_c_script_sets_its_medium_ai_difficulty_pair() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa4g_c mission and baked assets required");
        return;
    };
    let (_def, walk, _rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa4g_c");
    assert_eq!(walk.sim.mission_ai.skill_delta, [[0, 0], [2, -9], [0, 0]]);
}

/// spa4's `barrier19` covers the `cis_bomber` pad, so the vehicle notify
/// scan (`FUN_002e2e38`) never offers its greviousfighter to a bot: it is
/// still parked, with nobody aboard, when Objective 4 asks the player to
/// steal it.
#[test]
fn spa4g_c_bots_leave_the_barred_cis_bomber_parked() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa4g_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa4g_c");
    let bomber_class = walk
        .vehicle_classes
        .iter()
        .position(|c| c.eq_ignore_ascii_case("cis_fly_greviousfighter"))
        .expect("cis_fly_greviousfighter class loaded") as u32;
    let pad = glam::Vec2::new(-260.471, -1809.34);
    let (bomber, _) = walk
        .sim
        .alive_vehicles()
        .find(|(_, v)| v.class == bomber_class && glam::Vec2::new(v.pos.x, v.pos.z).distance(pad) < 1.0)
        .expect("the cis_bomber pad builds its greviousfighter");
    for _ in 0..1800 {
        walk.step_tick(&[core::sim::PadState::default()]);
        rt.step(
            &mut walk.sim,
            walk.player,
            None,
            &team_classes(&def),
            &BTreeMap::new(),
            &walk.vehicle_classes,
            core::TICK_DT,
        );
        assert!(
            !walk.sim.units.iter().any(|u| u.alive && u.mount.is_some_and(|m| m.vehicle == bomber)),
            "nobody boards the barred bomber"
        );
    }
    let v = walk.sim.vehicle(bomber).expect("the bomber is still there");
    assert!(glam::Vec2::new(v.pos.x, v.pos.z).distance(pad) < 1.0, "still on its pad: {:?}", v.pos);
}

#[test]
fn dea1g_c_landing_craft_rests_on_its_gear_and_the_rebel_waves_reach_the_hangar() {
    let Some((missions_dir, baked_dir)) = baked() else {
        return;
    };
    if !baked_dir
        .join("bf2_dea1")
        .join(manifest::world_anim::FILE)
        .exists()
    {
        eprintln!("SKIP: bf2_dea1 world animations not baked");
        return;
    }
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "dea1g_c");

    // 1. `incoming01` then `incoming03` set `trans01` down. The pad links the
    //    animation to its flyer AFTER the constructor added LandedHeight
    //    (FUN_003725f0 → FUN_00274738), so its origin ends LandedHeight
    //    (2.2516 m, -vis_min_y) over the 50.0 m deck, less land03's 1.6 cm.
    for _ in 0..(14 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
    }
    let pad = walk
        .sim
        .vehicle_pads
        .iter()
        .find(|p| p.def.name.eq_ignore_ascii_case("trans01"))
        .expect("trans01 pad");
    let ship = walk
        .sim
        .vehicle(pad.vehicles.last().expect("trans01 spawned").handle)
        .expect("trans01 alive");
    let rest = pad.def.pos.y + 2.2516367 - 8.93;
    assert!(
        (ship.pos.y - rest).abs() < 0.05,
        "trans01 must land on its gear at y {rest}, got {}",
        ship.pos.y
    );

    // 2. `SetProperty("CAM_CP2", "Value_ATK_Empire", 10)` and
    //    `..."Value_DEF_Alliance", 10)` reach the Empire (team 0 here) and
    //    the Alliance (team 1), by each team's faction.
    let cp2 = walk
        .sim
        .cps
        .iter()
        .find(|c| c.def.name.eq_ignore_ascii_case("cam_cp2"))
        .expect("cam_cp2");
    assert_eq!(cp2.def.value_atk[0], 10, "Value_ATK_Empire is team 0's");
    assert_eq!(cp2.def.value_def[1], 10, "Value_DEF_Alliance is team 1's");

    // 3. `all_spawn` stands 0.62 m under the corridor deck: its hub still
    //    resolves to that deck, not the level 28 m below, so the corridor
    //    from the Alliance's room is no trap.
    let spawn_hub = walk
        .sim
        .cps
        .iter()
        .find(|c| c.def.name.eq_ignore_ascii_case("all_spawn"))
        .and_then(|c| c.nav_hub)
        .expect("all_spawn hub");
    let floor = walk.sim.nav_floor(spawn_hub).expect("resolved");
    assert!(
        (floor - 50.0).abs() < 0.5,
        "all_spawn's hub floor {floor}, the corridor deck is 50.0"
    );
    assert!(!walk.sim.hub_trapped(spawn_hub), "the corridor is no trap");

    // 4. The hangar ambush fields all six of team 5 after `defend_ambush`'s
    //    three have died: the members keep their places.
    call_script(
        &mut rt,
        &mut walk.sim,
        "Ambush",
        vec![Value::str("defend_ambush"), Value::Num(3.0), Value::Num(5.0)],
    );
    let first: Vec<_> = walk
        .sim
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.team == 4 && u.alive)
        .map(|(i, u)| core::unit::UnitHandle {
            index: i as u32,
            gen: u.gen,
        })
        .collect();
    assert_eq!(first.len(), 3, "defend_ambush fields three");
    for h in first {
        walk.sim.kill_unit_by(h, None);
    }
    call_script(
        &mut rt,
        &mut walk.sim,
        "Ambush",
        vec![Value::str("hangar_ambush_spawn"), Value::Num(6.0), Value::Num(5.0)],
    );
    let alive = walk.sim.units.iter().filter(|u| u.team == 4 && u.alive).count();
    assert_eq!(alive, 6, "hangar_ambush_spawn fields all six of team 5");
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
        call_script(&mut rt, &mut walk.sim, "ScriptCB_EnableCommandPostVO", vec![Value::Num(n)]);
        assert_eq!(rt.command_post_vo(), on, "n = {n}");
    }
}

/// Tantive IV ("Recovering the Plans", `tan1g_c`): the boarding-room
/// `blastdoor` (`tan4_prop_door_charred`, `destructablebuilding` with no
/// `MaxHealth`) must stay invulnerable to gunfire, then explode and clear its
/// collision 1.7 s after the human spawns (`doorunlock` -> `KillObject`).
#[test]
fn tan1g_c_opening_blastdoor_blows_up_and_clears_collision_after_spawn() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: tan1g_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "tan1g_c");
    let Some(Ud::Station(si)) = rt.w.resolve_name(&walk.sim, "blastdoor") else {
        panic!(
            "blastdoor is not a station: {:?}",
            rt.w.resolve_name(&walk.sim, "blastdoor")
        );
    };
    let st = &walk.sim.stations[usize::from(si)];
    assert_eq!(st.health.max, 0.0);
    assert!(!st.destroyed);
    let inst = st.instance.expect("blastdoor placed instance");

    // Gunfire must not destroy a zero-MaxHealth scripted prop.
    let desc = core::weapon::DamageDesc {
        damage: 500.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    walk.sim.damage_station(si, &desc, 1.0, None, Some(0));
    assert!(
        !walk.sim.stations[usize::from(si)].destroyed,
        "weapon damage must not destroy a zero-MaxHealth destructible prop"
    );

    // Before doorunlock elapses, the door owns live collision parts.
    let pose = walk.sim.stations[usize::from(si)].pose();
    assert!(
        !walk.sim.collision.instance_parts(inst, pose).is_empty(),
        "blastdoor should have live collision before doorunlock"
    );

    // Spawn the human player and tick past the 1.7 s `doorunlock` timer.
    begin_objectives(&mut walk, &mut rt, &def);
    for _ in 0..(2 * core::TICK_HZ as usize) {
        tick(&mut walk, &mut rt, &def);
    }

    assert!(
        walk.sim.stations[usize::from(si)].destroyed,
        "blastdoor must be destroyed when doorunlock elapses"
    );
    assert!(
        walk.sim.collision.instance_parts(inst, pose).is_empty(),
        "blastdoor collision must clear once destroyed"
    );
}

/// Tantive IV ("Recovering the Plans", `tan1g_c`): Objective 4's `OnStart`
/// passes the return value of `AddUnitClass(DEF, "all_hero_leia", 1, 1)` to
/// `SelectCharacterClass` before `SpawnCharacter(..., GetPathPoint("leia_spawn", 0))`.
/// Leia must spawn alive on team `DEF` at `leia_spawn`.
#[test]
fn tan1g_c_objective4_spawns_princess_leia_at_leia_spawn() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: tan1g_c mission and baked assets required");
        return;
    };
    let (_def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "tan1g_c");
    // Start Objective4 via its ObjectiveAssault:Start() method so TargetType's
    // MapAddClassMarker(..., iconScale = 0.0) and OnStart() both run.
    let obj4 = rt.interp.global("Objective4");
    let start = {
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        rt.interp
            .index(&mut host, &obj4, &Value::str("Start"))
            .expect("Objective4.Start resolves")
    };
    {
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        rt.interp
            .call_value(&mut host, &start, vec![obj4])
            .expect("Objective4:Start runs");
    }
    let leia = walk
        .sim
        .units
        .iter()
        .find(|u| {
            u.alive
                && u.team == 1
                && rt
                    .w
                    .team_classes
                    .get(usize::from(u.team))
                    .and_then(|cs| cs.get(usize::from(u.spec.class)))
                    .is_some_and(|n| n.eq_ignore_ascii_case("all_hero_leia"))
        })
        .expect("Princess Leia must spawn on team DEF when Objective4 starts");
    let expected = glam::Vec3::new(-290.6831, 2.918823, 202.68283);
    assert!(
        (leia.soldier.pos - expected).length() < 1.5,
        "Leia spawned at {:?}, expected near {expected:?}",
        leia.soldier.pos
    );
    assert!(
        rt.objective_markers(&walk.sim).is_empty(),
        "Princess target sets iconScale = 0.0 so no 3D objective arrow should appear over Leia"
    );
    assert!(
        rt.map_markers(&walk.sim).is_empty(),
        "Princess target sets iconScale = 0.0 so no minimap marker should appear for Leia"
    );
    assert_eq!(rt.errors().count(), 0, "no script errors");
}

/// Tantive IV ("Recovering the Plans", `tan1g_c`): destroying `turbineconsole`
/// fires `destturbine` -> `RemoveRegion("turbinedeath")`, removing the turbine
/// kill volume so soldiers can pass through; respawning it fires `returbine`
/// -> `AddDeathRegion("turbinedeath")`, restoring the kill volume.
#[test]
fn tan1g_c_destroying_turbineconsole_removes_turbinedeath_region() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: tan1g_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "tan1g_c");
    let turbine_region = rt
        .w
        .regions
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("turbinedeath"))
        .map(|(_, r)| *r)
        .expect("turbinedeath region in bf2_tan1");
    assert!(
        walk.sim.death_regions.contains(&turbine_region),
        "turbinedeath should be active before turbineconsole is destroyed"
    );
    let Some(Ud::Station(console)) = rt.w.resolve_name(&walk.sim, "turbineconsole") else {
        panic!("turbineconsole is not a station");
    };

    // Destroy the console and step the runtime so OnObjectKillName fires.
    walk.step_tick(&[core::sim::PadState::default()]);
    walk.sim.kill_station(console, Some(walk.player));
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &team_classes(&def),
        &BTreeMap::new(),
        &[],
        core::TICK_DT,
    );
    assert!(
        !walk.sim.death_regions.contains(&turbine_region),
        "turbinedeath must be removed once turbineconsole is destroyed"
    );

    // Re-adding turbinedeath via returbine restores the kill volume.
    call_script(&mut rt, &mut walk.sim, "returbine", vec![]);
    assert!(
        walk.sim.death_regions.contains(&turbine_region),
        "turbinedeath must be restored when returbine runs"
    );
    assert_eq!(rt.errors().count(), 0, "no script errors");
}

#[test]
fn spa1g_c_animated_transports_and_corvettes_spawn_chunks_at_live_pose() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa1g_c mission and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa1g_c");
    let map_dir = baked_dir.join("bf2_spa1");
    let mut fx = crate::fx::Fx::for_walk(&baked_dir, &map_dir, "bf2", &walk);
    fx.set_lit_chunks(
        fx.chunk_models()
            .into_iter()
            .map(|m| m.to_ascii_lowercase()),
    );

    // Advance 10 seconds so trans01 (mini01) and frig01 (cor01) move away from their
    // initial placement origins.
    for _ in 0..(10 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }
    // Start cor01's animation group too so we verify animated corvettes as well.
    rt.w.anims.play("gate01");
    for _ in 0..(10 * core::TICK_HZ) {
        tick(&mut walk, &mut rt, &def);
        rt.close_popup();
    }

    for (name, chunk_prefix, expected_chunks) in [
        ("mini01", "spa1_prop_correliancruiser_chunk", 4usize),
        ("cor01", "all_prop_blockaderunner_chunk", 2usize),
    ] {
        let Some(Ud::Station(si)) = rt.w.resolve_name(&walk.sim, name) else {
            panic!("{name} is not a station");
        };
        let st = walk.sim.stations[usize::from(si)];
        let live_pos = st.transform.w_axis.truncate();
        assert!(
            live_pos.distance(st.pos) > 50.0,
            "{name} should have moved away from its initial placement origin ({} vs {})",
            live_pos,
            st.pos
        );

        walk.sim.events.clear();
        walk.sim.kill_station(si, Some(walk.player));
        let event_pos = walk
            .sim
            .events
            .iter()
            .find_map(|e| match *e {
                core::sim::Event::StationDestroyed {
                    station, pos, ..
                } if station == si => Some(pos),
                _ => None,
            })
            .expect("StationDestroyed event");
        assert!(
            event_pos.distance(live_pos) < 1e-3,
            "{name}: StationDestroyed pos {event_pos} != live animated pos {live_pos}"
        );

        fx.clear();
        fx.ingest(&walk);
        let pieces: Vec<(String, glam::Vec3)> = fx
            .lit_chunk_poses()
            .filter(|(m, _, _)| m.to_ascii_lowercase().starts_with(chunk_prefix))
            .map(|(m, mat, _)| (m.to_owned(), mat.w_axis.truncate()))
            .collect();
        assert_eq!(
            pieces.len(),
            expected_chunks,
            "{name}: expected {expected_chunks} destruction chunks, got {}",
            pieces.len()
        );
        let centroid =
            pieces.iter().map(|(_, p)| *p).sum::<glam::Vec3>() / pieces.len() as f32;
        assert!(
            centroid.distance(live_pos) < 150.0,
            "{name}: chunks spawned at {centroid} instead of live pos {live_pos} (old pos was {})",
            st.pos
        );
    }
}
