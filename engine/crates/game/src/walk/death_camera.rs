//! The death camera (`analysis/re-spawn-flow.md` §6.5, §6.6): after the
//! player dies outside a closed hull the view eases onto his body, and after
//! an enemy kill it turns from the body to the killer 1.5 s in and follows
//! him until the spawn map, which replays the ease onto the body.
//!
//! The same camera in both games: BF2 set-up `FUN_001bc5f8` (B2:65317),
//! update `FUN_001bc188` (B2:65050); BF1 set-up `FUN_001aaed8` (B1:52711),
//! update `FUN_001aaa68` (B1:52444). Presentation only: the sim never reads it.
//! Ground maps only; a space map keeps the chase camera.

use super::*;
use core::fmath;
use core::sim::{Event, SpawnStage};

/// Phase 1's ease-in: the weight is the timer over this (BF2 0x5fc51c, BF1 0x517484).
pub const EASE_IN: f32 = 1.0;
/// Phase 2 starts once the timer passes this, with a killer kept (BF2 0x5fc520, BF1 0x517488).
pub const KILLER_AFTER: f32 = 1.5;
/// Phase 2's weight grows by this per second past [`KILLER_AFTER`] (B2:65149-65151, B1 the same).
const KILLER_EASE_RATE: f32 = 0.5;
/// The two casts' length behind the start point; the eye stands at the
/// smaller hit fraction times this, less 1 m (BF2 0x5fc524, BF1 0x51748c).
pub const CAST: f32 = 6.0;
/// The body point the view looks at, above the feet (B2:65370-65375).
const LOOK_UP: f32 = 0.8;
/// The fixed point set at the death: 1 m behind along the last camera
/// facing and 1.0 + 0.5 m up (B2:65393-65405).
const FIXED_UP: f32 = 1.5;
/// The casts start this far above and below the view's start point
/// (B2:65182-65211, B1 the 0.75 vector).
const CAST_SPLIT: f32 = 0.75;
/// A killer on foot is looked at this far above his feet (live, BF2
/// Kamino, within 0.06 m; `re-spawn-flow.md` §6.5).
const KILLER_HEAD: f32 = 1.70;
/// PROVISIONAL: a mounted killer is looked at on his vehicle's origin.
/// trace: the vehicle point the PS2 reads (BF2 vehicle +0xd0, BF1 +0x100).
const KILLER_VEHICLE_UP: f32 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeathCam {
    /// Seconds since the camera (re)started.
    pub timer: f32,
    pub eye: Vec3,
    pub rot: Quat,
    /// The drawn camera when the player died: a replay starts from it again.
    start: (Vec3, Quat),
    /// The point fixed at the death, behind and above the body.
    fixed: Vec3,
    /// The body point, refreshed from the corpse while it is followed.
    pub(crate) look: Vec3,
    /// The killer kept at the start (none after a suicide, the world, or a hero's death).
    pub(crate) killer: Option<UnitHandle>,
    pub(crate) body: UnitHandle,
    /// Whether the corpse still feeds the body point (a replay drops it).
    follow_body: bool,
    /// The player's spawn screen at the last tick: the spawn map opening
    /// replays phase 1.
    stage: SpawnStage,
}

impl DeathCam {
    /// Start at the drawn camera `from` over the body at `feet`.
    pub fn start(
        body: UnitHandle,
        feet: Vec3,
        from: &Camera,
        killer: Option<UnitHandle>,
        stage: SpawnStage,
    ) -> Self {
        let rot = camera::camera_rotation(from.yaw, from.pitch, from.roll);
        let (s, c) = fmath::sin_cos(from.yaw);
        let back = Vec3::new(s, 0.0, c);
        Self {
            timer: 0.0,
            eye: from.position,
            rot,
            start: (from.position, rot),
            fixed: feet + back + Vec3::Y * FIXED_UP,
            look: feet + Vec3::Y * LOOK_UP,
            killer,
            body,
            follow_body: true,
            stage,
        }
    }

    /// The spawn map's replay (§6 item 3): phase 1 again from the camera at
    /// the death, with no body to follow and no killer.
    pub fn replay(&mut self) {
        (self.eye, self.rot) = self.start;
        self.timer = 0.0;
        self.killer = None;
        self.follow_body = false;
    }

    pub fn camera(&self, fov_y: f32) -> Camera {
        flyer_camera::camera_at(self.eye, self.rot, fov_y)
    }
}

/// The camera rotation looking along `dir` (unit), no roll.
fn look_rotation(dir: Vec3) -> Quat {
    let yaw = fmath::atan2(-dir.x, -dir.z);
    let pitch = fmath::asin(dir.y.clamp(-1.0, 1.0));
    camera::camera_rotation(yaw, pitch, 0.0)
}

