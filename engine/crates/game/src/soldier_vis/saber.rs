//! Textured saber edges and every authored hand attachment. The same
//! bindings supply the sim's baked pose sweeps.

use super::{SoldierVisuals, WeaponAsset};
use crate::hero_blades::BladeBinding;
use crate::walk::{TeamTemplate, WalkSim};
use glam::Mat4;
use render::{ParticleDraw, ParticleShape, Renderer, TextureHandle};
use std::{cell::Cell, collections::HashMap, path::Path};

pub(super) struct SaberVisual {
    pub(super) bindings: Vec<BladeBinding>,
    textures: Vec<Option<TextureHandle>>,
    glows: Vec<Option<TextureHandle>>,
    hilts: Vec<(String, WeaponAsset)>,
    rng: Cell<u32>,
}

fn blade_particle(
    segment: core::hero_weapon::BladeSegment,
    width: f32,
    texture: Option<TextureHandle>,
) -> ParticleDraw {
    ParticleDraw {
        shape: ParticleShape::CappedRibbon {
            head: segment.end,
            head_half: width * 0.8,
            tail: segment.start - (segment.end - segment.start) * 0.04,
            tail_half: width,
        },
        key: (segment.start + segment.end) * 0.5,
        color: [255; 4],
        texture,
        additive: true,
    }
}

fn glow_particle(
    segment: core::hero_weapon::BladeSegment,
    width: f32,
    texture: Option<TextureHandle>,
    rng: &Cell<u32>,
) -> ParticleDraw {
    let a = rng.get().wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
    let b = a.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
    rng.set(b);
    let fraction = f32::from_bits(((a >> 16 | b & 0xffff0000) >> 9) | 0x3f800000) - 1.0;
    let width = width * (1.5 + fraction);
    ParticleDraw {
        shape: ParticleShape::CappedRibbon {
            head: segment.end,
            head_half: width * 0.8,
            tail: segment.start,
            tail_half: width,
        },
        key: (segment.start + segment.end) * 0.5,
        color: [255, 255, 255, 0x33],
        texture,
        additive: true,
    }
}

fn edge_draws(
    visual: &SaberVisual,
    edge: usize,
    binding: &BladeBinding,
    at: Mat4,
    out: &mut Vec<ParticleDraw>,
) {
    if binding.blade.texture.is_empty() {
        return;
    }
    let Some(texture) = visual.textures[edge] else {
        return;
    };
    let segment = binding.segment(at);
    out.push(blade_particle(segment, binding.blade.width, Some(texture)));
    if let Some(glow) = visual.glows[edge] {
        out.push(glow_particle(
            segment,
            binding.blade.width,
            Some(glow),
            &visual.rng,
        ));
    }
}

impl SoldierVisuals {
    pub(super) fn load_sabers(
        &mut self,
        mut renderer: Option<&mut Renderer>,
        root: &Path,
        teams: &[TeamTemplate; 2],
    ) {
        let mut factions: Vec<_> = teams.iter().map(|t| t.faction.as_str()).collect();
        for class in teams.iter().flat_map(|t| &t.classes) {
            if let Some(side) = class.name.split('_').next() {
                if ["all", "imp", "rep", "cis"].contains(&side) && !factions.contains(&side) {
                    factions.push(side);
                }
            }
        }
        let classes = root.join("classes");
        let mut db = match crate::roster::load_classdb(&classes, "bf2", &factions) {
            Ok(db) => db,
            Err(error) => {
                eprintln!("saber classes: {error:#}");
                return;
            }
        };
        if let Err(error) = crate::dlc::load_classes(&classes, &mut db) {
            eprintln!("saber classes: {error:#}");
        }
        let mut cache = HashMap::new();
        let side = root.join("side_bf2");
        for (team, template) in teams.iter().enumerate() {
            let mut visuals = Vec::new();
            for class in &template.classes {
                let build = || -> anyhow::Result<Option<Vec<BladeBinding>>> {
                    let Some((weapon, _)) = crate::roster::primary_weapon(&db, &class.name) else {
                        return Ok(None);
                    };
                    if db.engine_base(&weapon) != Some("melee") {
                        return Ok(None);
                    }
                    Ok(Some(crate::hero_blades::blade_bindings(
                        root, &db, &weapon,
                    )?))
                };
                let bindings = match build() {
                    Ok(Some(bindings)) => bindings,
                    Ok(None) => {
                        visuals.push(None);
                        continue;
                    }
                    Err(error) => {
                        eprintln!("{}: saber draw: {error:#}", class.name);
                        visuals.push(None);
                        continue;
                    }
                };
                let mut textures = Vec::new();
                let mut glows = Vec::new();
                let mut hilts = Vec::new();
                let mut models: Vec<(String, String)> = Vec::new();
                for binding in &bindings {
                    let texture = renderer.as_deref_mut().and_then(|r| {
                        (!binding.blade.texture.is_empty())
                            .then(|| {
                                Self::texture(
                                    r,
                                    &side,
                                    Some(&binding.blade.texture),
                                    [0.0; 2],
                                    &mut cache,
                                )
                            })
                            .flatten()
                    });
                    textures.push(texture);
                    glows.push(renderer.as_deref_mut().and_then(|r| {
                        let name = binding.glow_texture.as_deref()?;
                        let group =
                            if pak::exists(side.join("textures").join(format!("{name}.ktx2"))) {
                                side.clone()
                            } else {
                                root.join("common_bf2")
                            };
                        Self::texture(r, &group, Some(name), [0.0; 2], &mut cache)
                    }));
                    if let Some(model) = &binding.geometry {
                        let key = (binding.blade.bone.clone(), model.clone());
                        if !models.contains(&key) {
                            let asset = Self::load_weapon(
                                renderer.as_deref_mut(),
                                root,
                                "bf2",
                                model,
                                &mut cache,
                            );
                            hilts.push((binding.blade.bone.clone(), asset));
                            models.push(key);
                        }
                    }
                }
                visuals.push(Some(SaberVisual {
                    bindings,
                    textures,
                    glows,
                    hilts,
                    // [ours] A class-local cosmetic stream keeps rendering
                    // independent of both simulation and other effects.
                    rng: Cell::new(0x53414252 ^ crate::classdb::fnv1a_lowercase(&class.name)),
                }));
            }
            self.sabers[team] = visuals;
        }
    }

