//! Felucia, Devaron and Tatooine campaign regressions.

use super::*;

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
        vec![
            Value::str("defend_ambush"),
            Value::Num(3.0),
            Value::Num(5.0),
        ],
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
        vec![
            Value::str("hangar_ambush_spawn"),
            Value::Num(6.0),
            Value::Num(5.0),
        ],
    );
    let alive = walk
        .sim
        .units
        .iter()
        .filter(|u| u.team == 4 && u.alive)
        .count();
    assert_eq!(alive, 6, "hangar_ambush_spawn fields all six of team 5");
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
    let turbine_region =
        rt.w.regions
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
