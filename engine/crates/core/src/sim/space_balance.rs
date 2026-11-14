use super::state::Sim;
use crate::fmath::FloatExt;
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::{VehicleHandle, VehicleKind};
use crate::weapon::{DamageDesc, OrdnanceClass};
use crate::TICK_HZ;
use glam::Vec3;

pub const BALANCE_EVERY_TICKS: u64 = 15;
pub const BALANCE_QUIET_SECS: f32 = 30.0;
pub const BALANCE_MARGIN: i32 = 20;
pub const BALANCE_DIVISOR: i32 = 10;
pub const BALANCE_NUDGE_MAX: i32 = 4;
pub const FIGHTER_PERIOD: [[f32; 2]; 2] = [[45.0, 15.0], [100.0, 60.0]];
pub const SYSTEM_PERIOD: [[f32; 2]; 2] = [[25.0, 10.0], [45.0, 12.0]];
pub const FIGHTER_ENEMY_SHARE: f32 = 0.4;
pub const SYSTEM_ENEMY_SHARE: [f32; 2] = [0.5, 0.25];
pub const UNSEEN_LIST_MAX: usize = 40;
pub const UNSEEN_FLYER_RADIUS: f32 = 100.0;
pub const UNSEEN_FLYER_FAR: f32 = 3000.0;
pub const UNSEEN_SYSTEM_FAR: f32 = 1500.0;
pub const VIEW_FAR_PER_ZOOM: f32 = 8.0;
pub const SHIELD_DAMAGE_FACTOR: f32 = 5.0;
pub const SHIELD_DAMAGE_CAP: f32 = 15_000.0;
pub const EXPLOSION_PROBE_DISTANCE: f32 = 2.0;
pub const SOLDIER_VIEW_FOV: f32 = 65.0;
pub const VIEW_TAN_Y_OVER_X: f32 = 0.7666;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpaceBalance {
    pub started: bool,
    pub fighter_at: f32,
    pub system_at: f32,
    pub seed: f32,
    pub nudge_human: i32,
    pub nudge_enemy: i32,
    pub fighter_kills: u32,
    pub system_hits: u32,
}

impl SpaceBalance {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if !self.started {
            return;
        }
        eat(&self.fighter_at.hash_bits().to_le_bytes());
        eat(&self.system_at.hash_bits().to_le_bytes());
        eat(&self.seed.hash_bits().to_le_bytes());
        eat(&self.nudge_human.to_le_bytes());
        eat(&self.nudge_enemy.to_le_bytes());
        eat(&self.fighter_kills.to_le_bytes());
        eat(&self.system_hits.to_le_bytes());
    }
}

pub fn balance_nudges(human: i32, enemy: i32) -> (i32, i32) {
    let diff = human - enemy;
    if diff.abs() <= BALANCE_MARGIN {
        return (0, 0);
    }
    let margin = if diff > 0 {
        -BALANCE_MARGIN
    } else {
        BALANCE_MARGIN
    };
    let enemy_nudge = (diff + margin) / BALANCE_DIVISOR;
    (
        (-enemy_nudge).clamp(-BALANCE_NUDGE_MAX, BALANCE_NUDGE_MAX),
        enemy_nudge.clamp(-BALANCE_NUDGE_MAX, BALANCE_NUDGE_MAX),
    )
}

pub fn balance_time_factor(nudge_human: i32) -> f32 {
    match nudge_human {
        n if n < 0 => -1.0 / (n - 1) as f32,
        n if n > 0 => n as f32,
        _ => 1.0,
    }
}

pub fn ordnance_value(ord: &OrdnanceClass, health_type: usize, target_radius: f32) -> f32 {
    let mut value = ord.max_damage * ord.scales[health_type];
    if let Some(e) = &ord.explosion {
        let at = EXPLOSION_PROBE_DISTANCE - target_radius * 0.5;
        let fall = if at < e.damage_inner {
            1.0
        } else if e.damage_outer <= at {
            0.0
        } else {
            (e.damage_outer - at) / (e.damage_outer - e.damage_inner)
        };
        value += e.damage * fall * e.scales[health_type];
    }
    value
}

pub fn fire_rate_value(per_shot: f32, clip: u32, shots: u32, delay: [f32; 2], reload: f32) -> f32 {
    let average = (delay[0] + delay[1]) * 0.5;
    let shots = shots.max(1);
    if clip == 0 || clip == u32::MAX || reload <= 0.0 {
        if average <= 0.0 {
            return 0.0;
        }
        return shots as f32 * per_shot / average;
    }
    clip as f32 * per_shot / ((clip / shots) as f32 * average + reload)
}

impl Sim {
    pub fn space_balance(&self) -> Option<SpaceBalance> {
        self.space_assault.as_ref().map(|sa| sa.balance)
    }

