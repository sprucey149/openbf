//! Health, ammo and vehicle service droids (`powerupstation`, card PWST;
//! `analysis/re-powerup-station.md` §§3–6, 14–15). Step 5c' follows item pickups.
//!
//! A station serves feet strictly inside its authored Radius in 3-D. Each has
//! its own PowerupDelay clock, initially zero, keeping the remainder and doing
//! at most one pass per tick. A pass gives each eligible entity health and
//! clips up to its cap, without checking team. Serving controls the shared
//! idle/active motion; collision and drawing now use the same station yaw.
//!
//! Destruction disables service, installs the authored wreck before applying
//! the death explosion, and excludes the source body from its own blast LOS
//! (§§14–15). Direct and splash damage use the authored health type and sphere.
//!
//! PROVISIONAL: only living entities are served; riders are absent from the
//! world gather but their vehicles are eligible. Vehicles receive health only
//! until seat guns have reserves. Gather order is unit slots then vehicles,
//! capped at the PS2's 64 entities. BF2 regeneration eligibility is not built.

pub use crate::station_motion::StationMotion;

use super::pickups::give_ammo;
use super::state::{Event, Sim};
use crate::unit::UnitHandle;
use crate::vehicle::VehicleHandle;
use crate::weapon::{Health, HealthType};
use glam::Vec3;

/// The most entities one pass serves (the gather's list, B1 236313).
pub const GATHER_MAX: usize = 64;
/// A station class's `PowerupDelay` default (s; B1 `FUN_002aa5d8`, B2
/// `FUN_00368898`).
pub const DELAY_DEFAULT: f32 = 0.5;

/// One `powerupstation` class (§2.1), built by `game::roster` from the disc.
/// SETUP data like `Sim::item_classes`: not hashed.
#[derive(Clone, Debug, PartialEq)]
pub struct StationClass {
    pub name: String,
    /// `PowerupDelay` (s): the time between two passes.
    pub delay: f32,
    /// Per health type (person, animal, droid, vehicle, building): health
    /// given per pass (`PersonHealth` .. ; `SoldierHealth` = person and
    /// droid).
    pub health: [f32; 5],
    /// Per health type: clips added to every weapon per pass.
    pub ammo: [u32; 5],
    /// What the game layer draws and plays (the sim reads none of it):
    /// `GeometryName`, `PowerupSound` (at the station per unit served) and
    /// `AmbientSound`.
    pub geometry: String,
    pub powerup_sound: Option<String>,
    pub ambient_sound: Option<String>,
    /// Authored idle and active node motion (§§2.1, 6).
    pub anim: StationAnim,
    /// Authored MaxHealth and damage category (§15).
    pub max_health: f32,
    /// Damage multipliers index this category, independently of the served type.
    pub health_type: HealthType,
    /// Geometry swapped in before the station's death explosion.
    pub destroyed_geometry: Option<String>,
    /// Authored death ordnance, applied once when health reaches zero.
    pub explosion: Option<crate::weapon::ExplosionClass>,
    /// Body-local authored blast sphere [x, y, z, radius] (§14).
    pub sphere: Option<[f32; 4]>,
    /// Wreck-local blast sphere, transformed by the death pose.
    pub wreck_sphere: Option<[f32; 4]>,
    pub damage_effects: Vec<StationDamageEffect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StationDamageEffect {
    pub band: crate::autoturret::DamageEffect,
    pub local: glam::Mat4,
    pub scale: f32,
}

/// Authored animation keys; defaults follow B1 FUN_002aa5d8 (§§2.1, 6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StationAnim {
    /// `IdleRotateSpeed` (rad/s): the idle turn about +Y; 0 = none.
    pub rotate_speed: f32,
    /// `IdleWaitTime` (s) between two idle turns.
    pub wait_time: f32,
    /// `IdleWobbleNode`: rocks about Z in a turn, bobs while serving.
    pub wobble_node: Option<String>,
    /// `IdleWobbleFactor`: the rock's amplitude (rad) and the bob's depth (m).
    pub wobble_factor: f32,
    /// `ActiveRotateNode` (the first 4): each swings about X while serving.
    pub rotate_nodes: Vec<String>,
    /// `ActiveSpinNode`: spins about Z while serving.
    pub spin_node: Option<String>,
}

impl StationClass {
    /// A station that gives soldiers (persons and droids) `health` and
    /// `ammo` every `delay` s.
    pub fn soldier(name: &str, delay: f32, health: f32, ammo: u32) -> Self {
        StationClass {
            name: name.to_owned(),
            delay,
            health: [health, 0.0, health, 0.0, 0.0],
            ammo: [ammo, 0, ammo, 0, 0],
            geometry: String::new(),
            powerup_sound: None,
            ambient_sound: None,
            anim: StationAnim::default(),
            max_health: 80.0,
            health_type: HealthType::Building,
            destroyed_geometry: None,
            explosion: None,
            sphere: None,
            wreck_sphere: None,
            damage_effects: Vec::new(),
        }
    }
}

