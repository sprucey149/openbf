//! BF2's animated prop (EntityPropAnimated): a placed or carried model that
//! loops its idle clip, plays another clip once when something comes near a
//! trigger hardpoint, can catch a soldier with a clip and kill him when the
//! clip ends, and moves its collision prims with its bones. Jabba's Palace
//! has four: the rancor in its pit, Jabba on his dais, the torture droid and
//! the trapdoor in front of Jabba that drops onto the pit.
//!
//! The rule is the disc's update, B2 `FUN_002312c0` [V][O] (the class's
//! lines are `bake::animated_prop`'s), each tick in this order:
//! 1. The first tick sets clip 0 (the idle), looping, at the prop's start
//!    time: a random point of the idle (the constructor's draw).
//! 2. A one-shot clip past its end (`time > frames/30`): time and blend go
//!    to 0, a held soldier dies (no killer; `KillSoldierSound` plays at the
//!    prop) and the idle loops again.
//! 3. While looping with nobody held, each trigger whose clip is not the
//!    current one, once the blend is at least 0.5, asks for anything within
//!    its radius of its hardpoint (the bind pose's): a hit starts that clip
//!    once, from time 0 and blend 0.
//! 4. With nobody held, each catch whose clip is playing, while the clip is
//!    no further in than its time, asks for a living soldier within its
//!    radius of its hardpoint on the POSED model: he is held.
//! 5. The clock steps by dt (the idle waits `IdleDelay` at its end), the
//!    idle wraps, the clip's pose is sampled, and the drawn pose moves to it
//!    by the blend — `pose += (clip − pose)·blend`, the whole clip once the
//!    blend reaches 1 — and the blend grows by the step (a fifth of it while
//!    the idle plays).
//! 6. A held soldier rides the catch hardpoint.
//!
//! Where the port differs, PROVISIONAL `[O]`:
//! - "anything" and "a soldier" are the LIVE units' feet (`soldier.pos`),
//!   tested `d² < r²` like the doors' (`crate::door`); the disc's collision
//!   query and its object filter are not traced;
//! - the start time is a fixed hash of the prop's index, not the disc's
//!   global random draw, so every peer starts the same;
//! - a pose between two frames interpolates them (translation lerp,
//!   rotation nlerp) and a clip's last frame holds to its end;
//! - the held soldier's feet are 1 m under the hardpoint (measure: the PS2
//!   puts his `bone_root` on it); he plays no `thrown_flail` and cannot act;
//! - the death is a death region's (`Sim::kill_environment`): one ticket.

use crate::fmath::FloatExt;
use crate::unit::UnitHandle;
use glam::{Quat, Vec3};
use std::sync::Arc;

/// The clips' frame rate (`FUN_002312c0`'s `* 0.033333335`).
pub const FPS: f32 = 30.0;
/// Where a held soldier's feet hang under the catch hardpoint. PROVISIONAL,
/// measure: the PS2's soldier `bone_root` height in `thrown_flail`.
pub const HELD_DROP: f32 = 1.0;

/// One trigger sphere (world space) and the clip it starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub clip: u8,
    pub center: Vec3,
    pub radius: f32,
}

/// One catch: its clip, radius, last time and its hardpoint's world
/// position at every frame of every clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Catch {
    pub clip: u8,
    pub radius: f32,
    pub time: f32,
    pub track: Vec<Vec<Vec3>>,
}

/// One collision body the bones move: its index in
/// [`crate::collision::CollisionWorld::bodies`] and its world pose at every
/// frame of every clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    pub body: usize,
    pub frames: Vec<Vec<(Quat, Vec3)>>,
}

/// What a prop is: SETUP data, shared between clones and not hashed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Setup {
    /// Each clip's frame count, clip 0 the idle.
    pub clips: Vec<u16>,
    pub triggers: Vec<Trigger>,
    pub catches: Vec<Catch>,
    pub bodies: Vec<Body>,
    pub idle_delay: f32,
    /// Where its sounds play.
    pub origin: Vec3,
    pub kill_sound: Option<String>,
    /// For the renderer: the manifest instance it is (or that carries it),
    /// its class and its world matrix.
    pub instance: usize,
    pub class: String,
    pub model: glam::Mat4,
    /// The prop IS its instance (not carried by it): the static draw of
    /// the instance gives way to the animated one.
    pub placed: bool,
}

