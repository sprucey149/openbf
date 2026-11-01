//! What drives a unit, and the log that replays it: the pad ([`PadState`] and
//! its [`buttons`]), the input log ([`InputLog`]) and [`Sim::run_log`]; and
//! step 2 of the tick, BF1's aim assist, which bends a player slot's sticks
//! toward the unit or vehicle its acquisition picked ([`AimTarget`],
//! `acquire_magnet`, `apply_assist`, [`Sim::magnet_at`]). The replay, pad-slot and aim-assist tests live here.

use super::state::Sim;
use crate::aim_assist::{self, AssistEnv, AssistState, Lead, Sticks};
use crate::collision::lead_point;
use crate::perception;
use crate::unit::UnitHandle;
use crate::vehicle::VehicleHandle;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Button bits for [`PadState::buttons`], named for the locked control design
/// (PLAN §Movement & feel): one contextual ROLL_STANCE button (moving = roll,
/// stationary = stance cycle), BF2-style SPRINT, BF1 passive aim assist (no
/// lock-on button). Physical mapping lives in the `input` crate; the sim only
/// sees these bits.
pub mod buttons {
    pub const FIRE: u32 = 1 << 0;
    pub const JUMP: u32 = 1 << 1;
    /// Contextual: roll while moving; stationary, a stand crouches, a crouch
    /// goes prone on a double tap and stands otherwise, a prone stands
    /// (`Soldier::stance_tap_target`).
    pub const ROLL_STANCE: u32 = 1 << 2;
    pub const SPRINT: u32 = 1 << 3;
    pub const ZOOM: u32 = 1 << 4;
    pub const RELOAD: u32 = 1 << 5;
    pub const ACTION: u32 = 1 << 6; // enter vehicle / interact
    pub const SWITCH_WEAPON: u32 = 1 << 7;
    pub const SECONDARY_FIRE: u32 = 1 << 8;
    /// Squad order FOLLOW ME (`core::squad`, card SQACK) — EDGE-TRIGGERED,
    /// read in `Sim::step` phase 0b.
    pub const SQUAD_FOLLOW: u32 = 1 << 9;
    /// Squad order HOLD POSITION.
    pub const SQUAD_HOLD: u32 = 1 << 10;
    /// Squad order AT EASE: releases the units this slot ordered.
    pub const SQUAD_AT_EASE: u32 = 1 << 11;
    /// Squad order MOVE OUT: the same release.
    pub const SQUAD_MOVE_OUT: u32 = 1 << 12;
    /// Spawn screen: next soldier class. Read ONLY while the player is dead
    /// (`Sim::step` phase 0a); an alive player's press does nothing.
    pub const SPAWN_CLASS_NEXT: u32 = 1 << 13;
    /// Spawn screen: previous soldier class.
    pub const SPAWN_CLASS_PREV: u32 = 1 << 14;
    /// Step channel 1's weapon slot (the PS2's "change secondary",
    /// `analysis/re-grenades.md` §2.4); [`SWITCH_WEAPON`] steps channel 0's.
    /// Both are edge-triggered, on foot (`Sim::step` phase 4).
    pub const SWITCH_SECONDARY: u32 = 1 << 15;
    /// Spawn map (card SPSEL): push the chosen POST toward screen-up on the
    /// map (world −z). Read ONLY while the player is dead (`Sim::step` phase
    /// 0a, `Sim::spawn_select`); the four arms combine into one push.
    pub const SPAWN_POST_UP: u32 = 1 << 16;
    /// Spawn map: push toward screen-down (world +z).
    pub const SPAWN_POST_DOWN: u32 = 1 << 17;
    /// Spawn map: push toward screen-left (world −x).
    pub const SPAWN_POST_LEFT: u32 = 1 << 18;
    /// Spawn map: push toward screen-right (world +x).
    pub const SPAWN_POST_RIGHT: u32 = 1 << 19;
    /// The spawn screens' accept (card SSELA; the PS2's X, re-spawn-flow §1):
    /// side -> spawn map -> class -> spawn. EDGE-TRIGGERED, read only while
    /// the slot is not on the field and `Sim::spawn_screens` is on.
    pub const SPAWN_ACCEPT: u32 = 1 << 20;
    /// The spawn screens' back (the PS2's Triangle): class -> spawn map;
    /// nothing on the spawn map (offline) or on the side screen. On the class
    /// screen a live soldier opened at a friendly post (issue #28, opened by
    /// `ACTION`), back to play.
    pub const SPAWN_BACK: u32 = 1 << 21;
    /// One-tick command from the confirmed pause-menu Respawn action (BUGX).
    /// Recorded/replayed with the other pad bits; no physical button maps to it.
    pub const RESPAWN: u32 = 1 << 22;
    /// The pad is aimed by the mouse (`mx`/`my`): aim assist leaves it alone.
    pub const MOUSE_AIM: u32 = 1 << 23;
    /// The current hero offer; mapped separately from movement/navigation.
    pub const HERO_ACCEPT: u32 = 1 << 24;
    pub const HERO_DECLINE: u32 = 1 << 25;
    pub const FIRST_PERSON: u32 = 1 << 26;
    pub const VIEW_TOGGLE: u32 = 1 << 27;
}

pub const LOOK_UNIT: f32 = 1.0 / 65536.0;

/// The right stick's scale before quantization (the `input` crate applies
/// it): the player's aim turns at this share of the class's turn rate at
/// full stick. Ours (the maintainer, 2026-10-04, FEEL_NOTES: the camera was too
/// sensitive); bots do not come through here. The aim assist's weight
/// reads the push before it (`aim_assist::AssistEnv::look_scale`).
pub const LOOK_SCALE: f32 = 0.80;

fn zero(v: &i32) -> bool {
    *v == 0
}

fn zero_u8(v: &u8) -> bool {
    *v == 0
}

/// One controller's state for one tick. Extend as input needs grow (axes are
/// quantized to i16 to keep the log bit-exact across platforms).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadState {
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
    pub buttons: u32,
    #[serde(default, skip_serializing_if = "zero")]
    pub mx: i32,
    #[serde(default, skip_serializing_if = "zero")]
    pub my: i32,
    /// [ours] (the maintainer, 2026-10-29): on a FIRE press, the share of
    /// the tick's stick turn still to come when the shot leaves, in 1/255
    /// (the window draws a controller's aim part way into the next tick's
    /// turn). 0: the shot leaves after the whole turn (bots, the mouse, old
    /// logs). See `Sim::step` phase 4.
    #[serde(default, skip_serializing_if = "zero_u8")]
    pub fire_lag: u8,
}

impl PadState {
    pub fn pressed(&self, bit: u32) -> bool {
        self.buttons & bit != 0
    }

    /// Axis as f32 in [-1, 1] (i16::MIN maps to -1.0 exactly via the /32767
    /// convention with a final clamp).
    pub fn axis(v: i16) -> f32 {
        (f32::from(v) / 32767.0).clamp(-1.0, 1.0)
    }

    /// Quantize an f32 axis in [-1, 1] to the log-exact i16 form.
    pub fn quantize(v: f32) -> i16 {
        (v.clamp(-1.0, 1.0) * 32767.0) as i16
    }

    pub fn look(&self) -> (f32, f32) {
        (self.mx as f32 * LOOK_UNIT, self.my as f32 * LOOK_UNIT)
    }

    pub fn has_look(&self) -> bool {
        self.mx != 0 || self.my != 0
    }

    pub fn mouse_aim(&self) -> bool {
        self.pressed(buttons::MOUSE_AIM)
    }

    pub fn quantize_look(rad: f64) -> i32 {
        (rad / f64::from(LOOK_UNIT))
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    }

    pub fn clear_look(&mut self) {
        self.mx = 0;
        self.my = 0;
    }

    /// The fields a frame's pad gives only its first tick: the mouse look
    /// and the fire lag.
    pub fn clear_once(&mut self) {
        self.clear_look();
        self.fire_lag = 0;
    }

    /// The fire lag of a press drawn `drawn` (0..1) of the way into the
    /// next tick's turn.
    pub fn lag_of(drawn: f32) -> u8 {
        ((1.0 - drawn.clamp(0.0, 1.0)) * 255.0).round() as u8
    }

    /// The share of the tick's turn taken before the shot (1 at lag 0).
    pub fn fire_share(&self) -> f32 {
        1.0 - f32::from(self.fire_lag) / 255.0
    }
}

