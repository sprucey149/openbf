//! Missiles (card UORD): the class data, the flight law and the lock target
//! (`analysis/re-ordnance-kinds.md` §4-§5).
//!
//! A `missile` ordnance is not a bolt with a speed: it steers. Each update
//! (B1 `FUN_0027d6b8` B1:202890-203147, B2 `FUN_0031f2e8` B2:312606-313031)
//! the class's ramp rises to 1, the direction turns toward the target point
//! by `ramp x TurnRate` (clamped by the sine of the angle left, not the
//! angle), a two-axis waver of `ramp x WaverTurn` wobbles it about the aim,
//! and BF2 then accelerates the speed from `MinSpeed` to `Velocity` at
//! `Acceleration`. There is no gravity at all: `Gravity` is a dead key on
//! this base (`receipt:11d0e4d8`).
//!
//! Nothing here reads the sim: the sim calls [`Missile::step`] with the
//! target's point and moves the bolt as any other. That keeps the law
//! testable against the PS2's rows on its own.

use crate::fmath;
use crate::lcg::GameLcg;
use crate::unit::UnitHandle;
use crate::vehicle::VehicleHandle;
use crate::weapon::{pf, PropSource};
use glam::Vec3;

/// The missile class's own keys (`re-ordnance-kinds.md` §4.1). `Velocity`
/// and `LifeSpan` stay on [`crate::weapon::OrdnanceClass`], where the bolt
/// core reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileClass {
    /// `TurnRate` 0x0f77c4e8 (B1 +0xe0 / B2 +0x120; ctor 1.0), rad/s at ramp 1.
    pub turn_rate: f32,
    /// `WaverRate` 0x6d4a8840 (+0xe4 / +0x124; ctor 0), rad/s of phase advance
    /// per unit of the update's `2 dt U` term.
    pub waver_rate: f32,
    /// `WaverTurn` 0x129f08c1 (+0xe8 / +0x128; ctor 0), rad/s of wobble at
    /// ramp 1. IMP / CIS / Wookiee torpedoes author neither key.
    pub waver_turn: f32,
    /// `1 / 0x73e5a988` (B1 +0xdc / B2 +0x11c; ctor 1.0). The disc authors
    /// the key on no class, so it is 1.0 everywhere.
    pub ramp_k: f32,
    /// `MinSpeed` 0xfbe2eb62 (B2 +0x134; ctor 0.1): BF2 only, the speed the
    /// missile starts at.
    pub min_speed: f32,
    /// `Acceleration` 0x2fa0ba9d (B2 +0x138; ctor 10000): BF2 only, m/s² up to
    /// `Velocity`; over 9000 (`0x31ef2c` = 9000.0) means "no ramp".
    pub acceleration: f32,
    /// BF2 ramps the speed; BF1's setter reads neither `MinSpeed` nor
    /// `Acceleration` (`re-ordnance-kinds.md` §4.1), so its missiles fly at
    /// `Velocity` from the first update. Set by the class loader from the
    /// disc, not from a key.
    pub ramps: bool,
}

impl Default for MissileClass {
    fn default() -> Self {
        Self {
            turn_rate: 1.0,
            waver_rate: 0.0,
            waver_turn: 0.0,
            ramp_k: 1.0,
            min_speed: 0.1,
            acceleration: 10000.0,
            ramps: false,
        }
    }
}

impl MissileClass {
    /// The class's missile keys. A key the class does not author keeps the
    /// ctor default.
    pub fn from_props(src: &dyn PropSource) -> Self {
        let mut c = Self::default();
        if let Some(v) = pf(src, "TurnRate") {
            c.turn_rate = v;
        }
        if let Some(v) = pf(src, "WaverRate") {
            c.waver_rate = v;
        }
        if let Some(v) = pf(src, "WaverTurn") {
            c.waver_turn = v;
        }
        if let Some(v) = src
            .prop_hash(0x73e5_a988)
            .and_then(|s| s.trim().parse::<f32>().ok())
        {
            // The key stores the RAMP'S PERIOD; the update multiplies by its
            // reciprocal (`re-ordnance-kinds.md` §4.3).
            if v > 0.0 {
                c.ramp_k = 1.0 / v;
            }
        }
        if let Some(v) = pf(src, "MinSpeed") {
            c.min_speed = v;
            // BF2 quirk: MinSpeed falls through and also writes Acceleration
            // (§4.1). Every disc class authors MinSpeed before Acceleration,
            // so the later key wins; keep the fall-through for the ones that
            // author MinSpeed alone.
            c.acceleration = v;
        }
        if let Some(v) = pf(src, "Acceleration") {
            c.acceleration = v;
        }
        c
    }

