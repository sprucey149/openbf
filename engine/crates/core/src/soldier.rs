//! Soldier v0: stance machine + ground movement, transcribed from the decomp
//! (spec: analysis/re-stance-roll.md, lead-verified 2026-08-30).
//!
//! Implements the LOCKED openbf scheme (PLAN §Movement & feel): BF2's
//! contextual button — moving = roll, stationary = stance cycle — with BF1's
//! prone restored in the cycle, BF2's impulse roll and sprint/energy system.
//! Sources cited per item:
//! - roll predicate: BF2 FUN_00247b88 (0.81 stick gate, 1.732 back-cone);
//! - roll action: BF2 FUN_002471b0 (impulse to RollSpeedFactor*MaxSpeed,
//!   speed in (0.1, target); RollSpeedFactor <= 0 disables);
//! - sprint predicate: BF2 FUN_00247ad8 (start +-30 deg / keep +-60 deg
//!   hysteresis, 0.9*MaxSpeed floor - ours on a slope, card SPUP);
//! - per-stance ControlSpeed factors: BF2 ctor defaults == BF1 hardcoded
//!   call-site values (FUN_00205cc8);
//! - movement model: BF2 FUN_002480a0 shape (target speed approach at
//!   Acceleration; backpedal capped by MaxStrafeSpeed — both games);
//! - the ground (card STHOP, analysis/re-soldier-ground.md [O]): no snap. A
//!   grounded soldier's velocity is steered in the plane of his last floor
//!   contact (BF2 FUN_002480a0 / BF1 FUN_002098b8), the contact is found again
//!   under the NEW feet after the move ([`Soldier::resolve_ground`]; n.y > 0.6,
//!   approach < 0.1, 2 cm), and stand falls by airtime ([`GroundRules`]);
//! - gravity -18; the jump (card JARC, re-stance-roll.md §4.3b): the update's
//!   ground mover runs first and the jump is applied to the velocity it left
//!   (BF2 FUN_00246ed0: vy += the jump velocity, a forward component x
//!   JumpForwardSpeedFactor; BF1 FUN_00208b78: vy = it, + a quarter of it x the
//!   forward stick). JUNI selects BF2 on every map (FEEL_NOTES),
//!   and the next update has no gravity ([`Soldier::take_off`]);
//! - the jump's gates (card JGATE, re-stance-roll.md §4.3c): a press is taken
//!   for 0.3 s after the floor is lost, a crouch stands up instead, BF1's
//!   stand originally waits 0.5 s; JUNI selects BF2 eligibility on all maps.
//!   The energy bar is overdrawn ([`Soldier::jump_open`],
//!   [`Soldier::pay_jump`]);
//! - the stick in the air (card AIRC, re-stance-roll.md §4.3a): BF2 adds it as
//!   an acceleration under a drag (FUN_00248560), BF1 runs a one-sided approach
//!   to a fifth of the walk speeds with no brake (FUN_002098b8) — both
//!   measured on open floor ([`Soldier::air_steer`]). JUNI selects BF2 for
//!   Jump/Fall everywhere; Thrown retains the original per-game rules;
//! - energy costs/restores: BF2 class defaults (ctor + INGAME.LVL
//!   com_inf_default, RE_LOG §BF2 soldier defaults);
//! - prone pitch clamp +-0.4 rad: BF1 FUN_00205cc8.

use crate::fmath;
use crate::sim::{buttons, PadState};
use glam::{Vec2, Vec3};

/// Per-stance input scale factors (fwd, strafe, turn) — BF2 `ControlSpeed`
/// ctor defaults; stand/crouch/prone match BF1's hardcoded values.
pub const CONTROL_SPEED: [[f32; 3]; 8] = [
    [1.0, 1.0, 1.0], // stand
    [0.7, 0.6, 1.0], // crouch
    [0.3, 0.2, 0.5], // prone
    // sprint (fwd = sprint speed multiplier). The turn factor 0.6 is OURS
    // (the maintainer, 2026-10-04, FEEL_NOTES): the ctor default is 0.2, BF2's classes
    // author 0.35 (card CSPD).
    [1.5, 0.5, 0.6],
    [0.03, 0.03, 0.5], // jump/air (x Acceleration: an acceleration, not a speed)
    [0.02, 0.02, 0.6], // roll
    [0.3, 0.3, 1.0],   // jet (the ctor default; a class's own is `jet_control`)
    [0.0, 0.0, 0.1],   // tumble / get-up / dead
];

/// Active BF2 melee movement returned by weapon vtable +0x6c.
#[derive(Debug, Clone, Copy)]
pub struct MotionTarget {
    /// Local forward/right values: ground desired m/s; air acceleration m/s².
    pub forward: f32,
    pub strafe: f32,
    pub weight: f32,
}

/// Soldier tuning, ODF-loadable (BF2 EntitySoldierClass ctor defaults; the
/// game layer overrides from baked classes.ron via ClassDb).
#[derive(Debug, Clone, Copy)]
pub struct SoldierConstants {
    /// Active weapon's per-tick movement output (BF2 vtable +0x6c).
    pub motion_target: Option<MotionTarget>,
    pub immune_to_mines: bool,
    pub max_speed: f32,
    pub max_strafe_speed: f32,
    pub max_turn_speed: f32,
    pub max_pitch_speed: f32,
    pub acceleration: f32,
    /// Stored as jump VELOCITY in both games; BF2's "JumpHeight" key is a
    /// height, converted at load (sqrt(2 x 18 x h): 1.78 -> 8.005 m/s, PCSX2).
    pub jump_vy: f32,
    pub jump_forward_factor: f32,
    pub jump_strafe_factor: f32,
    pub roll_speed_factor: f32,
    pub sprint_accelerate_time: f32,
    pub energy_bar: f32,
    pub energy_restore: f32,
    pub energy_restore_idle: f32,
    /// Per-tick active weapon override, after the stance chooses its rate.
    pub energy_override: Option<f32>,
    pub energy_drain_sprint: f32,
    pub energy_cost_jump: f32,
    pub energy_cost_jump_sprint: f32,
    pub energy_cost_roll: f32,
    /// Aim pitch limits, radians (down, up); prone clamps to +-0.4 (BF1).
    pub pitch_limits: (f32, f32),
    /// PINNED (re-anim-banks.md): BF2 roll = 0.75 * (frames-1)/30 with
    /// human_rifle_diveforward = 35 frames (INGAME.LVL, crc 0xDC2308EB)
    /// = 0.85 s. (BF1's own roll would be 0.75*35/30 = 0.875 s.)
    pub roll_duration: f32,
    /// BF2 stand-up soft lock after roll/landing (FUN_00246850).
    pub soft_lock: f32,
    /// Prone transition HARD locks (movement+turn zeroed) — BF1 clip
    /// durations, since BF2 cut the prone clips and openbf restores BF1
    /// prone: stand/crouch_getdown_prone = 40 frames = 1.3333 s,
    /// prone_getup_stand = 45 frames = 1.5 s (re-anim-banks.md). Each is
    /// half an update under its frames, so the float countdown lets go on
    /// update 40 / 45, as the clip ends (it went one late on 45).
    pub prone_down_lock: f32,
    pub prone_up_lock: f32,
    /// BF2's size term of the aim error at this soldier (card CLSE2,
    /// `analysis/re-soldier-ai.md` 18.3): 2 x class +0x370 = half the
    /// horizontal diagonal of the model's bind-pose box, set by the game
    /// layer per model (`game::roster::AIM_SIZE`); 1.0 for a model not in it.
    pub aim_size: f32,
    /// `CollisionRootScale` (BF2 key 0xc64a53ab, default 1.0; the wookiee
    /// 1.15, BF2's snipers 0.95): the bolt prims' scale (`Unit::bolt_hit`,
    /// issue #41; analysis/re-hitzones.md).
    pub collision_scale: f32,
    /// The jet (card JETP, `analysis/re-jetpack.md` §2): `JetJump`, the kick's
    /// size at the jet's start (engine default 7.0).
    pub jet_jump: f32,
    /// `JetPush`: the jump-jet's lift, m/s^2 (default 14.0; the hover ignores it).
    pub jet_push: f32,
    /// `JetAcceleration` (default 20.0): the jump-jet's stick and the hover's
    /// approach, m/s^2. BF1 has no such key; the game layer gives a BF1 class
    /// its BF2 twin's (`game::roster::BF1_JET_BORROW`).
    pub jet_acceleration: f32,
    /// `JetType = hover`: the hover jet (BF2 stance 7), else the jump-jet (6).
    pub jet_hover: bool,
    /// `JetFuelRechargeRate`, fuel (0..1) a second (default 0). A class has a
    /// jet when this is over 0 ([`SoldierConstants::has_jet`]).
    pub jet_fuel_recharge: f32,
    /// `JetFuelCost`: fuel a second in a jet stance (default 0.2).
    pub jet_fuel_cost: f32,
    /// `JetFuelInitialCost`: fuel the start takes (default 0).
    pub jet_fuel_initial: f32,
    /// `JetFuelMinBorder`: the fuel a start needs (default 0.1; equal passes).
    pub jet_fuel_min_border: f32,
    /// `JetEnergyDrain`: the energy bar's cost of a start (BF2; default 0).
    pub jet_energy_drain: f32,
    /// The `ControlSpeed jet` row (fwd, strafe, turn): the hover's speed
    /// factors and the turn factor of both jet stances (ctor default
    /// 0.3 / 0.3 / 1.0; the game layer reads the class chain's row).
    pub jet_control: [f32; 3],
    pub sprint_control: [f32; 2],
    pub hero_cameras: [Option<crate::unit::CameraSection>; 7],
    /// A channel-0 weapon is a lightsaber (B2 s+0x4e0 bit 0x10000, §3.3): a
    /// class that does not hover double-jumps instead of jetting.
    pub saber: bool,
}

impl Default for SoldierConstants {
    fn default() -> Self {
        Self {
            motion_target: None,
            immune_to_mines: false,
            max_speed: 9.0,
            max_strafe_speed: 7.0,
            max_turn_speed: 2.0,
            max_pitch_speed: 1.0,
            acceleration: 70.0,
            jump_vy: 8.0,
            jump_forward_factor: 1.3,
            jump_strafe_factor: 1.0,
            roll_speed_factor: 1.7,
            sprint_accelerate_time: 0.5,
            energy_bar: 100.0,
            energy_restore: 10.0,
            energy_restore_idle: 15.0,
            energy_override: None,
            energy_drain_sprint: 20.0,
            energy_cost_jump: 10.0,
            energy_cost_jump_sprint: 30.0,
            energy_cost_roll: 10.0,
            pitch_limits: (-0.8, 1.1),
            roll_duration: 0.75 * 34.0 / 30.0,
            soft_lock: 0.3,
            prone_down_lock: 39.5 / 30.0,
            prone_up_lock: 44.5 / 30.0,
            aim_size: 1.0,
            collision_scale: 1.0,
            jet_jump: 7.0,
            jet_push: 14.0,
            jet_acceleration: 20.0,
            jet_hover: false,
            jet_fuel_recharge: 0.0,
            jet_fuel_cost: 0.2,
            jet_fuel_initial: 0.0,
            jet_fuel_min_border: 0.1,
            jet_energy_drain: 0.0,
            jet_control: CONTROL_SPEED[6],
            sprint_control: [CONTROL_SPEED[3][0], CONTROL_SPEED[3][1]],
            hero_cameras: [None; 7],
            saber: false,
        }
    }
}

