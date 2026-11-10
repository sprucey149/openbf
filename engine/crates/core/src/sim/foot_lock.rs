//! The on-foot lock for a human's launcher (`analysis/re-ordnance-kinds.md`
//! §5), BF2's rule on every ground map: a hostile target in front is locked
//! (on its own, or by the lock button), the target is kept while it stays
//! roughly ahead, and the launcher's dwell clock runs only while that target
//! sits within 25.8° of the aim and in sight. Space rounds keep the magnet
//! dwell in `missiles.rs`.
//!
//! Read in the soldier's lock setup (B2:159266-159302), the tracker's pick
//! and hold (`FUN_0017a908`, B2:21224-21312, 21690-21740, 21828) and the
//! launcher's candidate (`FUN_0033a658`, B2:331689-331750). There a human's
//! lock never names a body part: the missile steers to the target's centre.
//! OURS: a vehicle's lock goes to its weak part in sight (`weak_lock.rs`).
//! The pick stays on the centre; for the dwell and the hold a weak part in
//! sight counts as the vehicle does.

use super::input::{buttons, AimTarget, PadState};
use super::lock_on::{
    lock_score, LockProbe, LockTarget, LOCK_CLEAR_HOLD_SECS, LOCK_CYCLE_MEMORY, LOCK_FIRE_COS,
};
use super::state::Sim;
use crate::collision::HullFilter;
use crate::missile::MissileTarget;
use crate::perception;
use crate::unit::UnitHandle;
use crate::weapon::WeaponClass;
use glam::Vec3;

/// The pick's cone: a new lock needs dot >= 0.8 to the aim (+0x960).
pub const FOOT_PICK_DOT: f32 = 0.8;
/// A locked target outside the hold this long in a row is dropped (+0x950).
pub const FOOT_LOSE_SECS: f32 = 2.0;
/// The hold's cone for the locked target, dot >= 0.1 (+0x954).
pub const FOOT_LOSE_DOT: f32 = 0.1;
/// OURS: the launcher locks on its own. BF2 clears AutoLock on every soldier
/// (+0x1570 high word bit 4), so there a lock needs the button; the B7
/// playtest asks for dwelling alone to show the box and curve the rocket.
/// While no target is held, or the held one has left the dwell cone, the
/// best target in the pick cone is taken; the button still cycles. `false`
/// is BF2's press rule. measure: maintainer playtest of the auto lock.
pub const FOOT_AUTO_LOCK: bool = true;

/// One player slot's on-foot lock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FootLock {
    pub target: Option<LockTarget>,
    /// Seconds in a row the target has been outside the hold.
    lose: f32,
    /// Targets the button already picked; the next press skips them.
    picked: Vec<LockTarget>,
    /// The held target came from the button: the auto lock leaves it be.
    chosen: bool,
    button: f32,
    down: bool,
}

/// A launcher the on-foot lock serves: a lock time, a missile, a kind to
/// lock.
fn foot_launcher(c: &WeaponClass) -> bool {
    c.lock_time > 0.0 && c.ordnance.missile.is_some() && c.target_kind != 0
}

impl Sim {
    /// Does the on-foot lock serve this round? Every ground map does.
    ///
    /// OURS: BF1 maps take BF2's lock too, so a launcher locks the same way
    /// in both games; BF1 counted its dwell on the aim magnet (175 m, 6° on,
    /// 10° off). Each launcher keeps its own LockTime and kinds.
    /// measure: maintainer playtest of the BF1 torpedo launchers.
    pub(super) fn foot_lock_rules(&self) -> bool {
        !self.space_round()
    }

    /// Slot `slot`'s soldier on foot with a lock launcher in hand.
    fn foot_launcher_unit(&self, slot: usize) -> Option<UnitHandle> {
        let h = self.players.get(slot).copied().flatten()?;
        let u = self.unit(h)?;
        let free = u.alive
            && u.mount.is_none()
            && self.controlled_guided(h).is_none()
            && self.controlled_droid(h).is_none();
        let c = self.weapon_classes.get(u.weapon_class)?;
        (free && foot_launcher(c)).then_some(h)
    }

