//! Guided rockets in the sim: launch, player and bot control routing, swept
//! collision, detonation and control return. Follows
//! `analysis/re-guided-rockets.md` (card GUIDED).

use super::{buttons, Event, PadState, Sim};
use crate::collision::{HullFilter, Layers};
use crate::guided::{GuidedClass, GuidedEnd, GuidedRocket, COLLISION_RADIUS_PROVISIONAL as RADIUS};
use crate::projectile::{Hit, Target};
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::hover::HoverInput;
use crate::weapon::DamageDesc;
use glam::Vec3;

impl Sim {
    pub fn guided_class(&self, r: &GuidedRocket) -> Option<&GuidedClass> {
        self.weapon_classes[r.weapon_class]
            .guided
            .as_ref()
            .map(|g| &g.class)
    }

    pub fn controlled_guided(&self, owner: UnitHandle) -> Option<&GuidedRocket> {
        self.guided
            .iter()
            .find(|r| r.owner == owner && r.pending_end.is_none())
    }

    pub fn flown_guided(&self, owner: UnitHandle) -> Option<&GuidedRocket> {
        self.controlled_guided(owner).filter(|r| r.live())
    }

    pub(super) fn guided_input(&mut self, i: usize, slot: Option<usize>, held: bool) -> bool {
        let u = &self.units[i];
        let class = slot.map_or(u.weapon_class, |k| u.held[k].class);
        let wc = &self.weapon_classes[class];
        if wc.guided.is_none() {
            return false;
        }
        let locked = self.weapon_locked(u, class);
        let has = self
            .guided
            .iter()
            .any(|r| r.owner == self.handle(i) && r.weapon_class == class);
        let u = &mut self.units[i];
        let state = match slot {
            Some(k) => &mut u.held[k].state,
            None => &mut u.weapon,
        };
        let release = state.remote_press == 2 && held;
        if !held {
            state.remote_press = 0;
        }
        locked || has || release
    }

    pub(super) fn launch_guided(&mut self, i: usize, weapon_class: usize) {
        let Some(c) = self.weapon_classes[weapon_class].guided.as_ref() else {
            return;
        };
        let u = &self.units[i];
        let owner = self.handle(i);
        if self.weapon_locked(u, weapon_class) || self.guided.iter().any(|r| r.owner == owner) {
            return;
        }
        let (fwd, right, up) = u.aim_frame();
        let offset = c.spawn_offset;
        let origin = u.muzzle();
        let pos = u.soldier.pos + right * offset.x + up * offset.y + fwd * offset.z;
        self.next_guided_id = self
            .next_guided_id
            .checked_add(1)
            .expect("guided identity exhausted");
        self.guided.push(GuidedRocket {
            id: self.next_guided_id,
            owner,
            team: u.team,
            weapon_class,
            seat: None,
            pos,
            vel: fwd * c.class.flight.min_speed,
            rot: GuidedRocket::body(fwd),
            flyer: GuidedRocket::flying(&c.class),
            health: c.class.health,
            age: 0.0,
            prev_buttons: u.soldier.prev_buttons() | buttons::FIRE,
            pending_end: None,
            spent: None,
            blind: 0.0,
        });
        let last = self.guided.len() - 1;
        if let Some((at, target)) = self.guided_contact(&self.guided[last], origin) {
            let mut r = self.guided[last].clone();
            r.pos = at;
            self.guided_strike(&mut r, target);
            self.guided[last] = r;
        }
        self.events.push(Event::Shot {
            unit: owner,
            weapon_class: weapon_class as u32,
            origin,
            dir: fwd,
        });
    }