impl SoldierConstants {
    /// Override the defaults from a soldier class's ODF props (BASE chain
    /// resolved by the source). Keys per RE_LOG §Soldier ODF props / §BF2
    /// soldier defaults; BF1 spells acceleration `Acceleraton` (sic).
    /// BF2's JumpHeight becomes the jump velocity sqrt(2 x g x h) (measured on
    /// the rebel soldier: 1.78 -> 8.0050 m/s, re-stance-roll.md §4.3a; the
    /// setter FUN_00252bb0 is sqrtf((h + h) x 18.0), §4.3c). BF1 has no such
    /// key: its 8.0 stays.
    pub fn from_props(src: &dyn crate::weapon::PropSource) -> Self {
        let mut c = Self::default();
        let f = |key: &str| -> Option<f32> {
            src.prop(key)?
                .split_whitespace()
                .next()?
                .parse::<f32>()
                .ok()
        };
        c.immune_to_mines = f("ImmuneToMines").is_some_and(|v| v != 0.0);
        let set = |key: &str, slot: &mut f32| {
            if let Some(v) = f(key) {
                *slot = v;
            }
        };
        set("MaxSpeed", &mut c.max_speed);
        set("MaxStrafeSpeed", &mut c.max_strafe_speed);
        set("MaxTurnSpeed", &mut c.max_turn_speed);
        set("CollisionRootScale", &mut c.collision_scale);
        set("MaxPitchSpeed", &mut c.max_pitch_speed);
        set("Acceleration", &mut c.acceleration);
        set("Acceleraton", &mut c.acceleration);
        if let Some(h) = f("JumpHeight").filter(|h| *h > 0.0) {
            c.jump_vy = (2.0 * GRAVITY * h).sqrt();
        }
        set("JumpForwardSpeedFactor", &mut c.jump_forward_factor);
        set("JumpStrafeSpeedFactor", &mut c.jump_strafe_factor);
        set("RollSpeedFactor", &mut c.roll_speed_factor);
        set("SprintAccelerateTime", &mut c.sprint_accelerate_time);
        set("EnergyBar", &mut c.energy_bar);
        set("EnergyRestore", &mut c.energy_restore);
        set("EnergyRestoreIdle", &mut c.energy_restore_idle);
        set("EnergyDrainSprint", &mut c.energy_drain_sprint);
        set("EnergyCostJump", &mut c.energy_cost_jump);
        set("EnergyCostJumpSprint", &mut c.energy_cost_jump_sprint);
        set("EnergyCostRoll", &mut c.energy_cost_roll);
        // The jet (card JETP, re-jetpack.md §2.1; BF2 setter FUN_00252bb0,
        // BF1 FUN_0020f2e0). `JetDuration` is read by neither game.
        set("JetJump", &mut c.jet_jump);
        set("JetPush", &mut c.jet_push);
        set("JetAcceleration", &mut c.jet_acceleration);
        set("JetFuelRechargeRate", &mut c.jet_fuel_recharge);
        set("JetFuelCost", &mut c.jet_fuel_cost);
        set("JetFuelInitialCost", &mut c.jet_fuel_initial);
        set("JetFuelMinBorder", &mut c.jet_fuel_min_border);
        set("JetEnergyDrain", &mut c.jet_energy_drain);
        // Any other JetType value is ignored (B2:171303-171311).
        c.jet_hover = src
            .prop("JetType")
            .is_some_and(|t| t.trim().eq_ignore_ascii_case("hover"));
        c
    }

    /// One `ControlSpeed` value, `"<stance> fwd strafe turn"` (BF2 only):
    /// the stance's name and its three factors.
    pub fn control_row(value: &str) -> Option<(&str, [f32; 3])> {
        let mut it = value.split_whitespace();
        let name = it.next()?;
        let mut row = [0.0; 3];
        for v in &mut row {
            *v = it.next()?.parse().ok()?;
        }
        Some((name, row))
    }

    /// A class has a jet when its fuel recharges: the PS2's own test for the
    /// HUD's fuel bar and the bots' jet behaviours (re-jetpack.md §9, §10).
    /// The Jedi's double jump is not one; it pays with the energy bar.
    pub fn has_jet(&self) -> bool {
        self.jet_fuel_recharge > 0.0
    }
}

/// What a soldier's update takes from beyond its class (card JETP).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirEnv {
    /// The mission's fly-height ceiling, world y ([`DEFAULT_FLY_CEILING`]
    /// when the script sets none; [`Soldier::fly_ceiling`]).
    pub ceiling: f32,
    /// A human's soldier: the jump button's taps in the air start and end a
    /// jet, and the jets turn by the class's jet row (B2:160704-160706). The
    /// PS2's bots do not press; they call the jet themselves (re-jetpack.md
    /// §3.1 "Bots"), which the port does not build yet (card JETP2).
    pub human: bool,
}

impl Default for AirEnv {
    fn default() -> Self {
        Self {
            ceiling: DEFAULT_FLY_CEILING,
            human: true,
        }
    }
}

/// The soldiers' fly-height ceiling when the mission sets none: BF2
/// `FUN_002a4030`'s +200 (re-jetpack.md §5.3, re-boundary.md T3b).
pub const DEFAULT_FLY_CEILING: f32 = 200.0;
/// The ceiling's spring: within this many metres of it the feet are pulled
/// toward this far under it, at [`FLY_CEILING_PULL`] per second per metre
/// (BF2 B2:160726-160730, BF1 B1:117678-117681).
pub const FLY_CEILING_BAND: f32 = 5.0;
pub const FLY_CEILING_PULL: f32 = 10.0;
/// The jet's kick (both games, B2 0x247750-0x247874): `normalize(R x strafe x
/// this + F x max(fwd, [`JET_KICK_BACK`]) + (0, [`JET_KICK_LIFT`], 0))`.
pub const JET_KICK_STRAFE: f32 = 0.1;
pub const JET_KICK_BACK: f32 = -0.1;
pub const JET_KICK_LIFT: f32 = 1.1;
/// The hover's damping, v x exp(-this x dt) (B2 0x24893c), and the lift its
/// flight axis takes on the aim's height before doubling it (0x248958).
pub const HOVER_DAMP: f32 = 2.0;
pub const HOVER_LIFT: f32 = 0.08;
/// A class with a jet spawns with this much fuel (B2:149862-149867).
pub const JET_SPAWN_FUEL: f32 = 0.5;

/// Gravity, both games' soldier integrators (BF1 FUN_002098b8 / BF2
/// FUN_00248560): -18 m/s^2.
pub const GRAVITY: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    Stand,
    Crouch,
    Prone,
    Sprint,
    Jump,
    Roll,
    Fall,
    /// A blast's push threw him (card KNOCK, `analysis/re-knockback.md` §2:
    /// BF1 state 5, BF2 9): ballistic, no fire, the jump's stick.
    Thrown,
    /// Landed fast from a throw (BF1 8, BF2 10): braking on the ground.
    Tumble,
    /// Coming to rest after a throw (BF1 9, BF2 11), then standing locked.
    GetUp,
    /// A corpse's body (BF1 state 3, BF2 0xf): it slides, is pushed and
    /// kicked by later blasts, and leaves the world after [`CORPSE_SECS`].
    Dead,
    /// The jump-jet (BF2 state 6; card JETP, re-jetpack.md §4.1): the dark
    /// trooper's leap. Fuel drains, `JetPush` lifts, the stick accelerates
    /// at `JetAcceleration`, gravity and the air drag act.
    Jet,
    /// The hover jet (BF2 state 7, re-jetpack.md §4.2): jet troopers, the
    /// jet-pack heroes. No gravity; flies along the aim.
    Hover,
}

impl Stance {
    fn control(self) -> [f32; 3] {
        CONTROL_SPEED[match self {
            Stance::Stand => 0,
            Stance::Crouch => 1,
            Stance::Prone => 2,
            Stance::Sprint => 3,
            // BF1's thrown state shares the jump / fall case (B1:117557).
            Stance::Jump | Stance::Fall | Stance::Thrown => 4,
            Stance::Roll => 5,
            Stance::Jet | Stance::Hover => 6,
            Stance::Tumble | Stance::GetUp | Stance::Dead => 7,
        }]
    }

    /// One of the two jet states (card JETP).
    pub fn jetting(self) -> bool {
        matches!(self, Stance::Jet | Stance::Hover)
    }

    /// Off the floor by the stance's own nature: the jump, the fall, a throw
    /// and the jets (the wall pass's air step, `sim/step.rs`).
    pub fn airborne(self) -> bool {
        matches!(
            self,
            Stance::Jump | Stance::Fall | Stance::Thrown | Stance::Jet | Stance::Hover
        )
    }

    /// The states a blast's push skips (with the two mounted ones, which
    /// the port tests by the seat): B1 FUN_002009a0 B1:112599, B2
    /// FUN_00238b80 (§1, §4).
    pub fn knocked(self) -> bool {
        matches!(self, Stance::Thrown | Stance::Tumble | Stance::GetUp)
    }
}

#[derive(Debug, Clone)]
pub struct Soldier {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Radians; positive yaw turns left (render::Camera convention).
    pub yaw: f32,
    pub pitch: f32,
    pub stance: Stance,
    pub time_in_stance: f32,
    /// Seconds upright (stand or sprint, which is ours and one stance to
    /// BF1): BF1's stand timer (s+0xbec), which gates its fall and is
    /// put at 5 s by a floor contact
    /// that ends an airborne spell (FUN_00205cc8 B1:117388-117414).
    pub stand_age: f32,
    /// Blocks stance/jump/roll commands while > 0. When `lock_hard` is also
    /// set (prone transitions, BF1 semantics), movement + turn inputs are
    /// zeroed for the duration too.
    pub lock_timer: f32,
    pub lock_hard: bool,
    pub energy: f32,
    /// BF2 depleted latch (soldier+0xab8): set when the pool empties during
    /// sprint; sprint can't re-engage until it clears. Recovery threshold is
    /// PROVISIONAL (25.0) — the decomp confirms the flag, not the clear rule
    /// (re-stance-roll.md §4.2); pin via PCSX2.
    pub energy_depleted: bool,
    pub grounded: bool,
    /// The last floor contact's normal; up when airborne (BF2 s+0x4b0, BF1
    /// s+0x960 — re-soldier-ground.md §1.1).
    pub ground_n: Vec3,
    /// Seconds since the last floor contact (the PS2 counts them on the stance
    /// timer, §1.6a).
    pub air_time: f32,
    /// What is left of the overlap the PS2's push leaves with the floor,
    /// metres along its normal ([`Soldier::resolve_ground`]; card SKIRT,
    /// re-soldier-ground.md §7.2 rule 4).
    pub floor_overlap: f32,
    /// Last tick's contact kept the flatter floor's normal over a steeper face
    /// it met ([`Soldier::resolve_ground`], the flattest normal; card SKIRT).
    pub kept_flat: bool,
    /// The draw a throw's landing compares with [`KNOCK_TUMBLE_DRAW`] (card
    /// KNOCK; the PS2 draws its LCG at the landing, the port at the push).
    pub knock_draw: f32,
    knock_rules: GroundRules,
    /// The get-up's lock also holds the trigger (§2: the PS2 clears the
    /// trigger bytes while S+0xc0c runs).
    pub fire_lock: bool,
    /// Jet fuel, 0..1 (card JETP; BF2 s+0xb90, BF1 s+0xc6c): spawned at
    /// [`Soldier::spawn_fuel`], recharged outside the jet stances, spent by
    /// a jet's start and its seconds (re-jetpack.md §5.1).
    pub fuel: f32,
    pub jump_animation: Option<u8>,
    pub jump_animation_serial: u32,
    /// The previous tick's pad word — the EDGE source for every contextual
    /// button, and (Phase D) for the squad commands `Sim::step` phase 0b reads.
    /// NOT hashed: it is a copy of a pad the input log already determines, so a
    /// replay reproduces it. A rollback netcode would have to carry it
    /// (PLAN §Determinism — not this phase).
    prev_buttons: u32,
}

