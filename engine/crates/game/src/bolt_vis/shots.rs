//! Match this tick's shot events to fresh rounds once, preserving salvo order.

use crate::walk::WalkSim;
use core::fmath::FloatExt;
use core::projectile::{Bolt, ShotFlight};
use core::sim::Event;
use core::unit::UnitHandle;
use glam::Vec3;

pub(super) struct Shot {
    pub event: usize,
    pub unit: UnitHandle,
    pub class: usize,
    pub origin: Vec3,
    pub dir: Vec3,
    pub bolt: Option<usize>,
    pub flight: Option<ShotFlight>,
}

impl Shot {
    pub fn muzzle(&self, walk: &WalkSim) -> Option<Vec3> {
        let ordinal = crate::walker_shots::turn(walk, self.unit, self.class, self.event);
        crate::walker_shots::frame(walk, self.unit, self.class, ordinal)
            .map(|m| m.w_axis.truncate())
    }
}

pub(super) struct Shots {
    pub events: Vec<Shot>,
    by_bolt: Vec<Option<usize>>,
}

impl Shots {
    pub fn of(walk: &WalkSim) -> Self {
        let sim = &walk.sim;
        let mut events: Vec<_> = sim
            .events
            .iter()
            .enumerate()
            .filter_map(|(event, e)| match *e {
                Event::Shot {
                    unit,
                    weapon_class,
                    origin,
                    dir,
                } => Some(Shot {
                    event,
                    unit,
                    class: weapon_class as usize,
                    origin,
                    dir,
                    bolt: None,
                    flight: sim
                        .shot_flights
                        .iter()
                        .find(|f| f.shot.event == event)
                        .copied(),
                }),
                _ => None,
            })
            .collect();
        let mut by_bolt = vec![None; sim.bolts.len()];
        for (i, b) in sim.bolts.iter().enumerate() {
            if let Some(identity) = b.visual_shot {
                // First-flight records survive removal. Late droid/turret
                // births have only a live ID until the next flight step; their
                // producer input tick precedes observed Sim.tick by one.
                if let Some(k) = events.iter().position(|s| {
                    s.bolt.is_none()
                        && s.event == identity.event
                        && s.unit == b.owner
                        && s.class == b.weapon_class
                        && (s.flight.is_some_and(|f| f.shot == identity)
                            || (s.flight.is_none()
                                && identity.tick.checked_add(1) == Some(sim.tick)))
                }) {
                    events[k].bolt = Some(i);
                    by_bolt[i] = Some(k);
                }
                continue;
            }
            if b.age < 0.0 || b.age > core::TICK_DT + 1e-6 {
                continue;
            }
            let Some((k, _)) = events
                .iter()
                .enumerate()
                .filter(|(_, s)| s.bolt.is_none() && s.flight.is_none())
                .filter_map(|(k, s)| launch_error(b, s).map(|error| (k, error)))
                .min_by(|(_, a), (_, b)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))
            else {
                continue;
            };
            events[k].bolt = Some(i);
            by_bolt[i] = Some(k);
        }
        Self { events, by_bolt }
    }

    pub fn for_bolt(&self, index: usize) -> Option<&Shot> {
        self.by_bolt[index].map(|k| &self.events[k])
    }
}

