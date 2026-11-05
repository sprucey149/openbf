use super::*;

/// The runtime default is the PC tier — PLAN §LOCKED FEATURE SET puts the
/// modern look on by default. A golden that renders it is a bug, which is
/// why the golden tests pin `Ps2` (game/tests/common/mod.rs).
#[test]
fn the_default_texture_tier_is_pc() {
    assert_eq!(TextureTier::default(), TextureTier::Pc);
}

/// `OPENBF_TEXTURE_TIER` spells the tiers 1 and 2, as the queue item and
/// `scripts/goldens.sh` do. Anything else falls back to the default
/// rather than failing a launch, and says so on stderr.
#[test]
fn the_tier_env_var_reads_1_and_2() {
    assert_eq!(tier_from_env_value(Some("1")), TextureTier::Ps2);
    assert_eq!(tier_from_env_value(Some("2")), TextureTier::Pc);
    for v in [
        None,
        Some(""),
        Some("0"),
        Some("3"),
        Some("ps2"),
        Some(" 1"),
    ] {
        assert_eq!(
            tier_from_env_value(v),
            TextureTier::default(),
            "{v:?} should have fallen back"
        );
    }
}

/// The grid's width is read off the row wrap, so appending the shipped
/// PTCH replacement vertices after it (`bake::terrain_glb`) leaves the
/// sim's heightfield exactly the grid it always was.
#[test]
fn the_terrain_grid_width_survives_appended_vertices() {
    let n = 8usize;
    let unit = 8.0f32;
    let half = n as f32 / 2.0 * unit;
    let mut pos: Vec<[f32; 3]> = (0..n)
        .flat_map(|z| {
            (0..n).map(move |x| {
                [
                    x as f32 * unit - half,
                    (x + z) as f32,
                    z as f32 * unit - (half - unit),
                ]
            })
        })
        .collect();
    assert_eq!(terrain_grid_size(&pos), Some(n));
    // A replacement mesh's vertices land anywhere inside the map, INCLUDING
    // on a row start — after the grid, so they cannot move the answer.
    pos.push([-half, 1.5, 0.0]);
    pos.push([1.0, 1.5, 2.0]);
    assert_eq!(terrain_grid_size(&pos), Some(n));
    // Not a grid: one row, or a mesh whose first row never wraps.
    assert_eq!(terrain_grid_size(&pos[..n]), None);
    assert_eq!(terrain_grid_size(&[]), None);
    // A wrap that cannot be a square grid is rejected rather than
    // truncating the heightfield to nonsense.
    assert_eq!(terrain_grid_size(&pos[..n + 2]), None);
}

#[test]
fn canonical_terrain_query_frame_preserves_float32_interpolation() {
    let terrain = |n: usize| {
        let unit = 4.0;
        let half = n as f32 * unit / 2.0;
        CpuMesh {
            positions: (0..n)
                .flat_map(|z| {
                    (0..n).map(move |x| {
                        let wx = x as f32 * unit - half;
                        [wx, wx, z as f32 * unit - (half - unit)]
                    })
                })
                .collect(),
            normals: vec![],
            uvs: vec![],
            colors: vec![],
            indices: vec![],
            submeshes: vec![],
            spec: vec![],
            light: vec![],
            scroll: vec![],
            sphere: None,
            vis_top: None,
            render_grid: None,
            contact_surface: false,
        }
    };
    let mut reference = scene("None");
    reference.terrain = Some(terrain(128));
    let canonical = reference.heightfield().unwrap();
    let mut donor = scene("None");
    donor.terrain = Some(terrain(256));
    let uncorrected = donor.heightfield().unwrap();
    let point = (106.14756, 104.78419);
    assert_ne!(
        canonical.height_at(point.0, point.1).to_bits(),
        uncorrected.height_at(point.0, point.1).to_bits()
    );
    donor.manifest.terrain_query_size = Some(128);
    let corrected = donor.heightfield().unwrap();
    assert_eq!(corrected.size, canonical.size);
    assert_eq!(
        (corrected.offset_x, corrected.offset_z),
        (canonical.offset_x, canonical.offset_z)
    );
    assert_eq!(corrected.heights, canonical.heights);
    for (x, z) in [point, (-14.554941, -124.45938), (-300.0, 300.0)] {
        assert_eq!(
            corrected.height_at(x, z).to_bits(),
            canonical.height_at(x, z).to_bits()
        );
    }
    assert_eq!(donor.terrain.as_ref().unwrap().positions.len(), 256 * 256);
    for invalid in [0, 1, 127, 257] {
        donor.manifest.terrain_query_size = Some(invalid);
        assert!(donor.heightfield().is_none());
    }
}

