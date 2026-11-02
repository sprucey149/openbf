use super::*;

fn in_polygon(polygon: &[[f32; 2]], x: f32, z: f32) -> bool {
    let mut inside = false;
    for (a, b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        let cross =
            (x - a[0]) as f64 * (b[1] - a[1]) as f64 - (z - a[1]) as f64 * (b[0] - a[0]) as f64;
        if cross.abs() <= 1e-7
            && x >= a[0].min(b[0])
            && x <= a[0].max(b[0])
            && z >= a[1].min(b[1])
            && z <= a[1].max(b[1])
        {
            return true;
        }
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// Does segment a-b reach the square of half-width `half` around (x, z)?
fn touches_box(a: [f32; 2], b: [f32; 2], x: f32, z: f32, half: f32) -> bool {
    let (mut low, mut high) = (0.0f64, 1.0f64);
    for (start, end, centre) in [(a[0], b[0], x), (a[1], b[1], z)] {
        let (start, delta) = (start as f64, end as f64 - start as f64);
        let near = centre as f64 - half as f64 - start;
        let far = centre as f64 + half as f64 - start;
        if delta == 0.0 {
            if near > 0.0 || far < 0.0 {
                return false;
            }
            continue;
        }
        let (t0, t1) = (near / delta, far / delta);
        low = low.max(t0.min(t1));
        high = high.min(t0.max(t1));
        if low > high {
            return false;
        }
    }
    true
}

/// A sample playable ground can read: inside the polygon, or a corner of a cell
/// it reaches. A height inside the line interpolates all four corners of its cell.
fn supports_play(polygon: &[[f32; 2]], x: f32, z: f32, unit: f32) -> bool {
    in_polygon(polygon, x, z)
        || polygon
            .iter()
            .zip(polygon.iter().cycle().skip(1))
            .take(polygon.len())
            .any(|(a, b)| touches_box(*a, *b, x, z, unit))
}

/// How far past the boundary a query from inside it can read terrain: the hover gun's aim ray.
const HEIGHT_REACH: f32 = 512.0;

/// Apply only approved samples playable ground can read, and fills that a query from inside the bounds can read.
/// Coordinates are world X/Z so a larger donor grid cannot shift the correction.
/// A smaller canonical query grid is returned separately: changing the render
/// grid would alter art, while leaving its offsets changes float32 interpolation.
pub fn apply_terrain(
    path: impl AsRef<Path>,
    terrain: &mut mungefs::terrain::Terrain,
) -> Result<Option<u32>> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
    let Some((_, file)) = correction(path, &bytes)? else {
        return Ok(None);
    };
    let matches: Vec<_> = file
        .terrain
        .iter()
        .filter(|t| t.at.name.eq_ignore_ascii_case(&terrain.name))
        .collect();
    if matches.len() > 1 {
        bail!("ambiguous terrain correction identity");
    }
    if let Some(t) = matches.first() {
        apply(t, terrain)?;
        return Ok((t.grid_size != terrain.info.grid_size).then_some(t.grid_size));
    }
    Ok(None)
}

pub(super) fn apply(t: &Terrain, terrain: &mut mungefs::terrain::Terrain) -> Result<()> {
    if !terrain.info.grid_unit.is_finite()
        || terrain.info.grid_unit <= 0.0
        || terrain.info.grid_unit != t.grid_unit
        || t.grid_size < 2
        || t.grid_size > terrain.info.grid_size
        || !(terrain.info.grid_size - t.grid_size).is_multiple_of(2)
        || terrain.info.height_scale != t.height_scale
    {
        bail!("terrain correction grid/height quantization does not match the source");
    }
    let n = terrain.info.grid_size as usize;
    let mut changes = Vec::new();
    let mut set = |x: f32, z: f32, y: f32| -> Result<()> {
        if ![x, y, z].iter().all(|v| v.is_finite()) {
            bail!("terrain height correction is not finite");
        }
        // The parser's row-major grid follows the exact inverse of the world
        // coordinate convention used by terrain_glb.
        let fx = x / terrain.info.grid_unit + n as f32 / 2.0;
        let fz = z / terrain.info.grid_unit + n as f32 / 2.0 - 1.0;
        if fx.fract() != 0.0
            || fz.fract() != 0.0
            || fx < 0.0
            || fz < 0.0
            || fx >= n as f32
            || fz >= n as f32
        {
            bail!("terrain height correction is not a source grid sample");
        }
        let raw = (y / terrain.info.height_scale).round();
        if raw < i32::MIN as f32 || raw >= i32::MAX as f32 || raw * terrain.info.height_scale != y {
            bail!("terrain height correction cannot be represented exactly");
        }
        changes.push((fz as usize * n + fx as usize, raw as i32));
        Ok(())
    };
    for &[x, z, y] in &t.height_samples {
        if !t
            .boundaries
            .iter()
            .any(|p| p.len() >= 3 && supports_play(p, x, z, t.grid_unit))
        {
            bail!("refusing a terrain height correction outside playable bounds");
        }
        set(x, z, y)?;
    }
    for &[x0, z0, columns, rows, y] in &t.height_fills {
        for row in 0..rows as u32 {
            for column in 0..columns as u32 {
                let x = x0 + column as f32 * t.grid_unit;
                let z = z0 + row as f32 * t.grid_unit;
                if !t
                    .boundaries
                    .iter()
                    .any(|p| p.len() >= 3 && supports_play(p, x, z, HEIGHT_REACH))
                {
                    bail!("refusing a terrain height fill past what a query from inside the bounds can read");
                }
                set(x, z, y)?;
            }
        }
    }
    // Validate every record before mutating the grid.
    for (index, raw) in changes {
        terrain.raw[index] = raw;
    }
    Ok(())
}
