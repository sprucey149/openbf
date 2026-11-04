//! Tier-1 tests for `fx__ -> Effect("Water")` and the `ParticleEmitter`
//! effects (`mungefs::particle`, card FXP1) against real disc data
//! (gitignored; these SKIP cleanly when it is absent). Expected values are the
//! census in the `mungefs::fx` module header, which this test file's own
//! measurements produced.
//!
//! `Water::patch_cells` is pinned here too rather than beside the other
//! `WATR` tests: bit 3's meaning and the `fx__` keys were decoded by the same
//! RE pass (`analysis/re-water.md`, 2026-09-09) and a lane reading one wants
//! the other.

use mungefs::fx::{LightningBoltConfig, LightningConfig, WaterConfig};
use mungefs::particle::{BlendMode, ColorMode, Effect, GeometryType, Range};
use mungefs::{chunks, content_chunks, open, water::Water, Chunk};

use super::support::{lvl, tern_of, walk_lvls};

macro_rules! require_data {
    ($e:expr, $what:expr) => {
        match $e {
            Some(v) => v,
            None => {
                eprintln!("SKIP: extracted {} not present", $what);
                return;
            }
        }
    };
}

/// The map's water config, from whichever of its 3-9 `fx__` chunks carries
/// the `Effect("Water")` — the caller in `bake::map` scans them the same way.
fn water_config(buf: &[u8]) -> Option<WaterConfig> {
    let inner = open(buf).unwrap();
    chunks(inner)
        .flatten()
        .filter(|c| &c.tag == b"fx__")
        .find_map(|c| WaterConfig::parse(&c).unwrap())
}

const BF1: &str = "bf1/lvl/DATA/_LVL_PS2";
const BF2: &str = "bf2/lvl/DATA/_LVL_PS2";

#[test]
fn bf2_hoth_sun_flare_is_a_root_scope_with_its_own_angle() {
    let buf = require_data!(lvl(&format!("{BF2}/HOT/HOT1.LVL")), "BF2 HOT1.LVL");
    let config = content_chunks(open(&buf).unwrap())
        .unwrap()
        .iter()
        .filter(|c| &c.tag == b"fx__")
        .find_map(|c| mungefs::sun_flare::SunFlare::parse(c).unwrap())
        .unwrap();
    assert_eq!(config.angle, [115.0, -20.0]);
    assert_eq!(config.color, [255.0; 4]);
    assert_eq!(config.size, 5.0);
    assert_eq!(config.flare_out_size, 20.0);
    assert_eq!(config.num_flare_outs, 40);
    assert_eq!(config.initial_alpha, 70.0);
    assert_eq!(
        config.halo_rings,
        [
            [0.0, 255.0, 255.0, 255.0, 255.0],
            [10.0, 255.0, 200.0, 0.0, 255.0],
            [30.0, 255.0, 127.0, 0.0, 0.0]
        ]
    );
    assert_eq!(config.spike_color, [230.0, 230.0, 0.0, 128.0]);
    assert_eq!(config.spike_size, 20.0);
}

#[test]
fn bf2_geonosis_fog_cloud_reads_authored_parameters() {
    let buf = require_data!(lvl(&format!("{BF2}/GEO/GEO1.LVL")), "BF2 GEO1.LVL");
    let config = content_chunks(open(&buf).unwrap())
        .unwrap()
        .iter()
        .filter(|c| &c.tag == b"fx__")
        .find_map(|c| mungefs::fx::FogCloudConfig::parse(c).unwrap())
        .unwrap();
    assert!(config.enable);
    assert_eq!(config.texture.as_deref(), Some("brownmist"));
    assert_eq!(config.range, Some([90.0, 120.0]));
    assert_eq!(config.color, Some([168.0, 172.0, 180.0, 128.0]));
    assert_eq!(config.velocity, Some([5.0, 0.0]));
    assert_eq!(config.rotation, Some(0.1));
    assert_eq!(config.height, Some(16.0));
    assert_eq!(config.particle_size, Some(28.0));
    assert_eq!(config.particle_density, Some(100.0));
}