/// One placed animated prop.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimatedProp {
    pub setup: Arc<Setup>,
    /// The clip playing; `None` before the first tick.
    pub anim: Option<u8>,
    pub time: f32,
    pub blend: f32,
    pub looping: bool,
    pub idle_timer: f32,
    /// The soldier held, and by which catch.
    pub held: Option<(UnitHandle, u8)>,
    /// The drawn pose of each catch hardpoint and each body.
    pub points: Vec<Vec3>,
    pub poses: Vec<(Quat, Vec3)>,
}

fn length(frames: u16) -> f32 {
    f32::from(frames) / FPS
}

/// A track at `t` seconds: the frame `t·30`, two frames between
/// interpolated, the last held.
fn at<T: Copy>(track: &[T], t: f32, mix: impl Fn(T, T, f32) -> T) -> T {
    let last = track.len() - 1;
    let x = (t * FPS).fmax(0.0);
    let i = (x as usize).min(last);
    let frac = x - i as f32;
    if i == last || frac <= 0.0 {
        track[i]
    } else {
        mix(track[i], track[i + 1], frac)
    }
}

fn lerp_v(a: Vec3, b: Vec3, f: f32) -> Vec3 {
    a.lerp(b, f)
}

fn lerp_q((qa, ta): (Quat, Vec3), (qb, tb): (Quat, Vec3), f: f32) -> (Quat, Vec3) {
    let qb = if qa.dot(qb) < 0.0 { -qb } else { qb };
    ((qa * (1.0 - f) + qb * f).normalize(), ta.lerp(tb, f))
}

impl AnimatedProp {
    pub fn new(setup: Setup, index: usize) -> Self {
        let n = (setup.catches.len(), setup.bodies.len());
        // [ours] the constructor's random start, as a fixed hash.
        let h = (index as u32).wrapping_add(1).wrapping_mul(0x9E37_79B9) >> 16;
        let start = setup
            .clips
            .first()
            .map_or(0.0, |&f| h as f32 / 65536.0 * length(f));
        Self {
            setup: Arc::new(setup),
            anim: None,
            time: start,
            blend: 1.0,
            looping: false,
            idle_timer: 0.0,
            held: None,
            points: vec![Vec3::ZERO; n.0],
            poses: vec![(Quat::IDENTITY, Vec3::ZERO); n.1],
        }
    }

    fn clip_time(&self) -> (usize, f32) {
        let k = usize::from(self.anim.unwrap_or(0));
        (k, self.time)
    }

    /// Move the drawn pose toward the current clip by the blend.
    fn pose(&mut self) {
        let (k, t) = self.clip_time();
        let setup = self.setup.clone();
        let whole = self.blend >= 1.0;
        for (c, p) in setup.catches.iter().zip(self.points.iter_mut()) {
            let Some(track) = c.track.get(k).filter(|t| !t.is_empty()) else {
                continue;
            };
            let d = at(track, t, lerp_v);
            *p = if whole { d } else { p.lerp(d, self.blend) };
        }
        for (b, p) in setup.bodies.iter().zip(self.poses.iter_mut()) {
            let Some(track) = b.frames.get(k).filter(|t| !t.is_empty()) else {
                continue;
            };
            let d = at(track, t, lerp_q);
            *p = if whole { d } else { lerp_q(*p, d, self.blend) };
        }
    }

    /// Set clip `k` (`SetAnimation`): nothing when it already plays.
    fn set(&mut self, k: u8, looping: bool) {
        if self.anim == Some(k) {
            return;
        }
        self.looping = looping;
        self.anim = Some(k);
        self.pose();
    }

