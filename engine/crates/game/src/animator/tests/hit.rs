use super::*;
use crate::animator::hit::Direction;
use crate::animator::hit::Reaction;

fn hit_clips(rig: &mut Rig, clips: &mut Vec<Clip>) {
    for d in Direction::ALL {
        let k = clips.len();
        let name = format!("human_rifle_{}", d.hit_name());
        clips.push(clip(k, &name, 1, false));
        rig.map.push(Some(k));
        rig.sets.clips.push(SetClip {
            name,
            frames: 1,
            looping: Some(false),
            blend: None,
        });
        if rig.game == Game::Bf2 {
            let pair = bf2::simple_pair(d.hit_name()).unwrap();
            for family in &mut rig.sets.bf2 {
                for slots in family.sets.values_mut() {
                    slots.resize(slots.len().max(pair * 2 + 2), None);
                    slots[2 * pair] = Some(k as u16);
                }
            }
        }
    }
}

#[test]
fn hit_directions_and_distribution_on_all_sides() {
    // The way the hit travels, at yaw 0 (the soldier faces -Z).
    let vectors = [Vec3::NEG_Z, Vec3::NEG_X, Vec3::Z, Vec3::X];
    for (d, dir) in Direction::ALL.into_iter().zip(vectors) {
        assert_eq!(Direction::from_hit(dir, 0.0), d);
        assert_eq!(
            Direction::from_hit(Quat::from_rotation_y(0.7) * dir, 0.7),
            d
        );
        let mut counts = [0; 4];
        for draw in 0..1000 {
            counts[d.pick(draw) as usize] += 1;
        }
        assert_eq!(counts[d as usize], 500);
        assert_eq!(counts[(d as usize + 1) % 4], 200);
        assert_eq!(counts[(d as usize + 3) % 4], 200);
        assert_eq!(counts[(d as usize + 2) % 4], 100);
    }
}

#[test]
fn hit_clock_masks_and_stance_gating_do_not_change_actions() {
    for mut rig in [rig_bf1(), rig_bf2()] {
        let mut clips = clips();
        hit_clips(&mut rig, &mut clips);
        let skel = skel();
        for stance in [
            Stance::Stand,
            Stance::Crouch,
            Stance::Prone,
            Stance::Sprint,
            Stance::Jump,
            Stance::Fall,
            Stance::Roll,
            Stance::Jet,
            Stance::Hover,
            Stance::Thrown,
            Stance::Tumble,
            Stance::GetUp,
            Stance::Dead,
        ] {
            for speed in [0.0, 3.0] {
                let i = AnimInput {
                    stance,
                    ..input(speed)
                };
                let body = stance_of(&i, rig.game).map_or_else(Body::Special, |s| {
                    Body::Feet(
                        s,
                        if speed == 0.0 {
                            Move::Idle
                        } else {
                            Move::WalkForward
                        },
                    )
                });
                let mut hit = Reaction::default();
                hit.clock = 1.0;
                hit.step(&rig, &clips, &skel, &i, body, 0.0625);
                let allowed = matches!(stance, Stance::Stand | Stance::Crouch);
                assert_eq!(hit.overlay.is_some(), allowed, "{stance:?}");
                if let Some((_, mask, weight)) = &hit.overlay {
                    assert_eq!(*weight, 0.5);
                    assert!(mask[ARM]);
                    assert_eq!(mask[LEG], speed == 0.0);
                    let mut drawn = Pose::bind(&skel);
                    hit.apply(&mut drawn);
                    let sample = hit.overlay.as_ref().unwrap().0 .0[ARM];
                    assert_eq!(drawn.0[ARM].translation, sample.translation * 0.5);
                    assert_eq!(
                        drawn.0[LEG].translation,
                        if speed == 0.0 {
                            sample.translation * 0.5
                        } else {
                            Vec3::ZERO
                        }
                    );
                }
                hit.step(&rig, &clips, &skel, &i, body, 0.0625);
                assert_eq!(hit.clock, 0.0);
                if let Some((_, _, weight)) = &hit.overlay {
                    assert_eq!(*weight, 0.25);
                }
                hit.step(&rig, &clips, &skel, &i, body, 0.0);
                assert!(hit.overlay.is_none());
            }
        }
        for fired in [false, true] {
            for reloading in [false, true] {
                let i = AnimInput {
                    fired,
                    reloading,
                    ..input(3.0)
                };
                let mut plain = Animator::new();
                let mut hit = Animator::new();
                hit.hit.clock = 1.0;
                for _ in 0..10 {
                    plain.step(&rig, &clips, &skel, &i, DT);
                    hit.step(&rig, &clips, &skel, &i, DT);
                    assert_eq!(plain.action(), hit.action());
                    assert_eq!(plain.upper_clip(), hit.upper_clip());
                    assert_eq!(plain.upper_phase(), hit.upper_phase());
                    assert_eq!(plain.lower_phase(), hit.lower_phase());
                    assert_eq!(plain.body(), hit.body());
                    assert_eq!(plain.twist(), hit.twist());
                }
                assert_eq!(
                    plain.pose(&skel).0,
                    hit.pose(&skel).0,
                    "overlay must not feed back into kept pose"
                );
            }
        }
    }
}

