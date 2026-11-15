//! The PS2 Ray area is a bounded angular slab, with retained Hold targets.

use super::*;
use crate::fmath::FloatExt;
use crate::unit::Controller;

/// B2 0032c408 case3; radius is the target's collision bounding sphere.
fn area_score(
    delta: Vec3,
    frame: (Vec3, Vec3, Vec3),
    radius: f32,
    power: &ForcePower,
    height: f32,
) -> Option<f32> {
    let (direction, right, up) = frame;
    let vertical = delta.dot(up).abs();
    let lateral = delta.dot(right).abs();
    let forward = delta.dot(direction);
    let ForceKind::Choke { angle, .. } = power.kind else {
        return None;
    };
    let (sine, cosine) = crate::fmath::sin_cos(angle);
    let distance = lateral * lateral + forward * forward;
    if vertical > radius + height * 0.5
        || lateral * cosine - radius > forward * sine
        || distance > (power.range + radius) * (power.range + radius)
    {
        return None;
    }
    let sideways = if lateral > 0.001 {
        let ratio = lateral / (forward * cosine + lateral * (1.0 - sine));
        ratio * ratio
    } else {
        0.0
    };
    let over = vertical - radius;
    let vertical_score = over.fmax(0.0) * over * (4.0 / (height * height)) * 2.2857144 - 1.2857144;
    Some(
        (distance / (power.range * power.range) * 2.2857144 - 1.2857144)
            .fmax(sideways)
            .fmax(vertical_score),
    )
}

impl Sim {
    fn choke_score(&self, i: usize, target: UnitHandle, power: &ForcePower) -> Option<f32> {
        let victim = self.unit(target)?;
        let ForceKind::Choke { height, .. } = power.kind else {
            return None;
        };
        if !self.force_can_target(i, target.index as usize, power) {
            return None;
        }
        let body_position = self.hero_body_prims(target.index as usize)?.pos;
        let caster = &self.units[i];
        let (from, frame) = if caster.is_human() {
            self.human_shot(i)
        } else {
            (caster.muzzle(), caster.aim_frame())
        };
        let (lift, radius) = super::super::blast::soldier_blast_sphere(victim.soldier.stance);
        let centre = body_position + Vec3::Y * lift;
        // B2 0x32c408 tests sight from the weapon fire point to the target's
        // collision-sphere centre, the same two points it scores. A feet-to-feet
        // ray lost standing targets behind any curb, step or rise (#173).
        if !crate::perception::los(self, from, centre) {
            return None;
        }
        let delta = centre - from;
        let score = area_score(delta, frame, radius, power, height)?;
        let preferred = match caster.controller {
            Controller::Player { slot } => self
                .reticle_pick
                .get(usize::from(slot))
                .and_then(|p| *p)
                .is_some_and(|(p, _)| p == crate::sim::input::AimTarget::Unit(target)),
            _ => caster
                .ai
                .target
                .is_some_and(|foe| foe.unit() == Some(target)),
        };
        Some(if preferred { score * 0.0001 } else { score })
    }

