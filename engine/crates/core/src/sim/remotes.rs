//! The remote droid in the sim: launch, control routing and hand-back, flight,
//! blaster, destruct, damage, salvo runs and the bot's droid pad (card DROIDAI).
//! `free_shots` fires the ownerless-gun bolts the auto-turret shares. Follows
//! `analysis/re-remote-droid.md`; friendly-fire checks are card TKFIX.

use super::{buttons, Event, PadState, Sim};
use crate::collision::{Capsule, HullFilter};
use crate::fmath::FloatExt;
use crate::projectile::{Bolt, Shooter};
use crate::remote::{
    Droid, DroidClass, DroidEnd, StrikeRun, BOT_BLAST, BOT_CROWD, BOT_FIGHT, BOT_FLIGHT, BOT_GUARD,
    BOT_LOW, BOT_RANGE, BOT_RECALL, BOT_REST, RADIUS,
};
use crate::unit::{Controller, UnitHandle};
use crate::weapon::{shot_direction, FireInput};
use glam::Vec3;

/// The presses that give a flown droid back to its soldier: the exit button or
/// a weapon switch forward on either channel (B2 `FUN_001ee668` B2:101050-101103,
/// B1 `FUN_001c9398` B1:73480-73540; `analysis/re-remote-droid.md` 6).
const HAND_BACK: u32 = buttons::ACTION | buttons::SWITCH_WEAPON | buttons::SWITCH_SECONDARY;

/// Card DROIDAI: what a bot's flight does this tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BotFlight {
    /// Flies on.
    Fly,
    /// Starts the destruct's countdown and flies on.
    Destruct,
    /// Hands control back: the body goes in itself.
    Home,
    /// Starts the countdown and hands control back.
    DestructHome,
}

impl Sim {
    pub fn controlled_droid(&self, h: UnitHandle) -> Option<&Droid> {
        self.droids.iter().find(|d| d.owner == h && d.controlled)
    }

    /// The droid `id`, while it flies.
    pub fn live_droid(&self, id: u64) -> Option<&Droid> {
        self.droids.iter().find(|d| d.id == id && d.alive())
    }

    pub fn droid_class(&self, d: &Droid) -> Option<&DroidClass> {
        self.weapon_classes[d.weapon_class]
            .remote
            .as_ref()
            .map(|r| &r.droid)
    }

    pub fn droid_of(&self, h: UnitHandle, weapon: usize) -> Option<&Droid> {
        self.droids
            .iter()
            .find(|d| d.owner == h && d.weapon_class == weapon)
    }

    pub(super) fn droid_input(
        &mut self,
        i: usize,
        slot: Option<usize>,
        weapon: usize,
        held: bool,
        blocked: bool,
    ) -> bool {
        let owner = self.handle(i);
        let droid = self
            .droids
            .iter()
            .position(|d| d.owner == owner && d.weapon_class == weapon);
        let state = match slot {
            Some(k) => &mut self.units[i].held[k].state,
            None => &mut self.units[i].weapon,
        };
        let fresh = held && state.remote_press == 0;
        state.remote_press = u8::from(held);
        let Some(k) = droid else {
            return false;
        };
        let take = self.weapon_classes[weapon]
            .remote
            .as_ref()
            .is_some_and(|r| r.take_control);
        if fresh && take && !blocked && !self.droids[k].controlled {
            let d = &mut self.droids[k];
            d.controlled = true;
            d.idle = 0.0;
            d.prev_buttons = self.units[i].soldier.prev_buttons() | buttons::SECONDARY_FIRE;
        }
        true
    }

    pub(super) fn launch_droid(&mut self, i: usize, weapon: usize, charge: f32) {
        let Some(r) = self.weapon_classes[weapon].remote.clone() else {
            return;
        };
        let owner = self.handle(i);
        let (pos, vel) = self.throw_from(i, weapon, r.launch_speed(charge));
        let u = &self.units[i];
        let (team, yaw, human) = (u.team, u.soldier.yaw, u.is_player);
        self.next_droid_id = self
            .next_droid_id
            .checked_add(1)
            .expect("droid identity exhausted");
        let mut droid = Droid::new(
            self.next_droid_id,
            owner,
            team,
            weapon,
            &r,
            pos,
            vel,
            yaw,
            human,
        );
        droid.prev_buttons = self.units[i].soldier.prev_buttons();
        self.droids.push(droid);
        self.events.push(Event::Throw {
            unit: owner,
            weapon_class: weapon as u32,
            origin: pos,
        });
    }

