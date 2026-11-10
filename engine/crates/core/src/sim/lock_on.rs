use super::input::{buttons, cone_score, AimTarget, PadState};
use super::state::Sim;
use crate::collision::HullFilter;
use crate::missile::MissileTarget;
use crate::perception;
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::VehicleHandle;
use glam::Vec3;

pub const LOCK_CYCLE_MEMORY: usize = 64;
pub const LOCK_CLEAR_HOLD_SECS: f32 = 0.5;
pub const LOCK_FIRE_COS: f32 = 0.9;
pub const LOCK_CHANNELS: usize = 2;
pub const VEHICLE_KIND_BIT: u32 = 1 << 3;
pub const BUILDING_KIND_BIT: u32 = 1 << 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockTarget {
    Aim(AimTarget),
    Station(u16),
}

impl LockTarget {
    pub(super) fn missile(self) -> Option<MissileTarget> {
        match self {
            LockTarget::Aim(AimTarget::Unit(h)) => Some(MissileTarget::Unit(h)),
            LockTarget::Aim(AimTarget::Vehicle(h)) => Some(MissileTarget::Vehicle(h)),
            LockTarget::Aim(AimTarget::Turret(id)) => Some(MissileTarget::Turret(id)),
            LockTarget::Aim(AimTarget::Droid(_) | AimTarget::Station(_)) => None,
            LockTarget::Station(s) => Some(MissileTarget::Station(s)),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LockOn {
    pub target: Option<LockTarget>,
    pub held: [f32; LOCK_CHANNELS],
    pub charging: [bool; LOCK_CHANNELS],
    pub(super) salvo_target: [Option<MissileTarget>; LOCK_CHANNELS],
    cycled: Vec<LockTarget>,
    button: f32,
    down: bool,
    pub incoming_missile: bool,
    pub incoming_distance: f32,
}

pub(super) struct LockProbe {
    pub(super) pos: Vec3,
    pub(super) aim: Vec3,
    pub(super) team: Option<u8>,
    pub(super) kind_bit: u32,
    pub(super) radius: f32,
    pub(super) up: Vec3,
    pub(super) hull: Option<VehicleHandle>,
    pub(super) building: Option<u32>,
}

pub(super) fn lock_score(off: Vec3, dir: Vec3) -> f32 {
    let mut d2 = off.length_squared();
    let dot = off.normalize_or_zero().dot(dir).max(0.1);
    if dot > 0.998 {
        d2 *= 1e-8;
    }
    if dot > 0.98 {
        d2 *= 1e-6;
    }
    d2 / (dot * dot)
}

impl Sim {
    pub(super) fn lock_probe(&self, target: LockTarget) -> Option<LockProbe> {
        match target {
            LockTarget::Aim(t) => {
                let c = self.aim_candidate(t)?;
                let up = t
                    .vehicle()
                    .and_then(|h| self.vehicle(h))
                    .map_or(Vec3::Y, |v| v.rot * Vec3::Y);
                Some(LockProbe {
                    pos: c.ctr,
                    aim: c.aim,
                    team: c.team,
                    kind_bit: 1 << c.kind,
                    radius: c.r,
                    up,
                    hull: c.hull,
                    building: c.building,
                })
            }
            LockTarget::Station(s) => {
                let st = self
                    .stations
                    .get(usize::from(s))
                    .filter(|st| !st.destroyed)?;
                let sa = self.space_assault.as_ref()?;
                let lockable = sa.lock_on.contains(&s);
                let known = lockable
                    || sa.critical.iter().any(|c| c.station == s)
                    || sa.shields.iter().any(|g| g.objects.contains(&s));
                if !known {
                    return None;
                }
                Some(LockProbe {
                    pos: st.sphere_centre,
                    aim: st.sphere_centre,
                    team: st.team,
                    kind_bit: if lockable {
                        VEHICLE_KIND_BIT
                    } else {
                        BUILDING_KIND_BIT
                    },
                    radius: st.sphere_radius,
                    up: st.transform.y_axis.truncate().normalize_or_zero(),
                    hull: None,
                    building: st.instance,
                })
            }
        }
    }

    pub(super) fn lock_candidates(
        &self,
        own: VehicleHandle,
        shooter: UnitHandle,
    ) -> impl Iterator<Item = LockTarget> + '_ {
        let units = self
            .alive_units()
            .filter(move |(h, u)| *h != shooter && u.mount.is_none())
            .map(|(h, _)| LockTarget::Aim(AimTarget::Unit(h)));
        let vehicles = self
            .alive_vehicles()
            .filter(move |(h, v)| *h != own && v.health.alive())
            .map(|(h, _)| LockTarget::Aim(AimTarget::Vehicle(h)));
        let turrets = self
            .turrets
            .iter()
            .map(|t| LockTarget::Aim(AimTarget::Turret(t.id)));
        let stations = (0..self.stations.len() as u16).map(LockTarget::Station);
        units.chain(vehicles).chain(turrets).chain(stations)
    }

    pub(super) fn lock_clear(
        &self,
        probe: &LockProbe,
        own: VehicleHandle,
        own_building: Option<u32>,
        pos: Vec3,
    ) -> bool {
        perception::los_seat(
            self,
            pos,
            probe.aim,
            HullFilter::Except([Some(own), probe.hull]),
            [own_building, probe.building],
        )
    }

    fn lock_pick(
        &self,
        state: &LockOn,
        team: u8,
        own: VehicleHandle,
        shooter: UnitHandle,
        (pos, dir): (Vec3, Vec3),
        (mask, max): (u32, f32),
    ) -> Option<LockTarget> {
        let own_building = self.vehicle(own).and_then(|v| v.instance);
        let mut best: Option<(f32, LockTarget)> = None;
        for t in self.lock_candidates(own, shooter) {
            if state.cycled.contains(&t) {
                continue;
            }
            let Some(p) = self.lock_probe(t) else {
                continue;
            };
            if mask & p.kind_bit == 0 || !p.team.is_some_and(|o| self.hostile(team, o)) {
                continue;
            }
            let off = p.pos - pos;
            if off.length_squared() > max * max {
                continue;
            }
            let score = lock_score(off, dir);
            if best.is_some_and(|(b, _)| b <= score) {
                continue;
            }
            if self.lock_clear(&p, own, own_building, pos) {
                best = Some((score, t));
            }
        }
        best.map(|(_, t)| t)
    }

    pub(super) fn tick_lock_on(&mut self, pads: &[PadState], dt: f32) {
        if self.lock_on.len() < self.players.len() {
            self.lock_on.resize(self.players.len(), LockOn::default());
        }
        for slot in 0..self.players.len() {
            let mounted = self.players[slot]
                .filter(|&h| self.flyer_of(h).is_some())
                .and_then(|h| Some((h, self.flyer_of(h)?)));
            let Some((h, own)) = mounted else {
                if self.lock_on[slot] != LockOn::default() {
                    self.lock_on[slot] = LockOn::default();
                }
                continue;
            };
            let mut state = std::mem::take(&mut self.lock_on[slot]);
            let flyer_pos = self.vehicle(own).map_or(Vec3::ZERO, |v| v.pos);
            let mut nearest_incoming: Option<f32> = None;
            for b in &self.bolts {
                let Some(m) = b.missile else {
                    continue;
                };
                if b.hit != crate::projectile::Hit::None {
                    continue;
                }
                let targeted = match m.target {
                    Some(MissileTarget::Vehicle(vh)) => vh == own,
                    Some(MissileTarget::Unit(uh)) => uh == h,
                    _ => false,
                };
                if targeted {
                    let d = (b.pos - flyer_pos).length();
                    nearest_incoming = Some(nearest_incoming.map_or(d, |min| min.min(d)));
                }
            }
            state.incoming_missile = nearest_incoming.is_some();
            state.incoming_distance = nearest_incoming.unwrap_or(0.0);
            let Some(gun) = self.lock_launcher(h) else {
                self.lock_on[slot] = state;
                continue;
            };
            let c = &self.weapon_classes[gun];
            let (mask, max, lose) = (c.target_kind, c.target_lock_max, c.target_lock_lose);
            let team = self.units[h.index as usize].team;
            let ray = self.flyer_ray(h);
            let pressed = pads
                .get(h.index as usize)
                .is_some_and(|p| p.pressed(buttons::RELOAD));
            let edge = pressed && !state.down;
            state.down = pressed;
            state.button = if pressed { state.button + dt } else { 0.0 };
            if edge {
                let mut next = self.lock_pick(&state, team, own, h, ray, (mask, max));
                if next.is_none() && !state.cycled.is_empty() {
                    state.cycled.clear();
                    next = self.lock_pick(&state, team, own, h, ray, (mask, max));
                }
                if let Some(t) = next {
                    if state.cycled.len() == LOCK_CYCLE_MEMORY {
                        state.cycled.remove(0);
                    }
                    state.cycled.push(t);
                    state.target = Some(t);
                    state.held = [0.0; LOCK_CHANNELS];
                }
            } else if pressed && state.button >= LOCK_CLEAR_HOLD_SECS {
                state.target = None;
                state.cycled.clear();
            }
            if let Some(t) = state.target {
                let keep = self.lock_probe(t).is_some_and(|p| {
                    mask & p.kind_bit != 0
                        && p.team.is_some_and(|o| self.hostile(team, o))
                        && (p.pos - ray.0).length_squared() <= lose * lose
                });
                if !keep {
                    state.target = None;
                }
            }
            for k in 0..LOCK_CHANNELS {
                let limit = state
                    .target
                    .and_then(|t| self.lock_dwell_limit(h, own, k, t, ray));
                state.charging[k] = limit.is_some();
                state.held[k] = limit.map_or(0.0, |l| (state.held[k] + dt).min(l));
            }
            self.lock_on[slot] = state;
        }
    }

    fn lock_dwell_limit(
        &self,
        h: UnitHandle,
        own: VehicleHandle,
        weapon: usize,
        target: LockTarget,
        (pos, dir): (Vec3, Vec3),
    ) -> Option<f32> {
        let setup = self.seat_gun_setup(h)?;
        let c = &self.weapon_classes[setup.weapons.get(weapon)?.class];
        if c.lock_time <= 0.0 || c.ordnance.missile.is_none() {
            return None;
        }
        let p = self.lock_probe(target)?;
        let raw = match target {
            LockTarget::Station(_) => BUILDING_KIND_BIT,
            LockTarget::Aim(_) => p.kind_bit,
        };
        if c.target_kind & raw == 0 && c.target_kind & p.kind_bit == 0 {
            return None;
        }
        if (p.pos - pos).normalize_or_zero().dot(dir) < LOCK_FIRE_COS {
            return None;
        }
        let own_building = self.vehicle(own).and_then(|v| v.instance);
        self.lock_clear(&p, own, own_building, pos)
            .then_some(c.lock_time)
    }

    pub(super) fn acquire_flyer_pick(&mut self, slot: usize, ph: UnitHandle) {
        let Some(own) = self.flyer_of(ph) else {
            return;
        };
        let Some(gun) = self.lock_gun(ph) else {
            return;
        };
        let c = &self.weapon_classes[gun];
        let (extreme, angle) = (c.extreme_range, c.lock_on_angle);
        let team = self.units[ph.index as usize].team;
        let (pos, dir) = self.flyer_ray(ph);
        let own_building = self.vehicle(own).and_then(|v| v.instance);
        let mut best: Option<(f32, AimTarget, i8)> = None;
        for t in self.lock_candidates(own, ph) {
            let Some(p) = self.lock_probe(t) else {
                continue;
            };
            let score = cone_score(p.radius, angle, extreme, pos, dir, p.pos);
            if score < 0.0 || best.is_some_and(|(b, _, _)| b <= score) {
                continue;
            }
            let end = p.aim + (pos - p.aim).normalize_or_zero() * p.radius;
            let clear = perception::los_seat(
                self,
                pos,
                end,
                HullFilter::Except([Some(own), p.hull]),
                [own_building, p.building],
            );
            if !clear {
                continue;
            }
            let pick = match t {
                LockTarget::Aim(a) => a,
                LockTarget::Station(s) => AimTarget::Station(s),
            };
            best = Some((score, pick, self.relation_to(team, p.team)));
        }
        self.reticle_pick[slot] = best.map(|(_, t, r)| (t, r));
    }

    pub(super) fn flyer_ray(&self, h: UnitHandle) -> (Vec3, Vec3) {
        match self.seat_look(h) {
            Some(look) => (look.eye, look.view),
            None => self.units[h.index as usize].aim_ray(self.ground_rules, self.views()),
        }
    }

    pub(super) fn relation_to(&self, team: u8, other: Option<u8>) -> i8 {
        match other {
            Some(o) if self.allied(team, o) => 1,
            Some(o) if self.hostile(team, o) => -1,
            _ => 0,
        }
    }

    pub fn flyer_of(&self, h: UnitHandle) -> Option<VehicleHandle> {
        let m = self.unit(h)?.mount?;
        let v = self.vehicle(m.vehicle)?;
        self.flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map(|_| m.vehicle)
    }

    pub(super) fn lock_gun(&self, h: UnitHandle) -> Option<usize> {
        let setup = self.seat_gun_setup(h)?;
        setup.weapons.first().map(|w| w.class)
    }

    /// The weapon the dwell lock charges: the first launcher (a class with a
    /// lock time and a missile), else the first weapon. `lock_gun` stays the
    /// reticle magnet's first weapon.
    pub(super) fn lock_launcher(&self, h: UnitHandle) -> Option<usize> {
        let setup = self.seat_gun_setup(h)?;
        setup
            .weapons
            .iter()
            .map(|w| w.class)
            .find(|c| {
                let wc = &self.weapon_classes[*c];
                wc.lock_time > 0.0 && wc.ordnance.missile.is_some()
            })
            .or_else(|| setup.weapons.first().map(|w| w.class))
    }

    pub fn lock_on_target(&self, slot: usize) -> Option<(LockTarget, Vec3)> {
        let t = self.lock_on.get(slot)?.target?;
        Some((t, self.lock_probe(t)?.pos))
    }

    pub fn lock_on_visible(&self, slot: usize) -> bool {
        let Some(t) = self.lock_on.get(slot).and_then(|s| s.target) else {
            return false;
        };
        let Some(h) = self.players.get(slot).copied().flatten() else {
            return false;
        };
        let (Some(p), Some(own)) = (self.lock_probe(t), self.flyer_of(h)) else {
            return false;
        };
        let pos = self.flyer_ray(h).0;
        let own_building = self.vehicle(own).and_then(|v| v.instance);
        self.lock_clear(&p, own, own_building, pos)
    }

    pub fn lock_on_charge(&self, slot: usize, weapon: usize) -> Option<f32> {
        let state = self.lock_on.get(slot)?;
        if !*state.charging.get(weapon)? {
            return None;
        }
        let h = self.players.get(slot).copied().flatten()?;
        let class = self.seat_gun_setup(h)?.weapons.get(weapon)?.class;
        let limit = self.weapon_classes.get(class)?.lock_time;
        (limit > 0.0).then(|| (state.held[weapon] / limit).clamp(0.0, 1.0))
    }

    pub fn incoming_missile(&self, slot: usize) -> Option<f32> {
        let state = self.lock_on.get(slot)?;
        if state.incoming_missile {
            Some(state.incoming_distance)
        } else {
            None
        }
    }

    pub(super) fn seat_lock_target(
        &mut self,
        unit: usize,
        class: usize,
        origin: Vec3,
        dir: Vec3,
    ) -> Option<MissileTarget> {
        let Controller::Player { slot } = self.units[unit].controller else {
            return None;
        };
        let slot = slot as usize;
        let channel = self
            .seat_gun_setup(self.handle(unit))?
            .weapons
            .iter()
            .position(|w| w.class == class)
            .filter(|k| *k < LOCK_CHANNELS);
        if let Some(target) = channel.and_then(|k| self.lock_on.get(slot)?.salvo_target[k]) {
            return Some(target);
        }
        let t = self.lock_on.get(slot)?.target?;
        let p = self.lock_probe(t)?;
        let c = &self.weapon_classes[class];
        let raw = match t {
            LockTarget::Station(_) => BUILDING_KIND_BIT,
            LockTarget::Aim(_) => p.kind_bit,
        };
        let lock_time = c.lock_time;
        let launcher = lock_time > 0.0;
        if c.target_kind & raw == 0 && !(launcher && c.target_kind & p.kind_bit != 0) {
            return None;
        }
        if (p.pos - origin).normalize_or_zero().dot(dir) < LOCK_FIRE_COS {
            return None;
        }
        if launcher && channel.is_none_or(|k| self.lock_on[slot].held[k] < lock_time) {
            return None;
        }
        let own = self.units[unit].mount?.vehicle;
        let own_building = self.vehicle(own).and_then(|v| v.instance);
        if !self.lock_clear(&p, own, own_building, origin) {
            return None;
        }
        if let Some(k) = channel.filter(|_| launcher) {
            self.lock_on[slot].held[k] = 0.0;
            self.lock_on[slot].salvo_target[k] = t.missile();
        }
        t.missile()
    }

    pub(super) fn lock_on_hash(&self, eat: &mut impl FnMut(&[u8])) {
        for state in &self.lock_on {
            // A salvo's target outlives a cleared lock and steers its missiles.
            for s in state.salvo_target.iter().flatten() {
                eat(b"SALVO");
                eat(&format!("{s:?}").into_bytes());
            }
            let Some(t) = state.target else {
                continue;
            };
            eat(b"LOCKON");
            match t {
                LockTarget::Station(s) => eat(&s.to_le_bytes()),
                LockTarget::Aim(AimTarget::Unit(h)) => eat(&h.index.to_le_bytes()),
                LockTarget::Aim(AimTarget::Vehicle(h)) => eat(&h.index.to_le_bytes()),
                LockTarget::Aim(AimTarget::Turret(id)) => eat(&id.to_le_bytes()),
                LockTarget::Aim(AimTarget::Droid(d)) => eat(&format!("{d:?}").into_bytes()),
                LockTarget::Aim(AimTarget::Station(s)) => eat(&s.to_le_bytes()),
            }
            for held in state.held {
                eat(&held.to_bits().to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests;