    /// Steps 1–2: the first tick's idle, and a one-shot clip's end. Returns
    /// the soldier who dies now.
    pub fn begin(&mut self) -> Option<UnitHandle> {
        if self.anim.is_none() {
            self.blend = 1.0;
            self.set(0, true);
        }
        let mut dies = None;
        if !self.looping {
            let k = usize::from(self.anim.unwrap_or(0));
            let len = self.setup.clips.get(k).copied().map_or(0.0, length);
            if self.time > len {
                self.time = 0.0;
                self.blend = 0.0;
                dies = self.held.take().map(|(h, _)| h);
                self.set(0, true);
            }
        } else if let Some((h, _)) = self.held.take() {
            dies = Some(h);
        }
        dies
    }

    /// Step 3: with `near(center, radius)` asking whether anything stands in
    /// a sphere. Returns the clip started, if any.
    pub fn trigger(&mut self, near: impl Fn(Vec3, f32) -> bool) -> Option<u8> {
        let setup = self.setup.clone();
        for t in &setup.triggers {
            if !self.looping || self.held.is_some() {
                break;
            }
            if self.anim == Some(t.clip) || self.blend < 0.5 {
                continue;
            }
            if near(t.center, t.radius) {
                self.time = 0.0;
                self.blend = 0.0;
                self.set(t.clip, false);
                return Some(t.clip);
            }
        }
        None
    }

    /// Step 4: `soldier(point, radius)` finds a living soldier in a sphere.
    pub fn catch(&mut self, soldier: impl Fn(Vec3, f32) -> Option<UnitHandle>) {
        if self.held.is_some() {
            return;
        }
        let setup = self.setup.clone();
        for (i, c) in setup.catches.iter().enumerate() {
            if self.anim != Some(c.clip) || self.time > c.time {
                continue;
            }
            if let Some(h) = soldier(self.points[i], c.radius) {
                self.held = Some((h, i as u8));
                return;
            }
        }
    }

    /// Step 5: the clock, the pose and the blend.
    pub fn advance(&mut self, dt: f32) {
        let idle = self.setup.clips.first().copied().map_or(0.0, length);
        let mut step = dt;
        if self.looping && self.time + dt >= idle {
            self.idle_timer += dt;
            if self.idle_timer > self.setup.idle_delay {
                self.idle_timer = 0.0;
            } else {
                step = 0.0;
            }
        }
        self.time += step;
        if self.looping && idle > 0.0 && self.time > idle {
            self.time -= idle;
        }
        self.pose();
        self.blend += if self.anim == Some(0) {
            step * 0.2
        } else {
            step
        };
    }