    pub(super) fn saber_visual(&self, team: u8, class: u8) -> Option<&SaberVisual> {
        self.sabers
            .get(usize::from(team))?
            .get(usize::from(class))?
            .as_ref()
    }

    fn flying_saber<'a>(
        &self,
        walk: &'a WalkSim,
        index: usize,
    ) -> Option<(
        &'a core::hero_weapon::SaberFlight,
        &'a core::hero_weapon::SaberThrow,
    )> {
        let u = &walk.sim.units[index];
        std::iter::once((u.weapon_class, &u.weapon))
            .chain(u.held.iter().map(|w| (w.class, &w.state)))
            .find_map(|(class, state)| {
                Some((
                    state.saber_flight.as_ref()?,
                    walk.sim.weapon_classes[class].saber_throw.as_ref()?,
                ))
            })
    }

    fn flying_transform(&self, walk: &WalkSim, index: usize) -> Option<Mat4> {
        let (flight, _) = self.flying_saber(walk, index)?;
        Some(flight.transform())
    }

    pub(super) fn primary_saber_flying(&self, walk: &WalkSim, index: usize, bone: &str) -> bool {
        bone.eq_ignore_ascii_case("hp_weapons") && self.flying_transform(walk, index).is_some()
    }

    pub fn saber_hilts(&self, walk: &WalkSim) -> Vec<render::DrawCmd> {
        let mut out = Vec::new();
        for (i, asset, model, palette) in self.posed(walk) {
            let u = &walk.sim.units[i];
            if u.mount.is_some() {
                continue;
            }
            let Some(visual) = self.saber_visual(u.team, u.spec.class) else {
                continue;
            };
            let flying = self.flying_transform(walk, i).is_some();
            for (bone, hilt) in &visual.hilts {
                if flying && bone.eq_ignore_ascii_case("hp_weapons") {
                    continue;
                }
                let Some(node) = asset.asset.skeleton.index_of(bone) else {
                    continue;
                };
                hilt.part_draws(
                    model * palette[node] * asset.asset.skeleton.global_bind[node],
                    &mut out,
                );
            }
        }
        // The native ordnance draws its primary weapon independently of the owner body.
        for (i, u) in walk.sim.units.iter().enumerate() {
            let Some(at) = self.flying_transform(walk, i) else {
                continue;
            };
            let Some(visual) = self.saber_visual(u.team, u.spec.class) else {
                continue;
            };
            for (bone, hilt) in &visual.hilts {
                if bone.eq_ignore_ascii_case("hp_weapons") {
                    hilt.part_draws(at, &mut out);
                }
            }
        }
        out
    }

    /// B2 00344a50 uses the shared capped ribbon, white before GS halving,
    /// with authored half-widths and one sheet for the core and halo.
    pub fn saber_draws(&self, walk: &WalkSim) -> Vec<ParticleDraw> {
        let mut out = Vec::new();
        for (i, asset, model, palette) in self.posed(walk) {
            let u = &walk.sim.units[i];
            if !u.alive || u.mount.is_some() {
                continue;
            }
            let Some(visual) = self.saber_visual(u.team, u.spec.class) else {
                continue;
            };
            let hidden = u.weapon.melee.as_ref().map_or(0, |s| s.hidden_edges);
            let flying = self.flying_transform(walk, i).is_some();
            for (edge, binding) in visual.bindings.iter().enumerate() {
                if !core::hero_weapon::blade_visible(hidden, edge)
                    || (flying && binding.blade.bone.eq_ignore_ascii_case("hp_weapons"))
                {
                    continue;
                }
                let Some(bone) = asset.asset.skeleton.index_of(&binding.blade.bone) else {
                    continue;
                };
                let at = model * palette[bone] * asset.asset.skeleton.global_bind[bone];
                edge_draws(visual, edge, binding, at, &mut out);
            }
        }
        for (i, u) in walk.sim.units.iter().enumerate() {
            let Some(at) = self.flying_transform(walk, i) else {
                continue;
            };
            let Some(visual) = self.saber_visual(u.team, u.spec.class) else {
                continue;
            };
            for (edge, binding) in visual.bindings.iter().enumerate() {
                if binding.blade.bone.eq_ignore_ascii_case("hp_weapons") {
                    edge_draws(visual, edge, binding, at, &mut out);
                }
            }
        }
        out.extend(self.saber_trail_draws(walk));
        out
    }
}

#[cfg(test)]
mod tests;
