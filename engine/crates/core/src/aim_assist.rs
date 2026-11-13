//! BF1 passive aim-assist magnetism, transcribed from the decomp
//! (BF1 FUN_0025fea8, lead-verified 2026-08-30; full spec in
//! analysis/re-aim-assist.md). Operates on stick values BEFORE they become
//! rotation, so assist can never out-turn the soldier.
//!
//! Three effects when a magnet target exists:
//! 1. sticky slowdown ×0.5 on both aim axes — ungated;
//! 2. yaw velocity tracking, rate-matched (dt-free) — ungated;
//! 3. fire magnetism toward the ballistic lead point, on a 1 s decay timer —
//!    the only gated term (`armed`: SP Auto Aim option / MP host setting).
//!
//! Pitch is never pulled, only slowed. The caller supplies the lead point
//! (BF1 solves ordnance intercept in FUN_0026d3f0 — ported when weapons land
//! at P4; dummy targets at P3 pass their own position).
//!
//! OURS (the maintainer, 2026-10-29): the tracking and pull terms are
//! clamped to [`speed_cap`] stick units, not the PS2's one: at most
//! [`ASSIST_SPEED_X_BF1`] times BF1's turn rate. The weight on both,
//! 1 - |yaw push|, reads the push before the pad's look scale (BF1's 0 at
//! a full push) and keeps a floor of [`W_FLOOR`].
//!
//! The intensity slider (PLAN §Movement & feel, an openbf addition) scales
//! the two PULL terms only; the 0.5 slowdown stays authentic at slider 1.0
//! and blends toward 1.0 (no slowdown) at slider 0.
//!
//! OURS, the lead (playtest 2026-10-28, BF1 Yavin 4: "the magnetic aim
//! assist [...] absolutely sucks when the guy you're shooting is actively
//! moving"; [`Lead`], [`AssistEnv::lead_vel`]). On the PS2 term 2 pulls a
//! share of the angle toward the lead point each frame and term 1 tracks
//! the target's motion only as far as the player pushes his own move stick,
//! so a player holding still on a runner trails him: at 30 m with a 80 m/s
//! rifle the pull closes 40 % a tick and the reticle sits about half a metre
//! behind the lead point; at 60 m it closes 5 % and never catches it. Ours:
//! the lead point is the target's centre plus its velocity, low-passed over
//! [`LEAD_VEL_TAU`], times the round's flight time, and that offset eases
//! in over [`LEAD_EASE_TAU`] at most [`LEAD_MAX_RATE`]; term 1 rate-matches
//! the lead point's sideways motion (the target's own, plus the offset's
//! easing) in full, whatever the move stick, while term 2's timer is live
//! (fire within the last [`MAGNET_TIMER_SECS`]; the maintainer, 2026-10-29:
//! no following a target the player is not shooting at).
//! So a runner who turns about is followed as he goes, and the lead swings
//! to his other side at no more than [`LEAD_MAX_RATE`]: the reticle's path
//! bends, it never jumps. The slowdown, term 2's gain, timer and fall-off
//! are the PS2's. During firing both shooter and lead motion use the same
//! full rate, so travelling together cancels (issue #164).
//! measure: controller tracking in all four shooter/target motion cases.
//!
//! OURS, the sniper's follow (playtest 2026-11-05 C5: "aim assist should
//! follow moving targets a little"; [`AssistEnv::follow`]). A sniper lines
//! his one shot up before the press, so with the rule above his magnet
//! only slows his stick while he does it. A scoped sniper rifle's term 1
//! matches [`SNIPER_FOLLOW`] of the lead point's sideways motion while the
//! pull is not live, under the Auto Aim option as the pull is; every other
//! weapon still follows nothing it is not shooting at.

use glam::Vec3;

/// Constants from FUN_0025fea8 (EE float literals, exact).
pub const SLOWDOWN: f32 = 0.5;
pub const GAIN: f32 = 0.75; // shared by tracking + magnetism
pub const MAGNET_TIMER_SECS: f32 = 1.0;

/// BF1's soldier turn rate (rad/s): the R its assist divides by, so its
/// tracking and pull, clamped to one stick unit, turn at most this fast.
pub const BF1_TURN_RATE: f32 = 0.675;
/// OURS (the maintainer, 2026-10-29): the assist's own turn is capped at
/// this many times BF1's. The port's stick unit is the class MaxTurnSpeed
/// (4.0 rad/s, ~5.9x BF1's), so the PS2's one-unit clamp let the assist
/// turn ~5.9x faster than on BF1.
pub const ASSIST_SPEED_X_BF1: f32 = 3.0;

