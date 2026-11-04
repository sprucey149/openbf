//! Skeleton + pose → joint palette. Format-independent: the LVL parsers fill a
//! `Skeleton` (bone names, parents, local bind transforms) and clips produce
//! per-bone local `Pose`s; `palette` turns a pose into the model-space joint
//! matrices the renderer's skinned path consumes (`joint_global * inverse_bind`).

use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub names: Vec<String>,
    /// Parent bone index; `None` for roots. Any order is accepted.
    pub parents: Vec<Option<usize>>,
    /// Bind transform of each bone in its parent's space.
    pub local_bind: Vec<Mat4>,
    /// Model-space bind transform of each bone.
    pub global_bind: Vec<Mat4>,
    /// Inverse of `global_bind` (model space → bone space).
    pub inverse_bind: Vec<Mat4>,
    /// The tentacle joints [`Skeleton::sway`] moves; empty on every other body.
    sway: Vec<SwayJoint>,
}

/// [ours] One tentacle joint: its chain and its place along it. No source has a
/// clip for Kit Fisto's tentacles, so they get a small presentation-only sway.
#[derive(Debug, Clone, Copy)]
struct SwayJoint {
    bone: usize,
    chain: u32,
    link: u32,
}

const SWAY_PREFIX: &str = "tentacle_bone_string_";
/// Radians at each link out from the head; three links to a chain.
const SWAY_REACH: [f32; 3] = [0.05, 0.09, 0.14];

impl Skeleton {
    pub fn new(names: Vec<String>, parents: Vec<Option<usize>>, local_bind: Vec<Mat4>) -> Self {
        assert_eq!(names.len(), parents.len());
        assert_eq!(names.len(), local_bind.len());
        let global_bind = globals(&parents, &local_bind);
        let inverse_bind = global_bind.iter().map(Mat4::inverse).collect();
        let sway = names
            .iter()
            .enumerate()
            .filter_map(|(bone, name)| {
                let number: u32 = name.strip_prefix(SWAY_PREFIX)?.parse().ok()?;
                let index = number.checked_sub(1)?;
                Some(SwayJoint {
                    bone,
                    chain: index / 3,
                    link: index % 3,
                })
            })
            .collect();
        Self {
            sway,
            names,
            parents,
            local_bind,
            global_bind,
            inverse_bind,
        }
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Lay the tentacles' sway over a pose, `t` seconds into this soldier's
    /// life. Two slow, unrelated waves per chain, later links trailing the first,
    /// so the chains drift apart instead of nodding together.
    pub fn sway(&self, pose: &mut Pose, t: f32) {
        for joint in &self.sway {
            let Some(bone) = pose.0.get_mut(joint.bone) else {
                continue;
            };
            let phase = joint.chain as f32 * 1.7 + joint.link as f32 * 0.6;
            let reach = SWAY_REACH[joint.link.min(2) as usize];
            let x = reach * (t * 3.1 + phase).sin();
            let y = reach * 0.6 * (t * 1.9 + phase * 1.3).sin();
            bone.rotation *= Quat::from_euler(glam::EulerRot::XYZ, x, y, 0.0);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n.eq_ignore_ascii_case(name))
    }

    /// Joint palette for a pose given as per-bone local transforms (parent
    /// space, same order as the skeleton). Bones the pose omits keep their
    /// bind transform.
    pub fn palette(&self, local_pose: &[Mat4]) -> Vec<Mat4> {
        let locals: Vec<Mat4> = (0..self.len())
            .map(|i| local_pose.get(i).copied().unwrap_or(self.local_bind[i]))
            .collect();
        let g = globals(&self.parents, &locals);
        g.iter()
            .zip(&self.inverse_bind)
            .map(|(gp, ib)| *gp * *ib)
            .collect()
    }
}

/// Model-space transforms from per-bone parent-space transforms, resolving
/// parents in any order (no assumption that parents precede children).
fn globals(parents: &[Option<usize>], locals: &[Mat4]) -> Vec<Mat4> {
    let n = locals.len();
    let mut out: Vec<Option<Mat4>> = vec![None; n];
    fn resolve(
        i: usize,
        parents: &[Option<usize>],
        locals: &[Mat4],
        out: &mut [Option<Mat4>],
        depth: usize,
    ) -> Mat4 {
        if let Some(m) = out[i] {
            return m;
        }
        assert!(
            depth < parents.len() + 1,
            "skeleton parent cycle at bone {i}"
        );
        let m = match parents[i] {
            Some(p) => resolve(p, parents, locals, out, depth + 1) * locals[i],
            None => locals[i],
        };
        out[i] = Some(m);
        m
    }
    (0..n)
        .map(|i| resolve(i, parents, locals, &mut out, 0))
        .collect()
}

/// One bone's local transform in a sampled clip frame (parent space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneXform {
    pub rotation: Quat,
    pub translation: Vec3,
}

