use core::heightfield::Heightfield;
use core::sim::{buttons, PadState, Sim};
use core::unit::Controller;
use game::roster::{class_slot_kind, load_classdb, ClassRoster, SlotKind};
use glam::Vec3;

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..Default::default()
    }
}

#[test]
fn map_local_and_all_side_databases_preserve_the_equipment_census() {
    use std::collections::BTreeSet;
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_all.ron").exists() || !root.join("classes/bf1_all.ron").exists() {
        eprintln!("SKIP: requires both local baked discs");
        return;
    }
    let files: Vec<_> = std::fs::read_dir(root.join("classes"))
        .unwrap()
        .map(|f| f.unwrap().path())
        .collect();
    let census = |db: &game::classdb::ClassDb| {
        let mut rows = BTreeSet::new();
        for name in db
            .names()
            .filter(|n| db.engine_base(n) == Some("soldier") && !n.contains("_hero_"))
        {
            for slot in db.weapon_slots(name) {
                let base = db.engine_base(&slot.weapon);
                let flyer = base == Some("remote")
                    && db
                        .prop(&slot.weapon, "OrdnanceName")
                        .and_then(|n| db.engine_base(n.trim()))
                        == Some("flyer");
                if flyer || base == Some("invisibility") {
                    rows.insert((name.to_owned(), slot.weapon, flyer));
                }
            }
        }
        rows
    };
    let maps = game::assets::discover_maps(&root);
    if maps.len() < 32 {
        eprintln!("SKIP: requires the local two-disc map bake");
        return;
    }
    for source in ["bf1", "bf2"] {
        let side_files: Vec<_> = files
            .iter()
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(source))
            .collect();
        let mut baseline = game::classdb::ClassDb::default();
        for file in &side_files {
            baseline.load(file).unwrap();
        }
        let expected = census(&baseline);
        if source == "bf1" {
            assert!(expected.is_empty());
        } else {
            assert_eq!(expected.iter().filter(|row| !row.2).count(), 4);
            assert_eq!(expected.iter().filter(|row| row.2).count(), 14);
        }
        for (key, _, dir) in maps.iter().filter(|m| m.1 == source) {
            let mut db = game::classdb::ClassDb::default();
            db.load_map(dir).unwrap();
            for file in &side_files {
                db.load(file).unwrap();
            }
            assert_eq!(census(&db), expected, "{key}");
        }
    }
}

/// The guided-flight tests read one launcher out of the BF2 class bake. A class
/// bake without that launcher still fails in `guided_walk`: that is a stale bake.
fn bf2_classes_baked() -> bool {
    let baked = game::assets::baked_root()
        .join("classes/bf2_all.ron")
        .exists();
    if !baked {
        eprintln!("SKIP: no BF2 class bake");
    }
    baked
}

fn guided_walk() -> game::walk::WalkSim {
    let root = game::assets::baked_root();
    let db = load_classdb(&root.join("classes"), "bf2", &["all"]).unwrap();
    let weapon = game::roster::weapon_from_db(&db, "all_weap_award_rocket_launcher").unwrap();
    let mut walk = game::walk::WalkSim::new(3, Vec3::ZERO, None);
    let class = walk.sim.add_weapon_class(weapon);
    let state = core::weapon::WeaponState::new(&walk.sim.weapon_classes[class], true, 0);
    let owner = walk.sim.unit_mut(walk.player).unwrap();
    owner.weapon_class = class;
    owner.weapon = state;
    let flyer = core::guided::GuidedRocket::flying(
        &walk.sim.weapon_classes[class]
            .guided
            .as_ref()
            .unwrap()
            .class,
    );
    walk.sim.guided.push(core::guided::GuidedRocket {
        id: 7,
        owner: walk.player,
        team: 0,
        weapon_class: class,
        seat: None,
        pos: Vec3::new(0.0, 10.0, -20.0),
        vel: Vec3::new(0.0, 0.0, -30.0),
        rot: core::guided::GuidedRocket::body(Vec3::NEG_Z),
        flyer,
        health: 1.0,
        age: 1.0,
        prev_buttons: 0,
        pending_end: None,
        spent: None,
        blind: 0.0,
    });
    walk.sim.next_guided_id = 7;
    walk
}

