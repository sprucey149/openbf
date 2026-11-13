use super::*;

const HDR: manifest::ManifestHdr = manifest::ManifestHdr {
    num_bloom_passes: 5,
    max_total_weight: 1.2,
    glow_threshold: 0.5,
};

/// Mustafar's tune softens its bloom and self-lit gain; the HDR block's
/// other numbers are the disc's.
#[test]
fn mustafar_takes_its_softer_look() {
    let t = look_tune("bf2", "mus1").expect("mus1 is tuned");
    assert_eq!((t.bloom_weight, t.glow_gain), (0.93, 0.9));
    let b = bloom_from(HDR, Some(t));
    assert_eq!((b.passes, b.threshold), (5, 0.5));
    assert!((b.weight - 1.116).abs() < 1e-5, "{}", b.weight);
}

/// Every other map, HDR ones with Mustafar's very block included, draws
/// exactly as authored.
#[test]
fn no_other_map_is_tuned() {
    assert_eq!(LOOK_TUNES.len(), 1);
    for (game, map) in [
        ("bf2", "kas2"),
        ("bf2", "nab2"),
        ("bf2", "fel1"),
        ("bf1", "mus1"),
    ] {
        assert_eq!(look_tune(game, map), None, "{game} {map}");
        let b = bloom_from(HDR, look_tune(game, map));
        assert_eq!(
            b,
            render::Bloom {
                passes: 5,
                weight: 1.2,
                threshold: 0.5
            },
            "{game} {map}"
        );
    }
    let l = render_lighting_from("bf2", &ranges(None, None), None);
    assert_eq!(l.glow_gain, 1.0);
}

/// The sheet's range, on the numbers the discs author
/// (`analysis/re-lowr.md` §4's table). The rule is pure — the env gate is
/// the only thing between it and `upload_scene`.
#[test]
fn the_sheet_runs_from_the_heightfields_end_to_maxdistance() {
    let with_far =
        |near: Option<[f32; 3]>, far_scene: Option<f32>, tex: Option<&str>, max: Option<f32>| {
            lowres_range(
                near.map(|n| FarCull {
                    terrain: n[2],
                    world: n[2],
                }),
                &manifest::ManifestLighting {
                    near_scene_range: near,
                    far_scene_range: far_scene,
                    ..manifest::ManifestLighting::default()
                },
                Some(&manifest::ManifestSky {
                    glow_masks_preserved: false,
                    texture: None,
                    models: Vec::new(),
                    ambient: [0.0; 3],
                    low_res_terrain: tex.map(str::to_owned),
                    low_res_max_distance: max,
                    objects: Vec::new(),
                    terrain_enable: None,
                    reflection_color: None,
                    stars: None,
                }),
            )
        };
    let with = |near: Option<[f32; 3]>, tex: Option<&str>, max: Option<f32>| {
        with_far(near, Some(2000.0), tex, max)
    };

    // BF1 TAT1: NearSceneRange(1, 100, 150), MaxDistance 3000 — the sheet
    // starts where the heightfield stops, not at the authored 100.
    assert_eq!(
        with(Some([1.0, 100.0, 150.0]), Some("tat1"), Some(3000.0)),
        Some((150.0, 3000.0))
    );
    // BF1 BES2's authored 400, and BF1 HOT1, which authors the scope but
    // no MaxDistance and so takes the ELF's 2000.
    assert_eq!(
        with(
            Some([0.8, 120.0, 180.0]),
            Some("bespin2_lowresterrain"),
            Some(400.0)
        ),
        Some((180.0, 400.0))
    );
    assert_eq!(
        with(Some([1.0, 200.0, 270.0]), Some("hoth"), None),
        Some((270.0, DEFAULT_LOW_RES_MAX_DISTANCE))
    );

    // BF2 DAG1 and NAB2: a sheet in the `tern`, no `LowResTerrain` scope
    // in the sky, so the disc never draws it and neither do we.
    assert_eq!(with(Some([0.2, 20.0, 60.0]), None, None), None);
    // BF2 END1: it ships a sheet AND names it (`end1_lowresterrain`, no
    // MaxDistance), but authors `FarSceneRange(0)` — no far layer, so the
    // disc composites nothing from the far scene and draws no sheet.
    // Its NearSceneRange (0.7, 90, 150) from the baked manifest.
    assert_eq!(
        with_far(
            Some([0.7, 90.0, 150.0]),
            Some(0.0),
            Some("end1_lowresterrain"),
            None
        ),
        None
    );
    // The same map with any real far layer would draw it.
    assert_eq!(
        with_far(
            Some([0.7, 90.0, 150.0]),
            Some(150.0),
            Some("end1_lowresterrain"),
            None
        ),
        Some((150.0, DEFAULT_LOW_RES_MAX_DISTANCE))
    );
    // No cull — `OPENBF_FAR_CULL` off, or a pre-item-5 manifest with no
    // `NearSceneRange`: no sheet, because there is no far scene to fill
    // and the coarse LOD would poke through the ground instead.
    assert_eq!(with(None, Some("tat1"), Some(3000.0)), None);
    // A MaxDistance inside the near scene draws nothing — no shipped map
    // authors one, and the ELF's own gate is `0 < MaxDistance`.
    assert_eq!(
        with(Some([1.0, 100.0, 150.0]), Some("tat1"), Some(0.0)),
        None
    );
    assert_eq!(
        with(Some([1.0, 100.0, 150.0]), Some("tat1"), Some(120.0)),
        None
    );
}

