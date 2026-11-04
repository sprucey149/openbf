//! Tier-1 round-trip tests against the real BF1 TAT1.LVL. These need the
//! locally extracted disc data (gitignored); they SKIP cleanly when absent so
//! CI stays runnable anywhere. Expected values from audit A2
//! (analysis/a2-bake-formats.md §3/§4), lead-verified by hand.

use mungefs::{chunks, open, rle};

fn tat1_bytes() -> Option<Vec<u8>> {
    // engine/crates/mungefs -> project root
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL"
    );
    std::fs::read(p).ok()
}

macro_rules! require_data {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => {
                eprintln!("SKIP: extracted TAT1.LVL not present");
                return;
            }
        }
    };
}

#[test]
fn tat1_walks_and_censuses() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let mut tex = 0;
    let mut modl = 0;
    let mut tern = 0;
    for c in chunks(inner) {
        let c = c.expect("clean walk");
        match &c.tag {
            b"tex_" => tex += 1,
            b"modl" => modl += 1,
            b"tern" => tern += 1,
            _ => {}
        }
    }
    assert_eq!(tex, 46, "A2: 46 top-level tex_ in TAT1");
    assert_eq!(tern, 1);
    assert!(modl > 0);
}

#[test]
fn tat1_first_texture_decodes_exact() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tex = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tex_")
        .expect("first tex_");
    assert_eq!(
        tex.child(b"NAME").unwrap().as_str(),
        Some("tat_skiff01_lowrez")
    );
    let info = tex.child(b"INFO").unwrap().payload.to_vec();
    let w = u16::from_le_bytes([info[0], info[1]]) as usize;
    let h = u16::from_le_bytes([info[2], info[3]]) as usize;
    let bpp = u16::from_le_bytes([info[4], info[5]]) as usize;
    let mips = u16::from_le_bytes([info[12], info[13]]) as usize;
    assert_eq!((w, h, bpp, mips), (32, 32, 4, 2));

    // palette: 16 colors × 4 B
    let pal = tex.child(b"pal_").unwrap();
    let pal_body = pal.child(b"BODY").unwrap();
    assert_eq!(rle::decode(pal_body.payload, 4).unwrap().len(), 16 * 4);

    // mip bodies decode to w*h*bpp/8, halving per mip
    let bodies: Vec<_> = tex.children_tagged(b"BODY").collect();
    assert_eq!(bodies.len(), 2);
    assert_eq!(
        rle::decode(bodies[0].payload, 1).unwrap().len(),
        32 * 32 * 4 / 8
    );
    assert_eq!(
        rle::decode(bodies[1].payload, 1).unwrap().len(),
        16 * 16 * 4 / 8
    );
}

#[test]
fn tat1_terrain_heightfield_decodes_exact() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tern = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("tern");
    assert_eq!(tern.child(b"NAME").unwrap().as_str(), Some("tat1"));
    let info = tern.child(b"INFO").unwrap().payload.to_vec();
    assert_eq!(info.len(), 28);
    let grid_unit = f32::from_le_bytes(info[0..4].try_into().unwrap());
    let height_scale = f32::from_le_bytes(info[4..8].try_into().unwrap());
    let grid_size = u16::from_le_bytes(info[16..18].try_into().unwrap()) as usize;
    assert_eq!(grid_unit, 8.0);
    assert!((height_scale - 0.06).abs() < 1e-6);
    assert_eq!(grid_size, 256);

    let hgt8 = tern.child(b"HGT8").unwrap();
    assert_eq!(
        rle::decode(hgt8.payload, 1).unwrap().len(),
        grid_size * grid_size,
        "HGT8 decodes to gridSize^2"
    );
    let hexp = tern.child(b"HEXP").unwrap();
    assert_eq!(hexp.payload.len(), 2048, "u16 per 8x8 patch, 1024 patches");
}

