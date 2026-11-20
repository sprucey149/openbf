//! Kamino, Utapau and Coruscant campaign regressions.

use super::*;

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
    assert_eq!(
        queued_def, 12,
        "all 12 DEF slots should be queued on waves[1]"
    );
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
    assert!(
        owned_by_player,
        "cam_cp4 must be capturable from the platform"
    );
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
            &[Value::Num(4.0), Value::Num(2.0), Value::Num(1.0)],
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
                && rt
                    .w
                    .team_classes
                    .get(usize::from(u.team))
                    .and_then(|cs| cs.get(usize::from(u.spec.class)))
                    .is_some_and(|n| n.eq_ignore_ascii_case("cis_hero_grievous"))
        })
        .map(|(i, _)| i);
    assert!(
        grievous.is_some(),
        "Grievous must be fielded for Objective 3b"
    );
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
    let appended_idx =
        crate::heroes::append(&mut rosters[0], &baked_dir.join("classes"), Some(&def), 0)
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
    assert_eq!(
        heroes.rules.unlock,
        core::sim::heroes::HeroUnlock::Points(i32::MAX)
    );
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
        if rt.objectives().iter().any(|o| {
            o.text
                .eq_ignore_ascii_case("level.cor1.objectives.Campaign.1")
                && o.state == 2
        }) {
            break;
        }
    }
    assert!(
        rt.objectives().iter().any(|o| o
            .text
            .eq_ignore_ascii_case("level.cor1.objectives.Campaign.1")
            && o.state == 2),
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
    let door =
        rt.w.instances
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
    let is_door_prop = blockers
        .is_prop(door)
        .expect("leftlibrarydoor must be a blocker");
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
        if rt.objectives().iter().any(|o| {
            o.text
                .eq_ignore_ascii_case("level.cor1.objectives.campaign.2.a")
                && o.state == 2
        }) {
            break;
        }
    }
    assert!(
        rt.objectives().iter().any(|o| o
            .text
            .eq_ignore_ascii_case("level.cor1.objectives.campaign.2.a")
            && o.state == 2),
        "Objective2a should be active, got {:?}",
        rt.objectives()
    );
    for _ in 0..(5 * core::TICK_HZ) {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let poses = rt.instance_poses();
    assert!(
        poses.contains_key(&door),
        "DoorOpen01 must pose leftlibrarydoor"
    );
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
    let libcase_indices: Vec<usize> =
        rt.w.station_names
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
        if rt.objectives().iter().any(|o| {
            o.text
                .eq_ignore_ascii_case("level.cor1.objectives.Campaign.2.c")
                && o.state == 2
        }) {
            break;
        }
    }
    assert!(
        rt.objectives().iter().any(|o| o
            .text
            .eq_ignore_ascii_case("level.cor1.objectives.Campaign.2.c")
            && o.state == 2),
        "Objective2c should be active, got {:?}",
        rt.objectives()
    );
    assert_eq!(
        walk.sim.stations[libcase_indices[0]].ai_damage_threshold,
        0.0
    );
    assert!(
        walk.sim.units.iter().any(|u| u.alive && u.team == 2),
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
            .any(|(&si, &before)| {
                walk.sim.stations[si].health.cur < before || walk.sim.stations[si].destroyed
            })
        {
            any_damaged = true;
            break;
        }
    }
    assert!(
        any_damaged,
        "Objective 2c AMB/GAR bots must attack and damage LibCase stations"
    );

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
        if rt.objectives().iter().any(|o| {
            o.text
                .eq_ignore_ascii_case("level.cor1.objectives.campaign.4")
                && o.state == 2
        }) {
            break;
        }
    }
    assert!(
        rt.objectives().iter().any(|o| o
            .text
            .eq_ignore_ascii_case("level.cor1.objectives.campaign.4")
            && o.state == 2),
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
        if rt.objectives().iter().any(|o| {
            o.text
                .eq_ignore_ascii_case("level.cor1.objectives.campaign.5")
                && o.state == 2
        }) {
            break;
        }
    }
    if walk.sim.hero_offer(0).map(|(c, _)| c) == Some(hero_class_idx) {
        saw_anakin_offer = true;
    }
    assert!(
        rt.objectives().iter().any(|o| o
            .text
            .eq_ignore_ascii_case("level.cor1.objectives.campaign.5")
            && o.state == 2),
        "Objective5 should be active, got {:?}",
        rt.objectives()
    );
    assert!(
        saw_anakin_offer,
        "Completing Objective 4 (UnlockHeroForTeam) must offer Anakin (F1/F2 prompt) to player 0, heroes state: {:?}",
        walk.sim.heroes
    );
    let holo2_idx =
        rt.w.flags
            .iter()
            .position(|f| f.name == "holocron2")
            .expect("holocron2 must be registered as a flag");
    let flag = &walk.sim.ctf.as_ref().unwrap().flags[rt.w.flags[holo2_idx].ctf];
    assert!(
        flag.allow_ai_pickup,
        "holocron2 must allow AI pickup for flagrunner"
    );
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
    assert!(
        runner_carried,
        "flagrunner (RUN team) must spawn and pick up holocron2 in Objective 5"
    );
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
            (
                pos,
                walk.sim
                    .units
                    .iter()
                    .position(|o| o.alive && o.team == 1 && o.spec.class == 0),
            )
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
    assert_eq!(
        rt.errors().count(),
        0,
        "no Lua script errors during cor1c_c run"
    );
}
