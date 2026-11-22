//! The animated props' tick (`crate::animated_prop`): triggers and catches
//! against the live units, the one-shot clip's kill, the bodies the bones
//! move and the held soldier's hold.

use super::{DeathCause, Event, Sim};
use crate::unit::UnitHandle;
use glam::{Mat4, Vec3};

impl Sim {
    /// One tick of every animated prop, before anyone moves (beside the
    /// doors). Nothing at all on a map without one.
    pub(crate) fn tick_props(&mut self, dt: f32) {
        if self.props.is_empty() {
            return;
        }
        for k in 0..self.props.len() {
            if let Some(h) = self.props[k].begin() {
                if self.unit(h).is_some() {
                    self.kill_environment(h, DeathCause::DeathRegion);
                    self.events.push(Event::PropKill { prop: k as u32 });
                }
            }
            // A held soldier who died otherwise (or changed life) is let go.
            if let Some((h, _)) = self.props[k].held {
                if self.unit(h).is_none() {
                    self.props[k].held = None;
                }
            }
            let feet: Vec<Vec3> = self
                .units
                .iter()
                .filter(|u| u.alive)
                .map(|u| u.soldier.pos)
                .collect();
            let near = |c: Vec3, r: f32| feet.iter().any(|p| (*p - c).length_squared() < r * r);
            if let Some(clip) = self.props[k].trigger(near) {
                self.events.push(Event::PropTriggered {
                    prop: k as u32,
                    clip,
                });
            }
            let units = &self.units;
            let soldier = |c: Vec3, r: f32| {
                units.iter().enumerate().find_map(|(i, u)| {
                    (u.alive && u.mount.is_none() && (u.soldier.pos - c).length_squared() < r * r)
                        .then_some(UnitHandle {
                            index: i as u32,
                            gen: u.gen,
                        })
                })
            };
            self.props[k].catch(soldier);
            self.props[k].advance(dt);
            let p = &self.props[k];
            for (b, (q, t)) in p.setup.bodies.iter().zip(p.poses.iter()) {
                self.collision
                    .move_body(b.body, Mat4::from_rotation_translation(*q, *t));
            }
        }
    }

    /// Where a soldier held by an animated prop hangs this tick, if he is.
    pub fn prop_hold(&self, i: usize) -> Option<Vec3> {
        let u = self.units.get(i)?;
        self.props.iter().find_map(|p| {
            p.held_at()
                .filter(|(h, _)| h.index as usize == i && h.gen == u.gen)
                .map(|(_, at)| at)
        })
    }

    /// A held soldier's pad is empty: he cannot move, aim or fire.
    pub(super) fn prop_locks(&mut self, pads: &mut [super::PadState]) {
        if self.props.is_empty() {
            return;
        }
        for (i, pad) in pads.iter_mut().enumerate() {
            if self.prop_hold(i).is_some() {
                *pad = super::PadState::default();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::animated_prop::{AnimatedProp, Catch, Setup, Trigger};
    use crate::sim::tests::flat_ground;
    use crate::sim::{Event, PadState, Sim};
    use crate::unit::UnitSpec;
    use glam::Vec3;

    /// A rancor-like prop: idle 30 frames, a 2 s grab started within 2 m of
    /// the origin, catching within 1.5 m of its hand (1.2 m up, 1 m ahead)
    /// in the first second. A player standing there is caught, held under
    /// the hand, and dies at the grab's end; one standing away is not, and
    /// the prop's state is in the hash.
    #[test]
    fn a_player_by_the_prop_is_caught_held_and_killed() {
        let make = |at: Vec3| {
            let mut s = Sim::empty(4);
            s.heightfield = Some(flat_ground());
            let hand = Vec3::new(0.0, 1.2, 1.0);
            s.props = vec![AnimatedProp::new(
                Setup {
                    clips: vec![30, 60],
                    triggers: vec![Trigger {
                        clip: 1,
                        center: Vec3::ZERO,
                        radius: 2.0,
                    }],
                    catches: vec![Catch {
                        clip: 1,
                        radius: 1.5,
                        time: 1.0,
                        track: vec![vec![hand + Vec3::Y * 4.0; 30], vec![hand; 60]],
                    }],
                    ..Setup::default()
                },
                0,
            )];
            s.spawn(&UnitSpec::player_in_slot(0, 0, at)).unwrap();
            s
        };
        let mut near = make(Vec3::new(0.0, 0.0, 0.5));
        let mut far = make(Vec3::new(0.0, 0.0, 10.0));
        let empty = {
            let mut s = Sim::empty(4);
            s.heightfield = Some(flat_ground());
            s.spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(0.0, 0.0, 0.5)))
                .unwrap();
            s
        };
        assert_ne!(near.hash(), empty.hash(), "a map with a prop hashes it");
        let mut held_ticks = 0;
        let mut killed_at = None;
        for tick in 0..120 {
            near.step(&[PadState::default()]);
            far.step(&[PadState::default()]);
            if near.prop_hold(0).is_some() {
                held_ticks += 1;
                // Held: his pad is dead and he hangs under the hand.
                let p = near.units[0].soldier.pos;
                assert!((p.z - 1.0).abs() < 0.3, "{p}");
            }
            if near
                .events
                .iter()
                .any(|e| matches!(e, Event::PropKill { prop: 0 }))
            {
                killed_at = Some(tick);
                break;
            }
        }
        assert!(held_ticks > 30, "{held_ticks}");
        let t = killed_at.expect("killed at the grab's end");
        assert!((60..=64).contains(&t), "{t}");
        assert!(!near.units[0].alive);
        assert_eq!(near.units[0].deaths, 1);
        assert!(far.units[0].alive);
        assert_eq!(far.props[0].anim, Some(0));
    }
}