    pub(super) fn launch_seat_guided(
        &mut self,
        i: usize,
        weapon_class: usize,
        vehicle: crate::vehicle::VehicleHandle,
        seat: u8,
        muzzle: Vec3,
        dir: Vec3,
    ) {
        let owner = self.handle(i);
        let Some(c) = self.weapon_classes[weapon_class].guided.as_ref() else {
            return;
        };
        if self.guided.iter().any(|r| r.owner == owner) {
            return;
        }
        let fwd = dir.normalize_or_zero();
        let right = {
            let cross = fwd.cross(Vec3::Y);
            if cross.length_squared() < 1e-6 {
                fwd.cross(Vec3::Z).normalize_or_zero()
            } else {
                cross.normalize_or_zero()
            }
        };
        let up = right.cross(fwd).normalize_or_zero();
        let offset = c.spawn_offset;
        let pos = muzzle + right * offset.x + up * offset.y + fwd * offset.z;
        let (speed, health, flyer) = (
            c.class.flight.min_speed,
            c.class.health,
            GuidedRocket::flying(&c.class),
        );
        let prev_buttons = self.units[i].soldier.prev_buttons() | buttons::FIRE;
        let team = self.units[i].team;
        self.next_guided_id = self
            .next_guided_id
            .checked_add(1)
            .expect("guided identity exhausted");
        self.guided.push(GuidedRocket {
            id: self.next_guided_id,
            owner,
            team,
            weapon_class,
            seat: Some((vehicle, seat)),
            pos,
            vel: fwd * speed,
            rot: GuidedRocket::body(fwd),
            flyer,
            health,
            age: 0.0,
            prev_buttons,
            pending_end: None,
            spent: None,
            blind: 0.0,
        });
        self.events.push(Event::Shot {
            unit: owner,
            weapon_class: weapon_class as u32,
            origin: muzzle,
            dir: fwd,
        });
    }

    fn guided_owner_placed(&self, r: &GuidedRocket) -> bool {
        self.unit(r.owner).is_some_and(|u| match r.seat {
            None => u.mount.is_none(),
            Some((vehicle, seat)) => u
                .mount
                .is_some_and(|m| m.vehicle == vehicle && m.seat == seat),
        })
    }

    pub(super) fn guided_pads(&mut self, pads: &mut [PadState]) -> Vec<PadState> {
        let mut routed = Vec::with_capacity(self.guided.len());
        for k in 0..self.guided.len() {
            let r = &self.guided[k];
            let owner = r.owner;
            if !self.guided_owner_placed(r) {
                self.guided[k].pending_end = Some(GuidedEnd::OwnerGone);
                routed.push(PadState::default());
                continue;
            }
            let Some(u) = self.unit(owner) else {
                self.guided[k].pending_end = Some(GuidedEnd::OwnerGone);
                routed.push(PadState::default());
                continue;
            };
            let (pad, lost) = if u.controller == Controller::Ai {
                match self.bot_guided_pad(k) {
                    Some(pad) => (pad, false),
                    None => (PadState::default(), true),
                }
            } else {
                (
                    pads.get(owner.index as usize).copied().unwrap_or_default(),
                    false,
                )
            };
            self.take_remote_pad(owner, pads);
            let r = &mut self.guided[k];
            if !r.live() {
                r.prev_buttons = pad.buttons;
                routed.push(PadState::default());
                continue;
            }
            r.blind = if lost { r.blind + crate::TICK_DT } else { 0.0 };
            let blind = r.blind >= crate::guided::BOT_BLIND - crate::TICK_DT * 0.5;
            if blind || pad.buttons & !r.prev_buttons & buttons::ACTION != 0 {
                r.pending_end = Some(GuidedEnd::Returned);
            } else if pad.buttons & !r.prev_buttons & buttons::SECONDARY_FIRE != 0 {
                r.pending_end = Some(GuidedEnd::Destroyed);
            }
            r.prev_buttons = pad.buttons;
            routed.push(pad);
        }
        routed
    }

    /// A bot's steering toward its target's aim point; `None` with no
    /// target it sees (lost, cloaked or behind cover).
    fn bot_guided_pad(&self, k: usize) -> Option<PadState> {
        let r = &self.guided[k];
        let target = self
            .unit(r.owner)
            .and_then(|u| u.ai.target)
            .and_then(|h| match h {
                crate::perception::Foe::Unit(h) => self
                    .unit(h)
                    .filter(|e| e.cloak.visible())
                    .map(|u| u.aim_point()),
                crate::perception::Foe::Droid(_) | crate::perception::Foe::Turret(_) => {
                    crate::perception::foe_motion(self, h).map(|(p, _)| p)
                }
            });
        let target = target.filter(|p| crate::perception::los(self, r.pos, *p))?;
        let local = r.frame().inverse() * (target - r.pos).normalize_or_zero();
        Some(PadState {
            rx: PadState::quantize(crate::fmath::atan2(-local.x, local.z) * 2.0),
            ry: PadState::quantize(crate::fmath::asin(local.y.clamp(-1.0, 1.0)) * 2.0),
            ..Default::default()
        })
    }

