//! PS2 melee block acceptance and projectile redirection (0x0033e8b8).

use super::*;
use crate::lcg::GameLcg;
use crate::projectile::{Bolt, Shooter};
use crate::unit::UnitHandle;
use crate::weapon::OrdKind;
use glam::Mat4;

pub(in crate::sim::saber) mod selection;
use selection::{direction_mask, reflected_origin};

impl Sim {
    /// `pos`: where the contact is drawn. The PS2 plays `DeflectEffect` there
    /// after the deflect sound (FUN_0033e8b8, B2:334743-334751).
    fn accept_deflect(&mut self, target: UnitHandle, direction: Vec3, pos: Vec3) -> bool {
        let Some(u) = self.unit(target) else {
            return false;
        };
        if !u.alive
            || u.mount.is_some()
            || u.choked.is_some()
            || u.soldier.no_fire()
            || self.weapon_classes[u.weapon_class].saber.is_none()
        {
            return false;
        }
        let Some(state) = u.weapon.melee.as_ref() else {
            return false;
        };
        let Some(deflect) = state.deflect else {
            return false;
        };
        let (s, c) = fmath::sin_cos(u.soldier.yaw);
        let facing = Vec3::new(-s, 0.0, -c);
        let left = Vec3::new(-c, 0.0, s);
        let incoming = -direction;
        let angle = fmath::atan2(incoming.dot(left), incoming.dot(facing)).to_degrees();
        if !deflect.covers(angle) {
            self.unit_mut(target)
                .unwrap()
                .weapon
                .melee
                .as_mut()
                .unwrap()
                .select_block_animation(None);
            return false;
        }
        let class = self.weapon_classes[u.weapon_class].saber.as_ref().unwrap();
        let options = class.deflect_animations(state.runtime.state);
        let available = options.iter().fold(0, |mask, (_, m)| mask | m);
        let mask = direction_mask(available, direction, facing, u.soldier.vel);
        let candidates: Vec<_> = options
            .into_iter()
            .filter(|(_, m)| mask & m != 0)
            .map(|(name, _)| name.to_owned())
            .collect();
        let previous = state.block_animation.clone();
        let weapon_class = u.weapon_class as u32;
        let sound = class
            .deflect_sound
            .as_deref()
            .map(crate::hero_weapon::sound_hash);
        self.melee_lcg_active = true;
        // Selection precedes the energy payment, including a failed payment.
        let selected = match candidates.len() {
            0 => previous,
            1 => candidates.into_iter().next(),
            n => Some(candidates[(self.melee_lcg.unit() * n as f32) as usize].clone()),
        };
        let u = self.unit_mut(target).unwrap();
        if !u.soldier.pay_energy(deflect.energy_cost) {
            u.weapon
                .melee
                .as_mut()
                .unwrap()
                .select_block_animation(None);
            return false;
        }
        u.weapon
            .melee
            .as_mut()
            .unwrap()
            .select_block_animation(selected.as_deref());
        if let Some(sound_hash) = sound {
            self.events.push(super::super::Event::ComboSound {
                unit: target,
                weapon_class,
                sound_hash,
            });
        }
        self.events.push(super::super::Event::SaberContact {
            pos,
            weapon_class,
            deflect: true,
        });
        true
    }

    /// Incoming travel points toward the defender. Non-projectile blocks
    /// reset precision; projectile RTTI selects its separate update below.
    pub(in crate::sim) fn saber_deflect(
        &mut self,
        target: UnitHandle,
        direction: Vec3,
        pos: Vec3,
    ) -> bool {
        if !self.accept_deflect(target, direction, pos) {
            return false;
        }
        self.unit_mut(target)
            .unwrap()
            .weapon
            .melee
            .as_mut()
            .unwrap()
            .deflect_precision = 0.0;
        true
    }

    pub(in crate::sim) fn deflect_bolt(&mut self, target: UnitHandle, bolt: &Bolt) -> bool {
        if !self.accept_deflect(target, bolt.vel.normalize_or_zero(), bolt.pos) {
            return false;
        }
        let u = self.unit(target).unwrap();
        let state = u.weapon.melee.as_ref().unwrap();
        let precision = state.deflect_precision;
        let time = state.runtime.time;
        let spread = if precision < 1.0 {
            (1.0 - precision) * 0.25 + precision * 0.05
        } else if time < 0.4 {
            0.02
        } else {
            0.05
        };
        let aim = u.aim_frame().0;
        let (s, c) = fmath::sin_cos(u.soldier.yaw);
        let right = Vec3::new(-c, 0.0, s);
        let across = aim.cross(right);
        let pose_forward = crate::combo::thrust::orient(
            Vec3::new(-s, 0.0, -c),
            state.retained_thrust,
            state.current_thrust_forward,
            false,
        );
        let pose_right = Vec3::Y.cross(pose_forward).normalize_or_zero();
        let pose_basis = Mat4::from_cols(
            pose_right.extend(0.0),
            pose_forward.cross(pose_right).extend(0.0),
            pose_forward.extend(0.0),
            u.soldier.pos.extend(1.0),
        );
        let origin = reflected_origin(
            self.weapon_classes[u.weapon_class].saber.as_ref().unwrap(),
            state.block_animation.as_deref(),
            pose_basis,
            bolt.pos,
        );
        let shooter = Shooter {
            owner: target,
            team: u.team,
            weapon_class: bolt.weapon_class,
            is_player: u.is_player,
        };
        let beam = self.weapon_classes[bolt.weapon_class].ordnance.kind == OrdKind::Beam;
        let (x, y) = if beam {
            // [ours] Stable shot fields replace native object-address entropy.
            let seed = bolt.owner.index
                ^ bolt.owner.gen.rotate_left(16)
                ^ (bolt.weapon_class as u32).rotate_left(8)
                ^ bolt.age.to_bits()
                ^ 0xc71c_2830;
            let mut random = GameLcg::new(seed);
            let phase = random.unit();
            let wobble = random.unit() * 2.0 - 1.0;
            let (sin, cos) = fmath::sin_cos((phase + time * wobble) * std::f32::consts::TAU);
            (cos * spread, sin * spread)
        } else {
            // Bullet and Laser both fail the OrdnanceBeam RTTI check.
            (
                (self.melee_lcg.unit() * 2.0 - 1.0) * spread,
                (self.melee_lcg.unit() * 2.0 - 1.0) * spread,
            )
        };
        let direction = (aim + right * x + across * y).normalize_or_zero();
        let ord = &self.weapon_classes[bolt.weapon_class].ordnance;
        // FUN_00314568 creates a fresh full-charge packet for the same class.
        let mut reflected = Bolt::fire(
            origin,
            direction,
            ord.charged(1.0).velocity,
            ord.charged(1.0).life_span,
            shooter,
            self.heightfield.as_ref(),
            self.water_plane(),
            &self.collision,
            crate::collision::HullFilter::All,
        );
        reflected.gravity = bolt.gravity;
        reflected.charge = 1.0;
        // The native class factory recreates missile steering, with no target
        // in the reflection packet (BF2 FUN_00314568). Do not launch a held
        // beam here: a reflection is one outgoing contact, not a new emitter.
        if let Some(mc) = ord.missile {
            reflected.as_missile(ord, &mc, None, &mut self.lcg);
        }
        self.bolts.push(reflected);
        self.unit_mut(target)
            .unwrap()
            .weapon
            .melee
            .as_mut()
            .unwrap()
            .deflect_precision = if beam {
            ((precision - TICK_DT).min(1.0) - (TICK_DT + TICK_DT)).max(0.0)
        } else {
            0.0
        };
        true
    }
}
