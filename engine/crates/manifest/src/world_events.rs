//! The bounded environment program baked alongside an Instant Action mission.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Anchor {
    Object(String),
    PathPoint(String, usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    PlayAnimation(String),
    PauseAnimation(String),
    KillObject(String),
    Barrier(String, bool),
    Arc(String, bool),
    Effect(String, Anchor),
    After(f32, Vec<Action>),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldEvents {
    pub start: Vec<Action>,
    pub object_destroyed: Vec<(String, Vec<Action>)>,
}

impl WorldEvents {
    pub fn is_empty(&self) -> bool {
        self.start.is_empty() && self.object_destroyed.is_empty()
    }
}
