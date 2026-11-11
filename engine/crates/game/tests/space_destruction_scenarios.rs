//! Fast (<1s) headless micro-scenarios for Space Battles / Shared Destruction (Issue #125).
//!
//! Covers:
//! 1. `ExplosionDestruct` vs `ExplosionName` resolution on AAT, Hailfire (both games), BF1 MTT,
//!    flyers, speeders, and turrets/stations (SWBFRONT lines 89800–89830, SWBFRNT2 lines 80169–80185, 124798–124808).
//! 2. BF2 Jedi starfighter empty chunk entry handling and turret/station chunk checks (SWBFRNT2 FUN_00213630, FUN_00215f80).
//! 3. Texture-upload cache reuse between intact vehicle and chunk loading, with counter verification proving
//!    elimination of duplicate texture uploads.
//! 4. Droid debris shaded mesh upload/draw path (verifying ordinary model shading instead of unlit SolidMesh).
//! 5. Turret and station destruction chunk piece launch (authored and explosion NumChunks pieces).
//! 6. Chunk world and live vehicle hull contact bounce per PS2 FUN_001d7740.

use core::collision::{HullPart, Layers, Shape};
use core::sim::{Event, Sim, Station, StationClass};
use core::unit::UnitSpec;
use core::vehicle::VehicleHandle;
use core::weapon::ExplosionClass;
use game::assets::{CpuMesh, CpuTexture, Scene};
use game::classdb::{fnv1a_lowercase, ClassDb, ClassDef};
use game::fx::chunks::Chunks;
use game::fx::emitter::Era;
use game::fx::FxAssets;
use game::vehicle_class::{explosion_name, has_chunks};
use game::viewer::VehicleVisuals;
use game::walk::WalkSim;
use glam::{Mat4, Quat, Vec3};
use manifest::Manifest;
use render::{DrawLayer, LightPath, Renderer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn make_test_db(defs: Vec<ClassDef>) -> ClassDb {
    let mut db = ClassDb::default();
    let temp_name = format!(
        "space_destruct_test_{}_{}.ron",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let temp_path = std::env::temp_dir().join(temp_name);
    let ron_str = ron::to_string(&defs).expect("serialize test class defs");
    std::fs::write(&temp_path, ron_str).expect("write temp class file");
    db.load(&temp_path)
        .expect("load temp class file into ClassDb");
    let _ = std::fs::remove_file(temp_path);
    db
}

fn class_def(name: &str, base: &str, props: &[(&str, &str)]) -> ClassDef {
    ClassDef {
        name: name.to_string(),
        base: base.to_string(),
        kind: "entc".to_string(),
        props: props
            .iter()
            .map(|(k, v)| (fnv1a_lowercase(k), v.to_string()))
            .collect(),
    }
}

fn make_test_mesh(texture_name: &str) -> CpuMesh {
    CpuMesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        colors: vec![[255, 255, 255, 255]; 3],
        indices: vec![0, 1, 2],
        submeshes: vec![(0, 3, texture_name.to_owned(), DrawLayer::Opaque, 0)],
        spec: vec![None],
        light: vec![LightPath::Lit],
        scroll: vec![[0.0, 0.0]],
        sphere: None,
        vis_top: None,
        render_grid: None,
        contact_surface: false,
    }
}

fn make_test_texture() -> CpuTexture {
    CpuTexture {
        width: 2,
        height: 2,
        rgba: vec![255; 16],
        extra_mips: Vec::new(),
        detail: None,
    }
}

// -----------------------------------------------------------------------------
// Test 1: ExplosionDestruct vs ExplosionName resolution
// -----------------------------------------------------------------------------
#[test]
fn test_explosion_destruct_vs_explosion_name_resolution() {
    let defs = vec![
        // AAT: authors ExplosionDestruct, no ExplosionName
        class_def(
            "cis_hover_aat",
            "hover",
            &[("ExplosionDestruct", "cis_hover_aat_exp")],
        ),
        // Hailfire: authors ExplosionDestruct on treaded base, no ExplosionName
        class_def(
            "cis_tread_hailfire",
            "treaded",
            &[("ExplosionDestruct", "cis_tread_hailfire_exp")],
        ),
        // MTT: authors ExplosionDestruct on commandhover base, no ExplosionName
        class_def(
            "cis_hover_mtt",
            "commandhover",
            &[("ExplosionDestruct", "cis_hover_mtt_exp")],
        ),
        // Flyer: authors ExplosionDestruct
        class_def(
            "rep_fly_anakinstarfighter_sc",
            "flyer",
            &[("ExplosionDestruct", "rep_fly_anakin_exp")],
        ),
        // Speeder: authors ExplosionName
        class_def(
            "imp_hover_speederbike",
            "hover",
            &[("ExplosionName", "imp_hover_speederbike_exp")],
        ),
        // Turret: building turret authors ExplosionName
        class_def(
            "tur_bldg_hoth_dishturret",
            "armedbuilding",
            &[("ExplosionName", "tur_bldg_hoth_dishturret_exp")],
        ),
        // Precedence test: when both are authored, ExplosionDestruct takes precedence
        class_def(
            "test_precedence_vehicle",
            "hover",
            &[
                ("ExplosionDestruct", "high_priority_exp"),
                ("ExplosionName", "low_priority_exp"),
                ("Explosion", "lowest_priority_exp"),
            ],
        ),
    ];

    let db = make_test_db(defs);

    // Verify hover AAT resolution
    assert_eq!(
        explosion_name(&db, "cis_hover_aat"),
        Some("cis_hover_aat_exp".to_string()),
        "AAT ExplosionDestruct must be resolved"
    );

    // Verify treaded Hailfire resolution
    assert_eq!(
        explosion_name(&db, "cis_tread_hailfire"),
        Some("cis_tread_hailfire_exp".to_string()),
        "Hailfire ExplosionDestruct must be resolved"
    );

    // Verify commandhover MTT resolution
    assert_eq!(
        explosion_name(&db, "cis_hover_mtt"),
        Some("cis_hover_mtt_exp".to_string()),
        "MTT ExplosionDestruct must be resolved"
    );

    // Verify flyer resolution
    assert_eq!(
        explosion_name(&db, "rep_fly_anakinstarfighter_sc"),
        Some("rep_fly_anakin_exp".to_string()),
        "Flyer ExplosionDestruct must be resolved"
    );

    // Verify speeder bike ExplosionName resolution
    assert_eq!(
        explosion_name(&db, "imp_hover_speederbike"),
        Some("imp_hover_speederbike_exp".to_string()),
        "Speederbike ExplosionName must be resolved"
    );

    // Verify building turret ExplosionName resolution
    assert_eq!(
        explosion_name(&db, "tur_bldg_hoth_dishturret"),
        Some("tur_bldg_hoth_dishturret_exp".to_string()),
        "Dish turret ExplosionName must be resolved"
    );

    // Verify precedence
    assert_eq!(
        explosion_name(&db, "test_precedence_vehicle"),
        Some("high_priority_exp".to_string()),
        "ExplosionDestruct must take precedence over ExplosionName"
    );
}

// -----------------------------------------------------------------------------
// Test 2: BF2 Jedi Starfighter Empty Chunk Entries & Turret/Station Chunks
// -----------------------------------------------------------------------------
#[test]
fn test_jedi_starfighter_empty_chunk_entries_and_has_chunks() {
    let defs = vec![
        // BF2 Jedi starfighter: has an empty ChunkGeometryName entry before valid entries
        class_def(
            "jed_fly_jedistarfighter",
            "flyer",
            &[
                ("CHUNKSECTION", "CHUNK1"),
                ("ChunkGeometryName", ""), // Empty entry
                ("CHUNKSECTION", "CHUNK2"),
                ("ChunkGeometryName", "jed_starfighter_chunk2"), // Valid entry
                ("CHUNKSECTION", "CHUNK3"),
                ("ChunkGeometryName", "   "), // Whitespace-only entry
                ("CHUNKSECTION", "CHUNK4"),
                ("ChunkGeometryName", "jed_starfighter_chunk4"), // Valid entry
            ],
        ),
        // Turret with only empty chunk entry: must NOT claim has_chunks
        class_def(
            "tur_bldg_tower",
            "armedbuilding",
            &[("CHUNKSECTION", "CHUNK1"), ("ChunkGeometryName", "")],
        ),
        // Vehicle with no chunks of its own, but its explosion class defines chunks
        class_def(
            "class_with_exp_chunks",
            "hover",
            &[("ExplosionDestruct", "exp_with_chunks")],
        ),
        class_def(
            "exp_with_chunks",
            "explosion",
            &[
                ("CHUNKSECTION", "CHUNK1"),
                ("ChunkGeometryName", "exp_chunk_piece"),
            ],
        ),
    ];

    let db = make_test_db(defs);

    // jed_fly_jedistarfighter has valid sibling chunks, so has_chunks is true
    assert!(
        has_chunks(&db, "jed_fly_jedistarfighter"),
        "Jedi starfighter with empty chunk entries must still have chunks via valid siblings"
    );

    // tur_bldg_tower has only empty chunk entries, so has_chunks is false
    assert!(
        !has_chunks(&db, "tur_bldg_tower"),
        "Building turret with empty ChunkGeometryName must not report has_chunks == true"
    );

    // class_with_exp_chunks has chunks via its explosion class
    assert!(
        has_chunks(&db, "class_with_exp_chunks"),
        "Vehicle without chunks must resolve has_chunks through its explosion class"
    );

    // Test ExplosionClass::from_props with NumChunks
    struct TestProps<'a>(&'a [(&'a str, &'a str)]);
    impl core::weapon::PropSource for TestProps<'_> {
        fn prop(&self, key: &str) -> Option<&str> {
            self.0
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| *v)
        }
        fn prop_hash(&self, hash: u32) -> Option<&str> {
            self.0
                .iter()
                .find(|(k, _)| fnv1a_lowercase(k) == hash)
                .map(|(_, v)| *v)
        }
    }

    let exp_props = [
        ("Damage", "500.0"),
        ("DamageRadius", "10.0"),
        ("NumChunks", "4"),
    ];
    let exp = ExplosionClass::from_props(&TestProps(&exp_props));
    assert_eq!(
        exp.num_chunks, 4,
        "ExplosionClass must parse NumChunks correctly"
    );

    // Missing NumChunks defaults to 0
    let exp_default = ExplosionClass::from_props(&TestProps(&[]));
    assert_eq!(
        exp_default.num_chunks, 0,
        "ExplosionClass default num_chunks must be 0 when unauthored"
    );
}

