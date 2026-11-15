//! OURS (issue #164, the maintainer's LAN playtest of 2026-11-14:
//! "emplaced turrets get controller aim assist out to long range"): aim
//! assist for a human gunner on a controller in a building turret.
//!
//! The PS2's stick magnetism (`core::aim_assist`, BF1 FUN_0025fea8) runs
//! for any controllable whose weapon authors a LockOnRange, but BF1's
//! building guns author none, BF2's mostly take no soldiers, and the port's
//! step 2 gave every non-flyer seat no magnet at all. Here a building
//! turret's gunner takes a magnet along the seat's camera ray with the
//! seat's first weapon ([`seat_magnet`]: out to the round's own reach, its
//! own cone or 1°; the weapon's Target* kinds plus soldiers, animals and
//! droids, plus vehicles when the round hurts them; enemies only, so an ally
//! or a neutral can be the pick but never the magnet; the weapon's authored
//! Target* bits, which the seat dwell lock and the target hints read, are
//! left as they are).
//!
//! The assist is a turret's own, not the soldier's. A turret's stick sets a
//! TARGET rate that the turn law slews toward through its filter
//! ([`gun::Turn::step`]), so the soldier's one-tick pull (an angle closed in
//! a tick, `aim_assist::apply` term 2) would overshoot and ring. The seat's:
//!
//! 1. the PS2's sticky slowdown on both sticks, blended by the intensity;
//! 2. while the pull's timer is live (the soldier's term-2 timer: armed,
//!    refilled by FIRE, 1 s to run out), a target rate of the lead point's
//!    own turn rate about the eye plus [`SEAT_PULL_DAMP`] x the turn filter
//!    x the yaw error left, under the soldier's weight (1 - |yaw push|,
//!    floor `W_FLOOR`) and term 2's GAIN x timer. That gain keeps the
//!    filter's loop at or past critical damping (no overshoot) for either
//!    slew law. The correction is capped at the error over a tick, so a
//!    stiff filter (one that takes the target rate within the tick, from
//!    about 200 at full gain) closes the error in the tick and never
//!    crosses it. Pitch is only slowed, as on foot.
//!
//! The lead is the soldier's (`Sim::assist_lead_point`) from the turret's
//! muzzle with the seat weapon's own round. The yaw error is the barrel's:
//! the turret's aimer lays the barrel on what the camera ray meets
//! (`Sim::aim_point`), so the camera is steered onto the point on the
//! muzzle's line through the lead that lies as far out as the ray now
//! meets, not onto the lead itself. Everything fades on one curve from
//! [`TURRET_ASSIST_FULL`] of the reach to nothing at the reach
//! (`aim_assist::sniper_range_scale`). Bots never come here (step 2 runs
//! only for player slots), and a mouse-aimed pad is never pulled.
//! measure: controller turret play on a crossing runner at half and three
//! quarters of the gun's reach, unzoomed and zoomed, against the feel of
//! the soldier's assist.

use super::input::{AimTarget, Assisted, LeadAsk, PadState};
use super::state::Sim;
use crate::aim_assist::{self, GAIN, MAGNET_TIMER_SECS, SLOWDOWN, W_FLOOR};
use crate::fmath::FloatExt;
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::gun::{self, Turn};
use crate::vehicle::{VehicleHandle, VehicleKind};
use crate::weapon::{WeaponClass, SNIPER_LOCK_ON_ANGLE, TARGET_SOLDIER_KINDS};
use glam::{Quat, Vec3};

/// OURS, PROVISIONAL: a turret's assist keeps full strength out to this
/// share of its round's reach, then fades to nothing at the reach.
/// measure: controller turret play at 50, 75 and 95 % of the reach.
pub const TURRET_ASSIST_FULL: f32 = 0.75;