    /// Sight from the eye to the target, passing its own hull or building.
    fn foot_clear(&self, p: &LockProbe, eye: Vec3) -> bool {
        perception::los_seat(
            self,
            eye,
            p.aim,
            HullFilter::Except([None, p.hull]),
            [None, p.building],
        )
    }

    /// Is `t` the launcher's dwell candidate for `u`: of a kind it takes,
    /// within 25.8° of the aim (dot >= 0.9) and in sight? A vehicle's weak
    /// part in sight inside the cone counts as the vehicle does, so aiming
    /// at the lock box keeps the clock running on a large hull.
    fn foot_in_dwell(&self, t: LockTarget, i: usize) -> bool {
        let u = &self.units[i];
        let c = &self.weapon_classes[u.weapon_class];
        let Some(p) = self.lock_probe(t) else {
            return false;
        };
        if c.target_kind & p.kind_bit == 0 {
            return false;
        }
        let (eye, dir) = u.aim_ray(self.ground_rules, self.views());
        let inside = |at: Vec3| (at - eye).normalize_or_zero().dot(dir) >= LOCK_FIRE_COS;
        (inside(p.pos) && self.foot_clear(&p, eye))
            || self.foot_weak_part(t, i).is_some_and(|(_, at)| inside(at))
    }

    /// The press's pick: hostile soldiers on foot, live vehicles and
    /// turrets of a kind the launcher locks, within TargetLockMaxDistance
    /// (and short of ...Lose), dot >= 0.8, in sight; the lowest
    /// [`lock_score`] wins. Neutral and friendly targets are refused.
    fn foot_pick(
        &self,
        state: &FootLock,
        h: UnitHandle,
        (eye, dir): (Vec3, Vec3),
    ) -> Option<LockTarget> {
        let u = self.unit(h)?;
        let team = u.team;
        let c = &self.weapon_classes[u.weapon_class];
        let (mask, max, lose) = (c.target_kind, c.target_lock_max, c.target_lock_lose);
        let units = self
            .alive_units()
            .filter(|(o, x)| *o != h && x.mount.is_none())
            .map(|(o, _)| LockTarget::Aim(AimTarget::Unit(o)));
        let vehicles = self
            .alive_vehicles()
            .filter(|(_, v)| v.health.alive())
            .map(|(v, _)| LockTarget::Aim(AimTarget::Vehicle(v)));
        let turrets = self
            .turrets
            .iter()
            .map(|t| LockTarget::Aim(AimTarget::Turret(t.id)));
        let mut best: Option<(f32, LockTarget)> = None;
        for t in units.chain(vehicles).chain(turrets) {
            if state.picked.contains(&t) {
                continue;
            }
            let Some(p) = self.lock_probe(t) else {
                continue;
            };
            if mask & p.kind_bit == 0 || !p.team.is_some_and(|o| self.hostile(team, o)) {
                continue;
            }
            let off = p.pos - eye;
            let d2 = off.length_squared();
            if d2 > max * max || d2 >= lose * lose {
                continue;
            }
            if off.normalize_or_zero().dot(dir) < FOOT_PICK_DOT {
                continue;
            }
            let score = lock_score(off, dir);
            if best.is_some_and(|(b, _)| b <= score) {
                continue;
            }
            if self.foot_clear(&p, eye) {
                best = Some((score, t));
            }
        }
        best.map(|(_, t)| t)
    }

