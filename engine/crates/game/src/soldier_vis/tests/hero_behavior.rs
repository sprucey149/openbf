use super::*;
use core::sim::{buttons, PadState};

#[test]
fn bespin_platform_underside_contains_heroes_infantry_and_jets() {
    let root = crate::assets::baked_root();
    if !root.join("bf1_bes2/manifest.ron").is_file() {
        eprintln!("SKIP: actual BF1 Bespin assets required");
        return;
    }
    let (scene, _) = crate::assets::load_battle_scene(&root.join("bf1_bes2"), "con").unwrap();
    let mut walk = WalkSim::new(198, Vec3::new(-241.0, -33.1, 186.6), scene.heightfield());
    scene.install_collision(&mut walk.sim);
    let db = crate::roster::load_classdb(&root.join("classes"), "bf1", &["all"]).unwrap();
    let base = crate::roster::ClassRoster::build(&db, "all_inf_soldier", 0, None)
        .unwrap()
        .register(&mut walk.sim, &mut vec![]);
    walk.sim.classes = [vec![base.clone()], vec![base]];
    let mut picker = crate::class_picker::ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    for class in [
        "rep_hero_anakin",
        "rep_inf_clone_trooper",
        "rep_inf_jet_trooper",
    ] {
        let choice = picker
            .choices
            .iter()
            .position(|c| c.class.class == class)
            .unwrap();
        assert!(picker.select(&mut walk, choice));
        let player = walk.player;
        for x in [-242.0, -241.0, -240.0] {
            walk.sim.unit_mut(player).unwrap().soldier =
                core::soldier::Soldier::new(Vec3::new(x, -33.1, 186.6));
            let cap = walk.sim.unit(player).unwrap().capsule();
            let head = cap.base + Vec3::Y * cap.height;
            let (distance, _) = walk
                .sim
                .collision
                .ray(head, Vec3::Y, 20.0, core::collision::Layers::SOLDIER)
                .unwrap();
            let ceiling = head.y + distance;
            walk.sim.cheats.infinite_stamina = true;
            for tick in 0..300 {
                walk.sim.step(&[PadState {
                    buttons: if tick % 6 == 0 { buttons::JUMP } else { 0 },
                    ..Default::default()
                }]);
                let u = walk.sim.unit(player).unwrap();
                assert!(
                    u.soldier.pos.y + u.capsule().height <= ceiling + 0.002,
                    "{class} x={x} tick={tick} feet={:?}",
                    u.soldier.pos
                );
            }
        }
    }
}

