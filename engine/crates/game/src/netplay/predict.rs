//! Local prediction for a lockstep peer (LAN §6): the local soldier, his
//! vehicle, the camera, the reticle and his own shot feedback drawn ahead of
//! the round trip. Presentation only.
//!
//! Rules:
//! - The confirmed sim is the single source of truth. A [`Predictor`] keeps
//!   a *copy* of it (`Sim: Clone`) stepped with [`Sim::step_predicted`] over
//!   the local player's pending inputs (sent, not yet stepped): the local
//!   slot plays each pending input, every other human slot repeats its last
//!   bundle pad (once-only fields cleared), and the bots replay their last
//!   think. Nothing predicted is ever written into the confirmed sim except
//!   between [`Predictor::swap_in`] and [`Predictor::swap_out`], which put
//!   the drawn values back bit for bit: `swap_in` keeps the confirmed unit
//!   and the vehicle it writes whole, and `swap_out` puts them back whole,
//!   so no field a drawn pose writes can stay behind (issue #168: a saber
//!   hero's melee clock, written by a predicted swing over an idle unit and
//!   not restored by the idle pose, parted the hero's own game from the
//!   others).
//! - When a new confirmed tick lands the copy is cloned again from it and
//!   stepped over the inputs still pending; a frame that only sent a new
//!   input steps the copy it has one more tick.
//! - Every pose is numbered by the newest local input it holds: a confirmed
//!   tick's by its bundle ([`TickBundle::consumed`], so a tick the host's
//!   queue starved holds the same input as the one before it, and a merged
//!   tick two), a predicted tick's by the pending input it stepped.
//! - Drawing runs on the input clock: the local unit's pose (and its
//!   vehicle's, when it rides one) is drawn the input clock's phase of the
//!   way from the pose before the newest input sent to the pose after it -
//!   the last two predicted ticks, or with nothing pending the confirmed
//!   tick before and the last one - so the drawn soldier advances one tick
//!   per input tick however the confirmed ticks arrive. Its aim is the
//!   newest predicted aim with the inputs not predicted folded on
//!   ([`folded_look`]). It is swapped in only while the predicted unit is
//!   the same life in the same seat as the confirmed one; otherwise the
//!   confirmed pose is drawn.
//! - Error easing: when a new prediction disagrees with the previous one
//!   about the pose after the same input (the player was knocked back,
//!   blocked, killed; the host repeated a tick his input missed), the
//!   difference becomes a drawn offset that falls to zero over
//!   [`EASE_SECS`]. A jump past [`UNIT_SNAP_M`] (a respawn, a teleport)
//!   snaps. The same input, not the same tick: an aim is the sum of the
//!   inputs it took, so a starved or merged tick is no aim error.
//! - Own shot feedback: every shot of the local unit is numbered per weapon
//!   class (the confirmed ticks' count, then on through the predicted
//!   ticks). A predicted shot whose number was not fed back yet is fed back
//!   now ([`Predictor::update`]'s result); the confirmed shot with that
//!   number is muted ([`Predictor::confirm_shots`]). Numbers the confirmed
//!   count has passed are forgotten, so a shot that never came stays
//!   unplayed only once.
//! - Budget guard: the depth is the pending inputs, capped at
//!   [`MAX_DEPTH`] and at what [`FRAME_BUDGET_MS`] of CPU per frame affords
//!   (measured clone and step costs). Past either cap fewer ticks are
//!   predicted ([`PredictMode::Reduced`]); when not even one fits, plain
//!   delay ([`PredictMode::Off`]), retried every [`RETRY_SECS`]. Time is
//!   read here, in the game crate; the core never reads a clock.