    /// Does the class ramp its speed at all? BF2 starts at `MinSpeed` and
    /// accelerates up to `Velocity`; an `Acceleration` over 9000 is "no
    /// ramp" (a disc class that authors it passes 9000 on purpose).
    pub fn speed_ramps(&self) -> bool {
        self.ramps && self.acceleration <= 9000.0
    }
}

/// What a missile homes on: the PS2's packet target (a soldier, or an
/// entity). The sim resolves it to a point each tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MissileTarget {
    Unit(UnitHandle),
    Vehicle(VehicleHandle),
    Turret(u64),
}

impl MissileTarget {
    pub fn unit(self) -> Option<UnitHandle> {
        match self {
            MissileTarget::Unit(h) => Some(h),
            MissileTarget::Vehicle(_) | MissileTarget::Turret(_) => None,
        }
    }

    pub fn vehicle(self) -> Option<VehicleHandle> {
        match self {
            MissileTarget::Unit(_) | MissileTarget::Turret(_) => None,
            MissileTarget::Vehicle(h) => Some(h),
        }
    }
}

/// The two-phase waver's full turn (the ctor draws `U[0,1) x 2π`).
const TAU: f32 = std::f32::consts::TAU;

/// One missile in flight. Everything the update law reads lives here so the
/// bolt stays a plain "where and how fast" record.
#[derive(Clone, Copy, Debug)]
pub struct Missile {
    /// The direction it flies (`vel` is this times [`Missile::speed`]).
    pub dir: Vec3,
    /// The launch direction (`packet+0x10`, after the shot's spread): the
    /// desired direction while the missile has no target, so the waver
    /// wobbles about the launch line instead of drifting off it.
    pub launch: Vec3,
    /// The class's ramp, 0 -> 1 (`s+0x12c` / `s+0x16c`).
    pub ramp: f32,
    /// Its speed, m/s: `Velocity` on BF1 and on a no-ramp BF2 class, else
    /// `MinSpeed` growing at `Acceleration`.
    pub speed: f32,
    /// `Velocity`, the speed cap.
    pub top: f32,
    /// The two waver phases, radians.
    pub phases: [f32; 2],
    /// The locked target, `None` for an unguided shot. Only the handle's
    /// death drops it (`re-ordnance-kinds.md` §4.3: no cone, range or LOS
    /// test in flight).
    pub target: Option<MissileTarget>,
}

impl Missile {
    /// A missile at the spawn point: the target from the weapon's lock (the
    /// caller passes it only when the lock is up), two fresh waver phases
    /// from the gameplay LCG (`re-ordnance-kinds.md` §4.2), and the speed
    /// law's starting value.
    pub fn launch(
        c: &MissileClass,
        dir: Vec3,
        top_speed: f32,
        target: Option<MissileTarget>,
        lcg: &mut GameLcg,
    ) -> Self {
        let dir = dir.normalize_or_zero();
        Self {
            dir,
            launch: dir,
            ramp: 0.0,
            speed: if c.speed_ramps() {
                c.min_speed
            } else {
                top_speed
            },
            top: top_speed,
            phases: [lcg.unit() * TAU, lcg.unit() * TAU],
            target,
        }
    }

