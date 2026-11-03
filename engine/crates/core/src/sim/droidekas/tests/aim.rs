use super::*;
use crate::fmath;
use crate::perception::Contact;
use crate::weapon::DEG;

fn ramp(degrees: f32) -> Sim {
    let mut s = Sim::empty(11);
    let (sn, cs) = fmath::sin_cos(degrees * DEG);
    let grade = sn / cs;
    s.heightfield = Some(crate::heightfield::Heightfield {
        size: 101,
        unit: 2.0,
        offset_x: 100.0,
        offset_z: 100.0,
        heights: (0..101)
            .flat_map(|z| std::iter::repeat_n(50.0 - (z as f32 * 2.0 - 100.0) * grade, 101))
            .collect(),
    });
    s
}

fn gun_class() -> Arc<DroidekaClass> {
    Arc::new(DroidekaClass {
        guns: vec![Vec3::new(-0.5, 0.81, 1.46), Vec3::new(0.5, 0.81, 1.46)],
        ..DroidekaClass::default()
    })
}

fn bot(s: &mut Sim) -> UnitHandle {
    s.spawn(&UnitSpec {
        controller: Controller::Ai,
        droideka: Some(gun_class()),
        ..deka(UnitSpec::trooper(1, Vec3::new(0.0, 50.0, 0.0)))
    })
    .unwrap()
}

fn foe(s: &mut Sim, z: f32) -> UnitHandle {
    let y = s.heightfield.as_ref().unwrap().height_at(0.0, z);
    s.spawn(&UnitSpec {
        controller: Controller::Idle,
        ..UnitSpec::trooper(0, Vec3::new(0.0, y, z))
    })
    .unwrap()
}

fn aim_at(s: &mut Sim, h: UnitHandle, t: UnitHandle) -> Vec3 {
    let at = s.unit(t).unwrap().capsule().center();
    let tick = s.tick;
    let u = s.unit_mut(h).unwrap();
    let off = at - u.ai_eye();
    u.soldier.yaw = crate::ai::yaw_toward(off);
    u.soldier.pitch = fmath::asin(off.normalize().y);
    u.ai.target = Some(t.into());
    u.ai.contacts[0] = Some(Contact {
        handle: t.into(),
        first_seen: 0,
        last_seen: tick,
        last_pos: at,
        seen_now: true,
        heard: false,
        told: false,
    });
    at
}

#[test]
fn a_bot_droideka_shot_line_passes_through_its_aim_point() {
    let mut s = ramp(0.0);
    let h = bot(&mut s);
    let t = foe(&mut s, -15.0);
    let at = aim_at(&mut s, h, t);
    for barrel in 0..2 {
        s.unit_mut(h).unwrap().droideka.as_mut().unwrap().barrel = barrel;
        let (origin, (dir, right, up)) = s.shot_frame(h.index as usize);
        let miss = (at - origin).cross(dir).length();
        assert!(miss < 0.1, "barrel {barrel}: miss {miss} m");
        assert!((dir.length() - 1.0).abs() < 1e-6);
        assert!(right.dot(dir).abs() < 1e-6 && up.dot(dir).abs() < 1e-6);
        assert!((right.cross(dir) - up).length() < 1e-6);
    }
}

#[test]
fn a_bot_droideka_on_an_upslope_hits_a_target_up_the_ramp() {
    let mut s = ramp(20.0);
    // Isolate parallax from ordinary aim error and weapon spread.
    let w = &mut s.weapon_classes[0];
    w.pitch_spread = 0.0;
    w.yaw_spread = 0.0;
    w.spread_per_shot = 0.0;
    w.spread_limit = 0.0;
    let h = bot(&mut s);
    let t = foe(&mut s, -15.0);
    s.unit_mut(t).unwrap().consts.aim_size = 0.0;
    // Begin with the tracker settled; startup tracking lag is a different
    // reason for a shot to be below the target.
    aim_at(&mut s, h, t);
    let dir = s.unit(h).unwrap().aim_frame().0;
    s.unit_mut(h).unwrap().ai.aim_dir = dir;
    // A recent hit faithfully starts a fight beyond the 10 m sight-entry range.
    s.unit_mut(h).unwrap().last_hit = Some((t, s.tick));
    let mut fired = 0;
    let mut early_ground = 0;
    let mut damage = 0.0f32;
    for _ in 0..240 {
        s.step(&[]);
        for e in &s.events {
            if let Event::Shot {
                unit, origin, dir, ..
            } = e
            {
                if *unit == h {
                    fired += 1;
                    let hit = s.terrain_ray(*origin, *dir, 10.0, 0.5);
                    early_ground += usize::from(hit.is_some());
                }
            }
        }
        let u = s.unit_mut(t).unwrap();
        damage += (u.health.max - u.health.cur).max(0.0);
        u.health.cur = u.health.max;
    }
    eprintln!("ramp: {fired} shots, {early_ground} early terrain rays, {damage} damage");
    assert!(fired > 0, "the test must exercise bot firing");
    assert!(damage > 0.0, "bot bolts must damage the uphill foe");
    assert_eq!(early_ground, 0, "no bolt may strike the ramp within 10 m");
}