/// Which game's ground rules a soldier moves by (re-soldier-ground.md §1.1,
/// §1.6a). The two integrators share the plane-following and the contact rule;
/// they differ on slopes and in the air:
///
/// | | BF1 `FUN_002098b8` | BF2 `FUN_002480a0` |
/// |---|---|---|
/// | stick on a slope | x \|N.y\| | an uphill target x (1 - uphill^2/\|target\|^2) |
/// | slope pull | the in-plane part of 0.4*Acceleraton, down | the clamp window shifted by 18*dt*(-T.y) |
/// | steep floor | x0.01 under N.y 0.75 | x0.1 under N.y 0.7 |
/// | airborne, standing | x0.2 | full |
/// | stand -> fall | 0.7 s (1.0 s for a stance >= 5 s old) | 1.0 s |
/// | jump / fall, the stick | to 0.2 x the walk speeds, never braking | + 0.03 x Acceleration, under a drag |
///
/// PROVISIONAL (BF1): the port keeps BF2's approach-the-target shape and
/// normalised axes for both games; BF1's x1.2 acceleration, its one-sided
/// approach with a separate brake and its un-normalised axes are not ported,
/// and its fall timer is code-only (no BF1 PCSX2 trace yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroundRules {
    Bf1,
    #[default]
    Bf2,
}

impl GroundRules {
    /// By the manifest's `source_game`, as [`crate::sim::omni_radius_for`].
    pub fn for_game(source_game: &str) -> Self {
        if source_game.eq_ignore_ascii_case("bf1") {
            GroundRules::Bf1
        } else {
            GroundRules::Bf2
        }
    }

    /// The floor normal under which the stick loses its hold (BF1 41.4 deg,
    /// BF2 45.6 deg): a floor steeper than this is one a soldier slides down
    /// and cannot climb, though up to [`FLOOR_MIN_NY`] he stands on it.
    pub fn climb_min_ny(self) -> f32 {
        match self {
            GroundRules::Bf1 => 0.75,
            GroundRules::Bf2 => 0.7,
        }
    }

    /// Acceleration factor on a floor of normal `ny`.
    fn slope_control(self, ny: f32) -> f32 {
        match self {
            _ if ny >= self.climb_min_ny() => 1.0,
            GroundRules::Bf1 => 0.01,
            GroundRules::Bf2 => 0.1,
        }
    }

    /// Acceleration factor of a standing stance with no floor contact.
    fn air_control(self) -> f32 {
        match self {
            GroundRules::Bf1 => 0.2,
            GroundRules::Bf2 => 1.0,
        }
    }

    /// Gravity in the thrown and get-up states ([`BF1_THROWN_GRAVITY`]).
    fn thrown_gravity(self) -> f32 {
        match self {
            GroundRules::Bf1 => BF1_THROWN_GRAVITY,
            GroundRules::Bf2 => GRAVITY,
        }
    }

    /// Airtime after which stand/sprint switch to Fall; `stance_age` = the
    /// stance's age when the soldier left the floor.
    fn fall_after(self, stance_age: f32) -> f32 {
        match self {
            GroundRules::Bf1 if stance_age < 5.0 => 0.7,
            _ => 1.0,
        }
    }
}

/// A floor contact needs a normal steeper than this (slopes < 53.13 deg): BF1
/// DAT_0051788c, BF2 literal in FUN_0023cfd0.
pub const FLOOR_MIN_NY: f32 = 0.6;
/// ... and an approach speed dot(v, n) under this.
pub const FLOOR_MAX_APPROACH: f32 = 0.1;
/// The PS2's push-out keeps up to 2 cm of overlap (BF1 0x203c48, BF2
/// 0x23e11c), so a fold that falls away by less keeps the contact. The port's
/// point foot is set ON the floor, so it gets the same 2 cm above it.
pub const FLOOR_CONTACT_GAP: f32 = 0.02;
/// After a floor contact the PS2 zeroes a horizontal velocity part under this
/// (m/s, each world axis; v.y is not touched): BF1 B1:114883-114888, BF2
/// B2:156366-156373 (re-soldier-ground.md §7.2 rule 3).
pub const FLOOR_DEAD_ZONE: f32 = 0.1;
/// With no overlap left, a move off a face steeper than the climb limit over
/// this (m in a tick) loses the floor for that tick (card SKIRT; the PS2's
/// sphere finds no contact; re-soldier-ground.md §7.4 — the 1e-5 m is the
/// prototype's, PROVISIONAL).
pub const FLOOR_LOSS_MOVE: f32 = 1e-5;
/// PROVISIONAL (ours): a soldier held on a face too steep to stand on whose
/// feet moved less than this in height over the tick is WEDGED — against a
/// wall, or in the fold of two steep faces — and stands. The PS2's post hook
/// stands a soldier whose steep contacts average to a floor (BF1 FUN_00203e70,
/// BF2 FUN_0023e380, re-soldier-ground.md §1.4); the port's point foot has one
/// contact, so it asks whether his feet changed height over the tick — which
/// a soldier at rest on an OPEN steep face, or walking along its contour,
/// passes too (he stands where the PS2's slides: card STHOP2). Without it he
/// hangs there in Fall with 3 % control for good (the leak census: 1,410 runs
/// ended airborne, 67 before, 193 with).
pub const WEDGE_MAX_DY: f32 = 0.005;

/// BF2's air drag (FUN_00248560: v *= exp(-0.012 * |v| * dt), every airborne
/// update of the jump / fall states; PCSX2: 9.0986 -> 9.0546 m/s in one tick
/// at |v| 12.12).
pub const AIR_DRAG: f32 = 0.012;
/// BF1's stick factor in the jump / fall states (FUN_00205cc8 cases 4-6 hand
/// FUN_002098b8 `fwd * 0.2, strafe * 0.2`; PCSX2: a standing jump reaches
/// 1.3003 m/s = 0.2 x MaxSpeed 6.5, a strafe 0.9003 = 0.2 x 4.5)...
pub const BF1_AIR_SPEED: f32 = 0.2;
/// ...and its acceleration: Acceleraton x 1.2, x 0.2 once the grounded bit is
/// clear (re-soldier-ground.md §1.1; PCSX2: 2.8031 m/s on the take-off
/// update, 0.5606 m/s on every later one, at Acceleraton 70).
pub const BF1_ACCEL: f32 = 1.2;
pub const BF1_AIR_ACCEL: f32 = 0.2;
/// A jump press is taken this long after the floor is lost, in the stand and
/// the sprint stance (card JGATE; BF2 FUN_00240bf8 B2:160304-160310 / 160530,
/// BF1 FUN_00205cc8 B1:117388-117414 for a stance 5 s old; re-stance-roll.md
/// §4.3c). The PS2's timer is 0 on the first update off the floor, so 9
/// updates take the press and the 10th does not.
pub const JUMP_OFF_FLOOR: f32 = 0.3;
/// A crouch or a prone stands up on the jump button this long into it (the
/// crouch: both games, BF2 B2:160394-160397; the prone: BF1's prone case,
/// `jump && t > 0.5`, re-stance-roll.md §3.2).
pub const CROUCH_RISE_WAIT: f32 = 0.5;
/// A stationary ROLL_STANCE tap is taken this long into a stand or a prone
/// (the port's stance cycle since card D13; a prone is held longer by its
/// own lock, `prone_down_lock`).
pub const STANCE_TAP_WAIT: f32 = 0.5;
/// `[ours]` The prone double tap (the maintainer, 2026-10-28, both games): a crouch tap
/// this long or less after the tap that crouched the soldier goes prone; a
/// later one stands him up. The crouch is entered only by that tap and is
/// instant, so the window is the crouch's own `time_in_stance`.
pub const PRONE_DOUBLE_TAP: f32 = 1.0;

/// A sprint starts over this share of MaxSpeed (BF2 B2:160328, the 0.9 at
/// the compare; [`Soldier::sprint_start_speed`]).
const SPRINT_START_SHARE: f32 = 0.9;

/// PROVISIONAL sprint-recovery threshold for the depleted latch.
pub const ENERGY_RECOVER_THRESHOLD: f32 = 25.0;

// ── A blast's push and what follows (card KNOCK, `analysis/re-knockback.md`) ──

/// One knock-down on every map (the maintainer, 2026-10-27, FEATURE_SET "One gameplay"):
/// the push, the throw, the tumble, the get-up and its lock all follow BF1's
/// rules, the game that throws the living (BF2 pushes only the dead by
/// default, re-knockback §4). A named deviation on BF2 maps.
pub const KNOCK_RULES: GroundRules = GroundRules::Bf1;
/// Ours, PROVISIONAL (FEEL_NOTES): the disc's Push x this. Both games' Push
/// with the falloff of `Sim::kick_and_push` threw the player too far.
pub const KNOCK_PUSH_SCALE: f32 = 0.75;

