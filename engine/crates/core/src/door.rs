//! BF2's `door` entity (queue DR2): a door that opens BY ITSELF while a
//! soldier is inside one of its trigger spheres, its collision boxes riding
//! the `open` clip out of the doorway, and closes again when nobody is.
//!
//! The rule is the disc's door tick `FUN_001ebb80` (`analysis/re-bf2-doors.md`
//! §2 hop 5 [V][O]): each tick, for each of the ≤ 3 `AnimationTrigger` pairs,
//! a radius query around the hardpoint (`d² < r²`, any team); any hit on the
//! soldier layer marks the door OCCUPIED. The animation parameter then moves
//! toward 1.0 (occupied) or 0.0 (not) by `dt·30/(frames−1)` per tick, clamped
//! — the clip end to end in (frames − 1)/30 s both ways. A LOCKED door does
//! not open in response to a soldier. Its class default can be overridden by
//! a mission's `SetProperty(..., "IsLocked", ...)`.
//!
//! Where the port differs, PROVISIONAL `[O]`:
//! - the query tests every LIVE unit's `soldier.pos` (the feet) — the disc's
//!   object position and its gated second query path `FUN_003f0990` are not
//!   traced (§2 hop 5, §8);
//! - the hardpoint is where the clip's frame 0 puts it, not re-resolved
//!   through the animated model each tick (`bake::door`);
//! - a pose between two clip frames interpolates them (translation lerp,
//!   rotation nlerp) — the disc's clip sampler between keys is not read;
//! - the door's tick runs inside `Sim::step` before movement.
//!
//! The SOUNDS (queue DRV) are edge-triggered by the disc's latch, bit 0x21
//! of `this+0x5f2` (`FUN_001ebb80`, B2:99659–99687 [V]): the first occupied
//! tick with the latch clear plays `OpenSound` and sets it, the first empty
//! tick with it set plays `CloseSound` and clears it — the TRIGGER's edges,
//! not the parameter's. [`Door::latch`] is that bit; the sim turns its edges
//! into `Event::DoorOpened` / `DoorClosed`. A locked door plays its
//! `LockedSound` after more than three seconds without a soldier in range
//! (initially eligible, BF2:99213, 99540–99542, 99691–99694, 99712–99713).
//! The drawn door rides the same parameter (`game::viewer::SceneGpu::pose_doors`).
//!
//! The BOTS (card MOUTH2): a leaf of a door that is opening is no wall to
//! the steering's sweep (`Sim::door_opening`, `ai::steer`'s `StopKind::Door`)
//! — a bot walks on at it and is through when it has slid clear, as the
//! PS2's are (`analysis/re-bf2-doors.md` 11).
//!
//! What it does NOT drive, recorded so it is not re-derived: the room portal
//! (§2 hop 6, visibility).

use glam::{Mat4, Quat, Vec3};

/// One `AnimationTrigger` sphere, in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trigger {
    pub center: Vec3,
    pub radius: f32,
}

/// One collision body the door moves: its index in
/// [`crate::collision::CollisionWorld::bodies`] and its world pose at every
/// clip frame, as (rotation, translation) — frame 0 is the door shut.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorBody {
    pub body: usize,
    pub frames: Vec<(Quat, Vec3)>,
}

/// One placed door. The pose and lock affect gameplay. The latch and
/// empty time only decide sound events.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Door {
    pub triggers: Vec<Trigger>,
    /// The clip's frame count (≥ 2).
    pub frames: u16,
    pub bodies: Vec<DoorBody>,
    /// The animation parameter `this[0x4e9]`: 0 = shut, 1 = open.
    pub open: f32,
    /// The door ignores an occupied trigger while locked.
    pub locked: bool,
    /// The sound latch, bit 0x21 of `this+0x5f2`: set by the open edge,
    /// cleared by the close edge. Starts clear (a shut door).
    pub latch: bool,
    /// Seconds since the last occupied tick. `None` is the initial, ready state.
    pub empty_secs: Option<f32>,
    /// The manifest instance the door is (after `apply_mode_layer`): what
    /// the renderer moves.
    pub instance: usize,
    /// The instance's world position, where its sounds play (the disc plays
    /// them at the entity, its field at +0x54).
    pub origin: Vec3,
    /// The ODF's `OpenSound` / `CloseSound`.
    pub open_sound: Option<String>,
    pub close_sound: Option<String>,
    pub locked_sound: Option<String>,
}