/// A placed station (`Sim::stations`). HASHED when there is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// Index into `Sim::station_classes`.
    pub class: u8,
    /// The sphere's centre: the instance's origin.
    pub pos: Vec3,
    /// The instance's `Radius` (m).
    pub radius: f32,
    /// The station's clock (s), from 0 to the class's delay.
    pub timer: f32,
    /// The last pass served someone (§4: state 3, else 1).
    pub serving: bool,
    pub health: Health,
    pub destroyed: bool,
    pub instance: Option<u32>,
    pub transform: glam::Mat4,
    pub team: Option<u8>,
    pub sphere_centre: Vec3,
    pub sphere_radius: f32,
    pub motion: StationMotion,
}

impl Station {
    pub fn new(class: u8, pos: Vec3, radius: f32) -> Self {
        Station {
            class,
            pos,
            radius,
            timer: 0.0,
            serving: false,
            health: Health::new(80.0, HealthType::Building),
            destroyed: false,
            instance: None,
            transform: glam::Mat4::from_translation(pos),
            team: None,
            sphere_centre: pos,
            sphere_radius: 0.0,
            motion: StationMotion::new(1),
        }
    }

    /// Strictly inside the sphere, in 3-D (§3).
    pub fn reaches(&self, feet: Vec3) -> bool {
        (feet - self.pos).length_squared() < self.radius * self.radius
    }

    pub(crate) fn repair_kind(&self) -> HealthType {
        if self.destroyed && self.health.health_type == HealthType::Building {
            HealthType::Building2
        } else {
            self.health.health_type
        }
    }

    pub fn pose(&self) -> glam::Mat4 {
        self.transform * glam::Mat4::from_quat(crate::vehicle::hover::quat_y(self.motion.yaw))
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        self.motion.hash_into(eat);
        eat(&[self.class, self.serving as u8]);
        eat(&[u8::from(self.destroyed)]);
        eat(&self.health.cur.to_bits().to_le_bytes());
        for v in [self.pos.x, self.pos.y, self.pos.z, self.radius, self.timer] {
            eat(&v.to_bits().to_le_bytes());
        }
    }
}

/// The class table's slot for a health type (the PS2's flags & 7, §2.1);
/// the port's extra building kinds are buildings.
fn type_slot(t: HealthType) -> usize {
    (t as usize).min(4)
}

/// re-powerups §5.3 as the station calls it: a non-zero amount, and the
/// unit below its cap (or a negative amount), gives min(cur + amount, max).
pub(super) fn give_health(h: &mut Health, amount: f32) -> bool {
    if amount == 0.0 || (h.cur >= h.max && amount > 0.0) {
        return false;
    }
    let new = (h.cur + amount).min(h.max);
    let changed = new != h.cur;
    h.cur = new;
    changed
}

impl Sim {
    /// Cache the live collision before destruction so rebuilding restores it.
    pub fn prepare_station_collision(&mut self, station: usize) {
        let s = self.stations[station];
        let parts = s
            .instance
            .filter(|_| s.radius > 0.0)
            .map_or_else(Vec::new, |i| self.collision.prepare_rotation(i, s.pose()));
        let live = s
            .instance
            .map_or_else(Vec::new, |i| self.collision.instance_parts(i, s.pose()));
        self.station_bodies
            .resize_with(self.stations.len(), Vec::new);
        self.station_live_collision
            .resize_with(self.stations.len(), Vec::new);
        self.station_bodies[station] = parts;
        self.station_live_collision[station] = live;
    }

    /// Apply direct or splash damage, then install the wreck and death blast
    /// exactly once. The source station/body cannot obstruct its own blast (§15).
    pub fn damage_station(
        &mut self,
        station: u16,
        desc: &crate::weapon::DamageDesc,
        scale: f32,
        attacker: Option<UnitHandle>,
        team: Option<u8>,
    ) {
        let Some(s) = self.stations.get_mut(usize::from(station)) else {
            return;
        };
        if s.destroyed {
            return;
        }
        let friendly = s.team.is_some() && s.team == team;
        s.health
            .take_damage(desc, scale, friendly, self.friendly_fire_scale);
        if s.health.alive() {
            return;
        }
        let (class, pos) = (s.class, s.pos);
        let station_team = s.team;
        self.install_station_wreck(station);
        self.events.push(Event::StationDestroyed {
            station,
            class,
            pos,
            killer: attacker,
        });
        if let Some(e) = self.station_classes[usize::from(class)].explosion.clone() {
            self.explode_from(
                &e,
                pos,
                None,
                station_team,
                Some(super::blast::BlastSource::Station(station)),
            );
        }
    }

