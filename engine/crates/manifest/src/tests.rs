//! Round-trip: what the bake writes is exactly what the runtime reads back.
//!
//! Before this crate the two sides were separate structs, and a field added to
//! one and forgotten on the other was only caught by a map failing to load.
//! Here it is one type, so the round trip is the whole contract: serialize a
//! manifest with every `Option` populated and every `Vec` non-empty, read it
//! back, and require equality.

use super::*;

/// Every field of the nav block populated, so the round trip covers it.
fn full_nav(layer: &str) -> ManifestNav {
    ManifestNav {
        plan: Some(ManifestNavPlan {
            hubs: vec![ManifestNavHub {
                name: "Hub0".into(),
                pos: [132.69946, 0.001129, 11.74349],
                radius: 5.701_843,
                arcs: vec![2, 3, 176],
            }],
            arcs: vec![ManifestNavArc {
                name: "Connection1".into(),
                a: 6,
                b: 7,
                type_mask: 7,
                flags: 3,
            }],
            routes: format!("nav/routes_{layer}.bin"),
            plnp: Some(ManifestPlnp {
                groups: vec![vec![0x6ddb_a5eb], vec![0x0bad_f00d, 0xdead_beef]],
                records: format!("nav/plnp_{layer}.bin"),
            }),
        }),
        hints: vec![full_hint()],
        barriers: vec![full_barrier()],
    }
}

/// The other variant: a world that authors hints but no planning graph — seven
/// BF2 maps ship mode layers like this, and the schema exists to keep them.
fn nav_without_a_plan() -> ManifestNav {
    ManifestNav {
        plan: None,
        hints: vec![full_hint()],
        barriers: vec![full_barrier()],
    }
}

fn full_hint() -> ManifestHint {
    ManifestHint {
        name: "hintnode60".into(),
        kind: 2,
        xform: [
            [-0.42418706, 0.0, -0.90557456],
            [0.0, 1.0, 0.0],
            [-0.9055742, 0.0, 0.4241879],
            [-283.95245, 16.553478, 164.5722],
        ],
        primary_stance: Some(2),
        secondary_stance: Some(9),
        mode: Some(0),
        radius: Some(2.5),
        command_post: Some("CP2".into()),
        props: vec![(0xbf0f_1cd1, "2".into()), (0x92c0_5a59, "9".into())],
    }
}

fn full_barrier() -> ManifestBarrier {
    ManifestBarrier {
        name: "barrier9".into(),
        pos: [-100.21072, 0.0, -251.00842],
        rows: [
            [-0.42418706, 0.0, -0.90557456],
            [0.0, 1.0, 0.0],
            [-0.9055742, 0.0, 0.4241879],
        ],
        size: [1.5746852, 0.0, 1.7452064],
        flags: 31,
    }
}

/// Exactly how `bake::map` writes a manifest.
fn to_ron<T: Serialize>(v: &T) -> String {
    ron::ser::to_string_pretty(v, ron::ser::PrettyConfig::default()).expect("serialize")
}

fn full_class() -> ManifestClass {
    ManifestClass {
        effects: Vec::new(),
        name: "com_bldg_controlzone".into(),
        base: "commandpost".into(),
        geometry: Some("com_bldg_controlzone".into()),
        collision: vec![
            ("soldier".into(), "collisionmesh".into()),
            ("vehicle_only".into(), "p_vehicle_3".into()),
        ],
        door: Some(ManifestDoor {
            triggers: vec![ManifestDoorTrigger {
                hardpoint: "hp_trigger_1".into(),
                radius: 4.0,
                position: [0.0, 1.5, 3.0],
            }],
            frames: 2,
            prims: vec![ManifestDoorPrim {
                name: "p_door_left".into(),
                motion: vec![
                    ([0.0, 0.0, 0.0, 1.0], [0.0; 3]),
                    ([0.0, 0.0, 0.0, 1.0], [-1.5, 0.0, 0.0]),
                ],
            }],
            parts: vec![ManifestDoorPart {
                submesh: 3,
                bone: "bone_door_left".into(),
                motion: vec![
                    ([0.0, 0.0, 0.0, 1.0], [0.0; 3]),
                    ([0.0, 0.0, 0.0, 1.0], [-1.5, 0.0, 0.0]),
                ],
            }],
            open_sound: Some("door_open".into()),
            close_sound: Some("door_close".into()),
        }),
        hardpoints: vec![ManifestHardpoint {
            name: "hp_fire_1".into(),
            pos: [0.25, 1.0, 2.5],
            axes: Some([[1.0, 0.0, 0.0], [0.0, 0.92, 0.38], [0.0, -0.38, 0.92]]),
        }],
        attach: vec![ManifestAttach {
            class: "nab_prop_leafpatch".into(),
            hardpoint: "hp_branch_1".into(),
            up: [0.0, 1.0, 0.0],
            pos: [1.5, 9.0, -0.5],
        }],
        build_zone: Some("com_prop_buildzone".into()),
        lights: vec![ManifestClassLight {
            class: "uta1_bldg_tower_flicker".into(),
            pos: [0.5, 24.75, -1.0],
            color: [230, 230, 255],
            radius: 12.0,
            is_static: false,
        }],
        turret_rig: Some(ManifestTurretRig {
            nodes: vec![ManifestRigNode {
                name: "aimer_y".into(),
                parent: None,
                bind: [
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 1.5, 0.0],
                ],
            }],
            parts: vec![None, Some(0)],
        }),
    }
}

fn full_geometry() -> ManifestGeometry {
    ManifestGeometry {
        name: "com_bldg_controlzone".into(),
        model: "com_bldg_controlzone".into(),
        lods: vec![
            ("LOD0".into(), "com_bldg_controlzone".into()),
            ("LOWD".into(), "com_bldg_controlzone_lowd".into()),
        ],
        far: Some("com_bldg_controlzone_lowd_far".into()),
        hardpoints: vec![ManifestHardpoint {
            name: "hp_smoke1".into(),
            pos: [0.5, 1.0, -2.0],
            axes: None,
        }],
    }
}

