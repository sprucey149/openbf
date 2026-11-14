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
//!   the drawn values back bit for bit.
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

/// What [`Predictor::swap_in`] overwrote.
#[derive(Clone, Debug)]
struct Saved {
    index: usize,
    unit: UnitPose,
    vehicle: Option<(usize, VehiclePose)>,
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
    /// The copy's local poses: the base tick's, then one per predicted tick.
    poses: Vec<LocalPose>,
    ease: Ease,
    ledger: ShotLedger,
    /// The copy's running shot counts.
    pred_counts: BTreeMap<(u32, u32), u64>,
    /// Every human slot's last bundle pad, repeated.
    others: Vec<PadState>,
    /// The newest local input the confirmed sim consumed.
    through: u32,
    /// The local poses after the last two confirmed ticks stepped.
    confirmed: [Option<LocalPose>; 2],
    retry: f32,
    clone_ms: Option<f64>,
    step_ms: Option<f64>,
    pub stats: PredictStats,
    saved: Option<Saved>,
    shots: Vec<PredictedShot>,
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
            poses: Vec::new(),
            ease: Ease::default(),
            ledger: ShotLedger::default(),
            pred_counts: BTreeMap::new(),
            others: Vec::new(),
            through: 0,
            confirmed: [None, None],
            retry: 0.0,
            clone_ms: None,
            step_ms: None,
            stats: PredictStats::default(),
            saved: None,
            shots: Vec::new(),
        }
    }

    /// A stepped bundle, `sim` after it: the pads other humans repeat in the
    /// prediction, the newest input of `slot` it consumed, and the local
    /// pose it left.
    pub fn note_bundle(&mut self, b: &TickBundle, sim: &Sim, slot: u8) {
        if let Some(seq) = b.consumed(slot) {
            self.through = seq;
        }
        let pose = sim
            .players
            .get(usize::from(slot))
            .copied()
            .flatten()
            .and_then(|h| LocalPose::of(sim, h))
            .map(|p| LocalPose {
                seq: self.through,
                ..p
            });
        self.confirmed = [self.confirmed[1].take(), pose];
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
        self.ledger.confirm(&sim.events, local.index)
    }

    /// The drawn correction offset now.
    pub fn ease(&self) -> &Ease {
        &self.ease
    }

    /// The shot ledger (for the meter and tests).
    pub fn ledger(&self) -> &ShotLedger {
        &self.ledger
    }

    /// The predicted copy, when one is held.
    pub fn sim(&self) -> Option<&Sim> {
        self.sim.as_ref()
    }

    /// The local poses of the current prediction (base tick first).
    pub fn poses(&self) -> &[LocalPose] {
        &self.poses
    }

    /// Ticks predicted (the copy's lead over the confirmed sim).
    pub fn depth(&self) -> usize {
        self.applied
    }

    /// Forget everything (a new round, a rejoin).
    pub fn reset(&mut self) {
        let (enabled, max_depth, budget_ms) = (self.enabled, self.max_depth, self.budget_ms);
        *self = Self {
            enabled,
            max_depth,
            budget_ms,
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
        debug_assert!(self.saved.is_none(), "update while swapped in");
        self.shots.clear();
        self.ease.advance(dt);
        let local = confirmed.players.get(usize::from(slot)).copied().flatten();
        let Some(local) = local.filter(|_| self.enabled) else {
            self.sim = None;
            self.poses.clear();
            self.applied = 0;
            self.ease.clear();
            self.stats = PredictStats {
                pending: pending.len(),
                ..PredictStats::default()
            };
            return &self.shots;
        };
        let started = Instant::now();
        let rebase =
            self.sim.is_none() || self.base_tick != confirmed.tick || pending.len() < self.applied;
        let allowed = self.afford(pending.len(), rebase, dt);
        let old = if rebase {
            let old = std::mem::take(&mut self.poses);
            let t0 = Instant::now();
            self.sim = Some(confirmed.clone());
            self.clone_ms = Self::smooth(self.clone_ms, t0.elapsed().as_secs_f64() * 1e3);
            self.base_tick = confirmed.tick;
            self.applied = 0;
            self.pred_counts = self.ledger.counts();
            let through = self.through;
            self.poses
                .extend(LocalPose::of(confirmed, local).map(|p| LocalPose { seq: through, ..p }));
            Some(old)
        } else {
            None
        };
        let sim = self.sim.as_mut().expect("predicted copy");
        let mut pads = self.others.clone();
        let s = usize::from(slot);
        if pads.len() <= s {
            pads.resize(s + 1, PadState::default());
        }
        while self.applied < allowed.max(self.applied.min(pending.len())) {
            if self.applied >= allowed || started.elapsed().as_secs_f64() * 1e3 > self.budget_ms {
                break;
            }
            pads[s] = pending[self.applied];
            let t0 = Instant::now();
            sim.step_predicted(&pads);
            self.step_ms = Self::smooth(self.step_ms, t0.elapsed().as_secs_f64() * 1e3);
            self.applied += 1;
            if let Some(h) = sim.players.get(s).copied().flatten() {
                self.ledger.predicted(
                    &mut self.pred_counts,
                    &sim.events,
                    h.index,
                    sim.tick,
                    &mut self.shots,
                );
                let seq = self.through + self.applied as u32;
                self.poses
                    .extend(LocalPose::of(sim, h).map(|p| LocalPose { seq, ..p }));
            }
        }
        if let Some(old) = old {
            self.correct(&old);
        }
        let cost = started.elapsed().as_secs_f64() * 1e3;
        self.stats = PredictStats {
            mode: match (self.applied, pending.len()) {
                (0, _) => PredictMode::Off,
                (a, p) if a < p => PredictMode::Reduced,
                _ => PredictMode::Full,
            },
            depth: self.applied,
            pending: pending.len(),
            cost_ms: cost,
            clone_ms: self.clone_ms.unwrap_or(0.0),
            step_ms: self.step_ms.unwrap_or(0.0),
            ..self.stats.clone()
        };
        &self.shots
    }

    /// Error easing against the previous prediction (module rules): the
    /// pose after the newest input both predictions hold.
    fn correct(&mut self, old: &[LocalPose]) {
        let Some((o, n)) = self
            .poses
            .iter()
            .rev()
            .find_map(|n| old.iter().find(|o| o.seq == n.seq).map(|o| (o, n)))
        else {
            return;
        };
        if !o.same_life(n) {
            self.ease.clear();
            return;
        }
        let d = o.unit.pos - n.unit.pos;
        let len = d.length();
        if len > UNIT_SNAP_M {
            self.ease.clear();
            return;
        }
        let yaw = lerp_angle(n.unit.yaw, o.unit.yaw, 1.0) - n.unit.yaw;
        let pitch = o.unit.pitch - n.unit.pitch;
        if len < ERROR_FLOOR_M && yaw.abs() < 1e-5 && pitch.abs() < 1e-5 {
            return;
        }
        self.ease.add(d, yaw, pitch);
        self.stats.corrections += 1;
        self.stats.last_error_m = len;
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
        if self.saved.is_some() {
            return false;
        }
        let (Some(cur), Some(sim)) = (self.poses.last(), self.sim.as_ref()) else {
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
        let vehicle = drawn.vehicle.as_ref().and_then(|(i, _)| {
            walk.sim
                .vehicles
                .get(*i)
                .map(|v| (*i, VehiclePose::of(&walk.sim, v)))
        });
        self.saved = Some(Saved {
            index: drawn.index,
            unit: confirmed,
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

    /// The values [`Predictor::swap_in`] overwrote, back (a no-op without one).
    pub fn swap_out(&mut self, walk: &mut WalkSim) {
        let Some(s) = self.saved.take() else {
            return;
        };
        if let Some(u) = walk.sim.units.get_mut(s.index) {
            s.unit.write_to(u);
        }
        if let Some((i, v)) = s.vehicle {
            if let Some(dst) = walk.sim.vehicles.get_mut(i) {
                v.write_to(dst);
            }
        }
    }

    /// Whether a predicted pose is swapped in now.
    pub fn swapped(&self) -> bool {
        self.saved.is_some()
    }
}

#[cfg(test)]
#[path = "predict_tests.rs"]
mod tests;
