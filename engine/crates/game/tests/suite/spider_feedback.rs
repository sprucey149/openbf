//! Spider Walker barrel convergence and BF2 destruction regressions.

use super::seat_camera::{map, missions_of, terrain_hit};
use core::sim::{buttons, Event, PadState};
use core::vehicle::VehicleSpec;

/// The Spider's lower cannon and upper beam have one barrel each, six
/// metres apart. Each must meet the same aim point, as the PS2 does.
/// Restricting barrel convergence to rings with multiple points makes the
/// lower cannon miss by 6.19 m in the first BF2 fixture.
#[test]
fn spider_weapons_meet_the_reticle_from_their_own_barrels() {
    for key in ["bf1_geo1", "bf2_geo1"] {
        let (mut m, spec) = spider_map(key);
        let h = m.walk.sim.spawn_vehicle(&spec);
        let player = m.walk.player;
        // Isolate barrel convergence from the weapons' authored dispersion.
        for wc in &mut m.walk.sim.weapon_classes {
            wc.pitch_spread = 0.0;
            wc.yaw_spread = 0.0;
            wc.spread_per_shot = 0.0;
            wc.posture_spread = [[0.0; 3]; 3];
        }
        assert!(m.walk.sim.seat_unit(player, h, 0));
        let (mut bolts, mut beams) = (0, 0);
        for (yaw, pitch) in [(0.0, -0.12), (0.25, -0.18), (-0.3, -0.1)] {
            let w = &mut m.walk.sim.vehicle_mut(h).unwrap().walker;
            (w.aim_yaw, w.aim_pitch) = (yaw, pitch);
            for _ in 0..60 {
                m.walk.sim.step(&[PadState::default()]);
            }
            let look = m.walk.sim.seat_look(player).unwrap();
            let ground = terrain_hit(&m.walk.sim, look.eye, look.view).unwrap();
            let target = m.walk.sim.collision.ray_with(
                look.eye,
                look.view,
                (ground - look.eye).length(),
                core::collision::Layers::ORDNANCE,
                core::collision::HullFilter::except(Some(h)),
            );
            let target = target.map_or(ground, |hit| look.eye + look.view * hit.t);
            let cam = m.walk.camera();
            let reticle = m.walk.reticle_y();
            for button in [buttons::FIRE, buttons::SECONDARY_FIRE] {
                let mut fired = 0;
                for _ in 0..20 {
                    m.walk.sim.step(&[PadState {
                        buttons: button,
                        ..Default::default()
                    }]);
                    for e in &m.walk.sim.events {
                        if let Event::Shot {
                            unit, origin, dir, ..
                        } = e
                        {
                            if *unit == player {
                                let delta = target - *origin;
                                let miss = (delta - *dir * delta.dot(*dir)).length();
                                assert!(miss < 0.25,
                                    "{key}, look ({yaw}, {pitch}), channel {button}: misses by {miss} m");
                                fired += 1;
                            }
                        }
                    }
                    for b in &m.walk.sim.beams {
                        if b.shooter.owner == player {
                            let hit = b.origin + b.dir * b.len;
                            let c = cam.view_proj(4.0 / 3.0) * hit.extend(1.0);
                            let screen = (0.5 + 0.5 * c.x / c.w, 0.5 - 0.5 * c.y / c.w);
                            assert!(
                                (screen.0 - 0.5).abs() < 0.01 && (screen.1 - reticle).abs() < 0.01,
                                "{key}: beam at {screen:?}, reticle (0.5, {reticle})"
                            );
                            beams += 1;
                        }
                    }
                }
                assert!(fired >= 1, "{key}: channel {button} never fired");
                bolts += fired;
                for _ in 0..90 {
                    m.walk.sim.step(&[PadState::default()]);
                }
            }
        }
        assert!(
            bolts >= 6 && beams >= 3,
            "{key}: {bolts} shots, {beams} beam samples"
        );
        eprintln!("{key}: {bolts} shots and {beams} beam samples meet the reticle");
    }
}