fn full_collision() -> ManifestCollision {
    ManifestCollision {
        geometry: "com_bldg_controlzone".into(),
        node: "collision".into(),
        glb: Some("collision/com_bldg_controlzone.glb".into()),
        prims: vec![ManifestPrim {
            name: "p_cylinder".into(),
            parent: "root".into(),
            kind: "cylinder".into(),
            dims: [1.5, 4.0, 0.0],
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [0.0, 2.0, 0.0],
            mask: 0x11,
            mask_own: true,
        }],
    }
}

/// Every `Option` is `Some` and every `Vec` has an entry, so the round trip
/// touches every field the schema has.
fn full_manifest() -> Manifest {
    Manifest {
        map: "tat1".into(),
        source_game: "bf2".into(),
        terrain_name: Some("tat1".into()),
        sky_name: Some("tat1sky".into()),
        terrain_glb: Some("terrain_tat1.glb".into()),
        terrain_query_size: Some(128),
        models: vec!["bldg_hut".into()],
        textures: vec!["sand".into()],
        transparent: vec!["glass".into()],
        texture_tiers: vec![("sand".into(), "pc".into())],
        classes: vec![full_class()],
        geometries: vec![full_geometry()],
        regions: vec![ManifestRegion {
            shape: "box".into(),
            name: "deathregion".into(),
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [1.0, 2.0, 3.0],
            size: [10.0, 20.0, 30.0],
            props: vec![(0xdead_beef, "Kill".into())],
        }],
        instances: vec![ManifestInstance {
            class: "com_bldg_controlzone".into(),
            name: "cp1".into(),
            rotation: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
            translation: [-4.5, 0.25, 88.0],
            // The gameplay props ride as RAW (hash, value) pairs — nothing in
            // the schema names them, which is why Phase B's ten per-faction
            // `Value_ATK_*` / `Value_DEF_*` hashes needed no manifest change
            // at all. They are here so the round trip covers them: five ATK
            // then five DEF, in `game::layout::PROP_VALUE_ATK` order.
            props: vec![
                (0x1234_5678, "1".into()),
                (0xA340_D6B5, "30".into()),
                (0x4DB4_4670, "30".into()),
                (0x9E78_B410, "30".into()),
                (0x00E7_3245, "30".into()),
                (0x573C_98A4, "10".into()),
                (0x44A2_C4A6, "50".into()),
                (0xA867_9457, "50".into()),
                (0x8F55_40CB, "50".into()),
                (0x5BF8_5AE8, "50".into()),
                (0xE689_EF93, "0".into()),
            ],
        }],
        paths: vec![ManifestPath {
            name: "spawnpath".into(),
            nodes: vec![([0.0, 1.0, 2.0], [1.0, 0.0, 0.0, 0.0])],
            props: vec![("SpawnPath".into(), 7)],
        }],
        layers: vec![
            ManifestLayer {
                name: "tat2_con".into(),
                worlds: vec!["tat2_con".into(), "tat2_turrets".into()],
                regions: vec![ManifestRegion {
                    shape: "sphere".into(),
                    name: "capture".into(),
                    rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    translation: [5.0, 0.0, 5.0],
                    size: [8.0, 8.0, 8.0],
                    props: vec![(1, "cp1".into())],
                }],
                instances: vec![ManifestInstance {
                    class: "com_bldg_controlzone".into(),
                    name: "cp2".into(),
                    rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    translation: [0.0, 0.0, 0.0],
                    props: vec![(2, "2".into())],
                }],
                nav: Some(full_nav("tat2_con")),
            },
            // A layer that authors hints but no plan: `nav` is Some, `plan` None.
            ManifestLayer {
                name: "hoth_objective".into(),
                worlds: vec!["hoth_objective".into()],
                regions: vec![],
                instances: vec![],
                nav: Some(nav_without_a_plan()),
            },
        ],
        lighting: ManifestLighting {
            source: "mixed".into(),
            sun_dir: [-0.4, -0.8, -0.45],
            sun_color: [1.0, 0.95, 0.8],
            ambient_sky: [0.2, 0.2, 0.25],
            ambient_ground: [0.1, 0.1, 0.12],
            character_ambient: Some([0.3, 0.3, 0.35]),
            fog: Some(ManifestFog {
                color: [0.5, 0.5, 0.6],
                near: 250.0,
                far: 350.0,
            }),
            near_scene_range: Some([1.0, 100.0, 150.0]),
            far_scene_range: Some(2000.0),
            sun_static: true,
            light2: Some(ManifestLight {
                dir: [0.0, -1.0, 0.0],
                color: [0.2, 0.3, 0.4],
                is_static: false,
            }),
            filter: Some(ManifestFilter {
                threshold: 50.0,
                intensity: 50.0,
            }),
        },
        sky: Some(ManifestSky {
            texture: Some("tat1sky".into()),
            models: vec![ManifestSkyModel {
                geometry: "tat1sky".into(),
                offset: -12.0,
                movement_scale: 1.0,
                reflect: true,
            }],
            ambient: [1.0, 1.0, 1.0],
            low_res_terrain: Some("tat1_lowres".into()),
            low_res_max_distance: Some(4000.0),
            objects: vec![ManifestSkyObject {
                geometry: "cru_star_destroyer".into(),
                count: 3,
                height: [400.0, 900.0],
                distance: 2500.0,
            }],
            terrain_enable: Some(0.0),
            reflection_color: Some([0.5, 0.5, 1.0]),
            stars: None,
        }),
        water: Some(ManifestWater {
            glb: "water.glb".into(),
            texture: Some("water_ps2".into()),
            height: 3.25,
            uv_velocity: [0.01, -0.02],
            layer_color: [1, 2, 3, 255],
            wet_cells: 128,
            patches: 16,
            triangles: 512,
            config: Some(full_water_config()),
            wet: Some(ManifestWetMask::from_nibbles(
                2,
                16.0,
                [16.0, 16.0],
                &[1, 0, 9, 15],
            )),
        }),
        collision: vec![full_collision()],
        terrain_cutters: vec![ManifestCutter {
            sphere: [40.0, 1.0, 2.0, 3.0],
            planes: vec![[0.0, 1.0, 0.0, -5.0], [1.0, 0.0, 0.0, -2.5]],
        }],
        nav: Some(full_nav("base")),
        texture_tier2: vec![("sand".into(), "sand.tier2.ktx2".into())],
        lowres_glb: Some("lowres_tat1.glb".into()),
        foliage: Some(ManifestFoliage {
            instances: "foliage.bin".into(),
            count: 2,
            meshes: vec![ManifestFoliageMesh {
                layer: 1,
                kind: FoliageKind::Model,
                name: Some("end_prop_fern5".into()),
                distance: 30.0,
                near_fade: 0.0,
                scale: 1.0,
                stiffness: 0.5,
                collision: false,
            }],
            path_props: vec![ManifestInstance {
                class: "end_prop_treebill".into(),
                name: String::new(),
                rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                translation: [1.0, 2.0, 3.0],
                props: vec![],
            }],
            grass: vec![ManifestGrassClass {
                name: "end_fern".into(),
                texture: "end_fern".into(),
                particles: 25,
                min_size: 1.2,
                max_size: 1.8,
                y_offset: 0.0,
                alpha: 1.0,
                max_distance: 35.0,
                radius_fade_min: 4.5,
                radius_fade_max: 5.5,
                darkness_min: 0.3,
                darkness_max: 0.7,
                num_parts: 3,
                rect: [-1.0, -1.0],
                raise: [0.3, 0.6],
                tall_size: 2.3,
                tall_lean: 0.35,
                shadow: 0.0,
                lean: 0.8,
                cycle: 6,
                thin: 0.3,
                still: false,
                atlas: vec![[[0.75, 0.0], [1.0, 0.0], [0.75, 0.5], [1.0, 0.5]]],
            }],
        }),
        terrain_layers_glb: Some("terrain_layers_tat1.glb".into()),
        leaf: vec![ManifestLeafClass {
            name: "nab2_prop_vine1".into(),
            texture: "nab2_leaf".into(),
            falling_count: 0,
            bird_count: 0,
            particles: 50,
            min_size: 1.2,
            max_size: 1.4,
            radius: 1.3,
            height: 13.0,
            scale: 1.6,
            offset: [0.0, 0.4, 0.0],
            taper: 8.0,
            seed: Some(937_377_218),
            layout: LeafLayout::Vine([2.0, 0.0]),
            vine_run: [2, 4],
            vine_step: 0.6,
            darkness_min: 0.4,
            darkness_max: 1.0,
            alpha: 1.0,
            max_distance: 80.0,
            num_parts: 1,
            wind: [1.2, 0.11],
        }],
        hdr: Some(ManifestHdr {
            num_bloom_passes: 3,
            max_total_weight: 1.0,
            glow_threshold: 0.5,
        }),
        point_lights: vec![ManifestPointLight {
            name: "macy16".into(),
            spot: true,
            cone: Some(ManifestSpotCone {
                dir: [0.0, -1.0, 0.0],
                outer: 1.047,
            }),
            pos: [12.0, 3.5, -40.25],
            color: [0.953, 0.992, 0.267],
            range: 10.0,
            is_static: true,
            overbright: true,
        }],
        lightning: Some(ManifestLightning {
            sky_dome_darken_factor: 0.4,
            sunlight_fade_factor: 0.1,
            color: [255.0, 255.0, 255.0],
            brightness_min: 1.0,
            fade_time: 0.2,
            time_between_flashes: [3.0, 5.0],
            time_between_sub_flashes: [0.01, 0.5],
            num_sub_flashes: [2.0, 5.0],
            horizon_angle: [30.0, 60.0],
            sound_crack: Some("kam_amb_thunder".into()),
            sound_sub_crack: Some("kam_amb_thundersub".into()),
        }),
        // BF2 KAM1's rain and wind, off the disc (card WTHR).
        precipitation: Some(ManifestPrecipitation {
            streaks: true,
            texture: Some("fx_snow".into()),
            particle_size: 0.06,
            range: 12.5,
            color: [216, 220, 228, 255],
            alpha_min_max: [0.8, 1.0],
            particle_density: 80.0,
            particle_density_range: 0.0,
            velocity: 8.0,
            velocity_range: 1.0,
            streak_length: 1.7,
            camera_cross_velocity_scale: 0.2,
            camera_axial_velocity_scale: 1.0,
            rotation: 0.0,
            rotation_range: 2.0,
            ground_effect: Some("com_sfx_rainsplash".into()),
            ground_effects_per_sec: 7.0,
            ground_effect_spread: 8.0,
        }),
        wind: Some(ManifestWind {
            velocity: [1.0, 0.3],
            velocity_range: 1.0,
            velocity_change_rate: 0.1,
        }),
    }
}