#[test]
fn tat1_sandcrawler_palette_channel_order() {
    // Palette entries are stored [A,B,G,R]; decoded color 0 of the (4-bit)
    // sandcrawler texture must be rusty brown, not red (RE_LOG "LVL textures").
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tex = chunks(inner)
        .flatten()
        .filter(|c| &c.tag == b"tex_")
        .find(|c| {
            c.child(b"NAME").and_then(|n| n.as_str().map(str::to_owned))
                == Some("sandcrawler".into())
        })
        .expect("sandcrawler tex_");
    let t = mungefs::texture::Texture::parse(&tex).unwrap();
    assert_eq!(t.palette[0], [178, 109, 72, 255]);
    assert_eq!(t.info.bpp, 4);
    assert_eq!(t.rgba8(0).len(), 256 * 256 * 4);
}

#[test]
fn tat1_256_color_clut_is_linear_not_csm1() {
    // ERRATUM regression (RE_LOG "256-color CLUTs are LINEAR"): the bake used
    // to CSM1-unswizzle 256-entry CLUTs, which permutes 16 of every 32 colors
    // and shatters an 8-bit texture into speckle. Measure it: mean absolute
    // luma step between horizontally adjacent texels. A photographic rock
    // face is locally smooth; the CSM1 permutation multiplies that step.
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tex = chunks(inner)
        .flatten()
        .filter(|c| &c.tag == b"tex_")
        .find(|c| {
            c.child(b"NAME").and_then(|n| n.as_str().map(str::to_owned))
                == Some("tat1_rocks1".into())
        })
        .expect("tat1_rocks1 tex_");
    let t = mungefs::texture::Texture::parse(&tex).unwrap();
    assert_eq!(t.info.bpp, 8);
    assert_eq!(t.palette.len(), 256);

    let (w, h) = t.mip_dims(0);
    let step = |pal: &[[u8; 4]]| -> f64 {
        let idx = &t.mips[0];
        let luma = |i: usize| {
            let c = pal[idx[i] as usize];
            0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64
        };
        let mut sum = 0.0;
        for y in 0..h as usize {
            for x in 1..w as usize {
                let i = y * w as usize + x;
                sum += (luma(i) - luma(i - 1)).abs();
            }
        }
        sum / ((w - 1) * h) as f64
    };
    let mut csm1 = t.palette.clone();
    for g in csm1.as_chunks_mut::<32>().0 {
        for i in 0..8 {
            g.swap(8 + i, 16 + i);
        }
    }
    let (linear_step, csm1_step) = (step(&t.palette), step(&csm1));
    // Measured on TAT1 tat1_rocks1: linear 9.37, csm1 19.78 (2.1x).
    assert!(
        csm1_step > linear_step * 1.75,
        "linear CLUT must decode far smoother than CSM1 \
         (linear {linear_step:.2} vs csm1 {csm1_step:.2})"
    );
}

#[test]
fn tat1_models_parse_and_dequantize() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let mut models = 0;
    let mut segments = 0;
    let mut triangles = 0usize;
    for c in chunks(inner).flatten().filter(|c| &c.tag == b"modl") {
        let m = mungefs::model::Model::parse(&c).expect("modl parses");
        for s in &m.segments {
            // buffer sizes must match vertex_count (the A2 stride check)
            assert_eq!(s.positions.len(), s.vertex_count as usize);
            assert_eq!(s.normals.len(), s.vertex_count as usize);
            assert_eq!(s.uvs.len(), s.vertex_count as usize);
            // all dequantized positions inside the model vertex box (+eps)
            for p in &s.positions {
                for ((&v, &lo), &hi) in p.iter().zip(&m.vertex_box.min).zip(&m.vertex_box.max) {
                    assert!(v >= lo - 1e-3 && v <= hi + 1e-3);
                }
            }
            // triangle indices in range
            for t in &s.triangles {
                for &ix in t {
                    assert!(ix < s.vertex_count);
                }
            }
            triangles += s.triangles.len();
            segments += 1;
        }
        models += 1;
    }
    assert!(models > 10, "TAT1 has many models, got {models}");
    assert!(segments > 50, "got {segments}");
    assert!(triangles > 10_000, "got {triangles}");
    println!("TAT1: {models} models, {segments} segments, {triangles} triangles");
}

