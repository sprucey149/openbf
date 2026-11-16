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
        ("spa3c_c", 0usize, [false, true], None),
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
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let _ = rt.take_voice_overs();
    let _ = rt.take_sounds();
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