/// OURS, PROVISIONAL: the turret pull's rate per radian of yaw error, as a
/// share of the turn law's filter (1/s). A first-order rate filter `f`
/// under a proportional gain `k` is critically damped at `k = f / 4`; 0.2
/// stays past that for either slew law. A filter stiff enough to reach the
/// target rate within a tick would make that gain close more than the error
/// in the tick (k x dt above 1: from a filter of about 200 at full gain), so
/// `Sim::seat_assisted` caps it at 1 / dt and the pull never overshoots.
/// measure: controller turret play on a crossing runner (no visible swing
/// past the target).
pub const SEAT_PULL_DAMP: f32 = 0.2;

/// What the turret magnet reads of its seat.
#[derive(Clone, Copy, Debug)]
pub(super) struct TurretAim {
    pub(super) vehicle: VehicleHandle,
    /// The seat's first weapon, its class index.
    pub(super) class: usize,
    /// The camera ray (zoomed: from the zoomed eye) and the fire point.
    pub(super) eye: Vec3,
    pub(super) view: Vec3,
    pub(super) muzzle: Vec3,
    /// The aimer's pivot, world: what the turret turns about.
    pub(super) pivot: Vec3,
    pub(super) rot: Quat,
    pub(super) vel: Vec3,
    /// The camera's (yaw, pitch), model frame.
    pub(super) look: (f32, f32),
    /// The turn law as it runs this tick (zoom-divided, `Sim::seat_turn`).
    pub(super) turn: Turn,
}

/// A turret magnet's reach (m), full-strength distance (m), cone slope
/// (rad) and kinds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SeatMagnet {
    pub(super) reach: f32,
    pub(super) full: f32,
    pub(super) angle: f32,
    pub(super) kinds: u32,
}

/// The turret magnet of a seat weapon of class `c` (the module doc).
pub(super) fn seat_magnet(c: &WeaponClass) -> SeatMagnet {
    let reach = c.round_reach();
    let mut kinds = c.target_kind | TARGET_SOLDIER_KINDS;
    if c.ordnance.scales[3] > 0.0 {
        kinds |= 1 << 3;
    }
    SeatMagnet {
        reach,
        full: TURRET_ASSIST_FULL * reach,
        angle: if c.lock_on_angle > 0.0 {
            c.lock_on_angle
        } else {
            SNIPER_LOCK_ON_ANGLE
        },
        kinds,
    }
}

impl Sim {
    /// `h`'s building turret seat, when `h` is a human gunner in one with a
    /// weapon; `None` on foot, in any other seat and for a bot.
    pub(super) fn gunner_turret(&self, h: UnitHandle) -> Option<TurretAim> {
        let u = self.unit(h)?;
        if !matches!(u.controller, Controller::Player { .. }) {
            return None;
        }
        let m = u.mount?;
        let v = self.vehicle(m.vehicle)?;
        if v.kind != VehicleKind::Turret {
            return None;
        }
        let look = self.seat_look(h)?;
        let zoom = v
            .guns
            .get(usize::from(m.seat))
            .and_then(Option::as_ref)
            .and_then(|g| g.zoom);
        let turn = self.seat_turn(&look.gun, zoom, false)?;
        let class = look.gun.weapons.first()?.class;
        Some(TurretAim {
            vehicle: m.vehicle,
            class,
            eye: look.eye,
            view: look.view,
            muzzle: look.muzzle,
            pivot: v.pos + look.rot * look.gun.pivot,
            rot: look.rot,
            vel: v.vel,
            look: look.look,
            turn,
        })
    }

    /// [`Sim::seat_assisted`] on slot `slot`'s pad, keeping its timer and
    /// lead.
    pub(super) fn apply_seat_assist(
        &mut self,
        slot: usize,
        ph: UnitHandle,
        m: AimTarget,
        pad: &PadState,
        dt: f32,
    ) -> PadState {
        let Some((out, st, lead)) = self.seat_assisted(slot, ph, m, pad, dt) else {
            return *pad;
        };
        self.assist[slot] = st;
        if let Some(l) = lead {
            self.assist_lead[slot] = Some(l);
        }
        out
    }