#[test]
fn bf2_hoth_fog_cloud_is_disabled() {
    let buf = require_data!(lvl(&format!("{BF2}/HOT/HOT1.LVL")), "BF2 HOT1.LVL");
    let config = content_chunks(open(&buf).unwrap())
        .unwrap()
        .iter()
        .filter(|c| &c.tag == b"fx__")
        .find_map(|c| mungefs::fx::FogCloudConfig::parse(c).unwrap())
        .unwrap();
    assert!(!config.enable);
}

/// Kamino is the map the `fx__` config changes most: `LAYR` names
/// `kam1_water`, which is the FOAM texture, and the surface is painted with
/// `MainTexture` = `kam1_water_2` (`re-water.md` §3).
#[test]
fn bf1_kam1_names_its_surface_and_its_foam_apart() {
    let buf = require_data!(lvl(&format!("{BF1}/KAM/KAM1.LVL")), "BF1 KAM1.LVL");
    let w = water_config(&buf).expect("KAM1 has an Effect(\"Water\")");

    assert_eq!(w.main_texture.as_deref(), Some("kam1_water_2"));
    assert_eq!(w.foam_texture.as_deref(), Some("kam1_water"));
    assert_eq!(w.ocean_enable, Some(1.0), "Kamino is the ocean map");
    assert_eq!(w.tile, Some([4.0, 4.0]));
    assert_eq!(w.foam_tile, Some([5.0, 5.0]));
    assert_eq!(w.velocity, Some([0.01, 0.05]));
    assert_eq!(w.phillips_constant, Some(1.0e-5));
    assert_eq!(w.wind_speed, Some(25.0));
    assert!(!w.disable_low_res, "Kamino draws far water");

    // The `LAYR` copy disagrees, which is the whole point of reading `fx__`.
    let watr = Water::parse(&tern_of(&buf)).unwrap().unwrap();
    assert_eq!(watr.layer.texture, "kam1_water");
    assert_eq!(watr.info.cell_size, 160.0);
    assert_eq!(watr.info.unknown_5, 1);
    // 160 m cells, Tile 4, INFO5 1 -> one repeat every 40 m, against the
    // 160 m the pre-`fx__` rule took from `LAYR`'s uv_tile of 1.0.
    assert_eq!(
        w.metres_per_repeat(watr.info.cell_size, watr.info.unknown_5),
        Some([40.0, 40.0])
    );

    // The explicit-list form of `SpeckleTextures` (both KAM1s are the only
    // maps that use it).
    let s = w.speckle.expect("KAM1 authors a speckle layer");
    assert_eq!(
        s.textures,
        [
            "water_specles0",
            "water_specles1",
            "water_specles2",
            "water_specles3"
        ]
    );
    assert_eq!(s.fps, None);
}

/// Kashyyyk Docks is the map whose `INFO5` is not 1: 128 m cells with
/// `INFO5` 4 and `Tile(2,2)` make one repeat every 16 m (`re-water.md` §3),
/// where the pre-`fx__` rule read `LAYR`'s uv_tile of 1.0 and got 128 m.
#[test]
fn bf1_kas2_tile_takes_the_info5_multiplier() {
    let buf = require_data!(lvl(&format!("{BF1}/KAS/KAS2.LVL")), "BF1 KAS2.LVL");
    let w = water_config(&buf).expect("KAS2 has an Effect(\"Water\")");
    let watr = Water::parse(&tern_of(&buf)).unwrap().unwrap();

    assert_eq!(w.tile, Some([2.0, 2.0]));
    assert_eq!(watr.info.cell_size, 128.0);
    assert_eq!(watr.info.unknown_5, 4, "INFO5 read off the disc");
    assert_eq!(
        w.metres_per_repeat(watr.info.cell_size, watr.info.unknown_5),
        Some([16.0, 16.0])
    );
    assert_eq!(w.velocity, Some([0.05, 0.05]));
    assert_eq!(
        w.scroll_per_second(watr.info.unknown_5),
        Some([0.0125, 0.0125])
    );
    // `LAYR` names no texture at all on this map; `MainTexture` does.
    assert_eq!(watr.layer.texture, "");
    assert_eq!(w.main_texture.as_deref(), Some("kas2_water"));
    assert_eq!(w.ocean_enable, Some(0.0));

    // The prefix form of `SpeckleTextures`, expanded to the names the bake
    // writes (`bf1_kas2/manifest.ron` lists exactly these 25).
    let s = w.speckle.expect("KAS2 authors a speckle layer");
    assert_eq!(s.textures.len(), 25);
    assert_eq!(s.textures[0], "water_specularmask_0");
    assert_eq!(s.textures[24], "water_specularmask_24");
    assert_eq!(s.fps, Some(4.0));
}

