//! Disc-authored cloth beside a soldier GLB. Older bakes keep their rigid
//! skinned fallback; a companion opts only its listed parts into simulation.
//! Written for every soldier that wears cloth, absent for one that wears none.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cloth {
    pub name: String,
    pub part: usize,
    pub parent: String,
    pub frame: [[f32; 3]; 4],
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub pinned: usize,
    pub pin_bones: Vec<String>,
    pub triangles: Vec<[u32; 3]>,
    /// DATA list order: stretch, cross, bend. The PS2 applies them backwards.
    pub constraints: [Vec<[u32; 2]>; 3],
    pub enabled: [bool; 3],
    pub shapes: Vec<Shape>,
    pub wind_direction: [f32; 3],
    pub wind_speed: f32,
    pub dampening: f32,
    pub drag: f32,
    pub mass: f32,
    pub max_acceleration: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub bone: String,
    pub kind: u32,
    pub params: [f32; 3],
    pub frame: [[f32; 3]; 4],
}

pub fn path(side: &Path, model: &str) -> PathBuf {
    side.join("soldiers").join(format!("{model}.cloths.ron"))
}
