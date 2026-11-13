//! A jet soldier's `JetEffect` (card JETP2, `analysis/re-jetpack.md` §8):
//! spawned once when he is in a jet stance and has none running, riding him
//! while he jets, stopped (its particles live out) when the jet ends, he
//! dies or he is gone - BF2 `FUN_00239bb0` (B2:153393-153421) and
//! `FUN_00239ad8`, BF1 `FUN_002013a0` (B1:113213-113236) and `FUN_002012c8`.
//!
//! Where it rides: the PS2 attaches it to the soldier (BF1 `FUN_0036ced0`
//! with no hard point: the soldier's own matrix; BF2 at the hard point
//! `DAT_005fdd48 + 0x11ab0`, not named). The disc's jet effects
//! (`jetpack`, `darktrooper_jetpack`, `rep_sfx_jetpack`, ...) emit along
//! their frame's -x from offsets x -0.24..-0.6, y -0.21, z +-0.15: two
//! nozzles below and behind a frame whose x runs up the spine and y ahead,
//! as the jet troopers' `bone_ribcage` does (x up, y ahead, z the model's
//! +x; 1.48 m up in the bind pose): 0.24 m under it is the bottom of the
//! dark trooper's pack (its part spans 1.22-1.72 m). PROVISIONAL [H]: the
//! effect rides the drawn `bone_ribcage` ([`Fx::set_jet_frames`], from
//! `SoldierVisuals::jet_frames`; the windowed game sets it every drawn frame
//! from the pose that frame draws, [`Fx::advance_frame`]), else - a soldier
//! drawn without a model, the tick before his first pose - that bone's bind
//! pose turned to his yaw ([`jet_frame`]).

use super::emitter::Frame;
use super::Fx;
use crate::soldier_vis::SoldierVisuals;
use crate::walk::WalkSim;
use core::sim::{PadState, Sim};
use core::unit::Unit;
use glam::{Mat3, Mat4, Vec3};

/// `bone_ribcage` in the jet troopers' and dark troopers' bind pose (both
/// games' models: 1.481 m up, 0.023 m behind the feet).
pub const JET_FRAME_AT: Vec3 = Vec3::new(0.0, 1.481, -0.023);

/// The frame a jet effect rides in for a soldier at `pos` turned to `yaw`
/// (the sim's: 0 faces -Z, the model faces +Z, `soldier_vis::model_of`):
/// x up, y his forward, z the model's +x.
pub fn jet_frame(pos: Vec3, yaw: f32, vel: Vec3) -> Frame {
    // About +y by yaw + pi (`core::fmath`, as every game file computes).
    let (sin, cos) = core::fmath::sin_cos(yaw + std::f32::consts::PI);
    let turn = Mat3::from_cols(Vec3::new(cos, 0.0, -sin), Vec3::Y, Vec3::new(sin, 0.0, cos));
    let rot = turn * Mat3::from_cols(Vec3::Y, Vec3::Z, Vec3::X);
    Frame {
        rot,
        pos: pos + turn * JET_FRAME_AT,
        vel,
    }
}

/// A drawn bone's world matrix as an effect frame (its axes unscaled).
pub fn bone_frame(m: Mat4, vel: Vec3) -> Frame {
    let rot = Mat3::from_mat4(m);
    Frame {
        rot: Mat3::from_cols(
            rot.x_axis.normalize_or_zero(),
            rot.y_axis.normalize_or_zero(),
            rot.z_axis.normalize_or_zero(),
        ),
        pos: m.w_axis.truncate(),
        vel,
    }
}

impl Fx {
    /// The drawn `bone_ribcage` of every soldier, by unit index
    /// (`SoldierVisuals::jet_frames`): the running jet effects move onto
    /// it now, and the next [`Fx::ingest`] spawns new ones on it.
    pub fn set_jet_frames(&mut self, frames: Vec<Option<Mat4>>) {
        for e in &mut self.live {
            if let Some(h) = e.jet {
                if let Some(Some(m)) = frames.get(h.index as usize) {
                    e.frame = bone_frame(*m, e.frame.vel);
                }
            }
        }
        self.jet_frames = frames;
    }

    /// The windowed game's effects step for one drawn frame, after its
    /// ticks: the soldiers' poses advance by `dt`, the effects that ride
    /// them move onto where this frame draws them, then the effects advance
    /// (and emit) by `dt`.
    pub fn advance_frame(
        &mut self,
        vis: Option<&mut SoldierVisuals>,
        walk: &mut WalkSim,
        look: &PadState,
        dt: f32,
    ) {
        let restore = !walk.blend.applied();
        if restore {
            crate::render_blend::blend_in_looking(walk, look);
        }
        self.follow_flyer_exhaust(walk);
        self.follow_hover_water(walk);
        if let Some(vis) = vis {
            vis.update(walk, dt);
            let (muzzles, jets, burns) = (
                vis.fire_points(walk),
                vis.jet_frames(walk),
                vis.burn_frames(walk),
            );
            if self.force_bolts() > 0 {
                self.set_force_origins(&vis.force_points(walk));
            }
            self.set_repair_origins(&muzzles);
            self.set_jet_frames(jets);
            self.set_burn_frames(burns);
        }
        // Registered exhaust samples must be born at the same presentation
        // position as the model, as well as the attached NoRegisterStep head.
        // Otherwise each new tick leaves bright particles ahead of a blended model.
        let saved = self.drawn_round_frames(&walk.sim);
        self.advance_space_dust(walk, dt);
        if restore {
            walk.blend_out();
        }
        self.update(dt);
        self.restore_round_frames(saved);
    }

    /// Card JETP2: keep a jet effect on every soldier in a jet stance whose
    /// class names one (`effect`), and stop the rest.
    pub(super) fn follow_jets<'b>(&mut self, sim: &Sim, effect: impl Fn(&Unit) -> Option<&'b str>) {
        let drawn = std::mem::take(&mut self.jet_frames);
        let frame = |i: usize, u: &Unit| match drawn.get(i) {
            Some(Some(m)) => bone_frame(*m, u.soldier.vel),
            _ => jet_frame(u.soldier.pos, u.soldier.yaw, u.soldier.vel),
        };
        for e in &mut self.live {
            let Some(h) = e.jet else { continue };
            match sim
                .unit(h)
                .filter(|u| u.alive && u.soldier.stance.jetting())
            {
                Some(u) => e.frame = frame(h.index as usize, u),
                None => {
                    e.stop();
                    e.jet = None;
                }
            }
        }
        for (h, u) in sim.alive_units() {
            if !u.soldier.stance.jetting() || self.live.iter().any(|e| e.jet == Some(h)) {
                continue;
            }
            let Some(name) = effect(u) else { continue };
            if self.spawn(name, frame(h.index as usize, u)) {
                if let Some(e) = self.live.last_mut() {
                    e.jet = Some(h);
                }
            }
        }
        self.jet_frames = drawn;
    }

    /// The soldiers whose jet effect runs now (tests, the referee).
    pub fn jets(&self) -> Vec<core::unit::UnitHandle> {
        self.live.iter().filter_map(|e| e.jet).collect()
    }
}