    /// A building turret gunner's assisted pad toward magnet `m` (the
    /// module doc), with the slot's timer after it and its lead. `None`
    /// when the seat or the magnet is gone.
    pub(super) fn seat_assisted(
        &self,
        slot: usize,
        ph: UnitHandle,
        m: AimTarget,
        pad: &PadState,
        dt: f32,
    ) -> Option<Assisted> {
        let a = self.gunner_turret(ph)?;
        let t = self.aim_candidate(m)?;
        let c = &self.weapon_classes[a.class];
        let mag = seat_magnet(c);
        let profile = self.assist_profile(slot);
        let center = t.assist_ctr;
        let (lead, lead_vel, kept) = self.assist_lead_point(
            &LeadAsk {
                slot,
                m,
                center,
                vel: t.vel,
                from: a.muzzle,
                eye: a.eye,
                ord: &c.ordnance,
                lead_on: profile.lead_on,
                sniper: false,
            },
            dt,
        );
        // The timer runs as term 2's: refilled by FIRE when armed, then
        // run down while live, whatever the strength.
        let mut st = self.assist.get(slot).copied().unwrap_or_default();
        if profile.armed && pad.pressed(super::input::buttons::FIRE) {
            st.magnet_timer = MAGNET_TIMER_SECS;
        }
        let timer = st.magnet_timer;
        if st.magnet_timer > 0.0 {
            st.magnet_timer -= dt;
        }
        let intensity = profile.intensity
            * aim_assist::sniper_range_scale((center - a.eye).length(), mag.full, mag.reach);
        if intensity <= 0.0 {
            return Some((*pad, st, kept));
        }
        let (mut yaw, mut pitch) = (PadState::axis(pad.rx), PadState::axis(pad.ry));
        let w = (1.0 - yaw.abs().fmin(1.0)).fmax(W_FLOOR);
        let slow = 1.0 + (SLOWDOWN - 1.0) * intensity;
        yaw *= slow;
        pitch *= slow;
        if profile.armed && timer > 0.0 {
            let lead = lead.unwrap_or(center);
            // The barrel meets what the camera ray meets: steer the ray onto
            // the point of the muzzle's line through the lead as far out.
            let riders: Vec<UnitHandle> = self
                .vehicle(a.vehicle)
                .map(|v| v.seats.iter().filter_map(|s| s.occupant).collect())
                .unwrap_or_default();
            let meets = self.aim_point(a.eye, a.view, &riders, a.vehicle);
            let to_lead = lead - a.muzzle;
            let far = (meets - a.muzzle).length().fmax(to_lead.length());
            let at = a.muzzle + to_lead.normalize_or_zero() * far;
            let inv = a.rot.inverse();
            let want = gun::angles(inv * (at - a.eye)).0;
            let err = gun::wrap(want - a.look.0);
            // The lead point's own turn about the pivot the turret turns on
            // (model yaw, rad/s), less the turret's own motion.
            let rel = inv * (lead - a.pivot);
            let vel = inv * (lead_vel.unwrap_or(t.vel) - a.vel);
            let flat = (rel.x * rel.x + rel.z * rel.z).fmax(1e-6);
            let follow = (rel.z * vel.x - rel.x * vel.z) / flat;
            // The correction closes at most the error left in this tick,
            // whatever the filter (`SEAT_PULL_DAMP`).
            let gain = (GAIN * timer.fmin(1.0) * SEAT_PULL_DAMP * a.turn.yaw_filter)
                .fmin(1.0 / dt.fmax(1e-6));
            let rate = w * (follow + gain * err);
            // The turn law's yaw is `-stick x speed`; the assist turns no
            // faster than the turret itself.
            let push = (-rate / a.turn.yaw_speed.fmax(1e-3)).clamp(-1.0, 1.0);
            yaw += push * intensity;
        }
        let out = PadState {
            rx: PadState::quantize(yaw.clamp(-1.0, 1.0)),
            ry: PadState::quantize(pitch),
            ..*pad
        };
        Some((out, st, kept))
    }
}

#[cfg(test)]
mod tests;
