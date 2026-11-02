//! The health, ammo and vehicle droids drawn moving (card PWST slice 2,
//! `analysis/re-powerup-station.md` §6 [O], B1 `FUN_002a9750`): each placed
//! station runs the PS2's four-state machine on its own clock, driven by
//! the sim's give pass (`core::sim::Station::serving`).
//!
//! - **1, wait**: the timer runs down; at 0 the next turn starts the other
//!   way, `T = (0.75 + 0.5u) pi / IdleRotateSpeed` long (u from the
//!   station's own generator, the PS2's LCG), and the wobble's frequency is
//!   `12 pi / T` (six rocks a turn). `IdleRotateSpeed` 0: no turn.
//! - **0, turn**: the whole droid turns about its +Y at `IdleRotateSpeed`
//!   (135 to 225 degrees a turn); the `IdleWobbleNode` rocks about Z by
//!   `IdleWobbleFactor x sin(f x timer)`. At the end: wait `IdleWaitTime`.
//! - **3, serving** (a give pass served someone): no turn; the wobble node
//!   bobs down by `IdleWobbleFactor x (1 - cos(2 pi t)) / 2` m (the gonk,
//!   1 Hz), each `ActiveRotateNode` i swings about its X by
//!   `0.15 sin(2 pi (t - i))` (the FX-7's arms) and the `ActiveSpinNode`
//!   turns about its Z by `pi sin(2 pi t)` (its socket), `t` the timer
//!   running down without end. A pass that serves nobody goes back to 1.
//! - State 2 is never entered (§6).
//!
//! Every frame the wobble node starts from its bind matrix; the arms and
//! the socket are written only while serving, so they keep their last
//! swing after it (the code as read; PROVISIONAL [H] whether the PS2's node
//! list keeps them). The feet (`IdleWobbleLeftFoot` / `RightFoot`) never
//! move on the discs' droids (§6: their idle angle never reaches the bones,
//! and their serving y = 0 is their bind y), so nothing writes them.
//!
//! A node's matrix is `parent(now) x local(bind) x R`: the PS2 multiplies
//! the bind quaternion by the axis one (Hamilton, `FUN_00445b28`), a turn
//! about the node's own axis. On both droids every parent is at identity.
//!
//! PROVISIONAL (ours): the PS2 animates only a station a viewport sees
//! within 60 m (§6 [H]); the port animates every one. The animation steps
//! on the sim's ticks (1/30 s), the PS2 per frame (1/60 s). The PS2's
//! idle turn writes the station matrix; the port shares the sim-owned yaw
//! with collision and blast spheres (§14).

use super::turret_pose::bind_of;
use glam::{Mat4, Quat, Vec3};
use manifest::ManifestTurretRig;
use render::DrawCmd;

/// The engine base of a droid class (`bake::turret_rig::STATION_BASE`).
pub const STATION_BASE: &str = "powerupstation";

/// The serving swing's amplitude (rad) of an `ActiveRotateNode`
/// (B1 235944, §6).
pub const ARM_SWING: f32 = 0.15;

pub use core::sim::StationMotion as StationAnimState;

/// The animated nodes of one station class in its rig.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StationNodes {
    pub wobble: Option<usize>,
    /// (node, its index i in the class's list: the swing's phase).
    pub arms: Vec<(usize, usize)>,
    pub spin: Option<usize>,
}

impl StationNodes {
    /// The class's node names found in `rig` (a name the model lacks moves
    /// nothing).
    pub fn of(rig: &ManifestTurretRig, anim: &core::sim::StationAnim) -> Self {
        let find = |n: &str| {
            rig.nodes
                .iter()
                .position(|x| x.name.eq_ignore_ascii_case(n))
        };
        StationNodes {
            wobble: anim.wobble_node.as_deref().and_then(find),
            arms: anim
                .rotate_nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| Some((find(n)?, i)))
                .collect(),
            spin: anim.spin_node.as_deref().and_then(find),
        }
    }
}

