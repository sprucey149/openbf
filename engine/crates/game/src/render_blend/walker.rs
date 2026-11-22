use super::{lerp_angle, lerp_look, mix};
use core::vehicle::{
    walker::{WalkerParams, ANIM_IDLE, MAX_PAIRS},
    Vehicle,
};

#[derive(Debug, Clone, PartialEq)]
pub struct WalkerPose {
    anim: u8,
    reversed: bool,
    time: f32,
    duration: f32,
    pitch: f32,
    h: (f32, f32),
    roll: (f32, f32),
    layers: [f32; MAX_PAIRS],
    aims: Vec<Option<(f32, f32)>>,
}

impl WalkerPose {
    pub(super) fn capture(v: &Vehicle, params: Option<&WalkerParams>) -> Self {
        Self {
            anim: v.walker.anim,
            reversed: v.walker.reversed,
            time: v.walker.clip_time,
            duration: params
                .and_then(|p| p.clips.get(v.walker.anim as usize))
                .and_then(Option::as_ref)
                .map_or(0.0, |c| c.duration),
            pitch: v.walker.pitch,
            h: v.walker.h,
            roll: v.walker.roll,
            layers: v.walker.layer_blend,
            aims: v.guns.iter().map(|g| g.as_ref().map(|g| g.aim)).collect(),
        }
    }

    pub(super) fn write(&self, v: &mut Vehicle) {
        // A blend across a clip change may draw the clip the tick left
        // ([`handover`]); the tick's own pose writes its own back.
        v.walker.anim = self.anim;
        v.walker.reversed = self.reversed;
        v.walker.clip_time = self.time;
        v.walker.pitch = self.pitch;
        v.walker.h = self.h;
        v.walker.roll = self.roll;
        v.walker.layer_blend = self.layers;
        for (g, a) in v.guns.iter_mut().zip(&self.aims) {
            if let (Some(g), Some(a)) = (g.as_mut(), a) {
                g.aim = *a;
            }
        }
    }

    pub(super) fn blend(&mut self, prev: &Self, t: f32) {
        if self.anim == prev.anim && self.reversed == prev.reversed {
            self.time = clip_time(prev.time, self.time, self.duration, self.reversed, t);
        } else if self.anim == prev.anim {
            // The walk turned round in its clip (re-walker §4.8): the clock
            // runs back from where it stood.
            self.time = mix(prev.time, self.time, t);
        } else if let Some((anim, reversed, time, duration)) = handover(prev, self, t) {
            (self.anim, self.reversed, self.time, self.duration) = (anim, reversed, time, duration);
        }
        self.pitch = mix(prev.pitch, self.pitch, t);
        self.h = pair(prev.h, self.h, t);
        self.roll = pair(prev.roll, self.roll, t);
        for (l, p) in self.layers.iter_mut().zip(prev.layers) {
            *l = mix(p, *l, t);
        }
        for (a, p) in self.aims.iter_mut().zip(&prev.aims) {
            if let (Some(a), Some(p)) = (a.as_mut(), p) {
                *a = lerp_look(*p, *a, t);
            }
        }
    }
}

/// The clip drawn `t` of the way through a tick that changed the clip
/// (anim, reversed, time, duration), or None to draw the tick's own.
///
/// The integrator plays a clip out and starts the next in the same tick
/// (re-walker §4.9, `advance`): the left-foot and right-foot walk clips hand
/// over at every footfall, so one tick in each step changes the clip. The
/// clock is continuous across that handover: the tick played `prev`'s last
/// `rem` seconds, then the new clip's first `el`. A frame `t` of the way
/// through draws `prev` until `t (rem + el)` passes `rem`, then the new
/// clip. Drawing the tick's own clip at every frame of that tick instead
/// jumped the pose a whole tick ahead and held it there (playtest
/// 2026-11-21: the AT-ST's body jerked sideways at each footfall).
///
/// Only idle is cut short (§4.7 starts a clip from idle): an idle that had
/// more of its clip left than the new clip has played was interrupted at
/// the tick's start, and the new clip is drawn from its start.
fn handover(prev: &WalkerPose, cur: &WalkerPose, t: f32) -> Option<(u8, bool, f32, f32)> {
    if prev.duration <= 0.0 || cur.duration <= 0.0 {
        return None;
    }
    // A walk that turned round this tick (§4.8 flips the direction before
    // the clip advances) played `prev` the new way to its end: walk clip to
    // walk clip keeps the direction, so the new clip's is the tick's.
    let walk = |a: u8| (5..=10).contains(&a);
    let back = if walk(prev.anim) && walk(cur.anim) {
        cur.reversed
    } else {
        prev.reversed
    };
    let rem = if back {
        prev.time
    } else {
        prev.duration - prev.time
    }
    .clamp(0.0, prev.duration);
    let el = if cur.reversed {
        cur.duration - cur.time
    } else {
        cur.time
    }
    .clamp(0.0, cur.duration);
    let from_start = |s: f32| {
        if cur.reversed {
            cur.duration - s
        } else {
            s
        }
    };
    if prev.anim == ANIM_IDLE && rem > el {
        return Some((cur.anim, cur.reversed, from_start(t * el), cur.duration));
    }
    let s = t * (rem + el);
    Some(if s < rem {
        let time = if back { prev.time - s } else { prev.time + s };
        (prev.anim, back, time, prev.duration)
    } else {
        (cur.anim, cur.reversed, from_start(s - rem), cur.duration)
    })
}

