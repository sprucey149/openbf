use super::*;

const DT: f32 = 1.0 / 30.0;

fn input(weapon: usize) -> Input {
    Input {
        weapon,
        row: 0,
        ..Default::default()
    }
}

fn settled(weapon: usize) -> Chooser {
    let mut c = Chooser::default();
    c.step(&input(weapon), true, false, DT);
    c
}

#[test]
fn bf1_rows_follow_the_weapon_set_switch() {
    let set = |n: &str| manifest::anim_sets::bf1::weapon_set(n).unwrap();
    assert_eq!(bf1_row(set("rifle")), 0);
    assert_eq!(bf1_row(set("bdroid")), 0);
    assert_eq!(bf1_row(set("lascannon")), 1);
    assert_eq!(bf1_row(set("bazooka")), 2);
    assert_eq!(bf1_row(set("pistol")), 3);
    assert_eq!(bf1_row(set("tool")), 3);
    assert_eq!(bf1_row(set("sbdroid")), 3);
    assert_eq!(bf1_row(set("lightsabre")), 3);
    assert_eq!(bf1_row(set("grenade")), 4);
    assert_eq!(clip_name(false, 1, column::RUN), "_1st_lascannon_run");
    assert_eq!(clip_name(false, 4, column::SHOOT), "_1st_grenade_shoot");
}

#[test]
fn bf2_rows_send_offhand_to_the_grenade_row_and_the_rest_by_base_class() {
    assert_eq!(bf2_row(true, Some("rifle")), 3);
    assert_eq!(bf2_row(false, Some("rifle")), 0);
    assert_eq!(bf2_row(false, Some("bazooka")), 1);
    assert_eq!(bf2_row(false, Some("pistol")), 2);
    assert_eq!(bf2_row(false, Some("melee")), 2);
    assert_eq!(bf2_row(false, None), 2);
    assert_eq!(clip_name(true, 1, column::RELOAD), "humanfp_bazooka_reload");
    assert_eq!(clip_name(true, 3, column::SHOOT), "humanfp_grenade_shoot");
}

#[test]
fn standing_idles_and_moving_runs_both_looped() {
    let mut c = Chooser::default();
    let p = c.step(&input(0), true, false, DT).unwrap();
    assert_eq!((p.column, p.looping), (column::IDLE, true));
    let run = Input {
        speed2: 0.1,
        ..input(0)
    };
    let p = c.step(&run, false, false, DT).unwrap();
    assert_eq!((p.column, p.looping, p.snap), (column::RUN, true, false));
    assert_eq!(c.step(&run, false, false, DT), None);
}

#[test]
fn a_shot_snaps_to_shoot_and_holds_it_until_it_ends() {
    let mut c = settled(0);
    let shot = Input {
        fire: Fire::Shooting,
        shot: true,
        ..input(0)
    };
    let p = c.step(&shot, false, false, DT).unwrap();
    assert_eq!((p.column, p.looping, p.snap), (column::SHOOT, false, true));
    let after = Input {
        fire: Fire::Shooting,
        ..input(0)
    };
    assert_eq!(c.step(&after, false, false, DT), None);
    assert_eq!(c.column, column::SHOOT);
    let p = c.step(&shot, false, false, DT).unwrap();
    assert_eq!((p.column, p.snap), (column::SHOOT, true));
    let p = c.step(&after, true, false, DT).unwrap();
    assert_eq!(p.column, column::IDLE);
}

#[test]
fn a_reload_plays_once() {
    let mut c = settled(0);
    let p = c
        .step(
            &Input {
                fire: Fire::Reloading,
                ..input(0)
            },
            false,
            false,
            DT,
        )
        .unwrap();
    assert_eq!(
        (p.column, p.looping, p.snap),
        (column::RELOAD, false, false)
    );
}

#[test]
fn a_weapon_change_lowers_the_hands_then_adopts_once_the_blend_is_done() {
    let mut c = settled(0);
    let p = c.step(&input(1), false, false, DT).unwrap();
    assert_eq!((p.column, p.snap), (column::HANDSDOWN, false));
    assert_eq!(c.weapon, Some(0));
    assert_eq!(c.step(&input(1), false, true, DT), None);
    assert_eq!(c.weapon, Some(0));
    let p = c.step(&input(1), false, false, DT).unwrap();
    assert_eq!(p.column, column::IDLE);
    assert_eq!(c.weapon, Some(1));
}