    fn choke_target(&self, i: usize, power: &ForcePower) -> Option<(UnitHandle, f32)> {
        if power.max_targets == 0 {
            return None;
        }
        let ForceKind::Choke { height, .. } = power.kind else {
            return None;
        };
        let caster = &self.units[i];
        let (from, (direction, _, _)) = if caster.is_human() {
            self.human_shot(i)
        } else {
            (caster.muzzle(), caster.aim_frame())
        };
        let cosine = power.cone_cos;
        let offset = if cosine <= 0.0 {
            0.0
        } else if cosine <= std::f32::consts::FRAC_1_SQRT_2 {
            power.range * cosine
        } else {
            power.range * 0.5 / cosine
        };
        let radius = ((if cosine <= 0.0 {
            power.range * power.range
        } else {
            offset * offset
        }) + height * height * 0.25)
            .sqrt();
        let centre = from + direction * offset;
        // [ours] Stable unit order for the native ten-object gather. Exact
        // spatial-list order in a crowded overlap remains a parity hold.
        (0..self.units.len())
            .filter(|&t| {
                let victim = &self.units[t];
                if t == i || !self.hero_body_active(t) {
                    return false;
                }
                let (lift, r) = super::super::blast::soldier_blast_sphere(victim.soldier.stance);
                self.hero_body_prims(t)
                    .is_some_and(|p| centre.distance(p.pos + Vec3::Y * lift) <= radius + r)
            })
            .take(10)
            .filter_map(|t| {
                let target = self.handle(t);
                self.choke_score(i, target, power)
                    .map(|score| (target, score))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Consume an area-effect weapon's input. Cannon emitters keep the
    /// ordinary shot cycle; only held choke needs a separate channel state.
    pub(in crate::sim) fn force_input(
        &mut self,
        i: usize,
        slot: Option<usize>,
        held: bool,
        blocked: bool,
    ) -> bool {
        let class = slot.map_or(self.units[i].weapon_class, |k| self.units[i].held[k].class);
        let Some(power) = self.weapon_classes[class].force.as_ref().cloned() else {
            return false;
        };
        let ForceKind::Choke {
            damage_per_second,
            falloff,
            ..
        } = power.kind
        else {
            return false;
        };
        let state = if let Some(k) = slot {
            &mut self.units[i].held[k].state
        } else {
            &mut self.units[i].weapon
        };
        let channel = state.force_channel.take();
        let starting = channel.is_none();
        let mut channel = channel.unwrap_or_default();
        let was_held = state.fire_cmd > 0;
        state.fire_cmd = if held {
            state.fire_cmd.saturating_add(1).max(1)
        } else if was_held {
            -1
        } else {
            0
        };
        if blocked || !held || self.units[i].soldier.energy_depleted {
            let state = if let Some(k) = slot {
                &mut self.units[i].held[k].state
            } else {
                &mut self.units[i].weapon
            };
            state.state = FireState::Idle;
            return true;
        }
        if starting && !self.units[i].soldier.pay_energy(power.energy) {
            return true;
        }
        let warmup = self.weapon_classes[class].initial_salvo_delay;
        channel.elapsed += TICK_DT;
        if channel.elapsed >= warmup {
            if !channel.acquired {
                channel.acquired = true;
                if let Some((target, score)) = self.choke_target(i, &power) {
                    channel.target = Some(target);
                    channel.target_score = score;
                }
                channel.retarget = 0.0;
            } else {
                channel.retarget += TICK_DT;
                if channel.retarget > 0.3333 {
                    channel.retarget = 0.0;
                    channel.target = channel.target.and_then(|target| {
                        self.choke_score(i, target, &power).map(|score| {
                            channel.target_score = score;
                            target
                        })
                    });
                }
            }
            if let Some(target) = channel.target.filter(|&h| self.unit(h).is_some()) {
                let mut desc = self.weapon_classes[class].ordnance.damage_desc(0.0, false);
                let attenuation = if falloff < 1.0 {
                    if channel.target_score < 1.0 {
                        let a = (1.0 - channel.target_score) * (1.0 - channel.target_score);
                        a + (1.0 - a) * falloff
                    } else {
                        falloff
                    }
                } else {
                    1.0
                };
                desc.damage = damage_per_second * TICK_DT * attenuation;
                desc.scales.fill(1.0);
                desc.shield_scale = 1.0;
                let direction = (self.unit(target).unwrap().soldier.pos
                    - self.units[i].soldier.pos)
                    .normalize_or_zero();
                self.force_damage(i, target, desc, direction, true);
                if self.unit(target).is_some() {
                    let victim = &mut self.units[target.index as usize];
                    // Like a knock, the hold is soldier state: a droideka only
                    // takes the damage (`Sim::hero_knock`). [ours]
                    // Issue #173, the maintainer's rule [ours]: a soldier on
                    // the floor - standing, crouching, prone or anything
                    // else - is stood up into the hold at once; one caught
                    // in an air stance (jump, fall, throw, jet) is held where
                    // he is, his jet cut off, and drops when it ends. The
                    // original holds only a standing or crouching soldier
                    // (B2 `FUN_00239850`).
                    if victim.mount.is_none()
                        && victim.droideka.is_none()
                        && victim.soldier.stance != crate::soldier::Stance::Dead
                    {
                        let first = victim.choked.is_none();
                        // By the stance first: a floor stance a few ticks off
                        // a kerb's edge is still a floor victim; only the air's
                        // own stances (and a tumble off the floor) are held
                        // where they are.
                        let stance = victim.soldier.stance;
                        let in_air = stance.airborne()
                            || (stance == crate::soldier::Stance::Tumble
                                && !victim.soldier.grounded);
                        let hold = victim.choked.get_or_insert_with(Default::default);
                        hold.left = 0.5;
                        if first && in_air {
                            hold.hold = Some(victim.soldier.pos);
                        }
                        if hold.hold.is_some() {
                            victim.soldier.catch_in_air();
                        } else {
                            victim.soldier.catch_upright();
                        }
                        victim.soldier.yaw = crate::fmath::atan2(-direction.x, -direction.z);
                        // Issue #173 [ours]: a fall after the hold is the
                        // caster's kill (refreshed while it holds).
                        let caster = self.handle(i);
                        self.note_forced_move(target, Some(caster));
                    }
                }
            }
        }
        let state = if let Some(k) = slot {
            &mut self.units[i].held[k].state
        } else {
            &mut self.units[i].weapon
        };
        state.state = if channel.acquired && channel.target.is_none() {
            FireState::Idle
        } else {
            FireState::Firing
        };
        state.force_channel = Some(channel);
        true
    }
}