/// `DisableLowRes` is a bare flag, and which maps set it decides which draw
/// far water at all (`re-water.md` §1). This is also the check that found
/// the §5 table's omission: **BF2 DAG1 authors it too.**
#[test]
fn disable_low_res_is_authored_by_four_maps() {
    let maps: [(&str, bool); 12] = [
        ("bf1 KAM/KAM1", false),
        ("bf1 KAS/KAS1", true),
        ("bf1 KAS/KAS2", false),
        ("bf1 NAB/NAB2", false),
        ("bf1 YAV/YAV1", true),
        ("bf2 KAM/KAM1", false),
        ("bf2 KAS/KAS2", false),
        ("bf2 NAB/NAB2", false),
        ("bf2 YAV/YAV1", true),
        ("bf2 DEA/DEA1", false),
        ("bf2 FEL/FEL1", false),
        ("bf2 DAG/DAG1", true),
    ];
    for (key, want) in maps {
        let (disc, rel) = key.split_once(' ').unwrap();
        let root = if disc == "bf1" { BF1 } else { BF2 };
        let name = format!("{}.LVL", rel.split('/').next_back().unwrap());
        let buf = require_data!(lvl(&format!("{root}/{rel}.LVL")), name);
        let w = water_config(&buf).unwrap_or_else(|| panic!("{key} has a Water effect"));
        assert_eq!(w.disable_low_res, want, "{key} DisableLowRes");
        // Every one of the twelve names its surface texture.
        assert!(w.main_texture.is_some(), "{key} MainTexture");
        // No map on either disc authors SpecularEnable — which is what makes
        // BF2's far water unfogged and its speckle layer absent (§1).
        assert_eq!(w.specular_enable, None, "{key} SpecularEnable");
    }
}

/// A map with no water carries `fx__` chunks all the same (`ColorControl`,
/// `Wind`, `Lightning`...), and none of them is a `Water` effect.
#[test]
fn a_dry_map_has_no_water_effect() {
    for (root, rel) in [(BF1, "TAT/TAT1"), (BF1, "HOT/HOT1"), (BF2, "TAT/TAT2")] {
        let buf = require_data!(lvl(&format!("{root}/{rel}.LVL")), rel);
        assert!(water_config(&buf).is_none(), "{rel}");
    }
}

/// `WMAP` bit 3 means "an `LPTC` for this cell follows in `CPAT`"
/// (`re-water.md` §4): the bit-3 cell count equals the patch count on all 14
/// shipped chunks, which is the evidence for the bit's meaning.
#[test]
fn bit_three_cells_match_the_patch_count() {
    let expect: [(&str, &str, usize); 12] = [
        (BF1, "KAM/KAM1", 0),
        (BF1, "KAS/KAS1", 0),
        (BF1, "KAS/KAS2", 0),
        (BF1, "NAB/NAB2", 4),
        (BF1, "YAV/YAV1", 0),
        (BF2, "KAM/KAM1", 0),
        (BF2, "KAS/KAS2", 0),
        (BF2, "NAB/NAB2", 8),
        (BF2, "YAV/YAV1", 0),
        (BF2, "DEA/DEA1", 1),
        (BF2, "FEL/FEL1", 0),
        (BF2, "DAG/DAG1", 0),
    ];
    for (root, rel, patches) in expect {
        let buf = require_data!(lvl(&format!("{root}/{rel}.LVL")), rel);
        let w = Water::parse(&tern_of(&buf)).unwrap().unwrap();
        assert_eq!(w.patches.len(), patches, "{rel} LPTC count");
        assert_eq!(w.patch_cells(), patches, "{rel} bit-3 cells == LPTC count");
    }
}

