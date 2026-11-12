//! Card TBLD (OURS, the maintainer 2026-10-28: "units with a fusion cutter when near a
//! broken turret should have a higher chance to go fix it as its next
//! objective, so turrets remain in play and get fixed. Make sure that
//! multiple engineers aren't given the objective of fixing the same turret at
//! the same time"). A bot on foot that carries a fusion cutter takes the
//! nearest broken building turret within [`FIX_REACH`] - destroyed, or
//! standing below MaxHealth and not held by the enemy - as its objective
//! ahead of its post, its sniper's spot and its fight's ring (a seen enemy
//! still comes first: the claim waits while it fights). It walks to the
//! turret, stands, turns its cutter on it and holds the trigger until the
//! turret is whole ([`serve_plan`]).
//!
//! **One engineer a turret**: a claim ([`FixState`]) no other bot's pick takes;
//! two that pick on one tick pick blind of each other, and the later claim
//! (then the higher index) gives way at its next think. A claim ends when
//! the turret is whole, when the bot dies (a dead bot's claim is nobody's),
//! when it is re-tasked (a squad order, a vehicle, off the level) or after
//! [`FIX_GIVE_UP`] with no health gained (then that turret is not picked
//! again for [`RETRY`]).
//!
//! The PS2's builder (`analysis/re-build-zones.md` 3 [O][V]: BF2 engineers /
//! BF1 pilots, a search every 5 s within 75 m, one claim per team, the walk to
//! 1.6 m, the 20 s and 8 s give-ups) is the model; the reach, the weight
//! over the bot's other picks and the claim per turret (not per team) are
//! ours. Every constant is OURS and PROVISIONAL.
//!
//! **Aid** (issue #35, the maintainer 2026-10-29: "don't make it a huge
//! priority, so bot engineers aren't just running around everywhere healing
//! rather than battling and fighting and being strategic"): with no broken
//! turret in reach, a fixer also takes a damaged friendly or neutral vehicle
//! or droid, or a landed portable turret (the PS2's search takes any friendly thing lacking health,
//! `re-build-zones.md` §3), but only when it knows of no enemy at all, stands
//! in no post that is not safely its team's, and the target is within
//! [`AID_REACH`] (not the PS2's 75 m: a bot never walks off for it). A seen
//! enemy ends an aid claim, and one with no health gained for [`AID_GIVE_UP`]
//! is given up. A moving vehicle with no human aboard is not served (the
//! PS2's rest rule). One fixer a target, as for turrets.

use super::state::AiState;
use crate::fmath;
use crate::sim::{buttons, PadState, Sim};
use crate::unit::{Unit, UnitHandle};
use crate::vehicle::{VehicleHandle, VehicleKind};
use crate::TICK_HZ;
use glam::Vec3;

/// How far a cutter carrier looks for a broken turret, m (straight).
pub const FIX_REACH: f32 = 50.0;
/// A claim with no health gained for this long is given up, ticks.
pub const FIX_GIVE_UP: u64 = 20 * TICK_HZ as u64;
/// A turret given up on is not picked again by that bot for this long.
pub const RETRY: u64 = 15 * TICK_HZ as u64;
/// The bot stands this far outside the turret's sphere, m.
const STAND_OFF: f32 = 1.0;
const SERVICE_MARGIN: f32 = 0.25;
/// Issue #35: how far a fixer looks for a vehicle or droid to aid, m.
pub const AID_REACH: f32 = 20.0;
/// Issue #35: an aid claim with no health gained for this long is given up
/// (the PS2 builder's 8 s no-rise give-up), ticks.
pub const AID_GIVE_UP: u64 = 8 * TICK_HZ as u64;
/// Issue #35: a vehicle moving faster than this with no human aboard is not
/// served (the PS2's rest rule, `re-build-zones.md` §3), m/s.
const AID_REST: f32 = 0.5;

