//! Spider Walker barrel convergence and BF2 destruction regressions.

use super::seat_camera::{map, missions_of};
use core::sim::{buttons, Event, PadState};
use core::vehicle::VehicleSpec;
use glam::Vec3;

fn terrain_hit(sim: &core::sim::Sim, from: Vec3, dir: Vec3) -> Option<Vec3> {
    let hf = sim.heightfield.as_ref()?;
    (1..6000)
        .map(|k| from + dir * (k as f32 * 0.05))
        .find(|p| p.y < hf.height_at(p.x, p.z))
}

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
