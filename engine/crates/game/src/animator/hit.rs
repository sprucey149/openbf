//! Cosmetic BF2 hit poses on every map. B2 FUN_00280e38:203467-203508,
//! FUN_0027caf0:200920-200929, FUN_0027dfe0:201979-202008.
use super::*;
use core::sim::{Event, Sim};

/// Circular order used by BF2's directional draw.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    Front,
    Left,
    /// BF2's starting side (PC EntitySoldier ctor).
    #[default]
    Back,
    Right,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::Front, Self::Left, Self::Back, Self::Right];

    /// `dir` is the way the hit travels (a round's flight, a swing, caster
    /// to target), NOT toward its source: the PS2 compares its two planar
    /// body-axis projections (B2:152435-152463), and PC BF2 passes the
    /// bolt's own velocity. So a shot in the chest from in front is `Back`:
    /// the side names say where the hit pushes the body.
    pub fn from_hit(dir: glam::Vec3, yaw: f32) -> Self {
        let source = body_vel(dir, yaw);
        if source.y.abs() > source.x.abs() {
            if source.y > 0.0 {
                Self::Front
            } else {
                Self::Back
            }
        } else if source.x > 0.0 {
            Self::Left
        } else {
            Self::Right
        }
    }

    /// BF2 FUN_0027bae0 B2:200648-200666 maps the stored contact side
    /// to the four standing death slots.
    pub fn death_name(self) -> &'static str {
        match self {
            Self::Front => "stand_death_forward",
            Self::Left => "stand_death_left",
            Self::Back => "stand_death_backward",
            Self::Right => "stand_death_right",
        }
    }

    pub fn hit_name(self) -> &'static str {
        match self {
            Self::Front => "stand_hitfront",
            Self::Left => "stand_hitleft",
            Self::Back => "stand_hitback",
            Self::Right => "stand_hitright",
        }
    }

    /// 50% source side, 20% each neighbour, 10% opposite.
    pub fn pick(self, draw: u32) -> Self {
        let offset = match draw % 10 {
            0..=4 => 0,
            5..=6 => 3,
            7..=8 => 1,
            _ => 2,
        };
        Self::ALL[(self as usize + offset) % 4]
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Reaction {
    seen: Option<(u64, usize)>,
    /// Dead at the previous step: a respawn clears the last life's hit side.
    was_dead: bool,
    pub(super) direction: Direction,
    pub(super) picked: Direction,
    pub(super) clock: f32,
    pub(super) overlay: Option<(Pose, Vec<bool>, f32)>,
}

impl Animator {
    /// Read each event once, including when a render frame steps no tick.
    /// Receives only shared sim references: no simulation state or RNG writes.
    pub fn note_hit(&mut self, sim: &Sim, event: usize) {
        let Some(Event::Hit {
            victim,
            dir,
            health_left,
            direct,
            ..
        }) = sim.events.get(event)
        else {
            return;
        };
        let key = (sim.tick, event);
        // A restarted sim begins again below the last tick read.
        if self.hit.seen.is_some_and(|(tick, _)| tick > sim.tick) {
            self.hit.seen = None;
        }
        if self.hit.seen.is_some_and(|seen| seen >= key) {
            return;
        }
        let Some(u) = sim
            .units
            .get(victim.index as usize)
            .filter(|u| u.gen == victim.gen)
        else {
            return;
        };
        self.hit.seen = Some(key);
        // Splash, emitters and burns keep the last direct hit's side for
        // the death clip (B2 FUN_00238e20 is the only setter).
        if !*direct {
            return;
        }
        self.hit.direction = Direction::from_hit(*dir, u.soldier.yaw);
        if *health_left > 0.0 && u.mount.is_none() {
            // Render-only hash; independent of frame count and simulation RNG.
            let mut seed = sim.tick ^ (u64::from(victim.index) << 32) ^ u64::from(victim.gen);
            seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            self.hit.picked = self.hit.direction.pick((seed ^ (seed >> 31)) as u32);
            self.hit.clock = 1.0;
        }
    }
}

impl Reaction {
    pub(super) fn clear_pose(&mut self) {
        self.clock = 0.0;
        self.overlay = None;
    }
    pub(super) fn step(
        &mut self,
        rig: &Rig,
        clips: &[Clip],
        skel: &Skeleton,
        i: &AnimInput,
        body: Body,
        dt: f32,
    ) {
        if self.was_dead && !i.dead {
            self.direction = Direction::default();
        }
        self.was_dead = i.dead;
        let weight = 0.5 * self.clock.min(1.0);
        self.clock = (self.clock - 8.0 * dt).max(0.0);
        self.overlay = None;
        if weight <= 0.0
            || i.dead
            || matches!(i.stance, Stance::Sprint | Stance::Prone)
            || !matches!(body, Body::Feet(0 | 1, _))
        {
            return;
        }
        let name = self.picked.hit_name();
        let clip = if rig.game == Game::Bf2 {
            bf2::simple_pair(name)
                .and_then(|p| rig.slot2(i.set, p, 0))
                .map(|s| s.clip)
        } else {
            find_clip(clips, &format!("human_rifle_{name}"))
        };
        let Some(c) = clip.and_then(|c| clips.get(c)) else {
            return;
        };
        let legs = matches!(
            body,
            Body::Feet(_, Move::Idle | Move::TurnLeft | Move::TurnRight)
        );
        let mask = rig
            .upper
            .iter()
            .zip(&rig.lower)
            .map(|(u, l)| *u || (legs && *l))
            .collect();
        self.overlay = Some((c.sample(skel, 0.0, false), mask, weight));
    }

    pub(super) fn apply(&self, pose: &mut Pose) {
        if let Some((hit, mask, weight)) = &self.overlay {
            for ((bone, hit), enabled) in pose.0.iter_mut().zip(&hit.0).zip(mask) {
                if *enabled {
                    *bone = bone.blend(*hit, *weight);
                }
            }
        }
    }
}