fn pair(prev: (f32, f32), cur: (f32, f32), t: f32) -> (f32, f32) {
    if prev == cur || t == 1.0 {
        return cur;
    }
    if t == 0.0 {
        return prev;
    }
    let a = lerp_angle(
        core::fmath::atan2(prev.1, prev.0),
        core::fmath::atan2(cur.1, cur.0),
        t,
    );
    let (s, c) = core::fmath::sin_cos(a);
    (c, s)
}

fn clip_time(prev: f32, cur: f32, duration: f32, reversed: bool, t: f32) -> f32 {
    if t == 0.0 {
        return prev;
    }
    if t == 1.0 || prev == cur {
        return cur;
    }
    let wrap = duration > 0.0 && if reversed { cur > prev } else { cur < prev };
    if wrap {
        let end = if reversed {
            cur - duration
        } else {
            cur + duration
        };
        mix(prev, end, t).rem_euclid(duration)
    } else {
        mix(prev, cur, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_restart_takes_the_short_path_forward_and_backward() {
        assert!((clip_time(0.98, 0.02, 1.0, false, 0.25) - 0.99).abs() < 1e-6);
        assert!((clip_time(0.98, 0.02, 1.0, false, 0.75) - 0.01).abs() < 1e-6);
        assert!((clip_time(0.02, 0.98, 1.0, true, 0.25) - 0.01).abs() < 1e-6);
        assert!((clip_time(0.02, 0.98, 1.0, true, 0.75) - 0.99).abs() < 1e-6);
        assert_eq!(clip_time(0.2, 0.1, 1.0, true, 0.5), 0.15);
        assert_eq!(clip_time(0.4, 0.4, 1.0, false, 0.5), 0.4);
    }

    fn walk(anim: u8, reversed: bool, time: f32) -> WalkerPose {
        WalkerPose {
            anim,
            reversed,
            time,
            duration: 1.0,
            pitch: 0.0,
            h: (1.0, 0.0),
            roll: (1.0, 0.0),
            layers: [0.0; MAX_PAIRS],
            aims: Vec::new(),
        }
    }

    fn blended(prev: &WalkerPose, cur: &WalkerPose, t: f32) -> (u8, f32) {
        let mut o = cur.clone();
        o.blend(prev, t);
        (o.anim, o.time)
    }

    #[test]
    fn footfall_handover_draws_the_clip_clock_through_the_tick() {
        // 0.98 into the left-foot clip, the tick plays its last 0.02 and
        // 0.02 of the right-foot clip.
        let (prev, cur) = (walk(7, false, 0.98), walk(8, false, 0.02));
        let (a, t) = blended(&prev, &cur, 0.25);
        assert_eq!(a, 7);
        assert!((t - 0.99).abs() < 1e-6);
        let (a, t) = blended(&prev, &cur, 0.75);
        assert_eq!(a, 8);
        assert!((t - 0.01).abs() < 1e-6);
        assert_eq!(blended(&prev, &cur, 1.0), (8, 0.02));
        // Walking backward the clocks run down.
        let (prev, cur) = (walk(8, true, 0.01), walk(7, true, 0.97));
        let (a, t) = blended(&prev, &cur, 0.125);
        assert_eq!(a, 8);
        assert!((t - 0.005).abs() < 1e-6);
        let (a, t) = blended(&prev, &cur, 0.6);
        assert_eq!(a, 7);
        assert!((t - 0.986).abs() < 1e-6);
        // Turned round just after a footfall: the left-foot clip ran back
        // to its start, then the right-foot clip back from its end.
        let (prev, cur) = (walk(7, false, 0.01), walk(8, true, 0.99));
        let (a, t) = blended(&prev, &cur, 0.25);
        assert_eq!(a, 7);
        assert!((t - 0.005).abs() < 1e-6);
        let (a, t) = blended(&prev, &cur, 0.75);
        assert_eq!(a, 8);
        assert!((t - 0.995).abs() < 1e-6);
    }

    #[test]
    fn a_clip_started_from_idle_is_drawn_from_its_start() {
        let (prev, cur) = (walk(ANIM_IDLE, false, 0.4), walk(5, false, 0.03));
        let (a, t) = blended(&prev, &cur, 0.5);
        assert_eq!(a, 5);
        assert!((t - 0.015).abs() < 1e-6);
        // Idle played out to its end hands over like a walk clip.
        let (prev, cur) = (walk(ANIM_IDLE, false, 0.99), walk(1, false, 0.02));
        assert_eq!(blended(&prev, &cur, 0.2).0, ANIM_IDLE);
    }

    #[test]
    fn the_drawn_clip_is_written_and_the_ticks_own_restores_it() {
        use core::vehicle::{VehicleKind, VehicleSpec};
        let mut sim = core::sim::Sim::empty(1);
        let h = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Walker,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: glam::Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: Vec::new(),
        });
        let v = &mut sim.vehicles[h.index as usize];
        let (prev, cur) = (walk(7, false, 0.98), walk(8, false, 0.02));
        let mut shown = cur.clone();
        shown.blend(&prev, 0.25);
        shown.write(v);
        assert_eq!((v.walker.anim, v.walker.clip_time), (7, shown.time));
        cur.write(v);
        assert_eq!((v.walker.anim, v.walker.clip_time), (8, 0.02));
    }

    #[test]
    fn leg_and_roll_pairs_blend_across_the_angle_seam() {
        let a = 3.13f32;
        let b = -3.13f32;
        let p = pair((a.cos(), a.sin()), (b.cos(), b.sin()), 0.5);
        assert!((p.1.atan2(p.0).abs() - std::f32::consts::PI).abs() < 1e-5);
    }
}