/// Every field of the `fx__` water config populated, so the round trip covers
/// it. The values are BF1 KAS2's, off the disc.
fn full_water_config() -> ManifestWaterConfig {
    ManifestWaterConfig {
        main_texture: Some("kas2_water".into()),
        foam_texture: Some("kam1_water".into()),
        metres_per_repeat: Some([16.0, 16.0]),
        foam_metres_per_repeat: Some([12.8, 12.8]),
        ocean: true,
        disable_low_res: true,
        specular_enable: true,
        oscillation_enable: false,
        min_diffuse: Some([32.0, 40.0, 32.0, 188.0]),
        max_diffuse: Some([128.0, 130.0, 128.0, 188.0]),
        border_diffuse: Some([160.0, 160.0, 160.0, 255.0]),
        specular_color: Some([192.0, 192.0, 192.0, 152.0]),
        speckle_textures: vec!["water_specularmask_0".into()],
        speckle_fps: Some(4.0),
        speckle_tile: Some([5.0, 5.0]),
        speckle_scroll: Some([0.11, 0.11]),
        speckle_coord_shift: Some([5.0, 5.0]),
        speckle_specular_color: Some([200.0, 200.0, 200.0, 150.0]),
        speckle_ambient_color: Some([50.0, 50.0, 50.0, 80.0]),
        light_azim_and_elev: Some([1.0, 0.0]),
        patch_divisions: Some([4.0, 4.0]),
        lod_decimation: Some(8.0),
    }
}

