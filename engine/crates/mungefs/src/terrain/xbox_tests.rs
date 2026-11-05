use super::*;

/// Real data: BF1 GEO1 and HOT1's Xbox terrain keeps the PS2's heights and
/// layer paint, its lightmap and hue read in the same orientation, the
/// cutter patches ship the same vertices, and the far sheet the same land.
#[test]
fn xbox_terrain_matches_the_ps2() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
    for (xbox, ps2) in [
        (
            "xbox_bf1/geo/geo1.lvl",
            "bf1/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL",
        ),
        (
            "xbox_bf1/hot/hot1.lvl",
            "bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL",
        ),
    ] {
        let (Ok(xb), Ok(pb)) = (
            std::fs::read(format!("{root}/{xbox}")),
            std::fs::read(format!("{root}/{ps2}")),
        ) else {
            eprintln!("SKIP: {xbox} not present");
            return;
        };
        let parse = |buf: &[u8]| {
            let c = crate::content_chunks(crate::open(buf).unwrap())
                .unwrap()
                .into_iter()
                .find(|c| &c.tag == b"tern")
                .unwrap();
            Terrain::parse(&c).unwrap()
        };
        let (x, p) = (parse(&xb), parse(&pb));
        // BF1 HOT1's heights really differ on the two discs; GEO1's do not.
        let height_off = x.raw.iter().zip(&p.raw).filter(|(a, b)| a != b).count();
        eprintln!("{xbox}: heights off {height_off}/{}", x.raw.len());
        if xbox.contains("geo") {
            assert_eq!(height_off, 0);
        }
        assert_eq!(x.patch_layer_masks.len(), p.patch_layer_masks.len());
        assert_eq!(x.patch_meshes.len(), p.patch_meshes.len());

        // Lightmap: Xbox and PS2 are the same painting, so the transposed
        // read must correlate far better than the raw one.
        let (xl, pl) = (x.lumi.as_ref().unwrap(), p.lumi.as_ref().unwrap());
        let n = x.info.grid_size as usize;
        let diff = |a: &[u8], b: &[u8]| -> f64 {
            a.iter()
                .zip(b)
                .map(|(u, v)| f64::from(u.abs_diff(*v)))
                .sum::<f64>()
                / a.len() as f64
        };
        let pearson = |a: &[u8], b: &[u8]| -> f64 {
            let m = |c: &[u8]| c.iter().map(|&v| f64::from(v)).sum::<f64>() / c.len() as f64;
            let (ma, mb) = (m(a), m(b));
            let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
            for (u, v) in a.iter().zip(b) {
                let (u, v) = (f64::from(*u) - ma, f64::from(*v) - mb);
                ab += u * v;
                aa += u * u;
                bb += v * v;
            }
            ab / (aa * bb).sqrt().max(1e-9)
        };
        let swapped = transpose(xl, n);
        let (straight, across) = (pearson(xl, pl), pearson(&swapped, pl));
        eprintln!(
            "{xbox}: lumi r {straight:.3}, transposed {across:.3}, mean diff {:.1}",
            diff(xl, pl)
        );
        assert!(straight > across, "{straight} vs {across}");

        // Hue: per-vertex Xbox colour against the same PS2 COLR cell.
        let (xc, pc) = (x.colr.as_ref().unwrap(), p.colr.as_ref().unwrap());
        let green = |c: &[[u8; 3]]| c.iter().map(|v| v[1]).collect::<Vec<u8>>();
        let right = pearson(&green(xc), &green(pc));
        let wrong = pearson(&green(&transpose(xc, n)), &green(pc));
        let own = pearson(&green(xc), xl);
        eprintln!("{xbox}: colr r {right:.3}, transposed {wrong:.3}, against own lumi {own:.3}");
        assert!(right > wrong, "{right} vs {wrong}");

        // Layer paint: over every patch where both discs name the same
        // layers, the nibbles agree closely (the Xbox's are re-derived).
        let (mut same, mut near) = (0usize, 0usize);
        for i in 0..x.patch_layer_masks.len() {
            if x.patch_layer_masks[i] != p.patch_layer_masks[i]
                || x.patch_meshes[i].is_some()
                || p.patch_meshes[i].is_some()
            {
                continue;
            }
            let (a, b) = (&x.patch_alpha[i], &p.patch_alpha[i]);
            assert_eq!(a.len(), b.len(), "patch {i}");
            for (u, v) in a.iter().zip(b) {
                same += 1;
                let v = if *v == 0xf { 0 } else { *v };
                near += usize::from(u.abs_diff(v) <= 2);
            }
        }
        eprintln!("{xbox}: nibbles within 2 steps {near}/{same}");
        assert!(near * 100 >= same * 80, "{near}/{same}");

        // Cutter patches: the same vertices and triangle count, on the
        // patches both discs agree are the same height range.
        let (mut both, mut alike) = (0usize, 0usize);
        for i in 0..x.patch_meshes.len() {
            if let (Some(a), Some(b)) = (&x.patch_meshes[i], &p.patch_meshes[i]) {
                if a.height_min != b.height_min || a.shift != b.shift {
                    continue;
                }
                both += 1;
                let tris = |s: &[u16]| crate::model::unstitch_strips(s).len();
                let (ta, tb) = (tris(&a.strips), tris(&b.strips));
                alike += usize::from(
                    a.positions.len() == b.positions.len() && ta.abs_diff(tb) * 10 <= tb.max(10),
                );
            }
        }
        eprintln!("{xbox}: cutter patches alike {alike}/{both}");
        assert!(alike * 10 >= both * 8, "{alike}/{both}");

        // Far sheet: same grid, and every patch covers the same land.
        let (xs, ps) = (x.lowres.as_ref().unwrap(), p.lowres.as_ref().unwrap());
        assert_eq!((xs.grid, xs.resolution), (ps.grid, ps.resolution));
        assert_eq!(xs.patches.len(), ps.patches.len());
        let y_mean = |s: &LowResSheet| {
            let (mut sum, mut c) = (0.0f64, 0usize);
            for v in s.patches.iter().flat_map(|p| &p.positions) {
                sum += f64::from(v[1]);
                c += 1;
            }
            sum / c.max(1) as f64
        };
        assert!((y_mean(xs) - y_mean(ps)).abs() < 2.0, "{xbox} sheet height");
        assert!(xs.triangle_count() > 0);
    }
}