/// What a fixer works on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FixTarget {
    /// `Sim::building_turrets` index.
    Turret(u16),
    /// `Sim::stations` index (issue #35).
    Station(u16),
    /// A vehicle (issue #35).
    Vehicle(VehicleHandle),
    /// A live portable turret, by non-reused identity (issue #91).
    Portable(u64),
}

impl FixTarget {
    /// A turret hashes as it did before the aid targets existed.
    fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        match *self {
            FixTarget::Turret(t) => eat(&t.to_le_bytes()),
            FixTarget::Station(s) => {
                eat(b"S");
                eat(&s.to_le_bytes());
            }
            FixTarget::Portable(id) => {
                eat(b"P");
                eat(&id.to_le_bytes());
            }
            FixTarget::Vehicle(v) => {
                eat(b"V");
                eat(&v.index.to_le_bytes());
                eat(&v.gen.to_le_bytes());
            }
        }
    }

    fn aid(&self) -> bool {
        !matches!(self, FixTarget::Turret(_))
    }
}

/// One bot's fix objective ([`super::fix`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixClaim {
    pub target: FixTarget,
    /// The tick it was claimed.
    pub since: u64,
    /// The turret's health when last it rose, and that tick.
    pub best: f32,
    pub progress_at: u64,
}

/// The claim, the turret given up last and when, and no search before
/// `next`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FixState {
    pub cur: Option<FixClaim>,
    pub left: Option<(FixTarget, u64)>,
    pub next: u64,
}

impl FixState {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"FIX");
        if let Some(c) = self.cur {
            c.target.hash_into(eat);
            eat(&c.since.to_le_bytes());
            eat(&c.best.to_bits().to_le_bytes());
            eat(&c.progress_at.to_le_bytes());
        }
        if let Some((t, at)) = self.left {
            t.hash_into(eat);
            eat(&at.to_le_bytes());
        }
        eat(&self.next.to_le_bytes());
    }
}

/// The cutter `u` carries: its class index and channel, `None` without one.
pub fn cutter(sim: &Sim, u: &Unit) -> Option<(usize, u8)> {
    let tool = |c: usize| {
        sim.weapon_classes
            .get(c)
            .is_some_and(|w| w.repair.is_some())
    };
    if tool(u.weapon_class) {
        return Some((u.weapon_class, 0));
    }
    u.held
        .iter()
        .find(|w| tool(w.class))
        .map(|w| (w.class, w.channel))
}

/// The target's health now, when `u` may fix it.
fn need_of(sim: &Sim, u: &Unit, class: usize, t: FixTarget) -> Option<f32> {
    let repair = sim.weapon_classes[class].repair?;
    let ours = |team: Option<u8>| team.is_none_or(|x| x == u.team);
    match t {
        FixTarget::Turret(i) => need(sim, u, class, usize::from(i)),
        FixTarget::Portable(id) => {
            let t = sim
                .live_turret(id)
                .filter(|t| t.landed && t.team == u.team)?;
            let c = sim.turret_class(t)?;
            (repair.health[c.health_type as usize] > 0.0 && t.health < c.body.max_health)
                .then_some(t.health)
        }
        FixTarget::Station(i) => {
            let s = sim.stations.get(usize::from(i))?;
            (ours(s.team)
                && repair.health[s.repair_kind() as usize] > 0.0
                && s.health.cur < s.health.max)
                .then_some(s.health.cur)
        }
        FixTarget::Vehicle(vh) => {
            let v = sim.vehicle(vh).filter(|v| v.alive)?;
            let human = v.seats.iter().any(|s| {
                s.occupant
                    .and_then(|h| sim.unit(h))
                    .is_some_and(|o| o.controller != crate::unit::Controller::Ai)
            });
            (v.kind != VehicleKind::Turret
                && ours(v.team)
                && repair.health[v.health.health_type as usize] > 0.0
                && v.health.cur < v.health.max
                && (human || v.vel.length() < AID_REST))
                .then_some(v.health.cur)
        }
    }
}

