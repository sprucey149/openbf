use super::*;
use crate::walk::WalkSim;

pub(super) fn setup_baked_campaign(
    missions_dir: &std::path::Path,
    baked_dir: &std::path::Path,
    script: &str,
) -> (crate::mission::MissionDef, WalkSim, CampaignRuntime) {
    setup_baked_campaign_with(missions_dir, baked_dir, script, None)
}

/// [`setup_baked_campaign`] with `hero` appended to the first army's roster
/// as the game's battle setup does (`heroes::append`).
pub(super) fn setup_baked_campaign_with(
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

pub(super) fn team_classes(def: &crate::mission::MissionDef) -> [Vec<String>; 2] {
    std::array::from_fn(|t| {
        def.teams[t]
            .classes
            .iter()
            .map(|(c, _, _)| c.clone())
            .collect()
    })
}

pub(super) fn tick(walk: &mut WalkSim, rt: &mut CampaignRuntime, def: &crate::mission::MissionDef) {
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
pub(super) fn begin_objectives(
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

/// The checkout's missions and bake, or another copy's: `OPENBF_CAMPAIGN_MISSIONS`
/// and `OPENBF_CAMPAIGN_BAKED` point these tests at a PC, Xbox or mixed build's
/// output, so the campaign is checked against whichever copy made it.
pub(super) fn baked() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let missions_dir = std::env::var_os("OPENBF_CAMPAIGN_MISSIONS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions"));
    let baked_dir = std::env::var_os("OPENBF_CAMPAIGN_BAKED")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::assets::baked_root);
    (missions_dir.join("campaign/bf2_geo1c_c.ron").is_file()
        && baked_dir.join("bf2_geo1/manifest.ron").is_file())
    .then_some((missions_dir, baked_dir))
}

pub(super) fn setup_baked_campaign_full(
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
    let mut rt = CampaignRuntime::new(
        spec,
        &layout,
        &scene.manifest,
        &walk.post_keys,
        saved,
        &walk.sim,
    );
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

pub(super) fn call_script(
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

#[path = "tests/fel_dea_tan.rs"]
mod fel_dea_tan;
#[path = "tests/hoth.rs"]
mod hoth;
#[path = "tests/kam_uta_cor.rs"]
mod kam_uta_cor;
#[path = "tests/myg_mus.rs"]
mod myg_mus;
#[path = "tests/spa.rs"]
mod spa;
#[path = "tests/startup.rs"]
mod startup;
#[path = "tests/tutorial_geo.rs"]
mod tutorial_geo;

fn synthetic_runtime(script: manifest::script_ir::ScriptDef) -> (WalkSim, CampaignRuntime) {
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    let u = &walk.sim.units[walk.player.index as usize];
    let uc = core::unit::UnitClass {
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
    walk.sim.classes[0] = vec![uc.clone()];
    walk.sim.classes[1] = vec![uc];
    walk.sim.class_enabled[0] = vec![true];
    walk.sim.class_enabled[1] = vec![true];
    walk.sim.battle.script_rules = true;
    let mut layout = crate::layout::MapLayout::default();
    layout.paths.push(crate::layout::SpawnPath {
        name: "spawn_path".into(),
        nodes: vec![crate::layout::SpawnNode {
            pos: Vec3::new(10.0, 0.0, 0.0),
            yaw: 0.0,
        }],
    });
    let manifest: manifest::Manifest = ron::from_str(
        r#"(
            map: "t", source_game: "bf2",
            terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: [], classes: [], geometries: [],
            regions: [], instances: [],
        )"#,
    )
    .expect("empty manifest");
    let mut spec = crate::mission::CampaignMissionDef::default();
    spec.script = Some(script);
    let mut rt = CampaignRuntime::new(
        spec,
        &layout,
        &manifest,
        &[],
        Vec::new(),
        &walk.sim,
    );
    rt.set_is_campaign(false);
    rt.set_team_rosters([vec!["hero".into()], vec!["trooper".into()]], [1, 20]);
    rt.start(&mut walk.sim);
    (walk, rt)
}

#[test]
fn unlimited_reinforcements_never_fires_ticket_count_change_defeat() {
    use manifest::script_ir::{BinOp, Expr, FnDef, ScriptDef, Stmt};
    // Mirrors stock Objective.lua + custom mode SetReinforcementCount(t, -1).
    let script = ScriptDef {
        main: 0,
        fns: vec![
            FnDef {
                name: "main".into(),
                params: 0,
                vararg: false,
                locals: 1,
                body: vec![
                    Stmt::SetGlobal("ScriptPostLoad".into(), Expr::Closure(1, vec![])),
                    Stmt::Return(vec![]),
                ],
            },
            FnDef {
                name: "ScriptPostLoad".into(),
                params: 0,
                vararg: false,
                locals: 1,
                body: vec![
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("OnTicketCountChange".into())),
                        vec![Expr::Closure(2, vec![])],
                    )),
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("SetReinforcementCount".into())),
                        vec![Expr::Num(1.0), Expr::Num(-1.0)],
                    )),
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("SetReinforcementCount".into())),
                        vec![Expr::Num(2.0), Expr::Num(-1.0)],
                    )),
                    Stmt::Return(vec![]),
                ],
            },
            FnDef {
                name: "on_ticket".into(),
                params: 2,
                vararg: false,
                locals: 2,
                body: vec![
                    Stmt::If(
                        Expr::Bin(
                            BinOp::Le,
                            Box::new(Expr::Local(1)),
                            Box::new(Expr::Num(0.0)),
                        ),
                        vec![Stmt::Do(Expr::Call(
                            Box::new(Expr::Global("MissionDefeat".into())),
                            vec![Expr::Local(0)],
                        ))],
                        vec![],
                    ),
                    Stmt::Return(vec![]),
                ],
            },
        ],
    };
    let (mut walk, mut rt) = synthetic_runtime(script);
    let tc = [vec!["hero".into()], vec!["trooper".into()]];
    rt.step(
        &mut walk.sim,
        walk.player,
        None,
        &tc,
        &BTreeMap::new(),
        &[],
        core::TICK_DT,
    );
    assert_eq!(
        walk.sim.outcome, None,
        "SetReinforcementCount(t, -1) must not trigger OnTicketCountChange defeat"
    );
}

