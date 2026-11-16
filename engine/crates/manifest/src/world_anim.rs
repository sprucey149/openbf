//! A map's world animations, beside its manifest as [`FILE`]: every `wrld`'s
//! `anim` and `anmg` chunks (`mungefs::world_anim`), each tagged with the
//! world that carries it so a mode layer the round does not load plays none
//! of its own. `bake world-anims` writes it, and `bake map` does too.

use serde::{Deserialize, Serialize};

pub const FILE: &str = "world_anims.ron";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldAnimKey {
    pub time: f32,
    pub value: [f32; 3],
    /// 0 step, 1 linear, 2 spline.
    pub interp: u8,
    pub tangent_out: [f32; 3],
    pub tangent_in: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldAnim {
    pub world: String,
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    pub local: bool,
    pub pos: Vec<WorldAnimKey>,
    pub rot: Vec<WorldAnimKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldAnimGroup {
    pub world: String,
    pub name: String,
    pub play: bool,
    pub restart_on_state: bool,
    pub no_hierarchy: bool,
    /// (animation name, object name).
    pub entries: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldAnims {
    pub anims: Vec<WorldAnim>,
    pub groups: Vec<WorldAnimGroup>,
}
