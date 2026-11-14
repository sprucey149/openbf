//! An emitter ordnance's data (`analysis/re-ordnance-kinds.md` §8): the
//! instant cone-and-chain shot of the arc casters, the officer's incinerator
//! and the hero flamethrowers. An emitter is not a projectile — the class's
//! create slot runs one query per weapon shot — so every key here feeds the
//! [`crate::emitter`] law instead of a [`crate::projectile::Bolt`].

use super::{pf, pf_hash, PropSource, DEG};
use crate::fmath::FloatExt;

/// `Push` (the same key an explosion authors): how hard each body hit is
/// shoved. 0 = no push at all (the BF1 arc caster; it must not draw).
const HASH_PUSH: u32 = 2_272_264_157;
/// `LightningEffect`: the arc a renderer draws between the query's links.
const HASH_LIGHTNING: u32 = 1_842_718_868;
/// `AffectFriends` (ctor default 1): BF2's team filter.
const HASH_AFFECT_FRIENDS: u32 = 2_870_179_545;
/// `AffectEnemies` (ctor default 1).
const HASH_AFFECT_ENEMIES: u32 = 3_558_787_040;
/// The flamethrowers' hit-all flag (`0x4c387ca1`, functionally identified;
/// its ODF key name is untraced).
const HASH_HIT_ALL: u32 = 1_278_770_337;
const HASH_EFFECT: u32 = 3_236_038_708;
const HASH_BOLTS: u32 = 1_664_633_633;
const HASH_BURN_TIME: u32 = 1_747_252_337;
const HASH_BURN_RATE: u32 = 276_226_006;
const HASH_BURN_BONE: u32 = 1_828_230_651;
const HASH_BURN_EFFECT: u32 = 1_496_297_824;
pub const BURN_SLOTS: usize = 5;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BurnClass {
    pub time: f32,
    pub rate: f32,
    pub effects: Vec<(String, String)>,
}

/// An `emitterordnance` class's own rules. `None` on a
/// [`super::WeaponClass`] means the class is not an emitter; an emitter
/// whose [`EmitterOrdnance::cone_length`] is 0 is the radius-ability path
/// ([`super::Ability`]) instead.
#[derive(Clone, Debug, PartialEq)]
pub struct EmitterOrdnance {
    /// `ConeLength`: the axial reach of a shot (m), and the half-side of
    /// BF1's gather box.
    pub cone_length: f32,
    /// `ConeAngle` in radians (the ODF authors degrees): the full angle.
    pub cone_angle: f32,
    /// `MaxTargets`: a ceiling on the chain; hit-all touches at most this
    /// many bodies.
    pub max_targets: u32,
    /// `JumpDeduction`: damage lost per chain link.
    pub jump_deduction: f32,
    /// `DamageThreshold`: the chain stops once its damage is under this.
    pub damage_threshold: f32,
    /// `MaxJumpDistance`: the axial reach of every link after the first.
    pub max_jump_distance: f32,
    /// `FirstRadius`: a ball at the muzzle the first link accepts besides the
    /// cone.
    pub first_radius: f32,
    /// `Push`: the shove on each body hit (0 = none).
    pub push: f32,
    /// BF2's `LightningEffect`: render-only; the arc the query draws.
    pub lightning: Option<String>,
    pub bolts: u32,
    /// BF2's `AffectFriends` / `AffectEnemies` (both default true): which
    /// teams the query takes. BF1 ignores them (no team test at all).
    pub affect_friends: bool,
    pub affect_enemies: bool,
    /// The `0x4c387ca1` hit-all mode (the flamethrowers and the incinerator):
    /// every body in the cone takes `max(Damage, D/N)` instead of a chain.
    pub hit_all: bool,
    /// `Radius`: with [`EmitterOrdnance::cone_length`] 0 the class is a
    /// radius ability, not a cone (the dispatcher keeps it on
    /// [`super::Ability`]'s path).
    pub radius: f32,
    pub effect: Option<String>,
    pub burn: BurnClass,
    /// BF2's rules: the true cone, vehicles in the gather, the team filter,
    /// the class's damage scales and hit-all. Set by the class loader
    /// (`ClassDb::is_bf2`), so a BF2 class on a BF1 map still follows BF2.
    pub bf2: bool,
}

