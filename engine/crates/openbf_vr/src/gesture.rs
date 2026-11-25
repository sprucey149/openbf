//! Preserve controller gestures between fixed simulation ticks.

use super::actions::{Input, Pose};
use super::rig::{self, World};
use core::sim::{buttons, TrackedHand, TrackedThrow};
use core::unit::UnitHandle;
use extensions::runtime::SimView;
use glam::Vec3;

/// OURS: scale controller motion for the game's throw distances.
/// measure: short tosses, drops and overhand throws with a headset.
const THROW_GAIN: f32 = 2.4;

#[derive(Default)]
pub(super) struct SaberSamples {
    pub hands: [Option<TrackedHand>; 2],
    pub waypoints: [Option<TrackedHand>; 2],
    epoch: u16,
    pending: bool,
}

impl SaberSamples {
    pub fn sample(&mut self, epoch: &mut u16, hands: [Option<TrackedHand>; 2]) {
        if self.epoch == *epoch
            && self
                .hands
                .iter()
                .zip(hands)
                .any(|(last, now)| last.is_some() && now.is_none())
        {
            // A grip can disappear and recover wholly between two sim ticks.
            *epoch = epoch.wrapping_add(1);
        }
        self.waypoints = if self.epoch == *epoch && self.pending {
            std::array::from_fn(|hand| {
                let now = hands[hand]?;
                self.hands[hand]
                    .filter(|&last| last != now)
                    .or(self.waypoints[hand])
            })
        } else {
            [None; 2]
        };
        self.epoch = *epoch;
        self.hands = hands;
        self.pending = true;
    }

