//! Which BODIES of a model belong to a collision LAYER — the rule the PS2
//! engines actually run, replacing the port's session-7 name rule.
//!
//! Source: `analysis/re-collision-layers.md` (Fable RE 2026-09-03,
//! lead-verified on the raw discs and in both decomps). The short version:
//!
//! * A primitive's NAME is never parsed by either engine (§0.1, §7). The
//!   `*vehicle*` / `*flyer*` / `*ordnance*` rule this module replaces was
//!   ours, and it was wrong in both directions.
//! * Membership is per ENTITY CLASS, from the `entc` PROP list:
//!   `SoldierCollision`, `VehicleCollision`, `BuildingCollision`,
//!   `TerrainCollision`, `OrdnanceCollision`, `TargetableCollision` and
//!   `VehicleCollisionOnly` (the seven hashes in
//!   `mungefs::entity::PROP_COLLISION_LAYERS`). Each line names ONE body —
//!   a primitive's name, or the literal `collisionmesh` for the `coll` mesh
//!   body — or the sentinels `none` / `clear` meaning "this layer's subset is
//!   EMPTY" (§2, handler `FUN_001E5200`).
//! * With NO property, every body of the model is in the soldier, vehicle,
//!   building, terrain and ordnance subsets (§0.3, BF1 `FUN_001676B0`).
//! * A property REPLACES its own layer's subset with the accumulated set of
//!   the bodies its lines have named so far, and touches no other layer
//!   (§8). So the FIRST `SoldierCollision` line drops every body it does not
//!   name; repeated lines OR together, in PROP order.
//! * `VehicleCollisionOnly` ADDS the body to the vehicle subset and REMOVES
//!   it from the soldier and ordnance subsets (§2, BF1 `FUN_001FB6E8`).
//! * BF2 additionally seeds each body from its per-primitive `MASK` byte
//!   before the properties run: bits s 0x01, v 0x02, b 0x04, t 0x08,
//!   o 0x10; 0 (or absent) = all five subsets (§2, BF2 `FUN_00167048`).
//!   `mungefs::coll` already carries the loader's stale-MASK quirk, so the
//!   byte that reaches here is the byte the engine would have used.
//!
//! PROVISIONAL (ours, not read off the ELF): the accumulator is treated as
//! per CLASS and per LAYER — the engine keeps it in a per-class static that
//! the `GeometryName` handler resets (`DAT_005177xx` / `DAT_005178xx`, §2),
//! and whether the six handlers share one static or hold one each was not
//! read. Per-layer is what the shipped data means (`nab2_bldg_b_19`'s two
//! `VehicleCollision` lines must not leak into its `SoldierCollision` set).
//!
//! KNOWN GAP (ours): the body list this sees is the MANIFEST's, and the bake
//! drops primitives of an undecoded `DATA` kind (28 of them over the 32 maps,
//! in 16 geometries — `kam_prop_antiaircraft`, `hoth_bldg_hoth_turret`,
//! `nab2_bldg_rotunda_bridge` …). On the PS2 those are real bodies with real
//! index bits, so a property naming one resolves here to "no such body" (no
//! write) where the engine would have named it. Nothing on the discs is known
//! to depend on it; it is the one place this rule can differ from the engine's
//! for a reason that is not the rule.
//!
//! WHAT THE RUNTIME ASKS FOR: [`Layer`] is the RE's five layers, the INPUT of
//! this rule; `core::collision::Layers` is the runtime's per-body query mask,
//! its OUTPUT. They stay two types, mapped
//! `Layer::Soldier -> Layers::SOLDIER`, `Layer::Ordnance -> Layers::ORDNANCE`
//! and `Layer::Vehicle -> Layers::VEHICLE` in
//! `game::assets::Scene::collision_world`, which builds the world as the
//! UNION of the three subsets with a mask per body.
//!
//! BUILT: soldier, ordnance and vehicle (flyers query it). `CollisionWorld`'s soldier queries (ground,
//! push-out, wall sweep) read the soldier subset and its ray reads the
//! ordnance subset — the subset a PS2 ray with the trailing flag tests (§4:
//! bolts `FUN_002764C0`, the reticle, AI line of sight `FUN_00162E88`). On
//! the discs the two subsets differ on 1,362 of 26,980 instance-bodies: the
//! BF2 prims whose MASK is ordnance-only (`p_-o-*`) or soldier-only
//! (`p_-s-*`), plus every class that names different bodies for the two keys
//! — `hoth_prop_bridge_*`, whose `SoldierCollision` lines cover the mesh and
//! `p_move*` so its `p_ordinance*` prims block bolts but not soldiers, and
//! `com_inv_col_*` (BF2 common), whose `OrdnanceCollision = none` makes the
//! invisible walls bolt-transparent.
//!

