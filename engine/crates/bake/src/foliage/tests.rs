//! `bake::foliage` against the PS2's own memory: every `Live` row below is a
//! foliage instance read out of a PCSX2 save state's EE RAM by the RE's
//! `livepool.py` (`engine/artifacts/lead_tmp/re/folg/live_instances.json`,
//! re-foliage §4), copied in as literals — the first four settled instances
//! of each mesh in BF1 END1 `cp1_cp2_postA_spawn` and BF2 FEL1
//! `cp3_cp3_1_postA_spawn`. The disc tests are data-gated: skipped when the
//! discs are not extracted locally.

use super::*;

struct Live {
    mesh: &'static str,
    model: bool,
    x: f32,
    /// Checked by `game::viewer::foliage`, which owns the height.
    #[allow(dead_code)]
    y: f32,
    z: f32,
    yaw: f32,
    rgba: [u8; 4],
}

#[rustfmt::skip]
const END1: &[Live] = &[
    Live { mesh: "end_fern", model: false, x: -242.63098, y: -3.7799997, z: 429.75894, yaw: 0.49947587, rgba: [211, 210, 205, 255] },
    Live { mesh: "end_fern", model: false, x: -254.20122, y: -2.8578293, z: 441.81104, yaw: 0.86722887, rgba: [199, 192, 206, 255] },
    Live { mesh: "end_fern", model: false, x: -217.81104, y: -3.7457151, z: 445.1666, yaw: 2.4026713, rgba: [198, 206, 203, 255] },
    Live { mesh: "end_fern", model: false, x: -212.73181, y: -3.4239073, z: 444.74637, yaw: 2.7070196, rgba: [206, 210, 211, 255] },
    Live { mesh: "end_prop_fern5", model: true, x: -249.56644, y: 0.16218832, z: 386.63098, yaw: 0.61459213, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_fern5", model: true, x: -248.85284, y: -3.128986, z: 414.2012, yaw: 2.298352, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_fern5", model: true, x: -249.81104, y: -3.2502723, z: 429.1666, yaw: 2.4026713, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_fern5", model: true, x: -244.73181, y: -3.78, z: 444.74637, yaw: 2.7070196, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage5", model: true, x: -200.85284, y: -0.90433407, z: 446.2012, yaw: 2.298352, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage5", model: true, x: -254.02887, y: 2.0474806, z: 357.23187, yaw: 3.1591506, rgba: [255, 255, 255, 231] },
    Live { mesh: "end_prop_foliage5", model: true, x: -281.81104, y: 6.8464813, z: 381.1666, yaw: 2.4026713, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage5", model: true, x: -257.4381, y: 1.356979, z: 325.2799, yaw: 1.8136833, rgba: [255, 255, 255, 5] },
    Live { mesh: "end_prop_foliage6", model: true, x: -242.63098, y: -2.8679366, z: 397.75894, yaw: 0.49947587, rgba: [255, 255, 255, 254] },
    Live { mesh: "end_prop_foliage6", model: true, x: -254.20122, y: -0.9500977, z: 409.81104, yaw: 0.86722887, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage6", model: true, x: -268.01602, y: 4.407327, z: 365.95032, yaw: 5.8693395, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage6", model: true, x: -282.4953, y: 4.810314, z: 354.93375, yaw: 4.210643, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage7", model: true, x: -252.7464, y: -3.032442, z: 440.8528, yaw: 0.42597443, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage7", model: true, x: -269.75894, y: 5.312252, z: 382.02887, yaw: 4.2513304, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage7", model: true, x: -167.79272, y: -3.3382947, z: 362.4953, yaw: 5.2314706, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_foliage7", model: true, x: -277.32727, y: 3.1451192, z: 329.12857, yaw: 1.3579347, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_grass", model: false, x: -252.48318, y: -2.9782476, z: 436.7318, yaw: 1.0464439, rgba: [211, 211, 211, 255] },
    Live { mesh: "end_prop_grass", model: false, x: -205.75896, y: -2.2391753, z: 446.02887, yaw: 4.2513304, rgba: [241, 241, 241, 255] },
    Live { mesh: "end_prop_grass", model: false, x: -245.27992, y: 0.19473176, z: 362.94742, yaw: 4.405279, rgba: [246, 246, 246, 255] },
    Live { mesh: "end_prop_grass", model: false, x: -261.77756, y: 3.2561443, z: 357.9072, yaw: 0.08776817, rgba: [243, 243, 243, 255] },
    Live { mesh: "end_prop_treeclump_1", model: true, x: -204.7464, y: -3.1801739, z: 440.8528, yaw: 0.42597443, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_treeclump_1", model: true, x: -206.20122, y: -1.3604261, z: 393.81104, yaw: 0.86722887, rgba: [255, 255, 255, 91] },
    Live { mesh: "end_prop_treeclump_1", model: true, x: -253.75896, y: 2.0218277, z: 366.02887, yaw: 4.2513304, rgba: [255, 255, 255, 255] },
    Live { mesh: "end_prop_treeclump_1", model: true, x: -265.12857, y: 3.9726477, z: 364.01602, yaw: 0.53297776, rgba: [255, 255, 255, 255] },
];

#[rustfmt::skip]
const FEL1: &[Live] = &[
    Live { mesh: "fel1_bldg_pitcher_bush", model: true, x: -20.556602, y: 34.143017, z: -140.06412, yaw: 0.7957418, rgba: [173, 175, 236, 255] },
    Live { mesh: "fel1_bldg_pitcher_bush", model: true, x: -31.136772, y: 32.969196, z: -123.12416, yaw: 0.9516069, rgba: [201, 160, 172, 255] },
    Live { mesh: "fel1_bldg_pitcher_bush", model: true, x: -35.624245, y: 27.48525, z: -112.86323, yaw: 2.3236732, rgba: [158, 213, 180, 255] },
    Live { mesh: "fel1_bldg_pitcher_bush", model: true, x: -11.935863, y: 31.470163, z: -137.33287, yaw: 0.15777513, rgba: [160, 237, 171, 255] },
    Live { mesh: "fel1_prop_foliage_b", model: true, x: 11.264533, y: 16.044113, z: -114.1304, yaw: 0.5985567, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_b", model: true, x: 7.028017, y: 16.88699, z: -114.81303, yaw: 3.7565036, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_b", model: true, x: 7.518755, y: 17.427984, z: -116.274956, yaw: 2.95556, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_b", model: true, x: 3.3103046, y: 18.862827, z: -117.723595, yaw: 1.6183035, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_d", model: true, x: 29.483753, y: 16.126461, z: -125.280785, yaw: 4.7700233, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_d", model: true, x: 27.01797, y: 16.667635, z: -125.66078, yaw: 2.2658448, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_d", model: true, x: 29.87431, y: 16.265797, z: -126.06548, yaw: 4.17049, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_foliage_d", model: true, x: 17.332888, y: 17.052368, z: -120.58497, yaw: 3.789422, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_thick_bush", model: false, x: 22.065483, y: 17.943157, z: -125.28928, yaw: 5.2504234, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_thick_bush", model: false, x: 16.584967, y: 19.047934, z: -124.84254, yaw: 4.405169, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_thick_bush", model: false, x: 20.846466, y: 17.57417, z: -123.4434, yaw: 3.1254582, rgba: [255, 255, 255, 255] },
    Live { mesh: "fel1_prop_thick_bush", model: false, x: 22.292479, y: 16.45682, z: -120.73472, yaw: 3.8690896, rgba: [255, 255, 255, 255] },
];

fn disc(rel: &str) -> Option<Vec<u8>> {
    let path = format!("{}/../../../extracted/{rel}", env!("CARGO_MANIFEST_DIR"));
    if !Path::new(&path).exists() {
        eprintln!("SKIP: {path} not present");
        return None;
    }
    Some(std::fs::read(path).unwrap())
}

/// The map's foliage file, FOLG mask (at the PS2's grid) and terrain.
fn load(buf: &[u8], game: &str) -> (FoliageFile, FolgMask, Terrain) {
    let inner = mungefs::open(buf).unwrap();
    let mut file = None;
    let mut tern = None;
    for c in mungefs::content_chunks(inner).unwrap() {
        if &c.tag == b"prp_" && file.is_none() {
            file = Some(FoliageFile::parse(&c).unwrap());
        } else if &c.tag == b"tern" && tern.is_none() && !Terrain::is_stub(&c) {
            let t = Terrain::parse(&c).unwrap();
            let (dim, _) = grid(game, &t);
            tern = Some((
                FolgMask::parse(&c, dim).expect("a FOLG at the PS2's grid"),
                t,
            ));
        }
    }
    let (mask, t) = tern.unwrap();
    (file.unwrap(), mask, t)
}

/// Every RAM instance is a placed one: the same mesh, x and z within 1 mm,
/// the yaw within 1e-4 — and, fed the RAM x and z, the tint is the PS2's to
/// the byte. Also counts how many x, z are the PS2's float to the BIT (the
/// EE's round-toward-zero reproduced), and asserts all are.
fn check(rel: &str, game: &str, live: &[Live]) {
    let Some(buf) = disc(rel) else { return };
    let Some(elf) = elf_for(
        &format!("{}/../../../extracted/{rel}", env!("CARGO_MANIFEST_DIR")),
        game,
    ) else {
        eprintln!("SKIP: no {game} ELF");
        return;
    };
    let table = rng_table(&std::fs::read(elf).unwrap(), game).unwrap();
    let (file, mask, t) = load(&buf, game);
    let (_, cell) = grid(game, &t);
    let placed = place(&mask, &file, cell, &table);
    let meshes: Vec<_> = file.layers.iter().flat_map(|l| &l.meshes).collect();
    let lumi = Lumi::of(&t).unwrap();
    let mut exact = 0;
    for r in live {
        let h = fnv1a_lowercase(r.mesh);
        let p = placed
            .iter()
            .find(|p| {
                meshes[p.mesh as usize].name_hash == h
                    && (p.x - r.x).abs() < 1e-3
                    && (p.z - r.z).abs() < 1e-3
            })
            .unwrap_or_else(|| panic!("{rel}: no placed {} at ({}, {})", r.mesh, r.x, r.z));
        assert!(
            (p.yaw - r.yaw).abs() < 1e-4,
            "{}: yaw {} vs {}",
            r.mesh,
            p.yaw,
            r.yaw
        );
        exact += (p.x.to_bits() == r.x.to_bits() && p.z.to_bits() == r.z.to_bits()) as usize;
        let cv = meshes[p.mesh as usize].color_variation;
        // A is the draw's fade alpha, written after the tint (re-foliage §6).
        assert_eq!(
            tint(lumi.sample(r.x, r.z), r.model, cv, r.x, r.z)[..3],
            r.rgba[..3],
            "{rel}: {} at ({}, {})",
            r.mesh,
            r.x,
            r.z
        );
    }
    eprintln!("{rel}: {}/{} bit-exact x, z", exact, live.len());
    assert_eq!(
        exact,
        live.len(),
        "{rel}: x, z not the PS2's floats to the bit"
    );
}

#[test]
fn live_bf1_end1() {
    check("bf1/lvl/DATA/_LVL_PS2/END/END1.LVL", "bf1", END1);
}

#[test]
fn live_bf2_fel1() {
    check("bf2/lvl/DATA/_LVL_PS2/FEL/FEL1.LVL", "bf2", FEL1);
}

/// The whole RE capture, when it is on this machine: every live instance of
/// every save state `livepool.py` read (the file is gitignored scratch, so
/// the test skips without it). Settled instances only — an unsettled one has
/// no tint yet.
#[test]
fn live_capture_every_instance() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/lead_tmp/re/folg/live_instances.json"
    );
    let Ok(text) = std::fs::read_to_string(p) else {
        eprintln!("SKIP: {p} not present");
        return;
    };
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let (mut n, mut exact) = (0, 0);
    for (state, s) in v.as_object().unwrap() {
        let game = s["game"].as_str().unwrap();
        let rel = format!("{game}/lvl/DATA/_LVL_PS2/{}", s["lvl"].as_str().unwrap());
        let Some(buf) = disc(&rel) else { return };
        let elf = elf_for(
            &format!("{}/../../../extracted/{rel}", env!("CARGO_MANIFEST_DIR")),
            game,
        )
        .unwrap();
        let table = rng_table(&std::fs::read(elf).unwrap(), game).unwrap();
        let (file, mask, t) = load(&buf, game);
        let placed = place(&mask, &file, grid(game, &t).1, &table);
        let meshes: Vec<_> = file.layers.iter().flat_map(|l| &l.meshes).collect();
        let lumi = Lumi::of(&t).unwrap();
        for r in s["live"].as_array().unwrap() {
            let f = |k: &str| r[k].as_f64().unwrap() as f32;
            let (x, z, yaw) = (f("x"), f("z"), f("yaw"));
            let h = fnv1a_lowercase(r["mesh"].as_str().unwrap());
            let p = placed
                .iter()
                .find(|p| {
                    meshes[p.mesh as usize].name_hash == h
                        && p.x == x
                        && p.z == z
                        && (p.yaw - yaw).abs() < 1e-4
                })
                .unwrap_or_else(|| panic!("{state}: no placed instance at ({x}, {z}) bit-exact"));
            n += 1;
            if r["settled"].as_u64() == Some(1) {
                let rgba: Vec<u8> = r["rgba"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c.as_u64().unwrap() as u8)
                    .collect();
                let m = meshes[p.mesh as usize];
                let model = m.kind == Some(MeshKind::Model);
                let got = tint(lumi.sample(p.x, p.z), model, m.color_variation, p.x, p.z);
                exact += (got[..3] == rgba[..3]) as usize;
                if got[..3] != rgba[..3] {
                    eprintln!(
                        "tint miss {state} {} ({x}, {z}): {got:?} vs {rgba:?}",
                        r["mesh"]
                    );
                }
            }
        }
    }
    eprintln!("live capture: {n} instances placed bit-exact; {exact} settled tints exact");
}

/// Both ELFs carry the same table, found through their program headers.
#[test]
fn both_elfs_hold_the_table() {
    let (Some(a), Some(b)) = (disc("bf1/SWBFRONT.ELF"), disc("bf2/SWBFRNT2.ELF")) else {
        return;
    };
    let (ta, tb) = (rng_table(&a, "bf1").unwrap(), rng_table(&b, "bf2").unwrap());
    assert_eq!(ta.map(f32::to_bits), tb.map(f32::to_bits));
    assert_eq!(ta[0], 0.288_184);
    assert!(ta.iter().all(|v| (0.0..1.0).contains(v)));
    // The wrong game's address is the wrong bytes, and says so.
    assert!(rng_table(&a, "bf2").is_err());
}

/// Both ELFs carry the same grass atlas block; each `NumParts` gets its
/// table (re-foliage §7): 3 → the two right-hand halves of the right quarter
/// and the left three quarters, 4 → the quarters, 1 → the whole sheet with
/// every second part u-mirrored, anything else → six parts.
#[test]
fn both_elfs_hold_the_grass_atlas() {
    let (Some(a), Some(b)) = (disc("bf1/SWBFRONT.ELF"), disc("bf2/SWBFRNT2.ELF")) else {
        return;
    };
    for n in [0, 1, 2, 3, 4, 5, 6] {
        assert_eq!(
            grass_atlas(&a, "bf1", n).unwrap(),
            grass_atlas(&b, "bf2", n).unwrap()
        );
    }
    let three = grass_atlas(&a, "bf1", 3).unwrap();
    assert_eq!(three.len(), 3);
    assert_eq!(three[2][0], [0.0, 0.0]);
    assert_eq!(three[2][3], [0.749_511_7, 0.999_511_7]);
    let one = grass_atlas(&a, "bf1", 1).unwrap();
    assert_eq!((one[1][0][0], one[1][1][0]), (0.999_511_7, 0.0));
    assert_eq!(grass_atlas(&a, "bf1", 4).unwrap().len(), 4);
    assert_eq!(grass_atlas(&a, "bf1", 6).unwrap().len(), 6);
    assert!(grass_atlas(&a, "bf2", 4).is_err());
}

/// A class's PROPs over the constructor's defaults: BF1 END1 `end_fern`
/// off the disc (its `classes.ron` row), a later `NumParts` overriding an
/// earlier one (BF1 NAB1 `nab1_prop_grass_tall` sets 1 then 6), and the
/// defaults of a bare class.
#[test]
fn grass_class_reads_the_setter_s_fields() {
    let p = |v: &[(u32, &str)]| -> Vec<(u32, String)> {
        v.iter().map(|&(h, s)| (h, s.to_owned())).collect()
    };
    let g = grass_class(
        "end_fern",
        &p(&[
            (0xeaebcf6e, "1.2"),
            (0x80ba1308, "1.8"),
            (0x5d8b6dab, "1.0"),
            (0xd133f7da, "25"),
            (0xe6915cf6, "35"),
            (0x3c6468f4, "end_fern"),
            (0x5ac02207, "3"),
            (0x80887a6f, "4.5"),
            (0x6e74d661, "5.5"),
            (0x13fac25e, "0.3"),
            (0x25e5c3fc, "0.7"),
            (0xf6582b9c, "0.35"),
            (0x231fcc79, "0.3 0.6"),
            (0xa9b14958, "2.3"),
            (0x97133eeb, "0.8"),
        ]),
    );
    assert_eq!(
        (
            g.particles,
            g.min_size,
            g.max_size,
            g.num_parts,
            g.texture.as_str()
        ),
        (25, 1.2, 1.8, 3, "end_fern")
    );
    assert_eq!(
        (g.raise, g.tall_size, g.tall_lean, g.lean),
        ([0.3, 0.6], 2.3, 0.35, 0.8)
    );
    assert_eq!(
        (g.cycle, g.thin, g.still, g.rect),
        (6, 0.3, false, [-1.0, -1.0])
    );
    let g = grass_class(
        "t",
        &p(&[(0x5ac02207, "1"), (0x5ac02207, "6"), (0x604db82e, "1")]),
    );
    assert_eq!((g.num_parts, g.still), (6, true));
    let d = grass_class("d", &[]);
    assert_eq!(
        (
            d.particles,
            d.min_size,
            d.max_size,
            d.alpha,
            d.max_distance,
            d.texture.as_str()
        ),
        (50, 10.0, 15.0, 0.5, 100.0, "grass")
    );
    assert_eq!(
        (d.raise, d.tall_size, d.tall_lean, d.lean, d.thin),
        ([0.2, 0.2], 2.0, 0.5, 0.1, 0.3)
    );
}

/// The EE rounds toward zero: 1/3 and −1/3 come out one ULP short of the
/// nearest float, an exact product is untouched.
#[test]
fn ee_arithmetic_rounds_toward_zero() {
    let third = ee_mul(1.0 / 3.0f32, 1.0);
    assert_eq!(third, 1.0 / 3.0f32);
    let near = (1.0f64 / 3.0) as f32;
    assert_eq!(chop(1.0 / 3.0), f32::from_bits(near.to_bits() - 1));
    assert_eq!(chop(-1.0 / 3.0), -f32::from_bits(near.to_bits() - 1));
    assert_eq!(ee_mul(1.5, 2.0), 3.0);
    assert_eq!(ee_add(0.1, 0.2), chop(0.1f32 as f64 + 0.2f32 as f64));
}

/// A 100 m path along +x at 28 m: props at the midpoints of the three full
/// steps (14, 42, 70), upright, x along the path, z = dir × up; the flip
/// turns them about y.
#[test]
fn path_props_step_along_the_path() {
    let file = FoliageFile {
        layers: std::array::from_fn(|_| mungefs::foliage::FoliageLayer {
            spread: 1,
            count: 1,
            meshes: Vec::new(),
        }),
        paths: vec![mungefs::foliage::PathLine {
            path_hash: fnv1a_lowercase("jungle1"),
            classes: vec![fnv1a_lowercase("end_prop_treebill")],
            distance: 28.0,
            flip: false,
        }],
    };
    let paths = [ManifestPath {
        name: "Jungle1".into(),
        nodes: vec![
            ([0.0, 2.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
            ([100.0, 2.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
        ],
        props: vec![],
    }];
    let classes = [ManifestClass {
        effects: Vec::new(),
        name: "end_prop_treebill".into(),
        base: "prop".into(),
        geometry: Some("end_prop_treebill".into()),
        collision: vec![],
        door: None,
        hardpoints: vec![],
        attach: vec![],
        build_zone: None,
        lights: vec![],
        turret_rig: None,
    }];
    let p = path_props(&file, &paths, &classes);
    let xs: Vec<f32> = p.iter().map(|i| i.translation[0]).collect();
    assert_eq!(xs, [14.0, 42.0, 70.0]);
    assert!(p
        .iter()
        .all(|i| i.translation[1] == 2.0 && i.class == "end_prop_treebill"));
    assert_eq!(
        p[0].rotation,
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
    let mut flipped = file.clone();
    flipped.paths[0].flip = true;
    let q = path_props(&flipped, &paths, &classes);
    assert_eq!(
        q[0].rotation,
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]
    );
    // An absent class or path places nothing.
    assert!(path_props(&file, &paths, &[]).is_empty());
    assert!(path_props(&file, &[], &classes).is_empty());
    // A Distance that is not positive places nothing (and the walk ends).
    for d in [0.0, -28.0] {
        let mut stuck = file.clone();
        stuck.paths[0].distance = d;
        assert!(path_props(&stuck, &paths, &classes).is_empty());
    }
    // A vertical segment has no side axis: no prop there, and none with a
    // NaN rotation after it.
    let climb = [ManifestPath {
        name: "Jungle1".into(),
        nodes: vec![
            ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
            ([0.0, 60.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
            ([60.0, 60.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
        ],
        props: vec![],
    }];
    let r = path_props(&file, &climb, &classes);
    assert!(!r.is_empty());
    assert!(r
        .iter()
        .all(|i| i.translation[0] > 0.0 && i.rotation.iter().flatten().all(|v| v.is_finite())));
}

/// A tree with no executable above it still gets its grass, from the shipped
/// numbers: a disc build must not lose its foliage to a missing file.
#[test]
fn a_tree_without_the_executable_keeps_its_foliage() {
    let dir = std::env::temp_dir().join(format!("openbf-foliage-noelf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lvl = dir.join("NOPE.LVL").to_string_lossy().into_owned();
    assert!(elf_for(&lvl, "bf1").is_none());
    let grass = [("patch".to_owned(), Vec::new())];
    let got = bake(
        &dir,
        &Inputs {
            lvl_path: &lvl,
            game: "bf1",
            terrain: None,
            mask: None,
            file: None,
            models: &[],
            classes: &[],
            paths: &[],
            grass: &grass,
        },
    )
    .unwrap()
    .expect("the grass class is baked");
    assert_eq!(got.grass.len(), 1);
    assert!(!got.grass[0].atlas.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

/// An executable that does not hold the tables (another release's, or a damaged
/// copy) is no reason to stop a bake or lose its foliage: the shipped numbers are
/// the same numbers.
#[test]
fn an_executable_without_the_tables_falls_back_to_the_shipped_numbers() {
    let dir = std::env::temp_dir().join(format!("openbf-foliage-badelf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lvl = dir.join("NOPE.LVL").to_string_lossy().into_owned();
    std::fs::write(dir.join(ELF_BF1), b"not an executable").unwrap();
    assert!(elf_for(&lvl, "bf1").is_some());
    let grass = [("patch".to_owned(), Vec::new())];
    let got = bake(
        &dir,
        &Inputs {
            lvl_path: &lvl,
            game: "bf1",
            terrain: None,
            mask: None,
            file: None,
            models: &[],
            classes: &[],
            paths: &[],
            grass: &grass,
        },
    )
    .unwrap()
    .expect("the grass class is baked");
    assert_eq!(
        got.grass[0].atlas,
        shipped_grass_atlas("bf1", got.grass[0].num_parts).unwrap()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