use crate::render_blend::{folded_look, lerp_angle, UnitPose, VehiclePose, UNIT_SNAP_M};
use crate::walk::WalkSim;
use core::fmath::FloatExt;
use core::sim::{Event, LookPreview, PadState, Sim};
use core::unit::UnitHandle;
use glam::Vec3;
use net::TickBundle;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// The most ticks predicted (about 250 ms).
pub const MAX_DEPTH: usize = 8;
/// The CPU a frame may spend predicting.
pub const FRAME_BUDGET_MS: f64 = 6.0;
/// A correction's drawn offset falls to zero over this.
pub const EASE_SECS: f32 = 0.25;
/// Over budget with nothing predicted: try one tick again after this.
pub const RETRY_SECS: f32 = 2.0;
/// Errors below this (m) are not eased (float noise).
const ERROR_FLOOR_M: f32 = 1e-4;

/// Which prediction a frame drew.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PredictMode {
    /// Every pending input predicted.
    Full,
    /// Fewer than the pending inputs (the depth cap or the CPU budget).
    Reduced,
    /// Plain delay: nothing predicted (switched off, nothing pending, or
    /// over budget).
    #[default]
    Off,
}

/// The net meter's prediction figures.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct PredictStats {
    pub mode: PredictMode,
    /// Ticks predicted this frame.
    pub depth: usize,
    /// Inputs pending.
    pub pending: usize,
    /// The CPU this frame's prediction took (ms).
    pub cost_ms: f64,
    /// Smoothed clone and predicted-step costs (ms).
    pub clone_ms: f64,
    pub step_ms: f64,
    /// Corrections eased so far, and the last one's size (m).
    pub corrections: u32,
    pub last_error_m: f32,
}

/// The local unit (and the vehicle it rides) as one tick left it.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalPose {
    pub tick: u64,
    /// The newest local input sequence the pose holds (module rules).
    pub seq: u32,
    pub index: usize,
    pub unit: UnitPose,
    pub vehicle: Option<(usize, VehiclePose)>,
}

impl LocalPose {
    /// `h`'s pose in `sim`.
    pub fn of(sim: &Sim, h: UnitHandle) -> Option<Self> {
        let index = h.index as usize;
        let u = sim.units.get(index)?;
        let vehicle = u.mount.and_then(|m| {
            let i = m.vehicle.index as usize;
            sim.vehicles.get(i).map(|v| (i, VehiclePose::of(sim, v)))
        });
        Some(Self {
            tick: sim.tick,
            seq: 0,
            index,
            unit: UnitPose::of(u),
            vehicle,
        })
    }

    fn same_life(&self, other: &LocalPose) -> bool {
        self.index == other.index
            && self.unit.gen == other.unit.gen
            && self.unit.alive == other.unit.alive
            && self.unit.seat == other.unit.seat
    }

    /// The pose `t` of the way from `prev` to this one (this one when they
    /// cannot blend).
    pub fn toward(&self, prev: &LocalPose, t: f32) -> LocalPose {
        let mut o = self.clone();
        let t = t.clamp(0.0, 1.0);
        if !self.same_life(prev)
            || prev.unit.pos.distance_squared(self.unit.pos) > UNIT_SNAP_M * UNIT_SNAP_M
        {
            return o;
        }
        o.unit.pos = prev.unit.pos.lerp(self.unit.pos, t);
        o.unit.yaw = lerp_angle(prev.unit.yaw, self.unit.yaw, t);
        o.unit.pitch = prev.unit.pitch + (self.unit.pitch - prev.unit.pitch) * t;
        if let (Some((i, v)), Some((j, pv))) = (o.vehicle.as_mut(), prev.vehicle.as_ref()) {
            if i == j {
                *v = v.toward(pv, t);
            }
        }
        o
    }
}

/// A drawn offset easing to zero ([`EASE_SECS`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ease {
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    left: f32,
}

impl Ease {
    /// The offset drawn now: position, yaw, pitch.
    pub fn now(&self) -> (Vec3, f32, f32) {
        let k = (self.left / EASE_SECS).clamp(0.0, 1.0);
        (self.pos * k, self.yaw * k, self.pitch * k)
    }

    /// A new error on top of what is still drawn; the ease starts over.
    pub fn add(&mut self, pos: Vec3, yaw: f32, pitch: f32) {
        let (p, y, q) = self.now();
        *self = Ease {
            pos: p + pos,
            yaw: y + yaw,
            pitch: q + pitch,
            left: EASE_SECS,
        };
    }

