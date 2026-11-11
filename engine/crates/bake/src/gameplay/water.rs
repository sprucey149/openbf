//! Dry cells for a fingerprinted donor whose authored grid is entirely wet.

use anyhow::{ensure, Result};
use manifest::ManifestWetMask;
use mungefs::water::Water;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WaterMaskCorrection {
    grid: u32,
    dry_runs: Vec<[u32; 3]>,
    sha256: String,
}

impl WaterMaskCorrection {
    pub(crate) fn apply(&self, source: &Water) -> Result<ManifestWetMask> {
        let info = &source.info;
        ensure!(
            info.grid > 0
                && self.grid <= 128
                && self.grid > info.grid
                && self.grid.is_multiple_of(info.grid)
                && source.cells.len() == (info.grid as usize).pow(2)
                && source.cells.iter().all(|&cell| cell != 0)
                && info.cell_size.is_finite()
                && info.cell_size > 0.0
                && info.half_extent.iter().all(|&half| {
                    half.is_finite()
                        && half > 0.0
                        && half * 2.0 == info.grid as f32 * info.cell_size
                }),
            "expected an aligned, entirely wet donor grid"
        );
        ensure!(
            !self.dry_runs.is_empty() && self.dry_runs.len() <= 128,
            "invalid dry water run count"
        );
        let width = self.grid as usize;
        let mut cells = vec![1; width * width];
        let mut next = (0, 0);
        let mut dry = 0;
        for &[z, x, len] in &self.dry_runs {
            ensure!(
                z < self.grid && x < self.grid && len > 0 && len <= self.grid - x && (z, x) >= next,
                "dry water runs must be in bounds, ordered and disjoint"
            );
            dry += len;
            ensure!(dry <= 256, "too many corrected dry water cells");
            let start = z as usize * width + x as usize;
            cells[start..start + len as usize].fill(0);
            next = (z, x + len);
        }
        ensure!(
            super::digest(&cells) == self.sha256,
            "corrected water mask fingerprint differs"
        );
        Ok(ManifestWetMask::from_nibbles(
            self.grid,
            info.cell_size / (self.grid / info.grid) as f32,
            info.half_extent,
            &cells,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mungefs::water::{WaterInfo, WaterLayer};

    #[test]
    fn water_mask_correction_refines_only_the_gameplay_cells() {
        let source = Water {
            info: WaterInfo {
                grid: 2,
                cell_size: 4.0,
                half_extent: [4.0; 2],
                unknown_1: 2,
                unknown_5: 1,
            },
            layer: WaterLayer {
                texture: String::new(),
                unknown: 10,
                height: -2.0,
                uv_velocity: [0.0; 2],
                uv_tile: [1.0; 2],
                color: [255; 4],
            },
            cells: vec![1; 4],
            patches: Vec::new(),
        };
        let mut cells = [1; 16];
        cells[5..7].fill(0);
        let mut correction = WaterMaskCorrection {
            grid: 4,
            dry_runs: vec![[1, 1, 2]],
            sha256: super::super::digest(&cells),
        };
        let mask = correction.apply(&source).unwrap();
        assert_eq!(mask.cells(), cells);
        assert_eq!(mask.cell_size, 2.0);
        assert_eq!(mask.half_extent, source.info.half_extent);
        assert_eq!(source.cells, [1; 4]);

        correction.dry_runs.push([1, 2, 1]);
        assert!(correction
            .apply(&source)
            .unwrap_err()
            .to_string()
            .contains("disjoint"));
        correction.dry_runs = vec![[1, 2, 1]];
        assert!(correction
            .apply(&source)
            .unwrap_err()
            .to_string()
            .contains("fingerprint"));
        correction.dry_runs = vec![[1, 4, 1]];
        assert!(correction
            .apply(&source)
            .unwrap_err()
            .to_string()
            .contains("in bounds"));
    }
}