#[test]
fn a_change_with_a_shot_adopts_at_once() {
    let mut c = settled(0);
    let p = c
        .step(
            &Input {
                fire: Fire::Shooting,
                shot: true,
                ..input(1)
            },
            false,
            false,
            DT,
        )
        .unwrap();
    assert_eq!(p.column, column::SHOOT);
    assert_eq!(c.weapon, Some(1));
}

#[test]
fn a_jump_plays_jump_then_the_landing_for_half_a_second() {
    let mut c = settled(0);
    let jump = Input {
        motion: Motion::Jump,
        ..input(0)
    };
    let p = c.step(&jump, false, false, DT).unwrap();
    assert_eq!((p.column, p.looping), (column::JUMP, false));
    assert_eq!(c.step(&jump, false, false, DT), None);
    let p = c.step(&input(0), false, false, DT).unwrap();
    assert_eq!(p.column, column::LAND);
    let mut ticks = 1;
    while c.step(&input(0), false, false, DT).is_none() {
        ticks += 1;
        assert!(ticks < 100);
    }
    assert_eq!(c.column, column::IDLE);
    assert!(
        (ticks as f32 * DT - LAND_SECS).abs() <= DT + 1e-4,
        "{ticks}"
    );
}

#[test]
fn a_fall_flails_looped_then_lowers_the_hands_for_a_second() {
    let mut c = settled(0);
    let fall = Input {
        motion: Motion::Fall,
        ..input(0)
    };
    let p = c.step(&fall, false, false, DT).unwrap();
    assert_eq!((p.column, p.looping), (column::FLAIL, true));
    let p = c.step(&input(0), false, false, DT).unwrap();
    assert_eq!(p.column, column::HANDSDOWN);
}

#[test]
fn a_roll_plays_the_jump_once_and_the_jet_keeps_its_clip() {
    let mut c = settled(0);
    let p = c
        .step(
            &Input {
                motion: Motion::Roll,
                ..input(0)
            },
            false,
            false,
            DT,
        )
        .unwrap();
    assert_eq!((p.column, p.looping), (column::JUMP, false));
    let jet = Input {
        motion: Motion::Jet,
        speed2: 30.0,
        ..input(0)
    };
    assert_eq!(c.step(&jet, false, false, DT), None);
    assert_eq!(c.column, column::JUMP);
}

#[test]
fn moving_prone_lowers_the_hands() {
    let mut c = settled(0);
    let crawl = Input {
        motion: Motion::Prone,
        speed2: 1.0,
        ..input(0)
    };
    let p = c.step(&crawl, false, false, DT).unwrap();
    assert_eq!((p.column, p.looping), (column::HANDSDOWN, true));
    let still = Input {
        motion: Motion::Prone,
        ..input(0)
    };
    assert_eq!(
        c.step(&still, false, false, DT).unwrap().column,
        column::IDLE
    );
}

#[test]
fn a_spawn_snaps_the_hands_down_then_raises_them_through_idle() {
    let mut c = settled(0);
    c.spawned();
    let p = c.step(&input(2), false, false, DT).unwrap();
    assert_eq!(
        (p.column, p.snap, p.looping),
        (column::HANDSDOWN, true, false)
    );
    assert_eq!(c.weapon, Some(2));
    let p = c.step(&input(2), false, false, DT).unwrap();
    assert_eq!(p.column, column::IDLE);
}

#[test]
fn the_run_rate_scales_with_speed_squared_up_to_twenty() {
    assert_eq!(run_rate(column::IDLE, 50.0), 1.0);
    assert!((run_rate(column::RUN, 10.0) - 0.375).abs() < 1e-6);
    assert!((run_rate(column::RUN, 20.0) - 0.75).abs() < 1e-6);
    assert!((run_rate(column::RUN, 64.0) - 0.75).abs() < 1e-6);
}

fn one_bone() -> (Skeleton, Vec<Clip>) {
    use crate::skin::BoneXform;
    let skel = Skeleton::new(vec!["dummyroot".into()], vec![None], vec![Mat4::IDENTITY]);
    let at = |x: f32| BoneXform {
        rotation: glam::Quat::IDENTITY,
        translation: Vec3::new(x, 0.0, 0.0),
    };
    let clips = vec![
        Clip {
            name: "a".into(),
            fps: FPS,
            frames: 2,
            tracks: vec![Some(vec![at(0.0), at(0.0)])],
        },
        Clip {
            name: "b".into(),
            fps: FPS,
            frames: 2,
            tracks: vec![Some(vec![at(1.0), at(1.0)])],
        },
    ];
    (skel, clips)
}

