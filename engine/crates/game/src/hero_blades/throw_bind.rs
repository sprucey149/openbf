//! Melee-throw construction samples ThrowAnimation at InitialSalvoDelay.
//! Its cached matrix follows the actor, without following later bone poses.

use super::*;
use crate::sim_pose::sample;
use core::hero_weapon::SaberThrowBinding;

pub(super) fn bindings(
    db: &ClassDb,
    soldier: &str,
    combo: &ComboDef,
    asset: &assets::SkinnedAsset,
    sets: &manifest::anim_sets::SoldierSets,
    family: &str,
    banks: &[&str],
) -> Result<Vec<SaberThrowBinding>> {
    let weapons: BTreeSet<_> = db
        .weapon_slots(soldier)
        .into_iter()
        .filter(|s| db.engine_base(&s.weapon) == Some("meleethrow"))
        .map(|s| s.weapon)
        .collect();
    if weapons.is_empty() {
        return Ok(Vec::new());
    }
    let hp = asset
        .skeleton
        .index_of("hp_weapons")
        .ok_or_else(|| anyhow!("{soldier}: throw class has no primary attachment"))?;
    let animation = combo
        .fields
        .iter()
        .filter(|f| f.keyword == K::ThrowAnimation)
        .filter_map(|f| f.text(0))
        .next_back()
        .ok_or_else(|| anyhow!("{soldier}: throw class has no ThrowAnimation"))?;
    let upper = clip_for_banks(&asset.clips, family, banks, animation, "upper");
    let lower = clip_for_banks(&asset.clips, family, banks, animation, "lower");
    let base = lower
        .or(upper)
        .ok_or_else(|| anyhow!("{soldier}: ThrowAnimation {animation} not baked"))?;
    weapons
        .into_iter()
        .map(|weapon| {
            // FUN00345c40 initialises zero; parser00345cf8 overrides this field.
            let seconds: f32 = scalar(db, &weapon, "InitialSalvoDelay")
                .unwrap_or("0")
                .parse()?;
            if !seconds.is_finite() || seconds < 0.0 {
                bail!("{weapon}: invalid constructor animation sample time");
            }
            let mut pose = sample(base, &asset.skeleton, seconds * 30.0);
            if let Some(upper) = upper {
                let up = sample(upper, &asset.skeleton, seconds * 30.0);
                for bone in &sets.upper_bones {
                    if let Some(i) = asset.skeleton.index_of(bone) {
                        pose.0[i] = up.0[i];
                    }
                }
            }
            let palette = asset.skeleton.palette(&pose.locals());
            let attachment = palette[hp] * asset.skeleton.global_bind[hp];
            if !attachment.is_finite() {
                bail!("{soldier}: nonfinite throw attachment");
            }
            Ok(SaberThrowBinding {
                weapon,
                animation: animation.into(),
                seconds,
                attachment,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sim_pose::blend, skin::BoneXform};
    use glam::Quat;

    #[test]
    fn fractional_constructor_sample_uses_shortest_path_and_normalises() {
        let a = BoneXform::IDENTITY;
        let b = BoneXform {
            rotation: -Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            translation: Vec3::X,
        };
        let sample = blend(a, b, 0.4);
        assert!((sample.rotation.length_squared() - 1.0).abs() < 1e-6);
        assert!((sample.rotation * Vec3::X).abs_diff_eq(Vec3::new(0.809017, 0.58778524, 0.0), 1e-6));
        assert_eq!(sample.translation, Vec3::X * 0.4);
        let almost = BoneXform {
            rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 1.001),
            translation: Vec3::ZERO,
        };
        assert!((blend(a, almost, 0.4).rotation.length_squared() - 1.0).abs() < 1e-6);
    }
}