/// A sound edge of [`Door::latch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Opened,
    Closed,
    Locked,
}

impl Door {
    /// Is any of `feet` strictly inside a trigger sphere (`d² < r²`)?
    pub fn occupied(&self, feet: impl IntoIterator<Item = Vec3> + Clone) -> bool {
        self.triggers.iter().any(|t| {
            let r2 = t.radius * t.radius;
            feet.clone()
                .into_iter()
                .any(|p| (p - t.center).length_squared() < r2)
        })
    }

    /// Advance `open` by one tick of `dt` seconds; true when it changed.
    pub fn advance(&mut self, occupied: bool, dt: f32) -> bool {
        if occupied && self.locked {
            return false;
        }
        let step = dt * 30.0 / f32::from(self.frames.max(2) - 1);
        let next = if occupied {
            (self.open + step).min(1.0)
        } else {
            (self.open - step).max(0.0)
        };
        let changed = next != self.open;
        self.open = next;
        changed
    }

    /// The latch's edge this tick, if any, updating it (the disc's order:
    /// the edge follows the parameter's move in the same tick).
    pub fn edge(&mut self, occupied: bool, dt: f32) -> Option<Edge> {
        let empty_secs = (self.empty_secs.unwrap_or(500.0) + dt).min(500.0);
        self.empty_secs = Some(if occupied { 0.0 } else { empty_secs });
        match (occupied, self.latch) {
            (true, _) if self.locked => (empty_secs > 3.0).then_some(Edge::Locked),
            (true, false) => {
                self.latch = true;
                Some(Edge::Opened)
            }
            (false, true) => {
                self.latch = false;
                Some(Edge::Closed)
            }
            _ => None,
        }
    }

    /// Body `k`'s world transform at the current `open`.
    pub fn pose(&self, k: usize) -> Mat4 {
        sample(&self.bodies[k].frames, self.open)
    }
}