/// OURS (the maintainer, 2026-10-29): the least weight the tracking and
/// pull keep under a full push. BF1's is 0 (a full stick breaks free);
/// the port's was 0.2, as the weight read the stick after `LOOK_SCALE`.
pub const W_FLOOR: f32 = 0.1;

/// The clamp on the tracking and pull terms in stick units: one stick unit
/// turns `turn_rate` (both divided by the zoom alike).
pub fn speed_cap(turn_rate: f32) -> f32 {
    (ASSIST_SPEED_X_BF1 * BF1_TURN_RATE / turn_rate.max(1e-3)).min(1.0)
}

/// Aim sticks in [-1, 1], post-deadzone. `yaw`/`pitch` are modified in place
/// by [`apply`]; `move_mag` is the length of both move axes, capped at one.
/// This is a port deviation: BF1 reads only the axis written to entity+0x64
/// (axis identity pending PCSX2 confirmation, report §8.3).
/// measure: controller strafe and diagonal tracking against the original game.
///
/// SIGN CONVENTION: `yaw` is POSITIVE-LEFT (turning toward -lat). This is
/// forced by the physics of term 1 — strafing along +lat makes the target
/// drift toward -lat in view, and the decomp ADDS the positive tracking
/// term — and makes term 2's `-=` pull toward the lead point. The absolute
/// pad-hardware sign is still a PCSX2 trace item (report §7); callers with
/// positive-right sticks negate on the way in and out.
#[derive(Debug, Clone, Copy)]
pub struct Sticks {
    pub move_mag: f32,
    pub yaw: f32,
    pub pitch: f32,
}

/// Everything FUN_0025fea8 reads about the world this tick.
#[derive(Debug, Clone, Copy)]
pub struct AssistEnv {
    /// Player world position (aim frame origin).
    pub self_pos: Vec3,
    pub self_vel: Vec3,
    /// Collision-prim radius of the player.
    pub self_radius: f32,
    /// Normalized aim forward.
    pub aim_fwd: Vec3,
    /// Normalized world lateral axis of the aim frame: normalize(up × fwd).
    pub lat: Vec3,
    /// Magnet target collision-prim center + velocity + radius.
    pub target_center: Vec3,
    pub target_vel: Vec3,
    pub target_radius: f32,
    /// Ballistically-led aim point (FUN_0026d3f0 output); the pull steers
    /// toward this, not the target center.
    pub lead_point: Option<Vec3>,
    /// Soldier max turn rate R (vcall vtbl294[0x0c]; MaxTurnSpeed per report).
    pub turn_rate: f32,
    /// Zoomed: R is divided by the weapon zoom factor (weapon+0xdc).
    pub zoom_factor: Option<f32>,
    /// Weapon LockOnRange (weaponClass+0xd4); <= 0 disables the weapon's
    /// assist upstream (no magnet target is ever acquired).
    pub lock_on_range: f32,
    pub fire_held: bool,
    /// Term-3 gate: SP Auto Aim option / MP host "aimassist" setting.
    pub armed: bool,
    /// OURS (the module doc's lead): the velocity of the point the pull aims
    /// at (the target's, plus its lead offset's easing), which term 1
    /// rate-matches in full while the pull's timer is live; `None` = the
    /// PS2's term 1 (unless `follow` applies).
    pub lead_vel: Option<Vec3>,
    /// OURS (the module doc's sniper follow): the share of relative sideways
    /// motion term 1 matches while the pull is not live and Auto Aim is on;
    /// 0 = the PS2's term 1. measure: scoped controller crossing-target play.
    pub follow: f32,
    /// The yaw stick a full push delivers on this pad (`LOOK_SCALE` for a
    /// human pad, 1 for a raw one): the weight reads the push before it.
    pub look_scale: f32,
}

/// Persistent per-controller state (ctrl+0x30 in BF1).
#[derive(Debug, Clone, Copy, Default)]
pub struct AssistState {
    pub magnet_timer: f32,
}

/// OURS (the maintainer, playtest 2026-11-05 C5: follow "a little"): the
/// share of a moving magnet a scoped sniper rifle's assist follows before
/// the press ([`AssistEnv::follow`]). PROVISIONAL. measure: a controller
/// playtest, scoped on a crossing runner at 50 and 100 m.
pub const SNIPER_FOLLOW: f32 = 0.5;

