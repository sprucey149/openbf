//! Mygeeto and Mustafar campaign regressions.

use super::*;

#[test]
fn myg1c_script_voice_lines_reach_the_commander_queue() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: campaign missions and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign(&missions_dir, &baked_dir, "myg1c_c");
    let lib = crate::sounds::SoundLib::load_for(&baked_dir.join(&def.map), &def)
        .expect("myg1 sound library");
    let mut v =
        crate::announce::Commander::for_battle(&def, &crate::layout::MapLayout::default(), &walk);
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
    assert!(
        (before.y - 9000.0 - after.y).abs() < 1.0,
        "{before} -> {after}"
    );
    assert!((before.x - after.x).abs() < 1e-3 && (before.z - after.z).abs() < 1e-3);
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
    let spawn =
        rt.w.paths
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
    assert!(
        flag.landed,
        "beacon should land on the conference room floor"
    );
    assert!(
        flag.current_pos.y < spawn.y - 1.0 && flag.current_pos.y > spawn.y - 5.0,
        "beacon should drop from bomb_spawn2 y={} onto the floor, got y={}",
        spawn.y,
        flag.current_pos.y
    );
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
    assert!(
        has_marker(&rt, s3),
        "living generator_01 marker must remain"
    );
    assert_eq!(rt.objective_markers(&walk.sim).len(), 2);
    assert_eq!(rt.map_markers(&walk.sim).len(), 2);

    call_script(
        &mut rt,
        &mut walk.sim,
        "SetProperty",
        vec![
            Value::str("backgen2"),
            Value::str("CurHealth"),
            Value::Num(0.0),
        ],
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