/// One `PCHS` of `count` cutter patches, each listing `k` layers.
fn cutter_pchs(count: usize, k: u8) -> Vec<u8> {
    let mut info = vec![k];
    info.extend(vec![0u8; k as usize + 6]);
    let mut vbuf = 0u32.to_le_bytes().to_vec();
    vbuf.extend(16u32.to_le_bytes());
    vbuf.extend(0x58a2u32.to_le_bytes());
    let mut ptch = crate::build_chunk(b"INFO", &info);
    ptch.extend(crate::build_chunk(b"VBUF", &vbuf));
    ptch.extend(crate::build_chunk(b"IBUF", &0u32.to_le_bytes()));
    let mut pchs = crate::build_chunk(b"COMN", &[]);
    for _ in 0..count {
        pchs.extend(crate::build_chunk(b"PTCH", &ptch));
    }
    crate::build_chunk(b"PCHS", &pchs)
}

#[test]
fn xbox_patches_are_bounded_by_the_grid() {
    let parse = |buf: &[u8], n, edge| {
        let c = crate::chunks(buf).next().unwrap().unwrap();
        parse_xbox_patches(&c, n, edge).map(|x| x.masks.len())
    };
    let four = cutter_pchs(4, 1);
    assert_eq!(parse(&four, 16, 8).unwrap(), 4);
    assert!(matches!(
        parse(&cutter_pchs(5, 1), 16, 8),
        Err(UcfbError::Malformed(_))
    ));
    assert!(matches!(
        parse(&cutter_pchs(1, 17), 16, 8),
        Err(UcfbError::Malformed(_))
    ));
    assert!(parse(&four, 0, 8).is_err());
    assert!(parse(&four, 16, 0).is_err());
}

#[test]
fn xbox_weights_become_over_nibbles() {
    let mut out = Vec::new();
    // Two layers listed as (0, 1): half and half.
    weights_to_nibbles(&[0, 1], 128, 0, &[0, 1], &mut out);
    assert_eq!(out, [OPAQUE_NIBBLE, 7]);
    // Listed out of order: the listed-first layer takes what is left.
    out.clear();
    weights_to_nibbles(&[1, 0], 255, 0, &[0, 1], &mut out);
    assert_eq!(out, [OPAQUE_NIBBLE, 0]);
    // Three layers: all weight on the top one.
    out.clear();
    weights_to_nibbles(&[0, 1, 2], 0, 255, &[0, 1, 2], &mut out);
    assert_eq!(out, [OPAQUE_NIBBLE, 0, 14]);
}