/// Deterministic input log: seed + per-tick pad states.
///
/// FLAT layout: `ticks` holds `slots` pads per tick back to back, so tick `t`
/// slot `s` is `ticks[t * slots + s]`. `slots` defaults to 1 on load, which is
/// exactly what the pre-P9 single-player logs on disk (no `slots` field) mean.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputLog {
    pub seed: u32,
    /// Player slots recorded per tick (>= 1).
    #[serde(default = "InputLog::one_slot")]
    pub slots: u8,
    pub ticks: Vec<PadState>,
}

impl Default for InputLog {
    fn default() -> Self {
        Self {
            seed: 0,
            slots: 1,
            ticks: Vec::new(),
        }
    }
}

impl InputLog {
    fn one_slot() -> u8 {
        1
    }

    /// Slots per tick, never 0 (a log claiming 0 slots is read as 1).
    pub fn slot_count(&self) -> usize {
        usize::from(self.slots).max(1)
    }

    /// Recorded ticks (a trailing partial tick is dropped).
    pub fn tick_count(&self) -> usize {
        self.ticks.len() / self.slot_count()
    }

    /// One slice of pads per tick, ready for [`Sim::step`].
    pub fn frames(&self) -> impl Iterator<Item = &[PadState]> {
        self.ticks.chunks_exact(self.slot_count())
    }
}

/// What a player's acquisition can pick (card HTGT, `analysis/re-target-hud.md`
/// §2 [O]): a soldier, or a vehicle or building turret — the PS2's candidates
/// are every collision object that is alive (or a building) with health.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AimTarget {
    Unit(UnitHandle),
    Vehicle(VehicleHandle),
    /// A dispensed auto-turret by its id: a building to the PS2's acquisition
    /// (`re-portable-turret.md` 10).
    Turret(u64),
    /// A remote droid by its id. Only ever the reticle's pick (its name,
    /// health and colour; the maintainer: "don't aim assist remote droids"):
    /// it has no [`Candidate`], so no magnet can hold it.
    Droid(u64),
}

impl AimTarget {
    /// The unit, when the target is one.
    pub fn unit(self) -> Option<UnitHandle> {
        match self {
            AimTarget::Unit(h) => Some(h),
            AimTarget::Vehicle(_) | AimTarget::Turret(_) | AimTarget::Droid(_) => None,
        }
    }

    /// The vehicle or turret, when the target is one.
    pub fn vehicle(self) -> Option<VehicleHandle> {
        match self {
            AimTarget::Vehicle(h) => Some(h),
            AimTarget::Unit(_) | AimTarget::Turret(_) | AimTarget::Droid(_) => None,
        }
    }

    pub(crate) fn hash_into(self, eat: &mut impl FnMut(&[u8])) {
        let (tag, i, g) = match self {
            AimTarget::Unit(h) => (0x55, h.index, h.gen),
            AimTarget::Vehicle(h) => (0x56, h.index, h.gen),
            AimTarget::Turret(id) => (0x54, id as u32, (id >> 32) as u32),
            AimTarget::Droid(id) => (0x44, id as u32, (id >> 32) as u32),
        };
        eat(&[tag]);
        eat(&i.to_le_bytes());
        eat(&g.to_le_bytes());
    }
}

/// What the acquisition and the assist read of a candidate.
pub(super) struct Candidate {
    /// The PS2's team nibble: `None` = team 0 (an empty vehicle, an unowned
    /// turret).
    team: Option<u8>,
    /// The object kind the weapon's `target_kind` tests ([`HealthType`]).
    kind: u32,
    /// The acquisition's bounds: centre and radius.
    pub(super) ctr: Vec3,
    r: f32,
    /// Where the LOS ray aims.
    pub(super) aim: Vec3,
    vel: Vec3,
    /// The assist's target radius (a soldier's capsule, a vehicle's bounds).
    assist_r: f32,
    /// The assist's target centre.
    assist_ctr: Vec3,
    /// A vehicle's hull and a turret's building, which the LOS ray passes.
    hull: Option<VehicleHandle>,
    building: Option<u32>,
}

/// The sticky target's cone widening while FIRE is held, π/4 (0x3f490fdb
/// @0x25f4a8, `analysis/re-aim-assist.md` §3.3).
const STICKY_WIDEN: f32 = std::f32::consts::FRAC_PI_4;

/// OURS, PROVISIONAL: the aim stick's deflection toward a challenger, past
/// which the player is steering to it ([`Sim::acquire_magnet_steering`]).
pub const MAGNET_STEER: f32 = 0.2;
/// OURS, PROVISIONAL: how long (s) a held magnet out of sight is kept.
pub const MAGNET_LOS_GRACE: f32 = 0.25;

/// How the player moves the aim this tick, for the magnet's hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Steer {
    /// The aim stick, (right, up) in -1..1.
    Stick(glam::Vec2),
    /// The mouse (no assist; the PS2's tournament every tick).
    Mouse,
}

impl Steer {
    /// The pad's steer.
    pub fn of(pad: &PadState) -> Self {
        if pad.mouse_aim() {
            Steer::Mouse
        } else {
            Steer::Stick(glam::Vec2::new(
                PadState::axis(pad.rx),
                PadState::axis(pad.ry),
            ))
        }
    }
}

/// FUN_00241b98 in its 3-D mode (`re-aim-assist.md` §3.4): `t + 16·d²` for a
/// target of radius `r` centred `ctr`, seen from `pos` along the unit `dir`
/// — `t` its distance along the axis (0 to `maxd + r`), `d` its distance
/// off it, inside the cone of radius `r + t·slope` — else −1. Lower is
/// better.
/// What [`Sim::assisted`] returns: the assisted pad, the slot's timer after
/// it and its lead (`None`: the lead is off).
type Assisted = (PadState, AssistState, Option<(AimTarget, Lead)>);

pub(crate) fn cone_score(r: f32, slope: f32, maxd: f32, pos: Vec3, dir: Vec3, ctr: Vec3) -> f32 {
    let v = ctr - pos;
    let t = v.dot(dir);
    if t < 0.0 || t > maxd + r {
        return -1.0;
    }
    let d2 = (v - dir * t).length_squared();
    let rr = r + t * slope;
    if d2 > rr * rr {
        return -1.0;
    }
    t + 16.0 * d2
}

impl Sim {
    /// What the acquisition reads of `t`, `None` when it is gone. A
    /// vehicle's bounds are its `aim_sphere` (a building turret's building),
    /// else its blast sphere (card VDTH; PROVISIONAL: the PS2 takes the
    /// object's bounds box), centred in its frame.
    pub(super) fn aim_candidate(&self, t: AimTarget) -> Option<Candidate> {
        match t {
            AimTarget::Unit(h) => {
                let u = self.unit(h).filter(|u| u.cloak.visible())?;
                let (ctr, r) = u.aim_bounds();
                let cap = u.capsule();
                Some(Candidate {
                    team: Some(u.team),
                    kind: u.kind(),
                    ctr,
                    r,
                    aim: u.aim_point(),
                    vel: u.soldier.vel,
                    assist_r: cap.radius,
                    assist_ctr: cap.center(),
                    hull: None,
                    building: None,
                })
            }
            AimTarget::Vehicle(h) => {
                let v = self.vehicle(h)?;
                let (c, r) = match (v.aim_sphere, self.blast_setup.get(v.class as usize)) {
                    (Some(s), _) => s,
                    (None, Some(Some(b))) => (b.sphere_centre, b.sphere_radius),
                    _ => (Vec3::ZERO, 0.0),
                };
                let ctr = v.pos + v.rot * c;
                Some(Candidate {
                    team: v.team,
                    kind: v.health.health_type as u32,
                    ctr,
                    r,
                    aim: ctr,
                    vel: v.vel,
                    assist_r: r,
                    assist_ctr: ctr,
                    hull: Some(h),
                    building: v.instance,
                })
            }
            AimTarget::Droid(_) => None,
            AimTarget::Turret(id) => {
                let t = self.live_turret(id)?;
                let ctr = t.centre();
                let r = crate::autoturret::RADIUS;
                Some(Candidate {
                    team: Some(t.team),
                    kind: self.turret_class(t)?.health_type as u32,
                    ctr,
                    r,
                    aim: ctr,
                    vel: t.vel,
                    assist_r: r,
                    assist_ctr: ctr,
                    hull: None,
                    building: None,
                })
            }
        }
    }