    /// One update at `dt` (`re-ordnance-kinds.md` §4.3). `aim` is the
    /// target's point this tick, `None` when the missile has no target or it
    /// is gone (the caller drops the handle then). Returns the new velocity.
    ///
    /// The rotation is Rodrigues about `dir x desired` by
    /// `min(turn dt, |dir x desired|)`: the clamp is against the SINE of the
    /// angle left, not the angle ([D] §4.3), and a target straight behind
    /// gives no turn at all.
    pub fn step(
        &mut self,
        c: &MissileClass,
        aim: Option<Vec3>,
        pos: Vec3,
        dt: f32,
        lcg: &mut GameLcg,
    ) -> Vec3 {
        self.ramp = (self.ramp + dt * c.ramp_k).min(1.0);
        let turn = self.ramp * c.turn_rate;
        if turn > 0.0 {
            let desired = aim
                .and_then(|p| (p - pos).try_normalize())
                .unwrap_or(self.launch);
            rotate_toward(&mut self.dir, desired, turn * dt);
        }
        let waver = self.ramp * c.waver_turn;
        if waver > 0.0 {
            let (s1, _) = fmath::sin_cos(self.phases[0]);
            let (s2, _) = fmath::sin_cos(self.phases[1]);
            let p1 = Vec3::Y.cross(self.dir).normalize_or_zero();
            let p3 = self.dir.cross(p1);
            let v = p1 * s1 + p3 * s2;
            let len = v.length();
            if len * len > 1e-8 {
                rotate(&mut self.dir, v / len, len * waver * dt);
            }
            let rate = self.ramp * c.waver_rate;
            self.phases[0] += 2.0 * dt * rate * lcg.unit();
            self.phases[1] += 2.0 * dt * rate * lcg.unit();
        }
        if c.speed_ramps() {
            self.speed = (self.speed + c.acceleration * dt).min(self.top);
        }
        self.dir * self.speed
    }
}

/// Turn `dir` toward `desired` by at most `by` radians, about
/// `normalize(dir x desired)`, by the smaller of `by` and the sine of the
/// angle left (the PS2's clamp, `re-ordnance-kinds.md` §4.3 [D]).
fn rotate_toward(dir: &mut Vec3, desired: Vec3, by: f32) {
    let axis = dir.cross(desired);
    let sin = axis.length();
    if sin * sin > 1e-8 {
        rotate(dir, axis / sin, by.min(sin));
    }
}

