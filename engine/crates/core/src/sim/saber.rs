//! The authored BF2 combo controls input, energy and the actual animated
//! blade sweeps. Ordinary weapons retain their existing shot path.

use super::{buttons, PadState, Sim};
use crate::combo::{self, ComboInput};
use crate::fmath;
use crate::soldier::{SoldierConstants, Stance};
use crate::TICK_DT;
use glam::Vec3;

mod deflect;
mod hits;
mod movement;

fn posture(stance: Stance) -> u16 {
    match stance {
        Stance::Stand => combo::STAND,
        Stance::Crouch => combo::POSTURE_CROUCH,
        Stance::Prone => combo::PRONE,
        Stance::Sprint => combo::POSTURE_SPRINT,
        Stance::Jump | Stance::Fall => combo::POSTURE_JUMP,
        Stance::Jet | Stance::Hover => combo::JET,
        Stance::Roll => combo::ROLL,
        _ => combo::TUMBLE,
    }
}

fn scaled(value: i16, factor: f32) -> i16 {
    (f32::from(value) * factor).clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

impl Sim {
    /// Called before movement, after both human and bot pads exist.
    pub(super) fn saber_inputs(&mut self, pads: &mut [PadState]) {
        for (i, pad) in pads.iter_mut().enumerate() {
            let wc = self.units[i].weapon_class;
            if self.weapon_classes[wc].saber.is_none()
                || !self.units[i].alive
                || self.units[i].mount.is_some()
                || self.units[i].choked.is_some()
            {
                continue;
            }
            let mut state = self.units[i].weapon.melee.take().unwrap_or_default();
            self.melee_lcg_active = true;
            state.movement_weight = (state.movement_weight
                + if state.movement_active {
                    6.6666665 * TICK_DT
                } else {
                    -6.6666665 * TICK_DT
                })
            .clamp(0.0, 1.0);
            state.previous_position = self.units[i].soldier.pos;
            state.previous_yaw = self.units[i].soldier.yaw;
            state.previous_pitch = self.units[i].soldier.pitch;
            let u = &self.units[i];
            let class = self.weapon_classes[wc].saber.as_ref().unwrap();
            let forward = PadState::axis(pad.ly);
            let strafe = PadState::axis(pad.lx);
            state.remember_thrust(&class.combo, forward, strafe);
            let thrust = (forward * forward + strafe * strafe).sqrt().min(1.0);
            let angle = fmath::atan2(-strafe, forward).to_degrees();
            let (sy, cy) = fmath::sin_cos(u.soldier.yaw);
            let mut buttons = 0;
            for (source, target) in [
                (buttons::FIRE, combo::FIRE),
                (buttons::JUMP, combo::JUMP),
                (buttons::ROLL_STANCE, combo::CROUCH),
                (buttons::SPRINT, combo::SPRINT),
                (buttons::RELOAD, combo::RELOAD),
            ] {
                if pad.pressed(source) {
                    buttons |= target;
                }
            }
            // R2 blocks on the PS2. The port swaps shoulder pairs: its
            // primary-switch shoulder blocks here. [ours] Reload also blocks.
            if pad.pressed(buttons::RELOAD) || pad.pressed(buttons::SWITCH_WEAPON) {
                buttons |= combo::FIRE_SECONDARY;
                pad.buttons &= !buttons::SWITCH_WEAPON;
            }
            if pad.pressed(buttons::SECONDARY_FIRE) {
                buttons &= !combo::FIRE;
            }
            let normal_restore_rate = if thrust > 0.0 {
                u.consts.energy_restore
            } else {
                u.consts.energy_restore_idle
            };

            // The original ground mover resolves sprint before attack selection.
            // Sample that transition so the selected combo can drive this move.
            let stance = if u.is_player && !u.soldier.look_frozen() {
                u.soldier
                    .sprint_stance(pad, &u.consts, self.ground_rules, forward, strafe)
            } else {
                u.soldier.stance
            };
            let input = ComboInput {
                buttons,
                posture: posture(stance),
                energy: u.soldier.energy,
                energy_depleted: u.soldier.energy_depleted,
                normal_restore_rate,
                thrust,
                thrust_angle: angle,
                forward,
                strafe: -strafe,
                retained_thrust: state.retained_thrust,
                velocity_y: u.soldier.vel.y,
                velocity_xz: Vec3::new(u.soldier.vel.x, 0.0, u.soldier.vel.z).length(),
                velocity_angle: fmath::atan2(
                    -cy * u.soldier.vel.x + sy * u.soldier.vel.z,
                    -sy * u.soldier.vel.x - cy * u.soldier.vel.z,
                )
                .to_degrees(),
                time_in_posture: if stance == u.soldier.stance {
                    u.soldier.time_in_stance
                } else {
                    0.0
                },
                animation_velocity: [0.0; 2],
            };
            // The thrown saber (FUN_0033cda8, B2:333212-333249): while it
            // flies the program holds IDLE and runs no transition; caught,
            // it enters CATCH_SABRE when standing or crouched (the posture
            // field 0 or 1).
            let flying = u.weapon.saber_flight.is_some()
                || u.held.iter().any(|w| w.state.saber_flight.is_some());
            if std::mem::take(&mut state.catch_pending)
                && !flying
                && matches!(u.soldier.stance, Stance::Stand | Stance::Crouch)
            {
                state.runtime.force(&class.combo, "CATCH_SABRE");
            }
            if flying && !state.runtime.in_state(&class.combo, "IDLE") {
                state.runtime.force(&class.combo, "IDLE");
            }
            let input = ComboInput {
                buttons: if flying { 0 } else { input.buttons },
                ..input
            };
            let out = state.runtime.step_with_animation(
                &class.combo,
                &class.state_seconds,
                input,
                TICK_DT,
                |name, _| {
                    name.and_then(|name| class.clips.iter().find(|c| c.name == name))
                        .map_or([0.0; 2], |clip| clip.root_speed)
                },
            );
            let sound = out.sound.map(crate::hero_weapon::sound_hash);
            let blast = out
                .play_explosion
                .then(|| self.weapon_classes[wc].ordnance.explosion.clone())
                .flatten();
            state.animation = out.animation.map(str::to_owned);
            state.animation_loop = out.animation_loop;
            state.hidden_edges = out.hidden_edges;
            state.deflect = out.deflect;
            state.advance_deflect(out.entered, out.deflect.is_some(), TICK_DT);
            state.aligned_to_thrust = out.aligned_to_thrust;
            state.movement = out.movement;
            state.movement_active = out.movement_active;
            state.gravity = out.gravity;
            if out.entered {
                state.gravity_done = false;
            }
            state.restore_rate = out.energy_restore_rate;
            state.turn_factor = out.turn_factor;
            state.input_lock = out.input_lock;
            if out.clear_hits {
                state.hit.clear();
            } else {
                for cache in &mut state.hit {
                    cache.end += out.cache_shift;
                }
            }
            for attack in &out.attacks {
                state.ensure_hit_cache(attack);
            }
            state.attacks = out.attacks;
            let u = &mut self.units[i];
            u.soldier.pay_energy(out.energy_cost);
            let lock = out.input_lock;
            for (mask, source) in [
                (combo::JUMP, buttons::JUMP),
                (combo::CROUCH, buttons::ROLL_STANCE),
                (combo::SPRINT, buttons::SPRINT),
            ] {
                if lock & mask != 0 {
                    pad.buttons &= !source;
                }
            }
            pad.ly = if lock & combo::THRUST != 0 {
                0
            } else {
                scaled(pad.ly, out.thrust_factor)
            };
            pad.lx = if lock & combo::STRAFE != 0 {
                0
            } else {
                scaled(pad.lx, out.strafe_factor)
            };
            // [ours] (the maintainer, 2026-10-30: "Make sure the hero can
            // rotate around while attacking and lightsaber throwing"): the
            // PS2's lock holds the look of a bot only; see `saber_look_locked`.
            if lock & combo::TURN != 0 && !u.is_human() {
                pad.rx = 0;
                pad.ry = 0;
                pad.mx = 0;
                pad.my = 0;
            }
            u.weapon.melee = Some(state);
            if let Some(blast) = blast {
                let pos = self.units[i].soldier.pos + Vec3::Y * 0.2;
                self.explode(&blast, pos, Some(self.handle(i)), Some(self.units[i].team));
                self.events.push(super::Event::WeaponBlast {
                    pos,
                    weapon_class: wc as u32,
                });
            }
            if let Some(sound_hash) = sound {
                self.events.push(super::Event::ComboSound {
                    unit: self.handle(i),
                    weapon_class: wc as u32,
                    sound_hash,
                });
            }
        }
    }

    /// Does unit `i`'s combo lock its look this tick ("InputLock All" holds
    /// Turn: [`Sim::saber_inputs`] zeroes the look stick and the mouse)?
    /// The window's look preview asks, so it draws no turn the tick will
    /// not take (playtest 2026-10-30: the camera shook during swings).
    /// [ours] A human is never locked: the PS2's rule holds for bots.
    pub(super) fn saber_look_locked(&self, i: usize) -> bool {
        let u = &self.units[i];
        !u.is_human()
            && self.weapon_classes[u.weapon_class].saber.is_some()
            && u.alive
            && u.mount.is_none()
            && u.choked.is_none()
            && u.weapon
                .melee
                .as_ref()
                .is_some_and(|m| m.input_lock & combo::TURN != 0)
    }

    pub(super) fn saber_constants(&self, i: usize, constants: &mut SoldierConstants) {
        let unit = &self.units[i];
        if !unit.alive || unit.mount.is_some() || unit.soldier.no_fire() || unit.choked.is_some() {
            return;
        }
        if let Some(state) = &self.units[i].weapon.melee {
            constants.saber = true;
            if let Some(rate) = state.restore_rate {
                constants.energy_restore = rate;
                constants.energy_restore_idle = rate;
            }
            constants.motion_target = state.movement.map(|movement| crate::soldier::MotionTarget {
                forward: movement.velocity[1],
                strafe: -movement.velocity[0],
                weight: state.movement_weight,
            });
            // [ours] A human turns at full speed through a combo's TurnFactor,
            // as through its InputLock; bots keep the PS2's slowed turn.
            if !unit.is_human() {
                constants.max_turn_speed *= state.turn_factor;
            }
        }
    }
}

#[cfg(test)]
mod tests;
