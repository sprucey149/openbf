//! Alert and the aim pose (card ANIMA; `analysis/re-soldier-anim-layers.md`
//! §3.4, §7 `[O]`): a soldier who fires or holds a target is ALERT for a
//! while — he plays the alert clip rows and his torso is turned onto the aim
//! line; otherwise only his neck and head look that way.
//!
//! * BF1: a timer armed to 2.5 s when it had run out, else 2.0 (`FUN_0022d150`),
//!   and to 2.0 by a shot (`FUN_0022a958`); not for the tool and sabre sets.
//!   Above 0 the alert rows play. The layers' weight ramps from 0.2 over
//!   0.4 s at each change; the aim weight `a` rises 0 → 1 over the first
//!   0.5 s and falls 1 → 0 over 0.5 s after the timer runs out; the torso
//!   takes `2a − 1`, the head `1 − 2a`. Prone always aims with the torso.
//! * BF2: 2.5 s while armed (`FUN_0024f078`), 0 for the tool and melee
//!   classes; the UPPER clip's `AimType` (the disc's `sanm`) says Head or
//!   Torso, faded over that clip's `BlendInTime`.
//! * The pose: yaw within ±0.6283 rad of the facing less the leg twist,
//!   pitch within ±1.0996 (prone: none downward), eased 15 % an update (BF1)
//!   or by min(1, 4.5 dt) (BF2). Torso: `bone_a_spine`, `bone_b_spine`, `bone_ribcage` stand at
//!   1/8, 4/8 and 8/8 of the angle (BF2's 0.125 + 0.375 + 0.5, cumulative,
//!   is the same three); head: `bone_neck` 2/8, `bone_head` 5/8.
//!
//! PROVISIONAL and ours: the turn is about the soldier's own right and up
//! axes, yaw then pitch (the PS2's order and handedness are unread, §10); it
//! is applied to a copy of the kept pose, so it never feeds the blend; BF1's
//! 15 % is taken per 1/60 s; a jump, fall, roll or death fades the turn out
//! over 0.15 s; `FullBody` aims as `Torso`; what makes a soldier alert is
//! the caller's (`soldier_vis`: he fires, holds the trigger or has a
//! target — the PS2's two soldier flags are unread).

use super::{Game, BF1_UPDATES};
use crate::skin::{Pose, Skeleton};
use core::fmath::FloatExt;
use glam::Quat;

/// The aim's limits from the legs (rad): `FUN_0022bdf0` / `FUN_00280158`.
pub const YAW_LIMIT: f32 = 0.6283;
pub const PITCH_LIMIT: f32 = 1.0996;
/// The timer a fresh alert and a held one are armed to (s).
pub const ALERT_FRESH: f32 = 2.5;
pub const ALERT_HELD: f32 = 2.0;
/// BF1: the aim weight's ramp at each end of the alert (s).
pub const AIM_RAMP: f32 = 0.5;
/// (bone, share of the angle its model-space turn stands at).
pub const TORSO: [(&str, f32); 3] = [
    ("bone_a_spine", 0.125),
    ("bone_b_spine", 0.5),
    ("bone_ribcage", 1.0),
];
pub const HEAD: [(&str, f32); 2] = [("bone_neck", 0.25), ("bone_head", 0.625)];
/// Ours: seconds a state without an aim pose fades it over.
const STATE_FADE: f32 = 0.15;

/// A BF2 clip's `AimType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AimKind {
    #[default]
    None,
    Head,
    Torso,
}

impl AimKind {
    pub fn of(name: &str) -> Self {
        match name {
            "Head" => Self::Head,
            "Torso" | "FullBody" => Self::Torso,
            _ => Self::None,
        }
    }
}

/// What one update gives the aim.
#[derive(Clone, Copy, Debug)]
pub struct AimStep {
    pub game: Game,
    pub dt: f32,
    /// The caller's alert reason holds, and the set in hand takes one.
    pub arm: bool,
    /// A shoot action started this update (BF1 arms on it).
    pub shot: bool,
    /// (yaw from the facing, pitch) of the aim line, rad.
    pub aim: (f32, f32),
    pub prone: bool,
    /// On his feet: the states that take an aim pose.
    pub feet: bool,
}