/// Every `.LVL` under `extracted/<game>/lvl`, sorted by path (the census
/// order of `lead_tmp/fxp/census2.py`).
fn all_lvls(game: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let root = format!(
        "{}/../../../extracted/{game}/lvl",
        env!("CARGO_MANIFEST_DIR")
    );
    walk_lvls(std::path::Path::new(&root), &mut out);
    out.sort();
    out
}

/// Every particle effect on a disc, once per NAME (first copy in path
/// order), with the LVL it came from.
fn disc_effects(game: &str) -> Vec<(String, Effect)> {
    let mut out: Vec<(String, Effect)> = Vec::new();
    for p in all_lvls(game) {
        let buf = std::fs::read(&p).unwrap();
        let Ok(inner) = open(&buf) else { continue };
        for c in content_chunks(inner).unwrap() {
            if &c.tag != b"fx__" {
                continue;
            }
            if let Some(e) = Effect::parse(&c).unwrap() {
                if !out.iter().any(|(_, x)| x.name_hash == e.name_hash) {
                    out.push((p.display().to_string(), e));
                }
            }
        }
    }
    out
}

/// re-fx §1's census: effects, emitters, types and blends per disc, and no
/// key the parsers do not know.
#[test]
fn particle_census_matches_re_fx() {
    // (game, effects, emitters, [PARTICLE incl. no Type, STREAK, EMITTER,
    // ANIMATED, GEOMETRY, BILLBOARD, SPARK], [NORMAL incl. unauthored,
    // ADDITIVE, BLUR])
    for (game, n_fx, n_em, types, blends) in [
        (
            "bf1",
            128,
            701,
            [518 + 6, 2, 48, 1, 39, 50, 37],
            [197 + 4, 487, 13],
        ),
        (
            "bf2",
            292,
            1702,
            [1104, 6, 85, 7, 212, 113, 175],
            [473, 1208, 21],
        ),
    ] {
        let fx = disc_effects(game);
        if fx.is_empty() {
            eprintln!("SKIP: extracted {game} not present");
            return;
        }
        let ems: Vec<_> = fx.iter().flat_map(|(_, e)| e.all_emitters()).collect();
        let mut t = [0usize; 7];
        let mut b = [0usize; 3];
        for e in &ems {
            t[e.geometry.kind as usize] += 1;
            b[e.geometry.blend as usize] += 1;
            assert!(
                e.unknown.is_empty(),
                "{game}: unknown keys {:x?}",
                e.unknown
            );
        }
        eprintln!(
            "{game}: {} effects, {} emitters, types {t:?}, blends {b:?}",
            fx.len(),
            ems.len()
        );
        assert_eq!((fx.len(), ems.len()), (n_fx, n_em), "{game}");
        assert_eq!(t, types, "{game} types");
        assert_eq!(b, blends, "{game} blends");
    }
}