// -----------------------------------------------------------------------------
// Test 3: Texture-Upload Cache Reuse (Intact Vehicle <-> Chunk Loading)
// -----------------------------------------------------------------------------
#[test]
fn test_texture_upload_cache_reuse_eliminates_duplicates() {
    let mut r = match Renderer::new_headless(64, 64) {
        Ok(renderer) => renderer,
        Err(e) => {
            eprintln!("Skipping GPU headless test (no adapter available): {e}");
            return;
        }
    };

    let shared_texture_name = "veh_hull_shared";

    let manifest: Manifest = ron::from_str(
        r#"(
        map: "test_space_map",
        source_game: "bf2",
        terrain_name: None,
        sky_name: None,
        terrain_glb: None,
        models: ["test_flyer", "test_flyer_chunk"],
        textures: ["veh_hull_shared"],
        classes: [],
        geometries: [
            (
                name: "test_flyer",
                model: "test_flyer",
                lods: [("LOD0", "test_flyer")],
                far: None,
                hardpoints: [],
            ),
            (
                name: "test_flyer_chunk",
                model: "test_flyer_chunk",
                lods: [("LOD0", "test_flyer_chunk")],
                far: None,
                hardpoints: [],
            ),
        ],
        regions: [],
        instances: [],
    )"#,
    )
    .expect("deserialize test manifest");

    let mut meshes = HashMap::new();
    meshes.insert(
        "test_flyer".to_string(),
        make_test_mesh(shared_texture_name),
    );
    meshes.insert(
        "test_flyer_chunk".to_string(),
        make_test_mesh(shared_texture_name),
    );

    let mut textures = HashMap::new();
    textures.insert(shared_texture_name.to_string(), make_test_texture());

    let scene = Scene {
        dir: PathBuf::from("dummy_dir"),
        manifest,
        meshes,
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures,
        collision: HashMap::new(),
        cutters: Vec::new(),
    };

    // Step A: Load the intact vehicle
    let mut vis = VehicleVisuals::load(
        &mut r,
        &scene,
        Path::new("dummy_assets"),
        &[Some("test_flyer".to_string())],
    );

    assert_eq!(
        vis.texture_uploads(),
        1,
        "Initial vehicle load must upload the texture exactly once"
    );
    assert_eq!(
        vis.texture_cache_hits(),
        0,
        "Initial vehicle load starts with 0 cache hits"
    );

    // Step B: Load the chunk model which shares the exact same texture
    let chunk_keys = vis.load_chunks(
        &mut r,
        &scene,
        Path::new("dummy_assets"),
        &["test_flyer_chunk".to_string()],
    );

    assert_eq!(
        chunk_keys.len(),
        1,
        "Chunk model must be successfully registered"
    );

    // Step C: Verify zero duplicate texture uploads and cache hit recorded!
    assert_eq!(
        vis.texture_uploads(),
        1,
        "Texture upload count must remain 1 — duplicate texture upload was eliminated!"
    );
    assert_eq!(
        vis.texture_cache_hits(),
        1,
        "Texture cache hit must be recorded on chunk loading reuse"
    );
}

