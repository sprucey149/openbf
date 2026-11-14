//! A seat's guns (card VTUR, `analysis/re-vehicles.md` §5, §9 [O]): the
//! weapons a MountedTurret section — or the pilot's own seat — fires, where
//! the bolt starts, and the aimer that turns the gun.
//!
//! - **Sections are gunner seats** (§5): TURRET1..8 each allocate a
//!   MountedTurret with up to 4 weapons (`WeaponName`, `WeaponAmmo` < 1 =
//!   infinite), its `YawLimits` / `PitchLimits` (degrees → radians) and
//!   turn rates; the BODY's props are the pilot seat's.
//! - **The aimer** (§9, B1 FUN_0026e260; defaults B1:192359-192366): pitch
//!   ±45°, yaw ±30°, PitchRate 1.0, YawRate 2.0 rad/s. It turns toward a
//!   world point at ≤ rate·dt per axis (B1:191984-192000) and is clamped to
//!   its limits (B1:192137). A vehicle seat casts a 512 m ray along its
//!   camera's aim (hover B1:98294-98302, MountedTurret B1:138065) and aims
//!   at the hit — [`AIM_RAY`].
//! - **The hover pilot's aim** (re-hover §5.1 I7-I11): aimPitch moves with
//!   the look stick at PitchRate and is clamped to PitchLimits; aimYaw is
//!   clamped to the class's +0x690/+0x694, which neither setter writes (0/0)
//!   — the pilot aims straight ahead, [`GunSetup::pilot`].
//!
//! Angles are in the vehicle's MODEL frame (+y up, +z forward): yaw turns
//! +z toward +x — the model's LEFT, where BF1's skiff carries `hp_firel`
//! and TURRET1 authors `YawLimits = 65.0 115.0` — and pitch > 0 looks up.
//! A stick pushed right lowers yaw, as it does a soldier's (`soldier.rs`).
//!
//! - **A building turret's turn law** (card VTURL, `analysis/re-turret-draw.md`
//!   2.2, 2.3 [O]; [`Turn`]): the stick drives a RATE, `-stick x
//!   MaxTurnSpeed` / `MaxPitchSpeed`, which the rate approaches through
//!   `TurnFilter` / `PitchFilter` (BF1 exponentially, BF2 linearly), and
//!   the look moves by the rate inside `YawLimits` / `PitchLimits` - limits
//!   that span the full circle (and the class default, ±10 RAW) WRAP at
//!   ±180°. The aimer chases the aim point in the turret's AIM FRAME (the
//!   look's own basis) at `AimerYawRate` / `AimerPitchRate`, inside
//!   `AimerYawLimits` / `AimerPitchLimits` ([`Aimer::chase`]); a class with
//!   no `TurretNodeName` aims in the model frame. It lays the BARREL's line
//!   on the aim point (card TURR, PS2 RAM: Mygeeto's settled aim point
//!   0.004 m off the barrels' line, 3.5 m off the pivot's), and the camera
//!   ray that finds the point passes the turret's own building (B1
//!   `FUN_00223850` B1:138064, BF2 `FUN_0026cea0` B2:189959): `Sim::seat_gun`.
//!   The barrel hangs at its bind offset ([`GunSetup::muzzle`]).
//!

use crate::fmath;
use crate::fmath::FloatExt;
use crate::weapon::WeaponState;
use glam::Vec3;

/// The camera ray's reach (B1:98294-98302, B1:138065).
pub const AIM_RAY: f32 = 512.0;
/// Most weapons one seat carries (the MountedTurret setter's four).
pub const MAX_SEAT_WEAPONS: usize = 4;

/// An aimer's limits (radians, model frame) and rates (rad/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aimer {
    pub yaw: [f32; 2],
    pub pitch: [f32; 2],
    pub yaw_rate: f32,
    pub pitch_rate: f32,
}

impl Default for Aimer {
    /// B1 FUN_0026e260's constructor (B1:192359-192366).
    fn default() -> Self {
        let d = std::f32::consts::PI / 180.0;
        Self {
            yaw: [-30.0 * d, 30.0 * d],
            pitch: [-45.0 * d, 45.0 * d],
            yaw_rate: 2.0,
            pitch_rate: 1.0,
        }
    }
}

impl Aimer {
    /// One tick toward (`yaw`, `pitch`) from (`cur_yaw`, `cur_pitch`): each
    /// axis moves at most rate·dt, then both are clamped to the limits.
    pub fn turn(&self, cur: (f32, f32), want: (f32, f32), dt: f32) -> (f32, f32) {
        let step = |c: f32, w: f32, r: f32| c + (w - c).clamp(-r * dt, r * dt);
        let y = step(cur.0, want.0, self.yaw_rate).clamp(self.yaw[0], self.yaw[1]);
        let p = step(cur.1, want.1, self.pitch_rate).clamp(self.pitch[0], self.pitch[1]);
        (y, p)
    }

    /// The limits' middle — where a seat's gun rests before anyone aims it.
    pub fn rest(&self) -> (f32, f32) {
        (mid(self.yaw), mid(self.pitch))
    }

