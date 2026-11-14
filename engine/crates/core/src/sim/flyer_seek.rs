use super::input::AimTarget;
use super::lock_on::LockTarget;
use super::state::Sim;
use crate::ai::flyer_drive::{clear_chord, draw};
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::flyer::{FlightPhase, FlyerAi, FlyerParams};
use crate::vehicle::{Vehicle, VehicleHandle, VehicleKind};
use glam::{Mat3, Quat, Vec3};

const SEEK_SALT: u32 = 0x5EEC;
const SEEK_RANGE: u32 = 101;
const SEEK_MASK: u32 = 0xF;
const SEEK_FROM: u32 = 8;
const SEEK_FAR: f32 = 400.0;
const SEEK_STEP: f32 = 150.0;
const CURVE_BEHIND: f32 = 30.0;
const CURVE_PAST: f32 = 200.0;
const RELOCATE_AFTER_SECS: f32 = 30.0;
const RELOCATE_PERIOD: f32 = 3.0;
const RELOCATE_MEAN: f32 = 7.0;
const RELOCATE_MIN: f32 = 50.0;
const RELOCATE_AHEAD_DOT: f32 = 0.25;
const RELOCATE_AHEAD_MIN: f32 = 500.0;
const RELOCATE_EDGE_PROBE: f32 = 300.0;
const RELOCATE_SIDE: f32 = 85.0;
const RELOCATE_NEAR: f32 = 85.0;
const RELOCATE_FAR: f32 = 700.0;
const RELOCATE_LANE: [f32; 2] = [-3.0, 4.0];
const RELOCATE_TRIES: u32 = 4;
const RELOCATE_CLEAR_RADII: f32 = 6.0;
const RELOCATE_RUN: f32 = 750.0;
const RELOCATE_BOOST_SECS: f32 = 6.0;

pub(crate) fn seeks_human(me: UnitHandle) -> bool {
    let roll = (draw(u64::from(me.gen), me, SEEK_SALT) * SEEK_RANGE as f32) as u32;
    roll & SEEK_MASK >= SEEK_FROM
}

fn straight_curve(from: Vec3, forward: Vec3, dir: Vec3, step: f32) -> [Vec3; 4] {
    [
        from - forward * CURVE_BEHIND,
        from,
        from + dir * step,
        from + dir * (step + CURVE_PAST),
    ]
}

impl Sim {
    /// The vehicle a human rides that is nearest `from` (a flyer only when
    /// `flyer`), with that human's team. Every player slot is asked in slot
    /// order and a tie keeps the lower slot, so with one human this is
    /// that human's ride (issue #168: the single player's rule, for any
    /// number of humans).
    fn human_ride(&self, from: Vec3, flyer: bool) -> Option<(&Vehicle, u8)> {
        let mut best: Option<(f32, &Vehicle, u8)> = None;
        for h in self.players.iter().flatten() {
            let Some(u) = self.unit(*h) else {
                continue;
            };
            let Some(v) = u.mount.and_then(|m| self.vehicle(m.vehicle)) else {
                continue;
            };
            if flyer && v.kind != VehicleKind::Flyer {
                continue;
            }
            let d2 = v.pos.distance_squared(from);
            if best.is_none_or(|(b, _, _)| d2 < b) {
                best = Some((d2, v, u.team));
            }
        }
        best.map(|(_, v, team)| (v, team))
    }

    pub(crate) fn flyer_seek_curve(
        &self,
        me: UnitHandle,
        v: &Vehicle,
        handle: VehicleHandle,
        p: &FlyerParams,
        ai: &mut FlyerAi,
    ) -> bool {
        if !p.bf2 || !p.follower.flyer_vs_flyer || !seeks_human(me) {
            return false;
        }
        let Some((human, _)) = self.human_ride(v.pos, false) else {
            return false;
        };
        let to = human.pos - v.pos;
        if to.length_squared() <= SEEK_FAR * SEEK_FAR {
            return false;
        }
        let dir = to.normalize();
        if !clear_chord(self, v.pos, v.pos + dir * SEEK_STEP, handle) {
            return false;
        }
        ai.curve = Some(straight_curve(v.pos, v.rot * Vec3::Z, dir, SEEK_STEP));
        ai.cursor = None;
        ai.goal = None;
        ai.t = 0.0;
        true
    }

