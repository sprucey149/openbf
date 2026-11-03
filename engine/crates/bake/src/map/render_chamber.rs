//! Last-resort donor-textured drawing for the Xbox Bespin chamber annex.
//!
//! Its two lower side assemblies are displaced by roughly 2.58 and 2.93 m.
//! The same-copy LOWD is also displaced; the other chamber resources do not
//! contain this annex. Donor translations improve its wall but break the
//! connecting faces. Keep the matching central/upper donor artwork and draw
//! only the annex from the player's unchanged canonical contact surfaces.

use anyhow::{ensure, Result};
use glam::{Mat3, Vec3};
use mungefs::{
    model::{Aabb, Model},
    Chunk,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const INPUTS: [(&str, &str); 2] = [
    (
        "bes2_bldg_chamber",
        "31d935bc48ab254c0016f9a49b87942eb15c28798214b34fff157b517d5dc86d",
    ),
    (
        "bes2_bldg_chamberlowd",
        "76a5f483862ba57280040c4dc00e58c2b973018c4de6610b43b6f98a7254f56a",
    ),
];
// The annex joins the central chamber at its 27.5 m west partition. This is
// a clipping instruction, not stored replacement geometry.
const ANNEX_END: f32 = -27.5;

pub(super) struct Chamber(HashSet<String>);

impl Chamber {
    pub(super) fn of(original: Option<&[u8]>) -> Result<Self> {
        let mut selected = HashSet::new();
        if let Some(original) = original {
            for chunk in mungefs::content_chunks(mungefs::open(original)?)? {
                if chunk.tag != *b"modl" {
                    continue;
                }
                let Some(name) = chunk.child_string(b"NAME") else {
                    continue;
                };
                let Some((_, expected)) =
                    INPUTS.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name))
                else {
                    continue;
                };
                if format!("{:x}", Sha256::digest(chunk.payload)) == *expected {
                    selected.insert(name.to_ascii_lowercase());
                }
            }
        }
        Ok(Self(selected))
    }

    pub(super) fn apply(
        &self,
        artwork: &mut Model,
        items: &[Chunk<'_>],
        alpha: &[crate::alpha::SegmentAlpha],
    ) -> Result<bool> {
        if !self.0.contains(&artwork.name.to_ascii_lowercase()) {
            return Ok(false);
        }
        ensure!(!artwork.bone_space, "render chamber: unposed donor");
        // The existing producer refuses unequal soldier/ordnance domains,
        // non-default class policies, unsupported bodies and nonopaque BF1.
        // The rejected geometric translations still place the donor's texture
        // projector near its canonical walls. This copy is never drawn: only
        // its own material/UV fields are sampled by the solid producer.
        let mut projector = artwork.clone();
        for segment in &mut projector.segments {
            for point in &mut segment.positions {
                if point[0] < ANNEX_END && point[1] < 18.1 {
                    point[2] += if point[2] < 0. { 2.5767 } else { -2.925_819_4 };
                }
            }
        }
        let solid = super::render_conform::Conformation::chamber_surface(&projector, items, alpha)?;
        let points = artwork
            .segments
            .iter()
            .chain(&solid.segments)
            .flat_map(|s| &s.positions);
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for point in points {
            let point = Vec3::from(*point);
            ensure!(point.is_finite(), "render chamber: nonfinite surface");
            low = low.min(point);
            high = high.max(point);
        }
        ensure!(
            low.x < ANNEX_END && high.x > ANNEX_END,
            "render chamber: changed annex bounds"
        );
        let mut result = artwork.clone();
        result.segments.clear();
        let retained = Aabb {
            min: [ANNEX_END, low.y, low.z],
            max: high.to_array(),
        };
        let annex = Aabb {
            min: low.to_array(),
            max: [ANNEX_END, high.y, high.z],
        };
        for (model, bounds) in [(artwork as &Model, retained), (&solid, annex)] {
            for segment in &model.segments {
                if let Some(segment) =
                    super::render_recovery::clipped(segment, Mat3::IDENTITY, Vec3::ZERO, bounds)
                {
                    result.segments.push(segment);
                }
            }
        }
        ensure!(!result.segments.is_empty(), "render chamber: empty result");
        *artwork = result;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_and_unrecognized_donor_bytes_never_select_the_annex() {
        assert!(Chamber::of(None).unwrap().0.is_empty());
        let name = b"bes2_bldg_chamber\0";
        let mut body = b"NAME".to_vec();
        body.extend((name.len() as u32).to_le_bytes());
        body.extend(name);
        while !body.len().is_multiple_of(4) {
            body.push(0);
        }
        let mut model = b"modl".to_vec();
        model.extend((body.len() as u32).to_le_bytes());
        model.extend(body);
        let mut bytes = b"ucfb".to_vec();
        bytes.extend((model.len() as u32).to_le_bytes());
        bytes.extend(model);
        assert!(Chamber::of(Some(&bytes)).unwrap().0.is_empty());
    }
}
