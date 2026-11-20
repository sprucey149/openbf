use crate::classdb::ClassDb;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use core::droideka::{ClipInfo, DroidekaClass, Sphere, CLIP_FRAMES, STATES};
use core::weapon::floats;
use glam::Vec3;
use manifest::{ManifestDroideka, ManifestHardpoint, ManifestWalker, VehicleManifest};

pub const GUNS: [&str; 2] = ["hp_gun_1", "hp_gun_2"];
pub const ORDNANCE_TAG: &str = "-o-";
pub const NOT_ORDNANCE: [&str; 2] = ["p_rollsphere", "p_base"];

#[derive(Clone, Debug, PartialEq)]
pub struct DroidekaBake {
    pub record: ManifestDroideka,
    pub hardpoints: Vec<ManifestHardpoint>,
}

pub fn bakes_in(sides: &[(std::path::PathBuf, VehicleManifest)]) -> HashMap<String, DroidekaBake> {
    let mut out = HashMap::new();
    for (_, vm) in sides {
        for d in &vm.droidekas {
            let hardpoints = vm
                .geometries
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(&d.geometry))
                .map(|g| g.hardpoints.clone())
                .unwrap_or_default();
            out.entry(d.class.to_ascii_lowercase())
                .or_insert_with(|| DroidekaBake {
                    record: d.clone(),
                    hardpoints,
                });
        }
    }
    out
}

pub fn bakes(map_dir: &Path, game: &str) -> HashMap<String, DroidekaBake> {
    bakes_in(&crate::vehicle_pads::side_vehicle_files(map_dir, game))
}

pub fn model_point(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[0], p[1], p[2])
}

pub fn clips(record: &ManifestDroideka) -> [ClipInfo; STATES] {
    std::array::from_fn(|s| match record.clips.get(s).and_then(Option::as_ref) {
        Some(c) => ClipInfo {
            frames: c.frames,
            root_travel: match (c.root_z.first(), c.root_z.last()) {
                (Some(a), Some(b)) => (b - a).abs(),
                _ => 0.0,
            },
        },
        None => ClipInfo {
            frames: CLIP_FRAMES[s],
            root_travel: 0.0,
        },
    })
}

pub fn ordnance(record: &ManifestDroideka) -> Vec<Sphere> {
    let tagged = record.prims.iter().any(|p| p.name.contains(ORDNANCE_TAG));
    record
        .prims
        .iter()
        .filter(|p| p.radius > 0.0)
        .filter(|p| {
            if tagged {
                p.name.contains(ORDNANCE_TAG)
            } else {
                !NOT_ORDNANCE.iter().any(|n| p.name.eq_ignore_ascii_case(n))
            }
        })
        .map(|p| Sphere {
            center: model_point(p.pos),
            radius: p.radius,
        })
        .collect()
}

pub fn guns(hardpoints: &[ManifestHardpoint]) -> Vec<Vec3> {
    GUNS.iter()
        .filter_map(|n| hardpoints.iter().find(|h| h.name.eq_ignore_ascii_case(n)))
        .map(|h| model_point(h.pos))
        .collect()
}

pub fn apply(c: &DroidekaClass, bake: &DroidekaBake) -> DroidekaClass {
    let mut out = c.clone().with_clips(clips(&bake.record));
    out.walk_ordnance = ordnance(&bake.record);
    out.guns = guns(&bake.hardpoints);
    out
}

fn apply_bakes(classes: &mut [crate::roster::ClassRoster], bakes: &HashMap<String, DroidekaBake>) {
    for c in classes {
        let Some(deka) = c.droideka.as_deref() else {
            continue;
        };
        match bakes.get(&c.class.to_ascii_lowercase()) {
            Some(b) => c.droideka = Some(Arc::new(apply(deka, b))),
            None => eprintln!("droideka {}: no side bake; class defaults kept", c.class),
        }
    }
}

pub fn install_classes(classes: &mut [crate::roster::ClassRoster], map_dir: &Path, game: &str) {
    if classes.iter().all(|c| c.droideka.is_none()) {
        return;
    }
    let bakes = bakes(map_dir, &game.to_ascii_lowercase());
    apply_bakes(classes, &bakes);
}