/// The push's lift: `normalize(d) + (0, this, 0)`, not renormalised (B1
/// FUN_002009a0, ELF 0x3f19999a; §1).
pub const KNOCK_LIFT: f32 = 0.6;
/// BF1's apply (FUN_002008b0, §1): a new v.y over this throws him...
pub const KNOCK_THROW_VY: f32 = 2.0;
/// ...else a ground speed over this (DAT_00517874) tumbles him.
pub const KNOCK_TUMBLE_SPEED: f32 = 2.0;
/// BF2's apply (FUN_002388b8, §4): thrown when v . floor normal is over
/// this, else tumbling when |v|^2 is over [`BF2_KNOCK_TUMBLE_SQ`].
pub const BF2_KNOCK_THROW_VN: f32 = 3.0;
pub const BF2_KNOCK_TUMBLE_SQ: f32 = 64.0;
/// BF2's airborne soldier is thrown only by a push this strong (§4: "a
/// flinch below scale 8 and 5"; PROVISIONAL [H]: which state takes which,
/// and the flinch itself, are not built).
pub const BF2_KNOCK_AIR_SCALE: f32 = 5.0;
/// BF1's gravity in the thrown and get-up states (B1:119278-119299; every
/// other state has [`GRAVITY`]). BF2 throws with its air integrator: 18
/// and the drag.
pub const BF1_THROWN_GRAVITY: f32 = 13.5;
/// A throw lands only this long into it (B1:117586-117607, B2
/// B2:160614-160653), then tumbles over [`KNOCK_LAND_FAST`]^2, or over
/// [`KNOCK_LAND_SLOW`]^2 when the draw is over [`KNOCK_TUMBLE_DRAW`]; else
/// it gets up.
pub const THROWN_LAND_AFTER: f32 = 0.3;
pub const KNOCK_LAND_FAST: f32 = 6.0;
pub const KNOCK_LAND_SLOW: f32 = 3.0;
pub const KNOCK_TUMBLE_DRAW: f32 = 0.3;
/// The integrator's brake (B1:119197-119214): on a floor of normal y over
/// [`KNOCK_BRAKE_MIN_NY`] (DAT_005178ac) the forward and side speeds each
/// fall by Acceleration x dt x this (DAT_005178a4) x the state's brake.
pub const KNOCK_BRAKE_SHARE: f32 = 0.6;
pub const KNOCK_BRAKE_MIN_NY: f32 = 0.75;
/// The tumble's brake (B1:117692-117714: 8.4 m/s^2 at Acceleration 70) and
/// its end: after [`TUMBLE_MIN`] under [`TUMBLE_END_SPEED`], or at the
/// game's longest (BF1 2.5 s, BF2 0.75 s).
pub const TUMBLE_BRAKE: f32 = 0.2;
pub const TUMBLE_MIN: f32 = 0.5;
pub const TUMBLE_END_SPEED: f32 = 4.0;
pub const BF1_TUMBLE_MAX: f32 = 2.5;
pub const BF2_TUMBLE_MAX: f32 = 0.75;
/// The get-up's brake (B1:117715-117737: 16.8 m/s^2) and its end on the
/// ground: under [`GETUP_END_SPEED`] or after [`GETUP_MAX`].
pub const GETUP_BRAKE: f32 = 0.4;
pub const GETUP_END_SPEED: f32 = 0.6;
pub const GETUP_MAX: f32 = 1.0;
/// The lock after the get-up: no move, turn, fire or jump. The PS2 (BF1)
/// locks the length of `stand_getupfront`, 58 frames (B1:118536-118538;
/// [M] 1.9333 s). Ours (the maintainer 2026-10-27, FEEL_NOTES): the lock ends when the
/// stand-up stops moving. Both games' clips hold still from frame 51 of 57
/// (front) and 49 of 52 (back), so BF1's playback (`frames / 30` s) is done
/// at update 52 and BF2's (`(frames - 1) / 30`) at 51; the back clip waits
/// at most 2 updates more. Half an update under 52, so the float countdown
/// lets go on update 52 and not 53.
pub const GETUP_LOCK: f32 = 51.5 / 30.0;
/// A death kicks the body this much along the hit (B1 FUN_00200f90,
/// DAT_00517878; BF2 FUN_002395a8's literal; §3 [M] 5.0 +- 0.15 m/s).
pub const DEATH_KICK: f32 = 5.0;
/// A corpse's body leaves the world this long into the dead state (B1
/// B1:117521-117555, B2 B2:160981-161039).
pub const CORPSE_SECS: f32 = 4.0;
/// BF2's corpse brake, m/s^2 at a second dead and after (min(t, 1)^2 x
/// this into FUN_002480a0's acceleration, B2:160997); BF1's is min(t, 1)^2
/// as the brake ([`KNOCK_BRAKE_SHARE`] x Acceleration: 42 m/s^2).
pub const BF2_CORPSE_BRAKE: f32 = 100.0;

