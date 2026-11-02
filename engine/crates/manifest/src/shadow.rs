//! Render-only shadow sidecar, separate from gameplay geometry.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowPiece {
    pub node: String,
    /// Model-space at bind, so a walker's skin palette also moves its shadow.
    pub positions: Vec<[f32; 3]>,
    pub faces: Vec<Vec<u16>>,
}