/// A manifest with a base graph and one conquest layer. `plan` names the
/// layer's own sidecar when it ships one, so the test can tell which graph
/// survived the merge.
fn scene(layer_nav: &str) -> Scene {
    let src = format!(
        r#"(
        map: "t", source_game: "bf2",
        terrain_name: None, sky_name: None, terrain_glb: None,
        models: [], textures: [], classes: [], geometries: [],
        regions: [], instances: [],
        layers: [(
            name: "t_conquest", worlds: ["t_conquest"],
            regions: [], instances: [],
            nav: {layer_nav},
        )],
        nav: Some((
            plan: Some((
                hubs: [(name: "b0", pos: (0,0,0), radius: 1.0, arcs: [])],
                arcs: [],
                routes: "nav/routes_base.bin",
            )),
            hints: [(name: "baseh", kind: 1,
                     xform: ((1,0,0),(0,1,0),(0,0,1),(0,0,0)))],
            barriers: [(name: "baseb", pos: (0,0,0),
                        rows: ((1,0,0),(0,1,0),(0,0,1)), size: (1,1,1), flags: 1)],
        )),
    )"#
    );
    Scene {
        dir: std::path::PathBuf::new(),
        manifest: ron::from_str(&src).expect("manifest parses"),
        meshes: HashMap::new(),
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures: HashMap::new(),
        collision: HashMap::new(),
        cutters: Vec::new(),
    }
}

#[test]
fn installed_water_keeps_dry_cells_and_clears_previous_scene_inputs() {
    let mut scene = scene("None");
    scene.manifest.water = Some(
        ron::from_str(
            r#"(glb: "water.glb", height: -2.0,
            wet: Some((grid: 2, cell_size: 10.0, half_extent: (10.0, 10.0),
                       nibbles: "1000")))"#,
        )
        .unwrap(),
    );
    let mut sim = core::sim::Sim::empty(1);
    scene.install_water(&mut sim);
    let wet = glam::Vec3::new(-5.0, 2.0, -5.0);
    let dry = glam::Vec3::new(5.0, 2.0, -5.0);
    let water = sim.water_plane().expect("installed water");
    assert_eq!(water.crossing(wet, glam::Vec3::NEG_Y), Some(4.0));
    assert_eq!(water.crossing(dry, glam::Vec3::NEG_Y), None);
    assert_eq!(sim.water_under(dry), None);

    // A legacy manifest must clear the preceding map's mask, not inherit it.
    scene.manifest.water.as_mut().unwrap().wet = None;
    scene.install_water(&mut sim);
    assert!(sim.water_mask.is_none());
    assert_eq!(sim.water_under(dry), Some(-2.0));
    scene.manifest.water = None;
    scene.install_water(&mut sim);
    assert!(sim.water_level.is_none() && sim.water_mask.is_none());
    assert!(sim.water_plane().is_none());
}

#[test]
fn installed_water_bob_obeys_both_ps2_ocean_and_oscillation_flags() {
    let mut scene = scene("None");
    for (game, ocean, oscillates, expected) in [
        ("bf1", false, false, true),
        ("bf1", true, true, false),
        ("bf2", false, true, true),
        ("bf2", false, false, false),
        ("bf2", true, true, false),
    ] {
        scene.manifest.source_game = game.into();
        scene.manifest.water = Some(ron::from_str(&format!(
            "(glb: \"water.glb\", height: 2.0, config: Some((ocean: {ocean}, oscillation_enable: {oscillates})))"
        )).unwrap());
        let mut sim = core::sim::Sim::empty(1);
        scene.install_water(&mut sim);
        assert_eq!(
            sim.water_oscillates, expected,
            "{game}, {ocean}, {oscillates}"
        );
        sim.tick = 25; // quarter period: +6 cm
        let height = sim.water_under(glam::Vec3::ZERO).unwrap();
        assert!((height - if expected { 2.06 } else { 2.0 }).abs() < 0.00001);
        scene.manifest.water = None;
        scene.install_water(&mut sim);
        assert!(!sim.water_oscillates);
    }
}

#[test]
fn installed_kashyyyk_water_preserves_baked_wet_and_dry_cells() {
    let path = baked_root().join("bf1_kas1/manifest.ron");
    if !path.exists() {
        eprintln!("SKIP: {} is not baked", path.display());
        return;
    }
    let mut scene = scene("None");
    scene.manifest = ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut sim = core::sim::Sim::empty(1);
    scene.install_water(&mut sim);
    let authored = scene.manifest.water.as_ref().unwrap();
    let mask = authored.wet.as_ref().expect("rebaked KAS1 WMAP");
    let cells = mask.cells();
    assert_eq!(sim.water_mask.as_ref().unwrap().wet_cells(), 4014);
    let plane = sim.water_plane().unwrap();
    for wet in [false, true] {
        let i = cells.iter().position(|&c| (c != 0) == wet).unwrap();
        let origin = glam::Vec3::new(
            ((i % mask.grid as usize) as f32 + 0.5) * mask.cell_size - mask.half_extent[0],
            authored.height + 2.0,
            ((i / mask.grid as usize) as f32 + 0.5) * mask.cell_size - mask.half_extent[1],
        );
        assert_eq!(
            plane.crossing(origin, glam::Vec3::NEG_Y),
            wet.then_some(2.0)
        );
        assert_eq!(sim.water_under(origin), wet.then_some(authored.height));
    }
}

