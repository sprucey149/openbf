//! `entc` (entity class) + `gmod` (geometry LOD table) parsing.
//!
//! Verified against TAT1.LVL raw bytes (2026-08-30, see RE_LOG §Entity classes):
//! - entc: BASE (parent class string), TYPE (class name string), PROP* — same
//!   [u32 fnv1a(key)][NUL-terminated value] encoding as wrld instances. The
//!   render-geometry link is PROP key fnv1a("geometryname") = 0x47C86B4A.
//! - gmod: NAME (geometry name), INFO, then LOD0/LOD1/LOD2/LOWD entries =
//!   NUL-terminated model (modl) name + a trailing u32, the model's triangle
//!   count (BF2's loader reads it as an int: `analysis/re-far-lowd.md` §2.1,
//!   §9 — 588 / 1200 for UTA1's cliffs).
//! - gmod INFO (BF2, B2 FUN_001c59e8): u32, f32, u32 count (8 slots; B2
//!   does not check it), then count NUL-terminated cloth names - the cloths a soldier of this geometry wears
//!   (`analysis/re-cloth.md` 2 [O]).
//!
//! Instance class → entc(GeometryName) → gmod(NAME) → LOD model → modl chunk.

use crate::world::Prop;
use crate::{Chunk, UcfbError};

/// fnv1a_lowercase("geometryname") — the entc PROP key naming the gmod.
pub const PROP_GEOMETRYNAME: u32 = 0x47C8_6B4A;

/// The eight entc PROP keys that select which BODIES of a model belong to a
/// collision LAYER, with the port's layer name for each — the only collision
/// keys either ELF consumes (analysis/re-collision-layers.md §2, §8; `FlyerCollision`
/// 0x090AFB1F is consumed by NEITHER engine and is deliberately absent).
/// Order here is the table's, not the discs'; the PROP list's own order is
/// what matters and [`EntityClass::collision_layers`] preserves it.
pub const PROP_COLLISION_LAYERS: [(u32, &str); 8] = [
    (0x5DFD_C07F, "soldier"),      // SoldierCollision      (BF1 FUN_001E5200)
    (0xDE53_65A1, "vehicle"),      // VehicleCollision      (BF1 FUN_001E4FA8)
    (0x9828_F43F, "building"),     // BuildingCollision     (BF1 FUN_001E4EE0)
    (0xAFE6_93CE, "terrain"),      // TerrainCollision      (BF1 FUN_001E5070)
    (0xFB2B_DF07, "ordnance"),     // OrdnanceCollision     (BF1 FUN_001E5138)
    (0xD23D_BAF8, "targetable"),   // TargetableCollision   (BF1 FUN_001E52C8)
    (0xC3F2_5CBB, "vehicle_only"), // VehicleCollisionOnly  (BF1 FUN_001FB6E8)
    (0xF31D_1478, "flyer_uses_vehicle"),
];

#[derive(Debug)]
pub struct EntityClass {
    /// Class name, matches wrld instance `class` (entc TYPE).
    pub name: String,
    /// Parent class (entc BASE), e.g. "hover", "prop", "building".
    pub base: String,
    pub props: Vec<Prop>,
}

impl EntityClass {
    /// First GeometryName property, if the class has render geometry.
    pub fn geometry_name(&self) -> Option<&str> {
        self.props
            .iter()
            .find(|p| p.key_hash == PROP_GEOMETRYNAME)
            .map(|p| p.value.as_str())
    }

    /// Every collision-layer property of the class, `(layer, body)` in PROP
    /// ORDER — the order the engine applies them in, and therefore the order
    /// the manifest must keep (analysis/re-collision-layers.md §2: a property
    /// REPLACES its layer's subset with the bodies accumulated so far, so
    /// repeated lines OR together and a later key does not see an earlier
    /// key's set). The value is lowercased verbatim: a primitive NAME, the
    /// literal `collisionmesh`, or the sentinels `none` / `clear` (the
    /// handlers' 0xADA7AFDB / 0x5C6E1222) meaning an EMPTY subset. Keys that
    /// are not one of the seven are ignored, as they are here today.
    pub fn collision_layers(&self) -> Vec<(String, String)> {
        self.props
            .iter()
            .filter_map(|p| {
                PROP_COLLISION_LAYERS
                    .iter()
                    .find(|(h, _)| *h == p.key_hash)
                    .map(|(_, layer)| ((*layer).to_owned(), p.value.to_ascii_lowercase()))
            })
            .collect()
    }

    pub fn parse(entc: &Chunk<'_>) -> Result<EntityClass, UcfbError> {
        let bad = || UcfbError::Truncated(entc.offset);
        let name = entc.child_string(b"TYPE").ok_or_else(bad)?;
        let base = entc.child_string(b"BASE").unwrap_or_default();
        let props = crate::world::parse_props(entc);
        Ok(EntityClass { name, base, props })
    }
}

/// One LOD entry of a gmod: which modl to draw, and its triangle count.
#[derive(Debug)]
pub struct GeomLod {
    /// "LOD0" | "LOD1" | "LOD2" | "LOWD" as stored.
    pub tier: String,
    /// modl chunk name.
    pub model: String,
    /// The trailing u32: the model's triangle count (0 when absent).
    pub tris: u32,
}

#[derive(Debug)]
pub struct GeomModel {
    /// Geometry name referenced by entc GeometryName.
    pub name: String,
    pub lods: Vec<GeomLod>,
    /// The INFO list's cloth names (BF2; empty when there is none). A name is
    /// worn only where a `cloth` class and a `CLTH` chunk of that name exist
    /// (`crate::cloth`, re-cloth 2).
    pub cloths: Vec<String>,
}