/// The cull's two distances, on the numbers the discs actually author
/// (`analysis/re-fog.md` §6's table). The rule is pure — the env gate is
/// the only thing between it and `upload_scene`.
#[test]
fn the_cull_takes_the_terrain_to_the_near_scene_and_the_world_to_the_far() {
    let with = |near: Option<[f32; 3]>, far: Option<f32>| {
        far_cull_from(&manifest::ManifestLighting {
            near_scene_range: near,
            far_scene_range: far,
            ..manifest::ManifestLighting::default()
        })
    };

    // BF1 TAT1: NearSceneRange(1, 100, 150), FarSceneRange 2000 — the
    // values `mungefs/tests/suite/tat1_lvl.rs` pins against the disc bytes.
    assert_eq!(
        with(Some([1.0, 100.0, 150.0]), Some(2000.0)),
        Some(FarCull {
            terrain: 150.0,
            world: 2000.0
        })
    );
    // BF1 KAS2 (1500) and BF2 KAM1 (800), the two maps the gate scans.
    assert_eq!(
        with(Some([1.0, 80.0, 100.0]), Some(1500.0)).unwrap().world,
        1500.0
    );
    assert_eq!(
        with(Some([1.0, 60.0, 90.0]), Some(800.0)).unwrap().terrain,
        90.0
    );

    // `FarSceneRange(0)` is "no far layer", NOT "cull everything at 0":
    // the near scene still draws models and water to NearSceneRange[2].
    assert_eq!(
        with(Some([1.0, 100.0, 150.0]), Some(0.0)),
        Some(FarCull {
            terrain: 150.0,
            world: 150.0
        })
    );
    // An unauthored FarSceneRange takes the ELF's 2000 m default.
    assert_eq!(with(Some([1.0, 100.0, 150.0]), None).unwrap().world, 2000.0);

    // No NearSceneRange (a manifest baked before the field existed) =
    // no cull at all, so a stale bake renders unchanged with the flag on.
    assert_eq!(with(None, Some(1500.0)), None);
    // Degenerate values are refused rather than deleting the world.
    assert_eq!(with(Some([1.0, 100.0, 0.0]), Some(1500.0)), None);
    assert_eq!(with(Some([1.0, 100.0, -5.0]), Some(1500.0)), None);
    assert_eq!(with(Some([1.0, 100.0, f32::NAN]), Some(1500.0)), None);
}

