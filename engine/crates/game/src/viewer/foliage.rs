//! The terrain foliage's MODEL instances as the scene draws them
//! (analysis/re-foliage.md §6, [O]): `bake::foliage` placed each one (x, z,
//! yaw, tint — the PS2's own floats) into `foliage.bin`; this stands it on
//! the terrain the viewer already loads and builds its matrix the way B1
//! `FUN_00347610` / B2 `FUN_0042d960` do:
//!
//! - y = the height query at (x, z), up = that triangle's normal
//!   ([`core::heightfield::Heightfield::normal_at`]), then with `Stiffness`
//!   k > 0 up = normal·(1−k) + (0,1,0)·k, normalised;
//! - a direction d = (sin yaw, 0, −cos yaw) (the ELF's table sin/cos,
//!   `FUN_004452c0` / `FUN_00445330`; which of the pair is sin is [H] — a
//!   random yaw hides it), right = normalize(up × d), forward = right × up;
//! - rows (right, up, forward) × the uniform `Scale`, at (x, y, z).
//!
//! The draw colour is the record's tint (255 = unity) and the draw fades
//! over [d, 1.45·d] of horizontal distance, with the entry's near fade
//! ([`render::Fade`]). Grass patches are `super::grass`'s; leaf patches are
//! `super::leaf`'s (card LEAF).

use crate::assets::Scene;
use glam::{Mat4, Vec3};
use manifest::foliage_bin::Record;
use manifest::{FoliageKind, ManifestFoliageMesh};

/// A model fades out between its distance d and `FADE_END × d` (B1
/// 332623–332667: skip beyond 1.45·d, alpha linear over [d, 1.45·d]).
pub const FADE_END: f32 = 1.45;

/// One foliage model to draw.
pub struct FoliageModel<'a> {
    pub model: &'a str,
    pub matrix: Mat4,
    pub color: [f32; 4],
    pub fade: Option<render::Fade>,
}

/// The records of the scene's `foliage.bin`, empty when the map has none or
/// the file does not read.
pub fn records(scene: &Scene) -> Vec<Record> {
    let Some(f) = &scene.manifest.foliage else {
        return Vec::new();
    };
    match pak::read(scene.dir.join(&f.instances)) {
        Ok(b) => manifest::foliage_bin::read(&b).unwrap_or_else(|| {
            eprintln!("foliage: {} is not a foliage.bin", f.instances);
            Vec::new()
        }),
        Err(e) => {
            eprintln!("foliage: {}: {e}", f.instances);
            Vec::new()
        }
    }
}

/// The matrix of a model instance standing on `ground` (see the module doc).
pub fn matrix(
    r: &Record,
    mesh: &ManifestFoliageMesh,
    ground: &core::heightfield::Heightfield,
) -> Mat4 {
    let y = ground.height_at(r.x, r.z);
    let mut up = Vec3::from(ground.normal_at(r.x, r.z));
    if mesh.stiffness > 0.0 {
        up = up * (1.0 - mesh.stiffness) + Vec3::Y * mesh.stiffness;
    }
    let up = up.normalize();
    let d = Vec3::new(r.yaw.sin(), 0.0, -r.yaw.cos());
    let right = up.cross(d).normalize();
    let fwd = right.cross(up);
    let s = mesh.scale;
    Mat4::from_cols(
        (right * s).extend(0.0),
        (up * s).extend(0.0),
        (fwd * s).extend(0.0),
        Vec3::new(r.x, y, r.z).extend(1.0),
    )
}

/// The fade of a model entry: out over [d, 1.45·d], plus its near fade;
/// none when d is 0 (the ELF skips the test then).
pub fn fade(mesh: &ManifestFoliageMesh) -> Option<render::Fade> {
    (mesh.distance > 0.0).then_some(render::Fade {
        begin: mesh.distance,
        end: mesh.distance * FADE_END,
        near: mesh.near_fade,
        opacity: 1.0,
    })
}