    /// Phase 2a, before the dwell: each player slot's lock button (RELOAD:
    /// Square / R) and the hold of its target. BF2's PS2 reloads on the
    /// D-pad and locks on Square (`re-zoom.md` §1); openbf has one button
    /// for both, so with a lock launcher in hand a press reloads when a
    /// reload can start and the lock comes on its own, and otherwise locks
    /// (or reloads when nothing is in the cone). A press that locks, and the
    /// held button after the press, never reload.
    pub(super) fn tick_foot_lock(&mut self, pads: &mut [PadState], dt: f32) {
        if !self.foot_lock_rules() {
            if self.foot_lock.iter().any(|s| *s != FootLock::default()) {
                self.foot_lock.clear();
            }
            return;
        }
        if self.foot_lock.len() < self.players.len() {
            self.foot_lock
                .resize(self.players.len(), FootLock::default());
        }
        for slot in 0..self.players.len() {
            let Some(h) = self.foot_launcher_unit(slot) else {
                if self.foot_lock[slot] != FootLock::default() {
                    self.foot_lock[slot] = FootLock::default();
                }
                continue;
            };
            let i = h.index as usize;
            let pressed = pads.get(i).is_some_and(|p| p.pressed(buttons::RELOAD));
            let mut state = std::mem::take(&mut self.foot_lock[slot]);
            let ray = self.units[i].aim_ray(self.ground_rules, self.views());
            let edge = pressed && !state.down;
            state.down = pressed;
            state.button = if pressed { state.button + dt } else { 0.0 };
            // Only the press's own tick may reach the reload.
            let mut reload = false;
            if edge {
                let c = &self.weapon_classes[self.units[i].weapon_class];
                let free = self.ammo_cheats(i).0;
                let reload_first = FOOT_AUTO_LOCK && self.units[i].weapon.can_start_reload(c, free);
                let mut next = None;
                if !reload_first {
                    // A press moves on from the held target, auto or not.
                    if let Some(t) = state.target.filter(|t| !state.picked.contains(t)) {
                        if state.picked.len() == LOCK_CYCLE_MEMORY {
                            state.picked.remove(0);
                        }
                        state.picked.push(t);
                    }
                    next = self.foot_pick(&state, h, ray);
                    if next.is_none() && !state.picked.is_empty() {
                        state.picked.clear();
                        next = self.foot_pick(&state, h, ray);
                    }
                }
                match next {
                    Some(t) => {
                        if state.picked.len() == LOCK_CYCLE_MEMORY {
                            state.picked.remove(0);
                        }
                        state.picked.push(t);
                        state.target = Some(t);
                        state.chosen = true;
                        state.lose = 0.0;
                    }
                    None => reload = true,
                }
            } else if pressed && state.button >= LOCK_CLEAR_HOLD_SECS {
                state.target = None;
                state.chosen = false;
                state.picked.clear();
            } else if FOOT_AUTO_LOCK
                && !state.chosen
                && !state.target.is_some_and(|t| self.foot_in_dwell(t, i))
            {
                if let Some(t) = self.foot_pick(&FootLock::default(), h, ray) {
                    if state.target != Some(t) {
                        state.target = Some(t);
                        state.lose = 0.0;
                    }
                }
            }
            if !reload {
                if let Some(p) = pads.get_mut(i) {
                    p.buttons &= !buttons::RELOAD;
                }
            }
            if let Some(t) = state.target {
                let c = &self.weapon_classes[self.units[i].weapon_class];
                let (mask, lose) = (c.target_kind, c.target_lock_lose);
                let team = self.units[i].team;
                let rider = match t {
                    LockTarget::Aim(AimTarget::Unit(o)) => {
                        self.unit(o).is_some_and(|x| x.mount.is_some())
                    }
                    _ => false,
                };
                let probe = self.lock_probe(t).filter(|p| {
                    !rider
                        && mask & p.kind_bit != 0
                        && p.team.is_some_and(|o| self.hostile(team, o))
                });
                match probe {
                    None => state.target = None,
                    Some(p) => {
                        let off = p.pos - ray.0;
                        // A weak part in sight keeps a vehicle whose centre
                        // is hidden.
                        let inside = off.normalize_or_zero().dot(ray.1) >= FOOT_LOSE_DOT
                            && off.length_squared() <= lose * lose
                            && (self.foot_clear(&p, ray.0) || self.foot_weak_part(t, i).is_some());
                        state.lose = if inside { 0.0 } else { state.lose + dt };
                        if state.lose >= FOOT_LOSE_SECS {
                            state.target = None;
                        }
                    }
                }
                if state.target.is_none() {
                    state.lose = 0.0;
                    state.chosen = false;
                }
            }
            self.foot_lock[slot] = state;
        }
    }

