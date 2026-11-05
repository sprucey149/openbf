use super::Terrain;

fn tern_of(path: &str) -> Option<Vec<u8>> {
    std::fs::read(format!(
        "{}/../../../extracted/{path}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .ok()
}

/// Data-gated: the layer header the bake now needs. HOT1 names three snow
/// layers scaled 16/32/64 m per repeat, and every one of its 1024 patches
/// paints layer 0 (`hoth_main_1`) — which is why the terrain bake uses
/// LTEX[0] as its single base layer.
#[test]
fn hot1_layer_header_and_patch_masks() {
    let Some(buf) = tern_of("bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL") else {
        eprintln!("SKIP: HOT1.LVL not present");
        return;
    };
    let inner = crate::open(&buf).unwrap();
    let tern = crate::chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("tern");
    let t = Terrain::parse(&tern).expect("parses");
    assert_eq!(
        t.layer_textures,
        ["hoth_main_1", "hoth_main_2", "hoth_main_4"]
    );
    assert_eq!(&t.layer_scale[..3], &[16.0, 32.0, 64.0]);
    assert!(t.layer_axis[..3].iter().all(|&a| a == 0));
    assert!(t.layer_rotation[..3].iter().all(|&r| r == 0.0));
    assert_eq!(t.patch_layer_masks.len(), 1024);
    assert_eq!(t.patch_layer_masks.iter().filter(|&&m| m == 0).count(), 0);
    assert!(
        t.patch_layer_masks.iter().all(|m| m & 1 != 0),
        "layer 0 is painted in every HOT1 patch"
    );
    assert!(
        t.patch_layer_masks.iter().all(|m| m >> 3 == 0),
        "no mask bit past textureCount = 3"
    );
}

/// Data-gated: the flagged `PTCH` replacement meshes on BF1 HOT1 — the
/// pre-clipped terrain the PS2 draws inside the two outpost rooms
/// (analysis/re-terrain-cutters.md §0.5 and §2). 23 of the 1024 render
/// patches carry one; PTCH 588 is the one under control point 3.
///
/// The dequant is pinned the way the RE proves it: every replacement
/// vertex that lands on a cell corner (POSI x and z multiples of 0x2000 =
/// 65536/8) reproduces the heightfield's own height to under 5 mm.
#[test]
fn hot1_flagged_patches_carry_a_replacement_mesh() {
    let Some(buf) = tern_of("bf1/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL") else {
        eprintln!("SKIP: HOT1.LVL not present");
        return;
    };
    let inner = crate::open(&buf).unwrap();
    let tern = crate::chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("tern");
    let t = Terrain::parse(&tern).expect("parses");
    assert_eq!(t.patch_meshes.len(), t.patch_layer_masks.len());
    assert_eq!(t.patch_grid(), Some((32, 8)));
    let flagged: Vec<usize> = t
        .patch_meshes
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.as_ref().map(|_| i))
        .collect();
    assert_eq!(flagged.len(), 23, "{flagged:?}");
    assert!(flagged.contains(&588) && flagged.contains(&458) && flagged.contains(&490));

    let pm = t.patch_meshes[588].as_ref().unwrap();
    assert_eq!(pm.positions.len(), 123);
    assert_eq!(pm.strips.len(), 235);
    assert_eq!((pm.height_min, pm.shift), (-35, 9));
    assert_eq!(pm.colr.as_ref().map(Vec::len), Some(123));
    let k = t.patch_layer_masks[588].count_ones() as usize;
    assert_eq!(pm.alpha.len(), k * 123);

    // Every strip index is inside POSI on every flagged patch, so the
    // strips can be unstitched straight into the mesh.
    for &p in &flagged {
        let m = t.patch_meshes[p].as_ref().unwrap();
        assert!(
            m.strips
                .iter()
                .all(|i| ((i & 0x7fff) as usize) < m.positions.len()),
            "patch {p}: STRP index past POSI"
        );
    }

    // The cell-corner check, on all 23 patches at once.
    let (per_row, cells) = t.patch_grid().unwrap();
    let mut corners = 0;
    for &p in &flagged {
        let m = t.patch_meshes[p].as_ref().unwrap();
        let world = t.patch_mesh_world(p);
        assert_eq!(world.len(), m.positions.len());
        let (px, pz) = (p % per_row, p / per_row);
        for (q, w) in m.positions.iter().zip(&world) {
            let step = 65536 / cells as u32;
            if !(q[0] as u32).is_multiple_of(step) || !(q[2] as u32).is_multiple_of(step) {
                continue;
            }
            let (cx, cz) = (
                px as u32 * cells as u32 + q[0] as u32 / step,
                pz as u32 * cells as u32 + q[2] as u32 / step,
            );
            if cx >= t.info.grid_size || cz >= t.info.grid_size {
                continue;
            }
            let dy = (w[1] - t.world_y(cx, cz)).abs();
            assert!(dy < 0.005, "patch {p} cell ({cx},{cz}): {dy} m");
            let (wx, wz) = t.world_xz(cx, cz);
            assert!((w[0] - wx).abs() < 0.01 && (w[2] - wz).abs() < 0.01);
            corners += 1;
        }
    }
    assert!(corners > 100, "only {corners} cell-corner vertices checked");
}

/// The base layer covers most of every map, and mask 0 marks the ground
/// outside the level (KAS1 is an island in an ocean of empty patches).
#[test]
fn base_layer_covers_the_textured_patches() {
    let (mut tested, mut skipped) = (0, 0);
    for (name, min_pct) in [
        ("bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL", 80),
        ("bf1/lvl/DATA/_LVL_PS2/KAS/KAS1.LVL", 80),
        ("bf2/lvl/DATA/_LVL_PS2/YAV/YAV1.LVL", 80),
    ] {
        let Some(buf) = tern_of(name) else {
            eprintln!("SKIP: {name} not present");
            skipped += 1;
            continue;
        };
        tested += 1;
        let inner = crate::open(&buf).unwrap();
        let tern = crate::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern")
            .unwrap_or_else(|| panic!("{name}: tern"));
        let t = Terrain::parse(&tern).unwrap_or_else(|e| panic!("{name}: {e}"));
        let textured: Vec<u16> = t
            .patch_layer_masks
            .iter()
            .copied()
            .filter(|&m| m != 0)
            .collect();
        assert!(!textured.is_empty(), "{name}");
        let with_base = textured.iter().filter(|m| *m & 1 != 0).count();
        let pct = 100 * with_base / textured.len();
        assert!(
            pct >= min_pct,
            "{name}: LTEX[0] painted in only {pct}% of textured patches"
        );
        assert!(
            !t.layer_textures.is_empty() && t.layer_scale[0] > 0.0,
            "{name}"
        );
    }
    eprintln!("base_layer_covers_the_textured_patches: {tested} maps tested, {skipped} skipped");
}

/// A synthetic `PCHS` with one flagged and one plain `PTCH`: the flag word
/// is what carries the replacement mesh, and the dequant places it in the
/// patch's own world box (analysis/re-terrain-cutters.md §1.2).
#[test]
fn flagged_ptch_parses_into_a_world_space_mesh() {
    use crate::build_chunk;
    // 16x16 cells, 1 m units, 2x2 render patches of 8 cells each.
    let n = 16u16;
    let mut info = vec![0u8; 28];
    info[0..4].copy_from_slice(&1f32.to_le_bytes()); // gridUnit
    info[4..8].copy_from_slice(&0.5f32.to_le_bytes()); // heightScale
    info[16..18].copy_from_slice(&n.to_le_bytes());
    // PTCH INFO: mask, count, heightMin, heightMax, shift, flag.
    let ptch_info = |min: i16, shift: u16, flag: u32| {
        let mut v = 1u16.to_le_bytes().to_vec();
        v.extend(0u16.to_le_bytes());
        v.extend(min.to_le_bytes());
        v.extend(64i16.to_le_bytes());
        v.extend(shift.to_le_bytes());
        v.extend(flag.to_le_bytes());
        v
    };
    // Three vertices: the patch's own corner, its far corner, and its
    // centre one height step up.
    let posi: Vec<u8> = [[0u16, 0, 0], [0xffff, 0, 0xffff], [0x8000, 0x100, 0x8000]]
        .iter()
        .flat_map(|v| v.iter().flat_map(|c| c.to_le_bytes()))
        .collect();
    let strp: Vec<u8> = [0x8000u16, 0x8001, 2]
        .iter()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let mut flagged = build_chunk(b"INFO", &ptch_info(-4, 8, 1));
    flagged.extend(build_chunk(b"POSI", &posi));
    flagged.extend(build_chunk(b"STRP", &strp));
    // COLR: three u16 (a run of 3 x 0x801f = red, alpha bit set).
    flagged.extend(build_chunk(b"COLR", &[0x82, 0x1f, 0x80]));
    // The mask (1) paints one layer, so ALPH is not read: 0xe fill.
    flagged.extend(build_chunk(b"ALPH", &[4, 5]));
    let mut pchs = Vec::new();
    for p in 0..4 {
        // Patch 1 (px=1, pz=0) is the flagged one.
        pchs.extend(build_chunk(
            b"PTCH",
            &if p == 1 {
                flagged.clone()
            } else {
                {
                    let mut short = ptch_info(0, 8, 0);
                    // Console PC/Xbox patches omit the PS2 replacement
                    // flag (and sometimes the shift), retaining the mask.
                    if p == 0 {
                        short.truncate(8);
                    } else if p == 2 {
                        short.truncate(10);
                    }
                    build_chunk(b"INFO", &short)
                }
            },
        ));
    }
    let mut body = build_chunk(b"NAME", b"t\0");
    body.extend(build_chunk(b"INFO", &info));
    // RLE: 0xFF = repeat the next byte 128 times; twice = 256 zero cells.
    body.extend(build_chunk(b"HGT8", &[0xFFu8, 0x00, 0xFF, 0x00]));
    body.extend(build_chunk(b"HEXP", &[0u8; 8]));
    body.extend(build_chunk(b"PCHS", &pchs));
    let buf = build_chunk(b"tern", &body);
    let c = crate::chunks(&buf).next().unwrap().unwrap();
    let t = Terrain::parse(&c).expect("parses");

    assert_eq!(t.patch_grid(), Some((2, 8)));
    assert_eq!(t.patch_layer_masks, vec![1, 1, 1, 1]);
    assert_eq!(t.patch_meshes.len(), 4);
    assert!(t.patch_meshes[0].is_none() && t.patch_meshes[2].is_none());
    let pm = t.patch_meshes[1].as_ref().expect("patch 1 is flagged");
    assert_eq!(pm.positions.len(), 3);
    assert_eq!(pm.strips, vec![0x8000, 0x8001, 2]);
    assert_eq!((pm.height_min, pm.shift), (-4, 8));
    assert_eq!(pm.colr.as_deref(), Some(&[[255u8, 0, 0]; 3][..]));
    assert_eq!(pm.alpha, vec![0xe; 3]);

    // Patch (1, 0) starts at cell (8, 0) = world (0, -7) and spans 8 m.
    let w = t.patch_mesh_world(1);
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    assert!(near(w[0][0], 0.0) && near(w[0][2], -7.0), "{:?}", w[0]);
    assert!(near(w[1][0], 8.0) && near(w[1][2], 1.0), "{:?}", w[1]);
    assert!(near(w[2][0], 4.0) && near(w[2][2], -3.0), "{:?}", w[2]);
    // Y = (heightMin + y*(65536 >> shift)/65536) * heightScale:
    // corners at -4 raw = -2 m, the centre 0x100 counts up = -3 raw.
    assert!(near(w[0][1], -2.0) && near(w[1][1], -2.0), "{:?}", w);
    assert!(near(w[2][1], -1.5), "{:?}", w[2]);
    // An unflagged patch has no mesh at all.
    assert!(t.patch_mesh_world(0).is_empty());
}

/// Data-gated, the TALPH census (analysis/re-terrain-layers.md §1, §4,
/// `lead_tmp/re/talph/census.txt`, `repl.txt`): on all 27 shipped
/// terrains the per-patch `ALPH` walk ends exactly at the chunk's end,
/// and the 122 cutter-replaced patches' own `COLR` / `ALPH` decode to
/// one entry per `POSI` vertex (per layer).
#[test]
fn alph_walks_every_shipped_terrain_exactly() {
    let (mut tested, mut skipped) = (0, 0);
    let lvls = [
        "bf1/BES/BES1",
        "bf1/BES/BES2",
        "bf1/END/END1",
        "bf1/GEO/GEO1",
        "bf1/HOT/HOT1",
        "bf1/KAM/KAM1",
        "bf1/KAS/KAS1",
        "bf1/KAS/KAS2",
        "bf1/NAB/NAB1",
        "bf1/NAB/NAB2",
        "bf1/RHN/RHN1",
        "bf1/RHN/RHN2",
        "bf1/TAT/TAT1",
        "bf1/TAT/TAT2",
        "bf1/TEST/TEST",
        "bf1/YAV/YAV1",
        "bf1/YAV/YAV2",
        "bf2/DAG/DAG1",
        "bf2/END/END1",
        "bf2/FEL/FEL1",
        "bf2/GEO/GEO1",
        "bf2/HOT/HOT1",
        "bf2/KAS/KAS2",
        "bf2/NAB/NAB2",
        "bf2/POL/POL1",
        "bf2/TAT/TAT2",
        "bf2/YAV/YAV1",
    ];
    let (mut replaced, mut multi_replaced, mut multi) = (0, 0, 0);
    for l in lvls {
        let (game, rest) = l.split_at(3);
        let Some(buf) = tern_of(&format!("{game}/lvl/DATA/_LVL_PS2{rest}.LVL")) else {
            eprintln!("SKIP: {l} not present");
            skipped += 1;
            continue;
        };
        tested += 1;
        let inner = crate::open(&buf).unwrap();
        let tern = crate::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern" && !Terrain::is_stub(c))
            .unwrap_or_else(|| panic!("{l}: tern"));
        let t = Terrain::parse(&tern).unwrap_or_else(|e| panic!("{l}: {e}"));
        assert_eq!(t.patch_edge, 8, "{l}");
        let alph = tern.child(b"PCHS").and_then(|p| p.child(b"ALPH"));
        let src = alph.as_ref().map(|a| a.payload).unwrap_or_default();
        let (_, used) = super::walk_alph(src, &t.patch_layer_masks, t.patch_edge)
            .unwrap_or_else(|| panic!("{l}: the ALPH walk fails"));
        assert_eq!(used, src.len(), "{l}: the walk ends inside the chunk");
        for (m, a) in t.patch_layer_masks.iter().zip(&t.patch_alpha) {
            assert_eq!(a.len(), m.count_ones() as usize * 81, "{l}");
            multi += usize::from(m.count_ones() > 1);
        }
        for (m, pm) in t.patch_layer_masks.iter().zip(&t.patch_meshes) {
            let Some(pm) = pm else { continue };
            replaced += 1;
            let n = pm.positions.len();
            assert_eq!(pm.colr.as_ref().map(Vec::len), Some(n), "{l}: COLR");
            assert_eq!(pm.alpha.len(), m.count_ones() as usize * n, "{l}: ALPH");
            multi_replaced += usize::from(m.count_ones() > 1);
        }
    }
    eprintln!("alph_walks_every_shipped_terrain_exactly: {tested} maps tested, {skipped} skipped");
    if skipped != 0 {
        eprintln!("SKIP: full 27-map aggregate census requires every terrain");
        return;
    }
    assert_eq!((replaced, multi_replaced), (122, 104));
    assert_eq!(multi, 4703, "multi-layer patches, census.txt's sum");
}

/// The in-patch layout (re-terrain-layers.md §1.4): per painted layer a
/// block of 9 rows (row = z) of 5 bytes, vertex x the low nibble of byte
/// x/2 for even x; a 1-layer patch reads no bytes and is 0xe throughout.
#[test]
fn nibble_order_is_low_first_row_z() {
    use crate::build_chunk;
    // One 16-cell terrain, P = 8: 2x2 patches, patch 0 paints layers 0
    // and 2; its ALPH is 2 x 45 literal bytes.
    let mut info = vec![0u8; 28];
    info[0..4].copy_from_slice(&1f32.to_le_bytes());
    info[16..18].copy_from_slice(&16u16.to_le_bytes());
    info[18..20].copy_from_slice(&8u16.to_le_bytes());
    let ptch = |mask: u16| {
        let mut v = mask.to_le_bytes().to_vec();
        v.extend([0u8; 12]);
        build_chunk(b"PTCH", &build_chunk(b"INFO", &v))
    };
    let mut pchs = ptch(0b101);
    for _ in 0..3 {
        pchs.extend(ptch(1));
    }
    // Layer block 0: byte b = b (so row r's bytes are 5r..5r+5); block 1
    // all 0x3c. Literal runs of at most 128.
    let mut raw: Vec<u8> = (0..45u8).collect();
    raw.extend([0x3c; 45]);
    let mut alph = vec![89u8];
    alph.extend(&raw);
    pchs.extend(build_chunk(b"ALPH", &alph));
    let mut body = build_chunk(b"NAME", b"t\0");
    body.extend(build_chunk(b"INFO", &info));
    body.extend(build_chunk(b"HGT8", &[0xFFu8, 0x00, 0xFF, 0x00]));
    body.extend(build_chunk(b"HEXP", &[0u8; 8]));
    body.extend(build_chunk(b"PCHS", &pchs));
    let buf = build_chunk(b"tern", &body);
    let c = crate::chunks(&buf).next().unwrap().unwrap();
    let t = Terrain::parse(&c).expect("parses");
    let a = &t.patch_alpha[0];
    assert_eq!(a.len(), 2 * 81);
    // Row z = 1, x = 0..9 reads bytes 5..10: 5 = (lo 5, hi 0), 6 = (6, 0) …
    assert_eq!(&a[9..18], &[5, 0, 6, 0, 7, 0, 8, 0, 9]);
    assert!(a[81..]
        .iter()
        .enumerate()
        .all(|(i, &v)| v == if i % 9 % 2 == 0 { 0xc } else { 3 }));
    assert_eq!(t.patch_alpha[1], vec![0xe; 81]);
}

/// Data-gated: BF2's model-built maps ship a NAME-only `tern` (COR1) or
/// NAME + WATR (DEA1). Those are stubs, not truncated terrain — the bake
/// used to abort on them.
#[test]
fn bf2_model_built_maps_have_stub_terrain() {
    let (mut tested, mut skipped) = (0, 0);
    for (name, stub) in [("COR/COR1", true), ("DEA/DEA1", true), ("HOT/HOT1", false)] {
        let p = format!(
            "{}/../../../extracted/bf2/lvl/DATA/_LVL_PS2/{name}.LVL",
            env!("CARGO_MANIFEST_DIR")
        );
        let Ok(buf) = std::fs::read(&p) else {
            eprintln!("SKIP: {name} not present");
            skipped += 1;
            continue;
        };
        tested += 1;
        let inner = crate::open(&buf).unwrap();
        let tern = crate::chunks(inner)
            .flatten()
            .find(|c| &c.tag == b"tern")
            .unwrap_or_else(|| panic!("{name} has a tern"));
        assert_eq!(Terrain::is_stub(&tern), stub, "{name}");
        if !stub {
            Terrain::parse(&tern).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
    eprintln!("bf2_model_built_maps_have_stub_terrain: {tested} maps tested, {skipped} skipped");
}

/// `gridSize` is a raw u16 that squares into every allocation, so 65520
/// asks for a 17 GB `raw` grid. Reject it before allocating anything.
#[test]
fn absurd_grid_size_errors() {
    use crate::build_chunk;
    let mut info = vec![0u8; 28];
    info[0..4].copy_from_slice(&8f32.to_le_bytes()); // gridUnit
    info[4..8].copy_from_slice(&0.05f32.to_le_bytes()); // heightScale
    info[16..18].copy_from_slice(&65520u16.to_le_bytes()); // gridSize
    let mut body = build_chunk(b"NAME", b"t\0");
    body.extend(build_chunk(b"INFO", &info));
    body.extend(build_chunk(b"HGT8", &[0x80, 0x00]));
    body.extend(build_chunk(b"HEXP", &[0u8; 8]));
    let buf = build_chunk(b"tern", &body);
    let c = crate::chunks(&buf).next().unwrap().unwrap();
    let Err(e) = Terrain::parse(&c) else {
        panic!("expected an error")
    };
    assert!(matches!(e, crate::UcfbError::Malformed(_)), "{e}");
}

/// A point is inside a cutter iff EVERY plane evaluates strictly < 0.
fn inside(c: &super::Cutter, p: [f32; 3]) -> bool {
    c.planes
        .iter()
        .all(|q| q[0] * p[0] + q[1] * p[1] + q[2] * p[2] + q[3] < 0.0)
}

/// Synthetic `CUTS`: a unit box around the origin as six planes, plus a
/// terrain with no `CUTS` at all (BF2 MUS1 ships none — parse must not
/// fail).
#[test]
fn synthetic_cuts_parses_and_absent_cuts_is_empty() {
    use crate::build_chunk;
    let planes: [[f32; 4]; 6] = [
        [1.0, 0.0, 0.0, -1.0],
        [-1.0, 0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0, -1.0],
        [0.0, -1.0, 0.0, -1.0],
        [0.0, 0.0, 1.0, -1.0],
        [0.0, 0.0, -1.0, -1.0],
    ];
    let mut plns = Vec::new();
    for pl in &planes {
        for v in pl {
            plns.extend(v.to_le_bytes());
        }
    }
    let mut cutr = build_chunk(b"INFO", &{
        let mut i = 6u32.to_le_bytes().to_vec();
        i.extend([0u8; 16]);
        i
    });
    cutr.extend(build_chunk(b"PLNS", &plns));
    let mut cuts = build_chunk(b"INFO", &1u32.to_le_bytes());
    cuts.extend(build_chunk(b"CUTR", &cutr));

    let mut info = vec![0u8; 28];
    info[0..4].copy_from_slice(&1f32.to_le_bytes());
    info[4..8].copy_from_slice(&1f32.to_le_bytes());
    info[16..18].copy_from_slice(&16u16.to_le_bytes());
    // RLE: 0xFF = repeat the next element 128 times; twice = 256 zeros.
    let hgt8 = [0xFFu8, 0x00, 0xFF, 0x00];
    let mut body = build_chunk(b"NAME", b"t\0");
    body.extend(build_chunk(b"INFO", &info));
    body.extend(build_chunk(b"HGT8", &hgt8));
    body.extend(build_chunk(b"HEXP", &[0u8; 8]));
    let with_cuts = {
        let mut b = body.clone();
        b.extend(build_chunk(b"CUTS", &cuts));
        build_chunk(b"tern", &b)
    };
    let c = crate::chunks(&with_cuts).next().unwrap().unwrap();
    let t = Terrain::parse(&c).expect("parses");
    assert_eq!(t.cutters.len(), 1);
    assert_eq!(t.cutters[0].planes.len(), 6);
    // This synthetic INFO zeroes the four sphere floats: no bound.
    assert_eq!(t.cutters[0].sphere, [0.0; 4]);
    assert!(inside(&t.cutters[0], [0.0, 0.0, 0.0]));
    assert!(!inside(&t.cutters[0], [2.0, 0.0, 0.0]));
    // On the plane is NOT inside (the test is strict).
    assert!(!inside(&t.cutters[0], [1.0, 0.0, 0.0]));

    let without = build_chunk(b"tern", &body);
    let c = crate::chunks(&without).next().unwrap().unwrap();
    assert!(Terrain::parse(&c)
        .expect("parses without CUTS")
        .cutters
        .is_empty());
}

/// Pinned by the lead on the raw disc (RE_LOG 2026-09-01 coll/prim/CUTR
/// measurement, Fable session 7): BF2 DAG1 ships 4 cutters with plane
/// counts 18/18/18/6, and the cave post cp4's capture centre is inside
/// the 6-plane one while the terrain surface above it is not.
#[test]
fn dag1_pinned_cutters() {
    let Some(buf) = tern_of("bf2/lvl/DATA/_LVL_PS2/DAG/DAG1.LVL") else {
        eprintln!("SKIP: BF2 DAG1.LVL not present");
        return;
    };
    let inner = crate::open(&buf).unwrap();
    let tern = crate::chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("tern");
    let t = Terrain::parse(&tern).expect("parses");
    let counts: Vec<usize> = t.cutters.iter().map(|c| c.planes.len()).collect();
    assert_eq!(counts, vec![18, 18, 18, 6]);
    let c3 = &t.cutters[3];
    let near = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 1e-4);
    // INFO's four floats are the bounding sphere (r, cx, cy, cz): a
    // 33.6 m sphere around the cave, which contains the post at
    // (16, −2, −50).
    assert!(
        near(c3.sphere, [33.636_5, 8.264_658, -2.201_917_6, -49.301_994]),
        "{:?}",
        c3.sphere
    );
    let d2 = (16.0f32 - c3.sphere[1]).powi(2)
        + (-2.0f32 - c3.sphere[2]).powi(2)
        + (-50.0f32 - c3.sphere[3]).powi(2);
    assert!(d2 < c3.sphere[0] * c3.sphere[0], "post inside the bound");
    assert!(
        near(c3.planes[0], [-0.9870979, 0.0, 0.16536956, -5.759_383]),
        "{:?}",
        c3.planes[0]
    );
    // |n| = 1.002 on the caps: stored as read, never renormalised.
    assert!(
        near(c3.planes[1], [0.0, -1.0020342, 0.0, -6.186556]),
        "{:?}",
        c3.planes[1]
    );
    assert!(
        near(c3.planes[2], [0.0, 1.0020342, 0.0, -1.8135252]),
        "{:?}",
        c3.planes[2]
    );
    // The cave post's capture centre is inside; the terrain surface above
    // it is not (the test is 3-D on the unit's own position).
    assert!(inside(c3, [16.0, -2.0, -50.0]));
    assert!(!inside(c3, [16.0, 4.7, -50.0]));
    // ... and no surface post is inside any cutter.
    assert!(!t.cutters.iter().any(|c| inside(c, [16.0, 4.7, -50.0])));
}