/// Every node's model-space matrix: the bind pose with `s`'s writes, onto
/// `last` (the previous frame's, for the nodes written only while serving).
pub fn pose(
    rig: &ManifestTurretRig,
    nodes: &StationNodes,
    anim: &core::sim::StationAnim,
    s: &StationAnimState,
    last: &mut Vec<Mat4>,
) {
    let bind: Vec<Mat4> = (0..rig.nodes.len()).map(|n| bind_of(rig, n)).collect();
    if last.len() != bind.len() {
        *last = bind.clone();
    }
    let parent = |n: usize| rig.nodes[n].parent.map(|p| p as usize);
    let local = |n: usize| match parent(n) {
        Some(p) => bind[p].inverse() * bind[n],
        None => bind[n],
    };
    // A node turned about its own axis, under its parent as it is now.
    let turned = |now: &[Mat4], n: usize, r: Quat| {
        parent(n).map_or(Mat4::IDENTITY, |p| now[p]) * local(n) * Mat4::from_quat(r)
    };
    let tau = std::f32::consts::TAU;
    if let Some(w) = nodes.wobble {
        // From the bind origin with no rotation (the identity rows), then
        // the rock or the bob.
        let at = local(w).w_axis.truncate();
        let p = parent(w).map_or(Mat4::IDENTITY, |p| last[p]);
        last[w] = p * match s.state {
            3 => Mat4::from_translation(
                at + Vec3::Y * anim.wobble_factor * ((s.timer * tau).cos() - 1.0) * 0.5,
            ),
            0 | 2 => {
                Mat4::from_translation(at)
                    * Mat4::from_rotation_z(anim.wobble_factor * (s.wobble_freq * s.timer).sin())
            }
            _ => Mat4::from_translation(at),
        };
    }
    if s.state == 3 {
        for &(n, i) in &nodes.arms {
            let a = ARM_SWING * ((s.timer - i as f32) * tau).sin();
            last[n] = turned(last, n, Quat::from_rotation_x(a));
        }
        if let Some(n) = nodes.spin {
            let a = std::f32::consts::PI * (s.timer * tau).sin();
            last[n] = turned(last, n, Quat::from_rotation_z(a));
        }
    }
}

/// One placed station's draws (card PWST): the manifest instance, its
/// matrix, every draw it pushed with the rig part each rides, and its
/// animation.
pub struct StationDraw {
    pub instance: usize,
    /// The instance's translation: the sim's station stands exactly there
    /// (`crate::roster::add_stations`), which is how the two are joined.
    pub pos: Vec3,
    model: Mat4,
    rig: Option<ManifestTurretRig>,
    /// (the draws in `SceneGpu::draws`, the rig part they ride).
    pub(super) parts: Vec<(std::ops::Range<usize>, Option<usize>)>,
    pub(super) wreck: Vec<DrawCmd>,
    pub anim: StationAnimState,
    now: Vec<Mat4>,
    last_tick: Option<u64>,
}

impl StationDraw {
    pub(super) fn new(
        instance: usize,
        pos: Vec3,
        model: Mat4,
        rig: Option<&ManifestTurretRig>,
    ) -> Self {
        StationDraw {
            instance,
            pos,
            model,
            rig: rig.cloned(),
            parts: Vec::new(),
            wreck: Vec::new(),
            anim: StationAnimState::new(instance as u32 + 1),
            now: Vec::new(),
            last_tick: None,
        }
    }

    fn sync(&mut self, tick: u64, anim: &core::sim::StationAnim, motion: StationAnimState) {
        if self.last_tick == Some(tick) && self.anim == motion {
            return;
        }
        self.last_tick = Some(tick);
        self.anim = motion;
        if let Some(rig) = &self.rig {
            let nodes = StationNodes::of(rig, anim);
            pose(rig, &nodes, anim, &self.anim, &mut self.now);
        }
    }