impl GeomModel {
    /// Best full-detail model: LOD0 first, then LOD1, LOD2, LOWD last.
    pub fn best_model(&self) -> Option<&str> {
        for tier in ["LOD0", "LOD1", "LOD2", "LOWD"] {
            if let Some(l) = self.lods.iter().find(|l| l.tier == tier) {
                return Some(&l.model);
            }
        }
        None
    }

    pub fn parse(gmod: &Chunk<'_>) -> Result<GeomModel, UcfbError> {
        let bad = || UcfbError::Truncated(gmod.offset);
        let name = gmod.child_string(b"NAME").ok_or_else(bad)?;
        let mut lods = Vec::new();
        for c in gmod.children().flatten() {
            if !matches!(&c.tag, b"LOD0" | b"LOD1" | b"LOD2" | b"LOWD") {
                continue;
            }
            let p = c.payload;
            let Some(end) = p.iter().position(|&b| b == 0) else {
                continue;
            };
            let Ok(model) = std::str::from_utf8(&p[..end]) else {
                continue;
            };
            // Name is NUL-padded; the last 4 bytes are the triangle count.
            let tris = if p.len() >= end + 5 {
                u32::from_le_bytes(p[p.len() - 4..].try_into().unwrap())
            } else {
                0
            };
            lods.push(GeomLod {
                tier: String::from_utf8_lossy(&c.tag).into_owned(),
                model: model.to_owned(),
                tris,
            });
        }
        let cloths = gmod
            .child(b"INFO")
            .map_or_else(Vec::new, |c| cloth_list(c.payload));
        Ok(GeomModel { name, lods, cloths })
    }
}

/// The gmod INFO's cloth names: u32, f32, u32 count, count NUL names. B2
/// FUN_001c59e8 stores them in 8 slots and does not check the count (B2:72386-72404).
/// OURS: a count over 8, or a name past the payload, reads as none.
fn cloth_list(info: &[u8]) -> Vec<String> {
    let Some(n) = info
        .get(8..12)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    else {
        return Vec::new();
    };
    if n > 8 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut rest = &info[12..];
    for _ in 0..n {
        let Some(end) = rest.iter().position(|&b| b == 0) else {
            return Vec::new();
        };
        out.push(String::from_utf8_lossy(&rest[..end]).into_owned());
        rest = &rest[end + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmod_info_lists_cloths() {
        let mut info = vec![0u8; 8];
        info.extend(2u32.to_le_bytes());
        info.extend(b"a_skirt\0b_cape\0\0");
        assert_eq!(cloth_list(&info), vec!["a_skirt", "b_cape"]);
        info[8] = 9;
        assert!(cloth_list(&info).is_empty());
        assert!(cloth_list(&[0; 12]).is_empty());
    }

    #[test]
    fn geometryname_prop_key_matches_fnv1a() {
        assert_eq!(crate::fnv1a_lowercase("GeometryName"), PROP_GEOMETRYNAME);
    }

    /// The seven collision keys are confirmed fnv1a preimages, like the sky /
    /// light tables' — the lead re-derived every one
    /// (analysis/re-collision-layers.md, header).
    #[test]
    fn collision_layer_prop_keys_match_fnv1a() {
        for (key, layer) in [
            ("SoldierCollision", "soldier"),
            ("VehicleCollision", "vehicle"),
            ("BuildingCollision", "building"),
            ("TerrainCollision", "terrain"),
            ("OrdnanceCollision", "ordnance"),
            ("TargetableCollision", "targetable"),
            ("VehicleCollisionOnly", "vehicle_only"),
        ] {
            let h = crate::fnv1a_lowercase(key);
            let found = PROP_COLLISION_LAYERS
                .iter()
                .find(|(hash, _)| *hash == h)
                .unwrap_or_else(|| panic!("{key} = 0x{h:08X} missing"));
            assert_eq!(found.1, layer, "{key}");
        }
        // FlyerCollision is consumed by neither ELF (§8) and must NOT be here.
        let flyer = crate::fnv1a_lowercase("FlyerCollision");
        assert_eq!(flyer, 0x090A_FB1F);
        assert!(!PROP_COLLISION_LAYERS.iter().any(|(h, _)| *h == flyer));
    }

    /// `nab2_bldg_b_19`'s four lines, in the disc's order (BF1 NAB2.LVL entc
    /// @0x3E99B4): the vector keeps the order and lowercases the values.
    #[test]
    fn collision_layers_keep_prop_order() {
        let h = crate::fnv1a_lowercase;
        let e = EntityClass {
            name: "nab2_bldg_b_19".into(),
            base: "prop".into(),
            props: vec![
                Prop {
                    key_hash: h("GeometryName"),
                    value: "nab2_bldg_b_19".into(),
                },
                Prop {
                    key_hash: h("VehicleCollision"),
                    value: "p_cube".into(),
                },
                Prop {
                    key_hash: h("VehicleCollision"),
                    value: "p_cube1".into(),
                },
                Prop {
                    key_hash: h("SoldierCollision"),
                    value: "CollisionMesh".into(),
                },
                Prop {
                    key_hash: h("OrdnanceCollision"),
                    value: "CollisionMesh".into(),
                },
            ],
        };
        assert_eq!(
            e.collision_layers(),
            vec![
                ("vehicle".to_string(), "p_cube".to_string()),
                ("vehicle".to_string(), "p_cube1".to_string()),
                ("soldier".to_string(), "collisionmesh".to_string()),
                ("ordnance".to_string(), "collisionmesh".to_string()),
            ]
        );
    }
}