#[test]
fn tat1_world_parses() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let wrld = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"wrld")
        .expect("wrld");
    let w = mungefs::world::World::parse(&wrld).unwrap();
    assert_eq!(w.name, "tat1");
    assert_eq!(w.terrain_name.as_deref(), Some("tat1"));
    // A2 tree excerpt: 15 regn, 249 inst
    assert_eq!(w.regions.len(), 15);
    assert_eq!(w.instances.len(), 249);
    // every instance has a sane rotation matrix (rows near unit length)
    for i in &w.instances {
        for row in &i.transform.rotation {
            let len2: f32 = row.iter().map(|x| x * x).sum();
            assert!((len2 - 1.0).abs() < 0.05, "bad rot row in {}", i.name);
        }
    }
    // spawn-path / CP style props resolve against known key hashes
    let known = mungefs::fnv1a_lowercase("spawnpath");
    let hits = w
        .instances
        .iter()
        .flat_map(|i| &i.props)
        .filter(|p| p.key_hash == known)
        .count();
    assert!(hits > 0, "expected spawnpath props on CP instances");
    println!(
        "TAT1 wrld: {} regions, {} instances, classes e.g. {:?}",
        w.regions.len(),
        w.instances.len(),
        w.instances[0].class
    );
}

#[test]
fn tat1_terrain_heights_match_decomp_trace() {
    // Reference values from analysis/re-terrain-heights.md, reproduced
    // independently by the lead against TAT1.LVL raw bytes.
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tern = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("tern");
    let t = mungefs::terrain::Terrain::parse(&tern).unwrap();
    assert_eq!(t.info.grid_size, 256);
    assert_eq!(t.layer_textures.len(), 6);
    assert!(t.lumi.is_some());

    // raw range [-10, 1057]; floor -0.6 exact
    let (lo, hi) = t
        .raw
        .iter()
        .fold((i32::MAX, i32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    assert_eq!((lo, hi), (-10, 1057));
    assert!((lo as f32 * t.info.height_scale - t.info.height_floor).abs() < 1e-4);

    // worked example: patch 17 (px=1,pz=1), local cell (5,3) => global (21,19)
    // h8=96, shift=2, offset=111 -> raw 495 -> y = 29.7; world (-856, -864)
    assert_eq!(t.raw[(19 * 256 + 21) as usize], 495);
    assert!((t.world_y(21, 19) - 29.7).abs() < 1e-4);
    let (wx, wz) = t.world_xz(21, 19);
    assert_eq!((wx, wz), (-856.0, -864.0));
}

#[test]
fn tat1_entity_classes_and_geom_models() {
    use mungefs::entity::{EntityClass, GeomModel};
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let mut classes = Vec::new();
    let mut gmods = Vec::new();
    for c in chunks(inner) {
        let c = c.expect("clean walk");
        match &c.tag {
            b"entc" => classes.push(EntityClass::parse(&c).expect("entc parses")),
            b"gmod" => gmods.push(GeomModel::parse(&c).expect("gmod parses")),
            _ => {}
        }
    }
    assert_eq!(classes.len(), 29, "TAT1 entc census");
    assert_eq!(gmods.len(), 34, "TAT1 gmod census");

    // The skiff class, lead-verified against raw bytes @0x1CFC4.
    let skiff = classes
        .iter()
        .find(|c| c.name == "tat_hover_skiff")
        .expect("skiff class present");
    assert_eq!(skiff.base, "hover");
    assert_eq!(skiff.geometry_name(), Some("all_hover_skiff"));

    // Its gmod carries LOD0 + LOWD; best model is the LOD0 entry.
    let g = gmods
        .iter()
        .find(|g| g.name == "all_hover_skiff")
        .expect("skiff gmod present");
    assert_eq!(g.best_model(), Some("all_hover_skiff"));
    assert!(g
        .lods
        .iter()
        .any(|l| l.tier == "LOWD" && l.model == "all_hover_skiffLOWD"));
}

/// `sky_` on BF1 TAT1, pinned to the raw bytes read by hand from the chunk
/// (`DATA` hashes 0x06c0d3e6 SkyInfo / 0x3780c8f9 SunInfo / 0x185ab6c2
/// DomeInfo, values as stored). See mungefs::sky for the reading.
#[test]
fn tat1_sky_parses() {
    use mungefs::sky::Sky;
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let chunk = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"sky_")
        .expect("TAT1 has exactly one sky_");
    let sky = Sky::parse(&chunk).expect("sky_ parses");

    assert_eq!(sky.info.fog_color, Some([0.0, 0.0, 10.0]));
    assert_eq!(sky.info.fog_range, Some([0.0, 2700.0]));
    assert_eq!(sky.info.ambient_color, Some([80.0, 75.0, 100.0]));
    assert_eq!(sky.info.character_ambient_color, Some([100.0, 95.0, 120.0]));
    assert_eq!(sky.info.near_scene_range, Some([1.0, 100.0, 150.0]));
    assert_eq!(sky.info.far_scene_range, Some(2000.0));

    let sun = sky.sun.as_ref().expect("TAT1 has SunInfo");
    assert_eq!(sun.angle, Some([130.0, -55.0]));
    assert_eq!(sun.color, Some([242.0, 219.0, 95.0]));
    // The PS2's RAM vector 0x69c590 on TAT1 (analysis/re-sun-direction.md §3).
    let l = sun.light_vector().unwrap();
    for (a, b) in l.iter().zip([-0.4394, -0.6428, 0.6275]) {
        assert!((a - b).abs() < 2e-4, "{l:?}");
    }

    let dome = sky.dome.as_ref().expect("TAT1 has DomeInfo");
    assert_eq!(dome.texture.as_deref(), Some("SKY_tatooine"));
    assert_eq!(dome.angle, Some(-90.0));
    assert_eq!(dome.ambient, Some([255.0, 255.0, 255.0]));
    // Three real shells; the fourth DomeModel on the disc has an empty
    // Geometry and is dropped.
    let names: Vec<&str> = dome.models.iter().map(|m| m.geometry.as_str()).collect();
    assert_eq!(
        names,
        ["tat1_sky_dome", "tat1_skyrocks_01", "tat1_skyrocks_02"]
    );
    assert_eq!(dome.models[1].offset, Some(100.0));
    assert_eq!(dome.models[1].movement_scale, Some(0.995));

    let lr = dome
        .low_res_terrain
        .as_ref()
        .expect("TAT1 has LowResTerrain");
    assert_eq!(lr.texture.as_deref(), Some("tat1"));
    assert_eq!(lr.max_distance, Some(3000.0));
    assert_eq!(lr.fog_near, Some(400.0));
    assert_eq!(lr.fog_far, Some(3500.0));
    assert_eq!(lr.fog_color, Some([164.0, 174.0, 194.0, 80.0]));

    // Four sky objects circle overhead on TAT1.
    assert_eq!(sky.objects.len(), 4);
    assert_eq!(sky.objects[0].geometry, "rep_fly_assault_DOME");
}

/// `lght` on BF2 TAT2 — the BF1 disc ships none, so this reads the BF2 map.
#[test]
fn tat2_bf2_lights_parse() {
    use mungefs::light::Lights;
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/TAT/TAT2.LVL"
    );
    let buf = require_data!(std::fs::read(p).ok());
    let inner = open(&buf).unwrap();
    let chunk = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"lght" && c.payload.len() > 12)
        .expect("TAT2 has one non-stub lght");
    let lights = Lights::parse(&chunk).expect("lght parses");

    assert_eq!(lights.lights.len(), 2);
    let names: Vec<&str> = lights.lights.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["bouncing", "sun"]);

    let g = lights.global.as_ref().expect("GlobalLights present");
    assert_eq!(g.light1.as_deref(), Some("Sun"));
    assert_eq!(g.top, Some([90.0, 90.0, 90.0]));
    assert_eq!(g.bottom, Some([120.0, 120.0, 120.0]));

    // Light1 names "Sun"; the block is spelled "sun" — the lookup folds case.
    let key = lights.key_light().expect("key light resolves");
    assert_eq!(key.name, "sun");
    assert_eq!(key.kind, Some(1.0));
    assert_eq!(key.color, Some([1.0, 1.0, 1.0]));
    let q = key.rotation.expect("sun has a rotation");
    for (got, want) in q.iter().zip([0.672421, 0.538740, 0.083629, -0.500614]) {
        assert!((got - want).abs() < 1e-5, "{q:?}");
    }
    assert!(key.cast_shadow);
    assert!(lights.fill_light().is_none());
}

