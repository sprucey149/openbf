//! Lag compensation (issue #168, `analysis/plan/LAN_MULTIPLAYER.md` §6,
//! "what you saw is what you hit").
//!
//! A LAN player sees the confirmed sim some ticks late. The host stamps how
//! late into his pad ([`PadState::view_lag`], at most [`VIEW_LAG_CAP`]), so
//! every peer knows it on the same tick. His shots are then judged against
//! the world he saw:
//!
//! - [`HitHistory`]: the hit volumes the bolt sweep meets (the soldiers' bolt
//!   prims, the droidekas' spheres and shield, each vehicle hull's pose) at
//!   the end of each of the last [`VIEW_LAG_CAP`] steps. It is derived from
//!   hashed state and the same on every peer, so it is NOT hashed; it is
//!   cloned with the sim.
//! - A round fired this tick (`T`) from a pad with `view_lag = L` is
//!   fast-forwarded `L` ticks at the press: its flight tick `s` (0-based) is
//!   flown and swept against the volumes of the end of step `T - L + s`
//!   (tick `T - L + 1 + s`), the frame the shooter was looking at plus `s`
//!   ticks of flight. It then flies on normally from the advanced point;
//!   this tick's ordinary flight is its flight tick `L + 1`, against the
//!   present. A hit in the rewound ticks is resolved by the ordinary step 5,
//!   so a blast goes off at the rewound impact point. Each kind flies its
//!   rewound ticks as stage 5 flies it:
//!   - straight bolts and rockets: the sweep, then the move; their flight
//!     ends where it would have (the press ray's wall or ground is static);
//!   - shells: the fall, the world along the tick's chord (terrain, water,
//!     map bodies; hulls are the rewound sweep's), the sweep, the move;
//!   - unguided missiles: the steering with no target (its waver draws the
//!     game LCG as in flight), the chord, the sweep, the move;
//!   - fat rays: the wide pass against each unit's rewound fat-ray point
//!     ([`super::combat::SweepBodies::fat`]); its strikes land after every
//!     new round has been fast-forwarded, in order;
//!   - tracers: the move only (they strike nothing), so a lagged round's
//!     tracer stays with it.
//! - A homing missile (one that left with a lock) keeps the old path. It
//!   steers at its target's present point every tick, so its shooter never
//!   leads it, and its lock was earned over the dwell time against the
//!   target as it moved; rewinding its first ticks would aim it at where the
//!   target was and gain nothing. Guided rockets (player-steered) likewise.
//! - A beam (`Sim::fly_bolts`) of a lagged shooter sweeps its ray, and its
//!   piercing chain, against the volumes `L` ticks back.
//! - Mines, droids, auto-turrets and guided rockets as targets are tested
//!   where they are. A vehicle that did not exist `L` ticks ago cannot be hit
//!   in the rewound ticks; one destroyed since cannot be hit.
//! - The cost (the design's trade-off): a soldier can be hit just after he
//!   stepped behind cover, up to the shooter's lag (≤ 8 ticks, 267 ms).
//! - `view_lag = 0` (single player, every bot, every log before it) takes
//!   exactly the old path: no pin moves. A history shorter than `L` (the
//!   round's first ticks) compensates only as far back as it reaches.
//!
//! Only player units read their pad's lag ([`Sim::view_lag_of`]); a bot's is
//! always 0.

use super::combat::SweepBodies;
use super::input::PadState;
use super::Sim;
use crate::collision::HullFilter;
use crate::projectile::Hit;
use crate::unit::Controller;
use std::collections::VecDeque;

/// The most ticks a pad's [`PadState::view_lag`] is honoured for (8 ticks,
/// about 267 ms at 30 Hz), and the history's length. OURS (design §6):
/// the LAN host's `lag_comp_cap` defaults to it. measure: LAN and VPN
/// playtests of #168 (how far back players still accept being hit).
pub const VIEW_LAG_CAP: u8 = 8;

/// One step's end: the tick it left the sim at and its hit volumes.
#[derive(Clone)]
struct HitFrame {
    tick: u64,
    bodies: SweepBodies,
}

/// The hit volumes of the last [`VIEW_LAG_CAP`] steps' ends, oldest first.
/// Derived, never hashed (module rules).
#[derive(Clone, Default)]
pub struct HitHistory {
    frames: VecDeque<HitFrame>,
}

impl HitHistory {
    /// The volumes the sim held at `tick` (the end of the step that made
    /// it), if the history still has them.
    pub(super) fn at(&self, tick: u64) -> Option<&SweepBodies> {
        self.frames
            .iter()
            .find(|f| f.tick == tick)
            .map(|f| &f.bodies)
    }