    /// Step 6: where the held soldier's feet are.
    pub fn held_at(&self) -> Option<(UnitHandle, Vec3)> {
        let (h, i) = self.held?;
        let p = *self.points.get(usize::from(i))?;
        Some((h, p - Vec3::Y * HELD_DROP))
    }

    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[self.anim.unwrap_or(255), u8::from(self.looping)]);
        for v in [self.time, self.blend, self.idle_timer] {
            eat(&v.hash_bits().to_le_bytes());
        }
        match self.held {
            Some((h, i)) => {
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
                eat(&[i]);
            }
            None => eat(&[255]),
        }
        // The drawn pose chases the clip by the blend, so it carries history
        // the clock alone does not: the catch and the moved bodies read it.
        for p in &self.points {
            for v in p.to_array() {
                eat(&v.hash_bits().to_le_bytes());
            }
        }
        for (q, t) in &self.poses {
            for v in q.to_array().into_iter().chain(t.to_array()) {
                eat(&v.hash_bits().to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-clip prop: idle 10 frames, grab 30 frames (1 s), a trigger at
    /// the origin of 2 m starting the grab, a catch of 1.5 m up to 0.5 s in
    /// whose point sits 1 m up during the grab.
    fn rancor() -> AnimatedProp {
        AnimatedProp::new(
            Setup {
                clips: vec![10, 30],
                triggers: vec![Trigger {
                    clip: 1,
                    center: Vec3::ZERO,
                    radius: 2.0,
                }],
                catches: vec![Catch {
                    clip: 1,
                    radius: 1.5,
                    time: 0.5,
                    track: vec![vec![Vec3::new(0.0, 5.0, 0.0); 10], vec![Vec3::Y; 30]],
                }],
                bodies: Vec::new(),
                ..Setup::default()
            },
            0,
        )
    }

    #[test]
    fn a_soldier_near_the_trigger_is_caught_and_dies_at_the_clip_end() {
        let dt = 1.0 / 30.0;
        let mut p = rancor();
        let h = UnitHandle { index: 3, gen: 1 };
        assert_eq!(p.begin(), None);
        assert_eq!(p.anim, Some(0));
        assert!(p.looping);
        // Nobody near: the idle loops.
        assert_eq!(p.trigger(|_, _| false), None);
        p.advance(dt);
        assert_eq!(p.trigger(|c, r| c.length() < r), Some(1));
        assert_eq!((p.time, p.blend, p.looping), (0.0, 0.0, false));
        let mut caught_at = None;
        let mut died = None;
        for tick in 0..60 {
            if let Some(d) = p.begin() {
                died = Some((tick, d));
                break;
            }
            p.trigger(|_, _| true);
            p.catch(|point, r| ((point - Vec3::Y).length() < r).then_some(h));
            if caught_at.is_none() && p.held.is_some() {
                caught_at = Some(tick);
            }
            p.advance(dt);
        }
        // The pose blends from the idle's 5 m toward the grab's 1 m: the
        // point comes within 1.5 m of (0, 1, 0) a few ticks in.
        let caught = caught_at.expect("caught");
        assert!(caught > 0 && caught < 15, "{caught}");
        let (tick, who) = died.expect("dies at the end of the grab");
        assert_eq!(who, h);
        assert!((30..=32).contains(&tick), "{tick}");
        assert_eq!((p.anim, p.looping, p.held), (Some(0), true, None));
    }

    #[test]
    fn nobody_in_range_no_catch_and_no_death() {
        let dt = 1.0 / 30.0;
        let mut p = rancor();
        p.begin();
        p.trigger(|_, _| true);
        for _ in 0..40 {
            assert_eq!(p.begin(), None);
            p.catch(|_, _| None);
            p.advance(dt);
        }
        assert_eq!(p.anim, Some(0));
    }

    #[test]
    fn the_trigger_waits_for_half_the_blend_back_to_the_idle() {
        let dt = 1.0 / 30.0;
        let mut p = rancor();
        p.begin();
        p.trigger(|_, _| true);
        // The grab: 30 frames, then back to the idle at blend 0.
        let mut ticks = 0;
        while p.anim != Some(0) || !p.looping {
            p.begin();
            p.advance(dt);
            ticks += 1;
            assert!(ticks < 100);
        }
        // Blend grows at a fifth of the step on the idle: 0.5 after 2.5 s.
        let mut again = 0;
        while p.trigger(|_, _| true).is_none() {
            p.begin();
            p.advance(dt);
            again += 1;
            assert!(again < 200);
        }
        assert!((72..=78).contains(&again), "{again}");
    }

    #[test]
    fn bodies_ride_their_frames() {
        let r = Quat::from_xyzw(0.0, 0.0, 0.70710677, 0.70710677);
        let mut p = AnimatedProp::new(
            Setup {
                clips: vec![2, 3],
                triggers: vec![Trigger {
                    clip: 1,
                    center: Vec3::ZERO,
                    radius: 1.0,
                }],
                bodies: vec![Body {
                    body: 0,
                    frames: vec![
                        vec![(Quat::IDENTITY, Vec3::ZERO); 2],
                        vec![(Quat::IDENTITY, Vec3::ZERO), (r, Vec3::X), (r, Vec3::X)],
                    ],
                }],
                ..Setup::default()
            },
            0,
        );
        p.begin();
        p.advance(1.0 / 30.0);
        assert_eq!(p.poses[0], (Quat::IDENTITY, Vec3::ZERO));
        p.trigger(|_, _| true);
        for _ in 0..40 {
            p.begin();
            p.advance(1.0 / 30.0);
        }
        // Back on the idle and blending home.
        assert_eq!(p.anim, Some(0));
        assert!(p.poses[0].1.x < 1.0);
    }
}