#[test]
fn bf2_geonosis_sky_keeps_motion_and_effect_attachments() {
    use mungefs::sky::Sky;
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL"
    );
    let buf = require_data!(std::fs::read(p).ok());
    let chunk = mungefs::content_chunks(open(&buf).unwrap())
        .unwrap()
        .into_iter()
        .find(|c| &c.tag == b"sky_")
        .unwrap();
    let sky = Sky::parse(&chunk).unwrap();
    let ship = sky
        .objects
        .iter()
        .find(|o| o.geometry.eq_ignore_ascii_case("rep_fly_gunship_dome"))
        .unwrap();
    assert_eq!(ship.num_objects, Some(20.0));
    assert_eq!(ship.velocity_y, Some([-10.0, 10.0]));
    assert_eq!(ship.velocity_z, Some([80.0, 120.0]));
    assert_eq!(ship.direction_factor, Some(0.5));
    let dome = sky.dome.unwrap();
    let effects = &dome
        .models
        .iter()
        .find(|m| m.geometry == "geo_sky_explosions")
        .unwrap()
        .effects;
    assert_eq!(effects.len(), 11);
    assert!(effects
        .iter()
        .all(|e| e.effect == "spa_sfx_skydomeexplosions" && e.parameter == Some(1.0)));
    assert!(effects.iter().any(|e| e.hardpoint == "hp_sky_15"));
}