impl WalkSim {
    /// The shooter credited with killing `victim` in the sim's current
    /// events, never `victim` himself.
    pub(super) fn killer_of(&self, victim: UnitHandle) -> Option<UnitHandle> {
        self.sim.events.iter().find_map(|e| match *e {
            Event::Kill { shooter, victim: v } if v == victim && shooter != victim => Some(shooter),
            _ => None,
        })
    }

    /// Start the death camera the first time the player is seen dead (once
    /// per life). `from` is the camera drawn before the death; `earlier` a
    /// killer read before this tick's step cleared the events.
    pub(super) fn notice_death(&mut self, from: &Camera, earlier: Option<UnitHandle>) {
        let h = self.player;
        let Some(me) = self
            .sim
            .units
            .get(h.index as usize)
            .filter(|u| u.gen == h.gen && !u.alive)
        else {
            return;
        };
        if self.death_seen == Some(h) {
            return;
        }
        self.death_seen = Some(h);
        // A pilot or rider lost inside the hull keeps his seat's view.
        if self.space_battle()
            || self.sim.outcome.is_some()
            || self.backdrop.is_some()
            || self.held_seat_view().is_some()
        {
            return;
        }
        // Only a death in play: at the level start the player is not on the
        // field yet.
        let stage = self.sim.spawn_stage.first().copied().unwrap_or_default();
        if !matches!(stage, SpawnStage::Accepted | SpawnStage::Alive) {
            return;
        }
        // A hero class passes no killer (B2:159456-159463).
        let hero = self.sim.hero_class(me.team) == Some(me.spec.class);
        let killer = (!hero)
            .then(|| self.killer_of(h).or(earlier))
            .flatten()
            .filter(|&k| k != h);
        let feet = me.soldier.pos;
        self.death_cam = Some(DeathCam::start(h, feet, from, killer, stage));
    }

