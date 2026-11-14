//! Console controller feedback from the local pilot's or soldier's tick.

use core::fmath::FloatExt;
use core::sim::{Event, Sim};
use core::{unit::UnitHandle, vehicle::Mount};
use input::RumbleMotor;

#[derive(Default)]
pub struct TickFeedback {
    next_fire: u64,
}

impl TickFeedback {
    pub fn pulse(
        &mut self,
        sim: &Sim,
        player: UnitHandle,
        prior_mount: Option<Mount>,
        on_fire: bool,
    ) -> Option<Pulse> {
        if let Some(impact) = for_tick(sim, player, prior_mount) {
            let ms = impact[0].duration_ms.max(impact[1].duration_ms);
            self.next_fire = sim.tick + (u64::from(ms) * u64::from(core::TICK_HZ)).div_ceil(1000);
            return Some(impact);
        }
        if !on_fire || sim.tick < self.next_fire {
            return None;
        }
        let shot = sim.events.iter().any(|event| {
            let Event::Shot {
                unit, weapon_class, ..
            } = *event
            else {
                return false;
            };
            unit == player
                && sim
                    .weapon_classes
                    .get(weapon_class as usize)
                    .is_some_and(|w| {
                        w.ordnance.max_damage > 0.0
                            && w.ordnance.sticky.is_none()
                            && w.ordnance.tow.is_none()
                            && w.force.is_none()
                            && w.saber.is_none()
                            && w.saber_throw.is_none()
                    })
        });
        if !shot {
            return None;
        }
        // PROVISIONAL (ours): a short, rate-limited firing cue.
        // measure: refine with authored weapon vibration and controller playtests.
        self.next_fire = sim.tick + u64::from(core::TICK_HZ).div_ceil(8);
        Some([motor(0.42, 55), motor(0.20, 45)])
    }
}

/// Weak and strong motor magnitude and duration in milliseconds.
pub type Pulse = [RumbleMotor; 2];

fn motor(magnitude: f32, duration_ms: u32) -> RumbleMotor {
    RumbleMotor {
        magnitude,
        duration_ms,
        fade_ms: 0,
    }
}

/// [ours] A short two-motor pulse to audition a controller setting.
pub fn preview(
    g: Option<&mut gilrs::Gilrs>,
    active: &input::ActivePad<gilrs::GamepadId>,
    effect: &mut Option<gilrs::ff::Effect>,
    strength: f32,
) {
    *effect = None;
    if strength <= 0.0 {
        return;
    }
    let Some(g) = g else { return };
    let Some((id, _)) = input::active_gamepad(g, active) else {
        return;
    };
    *effect = input::rumble(g, id, [motor(1.0, 280), motor(1.0, 280)], strength);
}

fn damage_pulse(damage: f32, remaining: f32, maximum: f32) -> Option<Pulse> {
    if damage <= 0.0 || damage <= 0.02 * remaining || maximum <= 0.0 {
        return None;
    }
    // B2 FUN_001dfb38, 91413–91420 and FUN_00433b28. The small
    // DualShock motor is binary (FUN_0059e950); the large one is analog.
    let share = (damage / maximum).fmin(2.0);
    Some([
        motor(1.0, (1000.0 * share).round() as u32),
        motor(
            (1.4 * share).fmin(1.0),
            (1000.0 * share / 1.2).round() as u32,
        ),
    ])
}

pub fn for_tick(sim: &Sim, player: UnitHandle, prior_mount: Option<Mount>) -> Option<Pulse> {
    let unit = sim
        .units
        .get(player.index as usize)
        .filter(|u| u.gen == player.gen)?;
    let occupies = |vehicle| {
        [unit.mount, prior_mount]
            .into_iter()
            .flatten()
            .any(|m| m.vehicle == vehicle)
    };
    sim.events.iter().rev().find_map(|event| match *event {
        Event::FlyerImpact { vehicle, .. } if occupies(vehicle) => {
            // B2 FUN_0020bff8 calls FUN_00432b68 with 0.6 and 0.3 s
            // (122512, 122606). The analog channel is scaled by 0.7.
            Some([
                motor(1.0, 600),
                RumbleMotor {
                    fade_ms: 300,
                    ..motor(0.28, 300)
                },
            ])
        }
        Event::Hit {
            victim,
            damage,
            health_left,
            ..
        } if victim == player => damage_pulse(damage, health_left, unit.health.max),
        Event::VehicleHit {
            vehicle, damage, ..
        } if occupies(vehicle) => {
            let hull = sim
                .vehicles
                .get(vehicle.index as usize)
                .filter(|v| v.gen == vehicle.gen)?;
            damage_pulse(damage, hull.health.cur, hull.health.max)
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_rumble_has_the_original_threshold_and_separate_motor_times() {
        assert!(damage_pulse(2.0, 100.0, 120.0).is_none());
        assert_eq!(
            damage_pulse(30.0, 90.0, 120.0),
            Some([motor(1.0, 250), motor(0.35, 208)])
        );
        assert_eq!(
            damage_pulse(500.0, 0.0, 100.0),
            Some([motor(1.0, 2000), motor(1.0, 1667)])
        );
    }

    #[test]
    fn a_fatal_hit_still_reaches_the_local_controllers_motors() {
        let mut sim = Sim::empty(42);
        let player = sim
            .spawn(&core::unit::UnitSpec::player(0, glam::Vec3::ZERO))
            .unwrap();
        sim.units[player.index as usize].alive = false;
        sim.events.push(Event::Hit {
            direct: true,
            shooter: player,
            victim: player,
            damage: 30.0,
            age: 0.0,
            health_left: 0.0,
            dir: glam::Vec3::ZERO,
        });
        assert!(for_tick(&sim, player, None).is_some());
        sim.units[player.index as usize].gen += 1;
        assert!(for_tick(&sim, player, None).is_none());
    }

    #[test]
    fn firing_feedback_is_optional_and_does_not_interrupt_impact_feedback() {
        use glam::Vec3;
        let mut sim = Sim::new(42);
        let player = sim.players[0].unwrap();
        let unit = sim.unit(player).unwrap();
        let class = unit.weapon_class;
        assert!(sim.weapon_classes[class].ordnance.max_damage > 0.0);
        let shot = Event::Shot {
            unit: player,
            weapon_class: class as u32,
            origin: Vec3::ZERO,
            dir: Vec3::NEG_Z,
        };
        sim.events.push(shot);
        let mut feedback = TickFeedback::default();
        assert!(feedback.pulse(&sim, player, None, false).is_none());
        assert!(feedback.pulse(&sim, player, None, true).is_some());
        assert!(feedback.pulse(&sim, player, None, true).is_none());
        sim.events.push(Event::Hit {
            direct: true,
            shooter: player,
            victim: player,
            damage: 100.0,
            age: 0.0,
            health_left: 150.0,
            dir: Vec3::ZERO,
        });
        let impact = feedback.pulse(&sim, player, None, true).unwrap();
        assert!(impact[0].duration_ms > 55);
        sim.tick += 5;
        sim.events = vec![shot];
        assert!(feedback.pulse(&sim, player, None, true).is_none());
    }
}