    pub fn consumed(&mut self) {
        self.waypoints = [None; 2];
        self.pending = false;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Grenade {
    unit: UnitHandle,
    class: usize,
    slot: u8,
}

#[derive(Default)]
pub(super) struct GrenadeGesture {
    held: Option<Grenade>,
    release: Option<(Grenade, Pose, Vec3)>,
    previous: Option<(i64, Vec3)>,
    wait_for_up: bool,
}

impl GrenadeGesture {
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    pub fn reset(&mut self) {
        *self = Self {
            wait_for_up: true,
            ..Self::default()
        };
    }

    pub fn sample(
        &mut self,
        view: Option<&SimView<'_>>,
        input: Input,
        time_ns: i64,
        ready_to_use_left: bool,
    ) {
        let down = input.buttons & buttons::SECONDARY_FIRE != 0;
        let selected = view
            .filter(|_| ready_to_use_left && !input.pause)
            .and_then(|view| {
                rig::infantry(view)?;
                let (class, slot, ready) = view.sim.tracked_grenade_slot(view.player)?;
                ready.then_some(Grenade {
                    unit: view.player,
                    class,
                    slot,
                })
            });
        let Some((selected, pose)) = selected.zip(input.hands[0].grip) else {
            self.held = None;
            self.release = None;
            self.previous = None;
            self.wait_for_up |= down;
            return;
        };
        if self.held.is_some_and(|held| held != selected) {
            self.held = None;
            self.wait_for_up |= down;
        }
        if self.release.is_some_and(|(key, _, _)| key != selected) {
            self.release = None;
        }
        let previous = self.previous.replace((time_ns, pose.position));
        if !down {
            self.wait_for_up = false;
            if let Some(grenade) = self.held.take() {
                let velocity = input
                    .left_velocity
                    .or_else(|| {
                        let (earlier, at) = previous?;
                        let ns = time_ns.checked_sub(earlier)?;
                        // Samples farther apart than this are a tracking gap.
                        (0 < ns && ns <= 100_000_000)
                            .then(|| (pose.position - at) / (ns as f32 * 1e-9))
                    })
                    .unwrap_or(Vec3::ZERO);
                self.release = Some((grenade, pose, velocity * THROW_GAIN));
            }
        } else if !self.wait_for_up && self.release.is_none() {
            self.held = Some(selected);
        }
    }

    pub fn held_class(&self, view: &SimView<'_>) -> Option<usize> {
        let held = self.held.filter(|held| held.unit == view.player)?;
        let (class, slot, true) = view.sim.tracked_grenade_slot(view.player)? else {
            return None;
        };
        (class == held.class && slot == held.slot).then_some(class)
    }

    pub fn release(&self, view: &SimView<'_>, world: World) -> Option<TrackedThrow> {
        let (key, pose, velocity) = self.release.filter(|(key, _, _)| key.unit == view.player)?;
        let feet = rig::infantry(view)?.soldier.pos;
        Some(TrackedThrow::new(
            key.class as u32,
            key.slot,
            world.pose(pose).position - feet,
            world.vector(velocity),
        ))
    }

    pub fn consumed(&mut self) {
        self.release = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Hand;
    use crate::rig::tests::{sim_view, test_sim};
    use core::sticky::StickyClass;
    use core::unit::HeldWeapon;
    use core::weapon::{OrdnanceClass, Thrown, WeaponClass, WeaponState};
    use glam::Quat;

    #[test]
    fn saber_waypoint_survives_unchanged_frames_and_clears_when_consumed_or_tracking_is_lost() {
        let mut samples = SaberSamples::default();
        let mut epoch = 1;
        let at = Some(TrackedHand::new(Vec3::ZERO, Quat::IDENTITY));
        let out = Some(TrackedHand::new(Vec3::X, Quat::IDENTITY));
        samples.sample(&mut epoch, [None, at]);
        samples.consumed();
        samples.sample(&mut epoch, [None, out]);
        samples.sample(&mut epoch, [None, at]);
        samples.sample(&mut epoch, [None, at]);
        assert_eq!(samples.waypoints, [None, out]);
        samples.consumed();
        assert_eq!(samples.waypoints, [None; 2]);
        samples.sample(&mut epoch, [None, out]);
        samples.sample(&mut epoch, [None; 2]);
        samples.sample(&mut epoch, [None, at]);
        assert_eq!(epoch, 2);
        assert_eq!(samples.waypoints, [None; 2]);
    }

    #[test]
    fn tracked_grenade_release_survives_frames_until_consumed_and_cancels_on_tracking_loss() {
        let (mut sim, player) = test_sim(7, Vec3::ZERO, None);
        let grenade = WeaponClass {
            rounds_per_clip: 1,
            ammo_per_shot: 1,
            thrown: Some(Thrown {
                shot_elevate: 0.0,
                hide_on_fire: true,
                hand: None,
                native_thermal_hand: false,
            }),
            ordnance: OrdnanceClass {
                sticky: Some(StickyClass::new(false)),
                ..Default::default()
            },
            ..WeaponClass::default()
        };
        let state = WeaponState::new(&grenade, true, 1);
        let class = sim.add_weapon_class(grenade);
        let unit = sim.unit_mut(player).unwrap();
        unit.cur1 = Some(0);
        unit.held = vec![HeldWeapon {
            slot: 1,
            channel: 1,
            class,
            state,
        }];
        let view = sim_view(&sim, player);
        let mut gesture = GrenadeGesture::default();
        let mut input = Input {
            buttons: buttons::SECONDARY_FIRE,
            hands: [
                Hand {
                    grip: Some(Pose {
                        position: Vec3::ZERO,
                        orientation: Quat::IDENTITY,
                    }),
                    aim: None,
                },
                Hand::default(),
            ],
            ..Default::default()
        };
        gesture.sample(Some(&view), input, 10_000_000, true);
        assert_eq!(gesture.held_class(&view), Some(class));
        assert!(gesture.release.is_none());
        input.hands[0].grip.as_mut().unwrap().position = Vec3::NEG_Z * 0.04;
        input.buttons = 0;
        gesture.sample(Some(&view), input, 20_000_000, true);
        let (_, pose, velocity) = gesture.release.unwrap();
        assert_eq!(pose.position, Vec3::NEG_Z * 0.04);
        assert!(velocity.abs_diff_eq(Vec3::NEG_Z * (4.0 * THROW_GAIN), 1e-5));
        gesture.sample(Some(&view), input, 22_000_000, true);
        assert!(gesture.release.is_some());
        assert!(!gesture.holding());
        gesture.consumed();
        assert!(gesture.release.is_none());

        input.buttons = buttons::SECONDARY_FIRE;
        gesture.sample(Some(&view), input, 30_000_000, true);
        assert!(gesture.holding());
        let grip = input.hands[0].grip.take();
        gesture.sample(Some(&view), input, 40_000_000, true);
        input.hands[0].grip = grip;
        input.buttons = 0;
        gesture.sample(Some(&view), input, 50_000_000, true);
        assert!(!gesture.holding());
        assert!(gesture.release.is_none());
    }
}
