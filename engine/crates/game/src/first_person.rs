//! First-person soldier arms, camera offset and animation state machine.
//!
//! Poses the 9-track FPM skeleton (`re-first-person-arms.md`) under the eye
//! camera (`re-first-person.md` §1), stepping clip choice and playback.

use crate::skin::{Clip, Pose, Skeleton};
use glam::{Mat4, Vec3};
use manifest::fpm::{BF1_TABLE, BF2_TABLE, COLUMNS};

pub const FPS: f32 = 30.0;
pub const BLEND_PER_SEC: f32 = 3.0;
pub const IDLE_SPEED2: f32 = 0.1;
pub const RUN_RATE_PER_SPEED2: f32 = 0.05;
pub const RUN_RATE_FULL_SPEED2: f32 = 20.0;
pub const RUN_RATE: f32 = 0.75;
pub const LAND_SECS: f32 = 0.5;
pub const FALL_SECS: f32 = 1.0;
pub const RAISE_SECS: f32 = 1.0;
pub const OFFSET_YAW: f32 = 3.0;
pub const OFFSET_ROLL: f32 = 0.05;
pub const BF1_OFFSET: Vec3 = Vec3::new(0.0, 0.35, -0.3);
pub const BF2_OFFSET: Vec3 = Vec3::new(0.0, 0.35, 0.05);
pub const CAMERA_BONE: &str = "hp_camera";
pub const WEAPON_BONE: &str = "hp_weapons";