/// One body of one model, in the manifest's body order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyRef {
    /// `collisionmesh` for the `coll` mesh body (that is the name the engine
    /// gives it — BF1 `FUN_00160DA0` names it `"CollisionMesh"` and the
    /// lookup is case-folded), else the primitive's name, LOWERCASED.
    pub name: String,
    /// The BF2 `MASK` byte as the engine saw it; 0 on BF1 and on every prim
    /// with no mask = "all five subsets". The mesh body always has 0.
    pub mask: u8,
}

impl BodyRef {
    /// The `coll` mesh body.
    pub fn mesh() -> BodyRef {
        BodyRef {
            name: MESH_BODY.to_owned(),
            mask: 0,
        }
    }

    /// A primitive body, by its manifest name.
    pub fn prim(name: &str, mask: u8) -> BodyRef {
        BodyRef {
            name: name.to_ascii_lowercase(),
            mask,
        }
    }
}

/// The name a collision-layer property uses for the `coll` mesh body.
pub const MESH_BODY: &str = "collisionmesh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Soldier,
    Vehicle,
    Building,
    Terrain,
    Ordnance,
    Targetable,
}

impl Layer {
    /// This layer's bit in the BF2 `MASK` byte (§2's measured letter table).
    pub fn mask_bit(self) -> u8 {
        match self {
            Layer::Soldier => 0x01,
            Layer::Vehicle => 0x02,
            Layer::Building => 0x04,
            Layer::Terrain => 0x08,
            Layer::Ordnance => 0x10,
            Layer::Targetable => 0,
        }
    }

    /// The manifest's spelling of this layer's `entc` key (the strings
    /// `bake::map` writes, from `mungefs::entity::PROP_COLLISION_LAYERS`).
    pub fn key(self) -> &'static str {
        match self {
            Layer::Soldier => "soldier",
            Layer::Vehicle => "vehicle",
            Layer::Building => "building",
            Layer::Terrain => "terrain",
            Layer::Ordnance => "ordnance",
            Layer::Targetable => "targetable",
        }
    }
}

/// The manifest spelling of `VehicleCollisionOnly`.
const VEHICLE_ONLY: &str = "vehicle_only";

/// A property value that empties its layer's subset: the two sentinels the
/// handlers test before looking a body up (hashes 0xADA7AFDB / 0x5C6E1222,
/// §2 — BF2 POL1's shields author `SoldierCollision = CLEAR`).
fn is_empty_sentinel(body: &str) -> bool {
    body == "none" || body == "clear"
}

/// Which of `bodies` are in `layer`, given the class's collision properties
/// in `entc` PROP ORDER. See the module docs for the rule and its source.
///
/// `props` are `(layer key, body)` exactly as `manifest::ManifestClass`
/// carries them; `bodies` are in the manifest's body order (the sim's).
pub const FLYER_USES_VEHICLE: &str = "flyer_uses_vehicle";

pub fn layer_bodies(bodies: &[BodyRef], props: &[(String, String)], layer: Layer) -> Vec<bool> {
    select_bodies(bodies, props, layer, false)
}

pub fn bf2_flyer_layer_bodies(
    bodies: &[BodyRef],
    props: &[(String, String)],
    layer: Layer,
) -> Vec<bool> {
    select_bodies(bodies, props, layer, true)
}

