//! What a struck bolt and an explosion spawn (card FXP3,
//! `analysis/re-fx.md` §7 [O]): the ordnance's `ImpactEffect*` names picked
//! by the struck surface, the round's `TrailEffect`, and its explosion
//! class's `Effect` / `WaterEffect`. Render-only, like
//! [`crate::laser::LaserLook`]: [`crate::weapon::OrdnanceClass::impact`] and
//! [`crate::weapon::ExplosionClass::look`] carry them, the sim never reads
//! them, and [`crate::sim::Event::Impact`] says which surface a bolt struck.
//!
//! Names are kept as authored; one the running game's effect table does not
//! hold spawns nothing, as on the PS2 (BF2 inherits eight BF1-only names, and
//! its bullet ctor's `dirtspray` / `watersplash_ord` defaults exist only on
//! the BF1 disc, re-fx §7).

/// Impact keys (fnv over byte|0x20; re-fx §7, AGENT_C §2).
pub const HASH_IMPACT_SOFT: u32 = 0xd622_0fe6;
pub const HASH_IMPACT_RIGID: u32 = 0xa309_1269;
pub const HASH_IMPACT_STATIC: u32 = 0x087e_a2f0;
pub const HASH_IMPACT_TERRAIN: u32 = 0x5a55_4273;
pub const HASH_IMPACT_WATER: u32 = 0xecaa_a59d;
/// BF1 only: BF2 never reads `ImpactEffectShield` (elfword 0) and keys its
/// shield slots `ImpactEffectShieldSmall` / `Large`, which no class authors.
pub const HASH_IMPACT_SHIELD: u32 = 0x4bc6_c22f;
pub const HASH_IMPACT_SHIELD_SMALL: u32 = 0x8fc7_9ebe;
pub const HASH_IMPACT_SHIELD_LARGE: u32 = 0xf73e_c52a;
pub const HASH_TRAIL_EFFECT: u32 = 0x7e71_e2c6;
/// `ImpactEffect`, the beam setter's one impact slot (B2:291502).
pub const HASH_BEAM_IMPACT: u32 = 0x1172_7a92;
/// The explosion class's keys.
pub const HASH_EFFECT: u32 = 0x6e6e_8d54;
pub const HASH_WATER_EFFECT: u32 = 0x8773_3801;
pub const HASH_DAMAGE_RADIUS: u32 = 0xb687_98a6;
pub const HASH_DAMAGE_RADIUS_OUTER: u32 = 0xa23b_939d;
pub const WATER_BLAST_DEPTH: f32 = 0.3;
pub const SPLASH_LIFT: f32 = 0.02;
/// `SoundProperty` on an explosion class (re-weapon-sounds.md §4.1).
pub const HASH_SOUND_PROPERTY: u32 = 0xf350_b4e5;

/// BF2's explosion keeps at most five `Effect` rows and detonates one at
/// random (B2 `FUN_0030bb88` +0xa4, `FUN_0030c8f8` B2:300201–300232).
pub const BF2_EFFECT_ROWS: usize = 5;

/// What a bolt struck, as the impact handler's collision class names it
/// (B1 `FUN_00276810` B1:197939–198009, B2 `FUN_00313c40` B2:304647–304744;
/// [D] the classes are the collision layers — soldiers, vehicles,
/// buildings, terrain).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// A soldier or a rider → `ImpactEffectSoft`.
    Soldier,
    /// A vehicle hull → `ImpactEffectRigid`.
    Vehicle,
    /// A building / world model → `ImpactEffectStatic`.
    Building,
    /// The terrain → `ImpactEffectTerrain`.
    Terrain,
    /// Water → `ImpactEffectWater`.
    Water,
    /// A shield → BF1 `ImpactEffectShield`, BF2 the small shield impact.
    Shield,
    /// A large capital-ship shield → BF2 large shield impact (`com_sfx_shieldimpact_xl`).
    ShieldLarge,
}

/// An ordnance class's impact effects (re-fx §7): its bullet-base slots after
/// the setters, replayed parent first ([`ImpactLook::replay`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImpactLook {
    pub soft: Option<String>,
    pub rigid: Option<String>,
    pub building: Option<String>,
    pub terrain: Option<String>,
    pub water: Option<String>,
    pub shield: Option<String>,
    pub shield_large: Option<String>,
    /// `TrailEffect`: attached to the round at launch (B1 FUN_002728a8).
    pub trail: Option<String>,
    /// The ordnance's `ExplosionName` class's look: detonated at the hit
    /// after the impact effect (B1 `FUN_00272d70` → `FUN_002701a0`).
    pub explosion: Option<ExplosionLook>,
}