    fn balance_uniform(&mut self, low: f32, high: f32) -> f32 {
        let r = (self.rng.next_rand() >> 8) as f32 * (1.0 / 8_388_608.0);
        low + (high.fmax(low) - low) * r
    }

    fn balance_percent(&mut self) -> f32 {
        let r = self.balance_uniform(0.0, 1.0);
        (101.0 * r) as i32 as f32
    }

    fn balance_lerp(&self, enemy: u8, pair: [f32; 2], nudge: i32) -> f32 {
        let skill = (i32::from(self.bot_skill(enemy)) + nudge).clamp(1, 20);
        pair[0] + (pair[1] - pair[0]) * ((skill - 1) as f32 * 0.052_631_58)
    }

    fn balance_period(&mut self, enemy: u8, period: [[f32; 2]; 2], nudge: i32) -> f32 {
        let low = self.balance_lerp(enemy, period[0], nudge);
        let high = self.balance_lerp(enemy, period[1], nudge);
        self.balance_uniform(low, high)
    }

    fn balance_human_team(&self) -> Option<u8> {
        let mut teams = self
            .players
            .iter()
            .flatten()
            .map(|h| self.units[h.index as usize].team);
        let first = teams.next().filter(|&t| t < 2)?;
        teams.all(|t| t == first).then_some(first)
    }

    pub(super) fn tick_space_balance(&mut self) {
        if self.outcome.is_some()
            || self.space_assault.is_none()
            || !self.tick.is_multiple_of(BALANCE_EVERY_TICKS)
        {
            return;
        }
        let Some(human) = self.balance_human_team() else {
            if let Some(sa) = self.space_assault.as_mut() {
                sa.balance.nudge_human = 0;
                sa.balance.nudge_enemy = 0;
            }
            return;
        };
        let enemy = 1 - human;
        let started = self
            .space_assault
            .as_ref()
            .is_some_and(|sa| sa.balance.started);
        if !started {
            let fighter_at = self.balance_period(enemy, FIGHTER_PERIOD, 0);
            let system_at = self.balance_period(enemy, SYSTEM_PERIOD, 0);
            let seed = self.balance_uniform(0.0, 1.0);
            if let Some(sa) = self.space_assault.as_mut() {
                sa.balance = SpaceBalance {
                    started: true,
                    fighter_at,
                    system_at,
                    seed,
                    ..SpaceBalance::default()
                };
            }
        }
        let Some(sa) = self.space_assault.as_mut() else {
            return;
        };
        let (nudge_human, nudge_enemy) =
            balance_nudges(sa.points[usize::from(human)], sa.points[usize::from(enemy)]);
        sa.balance.nudge_human = nudge_human;
        sa.balance.nudge_enemy = nudge_enemy;
        let (fighter_at, system_at) = (sa.balance.fighter_at, sa.balance.system_at);
        let now = self.tick as f32 / TICK_HZ as f32;
        if now < BALANCE_QUIET_SECS {
            return;
        }
        let factor = balance_time_factor(nudge_human);
        if fighter_at < now {
            let next = self.balance_period(enemy, FIGHTER_PERIOD, nudge_enemy);
            if let Some(sa) = self.space_assault.as_mut() {
                sa.balance.fighter_at = now + next * factor;
            }
            let r = self.balance_percent();
            let team = if FIGHTER_ENEMY_SHARE * 100.0 < r {
                human
            } else {
                enemy
            };
            self.balance_fighter_kill(team);
        }
        if now <= system_at {
            return;
        }
        let next = self.balance_period(enemy, SYSTEM_PERIOD, nudge_enemy);
        if let Some(sa) = self.space_assault.as_mut() {
            sa.balance.system_at = now + next * factor;
        }
        let r = self.balance_percent();
        let share = self.balance_lerp(enemy, SYSTEM_ENEMY_SHARE, nudge_enemy);
        let team = if share * 100.0 < r { human } else { enemy };
        self.balance_system_attack(team);
    }

    fn balance_seen(&self, pos: Vec3, radius: f32, far: f32) -> bool {
        let far = far + VIEW_FAR_PER_ZOOM;
        self.players.iter().flatten().any(|&h| {
            let Some(u) = self.unit(h) else {
                return false;
            };
            let fov = u
                .mount
                .and_then(|m| self.vehicle(m.vehicle))
                .and_then(|v| self.flyer_params.get(v.class as usize))
                .and_then(Option::as_ref)
                .map_or(SOLDIER_VIEW_FOV, |p| p.view_fov);
            let (eye, dir) = u.aim_ray(self.ground_rules, self.views());
            let dir = dir.normalize_or_zero();
            let right = dir.cross(Vec3::Y).normalize_or_zero();
            let up = right.cross(dir);
            let off = pos - eye;
            let depth = off.dot(dir);
            if depth < -radius || far + radius < depth {
                return false;
            }
            let (sin, cos) = crate::fmath::sin_cos(fov.to_radians() * 0.5);
            let tan_x = sin / cos;
            let tan_y = tan_x * VIEW_TAN_Y_OVER_X;
            let inside = |axis: Vec3, tan: f32| {
                let sec = (1.0 + tan * tan).sqrt();
                off.dot(axis).abs() - tan * depth <= radius * sec
            };
            inside(right, tan_x) && inside(up, tan_y)
        })
    }