    /// BF1's soldier acquisition, the PlayerController's FUN_0025e098
    /// (`analysis/re-aim-assist.md` §3.3, channel 0): keep the sticky
    /// magnet while it stays in the cone (widened by π/4 while FIRE is held)
    /// with a clear line; otherwise run the tournament over the units and
    /// vehicles in the query box — the lowest [`cone_score`] with a clear
    /// line wins, and once a filter-passing target is held only
    /// filter-passing candidates may replace it. The winner is the magnet
    /// only when it passes the weapon's Target* bits and lies within
    /// LockOnRange + r; it is the reticle's pick either way
    /// (`Sim::reticle_pick`), which the HUD's target readout shows
    /// (`analysis/re-target-hud.md` §2 [O], card HTGT).
    ///
    /// The candidates are alive units on foot, alive vehicles and building
    /// turrets but the shooter's own vehicle, and dispensed auto-turrets
    /// (BF2 B2:236430-236440:
    /// alive or a building; a rider is out of the collision world, so its
    /// vehicle stands for it). Port choices: units first in `alive_units()`
    /// order, then vehicles in index order (the PS2's collision-list order
    /// is [H]); the relation is same team = friend, team 0 = 0 (an empty
    /// vehicle, an unowned turret), else enemy (the PS2's default table for
    /// two armies); a destroyed turret is no candidate (the PS2 keeps a dead
    /// building; the port removes it); the query box is tested against each
    /// candidate's bounds sphere and has no 300 cap; the PS2's LockOffAngle
    /// hold (launchers only) and channel 1 are not ported.
    ///
    /// OURS (issue #39 and the maintainer, 2026-10-29: "another bot walks
    /// near the bot I was aiming at and the aim assist just flips over"):
    /// on the PS2 a challenger that scores lower than the held magnet takes
    /// it. Here a held magnet is kept while it is alive, in the held cone
    /// and in sight (out of sight up to [`MAGNET_LOS_GRACE`]), whatever a
    /// challenger scores; on the stick it changes hands only when the stick
    /// pushes past [`MAGNET_STEER`] toward the challenger (the tournament
    /// then over the enemies on that side). A mouse keeps the PS2's rule.
    /// (The tests' entry: the tick calls [`Sim::acquire_magnet_steering`].)
    #[cfg(test)]
    pub(super) fn acquire_magnet(&mut self, slot: usize, ph: UnitHandle, fire_held: bool) {
        self.acquire_magnet_steering(slot, ph, fire_held, Steer::Stick(glam::Vec2::ZERO));
    }

    /// [`Sim::acquire_magnet`] with how the player steers the aim.
    pub(super) fn acquire_magnet_steering(
        &mut self,
        slot: usize,
        ph: UnitHandle,
        fire_held: bool,
        steer: Steer,
    ) {
        let hidden_before = self.magnet_hidden.get(slot).copied().unwrap_or(0);
        if let Some(h) = self.magnet_hidden.get_mut(slot) {
            *h = 0;
        }
        let Some(p) = self.unit(ph) else {
            self.magnet[slot] = None;
            self.reticle_pick[slot] = None;
            return;
        };
        // A player flying a remote droid aims with it: the pick (the
        // reticle's colour) is the droid's, from its muzzle along its aim
        // with its blaster's ranges; it pulls no magnet, so the idle body
        // is never turned.
        let droid = self
            .controlled_droid(ph)
            .map(|d| (d.muzzle(), d.aim_frame().0, d.weapon_class));
        let c = &self.weapon_classes[droid.map_or(p.weapon_class, |d| d.2)];
        let (er, lor, angle) = (c.extreme_range, c.lock_on_range, c.lock_on_angle);
        let (team_bits, kind_bits) = (c.target_team, c.target_kind);
        // The cone is centred on the reticle's ray (`re-aim-assist.md`
        // §10.5), not on the view's centre.
        let (pos, dir) = match droid {
            Some((muzzle, aim, _)) => (muzzle, aim),
            None => p.aim_ray(self.ground_rules, self.views()),
        };
        let own = p.mount.map(|m| m.vehicle);
        let own_building = own.and_then(|h| self.vehicle(h)).and_then(|v| v.instance);
        let rel = |c: &Candidate| -> i8 {
            match c.team {
                None => 0,
                Some(t) if t == p.team => 1,
                Some(_) => -1,
            }
        };
        let passes =
            |c: &Candidate| (team_bits >> (rel(c) + 1)) & 1 != 0 && (kind_bits >> c.kind) & 1 != 0;
        // The LOS ray stops r short of the aim point, on the shooter's side,
        // and passes the shooter's own vehicle and the target's hull.
        let clear = |c: &Candidate| {
            perception::los_seat(
                self,
                pos,
                c.aim + (pos - c.aim).normalize_or_zero() * c.r,
                crate::collision::HullFilter::Except([own, c.hull]),
                [own_building, c.building],
            )
        };
        let live = |t: AimTarget| match t {
            AimTarget::Unit(h) => self.unit(h).is_some_and(|u| u.mount.is_none()),
            AimTarget::Vehicle(h) => self.vehicle(h).is_some() && Some(h) != own,
            AimTarget::Turret(id) => self.live_turret(id).is_some(),
            AimTarget::Droid(_) => false,
        };

        let mut target = self.magnet[slot].filter(|&t| self.aim_candidate(t).is_some());
        let mut valid = target.is_some();
        let mut best = f32::MAX;
        let mut hidden = 0u8;
        // The stick's push toward a challenger (ours): only enemies on that
        // side of the held one may take it.
        let mut toward: Option<(Vec3, glam::Vec2)> = None;
        let right = dir.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(dir);
        if let Some((t, u)) = target.and_then(|t| Some((t, self.aim_candidate(t)?))) {
            let cone = angle + if fire_held { STICKY_WIDEN } else { 0.0 };
            // OURS (the assist's lead): the held magnet is kept while its
            // lead point is in the cone, where the reticle tracking it is.
            let lead = match self.assist_lead.get(slot).copied().flatten() {
                Some((held, l)) if held == t => l.offset,
                _ => Vec3::ZERO,
            };
            let s = cone_score(u.r, cone, er, pos, dir, u.ctr + lead);
            if s >= 0.0 && live(t) && !clear(&u) {
                hidden = hidden_before.saturating_add(1);
            }
            let grace = f32::from(hidden) * crate::TICK_DT <= MAGNET_LOS_GRACE;
            if s >= 0.0 && live(t) && grace {
                valid = passes(&u) && (u.ctr - pos).length() <= lor + u.r;
                best = match steer {
                    Steer::Mouse => s,
                    Steer::Stick(v) if v.length() > MAGNET_STEER => {
                        toward = Some((u.ctr, v));
                        s
                    }
                    Steer::Stick(_) => 0.0,
                };
            } else {
                target = None;
                valid = false;
                hidden = 0;
            }
        }
        if 0.0 < best {
            let mid = pos + dir * (er * 0.5);
            let units = self.alive_units().map(|(h, _)| AimTarget::Unit(h));
            let vehicles = self.alive_vehicles().map(|(h, _)| AimTarget::Vehicle(h));
            let turrets = self.turrets.iter().map(|t| AimTarget::Turret(t.id));
            for t in units.chain(vehicles).chain(turrets) {
                if t == AimTarget::Unit(ph) || !live(t) {
                    continue;
                }
                let Some(u) = self.aim_candidate(t) else {
                    continue;
                };
                let pass = passes(&u);
                if valid && !pass {
                    continue;
                }
                if (u.ctr - mid).abs().cmpgt(Vec3::splat(er + u.r)).any()
                    || (droid.is_none() && p.in_aim_box(u.ctr))
                {
                    continue;
                }
                if let Some((held, v)) = toward {
                    let off = u.ctr - held;
                    let side = glam::Vec2::new(off.dot(right), off.dot(up));
                    if v.dot(side.normalize_or_zero()) <= MAGNET_STEER {
                        continue;
                    }
                }
                let s = cone_score(u.r, angle, er, pos, dir, u.ctr);
                if 0.0 <= s && s < best && clear(&u) {
                    best = s;
                    target = Some(t);
                    valid = pass && (u.ctr - pos).length() <= lor + u.r;
                }
            }
        }
        let mut pick = target.and_then(|t| self.aim_candidate(t).map(|u| (t, rel(&u))));
        // [ours] The maintainer: a remote droid under the reticle shows its
        // name, health and colour and gets no aim assist. So it is read here,
        // past the magnet's choice, and only ever into the pick; the same
        // cone and score the other targets' picks use.
        let mut near = target
            .and_then(|t| self.aim_candidate(t))
            .map(|u| cone_score(u.r, angle, er, pos, dir, u.ctr))
            .filter(|&s| s >= 0.0)
            .unwrap_or(f32::MAX);
        for d in &self.droids {
            if !d.alive() || (d.owner == ph && d.controlled) {
                continue;
            }
            let r = crate::remote::RADIUS;
            let s = cone_score(r, angle, er, pos, dir, d.pos);
            if s < 0.0 || s >= near {
                continue;
            }
            let c = Candidate {
                team: Some(d.team),
                kind: 0,
                ctr: d.pos,
                r,
                aim: d.pos,
                vel: d.vel,
                assist_r: r,
                assist_ctr: d.pos,
                hull: None,
                building: None,
            };
            if clear(&c) {
                near = s;
                pick = Some((AimTarget::Droid(d.id), rel(&c)));
            }
        }
        self.magnet[slot] = if valid && droid.is_none() {
            target
        } else {
            None
        };
        self.reticle_pick[slot] = pick;
        if let Some(h) = self.magnet_hidden.get_mut(slot) {
            *h = hidden;
        }
    }