/// OURS, PROVISIONAL: sniper assist stays full through half its nominal
/// range, then all three terms fade linearly to zero at LockOnRange.
/// measure: scoped controller tracking at 50, 64, 96, 120 and 128 m.
pub const SNIPER_FADE_START: f32 = 0.5;

/// The sniper's shared range taper, including the acquisition bounds fringe.
/// Sniper magnets are a port addition; this does not alter ordinary weapons.
pub(crate) fn sniper_range_scale(distance: f32, range: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    ((1.0 - distance / range) / (1.0 - SNIPER_FADE_START)).clamp(0.0, 1.0)
}

/// OURS: the time constant of the lead's low-pass on the target's velocity
/// (s): a runner who turns about swings the lead over about this long.
pub const LEAD_VEL_TAU: f32 = 0.25;
/// OURS: the time constant the lead offset eases toward its goal with (s).
pub const LEAD_EASE_TAU: f32 = 0.1;
/// OURS: the fastest the lead offset moves (m/s): a new magnet's lead
/// grows in, never at once.
pub const LEAD_MAX_RATE: f32 = 10.0;

/// OURS (the module doc's lead): one player slot's lead on its magnet - the
/// target's low-passed velocity and the offset from its centre the pull
/// aims at. A new magnet starts from its velocity as it is and no offset.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Lead {
    pub vel: Vec3,
    pub offset: Vec3,
}

impl Lead {
    pub fn start(vel: Vec3) -> Self {
        Lead {
            vel,
            offset: Vec3::ZERO,
        }
    }

    /// One tick of `dt`: low-pass the velocity toward `vel`, then ease the
    /// offset toward `goal(low-passed velocity)` (the velocity times the
    /// round's flight time, and the drop a falling round makes up). Returns
    /// the offset's own velocity this tick.
    pub fn step(&mut self, dt: f32, vel: Vec3, goal: impl FnOnce(Vec3) -> Vec3) -> Vec3 {
        self.vel += (vel - self.vel) * (dt / (LEAD_VEL_TAU + dt));
        let want = goal(self.vel);
        let ease = (want - self.offset) * (dt / (LEAD_EASE_TAU + dt));
        let ease = ease.clamp_length_max(LEAD_MAX_RATE * dt);
        self.offset += ease;
        ease / dt
    }

    pub(crate) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [self.vel, self.offset] {
            for c in v.to_array() {
                eat(&c.to_bits().to_le_bytes());
            }
        }
    }
}