#[test]
fn actual_heroes_compose_moving_attacks_blocks_and_directional_jumps_at_display_rates() {
    let root = crate::assets::baked_root();
    if !root.join("bf2_kam1/manifest.ron").is_file() {
        eprintln!("SKIP: actual BF2 hero assets required");
        return;
    }
    let manifest =
        ron::from_str(&pak::read_to_string(root.join("bf2_kam1/manifest.ron")).unwrap()).unwrap();
    let scene = crate::assets::Scene {
        dir: root.join("bf2_kam1"),
        manifest,
        meshes: Default::default(),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: Default::default(),
        collision: Default::default(),
        cutters: vec![],
    };
    let mut picker = crate::class_picker::ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    for hero in [
        "rep_hero_anakin",
        "rep_hero_obiwan",
        "cis_hero_darthmaul",
        "rep_hero_aalya",
        "cis_hero_countdooku",
    ] {
        let mut hashes = vec![];
        for fps in [60u32, 75, 120] {
            let mut walk = WalkSim::new(198, Vec3::ZERO, None);
            walk.blend.on = true;
            walk.sim.heightfield = Some(core::heightfield::Heightfield {
                size: 2,
                unit: 4000.0,
                offset_x: 2000.0,
                offset_z: 2000.0,
                heights: vec![0.0; 4],
            });
            let db = crate::roster::load_classdb(&root.join("classes"), "bf1", &["all"]).unwrap();
            let base = crate::roster::ClassRoster::build(&db, "all_inf_soldier", 0, None)
                .unwrap()
                .register(&mut walk.sim, &mut vec![]);
            walk.sim.classes = [vec![base.clone()], vec![base]];
            let mut local = crate::class_picker::ClassPicker::new(&root.join("classes"), &scene);
            local.choices = picker
                .choices
                .iter()
                .map(|c| crate::class_picker::Choice {
                    tab: c.tab,
                    game: c.game,
                    label: c.label.clone(),
                    class: c.class.clone(),
                })
                .collect();
            let choice = local
                .choices
                .iter()
                .position(|c| c.class.class == hero)
                .unwrap();
            assert!(local.select(&mut walk, choice));
            let mut vis = SoldierVisuals::load_cpu(&root, "bf2", &walk.teams);
            let mut fx = crate::fx::Fx::default();
            vis.sync_units(&walk);
            for (lx, ly, button) in [
                (0, 32767, buttons::FIRE),
                (0, -32767, buttons::FIRE),
                (-32767, 0, buttons::FIRE),
                (32767, 0, buttons::FIRE),
                (0, 32767, buttons::SWITCH_WEAPON),
                (-32767, 0, buttons::SWITCH_WEAPON),
            ] {
                let index = walk.player.index as usize;
                walk.sim.units[index].soldier = core::soldier::Soldier::new(Vec3::ZERO);
                walk.sim.units[index].soldier.energy = 100.0;
                walk.sim.units[index].weapon.melee = None;
                vis.units[index].animator = Animator::new();
                let mut checked = 0;
                let mut authored = 0;
                let mut smooth = 0;
                let mut previous = None;
                let start = walk.sim.tick;
                while walk.sim.tick < start + 60 {
                    let tick = walk.sim.tick - start;
                    let pad = PadState {
                        lx,
                        ly,
                        buttons: if tick == 6 || (button == buttons::SWITCH_WEAPON && tick == 36) {
                            button
                        } else {
                            0
                        },
                        ..Default::default()
                    };
                    let ticks = walk.advance_with(1.0 / fps as f32, &[pad], |w| vis.note_shots(w));
                    let hash = walk.sim.hash();
                    fx.advance_frame(Some(&mut vis), &mut walk, &pad, 1.0 / fps as f32);
                    assert_eq!(hash, walk.sim.hash());
                    walk.blend_in();
                    let u = &walk.sim.units[index];
                    if u.weapon
                        .melee
                        .as_ref()
                        .is_some_and(|m| m.pose().is_some() && m.movement_active)
                    {
                        authored += 1;
                    }
                    let phase = vis.units[index].animator.upper_phase();
                    let clip = vis.units[index].animator.upper_clip();
                    if ticks == 0 && u.weapon.melee.as_ref().and_then(|m| m.pose()).is_some() {
                        smooth += usize::from(previous.is_some_and(|(old_clip, old_phase)| {
                            clip == old_clip && phase != old_phase
                        }));
                    }
                    previous = Some((clip, phase));
                    if u.weapon
                        .melee
                        .as_ref()
                        .is_some_and(|m| m.pose().is_some() && !m.movement_active)
                        && Vec3::new(u.soldier.vel.x, 0.0, u.soldier.vel.z).length() > 0.5
                    {
                        let asset = vis.asset_for(u.team, u.spec.class).unwrap();
                        let lower = vis.units[index].animator.lower_clip().unwrap();
                        let name = &asset.asset.clips[lower].name;
                        assert!(
                            !name.contains("attack") && !name.contains("block"),
                            "{hero} {fps} {name}"
                        );
                        checked += 1;
                    }
                    walk.blend_out();
                    assert_eq!(hash, walk.sim.hash());
                }
                assert!(checked + authored > 5, "{hero} {fps} {button}");
                if button == buttons::FIRE {
                    assert!(
                        smooth > 1,
                        "{hero} {fps}: live melee clock did not advance between ticks"
                    );
                }
                let u = &walk.sim.units[index];
                let asset = vis.asset_for(u.team, u.spec.class).unwrap();
                let name = &asset.asset.clips[vis.units[index].animator.upper_clip().unwrap()].name;
                assert!(!name.contains("block"), "block must exit: {hero} {fps}");
            }
            for (lx, ly, direction) in [
                (0, 0, 0),
                (0, 32767, 1),
                (0, -32767, 2),
                (-32767, 0, 3),
                (32767, 0, 4),
            ] {
                let index = walk.player.index as usize;
                walk.sim.units[index].soldier = core::soldier::Soldier::new(Vec3::ZERO);
                walk.sim.units[index].soldier.energy = 100.0;
                walk.sim.units[index].weapon.melee = None;
                vis.units[index].animator = Animator::new();
                let start = walk.sim.tick;
                let mut checked = 0;
                while walk.sim.tick < start + 60 {
                    let tick = walk.sim.tick - start;
                    let pad = PadState {
                        lx,
                        ly,
                        buttons: if tick == 6 || tick == 15 {
                            buttons::JUMP
                        } else {
                            0
                        },
                        ..Default::default()
                    };
                    walk.advance_with(1.0 / fps as f32, &[pad], |w| vis.note_shots(w));
                    walk.blend_in();
                    vis.update(&walk, 1.0 / fps as f32);
                    let u = &walk.sim.units[index];
                    if tick > 17 && tick < 25 {
                        assert_eq!(u.soldier.jump_animation, Some(direction), "{hero} {fps}");
                        let asset = vis.asset_for(u.team, u.spec.class).unwrap();
                        let name = &asset.asset.clips
                            [vis.units[index].animator.lower_clip().unwrap()]
                        .name;
                        let expected = ["jump", "jump", "jump_backward", "jump_left", "jump_right"]
                            [direction as usize];
                        assert!(
                            name.contains(expected),
                            "{hero} {fps}: {name} wanted {expected}"
                        );
                        checked += 1;
                    }
                    walk.blend_out();
                }
                assert!(checked > 0);
            }
            let index = walk.player.index as usize;
            let lightning = walk.sim.units[index].held.iter().position(|w| {
                walk.sim.weapon_classes[w.class].force.as_ref().is_some_and(
                    |p| matches!(p.kind, core::hero_weapon::ForceKind::Emitter(e) if e.lightning),
                )
            });
            if let Some(slot) = lightning {
                let u = &mut walk.sim.units[index];
                u.soldier = core::soldier::Soldier::new(Vec3::ZERO);
                u.weapon.melee = None;
                u.cur1 = Some(slot as u8);
                for (k, held) in u.held.iter_mut().enumerate() {
                    held.state = core::weapon::WeaponState::with_clips(
                        &walk.sim.weapon_classes[held.class],
                        true,
                        0,
                    );
                    held.state.active = k == slot;
                }
                let class = walk.teams[u.team as usize].classes[u.spec.class as usize]
                    .anim
                    .clone();
                let ai = vis.asset_index(u.team, u.spec.class).unwrap();
                vis.units[index].cast.clear();
                walk.sim.cheats.infinite_stamina = true;
                let start = walk.sim.tick;
                let mut held_frames = 0;
                let mut windup_frames = 0;
                while walk.sim.tick < start + 120 {
                    let tick = walk.sim.tick - start;
                    let pad = PadState {
                        buttons: if tick < 90 {
                            buttons::SECONDARY_FIRE
                        } else {
                            0
                        },
                        ..Default::default()
                    };
                    walk.advance_with(1.0 / fps as f32, &[pad], |w| vis.note_shots(w));
                    let hash = walk.sim.hash();
                    fx.advance_frame(Some(&mut vis), &mut walk, &pad, 1.0 / fps as f32);
                    assert_eq!(hash, walk.sim.hash());
                    let frame = vis.units[index].cast.clone_frame(
                        &walk.sim.units[index],
                        &walk.sim.weapon_classes,
                        &vis.assets[ai].asset.clips,
                        &class,
                    );
                    if tick < 15
                        && walk.sim.units[index].active_weapon().2.state
                            == core::weapon::FireState::Firing
                        && !walk
                            .sim
                            .events
                            .iter()
                            .any(|e| matches!(e, core::sim::Event::Ability { .. }))
                    {
                        assert!(frame.is_some(), "cast starts during the weapon wind-up");
                        windup_frames += 1;
                    }
                    if (60..85).contains(&tick) {
                        let (name, seconds) =
                            frame.as_ref().expect("held lightning keeps its cast pose");
                        assert!(
                            *seconds > 1.0,
                            "{hero} {fps} {name}: cast restarted at {seconds}"
                        );
                        held_frames += 1;
                    }
                    if tick > 100 {
                        assert!(frame.is_none(), "release clears the cast");
                    }
                }
                assert!(held_frames > 10);
                assert!(windup_frames > 0);
                walk.sim.cheats.infinite_stamina = false;
            }
            hashes.push(walk.sim.hash());
        }
        assert_eq!(hashes[0], hashes[1], "{hero} 60/75 sim determinism");
        assert_eq!(hashes[0], hashes[2], "{hero} 60/120 sim determinism");
    }
}