/// `LOWR` is a child of `tern`'s `PCHS`, NOT of `tern` — which is why a
/// census that walked `tern`'s direct children reported it missing while a
/// raw byte grep found it. `Chunk::children()` drops nothing; the walk was
/// one level too shallow. Pinned here so the correction cannot be lost.
///
/// The chunk itself is parsed by `mungefs::terrain` since 2026-09-17 (queue
/// L); this test pins the TREE, `lowr_sheet_is_a_far_lod_of_tat1s_own_ground`
/// below pins what it decodes to.
#[test]
fn lowr_lives_under_terns_pchs_not_under_tern() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tern = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("TAT1 has a tern");

    // Directly under `tern`: no LOWR, and the walk is clean (no errors).
    let direct: Vec<[u8; 4]> = tern
        .children()
        .map(|c| c.expect("clean walk").tag)
        .collect();
    assert!(!direct.contains(b"LOWR"), "LOWR is not a direct child");
    assert!(direct.contains(b"PCHS"), "but PCHS is: {direct:?}");

    // One level down, under PCHS.
    let pchs = tern.child(b"PCHS").expect("tern PCHS");
    let lowr = pchs
        .children()
        .flatten()
        .find(|c| &c.tag == b"LOWR")
        .expect("LOWR under PCHS");

    // LOWR INFO: u32 grid, u32 resolution, then three f32 (extent, min y,
    // max y). TAT1 ships an 8x8 grid at resolution 5 over 2048 m, and the
    // resolution matches `sky_ DomeInfo.LowResTerrain.PatchResolution` = 5.
    let info = lowr.child(b"INFO").expect("LOWR INFO");
    assert!(info.payload.len() >= 20);
    let u32at = |o: usize| u32::from_le_bytes(info.payload[o..o + 4].try_into().unwrap());
    let f32at = |o: usize| f32::from_le_bytes(info.payload[o..o + 4].try_into().unwrap());
    assert_eq!((u32at(0), u32at(4)), (8, 5));
    assert_eq!(f32at(8), 2048.0);
    assert!((f32at(12) + 5.42).abs() < 0.01, "min y {}", f32at(12));
    assert!((f32at(16) - 55.14).abs() < 0.01, "max y {}", f32at(16));

    // grid x grid patches.
    let patches = lowr
        .child(b"PCHS")
        .expect("LOWR PCHS")
        .children_tagged(b"LPTC")
        .count();
    assert_eq!(patches, 64);
}