    /// Each part's model matrix now: the instance turned by the idle yaw,
    /// a node's part by `node(now) x bind^-1`.
    fn apply(&self, draws: &mut [DrawCmd]) {
        let body = self.model * Mat4::from_quat(core::vehicle::hover::quat_y(self.anim.yaw));
        for (range, k) in &self.parts {
            let moved = match (&self.rig, k) {
                (Some(rig), Some(k)) => rig.parts.get(*k).copied().flatten().and_then(|n| {
                    Some(*self.now.get(n as usize)? * bind_of(rig, n as usize).inverse())
                }),
                _ => None,
            };
            let m = body * moved.unwrap_or(Mat4::IDENTITY);
            for i in range.clone() {
                draws[i].model = m;
            }
        }
    }

    /// The rig of the instance's class (none for a model with no skeleton).
    pub fn rig(&self) -> Option<&ManifestTurretRig> {
        self.rig.as_ref()
    }

    /// The model matrix of the first draw of rig part `k` (a test's read of
    /// what is drawn).
    pub fn part_model(&self, draws: &[DrawCmd], k: usize) -> Option<Mat4> {
        let (range, _) = self.parts.iter().find(|(_, p)| *p == Some(k))?;
        Some(draws[range.start].model)
    }
}

impl super::SceneGpu {
    pub fn step_stations(&mut self, sim: &core::sim::Sim) {
        for sd in &mut self.stations {
            let Some(st) = sim.stations.iter().find(|s| s.pos == sd.pos) else {
                continue;
            };
            let Some(class) = sim.station_classes.get(usize::from(st.class)) else {
                continue;
            };
            if st.destroyed {
                sd.anim = st.motion;
                sd.last_tick = Some(sim.tick);
                continue;
            }
            sd.sync(sim.tick, &class.anim, st.motion);
        }
    }

    /// Pose the final station state. Single-tick callers may use this alone;
    /// multi-tick callers must also call `step_stations` after each sim tick
    /// to retain intermediate serving changes.
    pub fn pose_stations(&mut self, sim: &core::sim::Sim) {
        self.step_stations(sim);
        for sd in &self.stations {
            sd.apply(&mut self.draws);
        }
    }