    /// Apply core::aim_assist to slot `slot`'s pad against magnet `m`.
    pub(super) fn apply_assist(
        &mut self,
        slot: usize,
        ph: UnitHandle,
        m: AimTarget,
        pad: &PadState,
        dt: f32,
    ) -> PadState {
        let Some((out, st, lead)) = self.assisted(slot, ph, m, pad, dt) else {
            return *pad;
        };
        self.assist[slot] = st;
        if let Some(l) = lead {
            self.assist_lead[slot] = Some(l);
        }
        out
    }

    /// [`Sim::apply_assist`] without keeping its state ([`Assisted`]).
    /// `None` when the player or the magnet is gone (the pad as it is).
    pub(super) fn assisted(
        &self,
        slot: usize,
        ph: UnitHandle,
        m: AimTarget,
        pad: &PadState,
        dt: f32,
    ) -> Option<Assisted> {
        let (Some(p), Some(t)) = (self.unit(ph), self.aim_candidate(m)) else {
            return None;
        };
        // The pull steers the reticle's ray (card AIMA), not the chest's.
        let (_, lat, _) = p.aim_frame();
        let (eye, fwd) = p.aim_ray(self.ground_rules, self.views());
        let cap = p.capsule();
        let center = t.assist_ctr;
        let ord = &self.weapon_classes[p.weapon_class].ordnance;
        let speed = ord.velocity;
        let mut kept = None;
        let (lead, lead_vel) = if self.assist_lead_on {
            // OURS (`core::aim_assist`'s lead): the low-passed velocity times
            // the round's flight from where it leaves (the fire point, the
            // scope point scoped), plus the drop a falling round makes up.
            // The sweep meets the target where a tick left it: the flight's
            // last tick is met where it began, on average half a tick short
            // of the flight (term 1 carries the reticle along with the
            // target through the tick's move); a soldier is met
            // `HUMAN_SHOT_LAG` back, where the human sees him.
            let ahead = -0.5 * crate::TICK_DT
                - match m {
                    AimTarget::Unit(_) => crate::unit::HUMAN_SHOT_LAG,
                    AimTarget::Vehicle(_) | AimTarget::Turret(_) | AimTarget::Droid(_) => 0.0,
                };
            let from = if p.scope_path() {
                p.scope_point(self.ground_rules)
            } else {
                p.fire_point()
            };
            let fall = if ord.kind == crate::weapon::OrdKind::Shell {
                ord.gravity * crate::weapon::SHELL_G
            } else {
                0.0
            };
            let mut l = match self.assist_lead.get(slot).copied().flatten() {
                Some((held, l)) if held == m => l,
                _ => Lead::start(t.vel),
            };
            // A fast round meets a soldier where he was at the press
            // (`unit::human_meet_lag`): no flight to lead.
            let instant = matches!(m, AimTarget::Unit(_)) && crate::unit::fast_round(speed);
            let eased = l.step(dt, t.vel, |v| {
                if instant {
                    return v * -crate::unit::HUMAN_SHOT_LAG;
                }
                lead_point(center - from, v, speed).map_or(Vec3::ZERO, |rel| {
                    let secs = rel.length() / speed.max(1e-3);
                    rel - (center - from) + v * ahead + Vec3::Y * (0.5 * fall * secs * secs)
                })
            });
            kept = Some((m, l));
            (Some(center + l.offset), Some(t.vel + eased))
        } else {
            // Ballistic lead (FUN_0026d3f0, flat-fire branch — bolts have no
            // gravity).
            let lead = lead_point(center - eye, t.vel, speed).map(|rel| eye + rel);
            (lead, None)
        };
        // Sign bridge: aim_assist yaw stick is POSITIVE-LEFT (BF1 f2
        // semantics); our rx is positive-right.
        let mut sticks = Sticks {
            move_mag: PadState::axis(pad.ly),
            yaw: -PadState::axis(pad.rx),
            pitch: PadState::axis(pad.ry),
        };
        let env = AssistEnv {
            self_pos: eye,
            self_vel: p.soldier.vel,
            self_radius: cap.radius,
            aim_fwd: fwd,
            lat,
            target_center: center,
            target_vel: t.vel,
            target_radius: t.assist_r,
            lead_point: lead,
            turn_rate: p.consts.max_turn_speed,
            // Zoomed, the assist's rate is divided as the turn is
            // (`re-zoom.md` §6).
            zoom_factor: p.zoom.map(|z| z.divisor),
            lock_on_range: self.weapon_classes[p.weapon_class].lock_on_range,
            fire_held: pad.pressed(buttons::FIRE),
            armed: self.assist_armed,
            lead_vel,
            look_scale: LOOK_SCALE,
        };
        let mut st = self.assist.get(slot).copied().unwrap_or_default();
        aim_assist::apply(dt, &mut sticks, &env, &mut st, self.assist_intensity);
        let out = PadState {
            rx: PadState::quantize(-sticks.yaw),
            ry: PadState::quantize(sticks.pitch),
            ..*pad
        };
        Some((out, st, kept))
    }

    /// The aim magnet of player slot `slot` (`None` when that slot has no
    /// target, or no such slot). The HUD reads slot 0.
    pub fn magnet_at(&self, slot: usize) -> Option<AimTarget> {
        self.magnet.get(slot).copied().flatten()
    }

    /// Run a whole input log from scratch; returns the final hash.
    pub fn run_log(log: &InputLog) -> u64 {
        let mut sim = Sim::new(log.seed);
        for pads in log.frames() {
            sim.step(pads);
        }
        sim.hash()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::tests::{fire, flat_ground};
    use crate::unit::{Controller, UnitSpec};
    use glam::Vec3;

    fn demo_log() -> InputLog {
        InputLog {
            seed: 0xC0FF_EE00,
            slots: 1,
            ticks: (0..300)
                .map(|i| PadState {
                    lx: ((i * 37) % 65536 - 32768) as i16,
                    ly: ((i * 91) % 65536 - 32768) as i16,
                    buttons: if i % 7 < 3 { buttons::FIRE } else { 0 },
                    ..Default::default()
                })
                .collect(),
        }
    }

    /// The pre-P9 logs on disk (`artifacts/p3_walk/*.ron`) carry no `slots`
    /// field: they load as one slot and replay to exactly the hash the same
    /// log with `slots: 1` gives.
    #[test]
    fn single_pad_log_without_slots_field_replays_identically() {
        #[derive(Serialize)]
        struct OldLog<'a> {
            seed: u32,
            ticks: &'a [PadState],
        }
        let log = demo_log();
        assert_eq!(log.slots, 1);
        let text = ron::to_string(&OldLog {
            seed: log.seed,
            ticks: &log.ticks,
        })
        .unwrap();
        assert!(!text.contains("slots"), "serialised without a slots field");
        let back: InputLog = ron::from_str(&text).unwrap();
        assert_eq!(back.slots, 1, "a missing slots field reads as one slot");
        assert_eq!(back.tick_count(), log.tick_count());
        assert_eq!(Sim::run_log(&back), Sim::run_log(&log));
    }