#[test]
fn set_game_rules_campaign_disables_side_select_and_ambush_spawns_main_team() {
    use manifest::script_ir::{Expr, FnDef, ScriptDef, Stmt};
    let script = ScriptDef {
        main: 0,
        fns: vec![
            FnDef {
                name: "main".into(),
                params: 0,
                vararg: false,
                locals: 1,
                body: vec![
                    Stmt::SetGlobal("ScriptPostLoad".into(), Expr::Closure(1, vec![])),
                    Stmt::Return(vec![]),
                ],
            },
            FnDef {
                name: "ScriptPostLoad".into(),
                params: 0,
                vararg: false,
                locals: 3,
                body: vec![
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("ScriptCB_SetGameRules".into())),
                        vec![Expr::Str("campaign".into())],
                    )),
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("SetReinforcementCount".into())),
                        vec![Expr::Num(2.0), Expr::Num(0.0)],
                    )),
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("SetFlagGameplayType".into())),
                        vec![Expr::Str("1flag".into())],
                    )),
                    Stmt::SetLocal(
                        0,
                        Expr::Call(
                            Box::new(Expr::Global("GetTeamMember".into())),
                            vec![Expr::Num(2.0), Expr::Num(0.0)],
                        ),
                    ),
                    Stmt::Do(Expr::Call(
                        Box::new(Expr::Global("SpawnCharacter".into())),
                        vec![
                            Expr::Local(0),
                            Expr::Call(
                                Box::new(Expr::Global("GetPathPoint".into())),
                                vec![Expr::Str("spawn_path".into()), Expr::Num(0.0)],
                            ),
                        ],
                    )),
                    Stmt::Return(vec![]),
                ],
            },
        ],
    };
    let (walk, rt) = synthetic_runtime(script);
    assert_eq!(rt.can_switch_sides(), Some(false));
    assert!(rt.flag_ctf_hud());
    assert!(walk.sim.battle.campaign_rules);
    let team2_alive = walk
        .sim
        .units
        .iter()
        .filter(|u| u.alive && u.team == 1)
        .count();
    assert_eq!(
        team2_alive, 1,
        "GetTeamMember + SpawnCharacter must spawn team 2 bots even with no CPs"
    );
}