    pub(super) fn station_view_draws(
        &self,
        sim: &core::sim::Sim,
        hidden: Vec<std::ops::Range<usize>>,
        extra: Vec<DrawCmd>,
    ) -> Vec<DrawCmd> {
        let broken: Vec<_> = self
            .stations
            .iter()
            .filter(|d| {
                sim.stations
                    .iter()
                    .any(|s| s.instance == Some(d.instance as u32) && s.destroyed)
            })
            .collect();
        if broken.is_empty() && hidden.is_empty() && extra.is_empty() {
            return self.draws.clone();
        }
        let mut ranges: Vec<_> = broken
            .iter()
            .flat_map(|d| d.parts.iter().map(|(r, _)| r.clone()))
            .collect();
        ranges.extend(hidden);
        ranges.sort_by_key(|r| r.start);
        let mut ranges = ranges.into_iter().peekable();
        let mut out = Vec::with_capacity(self.draws.len());
        for (i, d) in self.draws.iter().enumerate() {
            while ranges.peek().is_some_and(|r| r.end <= i) {
                ranges.next();
            }
            if ranges.peek().is_none_or(|r| !r.contains(&i)) {
                out.push(*d);
            }
        }
        for d in broken {
            let pose = sim
                .stations
                .iter()
                .find(|s| s.instance == Some(d.instance as u32))
                .unwrap()
                .pose();
            let fade = self
                .faded
                .iter()
                .find(|(i, _)| *i as usize == d.instance)
                .map(|&(_, o)| render::Fade::see_through(o));
            for wreck in &d.wreck {
                let mut draw = *wreck;
                draw.model = pose;
                draw.fade = fade;
                out.push(draw);
            }
        }
        out.extend(extra);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sim::StationAnim;

    fn fx7() -> StationAnim {
        StationAnim {
            rotate_speed: 1.5,
            wait_time: 1.75,
            rotate_nodes: vec!["arm_a".into(), "arm_b".into()],
            spin_node: Some("socket".into()),
            ..Default::default()
        }
    }

    fn gonk() -> StationAnim {
        StationAnim {
            rotate_speed: 0.5,
            wait_time: 3.0,
            wobble_node: Some("gonk1".into()),
            wobble_factor: 0.08,
            ..Default::default()
        }
    }

    #[test]
    fn draw_reads_simulation_motion_once_and_keeps_serving_nodes_after_idle() {
        let a = fx7();
        let rig = rig();
        let mut d = StationDraw::new(0, Vec3::ZERO, Mat4::IDENTITY, Some(&rig));
        let mut motion = StationAnimState::new(1);
        motion.step(&a, true, core::TICK_DT);
        d.sync(1, &a, motion);
        let serving = d.now.clone();
        d.sync(1, &a, motion);
        assert_eq!(d.now, serving);
        motion.step(&a, false, core::TICK_DT);
        d.sync(2, &a, motion);
        assert_eq!(d.anim, motion);
        assert_eq!(d.now[1], serving[1]);
    }

    const DT: f32 = 1.0 / 30.0;

    /// Idle: wait, turn the other way by 135-225 degrees at the class's
    /// speed, wait `IdleWaitTime`, turn back.
    #[test]
    fn an_idle_droid_turns_one_way_then_the_other_with_waits_between() {
        let a = fx7();
        let mut s = StationAnimState::new(7);
        // (state, yaw) per tick for 30 s.
        let mut runs: Vec<(u8, f32, u32)> = Vec::new();
        let mut start_yaw = 0.0;
        for _ in 0..900 {
            let before = s.state;
            s.step(&a, false, DT);
            if s.state != before || runs.is_empty() {
                if let Some(r) = runs.last_mut() {
                    r.1 = s.yaw - start_yaw;
                }
                start_yaw = s.yaw;
                runs.push((s.state, 0.0, 0));
            }
            runs.last_mut().unwrap().2 += 1;
        }
        let turns: Vec<f32> = runs.iter().filter(|r| r.0 == 1).map(|r| r.1).collect();
        // A wait follows each turn; the turn before it is the yaw change.
        let mut signs = Vec::new();
        for w in runs.windows(2) {
            if w[0].0 == 0 && w[1].0 == 1 && w[0].2 > 3 {
                let deg = w[0].1.abs().to_degrees();
                assert!((133.0..=227.0).contains(&deg), "a turn of {deg} deg");
                signs.push(w[0].1.signum());
            }
            if w[0].0 == 1 && w[1].0 == 0 && w[0].2 > 3 {
                let secs = w[0].2 as f32 * DT;
                assert!((secs - 1.75).abs() < 0.07, "a wait of {secs} s");
            }
        }
        assert!(signs.len() >= 5, "{runs:?} {turns:?}");
        assert!(signs.windows(2).all(|p| p[0] != p[1]), "{signs:?}");
    }

    /// Serving: no turn; when the pass serves nobody the droid waits out
    /// what is left of its timer (already run down) and turns at once.
    #[test]
    fn a_serving_droid_stops_turning_and_turns_again_after() {
        let a = fx7();
        let mut s = StationAnimState::new(3);
        for _ in 0..100 {
            s.step(&a, false, DT);
        }
        s.step(&a, true, DT);
        assert_eq!(s.state, 3);
        let yaw = s.yaw;
        for _ in 0..150 {
            s.step(&a, true, DT);
        }
        assert_eq!(s.yaw, yaw);
        assert!(s.timer < -4.0);
        s.step(&a, false, DT);
        assert_eq!(
            s.state, 0,
            "the run-down timer starts a turn the next update"
        );
        s.step(&a, false, DT);
        assert_ne!(s.yaw, yaw);
    }

    fn rig() -> ManifestTurretRig {
        let n = |name: &str, parent, at: [f32; 3]| manifest::ManifestRigNode {
            name: name.into(),
            parent,
            bind: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], at],
        };
        ManifestTurretRig {
            nodes: vec![
                n("root", None, [0.0; 3]),
                n("arm_a", Some(0), [0.5, 1.2, 0.5]),
                n("arm_b", Some(0), [0.0, 1.2, -0.7]),
                n("socket", Some(0), [-0.2, 1.34, 0.2]),
                n("gonk1", Some(0), [0.0; 3]),
            ],
            parts: vec![Some(0), Some(1), Some(2), Some(3), Some(4)],
        }
    }

    /// Serving at timer t: each arm turned 0.15 sin(2 pi (t - i)) about its
    /// own X at its bind origin, the socket pi sin(2 pi t) about its Z; the
    /// gonk's body 0.08 (1 - cos 2 pi t) / 2 down. Idle: the body rocks
    /// about Z by 0.08 sin(f t), the arms keep their last swing.
    #[test]
    fn the_serving_nodes_swing_spin_and_bob_by_the_timer() {
        let r = rig();
        let mut both = fx7();
        both.wobble_node = Some("gonk1".into());
        both.wobble_factor = 0.08;
        let nodes = StationNodes::of(&r, &both);
        assert_eq!(nodes.arms, vec![(1, 0), (2, 1)]);
        let mut s = StationAnimState::new(1);
        s.state = 3;
        s.timer = -0.3;
        let mut now = Vec::new();
        pose(&r, &nodes, &both, &s, &mut now);
        let tau = std::f32::consts::TAU;
        let arm = ARM_SWING * (-0.3 * tau).sin();
        let want = Mat4::from_translation(Vec3::new(0.5, 1.2, 0.5)) * Mat4::from_rotation_x(arm);
        assert!(now[1].abs_diff_eq(want, 1e-5), "{:?}", now[1]);
        // Arm i = 1 is a whole period behind: in step with arm 0.
        let want = Mat4::from_translation(Vec3::new(0.0, 1.2, -0.7)) * Mat4::from_rotation_x(arm);
        assert!(now[2].abs_diff_eq(want, 1e-5), "{:?}", now[2]);
        let spin = std::f32::consts::PI * (-0.3 * tau).sin();
        let want = Mat4::from_translation(Vec3::new(-0.2, 1.34, 0.2)) * Mat4::from_rotation_z(spin);
        assert!(now[3].abs_diff_eq(want, 1e-5));
        let dy = 0.08 * ((-0.3 * tau).cos() - 1.0) * 0.5;
        assert!(now[4].w_axis.y < -0.05 && (now[4].w_axis.y - dy).abs() < 1e-6);
        // Back to idle, in a turn: the arms stay, the body rocks.
        let swung = now[1];
        s.state = 0;
        s.timer = 1.0;
        s.wobble_freq = 5.0;
        pose(&r, &nodes, &both, &s, &mut now);
        assert_eq!(now[1], swung);
        let rock = Mat4::from_rotation_z(0.08 * 5.0f32.sin());
        assert!(now[4].abs_diff_eq(rock, 1e-6));
        // Waiting: the body at its bind.
        s.state = 1;
        pose(&r, &nodes, &both, &s, &mut now);
        assert!(now[4].abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    /// The wobble's frequency is six rocks a turn: f T = 12 pi.
    #[test]
    fn a_gonk_rocks_six_times_a_turn() {
        let a = gonk();
        let mut s = StationAnimState::new(9);
        s.state = 1;
        s.timer = 0.0;
        s.step(&a, false, DT);
        assert_eq!(s.state, 0);
        let t = s.timer;
        assert!((t * a.rotate_speed / std::f32::consts::PI - 1.0).abs() <= 0.25 + 1e-6);
        assert!((s.wobble_freq * t - 12.0 * std::f32::consts::PI).abs() < 1e-3);
    }
}