fn select_bodies(
    bodies: &[BodyRef],
    props: &[(String, String)],
    layer: Layer,
    flyer: bool,
) -> Vec<bool> {
    let mesh_seed = flyer && matches!(layer, Layer::Soldier | Layer::Ordnance);
    // Seed from the MASK byte: 0 = no mask read = every subset.
    let bit = layer.mask_bit();
    let mut set: Vec<bool> = bodies
        .iter()
        .map(|b| layer != Layer::Targetable && (b.mask == 0 || b.mask & bit != 0))
        .collect();
    let mut acc: Vec<bool> = bodies
        .iter()
        .map(|body| mesh_seed && body.name == MESH_BODY)
        .collect();
    let key = layer.key();
    for (prop_layer, body) in props {
        if prop_layer == key {
            if is_empty_sentinel(body) && !mesh_seed {
                // `accum = 0; setSubset(0)` — the layer is emptied.
                acc.iter_mut().for_each(|a| *a = false);
                set.copy_from_slice(&acc);
            } else if let Some(i) = bodies.iter().position(|b| b.name == *body) {
                // `accum |= 1 << idx; setSubset(accum)` — the subset BECOMES
                // the accumulator, so unnamed bodies drop out of this layer.
                acc[i] = true;
                set.copy_from_slice(&acc);
            }
            // A name no body carries is the engine's `idx < 0` branch: no
            // write at all, so the subset keeps whatever it held.
        } else if prop_layer == VEHICLE_ONLY {
            // Adds to vehicle, removes from soldier and ordnance; every other
            // layer is untouched.
            if let Some(i) = bodies.iter().position(|b| b.name == *body) {
                match layer {
                    Layer::Vehicle => set[i] = true,
                    Layer::Soldier | Layer::Ordnance => set[i] = false,
                    _ => {}
                }
            }
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(lines: &[(&str, &str)]) -> Vec<(String, String)> {
        lines
            .iter()
            .map(|(l, b)| ((*l).to_owned(), (*b).to_owned()))
            .collect()
    }

    /// `bodies` -> the names that are in `layer`, for readable assertions.
    fn kept(bodies: &[BodyRef], p: &[(String, String)], layer: Layer) -> Vec<String> {
        layer_bodies(bodies, p, layer)
            .into_iter()
            .zip(bodies)
            .filter(|(k, _)| *k)
            .map(|(_, b)| b.name.clone())
            .collect()
    }

    fn bodies(prims: &[(&str, u8)]) -> Vec<BodyRef> {
        let mut v = vec![BodyRef::mesh()];
        v.extend(prims.iter().map(|(n, m)| BodyRef::prim(n, *m)));
        v
    }

    #[test]
    fn flyer_named_wings_keep_the_fuselage_in_soldier_and_ordnance_sets() {
        let b = bodies(&[("wing", 0), ("foot", 0)]);
        let p = props(&[
            ("soldier", "wing"),
            ("ordnance", "wing"),
            ("vehicle", "foot"),
            ("terrain", "foot"),
        ]);
        for layer in [Layer::Soldier, Layer::Ordnance] {
            assert_eq!(bf2_flyer_layer_bodies(&b, &p, layer), [true, true, false]);
            assert_eq!(layer_bodies(&b, &p, layer), [false, true, false]);
        }
        for layer in [Layer::Vehicle, Layer::Terrain] {
            assert_eq!(bf2_flyer_layer_bodies(&b, &p, layer), [false, false, true]);
        }
    }

    #[test]
    fn flyer_mesh_seed_does_not_select_it_as_a_homing_target() {
        let b = bodies(&[("wing", 0)]);
        assert_eq!(
            bf2_flyer_layer_bodies(&b, &[], Layer::Targetable),
            [false, false]
        );
        assert_eq!(
            bf2_flyer_layer_bodies(&b, &props(&[("targetable", "wing")]), Layer::Targetable),
            [false, true]
        );
    }

    #[test]
    fn flyer_soldier_and_ordnance_overrides_ignore_unknown_body_names_and_sentinels() {
        let b = bodies(&[("wing", 0)]);
        let p = props(&[
            ("ordnance", "wing"),
            ("ordnance", "none"),
            ("ordnance", "clear"),
            ("ordnance", "absent"),
        ]);
        assert_eq!(
            bf2_flyer_layer_bodies(&b, &p, Layer::Ordnance),
            [true, true]
        );
        assert_eq!(layer_bodies(&b, &p, Layer::Ordnance), [false, false]);
    }

    /// BF1 NAB2.LVL entc `nab2_bldg_b_19` @0x3E99B4 (identical on BF2 NAB2
    /// @0x3AAC90): the plaza building whose two boxes span the whole plaza
    /// and trapped units on both NAB2s. Soldiers and bolts see only the mesh;
    /// vehicles only the boxes; building and terrain keep the default. The
    /// rule is §2's (each key REPLACES its own layer's subset with the bodies
    /// its lines have named) with §8's worked example of this very class.
    #[test]
    fn nab2_bldg_b_19_gives_soldiers_the_mesh_only() {
        let b = bodies(&[("p_cube", 0), ("p_cube1", 0)]);
        let p = props(&[
            ("vehicle", "p_cube"),
            ("vehicle", "p_cube1"),
            ("soldier", MESH_BODY),
            ("ordnance", MESH_BODY),
        ]);
        assert_eq!(kept(&b, &p, Layer::Soldier), ["collisionmesh"]);
        assert_eq!(kept(&b, &p, Layer::Ordnance), ["collisionmesh"]);
        assert_eq!(kept(&b, &p, Layer::Vehicle), ["p_cube", "p_cube1"]);
        assert_eq!(
            kept(&b, &p, Layer::Building),
            ["collisionmesh", "p_cube", "p_cube1"]
        );
        assert_eq!(
            kept(&b, &p, Layer::Terrain),
            ["collisionmesh", "p_cube", "p_cube1"]
        );
    }

    /// BF1 KAS2.LVL `kas2_bldg_hut_round`: `VehicleCollisionOnly =
    /// p_vehicle_3`, the 6 x 20 m cylinder around a hut. The old name rule
    /// got this one right by accident.
    #[test]
    fn vehicle_collision_only_takes_a_body_out_of_the_soldier_world() {
        let b = bodies(&[("p_vehicle_3", 0)]);
        let p = props(&[(VEHICLE_ONLY, "p_vehicle_3")]);
        assert_eq!(kept(&b, &p, Layer::Soldier), ["collisionmesh"]);
        assert_eq!(kept(&b, &p, Layer::Ordnance), ["collisionmesh"]);
        assert_eq!(
            kept(&b, &p, Layer::Vehicle),
            ["collisionmesh", "p_vehicle_3"]
        );
    }

    /// BF1 KAM1.LVL `kam_bldg_mushplatform1`: no collision property at all,
    /// so the mesh AND `p_collision_floor` — the cylinder that IS the Kamino
    /// platform floor — are soldier bodies (§3).
    #[test]
    fn a_class_with_no_property_keeps_every_body() {
        let b = bodies(&[("p_collision_floor", 0)]);
        assert_eq!(
            kept(&b, &[], Layer::Soldier),
            ["collisionmesh", "p_collision_floor"]
        );
    }

    /// BF1 KAS1.LVL `kas1_bldg_hut_grand`: `VehicleCollisionOnly` names
    /// `p_vehicle_1, _6, _7, _8` — so `p_vehicle_2` IS a soldier body on the
    /// PS2, and the old NAME rule wrongly dropped it.
    #[test]
    fn kas1_hut_grand_keeps_the_vehicle_prim_nobody_named() {
        let b = bodies(&[
            ("p_vehicle_1", 0),
            ("p_vehicle_2", 0),
            ("p_vehicle_6", 0),
            ("p_vehicle_7", 0),
            ("p_vehicle_8", 0),
        ]);
        let p = props(&[
            (VEHICLE_ONLY, "p_vehicle_1"),
            (VEHICLE_ONLY, "p_vehicle_6"),
            (VEHICLE_ONLY, "p_vehicle_7"),
            (VEHICLE_ONLY, "p_vehicle_8"),
        ]);
        assert_eq!(
            kept(&b, &p, Layer::Soldier),
            ["collisionmesh", "p_vehicle_2"]
        );
    }

    /// BF2 HOT1 `hoth_prop_bridge_long`: seven `SoldierCollision` lines
    /// accumulate into exactly those seven bodies, so the `p_ordinance*`
    /// prims are NOT soldier bodies (with no `OrdnanceCollision` line the
    /// ordnance layer keeps its MASK-seeded subset; the BF1 twin below adds
    /// those lines).
    #[test]
    fn repeated_soldier_lines_accumulate_and_drop_everything_unnamed() {
        let mut prims: Vec<(String, u8)> = (0..6)
            .map(|i| {
                (
                    if i == 0 {
                        "p_move".to_string()
                    } else {
                        format!("p_move{i}")
                    },
                    0u8,
                )
            })
            .collect();
        prims.extend((1..=6).map(|i| (format!("p_ordinance{i}"), 0u8)));
        let refs: Vec<BodyRef> = std::iter::once(BodyRef::mesh())
            .chain(prims.iter().map(|(n, m)| BodyRef::prim(n, *m)))
            .collect();
        let mut lines = vec![("soldier".to_string(), MESH_BODY.to_string())];
        lines.extend(
            prims
                .iter()
                .take(6)
                .map(|(n, _)| ("soldier".to_string(), n.clone())),
        );
        let got = kept(&refs, &lines, Layer::Soldier);
        assert_eq!(
            got,
            [
                "collisionmesh",
                "p_move",
                "p_move1",
                "p_move2",
                "p_move3",
                "p_move4",
                "p_move5"
            ]
        );
    }

    /// BF2 common `com_inv_col_8`: one box prim, no coll mesh, and a single
    /// `OrdnanceCollision = none` line (`assets_baked/common_bf2/common.ron`).
    /// The invisible wall keeps blocking soldiers and becomes BOLT-TRANSPARENT
    /// — the `none` sentinel of §2, on the ordnance layer only.
    /// `com_inv_col_16/32/64` are the same shape with a coll mesh as well.
    #[test]
    fn com_inv_col_makes_the_invisible_wall_bolt_transparent() {
        let b = vec![BodyRef::prim("p_8ed", 0)];
        let p = props(&[("ordnance", "none")]);
        assert_eq!(kept(&b, &p, Layer::Soldier), ["p_8ed"]);
        assert!(kept(&b, &p, Layer::Ordnance).is_empty());
    }

    /// BF1 HOT1 `HOTH_prop_bridge_long` (`assets_baked/bf1_hot1/manifest.ron`):
    /// seven `SoldierCollision` lines and seven `OrdnanceCollision` lines over
    /// the same body list, naming DIFFERENT prims. Each key replaces its own
    /// layer's subset (§2, §8), so `p_move*` is soldier-only and
    /// `p_ordinance*` ordnance-only — the split that makes bolts stop where
    /// soldiers walk through and back.
    #[test]
    fn hoth_bridge_ordinance_prims_are_ordnance_only() {
        let mut prims: Vec<String> = (0..6)
            .map(|i| {
                if i == 0 {
                    "p_move".to_string()
                } else {
                    format!("p_move{i}")
                }
            })
            .collect();
        prims.extend((1..=6).map(|i| format!("p_ordinance{i}")));
        let refs: Vec<BodyRef> = std::iter::once(BodyRef::mesh())
            .chain(prims.iter().map(|n| BodyRef::prim(n, 0)))
            .collect();
        let mut lines = vec![
            ("soldier".to_string(), MESH_BODY.to_string()),
            ("ordnance".to_string(), MESH_BODY.to_string()),
        ];
        for n in &prims {
            let key = if n.starts_with("p_move") {
                "soldier"
            } else {
                "ordnance"
            };
            lines.push((key.to_string(), n.clone()));
        }
        assert_eq!(
            kept(&refs, &lines, Layer::Soldier),
            [
                "collisionmesh",
                "p_move",
                "p_move1",
                "p_move2",
                "p_move3",
                "p_move4",
                "p_move5"
            ]
        );
        assert_eq!(
            kept(&refs, &lines, Layer::Ordnance),
            [
                "collisionmesh",
                "p_ordinance1",
                "p_ordinance2",
                "p_ordinance3",
                "p_ordinance4",
                "p_ordinance5",
                "p_ordinance6"
            ]
        );
        // Per body: `p_ordinance*` ordnance-only, `p_move*` soldier-only.
        let s = layer_bodies(&refs, &lines, Layer::Soldier);
        let o = layer_bodies(&refs, &lines, Layer::Ordnance);
        for (i, r) in refs.iter().enumerate() {
            if r.name.starts_with("p_ordinance") {
                assert!(!s[i] && o[i], "{} is ordnance-only", r.name);
            } else if r.name.starts_with("p_move") {
                assert!(s[i] && !o[i], "{} is soldier-only", r.name);
            } else {
                assert!(s[i] && o[i], "{} is in both", r.name);
            }
        }
    }

    /// BF2 POL1 `pol1_prop_hanger_shield`: `SoldierCollision = CLEAR` empties
    /// the subset outright — the sentinel 0x5C6E1222 the RE resolved.
    #[test]
    fn clear_and_none_empty_the_subset() {
        let b = bodies(&[("p_shield", 0)]);
        // The bake lowercases property values, so "CLEAR" arrives as "clear".
        for sentinel in ["clear", "none"] {
            let p = props(&[("soldier", sentinel)]);
            assert!(kept(&b, &p, Layer::Soldier).is_empty(), "{sentinel}");
            // Other layers are untouched.
            assert_eq!(kept(&b, &p, Layer::Vehicle).len(), 2, "{sentinel}");
        }
    }

    /// BF2 MYG1 `myg1_bldg_large_platform` / `_small_spiral`: the MASK byte
    /// seeds membership before any property runs. 0x02 is the vehicle-only
    /// box that held MYG1 unit 6; 0x11 is soldier + ordnance; 0 is all five.
    #[test]
    fn the_bf2_mask_byte_seeds_the_subsets() {
        let b = bodies(&[("p_-v-rob", 0x02), ("p_-so-rob1", 0x11), ("p_plain", 0)]);
        assert_eq!(
            kept(&b, &[], Layer::Soldier),
            ["collisionmesh", "p_-so-rob1", "p_plain"]
        );
        assert_eq!(
            kept(&b, &[], Layer::Vehicle),
            ["collisionmesh", "p_-v-rob", "p_plain"]
        );
        assert_eq!(
            kept(&b, &[], Layer::Ordnance),
            ["collisionmesh", "p_-so-rob1", "p_plain"]
        );
    }

    /// A line naming a body the model does not have is the engine's
    /// `idx < 0`: no write at all, so the subset is left as it was.
    #[test]
    fn a_line_naming_an_unknown_body_changes_nothing() {
        let b = bodies(&[("p_cube", 0)]);
        let p = props(&[("soldier", "p_not_here")]);
        assert_eq!(kept(&b, &p, Layer::Soldier), ["collisionmesh", "p_cube"]);
        // ... and it does not swallow a later line either.
        let p = props(&[("soldier", "p_not_here"), ("soldier", "p_cube")]);
        assert_eq!(kept(&b, &p, Layer::Soldier), ["p_cube"]);
    }

    /// A property for another layer never touches this one (§8).
    #[test]
    fn another_layers_lines_are_inert_here() {
        let b = bodies(&[("p_cube", 0)]);
        let p = props(&[("vehicle", "p_cube"), ("targetable", MESH_BODY)]);
        assert_eq!(kept(&b, &p, Layer::Soldier), ["collisionmesh", "p_cube"]);
    }

    #[test]
    fn targetable_bodies_start_empty_and_obey_authored_clear_and_accumulation() {
        let b = bodies(&[("p_cube", 0), ("p_masked", 0x1f)]);
        assert!(kept(&b, &[], Layer::Targetable).is_empty());
        let p = props(&[("targetable", MESH_BODY), ("targetable", "p_cube")]);
        assert_eq!(kept(&b, &p, Layer::Targetable), [MESH_BODY, "p_cube"]);
        let p = props(&[
            ("targetable", MESH_BODY),
            ("targetable", "clear"),
            ("targetable", "p_masked"),
        ]);
        assert_eq!(kept(&b, &p, Layer::Targetable), ["p_masked"]);
    }
}