    /// One update of the PS2's aimer (B1 `FUN_0026d748` B1:191984-192005,
    /// BF2 MIPS 0x30269c-0x302754; re-turret-draw 2.3 [O]) from (`cur`
    /// yaw, pitch) toward the unit direction `want`, both in the aimer's
    /// base frame: `c = dir x want`, yaw moves by `c.y` and pitch by `-c.x`
    /// (pitch + = up), each at most rate·dt, then the limits. No angle is
    /// differenced, so nothing turns the long way round.
    pub fn chase(&self, cur: (f32, f32), want: Vec3, dt: f32) -> (f32, f32) {
        let c = dir(cur.0, cur.1).cross(want);
        let (ry, rp) = (self.yaw_rate * dt, self.pitch_rate * dt);
        let y = (cur.0 + c.y.clamp(-ry, ry)).clamp(self.yaw[0], self.yaw[1]);
        let p = (cur.1 - c.x.clamp(-rp, rp)).clamp(self.pitch[0], self.pitch[1]);
        (y, p)
    }
}

fn mid(l: [f32; 2]) -> f32 {
    if l[0] <= 0.0 && l[1] >= 0.0 {
        0.0
    } else {
        0.5 * (l[0] + l[1])
    }
}

/// How a turret's stick rate settles on its target (re-turret-draw 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slew {
    /// BF1: `rate += (target - rate) x min(filter·dt, 1)` (B1:137456-137466).
    Exp,
    /// BF2: `rate` steps by filter·dt toward the target, never past it
    /// (B2:189229-189262).
    Linear,
}

/// The PS2 MountedTurret's stick-to-rate law (card VTURL; BF1
/// `FUN_00223850` B1:137432-137520, BF2 `FUN_0026cea0` B2:189204-189385;
/// re-turret-draw 2.2 [O]). Angles are the port's look: model frame,
/// pitch positive UP - the PS2's turret pitch is positive nose DOWN, so its
/// `PitchLimits` arrive negated and swapped ([`Turn::from_class`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    /// Yaw and pitch limits (radians), each a clamp unless its `wrap`.
    pub yaw: [f32; 2],
    pub pitch: [f32; 2],
    /// Limits that span the circle: the angle wraps into ±180° instead.
    pub yaw_wrap: bool,
    pub yaw_positive: bool,
    pub pitch_wrap: bool,
    /// The rate at full stick (rad/s): `MaxTurnSpeed` / `TurnRate`,
    /// `MaxPitchSpeed` / `PitchRate`.
    pub yaw_speed: f32,
    pub pitch_speed: f32,
    /// `TurnFilter` / `PitchFilter`.
    pub yaw_filter: f32,
    pub pitch_filter: f32,
    pub slew: Slew,
}

/// The limits the PS2's tick clamps to; outside them it wraps
/// (B2:189323-189385, B1:137478-137520).
const CLAMP_LO: f32 = -3.1416926;
const CLAMP_HI: f32 = 3.1414928;

/// A turret class's defaults (B1 `FUN_00225cd8` B1:138857-138903, BF2
/// `FUN_00270540` B2:191365-191402): limits ±10 RAW (= wrap), turn speed
/// 2.0, pitch speed 0.5, both filters 10.
pub const TURN_SPEED: f32 = 2.0;
pub const PITCH_SPEED: f32 = 0.5;
pub const TURN_FILTER: f32 = 10.0;

impl Turn {
    /// From a class's authored values in the PS2's own terms: limits in
    /// degrees (turret pitch + = nose down; `None` = the RAW ±10 default),
    /// speeds and filters as authored.
    pub fn from_class(
        yaw_deg: Option<[f32; 2]>,
        pitch_deg: Option<[f32; 2]>,
        speed: (f32, f32),
        filter: (f32, f32),
        slew: Slew,
    ) -> Self {
        // The setter's degrees-to-radians constant (B2:191590, 191619).
        let rad = |l: Option<[f32; 2]>| {
            l.map_or([-10.0, 10.0], |l| [l[0] * 0.017453294, l[1] * 0.017453294])
        };
        let (y, p) = (rad(yaw_deg), rad(pitch_deg));
        let wraps = |l: [f32; 2]| !(CLAMP_LO < l[0] && l[1] < CLAMP_HI);
        let yaw_positive = slew == Slew::Linear
            && y[0] > 0.0
            && y[0] <= std::f32::consts::PI
            && y[1] >= std::f32::consts::PI
            && y[1] < std::f32::consts::TAU;
        Self {
            yaw: y,
            pitch: [-p[1], -p[0]],
            yaw_wrap: !yaw_positive && wraps(y),
            yaw_positive,
            pitch_wrap: wraps(p),
            yaw_speed: speed.0,
            pitch_speed: speed.1,
            yaw_filter: filter.0,
            pitch_filter: filter.1,
            slew,
        }
    }

    /// Where the look rests: 0 when the limits hold it, else their middle.
    pub fn rest(&self) -> (f32, f32) {
        let r = |l: [f32; 2], w: bool| if w { 0.0 } else { mid(l) };
        (r(self.yaw, self.yaw_wrap), r(self.pitch, self.pitch_wrap))
    }

    /// One update: the sticks (`rx` right +, `ry` up +) set the target
    /// rates (yaw `-rx x speed`: a stick pushed right lowers yaw; pitch
    /// `ry x speed`), the rates slew toward them, the look moves by the NEW
    /// rate and is clamped or wrapped. Returns (look, rate).
    pub fn step(
        &self,
        look: (f32, f32),
        rate: (f32, f32),
        stick: (f32, f32),
        dt: f32,
    ) -> ((f32, f32), (f32, f32)) {
        let slew = |r: f32, t: f32, f: f32| match self.slew {
            Slew::Exp => r + (t - r) * (f * dt).fmin(1.0),
            Slew::Linear => super::hover::slew(r, t, f, dt),
        };
        let ry = slew(rate.0, -stick.0 * self.yaw_speed, self.yaw_filter);
        let rp = slew(rate.1, stick.1 * self.pitch_speed, self.pitch_filter);
        ((self.hold(look.0 + dt * ry, look.1 + dt * rp)), (ry, rp))
    }