fn full_common() -> CommonManifest {
    CommonManifest {
        source: "ingame".into(),
        models: vec!["com_bldg_controlzone".into()],
        textures: vec!["com_bldg".into()],
        classes: vec![full_class()],
        geometries: vec![full_geometry()],
        collision: vec![full_collision()],
        texture_tier2: vec![("com_bldg".into(), "com_bldg.tier2.ktx2".into())],
    }
}

/// `foliage.bin` reads back what it wrote, and refuses a short or foreign file.
#[test]
fn foliage_bin_round_trips() {
    use foliage_bin::{read, write, Record};
    let recs = [
        Record {
            mesh: 0,
            x: -242.63098,
            z: 429.75894,
            yaw: 0.49947587,
            rgba: [211, 210, 205, 255],
        },
        Record {
            mesh: 7,
            x: 1.0,
            z: -2.0,
            yaw: 6.0,
            rgba: [255; 4],
        },
    ];
    let b = write(&recs);
    assert_eq!(b.len(), 12 + 2 * 20);
    assert_eq!(read(&b).unwrap(), recs);
    assert_eq!(read(&b[..b.len() - 1]), None);
    let mut wrong = b.clone();
    wrong[0] = b'X';
    assert_eq!(read(&wrong), None);
    assert_eq!(read(&write(&[])).unwrap(), []);
}

#[test]
fn manifest_round_trips_with_every_field_populated() {
    let m = full_manifest();
    let text = to_ron(&m);
    let back: Manifest = ron::from_str(&text).expect("deserialize manifest");
    assert_eq!(m, back);
    // And the text itself is stable, which is what makes a rebake diffable.
    assert_eq!(text, to_ron(&back));
}

#[test]
fn absent_terrain_query_frame_preserves_existing_manifest_bytes() {
    let mut m = full_manifest();
    m.terrain_query_size = None;
    let text = to_ron(&m);
    assert!(!text.contains("terrain_query_size"));
    let parsed: Manifest = ron::from_str(&text).unwrap();
    assert_eq!(parsed.terrain_query_size, None);
}

#[test]
fn common_manifest_round_trips_with_every_field_populated() {
    let c = full_common();
    let text = to_ron(&c);
    let back: CommonManifest = ron::from_str(&text).expect("deserialize common");
    assert_eq!(c, back);
    assert_eq!(text, to_ron(&back));
}

fn full_vehicles() -> VehicleManifest {
    VehicleManifest {
        source: "imp".into(),
        vehicles: vec![ManifestVehicle {
            class: full_class(),
            chunks: vec!["speederbike_chunk3".into(), "speederbike_chunk1".into()],
            sphere: Some([0.0, 0.403_274_57, 0.647_098_5, 1.936_126_4]),
            vis_min_y: Some(-1.778_591),
            walker: Some(ManifestWalker {
                bank: "imp_walk_atst".into(),
                clips: vec![
                    None,
                    Some(ManifestWalkerClip {
                        name: "walk_leftfoot_rightfoot".into(),
                        frames: 3,
                        duration: 2.0 / 30.0,
                        looping: true,
                        root_z: vec![0.0, 0.056_53, 0.171_8],
                    }),
                ],
                prims: vec![ManifestPrimPoint {
                    name: "p_l_foot".into(),
                    bone: "bone_l_toe".into(),
                    pos: [1.5, 0.1, 0.3],
                    radius: 1.0,
                }],
                skin: Some("imp_walk_atst".into()),
                layer_bones: vec![["bone_l_thigh".into(), "bone_r_thigh".into()]],
            }),
        }],
        geometries: vec![full_geometry()],
        models: vec!["imp_hover_speederbike".into()],
        textures: vec!["imp_speederbike".into()],
        collision: vec![full_collision()],
        droidekas: Vec::new(),
    }
}

#[test]
fn vehicle_manifest_round_trips_with_every_field_populated() {
    let v = full_vehicles();
    let text = to_ron(&v);
    let back: VehicleManifest = ron::from_str(&text).expect("deserialize vehicles");
    assert_eq!(v, back);
    assert_eq!(text, to_ron(&back));
}

/// The identity-only view reads a full manifest's text.
fn head_of(text: &str) -> ManifestHead {
    ron::from_str(text).expect("deserialize head")
}

#[test]
fn head_reads_a_full_manifest() {
    let h = head_of(&to_ron(&full_manifest()));
    assert_eq!(h.map, "tat1");
    assert_eq!(h.source_game, "bf2");
}

/// Every `#[serde(default)]` in the schema, exercised at once: a manifest text
/// holding ONLY the fields that predate them still loads. This is the promise
/// the defaults exist for — an old baked map keeps working — and it is what
/// would break if one were dropped while merging the two sides' structs.
#[test]
fn a_manifest_missing_every_defaulted_field_still_loads() {
    let old = r#"(
    map: "tat1",
    terrain_name: Some("tat1"),
    sky_name: None,
    terrain_glb: None,
    models: [],
    textures: [],
    classes: [],
    geometries: [],
    regions: [],
    instances: [],
)"#;
    let m: Manifest = ron::from_str(old).expect("pre-P4 manifest must still load");
    assert_eq!(m.map, "tat1");
    // source_game defaults to bf1: every bake that predates the field was BF1.
    assert_eq!(m.source_game, "bf1");
    assert!(m.transparent.is_empty());
    assert!(m.texture_tiers.is_empty());
    assert!(m.texture_tier2.is_empty());
    assert!(m.paths.is_empty());
    assert!(m.layers.is_empty());
    assert!(m.collision.is_empty());
    assert!(m.terrain_cutters.is_empty());
    assert_eq!(m.nav, None);
    assert_eq!(m.sky, None);
    assert_eq!(m.water, None);
    assert_eq!(m.foliage, None);
    // lighting falls back to the engine's old hardcoded look, not zeroes.
    assert_eq!(m.lighting, ManifestLighting::default());
    assert_eq!(m.lighting.source, "default");
    // A head parse of the same old text agrees.
    assert_eq!(head_of(old).source_game, "bf1");
}