    pub fn advance(&mut self, dt: f32) {
        self.left = (self.left - dt.fmax(0.0)).fmax(0.0);
    }

    pub fn clear(&mut self) {
        *self = Ease::default();
    }
}

/// A predicted shot of the local unit to feed back now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PredictedShot {
    pub unit: UnitHandle,
    pub weapon_class: u32,
    pub origin: Vec3,
    pub dir: Vec3,
    /// The predicted tick that fired it.
    pub tick: u64,
}

/// Own-shot feedback de-duplication (module rules): shots numbered per
/// (unit index, weapon class).
#[derive(Clone, Debug, Default)]
pub struct ShotLedger {
    /// Confirmed shots so far.
    counts: BTreeMap<(u32, u32), u64>,
    /// Shot numbers already fed back from a prediction.
    played: BTreeSet<(u32, u32, u64)>,
}

impl ShotLedger {
    /// One confirmed tick's `events`: counts `unit`'s shots; true when one
    /// of them was already fed back (its feedback is muted this tick).
    pub fn confirm(&mut self, events: &[Event], unit: u32) -> bool {
        let mut muted = false;
        for e in events {
            if let Event::Shot {
                unit: u,
                weapon_class,
                ..
            } = e
            {
                if u.index != unit {
                    continue;
                }
                let n = self.counts.entry((unit, *weapon_class)).or_default();
                *n += 1;
                muted |= self.played.remove(&(unit, *weapon_class, *n));
            }
        }
        let counts = &self.counts;
        self.played
            .retain(|(u, w, n)| counts.get(&(*u, *w)).is_none_or(|c| n > c));
        muted
    }

    /// A predicted tick's `events`, numbered on from `counts` (the
    /// prediction's running copy of the confirmed counts): the shots of
    /// `unit` not fed back yet.
    pub fn predicted(
        &mut self,
        counts: &mut BTreeMap<(u32, u32), u64>,
        events: &[Event],
        unit: u32,
        tick: u64,
        out: &mut Vec<PredictedShot>,
    ) {
        for e in events {
            if let Event::Shot {
                unit: u,
                weapon_class,
                origin,
                dir,
            } = *e
            {
                if u.index != unit {
                    continue;
                }
                let n = counts.entry((unit, weapon_class)).or_default();
                *n += 1;
                if self.played.insert((unit, weapon_class, *n)) {
                    out.push(PredictedShot {
                        unit: u,
                        weapon_class,
                        origin,
                        dir,
                        tick,
                    });
                }
            }
        }
    }

    /// Shot numbers fed back and not yet confirmed.
    pub fn outstanding(&self) -> usize {
        self.played.len()
    }

    fn counts(&self) -> BTreeMap<(u32, u32), u64> {
        self.counts.clone()
    }
}

/// What [`Predictor::swap_in`] overwrote: the confirmed unit and the
/// vehicle it wrote, whole (module rules).
#[derive(Clone, Debug)]
struct Saved {
    index: usize,
    unit: core::unit::Unit,
    vehicle: Option<(usize, core::vehicle::Vehicle)>,
}

/// One local slot's part of the prediction: its poses, correction, shot
/// ledger and what is swapped in for it (module rules). Local 0's is
/// [`Predictor`]'s own; each further local player of the machine (issue #194
/// B18) has one more, all stepped in the one predicted copy.
#[derive(Clone, Debug, Default)]
struct Track {
    slot: u8,
    /// The copy's poses of this slot's unit: the base tick's, then one per
    /// of its own pending inputs stepped (none past them, module rules).
    poses: Vec<LocalPose>,
    /// How many of this slot's own pending inputs the copy has stepped: the
    /// predictor's depth for this local (at most [`Predictor::depth`]).
    real: usize,
    ease: Ease,
    ledger: ShotLedger,
    /// The copy's running shot counts.
    pred_counts: BTreeMap<(u32, u32), u64>,
    /// The newest input of this slot the confirmed sim consumed.
    through: u32,
    /// This slot's poses after the last two confirmed ticks stepped.
    confirmed: [Option<LocalPose>; 2],
    saved: Option<Saved>,
    shots: Vec<PredictedShot>,
}

