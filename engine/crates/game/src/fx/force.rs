//! Immediate BF2 emitter effects and the ordinary damage-effect timer.
//!
//! B2 0x315f78 supplies chain endpoints; 0x459898/0x459c50 supply the
//! lightning envelope, seeded perpendicular jitter, taper and branches.
//! Cosmetic seeds are our own stream and never consume gameplay randoms.

use super::{
    emitter::{Frame, Lcg, SEED_XOR},
    Fx, Library,
};
use crate::classdb::{fnv1a_lowercase as hash, ClassDb};
use core::{
    sim::{Event, Sim},
    unit::UnitHandle,
};
use glam::{Mat3, Vec3};
use manifest::FxLightning;
use render::{Camera, ParticleDraw, ParticleShape, TextureHandle};

mod lightning;
use lightning::{byte, strip_draws, Seeds, Strip};

#[derive(Clone, Debug, Default)]
struct Look {
    emission: Option<String>,
    lightning: Option<String>,
    intensity: f32,
    minimum_bolts: usize,
    bolt_spread: f32,
}

impl Look {
    fn from_props<'a>(props: impl Iterator<Item = &'a (u32, String)>) -> Self {
        let mut out = Self {
            intensity: 1.0,
            minimum_bolts: 1,
            ..Self::default()
        };
        for (key, value) in props {
            match *key {
                0xc0e2_0834 => out.emission = Some(value.clone()),
                0x6dd5_a894 => out.lightning = Some(value.clone()),
                0x38ff_e98e => {
                    if let Ok(v) = value.parse::<f32>() {
                        if v.is_finite() {
                            out.intensity = v;
                        }
                    }
                }
                0x6338_4b21 => {
                    if let Ok(v) = value.parse::<f32>() {
                        out.minimum_bolts = (v as usize).min(64);
                    }
                }
                0xce53_2e64 => {
                    if let Ok(v) = value.parse::<f32>() {
                        if v.is_finite() {
                            out.bolt_spread = v;
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
}

#[derive(Debug)]
struct Bolt {
    def: FxLightning,
    from: Vec3,
    to: Vec3,
    owner: Option<UnitHandle>,
    left: f32,
    strength: f32,
    phase: f32,
    seeds: [u32; 2],
}

#[derive(Default, Debug)]
pub(super) struct State {
    looks: Vec<Look>,
    bolts: Vec<Bolt>,
    tick: Option<u64>,
    rng: Option<Lcg>,
}

impl State {
    pub fn extend(&mut self, db: &ClassDb, sim: &Sim) {
        let mut added = Self::load(db, sim);
        self.looks.extend(added.looks.drain(self.looks.len()..));
    }

    pub fn load(db: &ClassDb, sim: &Sim) -> Self {
        Self {
            looks: sim
                .weapon_classes
                .iter()
                .map(|class| {
                    let source = crate::roster::ClassSrc {
                        db,
                        class: &class.name,
                    };
                    let ordnance = core::weapon::WeaponClass::ordnance_name(&source)
                        .unwrap_or_else(|| class.name.clone());
                    Look::from_props(
                        db.chain(db.resolve(&ordnance).unwrap_or(&ordnance))
                            .into_iter()
                            .rev()
                            .flat_map(|c| &c.props),
                    )
                })
                .collect(),
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        self.bolts.clear();
        self.tick = None;
    }

    fn bolt(
        &mut self,
        lib: &Library,
        name: &str,
        from: Vec3,
        to: Vec3,
        owner: Option<UnitHandle>,
        strength: f32,
    ) {
        let Some(def) = lib.lightning.get(&hash(name)) else {
            return;
        };
        if !from.is_finite() || !to.is_finite() || from == to {
            return;
        }
        let rng = self.rng.get_or_insert(Lcg(0x4c49_4748));
        let seeds = [rng.seed(), rng.seed()];
        self.bolts.push(Bolt {
            def: def.clone(),
            from,
            to,
            owner,
            left: def.fade_time.unwrap_or(1.0),
            strength,
            phase: 0.0,
            seeds,
        });
    }

    pub fn update(&mut self, dt: f32) {
        let rng = self.rng.get_or_insert(Lcg(0x4c49_4748));
        for bolt in &mut self.bolts {
            bolt.left -= dt;
            bolt.phase += bolt.def.interpolation_speed.unwrap_or(10.0) * dt;
            // Native 0x4594f0 advances at most one seed pair per update.
            if bolt.phase > 1.0 {
                bolt.phase -= 1.0;
                bolt.seeds[0] = bolt.seeds[1];
                bolt.seeds[1] = rng.seed();
            }
        }
        self.bolts.retain(|b| b.left > 0.0);
    }

    pub fn draws(
        &self,
        camera: &Camera,
        texture: impl FnMut(&str) -> Option<TextureHandle>,
    ) -> Vec<ParticleDraw> {
        let mut texture = texture;
        let mut out = Vec::new();
        for bolt in &self.bolts {
            let Some(sheet) = bolt.def.texture.as_deref().and_then(&mut texture) else {
                continue;
            };
            let fade = (2.0 * bolt.left / bolt.def.fade_time.unwrap_or(1.0)).min(1.0)
                * (bolt.strength * 0.5 + 0.5);
            let width = bolt.strength * 0.7 + 0.3;
            let mut color = bolt.def.color.unwrap_or([255.0; 4]).map(byte);
            color[3] = byte(f32::from(color[3]) * fade);
            let spec = Strip {
                width: bolt.def.width.unwrap_or(2.0) * width,
                spread: bolt.def.spread_factor.unwrap_or(1.0),
                phase: bolt.phase,
                break_distance: bolt.def.break_distance.unwrap_or(2.0),
                texture_size: bolt.def.texture_size.unwrap_or(5.0),
                branch_factor: bolt.def.branch_factor.unwrap_or(0.3),
                branch_spread: bolt.def.branch_spread_factor.unwrap_or(0.8),
                branch_length: bolt.def.branch_length.unwrap_or(4.0),
                max_branches: (bolt.def.max_branches.unwrap_or(5.0) as usize).min(10),
            };
            strip_draws(
                [bolt.from, bolt.to],
                bolt.seeds,
                spec,
                color,
                Some(sheet),
                camera,
                &mut out,
            );
            let count = bolt.def.num_children.unwrap_or(3.0) as usize;
            if count > 0 {
                let count = ((count as f32 * bolt.strength) as usize).clamp(1, 64);
                // [ours] Child seeds stay stable within the same drawn frame;
                // the original draws from its session RNG each render call.
                let mut seed = Lcg(bolt.seeds[0].wrapping_add(bolt.seeds[1]));
                let mut child_color = bolt.def.child_color.unwrap_or([255.0; 4]).map(byte);
                child_color[3] = byte(f32::from(child_color[3]) * fade);
                for _ in 0..count {
                    strip_draws(
                        [bolt.from, bolt.to],
                        [seed.seed(), seed.seed()],
                        Strip {
                            width: bolt.def.child_width.unwrap_or(0.3) * width,
                            spread: bolt.def.child_spread_factor.unwrap_or(1.0),
                            phase: 0.0,
                            break_distance: bolt.def.child_break_distance.unwrap_or(1.5),
                            texture_size: bolt.def.child_texture_size.unwrap_or(3.0),
                            ..spec
                        },
                        child_color,
                        Some(sheet),
                        camera,
                        &mut out,
                    );
                }
            }
        }
        out
    }
}

fn aim_frame(pos: Vec3, direction: Vec3, velocity: Vec3) -> Frame {
    let z = direction.try_normalize().unwrap_or(Vec3::Z);
    let x = Vec3::Y.cross(z).try_normalize().unwrap_or(Vec3::X);
    Frame {
        pos,
        rot: Mat3::from_cols(x, z.cross(x), z),
        vel: velocity,
    }
}

/// Visual-only spare bolts use the ordinary world/terrain/body ray. Their
/// endpoints cannot hit through a wall or feed back into emitter damage.
fn bolt_endpoint(sim: &Sim, owner: UnitHandle, from: Vec3, direction: Vec3, range: f32) -> Vec3 {
    use core::collision::{ray_heightfield, HullFilter, Layers};
    let mut length = range;
    let mount = sim.unit(owner).and_then(|u| u.mount).map(|m| m.vehicle);
    if let Some(hit) = sim.collision.ray_with(
        from,
        direction,
        length,
        Layers::ORDNANCE,
        HullFilter::Except([mount, None]),
    ) {
        length = hit.t;
    }
    if let Some(hf) = &sim.heightfield {
        if let Some(hit) =
            ray_heightfield(hf, &sim.collision.cutters, from, direction, length, 0.25)
        {
            length = hit;
        }
    }
    for (handle, u) in sim.alive_units() {
        if handle != owner {
            if let Some((hit, _)) = u.bolt_hit(from, direction, length) {
                length = hit;
            }
        }
    }
    from + direction * length
}

impl Fx {
    pub(super) fn follow_force(&mut self, sim: &Sim) {
        let mut state = std::mem::take(&mut self.force);
        // A first bolt follows its caster's weapon frame, later chain links
        // retain their actual previous hit origin (0x316d3c).
        for bolt in &mut state.bolts {
            if let Some(h) = bolt.owner {
                if let Some(u) = sim.unit(h).filter(|u| u.alive) {
                    bolt.from = u.muzzle();
                }
            }
        }
        self.force = state;
    }

    pub(super) fn ingest_force(&mut self, sim: &Sim) {
        if self.force.tick == Some(sim.tick) {
            return;
        }
        self.force.tick = Some(sim.tick);
        let mut state = std::mem::take(&mut self.force);
        for (event_index, event) in sim.events.iter().enumerate() {
            match event {
                Event::Ability {
                    unit,
                    weapon_class,
                    pos,
                    charge,
                } => {
                    let Some(look) = state.looks.get(*weapon_class as usize).cloned() else {
                        continue;
                    };
                    let Some(u) = sim.unit(*unit) else { continue };
                    if let Some(effect) = &look.emission {
                        self.spawn(effect, aim_frame(*pos, u.aim_frame().0, u.soldier.vel));
                    }
                    if let (Some(lib), Some(name), Some(power)) = (
                        self.library.as_ref(),
                        look.lightning.as_deref(),
                        sim.weapon_classes
                            .get(*weapon_class as usize)
                            .and_then(|w| w.force.as_ref()),
                    ) {
                        let contacts = sim.events[event_index + 1..]
                            .iter()
                            .take_while(|e| !matches!(e, Event::Ability { .. }))
                            .filter(|e| {
                                matches!(e,
                            Event::ForceHit { owner, weapon_class: c, .. }
                                if owner == unit && c == weapon_class)
                            })
                            .count();
                        for _ in contacts..look.minimum_bolts {
                            let direction = if look.bolt_spread == 0.0 {
                                u.aim_frame().0
                            } else {
                                let rng = state.rng.get_or_insert(Lcg(0x4c49_4748));
                                let (sa, ca) = core::fmath::sin_cos(
                                    (1.0 - 2.0 * rng.float()) * look.bolt_spread,
                                );
                                let (sb, cb) = core::fmath::sin_cos(
                                    (1.0 - 2.0 * rng.float()) * look.bolt_spread,
                                );
                                let d = u.aim_frame().0;
                                let x = d.x * ca - d.z * sa;
                                Vec3::new(x * cb + d.y * sb, d.y * cb - x * sb, d.x * sa + d.z * ca)
                            };
                            let end = bolt_endpoint(sim, *unit, *pos, direction, power.range);
                            state.bolt(lib, name, *pos, end, Some(*unit), *charge * look.intensity);
                        }
                    }
                }
                Event::ForceHit {
                    owner,
                    from,
                    pos,
                    weapon_class,
                    blocked,
                    target,
                    charge,
                    ..
                } => {
                    let Some(look) = state.looks.get(*weapon_class as usize).cloned() else {
                        continue;
                    };
                    if !*blocked {
                        if let core::projectile::Target::Unit(target) = target {
                            if sim
                                .unit(*target)
                                .and_then(|u| u.burn)
                                .is_some_and(|b| b.weapon_class == *weapon_class as usize)
                            {
                                for effect in &mut self.live {
                                    if effect.burn.is_some_and(|(h, _, _)| h == *target) {
                                        effect.stop();
                                        effect.burn = None;
                                    }
                                }
                            }
                        }
                    }
                    if let (Some(lib), Some(name)) =
                        (self.library.as_ref(), look.lightning.as_deref())
                    {
                        let to = if *blocked {
                            *pos - (*pos - *from).normalize_or_zero() * 0.75
                        } else {
                            *pos
                        };
                        let follow = sim
                            .unit(*owner)
                            .filter(|u| from.distance_squared(u.muzzle()) < 0.0001)
                            .map(|_| *owner);
                        state.bolt(lib, name, *from, to, follow, *charge * look.intensity);
                    }
                }
                _ => {}
            }
        }
        self.force = state;
        self.follow_force(sim);
    }

    /// Running immediate bolts, exposed for the deterministic gallery checks.
    pub fn force_bolts(&self) -> usize {
        self.force.bolts.len()
    }

    pub fn force_damage_effects(&self) -> usize {
        self.live.iter().filter(|e| e.burn.is_some()).count()
    }
}

#[cfg(test)]
mod tests;