impl EmitterOrdnance {
    /// Parse an `emitterordnance` PROP list (B1 `FUN_00278208`
    /// B1:198836-199273; B2 `FUN_00315f78` B2:305952-307007).
    pub fn from_props(src: &dyn PropSource, bf2: bool) -> Self {
        Self {
            cone_length: pf(src, "ConeLength").unwrap_or(0.0).fmax(0.0),
            cone_angle: pf(src, "ConeAngle").unwrap_or(0.0).fmax(0.0) * DEG,
            max_targets: pf(src, "MaxTargets").unwrap_or(1.0).fmax(1.0) as u32,
            jump_deduction: pf(src, "JumpDeduction").unwrap_or(0.0).fmax(0.0),
            damage_threshold: pf(src, "DamageThreshold").unwrap_or(0.0),
            max_jump_distance: pf(src, "MaxJumpDistance").unwrap_or(0.0).fmax(0.0),
            first_radius: pf(src, "FirstRadius").unwrap_or(0.0).fmax(0.0),
            push: pf_hash(src, HASH_PUSH).unwrap_or(0.0),
            lightning: src.prop_hash(HASH_LIGHTNING).map(|s| s.trim().to_owned()),
            bolts: pf_hash(src, HASH_BOLTS).map_or(1, |n| n.fmax(0.0) as u32),
            affect_friends: pf_hash(src, HASH_AFFECT_FRIENDS).unwrap_or(1.0) != 0.0,
            affect_enemies: pf_hash(src, HASH_AFFECT_ENEMIES).unwrap_or(1.0) != 0.0,
            hit_all: pf_hash(src, HASH_HIT_ALL).unwrap_or(0.0) != 0.0,
            radius: pf(src, "Radius").unwrap_or(0.0).fmax(0.0),
            effect: src.prop_hash(HASH_EFFECT).map(|s| s.trim().to_owned()),
            burn: BurnClass {
                time: pf_hash(src, HASH_BURN_TIME).unwrap_or(0.0),
                rate: pf_hash(src, HASH_BURN_RATE).unwrap_or(0.0),
                effects: src
                    .prop_hash_all(HASH_BURN_BONE)
                    .into_iter()
                    .zip(src.prop_hash_all(HASH_BURN_EFFECT))
                    .take(BURN_SLOTS)
                    .map(|(b, e)| (b.trim().to_owned(), e.trim().to_owned()))
                    .collect(),
            },
            bf2,
        }
    }