/// Every model instance of the scene's foliage that resolves to a loaded
/// mesh, standing on the scene's terrain. Empty without a terrain.
pub fn models<'a>(scene: &'a Scene, records: &[Record]) -> Vec<FoliageModel<'a>> {
    let (Some(f), Some(ground)) = (&scene.manifest.foliage, scene.render_heightfield()) else {
        return Vec::new();
    };
    records
        .iter()
        .filter_map(|r| {
            let mesh = f.meshes.get(r.mesh as usize)?;
            if mesh.kind != FoliageKind::Model {
                return None;
            }
            let (model, _) = scene.meshes.get_key_value(mesh.name.as_deref()?)?;
            Some(FoliageModel {
                model,
                matrix: matrix(r, mesh, &ground),
                color: r.rgba.map(|c| c as f32 / 255.0),
                fade: fade(mesh),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(stiffness: f32, scale: f32) -> ManifestFoliageMesh {
        ManifestFoliageMesh {
            layer: 0,
            kind: FoliageKind::Model,
            name: Some("m".into()),
            distance: 40.0,
            near_fade: 0.0,
            scale,
            stiffness,
            collision: false,
        }
    }

    /// A plane rising 1 m per metre of x: the model stands at its height,
    /// tilted to the plane's normal, or upright at stiffness 1; yaw 0 faces
    /// −z; the scale scales every axis.
    #[test]
    fn a_model_stands_on_the_terrain() {
        let ground = core::heightfield::Heightfield {
            size: 4,
            unit: 1.0,
            offset_x: 0.0,
            offset_z: 0.0,
            heights: (0..16).map(|i| (i % 4) as f32).collect(),
        };
        let r = Record {
            mesh: 0,
            x: 1.5,
            z: 1.25,
            yaw: 0.0,
            rgba: [255; 4],
        };
        let m = matrix(&r, &mesh(0.0, 1.0), &ground);
        assert!((m.w_axis.y - 1.5).abs() < 1e-5);
        let up = m.y_axis.truncate();
        let n = Vec3::new(-1.0, 1.0, 0.0).normalize();
        assert!((up - n).length() < 1e-5, "{up}");
        let upright = matrix(&r, &mesh(1.0, 2.0), &ground);
        assert!((upright.y_axis.truncate() - Vec3::Y * 2.0).length() < 1e-5);
        assert!((upright.z_axis.truncate() - Vec3::NEG_Z * 2.0).length() < 1e-5);
        // Right-handed, orthogonal.
        let (x, y, z) = (
            m.x_axis.truncate(),
            m.y_axis.truncate(),
            m.z_axis.truncate(),
        );
        assert!(x.dot(y).abs() < 1e-5 && y.dot(z).abs() < 1e-5 && x.cross(y).dot(z) > 0.99);
    }

    /// The model instances' height against the PS2's RAM: (x, y, z) of the
    /// first settled instances of each model mesh in BF1 END1
    /// `cp1_cp2_postA_spawn` and BF2 FEL1 `cp3_cp3_1_postA_spawn`
    /// (`lead_tmp/re/folg/live_instances.json`, the same capture
    /// `bake::foliage`'s tests pin). The PS2 stands a model at its height
    /// query (re-foliage §6); the port's baked terrain answers within
    /// `Y_TOLERANCE`. Data-gated on the baked maps.
    #[test]
    fn models_stand_at_the_ps2_s_height() {
        /// A RAM instance's (x, y, z).
        type Ram = (f32, f32, f32);
        const Y_TOLERANCE: f32 = 1e-4;
        #[rustfmt::skip]
        let cases: &[(&str, &[Ram])] = &[
            ("bf1_end1", &[
                (-249.56644, 0.16218832, 386.63098), (-248.85284, -3.128986, 414.2012),
                (-200.85284, -0.90433407, 446.2012), (-254.02887, 2.0474806, 357.23187),
                (-281.81104, 6.8464813, 381.1666), (-242.63098, -2.8679366, 397.75894),
                (-268.01602, 4.407327, 365.95032), (-167.79272, -3.3382947, 362.4953),
                (-204.7464, -3.1801739, 440.8528), (-265.12857, 3.9726477, 364.01602),
            ]),
            ("bf2_fel1", &[
                (-20.556602, 34.143017, -140.06412), (-31.136772, 32.969196, -123.12416),
                (-35.624245, 27.48525, -112.86323), (11.264533, 16.044113, -114.1304),
                (3.3103046, 18.862827, -117.723595), (29.483753, 16.126461, -125.280785),
                (17.332888, 17.052368, -120.58497),
            ]),
        ];
        for (map, pts) in cases {
            let dir = crate::assets::baked_root().join(map);
            if !dir.join("manifest.ron").exists() {
                eprintln!("SKIP: {map} not baked");
                continue;
            }
            let scene = crate::assets::load_scene(&dir).unwrap();
            let ground = scene.heightfield().unwrap();
            let worst = pts
                .iter()
                .map(|&(x, y, z)| (ground.height_at(x, z) - y).abs())
                .fold(0.0f32, f32::max);
            eprintln!(
                "{map}: worst |y − RAM y| = {worst} m over {} models",
                pts.len()
            );
            assert!(worst < Y_TOLERANCE, "{map}: {worst}");
        }
    }

    #[test]
    fn the_fade_runs_from_d_to_one_point_four_five_d() {
        let f = fade(&mesh(0.0, 1.0)).unwrap();
        assert_eq!((f.begin, f.end, f.near), (40.0, 58.0, 0.0));
        let mut none = mesh(0.0, 1.0);
        none.distance = 0.0;
        assert_eq!(fade(&none), None);
    }
}