struct DummyAssets;
impl FxAssets for DummyAssets {
    fn texture(&mut self, _: &str) -> Option<render::TextureHandle> {
        None
    }
    fn model(&mut self, _: &str) -> Option<std::sync::Arc<game::fx::draw::EffectModel>> {
        None
    }
}

// -----------------------------------------------------------------------------
// Test 4: Droid Debris Shaded Mesh Upload & Draw Path
// -----------------------------------------------------------------------------
#[test]
fn test_droid_debris_shaded_mesh_upload_and_draw_path() {
    let mut r = match Renderer::new_headless(64, 64) {
        Ok(renderer) => renderer,
        Err(e) => {
            eprintln!("Skipping GPU headless test (no adapter available): {e}");
            return;
        }
    };

    let droid_chunk_model = "bdroid_chunk1";
    let droid_tex_name = "openbf_droid_bdroid";

    let manifest: Manifest = ron::from_str(
        r#"(
        map: "test_droid_map",
        source_game: "bf2",
        terrain_name: None,
        sky_name: None,
        terrain_glb: None,
        models: ["bdroid_chunk1"],
        textures: ["openbf_droid_bdroid"],
        classes: [],
        geometries: [
            (
                name: "bdroid_chunk1",
                model: "bdroid_chunk1",
                lods: [("LOD0", "bdroid_chunk1")],
                far: None,
                hardpoints: [],
            ),
        ],
        regions: [],
        instances: [],
    )"#,
    )
    .expect("deserialize test manifest");

    let mut meshes = HashMap::new();
    meshes.insert(
        droid_chunk_model.to_string(),
        make_test_mesh(droid_tex_name),
    );

    let mut textures = HashMap::new();
    textures.insert(droid_tex_name.to_string(), make_test_texture());

    let scene = Scene {
        dir: PathBuf::from("dummy_dir"),
        manifest,
        meshes,
        terrain: None,
        water: None,
        lowres: None,
        terrain_layers: None,
        textures,
        collision: HashMap::new(),
        cutters: Vec::new(),
    };

    let mut vis = VehicleVisuals::load(&mut r, &scene, Path::new("dummy_assets"), &[]);

    // Upload droid chunk model as ClassVis
    let loaded_chunks = vis.load_chunks(
        &mut r,
        &scene,
        Path::new("dummy_assets"),
        &[droid_chunk_model.to_string()],
    );
    assert_eq!(
        loaded_chunks,
        vec![droid_chunk_model.to_string()],
        "Droid chunk model must upload via ClassVis"
    );

    // Setup Chunks with droid class
    let defs = vec![class_def(
        "cis_inf_battledroid",
        "soldier",
        &[
            ("CHUNKSECTION", "CHUNK1"),
            ("ChunkGeometryName", droid_chunk_model),
            ("ChunkSpeed", "5.0"),
        ],
    )];
    let db = make_test_db(defs);
    let mut chunks = Chunks::load(&db, &["cis_inf_battledroid".to_string()], &[], Era::Bf2);

    // Register chunk model in lit set
    chunks.set_lit(loaded_chunks);

    // Launch droid chunk
    chunks.launch(
        "cis_inf_battledroid",
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::Y,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
    );
    assert_eq!(chunks.len(), 1, "One droid piece launched");

    // Verify draws() produces ZERO unlit/flat particle draws (since lit chunk bypasses unlit SolidMesh)
    let mut particle_draws = Vec::new();
    chunks.draws(
        Era::Bf2,
        Vec3::ZERO,
        Vec3::Z,
        1.0,
        0.0,
        &mut DummyAssets,
        &mut particle_draws,
    );
    assert_eq!(
        particle_draws.len(),
        0,
        "Lit droid debris must NOT draw through unlit particle/SolidMesh draws"
    );

    // Verify lit_poses yields the droid chunk piece
    let poses: Vec<_> = chunks.lit_poses().collect();
    assert_eq!(poses.len(), 1, "Lit poses must contain the droid piece");
    assert_eq!(poses[0].0, droid_chunk_model);

    // Verify vehicle_vis.chunk_draws produces lit DrawCmds
    let cmds = vis.chunk_draws(poses.into_iter());
    assert!(
        !cmds.is_empty(),
        "chunk_draws must produce DrawCmds for the lit droid piece"
    );
    assert_eq!(
        cmds[0].light,
        render::LightPath::Lit,
        "Droid debris draws must route to lit model shading"
    );
}

