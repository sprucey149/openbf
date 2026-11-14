//! Portable pose sampling for collision/setup inputs; drawing keeps its own sampler.

use crate::skin::{BoneXform, Clip, Pose, Skeleton};
use core::fmath;
use core::fmath::FloatExt;
use glam::Quat;

// Native shortest-path slerp and its near-linear/normalisation thresholds
// (B2 53a3e8). Transcendentals use the sim's portable implementation.
pub(crate) fn quat_slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let dot = a.dot(b);
    let cosine = dot.abs().fmin(1.0);
    let (wa, mut wb) = if 1.0 - cosine < 0.0001 {
        (1.0 - t, t)
    } else {
        let angle = fmath::atan2((1.0 - cosine * cosine).sqrt(), cosine);
        let denominator = fmath::sin_cos(angle).0;
        (
            fmath::sin_cos((1.0 - t) * angle).0 / denominator,
            fmath::sin_cos(t * angle).0 / denominator,
        )
    };
    if dot < 0.0 {
        wb = -wb;
    }
    let mut q = a * wa + b * wb;
    if (q.length_squared() - 1.0).abs() > 0.0001 {
        q = q.normalize();
    }
    q
}

pub(crate) fn blend(a: BoneXform, b: BoneXform, t: f32) -> BoneXform {
    BoneXform {
        rotation: quat_slerp(a.rotation, b.rotation, t),
        translation: a.translation.lerp(b.translation, t),
    }
}

pub(crate) fn sample(clip: &Clip, skel: &Skeleton, frame: f32) -> Pose {
    let frame = frame.clamp(0.0, clip.frames.saturating_sub(1) as f32);
    let index = frame as usize;
    let a = clip.sample_frame(skel, index);
    let fraction = frame - index as f32;
    if fraction == 0.0 {
        return a;
    }
    let b = clip.sample_frame(skel, index + 1);
    Pose(
        a.0.into_iter()
            .zip(b.0)
            .map(|(a, b)| blend(a, b, fraction))
            .collect(),
    )
}
