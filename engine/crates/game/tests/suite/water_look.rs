//! Data-gated: the twelve baked water maps get the look the disc authors
//! (queue item W2; the rule is `game::viewer::water_look`, its RE is
//! `analysis/re-water.md` §1/§3 [V] and the per-map census is §5).
//!
//! The unit tests beside the rule prove the ARITHMETIC on synthetic numbers.
//! This is the other half — that the numbers actually reaching it out of the
//! bake are the disc's, per map: which maps fog their water and from where,
//! how far the surface reaches, how translucent it is and its colour. A bake change that
//! silently dropped `fx__`'s LUT or its `DisableLowRes` would leave the unit
//! tests green and only fail here. Where the water reaches past its fog
//! crossover, the baked scene fog must begin no earlier than the crossover,
//! or the far water steps into fog at the seam. BF2 FEL1 fogs from 0 m but
//! has no far layer (`FarSceneRange(0)`, water ends at the crossover), so
//! it has no seam (`analysis/re-fog.md` §2, `re-water-flags.md` §5).
//!
//! Skips cleanly when the maps are not baked (`scripts/rebake.sh`).

use game::assets::Manifest;
use game::viewer::water_look;

/// The manifest alone — this test needs no meshes, and `load_scene` over
/// thirty-two maps would read a gigabyte of GLB to answer four numbers.
fn manifest_of(dir: &std::path::Path) -> Manifest {
    let txt = std::fs::read_to_string(dir.join("manifest.ron"))
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    ron::from_str(&txt).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
}

/// `(map, fog_begin, far, alpha, foam)` — the four numbers per water map,
/// read off the baked manifests on 2026-09-09 and derived by the ELF's rule:
///
/// * `fog_begin` = `NearSceneRange.c` on a map that draws a far pass of
///   water, and `None` everywhere else — BF2's FLAT sub-pass is gated on
///   `SpecularEnable`, which no disc map authors but the level reset sets to 1
///   (`analysis/re-water-flags.md`, `[O]`; queue WSE, 2026-09-25), an ocean's
///   far pass is fogged in both games (`analysis/re-kamino.md` §2 `[O]`, card
///   KSEA), and a `DisableLowRes` map has no far pass to fog;
/// * `far` = `0.9 · FarSceneRange` (`0.8` for an ocean), floored at
///   `NearSceneRange.c`, or exactly `c` on a `DisableLowRes` map;
/// * `alpha` = `mean(Min.a, Max.a) / 255`, or 1 on an ocean map (its base
///   record is opaque);
/// * `foam` = the map authors `OceanEnable` and a `FoamTexture`.
const EXPECTED: &[(&str, Option<f32>, f32, f32, bool)] = &[
    ("bf1_kam1", Some(90.0), 640.0, 1.0, true),
    ("bf1_kas1", None, 180.0, 80.0 / 255.0, false),
    ("bf1_kas2", Some(100.0), 1350.0, 188.0 / 255.0, false),
    ("bf1_nab2", Some(150.0), 2700.0, 128.0 / 255.0, false),
    ("bf1_yav1", None, 100.0, 1.0, false),
    ("bf2_dag1", None, 60.0, 1.0, false),
    ("bf2_dea1", Some(180.0), 180.0, 150.0 / 255.0, false),
    ("bf2_fel1", Some(52.0), 52.0, 188.0 / 255.0, false),
    ("bf2_kam1", Some(90.0), 640.0, 1.0, true),
    ("bf2_kas2", Some(100.0), 4500.0, 1.0, false),
    ("bf2_nab2", Some(150.0), 150.0, 128.0 / 255.0, false),
    ("bf2_yav1", None, 100.0, 1.0, false),
];