/// A per-frame (rotation, translation) track at parameter `open` ∈ [0, 1]:
/// frame `open·(n−1)`, two frames between interpolated (translation lerp,
/// rotation nlerp) — PROVISIONAL, see the header. Shared by the collision
/// bodies and the drawn door.
pub fn sample(f: &[(Quat, Vec3)], open: f32) -> Mat4 {
    let last = f.len() - 1;
    let x = open.clamp(0.0, 1.0) * last as f32;
    let i = (x as usize).min(last);
    let frac = x - i as f32;
    let (q, t) = if frac <= 0.0 || i == last {
        f[i]
    } else {
        let (q0, t0) = f[i];
        let (mut q1, t1) = f[i + 1];
        if q0.dot(q1) < 0.0 {
            q1 = -q1;
        }
        (
            (q0 * (1.0 - frac) + q1 * frac).normalize(),
            t0.lerp(t1, frac),
        )
    };
    Mat4::from_rotation_translation(q, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn door(frames: u16) -> Door {
        Door {
            triggers: vec![Trigger {
                center: Vec3::new(0.0, 0.0, -1.1),
                radius: 4.0,
            }],
            frames,
            bodies: vec![DoorBody {
                body: 0,
                frames: (0..frames)
                    .map(|f| {
                        let s = f as f32 / f32::from(frames - 1);
                        (Quat::IDENTITY, Vec3::new(0.0, 3.707 * s, 1.409 * s))
                    })
                    .collect(),
            }],
            ..Door::default()
        }
    }

    #[test]
    fn locked_sound_rearms_after_more_than_three_seconds_empty() {
        let mut d = door(31);
        d.locked = true;
        assert_eq!(d.edge(true, 0.25), Some(Edge::Locked));
        assert_eq!(d.edge(true, 4.0 / 30.0), None);
        assert_eq!(d.edge(false, 2.75), None);
        assert_eq!(d.edge(true, 0.25), None, "exactly three seconds");
        assert_eq!(d.edge(false, 3.0), None);
        assert_eq!(d.edge(true, 0.25), Some(Edge::Locked));
        assert_eq!(d.edge(true, 0.25), None);
        d.locked = false;
        assert_eq!(d.edge(true, 0.25), Some(Edge::Opened));
    }

    /// The disc's timing (§2 hop 5 + §3 hop 9): a 31-frame clip opens in
    /// 1.00 s — 30 ticks at 30 Hz — and closes in the same time.
    #[test]
    fn a_31_frame_door_opens_in_one_second_and_closes_in_one() {
        let mut d = door(31);
        let dt = 1.0 / 30.0;
        let mut ticks = 0;
        while d.open < 1.0 {
            assert!(d.advance(true, dt));
            ticks += 1;
            assert!(ticks <= 31, "too slow");
        }
        assert!((29..=31).contains(&ticks), "{ticks} ticks to open");
        assert!(!d.advance(true, dt), "fully open stays put");
        let mut ticks = 0;
        while d.open > 0.0 {
            d.advance(false, dt);
            ticks += 1;
            assert!(ticks <= 31, "too slow");
        }
        assert!((29..=31).contains(&ticks), "{ticks} ticks to close");
    }

    /// The trigger is strict (`d² < r²`) and any unit counts.
    #[test]
    fn the_trigger_is_a_strict_sphere() {
        let d = door(31);
        let c = Vec3::new(0.0, 0.0, -1.1);
        assert!(d.occupied([c + Vec3::X * 3.99]));
        assert!(!d.occupied([c + Vec3::X * 4.0]));
        assert!(!d.occupied(Vec::<Vec3>::new()));
        assert!(d.occupied([c + Vec3::X * 9.0, c]));
    }

    /// Shut is frame 0 exactly, open is the last frame exactly, and halfway
    /// interpolates.
    #[test]
    fn the_pose_follows_the_clip() {
        let mut d = door(31);
        assert_eq!(d.pose(0), Mat4::IDENTITY);
        d.open = 1.0;
        let end = d.pose(0).w_axis.truncate();
        assert!((end - Vec3::new(0.0, 3.707, 1.409)).length() < 1e-5);
        d.open = 0.5;
        let mid = d.pose(0).w_axis.truncate();
        assert!((mid - Vec3::new(0.0, 3.707, 1.409) * 0.5).length() < 1e-5);
        d.open = 0.51;
        let a = d.pose(0).w_axis.y;
        assert!(a > mid.y && a < end.y);
    }

    /// The disc's latch (`FUN_001ebb80`, B2:99659–99687): one `Opened` on
    /// the first occupied tick, none while it holds, one `Closed` on the
    /// first empty tick, none while it stays empty — whatever the
    /// parameter is doing, so a unit stepping out and back in mid-swing
    /// sounds both again.
    #[test]
    fn the_latch_fires_once_per_trigger_edge() {
        let mut d = door(31);
        let dt = 1.0 / 30.0;
        let tick = |d: &mut Door, occ: bool| {
            d.advance(occ, dt);
            d.edge(occ, 1.0 / 30.0)
        };
        assert_eq!(tick(&mut d, false), None, "shut and empty: silent");
        assert_eq!(tick(&mut d, true), Some(Edge::Opened));
        for _ in 0..40 {
            assert_eq!(tick(&mut d, true), None, "holding open: silent");
        }
        assert_eq!(d.open, 1.0);
        assert_eq!(tick(&mut d, false), Some(Edge::Closed));
        for _ in 0..5 {
            assert_eq!(tick(&mut d, false), None);
        }
        assert!(d.open > 0.0 && d.open < 1.0, "mid-swing");
        assert_eq!(tick(&mut d, true), Some(Edge::Opened), "back in mid-swing");
        assert_eq!(tick(&mut d, false), Some(Edge::Closed));
    }
}