/// The same for the nested types' own defaults, which the manifest-level
/// defaults would otherwise hide.
#[test]
fn nested_types_missing_their_defaulted_fields_still_load() {
    let m: Manifest = ron::from_str(
        r#"(
    map: "tat1",
    source_game: "bf2",
    terrain_name: None,
    sky_name: None,
    terrain_glb: None,
    models: [],
    textures: [],
    classes: [
        ( name: "c", base: "prop", geometry: None ),
        ( name: "d", base: "door", geometry: None, door: Some((
            triggers: [],
            frames: 1,
            prims: [],
        )) ),
    ],
    geometries: [],
    regions: [],
    instances: [],
    paths: [( name: "p", nodes: [] )],
    layers: [( name: "l", regions: [], instances: [] )],
    lighting: (
        sun_dir: (0.0, -1.0, 0.0),
        sun_color: (1.0, 1.0, 1.0),
        ambient_sky: (0.1, 0.1, 0.1),
        ambient_ground: (0.1, 0.1, 0.1),
    ),
    sky: Some((
        models: [( geometry: "s", offset: 0.0, movement_scale: 1.0 )],
        ambient: (1.0, 1.0, 1.0),
    )),
    water: Some(( glb: "water.glb", height: 1.0, config: Some(()) )),
    collision: [
        ( geometry: "g", glb: None ),
        ( geometry: "g2", glb: None, prims: [(
            name: "p_cube",
            kind: "box",
            dims: (1.0, 1.0, 1.0),
            rotation: ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)),
            translation: (0.0, 0.0, 0.0),
        )] ),
    ],
    terrain_cutters: [( planes: [] )],
    nav: Some(( hints: [( name: "h", kind: 2, xform: (
        (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), (0.0, 0.0, 0.0),
    ) )] )),
    foliage: Some(( instances: "foliage.bin", count: 0, meshes: [], path_props: [] )),
)"#,
    )
    .expect("nested defaults must hold");
    assert!(m.paths[0].props.is_empty());
    assert!(m.layers[0].worlds.is_empty());
    // A layer baked before the nav data has none, and falls back to the base.
    assert_eq!(m.layers[0].nav, None);
    assert_eq!(m.lighting.source, "");
    assert_eq!(m.lighting.character_ambient, None);
    assert_eq!(m.lighting.fog, None);
    assert_eq!(m.lighting.near_scene_range, None);
    assert_eq!(m.lighting.far_scene_range, None);
    assert!(!m.lighting.sun_static);
    assert_eq!(m.lighting.light2, None);
    let sky = m.sky.as_ref().unwrap();
    assert_eq!(sky.texture, None);
    assert_eq!(sky.low_res_terrain, None);
    assert_eq!(sky.low_res_max_distance, None);
    assert!(sky.objects.is_empty());
    assert_eq!(sky.terrain_enable, None);
    assert_eq!(sky.reflection_color, None);
    assert!(!sky.models[0].reflect);
    let water = m.water.as_ref().unwrap();
    assert_eq!(water.texture, None);
    assert_eq!(water.uv_velocity, [0.0, 0.0]);
    assert_eq!(water.layer_color, [0, 0, 0, 0]);
    assert_eq!((water.wet_cells, water.patches, water.triangles), (0, 0, 0));
    // A config with none of its keys: every value is unauthored, and the two
    // flags the PS2's level reset stores as 1 read back ON, not `false`.
    let cfg = water.config.as_ref().unwrap();
    assert_eq!((cfg.main_texture.as_ref(), cfg.min_diffuse), (None, None));
    assert!(!cfg.ocean && !cfg.disable_low_res);
    assert!(cfg.specular_enable && cfg.oscillation_enable);
    assert_eq!(m.collision[0].node, "");
    assert!(m.collision[0].prims.is_empty());
    // A class baked before the collision-layer properties were read has an
    // EMPTY vector, which is exactly the engine's default (every body in
    // every layer), and a prim baked before the BF2 MASK byte reads mask 0 =
    // all five subsets.
    assert!(m.classes[0].collision.is_empty());
    // A class from before doors and hardpoints has neither; a door from
    // before DRV has no drawn parts and no sounds.
    assert_eq!(m.classes[0].door, None);
    assert!(m.classes[0].hardpoints.is_empty());
    assert!(m.classes[0].attach.is_empty());
    let door = m.classes[1].door.as_ref().unwrap();
    assert!(door.parts.is_empty());
    assert_eq!(
        (door.open_sound.as_ref(), door.close_sound.as_ref()),
        (None, None)
    );
    assert_eq!(m.collision[1].prims[0].mask, 0);
    assert!(!m.collision[1].prims[0].mask_own);
    assert_eq!(m.terrain_cutters[0].sphere, [0.0; 4]);
    // Every part of a nav block is defaulted: a world with no plan and no
    // barrier reads back empty rather than failing, and a hint keeps only
    // what every hint authors.
    let nav = m.nav.as_ref().unwrap();
    assert_eq!(nav.plan, None);
    assert!(nav.barriers.is_empty());
    let hint = &nav.hints[0];
    assert_eq!(
        (hint.primary_stance, hint.secondary_stance, hint.mode),
        (None, None, None)
    );
    assert_eq!((hint.radius, hint.command_post.as_ref()), (None, None));
    assert!(hint.props.is_empty());
    // A foliage block from before FOLGG ships no grass classes.
    assert!(m.foliage.as_ref().unwrap().grass.is_empty());

    let c: CommonManifest = ron::from_str(
        r#"(
    source: "common",
    models: [],
    textures: [],
    classes: [],
    geometries: [],
)"#,
    )
    .expect("pre-collision common bake must still load");
    assert!(c.collision.is_empty());
    assert!(c.texture_tier2.is_empty());
}

/// Field ORDER is the wire format: RON writes fields in declaration order, so
/// reordering a struct silently rewrites every manifest, and the acceptance
/// test for this crate was a byte-identical rebake. Pin the order.
fn top_level_keys(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with("    ") && !l.starts_with("     "))
        .filter_map(|l| {
            l.trim_end()
                .split_once(':')
                .map(|(k, _)| k.trim().to_string())
        })
        .collect()
}