impl Track {
    fn new(slot: u8) -> Self {
        Self {
            slot,
            ..Self::default()
        }
    }

    fn note_bundle(&mut self, b: &TickBundle, sim: &Sim) {
        if let Some(seq) = b.consumed(self.slot) {
            self.through = seq;
        }
        let pose = sim
            .players
            .get(usize::from(self.slot))
            .copied()
            .flatten()
            .and_then(|h| LocalPose::of(sim, h))
            .map(|p| LocalPose {
                seq: self.through,
                ..p
            });
        self.confirmed = [self.confirmed[1].take(), pose];
    }

    /// Error easing against the previous prediction (module rules): the
    /// pose after the newest input both predictions hold. Returns the
    /// error eased, if any.
    fn correct(&mut self, old: &[LocalPose]) -> Option<f32> {
        let (o, n) = self
            .poses
            .iter()
            .rev()
            .find_map(|n| old.iter().find(|o| o.seq == n.seq).map(|o| (o, n)))?;
        if !o.same_life(n) {
            self.ease.clear();
            return None;
        }
        let d = o.unit.pos - n.unit.pos;
        let len = d.length();
        if len > UNIT_SNAP_M {
            self.ease.clear();
            return None;
        }
        let yaw = lerp_angle(n.unit.yaw, o.unit.yaw, 1.0) - n.unit.yaw;
        let pitch = o.unit.pitch - n.unit.pitch;
        if len < ERROR_FLOOR_M && yaw.abs() < 1e-5 && pitch.abs() < 1e-5 {
            return None;
        }
        self.ease.add(d, yaw, pitch);
        Some(len)
    }

    fn swap_in(
        &mut self,
        sim: Option<&Sim>,
        walk: &mut WalkSim,
        frac: f32,
        look_pads: &[PadState],
        share: f32,
    ) -> bool {
        if self.saved.is_some() {
            return false;
        }
        let (Some(cur), Some(sim)) = (self.poses.last(), sim) else {
            return false;
        };
        // Nothing predicted: the confirmed tick before the newest.
        let before = self.confirmed[0]
            .as_ref()
            .filter(|p| p.tick + 1 == cur.tick);
        let prev = self
            .poses
            .len()
            .checked_sub(2)
            .and_then(|i| self.poses.get(i))
            .or(before)
            .unwrap_or(cur);
        let mut drawn = cur.toward(prev, frac);
        let Some(u) = walk.sim.units.get(drawn.index) else {
            return false;
        };
        let confirmed = UnitPose::of(u);
        if (confirmed.gen, confirmed.alive, confirmed.seat)
            != (drawn.unit.gen, drawn.unit.alive, drawn.unit.seat)
        {
            return false;
        }
        // The aim: the newest predicted one with the rest folded on.
        let h = UnitHandle {
            index: drawn.index as u32,
            gen: drawn.unit.gen,
        };
        let (look, _) = folded_look(sim, h, look_pads, share);
        let (yaw, pitch) = match look {
            Some(LookPreview::Foot { unit, yaw, pitch }) if unit == drawn.index => (yaw, pitch),
            _ => (cur.unit.yaw, cur.unit.pitch),
        };
        let (dp, dy, dq) = self.ease.now();
        drawn.unit.pos += dp;
        drawn.unit.yaw = yaw + dy;
        drawn.unit.pitch = pitch + dq;
        if let (
            Some((vi, v)),
            Some(LookPreview::Seat {
                vehicle,
                seat,
                look,
                walker,
            }),
        ) = (drawn.vehicle.as_mut(), look)
        {
            if *vi == vehicle {
                if let Some(Some(l)) = v.looks.get_mut(seat) {
                    l[0] = look;
                }
                if walker {
                    v.aim = look;
                }
            }
        }
        if let Some((_, v)) = drawn.vehicle.as_mut() {
            v.pos += dp;
        }
        let vehicle = drawn
            .vehicle
            .as_ref()
            .and_then(|(i, _)| walk.sim.vehicles.get(*i).map(|v| (*i, v.clone())));
        self.saved = Some(Saved {
            index: drawn.index,
            unit: u.clone(),
            vehicle,
        });
        drawn.unit.write_to(&mut walk.sim.units[drawn.index]);
        if let Some((i, v)) = &drawn.vehicle {
            if let Some(dst) = walk.sim.vehicles.get_mut(*i) {
                v.write_to(dst);
            }
        }
        true
    }