impl BoneXform {
    pub const IDENTITY: Self = Self {
        rotation: Quat::IDENTITY,
        translation: Vec3::ZERO,
    };

    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation)
    }

    /// Blend towards `other` (slerp rotation, lerp translation); `t` in 0..1.
    pub fn blend(self, other: Self, t: f32) -> Self {
        Self {
            rotation: self.rotation.slerp(other.rotation, t),
            translation: self.translation.lerp(other.translation, t),
        }
    }
}

/// A sampled pose: one local transform per skeleton bone.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose(pub Vec<BoneXform>);

impl Pose {
    pub fn bind(skel: &Skeleton) -> Self {
        Self(
            skel.local_bind
                .iter()
                .map(|m| {
                    let (_, r, t) = m.to_scale_rotation_translation();
                    BoneXform {
                        rotation: r,
                        translation: t,
                    }
                })
                .collect(),
        )
    }

    pub fn blend(&self, other: &Pose, t: f32) -> Pose {
        Pose(
            self.0
                .iter()
                .zip(&other.0)
                .map(|(a, b)| a.blend(*b, t))
                .collect(),
        )
    }

    pub fn locals(&self) -> Vec<Mat4> {
        self.0.iter().map(|b| b.to_mat4()).collect()
    }
}

/// A clip: per-skeleton-bone tracks of per-frame local transforms (bones
/// without a track hold their bind transform).
#[derive(Debug, Clone)]
pub struct Clip {
    pub name: String,
    pub fps: f32,
    pub frames: usize,
    pub tracks: Vec<Option<Vec<BoneXform>>>,
}

impl Clip {
    /// Copy one baked frame exactly, clamped to the clip. Simulation setup
    /// uses this path so integer samples never pass through render slerp.
    pub fn sample_frame(&self, skel: &Skeleton, frame: usize) -> Pose {
        let bind = Pose::bind(skel);
        if self.frames == 0 {
            return bind;
        }
        let frame = frame.min(self.frames - 1);
        Pose(
            (0..skel.len())
                .map(|b| match self.tracks.get(b).and_then(Option::as_ref) {
                    Some(keys) if keys.len() == self.frames => keys[frame],
                    _ => bind.0[b],
                })
                .collect(),
        )
    }

    /// Duration in seconds by the BF1 rule (frames/fps); BF2 uses (frames−1)/fps.
    pub fn duration(&self) -> f32 {
        self.frames as f32 / self.fps
    }