/// BF1 COMMON's `ord_explosion` as re-fx §1 lists it: `Sparks` (2 ANIMATED
/// `a_sparks3`, 0.2 s, colour Reach white → orange, ADDITIVE) with a nested
/// `Flare` (1 `boom4` PARTICLE growing ×3 in 0.1 s, alpha 200 → 0).
#[test]
fn ord_explosion_has_sparks_and_a_flare() {
    let buf = require_data!(
        lvl(&format!("{BF1}/COMMON/ENG/COMMON.LVL")),
        "BF1 COMMON.LVL"
    );
    let inner = open(&buf).unwrap();
    let e = content_chunks(inner)
        .unwrap()
        .into_iter()
        .filter(|c| &c.tag == b"fx__")
        .find_map(|c| {
            Effect::parse(&c)
                .unwrap()
                .filter(|e| e.name_hash == 0xc42b_8278)
        })
        .expect("ord_explosion");
    assert_eq!(e.emitters.len(), 1);
    let s = &e.emitters[0];
    assert_eq!(s.label, "Sparks");
    assert_eq!(s.max_particles, Range::fixed(2.0));
    assert_eq!(s.burst_count, Range::fixed(2.0));
    assert_eq!(s.geometry.kind, GeometryType::Animated);
    assert_eq!(s.geometry.blend, BlendMode::Additive);
    assert_eq!(s.geometry.texture.as_deref(), Some("a_sparks3"));
    assert_eq!(
        (
            s.geometry.time_per_frame,
            s.geometry.frame_size,
            s.geometry.total_frames
        ),
        (0.015, 4.0, 16.0)
    );
    assert_eq!(s.transformer.lifetime, 0.2);
    assert_eq!(
        s.color_mode,
        ColorMode::Hsv,
        "the emitter level authors Hue"
    );
    assert_eq!(
        s.spawner.color_mode,
        ColorMode::Rgb,
        "the Spawner authors Red"
    );
    let c = &s.transformer.color;
    assert_eq!(c.len(), 2, "Reach white, Next Reach orange");
    assert_eq!(c[0].value, [1.0; 4]);
    let k = |v: f32| mungefs::particle::ps2_mul(v, mungefs::particle::COLOR_SCALE);
    assert_eq!(c[1].value, [1.0, k(128.0), 0.0, 1.0]);
    assert_eq!((c[0].lifetime, c[1].lifetime), (0.01, 0.19));

    let f = s.sibling.as_deref().expect("the nested Flare");
    assert_eq!(f.label, "Flare");
    assert_eq!(f.max_particles, Range::fixed(1.0));
    assert_eq!(f.geometry.kind, GeometryType::Particle);
    assert_eq!(f.geometry.texture.as_deref(), Some("boom4"));
    assert_eq!(f.transformer.lifetime, 0.1);
    assert_eq!(f.transformer.size[0].value, 3.0);
    assert_eq!(f.transformer.size[0].lifetime, 0.1);
    assert_eq!(f.spawner.color[3], Range::fixed(k(200.0)));
    assert_eq!(f.transformer.color[0].value[3], 0.0);
    assert_eq!(e.all_emitters().len(), 2);
}

