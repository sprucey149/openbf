//! A melee set's named offhand action over the ordinary movement pose.

use super::*;

impl Animator {
    pub fn offhand_aim(&mut self, aim: Option<(aim::AimKind, f32)>) {
        self.next_aim = aim;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn offhand_pose(
        &mut self,
        rig: &Rig,
        clips: &[Clip],
        skel: &Skeleton,
        class: &ClassAnim,
        weapon: &str,
        animation: &str,
        seconds: f32,
        legs: bool,
    ) -> bool {
        let (primary, fallback) = class.bank_pair_of(weapon);
        let banks: [&str; 2] = [primary, fallback.unwrap_or(primary)];
        let banks = if fallback.is_some() { &banks[..] } else { &banks[..1] };
        let upper =
            crate::hero_blades::clip_for_banks(clips, &class.family, banks, animation, "upper");
        let lower =
            crate::hero_blades::clip_for_banks(clips, &class.family, banks, animation, "lower");
        let Some(upper) = upper.or(lower) else {
            return false;
        };
        let mut pose = self.kept_pose(skel);
        for (clip, mask) in [
            (legs.then_some(lower).flatten(), &rig.lower),
            (Some(upper), &rig.upper),
        ] {
            if let Some(clip) = clip {
                let sample = clip.sample(skel, seconds * 30.0, false);
                for (i, kept) in pose.0.iter_mut().enumerate() {
                    if mask[i] {
                        *kept = sample.0[i];
                    }
                }
            }
        }
        self.kept = Some(pose);
        // Named offhand actions retain the ordinary leg twist and clip aim.
        self.combo_override = false;
        true
    }
}