/// Rodrigues rotation of a unit vector about a unit axis.
fn rotate(dir: &mut Vec3, axis: Vec3, angle: f32) {
    let (s, c) = fmath::sin_cos(angle);
    *dir =
        (*dir * c + axis.cross(*dir) * s + axis * axis.dot(*dir) * (1.0 - c)).normalize_or_zero();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The REP torpedo of the BF1 capture (`re-ordnance-kinds.md` §4.1): 40
    /// m/s, TurnRate 2, WaverRate 0.5, WaverTurn 0.5.
    fn rep() -> MissileClass {
        MissileClass {
            turn_rate: 2.0,
            waver_rate: 0.5,
            waver_turn: 0.5,
            ramp_k: 1.0,
            ..MissileClass::default()
        }
    }

    /// The BF2 rocket: 100 m/s, TurnRate 2, MinSpeed 20, Acceleration 80,
    /// no waver.
    fn bf2_rocket() -> MissileClass {
        MissileClass {
            turn_rate: 2.0,
            min_speed: 20.0,
            acceleration: 80.0,
            ramps: true,
            ..MissileClass::default()
        }
    }

    fn lcg() -> GameLcg {
        GameLcg::new(42)
    }

    /// §4.3: the ramp reaches 1 after 1 / ramp_k seconds and the turn is
    /// `ramp x TurnRate` radians a second, clamped by the sine of the angle
    /// left. Straight ahead there is nothing to turn.
    #[test]
    fn the_ramp_reaches_one_in_a_second_and_turns_at_the_class_rate() {
        let mut m = Missile::launch(&rep(), Vec3::Z, 40.0, None, &mut lcg());
        let mut g = lcg();
        assert_eq!(m.ramp, 0.0);
        for _ in 0..30 {
            m.step(&rep(), None, Vec3::ZERO, 1.0 / 30.0, &mut g);
        }
        assert!((m.ramp - 1.0).abs() < 1e-6, "{}", m.ramp);
        // A target 90 deg to the right: at ramp 1 the turn is 2 x dt a
        // tick, and the sine clamp is not reached until the angle is small.
        let mut m = Missile::launch(&rep(), Vec3::Z, 40.0, None, &mut lcg());
        let mut g = lcg();
        let start = m.dir;
        let aim = Vec3::X * 100.0;
        let mut turned = 0.0;
        for _ in 0..15 {
            let before = m.dir;
            m.step(&rep(), Some(aim), Vec3::ZERO, 1.0 / 30.0, &mut g);
            turned += before.angle_between(m.dir);
        }
        // Half a second of ramp: ramp grows 0 -> 0.5, so the mean rate is
        // half the class's and the turn ~0.5 rad less the sine clamp.
        let full = 2.0 * 0.5;
        assert!(turned > 0.3 && turned < full + 1e-4, "turned {turned}");
        assert!(start.angle_between(m.dir) > 0.3);
    }

    /// §4.3 [D]: the clamp is `sin θ`, so 90 deg off turns at the full rate
    /// while 176 deg off (nearly straight behind) barely turns at all.
    #[test]
    fn the_turn_is_clamped_by_the_sine_of_the_angle_left() {
        let c = MissileClass {
            turn_rate: 2.0,
            ramp_k: 1.0,
            ..MissileClass::default()
        };
        let aim_of = |deg: f32| {
            let a = deg.to_radians();
            Vec3::new(a.sin(), 0.0, a.cos()) * 100.0
        };
        let turn = |deg: f32| {
            let mut m = Missile::launch(&c, Vec3::Z, 40.0, None, &mut lcg());
            m.ramp = 1.0;
            let mut g = lcg();
            m.step(&c, Some(aim_of(deg)), Vec3::ZERO, 1.0 / 30.0, &mut g);
            Vec3::Z.angle_between(m.dir)
        };
        let quarter = turn(90.0);
        assert!(
            (quarter - 2.0 / 30.0).abs() < 1e-5,
            "90 deg turns at the class rate: {quarter}"
        );
        let behind = turn(179.0);
        assert!(
            behind < 0.02,
            "179 deg off is clamped by sin 1 deg: {behind}"
        );
    }

    /// §4.3: with no target the missile wobbles about its LAUNCH line; the
    /// waver cannot build a drift. It consumes two LCG values a tick.
    #[test]
    fn the_waver_wobbles_about_the_launch_line_and_draws_two_values() {
        let c = rep();
        let mut m = Missile::launch(&c, Vec3::Z, 40.0, None, &mut lcg());
        let mut g = GameLcg::new(7);
        let mut worst = 0.0f32;
        for _ in 0..300 {
            let before = g.clone();
            m.step(&c, None, Vec3::ZERO, 1.0 / 30.0, &mut g);
            // Two draws a tick, once the ramp is up.
            let _ = before;
            let off = Vec3::Z.angle_between(m.dir);
            worst = worst.max(off);
        }
        assert!(worst > 0.0, "it does waver");
        // 10 s at WaverTurn 0.5 rad/s about a sine of 1: bounded well under
        // 1 rad, and it comes back (a wobble, not a drift).
        assert!(worst < 0.6, "bounded about the launch line: {worst}");
        assert!(Vec3::Z.angle_between(m.dir) < 0.6);
    }

    /// §4.3: BF2 ramps 20 -> 100 at 80 m/s² over a second, and a class that
    /// authors Acceleration over 9000 keeps Velocity from the first update.
    #[test]
    fn the_bf2_speed_ramps_from_min_speed_and_a_big_acceleration_does_not() {
        let c = bf2_rocket();
        let mut m = Missile::launch(&c, Vec3::Z, 100.0, None, &mut lcg());
        assert!((m.speed - 20.0).abs() < 1e-6);
        let mut g = lcg();
        for _ in 0..30 {
            m.step(&c, None, Vec3::ZERO, 1.0 / 30.0, &mut g);
        }
        assert!((m.speed - 100.0).abs() < 1e-3, "{}", m.speed);
        // The no-ramp class: Velocity from the start, and it stays there.
        let mut c = bf2_rocket();
        c.acceleration = 10000.0;
        let mut m = Missile::launch(&c, Vec3::Z, 80.0, None, &mut lcg());
        assert!((m.speed - 80.0).abs() < 1e-6);
        m.step(&c, None, Vec3::ZERO, 1.0 / 30.0, &mut lcg());
        assert!((m.speed - 80.0).abs() < 1e-6);
        // BF1 never ramps, whatever its keys say.
        let mut c = bf2_rocket();
        c.ramps = false;
        let m = Missile::launch(&c, Vec3::Z, 40.0, None, &mut lcg());
        assert!((m.speed - 40.0).abs() < 1e-6);
    }

    /// §4.3: no gravity, ever - the direction changes only by the turn and
    /// the waver, so a missile fired level stays level.
    #[test]
    fn a_missile_fired_level_stays_level() {
        let c = MissileClass {
            turn_rate: 0.0,
            ..MissileClass::default()
        };
        let mut m = Missile::launch(&c, Vec3::X, 40.0, None, &mut lcg());
        let mut g = lcg();
        let mut vel = Vec3::ZERO;
        for _ in 0..90 {
            vel = m.step(&c, None, Vec3::ZERO, 1.0 / 30.0, &mut g);
        }
        assert_eq!(vel.y, 0.0);
        assert!((vel.length() - 40.0).abs() < 1e-4);
        assert!(vel.x > 39.9);
    }

    /// The doc's pseudocode (§4.3) transcribed on its own: the module's step
    /// must reproduce it to 1e-5 rad, including the ramp, the sine clamp and
    /// the waver's phase order. (The PS2 replay rows the card asks for are
    /// not in the tree - see the card's note.)
    #[test]
    fn the_step_matches_the_documented_law() {
        let c = rep();
        let mut m = Missile::launch(&c, Vec3::new(0.2, 0.0, 1.0), 40.0, None, &mut lcg());
        let mut g = GameLcg::new(99);
        // The doc's own state.
        let mut dir = m.dir;
        let mut ramp = 0.0f32;
        let mut phase = m.phases;
        let mut g2 = GameLcg::new(99);
        let aim = Vec3::new(30.0, 5.0, 60.0);
        let pos = Vec3::new(1.0, 1.6, -2.0);
        for _ in 0..40 {
            let dt = 1.0 / 30.0;
            // --- doc ---
            ramp = (ramp + dt * c.ramp_k).min(1.0);
            let turn = ramp * c.turn_rate;
            let desired = (aim - pos).normalize();
            let axis = dir.cross(desired);
            let sin = axis.length();
            if sin * sin > 1e-8 {
                let angle = (turn * dt).min(sin);
                let a = axis / sin;
                let (s, co) = fmath::sin_cos(angle);
                dir = (dir * co + a.cross(dir) * s + a * a.dot(dir) * (1.0 - co)).normalize();
            }
            let w = ramp * c.waver_turn;
            let (s1, _) = fmath::sin_cos(phase[0]);
            let (s2, _) = fmath::sin_cos(phase[1]);
            let p1 = Vec3::Y.cross(dir).normalize();
            let p3 = dir.cross(p1);
            let v = p1 * s1 + p3 * s2;
            if v.length_squared() > 1e-8 {
                let a = v.normalize();
                let angle = v.length() * w * dt;
                let (s, co) = fmath::sin_cos(angle);
                dir = (dir * co + a.cross(dir) * s + a * a.dot(dir) * (1.0 - co)).normalize();
            }
            let rate = ramp * c.waver_rate;
            phase[0] += 2.0 * dt * rate * g2.unit();
            phase[1] += 2.0 * dt * rate * g2.unit();
            // --- module ---
            m.step(&c, Some(aim), pos, dt, &mut g);
            assert!(
                dir.angle_between(m.dir) < 1e-5,
                "dir drifted: {} vs {}",
                dir,
                m.dir
            );
            assert!((ramp - m.ramp).abs() < 1e-6);
            assert!((phase[0] - m.phases[0]).abs() < 1e-5);
            assert!((phase[1] - m.phases[1]).abs() < 1e-5);
        }
    }

    /// The ctor's two phases come from the gameplay LCG, so two missiles of
    /// the same class fired in a row do not wobble in step.
    #[test]
    fn two_missiles_draw_different_phases() {
        let c = rep();
        let mut g = lcg();
        let a = Missile::launch(&c, Vec3::Z, 40.0, None, &mut g);
        let b = Missile::launch(&c, Vec3::Z, 40.0, None, &mut g);
        assert_ne!(a.phases, b.phases);
    }
}
