//! BF2's animated props into the sim (`core::animated_prop`): every placed
//! instance whose class has a rig (`manifest::animated_prop`), and every
//! rigged prop a placed class carries on a hardpoint (Jabba's Palace's
//! rancor in its pit, Jabba on his bed), in instance order, a carrier's own
//! rig before what it carries.

use core::animated_prop::{AnimatedProp, Body, Catch, Setup, Trigger};
use glam::{Mat4, Quat, Vec3};
use manifest::animated_prop::{AnimatedProps, AnimatedRig};

/// A prim's world pose at every frame of every clip: `instance · B(f) ·
/// XFRM` with `B` its bone's model-space transform.
pub fn body_frames(
    instance: Mat4,
    frames: &[Vec<([f32; 4], [f32; 3])>],
    local: Mat4,
) -> Vec<Vec<(Quat, Vec3)>> {
    frames
        .iter()
        .map(|clip| {
            clip.iter()
                .map(|(q, t)| {
                    let b = Mat4::from_rotation_translation(Quat::from_array(*q), Vec3::from(*t));
                    let (_, r, t) = (instance * b * local).to_scale_rotation_translation();
                    (r, t)
                })
                .collect()
        })
        .collect()
}

/// One prop's setup at world matrix `m`.
pub fn setup(
    rig: &AnimatedRig,
    m: Mat4,
    instance: usize,
    bodies: Vec<Body>,
    placed: bool,
) -> Setup {
    Setup {
        clips: rig.clips.iter().map(|c| c.frames).collect(),
        triggers: rig
            .triggers
            .iter()
            .map(|t| Trigger {
                clip: t.clip,
                center: m.transform_point3(Vec3::from(t.position)),
                radius: t.radius,
            })
            .collect(),
        catches: rig
            .attaches
            .iter()
            .map(|a| Catch {
                clip: a.clip,
                radius: a.radius,
                time: a.time,
                track: a
                    .track
                    .iter()
                    .map(|clip| {
                        clip.iter()
                            .map(|p| m.transform_point3(Vec3::from(*p)))
                            .collect()
                    })
                    .collect(),
            })
            .collect(),
        bodies,
        idle_delay: rig.idle_delay,
        origin: m.w_axis.truncate(),
        kill_sound: rig.kill_sound.clone(),
        instance,
        class: rig.class.clone(),
        model: m,
        placed,
    }
}

/// Every animated prop of `scene` (bar the instances named in `skip`),
/// with the bodies `collision_parts` built for each instance's own rig.
pub fn build(
    scene: &crate::assets::Scene,
    animated: &AnimatedProps,
    mut bodies: Vec<(usize, Body)>,
    skip: &[String],
) -> Vec<AnimatedProp> {
    let mut out = Vec::new();
    if animated.is_empty() {
        return out;
    }
    for (ii, inst) in scene.manifest.instances.iter().enumerate() {
        if skip.iter().any(|n| n.eq_ignore_ascii_case(&inst.name)) {
            continue;
        }
        let m = crate::viewer::instance_matrix(inst.rotation, inst.translation);
        if let Some(rig) = animated.rig(&inst.class) {
            let own: Vec<Body> = bodies
                .iter()
                .filter(|(i, _)| *i == ii)
                .map(|(_, b)| b.clone())
                .collect();
            bodies.retain(|(i, _)| *i != ii);
            let k = out.len();
            out.push(AnimatedProp::new(setup(rig, m, ii, own, true), k));
        }
        for a in animated
            .attached
            .iter()
            .filter(|a| a.parent.eq_ignore_ascii_case(&inst.class))
        {
            let Some(rig) = animated.rig(&a.class) else {
                continue;
            };
            let child = m * crate::viewer::instance_matrix(a.rotation, a.translation);
            let k = out.len();
            out.push(AnimatedProp::new(
                setup(rig, child, ii, Vec::new(), false),
                k,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-gated on the shared bake: BF2 Jabba's Palace builds its rancor
    /// in the pit, Jabba on his bed, the torture droid and the trapdoor,
    /// whose two leaves cover the hole in front of Jabba until it opens.
    #[test]
    fn tat3_builds_its_animated_props() {
        let dir = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets_baked/bf2_tat3"
        ));
        let dir = std::env::var("OPENBF_TEST_TAT3")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| dir.to_path_buf());
        if !dir.join(manifest::animated_prop::FILE).exists() {
            eprintln!("SKIP: {} has no animated props", dir.display());
            return;
        }
        let scene = crate::assets::load_scene(&dir).unwrap();
        let (mut world, _, _, props) = scene.collision_parts_for_test();
        let classes: Vec<&str> = props.iter().map(|p| p.setup.class.as_str()).collect();
        for c in [
            "tat3_bldg_rancor",
            "tat3_bldg_jabba",
            "tat3_prop_trapdoor",
            "tat3_bldg_torturedroid",
        ] {
            assert!(classes.contains(&c), "{c} missing: {classes:?}");
        }
        let rancor = props
            .iter()
            .find(|p| p.setup.class == "tat3_bldg_rancor")
            .unwrap();
        // The pit's hp_rancor: (-48.73, -13.80, 125.57).
        let at = rancor.setup.origin;
        assert!(
            (at - Vec3::new(-48.73, -13.80, 125.57)).length() < 0.05,
            "{at}"
        );
        // Its grab trigger stands 6.4 m in front of it (it faces -x), on
        // the pit floor.
        let t = rancor.setup.triggers[0].center;
        assert!(
            (t - Vec3::new(-55.12, -13.80, 125.55)).length() < 0.05,
            "{t}"
        );
        let trap = props
            .iter()
            .find(|p| p.setup.class == "tat3_prop_trapdoor")
            .unwrap();
        assert_eq!(trap.setup.bodies.len(), 2);
        // Shut, both leaves lie over the hole at the palace floor.
        for b in &trap.setup.bodies {
            let (_, p) = b.frames[0][0];
            assert!((p.x + 58.5).abs() < 1.6 && (p.z - 126.0).abs() < 2.1, "{p}");
            assert!((p.y - 0.1).abs() < 0.3, "{p}");
        }
        // Shut, a soldier stands on them; open, he drops into the pit.
        let centre = trap.setup.triggers[0].center;
        let top = centre + Vec3::Y;
        let shut = world.ray_down(top, 5.0).map(|(y, _)| y);
        assert!(
            shut.is_some_and(|y| (y - centre.y).abs() < 0.3),
            "{shut:?} at {centre}"
        );
        for b in &trap.setup.bodies {
            let open = b.frames[1][b.frames[1].len() / 2];
            world.move_body(
                b.body,
                glam::Mat4::from_rotation_translation(open.0, open.1),
            );
        }
        let open = world.ray_down(top, 40.0).map(|(y, _)| y);
        let ground = scene.heightfield().map(|h| h.height_at(centre.x, centre.z));
        eprintln!(
            "trapdoor at {centre}: shut floor {shut:?}, open floor {open:?}, terrain {ground:?}"
        );
        assert!(open.is_none_or(|y| y < centre.y - 5.0), "{open:?}");
    }
}