#[test]
fn manifest_field_order_is_the_wire_format() {
    assert_eq!(
        top_level_keys(&to_ron(&full_manifest())),
        [
            "map",
            "source_game",
            "terrain_name",
            "sky_name",
            "terrain_glb",
            "models",
            "textures",
            "transparent",
            "texture_tiers",
            "classes",
            "geometries",
            "regions",
            "instances",
            "paths",
            "layers",
            "lighting",
            "sky",
            "water",
            "collision",
            "terrain_cutters",
            "nav",
            "texture_tier2",
            "lowres_glb",
            "foliage",
            "terrain_layers_glb",
            "leaf",
            "hdr",
            "point_lights",
            "lightning",
            "precipitation",
            "wind",
            "terrain_query_size",
        ]
    );
}

/// The nav block's own field order, for the same reason: it is the wire
/// format, and `plan` wraps what used to sit directly in `ManifestNav`.
#[test]
fn nav_field_order_is_the_wire_format() {
    let nav = full_nav("base");
    assert_eq!(top_level_keys(&to_ron(&nav)), ["plan", "hints", "barriers"]);
    assert_eq!(
        top_level_keys(&to_ron(nav.plan.as_ref().unwrap())),
        ["hubs", "arcs", "routes", "plnp"]
    );
}

/// The water block's field order, for the same reason. `config` is APPENDED
/// after `triangles`, so a pre-`fx__` manifest's RON text is unchanged up to
/// that point.
#[test]
fn water_field_order_is_the_wire_format() {
    let w = full_manifest().water.unwrap();
    assert_eq!(
        top_level_keys(&to_ron(&w)),
        [
            "glb",
            "texture",
            "height",
            "uv_velocity",
            "layer_color",
            "wet_cells",
            "patches",
            "triangles",
            "config",
            "wet",
        ]
    );
    assert_eq!(
        top_level_keys(&to_ron(w.config.as_ref().unwrap())),
        [
            "main_texture",
            "foam_texture",
            "metres_per_repeat",
            "ocean",
            "disable_low_res",
            "specular_enable",
            "oscillation_enable",
            "min_diffuse",
            "max_diffuse",
            "border_diffuse",
            "specular_color",
            "speckle_textures",
            "speckle_fps",
            "speckle_tile",
            "speckle_scroll",
            "speckle_coord_shift",
            "speckle_specular_color",
            "speckle_ambient_color",
            "light_azim_and_elev",
            "patch_divisions",
            "lod_decimation",
            "foam_metres_per_repeat",
        ]
    );
}

#[test]
fn common_manifest_field_order_is_the_wire_format() {
    assert_eq!(
        top_level_keys(&to_ron(&full_common())),
        [
            "source",
            "models",
            "textures",
            "classes",
            "geometries",
            "collision",
            "texture_tier2",
        ]
    );
}

/// `vehicles.ron`, and one vehicle in it (`vis_min_y` went in before
/// `walker`, card VREST: this pins the order every side bake has written
/// since).
#[test]
fn vehicle_manifest_field_order_is_the_wire_format() {
    let v = full_vehicles();
    assert_eq!(
        top_level_keys(&to_ron(&v)),
        [
            "source",
            "vehicles",
            "geometries",
            "models",
            "textures",
            "collision",
        ]
    );
    assert_eq!(
        top_level_keys(&to_ron(&v.vehicles[0])),
        ["class", "chunks", "sphere", "vis_min_y", "walker"]
    );
}

/// The two structs A.4 extended: a new field goes at the END, or every baked
/// manifest is silently rewritten. `collision`, `door`, `hardpoints` after
/// `geometry` on the class (`turret_rig` last, card VTURD);
/// `mask`/`mask_own` after `translation` on the primitive.
#[test]
fn class_and_prim_field_order_is_the_wire_format() {
    assert_eq!(
        top_level_keys(&to_ron(&full_class())),
        [
            "name",
            "base",
            "geometry",
            "collision",
            "door",
            "hardpoints",
            "attach",
            "build_zone",
            "lights",
            "turret_rig"
        ]
    );
    assert_eq!(
        top_level_keys(&to_ron(&full_collision())),
        ["geometry", "node", "glb", "prims"]
    );
    assert_eq!(
        top_level_keys(&to_ron(&full_collision().prims[0])),
        [
            "name",
            "parent",
            "kind",
            "dims",
            "rotation",
            "translation",
            "mask",
            "mask_own",
        ]
    );
    // A geometry's far extension (card UTAS) is written only when it exists,
    // so the 31 maps without one bake byte-identical to before it.
    assert_eq!(
        top_level_keys(&to_ron(&full_geometry())),
        ["name", "model", "lods", "far", "hardpoints"]
    );
    let bare = ManifestGeometry {
        far: None,
        hardpoints: Vec::new(),
        ..full_geometry()
    };
    assert_eq!(top_level_keys(&to_ron(&bare)), ["name", "model", "lods"]);
}

// ---------------------------------------------------------------------------
// `sounds.ron` and `load.ron` — the two files that used to be hand-kept twins
// (`bake::sounds` vs `game::sounds`, `bake::loadscreen` vs
// `game::assets::load_loadscreen`; queue item 13). Same contract as the
// manifest above: one type, so the round trip IS the contract, and the field
// order is the wire format for the 34 `sounds.ron` and 33 `load.ron` files
// the current bake ships.
// ---------------------------------------------------------------------------

fn full_sound_prop() -> SoundProp {
    SoundProp {
        name_hash: 0x6a94_2216,
        kind: 3,
        gain: 0.75,
        pitch: 1.1,
        min_distance: 5.0,
        max_distance: 50.0,
        rolloff: 1.0,
        priority: 0.5,
        looping: true,
        samples: vec![(0xb75d_27cd, 1.0), (0x0bad_f00d, 0.5)],
        layers: vec![0xdead_beef],
        stream: Some(0x1234_5678),
        segments: vec![(7, 1.0)],
        mute_distance: Some(30.0),
        roll_in_distance: Some(2.0),
        flat: true,
        layer_delays: vec![(0.3, 0.1)],
        pitch_dev: Some(0.15),
        gain_dev: Some(0.05),
        play_interval: Some((0.5, 0.1)),
        play_probability: Some(0.5),
        level: 0x522c_fe83,
        bus: 0x7c3e_02a1,
        take_locks: vec![(25.0, 5.0), (0.0, 0.0)],
    }
}