/// Which maps fade into the sky instead of into a fog colour, on the
/// numbers the discs author (`analysis/re-fog.md` §6's table, which is
/// what the baked manifests carry).
#[test]
fn only_a_bf2_map_with_no_far_layer_fades_into_the_sky() {
    let with = |game: &str, near: Option<[f32; 3]>, far: Option<f32>| {
        far_fade_from(
            game,
            &manifest::ManifestLighting {
                near_scene_range: near,
                far_scene_range: far,
                ..manifest::ManifestLighting::default()
            },
        )
    };
    let fade = |begin: f32, end: f32| Some(render::FarFade { begin, end });

    // The six baked BF2 maps that author FarSceneRange(0). The first
    // three also author fog, which is the deviation this replaces
    // (Felucia's green-grey wall, Endor's grey one); the last three
    // author none and fade into the sky just the same.
    for (map, near, b, c) in [
        ("END1", [0.7, 90.0, 150.0], 90.0, 150.0),
        ("FEL1", [0.1, 28.0, 52.0], 28.0, 52.0),
        ("YAV1", [0.7, 50.0, 100.0], 50.0, 100.0),
        ("DAG1", [0.2, 20.0, 60.0], 20.0, 60.0),
        ("DEA1", [0.1, 120.0, 180.0], 120.0, 180.0),
        ("NAB2", [1.75, 100.0, 150.0], 100.0, 150.0),
    ] {
        assert_eq!(with("bf2", Some(near), Some(0.0)), fade(b, c), "BF2 {map}");
    }

    // BF1 END1 (100..150), YAV1 (50..100) and RHN2 (90..110) author
    // FarSceneRange(0) too and do NOT fade: BF1's fog gate has no layer
    // test, so BF1 fogs its near scene (re-fog.md §2.2 [V]).
    assert_eq!(with("bf1", Some([0.7, 100.0, 150.0]), Some(0.0)), None);
    assert_eq!(with("bf1", Some([0.7, 50.0, 100.0]), Some(0.0)), None);
    assert_eq!(with("bf1", Some([0.7, 90.0, 110.0]), Some(0.0)), None);

    // A BF2 map with a real far layer never fades — TAT2's 3000 m and
    // TAN1's 150 m are both far layers, however short.
    assert_eq!(with("bf2", Some([0.1, 70.0, 100.0]), Some(3000.0)), None);
    assert_eq!(with("bf2", Some([0.5, 60.0, 110.0]), Some(150.0)), None);
    // An unauthored FarSceneRange is not a zero one.
    assert_eq!(with("bf2", Some([0.1, 100.0, 150.0]), None), None);
    // Missing or degenerate NearSceneRange: no fade, rather than a band
    // that swallows the map.
    assert_eq!(with("bf2", None, Some(0.0)), None);
    assert_eq!(with("bf2", Some([0.1, 100.0, 100.0]), Some(0.0)), None);
    assert_eq!(with("bf2", Some([0.1, 150.0, 100.0]), Some(0.0)), None);
    assert_eq!(with("bf2", Some([0.1, 100.0, f32::NAN]), Some(0.0)), None);
}

