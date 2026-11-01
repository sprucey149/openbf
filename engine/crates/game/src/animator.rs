//! The soldier animator (visual only; never touches the sim): two clip
//! players into one kept pose, as the PS2 poses a soldier
//! (`analysis/re-soldier-anim-layers.md` §3, §7 `[O]`).
//!
//! * The LOWER player poses the lower-body bones with the movement clip, the
//!   UPPER player the upper-body bones with the version of that clip the set
//!   of the weapon in hand has — or with the set's shoot / reload clip while
//!   one runs. Every shot restarts the shoot clip; an action ends when its
//!   clip has run. Standing still, the legs join the action (BF1: the same
//!   clip, its blend 0.5 s longer; BF2: the action's lower slot, and only if
//!   he stood still when it started). A jump, fall, roll or death (BF2: a
//!   sprint too) plays its own pair and no action.
//! * The sets, the two bone masks and each clip's loop flag and blend time
//!   are baked beside the soldier (`soldiers/<model>.sets.ron`,
//!   `manifest::anim_sets`); [`Rig`] resolves them over the GLB's clips.
//! * Blending: BF1 pulls the kept pose toward the clip by `t / T` each update
//!   and overwrites it at 1 (§3.3); BF2 crossfades linearly over `T`, the old
//!   clip's `BlendTimeTo` for the new one, else min(new in, old out) (§7).
//! * Playback follows the engine (re-anim-keys.md §4.2): `t ∈ [0,1]` advances
//!   by `dt·30/F` on BF1 and `dt·30/(F−1)` on BF2, the sampled frame is
//!   `t·(F−1)`, loop clips wrap, others clamp.
//!
//! * The root twist (`analysis/re-roll-picture.md` 2, 3, 7 `[O]`): `DummyRoot`
//!   is turned about the up axis by an eased angle, the PS2's quarter of the
//!   rest an update at 30 updates a second. In a roll its target is the
//!   direction of travel against the facing (BF2's rule, used on BF1 maps
//!   too: the port's one roll is BF2's; a side roll's angle is BF1's 90
//!   degrees), else 0 - so a left roll dives head first to his left while
//!   the camera and the aim stay ahead. Ours: the turn is laid on the drawn
//!   pose about the model's up axis, not written into the kept pose (BF2
//!   overwrites the bone, BF1 multiplies it by `twist x` the legs' blend
//!   weight; the port's `DummyRoot` is in neither mask, so the two agree but
//!   for BF1's first 4-8 updates, where its weight lags the twist).
//!
//! * The legs (card ANIMM, `analysis/re-soldier-anim-layers.md` 3.5, 7 `[O]`):
//!   [`choose`] is the PS2's movement clip choice - a soldier with no strafe
//!   clips (stand, crouch) strafes on the forward or backward clip with the
//!   root twisted toward his travel, eased as the roll's; walk or run by BF1's
//!   5.5 / 5.0 m/s or BF2's speed mode; both players play a movement clip at
//!   `0.95 v / v_clip + 0.05` of the clock, `v_clip` the clip's root speed,
//!   so the feet keep to the ground. Standing, the twist keeps the legs where
//!   they were while the body turns, and past a stance's limit a turn clip
//!   takes them round ([`Animator::step`]); the aim pose turns the torso back
//!   by the twist.
//!
//! A class's `SkeletonRootScale` divides the velocity the chooser and the
//! clip rate read ([`AnimInput::root_scale`], card USKN); BF2's raised 10/s
//! limit under a scale below 1 and the scale's reset bracket (soldier
//! +0xc14) are not built (re-soldier-skeleton-skins 7).
//!
//! PROVISIONAL and ours, each its own card: BF2's alert soldier takes the BF1 alert
//! window (the soldier update's other branch, by a weapon's virtual at
//! vtable+0x5c, is unread `[H]`); the BF1 grenade's three throw clips and the
//! charge clip are not driven. The alert timer, its clip rows and the aim
//! pose are [`aim`]'s.

use crate::skin::{Clip, Pose, Skeleton};
use core::soldier::Stance;
use manifest::anim_sets::{bf1, bf2, SoldierSets};
use std::f32::consts::PI;
use turn::wrap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Game {
    Bf1,
    Bf2,
}

/// BF1's forward walk -> run speeds (m/s): run above [`BF1_RUN`], or above
/// [`BF1_RUN_STAY`] while the run clip plays (`FUN_0022e990`
/// B1:145127-145148; also a left strafe's, where a stance has strafe clips).
pub const BF1_RUN: f32 = 5.5;
pub const BF1_RUN_STAY: f32 = 5.0;
/// BF1's chooser gives idle above this speed (m/s; `v^2 > 64`).
const BF1_IDLE_ABOVE: f32 = 8.0;
/// BF2's speed mode 2 (the run) above this share of the class's MaxSpeed,
/// this one to stay (`FUN_0024f350` B2:168790-168800).
pub const BF2_RUN: f32 = 0.78;
pub const BF2_RUN_STAY: f32 = 0.71;
/// Idle under this speed (m/s): BF1 on both axes, BF2 the whole (0.2 once
/// moving; `FUN_0027ed18` B2:202186-202192).
const IDLE: f32 = 0.3;
/// The prone changes' clip names ([`Special::ProneDown`], [`Special::ProneUp`]).
const PRONE_CHANGES: [&str; 4] = [
    "stand_getdown_prone",
    "crouch_getdown_prone",
    "prone_getup_stand",
    "prone_getup_crouch",
];
const BF2_IDLE_STAY: f32 = 0.2;

/// The travel angles (rad, + = his left) that take the forward clip: inside
/// +-[`FWD_WINDOW`], +-[`FWD_WINDOW_STAY`] while a forward clip plays
/// (BF2: while the soldier's forward bit is up), [`ALERT_WINDOW`] when
/// alert; else the backward clip. The leg twist is the travel (+pi when
/// backward) limited to +-[`LEG_LIMIT`], [`ALERT_LEGS`] when alert (BF1
/// `FUN_0022e990` B1:145081-145124, BF2 `FUN_0024f350` B2:168718-168766).
const FWD_WINDOW: f32 = 1.256_637_1;
const FWD_WINDOW_STAY: f32 = 1.884_955_8;
const ALERT_WINDOW: (f32, f32) = (-1.806_415_8, 1.178_097_2);
const LEG_LIMIT: f32 = 1.256_637_1;
// -pi/4 is the ELF's word (-0.7853982).
const ALERT_LEGS: (f32, f32) = (-std::f32::consts::FRAC_PI_4, 0.549_778_7);

/// The playback rate of a movement clip: `0.95 v / v_clip + 0.05`, the
/// share of the speed over the clip's own root speed eased a twentieth
/// toward 1 (B1:145143, 145197; B2:202353).
pub fn move_rate(speed: f32, clip_speed: f32) -> f32 {
    if clip_speed.abs() < 1e-3 {
        return 1.0;
    }
    let k = speed.abs() / clip_speed.abs();
    k + (1.0 - k) * 0.05
}

/// The updates a second BF1's per-update pull is written for (§3.3): the
/// weight is turned into one that gives the same pose at any frame rate.
const BF1_UPDATES: f32 = 60.0;
const BF1_THROW_WINDUP: f32 = 0.3;

/// The direction of travel against the body's facing (rad, positive = his
/// left, the PS2's row 0): `atan2(v . left, v . forward)` of the planar
/// velocity, for a body drawn at sim yaw `yaw` ([`crate::soldier_vis`]'s
/// model turn: forward `(-sin, 0, -cos)`, left `(-cos, 0, sin)`). `None`
/// under 0.3 m/s, where the direction is noise.
pub fn travel(vel: glam::Vec3, yaw: f32) -> Option<f32> {
    let v = body_vel(vel, yaw);
    (v.length() >= IDLE).then(|| v.x.atan2(v.y))
}

/// The planar velocity in the body's frame: x his left, y ahead (m/s).
pub fn body_vel(vel: glam::Vec3, yaw: f32) -> glam::Vec2 {
    let (sy, cy) = yaw.sin_cos();
    glam::Vec2::new(-cy * vel.x + sy * vel.z, -sy * vel.x - cy * vel.z)
}