fn full_sounds_ron() -> SoundsRon {
    SoundsRon {
        source: "tat".into(),
        banks: vec![(0x1111_1111, 320)],
        samples: vec![(0xb75d_27cd, "tat/all_weap_inf_rifle_fire.wav".into())],
        props: vec![full_sound_prop()],
        odd_frequency: 4,
        streamed: 1,
        engines: vec![EngineSound {
            name_hash: 7,
            level: 9,
            layers: vec![EngineLayer {
                sound: 8,
                delay: 0.8,
                deviation: 0.1,
                graphs: vec![EngineGraph {
                    parameter: "speed".into(),
                    target: "Pitch".into(),
                    points: vec![(0.0, 0.8), (1.0, 1.0)],
                }],
            }],
        }],
        music: vec![MusicRon {
            name_hash: 11,
            stream: 12,
            fade_in: 1.0,
            fade_out: 1.5,
            min_playback: 20.0,
            min_inactive: 15.0,
            max_playback: Some(180.0),
            priority: 1.0,
        }],
    }
}

#[test]
fn sounds_ron_round_trips_with_every_field_populated() {
    let s = full_sounds_ron();
    let back: SoundsRon = ron::from_str(&to_ron(&s)).expect("read back");
    assert_eq!(back, s);
}

#[test]
fn sounds_field_order_is_the_wire_format() {
    assert_eq!(
        top_level_keys(&to_ron(&full_sounds_ron())),
        [
            "source",
            "banks",
            "samples",
            "props",
            "odd_frequency",
            "streamed",
            "engines",
            "music"
        ]
    );
    assert_eq!(
        top_level_keys(&to_ron(&full_sound_prop())),
        [
            "name_hash",
            "kind",
            "gain",
            "pitch",
            "min_distance",
            "max_distance",
            "rolloff",
            "priority",
            "looping",
            "samples",
            "layers",
            "stream",
            "segments",
            "mute_distance",
            "roll_in_distance",
            "flat",
            "layer_delays",
            "pitch_dev",
            "gain_dev",
            "play_interval",
            "play_probability",
            "level",
            "bus",
            "take_locks",
        ]
    );
    // The fields after `segments` are written only when set: a property
    // without them is the text a pre-SNDT bake wrote.
    let plain = SoundProp {
        mute_distance: None,
        roll_in_distance: None,
        flat: false,
        layer_delays: vec![],
        pitch_dev: None,
        gain_dev: None,
        play_interval: None,
        play_probability: None,
        level: 0,
        bus: 0,
        take_locks: vec![],
        ..full_sound_prop()
    };
    let text = to_ron(&plain);
    assert_eq!(top_level_keys(&text).last(), Some(&"segments".to_string()));
    assert_eq!(ron::from_str::<SoundProp>(&text).expect("read back"), plain);
}

/// The shape `assets_baked` shipped before queue item 15 added `stream` /
/// `segments` and before the `odd_frequency` / `streamed` censuses: a reader
/// built now must still load it. This is the text the old `bake::sounds` test
/// carried, kept as the compatibility pin.
#[test]
fn a_pre_item_15_sounds_ron_still_loads() {
    let older = r#"(
    source: "t",
    banks: [],
    samples: [],
    props: [
        (
            name_hash: 1,
            kind: 2,
            gain: 0.5,
            pitch: 1.0,
            min_distance: 1.0,
            max_distance: 2.0,
            rolloff: 1.0,
            priority: 0.5,
            looping: false,
            samples: [],
            layers: [],
        ),
    ],
)"#;
    let back: SoundsRon = ron::from_str(older).expect("new reader on old file");
    assert_eq!(back.props[0].stream, None);
    assert!(back.props[0].segments.is_empty());
    assert_eq!(back.props[0].mute_distance, None);
    assert_eq!(back.props[0].roll_in_distance, None);
    assert!(!back.props[0].flat);
    assert_eq!(back.odd_frequency, 0);
    assert_eq!(back.streamed, 0);
}

#[test]
fn load_screen_round_trips_and_its_field_order_is_the_wire_format() {
    let ls = LoadScreen {
        sequence: vec!["load_galacticmap_0".into(), "load_locale_tatooine_1".into()],
        extras: vec!["load_planet_tattoine".into()],
        style: LoadStyle::Sequence,
    };
    let text = to_ron(&ls);
    assert_eq!(top_level_keys(&text), ["sequence", "extras", "style"]);
    let back: LoadScreen = ron::from_str(&text).expect("read back");
    assert_eq!(back, ls);
    // BF2's pool is the other variant, and a bake written before `style`
    // existed still loads as the BF1 sequence.
    let pool = LoadScreen {
        style: LoadStyle::Pool,
        ..ls
    };
    let back: LoadScreen = ron::from_str(&to_ron(&pool)).expect("read back");
    assert_eq!(back.style, LoadStyle::Pool);
    let older = r#"(sequence: ["mos_1"], extras: [])"#;
    let back: LoadScreen = ron::from_str(older).expect("new reader on old file");
    assert_eq!(back.style, LoadStyle::Sequence);
}

#[test]
fn class_def_round_trips_and_its_field_order_is_the_wire_format() {
    let c = ClassDef {
        name: "rep_weap_inf_rifle".into(),
        kind: "wpnc".into(),
        base: "com_weap_inf_rifle".into(),
        // A repeated key stays repeated, in file order (BF2's WeaponName list).
        props: vec![
            (0xf889_9c5e, "rep_weap_inf_rifle_ord".into()),
            (0xf889_9c5e, "second".into()),
        ],
    };
    let text = to_ron(&c);
    assert_eq!(top_level_keys(&text), ["name", "kind", "base", "props"]);
    let back: ClassDef = ron::from_str(&text).expect("read back");
    assert_eq!(back, c);
    // A classes.ron baked before `kind` existed still loads, as an empty kind.
    let older = r#"(name: "all_inf_soldier", base: "soldier", props: [(1, "6.5")])"#;
    let back: ClassDef = ron::from_str(older).expect("new reader on old file");
    assert_eq!(back.kind, "");
    assert_eq!(back.props, [(1, "6.5".to_string())]);
}

