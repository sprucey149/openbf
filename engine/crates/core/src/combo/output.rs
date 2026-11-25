//! Numeric combat/movement outputs; the sim owns their application.

use super::*;

#[derive(Debug, Clone, Copy)]
pub struct Deflect {
    pub min_angle: f32,
    pub max_angle: f32,
    pub energy_cost: f32,
}

impl Deflect {
    pub fn covers(&self, angle: f32) -> bool {
        conditions::in_angle(angle, self.min_angle, self.max_angle)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttackWindow {
    pub index: usize,
    pub id: u32,
    /// Stable identity of this authored state/attack record.
    pub record: u64,
    /// Absolute weapon clock at the original damage-window end.
    pub cache_end: f32,
    pub edge: u8,
    pub damage: Option<f32>,
    pub length: Option<f32>,
    pub length_from_edge: bool,
    pub width: Option<f32>,
    pub width_from_edge: bool,
    pub push: f32,
    pub play_explosion: bool,
}

fn attack_damage(attack: &Field) -> Option<f32> {
    attack
        .find(K::Damage)
        .and_then(|field| number(field, 0))
        .map(|damage| quantize(damage, 1.0, 0, 4095))
}

/// Damage for a physical VR swing comes from the opening regular cut.
/// Some combo programs use other state names, so fall back to a direct cut
/// from those programs in authored order.
pub(crate) fn tracked_swing_damage(combo: &ComboDef) -> Option<f32> {
    let cut = |state: &Field| {
        state
            .all(K::Attack)
            .filter(|attack| attack.find(K::PlayExplosion).is_none())
            .filter_map(attack_damage)
            .find(|&damage| damage > 0.0)
    };
    combo
        .state("ATTACK1")
        .and_then(cut)
        .or_else(|| combo.states().find_map(cut))
}

#[derive(Debug, Clone, Copy)]
pub struct Movement {
    /// Local X/Z metres per second, including thrust/strafe terms.
    pub velocity: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
pub struct Gravity {
    pub factor: f32,
    pub target: Option<f32>,
    pub impulse: bool,
    pub zero_at_target: bool,
}

#[derive(Debug, Default)]
pub struct ComboOutput<'a> {
    pub state: usize,
    pub state_name: &'a str,
    pub state_time: f32,
    pub entered: bool,
    pub animation: Option<&'a str>,
    pub animation_loop: bool,
    pub restart_animation: bool,
    pub sound: Option<&'a str>,
    pub energy_cost: f32,
    /// Rebase existing absolute cache deadlines after native state exits.
    pub cache_shift: f32,
    pub clear_hits: bool,
    /// None means normal soldier recovery; Some replaces it.
    pub energy_restore_rate: Option<f32>,
    pub input_lock: u16,
    /// Zero is unrestricted; this constrains actions, not soldier pose.
    pub posture_mask: u16,
    pub turn_factor: f32,
    pub thrust_factor: f32,
    pub strafe_factor: f32,
    pub aligned_to_thrust: bool,
    pub hidden_edges: u8,
    pub movement: Option<Movement>,
    pub movement_active: bool,
    pub gravity: Option<Gravity>,
    pub attacks: Vec<AttackWindow>,
    pub deflect: Option<Deflect>,
    pub play_explosion: bool,
}

pub(super) fn hash(name: &str) -> u32 {
    name.bytes().fold(2_166_136_261u32, |h, c| {
        (h ^ u32::from(c | 0x20)).wrapping_mul(16_777_619)
    })
}

fn animated_velocity(field: Option<&Field>, animation_speed: f32) -> f32 {
    // 307a80 initializes both axes to packed50 + FromAnim. Explicit axis
    // records instead pack their authored number with *50/CVT (309178).
    let mut packed = field.map_or(50, |f| {
        (ee_mul(number(f, 0).unwrap_or(1.0), 50.0) as i32).clamp(-2048, 2047)
    });
    if field.is_none_or(|f| has_text(f, "FromAnim")) {
        // Preserve MIPS operand order at30a704..30a730: packed*.02 first,
        // then decoded animation speed*factor, then *50/CVT. Dividing by50
        // produces a different factor even for the implicit packed50.
        let factor = ee_mul(packed as f32, 0.02);
        packed = (ee_mul(ee_mul(animation_speed, factor), 50.0) as i32).clamp(-2048, 2047);
    }
    // Native getter33c4d0 also multiplies by.02 rather than dividing by50.
    ee_mul(packed as f32, 0.02)
}

pub(super) struct OutputFrame {
    pub clip: f32,
    pub input: ComboInput,
    pub before: f32,
    pub entered: bool,
    pub cache_origin: f32,
}

impl ComboRuntime {
    pub(super) fn output<'a>(
        &mut self,
        state: &'a Field,
        previous: Option<&'a Field>,
        frame: OutputFrame,
    ) -> ComboOutput<'a> {
        let OutputFrame {
            clip,
            input,
            before,
            entered,
            cache_origin,
        } = frame;
        let duration = duration(state, clip);
        let duration = if duration == 0.0 { self.time } else { duration };
        let scalar = |key| state.find(key).and_then(|f| number(f, 0));
        let mut out = ComboOutput {
            state: self.state,
            state_name: state.text(0).unwrap_or_default(),
            state_time: self.time,
            entered,
            turn_factor: quantize(scalar(K::TurnFactor).unwrap_or(1.0), 20.0, 0, 255),
            thrust_factor: quantize(scalar(K::ThrustFactor).unwrap_or(1.0), 20.0, 0, 255),
            strafe_factor: quantize(scalar(K::StrafeFactor).unwrap_or(1.0), 20.0, 0, 255),
            posture_mask: state
                .find(K::Posture)
                .map_or(0, |f| conditions::mask(f, 0, conditions::posture, 0)),
            restart_animation: state.find(K::RestartAnimation).is_some(),
            play_explosion: entered && state.find(K::PlayExplosion).is_some(),
            ..ComboOutput::default()
        };
        if let Some(animation) = state.find(K::Animation) {
            out.animation = animation.text(0);
            out.animation_loop = animation
                .find(K::Loop)
                .is_some_and(|f| number(f, 0) != Some(0.0));
        }
        if let Some(sound) = state.find(K::Sound) {
            let at = time(sound, 1, clip, duration);
            if !self.sound_played && self.time >= at {
                self.sound_played = true;
                out.sound = sound.text(0);
            }
        }
        if let Some(rate) = state.find(K::EnergyRestoreRate) {
            if let Some(value) = number(rate, 0) {
                out.energy_restore_rate = Some(
                    quantize(value, 5.0, -512, 511)
                        + if has_text(rate, "FromSoldier") {
                            input.normal_restore_rate
                        } else {
                            0.0
                        },
                );
            }
        }
        if let Some(lock) = state.find(K::InputLock) {
            let value = number(lock, 0).map(|_| time(lock, 0, clip, duration));
            if value.is_none_or(|t| t <= 0.0 || self.time < t) {
                // The PS2's InputLock is a seven-bit mask (B2 combo parser,
                // `All` = 0x7f: thrust, fire, secondary, jump, sprint,
                // crouch, reload): it never holds the turn, which the
                // state's TurnFactor slows instead.
                out.input_lock = conditions::mask(
                    lock,
                    usize::from(value.is_some()),
                    conditions::control,
                    ALL_CONTROLS,
                ) & !crate::combo::TURN;
            }
        }
        if let Some(aligned) = state.find(K::AlignedToThrust) {
            if number(aligned, 0) != Some(0.0) {
                out.aligned_to_thrust = true;
            }
        }
        if let Some(off) = state.find(K::TurnOffLightsaber) {
            out.hidden_edges = if off.args.is_empty() {
                u8::MAX
            } else {
                off.args
                    .iter()
                    .enumerate()
                    .filter_map(|(i, _)| number(off, i))
                    .filter(|i| *i >= 0.0 && *i < crate::hero_weapon::MAX_BLADE_EDGES as f32)
                    .fold(0, |m, i| m | (1 << i as u8))
            };
        }
        if let Some(movement) = state.find(K::AnimatedMove) {
            if !self.move_stopped {
                for group in movement
                    .children
                    .iter()
                    .filter(|f| matches!(f.keyword, K::Until | K::If | K::Or))
                {
                    let due = group
                        .find(K::Break)
                        .is_none_or(|f| self.time >= time(f, 0, clip, duration));
                    if due
                        && self
                            .group(group, self.time, duration, clip, input)
                            .is_some()
                    {
                        self.move_stopped = true;
                        break;
                    }
                }
            }
        }
        out.movement_active = state.find(K::AnimatedMove).is_some() && !self.move_stopped;
        let movement_state = if state.find(K::AnimatedMove).is_some() {
            Some(state)
        } else {
            previous
        };
        if let Some((movement_state, movement)) =
            movement_state.and_then(|s| s.find(K::AnimatedMove).map(|m| (s, m)))
        {
            let mul = |key| {
                quantize(
                    movement.find(key).and_then(|f| number(f, 0)).unwrap_or(0.0),
                    10.0,
                    0,
                    255,
                )
            };
            let mut velocity = glam::Vec3::new(
                animated_velocity(movement.find(K::VelocityX), input.animation_velocity[0]),
                0.0,
                animated_velocity(movement.find(K::VelocityZ), input.animation_velocity[1]),
            );
            let aligned = movement_state
                .find(K::AlignedToThrust)
                .is_some_and(|f| number(f, 0) != Some(0.0));
            if aligned {
                let axes = thrust::orient(
                    glam::Vec3::new(input.strafe, 0.0, input.forward),
                    input.retained_thrust,
                    input.forward,
                    true,
                );
                velocity.x += mul(K::VelocityFromStrafe) * axes.x;
                velocity.z += mul(K::VelocityFromThrust) * axes.z;
                velocity = thrust::orient(velocity, input.retained_thrust, input.forward, false);
            } else {
                velocity.x += mul(K::VelocityFromStrafe) * input.strafe;
                velocity.z += mul(K::VelocityFromThrust) * input.forward;
            }
            out.movement = Some(Movement {
                velocity: [velocity.x, velocity.z],
            });
        }

        let mut gravity = Gravity {
            factor: 1.0,
            target: None,
            impulse: false,
            zero_at_target: false,
        };
        let mut authored_gravity = false;
        for field in &state.children {
            if field.args.is_empty() {
                continue;
            }
            match field.keyword {
                K::Gravity => {
                    gravity.factor = quantize(number(field, 0).unwrap_or(0.0), 10.0, -127, 127);
                    gravity.impulse = false;
                    authored_gravity = true;
                }
                K::GravityVelocityTarget => {
                    gravity.target =
                        Some(quantize(number(field, 0).unwrap_or(0.0), 10.0, -2047, 2047));
                    if has_text(field, "Impulse") {
                        gravity.impulse = true;
                    }
                    gravity.zero_at_target = has_text(field, "ZeroGravity");
                    authored_gravity = true;
                }
                _ => {}
            }
        }
        if authored_gravity {
            out.gravity = Some(gravity);
        }
        for (index, attack) in state.all(K::Attack).enumerate() {
            let window = attack.find(K::DamageTime);
            let (begin, end) = window.map_or((0.2 * clip, 0.3 * clip), |f| {
                (time(f, 0, clip, duration), time(f, 1, clip, duration))
            });
            if self.time <= begin || before > end {
                continue;
            }
            let scalar = |key| attack.find(key).and_then(|f| number(f, 0));
            let id = attack.find(K::AttackId).and_then(|f| f.text(0));
            // PS2 0x3058c0 routes both property hashes to the same packed
            // length/FromEdge field. Authored order determines the last write.
            let length = attack
                .children
                .iter()
                .rev()
                .find(|f| matches!(f.keyword, K::DamageLength | K::DamageDistance));
            out.attacks.push(AttackWindow {
                index,
                id: id.map_or(0, |id| hash(id) & 0x000f_ffff),
                record: ((self.state as u64) << 32) | index as u64,
                cache_end: cache_origin + end,
                edge: scalar(K::Edge).unwrap_or(0.0).clamp(0.0, 15.0) as u8,
                damage: attack_damage(attack),
                length: length
                    .and_then(|f| number(f, 0))
                    .map(|n| quantize(n.abs(), 100.0, 0, 1023)),
                length_from_edge: length.is_some_and(|f| {
                    has_text(f, "FromEdge") || number(f, 0).is_some_and(|n| n < 0.0)
                }),
                width: scalar(K::DamageWidth).map(|n| quantize(n.abs(), 100.0, 0, 255)),
                width_from_edge: attack.find(K::DamageWidth).is_some_and(|f| {
                    has_text(f, "FromEdge") || number(f, 0).is_some_and(|n| n < 0.0)
                }),
                push: quantize(scalar(K::Push).unwrap_or(0.0), 10.0, 0, 255),
                play_explosion: attack.find(K::PlayExplosion).is_some(),
            });
        }
        if let Some(deflect) = state.find(K::Deflect) {
            let angles = deflect.find(K::DeflectAngle);
            out.deflect = Some(Deflect {
                min_angle: angles.and_then(|f| number(f, 0)).unwrap_or(-180.0),
                max_angle: angles.and_then(|f| number(f, 1)).unwrap_or(180.0),
                energy_cost: quantize(
                    deflect
                        .find(K::EnergyCost)
                        .and_then(|f| number(f, 0))
                        .unwrap_or(0.0),
                    10.0,
                    0,
                    1023,
                ),
            });
        }
        out
    }
}
