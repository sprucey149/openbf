//! Install service-droid collision and authored wreck models from map/common
//! resources (`analysis/re-powerup-station.md` §§14–15). The sim owns motion so
//! collision and rendering sample one deterministic pose.

use crate::assets::Scene;
use crate::classdb::ClassDb;
use anyhow::Result;
use core::sim::Sim;
use manifest::{CommonManifest, Manifest};
use std::collections::HashMap;
use std::path::Path;

pub(crate) fn is_destructible_building(base: &str) -> bool {
    base.eq_ignore_ascii_case("destructablebuilding")
}

/// A map's game's common bake: its folder and its manifest, `None` when it
/// is not baked.
pub type Common = (std::path::PathBuf, Option<CommonManifest>);

/// [`Common`] for map `m` in `map_dir`.
pub fn load_common(m: &Manifest, map_dir: &Path) -> Result<Common> {
    let root = map_dir.parent().unwrap_or(map_dir);
    let common_dir = root.join(format!("common_{}", m.source_game));
    let path = common_dir.join("common.ron");
    let common: Option<CommonManifest> = if pak::exists(&path) {
        Some(ron::from_str(&pak::read_to_string(&path)?)?)
    } else {
        None
    };
    Ok((common_dir, common))
}

/// The blast sphere of `class` drawn as map or common geometry `geometry`
/// (model space): the model's SPHR grown by the class's collision
/// primitives ([`crate::vehicle_class::blast_sphere`]); `None` when neither
/// the map nor its game's common bake has the model or it has no sphere.
pub fn geometry_sphere(
    m: &Manifest,
    map_dir: &Path,
    common: &Common,
    db: &ClassDb,
    geometry: &str,
    class: &str,
) -> Result<Option<[f32; 4]>> {
    let (common_dir, common) = (&common.0, &common.1);
    let entry = m
        .geometries
        .iter()
        .find(|g| {
            g.name.eq_ignore_ascii_case(geometry)
                && pak::exists(map_dir.join("models").join(format!("{}.glb", g.model)))
        })
        .map(|g| (map_dir.to_path_buf(), g.model.clone()))
        .or_else(|| {
            common
                .as_ref()?
                .geometries
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(geometry))
                .map(|g| (common_dir.to_path_buf(), g.model.clone()))
        });
    let Some((dir, model)) = entry else {
        return Ok(None);
    };
    let mesh = crate::assets::load_glb(&dir.join("models").join(format!("{model}.glb")))?;
    Ok(mesh.sphere.map(|s| {
        let ([x, y, z], r) = crate::vehicle_class::blast_sphere(db, class, Some(s));
        [x, y, z, r]
    }))
}

/// Bind placed stations to collision instances and load living/wreck spheres.
pub fn install_collision(sim: &mut Sim, m: &Manifest, map_dir: &Path, db: &ClassDb) -> Result<()> {
    if sim.stations.is_empty() {
        return Ok(());
    }
    let both = load_common(m, map_dir)?;
    let sphere =
        |geometry: &str, class: &str| geometry_sphere(m, map_dir, &both, db, geometry, class);
    let (common_dir, common) = (&both.0, &both.1);
    for c in &mut sim.station_classes {
        c.sphere = sphere(&c.geometry, &c.name)?;
        c.wreck_sphere = match &c.destroyed_geometry {
            Some(g) => sphere(g, &c.name)?,
            None => None,
        };
        if db
            .engine_base(&c.name)
            .is_some_and(is_destructible_building)
        {
            if let Some(class) = m
                .classes
                .iter()
                .find(|class| class.name.eq_ignore_ascii_case(&c.name))
            {
                c.damage_effects = crate::vehicle_class::damage_effects(db, &c.name)
                    .into_iter()
                    .filter_map(|effect| {
                        let local = match effect.attach.as_deref() {
                            Some(name) => {
                                let h = class
                                    .hardpoints
                                    .iter()
                                    .find(|h| h.name.eq_ignore_ascii_case(name))?;
                                crate::viewer::instance_matrix(
                                    h.axes.unwrap_or(glam::Mat3::IDENTITY.to_cols_array_2d()),
                                    h.pos,
                                )
                            }
                            None => glam::Mat4::IDENTITY,
                        };
                        Some(core::sim::StationDamageEffect {
                            band: core::autoturret::DamageEffect {
                                start: effect.start,
                                stop: effect.stop,
                                effect: effect.effect,
                            },
                            local,
                            scale: effect.scale,
                        })
                    })
                    .collect();
            }
        }
    }
    sim.station_bodies.clear();
    sim.station_live_collision.clear();
    for si in 0..sim.stations.len() {
        let s = &mut sim.stations[si];
        let c = &sim.station_classes[usize::from(s.class)];
        if let Some([x, y, z, r]) = c.sphere {
            s.sphere_centre = s.pose().transform_point3(glam::Vec3::new(x, y, z));
            s.sphere_radius = r;
        }
        sim.prepare_station_collision(si);
    }
    sim.station_wreck_collision = sim
        .station_classes
        .iter()
        .map(|c| {
            let Some(geometry) = &c.destroyed_geometry else {
                return Ok(Vec::new());
            };
            let entry = m
                .collision
                .iter()
                .find(|e| e.geometry.eq_ignore_ascii_case(geometry))
                .map(|e| (map_dir, e))
                .or_else(|| {
                    common
                        .as_ref()?
                        .collision
                        .iter()
                        .find(|e| e.geometry.eq_ignore_ascii_case(geometry))
                        .map(|e| (common_dir.as_path(), e))
                });
            let Some((dir, entry)) = entry else {
                return Ok(Vec::new());
            };
            let cg = crate::assets::load_collision_geom(dir, entry)?;
            let class = m
                .classes
                .iter()
                .chain(common.iter().flat_map(|m| &m.classes))
                .find(|k| k.name.eq_ignore_ascii_case(&c.name));
            Ok(crate::assets::hull_parts(
                &cg,
                class.map_or(&[], |c| c.collision.as_slice()),
                &[],
            ))
        })
        .collect::<Result<_>>()?;
    for si in 0..sim.stations.len() {
        let station = &sim.stations[si];
        if station.radius == 0.0 && !station.health.alive() {
            sim.install_station_wreck(si as u16);
        }
    }
    Ok(())
}

/// Load authored wreck meshes keyed by placed station instance.
pub fn wreck_models(scene: &Scene) -> Result<HashMap<usize, String>> {
    let mut out = HashMap::new();
    if !scene
        .manifest
        .classes
        .iter()
        .any(|c| c.base.eq_ignore_ascii_case("powerupstation") || is_destructible_building(&c.base))
    {
        return Ok(out);
    }
    let root = scene.dir.parent().unwrap_or(&scene.dir);
    let mut db = ClassDb::default();
    db.load_map(&scene.dir)?;
    for file in crate::roster::class_files(&scene.manifest.source_game, &[]) {
        let path = root.join("classes").join(file);
        if pak::exists(&path) {
            db.load(&path)?;
        }
    }
    for (i, inst) in scene.manifest.instances.iter().enumerate() {
        let base = db.engine_base(&inst.class);
        if base != Some("powerupstation") && !base.is_some_and(is_destructible_building) {
            continue;
        }
        if let Some(model) = db
            .prop(&inst.class, "DestroyedGeometryName")
            .and_then(|g| scene.model_for_geometry(g.trim()))
        {
            out.insert(i, model.to_owned());
        }
    }
    Ok(out)
}
