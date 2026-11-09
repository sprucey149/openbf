use super::{emitter::Frame, Fx};
use core::sim::{Sim, StationDamageEffect};
use core::vehicle::{Vehicle, VehicleHandle};
use glam::{Mat3, Mat4};

const DEAD_HEALTH: f32 = 0.0001;

fn showing(v: &Vehicle, effect: &StationDamageEffect) -> bool {
    v.health.alive()
        && v.health.cur >= DEAD_HEALTH
        && v.health.max > 0.0
        && effect.band.active(v.health.cur / v.health.max)
}

fn frame_on(v: &Vehicle, effect: &StationDamageEffect) -> Frame {
    let m = Mat4::from_rotation_translation(v.rot, v.pos) * effect.local;
    Frame {
        pos: m.w_axis.truncate(),
        rot: Mat3::from_mat4(m),
        vel: v.vel,
    }
}

impl Fx {
    pub(super) fn follow_vehicle_damage(&mut self, sim: &Sim) {
        for live in &mut self.live {
            let Some((h, slot)) = live.vehicle else {
                continue;
            };
            let owner = sim.vehicle(h).and_then(|v| {
                let effect = sim
                    .vehicle_damage_effects
                    .get(v.class as usize)?
                    .get(slot)?;
                showing(v, effect).then_some((v, effect))
            });
            match owner {
                Some((v, effect)) => {
                    live.frame = frame_on(v, effect);
                }
                None => {
                    live.stop();
                    live.vehicle = None;
                }
            }
        }
        if sim.vehicle_damage_effects.is_empty() {
            return;
        }
        let count = sim.vehicles.len();
        self.vehicle_health.resize(count, None);
        for (i, v) in sim.vehicles.iter().enumerate() {
            let h = VehicleHandle {
                index: i as u32,
                gen: v.gen,
            };
            let now = v.alive.then_some((v.gen, v.health.cur));
            let before = std::mem::replace(&mut self.vehicle_health[i], now);
            if !v.alive || before.is_none_or(|b| b == (v.gen, v.health.cur)) {
                continue;
            }
            let Some(effects) = sim.vehicle_damage_effects.get(v.class as usize) else {
                continue;
            };
            for (slot, effect) in effects.iter().enumerate() {
                if !showing(v, effect) || self.live.iter().any(|l| l.vehicle == Some((h, slot))) {
                    continue;
                }
                if self.spawn(&effect.band.effect, frame_on(v, effect)) {
                    let live = self.live.last_mut().unwrap();
                    live.vehicle = Some((h, slot));
                    for emitter in &mut live.instances {
                        emitter.size *= effect.scale;
                        emitter.inherit += effect.inherit;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::autoturret::DamageEffect;
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use core::weapon::HealthType;
    use glam::Vec3;

    fn band(start: f32, stop: f32, effect: &str) -> StationDamageEffect {
        StationDamageEffect {
            band: DamageEffect {
                start,
                stop,
                effect: effect.to_owned(),
            },
            local: Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0)),
            scale: 2.0,
            inherit: 0.75,
        }
    }

    const SMOKE: &str = "com_sfx_vehiclesmoke_sc";

    fn fx() -> Option<Fx> {
        let root = crate::assets::baked_root();
        let common = root.join("common_bf2");
        if !common.join("effects.ron").exists() {
            eprintln!("SKIP: {} is not baked", common.display());
            return None;
        }
        Some(Fx::with_library(super::super::Library::load(
            &root,
            &common,
            "bf2",
            &["all"],
        )))
    }

    fn ship() -> (Sim, VehicleHandle) {
        let mut sim = Sim::empty(1);
        let h = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 20.0, 0.0),
            yaw: 0.0,
            max_health: 100.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        sim.vehicle_mut(h).unwrap().rot = glam::Quat::IDENTITY;
        sim.vehicle_damage_effects = vec![vec![band(0.75, 0.25, SMOKE)]];
        (sim, h)
    }

    #[test]
    fn a_band_lights_on_the_hit_that_enters_it_and_rides_the_hull() {
        let (mut sim, h) = ship();
        let Some(mut fx) = fx() else {
            return;
        };
        fx.follow_vehicle_damage(&sim);
        assert!(fx.live.is_empty());
        sim.vehicle_mut(h).unwrap().health.cur = 50.0;
        fx.follow_vehicle_damage(&sim);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].instances[0].inherit, 0.75);
        assert_eq!(fx.live[0].frame.pos, Vec3::new(0.0, 20.0, 2.0));
        sim.vehicle_mut(h).unwrap().pos = Vec3::new(5.0, 20.0, 0.0);
        fx.follow_vehicle_damage(&sim);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].frame.pos, Vec3::new(5.0, 20.0, 2.0));
    }

    #[test]
    fn leaving_the_band_and_dying_stop_the_effect() {
        let (mut sim, h) = ship();
        let Some(mut first) = fx() else {
            return;
        };
        first.follow_vehicle_damage(&sim);
        sim.vehicle_mut(h).unwrap().health.cur = 50.0;
        first.follow_vehicle_damage(&sim);
        sim.vehicle_mut(h).unwrap().health.cur = 10.0;
        first.follow_vehicle_damage(&sim);
        assert!(first.live[0].is_stopped());
        let Some(mut fx) = fx() else {
            return;
        };
        sim.vehicle_mut(h).unwrap().health.cur = 100.0;
        fx.follow_vehicle_damage(&sim);
        sim.vehicle_mut(h).unwrap().health.cur = 50.0;
        fx.follow_vehicle_damage(&sim);
        sim.vehicle_mut(h).unwrap().health.cur = 0.0;
        fx.follow_vehicle_damage(&sim);
        assert!(fx.live[0].is_stopped());
    }

    #[test]
    fn a_finished_effect_waits_for_the_next_change_of_health() {
        let (mut sim, h) = ship();
        let Some(mut fx) = fx() else {
            return;
        };
        fx.follow_vehicle_damage(&sim);
        sim.vehicle_mut(h).unwrap().health.cur = 50.0;
        fx.follow_vehicle_damage(&sim);
        fx.live.clear();
        fx.follow_vehicle_damage(&sim);
        assert!(fx.live.is_empty());
        sim.vehicle_mut(h).unwrap().health.cur = 49.0;
        fx.follow_vehicle_damage(&sim);
        assert_eq!(fx.live.len(), 1);
    }
}