    pub(super) fn droid_pads(&mut self, unit_pads: &mut [PadState]) -> Vec<Option<PadState>> {
        (0..self.droids.len())
            .map(|k| self.droid_pad(k, unit_pads))
            .collect()
    }

    fn droid_pad(&mut self, k: usize, unit_pads: &mut [PadState]) -> Option<PadState> {
        let d = &self.droids[k];
        if !d.controlled {
            return None;
        }
        let i = d.owner.index as usize;
        if self.unit(d.owner).is_none_or(|u| u.mount.is_some()) {
            self.droids[k].controlled = false;
            return None;
        }
        let bot = self.units[i].controller == Controller::Ai;
        let (pad, flight) = if bot {
            self.bot_droid_pad(k)
        } else {
            (
                unit_pads.get(i).copied().unwrap_or_default(),
                BotFlight::Fly,
            )
        };
        if matches!(flight, BotFlight::Destruct | BotFlight::DestructHome) {
            self.start_charge(k);
        }
        let owner = self.droids[k].owner;
        self.take_remote_pad(owner, unit_pads);
        let d = &mut self.droids[k];
        let exit = HAND_BACK & pad.buttons & !d.prev_buttons != 0;
        let bot_done = matches!(flight, BotFlight::Home | BotFlight::DestructHome);
        if exit || bot_done {
            d.controlled = false;
            let owner = d.owner;
            self.release_droid(owner, pad.buttons);
            return None;
        }
        Some(pad)
    }

    /// The destruct's countdown, started by a bot (a player's starts on his
    /// press, in [`Sim::tick_droids`]).
    fn start_charge(&mut self, k: usize) {
        let d = &self.droids[k];
        let has = self.weapon_classes[d.weapon_class]
            .remote
            .as_ref()
            .is_some_and(|r| r.destruct.is_some());
        if !has || d.charge.is_some() {
            return;
        }
        let (pos, weapon_class) = (d.pos, d.weapon_class as u32);
        self.droids[k].charge = Some(0.0);
        self.events.push(Event::DroidCharge { pos, weapon_class });
    }

    /// Enemies on foot within `r` of `at`.
    fn enemies_near(&self, team: u8, at: Vec3, r: f32) -> usize {
        self.alive_units()
            .filter(|(_, u)| self.hostile(team, u.team) && u.mount.is_none())
            .filter(|(_, u)| (u.soldier.pos + Vec3::Y).distance_squared(at) <= r * r)
            .count()
    }

