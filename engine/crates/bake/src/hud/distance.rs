//! Offline signed distances from the player's glyph coverage. Coordinates of
//! the 50% contour retain sub-texel antialiasing; no installed/system font is used.

/// Piecewise-linear half-coverage contour, including the transparent border.
fn contour(alpha: &[u8], w: usize, h: usize) -> Vec<([f32; 2], [f32; 2])> {
    let at = |x: isize, y: isize| {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            0.0
        } else {
            f32::from(alpha[y as usize * w + x as usize]) / 255.0
        }
    };
    let mut edges = Vec::new();
    for y in -1..h as isize {
        for x in -1..w as isize {
            let corners = [
                [x as f32 + 0.5, y as f32 + 0.5],
                [x as f32 + 1.5, y as f32 + 0.5],
                [x as f32 + 1.5, y as f32 + 1.5],
                [x as f32 + 0.5, y as f32 + 1.5],
            ];
            let values = [at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1)];
            let mut cuts = [[0.0; 2]; 4];
            let mut count = 0;
            for i in 0..4 {
                let j = (i + 1) % 4;
                if (values[i] >= 0.5) != (values[j] >= 0.5) {
                    let t = (0.5 - values[i]) / (values[j] - values[i]);
                    cuts[count] = [
                        corners[i][0] + t * (corners[j][0] - corners[i][0]),
                        corners[i][1] + t * (corners[j][1] - corners[i][1]),
                    ];
                    count += 1;
                }
            }
            match count {
                2 => edges.push((cuts[0], cuts[1])),
                4 => {
                    // Resolve the saddle using the bilinear cell centre.
                    if (values.iter().sum::<f32>() >= 2.0) == (values[0] >= 0.5) {
                        edges.extend([(cuts[0], cuts[1]), (cuts[2], cuts[3])]);
                    } else {
                        edges.extend([(cuts[0], cuts[3]), (cuts[1], cuts[2])]);
                    }
                }
                _ => {}
            }
        }
    }
    edges
}

fn coverage(alpha: &[u8], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let (x, y) = (x - 0.5, y - 0.5);
    let (ix, iy) = (x.floor() as isize, y.floor() as isize);
    let at = |xx: isize, yy: isize| {
        if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
            0.0
        } else {
            f32::from(alpha[yy as usize * w + xx as usize]) / 255.0
        }
    };
    let (fx, fy) = (x - x.floor(), y - y.floor());
    (at(ix, iy) * (1.0 - fx) + at(ix + 1, iy) * fx) * (1.0 - fy)
        + (at(ix, iy + 1) * (1.0 - fx) + at(ix + 1, iy + 1) * fx) * fy
}

/// Two distance samples per source texel, four source texels of transparent
/// padding and distance range. These are encoding parameters, not layout.
pub(super) const SCALE: usize = 2;
pub(super) const PAD: usize = 4;
pub(super) fn field(alpha: &[u8], w: usize, h: usize) -> Option<(Vec<u8>, usize, usize)> {
    let edges = contour(alpha, w, h);
    if edges.is_empty() {
        return None;
    }
    let (dw, dh) = ((w + PAD * 2) * SCALE, (h + PAD * 2) * SCALE);
    let mut nearest = vec![(PAD * PAD) as f32; dw * dh];
    let to_sample = |v: f32| (v + PAD as f32) * SCALE as f32 - 0.5;
    let sample_at = |i: usize| (i as f32 + 0.5) / SCALE as f32 - PAD as f32;
    // A segment outside the encoded distance range cannot affect a sample.
    // Visit only the sample rectangle around each segment.
    let bounds = |a: f32, b: f32, limit: usize| {
        let first = to_sample(a.min(b) - PAD as f32).ceil() as usize;
        let last = to_sample(a.max(b) + PAD as f32).floor() as usize;
        first.min(limit)..last.saturating_add(1).min(limit)
    };
    for (a, b) in edges {
        let xs = bounds(a[0], b[0], dw);
        let ys = bounds(a[1], b[1], dh);
        let d = [b[0] - a[0], b[1] - a[1]];
        let len_sq = d[0] * d[0] + d[1] * d[1];
        for y in ys {
            let py = sample_at(y) - a[1];
            for x in xs.clone() {
                let px = sample_at(x) - a[0];
                let t = ((px * d[0] + py * d[1]) / len_sq).clamp(0.0, 1.0);
                let q = [px - t * d[0], py - t * d[1]];
                let at = &mut nearest[y * dw + x];
                *at = at.min(q[0] * q[0] + q[1] * q[1]);
            }
        }
    }
    let mut rgba = vec![255; dw * dh * 4];
    for y in 0..dh {
        for x in 0..dw {
            let p = [sample_at(x), sample_at(y)];
            let sign = if coverage(alpha, w, h, p[0], p[1]) >= 0.5 {
                1.0
            } else {
                -1.0
            };
            rgba[(y * dw + x) * 4 + 3] =
                ((0.5 + sign * nearest[y * dw + x].sqrt() / (2 * PAD) as f32) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
        }
    }
    Some((rgba, dw, dh))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_preserves_half_coverage_and_transparent_padding() {
        let (rgba, w, h) = field(&[255; 16], 4, 4).unwrap();
        let a = |x: usize, y: usize| rgba[(y * w + x) * 4 + 3];
        assert!(a(w / 2, h / 2) > 128);
        assert!(a(0, 0) < 10);
        // Equal distances on opposite sides of the straight edge.
        assert!((i32::from(a(7, 12)) + i32::from(a(8, 12)) - 255).abs() <= 1);
        assert!(field(&[0; 16], 4, 4).is_none());
    }
}