/// What picks a soldier class's sets (`game::roster::class_anim`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassAnim {
    /// BF2's skeleton family.
    pub family: String,
    /// `SkeletonRootScale` of a class with its own skeleton, else 1 (0 =
    /// unset, taken as 1): the chooser and the clip rate read the velocity
    /// over it ([`AnimInput::root_scale`]). The bone_root translation it
    /// scales is in the baked clips (`bake::soldier::PoseRig`, card USKN).
    pub root_scale: f32,
    /// (weapon class, lowercase; its animation bank) per weapon slot that
    /// names one.
    pub banks: Vec<(String, String)>,
    /// BF2 custom-animation descriptors, including explicit hero defaults.
    pub custom: custom::Descriptors,
}

impl ClassAnim {
    /// The bank of a weapon class; the rifle's for one that names none.
    pub fn bank_of(&self, weapon: &str) -> &str {
        self.banks
            .iter()
            .find(|(w, _)| w.eq_ignore_ascii_case(weapon))
            .map_or("rifle", |(_, b)| b)
    }
}

/// What the soldier is doing, read off the sim each update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimInput {
    pub stance: Stance,
    /// The planar velocity in the body's frame ([`body_vel`]: x his left, y
    /// ahead; m/s).
    pub vel: glam::Vec2,
    /// The body's facing (the sim's yaw, rad; + turns him left): standing,
    /// the legs keep their place while it turns.
    pub yaw: f32,
    pub mouse_aim: bool,
    /// BF2: the class's MaxSpeed (m/s), which the speed mode is measured on.
    pub max_speed: f32,
    /// The class's `SkeletonRootScale` ([`ClassAnim::root_scale`]; <= 0 is
    /// taken as 1): the movement clip's choice and rate read `vel / scale`
    /// (BF1 `FUN_0022e990` B1:144992-144995, BF2 `FUN_0027ed18`
    /// B2:202177-202181; `analysis/re-soldier-skeleton-skins.md` 7 [O]), so
    /// a 1.15 Wookiee walks at v / 1.15 on his longer legs.
    pub root_scale: f32,
    pub dead: bool,
    /// BF2 hero state 0x10, simple clip 0x1a (`stand_deadhero`).
    pub hero_dead: bool,
    /// In a hard lock (`Soldier::look_frozen`): after a get-up
    /// ([`Special::StandUp`]) or across a prone change ([`Special::ProneDown`],
    /// [`Special::ProneUp`]).
    pub hard_lock: bool,
    /// The set of the weapon in hand ([`Rig::weapon_set`]).
    pub set: usize,
    /// A shot left the weapon this update.
    pub fired: bool,
    pub throw: Option<u8>,
    pub throw_windup: f32,
    pub reloading: bool,
    /// The weapon's reload time (s): BF2 fits the reload clip to it.
    pub reload_time: f32,
    /// He has a reason to be alert (he fires, charges or holds a target).
    pub alert: bool,
    /// The aim line: (yaw from the body's facing, pitch), rad. The animator
    /// takes the root twist off the yaw.
    pub aim: (f32, f32),
}

impl AnimInput {
    /// Horizontal speed (m/s).
    pub fn speed(&self) -> f32 {
        self.vel.length()
    }

    /// The direction of travel against the facing (rad, + his left; the
    /// PS2's `atan2(left, ahead)`); `None` when too slow to tell (the
    /// animator keeps the last).
    pub fn travel(&self) -> Option<f32> {
        (self.speed() >= IDLE).then(|| self.vel.x.atan2(self.vel.y))
    }
}

/// The movement clip of a soldier on his feet.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Move {
    #[default]
    Idle,
    TurnLeft,
    TurnRight,
    WalkForward,
    WalkBackward,
    RunForward,
    RunBackward,
    /// Strafes: only a stance with strafe clips (BF1 prone) plays them.
    WalkLeft,
    WalkRight,
    RunLeft,
    RunRight,
}

impl Move {
    /// (BF1 action id, BF2 move id): the two games order them apart (§6.1).
    fn ids(self) -> (usize, usize) {
        match self {
            Move::Idle => (0, 0),
            Move::TurnLeft => (3, 3),
            Move::TurnRight => (4, 4),
            Move::WalkForward => (5, 5),
            Move::WalkBackward => (6, 7),
            Move::RunForward => (7, 6),
            Move::RunBackward => (8, 8),
            Move::WalkRight => (9, 9),
            Move::WalkLeft => (10, 11),
            Move::RunRight => (11, 10),
            Move::RunLeft => (12, 12),
        }
    }

    /// The clip of the same direction a missing one falls back to: BF2
    /// ships no `stand_walkbackward` and runs instead (§7); a crouch has no
    /// run and walks.
    fn other_gait(self) -> Self {
        match self {
            Move::WalkForward => Move::RunForward,
            Move::RunForward => Move::WalkForward,
            Move::WalkBackward => Move::RunBackward,
            Move::RunBackward => Move::WalkBackward,
            Move::WalkLeft => Move::RunLeft,
            Move::RunLeft => Move::WalkLeft,
            Move::WalkRight => Move::RunRight,
            Move::RunRight => Move::WalkRight,
            m => m,
        }
    }

    /// The clip's direction (0 ahead, 1 back, 2 left, 3 right): a walk <->
    /// run change of one direction keeps the phase (`FUN_0022f328`); `None`
    /// standing.
    fn dir(self) -> Option<u8> {
        match self {
            Move::Idle | Move::TurnLeft | Move::TurnRight => None,
            Move::WalkForward | Move::RunForward => Some(0),
            Move::WalkBackward | Move::RunBackward => Some(1),
            Move::WalkLeft | Move::RunLeft => Some(2),
            Move::WalkRight | Move::RunRight => Some(3),
        }
    }

    /// Does it move him (a walk, run or strafe; the PS2's ids 5 and up)?
    pub fn moving(self) -> bool {
        self.dir().is_some()
    }

    /// The root speed's axis of the clip (0 sideways, 1 ahead).
    fn axis(self) -> usize {
        usize::from(self.dir().is_some_and(|d| d < 2))
    }
}

/// What the legs keep between updates: the movement clip last chosen and
/// BF2's soldier state (the speed mode's run and the forward bit,
/// `FUN_0024f350`: up while standing, so a start from a stand has the wide
/// window).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Legs {
    pub mv: Move,
    pub bf2_run: bool,
    pub bf2_forward: bool,
}

impl Default for Legs {
    fn default() -> Self {
        Self {
            mv: Move::Idle,
            bf2_run: false,
            bf2_forward: true,
        }
    }
}