/// The fade and the fog are mutually exclusive, and only the fade maps
/// lose their fog: every other map keeps the exact ramp it had.
#[test]
fn the_fade_replaces_the_fog_and_only_on_the_maps_that_take_it() {
    let fogged = manifest::ManifestFog {
        color: [0.2, 0.3, 0.4],
        near: 90.0,
        far: 350.0,
    };

    // BF2 END1: fog authored, no far layer -> the fade, and NO fog.
    let l = manifest::ManifestLighting {
        near_scene_range: Some([0.7, 90.0, 150.0]),
        far_scene_range: Some(0.0),
        fog: Some(fogged),
        ..manifest::ManifestLighting::default()
    };
    let look = render_lighting_from("bf2", &l, None);
    assert_eq!(
        look.far_fade,
        Some(render::FarFade {
            begin: 90.0,
            end: 150.0
        })
    );
    assert_eq!(look.fog, None, "the disc draws no fog colour on END1");

    // BF1 END1: the same numbers on the other disc keep the fog ramp,
    // stretched by FOG_DENSITY_SCALE exactly as before.
    let look = render_lighting_from("bf1", &l, None);
    assert_eq!(look.far_fade, None);
    let fog = look.fog.expect("BF1 keeps its fog");
    assert_eq!(fog.near, 90.0);
    assert!((fog.far - (90.0 + (350.0 - 90.0) / FOG_DENSITY_SCALE)).abs() < 1e-3);

    // BF2 HOT1: fog and a real far layer -> unchanged, fade off.
    let look = render_lighting_from(
        "bf2",
        &manifest::ManifestLighting {
            near_scene_range: Some([0.5, 200.0, 270.0]),
            far_scene_range: Some(800.0),
            fog: Some(fogged),
            ..manifest::ManifestLighting::default()
        },
        None,
    );
    assert_eq!(look.far_fade, None);
    assert_eq!(look.fog.map(|f| f.near), Some(90.0));
}

/// BF1 lights its soldiers with `CharacterAmbientColor`
/// (`analysis/re-model-lighting.md` §12); BF2 has one ambient for every
/// draw, so the same manifest field is dropped there.
#[test]
fn only_bf1_takes_the_character_ambient() {
    // BF1 GEO1's baked pair.
    let l = manifest::ManifestLighting {
        ambient_sky: [0.187_820_76, 0.130_136_49, 0.072_271_86],
        ambient_ground: [0.187_820_76, 0.130_136_49, 0.072_271_86],
        character_ambient: Some([0.637_596_96, 0.332_451_58, 0.066_625_95]),
        ..manifest::ManifestLighting::default()
    };
    let bf1 = render_lighting_from("bf1", &l, None);
    assert_eq!(bf1.character_ambient, l.character_ambient);
    assert_eq!(
        bf1.ambient_sky, l.ambient_sky,
        "the world keeps AmbientColor"
    );
    assert_eq!(
        render_lighting_from("bf2", &l, None).character_ambient,
        None
    );
    let none = manifest::ManifestLighting {
        character_ambient: None,
        ..l
    };
    assert_eq!(
        render_lighting_from("bf1", &none, None).character_ambient,
        None
    );
}

/// One water config helper for the cases below: the LUT ends and the two
/// switches, exactly as the twelve baked manifests carry them.
fn water_cfg(
    min: [f32; 4],
    max: [f32; 4],
    ocean: bool,
    disable_low_res: bool,
) -> manifest::ManifestWater {
    manifest::ManifestWater {
        far_fog_color: None,
        glb: "water.glb".into(),
        texture: Some("kas2_water".into()),
        height: -5.0,
        uv_velocity: [0.0125, 0.0125],
        layer_color: [0; 4],
        wet_cells: 1,
        patches: 0,
        triangles: 2,
        vertex_diffuse: false,
        config: Some(manifest::ManifestWaterConfig {
            main_texture: Some("kas2_water".into()),
            foam_texture: ocean.then(|| "kam1_water".to_string()),
            min_diffuse: Some(min),
            max_diffuse: Some(max),
            ocean,
            disable_low_res,
            // The PS2's reset value, which every baked manifest carries.
            specular_enable: true,
            oscillation_enable: true,
            ..manifest::ManifestWaterConfig::default()
        }),
        wet: None,
        gameplay_wet: None,
    }
}

#[test]
fn kashyyyk_reference_water_haze_does_not_enable_world_fog() {
    let mut water = water_cfg([30.0; 4], [70.0; 4], false, false);
    water.far_fog_color = Some([104.0, 106.0, 108.0]);
    let fog = reference_water_fog("bf2", "kas2", Some(&water)).unwrap();
    assert_eq!([fog.near, fog.far], [90.0, 350.0]);
    for (actual, expected) in fog.color.iter().zip([104.0, 106.0, 108.0]) {
        assert!((render::srgb_encode(*actual) * 255.0 - expected).abs() < 0.001);
    }
    assert!(reference_water_fog("bf1", "kas2", Some(&water)).is_none());
    assert!(reference_water_fog("bf2", "yav1", Some(&water)).is_none());
    assert!(
        render_lighting_from("bf2", &manifest::ManifestLighting::default(), Some(&water))
            .fog
            .is_none()
    );
    water.config.as_mut().unwrap().disable_low_res = true;
    assert!(reference_water_fog("bf2", "kas2", Some(&water)).is_none());
}