    fn balance_unseen_flyer(&self, team: u8, own: bool) -> Option<(UnitHandle, VehicleHandle)> {
        let listed: Vec<(UnitHandle, VehicleHandle)> = self
            .alive_units()
            .filter(|(_, u)| matches!(u.controller, Controller::Ai))
            .filter(|(_, u)| self.hostile(team, u.team) != own)
            .take(UNSEEN_LIST_MAX)
            .filter_map(|(h, u)| Some((h, u.mount?.vehicle)))
            .collect();
        listed.into_iter().rev().find(|&(_, handle)| {
            let Some(v) = self
                .vehicle(handle)
                .filter(|v| v.kind == VehicleKind::Flyer)
            else {
                return false;
            };
            let human_aboard = v
                .seats
                .iter()
                .filter_map(|s| s.occupant)
                .filter_map(|o| self.unit(o))
                .any(|o| o.is_human());
            !human_aboard && !self.balance_seen(v.pos, UNSEEN_FLYER_RADIUS, UNSEEN_FLYER_FAR)
        })
    }

    fn balance_fighter_kill(&mut self, team: u8) {
        let Some((_, victim)) = self.balance_unseen_flyer(team, true) else {
            return;
        };
        let Some((killer, _)) = self.balance_unseen_flyer(team, false) else {
            return;
        };
        if let Some(v) = self.vehicle_mut(victim) {
            v.health.cur = 0.0;
        }
        if let Some(sa) = self.space_assault.as_mut() {
            sa.balance.fighter_kills += 1;
        }
        self.destroy_vehicle(victim, Some(killer));
    }

    fn balance_system_target(&mut self, team: u8) -> Option<u16> {
        let sa = self.space_assault.as_ref()?;
        let seed = sa.balance.seed;
        let open: Vec<u16> = sa
            .critical
            .iter()
            .filter(|c| c.shown)
            .map(|c| c.station)
            .filter(|&s| {
                let st = &self.stations[usize::from(s)];
                st.team == Some(team)
                    && !st.destroyed
                    && st.health.cur > 0.0
                    && !self.balance_seen(st.sphere_centre, st.sphere_radius, UNSEEN_SYSTEM_FAR)
            })
            .collect();
        if open.is_empty() {
            return None;
        }
        let r = self.balance_uniform(0.0, 1.0);
        let spot = (r * r + seed).rem_euclid(1.0);
        open.get((spot * open.len() as f32) as usize).copied()
    }

    fn balance_system_attack(&mut self, team: u8) {
        let Some((killer, _)) = self.balance_unseen_flyer(team, false) else {
            return;
        };
        let Some(target) = self.balance_system_target(team) else {
            return;
        };
        let st = &self.stations[usize::from(target)];
        let (kind, radius, shield) = (
            st.health.health_type as usize,
            st.sphere_radius,
            st.health.shield,
        );
        let Some(setup) = self.seat_gun_setup(killer) else {
            return;
        };
        let mut best = 0.0_f32;
        for gun in &setup.weapons {
            let Some(c) = self.weapon_classes.get(gun.class) else {
                continue;
            };
            let per_shot = ordnance_value(&c.ordnance, kind, radius);
            let rate = if c.thrown.is_some() {
                0.0
            } else {
                fire_rate_value(
                    per_shot,
                    c.rounds_per_clip,
                    c.shots_per_salvo,
                    c.shot_delay,
                    c.reload_time,
                )
            };
            best = best.fmax(rate.fmax(per_shot + per_shot));
        }
        if best <= 0.0 {
            return;
        }
        let damage = if shield > 0.0 {
            (best * SHIELD_DAMAGE_FACTOR)
                .fmin(SHIELD_DAMAGE_CAP)
                .fmin(shield)
        } else {
            best
        };
        let desc = DamageDesc {
            damage,
            scales: [1.0; 7],
            shield_scale: 1.0,
        };
        if let Some(sa) = self.space_assault.as_mut() {
            sa.balance.system_hits += 1;
        }
        let attacker_team = self.units[killer.index as usize].team;
        self.damage_station(target, &desc, 1.0, Some(killer), Some(attacker_team));
    }
}

#[cfg(test)]
mod tests;