pub mod column {
    pub const IDLE: usize = 0;
    pub const RUN: usize = 1;
    pub const SHOOT: usize = 2;
    pub const SHOOT2: usize = 3;
    pub const RELOAD: usize = 5;
    pub const JUMP: usize = 7;
    pub const FLAIL: usize = 8;
    pub const HANDSDOWN: usize = 9;
    pub const LAND: usize = 10;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FpWeapon {
    pub geometry: Option<String>,
    pub row: usize,
}

pub fn bf1_row(weapon_set: usize) -> usize {
    match weapon_set {
        0 | 7 => 0,
        1 => 2,
        3 => 4,
        8 => 1,
        _ => 3,
    }
}

pub fn bf2_row(offhand: bool, base_class: Option<&str>) -> usize {
    if offhand {
        return 3;
    }
    match base_class {
        Some("rifle") => 0,
        Some("bazooka") => 1,
        _ => 2,
    }
}

pub fn clip_name(bf2: bool, row: usize, column: usize) -> &'static str {
    if bf2 {
        BF2_TABLE[row.min(BF2_TABLE.len() - 1)][column.min(COLUMNS - 1)]
    } else {
        BF1_TABLE[row.min(BF1_TABLE.len() - 1)][column.min(COLUMNS - 1)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    #[default]
    Ground,
    Prone,
    Jump,
    Fall,
    Jet,
    Roll,
}

impl Motion {
    pub fn of(stance: core::soldier::Stance) -> Self {
        use core::soldier::Stance;
        match stance {
            Stance::Prone => Motion::Prone,
            Stance::Jump => Motion::Jump,
            Stance::Fall => Motion::Fall,
            Stance::Jet | Stance::Hover => Motion::Jet,
            Stance::Roll => Motion::Roll,
            _ => Motion::Ground,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fire {
    #[default]
    Idle,
    Shooting,
    Reloading,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    pub weapon: usize,
    pub row: usize,
    pub fire: Fire,
    pub shot: bool,
    pub motion: Motion,
    pub speed2: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Play {
    pub row: usize,
    pub column: usize,
    pub looping: bool,
    pub snap: bool,
}

#[derive(Debug, Clone)]
pub struct Chooser {
    pub column: usize,
    pub row: usize,
    pub weapon: Option<usize>,
    pub pending: usize,
    pub pending_secs: f32,
    pub pending_timer: f32,
    pub lower: bool,
}

impl Default for Chooser {
    fn default() -> Self {
        Self {
            column: COLUMNS,
            row: BF1_TABLE.len(),
            weapon: None,
            pending: 0,
            pending_secs: 0.0,
            pending_timer: 0.0,
            lower: false,
        }
    }
}

impl Chooser {
    pub fn spawned(&mut self) {
        self.lower = true;
        self.weapon = None;
    }

    pub fn step(&mut self, i: &Input, finished: bool, blending: bool, dt: f32) -> Option<Play> {
        let mut restart = false;
        let mut col = self.column;
        let mut looping = false;
        let mut snap = false;
        let mut pending_on = true;
        if matches!(self.column, column::SHOOT | column::SHOOT2) && !finished && !i.shot {
            return None;
        }
        let mut proceed = true;
        match self.weapon {
            Some(w) if w != i.weapon && !i.shot => {
                restart = true;
                if self.column == column::HANDSDOWN {
                    if blending {
                        return None;
                    }
                    self.weapon = Some(i.weapon);
                } else {
                    col = column::HANDSDOWN;
                    proceed = false;
                }
            }
            _ => self.weapon = Some(i.weapon),
        }
        if proceed {
            let mut moving = false;
            match i.fire {
                Fire::Shooting if i.shot => {
                    col = column::SHOOT;
                    snap = true;
                    restart = true;
                }
                Fire::Reloading => col = column::RELOAD,
                _ => moving = true,
            }
            if moving {
                match i.motion {
                    Motion::Jump => {
                        self.pending = column::LAND;
                        col = column::JUMP;
                        self.pending_secs = LAND_SECS;
                        self.pending_timer = 0.0;
                        pending_on = false;
                    }
                    Motion::Fall => {
                        self.pending = column::HANDSDOWN;
                        col = column::FLAIL;
                        self.pending_secs = FALL_SECS;
                        self.pending_timer = 0.0;
                        looping = true;
                        pending_on = false;
                    }
                    Motion::Jet => {}
                    Motion::Roll => col = column::JUMP,
                    Motion::Ground | Motion::Prone => {
                        col = if i.speed2 < IDLE_SPEED2 {
                            column::IDLE
                        } else {
                            column::RUN
                        };
                        looping = true;
                    }
                }
            }
        }
        if i.speed2 >= IDLE_SPEED2 && i.motion == Motion::Prone {
            self.pending_timer = 0.0;
            self.pending = column::HANDSDOWN;
            pending_on = false;
            self.pending_secs = 0.0;
            looping = true;
            col = column::HANDSDOWN;
        }
        if pending_on && self.pending_timer < self.pending_secs {
            col = self.pending;
            self.pending_timer += dt;
        }
        if self.lower {
            snap = true;
            col = column::HANDSDOWN;
            restart = true;
            self.lower = false;
            looping = false;
            self.pending_secs = RAISE_SECS;
            self.pending_timer = 0.0;
            self.pending = column::IDLE;
        }
        self.row = i.row;
        if col != self.column {
            restart = true;
        }
        self.column = col;
        restart.then_some(Play {
            row: i.row,
            column: col,
            looping,
            snap,
        })
    }
}

pub fn run_rate(column: usize, speed2: f32) -> f32 {
    if column != column::RUN {
        return 1.0;
    }
    let s = if speed2 <= RUN_RATE_FULL_SPEED2 {
        speed2 * RUN_RATE_PER_SPEED2
    } else {
        1.0
    };
    s * RUN_RATE
}

#[derive(Debug, Clone, Default)]
pub struct Playback {
    pub clip: Option<usize>,
    pub time: f32,
    pub looping: bool,
    pub blend: f32,
    pub pose: Option<Pose>,
}

impl Playback {
    pub fn play(&mut self, clip: Option<usize>, looping: bool, snap: bool) {
        self.clip = clip;
        self.time = 0.0;
        self.looping = looping;
        self.blend = if snap || self.pose.is_none() {
            0.0
        } else {
            1.0
        };
    }

    pub fn finished(&self, clips: &[Clip]) -> bool {
        match self.clip.and_then(|c| clips.get(c)) {
            Some(c) => !self.looping && self.time * FPS >= c.frames.saturating_sub(1) as f32,
            None => true,
        }
    }

    pub fn advance(&mut self, skel: &Skeleton, clips: &[Clip], rate: f32, dt: f32) -> &Pose {
        self.time += rate * dt;
        let sample = match self.clip.and_then(|c| clips.get(c)) {
            Some(c) => c.sample(skel, self.time * FPS, self.looping),
            None => Pose::bind(skel),
        };
        let pose = match self.pose.take() {
            Some(last) if self.blend > 0.0 => {
                let w = 1.0 - self.blend;
                self.blend -= dt * BLEND_PER_SEC;
                last.blend(&sample, w * w)
            }
            _ => sample,
        };
        self.pose.insert(pose)
    }
}

pub fn camera_offset(bf2: bool) -> Mat4 {
    let t = if bf2 { BF2_OFFSET } else { BF1_OFFSET };
    let (sz, cz) = core::fmath::sin_cos(OFFSET_ROLL * 0.5);
    let (sy, cy) = core::fmath::sin_cos(OFFSET_YAW * 0.5);
    let rz = glam::Quat::from_xyzw(0.0, 0.0, sz, cz);
    let ry = glam::Quat::from_xyzw(0.0, sy, 0.0, cy);
    Mat4::from_rotation_translation(rz * ry, t)
}

pub fn ps2_tan_half_fov_y(bf2: bool) -> f32 {
    let rules = if bf2 {
        core::soldier::GroundRules::Bf2
    } else {
        core::soldier::GroundRules::Bf1
    };
    let (_, first) = crate::walk::ps2_soldier_fovs(rules);
    let (s, c) = core::fmath::sin_cos(first.to_radians() * 0.5);
    crate::walk::PS2_FOV_TAN_SCALE * s / c
}

pub fn frame_scale(fov_y: f32, bf2: bool) -> f32 {
    let (s, c) = core::fmath::sin_cos(fov_y * 0.5);
    s / c / ps2_tan_half_fov_y(bf2)
}

pub fn model_matrix(eye: Mat4, camera_bone: Mat4, bf2: bool, frame: f32) -> Mat4 {
    eye * Mat4::from_scale(Vec3::new(frame, frame, 1.0)) * camera_bone * camera_offset(bf2)
}

pub fn bone_global(skel: &Skeleton, palette: &[Mat4], bone: usize) -> Mat4 {
    palette[bone] * skel.global_bind[bone]
}

#[cfg(test)]
mod tests;