impl ImpactLook {
    /// The bullet ctor's defaults (B1 `FUN_00277248` B1:198246–198248; B2
    /// `FUN_003148f0` B2:305158–305161): Terrain `dirtspray`, Water
    /// `watersplash_ord`, and BF2's small and large shield impacts.
    pub fn ctor(bf2: bool) -> Self {
        Self {
            terrain: Some("dirtspray".into()),
            water: Some("watersplash_ord".into()),
            shield: bf2.then(|| "com_sfx_shieldimpact_sm".into()),
            shield_large: bf2.then(|| "com_sfx_shieldimpact_xl".into()),
            ..Self::default()
        }
    }

    /// One PROP through the setters: each key writes its slot, so the last
    /// row wins.
    pub fn set(&mut self, hash: u32, value: &str, bf2: bool) {
        let v = || Some(value.trim().to_owned()).filter(|s| !s.is_empty());
        match hash {
            HASH_IMPACT_SOFT => self.soft = v(),
            HASH_IMPACT_RIGID => self.rigid = v(),
            HASH_IMPACT_STATIC => self.building = v(),
            HASH_IMPACT_TERRAIN => self.terrain = v(),
            HASH_IMPACT_WATER => self.water = v(),
            HASH_IMPACT_SHIELD if !bf2 => self.shield = v(),
            HASH_IMPACT_SHIELD_SMALL => self.shield = v(),
            HASH_IMPACT_SHIELD_LARGE => self.shield_large = v(),
            HASH_TRAIL_EFFECT => self.trail = v(),
            _ => {}
        }
    }

    /// An OrdnanceBeam's: its setter's one `ImpactEffect` (B1 `FUN_00275bd0`
    /// class +0x94, B2 `FUN_00312838` class +0xdc), spawned at the beam's end
    /// on every update it strikes something (B1:196819, B2:291073), with the
    /// ordnance base's `TrailEffect`. No bullet ctor default and no
    /// per-surface key: the beam's setter never reaches the bullet's. Water
    /// shows none: the user's PS2 capture of BF2 Kashyyyk (2026-11-12) draws
    /// the IFT-X beam's effect on the ground and none on the water; the
    /// original's ray flags for water are not traced.
    pub fn beam<'a>(props: impl IntoIterator<Item = (u32, &'a str)>) -> Self {
        let mut effect = None;
        let mut trail = None;
        for (h, v) in props {
            let v = Some(v.trim().to_owned()).filter(|s| !s.is_empty());
            match h {
                HASH_BEAM_IMPACT => effect = v,
                HASH_TRAIL_EFFECT => trail = v,
                _ => {}
            }
        }
        Self {
            soft: effect.clone(),
            rigid: effect.clone(),
            building: effect.clone(),
            terrain: effect.clone(),
            water: None,
            shield: effect.clone(),
            shield_large: effect,
            trail,
            explosion: None,
        }
    }

    /// The ctor, then `props` (the class chain, parent first, each class's
    /// in file order).
    pub fn replay<'a>(props: impl IntoIterator<Item = (u32, &'a str)>, bf2: bool) -> Self {
        let mut l = Self::ctor(bf2);
        for (h, v) in props {
            l.set(h, v, bf2);
        }
        l
    }

    /// The effect a hit on `s` spawns.
    pub fn effect(&self, s: Surface) -> Option<&str> {
        match s {
            Surface::Soldier => self.soft.as_deref(),
            Surface::Vehicle => self.rigid.as_deref(),
            Surface::Building => self.building.as_deref(),
            Surface::Terrain => self.terrain.as_deref(),
            Surface::Water => self.water.as_deref(),
            Surface::Shield => self.shield.as_deref(),
            Surface::ShieldLarge => self.shield_large.as_deref().or(self.shield.as_deref()),
        }
    }
}