    /// A cone shot (the arc casters) rather than a radius ability (the
    /// officer's area powers).
    pub fn is_cone(&self) -> bool {
        self.cone_length > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::test_props::List;

    /// `rep_weap_inf_arccaster_ord` (bf1_rep.ron).
    #[test]
    fn bf1_arc_caster_reads_its_class() {
        let props: &[(u32, &str)] = &[
            (2_470_609_998, "30"),
            (2_750_430_025, "50"),
            (2_630_363_935, "0.6"),
            (2_685_028_557, "4"),
            (4_228_734_580, "75"),
            (3_433_514_099, "75"),
            (2_906_092_622, "20"),
            (2_272_264_157, "0.0"),
            (1_842_718_868, "arccaster_lightning"),
        ];
        let e = EmitterOrdnance::from_props(&List(props), false);
        assert!(e.is_cone());
        assert_eq!(e.cone_length, 30.0);
        assert!((e.cone_angle - 50.0 * DEG).abs() < 1e-6);
        assert_eq!(e.max_targets, 4);
        assert_eq!(e.jump_deduction, 75.0);
        assert_eq!(e.damage_threshold, 75.0);
        assert_eq!(e.max_jump_distance, 20.0);
        assert_eq!(e.first_radius, 0.6);
        assert_eq!(e.push, 0.0);
        assert_eq!(e.lightning.as_deref(), Some("arccaster_lightning"));
        assert!(e.affect_friends && e.affect_enemies);
        assert!(!e.hit_all);
        assert!(!e.bf2);
        assert_eq!(e.bolts, 1);
    }

    #[test]
    fn force_lightning_reads_its_bolt_count() {
        let props: &[(u32, &str)] = &[(2_470_609_998, "10"), (1_664_633_633, "5")];
        assert_eq!(EmitterOrdnance::from_props(&List(props), true).bolts, 5);
    }

    /// `com_weap_inf_flamethrower_ord` (bf2_imp.ron): the hit-all mode and
    /// the team filter.
    #[test]
    fn bf2_flamethrower_reads_its_class() {
        let props: &[(u32, &str)] = &[
            (2_470_609_998, "10"),
            (2_750_430_025, "45"),
            (2_630_363_935, "2.0"),
            (2_685_028_557, "10"),
            (1_278_770_337, "1"),
            (4_228_734_580, "0"),
            (3_433_514_099, "0"),
            (2_906_092_622, "0"),
            (2_870_179_545, "0"),
            (3_558_787_040, "1"),
            (1_842_718_868, "com_sfx_flamethrower"),
        ];
        let e = EmitterOrdnance::from_props(&List(props), true);
        assert_eq!(e.cone_length, 10.0);
        assert_eq!(e.max_targets, 10);
        assert_eq!(e.first_radius, 2.0);
        assert!(e.hit_all);
        assert!(!e.affect_friends);
        assert!(e.affect_enemies);
        assert!(e.bf2);
    }

    #[test]
    fn bf2_flamethrower_and_incinerator_read_their_burn() {
        let flame: &[(u32, &str)] = &[
            (2_470_609_998, "10"),
            (3_236_038_708, "com_sfx_weap_flamethrower_ord"),
            (276_226_006, "50.0"),
            (1_747_252_337, "5.0"),
            (1_496_297_824, "com_sfx_inf_damage_fire"),
            (1_828_230_651, "bone_ribcage"),
            (1_496_297_824, "com_sfx_inf_damage_fire"),
            (1_828_230_651, "bone_head"),
            (1_496_297_824, "com_sfx_inf_damage_fire"),
            (1_828_230_651, "bone_pelvis"),
        ];
        let e = EmitterOrdnance::from_props(&List(flame), true);
        assert_eq!(e.effect.as_deref(), Some("com_sfx_weap_flamethrower_ord"));
        assert_eq!(e.burn.time, 5.0);
        assert_eq!(e.burn.rate, 50.0);
        let fire = "com_sfx_inf_damage_fire".to_owned();
        assert_eq!(
            e.burn.effects,
            vec![
                ("bone_ribcage".to_owned(), fire.clone()),
                ("bone_head".to_owned(), fire.clone()),
                ("bone_pelvis".to_owned(), fire),
            ]
        );

        let incinerator: &[(u32, &str)] = &[
            (2_470_609_998, "10"),
            (3_236_038_708, "com_sfx_weap_incinerate_ord"),
            (1_747_252_337, "1.5"),
            (1_496_297_824, "com_sfx_inf_damage_disintergrate"),
            (1_828_230_651, "bone_l_forearm"),
            (1_496_297_824, "com_sfx_inf_damage_disintergrate"),
            (1_828_230_651, "bone_r_forearm"),
        ];
        let e = EmitterOrdnance::from_props(&List(incinerator), true);
        assert_eq!(e.burn.time, 1.5);
        assert_eq!(e.burn.rate, 0.0);
        assert_eq!(e.burn.effects.len(), 2);

        let e = EmitterOrdnance::from_props(&List(&[(2_470_609_998, "30")]), true);
        assert_eq!(e.burn, BurnClass::default());
        assert_eq!(e.effect, None);
    }

    /// A radius `emitterordnance` (the droid incinerator) is not a cone; the
    /// dispatcher must leave it to the ability path.
    #[test]
    fn a_radius_class_is_not_a_cone() {
        let props: &[(u32, &str)] = &[
            (230_313_139, "12"),
            (2_470_609_998, "0"),
            (2_750_430_025, "120"),
        ];
        let e = EmitterOrdnance::from_props(&List(props), true);
        assert!(!e.is_cone());
        assert_eq!(e.radius, 12.0);
    }
}