#[test]
fn authored_modes_are_distinct_from_design_fallbacks() {
    let m: Manifest = ron::from_str(
        r#"(
        map: "cor1", source_game: "bf2", terrain_name: None, sky_name: None,
        terrain_glb: None, models: [], textures: [], classes: [], geometries: [],
        regions: [], instances: [], layers: [
            (name: "cor1_design", worlds: ["cor1_design"], regions: [], instances: []),
            (name: "cor1_soundregions", worlds: ["cor1_soundregions", "cor1_hunt"],
             regions: [], instances: []),
        ])"#,
    )
    .unwrap();
    assert_eq!(authored_mode_layer_index(&m, "hunt"), Some(1));
    assert_eq!(authored_mode_layer_index(&m, "con"), None);
    assert_eq!(authored_mode_layer_index(&m, "nonexistent"), None);
    assert_eq!(mode_layer_index(&m, "con"), Some(0));
}

/// Every space map's Assault layer, as its script and its worlds spell it, and
/// never its CTF or one-flag layer.
#[test]
fn space_assault_picks_each_maps_objective_layer() {
    for (worlds, want) in [
        (
            vec![
                vec!["spa6_ctf_spawn", "spa6_ctf_flags"],
                vec!["spa6_obj_hangar", "spa6_obj_spawn"],
            ],
            1,
        ),
        (
            vec![
                vec!["spa3_2_flag_ctf"],
                vec!["spa3_swing_obj", "spa3_cisfrigate"],
            ],
            1,
        ),
        (
            vec![
                vec!["spa8_ctf_spawn"],
                vec!["spa8_ass_paths", "spa8_ass_spawn"],
            ],
            1,
        ),
        (
            vec![
                vec!["spa1_transports"],
                vec!["spa1_swingassault", "spa1_mp_flyers"],
                vec!["spa1_ctf"],
            ],
            1,
        ),
    ] {
        let layers: Vec<String> = worlds
            .iter()
            .map(|w| {
                let names: Vec<String> = w.iter().map(|n| format!("{n:?}")).collect();
                format!(
                    r#"(name: {:?}, worlds: [{}], regions: [], instances: [])"#,
                    w[0],
                    names.join(",")
                )
            })
            .collect();
        let m: Manifest = ron::from_str(&format!(
            r#"(map: "s", source_game: "bf2", terrain_name: None, sky_name: None,
                terrain_glb: None, models: [], textures: [], classes: [], geometries: [],
                regions: [], instances: [], layers: [{}])"#,
            layers.join(",")
        ))
        .unwrap();
        assert_eq!(mode_layer_index(&m, "ass"), Some(want), "{worlds:?}");
    }
}

/// PLAN Phase A decision 1: the mode layer's PLAN replaces the base graph
/// when it has one, and its hints and barriers are added to the base
/// world's.
#[test]
fn a_mode_layer_plan_replaces_the_base_graph() {
    let mut s = scene(
        r#"Some((
            plan: Some((
                hubs: [(name: "l0", pos: (9,0,9), radius: 1.0, arcs: []),
                       (name: "l1", pos: (9,0,19), radius: 1.0, arcs: [])],
                arcs: [],
                routes: "nav/routes_t_conquest.bin",
            )),
            hints: [(name: "layerh", kind: 2,
                     xform: ((1,0,0),(0,1,0),(0,0,1),(1,1,1)))],
            barriers: [(name: "layerb", pos: (5,0,5),
                        rows: ((1,0,0),(0,1,0),(0,0,1)), size: (1,1,1), flags: 1)],
        ))"#,
    );
    assert_eq!(s.apply_mode_layer("con").as_deref(), Some("t_conquest"));
    let nav = s.manifest.nav.expect("nav survived the merge");
    let plan = nav.plan.expect("plan");
    assert_eq!(plan.routes, "nav/routes_t_conquest.bin");
    assert_eq!(plan.hubs.len(), 2, "the layer's graph, not the base's");
    assert_eq!(nav.hints.len(), 2, "base + layer");
    assert_eq!(nav.barriers.len(), 2, "base + layer");
}

/// Seven BF2 maps keep hints in a layer that ships no `plan`: the base
/// graph stays, the hints and barriers still merge.
#[test]
fn a_layer_without_a_plan_keeps_the_base_graph() {
    let mut s = scene(
        r#"Some((
            plan: None,
            hints: [(name: "layerh", kind: 2,
                     xform: ((1,0,0),(0,1,0),(0,0,1),(1,1,1)))],
            barriers: [],
        ))"#,
    );
    s.apply_mode_layer("con");
    let nav = s.manifest.nav.expect("nav");
    assert_eq!(nav.plan.expect("plan").routes, "nav/routes_base.bin");
    assert_eq!(nav.hints.len(), 2);
    assert_eq!(nav.barriers.len(), 1);
}

/// A layer with no AI data at all leaves the base world's alone.
#[test]
fn a_layer_without_nav_leaves_the_base_alone() {
    let mut s = scene("None");
    s.apply_mode_layer("con");
    let nav = s.manifest.nav.expect("nav");
    assert_eq!(nav.plan.expect("plan").routes, "nav/routes_base.bin");
    assert_eq!((nav.hints.len(), nav.barriers.len()), (1, 1));
}