    fn guided_flight_input(&self, r: &GuidedRocket, pad: &PadState) -> HoverInput {
        let (input, _) = super::seats::driver_input(pad);
        if self
            .unit(r.owner)
            .is_some_and(|u| matches!(u.controller, Controller::Player { .. }))
        {
            if !pad.has_look() {
                return input;
            }
            let (turn, pitch) = super::look::look_steer(input.turn, input.pitch, pad.look());
            HoverInput {
                turn,
                pitch,
                ..input
            }
        } else {
            HoverInput {
                human: false,
                boost: false,
                jump: false,
                ..input
            }
        }
    }

    pub(super) fn tick_guided(&mut self, pads: &[PadState], dt: f32) {
        let count = self.guided.len();
        for k in 0..count {
            let mut r = self.guided[k].clone();
            let c = self.guided_class(&r).unwrap();
            if !self.guided_owner_placed(&r) {
                r.pending_end = Some(GuidedEnd::OwnerGone);
            } else if r.seat.is_none()
                && self
                    .unit(r.owner)
                    .is_some_and(|u| u.weapon_class != r.weapon_class)
            {
                r.pending_end = Some(GuidedEnd::Returned);
            }
            if let Some((end, held)) = r.spent {
                let held = held + dt;
                r.spent = Some((end, held));
                if r.pending_end.is_none() && held > c.hold_time {
                    r.pending_end = Some(end);
                }
                self.guided[k] = r;
                continue;
            }
            if r.health <= 0.0 && r.pending_end.is_none() {
                r.pending_end = Some(GuidedEnd::Destroyed);
            }
            if r.pending_end.is_none() && r.energy() <= 0.0 {
                let end = if c.flight.boost_speed > 0.0
                    && r.speed() >= c.flight.boost_speed * c.critical_speed
                {
                    GuidedEnd::CriticalExpiry
                } else {
                    GuidedEnd::Expired
                };
                r.spent = Some((end, dt));
                self.guided_blast(&r, end);
                self.guided[k] = r;
                continue;
            }
            if r.pending_end.is_none() {
                let previous = r.pos;
                let pad = pads.get(k).copied().unwrap_or_default();
                let input = self.guided_flight_input(&r, &pad);
                let env = self.flyer_environment(
                    input.human,
                    self.landing_coverage(r.pos, 0.0),
                    None,
                    true,
                    [0.0; 4],
                );
                r.fly(c, input, env, dt);
                if let Some((pos, target)) = self.guided_contact(&r, previous) {
                    r.pos = pos;
                    self.guided_strike(&mut r, target);
                } else if r.pos.y < -5000.0 || self.death_regions.iter().any(|v| v.contains(r.pos))
                {
                    r.pending_end = Some(GuidedEnd::Destroyed);
                }
            }
            self.guided[k] = r;
        }
        let mut k = 0;
        while k < self.guided.len() {
            let Some(end) = self.guided[k].pending_end else {
                k += 1;
                continue;
            };
            let r = self.guided.remove(k);
            if r.live() {
                self.guided_blast(&r, end);
            }
            self.release_remote(r.owner, r.prev_buttons);
            if let Some((vehicle, seat)) = r.seat {
                let gun = self
                    .vehicle_mut(vehicle)
                    .and_then(|v| v.guns.get_mut(usize::from(seat)))
                    .and_then(Option::as_mut);
                if let Some(gun) = gun {
                    for w in &mut gun.weapons {
                        w.remote_press = 2;
                    }
                }
            }
            if let Some(u) = self.unit_mut(r.owner) {
                if u.weapon_class == r.weapon_class {
                    u.weapon.remote_press = 2;
                }
                for h in &mut u.held {
                    if h.class == r.weapon_class {
                        h.state.remote_press = 2;
                    }
                }
            }
        }
    }

    fn guided_blast(&mut self, r: &GuidedRocket, end: GuidedEnd) {
        let explosion = self.weapon_classes[r.weapon_class]
            .guided
            .as_ref()
            .and_then(|c| c.explosion(end))
            .cloned();
        if let Some(e) = explosion {
            let owner = self.item_owner(r.owner, r.team);
            self.explode(&e, r.pos, Some(owner), Some(r.team));
        }
        self.events.push(Event::GuidedGone {
            pos: r.pos,
            weapon_class: r.weapon_class as u32,
            end,
        });
    }