/// The PS2's movement clip for a soldier on his feet in stance `s` (0
/// stand, 1 crouch, 2 prone) moving at `v` (body frame, [`body_vel`]), and
/// the leg twist it asks for; `None` = idle (the turn on the spot is
/// [`Animator::step`]'s). `strafe`: the stance has strafe clips
/// ([`Rig::strafes`]). Updates `legs` (not its `mv`, which the caller sets
/// once the turn rules have run). BF1 `FUN_0022e990` (B1:144969-145199),
/// BF2 `FUN_0024f350` (B2:168676-168810) and `FUN_0027ed18`
/// (B2:202144-202356). Which of walk or run a stance HAS is left to
/// [`Rig::body_slots`]'s fallback (the PS2's tests come to the same clip).
///
/// `scale` is the class's `SkeletonRootScale`: the animator's own tests (BF1
/// all of them, BF2's idle test) read `v / scale`; BF2's speed mode is the
/// soldier's (`FUN_0024f350`) and reads `v` against MaxSpeed.
pub fn choose(
    game: Game,
    v: glam::Vec2,
    scale: f32,
    strafe: bool,
    alert: bool,
    max_speed: f32,
    legs: &mut Legs,
) -> Option<(Move, f32)> {
    let raw = v.length();
    let v = v / scale;
    let (side, fwd, speed) = (v.x, v.y, v.length());
    let a = side.atan2(fwd);
    let prev = legs.mv;
    let window = |stay: bool| {
        if alert {
            ALERT_WINDOW
        } else if stay {
            (-FWD_WINDOW_STAY, FWD_WINDOW_STAY)
        } else {
            (-FWD_WINDOW, FWD_WINDOW)
        }
    };
    let limit = |t: f32| {
        let (lo, hi) = if alert {
            ALERT_LEGS
        } else {
            (-LEG_LIMIT, LEG_LIMIT)
        };
        t.clamp(lo, hi)
    };
    match game {
        Game::Bf1 => {
            if speed > BF1_IDLE_ABOVE || (side.abs() < IDLE && fwd.abs() < IDLE) {
                return None;
            }
            if strafe && fwd.abs() < side.abs() {
                // Left runs above 5.0 (5.5 while walking left); right is
                // always the run clip: a negative speed against 5.0.
                return Some(if side > 0.0 {
                    let stay = if prev == Move::WalkLeft {
                        BF1_RUN
                    } else {
                        BF1_RUN_STAY
                    };
                    let run = side > stay;
                    (
                        if run { Move::RunLeft } else { Move::WalkLeft },
                        wrap(a - PI / 2.0),
                    )
                } else {
                    (Move::RunRight, wrap(a + PI / 2.0))
                });
            }
            let back = if strafe {
                fwd <= 0.0 && side.abs() * 0.1 <= fwd.abs()
            } else {
                let (lo, hi) = window(matches!(prev, Move::WalkForward | Move::RunForward));
                !(lo < a && a < hi)
            };
            // A stance with strafe clips twists the legs the whole angle.
            let twist = |t: f32| if strafe { t } else { limit(t) };
            Some(if back {
                (Move::RunBackward, twist(wrap(a + PI)))
            } else {
                let stay = prev == Move::RunForward;
                let run = speed > if stay { BF1_RUN_STAY } else { BF1_RUN };
                (
                    if run {
                        Move::RunForward
                    } else {
                        Move::WalkForward
                    },
                    twist(a),
                )
            })
        }
        Game::Bf2 => {
            let idle = if prev == Move::Idle {
                IDLE
            } else {
                BF2_IDLE_STAY
            };
            if speed < idle {
                (legs.bf2_run, legs.bf2_forward) = (false, true);
                return None;
            }
            // The speed mode: run above 0.78 MaxSpeed, 0.71 to stay.
            let k = if legs.bf2_run { BF2_RUN_STAY } else { BF2_RUN };
            legs.bf2_run = raw > max_speed * k;
            let gait = |walk: Move, run: Move| if legs.bf2_run { run } else { walk };
            // The soldier's twist and forward bit (`FUN_0024f350`).
            let (lo, hi) = window(legs.bf2_forward);
            let fwd_bit = lo < a && a < hi;
            legs.bf2_forward = fwd_bit;
            if strafe {
                // `FUN_0027ed18`'s own angles when the set has strafes.
                return Some(if fwd.abs() < side.abs() {
                    if side > 0.0 {
                        (gait(Move::WalkLeft, Move::RunLeft), wrap(a - PI / 2.0))
                    } else {
                        (gait(Move::WalkRight, Move::RunRight), wrap(a + PI / 2.0))
                    }
                } else if fwd <= 0.0 {
                    (gait(Move::WalkBackward, Move::RunBackward), wrap(a + PI))
                } else {
                    (gait(Move::WalkForward, Move::RunForward), a)
                });
            }
            Some(if fwd_bit {
                (gait(Move::WalkForward, Move::RunForward), limit(a))
            } else {
                (
                    gait(Move::WalkBackward, Move::RunBackward),
                    limit(wrap(a + PI)),
                )
            })
        }
    }
}

/// A state with its own pair of clips and no weapon action (§3.6, §7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Special {
    DirectionalJump(u8),
    Dead,
    HeroDead,
    Roll,
    Jump,
    Fall,
    /// Both jet stances, the dark trooper's jump-jet included
    /// (`jetpack_hover`: BF1 `FUN_0022a0a8` state 7 -> special 0x15, BF2
    /// `FUN_0027bae0` states 6 / 7 -> simple id 0x19; card JETP2). The one
    /// special a weapon action plays over (the stand's action row).
    JetHover,
    Landing(bool),
    /// BF2 only: BF1 sprints on the run clip.
    Sprint,
    /// Card KNOCK (re-knockback §2, B1 FUN_0022a0a8): thrown, by the
    /// velocity against the body - 0 front, 1 back, 2 left, 3 right
    /// (`thrown_flying*`, ids 6-9).
    Flying(u8),
    /// The tumble (`thrown_tumblefront / back`), the get-up state
    /// (`thrown_land*soft`) and, for one who died after a throw, the rest
    /// pose (`thrown_rest*soft`); `true` = back. PROVISIONAL: "front" is
    /// moving toward his facing, and the get-up state's bounce clips are
    /// not picked.
    Tumble(bool),
    Land(bool),
    Rest(bool),
    /// The lock after a get-up (`stand_getupfront / back`, ids 0x10 / 0x11):
    /// the side of the get-up clip before it (re-knockback §2).
    StandUp(bool),
    /// The prone change's hard lock (`prone_down_lock` / `prone_up_lock`,
    /// BF1's clip lengths): `stand_getdown_prone` / `crouch_getdown_prone`
    /// (`true` = from a crouch, ids 0x16 / 0x18) and `prone_getup_stand` /
    /// `prone_getup_crouch` (`true` = to a crouch, ids 0x17 / 0x19). BF2's
    /// are borrowed from BF1 ([`Rig::prone`]).
    ProneDown(bool),
    ProneUp(bool),
}