#[test]
fn a_change_chases_the_last_pose_by_one_minus_b_squared() {
    let (skel, clips) = one_bone();
    let mut p = Playback::default();
    p.play(Some(0), true, false);
    assert_eq!(p.blend, 0.0);
    p.advance(&skel, &clips, 1.0, DT);
    p.play(Some(1), true, false);
    assert_eq!(p.blend, 1.0);
    let x = p.advance(&skel, &clips, 1.0, 0.1).0[0].translation.x;
    assert_eq!(x, 0.0);
    let x = p.advance(&skel, &clips, 1.0, 0.1).0[0].translation.x;
    assert!((x - 0.09).abs() < 1e-5);
    let x = p.advance(&skel, &clips, 1.0, 0.1).0[0].translation.x;
    assert!((x - (0.09 + 0.91 * 0.36)).abs() < 1e-5);
    for _ in 0..8 {
        p.advance(&skel, &clips, 1.0, 0.1);
    }
    assert_eq!(p.pose.as_ref().unwrap().0[0].translation.x, 1.0);
}

#[test]
fn a_snap_skips_the_blend_and_a_one_shot_finishes_on_its_last_frame() {
    let (skel, clips) = one_bone();
    let mut p = Playback::default();
    p.play(Some(0), false, false);
    p.advance(&skel, &clips, 1.0, DT * 0.5);
    p.play(Some(1), false, true);
    assert_eq!(p.advance(&skel, &clips, 1.0, 0.0).0[0].translation.x, 1.0);
    assert!(!p.finished(&clips));
    p.advance(&skel, &clips, 1.0, DT);
    assert!(p.finished(&clips));
}

#[test]
fn the_model_hangs_below_and_ahead_of_the_eye_with_the_barrel_forward() {
    let cam = Mat4::from_translation(Vec3::new(0.0, -0.22, -0.4145));
    for bf2 in [false, true] {
        let m = model_matrix(Mat4::IDENTITY, cam, bf2, 1.0);
        let shoulders = m.transform_point3(Vec3::new(0.0, -0.25114378, -0.11909829));
        assert!((shoulders.y - -0.122).abs() < 2e-3, "{shoulders}");
        let ahead = if bf2 { -0.247 } else { -0.597 };
        assert!((shoulders.z - ahead).abs() < 2e-3, "{shoulders}");
        let barrel = m.transform_vector3(Vec3::NEG_Z);
        assert!(barrel.z > 0.98, "model -Z points back at the eye: {barrel}");
    }
    let eye = Mat4::from_translation(Vec3::new(5.0, 2.0, 1.0));
    let a = model_matrix(eye, cam, false, 1.0).transform_point3(Vec3::ZERO);
    let b = model_matrix(Mat4::IDENTITY, cam, false, 1.0).transform_point3(Vec3::ZERO);
    assert!((a - b - Vec3::new(5.0, 2.0, 1.0)).length() < 1e-5);
}

#[test]
fn a_wider_view_frames_the_arms_where_the_ps2_view_put_them() {
    assert!((ps2_tan_half_fov_y(false) - 0.34131).abs() < 1e-4);
    assert!((ps2_tan_half_fov_y(true) - 0.53678).abs() < 1e-4);
    let cam = Mat4::from_translation(Vec3::new(0.0, -0.22, -0.4145));
    let elbow = Vec3::new(-0.05, -0.3, 0.1);
    for bf2 in [false, true] {
        let ps2 = ps2_tan_half_fov_y(bf2);
        let ndc = |p: Vec3, tan: f32| (p.x / (-p.z * tan), p.y / (-p.z * tan));
        let at_ps2 = ndc(
            model_matrix(Mat4::IDENTITY, cam, bf2, 1.0).transform_point3(elbow),
            ps2,
        );
        for fov_y in [0.5f32, 0.98, 1.3] {
            let k = frame_scale(fov_y, bf2);
            let p = model_matrix(Mat4::IDENTITY, cam, bf2, k).transform_point3(elbow);
            let ours = ndc(p, (fov_y * 0.5).tan());
            assert!((ours.0 - at_ps2.0).abs() < 1e-4 && (ours.1 - at_ps2.1).abs() < 1e-4);
        }
        assert!((frame_scale(2.0 * ps2.atan(), bf2) - 1.0).abs() < 1e-5);
    }
}