// -----------------------------------------------------------------------------
// Test 5: Turret & Station Destruction Spawns Authored and Explosion Chunks
// -----------------------------------------------------------------------------
#[test]
fn test_turret_and_station_destruction_chunk_launch() {
    let defs = vec![
        // Ground / building turret with authored ChunkSECTION
        class_def(
            "tur_bldg_laserturret",
            "armedbuilding",
            &[
                ("CHUNKSECTION", "CHUNK1"),
                ("ChunkGeometryName", "tur_laser_chunk1"),
                ("ChunkSpeed", "4.0"),
                ("CHUNKSECTION", "CHUNK2"),
                ("ChunkGeometryName", "tur_laser_chunk2"),
                ("ChunkSpeed", "4.0"),
            ],
        ),
        // Destructible space station with death explosion NumChunks
        class_def(
            "all_station_shield",
            "powerupstation",
            &[("ExplosionDestruct", "all_station_shield_exp")],
        ),
        class_def(
            "all_station_shield_exp",
            "explosion",
            &[
                ("ChunkGeometryName", "station_debris_chunk"),
                ("NumChunks", "3"),
            ],
        ),
    ];
    let db = make_test_db(defs);

    let mut chunks = Chunks::load(
        &db,
        &[
            "tur_bldg_laserturret".to_string(),
            "all_station_shield".to_string(),
        ],
        &[],
        Era::Bf2,
    );

    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    walk.vehicle_classes = vec!["tur_bldg_laserturret".to_string()];
    walk.sim.station_classes = vec![StationClass::soldier("all_station_shield", 1.0, 100.0, 10)];
    walk.sim.stations = vec![Station::new(0, Vec3::new(20.0, 5.0, 20.0), 4.0)];

    // Trigger destruction events
    walk.sim.events = vec![
        Event::VehicleDestroyed {
            vehicle: VehicleHandle { index: 0, gen: 0 },
            class: 0,
            pos: Vec3::new(0.0, 2.0, 0.0),
            vel: Vec3::ZERO,
            killer: None,
        },
        Event::StationDestroyed {
            station: 0,
            class: 0,
            pos: Vec3::new(20.0, 5.0, 20.0),
            killer: None,
        },
    ];

    chunks.tick(&walk, None);

    // 2 pieces from turret + 3 pieces from station explosion NumChunks = 5 pieces total!
    assert_eq!(
        chunks.len(),
        5,
        "Turret and station destruction must spawn authored chunks (2) and explosion NumChunks pieces (3)"
    );
}