/// `effects.ron` (card FXP1) reads back what the bake wrote, nested
/// emitters, enums and tuple ranges included.
#[test]
fn an_effect_set_round_trips_through_ron() {
    let r = FxRange(0.3, 0.3);
    let geometry = FxGeometry {
        blend: FxBlend::Additive,
        kind: FxType::Animated,
        texture: Some("a_sparks3".into()),
        model: None,
        child: None,
        time_per_frame: 0.015,
        frame_size: 4.0,
        total_frames: 16.0,
        blur_value: 0.0,
        blur_res: 0,
        spark_length: 0.0,
        looping: true,
    };
    let spawner = FxSpawner {
        shape: FxShape::Spread,
        shape_ranges: [r; 9],
        offset: [r; 3],
        position_scale: r,
        velocity_scale: r,
        inherit_velocity_factor: r,
        size: r,
        color: [r; 4],
        hsv: false,
        fade_in_time: 0.0,
        start_rotation: FxRange(0.0, 360.0),
        rotation_velocity: FxRange(0.0, 0.0),
    };
    let transformer = FxTransformer {
        lifetime: 0.2,
        position: vec![FxPositionNode {
            lifetime: 0.5,
            op: FxPositionOp::None,
            value: [0.0; 3],
        }],
        size: vec![FxScalarNode {
            lifetime: 0.1,
            op: FxScalarOp::Scale,
            value: 3.0,
        }],
        rotation: vec![],
        color: vec![FxColorNode {
            lifetime: 0.01,
            op: FxColorOp::Reach,
            value: [1.0; 4],
        }],
    };
    let leaf = FxEmitter {
        label: "Flare".into(),
        start_delay: r,
        burst_delay: r,
        burst_count: r,
        max_particles: FxRange(-1.0, 0.0),
        color: [r; 4],
        hsv: true,
        size: r,
        spawner,
        transformer,
        geometry,
        min_lod_dist: 10.0,
        max_lod_dist: 50.0,
        bounding_radius: 5.0,
        no_register_step: false,
        world_aligned: true,
        sibling: None,
        sound_name: None,
    };
    let root = FxEmitter {
        label: "Sparks".into(),
        sibling: Some(Box::new(leaf.clone())),
        ..leaf
    };
    let set = FxSet {
        game: "bf1".into(),
        effects: vec![FxEffect {
            hash: 0xc42b_8278,
            name: Some("ord_explosion".into()),
            emitters: vec![root],
        }],
        beams: vec![FxBeam {
            hash: 0x3388_1368,
            name: "Blue_halo".into(),
            color: [50, 177, 219, 255],
            flare_intensity: 0.0,
            length: 1.0,
            initial_radius: [0.4, 0.1],
            final_radius: [0.65, 0.4],
            fade_length: 0.8,
            fade_factor: 0.3,
            draw_distance: 300.0,
            position: [0.0; 3],
            direction: [0.0, 1.0, 0.0],
        }],
        lightning: Vec::new(),
    };
    let text = ron::ser::to_string_pretty(&set, ron::ser::PrettyConfig::default()).unwrap();
    let back: FxSet = ron::from_str(&text).unwrap();
    assert_eq!(back, set);
}

/// The HUD bake (card BHUD): a layout tree with both argument kinds, a leaf
/// and a nested scope, a font and the index. An untagged argument has to read
/// back as the same variant — `1.0` a number, `"1"` a string.
#[test]
fn hud_files_round_trip() {
    use hud::*;
    let layout = HudLayout {
        name: "1playerhud".into(),
        nodes: vec![
            HudNode {
                name: "fileinfo".into(),
                args: vec![HudArg::Str("1playerhud".into())],
                children: vec![],
            },
            HudNode {
                name: "Group".into(),
                args: vec![HudArg::Str("player1health".into())],
                children: vec![
                    HudNode {
                        name: "Position".into(),
                        args: vec![HudArg::Num(0.06), HudArg::Num(0.84), HudArg::Num(0.0)],
                        children: vec![],
                    },
                    HudNode {
                        name: "#2b76908f".into(),
                        args: vec![HudArg::Num(1.0), HudArg::Str("1".into())],
                        children: vec![],
                    },
                ],
            },
        ],
    };
    let text = to_ron(&layout);
    let back: HudLayout = ron::from_str(&text).expect("read back layout");
    assert_eq!(back, layout);
    assert_eq!(text, to_ron(&back));

    let font = HudFont {
        name: "gamefont_medium".into(),
        line_height: 18,
        pages: vec!["gamefont_medium_tex0".into(), "gamefont_medium_tex1".into()],
        glyphs: vec![HudGlyph {
            code: 33,
            page: 0,
            advance: 5,
            x0: 0,
            x1: 4,
            y0: 0,
            y1: 16,
            uv: [0.1328125, 0.1640625, 0.0078125, 0.1328125],
        }],
    };
    let back: HudFont = ron::from_str(&to_ron(&font)).expect("read back font");
    assert_eq!(back, font);

    let index = HudIndex {
        layouts: vec!["hudtransforms".into(), "1playerhud".into()],
        fonts: vec!["gamefont_medium".into()],
        textures: vec![("white".into(), "CORE.LVL".into())],
    };
    let back: HudIndex = ron::from_str(&to_ron(&index)).expect("read back index");
    assert_eq!(back, index);
}

#[test]
fn old_leaf_rows_disable_airborne_particles() {
    let mut leaf = full_manifest().leaf.remove(0);
    leaf.falling_count = 6;
    let text = ron::ser::to_string(&leaf).unwrap();
    let parsed: ManifestLeafClass = ron::from_str(&text).unwrap();
    assert_eq!(parsed.falling_count, 6);
    let old = text.replace("falling_count:6,", "");
    let parsed: ManifestLeafClass = ron::from_str(&old).unwrap();
    assert_eq!(parsed.falling_count, 0);
}
