use super::SoldierVisuals;
use crate::walk::WalkSim;
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct WaterFeet {
    pub world: [Vec3; 2],
    pub height: [f32; 2],
}

impl SoldierVisuals {
    pub fn water_feet(&self, walk: &WalkSim) -> Vec<Option<WaterFeet>> {
        let mut out = vec![None; walk.sim.units.len()];
        for (i, sa, model, palette) in self.posed(walk) {
            let skeleton = &sa.asset.skeleton;
            let Some(left) = skeleton.index_of("bone_l_foot") else {
                continue;
            };
            let Some(right) = skeleton.index_of("bone_r_foot") else {
                continue;
            };
            let feet = [left, right]
                .map(|b| (palette[b] * skeleton.global_bind[b]).transform_point3(Vec3::ZERO));
            out[i] = Some(WaterFeet {
                world: feet.map(|p| model.transform_point3(p)),
                height: feet.map(|p| p.y),
            });
        }
        out
    }
}