impl Special {
    fn name(self) -> &'static str {
        match self {
            Special::Dead => "stand_death_forward",
            Special::HeroDead => "stand_deadhero",
            Special::Roll => "diveforward",
            Special::Jump => "jump",
            Special::DirectionalJump(d) => [
                "jump",
                "jump_forward",
                "jump_backward",
                "jump_left",
                "jump_right",
            ][usize::from(d.min(4))],
            Special::Fall => "fall",
            Special::JetHover => "jetpack_hover",
            Special::Landing(hard) => ["landsoft", "landhard"][usize::from(hard)],
            Special::Sprint => "sprint",
            Special::Flying(d) => [
                "thrown_flyingfront",
                "thrown_flyingback",
                "thrown_flyingleft",
                "thrown_flyingright",
            ][usize::from(d.min(3))],
            Special::Tumble(b) => ["thrown_tumblefront", "thrown_tumbleback"][usize::from(b)],
            Special::Land(b) => ["thrown_landfrontsoft", "thrown_landbacksoft"][usize::from(b)],
            Special::Rest(b) => ["thrown_restfrontsoft", "thrown_restbacksoft"][usize::from(b)],
            Special::StandUp(b) => ["stand_getupfront", "stand_getupback"][usize::from(b)],
            Special::ProneDown(c) => {
                ["stand_getdown_prone", "crouch_getdown_prone"][usize::from(c)]
            }
            Special::ProneUp(c) => ["prone_getup_stand", "prone_getup_crouch"][usize::from(c)],
        }
    }

    /// A clip of the throw (the rest pose included), and its side.
    fn thrown(self) -> Option<bool> {
        match self {
            Special::Flying(d) => Some(d == 1),
            Special::Tumble(b) | Special::Land(b) | Special::Rest(b) | Special::StandUp(b) => {
                Some(b)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Body {
    Special(Special),
    /// Stance 0 stand, 1 crouch, 2 prone.
    Feet(usize, Move),
}

/// The special state of an input, else the stance its feet are in (0
/// stand, 1 crouch, 2 prone; the movement is [`choose`]'s). BF1 sprints on
/// the stand's clips.
pub fn stance_of(i: &AnimInput, game: Game) -> Result<usize, Special> {
    // A body thrown by the blast that killed it flies first (card KNOCK).
    if i.dead && !i.stance.knocked() {
        return Err(if i.hero_dead && game == Game::Bf2 {
            Special::HeroDead
        } else {
            Special::Dead
        });
    }
    let back = i.vel.y < 0.0;
    match i.stance {
        Stance::Roll => Err(Special::Roll),
        Stance::Jump => Err(Special::Jump),
        // Both games play `jetpack_hover` in every jet stance, the
        // jump-jet's too (card JETP2, analysis/re-jetpack.md §8).
        Stance::Jet | Stance::Hover => Err(Special::JetHover),
        Stance::Fall => Err(Special::Fall),
        Stance::Dead => Err(Special::Dead),
        Stance::Thrown => Err(Special::Flying(if i.vel.y.abs() >= i.vel.x.abs() {
            u8::from(back)
        } else if i.vel.x > 0.0 {
            2
        } else {
            3
        })),
        Stance::Tumble => Err(Special::Tumble(back)),
        Stance::GetUp => Err(Special::Land(back)),
        Stance::Sprint if game == Game::Bf2 => Err(Special::Sprint),
        Stance::Crouch => Ok(1),
        Stance::Prone => Ok(2),
        Stance::Stand | Stance::Sprint => Ok(0),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionKind {
    Shoot,
    Throw(u8),
    Reload,
}

/// A clip a set's slot plays, with the facts its player needs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Slot {
    /// Index into the asset's clips.
    pub clip: usize,
    pub looping: bool,
    pub blend_in: f32,
    pub blend_out: f32,
    /// BF2: the animation's `AimType` on this layer.
    pub aim: aim::AimKind,
    /// BF2: the animation (pair) the slot belongs to.
    pair: Option<usize>,
}

/// A soldier's baked sets over his asset's skeleton and clips.
#[derive(Debug, Clone)]
pub struct Rig {
    pub game: Game,
    /// Per skeleton bone: posed by the upper / the lower player.
    upper: Vec<bool>,
    lower: Vec<bool>,
    /// `sets.clips` index → asset clip index.
    map: Vec<Option<usize>>,
    sets: SoldierSets,
    /// BF2: (family, weapon class) of each set, in [`Rig::weapon_set`]'s
    /// numbering.
    bf2: Vec<(usize, String)>,
    /// Per asset clip: its root speed (`[x, z]` m/s, the bake's
    /// `root_speed`), for a walk, run or sprint.
    root: Vec<Option<[f32; 2]>>,
    /// Per asset clip: a thrown flight (`thrown_flying*`), drawn over the
    /// soldier ([`Rig::root_bone`]).
    flight: Vec<bool>,
    /// `bone_root` and its rest translation. The flight clips hold it 1.3 to
    /// 1.8 m off the soldier's position (BF1 and BF2 alike: front +1.8 m z,
    /// back -1.65, right -1.25 x), so the drawn body left his place (and
    /// his hit prims) until he landed: off the screen's bottom when thrown
    /// toward the camera (the maintainer 2026-10-27). Ours, PROVISIONAL (the PS2's
    /// draw of these clips is not measured): a flight clip keeps the root's
    /// rest x and z; its height and every rotation are the clip's.
    root_bone: Option<(usize, glam::Vec3)>,
    /// BF2: the rifle's prone clips borrowed from BF1 (the bake's
    /// `BORROWED_PRONE`, issue #40; the PS2's BF2 has no prone): per
    /// [`bf2::MOVES`], then the shot and the reload, then the four
    /// [`PRONE_CHANGES`].
    prone: Vec<Option<usize>>,
    /// BF1: BF2's throw borrowed for every map (the bake's
    /// `BORROWED_THROW`, card GTHRW): `shoot_secondary` (underhand), then
    /// `shoot_secondary2` (overhand). `None` when not baked: BF1's own
    /// grenade clip plays, its wind-up stretched to the release.
    throw: [Option<usize>; 2],
}

/// BF2's rifle throw clips, which a BF1 soldier borrows ([`Rig::throw`]).
pub const BF2_THROW_CLIPS: [&str; 2] = [
    "human_rifle_stand_shoot_secondary_full",
    "human_rifle_stand_shoot_secondary2_full",
];

impl Rig {
    pub fn new(sets: SoldierSets, skel: &Skeleton, clips: &[Clip]) -> Self {
        let mask = |names: &[String]| -> Vec<bool> {
            skel.names
                .iter()
                .map(|b| names.iter().any(|n| n.eq_ignore_ascii_case(b)))
                .collect()
        };
        let bf2 = sets
            .bf2
            .iter()
            .enumerate()
            .flat_map(|(f, s)| s.sets.keys().map(move |k| (f, k.clone())))
            .collect();
        let map: Vec<Option<usize>> = sets
            .clips
            .iter()
            .map(|c| find_clip(clips, &c.name))
            .collect();
        let throw = BF2_THROW_CLIPS.map(|n| {
            if sets.game == "bf2" {
                None
            } else {
                find_clip(clips, n)
            }
        });
        let mut root = vec![None; clips.len()];
        for (c, at) in sets.clips.iter().zip(&map) {
            if let (Some(v), Some(&k)) = (sets.root_speed.get(&c.name), at.as_ref()) {
                root[k] = Some(*v);
            }
        }
        Self {
            game: if sets.game == "bf2" {
                Game::Bf2
            } else {
                Game::Bf1
            },
            upper: mask(&sets.upper_bones),
            lower: mask(&sets.lower_bones),
            map,
            sets,
            bf2,
            root,
            flight: clips
                .iter()
                .map(|c| c.name.to_ascii_lowercase().contains("thrown_flying"))
                .collect(),
            root_bone: skel
                .index_of("bone_root")
                .map(|b| (b, skel.local_bind[b].w_axis.truncate())),
            prone: bf2::MOVES
                .iter()
                .map(|m| format!("human_rifle_prone_{m}"))
                .chain(["shoot", "reload"].map(|a| format!("human_rifle_prone_{a}")))
                .chain(PRONE_CHANGES.iter().map(|c| format!("human_rifle_{c}")))
                .map(|n| find_clip(clips, &n))
                .collect(),
            throw,
        }
    }

    /// BF1: the borrowed BF2 throw clip of `variant` (overhand above 0),
    /// standing or crouched (BF2's crouch plays the stand clips; it has no
    /// prone throw, so a prone soldier keeps BF1's).
    fn borrowed_throw(&self, s: usize, variant: u8) -> Option<usize> {
        if self.game != Game::Bf1 || s == 2 {
            return None;
        }
        self.throw[usize::from(variant > 0)]
    }

    /// Does this BF1 soldier throw BF2's throw ([`Rig::throw`] baked)? Then a
    /// thrown weapon is drawn as BF2's offhand one: the primary stays in
    /// his hand and keeps its set.
    pub fn throws_offhand(&self) -> bool {
        self.throw.iter().any(Option::is_some)
    }

    /// Is asset clip `clip` a borrowed BF2 throw ([`Rig::throw`])?
    pub fn is_borrowed_throw(&self, clip: usize) -> bool {
        self.throw.contains(&Some(clip))
    }

    /// BF2: the borrowed prone clip `k` of [`Rig::prone`] as a whole-body
    /// slot.
    fn prone_slot(&self, k: usize, looping: bool) -> Option<Slot> {
        Some(Slot {
            clip: (*self.prone.get(k)?)?,
            looping,
            blend_in: 0.2,
            blend_out: 0.2,
            aim: aim::AimKind::None,
            pair: None,
        })
    }

    /// The set a weapon class puts in the soldier's hands: `bank` is the
    /// first word of its `AnimationBank` (BF2: or `CustomAnimationBank` /
    /// `ComboAnimationBank`), `family` the first word of the soldier class's
    /// `AnimationName` / `SkeletonName` (BF2 only; §4, §6.2). No match is the
    /// rifle's.
    pub fn weapon_set(&self, family: &str, bank: &str) -> usize {
        let bank = bank.trim().to_ascii_lowercase();
        if self.game == Game::Bf1 {
            return bf1::weapon_set(&bank).unwrap_or(0);
        }
        let fam = self
            .sets
            .bf2
            .iter()
            .position(|s| s.family.eq_ignore_ascii_case(family.trim()))
            .unwrap_or(0);
        let class = bank.split_once('_').map_or(bank.as_str(), |(_, n)| n);
        let find = |c: &str| self.bf2.iter().position(|(f, k)| *f == fam && k == c);
        find(class)
            .or_else(|| find(&bank))
            .or_else(|| find("rifle"))
            .unwrap_or(0)
    }

    /// The set's name, for a census.
    pub fn set_name(&self, set: usize) -> String {
        match self.game {
            Game::Bf1 => bf1::WEAPONS.get(set).copied().unwrap_or("?").to_owned(),
            Game::Bf2 => self.bf2.get(set).map_or("?".to_owned(), |(f, k)| {
                format!("{}_{k}", self.sets.bf2[*f].family)
            }),
        }
    }

    fn slot1(&self, i: Option<u16>) -> Option<Slot> {
        let c = self.sets.clips.get(usize::from(i?))?;
        let blend = c.blend.unwrap_or(0.15);
        Some(Slot {
            clip: self.map[usize::from(i?)]?,
            looping: c.looping.unwrap_or(false),
            blend_in: blend,
            blend_out: blend,
            aim: aim::AimKind::None,
            pair: None,
        })
    }

    /// BF2: layer 0 the upper slot, 1 the lower.
    fn slot2(&self, set: usize, pair: usize, layer: usize) -> Option<Slot> {
        let (f, k) = self.bf2.get(set)?;
        let i = usize::from((*self.sets.bf2[*f].sets[k].get(2 * pair + layer)?)?);
        let a = self.sets.bf2_animations.get(pair);
        let l = a.and_then(|a| if layer == 0 { &a.upper } else { &a.lower }.as_ref());
        Some(Slot {
            clip: self.map[i]?,
            looping: l.is_some_and(|l| l.looping),
            blend_in: l.map_or(0.0, |l| l.blend_in),
            blend_out: l.map_or(0.0, |l| l.blend_out),
            aim: l.map_or(aim::AimKind::None, |l| aim::AimKind::of(&l.aim)),
            pair: Some(pair),
        })
    }

    /// Has stance `s` strafe clips - a left and a right - in this set (BF1
    /// the base table's, `FUN_0022e990` B1:145010-145018; BF2 the set's own
    /// in the block `alert` picks, `FUN_0027ed18` B2:202195-202230)? The
    /// disc's stand and crouch have none.
    pub fn strafes(&self, set: usize, s: usize, alert: bool) -> bool {
        let has = |m: Move| match (self.game, &self.sets.bf1) {
            (Game::Bf1, Some(t)) => t
                .base
                .get(s)
                .and_then(|r| r.get(m.ids().0).copied().flatten())
                .is_some(),
            (Game::Bf2, _) => {
                let b = if alert { s + 3 } else { s };
                self.slot2(set, bf2::move_pair(b, m.ids().1), 1).is_some()
            }
            _ => false,
        };
        (has(Move::WalkLeft) || has(Move::RunLeft)) && (has(Move::WalkRight) || has(Move::RunRight))
    }

    /// The root speed of a movement clip along its axis (m/s), if baked.
    fn root_speed(&self, clip: usize, m: Move) -> Option<f32> {
        Some(self.clip_speed(clip)?[m.axis()])
    }

    /// An asset clip's baked root speed (`[x, z]` m/s), for a walk, run or
    /// sprint.
    pub fn clip_speed(&self, clip: usize) -> Option<[f32; 2]> {
        self.root.get(clip).copied().flatten()
    }

    /// Does the set take the alert timer? Not BF1's tool and sabre sets
    /// (`FUN_0022d150`), not BF2's tool and melee classes (§7).
    pub fn alerts(&self, set: usize) -> bool {
        match self.game {
            Game::Bf1 => !matches!(set, 4 | 5),
            Game::Bf2 => self
                .bf2
                .get(set)
                .is_none_or(|(_, k)| k != "tool" && k != "melee"),
        }
    }

    /// (upper, lower) clips of a body state with no action running; `alert`
    /// takes the alert rows where the set has them (§3.2, §7).
    pub fn body_slots(&self, set: usize, body: Body, alert: bool) -> (Option<Slot>, Option<Slot>) {
        match (self.game, &self.sets.bf1, body) {
            (Game::Bf1, Some(t), Body::Special(sp)) => {
                let Some(id) = bf1::SPECIAL.iter().position(|n| *n == sp.name()) else {
                    return (None, None);
                };
                let lower = self.slot1(t.special[id]);
                (self.slot1(t.s[set][id]).or(lower), lower)
            }
            (Game::Bf1, Some(t), Body::Feet(s, m)) => {
                // A slot the tables leave empty: the other gait, then idle.
                let ids = [m.ids().0, m.other_gait().ids().0, 0];
                // Alert rows are the standing soldier's: the legs take the
                // rifle's for idle and the two runs, the upper body the
                // weapon's.
                let row = |w: usize, id: usize| {
                    let id = (alert && s == 0).then_some(id)?;
                    self.slot1(*t.alert.get(w)?.get(id)?)
                };
                let lower = row(0, ids[0])
                    .filter(|_| matches!(ids[0], 0 | 7 | 8))
                    .or_else(|| ids.iter().find_map(|&id| self.slot1(t.base[s][id])));
                // The upper body idles through a turn (`FUN_0022f1c8`).
                let up_ids = if matches!(m, Move::TurnLeft | Move::TurnRight) {
                    [0; 3]
                } else {
                    ids
                };
                let upper = row(set, up_ids[0])
                    .or_else(|| up_ids.iter().find_map(|&id| self.slot1(t.w[s][set][id])));
                (upper.or(lower), lower)
            }
            (Game::Bf2, _, body) => {
                let pairs = match body {
                    Body::Special(sp) => [bf2::simple_pair(sp.name()), None, None, None],
                    Body::Feet(s, m) => [
                        // The alert blocks stand three after the plain.
                        alert.then(|| bf2::move_pair(s + 3, m.ids().1)),
                        Some(bf2::move_pair(s, m.ids().1)),
                        Some(bf2::move_pair(s, m.other_gait().ids().1)),
                        Some(bf2::move_pair(s, 0)),
                    ],
                };
                // A BF2 set's prone rows hold the stand idle (the resolver's
                // last fallback: the disc has no prone), so a prone soldier
                // plays the borrowed clips first: the move, its other gait,
                // the idle.
                let prone = match body {
                    Body::Feet(2, m) => [m.ids().1, m.other_gait().ids().1, 0]
                        .into_iter()
                        .find_map(|k| self.prone_slot(k, true)),
                    Body::Special(sp @ (Special::ProneDown(_) | Special::ProneUp(_))) => {
                        PRONE_CHANGES
                            .iter()
                            .position(|n| *n == sp.name())
                            .and_then(|k| self.prone_slot(bf2::MOVES.len() + 2 + k, false))
                    }
                    _ => None,
                };
                let at = |layer| {
                    prone.or_else(|| {
                        pairs
                            .iter()
                            .flatten()
                            .find_map(|&p| self.slot2(set, p, layer))
                            .or_else(|| self.slot2(set, bf2::move_pair(0, 0), layer))
                    })
                };
                (at(0), at(1))
            }
            _ => (None, None),
        }
    }

    /// (upper, lower) clips of a weapon action in stance `s`; `None` when
    /// the set has no clip for it (the action then does not run).
    pub fn action_slots(
        &self,
        set: usize,
        s: usize,
        kind: ActionKind,
    ) -> Option<(Slot, Option<Slot>)> {
        match (self.game, &self.sets.bf1) {
            (Game::Bf1, Some(t)) => {
                // BF2's throw on every map (card GTHRW): its slots' blends
                // (upper 0.15, lower 0.5; the bf2 `shoot_secondary2` pair).
                if let ActionKind::Throw(variant) = kind {
                    if let Some(clip) = self.borrowed_throw(s, variant) {
                        let up = Slot {
                            clip,
                            looping: false,
                            blend_in: 0.15,
                            blend_out: 0.15,
                            aim: aim::AimKind::None,
                            pair: None,
                        };
                        let legs = Slot {
                            blend_in: 0.5,
                            blend_out: 0.5,
                            ..up
                        };
                        return Some((up, Some(legs)));
                    }
                }
                let k = match kind {
                    ActionKind::Shoot => 1,
                    ActionKind::Throw(variant) => usize::from(variant.min(2)) + 1,
                    ActionKind::Reload => 5,
                };
                // A soldier who borrows BF2's throw keeps his primary's set
                // through it (BF2's offhand rule); a prone throw, which BF2
                // has none of, takes the grenade set's.
                let set = match kind {
                    ActionKind::Throw(_) if self.throw.iter().any(Option::is_some) => {
                        bf1::weapon_set("grenade").unwrap_or(set)
                    }
                    _ => set,
                };
                // The legs play the same clip, its blend 0.5 s longer (§3.2).
                let up = self.slot1(t.u[s][set][k])?;
                let legs = Slot {
                    blend_in: up.blend_in + 0.5,
                    ..up
                };
                Some((up, Some(legs)))
            }
            (Game::Bf2, _) => {
                let k = match kind {
                    ActionKind::Shoot => 0,
                    ActionKind::Throw(variant) => 2 + usize::from(variant > 0),
                    ActionKind::Reload => 5,
                };
                let p = bf2::action_pair(s, k);
                // A prone shot or reload: the borrowed clip (see `Rig::prone`).
                let n = bf2::MOVES.len();
                let prone = match kind {
                    ActionKind::Shoot if s == 2 => self.prone_slot(n, false),
                    ActionKind::Reload if s == 2 => self.prone_slot(n + 1, false),
                    _ => None,
                };
                if let Some(up) = prone {
                    return Some((up, Some(up)));
                }
                Some((self.slot2(set, p, 0)?, self.slot2(set, p, 1)))
            }
            _ => None,
        }
    }

    /// A still of a body state `frame` frames in, with a weapon action over
    /// it when one is named (the legs joined, as standing still): what a
    /// gallery shows of a set.
    pub fn still(
        &self,
        (clips, skel): (&[Clip], &Skeleton),
        set: usize,
        body: Body,
        action: Option<ActionKind>,
        frame: f32,
    ) -> Pose {
        let (mut up, mut low) = self.body_slots(set, body, false);
        if let (Body::Feet(s, _), Some(kind)) = (body, action) {
            if let Some((a, legs)) = self.action_slots(set, s, kind) {
                (up, low) = (Some(a), legs.or(low));
            }
        }
        let mut pose = Pose::bind(skel);
        for (slot, mask) in [(low, &self.lower), (up, &self.upper)] {
            let Some(slot) = slot else { continue };
            let sampled = clips[slot.clip].sample(skel, frame, true);
            for (b, x) in pose.0.iter_mut().enumerate() {
                if mask[b] {
                    *x = sampled.0[b];
                }
            }
        }
        pose
    }

    /// Seconds a change from `old` to `new` blends over.
    fn blend_secs(&self, old: Option<&Slot>, new: &Slot, upper: bool) -> f32 {
        if self.game == Game::Bf1 {
            return new.blend_in;
        }
        let Some(old) = old else { return 0.0 };
        let layer = |p: usize| {
            let a = self.sets.bf2_animations.get(p)?;
            if upper { &a.upper } else { &a.lower }.as_ref()
        };
        let to = old.pair.zip(new.pair).and_then(|(o, n)| {
            let name = &self.sets.bf2_animations.get(n)?.name;
            let l = layer(o)?;
            l.blend_to
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
        });
        to.map_or(new.blend_in.min(old.blend_out), |(_, t)| *t)
    }
}

/// What an update poses with.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    rig: &'a Rig,
    clips: &'a [Clip],
    skel: &'a Skeleton,
    dt: f32,
    /// BF1: the alert's factor on both layers' weight (§3.4).
    factor: f32,
    blend_updates: f32,
}

/// One clip player.
#[derive(Debug, Clone, Default)]
struct Player {
    slot: Option<Slot>,
    /// Phase in [0, 1].
    t: f32,
    /// Seconds on this clip.
    secs: f32,
    finished: bool,
    /// Seconds the change to this clip blends over.
    blend: f32,
    /// BF2: the crossfade weight already applied.
    weight: f32,
}

impl Player {
    /// Put `slot` on the player; `restart` replays a clip it already holds.
    fn set(&mut self, rig: &Rig, slot: Slot, upper: bool, restart: bool, phase: Option<f32>) {
        if !restart && self.slot.is_some_and(|s| s.clip == slot.clip) {
            return;
        }
        self.blend = rig.blend_secs(self.slot.as_ref(), &slot, upper);
        self.slot = Some(slot);
        self.t = phase.unwrap_or(0.0);
        self.secs = 0.0;
        self.finished = false;
        self.weight = 0.0;
    }

    fn advance(&mut self, clips: &[Clip], dt: f32, game: Game) {
        let Some(s) = self.slot else { return };
        let f = clips[s.clip].frames.max(2) as f32;
        let denom = match game {
            Game::Bf1 => f,
            Game::Bf2 => f - 1.0,
        };
        self.t += dt * 30.0 / denom;
        self.secs += dt.abs();
        if s.looping {
            self.t = self.t.rem_euclid(1.0);
        } else if self.t >= 1.0 {
            self.t = 1.0;
            self.finished = true;
        }
    }

    /// Write this player's clip into the masked bones of the kept pose.
    fn apply(&mut self, pose: &mut Pose, cx: &Ctx<'_>, upper: bool, snap: bool) {
        let Ctx {
            rig,
            clips,
            skel,
            dt,
            factor,
            blend_updates,
        } = *cx;
        let (mask, game) = (if upper { &rig.upper } else { &rig.lower }, rig.game);
        let Some(s) = self.slot else { return };
        let c = &clips[s.clip];
        let mut sampled = c.sample(skel, self.t * c.frames.saturating_sub(1) as f32, s.looping);
        if let (Some(true), Some((b, rest))) = (rig.flight.get(s.clip), rig.root_bone) {
            let x = &mut sampled.0[b].translation;
            (x.x, x.z) = (rest.x, rest.z);
        }
        let cur = factor
            * if self.blend > 0.0 {
                (self.secs / self.blend).min(1.0)
            } else {
                1.0
            };
        // The weight the kept pose moves toward the clip by, this update.
        let k = match game {
            _ if snap || cur >= 1.0 => 1.0,
            Game::Bf1 => 1.0 - (1.0 - cur).powf(dt * blend_updates),
            Game::Bf2 => ((cur - self.weight) / (1.0 - self.weight)).clamp(0.0, 1.0),
        };
        self.weight = cur;
        for (b, x) in pose.0.iter_mut().enumerate() {
            if !mask.get(b).copied().unwrap_or(false) {
                continue;
            }
            if k >= 1.0 {
                // An overwrite gives a bone the clip has no track for its
                // rest pose (`Clip::sample`'s).
                *x = sampled.0[b];
            } else if c.tracks.get(b).is_some_and(Option::is_some) {
                *x = x.blend(sampled.0[b], k);
            }
        }
    }
}

/// A weapon action in progress.
#[derive(Debug, Clone, Copy)]
struct Action {
    kind: ActionKind,
    upper: Slot,
    lower: Option<Slot>,
    /// The legs play it too.
    legs: bool,
    /// Playback rate (BF2 fits a reload clip to the weapon's reload time).
    rate: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Animator {
    upper: Player,
    lower: Player,
    /// The kept pose; `None` until the first update (which snaps).
    kept: Option<Pose>,
    combo_override: bool,
    combo_aim: bool,
    combo: Option<combo_pose::ComboPose>,
    hero_jump: Option<(u8, u32)>,
    jump_serial: u32,
    next_aim: Option<(aim::AimKind, f32)>,
    action: Option<Action>,
    was_reloading: bool,
    set: Option<usize>,
    /// The movement clip last chosen and BF2's speed mode ([`choose`]).
    legs: Legs,
    /// The body state of the last update.
    body: Option<Body>,
    /// The facing at the last update (rad): standing, its turn is taken off
    /// the twist.
    yaw: Option<f32>,
    /// The twist a turn on the spot takes the legs through (rad).
    turn: f32,
    /// The walk's leg angle the twist eases toward ([`choose`]'s).
    leg: f32,
    aim: aim::Aim,
    /// The root twist (rad, positive = his left) and the last travel angle.
    twist: f32,
    travel: f32,
    roll: RollClock,
    jump: jump::Jump,
    /// Seated (card VTURG): the seat clip's frame clock; the kept pose is
    /// the seat's and the twist and aim are not laid on it.
    seat: Option<f32>,
}

/// Where the soldier is in a roll, for a census and the tests.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RollClock {
    /// Updates into the roll (1 = its first); 0 out of one.
    pub updates: u32,
    /// Seconds since the last roll ended; `None` until one has.
    pub after: Option<f32>,
    /// [`RollClock::gap`] of the roll's last update, given on the one update
    /// after it.
    pub ended: Option<f32>,
    /// Travel less twist after this update of the roll.
    pub gap: f32,
}

impl Animator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Has it posed the soldier yet?
    pub fn posed(&self) -> bool {
        self.kept.is_some()
    }

    /// The clip on the upper / lower player.
    pub fn upper_clip(&self) -> Option<usize> {
        self.upper.slot.map(|s| s.clip)
    }

    pub fn lower_clip(&self) -> Option<usize> {
        self.lower.slot.map(|s| s.clip)
    }

    /// The lower player's phase in [0, 1] (a census's read of the feet).
    pub fn lower_phase(&self) -> f32 {
        self.lower.t
    }

    /// The upper player's phase in [0, 1].
    pub fn upper_phase(&self) -> f32 {
        self.upper.t
    }

    /// The alert timer and the aim pose.
    pub fn aim(&self) -> &aim::Aim {
        &self.aim
    }

    /// The root's turn about the up axis (rad, positive = his left).
    pub fn twist(&self) -> f32 {
        self.twist
    }

    /// The body state of the last update (`None` before the first).
    pub fn body(&self) -> Option<Body> {
        self.body
    }

    /// The travel angle the twist follows in a roll (the last one given).
    pub fn travel(&self) -> f32 {
        self.travel
    }

    pub fn roll(&self) -> RollClock {
        self.roll
    }

    /// The weapon action on the upper player, and whether the legs play it.
    pub fn action(&self) -> Option<(ActionKind, bool)> {
        self.action.map(|a| (a.kind, a.legs))
    }

    /// One update: pick the two clips, advance them by `dt` and write them
    /// into the kept pose.
    pub fn step(&mut self, rig: &Rig, clips: &[Clip], skel: &Skeleton, i: &AnimInput, dt: f32) {
        self.combo_override = false;
        self.combo_aim = false;
        self.seat = None;
        // The facing's turn since the last update (+ = to his left).
        let turned = self.yaw.map_or(0.0, |y| wrap(i.yaw - y));
        self.yaw = Some(i.yaw);
        // One who dies after a throw lies in its rest pose, not a death clip
        // (B1 FUN_0022a0a8: ids 0x12 / 0x13 after the thrown states).
        let stance = match (stance_of(i, rig.game), self.body) {
            (Err(Special::Dead | Special::HeroDead), Some(Body::Special(sp)))
                if sp.thrown().is_some() =>
            {
                Err(Special::Rest(sp.thrown().unwrap_or(false)))
            }
            (Ok(0), Some(Body::Special(Special::Land(b) | Special::StandUp(b)))) if i.hard_lock => {
                Err(Special::StandUp(b))
            }
            // The prone change plays its clip through the lock (issue: the
            // soldier lay or stood frozen after a short blend).
            (Ok(2), Some(Body::Feet(s @ (0 | 1), _))) if i.hard_lock => {
                Err(Special::ProneDown(s == 1))
            }
            (Ok(2), Some(Body::Special(Special::ProneDown(c)))) if i.hard_lock => {
                Err(Special::ProneDown(c))
            }
            (Ok(s @ (0 | 1)), Some(Body::Feet(2, _))) if i.hard_lock => {
                Err(Special::ProneUp(s == 1))
            }
            (Ok(_), Some(Body::Special(Special::ProneUp(c)))) if i.hard_lock => {
                Err(Special::ProneUp(c))
            }
            (s, _) => s,
        };
        let stance = self.jump.step(rig, clips, i, stance, dt);
        let stance = match (stance, self.hero_jump) {
            (Err(Special::Jump), Some((direction, _))) => Err(Special::DirectionalJump(direction)),
            _ => stance,
        };
        let scale = if i.root_scale > 0.0 {
            i.root_scale
        } else {
            1.0
        };
        if self.set != Some(i.set) {
            self.action = None;
            self.set = Some(i.set);
        }
        // The action: a shot (re)starts shoot, a reload's first update
        // starts reload; it ends with its clip, or in a special state.
        let mut restart = false;
        // A BF2 sprint is a special state on the legs, but the hands still
        // throw and shoot in it, as BF1's sprint on the stand's clips does:
        // the action takes the stand's clips and the legs keep sprinting.
        // The jet hover is the one special the PS2 draws an action over
        // (BF1 the `jal 0x22c7f0` at 0x22b214 with a2 = 1; BF2
        // `FUN_0027d9a8` from the stand's action row): the same here.
        let hands = match stance {
            Ok(s) => Some(s),
            Err(Special::Sprint | Special::JetHover) => Some(0),
            Err(_) => None,
        };
        if let Some(s) = hands {
            let kind = if let Some(variant) = i.throw {
                Some(ActionKind::Throw(variant))
            } else if i.fired {
                Some(ActionKind::Shoot)
            } else if i.reloading && !self.was_reloading {
                Some(ActionKind::Reload)
            } else {
                None
            };
            if let Some((kind, (upper, lower))) =
                kind.and_then(|k| Some((k, rig.action_slots(i.set, s, k)?)))
            {
                let secs = clips[upper.clip].frames as f32 / 30.0;
                let fit = rig.game == Game::Bf2 && kind == ActionKind::Reload;
                self.action = Some(Action {
                    kind,
                    upper,
                    lower,
                    // Set with the movement below.
                    legs: true,
                    rate: if fit && i.reload_time > 0.0 {
                        secs / i.reload_time
                    } else {
                        1.0
                    },
                });
                restart = true;
            } else if self.upper.finished
                && self
                    .action
                    .is_some_and(|a| self.upper_clip() == Some(a.upper.clip))
            {
                self.action = None;
            }
        } else {
            self.action = None;
        }
        self.was_reloading = i.reloading;
        let shot = restart && self.action.is_some_and(|a| a.kind == ActionKind::Shoot);
        let aim = aim::AimStep {
            game: rig.game,
            dt,
            arm: (i.alert || shot) && rig.alerts(i.set),
            shot,
            aim: i.aim,
            prone: stance == Ok(2),
            feet: stance.is_ok(),
        };
        let alert = self.aim.tick(&aim);

        // The movement clip, and what the twist does this update.
        let was = self.legs.mv;
        let body = match stance {
            Err(sp) => {
                self.legs = Legs::default();
                Body::Special(sp)
            }
            Ok(s) => {
                let strafe = rig.strafes(i.set, s, alert);
                let movement = if rig.game == Game::Bf1
                    && i.stance == Stance::Sprint
                    && (i.vel / scale).abs().max_element() >= IDLE
                {
                    Some((Move::RunForward, i.vel.x.atan2(i.vel.y)))
                } else {
                    choose(
                        rig.game,
                        i.vel,
                        scale,
                        strafe,
                        alert,
                        i.max_speed,
                        &mut self.legs,
                    )
                };
                let mv = match movement {
                    Some((m, leg)) => {
                        self.leg = leg;
                        m
                    }
                    None => self.turn_on_spot(rig, clips, s, turned, dt, i.mouse_aim),
                };
                self.legs.mv = mv;
                Body::Feet(s, mv)
            }
        };
        let was_body = self.body.replace(body);
        let mv = match body {
            Body::Feet(_, m) => Some(m),
            Body::Special(_) => None,
        };
        // Not a turn either: its clip stays on the legs (B1:142636).
        let still = mv == Some(Move::Idle);
        if let Some(a) = &mut self.action {
            // BF1's legs follow the action whenever he stands still; BF2's
            // leave it for good once he moves (§7).
            a.legs = match rig.game {
                Game::Bf1 => still,
                Game::Bf2 => a.legs && still,
            };
        }

        let (mut up, mut low) = rig.body_slots(i.set, body, alert);
        let mut up_action = false;
        if let Some(a) = self.action {
            up = Some(a.upper);
            up_action = true;
            if a.legs {
                low = a.lower.or(low);
            }
        }
        // A walk <-> run change of one direction keeps the legs' phase
        // (§3.2).
        let keep = match (was.dir(), mv.and_then(Move::dir)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let legs = self.action.is_some_and(|a| a.legs);
        let (combo_up, combo_low) = self.combo_slots(rig, i);
        up = combo_up.or(up);
        low = combo_low.or(low);
        let jump_restart = self
            .hero_jump
            .is_some_and(|(_, serial)| serial != self.jump_serial);
        if let Some((_, serial)) = self.hero_jump {
            self.jump_serial = serial;
        }
        if let Some(s) = low {
            let previous_lower = self.lower_clip();
            let phase = keep.then_some(self.lower.t);
            self.lower.set(
                rig,
                s,
                false,
                (restart && legs) || (jump_restart && combo_low.is_none()),
                phase,
            );
            if combo_low.is_some() {
                self.combo_transition(false, previous_lower);
            }
        }
        let previous_upper = self.upper_clip();
        if let Some(s) = up {
            // A movement clip starts on the upper body at the legs' phase.
            let phase = (!up_action && combo_up.is_none() && mv.is_some()).then_some(self.lower.t);
            self.upper.set(
                rig,
                s,
                true,
                restart || (jump_restart && combo_up.is_none()),
                phase,
            );
            if combo_up.is_some() {
                self.combo_transition(true, previous_upper);
            }
        }
        // A movement clip plays at the speed over its root speed (both
        // players: B1:142668 / 142704, B2:202353).
        let walk = match (mv, low) {
            (_, Some(l)) if body == Body::Special(Special::Sprint) && rig.game == Game::Bf2 => {
                let speed = rig.clip_speed(l.clip).map_or(11.5, |v| v[1].abs());
                i.speed() / if speed > 0.0 { speed } else { 11.5 }
            }
            (Some(m), Some(l)) if m.moving() => rig
                .root_speed(l.clip, m)
                .map_or(1.0, |v| move_rate(i.speed() / scale, v)),
            _ => 1.0,
        };
        let rate = self.action.map_or(1.0, |a| {
            if rig.game == Game::Bf1
                && a.kind == ActionKind::Throw(2)
                && !rig.is_borrowed_throw(a.upper.clip)
                && self.upper.t * clips[a.upper.clip].frames as f32 / 30.0 < BF1_THROW_WINDUP
                && i.throw_windup > 0.0
            {
                BF1_THROW_WINDUP / i.throw_windup
            } else {
                a.rate
            }
        });
        // A hard lock's clip (the stand-up, a prone change) plays at BF1's
        // rate on both games: the lock is timed by BF1's clip lengths, so
        // the animation ends as the soldier is free.
        let clock = match body {
            Body::Special(Special::StandUp(_) | Special::ProneDown(_) | Special::ProneUp(_)) => {
                Game::Bf1
            }
            _ => rig.game,
        };
        // A borrowed BF2 throw keeps BF2's clock on a BF1 rig: frame n at
        // n / 30 s, so its hand lets go at BF2's wind-up.
        let clock_of = |p: &Player| match p.slot {
            Some(s) if rig.is_borrowed_throw(s.clip) => Game::Bf2,
            _ => clock,
        };
        let (up_clock, low_clock) = (clock_of(&self.upper), clock_of(&self.lower));
        self.upper.advance(
            clips,
            dt * if combo_up.is_some() {
                1.0
            } else if up_action {
                rate
            } else {
                walk
            },
            up_clock,
        );
        self.lower.advance(
            clips,
            dt * if combo_low.is_some() {
                1.0
            } else if legs {
                rate
            } else {
                walk
            },
            low_clock,
        );
        self.combo_clock(clips, combo_up.is_some(), combo_low.is_some());

        let jump_pose = matches!(
            body,
            Body::Special(Special::Jump | Special::Fall | Special::Landing(_))
        );
        if rig.game == Game::Bf1 && jump_pose && previous_upper != self.upper_clip() {
            self.upper.secs = 0.0;
        }

        let snap = self.kept.is_none();
        let pose = self.kept.get_or_insert_with(|| Pose::bind(skel));
        let cx = Ctx {
            rig,
            clips,
            skel,
            dt,
            factor: self.jump.fade_factor()
                * if jump_pose {
                    1.0
                } else {
                    self.aim.layer_factor(rig.game)
                },
            blend_updates: if jump_pose || self.jump.fading() {
                jump::BLEND_UPDATES
            } else {
                BF1_UPDATES
            },
        };
        self.lower.apply(pose, &cx, false, snap);
        self.upper.apply(pose, &cx, true, snap);
        let (kind, blend_in) = self
            .next_aim
            .take()
            .unwrap_or_else(|| up.map_or((aim::AimKind::None, 0.0), |s| (s.aim, s.blend_in)));
        self.aim.ease(&aim, kind, blend_in);
        self.step_twist(i, body, was_body, mv, dt);
        if combo_low.is_some() && self.combo.as_ref().is_some_and(|c| c.forced()) {
            self.combo_override = true;
            self.combo_aim = kind != aim::AimKind::None;
        }
    }

    /// The pose the two players keep, without the aim's turn.
    pub fn kept_pose(&self, skel: &Skeleton) -> Pose {
        self.kept.clone().unwrap_or_else(|| Pose::bind(skel))
    }

    /// One update in a seat (card VTURG, `analysis/re-turret-draw.md` 3.3
    /// [O]): the seat's clip played - or, for a nine-pose seat, frame `a`
    /// blended toward frame `b` by `c` - written over the kept pose (BF1
    /// `FUN_0022aa10` B1:142393-142426, BF2 `FUN_0027caf0`
    /// B2:200931-200943). No action, aim or twist rides on it. Ours
    /// (PROVISIONAL, card GCL16): the PS2's blend into and out of the seat is unread; the pose snaps in,
    /// and the first [`Animator::step`] after it blends out from it.
    pub fn seat(
        &mut self,
        clips: &[Clip],
        skel: &Skeleton,
        clip: usize,
        nine: Option<crate::viewer::turret_pose::NinePose>,
        dt: f32,
    ) {
        let Some(c) = clips.get(clip) else { return };
        let t = self.seat.map_or(0.0, |t| t + dt * 30.0);
        self.seat = Some(t);
        self.kept = Some(match nine {
            Some(n) => c
                .sample(skel, n.a, false)
                .blend(&c.sample(skel, n.b, false), n.c),
            None => c.sample(skel, t, true),
        });
        self.action = None;
        // Seated updates replace foot motion; the next step starts its history here.
        self.yaw = None;
        self.legs = Legs::default();
        self.twist = 0.0;
        self.aim = aim::Aim::default();
        self.jump = jump::Jump::default();
    }

    /// Is the soldier posed by a seat ([`Animator::seat`])?
    pub fn seated(&self) -> bool {
        self.seat.is_some()
    }

    /// The kept pose with the root twisted and the torso or the head turned
    /// onto the aim (a seated soldier's: the seat's pose as it is).
    pub fn pose(&self, skel: &Skeleton) -> Pose {
        let mut pose = self.kept_pose(skel);
        if self.seated() {
            return pose;
        }
        if self.combo_override {
            if self.combo_aim {
                self.aim.apply(&mut pose, skel, 0.0);
            }
            return pose;
        }
        if let Some(r) = skel.index_of("DummyRoot").and_then(|b| pose.0.get_mut(b)) {
            // About the model's up axis: the root has no parent.
            r.rotation = glam::Quat::from_rotation_y(self.twist) * r.rotation;
        }
        self.aim.apply(&mut pose, skel, self.twist);
        pose
    }
}

/// Clip index by name.
pub fn find_clip(clips: &[Clip], name: &str) -> Option<usize> {
    // BF2's full-body clips carry `_full` on the disc, and are baked so.
    clips.iter().position(|c| c.name == name).or_else(|| {
        clips
            .iter()
            .position(|c| c.name.strip_suffix("_full") == Some(name))
    })
}

pub mod aim;
pub mod census;
mod combo_pose;
pub mod custom;
mod jump;
mod offhand_pose;
#[cfg(test)]
mod tests;
mod turn;
