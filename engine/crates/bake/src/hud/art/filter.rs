//! Reconstruct small sliced UI sheets on a denser grid. UVs continue to
//! address the original sheet. Monotone interpolation keeps the nearest
//! four source texels' range and avoids ringing beside transparent edges.

#[derive(Clone, Copy)]
struct Axis {
    taps: [usize; 4],
    weights: [f32; 4],
}

fn axis(len: usize, scale: usize) -> Vec<Axis> {
    (0..len * scale)
        .map(|i| {
            let src = (i as f32 + 0.5) / scale as f32 - 0.5;
            let left = src.floor() as isize;
            let taps = [-1, 0, 1, 2].map(|d| (left + d).clamp(0, len as isize - 1) as usize);
            let t = src - src.floor();
            let (t2, t3) = (t * t, t * t * t);
            let weights = [
                -0.5 * t + t2 - 0.5 * t3,
                1.0 - 2.5 * t2 + 1.5 * t3,
                0.5 * t + 2.0 * t2 - 1.5 * t3,
                -0.5 * t2 + 0.5 * t3,
            ];
            Axis { taps, weights }
        })
        .collect()
}

pub(super) fn upsample(rgba: &[u8], width: usize, height: usize, scale: usize) -> Vec<u8> {
    let source: Vec<_> = rgba
        .chunks_exact(4)
        .map(|p| {
            let a = f32::from(p[3]) / 255.0;
            [
                f32::from(p[0]) * a,
                f32::from(p[1]) * a,
                f32::from(p[2]) * a,
                a,
            ]
        })
        .collect();
    let xs = axis(width, scale);
    let ys = axis(height, scale);
    let mut out = Vec::with_capacity(xs.len() * ys.len() * 4);
    for y in &ys {
        for x in &xs {
            let mut sample = [0.0; 4];
            let mut lo = [f32::INFINITY; 4];
            let mut hi = [f32::NEG_INFINITY; 4];
            for (iy, (sy, wy)) in y.taps.into_iter().zip(y.weights).enumerate() {
                for (ix, (sx, wx)) in x.taps.into_iter().zip(x.weights).enumerate() {
                    let pixel = source[sy * width + sx];
                    let nearby = (1..=2).contains(&ix) && (1..=2).contains(&iy);
                    for c in 0..4 {
                        sample[c] += pixel[c] * wx * wy;
                        if nearby {
                            lo[c] = lo[c].min(pixel[c]);
                            hi[c] = hi[c].max(pixel[c]);
                        }
                    }
                }
            }
            for c in 0..4 {
                sample[c] = sample[c].clamp(lo[c], hi[c]);
            }
            let alpha = (255.0 * sample[3]).round() as u8;
            if alpha == 0 {
                out.extend_from_slice(&[0; 3]);
            } else {
                out.extend(sample[..3].iter().map(|c| (c / sample[3]).round() as u8));
            }
            out.push(alpha);
        }
    }
    pad_transparent_edges(&mut out, xs.len(), ys.len());
    out
}

/// Preserve the visible colour next to a silhouette for straight-alpha GPU
/// filtering. Only an adjacent nonzero-alpha texel can share a bilinear tap.
fn pad_transparent_edges(rgba: &mut [u8], width: usize, height: usize) {
    for y in 0..height {
        for x in 0..width {
            let at = (y * width + x) * 4;
            if rgba[at + 3] != 0 {
                continue;
            }
            let mut nearest = at;
            for sy in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for sx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let from = (sy * width + sx) * 4;
                    if rgba[from + 3] > rgba[nearest + 3] {
                        nearest = from;
                    }
                }
            }
            let rgb = [rgba[nearest], rgba[nearest + 1], rgba[nearest + 2]];
            rgba[at..at + 3].copy_from_slice(&rgb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconstruction_keeps_flat_regions_and_does_not_mix_in_hidden_rgb() {
        let flat = [33, 22, 11, 119];
        assert_eq!(upsample(&flat.repeat(4), 2, 2, 4), flat.repeat(64));

        let mask = [255, 0, 0, 255, 0, 255, 0, 0];
        let pixels = upsample(&mask, 2, 1, 4);
        let row = &pixels[..8 * 4];
        assert_eq!(&row[..4], &mask[..4]);
        assert_eq!(row[31], 0);
        let first_clear = row.chunks_exact(4).find(|p| p[3] == 0).unwrap();
        assert_eq!(first_clear, [255, 0, 0, 0]);
        assert!(row[11] > row[15] && row[15] > row[19] && row[19] > row[23]);
        assert!(row
            .chunks_exact(4)
            .filter(|p| p[3] != 0)
            .all(|p| p[..3] == [255, 0, 0]));
    }
}
