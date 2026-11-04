use crate::{assets::Scene, classdb::fnv1a_lowercase};
use anyhow::Result;
use glam::Vec3;

struct Volume {
    config: render::DustConfig,
    name: String,
    instances: Vec<glam::Mat4>,
    seed: u32,
}

pub(super) fn load(
    scene: &Scene,
    mut texture: impl FnMut(&str) -> Result<Option<render::TextureHandle>>,
) -> Result<Vec<render::Dust>> {
    let mut out = Vec::new();
    for volume in volumes(scene)? {
        if let Some(handle) = texture(&volume.name)? {
            out.push(render::Dust::new(
                volume.config,
                handle,
                volume.instances,
                volume.seed,
            ));
        }
    }
    Ok(out)
}

fn volumes(scene: &Scene) -> Result<Vec<Volume>> {
    if scene.manifest.source_game != "bf2" {
        return Ok(Vec::new());
    }
    let path = scene.dir.join(crate::classdb::MAP_CLASSES_FILE);
    if !pak::exists(&path) {
        return Ok(Vec::new());
    }
    let classes: Vec<manifest::ClassDef> = ron::from_str(&pak::read_to_string(&path)?)?;
    let mut out = Vec::new();
    for class in classes
        .iter()
        .filter(|c| c.base.eq_ignore_ascii_case("dusteffect"))
    {
        let instances: Vec<_> = scene
            .manifest
            .instances
            .iter()
            .filter(|i| i.class.eq_ignore_ascii_case(&class.name))
            .map(|i| super::instance_matrix(i.rotation, i.translation))
            .collect();
        if instances.is_empty() {
            continue;
        }
        let prop = |key: &str| {
            class
                .props
                .iter()
                .find(|(h, _)| *h == fnv1a_lowercase(key))
                .map(|(_, v)| v.as_str())
        };
        let number = |key: &str, default: f32| {
            prop(key)
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(default)
        };
        let vector = |key: &str, default: Vec3| {
            prop(key)
                .and_then(|v| {
                    let values = v
                        .split_whitespace()
                        .map(str::parse::<f32>)
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?;
                    (values.len() == 3 && values.iter().all(|v| v.is_finite()))
                        .then(|| Vec3::new(values[0], values[1], values[2]))
                })
                .unwrap_or(default)
        };
        let Some(name) = prop("Texture") else {
            continue;
        };
        if scene
            .textures
            .get(&name.to_ascii_lowercase())
            .is_some_and(|t| t.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255))
        {
            eprintln!(
                "dust texture {name}: opaque legacy sheet; rebake the map to restore its alpha"
            );
            continue;
        }
        let config = render::DustConfig {
            display_gain: super::look::atmosphere_display_gain(&scene.manifest),
            min_pos: vector("MinPos", Vec3::new(-20.0, -1.0, -20.0)),
            max_pos: vector("MaxPos", Vec3::new(20.0, 1.0, 20.0)),
            min_vel: vector("MinVel", Vec3::new(-3.0, -1.0, -3.0)),
            max_vel: vector("MaxVel", Vec3::new(3.0, 1.0, 3.0)),
            size: [number("MinSize", 10.0), number("MaxSize", 15.0)],
            lifetime: [number("MinLifeTime", 2.0), number("MaxLifeTime", 3.0)],
            color: [1.0, 1.0, 1.0, number("Alpha", 0.5)],
            count: number("NumParticles", 50.0).clamp(0.0, 4096.0) as usize,
            max_distance: number("MaxDistance", 500.0),
            height_scale: number("HeightScale", 0.2),
            radius_fade: [number("RadiusFadeMin", -1.0), number("RadiusFadeMax", -1.0)],
            camera_distance: number("CameraDistance", -1.0),
        };
        out.push(Volume {
            config,
            name: name.to_owned(),
            instances,
            seed: fnv1a_lowercase(&class.name),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn conquest_loads_the_placed_snow_and_mist_without_the_other_mode_layers() {
        let map = crate::assets::baked_root().join("bf2_hot1");
        if !map.join("manifest.ron").exists() {
            eprintln!("SKIP: BF2 Hoth bake absent");
            return;
        }
        let mut scene = crate::assets::load_battle_scene(&map, "con").unwrap().0;
        let volumes = super::volumes(&scene).unwrap();
        assert_eq!(
            volumes.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            ["whitedust", "hoth_mist"]
        );
        assert_eq!(volumes.len(), 2);
        assert_eq!(volumes[0].instances.len(), 52);
        assert_eq!(volumes[0].config.count, 80);
        assert_eq!(volumes[0].config.lifetime, [2.0, 3.0]);
        assert_eq!(volumes[1].instances.len(), 7);
        assert_eq!(volumes[1].config.count, 8);
        for pixel in scene
            .textures
            .get_mut("whitedust")
            .unwrap()
            .rgba
            .as_chunks_mut::<4>()
            .0
        {
            pixel[3] = 255;
        }
        let legacy = super::volumes(&scene).unwrap();
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].instances.len(), 7);
    }
}