impl Soldier {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            stance: Stance::Stand,
            time_in_stance: 0.0,
            stand_age: 0.0,
            lock_timer: 0.0,
            lock_hard: false,
            energy: SoldierConstants::default().energy_bar,
            energy_depleted: false,
            grounded: true,
            ground_n: Vec3::Y,
            air_time: 0.0,
            floor_overlap: FLOOR_CONTACT_GAP,
            kept_flat: false,
            knock_draw: 0.0,
            knock_rules: KNOCK_RULES,
            fire_lock: false,
            fuel: 0.0,
            jump_animation: None,
            jump_animation_serial: 0,
            prev_buttons: 0,
        }
    }

    /// The fuel a soldier of class `c` spawns with: [`JET_SPAWN_FUEL`] when
    /// it recharges, else none (both games, re-jetpack.md §5.1).
    pub fn spawn_fuel(c: &SoldierConstants) -> f32 {
        if c.has_jet() {
            JET_SPAWN_FUEL
        } else {
            0.0
        }
    }

    /// The pad word this soldier saw LAST tick: the edge source Phase D's
    /// squad-command pass compares against (`Sim::step` phase 0b).
    pub fn prev_buttons(&self) -> u32 {
        self.prev_buttons
    }

    /// A rider does not step (`Sim`'s seat pass) but still sees its pad, so
    /// the ACTION edge that seated it is not taken again next tick.
    pub(crate) fn set_prev_buttons(&mut self, b: u32) {
        self.prev_buttons = b;
    }

    /// What a stationary ROLL_STANCE tap does to a soldier in `stance` for
    /// `t` seconds (`None`: the tap is dropped). `[ours]` (the maintainer, 2026-10-28,
    /// both games): a stand crouches; a crouch goes prone when the tap is the
    /// second of a double tap ([`PRONE_DOUBLE_TAP`]) and stands up otherwise;
    /// a prone stands up. The stand and the prone take a tap after
    /// [`STANCE_TAP_WAIT`]; a crouch at once, so a quick double tap counts.
    pub fn stance_tap_target(stance: Stance, t: f32) -> Option<Stance> {
        match stance {
            Stance::Stand | Stance::Sprint if t > STANCE_TAP_WAIT => Some(Stance::Crouch),
            Stance::Crouch if t <= PRONE_DOUBLE_TAP => Some(Stance::Prone),
            Stance::Crouch => Some(Stance::Stand),
            Stance::Prone if t > STANCE_TAP_WAIT => Some(Stance::Stand),
            _ => None,
        }
    }

    /// What a stationary ROLL_STANCE press on this tick's pad would do when
    /// the soldier steps `dt` next (`None`: no edge, a lock, or a dropped
    /// tap). The bots' stance controller asks this (card D13), so it presses
    /// only where the press lands; it mirrors [`Soldier::step_looking`]'s
    /// own order: the clocks advance, then the lock and the tap are read.
    pub fn stance_tap_after(&self, dt: f32) -> Option<Stance> {
        let held = self.prev_buttons & buttons::ROLL_STANCE != 0;
        if held || (self.lock_timer - dt).max(0.0) > 0.0 {
            return None;
        }
        Self::stance_tap_target(self.stance, self.time_in_stance + dt)
    }

    /// Whether a JUMP press on this tick's pad stands a crouch or a prone up
    /// when the soldier steps `dt` next (the same mirror as
    /// [`Soldier::stance_tap_after`]).
    pub fn jump_stands_after(&self, dt: f32) -> bool {
        matches!(self.stance, Stance::Crouch | Stance::Prone)
            && self.prev_buttons & buttons::JUMP == 0
            && (self.lock_timer - dt).max(0.0) <= 0.0
            && self.time_in_stance + dt > CROUCH_RISE_WAIT
    }

    /// A commanded stance change. Prone changes carry BF1's hard anim locks
    /// (`prone_down_lock` / `prone_up_lock`); crouch <-> stand is instant
    /// (both games).
    fn change_stance(&mut self, next: Stance, c: &SoldierConstants) {
        if next == Stance::Prone {
            self.lock_timer = c.prone_down_lock;
            self.lock_hard = true;
        } else if self.stance == Stance::Prone {
            self.lock_timer = c.prone_up_lock;
            self.lock_hard = true;
        }
        self.set_stance(next);
    }

    fn set_stance(&mut self, s: Stance) {
        let upright = |s| matches!(s, Stance::Stand | Stance::Sprint);
        if !(upright(self.stance) && upright(s)) {
            self.stand_age = 0.0;
        }
        self.stance = s;
        self.time_in_stance = 0.0;
        if !s.knocked() {
            self.knock_rules = KNOCK_RULES;
        }
    }

    /// Merged jump eligibility (JUNI): BF2's floor-loss window on every map.
    /// Crouch rise, input edges, locks and energy are checked by the caller.
    pub fn jump_open(&self, dt: f32, _rules: GroundRules) -> bool {
        self.grounded || self.air_time < JUMP_OFF_FLOOR + 0.5 * dt
    }

    /// The jump's energy (BF2 FUN_00247528 -> FUN_001e2010, after the
    /// update's mover): the dearer of the two costs when the forward speed is
    /// over MaxSpeed. Only a depleted bar refuses; a bar that cannot cover the
    /// cost is emptied and marked depleted.
    fn pay_jump(&mut self, c: &SoldierConstants, fwd_axis: Vec2) -> bool {
        let fwd = Vec2::new(self.vel.x, self.vel.z).dot(fwd_axis);
        let cost = if fwd > c.max_speed + 1e-4 {
            c.energy_cost_jump.max(c.energy_cost_jump_sprint)
        } else {
            c.energy_cost_jump
        };
        self.pay_energy(cost)
    }

    /// The energy bar's take (BF2 FUN_001e2010, re-stance-roll.md §4.3d): a
    /// cost of 0 always passes; a depleted bar refuses; a bar that cannot
    /// cover the cost is emptied and marked depleted.
    pub(crate) fn pay_energy(&mut self, cost: f32) -> bool {
        if cost <= 0.0 {
            return true;
        }
        if self.energy_depleted {
            return false;
        }
        self.energy -= cost;
        if self.energy < 0.0 {
            self.energy_depleted = true;
            self.energy = 0.0;
        }
        true
    }

    /// BF2 roll predicate FUN_00247b88: stick >= 0.9 deflection, any
    /// direction except the straight-back +-30 deg cone.
    pub fn roll_predicate(fwd: f32, strafe: f32) -> bool {
        0.81 <= fwd * fwd + strafe * strafe && (0.0 < fwd || fwd * -1.732 < strafe.abs())
    }

    /// BF2 sprint predicate FUN_00247ad8. `keep` widens the cone from
    /// +-30 deg (start) to +-60 deg (stay) — the hysteresis.
    pub fn sprint_predicate(fwd: f32, strafe: f32, keep: bool) -> bool {
        let cone = if keep { 1.732 } else { 0.577 };
        0.81 <= fwd * fwd + strafe * strafe && fwd >= 0.0 && strafe.abs() < fwd * cone
    }

    /// The sprint start's speed test (card SPUP; re-stance-roll.md §4.2b).
    /// The PS2 asks `0.9 x MaxSpeed < v . the body's forward row` (BF2
    /// FUN_00240bf8 B2:160321-160329) - the facing's share of the velocity,
    /// level - and its run up a slope is MaxSpeed x cos^3, so its sprint
    /// starts up to 15.1 deg and never above (live: yes at 9.8, 13.3 and
    /// 14.8 deg, no at 16.1 and 19.3). Ours (the maintainer, 2026-10-05, FEEL_NOTES):
    /// the speed is taken along the floor and the floor is 0.9 x what the
    /// mover's slope rule allows a full stick there, so a sprint starts on
    /// any slope a soldier runs up. On a level floor this is the PS2's test.
    fn sprint_start_speed(&self, c: &SoldierConstants, rules: GroundRules) -> bool {
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let fwd = Vec3::new(-sy, 0.0, -cy);
        let n = if self.grounded {
            self.ground_n
        } else {
            Vec3::Y
        };
        let t1 = n.cross(Vec3::new(cy, 0.0, -sy)).normalize_or(fwd);
        let allowed = match rules {
            GroundRules::Bf1 => n.y.abs(),
            GroundRules::Bf2 if t1.y > 0.0 => 1.0 - t1.y * t1.y,
            GroundRules::Bf2 => 1.0,
        };
        SPRINT_START_SHARE * c.max_speed * allowed < self.vel.dot(t1)
    }

    /// One fixed tick: aim, stance, velocity and the move. The floor is the
    /// caller's next call, [`Soldier::resolve_ground`], with the floor under
    /// the feet this leaves (after the wall pass).
    pub fn step(&mut self, dt: f32, pad: &PadState, c: &SoldierConstants, rules: GroundRules) {
        self.step_looking(dt, pad, c, rules, 1.0, AirEnv::default());
    }

    pub fn look_frozen(&self) -> bool {
        self.lock_hard && self.lock_timer > 0.0
    }

    pub fn pitch_limits(&self, c: &SoldierConstants) -> (f32, f32) {
        if self.stance == Stance::Prone {
            (-0.4, 0.4)
        } else {
            c.pitch_limits
        }
    }

    pub fn mouse_scale(&self) -> f32 {
        match self.stance {
            Stance::Tumble | Stance::GetUp | Stance::Dead => self.stance.control()[2],
            _ => 1.0,
        }
    }

    /// The aim after one update of the look stick `(turn, pitch)`: the
    /// per-stance turn factor (a human's jet factor while jetting) and the
    /// pitch limits (BF1). `c` carries the zoom's divided rates.
    fn turned(
        &self,
        dt: f32,
        (turn_in, pitch_in): (f32, f32),
        c: &SoldierConstants,
        human: bool,
    ) -> (f32, f32) {
        let turn_k = if self.stance.jetting() && human {
            c.jet_control[2]
        } else {
            self.stance.control()[2]
        };
        let (lo, hi) = self.pitch_limits(c);
        (
            self.yaw - turn_in * c.max_turn_speed * turn_k * dt,
            (self.pitch + pitch_in * c.max_pitch_speed * turn_k * dt).clamp(lo, hi),
        )
    }

    /// The aim [`Soldier::step_looking`] turns to on its next update of
    /// `pad`'s look stick (the window's controller preview,
    /// `Sim::look_preview`): no turn under a hard lock that outlasts it.
    pub fn stick_turned(
        &self,
        dt: f32,
        pad: &PadState,
        c: &SoldierConstants,
        human: bool,
    ) -> (f32, f32) {
        let frozen = self.lock_hard && self.lock_timer - dt > 0.0;
        let look = if frozen {
            (0.0, 0.0)
        } else {
            (PadState::axis(pad.rx), PadState::axis(pad.ry))
        };
        self.turned(dt, look, c, human)
    }

    pub fn mouse_turned(&self, look: (f32, f32), scale: f32, c: &SoldierConstants) -> (f32, f32) {
        let k = scale * self.mouse_scale();
        let (lo, hi) = self.pitch_limits(c);
        (
            self.yaw - look.0 * k,
            (self.pitch + look.1 * k).clamp(lo, hi),
        )
    }

    pub fn step_looking(
        &mut self,
        dt: f32,
        pad: &PadState,
        c: &SoldierConstants,
        rules: GroundRules,
        look_scale: f32,
        env: AirEnv,
    ) {
        let fwd_in = PadState::axis(pad.ly);
        let strafe_in = PadState::axis(pad.lx);
        let turn_in = PadState::axis(pad.rx);
        let pitch_in = PadState::axis(pad.ry);
        let roll_stance_edge = pad.buttons & buttons::ROLL_STANCE != 0
            && self.prev_buttons & buttons::ROLL_STANCE == 0;
        let jump_edge = pad.buttons & buttons::JUMP != 0 && self.prev_buttons & buttons::JUMP == 0;

        self.time_in_stance += dt;
        self.stand_age += dt;
        self.lock_timer = (self.lock_timer - dt).max(0.0);
        if self.lock_timer <= 0.0 {
            self.lock_hard = false;
            self.fire_lock = false;
        }
        // Jet fuel (card JETP): BF2 recharges it outside the jet stances,
        // before the stance switch (B2:160108-160116), on every map (BF1
        // recharged it in them too).
        if !self.stance.jetting() {
            self.fuel = (self.fuel + c.jet_fuel_recharge * dt).min(1.0);
        }
        // BF1 hard lock (FUN_00205cc8 t<10 band): movement + turn frozen.
        let frozen = self.look_frozen();
        let (fwd_in, strafe_in, turn_in, pitch_in) = if frozen {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (fwd_in, strafe_in, turn_in, pitch_in)
        };

        // ── Aim (per-stance turn factor; prone pitch clamp — BF1) ─────────
        // The hover flies along the aim the PS2 wrote LAST update (its aim
        // update runs after the movement, re-jetpack.md §4.2 step 3).
        let aim0 = (self.yaw, self.pitch);
        (self.yaw, self.pitch) = self.turned(dt, (turn_in, pitch_in), c, env.human);
        if pad.has_look() && !frozen {
            (self.yaw, self.pitch) = self.mouse_turned(pad.look(), look_scale, c);
        }

        // ── Stance transitions ────────────────────────────────────────────
        let can_command = self.lock_timer <= 0.0;
        // The update after a take-off from the floor: the PS2's floor bit is
        // still set (no gravity, no landing — [`Soldier::take_off`]).
        let first_air =
            self.stance == Stance::Jump && self.time_in_stance <= dt && self.air_time <= dt;
        let mut jump = false;
        match self.stance {
            Stance::Roll => {
                if self.time_in_stance > c.roll_duration {
                    // openbf/BF2: roll always exits to stand + soft lock.
                    self.set_stance(Stance::Stand);
                    self.lock_timer = c.soft_lock;
                }
            }
            // A throw and its tail take no command; they end in the movement
            // below, after the update's integration, as on the PS2 (card
            // KNOCK: the landing row keeps the flight's speed).
            Stance::Thrown | Stance::Tumble | Stance::GetUp | Stance::Dead => {}
            Stance::Jump | Stance::Fall => {
                if self.grounded && !first_air {
                    self.set_stance(Stance::Stand);
                    self.lock_timer = c.soft_lock;
                }
            }
            // A jet lands on the floor with the light lock, whatever its
            // airtime (BF2 FUN_00246850 from 6 / 7, re-jetpack.md §4.4).
            Stance::Jet | Stance::Hover => {
                if self.grounded {
                    self.set_stance(Stance::Stand);
                    self.lock_timer = c.soft_lock;
                }
            }
            Stance::Stand | Stance::Crouch | Stance::Prone | Stance::Sprint => {
                if can_command && roll_stance_edge {
                    if Self::roll_predicate(fwd_in, strafe_in)
                        && self.stance != Stance::Prone
                        && c.roll_speed_factor > 0.0
                        && self.energy >= c.energy_cost_roll
                    {
                        self.energy -= c.energy_cost_roll;
                        self.set_stance(Stance::Roll);
                        // BF2 FUN_002471b0: scale planar velocity to exactly
                        // RollSpeedFactor*MaxSpeed when speed in (0.1, target).
                        let target = c.roll_speed_factor * c.max_speed;
                        let planar = Vec2::new(self.vel.x, self.vel.z);
                        let sp2 = planar.length_squared();
                        if 0.01 < sp2 && sp2 < target * target {
                            let s = target / sp2.sqrt();
                            self.vel.x *= s;
                            self.vel.z *= s;
                        }
                    } else if let Some(next) = (!Self::roll_predicate(fwd_in, strafe_in))
                        .then(|| Self::stance_tap_target(self.stance, self.time_in_stance))
                        .flatten()
                    {
                        // Stationary: [`Soldier::stance_tap_target`].
                        self.change_stance(next, c);
                    }
                }
                // Sprint enter/exit (hold-to-sprint; BF2 predicate).
                if self.stance == Stance::Sprint {
                    if self.energy <= 0.0 {
                        self.energy_depleted = true;
                        self.set_stance(Stance::Stand);
                    } else if !pad.pressed(buttons::SPRINT)
                        || !Self::sprint_predicate(fwd_in, strafe_in, true)
                    {
                        self.set_stance(Stance::Stand);
                    }
                } else if self.stance == Stance::Stand
                    && pad.pressed(buttons::SPRINT)
                    && Self::sprint_predicate(fwd_in, strafe_in, false)
                    && self.sprint_start_speed(c, rules)
                    && !self.energy_depleted
                    && self.energy > 0.0
                {
                    self.set_stance(Stance::Sprint);
                }
                // Jump: paid and applied after this update's ground move
                // ([`Soldier::pay_jump`], [`Soldier::take_off`]). A crouch
                // stands up instead (instant, no lock: both games), and so
                // does a prone (BF1's prone case; `[ours]` in BF2), through
                // the get-up's lock: neither launches a jump.
                if can_command && jump_edge {
                    match self.stance {
                        Stance::Stand | Stance::Sprint => jump = self.jump_open(dt, rules),
                        Stance::Crouch | Stance::Prone
                            if self.time_in_stance > CROUCH_RISE_WAIT =>
                        {
                            self.change_stance(Stance::Stand, c)
                        }
                        _ => {}
                    }
                }
            }
        }

        // ── Energy (BF2 pool; restore idle vs moving) ─────────────────────
        if let Some(rate) = c.energy_override {
            if rate < 0.0 {
                self.pay_energy(-rate * dt);
            } else {
                self.energy += rate * dt;
            }
        } else if self.stance == Stance::Sprint {
            self.energy -= c.energy_drain_sprint * dt;
        } else if self.stance.jetting() {
            // BF2's rate is 0 from stance 4 up (re-stance-roll.md §4.3d): a
            // hovering Vader neither regains nor spends energy (card JETP).
        } else {
            let idle = fwd_in == 0.0 && strafe_in == 0.0;
            let rate = if idle {
                c.energy_restore_idle
            } else {
                c.energy_restore
            };
            if rate < 0.0 {
                self.pay_energy(-rate * dt);
            } else {
                self.energy += rate * dt;
            }
        }
        self.energy = self.energy.clamp(0.0, c.energy_bar);
        if self.energy_depleted && self.energy >= ENERGY_RECOVER_THRESHOLD {
            self.energy_depleted = false;
        }

        // ── Movement ──────────────────────────────────────────────────────
        let mut ctl = self.stance.control();
        if self.stance == Stance::Sprint {
            ctl[..2].copy_from_slice(&c.sprint_control);
        }
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let fwd_axis = Vec2::new(-sy, -cy);
        let right_axis = Vec2::new(cy, -sy);

        // Backpedal is capped by MaxStrafeSpeed in both games (stand case:
        // `fwd > 0 ? MaxSpeed : MaxStrafeSpeed`).
        let fwd_cap = if fwd_in > 0.0 {
            if self.stance == Stance::Sprint {
                // Ramp MaxSpeed -> MaxSpeed*ControlSpeed[sprint].fwd over
                // SprintAccelerateTime (BF2 FUN_00240bf8 case 3).
                let t = (self.time_in_stance / c.sprint_accelerate_time).min(1.0);
                c.max_speed * (1.0 + (ctl[0] - 1.0) * t)
            } else {
                c.max_speed * ctl[0]
            }
        } else {
            c.max_strafe_speed * ctl[0]
        };
        let mut target_fwd = fwd_in * fwd_cap;
        let mut target_strafe = strafe_in * c.max_strafe_speed * ctl[1];
        // Native cases 0..3 blend desired locomotion before acceleration.
        // Roll and the jet states keep their own movers.
        if (rules == GroundRules::Bf2 || c.saber)
            && matches!(
                self.stance,
                Stance::Stand | Stance::Crouch | Stance::Prone | Stance::Sprint
            )
        {
            if let Some(motion) = c.motion_target.filter(|m| m.weight > 0.0) {
                if motion.weight >= 1.0 {
                    target_fwd = motion.forward;
                    target_strafe = motion.strafe;
                } else {
                    target_fwd += (motion.forward - target_fwd) * motion.weight;
                    target_strafe += (motion.strafe - target_strafe) * motion.weight;
                }
            }
        }

        // The axes lie in the plane of the last floor contact (BF2
        // FUN_002480a0: T1 = R x N, T2 = N x T1; mirrored for the port's
        // handedness: T1 = N x R, T2 = T1 x N). Airborne, N is up. A grounded
        // soldier loses the into-N part of his velocity.
        let bf1 = rules == GroundRules::Bf1;
        let n = if self.grounded {
            self.ground_n
        } else {
            Vec3::Y
        };
        let r3 = Vec3::new(right_axis.x, 0.0, right_axis.y);
        let t1 = n
            .cross(r3)
            .normalize_or(Vec3::new(fwd_axis.x, 0.0, fwd_axis.y));
        let t2 = t1.cross(n).normalize_or(r3);
        if self.grounded {
            if bf1 && !(self.stance.knocked() && self.knock_rules == GroundRules::Bf2) {
                // The in-plane part of (0, -0.4*Acceleraton*dt, 0).
                let g = -0.4 * c.acceleration * dt;
                self.vel += Vec3::new(0.0, g, 0.0) - n * (g * n.y);
            }
            let vn = self.vel.dot(n);
            if vn < 0.0 {
                self.vel -= n * vn;
            }
        }
        if self.stance == Stance::Thrown {
            // The jump's stick (BF1 cases 4-6; BF2's air integrator), with
            // no gravity while the floor bit is still set: the update after
            // the push moves him straight off it (§2 [M]).
            let grounded = self.grounded;
            self.air_steer(
                dt,
                fwd_in,
                strafe_in,
                (t1, t2),
                c,
                self.knock_rules,
                grounded,
            );
            if !grounded {
                self.vel.y -= self.knock_rules.thrown_gravity() * dt;
            }
            self.pos += self.vel * dt;
            self.fly_ceiling(dt, env.ceiling);
            if grounded && self.time_in_stance > THROWN_LAND_AFTER {
                let sp2 = self.vel.length_squared();
                let tumble = sp2 > KNOCK_LAND_FAST * KNOCK_LAND_FAST
                    || (sp2 > KNOCK_LAND_SLOW * KNOCK_LAND_SLOW
                        && self.knock_draw > KNOCK_TUMBLE_DRAW);
                self.set_stance(if tumble {
                    Stance::Tumble
                } else {
                    Stance::GetUp
                });
            }
            self.prev_buttons = pad.buttons;
            return;
        }
        if matches!(self.stance, Stance::Tumble | Stance::GetUp | Stance::Dead) {
            self.knock_slide(dt, (t1, t2), n, c, self.knock_rules);
            self.prev_buttons = pad.buttons;
            return;
        }
        if self.stance.jetting() {
            let tap = env.human && jump_edge;
            self.step_jet(dt, (fwd_in, strafe_in), tap, aim0, c, env.ceiling);
            self.prev_buttons = pad.buttons;
            return;
        }
        if matches!(self.stance, Stance::Jump | Stance::Fall) {
            // JUNI: Jump/Fall use BF2 on every map; Thrown above keeps
            // its original per-game integrator and gravity.
            self.air_steer(
                dt,
                fwd_in,
                strafe_in,
                (t1, t2),
                c,
                GroundRules::Bf2,
                first_air,
            );
            if !first_air {
                self.vel.y -= GRAVITY * dt;
            }
            self.pos += self.vel * dt;
            // Then the ceiling and a second tap: the jet (card JETP; B2
            // FUN_00240bf8 cases 4 / 8, B2:160614-160618), its kick added to
            // the velocity the NEXT move takes, as the jump's is.
            self.fly_ceiling(dt, env.ceiling);
            if env.human && jump_edge {
                self.jet_on(c, (fwd_in, strafe_in));
            }
            self.prev_buttons = pad.buttons;
            return;
        }
        let cur_fwd = self.vel.dot(t1);
        let cur_strafe = self.vel.dot(t2);
        // A roll steers with a tiny factor but keeps momentum; the standing
        // stances approach the target at Acceleration — less on a steep
        // floor, and in the air by the game's rule.
        let accel = c.acceleration
            * match self.stance {
                Stance::Roll => ctl[0].max(ctl[1]), // 0.02
                _ if self.grounded => rules.slope_control(n.y),
                _ => rules.air_control(),
            }
            * dt;
        let (mut target_fwd, mut target_strafe) = (target_fwd, target_strafe);
        let (mut s1, mut s2) = (0.0, 0.0);
        if self.grounded {
            if bf1 {
                target_fwd *= n.y.abs();
                target_strafe *= n.y.abs();
            } else {
                // The slope's gravity shifts the clamp window (DAT_005fd0ac = 1).
                s1 = GRAVITY * dt * -t1.y;
                s2 = GRAVITY * dt * -t2.y;
                let up = target_fwd * t1.y + target_strafe * t2.y;
                let t2sum = target_fwd * target_fwd + target_strafe * target_strafe;
                if up > 0.0 && t2sum > 0.0 {
                    let k = 1.0 - up * up / t2sum;
                    target_fwd *= k;
                    target_strafe *= k;
                }
            }
        }
        let step_to = |cur: f32, tgt: f32, slide: f32| -> f32 {
            cur + (tgt - cur).clamp(slide - accel, slide + accel)
        };
        let new_fwd = step_to(cur_fwd, target_fwd, s1);
        let new_strafe = step_to(cur_strafe, target_strafe, s2);
        self.vel += t1 * (new_fwd - cur_fwd) + t2 * (new_strafe - cur_strafe);

        // ── Gravity + move (contact: `resolve_ground`, after the move) ────
        if !self.grounded {
            self.vel.y -= GRAVITY * dt;
        }
        self.pos += self.vel * dt;
        if jump && self.pay_jump(c, fwd_axis) {
            self.take_off(c);
        }

        self.prev_buttons = pad.buttons;
    }

    /// The tumble, the get-up and a corpse (card KNOCK, §2-§4): no stick; on
    /// the floor the forward and side speeds each brake toward 0 by the
    /// state's rate ([`KNOCK_BRAKE_SHARE`] x Acceleration x the brake; BF1
    /// only on a floor of normal y over [`KNOCK_BRAKE_MIN_NY`], BF2 a tenth
    /// of it under its climb limit), in the air gravity (BF1's get-up
    /// [`BF1_THROWN_GRAVITY`]); then the state's end.
    fn knock_slide(
        &mut self,
        dt: f32,
        (t1, t2): (Vec3, Vec3),
        n: Vec3,
        c: &SoldierConstants,
        rules: GroundRules,
    ) {
        let bf1 = rules == GroundRules::Bf1;
        let t = self.time_in_stance;
        let decel = match self.stance {
            Stance::Tumble => c.acceleration * KNOCK_BRAKE_SHARE * TUMBLE_BRAKE,
            Stance::GetUp => c.acceleration * KNOCK_BRAKE_SHARE * GETUP_BRAKE,
            _ => {
                let k = t.min(1.0) * t.min(1.0);
                if bf1 {
                    c.acceleration * KNOCK_BRAKE_SHARE * k
                } else {
                    BF2_CORPSE_BRAKE * k
                }
            }
        };
        if self.grounded {
            let floor = match rules {
                GroundRules::Bf1 if n.y > KNOCK_BRAKE_MIN_NY => 1.0,
                GroundRules::Bf1 => 0.0,
                GroundRules::Bf2 => rules.slope_control(n.y),
            };
            let a = decel * floor * dt;
            let (f, s) = (self.vel.dot(t1), self.vel.dot(t2));
            let (nf, ns) = (f - f.clamp(-a, a), s - s.clamp(-a, a));
            self.vel += t1 * (nf - f) + t2 * (ns - s);
        } else {
            let g = if self.stance == Stance::GetUp {
                rules.thrown_gravity()
            } else {
                GRAVITY
            };
            self.vel.y -= g * dt;
        }
        self.pos += self.vel * dt;
        let sp2 = self.vel.length_squared();
        match self.stance {
            Stance::Tumble => {
                let max = if bf1 { BF1_TUMBLE_MAX } else { BF2_TUMBLE_MAX };
                if t > TUMBLE_MIN && (sp2 < TUMBLE_END_SPEED * TUMBLE_END_SPEED || t >= max) {
                    self.set_stance(Stance::GetUp);
                }
            }
            Stance::GetUp
                if self.grounded && (sp2 < GETUP_END_SPEED * GETUP_END_SPEED || t > GETUP_MAX) =>
            {
                self.set_stance(Stance::Stand);
                self.lock_timer = GETUP_LOCK;
                self.lock_hard = true;
                self.fire_lock = true;
            }
            _ => {}
        }
    }

    /// A blast's push (card KNOCK, §1 / §4): `dir` = normalize(d) + (0,
    /// [`KNOCK_LIFT`], 0), `scale` = Push x the falloff; `draw` is the
    /// landing's [`KNOCK_TUMBLE_DRAW`] roll. The caller has tested the
    /// skipped states ([`Stance::knocked`], a seat).
    /// - **BF1** (FUN_002008b0): v += dir x scale; a v.y over
    ///   [`KNOCK_THROW_VY`] throws him, else a ground speed over
    ///   [`KNOCK_TUMBLE_SPEED`] tumbles him.
    /// - **BF2** (FUN_002388b8): half the velocity against `dir` goes first;
    ///   thrown in the air by a push of [`BF2_KNOCK_AIR_SCALE`] or more, on
    ///   the floor when v . its normal is over [`BF2_KNOCK_THROW_VN`], else
    ///   tumbling over [`BF2_KNOCK_TUMBLE_SQ`] (its flinch is not built).
    ///
    /// The state setter refuses a corpse ([`Stance::Dead`]): it keeps the
    /// velocity and its state (B1 FUN_00209758).
    pub fn knock(&mut self, dir: Vec3, scale: f32, rules: GroundRules, draw: f32) {
        self.knock_draw = draw;
        let next = match rules {
            GroundRules::Bf1 => {
                self.vel += dir * scale;
                if self.vel.y > KNOCK_THROW_VY {
                    Some(Stance::Thrown)
                } else if self.vel.x * self.vel.x + self.vel.z * self.vel.z
                    > KNOCK_TUMBLE_SPEED * KNOCK_TUMBLE_SPEED
                {
                    Some(Stance::Tumble)
                } else {
                    None
                }
            }
            GroundRules::Bf2 => {
                let along = self.vel.dot(dir);
                if along < 0.0 {
                    self.vel -= dir * (0.5 * along);
                }
                self.vel += dir * scale;
                if self.stance == Stance::Thrown {
                    None
                } else if !self.grounded || matches!(self.stance, Stance::Jump | Stance::Fall) {
                    (scale >= BF2_KNOCK_AIR_SCALE).then_some(Stance::Thrown)
                } else if self.vel.dot(self.ground_n) > BF2_KNOCK_THROW_VN {
                    Some(Stance::Thrown)
                } else if self.vel.length_squared() > BF2_KNOCK_TUMBLE_SQ {
                    Some(Stance::Tumble)
                } else {
                    None
                }
            }
        };
        if let Some(s) = next.filter(|&s| s != self.stance && self.stance != Stance::Dead) {
            self.set_stance(s);
        }
    }

    pub fn force_knock(&mut self, dir: Vec3, scale: f32, draw: f32) {
        self.vel = Vec3::ZERO;
        self.knock(dir, scale, GroundRules::Bf2, draw);
        if self.stance.knocked() {
            self.knock_rules = GroundRules::Bf2;
        }
    }

    /// A hold takes him out of whatever he was doing, a throw's state included.
    pub fn catch_upright(&mut self) {
        if self.stance != Stance::Stand {
            self.set_stance(Stance::Stand);
        }
    }

    /// A death's kick: [`DEATH_KICK`] m/s along `dir` (§3).
    pub fn kick(&mut self, dir: Vec3) {
        self.vel += dir.normalize_or_zero() * DEATH_KICK;
    }

    /// Thrown, tumbling, getting up, dead, or in the get-up's lock: the
    /// trigger is held off (§2).
    pub fn no_fire(&self) -> bool {
        self.stance.knocked()
            || self.stance == Stance::Dead
            || (self.fire_lock && self.lock_timer > 0.0)
    }

    /// Merged take-off (JUNI, re-stance-roll.md §4.3b): BF2
    /// FUN_00246ed0 adds vertical speed, scales the positive forward
    /// component by JumpForwardSpeedFactor and the backward/side components
    /// by JumpStrafeSpeedFactor. BF1 FUN_00208b78 originally stored vertical
    /// speed and added facing * jump velocity * stick * 0.25. FEEL_NOTES
    /// records the deliberate BF1 deviations. The ground mover runs first;
    /// the first air update skips gravity. Class constants remain unchanged.
    fn take_off(&mut self, c: &SoldierConstants) {
        self.jump_animation = None;
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let fwd_axis = Vec2::new(-sy, -cy);
        self.vel.y += c.jump_vy;
        let planar = Vec2::new(self.vel.x, self.vel.z);
        let f = planar.dot(fwd_axis);
        let s = planar.dot(Vec2::new(cy, -sy));
        let k = if f > 0.0 {
            c.jump_forward_factor
        } else {
            c.jump_strafe_factor
        };
        let (f2, s2) = (f * k, s * c.jump_strafe_factor);
        self.vel.x = fwd_axis.x * f2 + cy * s2;
        self.vel.z = fwd_axis.y * f2 - sy * s2;
        self.set_stance(Stance::Jump);
    }

    /// The fly-height ceiling (both games after the air movers; re-jetpack.md
    /// §5.3): within [`FLY_CEILING_BAND`] of `ceiling` the vertical speed is
    /// pulled toward that far under it, after the update's move, so it bends
    /// the next one.
    pub fn fly_ceiling(&mut self, dt: f32, ceiling: f32) {
        let gap = ceiling - self.pos.y;
        if gap < FLY_CEILING_BAND {
            self.vel.y += (gap - FLY_CEILING_BAND) * dt * FLY_CEILING_PULL;
        }
    }

    /// A jet's start (BF2 `FUN_002476a0`, re-jetpack.md §3.1; card JETP,
    /// BF2's rules on every map): refused under `JetFuelMinBorder` (equal
    /// passes) or on a depleted energy bar (`JetEnergyDrain` is paid as the
    /// jump's cost is, [`Soldier::pay_energy`]); then `JetFuelInitialCost`
    /// (fuel floored at 0) and the kick, `v += normalize(R x strafe x 0.1 +
    /// F x max(fwd, -0.1) + (0, 1.1, 0)) x JetJump`, R and F the body's level
    /// rows. A sabre class that does not hover double-jumps (§3.3); else the
    /// hover or the jump-jet. `stick` = (fwd, strafe), right positive.
    pub fn jet_on(&mut self, c: &SoldierConstants, stick: (f32, f32)) -> bool {
        if self.fuel < c.jet_fuel_min_border || !self.pay_energy(c.jet_energy_drain) {
            return false;
        }
        self.fuel = (self.fuel - c.jet_fuel_initial).max(0.0);
        let (fwd_in, strafe_in) = stick;
        let (sy, cy) = fmath::sin_cos(self.yaw);
        let f = Vec3::new(-sy, 0.0, -cy);
        let r = Vec3::new(cy, 0.0, -sy);
        let dir = (r * (strafe_in * JET_KICK_STRAFE)
            + f * fwd_in.max(JET_KICK_BACK)
            + Vec3::Y * JET_KICK_LIFT)
            .normalize();
        self.vel += dir * c.jet_jump;
        if c.saber && !c.jet_hover {
            let direction = if fwd_in * fwd_in + strafe_in * strafe_in < 0.0625 {
                0
            } else if fwd_in > strafe_in.abs() {
                1
            } else if -fwd_in > strafe_in.abs() {
                2
            } else if strafe_in < 0.0 {
                3
            } else {
                4
            };
            let restart = self.jump_animation != Some(direction)
                || direction < 2
                || (self.stance == Stance::Jump && self.time_in_stance >= 0.5);
            // The Jedi's double jump: the kick's horizontal part is thrown
            // away for the stick's run speeds, and the rise is the jump's
            // again, or 1.5 times it (B2:162553-162600).
            let run = if fwd_in > 0.0 {
                c.max_speed
            } else {
                c.max_strafe_speed
            };
            let j = c.jump_vy;
            let vy = self.vel.y;
            self.vel = f * (fwd_in * run) + r * (strafe_in * c.max_strafe_speed);
            self.vel.y = if vy < 0.5 * j { vy + j } else { 1.5 * j };
            let age = self.time_in_stance;
            self.set_stance(Stance::Jump);
            self.jump_animation = Some(direction);
            if restart {
                self.jump_animation_serial = self.jump_animation_serial.wrapping_add(1);
            } else {
                self.time_in_stance = age;
            }
        } else {
            self.set_stance(if c.jet_hover {
                Stance::Hover
            } else {
                Stance::Jet
            });
        }
        true
    }

    /// The jet stances' update (BF2 `FUN_00240bf8` cases 6 / 7, re-jetpack.md
    /// §4.1-§4.2): the fuel's seconds (empty ends it), a tap ends it, else
    /// the stance's mover and the ceiling. Ending falls (stance 8) with no
    /// move this update; a sabre class tries its jet again at once (Vader and
    /// the Emperor, whose border is a full tank, are refused). `aim` is the
    /// aim before this update's turn (yaw, pitch).
    fn step_jet(
        &mut self,
        dt: f32,
        stick: (f32, f32),
        tap: bool,
        aim: (f32, f32),
        c: &SoldierConstants,
        ceiling: f32,
    ) {
        self.fuel -= c.jet_fuel_cost * dt;
        let empty = self.fuel < 0.0;
        if empty {
            self.fuel = 0.0;
        }
        if empty || tap {
            self.set_stance(Stance::Fall);
            if c.saber {
                self.jet_on(c, stick);
            }
            return;
        }
        let (fwd_in, strafe_in) = stick;
        let (sy, cy) = fmath::sin_cos(aim.0);
        let f = Vec3::new(-sy, 0.0, -cy);
        let r = Vec3::new(cy, 0.0, -sy);
        if self.stance == Stance::Jet {
            // §4.1: JetPush up, then BF2's air mover with the stick as an
            // acceleration of JetAcceleration (FUN_00248560): the drag,
            // the stick, gravity off the floor.
            self.vel.y += c.jet_push * dt;
            self.vel *= fmath::exp(-AIR_DRAG * self.vel.length() * dt);
            self.vel += (f * fwd_in + r * strafe_in) * (c.jet_acceleration * dt);
            if !self.grounded {
                self.vel.y -= GRAVITY * dt;
            }
        } else {
            // §4.2 (FUN_00248898): on the floor the into-floor part goes; all
            // speed fades by exp(-2 dt); along the flight axis D (the aim with
            // its height lifted by 0.08 and doubled) and the level right axis
            // the speed moves toward the jet row's targets by at most
            // JetAcceleration x dt. No gravity, no JetPush.
            if self.grounded {
                let vn = self.vel.dot(self.ground_n);
                if vn < 0.0 {
                    self.vel -= self.ground_n * vn;
                }
            }
            self.vel *= fmath::exp(-HOVER_DAMP * dt);
            let (sp, cp) = fmath::sin_cos(aim.1);
            let d = Vec3::new(f.x * cp, 2.0 * (sp + HOVER_LIFT), f.z * cp).normalize();
            let run = if fwd_in > 0.0 {
                c.max_speed
            } else {
                c.max_strafe_speed
            };
            let fwd_t = fwd_in * run * c.jet_control[0];
            let side_t = strafe_in * c.max_strafe_speed * c.jet_control[1];
            let a = c.jet_acceleration * dt;
            self.vel += d * (fwd_t - self.vel.dot(d)).clamp(-a, a);
            self.vel += r * (side_t - self.vel.dot(r)).clamp(-a, a);
        }
        self.pos += self.vel * dt;
        self.fly_ceiling(dt, ceiling);
    }

    /// Air steering: Jump/Fall select BF2 everywhere (JUNI); Thrown selects
    /// its original game. Original rules (card AIRC; re-stance-roll.md
    /// §4.3a, both games measured on open floor with PCSX2). `axes` are the
    /// level forward and right axes; `took_off` = the first update after the
    /// take-off (the PS2's floor bit is still set).
    ///
    /// - **BF2** (FUN_00240bf8 cases 4 / 8 -> FUN_00248560): the velocity is
    ///   scaled by exp(-[`AIR_DRAG`] * |v| * dt), then the stick is ADDED as an
    ///   acceleration, stick x ControlSpeed[jump] x Acceleration (2.1 m/s^2),
    ///   with no target and no cap. A standing jump with the stick forward
    ///   lands 0.94 m ahead at 1.85 m/s; a running jump keeps its speed but
    ///   for the drag, and the stick held back takes 2.6 m/s off it.
    /// - **BF1** (FUN_00205cc8 cases 4-6 -> FUN_002098b8, brake 0): each axis
    ///   is raised toward stick x [`BF1_AIR_SPEED`] x MaxSpeed (forward AND
    ///   back) / MaxStrafeSpeed at Acceleraton x [`BF1_ACCEL`] x
    ///   [`BF1_AIR_ACCEL`], and never lowered: a standing jump moves at
    ///   1.3 m/s from its first update (1.21 m), a running jump keeps its
    ///   speed with the stick released or held, and the stick held back
    ///   brings it to -1.3 m/s in 14 updates. The take-off update still has
    ///   the grounded bit, so its acceleration is the ground's (x 1).
    #[allow(clippy::too_many_arguments)]
    fn air_steer(
        &mut self,
        dt: f32,
        fwd_in: f32,
        strafe_in: f32,
        (t1, t2): (Vec3, Vec3),
        c: &SoldierConstants,
        rules: GroundRules,
        took_off: bool,
    ) {
        let ctl = self.stance.control();
        match rules {
            GroundRules::Bf2 => {
                self.vel *= fmath::exp(-AIR_DRAG * self.vel.length() * dt);
                if let Some(motion) = c.motion_target.filter(|m| m.weight > 0.0) {
                    // Native cases 4/8/9 replace the stick acceleration;
                    // this output is neither a velocity cap nor a lerp.
                    let weight = motion.weight.min(1.0);
                    self.vel +=
                        t1 * (motion.forward * weight * dt) + t2 * (motion.strafe * weight * dt);
                } else {
                    self.vel += t1 * (fwd_in * ctl[0] * c.acceleration * dt)
                        + t2 * (strafe_in * ctl[1] * c.acceleration * dt);
                }
            }
            GroundRules::Bf1 => {
                let air = if took_off { 1.0 } else { BF1_AIR_ACCEL };
                let a = c.acceleration * BF1_ACCEL * air * dt;
                let raise = |cur: f32, tgt: f32, stick: f32| -> f32 {
                    if stick > 0.0 && cur < tgt {
                        (cur + a).min(tgt)
                    } else if stick < 0.0 && tgt < cur {
                        (cur - a).max(tgt)
                    } else {
                        cur
                    }
                };
                let (cur_fwd, cur_strafe) = (self.vel.dot(t1), self.vel.dot(t2));
                let new_fwd = raise(cur_fwd, fwd_in * BF1_AIR_SPEED * c.max_speed, fwd_in);
                let new_strafe = raise(
                    cur_strafe,
                    strafe_in * BF1_AIR_SPEED * c.max_strafe_speed,
                    strafe_in,
                );
                self.vel += t1 * (new_fwd - cur_fwd) + t2 * (new_strafe - cur_strafe);
            }
        }
    }

    /// A floor contact. BF1's stand timer restarts at 5 s when it ends an
    /// airborne spell (B1:117407-117409), retaining the long fall time.
    /// JUNI jump eligibility no longer depends on this timer.
    fn touch_down(&mut self, rules: GroundRules) {
        if rules == GroundRules::Bf1 && self.air_time > 0.0 {
            self.stand_age = self.stand_age.max(5.0);
        }
        self.grounded = true;
        self.air_time = 0.0;
    }

    /// The floor under the NEW feet (`floor_y`, its normal), once per tick
    /// after [`Soldier::step`]. The PS2 clears the grounded bit every update
    /// and its collision response sets it again (BF1 FUN_00202f38, BF2
    /// FUN_0023cfd0): contact when the feet are within [`FLOOR_CONTACT_GAP`]
    /// over the floor, the floor is walkable ([`FLOOR_MIN_NY`]) and the
    /// soldier is not leaving it ([`FLOOR_MAX_APPROACH`]). Otherwise he is
    /// airborne: N = up, gravity next tick, and the stance falls by airtime
    /// (BF2 FUN_00240bf8, BF1 FUN_00205cc8 — re-soldier-ground.md §1.6a).
    /// `from_y` is the feet's height before the tick's move (the wedge test,
    /// [`WEDGE_MAX_DY`]).
    pub fn resolve_ground(
        &mut self,
        dt: f32,
        floor_y: f32,
        floor_n: Vec3,
        from_y: f32,
        rules: GroundRules,
    ) {
        let approach = self.vel.dot(floor_n);
        let walkable = floor_n.y > FLOOR_MIN_NY;
        let near = self.pos.y <= floor_y + FLOOR_CONTACT_GAP;
        // Card SKIRT (re-soldier-ground.md §7): three rules of the PS2's
        // contact code that carry a soldier up a short band steeper than the
        // climb limit on his momentum, both games.
        // The overlap (§7.2 rule 4): the PS2's push leaves 2 cm of it only
        // vertically — FLOOR_CONTACT_GAP x n.y along the normal on a touch-down
        // or a push into the floor, then n.y^2 of it a tick, less the tick's
        // move off the face. With none left, a move off a face under the climb
        // limit loses the floor for this tick: airborne, N up, full control —
        // how the PS2's soldier lifts off the band when his speed runs out.
        let mut lost = false;
        if near && walkable && approach < FLOOR_MAX_APPROACH {
            let fresh = FLOOR_CONTACT_GAP * floor_n.y;
            if !self.grounded || approach < 0.0 && -approach * dt * floor_n.y >= FLOOR_CONTACT_GAP {
                self.floor_overlap = fresh;
            } else {
                self.floor_overlap =
                    (self.floor_overlap * floor_n.y * floor_n.y - approach * dt).min(fresh);
                lost = floor_n.y < rules.climb_min_ny()
                    && self.floor_overlap <= 0.0
                    && approach * dt > FLOOR_LOSS_MOVE;
            }
        }
        if near && walkable && approach < FLOOR_MAX_APPROACH && !lost {
            self.pos.y = floor_y;
            if approach < 0.0 {
                self.vel -= floor_n * approach;
            }
            // The dead zone (§7.2 rule 3).
            if self.vel.x.abs() < FLOOR_DEAD_ZONE {
                self.vel.x = 0.0;
            }
            if self.vel.z.abs() < FLOOR_DEAD_ZONE {
                self.vel.z = 0.0;
            }
            // The flattest normal (§7.2 rule 1): the PS2 keeps the flattest
            // floor contact of a pass, and its foot sphere meets a steeper face
            // while it still stands on the floor before it — for that one tick
            // N is the old floor's: full hold, then the projection on the face.
            // The port keeps it every time; the PS2 on 8 of 18 run-ups (the
            // update's phase) — a named deviation, FEEL_NOTES.
            let keep = self.grounded && !self.kept_flat && floor_n.y < self.ground_n.y - 1e-3;
            self.kept_flat = keep;
            if !keep {
                self.ground_n = floor_n;
            }
            self.touch_down(rules);
            return;
        }
        self.kept_flat = false;
        if lost {
            self.grounded = false;
            self.ground_n = Vec3::Y;
            self.air_time += dt;
            return;
        }
        // A face too steep to stand on still holds the feet out: he slides
        // along it under gravity, airborne — unless his feet kept their
        // height: wedged, he stands, level. PROVISIONAL (ours, the point
        // foot): the whole into-face velocity goes, where the PS2's wall
        // contact removes only the horizontal part and lifts by the true
        // normal (re-soldier-ground.md §1.3; card STHOP2).
        if near && !walkable && floor_n.y > 0.0 {
            if self.pos.y < floor_y {
                self.pos.y = floor_y;
                if approach < 0.0 {
                    self.vel -= floor_n * approach;
                }
            }
            if (self.pos.y - from_y).abs() < WEDGE_MAX_DY {
                self.vel.y = self.vel.y.max(0.0);
                self.ground_n = Vec3::Y;
                self.touch_down(rules);
                return;
            }
        }
        self.grounded = false;
        self.ground_n = Vec3::Y;
        self.air_time += dt;
        // Stand/sprint fall after the game's airtime, crouch stands up after
        // 0.3 s and prone falls after 0.3 s (BF2's cases 1 and 2; PROVISIONAL
        // for BF1, whose crouch and prone cases are unread).
        let stance_age = self.stand_age - self.air_time;
        match self.stance {
            Stance::Stand | Stance::Sprint if self.air_time > rules.fall_after(stance_age) => {
                self.set_stance(Stance::Fall)
            }
            Stance::Crouch if self.air_time > 0.3 => self.set_stance(Stance::Stand),
            Stance::Prone if self.air_time > 0.3 => self.set_stance(Stance::Fall),
            _ => {}
        }
    }

    /// Feed the soldier's state into the sim hash (bit-exact).
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [
            self.pos.x,
            self.pos.y,
            self.pos.z,
            self.vel.x,
            self.vel.y,
            self.vel.z,
            self.yaw,
            self.pitch,
            self.time_in_stance,
            self.stand_age,
            self.lock_timer,
            self.energy,
            self.ground_n.x,
            self.ground_n.y,
            self.ground_n.z,
            self.air_time,
            self.floor_overlap,
            self.knock_draw,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        eat(&[
            self.stance as u8,
            self.grounded as u8,
            self.kept_flat as u8,
            self.fire_lock as u8,
        ]);
        // These latches carry history that energy and lock duration do not.
        let latches = u8::from(self.energy_depleted) | (u8::from(self.lock_hard) << 1);
        if latches != 0 {
            eat(b"SOLDIER_LATCH");
            eat(&[latches]);
        }
        // Jet fuel (card JETP), when there is any: a class with no jet keeps
        // 0 and hashes as before. The jet state is the stance, above.
        if self.fuel != 0.0 {
            eat(b"SOLDIER_FUEL");
            eat(&self.fuel.to_bits().to_le_bytes());
        }
        if self.jump_animation.is_some() || self.jump_animation_serial != 0 {
            eat(b"HERO_JUMP");
            eat(&[self.jump_animation.unwrap_or(u8::MAX)]);
            eat(&self.jump_animation_serial.to_le_bytes());
        }
        if self.knock_rules != KNOCK_RULES {
            eat(b"FORCE_KNOCK");
        }
    }
}

#[cfg(test)]
mod tests;