/// One tick of FUN_0025fea8, given a valid magnet target. `intensity` = 1.0
/// gives the full configured port assist; the caller skips this entirely when
/// no target exists.
pub fn apply(dt: f32, sticks: &mut Sticks, env: &AssistEnv, st: &mut AssistState, intensity: f32) {
    // Weight from how hard the player is already steering (1 - |yaw push|),
    // the push read before the pad's look scale. [ours] floor: BF1 skips
    // the pull and tracking at w = 0 (a full push).
    let push = (sticks.yaw.abs() / env.look_scale.max(1e-3)).min(1.0);
    let w = (1.0 - push).max(W_FLOOR);

    // Sticky slowdown (blend toward no-op as intensity drops below 1).
    let slow = 1.0 + (SLOWDOWN - 1.0) * intensity;
    sticks.yaw *= slow;
    sticks.pitch *= slow;

    let r = match env.zoom_factor {
        Some(z) => env.turn_rate / z,
        None => env.turn_rate,
    };
    // [ours] ±cap where the PS2 clamps to ±1 (`speed_cap`).
    let cap = speed_cap(env.turn_rate);

    // Term 2 timer: refilled while fire is held, if armed. Refilled before
    // term 1 so the lead's tracking sees this tick's press.
    if env.armed && env.fire_held {
        st.magnet_timer = MAGNET_TIMER_SECS;
    }

    // Term 1: velocity tracking (yaw only, rate-matched, no dt). With the
    // lead (ours), match full relative velocity while the pull is live
    // (fire within MAGNET_TIMER_SECS). Not pulling, a scoped sniper matches
    // `follow` of relative motion ([ours] C5), or the PS2's share where
    // that is larger (either movement axis).
    // measure: controller tracking on crossing and co-moving targets.
    let fwd_dist = (env.target_center - env.self_pos).dot(env.aim_fwd);
    let pulling = env.armed && st.magnet_timer > 0.0;
    let share = match pulling {
        true => env.lead_vel.map(|v| (v, 1.0)),
        false => (env.armed && env.follow > 0.0).then(|| {
            let ps2 = sticks.move_mag.abs() * GAIN;
            (env.lead_vel.unwrap_or(env.target_vel), env.follow.max(ps2))
        }),
    };
    let track = match share {
        None => {
            let lat_vel = (env.self_vel - env.target_vel).dot(env.lat);
            (sticks.move_mag.abs() * w * GAIN * lat_vel) / (fwd_dist * r)
        }
        Some((v, k)) => {
            // Match relative motion with the same weight on shooter and
            // lead: travelling together must not drift the reticle. The
            // firing extension already follows a still shooter's target in
            // full; it must also compensate the moving shooter's view.
            w * k * (env.self_vel - v).dot(env.lat) / (fwd_dist * r)
        }
    };
    sticks.yaw += track.clamp(-cap, cap) * intensity;

    if st.magnet_timer > 0.0 {
        let mut pull = w * st.magnet_timer * GAIN;
        let radii = env.target_radius + env.self_radius;
        let range = env.lock_on_range;
        if radii < fwd_dist && radii < range {
            pull *= (1.0 - (fwd_dist - radii) / (range - radii)).clamp(0.0, 1.0);
        }
        if pull > 0.0 {
            if let Some(lead) = env.lead_point {
                let dir = (lead - env.self_pos).normalize_or_zero();
                let step = (dir.dot(env.lat) / (r * dt)).clamp(-cap, cap);
                sticks.yaw -= pull * step * intensity;
            }
        }
        // Decay runs whenever the timer is live, even with zero pull
        // (falloff edge) — matches the decomp's block structure.
        st.magnet_timer -= dt;
    }
}

#[cfg(test)]
mod tests {
    //! Reference vectors T1-T6 from analysis/re-aim-assist.md §7, hand-computed
    //! from the FUN_0025fea8 algorithm.
    use super::*;

    const DT: f32 = 1.0 / 30.0;
    const EPS: f32 = 1e-5;

    fn env() -> AssistEnv {
        AssistEnv {
            self_pos: Vec3::ZERO,
            self_vel: Vec3::ZERO,
            self_radius: 0.5,
            aim_fwd: Vec3::NEG_Z,
            lat: Vec3::X,
            target_center: Vec3::new(0.0, 0.0, -20.0),
            target_vel: Vec3::ZERO,
            target_radius: 0.5,
            lead_point: None,
            turn_rate: 5.0,
            zoom_factor: None,
            lock_on_range: 50.0,
            fire_held: false,
            armed: false,
            lead_vel: None,
            follow: 0.0,
            look_scale: 1.0,
        }
    }

    #[test]
    fn t1_slowdown_only() {
        let mut s = Sticks {
            move_mag: 0.0,
            yaw: 0.4,
            pitch: 0.6,
        };
        let mut st = AssistState::default();
        apply(DT, &mut s, &env(), &mut st, 1.0);
        assert!((s.yaw - 0.2).abs() < EPS, "yaw {}", s.yaw);
        assert!((s.pitch - 0.3).abs() < EPS, "pitch {}", s.pitch);
        assert_eq!(st.magnet_timer, 0.0);
    }

    /// [ours]: a full stick keeps the floor's share, not BF1's bypass
    /// (here 0.1 x 0.0375 of tracking, the move stick's strafe).
    #[test]
    fn t2_full_stick_keeps_only_the_floor() {
        let mut e = env();
        e.self_vel = Vec3::X * 5.0; // would produce tracking if not bypassed
        let mut s = Sticks {
            move_mag: 1.0,
            yaw: 1.0,
            pitch: 0.6,
        };
        let mut st = AssistState::default();
        apply(DT, &mut s, &e, &mut st, 1.0);
        assert!(
            (s.yaw - (0.5 + W_FLOOR * 0.0375)).abs() < EPS,
            "yaw {}",
            s.yaw
        );
        assert!((s.pitch - 0.3).abs() < EPS);
    }