#[test]
fn test_choke_victim_has_animation() {
    let root = crate::assets::baked_root();
    if !root.join("bf2_kam1/manifest.ron").is_file() {
        eprintln!("SKIP: actual BF2 hero assets required");
        return;
    }
    let manifest =
        ron::from_str(&pak::read_to_string(root.join("bf2_kam1/manifest.ron")).unwrap()).unwrap();
    let scene = crate::assets::Scene {
        dir: root.join("bf2_kam1"),
        manifest,
        meshes: Default::default(),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: Default::default(),
        collision: Default::default(),
        cutters: vec![],
    };
    let mut walk = WalkSim::new(198, Vec3::ZERO, None);
    let db =
        crate::roster::load_classdb(&root.join("classes"), "bf2", &["all", "rep", "cis", "imp"])
            .unwrap();
    let base = crate::roster::ClassRoster::build(&db, "all_inf_rifleman", 0, None)
        .unwrap()
        .register(&mut walk.sim, &mut vec![]);
    walk.sim.classes = [vec![base.clone()], vec![base]];
    let mut picker = crate::class_picker::ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    let choice = picker
        .choices
        .iter()
        .position(|c| c.class.class == "rep_hero_anakin")
        .unwrap();
    assert!(picker.select(&mut walk, choice));
    let target = &picker
        .choices
        .iter()
        .find(|c| c.class.class == "rep_inf_ep3_rifleman")
        .unwrap()
        .class;
    walk.teams[1].classes = vec![crate::walk::ClassVisual::from_roster(target)];
    let target_spec = core::unit::UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -4.0));
    let victim = walk.sim.spawn(&target_spec).unwrap();
    let mut vis = SoldierVisuals::load_cpu(&root, "bf2", &walk.teams);
    vis.sync_units(&walk);

    // Victim is alive, choked:
    walk.sim.units[victim.index as usize].choked = Some(core::hero_weapon::Choked {
        left: 0.5,
        age: 0.5,
    });
    vis.update(&walk, 1.0 / 60.0);

    let sa = vis.asset_for(1, 0).expect("victim asset exists");
    let choke_clip = sa
        .asset
        .clips
        .iter()
        .find(|c| c.name == "human_choking")
        .unwrap();
    let sample = choke_clip.sample(&sa.asset.skeleton, 15.0, false);
    let pose = vis.units[victim.index as usize]
        .animator
        .pose(&sa.asset.skeleton);
    assert_eq!(pose.0, sample.0, "BF2 victim should match human_choking");

    // BF1 soldier has no human_choking clip; verify status_pose falls back to flail:
    let bf1_clip = sa
        .asset
        .clips
        .iter()
        .find(|c| c.name.contains("flail"))
        .unwrap();
    let mut animator = crate::animator::Animator::default();
    // Simulate BF1 clips list (without human_choking):
    let bf1_clips: Vec<_> = sa
        .asset
        .clips
        .iter()
        .filter(|c| !c.name.contains("choking"))
        .cloned()
        .collect();
    assert!(animator.status_pose(&bf1_clips, &sa.asset.skeleton, "human", 0.5));
    let pose_bf1 = animator.pose(&sa.asset.skeleton);
    let flail_sample = bf1_clip.sample(
        &sa.asset.skeleton,
        15.0 % (bf1_clip.frames.saturating_sub(1).max(1) as f32),
        false,
    );
    assert_eq!(
        pose_bf1.0, flail_sample.0,
        "BF1 victim should match flail fallback sample"
    );
}