    /// One tick of the death camera, or its end: the player alive again, the
    /// round decided, or the level start's backdrop up.
    pub(super) fn step_death_cam(&mut self) {
        let Some(mut d) = self.death_cam else {
            return;
        };
        let alive = self.sim.unit(self.player).is_some();
        if alive || self.sim.outcome.is_some() || self.backdrop.is_some() {
            self.death_cam = None;
            return;
        }
        let stage = self.sim.spawn_stage.first().copied().unwrap_or_default();
        if d.stage == SpawnStage::Accepted && stage == SpawnStage::Map {
            d.replay();
        }
        d.stage = stage;
        d.timer += core::TICK_DT;
        if d.follow_body {
            if let Some(u) = self
                .sim
                .units
                .get(d.body.index as usize)
                .filter(|u| u.gen == d.body.gen && !u.alive)
            {
                d.look = u.soldier.pos + Vec3::Y * LOOK_UP;
            }
        }
        let (weight, from, target) = match d.killer.filter(|_| d.timer > KILLER_AFTER) {
            Some(k) => {
                let w = ((d.timer - KILLER_AFTER) * KILLER_EASE_RATE).min(1.0);
                // A killer gone keeps phase 1's points at phase 2's weight.
                match self.killer_point(k) {
                    Some(p) => (w, d.look, p),
                    None => (w, d.fixed, d.look),
                }
            }
            None => ((d.timer / EASE_IN).min(1.0), d.fixed, d.look),
        };
        let mut dir = (target - from).normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = d.rot * Vec3::NEG_Z;
        }
        let f = self
            .death_cast(from + Vec3::Y * CAST_SPLIT, -dir)
            .min(self.death_cast(from - Vec3::Y * CAST_SPLIT, -dir));
        let goal_eye = from - dir * (CAST * f - 1.0);
        d.eye += (goal_eye - d.eye) * weight;
        d.rot = crate::sim_pose::quat_slerp(d.rot, look_rotation(dir), weight);
        self.death_cam = Some(d);
    }

    /// Where the view looks at a living killer: his vehicle when he rides,
    /// else his head.
    fn killer_point(&self, k: UnitHandle) -> Option<Vec3> {
        let u = self.sim.unit(k)?;
        if let Some(v) = u.mount.and_then(|m| self.sim.vehicle(m.vehicle)) {
            return Some(v.pos + Vec3::Y * KILLER_VEHICLE_UP);
        }
        Some(u.soldier.pos + Vec3::Y * KILLER_HEAD)
    }

    /// The clear fraction of a [`CAST`] m cast from `origin` along `dir`
    /// against the terrain sheet and the world's big models.
    fn death_cast(&self, origin: Vec3, dir: Vec3) -> f32 {
        let models = if self.camera_blockers.is_empty() {
            1.0
        } else {
            self.camera_blockers.fraction(
                origin,
                origin + dir * CAST,
                None,
                |i| self.station_pose(i),
                &self.sim.doors,
            )
        };
        let terrain = self.sim.heightfield.as_ref().and_then(|h| {
            let cutters = &self.sim.collision.cutters;
            core::collision::ray_heightfield(h, cutters, origin, dir, CAST, 0.25)
        });
        terrain.map_or(models, |t| models.min(t / CAST))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat() -> Heightfield {
        Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }
    }

    /// A walk whose player died on flat ground at the origin this tick,
    /// facing `yaw`, with the death camera started.
    fn dead_walk(yaw: f32) -> (WalkSim, Vec3) {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat()));
        w.sim.units[w.player.index as usize].soldier.yaw = yaw;
        for _ in 0..10 {
            w.step_tick(&[PadState::default()]);
        }
        w.sim
            .kill_environment(w.player, core::sim::DeathCause::Suicide);
        w.step_tick(&[PadState::default()]);
        assert!(w.death_cam.is_some(), "the death camera starts");
        let feet = w.sim.units[w.player.index as usize].soldier.pos;
        (w, feet)
    }

    fn ticks(w: &mut WalkSim, n: usize) {
        for _ in 0..n {
            w.step_tick(&[PadState::default()]);
        }
    }

    fn forward(w: &WalkSim) -> Vec3 {
        w.death_cam.unwrap().rot * Vec3::NEG_Z
    }

    /// The share of the start-to-goal distance left after `n` updates: the
    /// weight is k / 30 on the k-th (the PS2's 30 Hz is the port's tick).
    fn left(n: u32) -> f32 {
        (1..=n).map(|k| 1.0 - k as f32 / 30.0).product()
    }

    #[test]
    fn a_death_with_no_killer_holds_the_body_from_6_22_m() {
        let yaw = 0.7f32;
        let (mut w, feet) = dead_walk(yaw);
        let d = w.death_cam.unwrap();
        assert_eq!(d.killer, None);
        let look = feet + Vec3::Y * LOOK_UP;
        let start = d.eye;
        // The unobstructed goal: 5 m past the fixed point on the line from it.
        let (s, c) = fmath::sin_cos(yaw);
        let fixed = feet + Vec3::new(s, FIXED_UP, c);
        let goal = fixed + (fixed - look).normalize() * 5.0;
        assert!(((goal - look).length() - 6.2207).abs() < 1e-3);
        let total = (start - goal).length();
        assert!(total > 1.0, "the chase camera is not already there");
        let mut n = 0;
        for want in [1, 3, 7] {
            ticks(&mut w, want - n);
            n = want;
            let got = (w.death_cam.unwrap().eye - goal).length() / total;
            assert!(
                (got - left(n as u32)).abs() < 0.01,
                "after {n} updates {got} left, want {}",
                left(n as u32)
            );
        }
        assert!((left(1) - 0.967).abs() < 1e-3 && (left(3) - 0.812).abs() < 1e-3);
        assert!((left(7) - 0.360).abs() < 1e-3);
        ticks(&mut w, 30);
        let cam = w.camera();
        assert!(((cam.position - look).length() - 6.2207).abs() < 0.01);
        // It looks at the body along the last facing.
        let to_body = (look - cam.position).normalize();
        assert!(forward(&w).dot(to_body) > 0.99999);
        let yaw_err = (cam.yaw - yaw).abs().to_degrees();
        assert!(yaw_err < 0.1, "yaw off by {yaw_err} deg");
        assert!(!w.player_body_hidden(), "the body is drawn");
    }

    #[test]
    fn an_enemy_kill_turns_to_the_killer_after_1_5_s() {
        let (mut w, feet) = dead_walk(0.0);
        let enemy = w
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                1,
                feet + Vec3::new(0.0, 0.0, -15.0),
            ))
            .unwrap();
        w.death_cam.as_mut().unwrap().killer = Some(enemy);
        let look = feet + Vec3::Y * LOOK_UP;
        // Phase 1 until the timer passes 1.5 s; the first update after it turns.
        ticks(&mut w, 40);
        let head = |w: &WalkSim| w.sim.unit(enemy).unwrap().soldier.pos + Vec3::Y * KILLER_HEAD;
        let mut n = 40;
        loop {
            let held = w.death_cam.unwrap();
            assert!(forward(&w).dot((look - held.eye).normalize()) > 0.999);
            ticks(&mut w, 1);
            n += 1;
            let d = w.death_cam.unwrap();
            if d.timer > KILLER_AFTER {
                assert_ne!(d.eye, held.eye, "phase 2 starts");
                break;
            }
            assert!(
                (d.eye - held.eye).length() < 1e-4,
                "still phase 1 at {}",
                d.timer
            );
        }
        assert!((45..=46).contains(&n), "phase 2 on update {n}");
        // Settled by 2.3 s: on the body point, looking at the killer's head.
        ticks(&mut w, 69 - n);
        let d = w.death_cam.unwrap();
        let to_head = (head(&w) - d.eye).normalize();
        let off = forward(&w)
            .dot(to_head)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        assert!(off < 2.0, "view {off} deg off the killer");
        let gap = (d.eye - look).length();
        assert!(gap < 1.2, "eye {gap} m from the body point");
        // He moves 10 m sideways: followed within 0.7 s.
        w.sim.units[enemy.index as usize].soldier.pos.x += 10.0;
        ticks(&mut w, 21);
        let to_head = (head(&w) - w.death_cam.unwrap().eye).normalize();
        let off = forward(&w)
            .dot(to_head)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        assert!(off < 2.0, "view {off} deg off the moved killer");
    }

    /// SSELC's open question: with nothing behind the body the phase-2 eye
    /// stands 5 m back from the body point; it is the floor that brings it
    /// in on level ground.
    #[test]
    fn phase_two_unobstructed_eye_is_5_m_back() {
        let (mut w, feet) = dead_walk(0.0);
        let enemy = w
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                1,
                feet + Vec3::new(0.0, 0.0, -15.0),
            ))
            .unwrap();
        w.death_cam.as_mut().unwrap().killer = Some(enemy);
        ticks(&mut w, 20);
        // Put the killer well below the body: the casts go up, clear.
        w.sim.units[enemy.index as usize].soldier.pos.y = feet.y - 30.0;
        ticks(&mut w, 100);
        let d = w.death_cam.unwrap();
        let gap = (d.eye - d.look).length();
        assert!((gap - 5.0).abs() < 0.01, "eye {gap} m from the body point");
    }

    #[test]
    fn a_suicide_or_hero_death_never_turns() {
        let (mut w, _) = dead_walk(0.0);
        ticks(&mut w, 30);
        let held = w.death_cam.unwrap();
        ticks(&mut w, 60);
        let d = w.death_cam.unwrap();
        assert!(d.killer.is_none());
        assert!((d.eye - held.eye).length() < 1e-4);
        assert!(d.rot.dot(held.rot).abs() > 0.99999);
        // A credited kill keeps its killer; a hero's death passes none.
        for hero in [false, true] {
            let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat()));
            let me = w.player;
            let class = w.sim.units[me.index as usize].spec.class;
            if hero {
                w.sim.heroes = Some(core::sim::heroes::Heroes::new([Some(class), None]));
                assert_eq!(w.sim.hero_class(0), Some(class));
            }
            let enemy = w
                .sim
                .spawn(&core::unit::UnitSpec::trooper(
                    1,
                    Vec3::new(0.0, 0.0, -15.0),
                ))
                .unwrap();
            w.sim.events.push(Event::Kill {
                shooter: enemy,
                victim: me,
            });
            assert_eq!(w.killer_of(me), Some(enemy));
            w.sim.units[me.index as usize].alive = false;
            let from = w.camera();
            w.notice_death(&from, None);
            let want = (!hero).then_some(enemy);
            assert_eq!(w.death_cam.unwrap().killer, want, "hero {hero}");
        }
    }

    #[test]
    fn the_spawn_map_replays_phase_one() {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat()));
        ticks(&mut w, 10);
        // The spawn screens on, the player spawned (accepted).
        w.sim.spawn_screens = true;
        w.sim.spawn_stage = vec![SpawnStage::Accepted];
        let before = w.camera();
        w.sim
            .kill_environment(w.player, core::sim::DeathCause::Suicide);
        ticks(&mut w, 1);
        let start = w.death_cam.unwrap();
        assert_eq!((start.eye, start.timer), (before.position, 0.0));
        let mut replayed = false;
        for _ in 0..200 {
            ticks(&mut w, 1);
            let d = w.death_cam.unwrap();
            if w.sim.spawn_stage[0] == SpawnStage::Map {
                // The first update after the replay: one step from the start.
                assert!(d.timer < 0.05, "replayed (timer {})", d.timer);
                let goal_share = (d.eye - before.position).length();
                assert!(goal_share < 0.5, "back near the start ({goal_share} m)");
                replayed = true;
                break;
            }
        }
        assert!(replayed, "the spawn map opened");
    }
}