    pub fn target(&self, yaw: f32, pitch: f32) -> (f32, f32) {
        let yaw = if self.yaw_positive && yaw < 0.0 {
            yaw + std::f32::consts::TAU
        } else {
            yaw
        };
        self.hold(yaw, pitch)
    }

    pub fn hold(&self, yaw: f32, pitch: f32) -> (f32, f32) {
        let hold = |a: f32, l: [f32; 2], w: bool| if w { wrap(a) } else { a.clamp(l[0], l[1]) };
        (
            hold(yaw, self.yaw, self.yaw_wrap),
            hold(pitch, self.pitch, self.pitch_wrap),
        )
    }
}

/// An angle into [-π, π] by whole turns, as the PS2's loops do.
pub fn wrap(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// The turret's aim frame in the model frame (re-turret-draw 2.2: the look's
/// basis, `X = up x D`, `Y = D x X`, `Z = D`), as its three axes.
pub fn frame(look: (f32, f32)) -> [Vec3; 3] {
    let z = dir(look.0, look.1);
    let (s, c) = fmath::sin_cos(look.0);
    let x = Vec3::new(c, 0.0, -s);
    [x, z.cross(x), z]
}

/// The model-frame (yaw, pitch) of the aimer at `rel` in the aim frame of
/// `look`.
pub fn compose(look: (f32, f32), rel: (f32, f32)) -> (f32, f32) {
    let [x, y, z] = frame(look);
    let d = dir(rel.0, rel.1);
    angles(x * d.x + y * d.y + z * d.z)
}

/// One weapon of a seat: its `Sim::weapon_classes` index and ammo.
#[derive(Clone, Debug, PartialEq)]
pub struct GunWeapon {
    pub class: usize,
    /// `WeaponAmmo` < 1: never runs dry.
    pub infinite: bool,
    /// Otherwise the spare clips (`WeaponState::new`'s).
    pub reserve_clips: u32,
    pub fire_points: Vec<Vec3>,
    /// Each fire point's aim limits, model frame: [yaw, pitch] in radians.
    pub fire_limits: Vec<[[f32; 2]; 2]>,
    /// Each fire point's own aimer node, model space: the PS2 turns a
    /// barrel with the aimer that carries it (`analysis/re-turret-draw.md`
    /// 2.3), so the AT-TE's rear barrels swing about `aimer_Rgun_1` and
    /// `aimer_Rgun_2`, not about each other. Empty = the seat's pivot.
    pub fire_pivots: Vec<Vec3>,
}

/// A seat's guns — SETUP data per class, like `SeatSetup`, NOT hashed.
#[derive(Clone, Debug, PartialEq)]
pub struct GunSetup {
    /// ≤ [`MAX_SEAT_WEAPONS`], in ODF order; FIRE fires the first and
    /// SECONDARY_FIRE the second (a feel call, `FEEL_NOTES.md`).
    pub weapons: Vec<GunWeapon>,
    /// `FirePointName`'s bind origin, model space.
    pub fire_point: Vec3,
    /// The aimer's pivot, model space (`TurretNodeName`'s node, else the
    /// fire point).
    pub pivot: Vec3,
    /// `EyePointOffset` (x y z) from the pivot.
    pub eye: Vec3,
    pub aimer: Aimer,
    /// The hover pilot's seat: yaw held at 0 (re-hover §5.1 I11).
    pub pilot: bool,
    /// A building turret's turn law (card VTURL); `None` = the port's
    /// PROVISIONAL look (vehicles, the pilot).
    pub turn: Option<Turn>,
    /// Under [`GunSetup::turn`]: the aimer rides the turret's aim frame (the
    /// class has a `TurretNodeName`, B1:136779-136783, B2:188004-188011);
    /// else its base is the model frame.
    pub framed: bool,
    pub aimer_bind: glam::Quat,
    /// A pilot seat's weapon aimer when the seat authors one (`AimerPitchLimits`
    /// / `AimerYawLimits` / rates of its first aimer): the guns chase the look's
    /// aim point inside these limits while the look itself keeps [`Self::aimer`]
    /// (`PitchLimits`, re-hover §5.1 I11). The IFT-X looks over -8..25 deg and
    /// its cannons turn over -30..90 deg (`analysis/re-hover-camera.md`).
    pub weapon_aimer: Option<Aimer>,
    /// The seat looks along its owner's eye (BF2 key 0x865061fc, K+0x84c):
    /// the turret tick takes the owner's E+0x350 direction into model space
    /// instead of its own angles (B2 `FUN_0026cea0` B2:188899-188939). PS2
    /// RAM (BF2 AT-ST, passenger seated): that direction is the walker's aim
    /// (E+0x50c, E+0x510) to 1e-6 rad.
    pub follows_owner: bool,
    /// A building turret's drawn gunner: where his body stands
    /// ([`GunnerSeat`]); `None` = at the model's origin.
    pub gunner: Option<GunnerSeat>,
}

/// Where a building turret's drawn gunner stands (`analysis/re-turret-draw.md`
/// 3.2, `re-turret-collision.md` 3.2 [O]): the PS2 puts his body at `L x E`
/// in the model frame - `L` the `PilotPosition` hardpoint's bind relative to
/// its parent node, `E` that node now. A turret-node parent turns with the
/// look; any other parent is taken at its bind (PROVISIONAL for the aimer
/// node's seats).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunnerSeat {
    /// The parent node's bind, model space.
    pub node_bind: glam::Mat4,
    /// `L`: the hardpoint relative to the parent.
    pub local: glam::Mat4,
    /// The parent is the turret node, posed by the look.
    pub turns: bool,
    /// `PitchTurnFactor` of the turret node's pitch.
    pub pitch_factor: f32,
}