/// The target's sphere: centre and radius.
fn sphere_of(sim: &Sim, t: FixTarget) -> Option<(Vec3, f32)> {
    match t {
        FixTarget::Turret(i) => sim.building_turrets.get(usize::from(i)).map(|t| t.sphere()),
        FixTarget::Station(i) => sim
            .stations
            .get(usize::from(i))
            .map(|s| (s.sphere_centre, s.sphere_radius)),
        FixTarget::Vehicle(vh) => sim.vehicle_sphere(vh),
        FixTarget::Portable(id) => sim
            .live_turret(id)
            .map(|t| (t.centre(), crate::autoturret::RADIUS)),
    }
}

/// May `u` look for a vehicle or droid to aid: no enemy known, and not in
/// a post its team does not hold safely (it is fighting for that post).
fn aid_ok(sim: &Sim, u: &Unit, st: &AiState) -> bool {
    st.contacts.iter().all(Option::is_none)
        && !sim.cps.iter().any(|cp| {
            cp.def.capture.contains(u.soldier.pos)
                && (cp.owner != Some(u.team) || cp.capture_team.is_some_and(|t| t != u.team))
        })
}

/// Turret `i`'s health now (down or standing) and whether `u` may fix it
/// (broken, the cutter gives its kind something, not held by the enemy).
fn need(sim: &Sim, u: &Unit, class: usize, i: usize) -> Option<f32> {
    let t = sim.building_turrets.get(i)?;
    let repair = sim.weapon_classes[class].repair?;
    if repair.health[t.repair_kind() as usize] <= 0.0 {
        return None;
    }
    if let Some(h) = t.down {
        return Some(h);
    }
    let v = sim.vehicle(t.handle).filter(|v| v.alive)?;
    if v.team.is_some_and(|team| team != u.team) || !t.broken(Some(v.health.cur)) {
        return None;
    }
    Some(v.health.cur)
}

/// Once a think, before the route is planned: keep, end or make the bot's
/// claim (kept while it fights, its give-up clock running).
pub(super) fn upkeep(sim: &Sim, me: UnitHandle, u: &Unit, st: &mut AiState) {
    let Some((class, _)) = cutter(sim, u) else {
        st.fix.cur = None;
        return;
    };
    let busy = u.mount.is_some()
        || st.lost.is_some()
        || st.cmd.is_some()
        || u.command.is_some()
        || st.ride.is_some_and(|r| r.board.is_some());
    if busy {
        st.fix.cur = None;
        return;
    }
    if let Some(mut c) = st.fix.cur {
        // Another bot's older claim (then the lower index) keeps the target.
        let taken = sim.alive_units().any(|(h, o)| {
            h != me
                && o.ai.fix.cur.is_some_and(|x| {
                    x.target == c.target
                        && (x.since < c.since || x.since == c.since && h.index < me.index)
                })
        });
        let give_up = if c.target.aid() {
            AID_GIVE_UP
        } else {
            FIX_GIVE_UP
        };
        match need_of(sim, u, class, c.target) {
            _ if taken => st.fix.cur = None,
            None => {
                st.fix.cur = None;
                st.fix.next = sim.tick;
            }
            // Issue #35: an enemy seen ends an aid claim.
            Some(_) if sim.tick >= c.progress_at + give_up || (c.target.aid() && fighting(st)) => {
                st.fix.cur = None;
                st.fix.left = Some((c.target, sim.tick));
            }
            Some(h) => {
                if h > c.best {
                    c.best = h;
                    c.progress_at = sim.tick;
                }
                st.fix.cur = Some(c);
            }
        }
    }
    if st.fix.cur.is_some() || sim.tick < st.fix.next {
        return;
    }
    let pos = u.soldier.pos;
    let left = st
        .fix
        .left
        .filter(|(_, at)| sim.tick < at + RETRY)
        .map(|(t, _)| t);
    let claimed = |t: FixTarget| {
        sim.alive_units()
            .any(|(o, x)| o != me && x.ai.fix.cur.is_some_and(|c| c.target == t))
    };
    type Best = Option<(f32, FixTarget, f32)>;
    let consider = |best: &mut Best, t: FixTarget, reach: f32| {
        if left == Some(t) {
            return;
        }
        let Some(h) = need_of(sim, u, class, t) else {
            return;
        };
        let Some((centre, _)) = sphere_of(sim, t) else {
            return;
        };
        let d2 = (centre - pos).length_squared();
        if d2 > reach * reach || best.is_some_and(|(b, _, _)| d2 >= b) || claimed(t) {
            return;
        }
        *best = Some((d2, t, h));
    };
    let mut best: Best = None;
    for i in 0..sim.building_turrets.len() {
        consider(&mut best, FixTarget::Turret(i as u16), FIX_REACH);
    }
    // Nearby aid only with no placed turret to fix, and idle (issues #35/#91).
    if best.is_none() && aid_ok(sim, u, st) {
        for t in &sim.turrets {
            consider(&mut best, FixTarget::Portable(t.id), AID_REACH);
        }
        for i in 0..sim.stations.len() {
            consider(&mut best, FixTarget::Station(i as u16), AID_REACH);
        }
        for (vh, _) in sim.alive_vehicles() {
            consider(&mut best, FixTarget::Vehicle(vh), AID_REACH);
        }
    }
    st.fix.cur = best.map(|(_, t, h)| FixClaim {
        target: t,
        since: sim.tick,
        best: h,
        progress_at: sim.tick,
    });
    if st.fix.cur.is_none() {
        st.fix.next = sim.tick + 5 * TICK_HZ as u64;
    }
}