fn spider_map(key: &str) -> (super::seat_camera::Map, VehicleSpec) {
    let dir = game::assets::baked_root().join(key);
    let mission = missions_of(key)
        .into_iter()
        .next()
        .expect("Geonosis mission");
    let m = map(&dir, &mission.factions()).expect("Geonosis bake");
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "cis_walk_spider")
        .expect("Spider pad");
    (m, spec)
}

/// PS2 slot 10: the idle Spider reaches entry 4 within five video frames.
/// Its death01 entry has zero duration; this is not a missing port clip.
/// A walker killed during a step still finishes that step before removal.
#[test]
fn spider_death_has_no_collapse_clip_but_finishes_the_current_step() {
    use core::vehicle::walker::{ANIM_DEATH, ANIM_WALK};
    let key = "bf2_geo1";
    let (mut m, spec) = spider_map(key);
    let params = m.walk.sim.walker_params[spec.class as usize]
        .as_ref()
        .unwrap();
    assert!(params.clips[ANIM_DEATH as usize].is_none());
    let duration = params.clips[ANIM_WALK].as_ref().unwrap().duration;
    for walking in [false, true] {
        let h = m.walk.sim.spawn_vehicle(&spec);
        let w = &mut m.walk.sim.vehicle_mut(h).unwrap().walker;
        if walking {
            w.anim = ANIM_WALK as u8;
            w.clip_time = duration * 0.5;
            w.dir = 1;
        }
        m.walk.sim.destroy_vehicle(h, None);
        assert!(m.walk.sim.vehicle(h).unwrap().walker.dead);
        let end = (1..=120)
            .find(|_| {
                m.walk.sim.step(&[PadState::default()]);
                m.walk.sim.vehicle(h).is_none()
            })
            .expect("Spider destruction finishes");
        if walking {
            assert!(end > 2, "{key}: skipped the current step");
        } else {
            assert!(end <= 3, "{key}: invented a death delay");
        }
        eprintln!("{key}: walking={walking}, destruction after {end} ticks");
    }
}

/// The Spider's beam is drawn from its turret's barrel as the turret turns
/// (playtest 2026-11-14, both Geonosis maps: "the beam is not following the
/// piece that shoots it, instead coming from the air"). The PS2 re-reads a
/// seat beam's origin from its fire point every frame (B2 `FUN_00311d00`).
/// The sim's origin is the seat's rest point: on BF2 it stays put, on BF1 it
/// swings the other way, 2.6 m off the barrel at 0.6 rad. Mutation:
/// `walker_shots::beam_muzzle` returning `Beam::origin` -> both maps fail.
#[test]
fn spider_beam_leaves_the_turned_turret() {
    for key in ["bf1_geo1", "bf2_geo1"] {
        let (mut m, spec) = spider_map(key);
        let h = m.walk.sim.spawn_vehicle(&spec);
        let player = m.walk.player;
        assert!(m.walk.sim.seat_unit(player, h, 0));
        let mut samples = 0;
        for yaw in [0.6f32, -0.5] {
            let w = &mut m.walk.sim.vehicle_mut(h).unwrap().walker;
            (w.aim_yaw, w.aim_pitch) = (yaw, -0.1);
            for _ in 0..60 {
                m.walk.step_tick(&[PadState::default()]);
            }
            for button in [buttons::FIRE, buttons::SECONDARY_FIRE] {
                for _ in 0..10 {
                    m.walk.step_tick(&[PadState {
                        buttons: button,
                        ..Default::default()
                    }]);
                }
                let v = m.walk.sim.vehicle(h).unwrap().clone();
                for b in m
                    .walk
                    .sim
                    .beams
                    .iter()
                    .filter(|b| b.shooter.owner == player)
                {
                    let muzzle = game::walker_shots::beam_muzzle(&m.walk, b).unwrap();
                    let barrel =
                        game::walker_shots::frame(&m.walk, player, b.shooter.weapon_class, 0)
                            .unwrap()
                            .w_axis
                            .truncate();
                    assert!(
                        muzzle.distance(barrel) < 1e-4,
                        "{key}: {muzzle} vs {barrel}"
                    );
                    let local = v.rot.inverse() * (muzzle - v.pos);
                    assert!(
                        local.x * yaw.signum() > 1.0,
                        "{key}, aim {yaw}: barrel at {local}"
                    );
                    samples += 1;
                }
                for _ in 0..90 {
                    m.walk.step_tick(&[PadState::default()]);
                }
            }
        }
        assert!(samples >= 2, "{key}: {samples} beam samples");
    }
}