    /// The launcher's dwell candidate on a ground map: the slot's locked
    /// target, of a kind the launcher takes, within 25.8° of the aim
    /// (dot >= 0.9) and in sight, checked every tick.
    pub(super) fn foot_dwell_candidate(&self, slot: usize, i: usize) -> Option<MissileTarget> {
        let t = self.foot_lock.get(slot)?.target?;
        if self.units[i].mount.is_some() || !self.foot_in_dwell(t, i) {
            return None;
        }
        t.missile()
    }

    /// What the lock box shows on foot: the slot's locked target, at a
    /// vehicle's weak part in sight (where a locked rocket will steer), else
    /// at its centre. `None` in space rounds.
    pub fn foot_lock_target(&self, slot: usize) -> Option<(LockTarget, Vec3)> {
        if !self.foot_lock_rules() {
            return None;
        }
        let h = self.foot_launcher_unit(slot)?;
        let t = self.foot_lock.get(slot)?.target?;
        let centre = self.lock_probe(t)?.pos;
        let weak = self.foot_weak_part(t, h.index as usize);
        Some((t, weak.map_or(centre, |(_, at)| at)))
    }

    /// Is the foot lock box on a vehicle's weak part?
    pub fn foot_lock_weak(&self, slot: usize) -> bool {
        self.foot_lock_rules()
            && self
                .foot_launcher_unit(slot)
                .zip(self.foot_lock.get(slot).and_then(|s| s.target))
                .is_some_and(|(h, t)| self.foot_weak_part(t, h.index as usize).is_some())
    }

    /// Is the foot lock box's point in sight from the eye? A weak part is
    /// shown only while it is.
    pub fn foot_lock_visible(&self, slot: usize) -> bool {
        let Some((t, _)) = self.foot_lock_target(slot) else {
            return false;
        };
        let Some(h) = self.players.get(slot).copied().flatten() else {
            return false;
        };
        let (Some(p), Some(u)) = (self.lock_probe(t), self.unit(h)) else {
            return false;
        };
        self.foot_lock_weak(slot)
            || self.foot_clear(&p, u.aim_ray(self.ground_rules, self.views()).0)
    }

    /// The hands' launcher's dwell fill, 0..1, while it has a candidate
    /// (the reticle ring, `FUN_003b3398`'s state-3 heat). `None` in space
    /// rounds and with no candidate.
    pub fn foot_lock_charge(&self, slot: usize) -> Option<f32> {
        if self.space_round() {
            return None;
        }
        let u = self.unit(self.foot_launcher_unit(slot)?)?;
        u.weapon.lock_target?;
        let limit = self.weapon_classes[u.weapon_class].lock_time;
        Some((u.weapon.lock_held / limit).clamp(0.0, 1.0))
    }

    /// Hash the slots holding a lock; silent while none does.
    pub(super) fn foot_lock_hash(&self, eat: &mut impl FnMut(&[u8])) {
        for (slot, state) in self.foot_lock.iter().enumerate() {
            let Some(t) = state.target else {
                continue;
            };
            eat(b"FOOTLOCK");
            eat(&(slot as u32).to_le_bytes());
            let (i, g) = match t {
                LockTarget::Aim(AimTarget::Unit(h)) => (h.index, h.gen),
                LockTarget::Aim(AimTarget::Vehicle(h)) => (h.index, h.gen),
                LockTarget::Aim(AimTarget::Turret(id)) => (id as u32, (id >> 32) as u32),
                LockTarget::Aim(AimTarget::Droid(_)) | LockTarget::Aim(AimTarget::Station(_)) => {
                    (u32::MAX, 0)
                }
                LockTarget::Station(s) => (u32::from(s), u32::MAX),
            };
            eat(&i.to_le_bytes());
            eat(&g.to_le_bytes());
            eat(&state.lose.to_bits().to_le_bytes());
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