/// PC BF2 `EntitySoldier::ApplyDamage` dots the bolt's own velocity with
/// the body's forward: a shot in the chest from in front pushes the body
/// back, and its death falls backward.
#[test]
fn a_shot_from_in_front_is_a_hit_from_the_back() {
    let toward_his_chest = Vec3::Z; // the shooter stands at -Z, in front.
    assert_eq!(Direction::from_hit(toward_his_chest, 0.0), Direction::Back);
    assert_eq!(Direction::Back.death_name(), "stand_death_backward");
    assert_eq!(Direction::default(), Direction::Back);
}

#[test]
fn splash_keeps_the_last_direct_hits_death_side() {
    let mut walk = crate::walk::WalkSim::new(42, Vec3::ZERO, None);
    let victim = walk.player;
    let mut a = Animator::new();
    let hit = |direct, dir| core::sim::Event::Hit {
        direct,
        shooter: victim,
        victim,
        dir,
        damage: 1.0,
        health_left: 50.0,
        age: 0.0,
    };
    walk.sim.events = vec![hit(true, Vec3::X), hit(false, Vec3::Z)];
    a.note_hit(&walk.sim, 0);
    let side = a.hit.direction;
    assert_ne!(side, Direction::from_hit(Vec3::Z, 0.0));
    a.note_hit(&walk.sim, 1);
    assert_eq!(a.hit.direction, side, "splash does not move the death side");
}

#[test]
fn hit_events_are_read_once_rearm_and_leave_sim_hash_unchanged() {
    let mut walk = crate::walk::WalkSim::new(42, Vec3::ZERO, None);
    let victim = walk.player;
    let mut a = Animator::new();
    let event = |age, health_left| core::sim::Event::Hit {
        direct: age != core::sim::BLAST_AGE,
        shooter: victim,
        victim,
        dir: Vec3::Z,
        damage: 0.0,
        health_left,
        age,
    };
    walk.sim.events = vec![event(core::sim::BLAST_AGE, 100.0), event(0.0, 100.0)];
    let before = walk.sim.hash();
    a.note_hit(&walk.sim, 0);
    assert_eq!(a.hit.clock, 0.0, "splash excluded");
    a.note_hit(&walk.sim, 1);
    assert_eq!(a.hit.clock, 1.0, "zero-damage direct contact included");
    let picked = a.hit.picked;
    a.hit.clock = 0.3;
    a.note_hit(&walk.sim, 1);
    assert_eq!(a.hit.clock, 0.3, "retained tick cannot rearm");
    assert_eq!(walk.sim.hash(), before);
    walk.sim.tick += 1;
    a.note_hit(&walk.sim, 1);
    assert_eq!(a.hit.clock, 1.0);
    let mut replay = Animator::new();
    walk.sim.tick -= 1;
    replay.note_hit(&walk.sim, 1);
    assert_eq!(picked, replay.hit.picked);
    walk.sim.tick += 1;
    walk.sim.events = vec![event(0.0, 100.0)];
    if let core::sim::Event::Hit { direct, .. } = &mut walk.sim.events[0] {
        *direct = false;
    }
    a.hit.clock = 0.0;
    a.note_hit(&walk.sim, 0);
    assert_eq!(
        a.hit.clock, 0.0,
        "non-direct contact with age zero excluded"
    );
    walk.sim.tick += 1;
    walk.sim.events = vec![event(0.0, 0.0)];
    a.hit.clock = 0.0;
    a.note_hit(&walk.sim, 0);
    assert_eq!(a.hit.clock, 0.0, "killing hit cannot flinch");
}