fn launch_error(b: &Bolt, shot: &Shot) -> Option<(f32, f32)> {
    if b.owner != shot.unit || b.weapon_class != shot.class {
        return None;
    }
    // A surviving fresh round has moved once with its current velocity.
    // Shell gravity was applied before that move, after the Shot event.
    let origin = b.pos - b.vel * b.age;
    let initial = (b.vel + Vec3::Y * (b.gravity * b.age)).normalize_or_zero();
    let tolerance = (8.0 * f32::EPSILON * shot.origin.abs().max_element()).fmax(1e-3);
    // Resolve exact/nearest launch directions before tolerant alternatives.
    // An earlier almost-parallel round may already have struck this tick.
    (origin.abs_diff_eq(shot.origin, tolerance) && initial.abs_diff_eq(shot.dir, 1e-5)).then_some((
        initial.distance_squared(shot.dir),
        origin.distance_squared(shot.origin),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::collision::{CollisionWorld, HullFilter};
    use core::projectile::Shooter;

    fn shot(w: &WalkSim, class: usize, dir: Vec3) -> Event {
        Event::Shot {
            unit: w.player,
            weapon_class: class as u32,
            origin: Vec3::ZERO,
            dir,
        }
    }

    fn bolt(w: &WalkSim, class: usize, dir: Vec3) -> Bolt {
        let mut b = Bolt::fire(
            Vec3::ZERO,
            dir,
            100.0,
            3.0,
            Shooter {
                owner: w.player,
                team: 0,
                weapon_class: class,
                is_player: true,
            },
            None,
            None,
            &CollisionWorld::default(),
            HullFilter::All,
        );
        b.advance(core::TICK_DT);
        b
    }

    #[test]
    fn fresh_near_parallel_and_identical_rounds_each_take_their_own_event() {
        let mut w = WalkSim::new(42, Vec3::ZERO, None);
        let class = w.sim.unit(w.player).unwrap().weapon_class;
        let directions = [Vec3::Z, (Vec3::Z + Vec3::X * 0.001).normalize(), Vec3::Z];
        let mut older = bolt(&w, class, Vec3::X);
        older.advance(core::TICK_DT);
        w.sim.bolts.push(older);
        for dir in directions {
            w.sim.events.push(shot(&w, class, dir));
            w.sim.bolts.push(bolt(&w, class, dir));
        }
        let s = Shots::of(&w);
        assert!(s.for_bolt(0).is_none());
        for k in 0..directions.len() {
            assert_eq!(s.for_bolt(k + 1).unwrap().event, k);
            assert_eq!(s.events[k].bolt, Some(k + 1));
        }
    }

    #[test]
    fn a_surviving_almost_parallel_second_shot_does_not_take_the_removed_first_shot() {
        let mut w = WalkSim::new(42, Vec3::ZERO, None);
        let class = w.sim.unit(w.player).unwrap().weapon_class;
        let later = (Vec3::Z + Vec3::X * 0.000005).normalize();
        w.sim.events.push(shot(&w, class, Vec3::Z));
        w.sim.events.push(shot(&w, class, later));
        // The first round resolved inside the tick; only the second survived.
        w.sim.bolts.push(bolt(&w, class, later));
        let shots = Shots::of(&w);
        assert_eq!(shots.for_bolt(0).unwrap().event, 1);
        assert_eq!(shots.events[0].bolt, None);
    }

    #[test]
    fn an_identical_tagged_second_shot_keeps_its_identity_after_the_first_is_removed() {
        use core::projectile::{ShotFlight, VisualShot};
        let mut w = WalkSim::new(42, Vec3::ZERO, None);
        let class = w.sim.unit(w.player).unwrap().weapon_class;
        w.sim.tick = 1;
        for event in 0..2 {
            w.sim.events.push(shot(&w, class, Vec3::Z));
            w.sim.shot_flights.push(ShotFlight {
                shot: VisualShot {
                    tick: w.sim.tick.saturating_sub(1),
                    event,
                },
                end: Vec3::Z * if event == 0 { 2.0 } else { 100.0 },
            });
        }
        let mut survivor = bolt(&w, class, Vec3::Z);
        survivor.visual_shot = Some(w.sim.shot_flights[1].shot);
        w.sim.bolts.push(survivor);
        let shots = Shots::of(&w);
        assert_eq!(shots.for_bolt(0).unwrap().event, 1);
        assert_eq!(shots.events[0].bolt, None);
    }

    #[test]
    fn a_late_born_tagged_round_matches_without_a_first_flight_record() {
        use core::projectile::VisualShot;
        let mut w = WalkSim::new(42, Vec3::ZERO, None);
        let class = w.sim.unit(w.player).unwrap().weapon_class;
        w.sim.tick = 1;
        w.sim.events.push(shot(&w, class, Vec3::Z));
        let mut late = bolt(&w, class, Vec3::Z);
        late.age = 0.0;
        late.pos = Vec3::ZERO;
        late.visual_shot = Some(VisualShot { tick: 0, event: 0 });
        w.sim.bolts.push(late);
        assert_eq!(Shots::of(&w).for_bolt(0).unwrap().event, 0);
        w.sim.tick += 1;
        w.sim.bolts[0].advance(core::TICK_DT);
        // Event zero now belongs to a new shot, not the preceding late birth.
        assert!(
            Shots::of(&w).for_bolt(0).is_none(),
            "old late-born round reused a new shot event"
        );
    }

    #[test]
    fn a_fresh_shell_matches_its_launch_direction_before_gravity() {
        let mut w = WalkSim::new(42, Vec3::ZERO, None);
        let class = w.sim.unit(w.player).unwrap().weapon_class;
        w.sim.events.push(shot(&w, class, Vec3::Z));
        let mut b = bolt(&w, class, Vec3::Z);
        b.age = 0.0;
        b.pos = Vec3::ZERO;
        b.gravity = 9.8;
        b.vel.y -= b.gravity * core::TICK_DT;
        b.advance(core::TICK_DT);
        w.sim.bolts.push(b);
        assert_eq!(Shots::of(&w).for_bolt(0).unwrap().event, 0);
    }
}