/// The speckle layer on BF1 KAS2's authored numbers (the manifest's
/// `fx__` block): 25 frames at 4 fps, `SpeckleTile(5, 5)` over 128 m cells
/// whose mesh UV repeats every 16 m.
#[test]
fn the_speckle_takes_the_maps_tile_and_rate() {
    let mut w = water_cfg([0.0; 4], [0.0; 4], false, false);
    let c = w.config.as_mut().unwrap();
    c.metres_per_repeat = Some([16.0, 16.0]);
    c.speckle_textures = (0..25).map(|i| format!("water_specularmask_{i}")).collect();
    c.speckle_fps = Some(4.0);
    c.speckle_tile = Some([5.0, 5.0]);
    c.speckle_scroll = Some([0.11, 0.11]);
    c.speckle_specular_color = Some([200.0, 200.0, 200.0, 150.0]);
    let s = speckle_look("bf1", &w, 128.0).expect("BF1 flat water speckles");
    assert_eq!(s.frames.len(), 25);
    assert_eq!(s.fps, 4.0);
    assert_eq!(s.uv_scale, [0.625, 0.625], "5 repeats per 128 m cell");
    assert_eq!(s.scroll, [0.11, 0.11]);

    // BF2 on the flag's reset value (every shipped BF2 map) draws the
    // same layer; an authored `SpecularEnable(0)`, an ocean and no frames
    // draw none.
    assert_eq!(speckle_look("bf2", &w, 128.0), Some(s));
    w.config.as_mut().unwrap().specular_enable = false;
    assert_eq!(speckle_look("bf2", &w, 128.0), None);
    assert!(speckle_look("bf1", &w, 128.0).is_some(), "BF1 has no gate");
    w.config.as_mut().unwrap().specular_enable = true;
    w.config.as_mut().unwrap().ocean = true;
    assert_eq!(speckle_look("bf1", &w, 128.0), None);
    w.config.as_mut().unwrap().ocean = false;
    w.config.as_mut().unwrap().speckle_textures.clear();
    assert_eq!(speckle_look("bf1", &w, 128.0), None);
}

/// The flipbook: ⌊t·fps⌋ mod n and the next, cross-faded by the fraction.
#[test]
fn the_speckle_frames_wrap_and_cross_fade() {
    assert_eq!(speckle_frames(0.0, 4.0, 25), (0, 1, 0.0));
    assert_eq!(speckle_frames(0.375, 4.0, 25), (1, 2, 0.5));
    assert_eq!(speckle_frames(6.0, 4.0, 25), (24, 0, 0.0));
    assert_eq!(speckle_frames(6.25, 4.0, 25), (0, 1, 0.0));
    assert_eq!(speckle_frames(1.0, 4.0, 0), (0, 0, 0.0));
}

