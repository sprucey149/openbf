//! Install service-droid and destructible-prop collision, authored wrecks and
//! prop damage effects from map/common resources (`analysis/re-powerup-station.md`
//! §§14–15 and `analysis/re-fx.md` §9). Collision and drawing use the sim's pose.

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

pub(crate) fn is_station_base(base: &str) -> bool {
    base.eq_ignore_ascii_case(crate::viewer::stations::STATION_BASE)
        || is_destructible_building(base)
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
        if let Some(name) = db.resolve(&c.name).filter(|name| {
            db.engine_base(name).is_some_and(|base| {
                is_destructible_building(base) || crate::vehicle_class::is_turret_base(base)
            })
        }) {
            if let Some(class) = m
                .classes
                .iter()
                .find(|class| class.name.eq_ignore_ascii_case(&c.name))
            {
                c.damage_effects = crate::vehicle_class::damage_effects(db, name)
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
                            inherit: effect.inherit_velocity,
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
        if !station.serves() && !station.health.alive() {
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
        .any(|c| is_station_base(&c.base))
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
        let Some(name) = db.resolve(&inst.class) else {
            continue;
        };
        if !db.engine_base(name).is_some_and(is_station_base) {
            continue;
        }
        if let Some(model) = db
            .prop(name, "DestroyedGeometryName")
            .and_then(|g| scene.model_for_geometry(g.trim()))
        {
            out.insert(i, model.to_owned());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn coruscant_library_stacks_resolve_their_mixed_case_wreck_class() {
        let map = crate::assets::baked_root().join("bf2_cor1");
        if !pak::exists(map.join("manifest.ron")) {
            eprintln!("SKIP: Coruscant bake absent");
            return;
        }
        let (scene, _) = crate::assets::load_battle_scene(&map, "con").unwrap();
        let wrecks = super::wreck_models(&scene).unwrap();
        let stacks: Vec<_> = scene
            .manifest
            .instances
            .iter()
            .enumerate()
            .filter(|(_, instance)| instance.class == "cor1_prop_librarystack")
            .collect();
        assert!(!stacks.is_empty());
        for (index, _) in stacks {
            assert_eq!(
                wrecks.get(&index).map(String::as_str),
                Some("cor1_prop_librarystack_dest"),
            );
        }
    }
}