    pub(crate) fn tick_flyer_seek(&mut self, dt: f32) {
        if self.human_ride(Vec3::ZERO, true).is_none() {
            return;
        }
        for idx in 0..self.vehicles.len() {
            let v = &self.vehicles[idx];
            if !v.alive || v.kind != VehicleKind::Flyer || v.flyer.phase != FlightPhase::Flying {
                continue;
            }
            let Some(me) = v.seats.first().and_then(|s| s.occupant) else {
                continue;
            };
            if self.unit(me).map(|u| u.controller) != Some(Controller::Ai) {
                continue;
            }
            let handle = VehicleHandle {
                index: idx as u32,
                gen: v.gen,
            };
            let left = v.flyer.ai.relocate_in - dt;
            let fires = left <= 0.0;
            let next = if fires {
                let r = draw(self.tick, me, SEEK_SALT ^ 1);
                (r - 0.5) * RELOCATE_PERIOD + RELOCATE_MEAN
            } else {
                left
            };
            self.vehicles[idx].flyer.ai.relocate_in = next;
            if fires {
                self.try_relocate(me, handle);
            }
        }
    }

    fn try_relocate(&mut self, me: UnitHandle, handle: VehicleHandle) {
        if self.tick as f32 * crate::TICK_DT < RELOCATE_AFTER_SECS {
            return;
        }
        let Some(v) = self.vehicle(handle) else {
            return;
        };
        let Some((human_pos, human_rot, human_team)) = self
            .human_ride(v.pos, true)
            .map(|(h, team)| (h.pos, h.rot, team))
        else {
            return;
        };
        let Some(p) = self
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
        else {
            return;
        };
        if !p.bf2 || !p.follower.flyer_vs_flyer || p.follower.formations == 0 || !seeks_human(me) {
            return;
        }
        let [right, up, forward] = [Vec3::X, Vec3::Y, Vec3::Z].map(|a| human_rot * a);
        let probe = human_pos + forward * RELOCATE_EDGE_PROBE;
        if self.boundary.outside(probe.x, probe.z) {
            return;
        }
        let off = v.pos - human_pos;
        let dist2 = off.length_squared();
        if dist2 < RELOCATE_MIN * RELOCATE_MIN {
            return;
        }
        if forward.dot(off) > RELOCATE_AHEAD_DOT && dist2 < RELOCATE_AHEAD_MIN * RELOCATE_AHEAD_MIN
        {
            return;
        }
        let ai = v.flyer.ai;
        if ai.pursuit || ai.goal.is_some() {
            return;
        }
        if self.on_a_hint_list(LockTarget::Aim(AimTarget::Vehicle(handle))) {
            return;
        }
        let Some(team) = self.unit(me).map(|u| u.team) else {
            return;
        };
        let t = self.tick;
        if team == human_team && (draw(t, me, SEEK_SALT ^ 2) * 4.0) as u32 != 0 {
            return;
        }
        let along = draw(t, me, SEEK_SALT ^ 3) >= 0.5;
        let (lane, reach) = if along {
            (RELOCATE_LANE[1], RELOCATE_NEAR)
        } else {
            (RELOCATE_LANE[0], RELOCATE_FAR)
        };
        let clear_radius = RELOCATE_CLEAR_RADII * self.vehicle_blast_sphere(v).1;
        let spot = (0..RELOCATE_TRIES).find_map(|n| {
            let salt = SEEK_SALT ^ (0x10 + n * 2);
            let rx = (draw(t, me, salt) - 0.5) * 2.0;
            let ry = (draw(t, me, salt + 1) - 0.5) * 2.0;
            let side = (right * rx + up * ry).try_normalize()?;
            let at = human_pos + side * RELOCATE_SIDE + forward * reach;
            [Vec3::X, Vec3::Y, Vec3::Z]
                .iter()
                .all(|&a| clear_chord(self, at - a * clear_radius, at + a * clear_radius, handle))
                .then_some((at, side))
        });
        let Some((at, side)) = spot else {
            return;
        };
        let aim = (forward * lane - side).normalize_or_zero();
        let run = clear_chord(self, at, at + aim * RELOCATE_RUN, handle);
        let heading = if along { forward } else { -forward };
        let world_up = human_rot * Vec3::Y;
        let right_new = world_up.cross(heading).normalize_or_zero();
        let up_new = heading.cross(right_new);
        let rot = Quat::from_mat3(&Mat3::from_cols(right_new, up_new, heading));
        let speed = self.vehicle(handle).map_or(0.0, |v| v.vel.length());
        let Some(v) = self.vehicle_mut(handle) else {
            return;
        };
        v.pos = at;
        v.rot = rot;
        v.vel = heading * speed;
        let ai = &mut v.flyer.ai;
        ai.cursor = None;
        ai.goal = None;
        ai.t = 0.0;
        ai.curve = run.then(|| straight_curve(at, heading, aim, RELOCATE_RUN));
        ai.escape = if along { RELOCATE_BOOST_SECS } else { 0.0 };
    }
}