/// The water's look, on the four shapes the discs ship
/// (`analysis/re-water.md` §1/§3, the §5 census).
#[test]
fn the_water_takes_the_luts_mean_and_the_discs_own_fog_rule() {
    // The LUT every map but BF2 KAS2 and BF2 DEA1 authors.
    let (min, max) = ([32.0, 40.0, 32.0, 188.0], [128.0, 130.0, 128.0, 188.0]);

    // BF1 KAS2: far water on, so the fog starts where the near scene
    // ends (100 m) and the surface reaches 0.9 * 1500.
    let l = manifest::ManifestLighting {
        near_scene_range: Some([2.0, 80.0, 100.0]),
        far_scene_range: Some(1500.0),
        ..manifest::ManifestLighting::default()
    };
    let w = water_look("bf1", &l, &water_cfg(min, max, false, false));
    // The LUT's mean is MaxDiffuse, in GS terms (byte / 255, gamma —
    // `fs_water` linearises it after the view-angle term).
    assert_eq!(w.tint[..3], [128.0 / 255.0, 130.0 / 255.0, 128.0 / 255.0]);
    assert!(
        (w.tint[3] - 188.0 / 255.0).abs() < 1e-6,
        "alpha is a weight"
    );
    assert_eq!(w.fog_begin, Some(100.0));
    assert_eq!(w.far, Some(1350.0));
    assert_eq!(w.foam_tint, None, "flat water has no foam layer");

    // BF1 KAS1: `DisableLowRes` -> no far pass at all, so nothing to fog
    // and the surface stops with the near scene.
    let l = manifest::ManifestLighting {
        near_scene_range: Some([0.7, 150.0, 180.0]),
        far_scene_range: Some(1000.0),
        ..manifest::ManifestLighting::default()
    };
    let w = water_look("bf1", &l, &water_cfg(min, max, false, true));
    assert_eq!(w.fog_begin, None);
    assert_eq!(w.far, Some(180.0));

    // BF2 KAS2's twin numbers: `SpecularEnable` is on (the reset's 1),
    // so the far water takes the fog past the near scene as BF1's does;
    // an authored 0 or `DisableLowRes` fogs none.
    let l = manifest::ManifestLighting {
        near_scene_range: Some([0.5, 85.0, 100.0]),
        far_scene_range: Some(5000.0),
        ..manifest::ManifestLighting::default()
    };
    let w = water_look("bf2", &l, &water_cfg(min, max, false, false));
    assert_eq!(w.fog_begin, Some(100.0));
    assert_eq!(w.far, Some(4500.0));
    let mut off = water_cfg(min, max, false, false);
    off.config.as_mut().unwrap().specular_enable = false;
    assert_eq!(water_look("bf2", &l, &off).fog_begin, None);
    assert_eq!(
        water_look("bf1", &l, &off).fog_begin,
        Some(100.0),
        "BF1 has no gate"
    );
    let w = water_look("bf2", &l, &water_cfg(min, max, false, true));
    assert_eq!(w.fog_begin, None, "DisableLowRes: no far water to fog");
    let w = water_look("bf2", &l, &water_cfg(min, max, true, false));
    assert_eq!(
        w.fog_begin,
        Some(100.0),
        "the gate is the flat renderer's, not the ocean's"
    );
    let mut ocean_off = water_cfg(min, max, true, false);
    ocean_off.config.as_mut().unwrap().specular_enable = false;
    assert_eq!(water_look("bf2", &l, &ocean_off).fog_begin, Some(100.0));

    // BF2 NAB2: `FarSceneRange(0)` floors the water at the near scene's
    // own end rather than at zero.
    let l = manifest::ManifestLighting {
        near_scene_range: Some([1.75, 100.0, 150.0]),
        far_scene_range: Some(0.0),
        ..manifest::ManifestLighting::default()
    };
    assert_eq!(
        water_look("bf2", &l, &water_cfg(min, max, false, false)).far,
        Some(150.0)
    );

    // BF1 KAM1, the ocean: base OPAQUE and at UNITY whatever the LUT
    // says (the engine's white halved), the foam at unity too, and 0.8
    // of the far range.
    let l = manifest::ManifestLighting {
        near_scene_range: Some([0.8, 60.0, 90.0]),
        far_scene_range: Some(800.0),
        ..manifest::ManifestLighting::default()
    };
    let w = water_look(
        "bf1",
        &l,
        &water_cfg(min, [128.0, 130.0, 128.0, 60.0], true, false),
    );
    assert_eq!(w.tint, [1.0; 4], "unity, and the base record is 0x4100");
    assert_eq!(w.foam_tint, Some([1.0; 4]));
    assert_eq!(w.far, Some(640.0));
    assert_eq!((w.sun, w.dim), (0.0, None), "BF1's ocean is unlit");

    // BF2 KAM1 (card KSEAL): the same ocean is lit by the sun, its foam
    // blends at half weight, and from FogRange's 400 m it loses colour
    // down to half at the far pass's 640 m. Without a fog there is no
    // distance to start from, and no loss.
    let ocean = water_cfg(min, [128.0, 130.0, 128.0, 60.0], true, false);
    let fogged = manifest::ManifestLighting {
        fog: Some(manifest::ManifestFog {
            color: [0.0; 3],
            near: 400.0,
            far: 1356.0,
        }),
        ..l.clone()
    };
    let w = water_look("bf2", &fogged, &ocean);
    assert_eq!(w.tint, [1.0; 4]);
    assert_eq!(w.foam_tint, Some([1.0, 1.0, 1.0, OCEAN_LIT_FOAM_ALPHA]));
    assert_eq!(w.sun, OCEAN_SUN_WEIGHT);
    let [begin, rate] = w.dim.expect("a lit ocean under a fog dims");
    assert_eq!(begin, 400.0);
    assert!((1.0 - rate * (640.0 - begin) - OCEAN_FAR_KEEP).abs() < 1e-6);
    assert_eq!(water_look("bf2", &l, &ocean).dim, None);
    assert_eq!(water_look("bf1", &fogged, &ocean).dim, None);

    // A manifest baked before the `fx__` reader landed: no tint, no foam,
    // and the two distances still come from `sky_`.
    let mut old = water_cfg(min, max, false, false);
    old.config = None;
    let w = water_look("bf1", &l, &old);
    assert_eq!(w.tint, [1.0; 4], "no config invents no colour");
    assert_eq!(w.foam_tint, None);
}

