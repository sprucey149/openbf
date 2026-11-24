//! A soldier's hit volumes (the bolt prims, `analysis/re-hitzones.md` 1):
//! which posture's list he is hit by, where his model puts the head and
//! torso in it, and the three prims, in his own frame (x right, y up from
//! the feet, +z his facing).
//!
//! The shapes are BF2's posture lists (FUN_002517e8 B2:170187-170456):
//! standing and crouched a head sphere, a torso sphere and an upright body
//! cylinder; prone a head sphere and a box lying along his facing. BF2
//! moves the head and torso spheres each frame to the animation's
//! bone_head and bone_ribcage points (FUN_0024f350 B2:168821-169060).
//!
//! OURS (playtest 2026-11-22, fourth LAN test: "hit boxes in crouch or
//! prone seemed messed up"), two named deviations:
//!
//! - Size. The PS2 sizes the lists by `CollisionRootScale`, which most
//!   classes leave at 1, whatever their model; with the lists' fixed head
//!   and torso places an Ewok (drawn head 1.05 m) was hit 0.6 m over his
//!   head and Vader's drawn head (1.94 m) stood over his head sphere (BF2's
//!   per-frame head reads a table per animation bank, not per model, and
//!   is not measured for them). Here a class's model sets it ([`StanceBody`]): its
//!   drawn head and ribcage points in each posture, and its size, the drawn
//!   standing head's height over the trooper's. Where the class authors a
//!   `CollisionRootScale` the two agree within 0.04 (Wookiee 1.149 / 1.15,
//!   Jawa 0.581 / 0.6, Yoda 0.382 / 0.42). A model with no row keeps the
//!   posture lists' own points scaled by `CollisionRootScale`.
//!   measure: a PS2 Ewok's and Vader's head prims in RAM against the
//!   drawn heads.
//! - Changes into and out of prone. The stance changes on the press, but
//!   the change's clip takes 1.33 s down and 1.5 s up (BF1's, which the
//!   port plays in both games): a soldier was hit as prone while he still
//!   stood, and as standing while he still lay. Here his posture follows
//!   the clip ([`Posture::of`]): the one whose head height the drawn head
//!   (the trooper's, frame by frame) is nearest. measure: the same frames
//!   on another class's rig (the clips are the shared human bank's).

use super::{ray_sphere, ray_upright_cylinder, ray_yawed_box, BoltPrims, HitZone};
use crate::fmath::FloatExt;
use crate::soldier::{Soldier, Stance};
use glam::Vec3;

/// Which posture's prims a soldier is hit by (BF2's three lists, select
/// FUN_0024f0b0 B2:168302-168416: 0 stand, 1 crouch, 2 prone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posture {
    Stand,
    Crouch,
    Prone,
}

/// The clip frame (30 a second) from which a prone change's drawn head is
/// nearer the next posture's head height than the last's (midpoints 1.467 m
/// stand / crouch and 0.787 m crouch / prone, the trooper's
/// [`StanceBody`] heads): `human_rifle_stand_getdown_prone` crouches at
/// frame 9 (1.459 m) and lies at 16 (0.675 m);
/// `human_rifle_crouch_getdown_prone` lies at 15 (0.706 m);
/// `human_rifle_prone_getup_stand` crouches at 30 (0.894 m) and stands at
/// 36 (1.541 m); `human_rifle_prone_getup_crouch` crouches at 31 (0.846 m).
/// Measured on the BF2 clone trooper's baked rig. Each is half a frame
/// early, as the prone locks are, so the tick that lands on the frame
/// takes it whatever the float sum of its steps.
const DOWN_CROUCHED: f32 = 8.5 / 30.0;
const DOWN_FROM_STAND: f32 = 15.5 / 30.0;
const DOWN_FROM_CROUCH: f32 = 14.5 / 30.0;
const UP_TO_STAND_CROUCHED: f32 = 29.5 / 30.0;
const UP_TO_STAND: f32 = 35.5 / 30.0;
const UP_TO_CROUCH: f32 = 30.5 / 30.0;