/// `PCHS -> LOWR` decoded: TAT1's far-LOD sheet (analysis/re-lowr.md §1/§4).
///
/// The counts are the session-14 census's, re-measured off the disc: 64
/// `LPTC`s, 2,326 vertices, 4,458 strip indices. The two claims that make it
/// a LOD rather than an outside fill are checked here too — every vertex is
/// inside the SHEET box, and the sheet sits ON the heightfield (the census
/// measured a 1.08 m median |dy| on this map, the worst of the sixteen; a
/// swapped axis or a wrong offset lands tens of metres out). Neither check can
/// see the one alternative divisor a parser would plausibly pick — /65535 for
/// /65536 moves a vertex at most 3 cm — so the divisor rests on
/// `analysis/re-lowr.md` §1, not on this test.
#[test]
fn lowr_sheet_is_a_far_lod_of_tat1s_own_ground() {
    use mungefs::terrain::Terrain;

    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tern = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("TAT1 has a tern");
    let t = Terrain::parse(&tern).expect("parses");
    let sheet = t.lowres.as_ref().expect("TAT1 ships a LOWR");

    assert_eq!((sheet.grid, sheet.resolution), (8, 5));
    assert_eq!(sheet.extent, 2048.0);
    assert_eq!(sheet.patches.len(), 64);
    assert_eq!(sheet.vertex_count(), 2326);
    // 4,458 strip indices unstitch to 3,226 triangles — cross-checked against
    // an independent python decode of the same bytes
    // (`engine/artifacts/lead_tmp/lowr-sheet/sheet_stats.py`).
    assert_eq!(sheet.triangle_count(), 3226);

    // extent = gridUnit * gridSize on every shipped sheet, so the box is the
    // terrain's own footprint.
    let half = t.info.grid_unit * t.info.grid_size as f32 / 2.0;
    assert_eq!(sheet.extent, t.info.grid_unit * t.info.grid_size as f32);
    let (mut dy, mut outside) = (Vec::new(), 0);
    for p in &sheet.patches {
        for v in &p.positions {
            let in_box = (-half..=sheet.extent - half).contains(&v[0])
                && (-half..=sheet.extent - half).contains(&v[2])
                && (sheet.y_min..=sheet.y_max).contains(&v[1]);
            if !in_box {
                outside += 1;
                continue;
            }
            // Cell under this vertex, clamped the way the PS2's own height
            // query clamps (re-terrain-heights.md §5).
            let gs = t.info.grid_size;
            let cx = (((v[0] + half) / t.info.grid_unit) as u32).min(gs - 1);
            let cz = (((v[2] + half - t.info.grid_unit) / t.info.grid_unit) as u32).min(gs - 1);
            dy.push((v[1] - t.world_y(cx, cz)).abs());
        }
    }
    assert_eq!(outside, 0, "every sheet vertex is inside the sheet box");
    dy.sort_by(f32::total_cmp);
    let median = dy[dy.len() / 2];
    assert!(
        median < 2.0,
        "sheet sits on the terrain: median |dy| {median}"
    );
}

/// `PCHS -> COLR` is `LUMI` in colour: the same lightmap, 5 bits per channel,
/// on the same grid. Reconstructing LUMI from COLR's green channel must land
/// within the 5-bit quantisation error — that is the proof of both the pixel
/// format (PSMCT16 R5G5B5A1) and the column-major patch/sample layout.
#[test]
fn terrain_colr_reconstructs_lumi() {
    use mungefs::terrain::Terrain;
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tern = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("TAT1 tern");
    let t = Terrain::parse(&tern).expect("terrain parses");

    let lumi = t.lumi.as_deref().expect("TAT1 has LUMI");
    let colr = t.colr.as_deref().expect("TAT1 has COLR");
    assert_eq!(colr.len(), lumi.len(), "same grid");

    // Green channel vs LUMI, mean absolute difference over the whole grid.
    let sum: u64 = colr
        .iter()
        .zip(lumi)
        .map(|(c, &l)| u64::from(c[1].abs_diff(l)))
        .sum();
    let mad = sum as f64 / lumi.len() as f64;
    assert!(
        mad < 6.0,
        "COLR green vs LUMI mean abs diff {mad:.2} of 255"
    );

    // TAT1's lightmap is grey: the channels agree with each other.
    let coloured = colr.iter().filter(|c| c[0].abs_diff(c[2]) > 16).count();
    assert!(
        coloured * 100 < colr.len(),
        "TAT1's lightmap should be essentially grey, {coloured} tinted"
    );
}