#[test]
fn a_bot_droideka_still_deploys_on_a_slope() {
    let mut s = ramp(20.0);
    let h = bot(&mut s);
    for _ in 0..25 {
        s.step(&[]);
    }
    assert_eq!(state(&s, h), ROLLING);
    let t = foe(&mut s, -8.0);
    s.unit_mut(h).unwrap().last_hit = Some((t, s.tick));
    for _ in 0..150 {
        s.step(&[]);
        let u = s.unit_mut(t).unwrap();
        u.health.cur = u.health.max;
    }
    let u = s.unit(h).unwrap();
    assert_eq!(u.ai.behaviour, crate::ai::Behaviour::Fight);
    assert_eq!(state(&s, h), IDLE);
    assert!(
        u.soldier.ground_n.y < 0.95,
        "must actually stand on a slope"
    );
}

#[test]
fn a_bot_droideka_pitch_is_world_level_and_clamped() {
    for degrees in [-20.0, 20.0] {
        for sign in [-1.0, 1.0] {
            let mut s = ramp(degrees);
            let h = bot(&mut s);
            let u = s.unit_mut(h).unwrap();
            u.ai.behaviour = crate::ai::Behaviour::Fight;
            let p = PadState {
                ry: PadState::quantize(sign),
                ..Default::default()
            };
            for _ in 0..120 {
                s.step_droideka(h.index as usize, &p, crate::TICK_DT);
            }
            let u = s.unit(h).unwrap();
            let want = sign * 25.0 * DEG;
            assert!((u.soldier.pitch - want).abs() < 1e-6);
            let (fwd, right, _) = u.aim_frame();
            assert!((fmath::asin(fwd.y) - want).abs() < 1e-6);
            assert_eq!(right.y, 0.0);
            assert!(u.soldier.ground_n.y < 0.95);
        }
    }
}

#[test]
fn human_droideka_shots_keep_the_reticle_path() {
    let mut s = ramp(20.0);
    let h = s
        .spawn(&UnitSpec {
            droideka: Some(gun_class()),
            ..UnitSpec::player(0, Vec3::new(0.0, 50.0, 0.0))
        })
        .unwrap();
    let t = foe(&mut s, -15.0);
    aim_at(&mut s, h, t);
    for barrel in 0..2 {
        s.unit_mut(h).unwrap().droideka.as_mut().unwrap().barrel = barrel;
        let u = s.unit(h).unwrap();
        let origin = u.droideka_muzzle().unwrap();
        let dir = (s.aim_hit(h.index as usize) - origin).normalize();
        let actual = s.shot_frame(h.index as usize);
        assert_eq!(actual.0, origin);
        assert_eq!(actual.1 .0, dir);
        assert_eq!(actual, s.human_shot(h.index as usize));
    }
}

#[test]
fn a_bot_droideka_converges_on_its_steered_aim_not_the_target() {
    let mut s = ramp(0.0);
    let h = bot(&mut s);
    let t = foe(&mut s, -15.0);
    aim_at(&mut s, h, t);
    s.unit_mut(h).unwrap().soldier.pitch = 0.1;
    let u = s.unit(h).unwrap();
    let eye = u.ai_eye();
    let at = eye + u.aim_frame().0 * u.ai.contacts[0].unwrap().last_pos.distance(eye);
    let (origin, (dir, ..)) = s.shot_frame(h.index as usize);
    assert!((at - origin).cross(dir).length() < 1e-4);
    assert!(
        (s.unit(t).unwrap().capsule().center() - origin)
            .cross(dir)
            .length()
            > 1.0
    );
}