#[test]
fn every_water_map_takes_the_look_its_disc_authors() {
    let root = game::assets::baked_root();
    let maps = game::assets::discover_maps(&root);
    if maps.is_empty() {
        eprintln!("SKIP: no baked maps under {}", root.display());
        return;
    }
    let mut seen = 0;
    for (key, _, path) in &maps {
        let manifest = manifest_of(path);
        let Some(water) = manifest.water.as_ref() else {
            assert!(
                !EXPECTED.iter().any(|e| e.0 == key.as_str()),
                "{key} ships a WATR and the bake dropped it"
            );
            continue;
        };
        let row = EXPECTED
            .iter()
            .find(|e| e.0 == key.as_str())
            .unwrap_or_else(|| panic!("{key} has water and is not in the census"));
        let look = water_look(&manifest.source_game, &manifest.lighting, water);
        if let (Some(fog), Some(begin), Some(far)) =
            (manifest.lighting.fog.as_ref(), look.fog_begin, look.far)
        {
            assert!(
                far <= begin || fog.near >= begin,
                "{key}: scene fog starts at {} before water fog crossover {begin} \
                 (water reaches {far})",
                fog.near
            );
        }
        assert_eq!(look.fog_begin, row.1, "{key}: water fog crossover");
        assert!(
            look.far.is_some_and(|f| (f - row.2).abs() < 1e-3),
            "{key}: water reaches {:?}, expected {}",
            look.far,
            row.2
        );
        assert!(
            (look.tint[3] - row.3).abs() < 1e-6,
            "{key}: blend weight {}, expected {}",
            look.tint[3],
            row.3
        );
        assert_eq!(look.foam_tint.is_some(), row.4, "{key}: ocean foam layer");
        // The colour is the LUT's mean, `MaxDiffuse` / 255 in GS terms, on
        // flat water, and unity on an ocean (`re-water-colour.md` §1.5/§1.9).
        let max = water.config.as_ref().and_then(|c| c.max_diffuse);
        let want = match (row.4, max) {
            (false, Some(m)) => [m[0] / 255.0, m[1] / 255.0, m[2] / 255.0],
            _ => [1.0; 3],
        };
        assert_eq!(look.tint[..3], want, "{key}: tint");
        // BF1's ocean is the unity draw; BF2's is lit, with its foam at the
        // fitted weight and a loss of colour from FogRange's near distance
        // (card KSEAL). No flat water is lit.
        let lit = row.4 && manifest.source_game != "bf1";
        if row.4 {
            let foam = look.foam_tint.expect("checked above");
            assert_eq!(foam[..3], [1.0; 3], "{key}: foam colour at unity");
            assert_eq!(foam[3] < 1.0, lit, "{key}: foam weight {}", foam[3]);
        }
        assert_eq!(look.sun > 0.0, lit, "{key}: sun weight {}", look.sun);
        assert_eq!(look.dim.is_some(), lit, "{key}: far loss {:?}", look.dim);
        seen += 1;
    }
    assert_eq!(seen, EXPECTED.len(), "the disc ships twelve water maps");

    // BF2 KAS2 is one of the two maps with their own LUT (30/30/30 to
    // 70/70/70, a grey where the others are green-blue): its mean is
    // MaxDiffuse, 70/255. Spot-checked against the literal so a bake that
    // lost the LUT cannot pass by agreeing with itself above.
    let path = root.join("bf2_kas2");
    if path.join("manifest.ron").exists() {
        let m = manifest_of(&path);
        let look = water_look(
            &m.source_game,
            &m.lighting,
            m.water.as_ref().expect("bf2_kas2 water"),
        );
        for c in 0..3 {
            assert!(
                (look.tint[c] - 70.0 / 255.0).abs() < 1e-6,
                "bf2_kas2 tint {:?}",
                look.tint
            );
        }
    }
}

#[test]
fn water_hdr_gain_only_applies_to_bf2_flat_water_on_hdr_maps() {
    let path = game::assets::baked_root().join("bf2_kas2");
    if !path.join("manifest.ron").exists() {
        eprintln!("SKIP: no baked BF2 Kashyyyk");
        return;
    }
    let mut m = manifest_of(&path);
    assert_eq!(game::water_anim::flat_display_gain(&m), 255.0 / 128.0);
    m.water.as_mut().unwrap().config.as_mut().unwrap().ocean = true;
    assert_eq!(game::water_anim::flat_display_gain(&m), 1.0);
    m.water.as_mut().unwrap().config.as_mut().unwrap().ocean = false;
    let hdr = m.hdr.take();
    assert_eq!(game::water_anim::flat_display_gain(&m), 1.0);
    m.hdr = hdr;
    m.source_game = "bf1".into();
    assert_eq!(game::water_anim::flat_display_gain(&m), 1.0);
    m.source_game = "bf2".into();
    m.water = None;
    assert_eq!(game::water_anim::flat_display_gain(&m), 1.0);
}

#[test]
fn kashyyyk_far_water_uses_32_metre_colour_cells_and_the_authored_crossover() {
    let path = game::assets::baked_root().join("bf2_kas2");
    if !path.join("manifest.ron").exists() {
        eprintln!("SKIP: no baked BF2 Kashyyyk");
        return;
    }
    let manifest = manifest_of(&path);
    let mut water = manifest.water.unwrap();
    if !water.vertex_diffuse {
        eprintln!("SKIP: BF2 Kashyyyk requires the vertex diffuse rebake");
        return;
    }
    let mesh = game::assets::load_glb(&path.join(&water.glb)).unwrap();
    let range = &manifest.lighting;
    let far = game::water_anim::flat_far_diffuse(&mesh, &water, range).unwrap();
    assert_eq!(far.dims, [4, 4]);
    assert_eq!(far.step, [32.0, 32.0]);
    assert_eq!(far.range, [85.0, 100.0]);
    let rgb: Vec<_> = far
        .colors
        .iter()
        .flatten()
        .take(16)
        .map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(
        rgb.iter().map(|c| c[0]).collect::<Vec<_>>(),
        [94, 102, 110, 54, 38, 38, 70, 102, 46, 62, 110, 62, 62, 38, 38, 62]
    );
    assert!(rgb
        .iter()
        .all(|c| c[0] >= 30 && c[0] <= 110 && c[0] == c[1] && c[1] == c[2]));
    assert!(
        rgb.iter().map(|c| c[0]).max().unwrap() - rgb.iter().map(|c| c[0]).min().unwrap() >= 64
    );
    water.config.as_mut().unwrap().lod_decimation = Some(100.0);
    assert_eq!(
        game::water_anim::flat_far_diffuse(&mesh, &water, range).unwrap(),
        far
    );
    water.config.as_mut().unwrap().disable_low_res = true;
    assert!(game::water_anim::flat_far_diffuse(&mesh, &water, range).is_none());
    water.config.as_mut().unwrap().disable_low_res = false;
    water.config.as_mut().unwrap().ocean = true;
    assert!(game::water_anim::flat_far_diffuse(&mesh, &water, range).is_none());
    water.config.as_mut().unwrap().ocean = false;
    water.vertex_diffuse = false;
    assert!(game::water_anim::flat_far_diffuse(&mesh, &water, range).is_none());
}
