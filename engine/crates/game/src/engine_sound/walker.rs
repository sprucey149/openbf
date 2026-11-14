//! Animated walker footsteps. BF2 FUN_0025aaa0:176135-176157,
//! 176237; BF1 FUN_00216b30:128139-128158,128224.
//! See analysis/re-vehicle-sounds.md §7. Presentation only.
use super::{JetShot, JetShotKind};
use crate::{classdb::ClassDb, walk::WalkSim};
use glam::Vec3;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Foot {
    pub bone: String,
    /// Underside in bind model space; the palette carries it with its bone.
    pub bottom: Vec3,
    pub sound: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    pub feet: Vec<Foot>,
    pub detection: u8,
    pub threshold: f32,
    pub bf2: bool,
}

impl Class {
    pub fn from_db(db: &ClassDb, name: &str, prims: &[manifest::ManifestPrim]) -> Option<Self> {
        let walker = crate::vehicle_class::WalkerClass::from_db(db, name)?;
        let names: Vec<_> = if db.is_bf2() {
            walker
                .leg_pairs
                .iter()
                .flat_map(|p| p.feet.iter().flatten().map(String::as_str))
                .collect()
        } else {
            db.props_all(name, "TerrainCollision")
        };
        let feet = names
            .iter()
            .enumerate()
            .filter_map(|(i, foot_name)| {
                let p = prims
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(foot_name))?;
                let half_height = match p.kind.as_str() {
                    "cylinder" => p.dims[1] * 0.5,
                    "sphere" => p.dims[0],
                    "box" => p.dims[1],
                    _ => return None,
                };
                Some(Foot {
                    bone: p.parent.clone(),
                    bottom: Vec3::from(p.translation) - Vec3::from(p.rotation[1]) * half_height,
                    sound: crate::roster::sound_key(db, name, &format!("FootstepSound{i}")),
                })
            })
            .collect();
        Some(Self {
            feet,
            detection: db.prop_f32(name, "StompDetectionType").unwrap_or(0.0) as u8,
            threshold: if db.is_bf2() {
                db.prop_f32(name, "StompThreshold").unwrap_or(0.15)
            } else {
                0.05
            },
            bf2: db.is_bf2(),
        })
    }
}

#[derive(Default)]
struct State {
    previous: Option<f32>,
    low: Option<f32>,
    latched: bool,
}
impl State {
    fn sample(&mut self, h: f32, c: &Class) -> bool {
        let prev = self.previous.replace(h).unwrap_or(h);
        let low = self.low.unwrap_or(f32::MAX);
        let step = if c.detection == 0 {
            (!c.bf2 || h < prev) && h < low + c.threshold && low + c.threshold < prev
        } else if !self.latched && prev - h < 0.1 {
            self.latched = true;
            true
        } else {
            if self.latched && prev - h > 0.1 {
                self.latched = false;
            }
            false
        };
        self.low = Some(low.min(h));
        step
    }
}

#[derive(Default)]
pub struct Steps {
    vehicles: HashMap<(usize, u32), Vec<State>>,
}
impl Steps {
    pub fn tick(&mut self, walk: &WalkSim) -> Vec<JetShot> {
        let mut shots = Vec::new();
        let mut live = Vec::new();
        for (i, v) in walk
            .sim
            .vehicles
            .iter()
            .enumerate()
            .filter(|(_, v)| v.alive)
        {
            let class = v.class as usize;
            let Some(c) = walk
                .vehicle_sounds
                .get(class)
                .and_then(|c| c.walker_steps.as_ref())
            else {
                continue;
            };
            let key = (i, v.gen);
            live.push(key);
            let states = self.vehicles.entry(key).or_default();
            states.resize_with(c.feet.len(), State::default);
            if v.walker.anim == core::vehicle::walker::ANIM_IDLE {
                for s in states {
                    s.latched = false;
                }
                continue;
            }
            let Some(rig) = walk.walker_rigs.get(class).and_then(Option::as_ref) else {
                continue;
            };
            let Some(p) = walk.sim.walker_params.get(class).and_then(Option::as_ref) else {
                continue;
            };
            let palette = rig.aimed_palette(v, p.root_scale);
            for (foot, state) in c.feet.iter().zip(states) {
                let Some(bone) = rig
                    .skeleton
                    .names
                    .iter()
                    .position(|b| b.eq_ignore_ascii_case(&foot.bone))
                else {
                    continue;
                };
                let bottom = v.rot * palette[bone].transform_point3(foot.bottom);
                if state.sample(bottom.y, c) {
                    if let Some(name) = &foot.sound {
                        shots.push(JetShot {
                            unit: i as u32,
                            kind: JetShotKind::WalkerStep,
                            name: name.clone(),
                            pos: Vec3::new(v.pos.x + bottom.x, v.pos.y, v.pos.z + bottom.z),
                        });
                    }
                }
            }
        }
        self.vehicles.retain(|k, _| live.contains(k));
        shots
    }
}