    pub(super) fn guided_strike(&mut self, r: &mut GuidedRocket, target: Option<Target>) {
        if r.energy() <= 0.0 {
            return;
        }
        let Some(c) = self.guided_class(r) else {
            return;
        };
        let f = &c.flight;
        let [low, high] = c.contact_damage;
        let damage = if f.boost_speed <= f.max_speed {
            low
        } else {
            let s = ((r.speed() - f.max_speed) / (f.boost_speed - f.max_speed)).clamp(0.0, 1.0);
            (1.0 - s) * low + s * high
        };
        r.flyer.energy = 0.0;
        let Some(target) = target else {
            return;
        };
        let desc = DamageDesc {
            damage,
            scales: [1.0; 7],
            shield_scale: 1.0,
        };
        let owner = self.item_owner(r.owner, r.team);
        let dir = r.vel.normalize_or_zero();
        match target {
            Target::Unit(h) => {
                let shooter_is_player = self.unit(owner).is_some_and(|u| u.is_player);
                let mult = self.buff_scale(Some(owner), h);
                let Some(dealt) =
                    self.shot_strikes_unit(r.team, shooter_is_player, h, &desc, mult, false)
                else {
                    return;
                };
                self.finish_direct_damage(h, Some(owner), dealt, 0.0, dir);
            }
            Target::Vehicle(vh) => {
                self.damage_vehicle_by(vh, &desc, 1.0, Some(owner), Some(r.team), Some(dir));
            }
            Target::Station(s) => {
                self.damage_station(s, &desc, 1.0, Some(owner), Some(r.team));
            }
            t @ (Target::Droid(_) | Target::Turret(_)) => {
                self.damage_item(t, damage, &desc.scales, Some(r.team), Some(owner));
            }
            _ => {}
        }
    }

    fn guided_contact(&self, r: &GuidedRocket, from: Vec3) -> Option<(Vec3, Option<Target>)> {
        let delta = r.pos - from;
        let len = delta.length();
        if len <= 0.0 {
            return None;
        }
        let dir = delta / len;
        let mut nearest: Option<(f32, Option<Target>)> = None;
        let mut offer = |distance: f32, target: Option<Target>| {
            if distance <= len && nearest.is_none_or(|(t, _)| distance < t) {
                nearest = Some((distance, target));
            }
        };
        if let Some(hit) = self.terrain_ray(from, dir, len, crate::projectile::TERRAIN_RAY_STEP) {
            offer(hit, None);
        }
        if let Some(hit) =
            self.collision
                .ray_with(from, dir, len, Layers::ORDNANCE, HullFilter::All)
        {
            let target = match hit.owner {
                Some(v) => Some(Target::Vehicle(v)),
                None => match crate::projectile::building_hit(&self.collision, hit.instance) {
                    Hit::Target(t) => Some(t),
                    _ => None,
                },
            };
            offer(hit.t, target);
        }
        if let Some(t) = self
            .water_plane()
            .and_then(|water| water.crossing(from, dir))
        {
            offer(t, None);
        }
        for (h, u) in self
            .alive_units()
            .filter(|(h, u)| *h != r.owner && u.mount.is_none())
        {
            if let Some((t, _)) = u.bolt_prims().hit(from, dir, len) {
                offer(t, Some(Target::Unit(h)));
            }
        }
        for d in self
            .droids
            .iter()
            .filter(|d| d.owner.index != r.owner.index)
        {
            if let Some(t) =
                crate::unit::ray_sphere(from, dir, d.pos, crate::remote::RADIUS + RADIUS)
            {
                offer(t, Some(Target::Droid(d.id)));
            }
        }
        for turret in self.turrets.iter().filter(|t| t.health > 0.0) {
            if let Some(t) = crate::unit::ray_sphere(
                from,
                dir,
                turret.centre(),
                crate::autoturret::RADIUS + RADIUS,
            ) {
                offer(t, Some(Target::Turret(turret.id)));
            }
        }
        nearest.map(|(t, target)| (from + dir * t, target))
    }
}

#[cfg(test)]
mod tests;