/// An explosion class's effects (re-fx §7): `Effect` at the blast (BF2: up to
/// [`BF2_EFFECT_ROWS`], one at random) or `WaterEffect` when the blast is at
/// or below the water + 0.3 m (B1 `FUN_002701a0` B1:193761–193807).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExplosionLook {
    /// The explosion class's name, lowercase: its debris
    /// (`game::fx::chunks::Chunks::blast`) goes off with it.
    pub class: String,
    pub effects: Vec<String>,
    pub water_effect: Option<String>,
    /// `SoundProperty`: played once per explosion, at the blast, whatever
    /// else it does (B1 `FUN_002701a0` 193760, B2 `FUN_0030c8f8` 300192).
    pub sound: Option<String>,
    pub radius_outer: f32,
}

impl ExplosionLook {
    /// `props` parent first: BF1's `Effect` is one slot (the last row wins),
    /// BF2's a list that keeps its first [`BF2_EFFECT_ROWS`] rows
    /// (PROVISIONAL [H]: whether a subclass's rows append to its parent's —
    /// no class on either disc authors two).
    pub fn replay<'a>(props: impl IntoIterator<Item = (u32, &'a str)>, bf2: bool) -> Self {
        let mut l = Self::default();
        for (h, v) in props {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            match h {
                HASH_EFFECT if bf2 => {
                    if l.effects.len() < BF2_EFFECT_ROWS {
                        l.effects.push(v.to_owned());
                    }
                }
                HASH_EFFECT => l.effects = vec![v.to_owned()],
                HASH_WATER_EFFECT => l.water_effect = Some(v.to_owned()),
                HASH_DAMAGE_RADIUS | HASH_DAMAGE_RADIUS_OUTER => {
                    if let Ok(r) = v.parse() {
                        l.radius_outer = r;
                    }
                }
                // `"<name> defer"`: the second token only says when the PS2
                // resolves the name (re-weapon-sounds.md §3.1).
                HASH_SOUND_PROPERTY => l.sound = v.split_whitespace().next().map(str::to_owned),
                _ => {}
            }
        }
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A beam spawns its one `ImpactEffect` on every surface but water
    /// (B2:291073, the PS2 capture), and takes no bullet default or
    /// per-surface key.
    #[test]
    fn a_beam_has_one_impact_for_every_surface() {
        let l = ImpactLook::beam([
            (HASH_BEAM_IMPACT, "com_sfx_weap_beam_exp"),
            (HASH_IMPACT_WATER, "com_sfx_watersplash_md"),
        ]);
        for s in [
            Surface::Soldier,
            Surface::Vehicle,
            Surface::Building,
            Surface::Terrain,
            Surface::Shield,
            Surface::ShieldLarge,
        ] {
            assert_eq!(l.effect(s), Some("com_sfx_weap_beam_exp"), "{s:?}");
        }
        assert_eq!(l.effect(Surface::Water), None);
    }

    #[test]
    fn surface_keys_replay_over_the_ctor() {
        let l = ImpactLook::replay(
            [
                (HASH_IMPACT_SOFT, "ord_explosion"),
                (HASH_IMPACT_TERRAIN, "a"),
                (HASH_IMPACT_TERRAIN, "ord_explosion"),
                (HASH_IMPACT_SHIELD, "ord_explosion"),
            ],
            false,
        );
        assert_eq!(l.effect(Surface::Soldier), Some("ord_explosion"));
        assert_eq!(l.effect(Surface::Terrain), Some("ord_explosion"));
        assert_eq!(l.effect(Surface::Water), Some("watersplash_ord"));
        assert_eq!(l.effect(Surface::Shield), Some("ord_explosion"));
        assert_eq!(l.effect(Surface::Vehicle), None);
        // BF2 never reads ImpactEffectShield: its ctor's small impact stands.
        let l = ImpactLook::replay([(HASH_IMPACT_SHIELD, "x")], true);
        assert_eq!(l.effect(Surface::Shield), Some("com_sfx_shieldimpact_sm"));
    }

    #[test]
    fn bf1_effect_is_one_slot_bf2_a_list_of_five() {
        let rows: Vec<(u32, &str)> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|s| (HASH_EFFECT, *s))
            .collect();
        assert_eq!(ExplosionLook::replay(rows.clone(), false).effects, ["f"]);
        for v in [" a_exp ", "a_exp defer"] {
            let l = ExplosionLook::replay([(HASH_SOUND_PROPERTY, v)], false);
            assert_eq!(l.sound.as_deref(), Some("a_exp"));
        }
        assert_eq!(
            ExplosionLook::replay(rows, true).effects,
            ["a", "b", "c", "d", "e"]
        );
    }
}