    #[test]
    fn t3_strafe_tracking() {
        let mut e = env();
        e.self_vel = Vec3::X * 5.0;
        let mut s = Sticks {
            move_mag: 1.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut st = AssistState::default();
        apply(DT, &mut s, &e, &mut st, 1.0);
        // (1 * 1 * 0.75 * 5) / (20 * 5) = 0.0375
        assert!((s.yaw - 0.0375).abs() < EPS, "yaw {}", s.yaw);

        // Doubling distance halves it.
        e.target_center.z = -40.0;
        let mut s2 = Sticks {
            move_mag: 1.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        apply(DT, &mut s2, &e, &mut AssistState::default(), 1.0);
        assert!((s2.yaw - 0.01875).abs() < EPS);
    }

    #[test]
    fn t4_fire_magnetism_pulls_toward_lead() {
        let mut e = env();
        e.armed = true;
        e.fire_held = true;
        // Lead point with lateral direction-sine 0.1: unit dir has dot(lat)=0.1.
        let d = 30.0_f32;
        e.lead_point = Some(Vec3::new(0.1 * d, 0.0, -(1.0 - 0.01_f32).sqrt() * d));
        let mut s = Sticks {
            move_mag: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut st = AssistState::default();
        apply(DT, &mut s, &e, &mut st, 1.0);
        // pull = 1 * 1 * 0.75 * (1 - 19/49); step = clamp(0.1/(5/30)) = 0.6,
        // over the [ours] cap 3 x 0.675 / 5 = 0.405.
        let pull = 0.75 * (1.0 - 19.0 / 49.0);
        assert!((s.yaw - (-pull * 0.405)).abs() < 1e-4, "yaw {}", s.yaw);
        assert!((st.magnet_timer - (1.0 - DT)).abs() < EPS);
    }

    #[test]
    fn t5_falloff_edge_no_pull_but_timer_decays() {
        let mut e = env();
        e.armed = true;
        e.fire_held = true;
        e.target_center.z = -50.0; // fwdDist == LockOnRange -> fade 0
        e.lead_point = Some(Vec3::new(5.0, 0.0, -49.0));
        let mut s = Sticks {
            move_mag: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut st = AssistState::default();
        apply(DT, &mut s, &e, &mut st, 1.0);
        assert_eq!(s.yaw, 0.0, "no pull at range edge");
        assert!((st.magnet_timer - (1.0 - DT)).abs() < EPS, "timer decays");
    }

    #[test]
    fn t6_unarmed_keeps_slowdown_and_tracking_only() {
        let mut e = env();
        e.armed = false; // Auto Aim OFF
        e.fire_held = true;
        e.self_vel = Vec3::X * 5.0;
        e.lead_point = Some(Vec3::new(3.0, 0.0, -20.0));
        let mut s = Sticks {
            move_mag: 1.0,
            yaw: 0.0,
            pitch: 0.4,
        };
        let mut st = AssistState::default();
        apply(DT, &mut s, &e, &mut st, 1.0);
        assert!((s.yaw - 0.0375).abs() < EPS, "tracking still on: {}", s.yaw);
        assert!((s.pitch - 0.2).abs() < EPS, "slowdown still on");
        assert_eq!(st.magnet_timer, 0.0, "timer never refills unarmed");
    }

    #[test]
    fn intensity_zero_is_a_no_op() {
        let mut e = env();
        e.armed = true;
        e.fire_held = true;
        e.self_vel = Vec3::X * 5.0;
        e.lead_point = Some(Vec3::new(3.0, 0.0, -20.0));
        let mut s = Sticks {
            move_mag: 1.0,
            yaw: 0.4,
            pitch: 0.6,
        };
        apply(DT, &mut s, &e, &mut AssistState::default(), 0.0);
        assert_eq!((s.yaw, s.pitch), (0.4, 0.6));
    }

    /// The lead (ours): the offset grows in from nothing at no more than
    /// LEAD_MAX_RATE, settles on its goal, and a velocity that turns about
    /// swings it over the low-pass rather than at once.
    #[test]
    fn the_lead_eases_in_and_swings_smoothly() {
        let flight = 0.4;
        let mut l = Lead::start(Vec3::X * 7.0);
        let mut last = l.offset;
        for _ in 0..60 {
            let rate = l.step(DT, Vec3::X * 7.0, |v| v * flight);
            assert!((l.offset - last).length() <= LEAD_MAX_RATE * DT + 1e-6);
            assert!((rate * DT - (l.offset - last)).length() < 1e-6);
            last = l.offset;
        }
        assert!((l.offset.x - 2.8).abs() < 0.01, "settled: {}", l.offset);
        // He turns about: one tick later the lead has barely moved, and it
        // never steps further than the cap.
        l.step(DT, Vec3::X * -7.0, |v| v * flight);
        assert!(l.offset.x > 2.4, "no flip: {}", l.offset);
        last = l.offset;
        for _ in 0..60 {
            l.step(DT, Vec3::X * -7.0, |v| v * flight);
            assert!((l.offset - last).length() <= LEAD_MAX_RATE * DT + 1e-6);
            last = l.offset;
        }
        assert!((l.offset.x + 2.8).abs() < 0.05, "swung: {}", l.offset);
    }

    /// With the lead's velocity, term 1 matches the target's crossing in
    /// full with the move stick at rest (the PS2's needs the move stick),
    /// and leaves the PS2's own-motion share alone.
    #[test]
    fn term_one_with_the_lead_tracks_a_crossing_target_at_rest() {
        let mut e = env();
        e.armed = true;
        e.target_vel = Vec3::X * 5.0;
        let still = Sticks {
            move_mag: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut ps2 = still;
        apply(DT, &mut ps2, &e, &mut AssistState::default(), 1.0);
        assert_eq!(ps2.yaw, 0.0, "the PS2's term 1 needs the move stick");
        e.lead_vel = Some(Vec3::X * 5.0);
        e.fire_held = true;
        let mut ours = still;
        apply(DT, &mut ours, &e, &mut AssistState::default(), 1.0);
        // −5 / (20 × 5): the target's whole angular rate, turning right.
        assert!((ours.yaw + 0.05).abs() < EPS, "yaw {}", ours.yaw);
        // Unarmed (Auto Aim off): the PS2's term 1.
        e.armed = false;
        let mut off = still;
        apply(DT, &mut off, &e, &mut AssistState::default(), 1.0);
        assert_eq!(off.yaw, 0.0);
    }

    /// The lead's tracking runs only while the pull's timer is live (fire
    /// within the last second), as BF1's pull; otherwise term 1 is the PS2's.
    #[test]
    fn the_lead_tracks_only_while_the_pull_is_live() {
        let mut e = env();
        e.armed = true;
        e.target_vel = Vec3::X * 5.0;
        e.lead_vel = Some(Vec3::X * 5.0);
        let still = Sticks {
            move_mag: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut st = AssistState::default();
        let mut idle = still;
        apply(DT, &mut idle, &e, &mut st, 1.0);
        assert_eq!(idle.yaw, 0.0, "no fire yet: the PS2's term 1");
        e.fire_held = true;
        let mut firing = still;
        apply(DT, &mut firing, &e, &mut st, 1.0);
        assert!((firing.yaw + 0.05).abs() < EPS, "firing: {}", firing.yaw);
        e.fire_held = false;
        let mut after = still;
        apply(DT, &mut after, &e, &mut st, 1.0);
        assert!((after.yaw + 0.05).abs() < EPS, "timer live: {}", after.yaw);
        for _ in 0..40 {
            apply(DT, &mut still.clone(), &e, &mut st, 1.0);
        }
        let mut late = still;
        apply(DT, &mut late, &e, &mut st, 1.0);
        assert_eq!(late.yaw, 0.0, "timer out: the PS2's term 1");
    }

    #[test]
    fn firing_tracking_matches_relative_motion_in_all_four_cases() {
        for (own, target) in [(0.0, 0.0), (5.0, 0.0), (0.0, 5.0), (5.0, 5.0)] {
            let mut e = env();
            e.armed = true;
            e.fire_held = true;
            e.self_vel = Vec3::X * own;
            e.target_vel = Vec3::X * target;
            e.lead_vel = Some(e.target_vel);
            for movement in [0.0, 1.0] {
                let mut s = Sticks {
                    move_mag: movement,
                    yaw: 0.0,
                    pitch: 0.0,
                };
                apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
                let want = (own - target) / (20.0 * e.turn_rate);
                assert!(
                    (s.yaw - want).abs() < EPS,
                    "own {own} target {target} move {movement}: {} vs {want}",
                    s.yaw
                );
            }
        }
    }

    /// [ours] C5: a scoped sniper (`follow`) matches its share of the lead
    /// point's motion before the press, in full once the pull is live, and
    /// nothing with Auto Aim off.
    #[test]
    fn a_scoped_sniper_follows_a_share_before_the_press() {
        let still = Sticks {
            move_mag: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let run = |armed: bool, follow: f32, fire: bool| {
            let mut e = env();
            e.armed = armed;
            e.target_vel = Vec3::X * 5.0;
            e.lead_vel = Some(Vec3::X * 5.0);
            e.follow = follow;
            e.fire_held = fire;
            let mut s = still;
            apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
            s.yaw
        };
        assert!(
            (run(true, 0.5, false) + 0.025).abs() < EPS,
            "half of firing's"
        );
        assert!((run(true, 0.5, true) + 0.05).abs() < EPS, "firing: in full");
        assert_eq!(run(false, 0.5, false), 0.0, "Auto Aim off");
        assert_eq!(run(true, 0.0, false), 0.0, "not a sniper: the PS2's");
    }

    /// The assist's own turn is capped at three times BF1's (0.675 rad/s):
    /// the tracking and the pull each at most 3 x 0.675 / R stick units,
    /// zoomed or not (the zoom divides the assist's rate and the cap alike).
    #[test]
    fn the_assist_turns_no_faster_than_three_times_bf1() {
        let cap = 3.0 * 0.675 / 5.0;
        let still = Sticks {
            move_mag: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        for zoom in [None, Some(4.0)] {
            // Tracking a 50 m/s crossing at 20 m: 0.5 stick unzoomed.
            let mut e = env();
            e.zoom_factor = zoom;
            e.armed = true;
            e.fire_held = true;
            e.target_vel = Vec3::X * 50.0;
            e.lead_vel = Some(Vec3::X * 50.0);
            let mut s = still;
            apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
            assert!((s.yaw + cap).abs() < EPS, "{zoom:?} track {}", s.yaw);
            // The pull toward a lead point 0.1 rad off: 0.6 stick unzoomed.
            let mut e = env();
            e.zoom_factor = zoom;
            e.armed = true;
            e.fire_held = true;
            let d = 30.0_f32;
            e.lead_point = Some(Vec3::new(0.1 * d, 0.0, -(1.0 - 0.01_f32).sqrt() * d));
            let mut s = still;
            apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
            let pull = 0.75 * (1.0 - 19.0 / 49.0);
            assert!((s.yaw + pull * cap).abs() < 1e-4, "{zoom:?} pull {}", s.yaw);
        }
    }

    /// The weight reads the push before the pad's look scale, so a full
    /// stick gives BF1's 0, kept at a floor of 0.1 ([ours]): a full push
    /// on a human pad (0.8 after `LOOK_SCALE`) leaves a tenth of the pull.
    #[test]
    fn the_weight_reads_the_push_before_the_look_scale_with_a_floor() {
        let mut e = env();
        e.armed = true;
        e.fire_held = true;
        e.target_vel = Vec3::X * 5.0;
        e.lead_vel = Some(Vec3::X * 5.0);
        e.look_scale = 0.8;
        // (stick, weight): full push, half push.
        for (yaw, w) in [(0.8, 0.1), (0.4, 0.5)] {
            let mut s = Sticks {
                move_mag: 0.0,
                yaw,
                pitch: 0.0,
            };
            apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
            let want = yaw * 0.5 - w * 0.05;
            assert!(
                (s.yaw - want).abs() < EPS,
                "stick {yaw}: {} vs {want}",
                s.yaw
            );
        }
        // Issue #142: a look sensitivity of 0.5 on the human pad: its full
        // push (0.4) still gives the floor, its half push half the pull.
        e.look_scale = 0.8 * 0.5;
        for (yaw, w) in [(0.4, 0.1), (0.2, 0.5)] {
            let mut s = Sticks {
                move_mag: 0.0,
                yaw,
                pitch: 0.0,
            };
            apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
            let want = yaw * 0.5 - w * 0.05;
            assert!(
                (s.yaw - want).abs() < EPS,
                "sensitivity 0.5, stick {yaw}: {} vs {want}",
                s.yaw
            );
        }
        // A raw pad's full stick: the floor, not BF1's bypass.
        e.look_scale = 1.0;
        let mut s = Sticks {
            move_mag: 0.0,
            yaw: 1.0,
            pitch: 0.0,
        };
        apply(DT, &mut s, &e, &mut AssistState::default(), 1.0);
        assert!(
            (s.yaw - (0.5 - 0.1 * 0.05)).abs() < EPS,
            "raw full: {}",
            s.yaw
        );
    }
}