    fn swap_out(&mut self, walk: &mut WalkSim) {
        let Some(s) = self.saved.take() else {
            return;
        };
        if let Some(u) = walk.sim.units.get_mut(s.index) {
            *u = s.unit;
        }
        if let Some((i, v)) = s.vehicle {
            if let Some(dst) = walk.sim.vehicles.get_mut(i) {
                *dst = v;
            }
        }
    }
}

/// The local prediction (module rules).
pub struct Predictor {
    /// Off: plain delay.
    pub enabled: bool,
    pub max_depth: usize,
    pub budget_ms: f64,
    sim: Option<Sim>,
    /// The confirmed tick the copy was cloned at.
    base_tick: u64,
    /// Pending inputs stepped into the copy.
    applied: usize,
    /// Local 0's slot's part (its slot is the one `update` names).
    main: Track,
    /// Each further local player's part, local 1 first (issue #194 B18).
    extra: Vec<Track>,
    /// Every human slot's last bundle pad, repeated (the local slots' are
    /// replaced by their pending inputs while predicting).
    others: Vec<PadState>,
    retry: f32,
    clone_ms: Option<f64>,
    step_ms: Option<f64>,
    pub stats: PredictStats,
}

impl Default for Predictor {
    fn default() -> Self {
        Self::new()
    }
}

