//! A moving vehicle running a soldier over: the soldier's own collision
//! response meeting a vehicle body (`analysis/re-hover.md` §13.9).
//!
//! B2 `FUN_0023cfd0` (B2:154856) is the soldier's response to every contact.
//! Against a vehicle body (layer 8) it keeps only the soldier's vertical
//! velocity and takes the vehicle's velocity off it (B2:155038-155052), then
//! runs the landing law of [`super::landing::damage`] on that relative
//! velocity and the contact normal (B2:155086-155179). The vehicle class's
//! factor (class vtable slot +0x40) is 1.0 for a hover (0x61ca18 →
//! `0x26bcb8`, which returns 1.0). The damage names the vehicle as its
//! source; the port credits the hover's driver, and a driverless hover's
//! kill is the same collision damage a fall deals (untraced: who the PS2
//! credits for a vehicle entity's damage). In BF2 the vehicle the soldier
//! last stood on (soldier +0x480, set by a contact with n.y over 0.6,
//! B2:155449-155520; tested B2:155086-155113) deals none. BF1's twin
//! `FUN_00202f38` (B1:114213-114297, vehicle layer 4, hover factor
//! `0x222c80` = 1.0) has no such exception.

use super::{DeathCause, Sim};
use crate::collision::Capsule;
use crate::soldier::GroundRules;
use crate::vehicle::VehicleKind;
use crate::weapon::DamageDesc;
use glam::Vec3;

impl Sim {
    /// Every hover hull `cap` is pressed into deals the PS2's impact damage
    /// to unit `i`. Run before the soldier's wall pass pushes him out.
    pub(super) fn vehicle_impacts(&mut self, i: usize, cap: &Capsule) {
        let u = &self.units[i];
        if !u.alive || u.is_player && self.cheats.invincible {
            return;
        }
        for (vh, normal) in self.collision.soldier_hull_contacts(cap) {
            let u = &self.units[i];
            let deck = self.ground_rules == GroundRules::Bf2 && u.stood_on == Some(vh);
            if !u.alive || deck {
                continue;
            }
            let Some(v) = self.vehicle(vh).filter(|v| v.alive && v.kind == VehicleKind::Hover)
            else {
                continue;
            };
            let relative = Vec3::new(0.0, u.soldier.vel.y, 0.0) - v.vel;
            let amount = super::landing::damage(&u.consts, relative, normal, u.soldier.yaw);
            if amount <= 0.0 {
                continue;
            }
            let team = v.team;
            let driver = v.seats.first().and_then(|s| s.occupant);
            let driver_is_player = driver
                .and_then(|d| self.unit(d))
                .is_some_and(|d| d.is_player);
            let victim = self.handle(i);
            let desc = DamageDesc {
                damage: amount,
                scales: [1.0; 7],
                shield_scale: 1.0,
            };
            let damage = match team {
                Some(team) => self.shot_strikes_unit(team, driver_is_player, victim, &desc, 1.0, false),
                None => self
                    .unit_mut(victim)
                    .map(|v| v.health.take_damage(&desc, 1.0, false, 1.0)),
            };
            let Some(damage) = damage else {
                continue;
            };
            match driver {
                Some(driver) => {
                    self.finish_unit_damage(victim, Some(driver), damage, 0.0, -normal);
                }
                None => {
                    let u = &mut self.units[i];
                    if damage > 0.0 {
                        u.cloak.strength = 0.0;
                    }
                    if !u.health.alive() {
                        self.unit_killed_with_cause(victim, None, DeathCause::Fall);
                    }
                }
            }
        }
    }
}