/// The overlay's constant block on BF1 YAV1's numbers: the baked sun IS
/// the PS2's L (the RAM vector of the YAV1 state, `re-water-colour.md`
/// §1.4), ĥ its horizontal, and the authored colours / 255. An ocean and
/// a dry map get no term at all.
#[test]
fn the_water_shading_is_the_overlays_constant_block() {
    let l = manifest::ManifestLighting {
        sun_dir: [-0.556_670_4, -0.766_044_4, 0.321_393_8],
        ..manifest::ManifestLighting::default()
    };
    let ram = Vec3::new(-0.5567, -0.766, 0.3214);

    let mut w = water_cfg(
        [32.0, 40.0, 32.0, 255.0],
        [128.0, 130.0, 128.0, 255.0],
        false,
        false,
    );
    let c = w.config.as_mut().unwrap();
    c.specular_color = Some([192.0; 4]);
    c.speckle_ambient_color = Some([0.0, 0.0, 0.0, 255.0]);
    c.speckle_specular_color = Some([160.0, 160.0, 160.0, 255.0]);
    c.speckle_coord_shift = Some([10.0, 10.0]);
    c.light_azim_and_elev = Some([1.0, 0.0]);
    let s = water_shading(&l, Some(&w));
    let h = Vec3::new(ram.x, 0.0, ram.z).normalize();
    assert!(
        (s.eye_shift - 10.0 * h).abs().max_element() < 1e-3,
        "{:?}",
        s.eye_shift
    );
    assert_eq!(s.specular, [192.0 / 255.0; 4]);
    assert_eq!(s.speckle_ambient, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        s.speckle_specular,
        [160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 1.0]
    );
    assert_eq!(s.speckle_axis, Vec3::new(1.0, -0.0, 1.0));
    // LightAzimAndElev (1, 0): l̂ = −ĥ, the lobe points at the sun's azimuth.
    assert!((s.speckle_light + h).abs().max_element() < 1e-3);
    assert!((s.speckle_shift - 10.0 * h).abs().max_element() < 1e-3);

    w.config.as_mut().unwrap().ocean = true;
    assert_eq!(water_shading(&l, Some(&w)), render::WaterShading::default());
    assert_eq!(water_shading(&l, None), render::WaterShading::default());
}

