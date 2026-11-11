//! A soldier standing on a flyer (BF2's platform, soldier s+0x480):
//! `analysis/re-flyer-deck.md`. A floor contact on a flyer records it
//! (`FUN_0023cfd0`); each update the flyer's motion carries him, a sudden loss
//! of its speed throws him, a tilt past the limit tumbles him, and half a
//! second off its floor drops him (`FUN_00240bf8`). The state setter adds its
//! velocity to a jump, a jet or a throw (`FUN_00247bf8`). BF1 has no such
//! carry (`trace:` its s+0x950 setter's caller).

use super::Sim;
use crate::collision::{
    Capsule, HullFilter, Layers, FOOT_RADIUS, FOOT_RING, FOOT_RING_DIRS, STEP_HEIGHT,
};
use crate::soldier::{GroundRules, Stance, FLOOR_CONTACT_GAP};
use crate::unit::Deck;
use crate::vehicle::{VehicleHandle, VehicleKind};
use glam::{Mat4, Vec3};

/// A loss of the flyer's speed in one update over this throws its rider
/// (0x24120c, `lui 0x4103 / ori 0x3333`).
pub const DECK_THROW_DROP: f32 = 8.2;
/// The throw's lift over the flyer's old heading (0x24124c, 0.3).
pub const DECK_THROW_LIFT: f32 = 0.3;
/// A flyer whose up axis has a height under this tumbles its rider
/// (0x2412dc, `lui 0x3f0c / ori 0xcccd`).
pub const DECK_TIP_UP_Y: f32 = 0.55;
/// Seconds a rider is off the flyer's floor before the carry lets him go
/// (`FUN_00240bf8`, the stance timer since the airborne latch).
pub const DECK_LET_GO: f32 = 0.5;

fn hull_pose(v: &crate::vehicle::Vehicle) -> Mat4 {
    Mat4::from_rotation_translation(v.rot, v.pos)
}

impl Sim {
    /// Before soldier `i`'s update: the carry, the throw, the tip and the
    /// let-go.
    pub(super) fn carry_on_deck(&mut self, i: usize) {
        let Some(deck) = self.units[i].deck else {
            return;
        };
        let live = self
            .vehicle(deck.vehicle)
            .map(|v| (hull_pose(v), v.vel, (v.rot * Vec3::Y).y));
        let s = &self.units[i].soldier;
        let upright = matches!(
            s.stance,
            Stance::Stand | Stance::Crouch | Stance::Prone | Stance::Sprint
        );
        let let_go = upright && !s.grounded && s.air_time.min(s.time_in_stance) >= DECK_LET_GO;
        let Some((pose, vel, up_y)) = live.filter(|_| !let_go && s.platform_vel.is_some()) else {
            self.leave_deck(i);
            return;
        };
        let delta = pose * deck.pose.inverse();
        let s = &mut self.units[i].soldier;
        s.pos = delta.transform_point3(s.pos);
        let (sy, cy) = crate::fmath::sin_cos(s.yaw);
        let f = delta.transform_vector3(Vec3::new(-sy, 0.0, -cy));
        if f.x != 0.0 || f.z != 0.0 {
            s.yaw = crate::fmath::atan2(-f.x, -f.z);
        }
        s.platform_vel = Some(vel);
        let (was, now) = (deck.vel.length(), vel.length());
        if now < was && was - now > DECK_THROW_DROP {
            let draw = self.lcg.unit();
            let dir = deck.vel / was + Vec3::Y * DECK_THROW_LIFT;
            self.units[i]
                .soldier
                .knock(dir, was - now, GroundRules::Bf2, draw);
        } else if up_y < DECK_TIP_UP_Y {
            self.units[i].soldier.tip_off();
        }
        if self.units[i].soldier.platform_vel.is_some() {
            self.units[i].deck = Some(Deck {
                vehicle: deck.vehicle,
                pose,
                vel,
            });
        } else {
            self.units[i].deck = None;
        }
    }

    /// After soldier `i`'s floor contact: a floor on a flyer is his deck; any
    /// other floor ends it; in the air he keeps what he had.
    pub(super) fn touch_deck(&mut self, i: usize) {
        let s = &self.units[i].soldier;
        if !s.grounded {
            if s.platform_vel.is_none() {
                self.units[i].deck = None;
            }
            return;
        }
        let deck = self.flyer_under(s.pos).and_then(|h| {
            let v = self.vehicle(h)?;
            Some(Deck {
                vehicle: h,
                pose: hull_pose(v),
                vel: v.vel,
            })
        });
        match deck {
            Some(d) => {
                self.units[i].soldier.platform_vel = Some(d.vel);
                self.units[i].deck = Some(d);
            }
            None => self.leave_deck(i),
        }
    }

    pub(super) fn leave_deck(&mut self, i: usize) {
        self.units[i].deck = None;
        self.units[i].soldier.platform_vel = None;
    }

    /// PROVISIONAL (ours): after a carried soldier's wall pass, his whole
    /// move from `from` is swept at the waist against the map's soldier
    /// bodies, and a crossing stops him one radius short of it. The PS2
    /// sweeps every change of his position (`FUN_00163d08`), and a moving
    /// ship meets him as a velocity response. The port's pass pushes his
    /// position out of the flyer's hull too, and that push could carry him
    /// across a hangar shield the pass held him at.
    /// trace: a PCSX2 rider held at a hangar shield while the ship's tail
    /// passes him. replace: the PS2's own vehicle response.
    pub(super) fn hold_at_walls(&mut self, i: usize, from: &Capsule) {
        let to = self.units[i].capsule();
        let waist = |c: &Capsule| {
            let s = c.stepped();
            s.base + Vec3::Y * (s.height * 0.5)
        };
        let (a, b) = (waist(from), waist(&to));
        let run = b - a;
        let len = run.length();
        if len <= 0.0 {
            return;
        }
        let Some(hit) =
            self.collision
                .ray_with(a, run / len, len, Layers::SOLDIER, HullFilter::Static)
        else {
            return;
        };
        let keep = ((hit.t - from.radius).max(0.0) / len).min(1.0);
        let back = (to.base - from.base) * (1.0 - keep);
        let s = &mut self.units[i].soldier;
        s.pos.x -= back.x;
        s.pos.z -= back.z;
    }

    /// The flyer whose hull is the floor the soldier at `feet` stands on: the
    /// foot's centre ray, else its ring, meeting a flyer hull at his feet.
    fn flyer_under(&self, feet: Vec3) -> Option<VehicleHandle> {
        let probe = feet.y + STEP_HEIGHT;
        let reach = STEP_HEIGHT + FLOOR_CONTACT_GAP;
        let lift = FOOT_RADIUS - (FOOT_RADIUS * FOOT_RADIUS - FOOT_RING * FOOT_RING).sqrt();
        let at = |dx: f32, dz: f32, sag: f32| {
            let (y, _, owner) = self.collision.ray_down_owned(
                Vec3::new(feet.x + dx, probe, feet.z + dz),
                reach + sag,
                HullFilter::Hulls,
            )?;
            let owner = owner?;
            ((y - sag - feet.y).abs() <= FLOOR_CONTACT_GAP
                && self
                    .vehicle(owner)
                    .is_some_and(|v| v.kind == VehicleKind::Flyer))
            .then_some(owner)
        };
        at(0.0, 0.0, 0.0).or_else(|| {
            FOOT_RING_DIRS
                .iter()
                .find_map(|&(dx, dz)| at(dx * FOOT_RING, dz * FOOT_RING, lift))
        })
    }
}

#[cfg(test)]
mod tests;