    /// Two occupied slots, one pad each: slot 0 walks forward (yaw 0 faces
    /// −Z), slot 1 walks backward, and neither sees the other's stick.
    #[test]
    fn two_slots_drive_two_units_independently() {
        let mut s = Sim::empty(4);
        s.heightfield = Some(flat_ground());
        let a = s
            .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        let b = s
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
            .unwrap();
        assert_eq!(s.players, vec![Some(a), Some(b)], "one unit per slot");
        let fwd = PadState {
            ly: 32767,
            ..Default::default()
        };
        let back = PadState {
            ly: -32767,
            ..Default::default()
        };
        for _ in 0..60 {
            s.step(&[fwd, back]);
        }
        let (pa, pb) = (
            s.unit(a).unwrap().soldier.pos,
            s.unit(b).unwrap().soldier.pos,
        );
        assert!(pa.z < -5.0, "slot 0 walked forward: {pa}");
        assert!(pb.z > 2.0, "slot 1 walked backward: {pb}");
        // Swapping the pads swaps who goes which way.
        let mut s2 = Sim::empty(4);
        s2.heightfield = Some(flat_ground());
        let a2 = s2
            .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        let b2 = s2
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
            .unwrap();
        for _ in 0..60 {
            s2.step(&[back, fwd]);
        }
        assert_eq!(s2.unit(a2).unwrap().soldier.pos.z, pb.z);
        assert_eq!(s2.unit(b2).unwrap().soldier.pos.z, pa.z);
    }

    /// A slot the pad slice does not reach idles — documented behaviour, not
    /// an error (a splitscreen player who has not joined; a net peer whose
    /// packet has not arrived).
    #[test]
    fn missing_pad_idles_the_slot() {
        let mut s = Sim::empty(4);
        s.heightfield = Some(flat_ground());
        let a = s
            .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
            .unwrap();
        let b = s
            .spawn(&UnitSpec::player_in_slot(1, 0, Vec3::new(20.0, 0.0, 0.0)))
            .unwrap();
        let parked = s.unit(b).unwrap().soldier.pos;
        let fwd = PadState {
            ly: 32767,
            ..Default::default()
        };
        // One pad, two slots: slot 1 gets PadState::default().
        for _ in 0..60 {
            s.step(&[fwd]);
        }
        assert!(s.unit(a).unwrap().soldier.pos.z < -5.0, "slot 0 walked");
        assert_eq!(s.unit(b).unwrap().soldier.pos, parked, "slot 1 never moved");
        // An EMPTY slice idles every slot: the sim settles and then holds.
        for _ in 0..60 {
            s.step(&[]);
        }
        let held = s.unit(a).unwrap().soldier.pos;
        for _ in 0..30 {
            s.step(&[]);
        }
        assert_eq!(s.unit(a).unwrap().soldier.pos, held, "no pads: idle");
    }

    #[test]
    fn different_seed_different_hash() {
        let mut a = demo_log();
        let h1 = Sim::run_log(&a);
        a.seed ^= 1;
        assert_ne!(h1, Sim::run_log(&a));
    }

    #[test]
    fn input_log_ron_round_trip() {
        let log = demo_log();
        let s = ron::to_string(&log).unwrap();
        let back: InputLog = ron::from_str(&s).unwrap();
        assert_eq!(Sim::run_log(&log), Sim::run_log(&back));
    }

    /// A player at the origin facing −Z (the rifle: LockOnRange 64,
    /// ExtremeRange 120, LockOnAngle 1°) and the given soldiers, spawned in
    /// order after it.
    fn range(others: &[(u8, Vec3)]) -> (Sim, UnitHandle, Vec<UnitHandle>) {
        let mut s = Sim::empty(5);
        s.heightfield = Some(flat_ground());
        let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        // A level aim: the spawn pitch's ray is in the floor 33 m out.
        s.units[p.index as usize].soldier.pitch = 0.0;
        let o = others
            .iter()
            .map(|&(team, pos)| s.spawn(&UnitSpec::trooper(team, pos)).unwrap())
            .collect();
        s.sync_slot_tables();
        (s, p, o)
    }