fn ranges(near: Option<[f32; 3]>, far: Option<f32>) -> manifest::ManifestLighting {
    manifest::ManifestLighting {
        near_scene_range: near,
        far_scene_range: far,
        ..manifest::ManifestLighting::default()
    }
}

/// Card LOD: the switch's band is `NearSceneRange[1]..[2]` (BF1 NAB1
/// authors 100 / 130 / 1000); a map with no far scene draws no LOWD.
#[test]
fn lod_bands_come_from_the_scene_ranges() {
    let nab1 = lod_bands(&ranges(Some([1.0, 100.0, 130.0]), Some(1000.0)));
    assert_eq!(
        nab1,
        Some(LodBands {
            begin: 100.0,
            end: 130.0
        })
    );
    // FarSceneRange(0): no far layer, no LOWD (BF1 YAV1 50 / 100 / 0).
    assert_eq!(
        lod_bands(&ranges(Some([1.0, 50.0, 100.0]), Some(0.0))),
        None
    );
    // A far scene that never reaches past NearSceneRange[1].
    assert_eq!(
        lod_bands(&ranges(Some([1.0, 100.0, 150.0]), Some(100.0))),
        None
    );
    // No NearSceneRange (a stale bake): nothing to switch on.
    assert_eq!(lod_bands(&ranges(None, Some(1000.0))), None);
    // No FarSceneRange authored: the ELF's default, which reaches past.
    assert!(lod_bands(&ranges(Some([1.0, 100.0, 130.0]), None)).is_some());
    let anchor = glam::Vec3::new(1.0, 2.0, 3.0);
    let (m, l) = nab1.unwrap().sides(LodMode::ClassicPlus, None, anchor, 5.0);
    assert_eq!(
        (m.side, l.side),
        (render::LodSide::Model, render::LodSide::Lowd)
    );
    assert_eq!((m.at, m.anchor, m.radius), (l.at, l.anchor, l.radius));
    // Classic-plus switches at twice the near scene's end; the PS2 mode
    // between the far scene's reach and the near scene's end.
    assert_eq!(m.at, 260.0);
    assert_eq!(nab1.unwrap().at(LodMode::Ps2, None), 115.0);
    // A leaf-patch canopy caps classic-plus at its reach, never nearer
    // than the PS2's switch.
    let b = nab1.unwrap();
    assert_eq!(b.at(LodMode::ClassicPlus, Some(180.0)), 180.0);
    assert_eq!(b.at(LodMode::ClassicPlus, Some(80.0)), 115.0);
    assert_eq!(b.at(LodMode::ClassicPlus, Some(400.0)), 260.0);
    assert_eq!(b.at(LodMode::Ps2, Some(80.0)), 115.0);
}

#[test]
fn terrain_lod_requires_a_drawable_sheet_and_preserves_world_ranges() {
    let lighting = ranges(Some([0.5, 85.0, 100.0]), Some(5000.0));
    let sky = manifest::ManifestSky {
        glow_masks_preserved: false,
        texture: None,
        models: Vec::new(),
        ambient: [0.0; 3],
        low_res_terrain: Some("kas2".into()),
        low_res_max_distance: None,
        objects: Vec::new(),
        terrain_enable: None,
        reflection_color: None,
        stars: None,
    };
    assert_eq!(
        terrain_lod_range(true, true, &lighting, Some(&sky)),
        Some((100.0, 2000.0))
    );
    assert_eq!(terrain_lod_range(false, true, &lighting, Some(&sky)), None);
    assert_eq!(terrain_lod_range(true, false, &lighting, Some(&sky)), None);
    assert_eq!(terrain_lod_range(true, true, &lighting, None), None);
    assert_eq!(far_cull_from(&lighting).unwrap().world, 5000.0);
    let no_far = ranges(lighting.near_scene_range, Some(0.0));
    assert_eq!(terrain_lod_range(true, true, &no_far, Some(&sky)), None);
}