impl GunnerSeat {
    /// His drawn matrix `E x L` in the model frame with the seat's camera
    /// at `look`; the soldier model faces its +Z.
    pub fn frame(&self, look: (f32, f32)) -> glam::Mat4 {
        let e = if self.turns {
            turret_node(&self.node_bind, look.0, -look.1 * self.pitch_factor)
        } else {
            self.node_bind
        };
        e * self.local
    }

    /// His feet in the model frame with the seat's camera at `look`.
    pub fn feet(&self, look: (f32, f32)) -> Vec3 {
        self.frame(look).w_axis.truncate()
    }
}

/// The turret node's model-space matrix with the look at (`yaw`,
/// `pitch_down`): +Z along the look, about the bind's up, at the bind's
/// origin (`analysis/re-turret-draw.md` 2.2).
pub fn turret_node(bind: &glam::Mat4, yaw: f32, pitch_down: f32) -> glam::Mat4 {
    let (sa, ca) = fmath::sin_cos(yaw);
    let (sb, cb) = fmath::sin_cos(pitch_down);
    let dir = Vec3::new(sa * cb, -sb, ca * cb);
    let z = dir.normalize();
    let x = bind.y_axis.truncate().cross(dir).normalize();
    let y = z.cross(x);
    glam::Mat4::from_cols(
        x.extend(0.0),
        y.extend(0.0),
        z.extend(0.0),
        bind.w_axis.truncate().extend(1.0),
    )
}

/// X = norm(up × dir), Z = norm(dir), Y = Z × X, at `origin`.
fn look_along(dir: Vec3, up: Vec3, origin: Vec3) -> glam::Mat4 {
    let z = dir.normalize();
    let x = up.cross(dir).normalize();
    let y = z.cross(x);
    glam::Mat4::from_cols(
        x.extend(0.0),
        y.extend(0.0),
        z.extend(0.0),
        origin.extend(1.0),
    )
}

/// The `AimerNodeName` node (`analysis/re-turret-draw.md` 2.3): `local` is
/// its bind matrix relative to its parent, `parent` the parent's matrix now,
/// `dir` the aimer's direction in the building's model space.
pub fn aimer_node(parent: &glam::Mat4, local: &glam::Mat4, dir: Vec3) -> glam::Mat4 {
    let carried = *parent * *local;
    look_along(dir, carried.y_axis.truncate(), carried.w_axis.truncate())
}

/// A barrel node: `parent` its parent's matrix now, `offset` its bind
/// origin relative to that parent, `slide` = `state × BarrelRecoil` (the
/// caller passes 0 while the state is not above 0).
pub fn barrel_node(parent: &glam::Mat4, offset: Vec3, slide: f32) -> glam::Mat4 {
    let mut m = *parent;
    m.w_axis = parent
        .transform_point3(offset - Vec3::new(0.0, 0.0, slide))
        .extend(1.0);
    m
}

/// A building turret's node rig and the nodes its gunner seat moves
/// (card VTURD, `analysis/re-turret-draw.md` 2): what the draw poses its
/// parts by and the sim poses its collision prims by (card TCOL,
/// `re-turret-collision.md` 1.6). Matrices are glam's, model space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rig {
    /// Each node's model-space bind matrix.
    pub binds: Vec<glam::Mat4>,
    pub parents: Vec<Option<usize>>,
    /// The `TurretNodeName` node and its `PitchTurnFactor`.
    pub turret: Option<usize>,
    pub pitch_factor: f32,
    pub aimer: Option<usize>,
    /// (node, `BarrelRecoil`).
    pub barrels: Vec<(usize, f32)>,
}

impl Rig {
    /// Every node's model-space matrix with the seat's camera at `look` and
    /// its aimer at `aim` (the port's angles: yaw turns +z toward +x,
    /// positive pitch is up) and each barrel's recoil state in `recoil` (missing = at
    /// rest). A node nobody writes stays at its bind; nothing composes
    /// parents but the aimer's and the barrels' own carry (re-turret-draw 2.4).
    pub fn pose(&self, look: (f32, f32), aim: (f32, f32), recoil: &[f32]) -> Vec<glam::Mat4> {
        let bind = &self.binds;
        let mut now = bind.clone();
        let parent = |n: usize| self.parents.get(n).copied().flatten();
        let local = |n: usize| match parent(n) {
            Some(p) => bind[p].inverse() * bind[n],
            None => bind[n],
        };
        if let Some(t) = self.turret {
            now[t] = turret_node(&bind[t], look.0, -look.1 * self.pitch_factor);
        }
        if let Some(a) = self.aimer {
            let p = parent(a).map_or(glam::Mat4::IDENTITY, |p| now[p]);
            now[a] = aimer_node(&p, &local(a), dir(aim.0, aim.1));
        }
        for (i, &(b, r)) in self.barrels.iter().enumerate() {
            let p = parent(b).map_or(glam::Mat4::IDENTITY, |p| now[p]);
            let state = recoil.get(i).copied().unwrap_or(0.0);
            let slide = if state > 0.0 { state * r } else { 0.0 };
            now[b] = barrel_node(&p, local(b).w_axis.truncate(), slide);
        }
        now
    }

