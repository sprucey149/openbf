//! The class carousel uses the deployed droideka skin, with its own idle clock.
//! UI models already face the camera; the battle model's yaw offset does not apply.

use super::*;
use core::fmath::FloatExt;

#[derive(Default)]
pub(super) struct Preview {
    clocks: HashMap<(u8, u8), f32>,
    posed: HashMap<usize, crate::viewer::vehicles::PosedMeshes<(u32, usize)>>,
}

impl DroidekaVisuals {
    pub fn preview_draws(
        &mut self,
        r: &mut Renderer,
        team: u8,
        class: u8,
        c: &DroidekaClass,
        transform: Mat4,
        dt: f32,
    ) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        let Some(k) = self
            .by_class
            .get(usize::from(team))
            .and_then(|v| v.get(usize::from(class)))
            .copied()
            .flatten()
        else {
            return out;
        };
        let model = &self.models[k];
        let Some(rig) = model.rig() else { return out };
        let clock = self.preview.clocks.entry((team, class)).or_default();
        let duration = c.duration[usize::from(core::droideka::IDLE)];
        *clock = if duration > 0.0 {
            (*clock + dt.fmax(0.0)).rem_euclid(duration)
        } else {
            0.0
        };
        let mut d = Droideka::new(c, core::droideka::Order::Deploy);
        d.clip_t = *clock;
        let palette = rig.palette(&walker_state(&d, c), 1.0);
        let slot = usize::from(team) * 256 + usize::from(class);
        let meshes = match pose_class(
            r,
            &mut self.preview.posed,
            slot,
            (0, k),
            model,
            &palette,
            true,
        ) {
            Ok(meshes) => meshes,
            Err(e) => {
                eprintln!("droideka preview {team}/{class}: {e:#}");
                return out;
            }
        };
        model.draws(&mut out, Some(&meshes), transform, (None, None), None);
        out
    }
}