#[test]
fn guided_turning_uses_display_interpolation_with_mouse_and_controller_input() {
    if !bf2_classes_baked() {
        return;
    }
    for hz in [60, 75, 144] {
        for mouse in [false, true] {
            let mut walk = guided_walk();
            walk.blend.on = true;
            let mut reference = guided_walk();
            let input = if mouse {
                PadState {
                    mx: PadState::quantize_look(f64::from(core::sim::MOUSE_STEER)),
                    buttons: buttons::MOUSE_AIM,
                    ..Default::default()
                }
            } else {
                PadState {
                    rx: i16::MAX,
                    ..Default::default()
                }
            };
            let mut last_yaw = 0.0;
            let mut tick_yaws = [reference.camera().yaw; 2];
            let mut saw_between_ticks = false;
            for frame in 0..hz {
                walk.advance(1.0 / hz as f32, &[input]);
                let previous_tick = reference.sim.tick;
                reference.advance(1.0 / hz as f32, &[input]);
                let tick = walk.sim.tick;
                let hash = walk.sim.hash();
                if reference.sim.tick > previous_tick {
                    tick_yaws = [tick_yaws[1], reference.camera().yaw];
                }
                if hz != 60 && tick > 1 {
                    walk.blend_in();
                    let drawn = walk.camera().yaw;
                    saw_between_ticks |= drawn > tick_yaws[1] + 1e-5 && drawn < tick_yaws[0] - 1e-5;
                    walk.blend_out();
                }
                let pending = PadState {
                    buttons: if mouse { buttons::MOUSE_AIM } else { 0 },
                    mx: if mouse && frame % 2 == 0 { 500 } else { 0 },
                    ..Default::default()
                };
                game::render_blend::blend_in_looking(&mut walk, &pending);
                let yaw = walk.camera().yaw;
                if tick > 1 {
                    assert!(
                        yaw <= last_yaw + 0.00001,
                        "{hz} Hz mouse={mouse} frame={frame}: yaw={yaw}, last={last_yaw}"
                    );
                }
                last_yaw = yaw;
                walk.blend_out();
                assert_eq!(walk.sim.hash(), hash);
                assert_eq!(walk.sim.hash(), reference.sim.hash());
            }
            assert!(last_yaw < -0.1, "{hz} Hz mouse={mouse}: turned {last_yaw}");
            assert!(
                hz == 60 || saw_between_ticks,
                "{hz} Hz mouse={mouse}: no interpolated camera angle"
            );
        }
    }
}

#[test]
fn guided_camera_and_interpolation_restore_the_sim_exactly() {
    if !bf2_classes_baked() {
        return;
    }
    let mut chased = guided_walk();
    for _ in 0..60 {
        chased.step_tick(&[PadState::default()]);
    }
    let r = chased.sim.guided[0].clone();
    let eye = chased.camera().position;
    let behind = (eye - r.pos).dot(-r.forward());
    assert!((2.0..6.0).contains(&behind), "behind {behind}");
    let mut walk = guided_walk();
    let initial = walk.camera();
    assert!(initial.yaw.abs() < 1e-5 && initial.roll.abs() < 1e-5);
    let level = walk.sim.guided[0].rot;
    let roll = 0.4;
    walk.sim.guided[0].rot = level * glam::Quat::from_rotation_z(roll);
    let rolled = walk.camera();
    assert!(
        (rolled.roll.abs() - roll).abs() < 0.01,
        "roll {}",
        rolled.roll
    );
    let want_right = walk.sim.guided[0].rot * Vec3::NEG_X;
    assert!(
        rolled.right().distance(want_right) < 1e-5,
        "right {} / {want_right}",
        rolled.right()
    );
    assert!(
        walk.reticle_y() > 0.35 && walk.reticle_y() < 0.5,
        "reticle at {}",
        walk.reticle_y()
    );
    walk.sim.guided[0].rot = level;
    let mut blend = game::render_blend::Blend::default();
    blend.on = true;
    blend.before_tick(&walk);
    walk.sim.guided[0].pos.x = 2.0;
    let hash = walk.sim.hash();
    assert!(blend.apply(&mut walk, 0.5));
    assert_eq!(walk.sim.guided[0].pos.x, 1.0);
    assert!(walk
        .sim
        .look_preview(
            walk.player,
            &PadState {
                mx: 2000,
                ..Default::default()
            },
        )
        .is_none());
    assert!(walk.camera().yaw.abs() < 1e-5);
    blend.restore(&mut walk);
    assert_eq!(walk.sim.hash(), hash);
    assert_eq!(walk.sim.guided[0].pos.x, 2.0);
}