    /// Does the seat move node `n` (the turret, aimer or a barrel node)?
    pub fn moves(&self, n: usize) -> bool {
        self.turret == Some(n) || self.aimer == Some(n) || self.barrels.iter().any(|b| b.0 == n)
    }
}

impl GunSetup {
    /// The yaw the barrel has in the bind pose — the fire point's heading
    /// from the pivot (BF1's skiff: 0 for the nose gun, about +90° for
    /// `gunl`). 0 when the fire point sits on the pivot's vertical.
    pub fn bind_yaw(&self) -> f32 {
        let d = self.fire_point - self.pivot;
        if d.x * d.x + d.z * d.z < 1e-6 {
            0.0
        } else {
            fmath::atan2(d.x, d.z)
        }
    }

    /// A building turret's zoomed eye, model space (card VTURZ,
    /// re-turret-draw 4 [O]): `EyePointOffset` on the aim frame of `look`
    /// (its yaw AND pitch, [`frame`]) from the pivot - where the PS2's
    /// zoomed camera stands (0.00 m from its camera on every zoomed row of
    /// `lead_tmp/vturd/ps2/{nab1_gty,myg1_zoom}`, `lead_tmp/vturz/mk_zoom_pairs.py`).
    pub fn scope_eye(&self, look: (f32, f32)) -> Vec3 {
        let [x, y, z] = frame(look);
        self.pivot + x * self.eye.x + y * self.eye.y + z * self.eye.z
    }

    /// Where the seat's camera ray leaves, model space: the zoomed eye while
    /// the seat is zoomed (card VTURZ: the shot goes where the scope's
    /// centre looks); a pilot's eye E+0x340, the BODY's `EyePointOffset` on
    /// the vehicle's matrix, not turned by the aim (hover B2:132251, walker
    /// B2 FUN_00261208, both `FUN_001ce880` on E+0x120); else the port's
    /// PROVISIONAL eye - `EyePointOffset` turned by the look's yaw alone.
    pub fn ray_eye(&self, look: (f32, f32), zoomed: bool) -> Vec3 {
        if zoomed {
            self.scope_eye(look)
        } else if self.pilot {
            self.pivot + self.eye
        } else {
            self.pivot + turned(self.eye, Vec3::ZERO, look.0, 0.0)
        }
    }

    /// The gun at rest: (look, the aimer's own angles, aim - model frame).
    pub fn rest(&self) -> ((f32, f32), (f32, f32), (f32, f32)) {
        let rel = self.aimer.rest();
        match self.turn {
            Some(t) => {
                let look = t.rest();
                (look, rel, self.aim_of(look, rel))
            }
            None => (rel, rel, rel),
        }
    }

    /// The look's yaw rate at full stick.
    pub fn yaw_speed(&self) -> f32 {
        self.turn.map_or(self.aimer.yaw_rate, |t| t.yaw_speed)
    }

    /// The aimer's model-frame (yaw, pitch) at `rel` with the look at `look`.
    pub fn aim_of(&self, look: (f32, f32), rel: (f32, f32)) -> (f32, f32) {
        if self.turn.is_some() && self.framed {
            compose(look, rel)
        } else if self.aimer_bind != glam::Quat::IDENTITY {
            angles(self.aimer_bind * dir(rel.0, rel.1))
        } else {
            rel
        }
    }

    /// The fire point (model space) with the aimer at `aim`: the bind-pose
    /// barrel, turned back to rest along +z, then pitched and yawed. A
    /// building turret's barrel rests along +z already, wherever it sits:
    /// the PS2 hangs each barrel off the aimer node at its bind offset
    /// (re-turret-draw 2.3, A3 of `lead_tmp/vturd/replay_aimer.py`), so its
    /// fire point is turned as it is (card TURR: Mygeeto's barrel 1.28 m
    /// to the side was moved onto the axis, 0.9 m off at 20 m).
    pub fn muzzle(&self, aim: (f32, f32)) -> Vec3 {
        self.muzzle_from(self.fire_point, aim)
    }

    /// [`GunSetup::muzzle`] for another of the seat's bind-pose points.
    pub fn muzzle_from(&self, point: Vec3, aim: (f32, f32)) -> Vec3 {
        self.muzzle_about(point, self.pivot, aim)
    }

    /// [`GunSetup::muzzle_from`] for a barrel turned about its own aimer's
    /// `pivot`.
    pub fn muzzle_about(&self, point: Vec3, pivot: Vec3, aim: (f32, f32)) -> Vec3 {
        let rest = if self.aimer_bind != glam::Quat::IDENTITY {
            pivot + self.aimer_bind.inverse() * (point - pivot)
        } else if self.turn.is_some() {
            point
        } else {
            turned(point, pivot, -self.bind_yaw(), 0.0)
        };
        turned(rest, pivot, aim.0, aim.1)
    }