#[derive(Debug, Clone)]
pub struct Aim {
    /// The alert timer (s): BF1's runs on below 0 through its fade.
    timer: f32,
    /// The eased (yaw, pitch).
    eased: (f32, f32),
    /// The torso's and the head's weight.
    torso: f32,
    head: f32,
    /// 1 on his feet, 0 in a state without an aim pose.
    state: f32,
    fresh: bool,
}

impl Default for Aim {
    fn default() -> Self {
        Self {
            timer: -1.0,
            eased: (0.0, 0.0),
            torso: 0.0,
            head: 0.0,
            state: 0.0,
            fresh: true,
        }
    }
}

impl Aim {
    /// Arm and run the timer; is the soldier alert this update?
    pub fn tick(&mut self, s: &AimStep) -> bool {
        match s.game {
            Game::Bf1 => {
                if s.arm {
                    self.timer = if self.timer < 0.0 {
                        ALERT_FRESH
                    } else {
                        ALERT_HELD
                    };
                    if s.shot {
                        self.timer = ALERT_HELD;
                    }
                }
                self.timer = (self.timer - s.dt).fmax(-1.0);
            }
            Game::Bf2 => {
                self.timer = if s.arm {
                    ALERT_FRESH
                } else {
                    (self.timer - s.dt).fmax(0.0)
                };
            }
        }
        self.alert()
    }

    pub fn alert(&self) -> bool {
        self.timer > 0.0
    }

    pub fn timer(&self) -> f32 {
        self.timer
    }

    /// (torso, head) weights the pose is turned with.
    pub fn weights(&self) -> (f32, f32) {
        (self.torso * self.state, self.head * self.state)
    }

    /// The eased (yaw from the facing, pitch).
    pub fn eased(&self) -> (f32, f32) {
        self.eased
    }

    pub fn turn(&self, twist: f32) -> (f32, f32) {
        (self.eased.0 - twist, self.eased.1)
    }

    /// BF1: the factor on both layers' weight, 0.2 rising to 1 over 0.4 s at
    /// each change between the alert and the plain rows (§3.4).
    pub fn layer_factor(&self, game: Game) -> f32 {
        let t = self.timer;
        if game != Game::Bf1 {
            1.0
        } else if t > ALERT_HELD {
            (1.2 - 2.0 * (t - ALERT_HELD)).fmin(1.0)
        } else if t <= 0.0 && t > -AIM_RAMP {
            (0.2 - 2.0 * t).fmin(1.0)
        } else {
            1.0
        }
    }

    /// BF1's aim weight `a` of the timer (142859-142877).
    fn bf1_weight(t: f32) -> f32 {
        if t <= -AIM_RAMP {
            0.0
        } else if t > ALERT_HELD {
            1.0 - 2.0 * (t - ALERT_HELD)
        } else if t > 0.0 {
            1.0
        } else {
            1.0 + 2.0 * t
        }
    }

