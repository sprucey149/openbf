//! Restore the PC Cloud City ramp's missing rails from the donor's own bars.
//!
//! The map already supplies the same three-bar mesh as a separate prop. Copy
//! its segment, keep the donor material/UVs, stretch its span and move it below
//! the ramp. The recipe stores only a transform and raw dependency hashes;
//! collision, placements, PS2 outputs and gameplay model headers stay intact.

use anyhow::{anyhow, ensure, Result};
use glam::{Mat3, Vec3};
use mungefs::model::Model;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use super::render_reassembly::Reassembled;

const RAMP: &str = "bes2_bldg_skyway_ramp";
const RAMP_SHA256: &str = "e48b495213113f8df57d7682b7aef2b4088a766cb5518b1a80afa7f0cef38710";
const BARS: &str = "bes2_bldg_bars";
const BARS_SHA256: &str = "5dfa7204d417c9c0161be799f5de10b84c0d5ba7bbbd4947083b609f8068bd70";

/// Called only for a donor map, after the ordinary models have been posed.
pub(super) fn models(original: &[u8], models: &HashMap<String, Model>) -> Result<Vec<Reassembled>> {
    let chunks = mungefs::content_chunks(mungefs::open(original)?)?;
    let mut ramp = false;
    let mut bars = false;
    for chunk in chunks {
        if chunk.tag != *b"modl" {
            continue;
        }
        let Some(name) = chunk.child_string(b"NAME") else {
            continue;
        };
        let expected = if name.eq_ignore_ascii_case(RAMP) {
            RAMP_SHA256
        } else if name.eq_ignore_ascii_case(BARS) {
            BARS_SHA256
        } else {
            continue;
        };
        let matched = format!("{:x}", Sha256::digest(chunk.payload)) == expected;
        if name.eq_ignore_ascii_case(RAMP) {
            ramp |= matched;
        } else {
            bars |= matched;
        }
    }
    if !ramp {
        return Ok(Vec::new());
    }
    ensure!(bars, "Cloud City rail donor is missing or changed");
    let base = models
        .get(RAMP)
        .ok_or_else(|| anyhow!("Cloud City ramp is missing after posing"))?;
    let donor = models
        .get(BARS)
        .ok_or_else(|| anyhow!("Cloud City bars are missing after posing"))?;
    ensure!(
        !base.bone_space && !donor.bone_space && base.segments.len() == 2,
        "Cloud City ramp pose or segmentation changed"
    );
    ensure!(
        donor.segments.len() == 1
            && donor.segments[0].positions.len() == 72
            && donor.segments[0].triangles.len() == 36,
        "Cloud City bars segmentation changed"
    );
    let mut model = base.clone();
    let mut lighting: Vec<_> = base
        .segments
        .iter()
        .map(|segment| crate::glb::light_path(base, segment))
        .collect();
    let mut segment = donor.segments[0].clone();
    lighting.push(crate::glb::light_path(donor, &segment));
    // The donor bars already match the native rail cross-section and spacing.
    // Stretch their existing 5.19 m span to 14.02 m, retaining the complete
    // three-bar topology. The unchanged collision supplies no new vertices.
    let matrix = Mat3::from_diagonal(Vec3::new(1., 1., 2.700_41));
    let translation = Vec3::new(30.795_53, -12.129_23, -4.029_935);
    let normal = matrix.inverse().transpose();
    for position in &mut segment.positions {
        *position = (matrix * Vec3::from(*position) + translation).to_array();
    }
    for value in &mut segment.normals {
        *value = (normal * Vec3::from(*value)).normalize_or_zero().to_array();
    }
    model.segments.push(segment);
    eprintln!("render alignment: restored Cloud City ramp rails from donor bars");
    Ok(vec![Reassembled { model, lighting }])
}