impl Posture {
    /// The posture soldier `s` is hit in now. The tumble and get-up take
    /// the crouch's, a roll and a corpse the prone's (card KNOCK,
    /// `re-knockback.md` §2 / §3: posture 1 / 2; the roll PROVISIONAL, as
    /// in [`super::capsule_height`], measure: a PS2 roll's posture byte);
    /// every other stance the stand's.
    pub fn of(s: &Soldier) -> Posture {
        let t = s.time_in_stance;
        match (s.stance, s.prone_from) {
            (Stance::Prone, Some(Stance::Crouch)) if t < DOWN_FROM_CROUCH => Posture::Crouch,
            (Stance::Prone, Some(Stance::Crouch)) => Posture::Prone,
            (Stance::Prone, Some(_)) if t < DOWN_CROUCHED => Posture::Stand,
            (Stance::Prone, Some(_)) if t < DOWN_FROM_STAND => Posture::Crouch,
            (Stance::Crouch, Some(Stance::Prone)) if t < UP_TO_CROUCH => Posture::Prone,
            (_, Some(Stance::Prone)) if t < UP_TO_STAND_CROUCHED => Posture::Prone,
            (_, Some(Stance::Prone)) if t < UP_TO_STAND => Posture::Crouch,
            (Stance::Crouch | Stance::Tumble | Stance::GetUp, _) => Posture::Crouch,
            (Stance::Prone | Stance::Roll | Stance::Dead, _) => Posture::Prone,
            _ => Posture::Stand,
        }
    }
}

/// The trooper's drawn standing head height (BF2 clone trooper, the rifle
/// idle at 0.1333 s): [`StanceBody::scale`] 1.
pub const TROOPER_HEAD_Y: f32 = 1.673;

/// One soldier model's drawn head and torso centres, in his frame from the
/// feet: BF2's head and torso points (bone_head x (0.035, 0.03, 0) and
/// bone_ribcage x (-0.12, -0.05, 0), FUN_00282f78 B2:216195-216340) on the
/// baked rig, in the stand, crouch and prone rifle idles at 0.1333 s (the
/// time BF2 samples them at). The prone torso is not kept: BF2's prone list
/// has none (a 0.01 m sphere at the feet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StanceBody {
    /// Stand, crouch, prone.
    pub head: [Vec3; 3],
    /// Stand, crouch.
    pub torso: [Vec3; 2],
}

impl StanceBody {
    /// The body's size: its standing head's height over the trooper's.
    pub fn scale(&self) -> f32 {
        self.head[0].y / TROOPER_HEAD_Y
    }

    /// The head and torso centres in `posture` (prone: no torso, zero).
    pub fn points(&self, posture: Posture) -> (Vec3, Vec3) {
        match posture {
            Posture::Stand => (self.head[0], self.torso[0]),
            Posture::Crouch => (self.head[1], self.torso[1]),
            Posture::Prone => (self.head[2], Vec3::ZERO),
        }
    }
}

/// The posture lists' own head and torso centres at scale `s`
/// (`re-hitzones.md` 1, BF2's table).
pub fn list_points(posture: Posture, s: f32) -> (Vec3, Vec3) {
    match posture {
        Posture::Stand => (
            Vec3::new(0.0, 1.65 * s, 0.16),
            Vec3::new(0.0, 1.15 * s, 0.0),
        ),
        Posture::Crouch => (Vec3::new(0.0, 1.2 * s, 0.25), Vec3::new(0.0, 0.7 * s, 0.0)),
        Posture::Prone => (Vec3::new(0.0, 0.28 * s, 0.75 * s), Vec3::ZERO),
    }
}

/// One bolt prim in the soldier's frame.
#[derive(Clone, Copy, Debug)]
pub enum Prim {
    Sphere {
        c: Vec3,
        r: f32,
    },
    /// Upright, centre `c`, half height `hh`.
    Cylinder {
        c: Vec3,
        r: f32,
        hh: f32,
    },
    /// Axis-aligned in his frame, half extents `half`.
    Box {
        c: Vec3,
        half: Vec3,
    },
}

