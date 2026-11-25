//! Saber trails keep actual posed world frames rather than resampling a
//! single previous clip key. Their clocks never enter gameplay state.

use super::{SoldierVisuals, WalkSim};
use render::{ParticleDraw, ParticleShape};

mod history;
use history::History;

#[derive(Default)]
pub(super) struct State {
    wearer: Option<(u32, usize)>,
    edges: Vec<History>,
}

impl SoldierVisuals {
    pub(super) fn step_saber_trails(&mut self, walk: &WalkSim, dt: f32) {
        let any_saber = walk.sim.units.iter().any(|u| {
            u.alive
                && u.mount.is_none()
                && u.weapon.melee.is_some()
                && self.saber_visual(u.team, u.spec.class).is_some()
        });
        if !any_saber && self.saber_trails.iter().all(|s| s.edges.is_empty()) {
            return;
        }
        let mut packets = vec![None; walk.sim.units.len()];
        for (i, u) in walk.sim.units.iter().enumerate() {
            if !u.alive || u.mount.is_some() {
                continue;
            }
            let Some(melee) = u.weapon.melee.as_ref() else {
                continue;
            };
            let Some(visual) = self.saber_visual(u.team, u.spec.class) else {
                continue;
            };
            let Some((asset, model, palette)) = self.pose_unit(walk, i, false) else {
                continue;
            };
            let mut edges = Vec::new();
            for (edge, binding) in visual.bindings.iter().enumerate() {
                let frame = binding.blade.trail_color.and_then(|color| {
                    if binding.blade.texture.is_empty()
                        || !core::hero_weapon::blade_visible(melee.hidden_edges, edge)
                    {
                        return None;
                    }
                    let bone = asset.asset.skeleton.index_of(&binding.blade.bone)?;
                    let at = model * palette[bone] * asset.asset.skeleton.global_bind[bone];
                    let segment = binding.segment(at);
                    Some((segment.start, segment.end, binding.blade.width, color))
                });
                let active = frame.is_some()
                    && !self.primary_saber_flying(walk, i, &binding.blade.bone)
                    && melee.attacks.iter().any(|a| usize::from(a.edge) == edge);
                edges.push((frame, active));
            }
            packets[i] = Some((self.asset_index(u.team, u.spec.class).unwrap(), edges));
        }
        self.saber_trails
            .resize_with(walk.sim.units.len(), State::default);
        for (i, state) in self.saber_trails.iter_mut().enumerate() {
            let u = &walk.sim.units[i];
            if !u.alive || state.wearer.is_some_and(|(gen, _)| gen != u.gen) {
                *state = State::default();
            }
            if let Some((asset, frames)) = packets[i].take() {
                if state.wearer != Some((u.gen, asset)) {
                    *state = State {
                        wearer: Some((u.gen, asset)),
                        edges: Vec::new(),
                    };
                }
                state.edges.resize_with(frames.len(), History::default);
                for (history, (frame, active)) in state.edges.iter_mut().zip(frames) {
                    history.step(dt, frame, active);
                }
            } else {
                for history in &mut state.edges {
                    history.step(dt, None, false);
                }
            }
        }
    }

    pub(super) fn saber_trail_draws(&self, walk: &WalkSim) -> Vec<ParticleDraw> {
        let mut draws = Vec::new();
        for (i, state) in self.saber_trails.iter().enumerate() {
            if state.edges.is_empty()
                || !walk.sim.units.get(i).is_some_and(|u| u.alive)
                || self.pose_unit(walk, i, false).is_none()
            {
                continue;
            }
            for history in &state.edges {
                let sections = history.sections();
                if sections.len() < 2 || !sections.iter().any(|s| s.3[3] != 0) {
                    continue;
                }
                let key = sections.last().unwrap().0;
                draws.push(ParticleDraw {
                    shape: ParticleShape::Trail {
                        sections,
                        length: history.length,
                    },
                    key,
                    color: [255; 4],
                    texture: None,
                    additive: true,
                });
            }
        }
        draws
    }
}