pub fn install(rosters: &mut [crate::roster::FactionRoster; 2], map_dir: &Path, game: &str) {
    let bakes = bakes(map_dir, &game.to_ascii_lowercase());
    for r in rosters {
        apply_bakes(&mut r.classes, &bakes);
    }
}

pub fn camera_sections(db: &ClassDb, class: &str) -> Vec<(u32, core::unit::CameraSection)> {
    let key = crate::classdb::fnv1a_lowercase;
    let (section, eye, track, tilt) = (
        key("CameraSection"),
        key("EyePointOffset"),
        key("TrackOffset"),
        key("TiltValue"),
    );
    let mut out: Vec<(u32, core::unit::CameraSection)> = Vec::new();
    let Some(def) = db.get(class) else {
        return out;
    };
    for (k, v) in &def.props {
        if *k == section {
            out.push((
                key(v.trim()),
                core::unit::CameraSection {
                    eye_y: 0.0,
                    offset: Vec3::ZERO,
                    tilt: 0.0,
                },
            ));
            continue;
        }
        let Some((_, s)) = out.last_mut() else {
            continue;
        };
        if *k == eye {
            if let Some([_, y, _]) = floats::<3>(v) {
                s.eye_y = y;
            }
        } else if *k == track {
            if let Some(offset) = floats::<3>(v) {
                s.offset = Vec3::from_array(offset);
            }
        } else if *k == tilt {
            if let Some([t]) = floats::<1>(v) {
                s.tilt = t.to_radians();
            }
        }
    }
    out
}

/// The class's shield weapon: the slot whose class chain ends at `shield`.
pub fn shield_weapon(db: &ClassDb, class: &str) -> Option<String> {
    db.weapon_slots(class)
        .into_iter()
        .filter_map(|s| db.resolve(&s.weapon).map(str::to_owned))
        .find(|w| {
            db.engine_base(w)
                .is_some_and(|b| b.eq_ignore_ascii_case("shield"))
        })
}

pub fn droideka_class(
    native: &ClassDb,
    play: &ClassDb,
    class: &str,
) -> core::droideka::DroidekaClass {
    let shield = shield_weapon(play, class);
    let src = crate::roster::ClassSrc { db: play, class };
    let shield_src = shield
        .as_deref()
        .map(|w| crate::roster::ClassSrc { db: play, class: w });
    let mut c = core::droideka::DroidekaClass::from_props(
        &src,
        shield_src
            .as_ref()
            .map(|s| s as &dyn core::weapon::PropSource),
    );
    if let Some(w) = shield_weapon(native, class) {
        let sounds = core::droideka::ShieldClass::from_props(&crate::roster::ClassSrc {
            db: native,
            class: &w,
        });
        c.shield.sound = sounds.sound;
        c.shield.off_sound = sounds.off_sound;
    }
    c.cameras = camera_sections(native, class);
    c.explosion = native
        .prop(class, "ExplosionName")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .and_then(|n| crate::roster::explosion_class(native, n));
    c
}

pub fn shield_gain(points: f32, max: f32) -> f32 {
    let lost = 1.0
        - if max > 0.0 {
            (points / max).clamp(0.0, 1.0)
        } else {
            0.0
        };
    1.0 - lost * lost
}

pub fn shield_loops(sim: &core::sim::Sim) -> Vec<(core::unit::UnitHandle, &str, f32, Vec3)> {
    sim.alive_units()
        .filter_map(|(h, u)| {
            let (d, c) = (u.droideka.as_deref()?, u.droideka_class()?);
            let name = c.shield.sound.as_deref()?;
            let bubble = u.droideka_shield()?;
            Some((
                h,
                name,
                shield_gain(d.shield.points, c.shield.max),
                bubble.center,
            ))
        })
        .collect()
}

pub fn as_walker(d: &ManifestDroideka) -> ManifestWalker {
    ManifestWalker {
        bank: d.bank.clone(),
        clips: d.clips.clone(),
        prims: d.prims.clone(),
        skin: d.skin.clone(),
        layer_bones: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
