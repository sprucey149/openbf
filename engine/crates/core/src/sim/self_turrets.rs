//! BF2 placed `PilotType=self` seats (issue #91). No soldier is invented:
//! the building owns the controller and the ordinary mounted gun owns shots.
//! PS2 reference: re-turret-ai §§2–3, 6–7; 100 m view, range + 10 m fight,
//! 22.5 degree fire cone, idle yaw ±30 degrees every 18 thinks. The 0.5 s
//! think cadence is provisional port policy; bursts share the PS2 heat and
//! skill laws with soldier gunners.
use super::{buttons, PadState, Sim};
use crate::autoturret::{Aim, AIM_CONE, FIGHT_MARGIN, LOOK_EVERY, LOOK_SPREAD, THINK, VIEW};
use crate::collision::HullFilter;
use crate::fmath;
use crate::vehicle::{gun, Mount, VehicleHandle};
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threat {
    pub aim: Aim,
    pub seen: bool,
    pub tested_at: u64,
    pub seen_at: u64,
}

#[derive(Clone, Debug)]
pub struct SelfPilot {
    pub aim_block: crate::unit::AimBlock,
    pub threats: [Option<Threat>; THREAT_SLOTS],
    pub visible: Option<u8>,
    pub seat: u8,
    pub target: Option<Aim>,
    pub fighting: bool,
    pub next_think: u64,
    pub thinks: u32,
    pub look: f32,
    pub shots: u32,
    pub burst: u8,
    pub pause_until: u64,
}