#[test]
fn every_ordinary_disc_guided_launcher_is_gated_and_flies() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires the local baked BF2 disc");
        return;
    }
    let dir = root.join("classes");
    let db = load_classdb(&dir, "bf2", &["all", "imp", "cis", "rep"]).unwrap();
    let mut names: Vec<_> = db
        .names()
        .filter(|n| db.engine_base(n) == Some("soldier") && !n.contains("_hero_"))
        .collect();
    names.sort();
    let mut carriers = Vec::new();
    let mut weapons = std::collections::BTreeSet::new();
    for name in names {
        let Some(slot) = db
            .weapon_slots(name)
            .into_iter()
            .find(|s| class_slot_kind(&db, name, &s.weapon, s.channel) == SlotKind::Guided)
        else {
            continue;
        };
        carriers.push(name);
        weapons.insert(slot.weapon.clone());
        let roster = ClassRoster::build(&db, name, 1, None).unwrap();
        let weapon = &roster
            .held
            .iter()
            .find(|h| h.slot == slot.slot)
            .unwrap()
            .weapon;
        let guide = weapon.guided.as_ref().unwrap();
        let c = &guide.class;
        let f = &c.flight;
        assert!(guide.requires_award);
        assert_eq!(guide.spawn_offset, Vec3::new(-0.5, 1.0, 5.0));
        assert_eq!(
            (
                f.acceleration,
                [f.min_speed, f.mid_speed, f.max_speed],
                f.boost_speed,
                f.boost_acceleration
            ),
            (80.0, [30.0; 3], 80.0, 50.0)
        );
        assert_eq!(
            (f.pitch_rate, f.turn_rate, f.roll_rate, f.level_filter),
            (1.2, 1.2, 3.15, 1.5)
        );
        assert_eq!(
            (
                f.energy_max,
                f.energy_auto_restore,
                f.energy_boost_drain,
                c.health
            ),
            (200.0, -5.0, 20.0, 1.0)
        );
        assert_eq!(
            (c.view.track_center, c.view.track_offset),
            (Some([0.0, 1.0, -3.0]), Some([0.0, 2.0, 6.0]))
        );
        assert_eq!(
            (c.hold_time, c.hold_speed, c.hold_distance, c.critical_speed),
            (1.0, 60.0, 20.0, 0.8)
        );
        assert_eq!(c.explosion.as_ref().unwrap().damage, 500.0);
        assert_eq!(c.noncritical.as_ref().unwrap().damage, 50.0);
        assert_eq!(
            weapon.ordnance.geometry.as_deref(),
            Some("com_weap_missile")
        );
        // The rocket's model comes from a bake made with card GUIDED's
        // `scripts/rebake.sh side`; an older bake skips only this check.
        if !root.join("side_bf2/weapons/com_weap_missile.glb").exists() {
            eprintln!("SKIP (model check): no side_bf2 com_weap_missile bake");
        }
        assert!(weapon.remote.is_none());
        assert!(weapon.thrown.is_none());
        let mut sim = Sim::empty(42);
        sim.heightfield = Some(Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        });
        let class = roster.register(&mut sim, &mut Vec::new());
        let owner = sim
            .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
            .unwrap();
        for _ in 0..5 {
            sim.step(&[pad(buttons::SWITCH_WEAPON)]);
            sim.step(&[pad(0)]);
            assert!(sim.weapon_classes[sim.unit(owner).unwrap().weapon_class]
                .guided
                .is_none());
        }
        sim.cheats.all_awards = true;
        for _ in 0..5 {
            if sim.weapon_classes[sim.unit(owner).unwrap().weapon_class]
                .guided
                .is_some()
            {
                break;
            }
            sim.step(&[pad(buttons::SWITCH_WEAPON)]);
            sim.step(&[pad(0)]);
        }
        assert!(
            sim.weapon_classes[sim.unit(owner).unwrap().weapon_class]
                .guided
                .is_some(),
            "{name}"
        );
        sim.unit_mut(owner).unwrap().soldier.pitch = 0.0;
        sim.step(&[pad(buttons::FIRE)]);
        for _ in 0..30 {
            if sim.controlled_guided(owner).is_some() {
                break;
            }
            sim.step(&[pad(0)]);
        }
        assert!(sim.controlled_guided(owner).is_some(), "{name}");
        assert!(sim.droids.is_empty());
        let before = sim.guided[0].pos;
        for _ in 0..15 {
            sim.step(&[pad(0)]);
        }
        assert!(sim.guided[0].pos.z < before.z - 14.0, "{name}");
        sim.step(&[pad(buttons::ACTION)]);
        assert!(sim.controlled_guided(owner).is_none());
        assert!(sim.guided.is_empty());
    }
    assert_eq!(
        carriers,
        [
            "all_inf_default_rocketeer",
            "all_inf_rocketeer",
            "all_inf_rocketeer_fleet",
            "all_inf_rocketeer_jungle",
            "all_inf_rocketeer_snow",
            "cis_inf_default_rocketeer",
            "cis_inf_rocketeer",
            "imp_inf_default_rocketeer",
            "imp_inf_rocketeer",
            "imp_inf_rocketeer_snow",
            "rep_inf_default_rocketeer",
            "rep_inf_ep2_rocketeer",
            "rep_inf_ep2_rocketeer_chaingun",
            "rep_inf_ep3_rocketeer",
        ]
    );
    assert_eq!(
        weapons.into_iter().collect::<Vec<_>>(),
        [
            "all_weap_award_rocket_launcher",
            "cis_weap_award_rocket_launcher",
            "imp_weap_award_rocket_launcher",
            "rep_weap_award_rocket_launcher"
        ]
    );
    let bf1 = load_classdb(&dir, "bf1", &["all", "imp", "rep", "cis"]).unwrap();
    assert!(!bf1.names().any(|n| bf1.engine_base(n) == Some("remote")
        && bf1.prop(n, "OrdnanceName").and_then(|o| bf1.engine_base(o)) == Some("flyer")));
    assert_eq!(
        class_slot_kind(
            &db,
            "all_hero_chewbacca",
            "all_weap_hero_rocket_launcher",
            1
        ),
        SlotKind::Guided
    );
    let hero = game::roster::weapon_from_db(&db, "all_weap_hero_rocket_launcher").unwrap();
    assert!(!hero.guided.as_ref().unwrap().requires_award);
}

#[test]
fn tie_bomber_rocket_exhaust_runs_from_a_quarter_at_launch_to_three_quarters_at_boost() {
    if !bf2_classes_baked() {
        return;
    }
    let root = game::assets::baked_root();
    let db = load_classdb(&root.join("classes"), "bf2", &["imp"]).unwrap();
    let weapon = game::roster::weapon_from_db(&db, "imp_weap_fly_tiebomber_guided_rocket").unwrap();
    let c = &weapon.guided.as_ref().unwrap().class;
    assert_eq!(c.trail.as_deref(), Some("com_sfx_exhaust_xwing"));
    assert_eq!(c.trail_scale, [0.25, 0.75, 0.0]);
    assert_eq!(c.trail_size(c.flight.min_speed), 0.25);
    assert!((c.trail_size(c.top_speed()) - 0.75).abs() < 1e-6);
}