    /// Card DROIDAI [ours]: a bot's droid flight, one tick. The body comes
    /// first: hit, or an enemy within `BOT_GUARD` with sight of it, ends the
    /// flight; so do `BOT_FLIGHT` in the air and `BOT_RECALL` with no enemy
    /// near the droid. An ending flight sets off the destruct when an enemy
    /// is within `BOT_BLAST` of the droid. Flying, it shoots the nearest
    /// enemy it sees within `BOT_RANGE`, else flies at the nearest enemy the
    /// bot knows of; among `BOT_CROWD` enemies, or nearly dead
    /// (`BOT_LOW`) beside one, it destructs.
    fn bot_droid_pad(&self, k: usize) -> (PadState, BotFlight) {
        let d = &self.droids[k];
        let Some(r) = self.weapon_classes[d.weapon_class].remote.as_ref() else {
            return (PadState::default(), BotFlight::Home);
        };
        // Card TKFIX (ours): no destruct with a friend on foot in its reach.
        let has_destruct = r.destruct.is_some()
            && d.charge.is_none()
            && !crate::ai::friend_near_blast(self, d.team, d.pos, self.destruct_reach(d));
        let near = self.enemies_near(d.team, d.pos, BOT_BLAST);
        let u = &self.units[d.owner.index as usize];
        // (Hit on this tick or the last: the damage pass runs after this.)
        let hit = u.last_hit.is_some_and(|(_, t)| t + 1 >= self.tick);
        let body = u.capsule().center();
        let seen = || {
            self.alive_units().any(|(_, e)| {
                self.hostile(u.team, e.team)
                    && e.soldier.pos.distance_squared(u.soldier.pos) <= BOT_GUARD * BOT_GUARD
                    && crate::perception::los(self, e.muzzle(), body)
            })
        };
        if hit || d.idle > BOT_RECALL || d.age >= BOT_FLIGHT || seen() {
            let flight = if has_destruct && near > 0 {
                BotFlight::DestructHome
            } else {
                BotFlight::Home
            };
            return (PadState::default(), flight);
        }
        let low = d.health < r.droid.max_health * BOT_LOW;
        let flight = if has_destruct && (near >= BOT_CROWD || (low && near > 0)) {
            BotFlight::Destruct
        } else {
            BotFlight::Fly
        };
        let blaster = &self.weapon_classes[r.blaster_index];
        let eye = d.pos;
        let seen_target = self
            .droid_enemies(d)
            .filter(|p| crate::perception::los(self, eye, *p))
            .min_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        // Else the nearest enemy the bot (or a friend who told it) knows of.
        let known = || {
            u.ai.contacts
                .iter()
                .flatten()
                .filter(|c| c.handle.live_unit(self).is_some_and(|e| e.alive))
                .map(|c| c.last_pos + Vec3::Y)
                .filter(|p| p.distance_squared(u.soldier.pos) <= BOT_FIGHT * BOT_FIGHT)
                .min_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)))
        };
        let Some(target) = seen_target.or_else(known) else {
            return (PadState::default(), flight);
        };
        let to = target - eye;
        let dist = to.length();
        let want_yaw = crate::fmath::atan2(-to.x, -to.z);
        let want_pitch = crate::fmath::atan2(to.y, to.with_y(0.0).length());
        let yaw_err = crate::vehicle::gun::wrap(want_yaw - d.yaw);
        let pitch_err = want_pitch - d.pitch;
        let [_, optimal, max] = blaster.ai_range;
        let forward = if seen_target.is_none() || dist > optimal {
            1.0
        } else if dist < optimal * 0.5 {
            -0.5
        } else {
            0.0
        };
        // Card TKFIX (ours): no shot through a friend on foot, as a bot's.
        let fire = seen_target.is_some()
            && yaw_err.abs() < 0.15
            && dist <= max
            && d.prev_buttons & buttons::FIRE == 0
            && !crate::ai::item_line_blocked(
                self,
                d.team,
                d.muzzle(),
                d.aim_frame(),
                target,
                d.blaster
                    .yaw_spread(blaster)
                    .fmax(d.blaster.pitch_spread(blaster)),
            );
        let pad = PadState {
            ly: PadState::quantize(forward),
            rx: PadState::quantize(-yaw_err * 2.0),
            ry: PadState::quantize(pitch_err * 2.0),
            buttons: if fire { buttons::FIRE } else { 0 },
            ..PadState::default()
        };
        (pad, flight)
    }

    /// Card TKFIX: the farthest a droid's destruct hurts from where it is (m):
    /// its burst, its own explosion, and its strike run's shells (their
    /// scatter round the droid and their blast).
    fn destruct_reach(&self, d: &Droid) -> f32 {
        let Some(r) = self.weapon_classes[d.weapon_class].remote.as_ref() else {
            return 0.0;
        };
        let x = r.destruct.as_ref();
        let burst = x
            .and_then(|x| x.burst.as_ref())
            .map_or(0.0, |e| e.damage_outer);
        let own = r.droid.explosion.as_ref().map_or(0.0, |e| e.damage_outer);
        let salvo = x.and_then(|x| x.salvo.as_ref()).map_or(0.0, |s| {
            let shell = self.weapon_classes[r.shell_index]
                .ordnance
                .explosion
                .as_ref();
            2.0 * s.spread + shell.map_or(0.0, |e| e.damage_outer)
        });
        burst.fmax(own).fmax(salvo)
    }

    fn droid_enemies(&self, d: &Droid) -> impl Iterator<Item = Vec3> + '_ {
        let (team, eye) = (d.team, d.pos);
        self.alive_units()
            .filter(move |(_, u)| {
                self.hostile(team, u.team) && u.mount.is_none() && u.cloak.visible()
            })
            .map(|(_, u)| u.soldier.pos + Vec3::Y)
            .filter(move |p| p.distance_squared(eye) <= BOT_RANGE * BOT_RANGE)
    }

    pub(super) fn tick_droids(&mut self, dt: f32, pads: &[Option<PadState>]) {
        self.tick_strikes(dt);
        let mut k = 0;
        let mut pad_at = 0;
        while k < self.droids.len() {
            let routed = pads.get(pad_at).copied().flatten();
            pad_at += 1;
            let mut d = self.droids.remove(k);
            let Some(r) = self.weapon_classes[d.weapon_class].remote.as_deref() else {
                continue;
            };
            let (c, blaster) = (&r.droid, r.blaster_index);
            let charge_time = r.destruct.as_ref().map(|x| x.charge_time);
            let owner = self
                .unit(d.owner)
                .map(|u| (u.is_player, u.look_scale(), u.controller, self.cheats_of(u)));
            let Some((is_player, look, controller, owner_cheats)) = owner else {
                // [ours] The owner died: the droid vanishes, no explosion.
                self.droid_gone(&d, DroidEnd::OwnerDied);
                continue;
            };
            let pad = routed.unwrap_or_default();
            if controller == Controller::Ai && d.controlled {
                d.idle = if self.droid_enemies(&d).next().is_some() {
                    0.0
                } else {
                    d.idle + dt
                };
            }
            d.yaw -= PadState::axis(pad.rx) * c.max_turn_speed * dt;
            let pitch_rate = if controller == Controller::Ai {
                c.max_pitch_speed
            } else {
                c.max_turn_speed
            };
            d.pitch += PadState::axis(pad.ry) * pitch_rate * dt;
            (d.yaw, d.pitch) = d.mouse_turned(c, pad.look(), look);
            let (floor, normal) = self.ground_point_under_n(d.pos, d.pos.y);
            let ground = (floor > crate::collision::NO_GROUND).then_some((d.pos.y - floor, normal));
            d.fly(
                c,
                dt,
                PadState::axis(pad.ly),
                PadState::axis(pad.lx),
                ground,
            );
            self.droid_walls(&mut d);
            // [ours] A 90 s clock, apart from health; the timeout is quiet.
            let mut end = if d.alive() {
                d.age_by(dt)
            } else {
                Some(DroidEnd::Destroyed)
            };
            // The PS2 tests what a member controls: the droid is in that slot from
            // hand-over to hand-back (`re-boundary.md` T2b), so one left flying is
            // not tested. The cheat spares a human's own, as the boundary's kill
            // does (T1).
            let time = crate::boundary::leaving_time(self.ground_rules);
            let outside = d.controlled && self.boundary.outside(d.pos.x, d.pos.z);
            if d.tick_leaving(outside, time, dt)
                && end.is_none()
                && !(owner_cheats.invincible && controller != Controller::Ai)
            {
                end = Some(DroidEnd::Destroyed);
            }
            if d.tick_adrift(floor <= crate::collision::NO_GROUND, dt) && end.is_none() {
                end = Some(DroidEnd::Destroyed);
            }
            let press = pad.pressed(buttons::SECONDARY_FIRE)
                && d.prev_buttons & buttons::SECONDARY_FIRE == 0;
            if let Some(t) = d.charge {
                d.charge = Some(t + dt);
            } else if charge_time.is_some() && press && end.is_none() {
                d.charge = Some(0.0);
                self.events.push(Event::DroidCharge {
                    pos: d.pos,
                    weapon_class: d.weapon_class as u32,
                });
            }
            if d.charge.zip(charge_time).is_some_and(|(t, max)| t >= max) && end.is_none() {
                end = Some(DroidEnd::Destruct);
            }
            let fire = pad.pressed(buttons::FIRE) && end.is_none();
            let mut shots = Vec::new();
            let input = FireInput {
                held: fire,
                blocked: false,
                infinite_ammo: false,
                no_reload: false,
                ai_dispersion: None,
            };
            d.blaster.update(
                dt,
                &self.weapon_classes[blaster],
                input,
                &mut self.lcg,
                &mut shots,
            );
            // BF1's droid blaster authors a click. PROVISIONAL [D], measure:
            // a PS2 log of a press on a human's overheated droid blaster (the
            // droid's own player index was not read).
            if is_player
                && d.blaster
                    .press_clicks(&self.weapon_classes[blaster], input, !shots.is_empty())
            {
                self.events.push(Event::FireEmpty {
                    unit: d.owner,
                    weapon_class: blaster as u32,
                    pos: d.pos,
                });
            }
            if !shots.is_empty() {
                let who = Shooter {
                    owner: self.item_owner(d.owner, d.team),
                    team: d.team,
                    weapon_class: blaster,
                    is_player,
                };
                self.free_shots(who, d.muzzle(), d.aim_frame(), &shots);
            }
            if let Some(p) = routed {
                d.prev_buttons = p.buttons;
            }
            if d.pos.y < crate::sticky::WORLD_FLOOR {
                end = Some(DroidEnd::Destroyed);
            }
            if let Some(end) = end {
                if end == DroidEnd::Destruct {
                    self.destruct(&d);
                }
                if d.controlled {
                    self.release_droid(d.owner, d.prev_buttons);
                }
                self.droid_gone(&d, end);
                continue;
            }
            self.droids.insert(k, d);
            k += 1;
        }
    }

    pub(super) fn take_remote_pad(&self, owner: UnitHandle, pads: &mut [PadState]) {
        if let Some(pad) = pads.get_mut(owner.index as usize) {
            *pad = PadState::default();
        }
    }

    /// The owner takes his own pad back from a remote (droid or guided
    /// rocket); the held buttons do not re-press.
    pub(super) fn release_remote(&mut self, owner: UnitHandle, buttons: u32) {
        if let Some(u) = self.unit_mut(owner) {
            u.soldier.set_prev_buttons(buttons);
        }
    }

    /// [`Sim::release_remote`] for a droid flight.
    fn release_droid(&mut self, owner: UnitHandle, buttons: u32) {
        self.release_remote(owner, buttons);
        let rest = self.tick + (BOT_REST * crate::TICK_HZ as f32) as u64;
        if let Some(u) = self.unit_mut(owner) {
            // Card DROIDAI: a bot rests from its droid after a flight.
            if u.controller == Controller::Ai {
                u.ai.droid_rest = rest;
            }
        }
    }

    fn free_bolt(&self, origin: Vec3, dir: Vec3, speed: f32, life: f32, who: Shooter) -> Bolt {
        Bolt::fire(
            origin,
            dir,
            speed,
            life,
            who,
            self.heightfield.as_ref(),
            self.water_plane(),
            &self.collision,
            HullFilter::All,
        )
        .shaped(&self.weapon_classes[who.weapon_class].ordnance)
    }

    fn droid_walls(&self, d: &mut Droid) {
        if let Some(hf) = &self.heightfield {
            // [ours] Issue #59: the sheet's end stops a droid. The PS2 has no such
            // rule, and on a map with no boundary its droid flies on past the
            // terrain until its energy runs out.
            let (x, z) = hf.clamp_xz(d.pos.x, d.pos.z);
            if x != d.pos.x {
                (d.pos.x, d.vel.x) = (x, 0.0);
            }
            if z != d.pos.z {
                (d.pos.z, d.vel.z) = (z, 0.0);
            }
            let t = hf.height_at(d.pos.x, d.pos.z) + RADIUS;
            if d.pos.y < t && !self.collision.is_cut(d.pos) {
                d.pos.y = t;
                d.vel.y = d.vel.y.fmax(0.0);
            }
        }
        if self.collision.is_empty() {
            return;
        }
        let cap = Capsule {
            base: d.pos - Vec3::Y * RADIUS,
            height: 2.0 * RADIUS,
            radius: RADIUS,
        };
        let push = self.collision.push_out(&cap);
        let len = push.length();
        if len > 0.0 {
            let n = push / len;
            d.pos += push;
            let vn = d.vel.dot(n);
            if vn < 0.0 {
                d.vel -= n * vn;
            }
        }
    }

    pub fn is_item_gun(&self, weapon_class: usize) -> bool {
        self.weapon_classes.iter().any(|w| {
            w.remote
                .as_ref()
                .is_some_and(|r| r.blaster_index == weapon_class)
                || w.turret
                    .as_ref()
                    .is_some_and(|t| t.item.gun_index == weapon_class)
        })
    }

    pub(super) fn free_shots(
        &mut self,
        who: Shooter,
        origin: Vec3,
        (fwd, right, up): (Vec3, Vec3, Vec3),
        shots: &[crate::weapon::ShotSpec],
    ) {
        for spec in shots {
            let dir = shot_direction(
                &mut self.lcg,
                fwd,
                right,
                up,
                spec.pitch_spread,
                spec.yaw_spread,
                spec.tilt,
            );
            let blend = self.weapon_classes[who.weapon_class]
                .ordnance
                .charged(spec.charge);
            let mut bolt = self.free_bolt(origin, dir, blend.velocity, blend.life_span, who);
            self.note_bolt_shot(&mut bolt, origin, dir);
            self.bolts.push(bolt);
        }
    }

    fn destruct(&mut self, d: &Droid) {
        let Some(x) = self.weapon_classes[d.weapon_class]
            .remote
            .as_ref()
            .and_then(|r| r.destruct.as_ref())
        else {
            return;
        };
        let strike = x.salvo.as_ref().map(|s| StrikeRun {
            owner: d.owner,
            team: d.team,
            weapon_class: d.weapon_class,
            target: d.pos,
            left: s.count,
            timer: s.initial,
        });
        let burst = x.burst.clone();
        self.strikes.extend(strike);
        if let Some(e) = burst {
            let owner = self.item_owner(d.owner, d.team);
            self.explode(&e, d.pos, Some(owner), Some(d.team));
        }
    }

    fn droid_gone(&mut self, d: &Droid, end: DroidEnd) {
        self.events.push(Event::DroidGone {
            pos: d.pos,
            weapon_class: d.weapon_class as u32,
            end,
        });
        let e = self.droid_class(d).and_then(|c| c.explosion.clone());
        if let Some(e) = e.filter(|_| end.explodes()) {
            let owner = self.item_owner(d.owner, d.team);
            self.explode(&e, d.pos, Some(owner), Some(d.team));
        }
    }

    fn tick_strikes(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.strikes.len() {
            self.strikes[k].timer -= dt;
            if self.strikes[k].timer > 0.0 {
                k += 1;
                continue;
            }
            let run = self.strikes[k].clone();
            let Some(r) = self.weapon_classes[run.weapon_class].remote.as_deref() else {
                self.strikes.remove(k);
                continue;
            };
            let Some(s) = r.destruct.as_ref().and_then(|x| x.salvo.as_ref()) else {
                self.strikes.remove(k);
                continue;
            };
            let draws = [
                self.lcg.unit(),
                self.lcg.unit(),
                self.lcg.unit(),
                self.lcg.unit(),
            ];
            let (origin, dir) = s.shot(run.target, draws);
            let (shell, delay) = (r.shell_index, s.delay);
            let ord = &self.weapon_classes[shell].ordnance;
            let owner = self.item_owner(run.owner, run.team);
            let who = Shooter {
                owner,
                team: run.team,
                weapon_class: shell,
                is_player: self
                    .units
                    .get(owner.index as usize)
                    .is_some_and(|u| u.is_player),
            };
            let bolt = self.free_bolt(origin, dir, ord.velocity, ord.life_span, who);
            self.bolts.push(bolt);
            let run = &mut self.strikes[k];
            run.left -= 1;
            run.timer += delay;
            if run.left == 0 {
                self.strikes.remove(k);
            } else {
                k += 1;
            }
        }
    }

    pub(super) fn droid_damage(
        &self,
        d: &Droid,
        damage: f32,
        scales: &[f32; 7],
        team: Option<u8>,
    ) -> f32 {
        let ht = self
            .droid_class(d)
            .map_or(crate::weapon::HealthType::Droid, |c| c.health_type);
        self.item_damage(damage, scales, Some(ht), d.team, team)
    }
}

#[cfg(test)]
mod tests;