impl SelfPilot {
    pub fn new(seat: u8) -> Self {
        Self {
            aim_block: crate::unit::AimBlock::default(),
            threats: [None; THREAT_SLOTS],
            visible: None,
            seat,
            target: None,
            fighting: false,
            next_think: 0,
            thinks: 0,
            look: 0.0,
            shots: 0,
            burst: 0,
            pause_until: 0,
        }
    }

    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"SELF");
        eat(&[self.seat, u8::from(self.fighting), self.burst]);
        if self.aim_block.axis != Vec3::ZERO {
            self.aim_block.hash_into(eat);
        }
        match self.target {
            None => eat(&[0]),
            Some(Aim::Unit(h)) => {
                eat(&[1]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            Some(Aim::Vehicle(h)) => {
                eat(&[2]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
        }
        eat(&self.next_think.to_le_bytes());
        eat(&self.thinks.to_le_bytes());
        eat(&self.look.to_bits().to_le_bytes());
        eat(&self.shots.to_le_bytes());
        eat(&self.pause_until.to_le_bytes());
        eat(&[self.visible.map_or(u8::MAX, |v| v)]);
        for t in self.threats.iter().flatten() {
            let (kind, index, gen) = match t.aim {
                Aim::Unit(h) => (1u8, h.index, h.gen),
                Aim::Vehicle(h) => (2u8, h.index, h.gen),
            };
            eat(&[kind, u8::from(t.seen)]);
            eat(&index.to_le_bytes());
            eat(&gen.to_le_bytes());
            eat(&t.tested_at.to_le_bytes());
            eat(&t.seen_at.to_le_bytes());
        }
    }
}

pub use crate::vehicle::flyer::THREAT_SLOTS;
pub(super) const LEVEL_THINK: [f32; 5] = [4.0, 3.0, 2.0, 1.0, 0.25];
pub(super) const COMBAT_THINK_FACTOR: f32 = 0.4;
pub(super) const RAY_INTERVAL_BY_LEVEL: [f32; 5] = [4.0, 4.0, 2.0, 1.0, 1.0];
pub(super) const RAY_REQUESTS_BY_LEVEL: [usize; 5] = [1, 1, 2, 4, 5];
const LEVEL_NEAR: f32 = 7.0;
const LEVEL_FAR: f32 = 20.0;
const OMNI_SIGHT: f32 = 10.0;
pub(super) const THREAT_SWITCH_MARGIN: i32 = 10;
const AIM_LINE_NEAR: f32 = 50.0;
const AIM_LINE_MISS: f32 = 20.0;
const AIM_ERROR_AGAINST_BOT: f32 = 0.3;
const ARMED_VIEW_FACTOR: f32 = 10.0;
const HUMAN_VIEW_FACTOR: f32 = 2.0;

pub(super) fn threat_score(
    status: u8,
    key: i32,
    human: bool,
    facing: f32,
    skill: u8,
    age: f32,
) -> i32 {
    let mut visual = 100 - (key as f32 * 0.0016666667) as i32;
    if human {
        let angle = f32::from(skill.max(1) - 1) * 0.05263158 * 0.7;
        let floor = (angle * 2.0 - 1.0).max(0.0);
        visual = (visual as f32 * ((facing - 0.6) * 2.5).clamp(floor, 1.0)) as i32;
    } else {
        visual /= 2;
    }
    let fade = (age * 10.0) as i32 * if status == 2 { 2 } else { 1 };
    (18 * i32::from(status) + 100 + visual - fade).clamp(0, 255)
}

impl Sim {
    fn self_view(&self, a: Aim, team: u8, max_range: f32) -> f32 {
        let Aim::Vehicle(vh) = a else {
            return VIEW;
        };
        let Some(v) = self.vehicle(vh) else {
            return VIEW;
        };
        let radius = self
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |s| s.radius);
        let human = v
            .seats
            .iter()
            .filter_map(|s| s.occupant)
            .filter_map(|o| self.unit(o))
            .any(|o| o.is_human());
        let seen = (20.0 * radius + 30.0)
            * self.ai_tier(team).view_mult
            * ARMED_VIEW_FACTOR
            * if human { HUMAN_VIEW_FACTOR } else { 1.0 };
        VIEW.max(seen.min(max_range))
    }

    fn self_lead(&self, a: Aim, at: Vec3, eye: Vec3, speed: f32) -> Vec3 {
        let Aim::Vehicle(vh) = a else {
            return at;
        };
        let Some(v) = self.vehicle(vh) else {
            return at;
        };
        crate::collision::lead_point(at - eye, v.vel, speed).map_or(at, |rel| eye + rel)
    }

    pub(super) fn self_level(&self, pos: Vec3, eye: Vec3, view: Vec3) -> usize {
        let mut level = 0;
        for (_, u) in self.alive_units().filter(|(_, u)| u.is_human()) {
            let at = u
                .mount
                .and_then(|m| self.vehicle(m.vehicle))
                .map_or(u.soldier.pos, |v| v.pos);
            let d = at.distance(pos);
            let near = if d < LEVEL_NEAR {
                4
            } else if d > LEVEL_FAR {
                2
            } else {
                3
            };
            let off = at - eye;
            let seen = crate::perception::in_cone(view, off, off.length_squared(), OMNI_SIGHT);
            level = level.max(if seen { near } else { near - 1 });
        }
        level
    }

    fn flyer_aim_error(&self, p: &SelfPilot, d: f32, class: usize) -> Vec3 {
        let Some(Aim::Vehicle(h)) = p.target else {
            return Vec3::ZERO;
        };
        let Some(v) = self
            .vehicle(h)
            .filter(|v| v.kind == crate::vehicle::VehicleKind::Flyer)
        else {
            return Vec3::ZERO;
        };
        let Some(params) = self
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .filter(|f| f.bf2)
        else {
            return Vec3::ZERO;
        };
        let Some(pilot) = v
            .seats
            .iter()
            .find_map(|s| s.occupant)
            .and_then(|o| self.unit(o))
        else {
            return Vec3::ZERO;
        };
        let human = v
            .seats
            .iter()
            .filter_map(|s| s.occupant)
            .filter_map(|o| self.unit(o))
            .any(|o| o.is_human());
        let cap = self.enemy_skill_of(pilot.team).err_cap_flyer;
        let moving = v.vel.length() / params.max_speed.max(0.1);
        let r =
            crate::ai::gunner_foot::radius(params.aim_box, 1.0, pilot.aim.err, moving, 1.0, cap);
        let wc = &self.weapon_classes[class];
        let mut off = p.aim_block.dir * r * wc.ai_bubble_size;
        if v.flyer.phase != crate::vehicle::flyer::FlightPhase::Flying {
            off.y = off.y.max(0.0);
        }
        if !human {
            off *= AIM_ERROR_AGAINST_BOT;
        }
        off * (d / wc.ai_bubble_dist + 0.5).min(wc.ai_bubble_clamp)
    }

    fn self_foe(&self, a: Aim, team: u8) -> Option<(Vec3, Vec3, bool, i32)> {
        match a {
            Aim::Unit(u) => {
                if crate::ai::dfocus::just_spawned(self, u) {
                    return None;
                }
                let u = self.unit(u).filter(|u| {
                    u.alive && self.hostile(team, u.team) && u.mount.is_none() && u.cloak.visible()
                })?;
                let (s, c) = fmath::sin_cos(u.soldier.yaw);
                Some((u.capsule().center(), Vec3::new(s, 0.0, c), u.is_human(), 5))
            }
            Aim::Vehicle(vh) => {
                let v = self
                    .vehicle(vh)
                    .filter(|v| v.alive && v.health.alive() && v.team.is_some_and(|t| t != team))?;
                let human = v
                    .seats
                    .iter()
                    .filter_map(|s| s.occupant)
                    .filter_map(|o| self.unit(o))
                    .any(|o| o.is_human());
                let relative = if v.kind == crate::vehicle::VehicleKind::Walker {
                    20
                } else {
                    10
                };
                Some((
                    self.vehicle_blast_sphere(v).0,
                    v.rot * Vec3::Z,
                    human,
                    relative,
                ))
            }
        }
    }

    fn visual_key(at: Vec3, eye: Vec3, human: bool, relative: i32) -> i32 {
        let key = (at.distance(eye) as i32).min(1500) * (21 - relative);
        if human {
            key
        } else {
            key * 2
        }
    }

    fn threat_priority(&self, t: &Threat, team: u8, eye: Vec3, skill: u8) -> i32 {
        let Some((at, nose, human, relative)) = self.self_foe(t.aim, team) else {
            return 0;
        };
        let key = Self::visual_key(at, eye, human, relative);
        let facing = nose.dot((eye - at).normalize_or_zero());
        let status = if t.seen { 3 } else { 1 };
        let age = self.tick.saturating_sub(t.seen_at) as f32 / crate::TICK_HZ as f32;
        threat_score(status, key, human, facing, skill, age)
    }

    fn update_threat(&self, p: &mut SelfPilot, aim: Aim, seen: bool, team: u8, eye: Vec3) {
        let skill = self.bot_skill(team);
        let slot = p
            .threats
            .iter()
            .position(|t| t.is_some_and(|t| t.aim == aim));
        let slot = match slot {
            Some(i) => i,
            None => {
                if !seen {
                    return;
                }
                let new = Threat {
                    aim,
                    seen,
                    tested_at: self.tick,
                    seen_at: self.tick,
                };
                let mut worst = self.threat_priority(&new, team, eye, skill);
                let mut pick = None;
                for (i, t) in p.threats.iter().enumerate() {
                    match t {
                        None if worst > 0 => {
                            worst = 0;
                            pick = Some(i);
                        }
                        None => {}
                        Some(t) => {
                            let priority = self.threat_priority(t, team, eye, skill);
                            if priority < worst {
                                worst = priority;
                                pick = Some(i);
                            }
                        }
                    }
                }
                let Some(i) = pick else { return };
                p.threats[i] = Some(Threat {
                    aim,
                    seen,
                    tested_at: self.tick,
                    seen_at: 0,
                });
                i
            }
        };
        if let Some(t) = p.threats[slot].as_mut() {
            t.seen = seen;
            t.tested_at = self.tick;
            if seen {
                t.seen_at = self.tick;
            }
        }
    }

    fn threat_think(
        &mut self,
        h: VehicleHandle,
        p: &mut SelfPilot,
        team: u8,
        (pos, eye, view): (Vec3, Vec3, Vec3),
        target_kinds: u32,
        max: f32,
    ) {
        let skill = self.bot_skill(team);
        let level = self.self_level(pos, eye, view);
        for t in &mut p.threats {
            if t.is_some_and(|k| self.threat_priority(&k, team, eye, skill) == 0) {
                *t = None;
            }
        }
        let priority = |sim: &Self, p: &SelfPilot, i: usize| {
            p.threats[i].map_or(0, |t| sim.threat_priority(&t, team, eye, skill))
        };
        let mut best: Option<(u8, i32)> = None;
        for i in 0..THREAT_SLOTS {
            if p.threats[i].is_some_and(|t| t.seen) {
                let score = priority(self, p, i);
                if best.is_none_or(|(_, b)| score > b) {
                    best = Some((i as u8, score));
                }
            }
        }
        let best_slot = best.map(|(i, _)| i);
        if best_slot != p.visible {
            match (best, p.visible) {
                (Some((i, score)), Some(v)) => {
                    if score > priority(self, p, usize::from(v)) + THREAT_SWITCH_MARGIN {
                        p.visible = Some(i);
                    }
                }
                _ => p.visible = best_slot,
            }
        }
        let visible = p
            .visible
            .and_then(|v| p.threats[usize::from(v)])
            .map(|t| t.aim);
        let factor = if visible.is_some() {
            COMBAT_THINK_FACTOR
        } else {
            1.0
        };
        p.next_think = self.tick + (LEVEL_THINK[level] * factor * crate::TICK_HZ as f32) as u64;
        p.target = visible;
        p.fighting = p
            .visible
            .and_then(|v| p.threats[usize::from(v)])
            .is_some_and(|t| t.seen);

        let units = self
            .alive_units()
            .filter(|(_, u)| target_kinds & (1 << u.kind()) != 0)
            .map(|(h, _)| Aim::Unit(h));
        let vehicles = self
            .alive_vehicles()
            .filter(|(_, v)| target_kinds & (1 << v.health.health_type as u32) != 0)
            .map(|(h, _)| Aim::Vehicle(h));
        let mut list: Vec<(i32, Aim, bool)> = units
            .chain(vehicles)
            .filter_map(|a| {
                let (at, _, human, relative) = self.self_foe(a, team)?;
                let off = at - eye;
                let d2 = off.length_squared();
                let far = self.self_view(a, team, max);
                (d2 < far * far && crate::perception::in_cone(view, off, d2, OMNI_SIGHT))
                    .then(|| (Self::visual_key(at, eye, human, relative), a, human))
            })
            .collect();
        list.sort_by_key(|c| c.0);
        for i in 0..THREAT_SLOTS {
            let Some(t) = p.threats[i] else { continue };
            if t.seen && !list.iter().any(|c| c.1 == t.aim) {
                self.update_threat(p, t.aim, false, team, eye);
            }
        }
        let visible_human = visible
            .and_then(|a| self.self_foe(a, team))
            .map(|(_, _, human, _)| human);
        let interval = (RAY_INTERVAL_BY_LEVEL[level] * crate::TICK_HZ as f32) as u64;
        let mut requests = 0;
        for &(_, aim, human) in &list {
            if requests >= RAY_REQUESTS_BY_LEVEL[level] {
                break;
            }
            let due = p
                .threats
                .iter()
                .flatten()
                .find(|t| t.aim == aim)
                .is_none_or(|t| self.tick - t.tested_at >= interval);
            let test = match (human, visible_human) {
                (false, None) => true,
                (false, Some(true)) => false,
                _ => due,
            };
            if !test {
                continue;
            }
            requests += 1;
            let seen = self.self_target(h, aim, team, eye, f32::INFINITY).is_some();
            self.update_threat(p, aim, seen, team, eye);
            if human {
                p.next_think = self.tick + 1;
            }
            if let (true, Aim::Vehicle(target)) = (seen, aim) {
                self.force_pilot_threat(target, crate::vehicle::flyer::ThreatObject::Vehicle(h));
            }
        }
        if visible.is_none() && p.thinks % LOOK_EVERY == 1 {
            p.look = self.lcg.uniform(-LOOK_SPREAD, LOOK_SPREAD);
        }
    }

    /// Enable only the authored self seats of this placed building.
    pub fn set_building_self_pilots(&mut self, instance: u32, seats: &[u8]) {
        if let Some(t) = self
            .building_turrets
            .iter_mut()
            .find(|t| t.instance == instance)
        {
            t.self_pilots = seats.iter().copied().map(SelfPilot::new).collect();
        }
    }

    pub(super) fn tick_self_turrets(&mut self) {
        for i in 0..self.building_turrets.len() {
            let t = &self.building_turrets[i];
            if t.down.is_some() || t.self_pilots.is_empty() {
                continue;
            }
            let h = t.handle;
            let mut pilots = std::mem::take(&mut self.building_turrets[i].self_pilots);
            for p in &mut pilots {
                self.self_turret(h, p);
            }
            self.building_turrets[i].self_pilots = pilots;
        }
    }

    /// The aim point on `a`, if it is a live enemy in `view` metres with a clear
    /// line. The line is the costly part, so distance comes first.
    fn self_target(
        &self,
        h: VehicleHandle,
        a: Aim,
        team: u8,
        from: Vec3,
        view: f32,
    ) -> Option<Vec3> {
        let own = self.vehicle(h)?;
        let (at, target_hull, target_building) = match a {
            Aim::Unit(u) => {
                if crate::ai::dfocus::just_spawned(self, u) {
                    return None;
                }
                let u = self.unit(u).filter(|u| {
                    u.alive && self.hostile(team, u.team) && u.mount.is_none() && u.cloak.visible()
                })?;
                (u.capsule().center(), None, None)
            }
            Aim::Vehicle(vh) => {
                let v = self
                    .vehicle(vh)
                    .filter(|v| v.alive && v.health.alive() && v.team.is_some_and(|t| t != team))?;
                (self.vehicle_blast_sphere(v).0, Some(vh), v.instance)
            }
        };
        if at.distance(from) > view {
            return None;
        }
        crate::perception::los_seat(
            self,
            from,
            at,
            HullFilter::Except([Some(h), target_hull]),
            [own.instance, target_building],
        )
        .then_some(at)
    }

    fn self_turret(&mut self, h: VehicleHandle, p: &mut SelfPilot) {
        let Some(v) = self.vehicle(h).filter(|v| v.alive) else {
            return;
        };
        let Some(team) = v.team else { return };
        let seat = usize::from(p.seat);
        if v.seats.get(seat).is_none_or(|s| s.occupant.is_some()) {
            return;
        }
        let Some(g) = self
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .and_then(|s| s.guns.get(seat))
            .cloned()
            .flatten()
        else {
            return;
        };
        let Some(w) = g.weapons.first() else { return };
        let class = w.class;
        let (pos, rot) = (v.pos, v.rot);
        self.arm(h.index as usize);
        let Some(st) = self
            .vehicle(h)
            .and_then(|v| v.guns.get(seat))
            .and_then(Option::as_ref)
        else {
            return;
        };
        let (look, aim) = (st.look, st.aim);
        let heat = st.weapons.first().map_or(0.0, |w| w.heat);
        let spread = st.weapons.first().map_or(0.0, |w| {
            let wc = &self.weapon_classes[class];
            w.yaw_spread(wc).max(w.pitch_spread(wc))
        });
        let unlimited = w.infinite || self.weapon_classes[class].infinite_ammo();
        let eye = pos + rot * g.muzzle(aim);
        let wc = &self.weapon_classes[class];
        let [min, _, max] = wc.ai_range;
        let target_kinds = wc.ai_target_kind;
        let space = self.space_assault.is_some();
        if space {
            if self.tick >= p.next_think {
                p.thinks += 1;
                let view = rot * gun::dir(look.0, look.1);
                self.threat_think(h, p, team, (pos, eye, view), target_kinds, max);
            }
            p.fighting = p
                .visible
                .and_then(|v| p.threats[usize::from(v)])
                .is_some_and(|t| t.seen);
            if (self.tick + u64::from(h.index)).is_multiple_of(crate::unit::AimBlock::PERIOD) {
                let tick = self.tick;
                let draw = |k: u32| {
                    let salt = crate::ai::SALT_ERR_AXIS.wrapping_add(k);
                    2.0 * crate::ai::unit_unit(h.index, tick, h.gen, salt) - 1.0
                };
                let jolt = Vec3::new(draw(0), draw(0x51ED_2701), draw(0xA3DA_4E03));
                p.aim_block.think(
                    crate::unit::AimBlock::PERIOD as f32 * crate::TICK_DT,
                    Vec3::ZERO,
                    1.0,
                    0.0,
                    0.0,
                    jolt,
                );
            }
        } else if self.tick >= p.next_think {
            p.next_think = self.tick + (THINK * crate::TICK_HZ as f32) as u64;
            p.thinks += 1;
            let held = p
                .target
                .and_then(|a| self.self_target(h, a, team, eye, f32::INFINITY));
            if !(p.fighting && held.is_some_and(|at| at.distance(eye) <= max + FIGHT_MARGIN)) {
                let units = self
                    .alive_units()
                    .filter(|(_, u)| wc.ai_target_kind & (1 << u.kind()) != 0)
                    .map(|(h, _)| Aim::Unit(h));
                let vehicles = self
                    .alive_vehicles()
                    .filter(|(_, v)| wc.ai_target_kind & (1 << v.health.health_type as u32) != 0)
                    .map(|(h, _)| Aim::Vehicle(h));
                let found = units
                    .chain(vehicles)
                    .filter_map(|a| {
                        self.self_target(h, a, team, eye, self.self_view(a, team, max))
                            .map(|at| (a, at.distance(eye)))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                p.target = found.map(|(a, _)| a);
                p.fighting = found.is_some_and(|(_, d)| (min..=max).contains(&d));
            }
            if p.target.is_none() && p.thinks % LOOK_EVERY == 1 {
                p.look = self.lcg.uniform(-LOOK_SPREAD, LOOK_SPREAD);
            }
        }
        let shot_speed = self.weapon_classes[class].shot_ordnance().velocity;
        let at = p.target.and_then(|a| {
            if space {
                self.self_foe(a, team).map(|f| f.0)
            } else {
                self.self_target(h, a, team, eye, f32::INFINITY)
            }
            .map(|at| self.self_lead(a, at, eye, shot_speed))
        });
        let wanted = at.map(|at| {
            if space {
                at + self.flyer_aim_error(p, at.distance(eye), class)
            } else {
                at
            }
        });
        let want = wanted.map_or((p.look, 0.0), |at| gun::angles(rot.inverse() * (at - eye)));
        let mut pad = PadState::default();
        if let Some(t) = g.turn {
            let (s, c) = fmath::sin_cos(gun::wrap(want.0 - look.0));
            let yaw = if c < 0.0 { s.signum() } else { s };
            pad.rx = PadState::quantize(-crate::ai::hover_drive::shaped(
                yaw,
                4.5 - t.yaw_speed.clamp(1.0, 3.0),
            ));
            pad.ry = PadState::quantize(crate::ai::hover_drive::shaped(
                fmath::sin_cos(want.1 - look.1).0,
                5.0 - t.pitch_speed.clamp(1.0, 3.0),
            ));
        }
        let wc = &self.weapon_classes[class];
        let fwd = rot * gun::dir(aim.0, aim.1);
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let skill = self.difficulty_of(team).skill_at(self.bot_skill(team));
        let burst = crate::ai::burst_len(
            self.ground_rules,
            wc,
            crate::unit::UnitHandle {
                index: h.index,
                gen: h.gen,
            },
            self.tick,
            &skill,
        );
        let delay = crate::ai::pause_ticks(self.ground_rules, wc, &skill);
        if p.burst > 0 && crate::ai::heat_spent(wc, heat) {
            p.burst = 0;
            p.shots = 0;
            p.pause_until = self.tick + delay;
        }
        if p.burst == 0 && self.tick >= p.pause_until && crate::ai::heat_fits(wc, burst, heat) {
            p.burst = burst;
            p.shots = 0;
        }
        let fire = p.burst > 0
            && p.fighting
            && self.tick >= p.pause_until
            && at.is_some_and(|at| {
                let d = at.distance(eye);
                let cone = fmath::sin_cos(AIM_CONE).1;
                let line = if space {
                    rot * gun::dir(look.0, look.1)
                } else {
                    fwd
                };
                (min..=max).contains(&d)
                    && (at - eye).normalize_or_zero().dot(line) >= cone
                    && (!space
                        || (line.dot(fwd) >= cone
                            && (d < AIM_LINE_NEAR
                                || (at - eye).cross(line).length() < AIM_LINE_MISS)))
                    && !crate::ai::item_line_blocked(
                        self,
                        team,
                        eye,
                        (fwd, right, right.cross(fwd)),
                        at,
                        spread,
                    )
            });
        // The shared weapon update handles cooldown, salvos, heat and ammo.
        // Single-trigger guns receive edges. Unlimited seat weapons burst on
        // heat, never a shot count (PS2 FUN_002bac98, re-soldier-ai §23.2).
        if fire && (!wc.trigger_single || self.tick.is_multiple_of(2)) {
            pad.buttons |= buttons::FIRE;
        }
        let before = self.events.len();
        self.drive_seat_gun(
            Mount {
                vehicle: h,
                seat: p.seat,
            },
            g,
            &pad,
            None,
            at,
        );
        if !unlimited {
            p.shots += self.events[before..]
                .iter()
                .filter(|e| matches!(e, super::Event::Shot { .. }))
                .count() as u32;
        }
        if !unlimited && p.shots >= u32::from(p.burst.max(1)) {
            p.burst = 0;
            p.shots = 0;
            p.pause_until = self.tick + delay;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, Shape};
    use crate::sim::{Event, TurretFall};
    use crate::unit::{UnitHandle, UnitSpec};
    use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon, Turn};
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    use crate::weapon::{HealthType, WeaponClass};
    use glam::{Mat4, Quat};

    fn world() -> (Sim, UnitHandle) {
        let mut s = Sim::empty(42);
        s.heightfield = Some(crate::sim::tests::flat_ground());
        let mut wc = WeaponClass::bf1_rifle();
        wc.ai_range = [0.0, 30.0, 80.0];
        wc.ordnance.max_damage = 0.0;
        wc.trigger_single = false;
        s.weapon_classes[0] = wc;
        s.seat_setup = vec![Some(SeatSetup {
            radius: 1.0,
            boardable: vec![false],
            body_active: vec![],
            exit_drop: 15.0,
            ai_size: 1,
            guns: vec![Some(GunSetup {
                weapons: vec![GunWeapon {
                    class: 0,
                    infinite: true,
                    reserve_clips: 0,
                }],
                fire_point: Vec3::Y,
                pivot: Vec3::Y,
                eye: Vec3::ZERO,
                aimer: Aimer::default(),
                pilot: false,
                turn: Some(Turn::from_class(
                    None,
                    Some([-45.0, 45.0]),
                    (2.0, 0.5),
                    (10.0, 10.0),
                    gun::Slew::Linear,
                )),
                framed: true,
                aimer_bind: Quat::IDENTITY,
                gunner: None,
            })],
        })];
        s.add_building_turret(
            VehicleSpec {
                kind: VehicleKind::Turret,
                class: 0,
                home_team: Some(0),
                neutral_when_empty: false,
                spawn_pad: None,
                pos: Vec3::ZERO,
                yaw: 0.0,
                max_health: 50.0,
                health_type: HealthType::Building,
                seats: vec![SeatKind::Gunner(0)],
            },
            Quat::IDENTITY,
            None,
            123,
            (TurretFall::Hidden, vec![]),
        );
        s.set_building_self_pilots(123, &[0]);
        let u = s
            .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, 60.0)))
            .unwrap();
        (s, u)
    }

    fn shots(s: &mut Sim, ticks: usize) -> usize {
        (0..ticks)
            .map(|_| {
                s.step(&[PadState::default()]);
                s.events
                    .iter()
                    .filter(|e| matches!(e, Event::Shot { unit, .. } if unit.index == u32::MAX))
                    .count()
            })
            .sum()
    }

    fn flyer(s: &mut Sim, pos: Vec3) -> crate::vehicle::VehicleHandle {
        s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 100.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        })
    }

    #[test]
    fn a_self_seat_sees_a_ship_far_past_its_foot_view_and_a_humans_twice_as_far() {
        let (mut s, u) = world();
        let v = flyer(&mut s, Vec3::new(0.0, 0.0, 300.0));
        let seen = 50.0 * s.ai_tier(0).view_mult * ARMED_VIEW_FACTOR;
        assert!(seen > VIEW);
        assert_eq!(s.self_view(Aim::Vehicle(v), 0, 1.0e5), seen);
        assert_eq!(s.self_view(Aim::Vehicle(v), 0, seen / 2.0), seen / 2.0);
        assert_eq!(s.self_view(Aim::Unit(u), 0, 1.0e5), VIEW);
        s.vehicle_mut(v).unwrap().seats[0].occupant = Some(u);
        assert_eq!(
            s.self_view(Aim::Vehicle(v), 0, 1.0e5),
            seen * HUMAN_VIEW_FACTOR
        );
    }

    #[test]
    fn a_self_seat_leads_a_moving_ship_and_not_a_soldier() {
        let (mut s, u) = world();
        let at = Vec3::new(0.0, 0.0, 300.0);
        let v = flyer(&mut s, at);
        s.vehicle_mut(v).unwrap().vel = Vec3::new(50.0, 0.0, 0.0);
        let led = s.self_lead(Aim::Vehicle(v), at, Vec3::ZERO, 300.0);
        assert!(led.x > 40.0, "{led}");
        assert_eq!(s.self_lead(Aim::Unit(u), at, Vec3::ZERO, 300.0), at);
    }

    fn think_gaps(s: &mut Sim, ticks: usize) -> Vec<u64> {
        let mut last = s.building_turrets[0].self_pilots[0].next_think;
        let mut gaps = Vec::new();
        for _ in 0..ticks {
            s.step(&[PadState::default()]);
            let now = s.building_turrets[0].self_pilots[0].next_think;
            if now != last && last != 0 {
                gaps.push(now - last);
            }
            last = now;
        }
        gaps
    }

    #[test]
    fn a_space_round_thinks_at_the_turret_periods() {
        let (mut s, u) = world();
        s.space_assault = Some(crate::sim::space_assault::SpaceAssault::new(10));
        s.unit_mut(u).unwrap().is_player = false;
        let fighting = think_gaps(&mut s, 400);
        assert!(fighting.len() >= 4, "{fighting:?}");
        assert!(fighting.iter().all(|&g| g == 48), "{fighting:?}");
        s.unit_mut(u).unwrap().alive = false;
        let waiting = think_gaps(&mut s, 800);
        assert!(waiting.len() >= 4, "{waiting:?}");
        assert!(waiting[1..].iter().all(|&g| g == 120), "{waiting:?}");
    }

    #[test]
    fn a_human_in_view_makes_a_space_seat_think_at_his_level() {
        let (mut s, _) = world();
        s.space_assault = Some(crate::sim::space_assault::SpaceAssault::new(10));
        let gaps = think_gaps(&mut s, 400);
        assert!(gaps.contains(&24), "{gaps:?}");
        assert!(gaps.iter().all(|&g| g == 24 || g == 1), "{gaps:?}");
    }

    #[test]
    fn self_seat_fires_without_soldier_tracks_outside_range_and_releases_dead_target() {
        let (mut s, u) = world();
        let n = shots(&mut s, 180);
        assert!(
            n > 2,
            "shots={n}, pilot={:?}, unit={:?}, gun={:?}",
            s.building_turrets[0].self_pilots,
            s.unit(u).map(|u| (u.alive, u.soldier.pos, u.kind())),
            s.vehicle(s.building_turrets[0].handle)
                .map(|v| (&v.guns, v.team))
        );
        assert!(s.vehicle(s.building_turrets[0].handle).unwrap().seats[0]
            .occupant
            .is_none());
        s.unit_mut(u).unwrap().soldier.pos.z = 95.0;
        // A salvo committed at the preceding tick boundary may still leave;
        // releasing a cannon does not cancel its already-started cycle.
        assert!(shots(&mut s, 2) <= 1);
        assert_eq!(shots(&mut s, 88), 0);
        assert_eq!(
            s.building_turrets[0].self_pilots[0].target,
            Some(Aim::Unit(u))
        );
        s.unit_mut(u).unwrap().alive = false;
        assert_eq!(shots(&mut s, 30), 0);
        assert_eq!(s.building_turrets[0].self_pilots[0].target, None);
    }

    #[test]
    fn self_seat_respects_team_occlusion_death_and_hashes_controller() {
        let (mut s, u) = world();
        s.unit_mut(u).unwrap().team = 0;
        assert_eq!(shots(&mut s, 60), 0);
        s.unit_mut(u).unwrap().team = 1;
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 2.0, 30.0)),
            Shape::Box {
                half: Vec3::new(5.0, 3.0, 1.0),
            },
        ));
        s.collision.rebuild_broadphase();
        assert_eq!(shots(&mut s, 60), 0);
        let before = s.hash();
        s.building_turrets[0].self_pilots[0].pause_until += 1;
        assert_ne!(s.hash(), before);
        let vh = s.building_turrets[0].handle;
        s.vehicle_mut(vh).unwrap().alive = false;
        assert_eq!(shots(&mut s, 60), 0);
    }

    #[test]
    fn a_space_seat_holds_its_shots_until_the_turret_axis_is_on_the_line() {
        let (mut s, u) = world();
        s.space_assault = Some(crate::sim::space_assault::SpaceAssault::new(10));
        s.unit_mut(u).unwrap().is_player = false;
        s.unit_mut(u).unwrap().soldier.pos = Vec3::new(50.0, 0.0, 30.0);
        let mut fired = 0;
        for _ in 0..900 {
            s.step(&[PadState::default()]);
            let n = s
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { unit, .. } if unit.index == u32::MAX))
                .count();
            if n > 0 {
                let v = s.vehicle(s.building_turrets[0].handle).unwrap();
                let st = v.guns[0].as_ref().unwrap();
                let line = v.rot * gun::dir(st.look.0, st.look.1);
                let at = s.unit(u).unwrap().capsule().center();
                let off = (at - v.pos).cross(line).length();
                assert!(line.dot(at - v.pos) > 0.0, "the turret faced away");
                assert!(off < AIM_LINE_MISS + 2.0, "fired {off} m off the line");
                fired += n;
            }
        }
        assert!(fired > 0);
    }

    #[test]
    fn the_aim_error_follows_distance_the_pilot_and_who_flies() {
        let (mut s, u) = world();
        s.weapon_classes[0].ai_bubble_size = 0.6;
        s.weapon_classes[0].ai_bubble_dist = 100.0;
        s.weapon_classes[0].ai_bubble_clamp = 3.0;
        s.flyer_params = vec![Some(crate::vehicle::flyer::FlyerParams {
            bf2: true,
            aim_box: 12.0,
            ..crate::vehicle::flyer::FlyerParams::default()
        })];
        let v = flyer(&mut s, Vec3::new(0.0, 0.0, 300.0));
        s.vehicle_mut(v).unwrap().seats[0].occupant = Some(u);
        s.vehicle_mut(v).unwrap().flyer.phase = crate::vehicle::flyer::FlightPhase::Flying;
        s.unit_mut(u).unwrap().aim.err = 1.0;
        let mut p = SelfPilot::new(0);
        p.target = Some(Aim::Vehicle(v));
        p.aim_block.dir = Vec3::X;
        let human = s.flyer_aim_error(&p, 100.0, 0);
        assert!(human.x > 0.0, "{human}");
        let far = s.flyer_aim_error(&p, 1000.0, 0);
        assert!((far.x / human.x - 2.0).abs() < 1e-4, "{far} {human}");
        s.unit_mut(u).unwrap().is_player = false;
        let bot = s.flyer_aim_error(&p, 100.0, 0);
        assert!((bot.x / human.x - AIM_ERROR_AGAINST_BOT).abs() < 1e-4);
        p.aim_block.dir = -Vec3::Y;
        assert!(s.flyer_aim_error(&p, 100.0, 0).y < 0.0);
        s.vehicle_mut(v).unwrap().flyer.phase = crate::vehicle::flyer::FlightPhase::Landed;
        assert_eq!(s.flyer_aim_error(&p, 100.0, 0).y, 0.0);
    }
}