#[test]
#[ignore = "requires complete owned side bakes via OPENBF_HIT_BAKES"]
fn baked_hit_poses_resolve_for_every_model_and_bf2_weapon_set() {
    let root =
        std::path::PathBuf::from(std::env::var_os("OPENBF_HIT_BAKES").expect("owned bake root"));
    let mut models = 0;
    let mut sets = 0;
    for source in ["ps2", "xbox", "pc1", "mixed"] {
        let mut count = 0;
        for game in ["bf1", "bf2"] {
            let side = root.join(source).join(format!("side_{game}"));
            for side in [side.clone(), side.join("locals")] {
                for entry in std::fs::read_dir(side.join("soldiers")).unwrap() {
                    let path = entry.unwrap().path();
                    let name = path.file_name().unwrap().to_str().unwrap();
                    if !name.ends_with(".glb")
                        || name.contains(".low.")
                        || name.contains(".render-clips.")
                    {
                        continue;
                    }
                    let asset = crate::assets::load_skinned_glb(&path).unwrap();
                    let rig = crate::soldier_vis::SoldierVisuals::load_rig(
                        &side,
                        path.file_stem().unwrap().to_str().unwrap(),
                        &asset,
                    )
                    .unwrap();
                    for set in 0..rig.bf2.len().max(1) {
                        for picked in Direction::ALL {
                            let mut hit = Reaction::default();
                            hit.clock = 1.0;
                            hit.picked = picked;
                            hit.step(
                                &rig,
                                &asset.clips,
                                &asset.skeleton,
                                &AnimInput { set, ..input(0.0) },
                                Body::Feet(0, Move::Idle),
                                0.0,
                            );
                            assert!(
                                hit.overlay.is_some(),
                                "{source}: {name}, set {set}, {picked:?}"
                            );
                        }
                        sets += 1;
                    }
                    count += 1;
                }
            }
        }
        // The mixed scan also supplies the two approved DLC heroes.
        assert_eq!(
            count,
            if source == "mixed" { 81 } else { 79 },
            "complete side group: {source}"
        );
        models += count;
    }
    println!(
        "hit pose slots resolved: {models} animated models, {sets} sets, four directions each"
    );
}

#[test]
fn a_restarted_sim_with_an_earlier_tick_still_flinches() {
    let mut walk = crate::walk::WalkSim::new(42, Vec3::ZERO, None);
    let victim = walk.player;
    let mut a = Animator::new();
    walk.sim.events = vec![core::sim::Event::Hit {
        direct: true,
        shooter: victim,
        victim,
        dir: Vec3::Z,
        damage: 0.0,
        health_left: 100.0,
        age: 0.0,
    }];
    walk.sim.tick = 500;
    a.note_hit(&walk.sim, 0);
    assert_eq!(a.hit.clock, 1.0);
    a.hit.clock = 0.0;
    walk.sim.tick = 3;
    a.note_hit(&walk.sim, 0);
    assert_eq!(
        a.hit.clock, 1.0,
        "a restart below the last read tick is not ignored"
    );
}
