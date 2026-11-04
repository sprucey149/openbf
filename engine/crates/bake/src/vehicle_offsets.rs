//! Fingerprinted displacements of donor vehicle artwork after bind posing.
//! Only drawing vertices move. Canonical collision, skeletons, joints and
//! hardpoints stay unchanged; PS2 input never selects a recipe.
use crate::map::render_offsets::{Offsets, Recipe};
use anyhow::Result;

/// An unrecognized donor vehicle (a patched, modded or regional copy) keeps
/// its drawing unfitted rather than failing the whole side bake, like the
/// sibling `vehicle_render` instructions.
pub(crate) fn of(original: Option<&[u8]>) -> Result<Offsets> {
    match Offsets::select(original, RECIPES) {
        Ok(offsets) => Ok(offsets),
        Err(e) => {
            eprintln!("vehicle art: {e:#}; leaving donor drawing unfitted");
            Offsets::select(None, RECIPES)
        }
    }
}

include!("vehicle_offsets_table.rs");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unrecognized_donor_vehicle_bakes_unfitted_instead_of_failing() {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = tag.to_vec();
            bytes.extend((payload.len() as u32).to_le_bytes());
            bytes.extend(payload);
            bytes.resize(bytes.len().next_multiple_of(4), 0);
            bytes
        }
        let payload = chunk(b"NAME", b"cis_tread_hailfire\0");
        let bytes = chunk(b"ucfb", &chunk(b"modl", &payload));
        assert!(of(Some(&bytes)).unwrap().is_empty());
    }

    #[test]
    fn table_is_finite_in_range_and_unique_per_fingerprint() {
        let mut identities = std::collections::HashSet::new();
        for recipe in RECIPES {
            assert!(identities.insert((recipe.name, recipe.input)));
            assert_eq!(recipe.input.len(), 64);
            if recipe.name == "cis_tread_hailfire" {
                assert_eq!(
                    &recipe.shapes[recipe.shapes.len() - 2..],
                    &[(756, 252), (756, 252)]
                );
            }
            let mut vertices = std::collections::HashSet::new();
            for &(p, v, d) in recipe.moves {
                assert!(vertices.insert((p, v)));
                assert!((v as u32) < recipe.shapes[p as usize].0);
                assert!(d.iter().all(|n| n.is_finite()));
                if recipe.name == "cis_tread_hailfire" && p as usize >= recipe.shapes.len() - 2 {
                    assert_eq!(d[2], 0., "casing fill must stay behind donor mouths");
                }
            }
            for &(p, f) in recipe.removals {
                assert!((f as u32) < recipe.shapes[p as usize].1);
            }
        }
    }
}
