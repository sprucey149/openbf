//! Hoth campaign cutscene and transition regressions.

use super::*;

#[test]
fn hoth_script_requests_both_mid_mission_cutscenes() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: Hoth campaign and baked assets required");
        return;
    };
    let (_, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "hot1g_c");
    assert_eq!(rt.take_movies(), ["hotmon01"]);
    let falcon_names = [
        "falcon", "falconl1", "falconl2", "falconl3", "falconl4", "falconl5",
    ];
    let props = falcon_names.map(|name| {
        let Some(Ud::Entity(i)) = rt.w.resolve_name(&walk.sim, name) else {
            panic!("{name} must resolve to a placed prop");
        };
        assert!(!rt.hidden_instances().contains(&i));
        i
    });
    let collision = |sim: &core::sim::Sim| {
        sim.collision
            .bodies()
            .iter()
            .filter(|b| b.instance.is_some_and(|i| props.contains(&i)))
            .map(|b| b.layers)
            .collect::<Vec<_>>()
    };
    let initial_layers = collision(&walk.sim);
    assert!(initial_layers
        .iter()
        .any(|&layers| layers != core::collision::Layers::NONE));

    for name in ["Objective2", "Objective9"] {
        let objective = rt.interp.global(name);
        let mut host = Ctx {
            w: &mut rt.w,
            sim: &mut walk.sim,
        };
        let complete = rt
            .interp
            .index(&mut host, &objective, &Value::str("OnComplete"))
            .unwrap();
        rt.interp
            .call_value(&mut host, &complete, vec![objective])
            .unwrap();
        assert!(rt.has_movies());
    }
    let clips = rt.take_movies();
    assert_eq!(clips, ["hot1cam2", "hot1cam1"]);
    assert!(!rt.has_movies());
    assert!(props.iter().all(|i| rt.hidden_instances().contains(i)));
    assert!(props
        .iter()
        .all(|i| rt.excluded_camera_instances().any(|id| id == *i)));
    assert!(collision(&walk.sim)
        .iter()
        .all(|&layers| layers == core::collision::Layers::NONE));
    let gun = walk
        .sim
        .building_turrets
        .iter()
        .find(|turret| rt.w.instances[turret.instance as usize].0 == "gun1")
        .expect("Hoth roof turret");
    assert!(gun.down.is_some());
    let (instance, original_y) = (gun.instance, gun.spec.pos.y);
    for _ in 0..4 {
        rt.w.anims.step(&mut walk.sim, core::TICK_DT);
    }
    let pose = rt.instance_poses()[&instance];
    assert!((pose.w_axis.y - (original_y - 100.0)).abs() < 0.001);
    assert!(walk
        .sim
        .collision
        .instance_bodies(instance)
        .iter()
        .all(|(index, pose)| {
            walk.sim.collision.bodies()[*index].layers == core::collision::Layers::NONE
                || pose.w_axis.y < original_y - 90.0
        }));
    for name in falcon_names {
        rt.w.set_property(&mut walk.sim, &Value::str(name), "IsCollidable", "1");
        rt.w.set_property(&mut walk.sim, &Value::str(name), "IsVisible", "1");
    }
    assert!(props.iter().all(|i| !rt.hidden_instances().contains(i)));
    assert_eq!(collision(&walk.sim), initial_layers);
    let movie = crate::shell_movie::ShellMovie::load(&baked_dir).expect("baked movie index");
    for clip in clips {
        let (rgba, w, h) = movie.frame_now(&clip, 0, 0).expect("first cutscene frame");
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        let path = movie.audio_path(&clip).expect("cutscene audio");
        audio::decode(path).expect("decode cutscene audio");
    }
}

#[test]
fn hoth_echo_base_transition_dismounts_imperial_ai() {
    let Some((missions_dir, baked_dir)) = baked() else {
        eprintln!("SKIP: Hoth campaign and baked assets required");
        return;
    };
    let (def, mut walk, mut rt) = setup_baked_campaign_full(&missions_dir, &baked_dir, "hot1g_c");
    begin_objectives(&mut walk, &mut rt, &def);

    let (walker, pos) = walk
        .sim
        .alive_vehicles()
        .find(|(_, v)| v.kind == core::vehicle::VehicleKind::Walker && v.team == Some(0))
        .map(|(handle, v)| (handle, v.pos))
        .expect("Imperial command walker");
    let mut spec = core::unit::UnitSpec::player(0, pos);
    spec.controller = core::unit::Controller::Ai;
    let pilot = walk.sim.spawn(&spec).expect("Imperial pilot");
    assert!(walk.sim.enter_vehicle(pilot, walker, 0));

    let objective = rt.interp.global("Objective4");
    let mut host = Ctx {
        w: &mut rt.w,
        sim: &mut walk.sim,
    };
    let complete = rt
        .interp
        .index(&mut host, &objective, &Value::str("OnComplete"))
        .expect("CP5 completion callback");
    rt.interp
        .call_value(&mut host, &complete, vec![objective])
        .expect("run CP5 completion callback");

    assert!(walk.sim.seat_of(pilot).is_none());
    assert!(!walk.sim.enter_vehicle(pilot, walker, 0));
}