/// re-fx §4, as a test: 20 BF1 emitter templates read out of the PS2's own
/// RAM (NAB1 spawn state, `tests/data/fx_ram_bf1_nab1.json`, made by
/// `lead_tmp/fxp1/ramvec.py`), each matched to its disc emitter, compared
/// field by field with what this parser builds — every emitter, Spawner,
/// Transformer-chain and Geometry field the PS2 keeps, not only the ones the
/// match used.
#[test]
fn twenty_templates_match_the_ps2_ram() {
    use mungefs::particle::{Emitter, Shape};
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../data/fx_ram_bf1_nab1.json")).unwrap();
    let f = |x: &serde_json::Value| x.as_f64().unwrap() as f32;
    let u = |x: &serde_json::Value| x.as_u64().unwrap() as u32;
    let range = |x: &serde_json::Value, r: Range, what: &str| {
        assert_eq!((f(&x[0]), f(&x[1])), (r.base, r.span), "{what}");
        assert_eq!(u(&x[2]) != 0, r.random(), "{what} random word");
    };
    let mut lvls: Vec<(String, Vec<u8>)> = Vec::new();
    let templates = v["templates"].as_array().unwrap();
    assert_eq!(templates.len(), 20);
    for t in templates {
        let lvl_name = t["lvl"].as_str().unwrap();
        let rel = match lvl_name {
            "COMMON" => format!("{BF1}/COMMON/ENG/COMMON.LVL"),
            _ => format!("{BF1}/NAB/NAB1.LVL"),
        };
        if !lvls.iter().any(|(n, _)| n == lvl_name) {
            let buf = require_data!(lvl(&rel), rel);
            lvls.push((lvl_name.to_owned(), buf));
        }
        let buf = &lvls.iter().find(|(n, _)| n == lvl_name).unwrap().1;
        let hash = u(&t["effect"]);
        let fx = content_chunks(open(buf).unwrap())
            .unwrap()
            .into_iter()
            .filter(|c| &c.tag == b"fx__")
            .find_map(|c| Effect::parse(&c).unwrap().filter(|e| e.name_hash == hash))
            .unwrap_or_else(|| panic!("effect {hash:#x} in {lvl_name}"));
        let all = fx.all_emitters();
        let e: &Emitter = all[t["index"].as_u64().unwrap() as usize];
        let id = format!("{lvl_name} {hash:#x}#{} {}", t["index"], e.label);
        assert_eq!(e.label, t["label"].as_str().unwrap(), "{id}");
        let r = &t["ram"];
        range(
            &r["start_delay"],
            e.start_delay,
            &format!("{id} StartDelay"),
        );
        range(
            &r["burst_delay"],
            e.burst_delay,
            &format!("{id} BurstDelay"),
        );
        range(
            &r["burst_count"],
            e.burst_count,
            &format!("{id} BurstCount"),
        );
        range(
            &r["max_particles"],
            e.max_particles,
            &format!("{id} MaxParticles"),
        );
        for i in 0..4 {
            range(&r["color"][i], e.color[i], &format!("{id} colour {i}"));
        }
        assert_eq!(u(&r["color_mode"]), e.color_mode as u32, "{id} mode");
        range(&r["size"], e.size, &format!("{id} Size"));
        assert_eq!(f(&r["min_lod_dist"]), e.min_lod_dist, "{id}");
        assert_eq!(f(&r["max_lod_dist"]), e.max_lod_dist, "{id}");
        assert_eq!(f(&r["bounding_radius"]), e.bounding_radius, "{id}");
        assert_eq!(u(&r["no_register_step"]) != 0, e.no_register_step, "{id}");
        assert_eq!(u(&r["world_aligned"]) != 0, e.world_aligned, "{id}");

        let (s, rs) = (&e.spawner, &r["spawner"]);
        let shape = [
            Shape::None,
            Shape::Spread,
            Shape::Vortex,
            Shape::Circle,
            Shape::Type4,
        ];
        assert_eq!(shape[u(&rs["shape"]) as usize], s.shape, "{id} shape");
        for i in 0..9 {
            range(
                &rs["shape_ranges"][i],
                s.shape_ranges[i],
                &format!("{id} shape slot {i}"),
            );
        }
        for i in 0..3 {
            range(&rs["offset"][i], s.offset[i], &format!("{id} Offset {i}"));
        }
        range(
            &rs["position_scale"],
            s.position_scale,
            &format!("{id} PositionScale"),
        );
        range(
            &rs["velocity_scale"],
            s.velocity_scale,
            &format!("{id} VelocityScale"),
        );
        range(
            &rs["inherit_velocity_factor"],
            s.inherit_velocity_factor,
            &format!("{id} Inherit"),
        );
        range(&rs["size"], s.size, &format!("{id} S.Size"));
        for i in 0..4 {
            range(&rs["color"][i], s.color[i], &format!("{id} S.colour {i}"));
        }
        assert_eq!(u(&rs["color_mode"]), s.color_mode as u32, "{id} S.mode");
        assert_eq!(f(&rs["fade_in_time"]), s.fade_in_time, "{id}");
        range(
            &rs["start_rotation"],
            s.start_rotation,
            &format!("{id} StartRotation"),
        );
        range(
            &rs["rotation_velocity"],
            s.rotation_velocity,
            &format!("{id} RotationVelocity"),
        );

        let (tr, rt) = (&e.transformer, &r["transformer"]);
        assert_eq!(f(&rt["lifetime"]), tr.lifetime, "{id} LifeTime");
        let pos = rt["position"].as_array().unwrap();
        assert_eq!(pos.len(), tr.position.len(), "{id} Position chain");
        for (x, n) in pos.iter().zip(&tr.position) {
            assert_eq!(f(&x[0]), n.lifetime, "{id}");
            assert_eq!(u(&x[1]), n.op as u32, "{id} Position op");
            // An op-less node's words and Scale's y/z are never written.
            let used = match n.op as u32 {
                0 => 0,
                3 => 1,
                _ => 3,
            };
            for i in 0..used {
                assert_eq!(f(&x[2 + i]), n.value[i], "{id} Position value");
            }
        }
        for (key, chain) in [("size", &tr.size), ("rotation", &tr.rotation)] {
            let xs = rt[key].as_array().unwrap();
            assert_eq!(xs.len(), chain.len(), "{id} {key} chain");
            for (x, n) in xs.iter().zip(chain) {
                assert_eq!(f(&x[0]), n.lifetime, "{id}");
                assert_eq!(u(&x[1]), n.op as u32, "{id} {key} op");
                if n.op as u32 != 0 {
                    assert_eq!(f(&x[2]), n.value, "{id} {key} value");
                }
            }
        }
        let col = rt["color"].as_array().unwrap();
        assert_eq!(col.len(), tr.color.len(), "{id} Color chain");
        for (x, n) in col.iter().zip(&tr.color) {
            assert_eq!(f(&x[0]), n.lifetime, "{id}");
            assert_eq!(u(&x[1]), n.op as u32, "{id} Color op");
            if n.op as u32 != 0 {
                for i in 0..4 {
                    assert_eq!(f(&x[2 + i]), n.value[i], "{id} Color value");
                }
            }
        }

        let (g, rg) = (&e.geometry, &r["geometry"]);
        assert_eq!(u(&rg["blend"]), g.blend as u32, "{id} blend");
        assert_eq!(u(&rg["kind"]), g.kind as u32, "{id} Type");
        assert_eq!(f(&rg["time_per_frame"]), g.time_per_frame, "{id}");
        assert_eq!(f(&rg["frame_size"]), g.frame_size, "{id}");
        assert_eq!(f(&rg["total_frames"]), g.total_frames, "{id}");
        assert_eq!(f(&rg["blur_value"]), g.blur_value, "{id}");
        assert_eq!(u(&rg["blur_res"]) as i32, g.blur_res, "{id}");
        assert_eq!(f(&rg["spark_length"]), g.spark_length, "{id}");
    }
}