    /// How many frames it holds.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

impl Sim {
    /// The end of every step (`Sim::step`, `Sim::step_predicted`): keep this
    /// tick's hit volumes, dropping the oldest past [`VIEW_LAG_CAP`].
    pub(super) fn record_hit_frame(&mut self) {
        let mut bodies = self.sweep_bodies();
        bodies.hulls = Some(self.collision.hull_poses().collect());
        let h = &mut self.hit_history.frames;
        if h.len() >= usize::from(VIEW_LAG_CAP) {
            h.pop_front();
        }
        h.push_back(HitFrame {
            tick: self.tick,
            bodies,
        });
    }

    /// Step 4's start: each unit's lag this tick from its pad - a player
    /// unit's [`PadState::view_lag`] capped at [`VIEW_LAG_CAP`], 0 for every
    /// other controller (a bot never gets one). Indexed by unit.
    pub(super) fn take_view_lags(&mut self, unit_pads: &[PadState]) {
        self.view_lags.clear();
        self.view_lags.extend(
            self.units
                .iter()
                .zip(unit_pads)
                .map(|(u, p)| match u.controller {
                    Controller::Player { .. } => p.view_lag.min(VIEW_LAG_CAP),
                    _ => 0,
                }),
        );
    }

    /// The lag unit `index`'s pad carried this tick (0 for a bot, and for a
    /// unit that had no pad).
    pub fn view_lag_of(&self, index: u32) -> u8 {
        self.view_lags.get(index as usize).copied().unwrap_or(0)
    }

    /// How many ticks back a shot of `lag` can be compensated this tick:
    /// `lag`, or less when the history does not reach that far.
    fn reachable_lag(&self, lag: u8) -> u8 {
        let mut l = 0;
        while l < lag
            && self
                .tick
                .checked_sub(u64::from(l))
                .is_some_and(|t| self.hit_history.at(t).is_some())
        {
            l += 1;
        }
        l
    }

    /// The rewound volumes a beam of `owner` sweeps this tick (module rules),
    /// `None` for no lag.
    pub(super) fn rewound_for(&self, owner: crate::unit::UnitHandle) -> Option<&SweepBodies> {
        if self.units.get(owner.index as usize).map(|u| u.gen) != Some(owner.gen) {
            return None;
        }
        let l = self.reachable_lag(self.view_lag_of(owner.index));
        (l > 0)
            .then(|| self.hit_history.at(self.tick + 1 - u64::from(l)))
            .flatten()
    }

    /// End of step 4: fast-forward every round fired this tick (the ones
    /// from `first` on) whose shooter's pad had a lag (module rules).
    pub(super) fn compensate_new_bolts(&mut self, first: usize, dt: f32) {
        if self.view_lags.iter().all(|&l| l == 0) || first >= self.bolts.len() {
            return;
        }
        let mut bolts = std::mem::take(&mut self.bolts);
        let mut strikes = Vec::new();
        for b in &mut bolts[first..] {
            let ord = &self.weapon_classes[b.weapon_class].ordnance;
            let (fat, missile) = (ord.fat_ray, ord.missile);
            let homing = b.missile.is_some_and(|m| m.target.is_some());
            let owned = self
                .units
                .get(b.owner.index as usize)
                .is_some_and(|u| u.gen == b.owner.gen);
            if homing || !owned {
                continue;
            }
            let l = self.reachable_lag(self.view_lag_of(b.owner.index));
            for s in 0..u64::from(l) {
                let tick = self.tick + 1 + s - u64::from(l);
                let Some(frame) = self.hit_history.at(tick) else {
                    break;
                };
                if let Some(class) = fat {
                    let hits = super::fatray::fat_ray_pass(b, &class, dt, &frame.fat);
                    strikes.extend(hits.into_iter().map(|(v, d)| (b.clone(), v, d)));
                } else {
                    // Stage 5's flight for this tick (`fly_missiles`,
                    // `fly_shells`): steer or fall, then the world's chord.
                    if b.hit == Hit::None {
                        if let (Some(c), Some(_)) = (missile, b.missile) {
                            b.guide(&c, None, dt, &mut self.lcg);
                        } else if b.gravity > 0.0 {
                            b.vel.y -= b.gravity * dt;
                        }
                    }
                    if b.missile.is_some() || b.gravity > 0.0 {
                        if let Some((hit, end, left)) = self.chord_hit(b, dt, HullFilter::Static) {
                            (b.hit, b.end, b.remaining) = (hit, end, left);
                        }
                    }
                    // A tracer's sweep strikes nothing (`sweep_bolt`).
                    self.sweep_bolt(frame, b, dt, &[]);
                }
                if b.advance(dt) {
                    // Its flight is over (a rewound hit, or the world): step
                    // 5 resolves it without sweeping again.
                    break;
                }
            }
        }
        self.bolts = bolts;
        for (b, victim, damage) in strikes {
            self.fat_ray_strike(b, victim, damage);
        }
    }
}

#[cfg(test)]
mod tests;