/// Where the claimed target's fixer stands: on the ground outside its
/// sphere, on the bot's side.
fn stand(sim: &Sim, u: &Unit, target: FixTarget) -> Option<(Vec3, Vec3, f32)> {
    let (centre, radius) = sphere_of(sim, target)?;
    let mut away = u.soldier.pos - centre;
    away.y = 0.0;
    let away = away.try_normalize().unwrap_or(Vec3::Z);
    let mut p = centre + away * (radius + STAND_OFF);
    p.y = sim.spawn_ground(p);
    Some((p, centre, radius))
}

/// Is the bot fighting (an enemy seen now)? The claim then waits.
fn fighting(st: &AiState) -> bool {
    st.contacts.iter().flatten().any(|c| c.seen_now)
}

/// The point a fixer walks to this tick: its stand by the claimed turret,
/// `None` with no claim or while it fights.
pub(super) fn walk(sim: &Sim, me: UnitHandle, st: &AiState) -> Option<Vec3> {
    let c = st.fix.cur.filter(|_| !fighting(st))?;
    let u = sim.unit(me)?;
    stand(sim, u, c.target).map(|(p, _, _)| p)
}

/// A fixer at its stand can still be short of the cutter's 3D range.
/// OURS: correct the last few steps on its floor, after the normal route.
/// measure: the elevated BF1 Yavin turret and the cutter arrival boundary (#108).
pub(super) fn closer_stand(sim: &Sim, u: &Unit, st: &AiState, nominal: Vec3) -> Option<Vec3> {
    let claim = st.fix.cur?;
    if claim.target.aid()
        || !u.soldier.grounded
        || crate::nav::xz_dist(u.soldier.pos, nominal) > super::HOLD_ARRIVE + super::ARRIVED_SLACK
        || (u.soldier.pos.y - nominal.y).abs() > crate::collision::STEP_HEIGHT
    {
        return None;
    }
    let (centre, radius) = sphere_of(sim, claim.target)?;
    let (class, _) = cutter(sim, u)?;
    let reach = sim.weapon_classes[class].lock_on_range + radius - SERVICE_MARGIN;
    let eye = u.muzzle();
    if eye.distance_squared(centre) <= reach * reach {
        return None;
    }
    let offset = u.soldier.pos - centre;
    let away = Vec3::new(offset.x, 0.0, offset.z).try_normalize()?;
    let rise = centre.y - eye.y;
    let reach_xz_sq = reach * reach - rise * rise;
    if reach <= 0.0 || reach_xz_sq <= crate::fmath::square(super::HOLD_ARRIVE) {
        return None;
    }
    let mut p = centre + away * (reach_xz_sq.sqrt() - super::HOLD_ARRIVE);
    p.y = sim.ground_under(Vec3::new(p.x, u.soldier.pos.y, p.z), false);
    let from = p + (eye - u.soldier.pos);
    let dir = (centre - from).normalize_or_zero();
    (p.y > crate::collision::NO_GROUND
        && (p.y - u.soldier.pos.y).abs() <= crate::collision::STEP_HEIGHT
        && from.distance_squared(centre) <= reach * reach
        && crate::perception::los_terrain(sim, from, centre - dir * radius))
    .then_some(p)
}