/// Card KSKY (`analysis/re-kamino.md` §1): the three maps that author an
/// enabled `Effect("Lightning")` rest their sky dome at 153, both Kaminos
/// carry the `skybolt`, and a map without the effect parses to none.
#[test]
fn lightning_is_authored_on_kamino_and_bf1_yavin_2() {
    let lightning = |buf: &[u8]| {
        let inner = open(buf).unwrap();
        let fx: Vec<Chunk<'_>> = chunks(inner)
            .flatten()
            .filter(|c| &c.tag == b"fx__")
            .collect();
        let l = fx.iter().find_map(|c| LightningConfig::parse(c).unwrap());
        let bolts: Vec<LightningBoltConfig> = fx
            .iter()
            .flat_map(|c| LightningBoltConfig::parse_all(c).unwrap())
            .collect();
        (l, bolts)
    };
    for (rel, crack, bolts) in [
        (format!("{BF1}/KAM/KAM1.LVL"), "kam_amb_thunder", 1),
        (format!("{BF2}/KAM/KAM1.LVL"), "kam_amb_thunder", 1),
        (format!("{BF1}/YAV/YAV2.LVL"), "yav_amb_thunder", 0),
    ] {
        let buf = require_data!(lvl(&rel), rel);
        let (l, b) = lightning(&buf);
        let l = l.unwrap_or_else(|| panic!("{rel}: no Lightning"));
        assert!(l.enable, "{rel}");
        assert_eq!(l.sky_dome_darken_factor, 0.4, "{rel}");
        assert_eq!(l.resting_dome_colour(), Some(153), "{rel}");
        assert_eq!(l.sunlight_fade_factor, 0.1, "{rel}");
        assert_eq!(l.time_between_flashes, [3.0, 5.0], "{rel}");
        assert_eq!(l.num_sub_flashes, [2.0, 5.0], "{rel}");
        assert_eq!(l.sound_crack.as_deref(), Some(crack), "{rel}");
        assert_eq!(b.len(), bolts, "{rel}");
        if let Some(b) = b.first() {
            assert_eq!(b.name, "skybolt");
            assert_eq!(b.texture.as_deref(), Some("lightning"));
            assert_eq!(b.child_color, Some([255.0, 255.0, 255.0, 150.0]));
        }
    }
    let rel = format!("{BF1}/KAS/KAS2.LVL");
    let buf = require_data!(lvl(&rel), rel);
    let (l, b) = lightning(&buf);
    assert_eq!(l, None);
    assert!(b.is_empty());
}