    /// The barrel of `weapon`'s fire ring its next shot leaves from, after
    /// `shots_fired` shots (re-blaster-damage 3.1).
    pub fn next_point(&self, weapon: usize, shots_fired: u64) -> Option<usize> {
        let n = self.weapons.get(weapon)?.fire_points.len() as u64;
        (n > 0).then(|| (shots_fired % n) as usize)
    }

    /// Where a weapon's shot leaves, model space: the barrel `point` of its
    /// fire ring (`GunWeapon::fire_points`) turned with the aim, else the
    /// seat's fire point. A flyer pilot's guns do not turn: his points stay
    /// where the model binds them.
    pub fn shot_point(
        &self,
        weapon: usize,
        point: Option<usize>,
        aim: (f32, f32),
        fixed: bool,
    ) -> Vec3 {
        let w = self.weapons.get(weapon);
        match point.and_then(|p| Some((*w?.fire_points.get(p)?, p))) {
            Some((p, _)) if fixed => p,
            Some((p, k)) => match w.and_then(|w| w.fire_pivots.get(k)) {
                Some(&pivot) => self.muzzle_about(p, pivot, aim),
                None => self.muzzle_from(p, aim),
            },
            None => self.muzzle(aim),
        }
    }
}

/// Model-frame direction of (yaw, pitch).
pub fn dir(yaw: f32, pitch: f32) -> Vec3 {
    let (sy, cy) = fmath::sin_cos(yaw);
    let (sp, cp) = fmath::sin_cos(pitch);
    Vec3::new(sy * cp, sp, cy * cp)
}

/// (yaw, pitch) of a model-frame direction.
pub fn angles(d: Vec3) -> (f32, f32) {
    let h = (d.x * d.x + d.z * d.z).sqrt();
    (fmath::atan2(d.x, d.z), fmath::atan2(d.y, h))
}

/// Turn `p` (model space) about `pivot` by (yaw, pitch): pitch about the
/// model x axis, then yaw about y — a gun resting along +z.
pub fn turned(p: Vec3, pivot: Vec3, yaw: f32, pitch: f32) -> Vec3 {
    let r = p - pivot;
    let (sp, cp) = fmath::sin_cos(pitch);
    // Pitch up lifts +z toward +y.
    let r = Vec3::new(r.x, r.y * cp + r.z * sp, -r.y * sp + r.z * cp);
    let (sy, cy) = fmath::sin_cos(yaw);
    // Yaw turns +z toward +x.
    let r = Vec3::new(r.x * cy + r.z * sy, r.y, -r.x * sy + r.z * cy);
    pivot + r
}

/// A seat's gun at run time — hashed.
#[derive(Clone, Debug)]
pub struct GunState {
    /// The camera's (yaw, pitch), model frame.
    pub look: (f32, f32),
    /// The aimer's (yaw, pitch), model frame.
    pub aim: (f32, f32),
    /// Under a [`Turn`]: the look's rates (rad/s) and the aimer's own
    /// angles in its base frame ([`GunSetup::framed`]); else 0 and `aim`.
    pub rate: (f32, f32),
    pub rel: (f32, f32),
    /// One per [`GunSetup::weapons`].
    pub weapons: Vec<WeaponState>,
    /// A building turret's zoom (card VTURZ, `analysis/re-turret-draw.md`
    /// 4 [O]): the first weapon's zoom value while the turret's zoom flag
    /// is set (B1 T+0x30, B2 T+0x180), `None` unzoomed. A human gunner's R3
    /// steps it through the soldier's toggle; a pilot change clears it.
    pub zoom: Option<f32>,
}