/// LUMI's orientation (card TLUMT): the lightmap as decoded ([z][x], like
/// `raw`) follows the relief — its best Pearson r against a hillshade of the
/// heights, over 72 azimuths × 4 elevations, is ~0.31 on TAT1, against ~0.06
/// for its transpose (the disc's x-major order read as [z][x]).
#[test]
fn terrain_lumi_follows_the_relief() {
    use mungefs::terrain::Terrain;
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let tern = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"tern")
        .expect("TAT1 tern");
    let t = Terrain::parse(&tern).expect("terrain parses");
    let n = t.info.grid_size as usize;
    let lumi = t.lumi.as_deref().expect("TAT1 has LUMI");
    let h = |x: usize, z: usize| t.world_y(x as u32, z as u32) as f64;
    let unit = f64::from(t.info.grid_unit);
    let normals: Vec<[f64; 3]> = (0..n * n)
        .map(|i| {
            let (x, z) = (i % n, i / n);
            let d = |a: usize, b: usize| (b - a).max(1) as f64 * unit;
            let (x0, x1) = (x.saturating_sub(1), (x + 1).min(n - 1));
            let (z0, z1) = (z.saturating_sub(1), (z + 1).min(n - 1));
            let gx = (h(x1, z) - h(x0, z)) / d(x0, x1);
            let gz = (h(x, z1) - h(x, z0)) / d(z0, z1);
            let l = (gx * gx + 1.0 + gz * gz).sqrt();
            [-gx / l, 1.0 / l, -gz / l]
        })
        .collect();
    let pearson = |a: &[f64], b: &[f64]| {
        let m = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (ma, mb) = (m(a), m(b));
        let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
        for (x, y) in a.iter().zip(b) {
            sab += (x - ma) * (y - mb);
            saa += (x - ma) * (x - ma);
            sbb += (y - mb) * (y - mb);
        }
        sab / (saa * sbb).sqrt()
    };
    let best_r = |l: &[f64]| {
        let mut best = -1.0f64;
        for az in (0..360).step_by(5) {
            for el in [20.0f64, 35.0, 50.0, 65.0] {
                let (a, e) = ((az as f64).to_radians(), el.to_radians());
                let s = [e.cos() * a.sin(), e.sin(), e.cos() * a.cos()];
                let hs: Vec<f64> = normals
                    .iter()
                    .map(|v| (v[0] * s[0] + v[1] * s[1] + v[2] * s[2]).max(0.0))
                    .collect();
                best = best.max(pearson(&hs, l));
            }
        }
        best
    };
    let as_decoded: Vec<f64> = lumi.iter().map(|&v| f64::from(v)).collect();
    let transposed: Vec<f64> = (0..n * n)
        .map(|i| f64::from(lumi[(i % n) * n + i / n]))
        .collect();
    let (r, rt) = (best_r(&as_decoded), best_r(&transposed));
    eprintln!("TAT1 LUMI vs hillshade: r={r:.3}, transposed r={rt:.3}");
    assert!(
        r > 0.25 && r > rt + 0.15,
        "LUMI vs hillshade: decoded r={r:.3}, transposed r={rt:.3}"
    );
}

