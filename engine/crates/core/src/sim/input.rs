//! What drives a unit, and the log that replays it: the pad ([`PadState`] and
//! its [`buttons`]), the input log ([`InputLog`]) and [`Sim::run_log`]; and
//! step 2 of the tick, BF1's aim assist, which bends a player slot's sticks
//! toward the unit or vehicle its acquisition picked ([`AimTarget`],
//! `acquire_magnet`, `apply_assist`, [`Sim::magnet_at`]). The replay, pad-slot and aim-assist tests live here.

use super::state::Sim;
use crate::aim_assist::{self, AssistEnv, AssistState, Lead, Sticks};
use crate::collision::lead_point;
use crate::perception;
use crate::unit::{SpawnPost, UnitHandle};
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
    /// [ours] (PR #102): a pointer pick on the spawn screens, applied by
    /// [`Sim::spawn_select`] before this pad's accept. `None` for a pad, a
    /// bot and every log written before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<ScreenPick>,
}

/// A mouse click's choice on a spawn screen, carried in the pad so a replay
/// or a lockstep peer applies it on the same tick ([`PadState::pick`]). The
/// sim takes it only on the screen it belongs to, and a post only while it
/// is one of the slot's team's spawn posts that tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenPick {
    /// The side screen's row: 0 team 1, 1 team 2, anything else Auto Assign.
    Side(u8),
    /// A command post by index ([`SpawnPost::Post`]).
    Post(u16),
    /// A command walker's post ([`SpawnPost::Walker`]).
    Walker { index: u32, gen: u32 },
}

impl ScreenPick {
    /// The pick of a spawn-map post.
    pub fn post(post: SpawnPost) -> Self {
        match post {
            SpawnPost::Post(i) => ScreenPick::Post(i),
            SpawnPost::Walker(h) => ScreenPick::Walker {
                index: h.index,
                gen: h.gen,
            },
        }
    }

    /// The spawn-map post picked, `None` for a side row.
    pub fn spawn_post(self) -> Option<SpawnPost> {
        match self {
            ScreenPick::Side(_) => None,
            ScreenPick::Post(i) => Some(SpawnPost::Post(i)),
            ScreenPick::Walker { index, gen } => {
                Some(SpawnPost::Walker(VehicleHandle { index, gen }))
            }
        }
    }

    /// The side row picked, `None` for a post.
    pub fn side(self) -> Option<super::spawn::SideChoice> {
        use super::spawn::SideChoice;
        match self {
            ScreenPick::Side(0) => Some(SideChoice::Team1),
            ScreenPick::Side(1) => Some(SideChoice::Team2),
            ScreenPick::Side(_) => Some(SideChoice::Auto),
            ScreenPick::Post(_) | ScreenPick::Walker { .. } => None,
        }
    }
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

    /// The fields a frame's pad gives only its first tick: the mouse look,
    /// the fire lag and the screen pick.
    pub fn clear_once(&mut self) {
        self.clear_look();
        self.fire_lag = 0;
        self.pick = None;
    }

    /// Whether [`PadState::clear_once`] would change anything.
    pub fn has_once(&self) -> bool {
        self.has_look() || self.fire_lag != 0 || self.pick.is_some()
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
                Some(t) if self.allied(p.team, t) => 1,
                Some(t) if self.hostile(p.team, t) => -1,
                _ => 0,
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

    /// The buttons unit `unit` pressed this tick (held now, not last tick);
    /// 0 for a bot or no such unit. Read by the black box.
    pub fn press_edges_of(&self, unit: usize) -> u32 {
        self.press_edges.get(unit).copied().unwrap_or(0)
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
mod tests;