    /// Sample at a fractional frame (linear blend between neighbouring
    /// frames; wraps when `looping`, clamps otherwise).
    pub fn sample(&self, skel: &Skeleton, frame: f32, looping: bool) -> Pose {
        let bind = Pose::bind(skel);
        if self.frames == 0 {
            return bind;
        }
        let n = self.frames as f32;
        let f = if looping {
            let wrapped = frame.rem_euclid(n);
            // Tiny negative inputs can round the remainder up to the clip length.
            if wrapped == n {
                0.0
            } else {
                wrapped
            }
        } else {
            frame.clamp(0.0, n - 1.0)
        };
        let i0 = f.floor() as usize;
        let t = f - i0 as f32;
        let i1 = if looping {
            (i0 + 1) % self.frames
        } else {
            (i0 + 1).min(self.frames - 1)
        };
        Pose(
            (0..skel.len())
                .map(|b| match self.tracks.get(b).and_then(|t| t.as_ref()) {
                    Some(keys) if keys.len() == self.frames => keys[i0].blend(keys[i1], t),
                    _ => bind.0[b],
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_bodys_tentacle_joints_sway_and_their_tips_most() {
        let names = [
            "bone_head",
            "tentacle_bone_string_1",
            "tentacle_bone_string_3",
        ];
        let skel = Skeleton::new(
            names.iter().map(|n| (*n).to_owned()).collect(),
            vec![None, Some(0), Some(1)],
            vec![Mat4::IDENTITY; 3],
        );
        let angle = |pose: &Pose, i: usize| pose.0[i].rotation.to_axis_angle().1;
        let mut peak = [0.0f32; 3];
        for frame in 0..200 {
            let mut pose = Pose::bind(&skel);
            skel.sway(&mut pose, frame as f32 * 0.05);
            for (bone, peak) in peak.iter_mut().enumerate() {
                *peak = peak.max(angle(&pose, bone));
            }
        }
        assert_eq!(peak[0], 0.0, "the head is the clip's");
        assert!(peak[1] > 0.0 && peak[2] > peak[1], "{peak:?}");
        assert!(peak[2] < 0.3, "a small sway: {peak:?}");

        let retail = Skeleton::new(
            vec!["bone_head".into(), "bone_string_1".into()],
            vec![None, Some(0)],
            vec![Mat4::IDENTITY; 2],
        );
        let mut pose = Pose::bind(&retail);
        retail.sway(&mut pose, 1.0);
        assert!(pose.0.iter().all(|b| b.rotation == Quat::IDENTITY));
    }

    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn two_bone_chain() -> Skeleton {
        // bone0 at the origin, bone1 one unit up its parent's y.
        Skeleton::new(
            vec!["bone_root".into(), "bone_tip".into()],
            vec![None, Some(0)],
            vec![Mat4::IDENTITY, Mat4::from_translation(Vec3::Y)],
        )
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn bind_pose_palette_is_identity() {
        let s = two_bone_chain();
        assert!(close(
            s.global_bind[1].transform_point3(Vec3::ZERO),
            Vec3::Y
        ));
        for m in s.palette(&s.local_bind) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-6));
        }
    }

    #[test]
    fn rotating_the_root_swings_the_tip_and_its_skinned_vertices() {
        let s = two_bone_chain();
        let pose = [Mat4::from_rotation_z(FRAC_PI_2), s.local_bind[1]];
        let pal = s.palette(&pose);
        // The tip bone's origin (bind: (0,1,0)) ends at (-1,0,0).
        assert!(close(pal[1].transform_point3(Vec3::Y), Vec3::NEG_X));
        // A vertex bound to the tip at (0,1.5,0) follows: (-1.5,0,0).
        assert!(close(
            pal[1].transform_point3(Vec3::new(0.0, 1.5, 0.0)),
            Vec3::new(-1.5, 0.0, 0.0)
        ));
        // Root-bound vertices just rotate.
        assert!(close(pal[0].transform_point3(Vec3::X), Vec3::Y));
    }

    #[test]
    fn parents_may_follow_children_in_bone_order() {
        // Same chain, listed tip-first.
        let s = Skeleton::new(
            vec!["bone_tip".into(), "bone_root".into()],
            vec![Some(1), None],
            vec![
                Mat4::from_translation(Vec3::Y),
                Mat4::from_translation(Vec3::X),
            ],
        );
        assert!(close(
            s.global_bind[0].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 1.0, 0.0)
        ));
        assert_eq!(s.index_of("BONE_ROOT"), Some(1));
    }

    #[test]
    fn pose_blend_halfway() {
        let s = two_bone_chain();
        let a = Pose::bind(&s);
        let mut b = a.clone();
        b.0[0].translation = Vec3::new(2.0, 0.0, 0.0);
        let h = a.blend(&b, 0.5);
        assert!(close(h.0[0].translation, Vec3::X));
        assert_eq!(h.0[1], a.0[1]);
    }

    #[test]
    fn clip_sampling_blends_wraps_and_clamps() {
        let s = two_bone_chain();
        let key = |x: f32| BoneXform {
            rotation: Quat::IDENTITY,
            translation: Vec3::new(x, 0.0, 0.0),
        };
        let clip = Clip {
            name: "t".into(),
            fps: 30.0,
            frames: 3,
            tracks: vec![Some(vec![key(0.0), key(1.0), key(2.0)]), None],
        };
        assert!(close(
            clip.sample(&s, 0.5, false).0[0].translation,
            Vec3::new(0.5, 0.0, 0.0)
        ));
        // Untracked bone keeps its bind transform.
        assert!(close(clip.sample(&s, 1.0, false).0[1].translation, Vec3::Y));
        // Clamped past the end; wrapped 2.5 -> between frame 2 and frame 0.
        assert!(close(
            clip.sample(&s, 7.0, false).0[0].translation,
            Vec3::new(2.0, 0.0, 0.0)
        ));
        for frame in [2.5, -0.5] {
            assert!(close(
                clip.sample(&s, frame, true).0[0].translation,
                Vec3::new(1.0, 0.0, 0.0)
            ));
        }
        // A very small negative offset rounds the loop remainder to 3.0.
        assert_eq!(
            clip.sample(&s, -f32::EPSILON / 2.0, true).0[0].translation,
            Vec3::ZERO
        );
        assert!((clip.duration() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn integer_frame_sampling_preserves_stored_key_bits_without_normalizing() {
        let skel = two_bone_chain();
        let key = BoneXform {
            rotation: Quat::from_xyzw(1.5, 0.0, 0.0, 0.0),
            translation: Vec3::new(f32::from_bits(0x3f800001), -0.0, -7.25),
        };
        let clip = Clip {
            name: "raw".into(),
            fps: 30.0,
            frames: 2,
            tracks: vec![Some(vec![BoneXform::IDENTITY, key]), None],
        };
        for frame in [1, usize::MAX] {
            let sampled = clip.sample_frame(&skel, frame);
            assert_eq!(
                sampled.0[0].rotation.to_array().map(f32::to_bits),
                key.rotation.to_array().map(f32::to_bits)
            );
            assert_eq!(
                sampled.0[0].translation.to_array().map(f32::to_bits),
                key.translation.to_array().map(f32::to_bits)
            );
            assert_eq!(sampled.0[1], Pose::bind(&skel).0[1]);
        }
    }
}