    /// Ease the pair and the weights; `kind` and `blend_in` are the upper
    /// clip's (BF2 reads them).
    pub fn ease(&mut self, s: &AimStep, kind: AimKind, blend_in: f32) {
        let low = if s.prone { 0.0 } else { -PITCH_LIMIT };
        let to = (
            s.aim.0.clamp(-YAW_LIMIT, YAW_LIMIT),
            s.aim.1.clamp(low, PITCH_LIMIT),
        );
        let k = match s.game {
            _ if self.fresh => 1.0,
            Game::Bf1 => 1.0 - 0.85f32.powf(s.dt * BF1_UPDATES),
            Game::Bf2 => (4.5 * s.dt).fmin(1.0),
        };
        self.eased.0 += (to.0 - self.eased.0) * k;
        self.eased.1 += (to.1 - self.eased.1) * k;
        let toward = |x: f32, to: f32, secs: f32| {
            if secs <= 0.0 {
                to
            } else {
                x + (to - x).clamp(-s.dt / secs, s.dt / secs)
            }
        };
        match s.game {
            Game::Bf1 => {
                let a = Self::bf1_weight(self.timer).clamp(0.0, 1.0);
                (self.torso, self.head) = if s.prone {
                    (1.0, 0.0)
                } else {
                    ((2.0 * a - 1.0).fmax(0.0), (1.0 - 2.0 * a).fmax(0.0))
                };
            }
            Game::Bf2 => {
                let secs = if self.fresh { 0.0 } else { blend_in };
                let on = |k| f32::from(u8::from(kind == k));
                self.torso = toward(self.torso, on(AimKind::Torso), secs);
                self.head = toward(self.head, on(AimKind::Head), secs);
            }
        }
        let secs = if self.fresh { 0.0 } else { STATE_FADE };
        self.state = toward(self.state, f32::from(u8::from(s.feet)), secs);
        self.fresh = false;
    }

    /// Turn `pose`'s torso and head onto the eased aim.
    pub fn apply(&self, pose: &mut Pose, skel: &Skeleton, twist: f32) {
        let (torso, head) = self.weights();
        let (yaw, pitch) = self.turn(twist);
        let turn =
            |share: f32| Quat::from_rotation_y(yaw * share) * Quat::from_rotation_x(-pitch * share);
        for (bones, w) in [(&TORSO[..], torso), (&HEAD[..], head)] {
            if w <= 0.0 {
                continue;
            }
            // Each bone's turn is measured from where the pose (with the
            // torso's turn, for the head) had it: read them all first.
            let at: Vec<_> = bones
                .iter()
                .map(|(n, share)| {
                    let b = skel.index_of(n)?;
                    Some((b, turn(share * w) * global(pose, skel, b)))
                })
                .collect();
            for (b, to) in at.into_iter().flatten() {
                let parent = skel.parents[b].map_or(Quat::IDENTITY, |p| global(pose, skel, p));
                pose.0[b].rotation = (parent.inverse() * to).normalize();
            }
        }
    }
}