// -----------------------------------------------------------------------------
// Test 6: Chunk Contact and Bounce Against Live Vehicle Hull
// -----------------------------------------------------------------------------
#[test]
fn test_chunk_contact_and_bounce_on_live_vehicle_hull() {
    let defs = vec![class_def(
        "test_bounce_vehicle",
        "hover",
        &[
            ("CHUNKSECTION", "CHUNK1"),
            ("ChunkGeometryName", "test_bounce_piece"),
            ("ChunkSpeed", "0.0"),
            ("ChunkBounciness", "0.8"),
            ("ChunkTerrainCollisions", "0"), // Budget = 0: terrain bounces would be suppressed!
            ("ChunkGravity", "0.0 0.0 0.0"), // No gravity for clean impulse measurement
        ],
    )];
    let db = make_test_db(defs);

    let mut chunks = Chunks::load(&db, &["test_bounce_vehicle".to_string()], &[], Era::Bf2);

    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    // Heightfield placed far below so terrain is not contacted
    walk.sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 100.0,
        offset_x: 50.0,
        offset_z: 50.0,
        heights: vec![-100.0; 4],
    });

    // Place a live vehicle hull body at the origin: box half-extents (5, 0.5, 5), top surface at y = 0.5
    let owner = VehicleHandle { index: 1, gen: 0 };
    let hull_part = HullPart::new(
        Mat4::IDENTITY,
        Shape::Box {
            half: Vec3::new(5.0, 0.5, 5.0),
        },
        Layers::VEHICLE,
    );
    walk.sim
        .collision
        .set_hull(owner, &[hull_part], Mat4::IDENTITY);

    // Launch chunk directly above the vehicle hull, falling downwards
    chunks.launch(
        "test_bounce_vehicle",
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::ZERO,
        Quat::IDENTITY,
        1.0,
        1.0,
        None,
    );
    assert_eq!(chunks.len(), 1, "One chunk piece launched");

    // Impart downward velocity toward the hull
    chunks.set_piece_vel(0, 0, Vec3::new(0.0, -10.0, 0.0));

    // Tick simulation
    chunks.tick(&walk, None);

    let states = chunks.piece_states();
    assert_eq!(states.len(), 1);
    let (pos, vel) = states[0];

    // Verify upward bounce impulse against the vehicle hull
    assert!(
        vel.y > 0.0,
        "Chunk must bounce upwards upon contacting vehicle hull (got vel.y = {})",
        vel.y
    );
    assert!(
        pos.y >= 0.5,
        "Chunk must be pushed out above the vehicle hull top (got pos.y = {})",
        pos.y
    );
}