/// The walk to the turret failed: give it up for [`RETRY`].
pub(super) fn walk_failed(sim: &Sim, st: &mut AiState) {
    if let Some(c) = st.fix.cur.take() {
        st.fix.left = Some((c.target, sim.tick));
    }
}

#[derive(Clone, Copy)]
pub(super) struct ServePlan {
    pub rx: i16,
    pub ry: i16,
    pub button: u32,
}

/// How to turn the cutter onto the target and whether to switch or fire.
/// The fixer must have a claim in reach and see no enemy.
pub(super) fn serve_plan(sim: &Sim, u: &Unit, st: &AiState) -> Option<ServePlan> {
    let c = st.fix.cur.filter(|_| !fighting(st))?;
    let (class, channel) = cutter(sim, u)?;
    let (_, centre, radius) = stand(sim, u, c.target)?;
    let wc = &sim.weapon_classes[class];
    let eye = u.muzzle();
    // The cutter's own limits (`Sim::repair_candidate`): the reach is 3D and
    // the terrain may not rise before the target's radius. A target far below
    // his eye, like Kashyyyk's sunk pad, needs him closer than its flat distance.
    let to = centre - eye;
    let dir = to.normalize_or_zero();
    if to.length() > wc.lock_on_range + radius - SERVICE_MARGIN
        || !crate::perception::los_terrain(sim, eye, centre - dir * radius)
    {
        return None;
    }
    let dt = crate::TICK_DT;
    let want_yaw = super::gunner_foot::yaw_toward(dir);
    let want_pitch = fmath::asin(dir.y.clamp(-1.0, 1.0));
    let e_yaw = (want_yaw - u.soldier.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    // The cutter up: a switch press on alternate ticks (the press is an
    // edge) until its channel's current weapon is the cutter.
    let up = match channel {
        0 => u.weapon_class == class,
        _ => u
            .cur1
            .and_then(|k| u.held.get(usize::from(k)))
            .is_some_and(|w| w.class == class),
    };
    let button = match (up, channel == 0) {
        (true, true) => buttons::FIRE,
        (true, false) => buttons::SECONDARY_FIRE,
        (false, true) if sim.tick.is_multiple_of(2) => buttons::SWITCH_WEAPON,
        (false, false) if sim.tick.is_multiple_of(2) => buttons::SWITCH_SECONDARY,
        _ => 0,
    };
    Some(ServePlan {
        rx: PadState::quantize(-e_yaw / (u.consts.max_turn_speed * dt)),
        ry: PadState::quantize((want_pitch - u.soldier.pitch) / (u.consts.max_pitch_speed * dt)),
        button,
    })
}

#[cfg(test)]
mod tests;