    /// The PS2 filters the magnet by team (`re-aim-assist.md` §3.3): a
    /// friend square on the crosshair is the reticle's pick (rel 1), never
    /// the magnet; an enemy there is both.
    #[test]
    fn a_friend_on_the_crosshair_is_picked_but_never_the_magnet() {
        let (mut s, p, o) = range(&[(0, Vec3::new(0.0, 0.0, -20.0))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], None);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), 1)));
        let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), -1)));
    }

    /// It is a cone, not a ray: an enemy 1 m off the axis at 20 m is inside
    /// r + t·slope (1.42 + 0.35) and becomes the magnet.
    #[test]
    fn an_enemy_just_off_the_ray_is_the_magnet() {
        let (mut s, p, o) = range(&[(1, Vec3::new(1.0, 0.0, -20.0))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
        // Well outside the cone: nothing.
        let (mut s, p, _) = range(&[(1, Vec3::new(3.0, 0.0, -20.0))]);
        s.acquire_magnet(0, p, false);
        assert_eq!((s.magnet[0], s.reticle_pick[0]), (None, None));
    }

    /// Past LockOnRange + r but inside ExtremeRange the enemy is picked
    /// (the reticle turns red) but is not the magnet.
    #[test]
    fn past_lock_on_range_is_picked_but_not_the_magnet() {
        let r = (crate::unit::AIM_BOX_MAX - crate::unit::AIM_BOX_MIN).length() * 0.5;
        let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -(64.0 + r + 1.0)))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], None);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), -1)));
        // Past ExtremeRange + r: not even picked.
        let (mut s, p, _) = range(&[(1, Vec3::new(0.0, 0.0, -(120.0 + r + 1.0)))]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], None);
    }

    /// OURS (sniper aim assist): the disc's sniper (LockOnRange 0) takes no
    /// magnet; with the loader's override it takes an enemy at 50 m and at
    /// 120 m, scoped and not, and none past its 128 m reach.
    #[test]
    fn a_sniper_rifle_takes_a_magnet_out_to_its_max_range() {
        let r = (crate::unit::AIM_BOX_MAX - crate::unit::AIM_BOX_MIN).length() * 0.5;
        let disc = crate::weapon::tests::disc_sniper(0.0);
        let mut ours = disc.clone();
        ours.give_sniper_assist();
        for (class, dist, want) in [
            (&disc, 50.0, false),
            (&ours, 50.0, true),
            (&ours, 120.0, true),
            (&ours, 128.0 + r + 1.0, false),
        ] {
            for scoped in [false, true] {
                let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -dist))]);
                let w = s.unit(p).unwrap().weapon_class;
                s.weapon_classes[w] = class.clone();
                if scoped {
                    s.units[p.index as usize].zoom = Some(crate::unit::Zoom {
                        step: 1,
                        value: 8.0,
                        divisor: 8.0,
                        scope: true,
                    });
                    assert!(s.unit(p).unwrap().scoped());
                }
                s.acquire_magnet(0, p, false);
                assert_eq!(
                    s.magnet[0],
                    want.then_some(AimTarget::Unit(o[0])),
                    "range {} at {dist} m scoped {scoped}",
                    class.lock_on_range
                );
            }
        }
    }

    /// The held magnet's cone widens by π/4 while FIRE is held: an enemy
    /// that steps 3 m off the axis at 20 m is kept while firing and dropped
    /// when not.
    #[test]
    fn the_sticky_magnet_holds_a_quarter_turn_wider_while_firing() {
        for (firing, kept) in [(true, true), (false, false)] {
            let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
            s.acquire_magnet(0, p, false);
            assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
            s.unit_mut(o[0]).unwrap().soldier.pos.x = 3.0;
            s.acquire_magnet(0, p, firing);
            assert_eq!(s.magnet[0].is_some(), kept, "firing {firing}");
        }
    }

    /// Two enemies side by side, the second scoring a little better than
    /// the held first: still, or the stick pushed away from the second, the
    /// first is kept; the stick pushed toward the second (left) or a mouse
    /// gives it to the second. A far nearer enemy on the aim's line does not
    /// take a held magnet either.
    #[test]
    fn the_magnet_holds_its_target_unless_the_player_steers_to_another() {
        let a = (1, Vec3::new(0.3, 0.0, -20.0));
        let b = (1, Vec3::new(-0.6, 0.0, -20.0));
        let stick = |x: f32| Steer::Stick(glam::Vec2::new(x, 0.0));
        for (steer, want) in [
            (stick(0.0), 0),
            (stick(0.1), 0),
            (stick(0.6), 0),
            (stick(-0.6), 1),
            (Steer::Mouse, 1),
        ] {
            let (mut s, p, o) = range(&[a, b]);
            s.magnet[0] = Some(AimTarget::Unit(o[0]));
            // Aim a little toward the second.
            s.units[p.index as usize].soldier.yaw = 0.01;
            s.acquire_magnet_steering(0, p, false, steer);
            assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[want])), "{steer:?}");
        }
        // On the aim's line (the eye 1.3 m over a body's middle): the held
        // one at 20 m, the newcomer at 6 m.
        let (mut s, p, o) = range(&[
            (1, Vec3::new(0.3, 0.0, -20.0)),
            (1, Vec3::new(0.0, 0.0, -6.0)),
        ]);
        for &h in &o {
            s.unit_mut(h).unwrap().soldier.pos.y = 1.3;
        }
        s.magnet[0] = Some(AimTarget::Unit(o[0]));
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])), "a nearer enemy");
    }

    /// The maintainer's report (2026-10-29): aiming at one bot 20 m out,
    /// another walks across the aim's line in front of it or behind it. The
    /// player holds the stick still, or nudges it up or down (fine aim, not
    /// toward the passer). The magnet stays on the first all the way; on
    /// the old rule a nudge gave it to the passer 3 m in front, and a still
    /// stick to one 11 m in front.
    #[test]
    fn a_bot_walking_past_the_held_one_does_not_take_the_magnet() {
        let up = Steer::Stick(glam::Vec2::new(0.0, 0.3));
        let still = Steer::Stick(glam::Vec2::ZERO);
        for (z, steer) in [(-17.0, up), (-23.0, up), (-9.0, still), (-17.0, still)] {
            let (mut s, p, o) = range(&[
                (1, Vec3::new(0.0, 0.0, -20.0)),
                (1, Vec3::new(-4.0, 0.0, z)),
            ]);
            // Their middles on the aim's line (the eye 1.3 m over a body's).
            for &h in &o {
                s.unit_mut(h).unwrap().soldier.pos.y = 1.3;
            }
            s.acquire_magnet(0, p, true);
            assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
            for _ in 0..60 {
                s.unit_mut(o[1]).unwrap().soldier.pos.x += 8.0 / 60.0;
                s.acquire_magnet_steering(0, p, true, steer);
                assert_eq!(
                    s.magnet[0],
                    Some(AimTarget::Unit(o[0])),
                    "passer at z {z}, x {:.2}, {steer:?}",
                    s.unit(o[1]).unwrap().soldier.pos.x
                );
            }
        }
    }

    /// The held magnet dies: the enemy beside it takes the magnet.
    #[test]
    fn a_held_magnet_that_dies_hands_over() {
        let (mut s, p, o) = range(&[
            (1, Vec3::new(0.3, 0.0, -20.0)),
            (1, Vec3::new(-0.8, 0.0, -20.0)),
        ]);
        s.magnet[0] = Some(AimTarget::Unit(o[0]));
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
        s.unit_killed(o[0], None);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[1])));
    }

    /// Out of sight for up to `MAGNET_LOS_GRACE` (7 ticks) the held magnet
    /// is kept; longer, it is let go.
    #[test]
    fn a_held_magnet_out_of_sight_is_kept_briefly() {
        use crate::collision::{Body, CollisionWorld, Shape};
        let wall = || {
            Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
                Shape::Box {
                    half: Vec3::new(5.0, 5.0, 0.5),
                },
            )
        };
        for (hidden, kept) in [(7, true), (8, false)] {
            let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
            s.acquire_magnet(0, p, false);
            assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
            s.collision.push_body(wall());
            for _ in 0..hidden {
                s.acquire_magnet(0, p, false);
            }
            assert_eq!(s.magnet[0].is_some(), kept, "{hidden} ticks hidden");
            // Back in sight: held again only if it was kept.
            s.collision = CollisionWorld::new(Vec::new(), Vec::new());
            s.acquire_magnet(0, p, false);
            assert_eq!(s.magnet_hidden[0], 0);
        }
    }

    /// The flicker in play: two enemies 1.5 m apart 20 m out walking across
    /// the view at 4 m/s, the player holding FIRE and not touching the aim
    /// stick for 4 s, the pull on. On the PS2's rule the magnet changes
    /// hands 17 times; it is held.
    #[test]
    fn two_enemies_side_by_side_do_not_trade_the_magnet() {
        let (mut s, _, o) = range(&[
            (1, Vec3::new(0.5, 0.0, -20.0)),
            (1, Vec3::new(-1.0, 0.0, -20.0)),
        ]);
        for &h in &o {
            s.unit_mut(h).unwrap().health.cur = 1.0e9;
            s.unit_mut(h).unwrap().health.max = 1.0e9;
        }
        // The pull on (the SP Auto Aim option): it is what turns the aim.
        s.assist_armed = true;
        let pad = PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        };
        let (mut last, mut changes) = (None, 0);
        for _ in 0..120 {
            // Walking side by side (placed: a bot's own walk would turn).
            for &h in &o {
                let u = s.unit_mut(h).unwrap();
                u.soldier.pos.x += 4.0 / 30.0;
                u.soldier.pos.z = -20.0;
            }
            s.step(&[pad]);
            let m = s.magnet[0];
            if m.is_some() && last.is_some() && m != last {
                changes += 1;
            }
            last = m.or(last);
        }
        assert!(last.is_some(), "a magnet");
        assert!(changes <= 1, "the magnet changed hands {changes} times");
    }

    /// The tournament is order-dependent (§3.3): with nothing held, a
    /// better-scoring friend found FIRST takes the pick and the enemy behind
    /// cannot beat it — no magnet; found after a held enemy, the friend is
    /// skipped. The port's order is the unit table's.
    #[test]
    fn a_friend_found_first_shadows_the_enemy() {
        let friend = (0, Vec3::new(0.0, 0.0, -10.0));
        let enemy = (1, Vec3::new(0.5, 0.0, -20.0));
        let (mut s, p, o) = range(&[friend, enemy]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], None);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), 1)));
        let (mut s, p, o) = range(&[enemy, friend]);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.magnet[0], Some(AimTarget::Unit(o[0])));
    }

    /// A vehicle 20 m down the crosshair, its blast sphere 2 m about a
    /// point 1 m up; `kind` Hover or a building Turret (no pilot seat).
    fn put_vehicle(s: &mut Sim, kind: crate::vehicle::VehicleKind) -> VehicleHandle {
        use crate::vehicle::{BlastSetup, SeatKind, VehicleKind, VehicleSpec};
        use crate::weapon::HealthType;
        let turret = kind == VehicleKind::Turret;
        let v = s.spawn_vehicle(&VehicleSpec {
            kind,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 0.0, -20.0),
            yaw: 0.0,
            max_health: 500.0,
            health_type: if turret {
                HealthType::Building
            } else {
                HealthType::Vehicle
            },
            seats: vec![if turret {
                SeatKind::Gunner(0)
            } else {
                SeatKind::Pilot
            }],
        });
        s.blast_setup = vec![Some(BlastSetup {
            explosion: None,
            sphere_centre: Vec3::new(0.0, 1.0, 0.0),
            sphere_radius: 2.0,
        })];
        v
    }

    /// Card HTGT (`re-target-hud.md` §2): a vehicle on the crosshair is the
    /// reticle's pick, team 0 while empty (relation 0) and an enemy once
    /// the other team holds it; it is the magnet only for a weapon whose
    /// TargetVehicle bit is set and that may take its relation.
    #[test]
    fn a_vehicle_on_the_crosshair_is_picked_and_magnets_by_the_weapons_bits() {
        use crate::vehicle::VehicleKind;
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, VehicleKind::Hover);
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].target_team = 1;
        s.weapon_classes[w].target_kind = 0x1f;
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
        assert_eq!(
            s.magnet[0], None,
            "an enemy-only weapon: team 0 is no magnet"
        );
        s.vehicles[v.index as usize].team = Some(1);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), -1)));
        assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)));
        // BF1's rifles: TargetVehicle 0 - picked, never the magnet.
        s.weapon_classes[w].target_kind = 0x07;
        s.magnet[0] = None;
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), -1)));
        assert_eq!(s.magnet[0], None);
        // Its own team's: a friend.
        s.vehicles[v.index as usize].team = Some(0);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 1)));
    }

    /// A soldier in front of a tank scores lower and takes the pick; a
    /// destroyed vehicle is no candidate.
    #[test]
    fn a_soldier_before_a_vehicle_wins_and_a_wreck_is_no_target() {
        use crate::vehicle::VehicleKind;
        let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -10.0))]);
        let v = put_vehicle(&mut s, VehicleKind::Hover);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Unit(o[0]), -1)));
        s.units[o[0].index as usize].alive = false;
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
        s.vehicles[v.index as usize].alive = false;
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], None);
    }

    /// A building turret is picked through its own building's bodies (its
    /// centre sits inside them); another building in the way still blocks.
    #[test]
    fn a_turret_is_picked_through_its_own_building() {
        use crate::collision::{Body, Shape};
        use crate::vehicle::VehicleKind;
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, VehicleKind::Turret);
        // Its building's bounds win over a blast sphere of radius 0.
        s.blast_setup[0].as_mut().unwrap().sphere_radius = 0.0;
        s.vehicles[v.index as usize].aim_sphere = Some((Vec3::new(0.0, 1.0, 0.0), 3.0));
        s.vehicles[v.index as usize].instance = Some(7);
        let mut shell = Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 1.0, -20.0)),
            Shape::Box {
                half: Vec3::splat(3.0),
            },
        );
        shell.instance = Some(7);
        s.collision.push_body(shell);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
        s.vehicles[v.index as usize].instance = Some(8);
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], None, "another building's shell blocks");
    }

    /// The shooter's own vehicle is never its pick.
    #[test]
    fn the_shooters_own_vehicle_is_not_picked() {
        use crate::vehicle::VehicleKind;
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, VehicleKind::Hover);
        assert!(s.seat_unit(p, v, 0));
        s.acquire_magnet(0, p, false);
        assert_eq!(s.reticle_pick[0], None);
    }

    /// Acceptance 3 of card HTGT: the readout reads the picked vehicle's
    /// health every tick, so shots into its hull move it while the pick
    /// holds - no hit event in between.
    #[test]
    fn shots_into_a_picked_vehicle_drop_its_health_while_it_stays_picked() {
        use crate::collision::{HullPart, Layers, Shape};
        use crate::vehicle::VehicleKind;
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, VehicleKind::Hover);
        s.vehicles[v.index as usize].team = Some(1);
        s.hull_parts = vec![Some(
            vec![HullPart::new(
                glam::Mat4::from_translation(Vec3::Y),
                Shape::Box {
                    half: Vec3::new(2.0, 1.0, 3.0),
                },
                Layers::ALL,
            )]
            .into(),
        )];
        s.sync_hulls();
        // The test rifle's bolt does nothing to a vehicle (scale 0, as
        // BF1's): give it a launcher's full vehicle scale.
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].ordnance.scales[crate::weapon::HealthType::Vehicle as usize] = 1.0;
        let hull = &mut s.vehicles[v.index as usize].health;
        (hull.max, hull.cur) = (5000.0, 5000.0);
        let max = hull.max;
        let mut seen = Vec::new();
        for _ in 0..90 {
            s.step(&[fire()]);
            assert_eq!(
                s.reticle_pick[0].map(|(t, _)| t),
                Some(AimTarget::Vehicle(v))
            );
            seen.push(s.vehicles[v.index as usize].health.cur);
        }
        let last = *seen.last().unwrap();
        assert!(last < max, "hull {last} of {max}");
        assert!(seen.windows(2).all(|w| w[1] <= w[0]), "never rises");
        eprintln!("hull after 90 ticks of fire: {last} of {max}");
    }

    /// The assist pulls toward a vehicle magnet as toward a soldier.
    #[test]
    fn aim_assist_pulls_toward_a_vehicle_target() {
        use crate::vehicle::VehicleKind;
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, VehicleKind::Hover);
        s.vehicles[v.index as usize].pos.x = 0.8;
        s.vehicles[v.index as usize].team = Some(1);
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].target_kind = 0x1f;
        for _ in 0..30 {
            s.step(&[fire()]);
        }
        assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)));
        assert!(
            s.unit(p).unwrap().soldier.yaw < -0.005,
            "yaw {}",
            s.unit(p).unwrap().soldier.yaw
        );
    }

    #[test]
    fn aim_assist_leaves_a_mouse_aimed_pad_alone() {
        use crate::vehicle::VehicleKind;
        let run = |mouse: bool| {
            let (mut s, p, _) = range(&[]);
            let v = put_vehicle(&mut s, VehicleKind::Hover);
            s.vehicles[v.index as usize].pos.x = 0.8;
            s.vehicles[v.index as usize].team = Some(1);
            let w = s.unit(p).unwrap().weapon_class;
            s.weapon_classes[w].target_kind = 0x1f;
            let mut pad = fire();
            if mouse {
                pad.buttons |= buttons::MOUSE_AIM;
            }
            for _ in 0..30 {
                s.step(&[pad]);
            }
            assert_eq!(s.magnet[0], Some(AimTarget::Vehicle(v)));
            s.unit(p).unwrap().soldier.yaw
        };
        assert!(run(false) < -0.005);
        assert_eq!(run(true), 0.0);
    }

    /// HTGTW: an off-pivot centre turns with the turret, rather than orbiting
    /// the unturned location. The radius is too small to cover both rays.
    #[test]
    fn htgtw_turned_turret_uses_the_rotated_sphere() {
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, crate::vehicle::VehicleKind::Turret);
        let turret = &mut s.vehicles[v.index as usize];
        turret.pos = Vec3::new(4.0, 0.0, -16.0);
        turret.rot = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        turret.aim_sphere = Some((Vec3::new(4.0, 1.0, 0.0), 0.25));
        let candidate = s.aim_candidate(AimTarget::Vehicle(v)).unwrap();
        // +X becomes -Z: (4,0,-16) + (0,1,-4).
        assert!(candidate.ctr.distance(Vec3::new(4.0, 1.0, -20.0)) < 1e-5);
        let eye = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views()).0;
        let aim = |s: &mut Sim, point: Vec3| {
            let d = point - eye;
            let player = &mut s.units[p.index as usize];
            player.soldier.yaw = crate::fmath::atan2(-d.x, -d.z);
            player.soldier.pitch = crate::fmath::atan2(d.y, (d.x * d.x + d.z * d.z).sqrt());
            s.acquire_magnet(0, p, false);
        };
        aim(&mut s, Vec3::new(4.0, 1.0, -20.0));
        assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), 0)));
        aim(&mut s, Vec3::new(8.0, 1.0, -16.0));
        assert_eq!(s.reticle_pick[0], None);
    }

    #[test]
    fn htgtw_vehicle_only_weapon_excludes_building_turrets() {
        use crate::vehicle::VehicleKind;
        for (kind, vehicle_magnet) in [(VehicleKind::Hover, true), (VehicleKind::Turret, false)] {
            let (mut s, p, _) = range(&[]);
            let v = put_vehicle(&mut s, kind);
            s.vehicles[v.index as usize].team = Some(1);
            let w = s.unit(p).unwrap().weapon_class;
            s.weapon_classes[w].target_kind = 1 << 3;
            s.acquire_magnet(0, p, false);
            assert_eq!(s.reticle_pick[0], Some((AimTarget::Vehicle(v), -1)));
            assert_eq!(
                s.magnet[0],
                vehicle_magnet.then_some(AimTarget::Vehicle(v)),
                "{kind:?}"
            );
            // Invert the weapon's bits, retaining the same geometry and target.
            s.magnet[0] = None;
            s.weapon_classes[w].target_kind = 1 << 4;
            s.acquire_magnet(0, p, false);
            assert_eq!(
                s.magnet[0],
                (!vehicle_magnet).then_some(AimTarget::Vehicle(v)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn htgtw_crossing_vehicle_assist_leads_in_its_travel_direction() {
        let (mut s, p, _) = range(&[]);
        let v = put_vehicle(&mut s, crate::vehicle::VehicleKind::Hover);
        // Centre exactly on the player's ray: stationary fire has no yaw
        // pull. With no movement stick, tracking cannot supply the lead.
        let (eye, fwd) = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views());
        s.vehicles[v.index as usize].aim_sphere = Some((Vec3::ZERO, 0.5));
        s.vehicles[v.index as usize].pos = eye + fwd * 20.0;
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].ordnance.velocity = 40.0;
        let mut outputs = Vec::new();
        for speed in [0.0, 4.0, -4.0] {
            s.vehicles[v.index as usize].vel = Vec3::X * speed;
            s.assist[0] = Default::default();
            s.assist_lead[0] = None;
            let pad = s.apply_assist(0, p, AimTarget::Vehicle(v), &fire(), 1.0 / 30.0);
            outputs.push(pad.rx);
        }
        assert_eq!(outputs[0], 0);
        assert!(outputs[1] > 100, "rightward lead: {outputs:?}");
        assert!(outputs[2] < -100, "leftward lead: {outputs:?}");
        assert_eq!(outputs[1], -outputs[2]);
    }

    /// An enemy behind a wall fails the LOS ray and is not picked.
    #[test]
    fn an_enemy_behind_a_wall_is_not_picked() {
        use crate::collision::{Body, Shape};
        let (mut s, p, _) = range(&[(1, Vec3::new(0.0, 0.0, -20.0))]);
        s.collision.push_body(Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        s.acquire_magnet(0, p, false);
        assert_eq!((s.magnet[0], s.reticle_pick[0]), (None, None));
    }

    /// FUN_00241b98's two rejections and its score.
    #[test]
    fn cone_score_matches_the_ps2_formula() {
        let (pos, dir) = (Vec3::ZERO, Vec3::NEG_Z);
        assert_eq!(
            cone_score(1.0, 0.0, 10.0, pos, dir, Vec3::new(0.0, 0.0, 1.0)),
            -1.0
        );
        assert_eq!(
            cone_score(1.0, 0.0, 10.0, pos, dir, Vec3::new(0.0, 0.0, -11.5)),
            -1.0
        );
        assert_eq!(
            cone_score(1.0, 0.0, 10.0, pos, dir, Vec3::new(1.5, 0.0, -5.0)),
            -1.0
        );
        assert_eq!(
            cone_score(1.0, 0.1, 10.0, pos, dir, Vec3::new(1.5, 0.0, -5.0)),
            5.0 + 16.0 * 2.25
        );
    }

    /// Aim assist in the world: neutral stick + fire, target slightly right
    /// → yaw pulled negative (positive yaw = left).
    #[test]
    fn aim_assist_pulls_toward_the_unit_target() {
        let mut s = Sim::empty(11);
        let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        s.spawn(&UnitSpec::trooper(1, Vec3::new(0.4, 0.0, -20.0)))
            .unwrap();
        for _ in 0..30 {
            s.step(&[fire()]);
        }
        assert!(s.magnet[0].is_some(), "magnet acquired in the cone");
        assert!(
            s.unit(p).unwrap().soldier.yaw < -0.005,
            "yaw {}",
            s.unit(p).unwrap().soldier.yaw
        );
    }

    /// What [`strafe`] measured.
    struct Strafe {
        hits: u32,
        shots: u32,
        /// The player's yaw after each tick.
        yaws: Vec<f32>,
        /// Ticks the reticle's pick (the HUD's colour and readout) was not
        /// the runner.
        unpicked: u32,
        /// The runner's speed.
        run: f32,
    }

    /// A runner `dist` m ahead crossing at his class's run speed and turning
    /// about every `period` ticks, around the player's line; the player's
    /// rifle firing a round of `speed` and `life`. The player holds his
    /// sticks still, aimed at the runner's centre at the start, and taps
    /// FIRE every 10 ticks for `ticks`, the magnet already acquired (a press
    /// with no magnet arms nothing). `lead`: the assist's lead on or off
    /// (off = the PS2's assist).
    fn strafe(lead: bool, dist: f32, speed: f32, life: f32, period: u32, ticks: u32) -> Strafe {
        let (mut s, p, o) = range(&[(1, Vec3::new(0.0, 0.0, -dist))]);
        s.assist_lead_on = lead;
        let w = s.unit(p).unwrap().weapon_class;
        s.weapon_classes[w].ordnance.velocity = speed;
        s.weapon_classes[w].ordnance.life_span = life;
        let e = o[0];
        let run = s.unit(e).unwrap().consts.max_speed;
        let u = &mut s.units[e.index as usize];
        u.soldier.pos.x = -run * period as f32 * crate::TICK_DT * 0.5;
        u.controller = Controller::Pacer {
            vel: Vec3::X * run,
            period,
        };
        u.health.cur = 1.0e9;
        let (eye, _) = s.unit(p).unwrap().aim_ray(s.ground_rules, s.views());
        let c = s.unit(e).unwrap().capsule().center() - eye;
        let me = &mut s.units[p.index as usize].soldier;
        me.yaw = crate::fmath::atan2(-c.x, -c.z);
        me.pitch = crate::fmath::atan2(c.y, (c.x * c.x + c.z * c.z).sqrt());
        s.acquire_magnet(0, p, false);
        let mut out = Strafe {
            hits: 0,
            shots: 0,
            yaws: Vec::new(),
            unpicked: 0,
            run,
        };
        for k in 0..ticks {
            let pad = if k % 10 == 0 {
                fire()
            } else {
                PadState::default()
            };
            s.step(&[pad]);
            for ev in &s.events {
                match *ev {
                    crate::sim::Event::Shot { unit, .. } if unit == p => out.shots += 1,
                    crate::sim::Event::Hit { victim, .. } if victim == e => out.hits += 1,
                    _ => {}
                }
            }
            s.units[e.index as usize].health.cur = 1.0e9;
            out.yaws.push(s.unit(p).unwrap().soldier.yaw);
            if s.reticle_pick[0].map(|(t, _)| t) != Some(AimTarget::Unit(e)) {
                out.unpicked += 1;
            }
        }
        out
    }

    /// The ticks the reticle's turn changes sense (a wobble), counting only
    /// turns over 0.1 mrad.
    fn wobbles(yaws: &[f32]) -> usize {
        let d: Vec<f32> = yaws
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|d| d.abs() > 1e-4)
            .collect();
        d.windows(2).filter(|w| w[0] * w[1] < 0.0).count()
    }

    /// The largest turn of the reticle in one tick (rad).
    fn largest_step(yaws: &[f32]) -> f32 {
        yaws.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max)
    }

    /// The lead (ours, playtest 2026-10-28): a player holding his sticks
    /// still on a runner crossing at run speed, tapping FIRE, hits far more
    /// often than under the PS2's assist - a rifle's 80 m/s round and a
    /// sniper's 2000 m/s one, at 30 and 60 m. (The disc snipers ship
    /// LockOnRange 0; the loader's `WeaponClass::give_sniper_assist` gives
    /// them a 128 m, 1° magnet; the 2000 m/s round here is the rifle's
    /// magnet with a sniper's speed.) The runner
    /// turns about every 5 s.
    #[test]
    fn the_lead_hits_a_crossing_runner_under_a_still_stick() {
        for (name, speed, life, dist, floor) in [
            ("rifle", 80.0, 1.5, 30.0, 0.6),
            ("rifle", 80.0, 1.5, 60.0, 0.4),
            ("sniper", 2000.0, 0.15, 30.0, 0.9),
            ("sniper", 2000.0, 0.15, 60.0, 0.8),
        ] {
            let ps2 = strafe(false, dist, speed, life, 150, 600);
            let ours = strafe(true, dist, speed, life, 150, 600);
            println!(
                "{name} at {dist} m, runner {} m/s: PS2 assist {}/{}, with the lead {}/{}",
                ours.run, ps2.hits, ps2.shots, ours.hits, ours.shots
            );
            assert_eq!(ps2.shots, ours.shots, "{name} {dist}");
            let share = ours.hits as f32 / ours.shots as f32;
            assert!(share >= floor, "{name} at {dist} m: {share}");
            // Twice the PS2's and ten more, or every shot.
            assert!(
                ours.hits >= (ps2.hits * 2 + 10).min(ours.shots),
                "{name} at {dist} m: {} vs {}",
                ours.hits,
                ps2.hits
            );
            assert_eq!(ours.unpicked, 0, "{name} {dist}: the HUD's pick held");
        }
    }

    /// No jump when the runner turns about (every 1.5 s here): the reticle
    /// never turns further in a tick than the runner's own swing across
    /// that tick (his speed both ways) and the lead's easing
    /// ([`aim_assist::LEAD_MAX_RATE`]) take it, and it changes the sense
    /// of its turn only when he does (no wobble). The HUD's pick (the
    /// reticle's colour and the target readout) stays the runner
    /// throughout.
    #[test]
    fn the_reticle_does_not_jump_when_the_runner_turns_about() {
        const PERIOD: u32 = 45;
        const TICKS: u32 = 600;
        for (speed, life, dist) in [(80.0, 1.5, 30.0), (80.0, 1.5, 60.0), (2000.0, 0.15, 30.0)] {
            let ours = strafe(true, dist, speed, life, PERIOD, TICKS);
            let bound = (2.0 * ours.run + aim_assist::LEAD_MAX_RATE) / dist * crate::TICK_DT;
            let step = largest_step(&ours.yaws);
            let turns = (TICKS / PERIOD) as usize;
            println!(
                "{speed} m/s round at {dist} m: largest step {step:.4} rad of {bound:.4}, {} wobbles for {turns} turns",
                wobbles(&ours.yaws)
            );
            assert!(step <= bound, "{speed} at {dist} m: {step} > {bound}");
            assert!(wobbles(&ours.yaws) <= turns + 1, "{speed} at {dist} m");
            assert_eq!(ours.unpicked, 0, "{speed} at {dist} m: the HUD's pick held");
        }
    }
}