    pub fn install_station_wreck(&mut self, station: u16) {
        let s = &mut self.stations[usize::from(station)];
        s.health.cur = 0.0;
        s.destroyed = true;
        s.serving = false;
        let (class, instance, transform) = (s.class, s.instance, s.pose());
        let c = &self.station_classes[usize::from(class)];
        if let Some([x, y, z, r]) = c.wreck_sphere {
            s.sphere_centre = transform.transform_point3(Vec3::new(x, y, z));
            s.sphere_radius = r;
        }
        if let Some(i) = instance {
            if self
                .station_live_collision
                .get(usize::from(station))
                .is_none_or(Vec::is_empty)
            {
                self.prepare_station_collision(usize::from(station));
            }
            let parts = self
                .station_wreck_collision
                .get(usize::from(class))
                .map_or(&[][..], Vec::as_slice);
            self.collision.replace_instance(i, parts, transform);
        }
    }

    /// Register a station class (one entry per name) and return its index.
    pub fn add_station_class(&mut self, c: StationClass) -> u8 {
        if let Some(i) = self.station_classes.iter().position(|x| x.name == c.name) {
            return i as u8;
        }
        self.station_classes.push(c);
        (self.station_classes.len() - 1) as u8
    }

    /// Step 5c': every station's clock, and the passes that come due.
    pub(super) fn tick_stations(&mut self, dt: f32) {
        for si in 0..self.stations.len() {
            let s = self.stations[si];
            if s.destroyed || s.radius <= 0.0 {
                continue;
            }
            let Some(c) = self.station_classes.get(usize::from(s.class)) else {
                continue;
            };
            let delay = c.delay;
            let timer = s.timer + dt;
            self.stations[si].timer = timer;
            if timer >= delay {
                self.stations[si].timer -= delay;
                let served = self.station_pass(si);
                self.stations[si].serving = served;
            }
            let c = &self.station_classes[usize::from(s.class)];
            let s = &mut self.stations[si];
            let previous_yaw = s.motion.yaw;
            s.motion.step(&c.anim, s.serving, dt);
            let pose = s.pose();
            if let Some([x, y, z, r]) = c.sphere {
                s.sphere_centre = pose.transform_point3(Vec3::new(x, y, z));
                s.sphere_radius = r;
            }
            if s.motion.yaw != previous_yaw {
                if let Some(parts) = self.station_bodies.get(si) {
                    for &(i, local) in parts {
                        self.collision.move_body(i, pose * local);
                    }
                }
            }
        }
    }

    /// One pass of station `si` (§4): gives to everyone inside; true when
    /// it gave to anyone.
    fn station_pass(&mut self, si: usize) -> bool {
        let s = self.stations[si];
        let c = &self.station_classes[usize::from(s.class)];
        let mut gathered = 0;
        let mut served = false;
        for ui in 0..self.units.len() {
            if gathered >= GATHER_MAX {
                return served;
            }
            let u = &self.units[ui];
            if !u.alive || u.mount.is_some() || !s.reaches(u.soldier.pos) {
                continue;
            }
            gathered += 1;
            let t = type_slot(u.health.health_type);
            let hero = self.ground_rules == crate::soldier::GroundRules::Bf2
                && self.hero_class(u.team) == Some(u.spec.class);
            let classes = &self.weapon_classes;
            let u = &mut self.units[ui];
            // PS2 0x369770 refuses SetHeroClass health unless its caller
            // explicitly bypasses the check (kill-heal does; stations do not).
            let health = !hero && give_health(&mut u.health, c.health[t]);
            let ammo = give_ammo(classes, u, c.ammo[t]);
            if health || ammo {
                served = true;
                let unit = UnitHandle {
                    index: ui as u32,
                    gen: u.gen,
                };
                self.events.push(Event::StationServed {
                    station: si as u16,
                    unit: Some(unit),
                    vehicle: None,
                    pos: s.pos,
                    health,
                    ammo,
                    energy: false,
                });
            }
        }
        for vi in 0..self.vehicles.len() {
            if gathered >= GATHER_MAX {
                break;
            }
            let v = &mut self.vehicles[vi];
            if !v.alive || !s.reaches(v.pos) {
                continue;
            }
            gathered += 1;
            let t = type_slot(v.health.health_type);
            if give_health(&mut v.health, c.health[t]) {
                served = true;
                let vehicle = VehicleHandle {
                    index: vi as u32,
                    gen: v.gen,
                };
                self.events.push(Event::StationServed {
                    station: si as u16,
                    unit: None,
                    vehicle: Some(vehicle),
                    pos: s.pos,
                    health: true,
                    ammo: false,
                    energy: false,
                });
            }
        }
        served
    }
}

#[cfg(test)]
mod tests;