/// Pinned `Hint` / `BARR` vectors from RE_LOG 2026-09-01
/// "`plan` / `plnp` / `Hint` / `BARR`". Both the CHUNK OFFSET and the decoded
/// values are asserted, so this is a byte oracle and not just a shape check.
#[test]
fn tat1_first_hint_and_barrier_are_the_pinned_bytes() {
    let buf = require_data!(tat1_bytes());
    let inner = open(&buf).unwrap();
    let wrld = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"wrld")
        .expect("wrld");
    // The FIRST Hint child sits at file offset 0x31F2C4, the first BARR at
    // 0x323400 (chunk.offset + 8 == the file offset).
    let first = |tag: &[u8; 4]| {
        wrld.children()
            .flatten()
            .find(|c| &c.tag == tag)
            .map(|c| c.offset + 8)
    };
    assert_eq!(first(b"Hint"), Some(0x31F2C4));
    assert_eq!(first(b"BARR"), Some(0x323400));

    let w = mungefs::world::World::parse(&wrld).unwrap();
    assert_eq!(w.hints.len(), 110);
    assert_eq!(w.barriers.len(), 592);

    let h = &w.hints[0];
    assert_eq!(h.kind, 2); // TYPE "2" = CoverHint
    assert_eq!(h.name, "hintnode60");
    let t = h.transform.translation;
    assert!((t[0] + 283.95245).abs() < 1e-3, "{t:?}");
    assert!((t[1] - 16.553478).abs() < 1e-3, "{t:?}");
    assert!((t[2] - 164.5722).abs() < 1e-3, "{t:?}");
    assert_eq!(h.primary_stance, Some(2));
    assert_eq!(h.secondary_stance, Some(9));
    assert_eq!(h.mode, Some(0));
    assert_eq!(h.radius, None);
    assert_eq!(h.command_post, None);
    assert_eq!(h.props.len(), 3);
    assert_eq!(h.props[0].key_hash, mungefs::world::HINT_PRIMARY_STANCE);
    assert_eq!(h.props[0].value, "2");

    let b = &w.barriers[0];
    assert_eq!(b.name, "barrier9");
    let t = b.transform.translation;
    assert!((t[0] + 100.21072).abs() < 1e-3, "{t:?}");
    assert_eq!(t[1], 0.0);
    // AS STORED here; the loaders (and bake::map) negate z for world space.
    assert!((t[2] - 251.00842).abs() < 1e-3, "{t:?}");
    let r = b.transform.rotation;
    assert!((r[0][0] + 0.42418706).abs() < 1e-5, "{r:?}");
    assert!((r[0][2] + 0.90557456).abs() < 1e-5, "{r:?}");
    assert_eq!(r[1], [0.0, 1.0, 0.0]);
    assert!((r[2][0] + 0.9055742).abs() < 1e-5, "{r:?}");
    assert!((r[2][2] - 0.4241879).abs() < 1e-5, "{r:?}");
    assert!((b.size[0] - 1.5746852).abs() < 1e-5, "{:?}", b.size);
    assert_eq!(b.size[1], 0.0);
    assert!((b.size[2] - 1.7452064).abs() < 1e-5, "{:?}", b.size);
    assert_eq!(b.flags, 31);
}

#[test]
fn tat2_first_hint_and_barrier_are_the_pinned_bytes() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/TAT/TAT2.LVL"
    );
    let buf = require_data!(std::fs::read(p).ok());
    let inner = open(&buf).unwrap();
    let wrld = chunks(inner)
        .flatten()
        .find(|c| &c.tag == b"wrld")
        .expect("wrld");
    let first = |tag: &[u8; 4]| {
        wrld.children()
            .flatten()
            .find(|c| &c.tag == tag)
            .map(|c| c.offset + 8)
    };
    assert_eq!(first(b"Hint"), Some(0x31389C));
    assert_eq!(first(b"BARR"), Some(0x31EEC4));

    let w = mungefs::world::World::parse(&wrld).unwrap();
    let h = &w.hints[0];
    assert_eq!(h.kind, 2);
    assert_eq!(h.name, "hintnode0");
    let t = h.transform.translation;
    assert!((t[0] - 126.6715).abs() < 1e-3, "{t:?}");
    assert!((t[1] - 0.052501).abs() < 1e-3, "{t:?}");
    assert!((t[2] - 84.41707).abs() < 1e-3, "{t:?}");
    assert_eq!(h.primary_stance, Some(3));
    assert_eq!(h.secondary_stance, Some(10));
    assert_eq!(h.mode, Some(0));

    let b = &w.barriers[0];
    assert_eq!(b.name, "barrier2");
    let t = b.transform.translation;
    assert!((t[0] - 86.981_9).abs() < 1e-3, "{t:?}");
    assert_eq!(t[1], 0.0);
    assert!((t[2] + 24.37025).abs() < 1e-3, "{t:?}"); // world z is +24.37
    assert!((b.size[0] - 8.071_342).abs() < 1e-5, "{:?}", b.size);
    assert_eq!(b.size[1], 0.0);
    assert!((b.size[2] - 8.618_071).abs() < 1e-5, "{:?}", b.size);
    assert_eq!(b.flags, 31);
}
