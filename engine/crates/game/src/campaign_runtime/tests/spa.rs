//! Space-battle (spa1/2/3/4) campaign regressions.

use super::*;

#[test]
fn spa2c_comm_array_is_a_destructible_station() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: spa2c_c mission and baked assets required");
        return;
    };
    let (_def, walk, rt) = setup_baked_campaign(&missions_dir, &baked_dir, "spa2c_c");
    for name in ["comms_cis", "cisbridge01", "ciseng01", "ciseng02"] {
        let Some(Ud::Station(si)) = rt.w.resolve_name(&walk.sim, name) else {
            panic!(
                "{name} is not a station: {:?}",
                rt.w.resolve_name(&walk.sim, name)
            );
        };
        let st = &walk.sim.stations[usize::from(si)];
        assert!(
            st.health.max > 0.0 && st.health.max.is_finite() && !st.destroyed,
            "{name}"
        );
    }
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
        hud.shield
            .as_ref()
            .is_some_and(|(label, f)| label == "Shield" && *f > 0.99),
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
        walk.sim.stations[usize::from(comms)].health.shield,
        0.0,
        "shield must stay 0.0 once drained on a Sim::step tick"
    );
    assert_eq!(
        walk.sim.stations[usize::from(comms)].add_shield,
        0.0,
        "LinkedShields must see cur == 0.0 and zero AddShield"
    );
    step(&mut walk, &mut rt, idle);
    assert_eq!(
        walk.sim.reticle_pick[0],
        Some((core::sim::AimTarget::Station(comms), -1))
    );
    assert!(walk.sim.stations[usize::from(comms)].health.shield <= 0.0);
    hud = feed
        .target(&walk.sim, 0)
        .expect("HUD remains for unshielded station");
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

    let cp1 =
        rt.w.region_id("CP1Control")
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
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
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
        assert_eq!(
            walk.sim.battle.unlimited_teams, want_unlimited_teams,
            "{script}"
        );
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
            if let core::sim::Event::FlyerImpact {
                pos, lethal: true, ..
            } = ev
            {
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
    let call_obj =
        |rt: &mut CampaignRuntime, walk: &mut WalkSim, obj: &str, method: &str, arg: f32| {
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
    call_obj(
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
        assert!(
            v.add_shield > 0.0,
            "LinkedShields sets AddShield on turrets"
        );
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
            let hud = feed
                .target(&walk.sim, 0)
                .expect("HUD target on ship turret");
            assert!(
                hud.shield
                    .as_ref()
                    .is_some_and(|(label, _)| label == "Shield"),
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
    assert_eq!(
        tv.health.cur, tv.health.max,
        "hull untouched while shield holds"
    );

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
        walk.sim.stations[usize::from(cruiser1)].health.shield,
        0.0,
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
        walk.sim.stations[usize::from(comms)].health.max,
        20_000.0,
        "Objective5.OnStart must activate after Objective4 completes"
    );
    assert_eq!(
        walk.sim.stations[usize::from(comms)].health.cur,
        20_000.0,
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
        lib.voice_take("SPA2_obj_62")
            .is_some_and(|(_, _, secs)| secs > 1.0),
        "SPA2_obj_62 must decode with non-zero length"
    );
    let mut v =
        crate::announce::Commander::for_battle(&def, &crate::layout::MapLayout::default(), &walk);
    v.route_streams(rt.stream_slots());
    begin_objectives(&mut walk, &mut rt, &def);
    rt.close_popup();
    let _ = rt.take_voice_overs();
    let _ = rt.take_sounds();
    let call_obj =
        |rt: &mut CampaignRuntime, walk: &mut WalkSim, obj: &str, method: &str, arg: f32| {
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
    let _ = v.frame(&walk.sim, Some(walk.player), core::TICK_DT, |line| {
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
    call_obj(
        &mut rt,
        &mut walk,
        "objectiveSequence",
        "ActivateObjectiveSet",
        6.0,
    );
    for _ in 0..30 {
        tick(&mut walk, &mut rt, &def);
        let mut script_lines = rt.take_voice_overs();
        script_lines.extend(rt.take_sounds());
        v.ask_script_sounds(script_lines);
        assert!(v.gate_script_sounds(&lib).is_empty());
        let _ = v.frame(&walk.sim, Some(walk.player), core::TICK_DT, |line| {
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
    assert!(
        v.disable > 0.0,
        "the LAAT must be disabled until the player arrives"
    );
    assert!(
        walk.sim
            .units
            .iter()
            .all(|u| u.mount.map(|m| m.vehicle) != Some(pad.vehicles[0].handle)),
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
    let si =
        rt.w.station_names
            .iter()
            .position(|n| n.eq_ignore_ascii_case("life_int_cis"))
            .unwrap();
    let origin = walk.sim.stations[si].pos;
    let centre = walk
        .sim
        .collision
        .instance_centre(walk.sim.stations[si].instance.unwrap())
        .unwrap();
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
    let Some((missions_dir, baked_dir)) = baked() else {
        return;
    };
    let (def, walk, rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "spa3c_c");
    assert!(
        rt.w.flags
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case("plans")),
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
        rt2.w
            .flags
            .iter()
            .all(|f| !f.name.eq_ignore_ascii_case("holocron")),
        "the tutorial holocron is not a placed instance"
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
    assert!(
        fp.command_flyer,
        "cis_fly_droidgunship must be a commandflyer"
    );
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
                .filter(|(_, u)| {
                    u.alive && u.team == 1 && (u.soldier.pos - gunship_pos).length() < 40.0
                })
                .map(|(i, u)| core::unit::UnitHandle {
                    index: i as u32,
                    gen: u.gen,
                })
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
        walk.sim
            .command_walkers(1)
            .all(|(h, _)| h != gunship_handle),
        "a flying CommandFlyer must not be an eligible spawn post"
    );
    // Landing re-enables it.
    walk.sim.vehicle_mut(gunship_handle).unwrap().flyer.phase =
        core::vehicle::flyer::FlightPhase::Landed;
    assert!(
        walk.sim
            .command_walkers(1)
            .any(|(h, _)| h == gunship_handle),
        "a landed CommandFlyer must be an eligible spawn post again"
    );
    // Destroying it removes it as a spawn post.
    walk.sim.vehicle_mut(gunship_handle).unwrap().health.cur = 0.0;
    assert!(
        walk.sim
            .command_walkers(1)
            .all(|(h, _)| h != gunship_handle),
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
        .find(|(_, v)| {
            v.class == bomber_class && glam::Vec2::new(v.pos.x, v.pos.z).distance(pad) < 1.0
        })
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
            !walk
                .sim
                .units
                .iter()
                .any(|u| u.alive && u.mount.is_some_and(|m| m.vehicle == bomber)),
            "nobody boards the barred bomber"
        );
    }
    let v = walk.sim.vehicle(bomber).expect("the bomber is still there");
    assert!(
        glam::Vec2::new(v.pos.x, v.pos.z).distance(pad) < 1.0,
        "still on its pad: {:?}",
        v.pos
    );
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
                core::sim::Event::StationDestroyed { station, pos, .. } if station == si => {
                    Some(pos)
                }
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
        let centroid = pieces.iter().map(|(_, p)| *p).sum::<glam::Vec3>() / pieces.len() as f32;
        assert!(
            centroid.distance(live_pos) < 150.0,
            "{name}: chunks spawned at {centroid} instead of live pos {live_pos} (old pos was {})",
            st.pos
        );
    }
}