impl Prim {
    /// Where the ray (in his frame) first meets the prim grown by `grow`.
    pub fn ray(&self, from: Vec3, dir: Vec3, grow: f32) -> Option<f32> {
        match *self {
            Prim::Sphere { c, r } => ray_sphere(from, dir, c, r + grow),
            Prim::Cylinder { c, r, hh } => ray_upright_cylinder(from, dir, c, r + grow, hh + grow),
            Prim::Box { c, half } => {
                ray_yawed_box(from, dir, c, Vec3::X, Vec3::Z, half + Vec3::splat(grow))
            }
        }
    }
}

impl BoltPrims {
    /// The prims of a soldier at `pos` turned `yaw`, hit in `posture`:
    /// on his model's points where it has a row, else on the lists' at
    /// `collision_scale`.
    pub fn new(
        pos: Vec3,
        yaw: f32,
        posture: Posture,
        collision_scale: f32,
        body: Option<&StanceBody>,
    ) -> Self {
        let scale = body.map_or(collision_scale, StanceBody::scale);
        let (head, torso) = body.map_or_else(|| list_points(posture, scale), |b| b.points(posture));
        Self {
            pos,
            yaw,
            posture,
            scale,
            head,
            torso,
            posed: None,
        }
    }

    /// The three prims in his frame, head first. Sizes are BF2's at the
    /// body's scale `s`: the head 0.17 x max(s, 1), the torso 0.4 s, the
    /// body cylinder 0.35 s wide to 1.1 s standing and 0.7 s crouched, the
    /// prone box half (0.25, 0.2, 0.87) s at (0, 0.2 s, -0.25 s) (BF2 sizes
    /// only the box's length and the cylinders' heights; the rest is OURS,
    /// so a small model is not wider than he is drawn; measure: the drawn
    /// Ewok's and Yoda's widths). A seated rider's
    /// ([`BoltPrims::posed`]) are his drawn ones ([`BoltPrims::posed_sizes`]).
    pub fn prims(&self) -> [(Prim, HitZone); 3] {
        let s = self.scale;
        let head_r = 0.17 * s.fmax(1.0);
        if let Some([head, torso]) = self.posed {
            let (torso_r, hh) = self.posed_sizes(head, torso);
            return [
                (Prim::Sphere { c: head, r: head_r }, HitZone::Head),
                (
                    Prim::Sphere {
                        c: torso,
                        r: torso_r,
                    },
                    HitZone::Torso,
                ),
                (
                    Prim::Cylinder {
                        c: Vec3::new(0.0, hh, 0.0),
                        r: 0.35,
                        hh,
                    },
                    HitZone::Body,
                ),
            ];
        }
        let head = (
            Prim::Sphere {
                c: self.head,
                r: head_r,
            },
            HitZone::Head,
        );
        let torso = (
            Prim::Sphere {
                c: self.torso,
                r: 0.4 * s,
            },
            HitZone::Torso,
        );
        let cylinder = |hh: f32| {
            (
                Prim::Cylinder {
                    c: Vec3::new(0.0, hh, 0.0),
                    r: 0.35 * s,
                    hh,
                },
                HitZone::Body,
            )
        };
        match self.posture {
            Posture::Stand => [head, torso, cylinder(0.55 * s)],
            Posture::Crouch => [head, torso, cylinder(0.35 * s)],
            Posture::Prone => {
                let body = (
                    Prim::Box {
                        c: Vec3::new(0.0, 0.2 * s, -0.25 * s),
                        half: Vec3::new(0.25, 0.2, 0.87) * s,
                    },
                    HitZone::Body,
                );
                // No torso prone: the box twice, the nearer is the same.
                [head, body, body]
            }
        }
    }

    /// A world ray in his frame: (from, dir).
    pub fn to_local(&self, from: Vec3, dir: Vec3) -> (Vec3, Vec3) {
        let (sy, cy) = crate::fmath::sin_cos(self.yaw);
        let right = Vec3::new(cy, 0.0, -sy);
        let fwd = Vec3::new(-sy, 0.0, -cy);
        let p = from - self.pos;
        (
            Vec3::new(p.dot(right), p.y, p.dot(fwd)),
            Vec3::new(dir.dot(right), dir.y, dir.dot(fwd)),
        )
    }
}