impl GunState {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [self.look.0, self.look.1, self.aim.0, self.aim.1] {
            eat(&v.hash_bits().to_le_bytes());
        }
        // Hashed only off their no-[`Turn`] values, so a vehicle's gun
        // hashes as before card VTURL.
        if self.rate != (0.0, 0.0) || self.rel != self.aim {
            for v in [self.rate.0, self.rate.1, self.rel.0, self.rel.1] {
                eat(&v.hash_bits().to_le_bytes());
            }
        }
        eat(&[self.weapons.len() as u8]);
        for w in &self.weapons {
            w.hash_into(eat);
        }
        // Hashed only when set, so an unzoomed gun hashes as before VTURZ.
        if let Some(z) = self.zoom {
            eat(&[0x5a]);
            eat(&z.hash_bits().to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEG: f32 = std::f32::consts::PI / 180.0;

    #[test]
    fn the_aimer_is_rate_limited_then_clamped() {
        let a = Aimer::default();
        let dt = 1.0 / 30.0;
        let (y, p) = a.turn((0.0, 0.0), (1.0, -1.0), dt);
        assert!((y - 2.0 * dt).abs() < 1e-6, "yaw at YawRate: {y}");
        assert!((p + 1.0 * dt).abs() < 1e-6, "pitch at PitchRate: {p}");
        // Far past the limit, many ticks: held at ±30° / ±45°.
        let mut s = (0.0, 0.0);
        for _ in 0..100 {
            s = a.turn(s, (3.0, 3.0), dt);
        }
        assert_eq!(s, (a.yaw[1], a.pitch[1]));
    }

    #[test]
    fn a_side_guns_rest_is_the_middle_of_its_arc() {
        let d = std::f32::consts::PI / 180.0;
        let a = Aimer {
            yaw: [65.0 * d, 115.0 * d],
            ..Aimer::default()
        };
        assert!((a.rest().0 - 90.0 * d).abs() < 1e-5);
        assert_eq!(Aimer::default().rest(), (0.0, 0.0));
    }

    #[test]
    fn yaw_turns_forward_toward_plus_x_and_pitch_lifts_it() {
        let q = std::f32::consts::FRAC_PI_2;
        assert!(dir(q, 0.0).distance(Vec3::X) < 1e-5);
        assert!(dir(0.0, q).distance(Vec3::Y) < 1e-5);
        let (y, p) = angles(dir(0.4, -0.3));
        assert!((y - 0.4).abs() < 1e-5 && (p + 0.3).abs() < 1e-5);
        // A fire point 1 m ahead of its pivot follows dir().
        let piv = Vec3::new(1.0, 2.0, 3.0);
        let t = turned(piv + Vec3::Z, piv, 0.4, -0.3);
        assert!((t - piv).distance(dir(0.4, -0.3)) < 1e-5);
    }

    /// BF1's skiff `gunl` (the baked nodes, rounded): its barrel points +x
    /// in the bind pose, so aimed at its bind yaw the fire point stays put,
    /// and aimed at 65° it swings forward, not behind.
    #[test]
    fn a_side_barrel_turns_from_its_bind_pose() {
        let g = GunSetup {
            weapons: Vec::new(),
            fire_point: Vec3::new(2.334, 0.911, 0.006),
            pivot: Vec3::new(1.392, 0.815, -0.001),
            eye: Vec3::ZERO,
            aimer: Aimer::default(),
            pilot: false,
            turn: None,
            framed: false,
            aimer_bind: glam::Quat::IDENTITY,
            weapon_aimer: None,
            follows_owner: false,
            gunner: None,
        };
        let b = g.bind_yaw();
        assert!((b.to_degrees() - 89.6).abs() < 0.2, "{}", b.to_degrees());
        assert!(g.muzzle((b, 0.0)).distance(g.fire_point) < 1e-4);
        let m = g.muzzle((65f32.to_radians(), 0.0));
        assert!(m.z > 0.3 && (m.y - 0.911).abs() < 1e-4, "{m:?}");
    }

    #[test]
    fn rear_yaw_arc_keeps_its_positive_angle_domain() {
        let t = Turn::from_class(
            Some([120.0, 240.0]),
            Some([-17.0, 20.0]),
            (1.0, 1.0),
            (3.75, 3.75),
            Slew::Linear,
        );
        assert!(t.yaw_positive && !t.yaw_wrap);
        assert!((t.rest().0 - std::f32::consts::PI).abs() < 1e-5);
        for (input, expected) in [(-170.0f32, 190.0f32), (100.0, 120.0), (270.0, 240.0)] {
            assert!((t.target(input.to_radians(), 0.0).0 - expected.to_radians()).abs() < 1e-5);
        }
        assert!((t.hold(-1.0, 0.0).0 - 120.0f32.to_radians()).abs() < 1e-5);
        let mut look = t.rest();
        let mut rate = (0.0, 0.0);
        for _ in 0..120 {
            (look, rate) = t.step(look, rate, (1.0, 0.0), crate::TICK_DT);
        }
        assert!((look.0 - 120.0f32.to_radians()).abs() < 1e-5);
        for _ in 0..180 {
            (look, rate) = t.step(look, rate, (-1.0, 0.0), crate::TICK_DT);
        }
        assert!((look.0 - 240.0f32.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn a_rear_aimer_keeps_its_authored_bind_direction_and_muzzle() {
        let g = GunSetup {
            weapons: Vec::new(),
            fire_point: Vec3::new(1.0, 2.0, -3.0),
            pivot: Vec3::new(1.0, 2.0, -1.0),
            eye: Vec3::ZERO,
            aimer: Aimer::default(),
            pilot: false,
            turn: None,
            framed: false,
            aimer_bind: glam::Quat::from_rotation_y(std::f32::consts::PI),
            weapon_aimer: None,
            follows_owner: false,
            gunner: None,
        };
        let aim = g.aim_of((0.0, 0.0), (0.0, 0.0));
        assert!(dir(aim.0, aim.1).abs_diff_eq(-Vec3::Z, 1e-5));
        assert!(g.muzzle(aim).abs_diff_eq(g.fire_point, 1e-5));
        let aim = g.aim_of((0.0, 0.0), (0.4, 0.2));
        assert!((g.muzzle(aim) - g.pivot)
            .normalize()
            .abs_diff_eq(dir(aim.0, aim.1), 1e-5));
    }

    fn dish(slew: Slew) -> Turn {
        // BF2 Hoth's dish turret: YawLimits -180 180, PitchLimits -10 35,
        // MaxTurnSpeed / MaxPitchSpeed 1, filters the default 10.
        Turn::from_class(
            Some([-180.0, 180.0]),
            Some([-10.0, 35.0]),
            (1.0, 1.0),
            (10.0, 10.0),
            slew,
        )
    }

    /// The limits as the PS2's tick reads them (B2:189323-189385): a
    /// full-circle or default pair wraps, a narrower one clamps; the
    /// turret's pitch (+ = nose down) arrives negated.
    #[test]
    fn full_circle_and_default_limits_wrap_narrower_ones_clamp() {
        let t = dish(Slew::Linear);
        assert!(t.yaw_wrap && !t.pitch_wrap);
        assert!((t.pitch[0] + 35.0 * DEG).abs() < 1e-5 && (t.pitch[1] - 10.0 * DEG).abs() < 1e-5);
        let n = Turn::from_class(
            Some([-170.0, 170.0]),
            None,
            (1.0, 1.0),
            (10.0, 10.0),
            Slew::Exp,
        );
        assert!(!n.yaw_wrap && n.pitch_wrap, "{n:?}");
        assert_eq!(t.rest(), (0.0, 0.0));
    }

    /// The rate law against the PS2's rows at full stick
    /// (`lead_tmp/vturl/rates.py`): BF2's dish steps its rate by a third
    /// a tick (-0.333, -0.667, -1.000 in `hot1_dish`), BF1's gun turret
    /// approaches 0.85 exponentially; the look moves by the new rate.
    #[test]
    fn the_stick_drives_a_slewed_rate() {
        let dt = 1.0 / 30.0;
        let t = dish(Slew::Linear);
        let (mut look, mut rate) = ((0.0, 0.0), (0.0, 0.0));
        let mut seen = Vec::new();
        for _ in 0..4 {
            (look, rate) = t.step(look, rate, (1.0, 0.0), dt);
            seen.push(rate.0);
        }
        for (got, want) in seen.iter().zip([-1.0 / 3.0, -2.0 / 3.0, -1.0, -1.0]) {
            assert!((got - want).abs() < 1e-5, "{seen:?}");
        }
        assert!((look.0 + dt * (1.0 / 3.0 + 2.0 / 3.0 + 2.0)).abs() < 1e-5);
        // Let go: the rate slews back to 0, no further.
        for _ in 0..5 {
            (look, rate) = t.step(look, rate, (0.0, 0.0), dt);
        }
        assert_eq!(rate.0, 0.0);
        let g = Turn::from_class(
            Some([-180.0, 180.0]),
            Some([-45.0, 45.0]),
            (0.85, 0.75),
            (10.0, 10.0),
            Slew::Exp,
        );
        let (mut look, mut rate) = ((0.0, 0.0), (0.0, 0.0));
        (look, rate) = g.step(look, rate, (0.0, 1.0), dt);
        assert!(
            (rate.1 - 0.75 / 3.0).abs() < 1e-6 && look.1 > 0.0,
            "stick up looks up"
        );
        for _ in 0..60 {
            (look, rate) = g.step(look, rate, (0.0, 1.0), dt);
        }
        assert!((rate.1 - 0.75).abs() < 1e-3);
        assert!((look.1 - 45.0 * DEG).abs() < 1e-5, "held at the limit");
    }

    /// A full-circle turret held at full stick passes 180 degrees and keeps
    /// turning the same way, never more than a tick's rate at a step.
    #[test]
    fn a_full_circle_turret_turns_through_the_seam() {
        let dt = 1.0 / 30.0;
        let t = dish(Slew::Linear);
        let (mut look, mut rate) = ((3.0, 0.0), (0.0, 0.0));
        let mut turned = 0.0;
        for _ in 0..240 {
            let (l, r) = t.step(look, rate, (1.0, 0.0), dt);
            let d = wrap(l.0 - look.0);
            assert!(d <= 0.0 && d >= -dt * 1.0 - 1e-5, "step {d}");
            assert!(l.0.abs() <= std::f32::consts::PI);
            turned += d;
            (look, rate) = (l, r);
        }
        assert!(turned < -7.5, "turned {turned} rad");
    }

    /// The aimer chases a direction in its frame at its rates and stops at
    /// its limits; `AimerYawLimits 0 0` pins it to the turret's heading,
    /// whatever the look.
    #[test]
    fn the_aimer_chases_in_the_aim_frame() {
        let dt = 1.0 / 30.0;
        let a = Aimer::default();
        let mut r = (0.0, 0.0);
        for _ in 0..60 {
            r = a.chase(r, dir(0.3, -0.2), dt);
        }
        assert!(
            (r.0 - 0.3).abs() < 1e-3 && (r.1 + 0.2).abs() < 1e-3,
            "{r:?}"
        );
        let r1 = a.chase((0.0, 0.0), dir(1.0, 0.0), dt);
        assert!((r1.0 - 2.0 * dt).abs() < 1e-6, "at most YawRate·dt: {r1:?}");
        let pinned = Aimer {
            yaw: [0.0, 0.0],
            pitch: [-15.0 * DEG, 40.0 * DEG],
            ..Aimer::default()
        };
        let p = pinned.chase((0.0, 0.0), dir(0.5, 0.3), dt);
        assert_eq!(p.0, 0.0);
        for look in [(3.1, 0.1), (-3.1, -0.3), (0.4, 0.0)] {
            let m = compose(look, (0.0, 0.25));
            assert!(wrap(m.0 - look.0).abs() < 1e-4, "{look:?} -> {m:?}");
            assert!((m.1 - (look.1 + 0.25)).abs() < 1e-4, "{look:?} -> {m:?}");
        }
        // The frame is the look's: its forward is dir(look).
        let [x, y, z] = frame((0.7, -0.2));
        assert!(z.distance(dir(0.7, -0.2)) < 1e-6);
        assert!(x.dot(y).abs() < 1e-6 && x.dot(z).abs() < 1e-6 && (y.y > 0.0));
    }
}