fn baked_root(var: &str, fallback: &str) -> std::path::PathBuf {
    std::env::var_os(var).map_or_else(
        || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(fallback),
        std::path::PathBuf::from,
    )
}

fn mission_frigates(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| l.contains("\"frigates\", 18)"))
        .count()
}

#[test]
fn baked_frigates_break_into_their_hull_pieces_for_eight_seconds_on_all_six_maps() {
    let root = baked_root("OPENBF_ASSETS_BAKED", "../../assets_baked");
    let missions = baked_root("OPENBF_MISSIONS", "../../missions");
    if !root.join("bf2_spa1").exists() {
        assert!(
            std::env::var_os("OPENBF_REQUIRE_SPACE_ASSETS").is_none(),
            "space assets required"
        );
        eprintln!("skipping frigate destruction: local baked assets unavailable");
        return;
    }
    let authored = [
        ("all_prop_blockaderunner", 2usize),
        ("spa1_prop_minidestroyer2", 3),
        ("spa1_prop_correliancruiser", 4),
        ("spa2_prop_minifedcruiser", 4),
        ("spa2_prop_minirepassaultship", 5),
    ];
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for map in [
        "bf2_spa1", "bf2_spa3", "bf2_spa6", "bf2_spa7", "bf2_spa8", "bf2_spa9",
    ] {
        let mut scene = game::assets::load_scene(&root.join(map)).unwrap();
        scene.apply_mode_layer("ass").expect("the mission layer");
        let mission =
            game::mission::MissionDef::load(&missions.join(format!("{map}_ass.ron"))).unwrap();
        let def = mission.space_assault.clone().expect("space assault");
        let build = || {
            let mut sim = Sim::empty(137);
            scene.install_collision(&mut sim);
            game::vehicle_pads::install_with_objective(
                &mut sim,
                &scene.manifest,
                &scene.dir,
                &root.join("classes"),
                &mission.factions(),
                Some((&def, 180)),
            )
            .unwrap();
            sim
        };
        let sim = build();
        let placed = mission_frigates(&missions.join(format!("{map}_ass.ron")));
        let mut db = ClassDb::default();
        db.load_map(&scene.dir).unwrap();
        let names: Vec<String> = sim.station_classes.iter().map(|c| c.name.clone()).collect();
        let dirs = [scene.dir.join("models")];
        let mut map_frigates = 0;
        for si in 0..sim.stations.len() {
            let class = sim.station_classes[usize::from(sim.stations[si].class)].clone();
            let Some(&(_, pieces)) = authored
                .iter()
                .find(|(n, _)| class.name.eq_ignore_ascii_case(n))
            else {
                continue;
            };
            let Some(points) = sim
                .space_assault
                .as_ref()
                .and_then(|sa| sa.critical.iter().find(|c| c.station == si as u16))
                .map(|c| c.points)
            else {
                continue;
            };
            map_frigates += 1;
            *seen.entry(class.name.to_ascii_lowercase()).or_default() += 1;
            let mut walk = WalkSim::new(1, Vec3::ZERO, None);
            walk.sim = build();
            let sim = &mut walk.sim;
            let station = sim.stations[si];
            let own = station.team.expect("a frigate has a side");
            let instance = station.instance.expect("a placed frigate");
            let solid = |sim: &Sim| {
                sim.collision
                    .bodies()
                    .iter()
                    .filter(|b| b.instance == Some(instance) && b.layers != Layers::NONE)
                    .count()
            };
            assert!(solid(sim) > 0, "{map} {} hull collision", class.name);
            assert_eq!(class.destroyed_geometry, None, "{map} {}", class.name);
            let killer = sim
                .spawn(&UnitSpec::trooper(1 - own, station.pos + Vec3::Y * 2000.0))
                .unwrap();
            let before = sim.space_assault.as_ref().unwrap().points;
            let desc = core::weapon::DamageDesc {
                damage: class.max_health - 1.0,
                scales: [1.0; 7],
                shield_scale: 1.0,
            };
            sim.events.clear();
            sim.damage_station(si as u16, &desc, 1.0, Some(killer), Some(1 - own));
            assert!(!sim.stations[si].destroyed, "{map} {} survives", class.name);
            assert!(solid(sim) > 0, "{map} {} stays solid alive", class.name);
            assert!(sim
                .events
                .iter()
                .all(|e| !matches!(e, Event::StationDestroyed { .. })));
            sim.damage_station(si as u16, &desc, 1.0, Some(killer), Some(1 - own));
            sim.damage_station(si as u16, &desc, 1.0, Some(killer), Some(1 - own));
            assert!(sim.stations[si].destroyed);
            assert_eq!(solid(sim), 0, "{map} {} contact ends at death", class.name);
            let deaths = sim
                .events
                .iter()
                .filter(|e| matches!(e, Event::StationDestroyed { station, .. } if usize::from(*station) == si))
                .count();
            assert_eq!(deaths, 1, "{map} {} dies once", class.name);
            let mut want = before;
            want[usize::from(1 - own)] += points;
            assert_eq!(
                sim.space_assault.as_ref().unwrap().points,
                want,
                "{map} {} scores once at death",
                class.name
            );
            let mut chunks = Chunks::load(&db, &names, &dirs, Era::Bf2);
            chunks.tick(&walk, None);
            assert_eq!(chunks.len(), pieces, "{map} {} pieces", class.name);
            walk.sim.events.clear();
            let mut ticks = 1;
            while !chunks.is_empty() {
                chunks.tick(&walk, None);
                ticks += 1;
                assert!(ticks < 400, "{map} {} cleanup is bounded", class.name);
            }
            {
                let (mut t, mut life) = (0f32, 1);
                while t < 8.0 {
                    t += core::TICK_DT;
                    life += 1;
                }
                assert_eq!(ticks, life, "{map} {} piece life", class.name);
            }
            assert_eq!(
                walk.sim.space_assault.as_ref().unwrap().points,
                want,
                "{map} {} keeps its score",
                class.name
            );
        }
        assert_eq!(map_frigates, placed, "{map} frigates");
        let hulls = sim
            .stations
            .iter()
            .filter(|s| {
                let c = &sim.station_classes[usize::from(s.class)];
                c.name.contains("_cap_") || c.name.contains("_fly_fedcruiser")
            })
            .count();
        assert!(hulls > 0, "{map} capital hulls");
    }
    eprintln!("frigate classes killed: {seen:?}");
    assert_eq!(seen.len(), authored.len(), "{seen:?}");
}