/// Issue #182 (playtest 2026-11-16, BF1 Geonosis): "when aiming at something
/// nearby it sometimes shoots straight up or right over the target while
/// the targeter is right on it". Each Spider weapon hangs on its own pair
/// of aimers (`turret_y_*` yawing `turret_x_*`), and the PS2 steers each
/// from its own node toward the seat's aim point. The port turned both
/// barrels about one seat point (BF1: the lower cannon's muzzle) and then
/// fired from the muzzle at the point: a target 3-8 m out lay behind the
/// muzzle, so the shot turned round and left at the 25 deg limit, up and
/// back (BF1 laser at 3 m: model direction (-0.24, 0.42, -0.87)). A
/// soldier is held on the reticle's ray 3-15 m out from the walker; every
/// shot must go out toward him, and every shot whose barrel can reach him
/// (the reticle's point's pitch from the barrel's pivot inside its limits,
/// beyond its length) must pass within 0.1 m of the point the reticle
/// covers on his capsule. Mutation: either half of the fix
/// undone (`GunSetup::line_aim` unused, or the yaw-only carrier dropping
/// the pivot in `fire_ring`) fails both maps.
#[test]
fn spider_shots_meet_close_targets_on_the_reticle() {
    use core::unit::{Controller, UnitHandle, UnitSpec};
    use core::vehicle::gun;
    use glam::Vec3;
    for key in ["bf1_geo1", "bf2_geo1"] {
        let (mut m, spec) = spider_map(key);
        let h = m.walk.sim.spawn_vehicle(&spec);
        let player = m.walk.player;
        for wc in &mut m.walk.sim.weapon_classes {
            wc.pitch_spread = 0.0;
            wc.yaw_spread = 0.0;
            wc.spread_per_shot = 0.0;
            wc.posture_spread = [[0.0; 3]; 3];
        }
        assert!(m.walk.sim.seat_unit(player, h, 0));
        let setup = m.walk.sim.seat_look(player).unwrap().gun;
        // Each weapon's one barrel: the bind of the `turret_x_*` aimer that
        // pitches it (the upper turret's over 6 m, the lower's under; both
        // discs' models agree), its length and its pitch limits.
        let barrels: Vec<_> = setup
            .weapons
            .iter()
            .map(|w| {
                let pivot = if w.fire_points[0].y > 6.0 {
                    Vec3::new(0.0, 9.2316, 0.1257)
                } else {
                    Vec3::new(0.0, 3.1053, 0.0)
                };
                (
                    pivot,
                    (w.fire_points[0] - pivot).length(),
                    w.fire_limits[0][1],
                )
            })
            .collect();
        let enemy = 1 - m.walk.sim.unit(player).unwrap().team.min(1);
        let target = UnitSpec {
            controller: Controller::Idle,
            max_health: 1.0e9,
            ..UnitSpec::trooper(enemy, Vec3::new(0.0, -500.0, 0.0))
        };
        let t = m.walk.sim.spawn(&target).unwrap();
        // Hold the target on the reticle's ray, `d` m out from the walker's
        // centre along the ground; its capsule's middle on the ray.
        let hold = |m: &mut super::seat_camera::Map, t: UnitHandle, d: f32| {
            let look = m.walk.sim.seat_look(player).unwrap();
            let v = m.walk.sim.vehicle(h).unwrap();
            let flat = look.view.with_y(0.0);
            let r = ((v.pos - look.eye).with_y(0.0).dot(flat.normalize()) + d) / flat.length();
            let at = look.eye + look.view * r;
            let u = m.walk.sim.unit_mut(t).unwrap();
            u.soldier.pos = at - Vec3::Y * 0.5 * core::unit::capsule_height(u.soldier.stance);
            u.soldier.vel = Vec3::ZERO;
            at
        };
        let (mut reached, mut checked) = (0, 0);
        for (yaw, pitch) in [
            (0.0f32, -0.3f32),
            (0.0, -0.1),
            (0.9, -0.25),
            (-2.0, -0.2),
            (3.0, -0.3),
        ] {
            for d in [3.0f32, 5.0, 8.0, 11.0, 15.0] {
                let w = &mut m.walk.sim.vehicle_mut(h).unwrap().walker;
                (w.aim_yaw, w.aim_pitch) = (yaw, pitch);
                for _ in 0..40 {
                    hold(&mut m, t, d);
                    m.walk.sim.step(&[PadState::default()]);
                }
                for (k, button) in [buttons::FIRE, buttons::SECONDARY_FIRE]
                    .into_iter()
                    .enumerate()
                {
                    let mut fired = false;
                    for _ in 0..25 {
                        let at = hold(&mut m, t, d);
                        m.walk.sim.step(&[PadState {
                            buttons: button,
                            ..Default::default()
                        }]);
                        let shot = m.walk.sim.events.iter().find_map(|e| match e {
                            Event::Shot {
                                unit, origin, dir, ..
                            } if *unit == player => Some((*origin, *dir)),
                            _ => None,
                        });
                        let Some((origin, dir)) = shot.filter(|_| !fired) else {
                            continue;
                        };
                        fired = true;
                        checked += 1;
                        let v = m.walk.sim.vehicle(h).unwrap();
                        let (pivot, length, limits) = barrels[k];
                        let from = v.rot.inverse() * (at - v.pos) - pivot;
                        let local = v.rot.inverse() * dir;
                        assert!(
                            local.with_y(0.0).dot(from.with_y(0.0)) > 0.0,
                            "{key}, aim ({yaw}, {pitch}), {d} m, channel {button}: shot {local} (model) turns away from the target at {from} off its turret"
                        );
                        // The reticle's point: where the camera ray meets him.
                        let look = m.walk.sim.seat_look(player).unwrap();
                        let cap = m.walk.sim.unit(t).unwrap().capsule();
                        let hit = cap
                            .ray_hit(look.eye, look.view, 600.0)
                            .expect("the reticle on him");
                        let p = look.eye + look.view * hit;
                        let from = v.rot.inverse() * (p - v.pos) - pivot;
                        let (_, need) = gun::angles(from);
                        if from.length() < length + 0.5
                            || need < limits[0] + 0.03
                            || need > limits[1] - 0.03
                        {
                            continue;
                        }
                        let off = p - origin;
                        let miss = (off - dir * off.dot(dir)).length();
                        assert!(
                            miss < 0.1 && off.dot(dir) > 0.0,
                            "{key}, aim ({yaw}, {pitch}), {d} m, channel {button}: shot from {origin} along {dir} passes {miss} m from the reticle's point {p}"
                        );
                        reached += 1;
                    }
                    assert!(fired, "{key}: channel {button} never fired");
                    for _ in 0..40 {
                        hold(&mut m, t, d);
                        m.walk.sim.step(&[PadState::default()]);
                    }
                }
            }
        }
        eprintln!("{key}: {checked} close shots go toward the target; {reached} within the barrels' reach pass through it");
        assert!(reached >= 15, "{key}: only {reached} reachable close shots");
    }
}