#[test]
fn a_bot_droideka_shot_stays_within_authored_aimer_limits() {
    let mut s = ramp(20.0);
    let h = bot(&mut s);
    let t = foe(&mut s, -0.1);
    aim_at(&mut s, h, t);
    // Deliberately narrower and asymmetric than the defaults, to catch an
    // implementation that hardcodes 25/40 or clamps relative to the aim.
    let mut c = (*gun_class()).clone();
    c.aimer_pitch_limits = (-12.0 * DEG, 18.0 * DEG);
    c.aimer_yaw_limits = (-10.0 * DEG, 30.0 * DEG);
    s.unit_mut(h).unwrap().spec.droideka = Some(Arc::new(c));
    let mut clamped = 0;
    for yaw in [-3.0, -0.8, 0.0, 2.5, 3.0] {
        for pitch in [-25.0 * DEG, 0.0, 25.0 * DEG] {
            for barrel in 0..2 {
                let u = s.unit_mut(h).unwrap();
                u.soldier.yaw = yaw;
                u.soldier.pitch = pitch;
                u.droideka.as_mut().unwrap().barrel = barrel;
                let u = s.unit(h).unwrap();
                let (sy, cy) = fmath::sin_cos(yaw);
                let body = Vec3::new(-sy, 0.0, -cy);
                let right = Vec3::new(cy, 0.0, -sy);
                let c = u.droideka_class().unwrap();
                let (_, (dir, ..)) = s.shot_frame(h.index as usize);
                let p = fmath::asin(dir.y);
                let y = fmath::atan2(dir.dot(right), dir.dot(body));
                assert!(
                    p >= c.aimer_pitch_limits.0 - 1e-6 && p <= c.aimer_pitch_limits.1 + 1e-6,
                    "pitch {p}"
                );
                assert!(
                    y >= c.aimer_yaw_limits.0 - 1e-6 && y <= c.aimer_yaw_limits.1 + 1e-6,
                    "yaw {y}"
                );
                clamped += usize::from(
                    (p - c.aimer_pitch_limits.0).abs() < 1e-6
                        || (p - c.aimer_pitch_limits.1).abs() < 1e-6,
                );
                assert!((dir.length() - 1.0).abs() < 1e-6);
            }
        }
    }
    assert!(clamped > 0, "must exercise a limit");
}

#[test]
fn droideka_perception_sees_over_a_wall_from_its_class_eye() {
    use crate::collision::{Body, Shape};
    use glam::Mat4;
    let mut s = ramp(0.0);
    let h = bot(&mut s);
    let t = foe(&mut s, -8.0);
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 50.8, -1.0)),
        Shape::Box {
            half: Vec3::new(3.0, 0.8, 0.1),
        },
    ));
    let u = s.unit(h).unwrap();
    let at = s.unit(t).unwrap().capsule().center();
    assert!(
        !crate::perception::los(&s, u.muzzle(), at),
        "the old 1.425 m origin is blocked"
    );
    assert!(
        crate::perception::los(&s, u.droideka_point(u.droideka_class().unwrap().eye), at),
        "the 1.8 m class eye clears it"
    );
    let mut st = u.ai;
    crate::perception::scan(&s, h, &mut st, s.unit_tier(u));
    assert!(
        crate::perception::contact_of(&st, t).is_some_and(|c| c.seen_now),
        "the scan must use the class eye"
    );
}

#[test]
fn a_droidekas_ai_eye_uses_its_class_point_and_yaw_only() {
    let mut s = ramp(20.0);
    let h = bot(&mut s);
    let mut c = (*gun_class()).clone();
    c.eye = Vec3::new(0.3, 1.8, 0.2);
    let u = s.unit_mut(h).unwrap();
    u.spec.droideka = Some(Arc::new(c));
    u.soldier.yaw = 0.7;
    u.soldier.pitch = 25.0 * DEG;
    let (sn, cs) = fmath::sin_cos(0.7);
    let expected = u.soldier.pos + Vec3::new(cs * 0.3 - sn * 0.2, 1.8, -sn * 0.3 - cs * 0.2);
    assert!((u.ai_eye() - expected).length() < 1e-5);
    let t = foe(&mut s, -15.0);
    assert_eq!(s.unit(t).unwrap().ai_eye(), s.unit(t).unwrap().muzzle());
}