impl Predictor {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_depth: MAX_DEPTH,
            budget_ms: FRAME_BUDGET_MS,
            sim: None,
            base_tick: 0,
            applied: 0,
            main: Track::default(),
            extra: Vec::new(),
            others: Vec::new(),
            retry: 0.0,
            clone_ms: None,
            step_ms: None,
            stats: PredictStats::default(),
        }
    }

    /// The further local players' slots (issue #194 B18), local 1 first: each
    /// is predicted from its own pending inputs in the same copy. A slot
    /// already held keeps its part; a new one starts empty.
    pub fn set_extra_slots(&mut self, slots: &[u8]) {
        if self.extra.iter().map(|t| t.slot).eq(slots.iter().copied()) {
            return;
        }
        let old = std::mem::take(&mut self.extra);
        self.extra = slots
            .iter()
            .map(|&s| {
                old.iter()
                    .find(|t| t.slot == s)
                    .cloned()
                    .unwrap_or_else(|| Track::new(s))
            })
            .collect();
    }

    fn track(&self, i: usize) -> Option<&Track> {
        match i {
            0 => Some(&self.main),
            _ => self.extra.get(i - 1),
        }
    }

    fn track_mut(&mut self, i: usize) -> Option<&mut Track> {
        match i {
            0 => Some(&mut self.main),
            _ => self.extra.get_mut(i - 1),
        }
    }

    /// A stepped bundle, `sim` after it: the pads other humans repeat in the
    /// prediction, the newest input of `slot` it consumed, and the local
    /// pose it left (and each further local's, issue #194 B18).
    pub fn note_bundle(&mut self, b: &TickBundle, sim: &Sim, slot: u8) {
        self.main.slot = slot;
        self.main.note_bundle(b, sim);
        for t in &mut self.extra {
            t.note_bundle(b, sim);
        }
        self.others.clear();
        for (slot, pad) in &b.pads {
            let s = usize::from(*slot);
            if self.others.len() <= s {
                self.others.resize(s + 1, PadState::default());
            }
            self.others[s] = net::bundle::repeat_pad(pad);
        }
    }

    /// A confirmed tick just stepped: counts the local unit's shots; true
    /// when its shot feedback this tick was already given (mute it).
    pub fn confirm_shots(&mut self, sim: &Sim, local: UnitHandle) -> bool {
        self.main.ledger.confirm(&sim.events, local.index)
    }

    /// [`Predictor::confirm_shots`] for local player `i`'s unit `local` (0 is
    /// the first; false for a local with no part).
    pub fn confirm_shots_of(&mut self, i: usize, sim: &Sim, local: UnitHandle) -> bool {
        self.track_mut(i)
            .is_some_and(|t| t.ledger.confirm(&sim.events, local.index))
    }

    /// The drawn correction offset now.
    pub fn ease(&self) -> &Ease {
        &self.main.ease
    }

    /// The shot ledger (for the meter and tests).
    pub fn ledger(&self) -> &ShotLedger {
        &self.main.ledger
    }

    /// The predicted copy, when one is held.
    pub fn sim(&self) -> Option<&Sim> {
        self.sim.as_ref()
    }

    /// The local poses of the current prediction (base tick first).
    pub fn poses(&self) -> &[LocalPose] {
        &self.main.poses
    }

    /// Local player `i`'s poses of the current prediction (0 is the first).
    pub fn poses_of(&self, i: usize) -> &[LocalPose] {
        self.track(i).map_or(&[], |t| &t.poses)
    }

    /// Local player `i`'s predicted shots to feed back now (0 is the first,
    /// what [`Predictor::update`] returned).
    pub fn shots_of(&self, i: usize) -> &[PredictedShot] {
        self.track(i).map_or(&[], |t| &t.shots)
    }

    /// Ticks predicted (the copy's lead over the confirmed sim).
    pub fn depth(&self) -> usize {
        self.applied
    }

    /// Local player `i`'s own pending inputs in its predicted pose (0 is the
    /// first): [`Predictor::depth`] cut to its own queue, the pads its look
    /// fold starts after.
    pub fn depth_of(&self, i: usize) -> usize {
        self.track(i).map_or(0, |t| t.real)
    }

    /// The local player index (0 is the first) whose part predicts `slot`.
    pub fn index_of_slot(&self, slot: u8) -> Option<usize> {
        std::iter::once(&self.main)
            .chain(&self.extra)
            .position(|t| t.slot == slot)
    }

    /// Forget everything (a new round, a rejoin). The further locals' slots
    /// stay, their parts start over.
    pub fn reset(&mut self) {
        let (enabled, max_depth, budget_ms) = (self.enabled, self.max_depth, self.budget_ms);
        let extra = self.extra.iter().map(|t| Track::new(t.slot)).collect();
        *self = Self {
            enabled,
            max_depth,
            budget_ms,
            extra,
            ..Self::new()
        };
    }

    /// How many ticks this frame can afford (module rules).
    fn afford(&mut self, want: usize, rebase: bool, dt: f32) -> usize {
        let want = want.min(self.max_depth);
        if want == 0 {
            return 0;
        }
        let clone = if rebase {
            self.clone_ms.unwrap_or(0.0)
        } else {
            0.0
        };
        let room = self.budget_ms - clone;
        let n = match self.step_ms {
            _ if room <= 0.0 => 0,
            Some(s) if s > 0.0 => (room / s).floor() as usize,
            _ => want,
        };
        if n == 0 {
            self.retry -= dt;
            if self.retry <= 0.0 {
                self.retry = RETRY_SECS;
                return 1;
            }
            return 0;
        }
        n.min(want)
    }

    fn smooth(old: Option<f64>, v: f64) -> Option<f64> {
        Some(old.map_or(v, |o| o + (v - o) * 0.2))
    }

    /// One frame: re-predict from `confirmed` when it moved on (or step the
    /// copy over newly sent inputs), measure, ease. `slot` is the local
    /// player slot, `pending` its inputs sent and not yet stepped. Returns
    /// the local unit's predicted shots to feed back now.
    pub fn update(
        &mut self,
        confirmed: &Sim,
        slot: u8,
        pending: &[PadState],
        dt: f32,
    ) -> &[PredictedShot] {
        self.update_locals(confirmed, slot, pending, &[], dt)
    }

    /// [`Predictor::update`] for a machine with several local players (issue
    /// #194 B18): `extra_pending[k]` is local `k + 1`'s pending inputs (its
    /// slot set by [`Predictor::set_extra_slots`]). One copy is stepped
    /// with every local slot playing its own pending input (the other
    /// humans repeat their last pads), so the depth is the longest pending
    /// queue's. Past a local's own shorter queue (a slot seated late) the
    /// copy repeats its last pending input (its bundle pad when none is
    /// pending), but its poses and shots stop at its own inputs
    /// ([`Predictor::depth_of`]); its queue growing into the repeated ticks
    /// re-predicts from the confirmed sim. Each local's poses, shots and
    /// correction are its own; the meter's correction figures are local
    /// 0's.
    pub fn update_locals(
        &mut self,
        confirmed: &Sim,
        slot: u8,
        pending: &[PadState],
        extra_pending: &[&[PadState]],
        dt: f32,
    ) -> &[PredictedShot] {
        debug_assert!(
            self.main.saved.is_none() && self.extra.iter().all(|t| t.saved.is_none()),
            "update while swapped in"
        );
        self.main.slot = slot;
        self.main.shots.clear();
        self.main.ease.advance(dt);
        for t in &mut self.extra {
            t.shots.clear();
            t.ease.advance(dt);
        }
        let unit_of = |s: u8| confirmed.players.get(usize::from(s)).copied().flatten();
        let local = unit_of(slot);
        let any_extra = self.extra.iter().any(|t| unit_of(t.slot).is_some());
        let want = extra_pending
            .iter()
            .map(|p| p.len())
            .fold(pending.len(), usize::max);
        if !self.enabled || (local.is_none() && !any_extra) {
            self.sim = None;
            self.applied = 0;
            for t in std::iter::once(&mut self.main).chain(self.extra.iter_mut()) {
                t.poses.clear();
                t.ease.clear();
                t.real = 0;
            }
            self.stats = PredictStats {
                pending: want,
                ..PredictStats::default()
            };
            return &self.main.shots;
        }
        let started = Instant::now();
        // Each track's own queue length, local 0 first.
        let own: Vec<usize> = std::iter::once(pending.len())
            .chain((0..self.extra.len()).map(|k| extra_pending.get(k).map_or(0, |p| p.len())))
            .collect();
        // A local the copy stepped past its own inputs (repeating its last)
        // whose queue has grown since: those ticks hold the wrong pad.
        let stale = std::iter::once(&self.main)
            .chain(&self.extra)
            .zip(&own)
            .any(|(t, &n)| t.real < self.applied && n > t.real);
        let rebase =
            self.sim.is_none() || self.base_tick != confirmed.tick || want < self.applied || stale;
        let allowed = self.afford(want, rebase, dt);
        let mut olds: Vec<Vec<LocalPose>> = Vec::new();
        if rebase {
            let t0 = Instant::now();
            self.sim = Some(confirmed.clone());
            self.clone_ms = Self::smooth(self.clone_ms, t0.elapsed().as_secs_f64() * 1e3);
            self.base_tick = confirmed.tick;
            self.applied = 0;
            for t in std::iter::once(&mut self.main).chain(self.extra.iter_mut()) {
                olds.push(std::mem::take(&mut t.poses));
                t.real = 0;
                t.pred_counts = t.ledger.counts();
                let through = t.through;
                if let Some(h) = unit_of(t.slot) {
                    t.poses.extend(
                        LocalPose::of(confirmed, h).map(|p| LocalPose { seq: through, ..p }),
                    );
                }
            }
        }
        let sim = self.sim.as_mut().expect("predicted copy");
        let mut pads = self.others.clone();
        let top = self
            .extra
            .iter()
            .map(|t| usize::from(t.slot))
            .fold(usize::from(slot), usize::max);
        if pads.len() <= top {
            pads.resize(top + 1, PadState::default());
        }
        while self.applied < allowed.max(self.applied.min(want)) {
            if self.applied >= allowed || started.elapsed().as_secs_f64() * 1e3 > self.budget_ms {
                break;
            }
            let a = self.applied;
            if let Some(p) = pending.get(a) {
                pads[usize::from(slot)] = *p;
            }
            for (t, p) in self.extra.iter().zip(extra_pending) {
                if let Some(p) = p.get(a) {
                    pads[usize::from(t.slot)] = *p;
                }
            }
            let t0 = Instant::now();
            sim.step_predicted(&pads);
            self.step_ms = Self::smooth(self.step_ms, t0.elapsed().as_secs_f64() * 1e3);
            self.applied += 1;
            for (t, &n) in std::iter::once(&mut self.main)
                .chain(self.extra.iter_mut())
                .zip(&own)
            {
                // Past its own inputs: a repeated pad, no pose of its own.
                if a >= n {
                    continue;
                }
                t.real += 1;
                if let Some(h) = sim.players.get(usize::from(t.slot)).copied().flatten() {
                    t.ledger.predicted(
                        &mut t.pred_counts,
                        &sim.events,
                        h.index,
                        sim.tick,
                        &mut t.shots,
                    );
                    let seq = t.through + t.real as u32;
                    t.poses
                        .extend(LocalPose::of(sim, h).map(|p| LocalPose { seq, ..p }));
                }
            }
        }
        for (i, (t, old)) in std::iter::once(&mut self.main)
            .chain(self.extra.iter_mut())
            .zip(&olds)
            .enumerate()
        {
            // The meter's figures are local 0's own.
            if let Some(len) = t.correct(old).filter(|_| i == 0) {
                self.stats.corrections += 1;
                self.stats.last_error_m = len;
            }
        }
        let cost = started.elapsed().as_secs_f64() * 1e3;
        self.stats = PredictStats {
            mode: match (self.applied, want) {
                (0, _) => PredictMode::Off,
                (a, p) if a < p => PredictMode::Reduced,
                _ => PredictMode::Full,
            },
            depth: self.applied,
            pending: want,
            cost_ms: cost,
            clone_ms: self.clone_ms.unwrap_or(0.0),
            step_ms: self.step_ms.unwrap_or(0.0),
            ..self.stats.clone()
        };
        &self.main.shots
    }

    /// Writes the predicted local pose into `walk` for drawing: `frac` of the
    /// way from the pose before the newest input to the pose after it
    /// (module rules), plus the correction, aimed by `look_pads` (the inputs
    /// not predicted, the open input tick last) folded on at `share`.
    /// Returns whether it wrote; [`Predictor::swap_out`] must follow before
    /// anything but drawing reads `walk`.
    pub fn swap_in(
        &mut self,
        walk: &mut WalkSim,
        frac: f32,
        look_pads: &[PadState],
        share: f32,
    ) -> bool {
        self.swap_in_local(0, walk, frac, look_pads, share)
    }

    /// [`Predictor::swap_in`] for local player `i` (0 is the first), its own
    /// look pads; [`Predictor::swap_out`] puts back every one swapped in.
    pub fn swap_in_local(
        &mut self,
        i: usize,
        walk: &mut WalkSim,
        frac: f32,
        look_pads: &[PadState],
        share: f32,
    ) -> bool {
        let sim = self.sim.as_ref();
        let track = match i {
            0 => Some(&mut self.main),
            _ => self.extra.get_mut(i - 1),
        };
        track.is_some_and(|t| t.swap_in(sim, walk, frac, look_pads, share))
    }

    /// The values every [`Predictor::swap_in`] overwrote, back, the last one
    /// first (a no-op without one).
    pub fn swap_out(&mut self, walk: &mut WalkSim) {
        for t in self.extra.iter_mut().rev() {
            t.swap_out(walk);
        }
        self.main.swap_out(walk);
    }

    /// Whether a predicted pose is swapped in now.
    pub fn swapped(&self) -> bool {
        self.main.saved.is_some() || self.extra.iter().any(|t| t.saved.is_some())
    }
}

#[cfg(test)]
#[path = "predict_tests.rs"]
mod tests;