/// A bone's rotation in the model's frame.
pub fn global(pose: &Pose, skel: &Skeleton, bone: usize) -> Quat {
    let mut q = pose.0[bone].rotation;
    let mut at = skel.parents[bone];
    while let Some(p) = at {
        q = pose.0[p].rotation * q;
        at = skel.parents[p];
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    const DT: f32 = 1.0 / 60.0;
    const NAMES: [&str; 9] = [
        "DummyRoot",
        "bone_root",
        "bone_a_spine",
        "bone_b_spine",
        "bone_ribcage",
        "bone_neck",
        "bone_head",
        "bone_r_upperarm",
        "hp_weapons",
    ];
    const RIB: usize = 4;
    const NECK: usize = 5;
    const HEAD_BONE: usize = 6;
    const GUN: usize = 8;

    /// A spine of tilted bones; the gun's bind rotation in the model is the
    /// identity, so its forward is the model's +Z, the soldier's front.
    fn skel() -> Skeleton {
        let parents = vec![
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(4),
            Some(7),
        ];
        let mut locals: Vec<Mat4> = (0..9)
            .map(|b| {
                let r = Quat::from_euler(
                    glam::EulerRot::YXZ,
                    0.3 * b as f32,
                    0.2 - 0.1 * b as f32,
                    0.15 * b as f32,
                );
                Mat4::from_rotation_translation(r, Vec3::new(0.0, 0.2, 0.01 * b as f32))
            })
            .collect();
        let s = Skeleton::new(
            NAMES.iter().map(|n| (*n).to_string()).collect(),
            parents.clone(),
            locals.clone(),
        );
        let arm = global(&Pose::bind(&s), &s, 7);
        locals[GUN] = Mat4::from_quat(arm.inverse());
        Skeleton::new(
            NAMES.iter().map(|n| (*n).to_string()).collect(),
            parents,
            locals,
        )
    }

    fn step(game: Game, arm: bool, pitch: f32) -> AimStep {
        AimStep {
            game,
            dt: DT,
            arm,
            shot: false,
            aim: (0.0, pitch),
            prone: false,
            feet: true,
        }
    }

    /// `n` updates; BF2's upper clip aims with the torso while alert.
    fn run(a: &mut Aim, s: &AimStep, n: usize) {
        for _ in 0..n {
            let kind = if a.tick(s) {
                AimKind::Torso
            } else {
                AimKind::Head
            };
            a.ease(s, kind, 0.25);
        }
    }

    fn posed(a: &Aim, s: &Skeleton) -> Pose {
        let mut p = Pose::bind(s);
        a.apply(&mut p, s, 0.0);
        p
    }

    /// The angle a bone was turned by from its bind pose, in the model.
    fn turned(a: &Aim, s: &Skeleton, bone: usize) -> f32 {
        let (now, was) = (
            global(&posed(a, s), s, bone),
            global(&Pose::bind(s), s, bone),
        );
        (now * was.inverse()).to_axis_angle().1
    }

    fn line(pitch: f32) -> Vec3 {
        Vec3::new(0.0, pitch.sin(), pitch.cos())
    }

    #[test]
    fn alert_turns_the_gun_onto_the_aim_line() {
        let s = skel();
        for game in [Game::Bf1, Game::Bf2] {
            let mut a = Aim::default();
            run(&mut a, &step(game, true, 0.5), 120);
            let gun = global(&posed(&a, &s), &s, GUN) * Vec3::Z;
            let off = gun.angle_between(line(0.5));
            assert!(off < 0.02, "{game:?}: the gun is {off} rad off the aim");
            // Down too, and the spine shares it out: 1/8, 4/8, 8/8.
            run(&mut a, &step(game, true, -0.4), 240);
            let gun = global(&posed(&a, &s), &s, GUN) * Vec3::Z;
            assert!(gun.angle_between(line(-0.4)) < 0.02, "{game:?} down");
            for (bone, share) in [(2, 0.125), (3, 0.5), (RIB, 1.0)] {
                let t = turned(&a, &s, bone);
                assert!((t - 0.4 * share).abs() < 0.01, "{game:?} bone {bone}: {t}");
            }
        }
    }

    #[test]
    fn not_alert_only_the_head_looks() {
        let s = skel();
        for game in [Game::Bf1, Game::Bf2] {
            let mut a = Aim::default();
            run(&mut a, &step(game, false, 0.5), 120);
            assert!(!a.alert());
            assert!(
                turned(&a, &s, RIB) < 1e-4,
                "{game:?}: the ribcage is unturned"
            );
            let gun = global(&posed(&a, &s), &s, GUN) * Vec3::Z;
            assert!(
                gun.angle_between(Vec3::Z) < 1e-3,
                "{game:?}: the gun is level"
            );
            let (neck, head) = (turned(&a, &s, NECK), turned(&a, &s, HEAD_BONE));
            assert!((neck - 0.25 * 0.5).abs() < 0.01, "{game:?} neck {neck}");
            assert!((head - 0.625 * 0.5).abs() < 0.01, "{game:?} head {head}");
        }
    }

    #[test]
    fn the_aim_is_limited() {
        let s = skel();
        for game in [Game::Bf1, Game::Bf2] {
            let mut a = Aim::default();
            run(&mut a, &step(game, true, 1.5), 300);
            assert!((a.eased().1 - PITCH_LIMIT).abs() < 1e-3, "{game:?}");
            let gun = global(&posed(&a, &s), &s, GUN) * Vec3::Z;
            assert!(gun.angle_between(line(PITCH_LIMIT)) < 0.02, "{game:?}");
            run(&mut a, &step(game, true, -1.5), 300);
            assert!((a.eased().1 + PITCH_LIMIT).abs() < 1e-3, "{game:?} down");
            // Prone: no downward pitch.
            let mut prone = step(game, true, -1.5);
            prone.prone = true;
            run(&mut a, &prone, 300);
            assert!(a.eased().1.abs() < 1e-3, "{game:?} prone");
            // Yaw from the legs.
            let mut side = step(game, true, 0.0);
            side.aim = (2.0, 0.0);
            run(&mut a, &side, 300);
            assert!((a.eased().0 - YAW_LIMIT).abs() < 1e-3, "{game:?} yaw");
        }
    }

    #[test]
    fn bf1_alert_runs_its_time_and_the_aim_fades_after_it() {
        let mut a = Aim::default();
        let (on, off) = (step(Game::Bf1, true, 0.5), step(Game::Bf1, false, 0.5));
        run(&mut a, &on, 1);
        // A fresh alert: 2.5 s, the torso's weight still 0, the layers at 0.2.
        assert!((a.timer() - (ALERT_FRESH - DT)).abs() < 1e-4);
        assert!(a.weights().0 < 0.01);
        assert!((a.layer_factor(Game::Bf1) - 0.2).abs() < 0.05);
        let mut alert = 1;
        for _ in 0..600 {
            run(&mut a, &off, 1);
            alert += usize::from(a.alert());
            let (t, (torso, head)) = (a.timer(), a.weights());
            if t < 2.0 && t > 0.0 {
                assert!((torso - 1.0).abs() < 1e-4 && head == 0.0, "torso at {t}");
            } else if (t + 0.25).abs() < 0.2 * DT {
                assert!(torso < 0.01 && head < 0.01, "half way out at {t}");
            }
        }
        assert!(
            (alert as f32 * DT - ALERT_FRESH).abs() < 2.0 * DT,
            "{alert}"
        );
        assert_eq!(a.weights(), (0.0, 1.0));
        assert!((a.layer_factor(Game::Bf1) - 1.0).abs() < 1e-6);
        // Held, the timer stands at 2.0: the torso at full weight.
        run(&mut a, &on, 1);
        run(&mut a, &on, 30);
        assert!((a.timer() - (ALERT_HELD - DT)).abs() < 1e-4);
        assert_eq!(a.weights(), (1.0, 0.0));
        // A shot arms 2.0 at once.
        let mut b = Aim::default();
        let mut shot = on;
        shot.shot = true;
        run(&mut b, &shot, 1);
        assert!((b.timer() - (ALERT_HELD - DT)).abs() < 1e-4);
    }

    #[test]
    fn bf2_alert_is_two_and_a_half_seconds_and_the_clip_fades_the_aim() {
        let mut a = Aim::default();
        let (on, off) = (step(Game::Bf2, true, 0.5), step(Game::Bf2, false, 0.5));
        run(&mut a, &off, 60);
        assert_eq!(a.weights(), (0.0, 1.0));
        run(&mut a, &on, 60);
        let mut alert = 0;
        for k in 0..300 {
            run(&mut a, &off, 1);
            alert += usize::from(a.alert());
            if k == 60 {
                assert_eq!(a.weights(), (1.0, 0.0));
            }
        }
        assert!(
            (alert as f32 * DT - ALERT_FRESH).abs() < 2.0 * DT,
            "{alert}"
        );
        assert_eq!(a.weights(), (0.0, 1.0));
        // Half the clip's 0.25 s BlendInTime after the change: half way.
        run(&mut a, &on, 8);
        let (torso, head) = a.weights();
        assert!((torso - 0.53).abs() < 0.05 && (head - 0.47).abs() < 0.05);
    }

    #[test]
    fn a_state_without_an_aim_pose_fades_it_out() {
        let s = skel();
        let mut a = Aim::default();
        run(&mut a, &step(Game::Bf2, true, 0.5), 60);
        let mut jump = step(Game::Bf2, true, 0.5);
        jump.feet = false;
        run(&mut a, &jump, 30);
        assert_eq!(a.weights(), (0.0, 0.0));
        assert_eq!(posed(&a, &s), Pose::bind(&s));
    }
}
