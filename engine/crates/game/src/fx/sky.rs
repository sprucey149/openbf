use super::{emitter::Frame, props::Attached, FxAssets, Library};
use core::fmath::FloatExt;
use glam::{Mat3, Mat4, Vec3};
use render::{Camera, ParticleDraw, ParticleShape};

pub(super) struct SkyEffect {
    attached: Attached,
    offset: f32,
    movement: f32,
    scale: f32,
    rotation: Option<[f32; 4]>,
}

pub(super) fn load(scene: &crate::assets::Scene, library: &Library) -> Vec<SkyEffect> {
    let Some(sky) = &scene.manifest.sky else {
        return Vec::new();
    };
    let scale = sky
        .models
        .iter()
        .find_map(|m| {
            let mesh = scene.meshes.get(&m.geometry)?;
            let radius = mesh
                .positions
                .iter()
                .map(|p| p[0].abs().fmax(p[2].abs()))
                .fold(0.0f32, core::fmath::max);
            (radius > 0.001).then_some(1000.0 / radius)
        })
        .unwrap_or(1.0);
    let mut out = Vec::new();
    for model in &sky.models {
        for effect in &model.effects {
            let Some(def) = library.get(&effect.effect) else {
                continue;
            };
            let lo = effect.parameter.unwrap_or(0.0).fmax(0.0);
            let hi = effect.max_delay.unwrap_or(lo).fmax(lo);
            let frame = Frame {
                pos: Vec3::from(effect.pos),
                rot: Mat3::from_cols_array_2d(&effect.axes),
                vel: Vec3::ZERO,
            };
            out.push(SkyEffect {
                attached: Attached::new(
                    def.clone(),
                    frame,
                    Some([lo, hi]),
                    crate::classdb::fnv1a_lowercase(&effect.hardpoint),
                ),
                offset: model.offset,
                movement: model.movement_scale,
                scale,
                rotation: model.rotation,
            });
        }
    }
    out
}

impl SkyEffect {
    pub(super) fn update(&mut self, dt: f32) {
        self.attached.step(dt);
    }

    pub(super) fn draws(
        &self,
        camera: &Camera,
        time: f64,
        zoom: f32,
        assets: &mut dyn FxAssets,
        out: &mut Vec<ParticleDraw>,
    ) {
        let Some(effect) = &self.attached.live else {
            return;
        };
        let rotation = self
            .rotation
            .and_then(|[speed, x, y, z]| render::sky_spin(speed, Vec3::new(x, y, z), time))
            .unwrap_or(Mat4::IDENTITY);
        let origin = camera.position
            + Vec3::Y * ((self.offset - camera.position.y) * (1.0 - self.movement) * self.scale);
        let matrix =
            Mat4::from_translation(origin) * rotation * Mat4::from_scale(Vec3::splat(self.scale));
        let inverse = matrix.inverse();
        let mut local = Vec::new();
        effect.draws_with(
            inverse.transform_point3(camera.position),
            inverse.transform_vector3(camera.forward()).normalize(),
            zoom,
            assets,
            &mut local,
        );
        for mut draw in local {
            draw.key = matrix.transform_point3(draw.key);
            match &mut draw.shape {
                ParticleShape::Sprite { pos, half, .. } => {
                    *pos = matrix.transform_point3(*pos);
                    *half *= self.scale;
                }
                ParticleShape::Quad { pos, a, b } | ParticleShape::CutoutQuad { pos, a, b } => {
                    *pos = matrix.transform_point3(*pos);
                    *a = matrix.transform_vector3(*a);
                    *b = matrix.transform_vector3(*b);
                }
                ParticleShape::Ribbon { points } => {
                    for (pos, half, _) in points {
                        *pos = matrix.transform_point3(*pos);
                        *half *= self.scale;
                    }
                }
                ParticleShape::Mesh { tris }
                | ParticleShape::SolidMesh { tris }
                | ParticleShape::CutoutMesh { tris } => {
                    for (pos, _) in tris {
                        *pos = matrix.transform_point3(*pos);
                    }
                }
                ParticleShape::Shaded { tris } => {
                    for (pos, _, _) in tris {
                        *pos = matrix.transform_point3(*pos);
                    }
                }
                _ => continue,
            }
            out.push(draw);
        }
    }
}
