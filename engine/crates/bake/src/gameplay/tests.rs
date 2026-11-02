use super::*;
use rebuild::chunk;
use serde_json::json;

fn wrap(bytes: &[u8]) -> Vec<u8> {
    chunk(b"ucfb", bytes)
}
fn text(tag: &[u8; 4], value: &str) -> Vec<u8> {
    chunk(tag, format!("{value}\0").as_bytes())
}
fn first(bytes: &[u8]) -> mungefs::Chunk<'_> {
    mungefs::chunks(bytes).next().unwrap().unwrap()
}

fn flat_terrain() -> Vec<u8> {
    let mut info = Vec::new();
    for f in [1.0f32, 0.5, 0.0, 10.0] {
        info.extend(f.to_le_bytes());
    }
    info.extend(16u16.to_le_bytes());
    info.resize(28, 0);
    let mut body = text(b"NAME", "ground");
    body.extend(chunk(b"INFO", &info));
    body.extend(chunk(b"HGT8", &[255, 0, 255, 0]));
    body.extend(chunk(b"HEXP", &[0; 8]));
    chunk(b"tern", &body)
}

fn fixture() -> (Table, File, Vec<u8>) {
    let speed = mungefs::fnv1a_lowercase("MaxSpeed");
    let label = mungefs::fnv1a_lowercase("Label");
    let class = [
        text(b"TYPE", "unit"),
        text(b"BASE", "soldier"),
        chunk(b"PROP", &[speed.to_le_bytes().as_slice(), b"12\0"].concat()),
        chunk(
            b"PROP",
            &[label.to_le_bytes().as_slice(), b"Donor\0"].concat(),
        ),
    ]
    .concat();
    let bytes = wrap(&[chunk(b"entc", &class), flat_terrain()].concat());
    let table: Table = serde_json::from_value(json!({
        "schema":2,"reference":"ps2-v1","game":"bf1","platform":"pc",
        "presentation_hashes":[label],"files":[]
    }))
    .unwrap();
    let xform = [1, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4];
    let file: File = serde_json::from_value(json!({
        "file":"fixture.lvl","sha256":digest(&bytes),
        "classes":[{"scope":[],"name":"unit","ordinal":0,"kind":"entc","base":"soldier","properties":[[speed,"4"]]},
                   {"scope":[123],"name":"missing","ordinal":0,"kind":"wpnc","base":"weapon","properties":[]}],
        "world":[{"scope":[],"name":"world","ordinal":0,"terrain_name":"ground","sky_name":"sky","records":[
            {"kind":"inst","name":"post","class":"unit","transform":xform,"properties":[]}
        ]}],
        "primitives":[{"scope":[],"name":"body","ordinal":0,"records":[
            {"name":"box","parent":"root","mask":3,"mask_own":1,"kind":4,"dimensions":[2,3,4],"transform":xform}
        ]}],
        "terrain":[{"scope":[],"name":"ground","ordinal":0,"grid_size":16,"grid_unit":1,"height_scale":0.5,
            "height_samples":[[0,1,1]],"boundaries":[[[-2,-2],[2,-2],[2,2],[-2,2]]],
            "cutters":[{"sphere":[2,0,0,0],"planes":[[0,1,0,-1]]}]}],
        "sky":[{"scope":[],"name":"","ordinal":0,"nodes":[
            {"hash":mungefs::fnv1a_lowercase("SkyInfo"),"arguments":[],"children":[
                {"hash":mungefs::fnv1a_lowercase("FogRange"),"arguments":[10,50],"children":[]}
            ]}
        ]}]
    })).unwrap();
    (table, file, bytes)
}

#[test]
fn absolute_corrections_apply_together_before_all_readers() {
    let (table, file, bytes) = fixture();
    let rebuilt = rebuild::apply(&bytes, &table, &file).unwrap();
    let inner = mungefs::open(&rebuilt).unwrap();
    let chunks = mungefs::content_chunks(inner).unwrap();
    let c = chunks.iter().find(|c| &c.tag == b"entc").unwrap();
    let class = mungefs::entity::EntityClass::parse(c).unwrap();
    assert_eq!(
        class
            .props
            .iter()
            .find(|p| p.key_hash == mungefs::fnv1a_lowercase("MaxSpeed"))
            .unwrap()
            .value,
        "4"
    );
    assert_eq!(
        class
            .props
            .iter()
            .find(|p| p.key_hash == mungefs::fnv1a_lowercase("Label"))
            .unwrap()
            .value,
        "Donor"
    );
    assert!(chunks
        .iter()
        .any(|c| &c.tag == b"wpnc" && c.child_str(b"TYPE") == Some("missing")));
    let w =
        mungefs::world::World::parse(chunks.iter().find(|c| &c.tag == b"wrld").unwrap()).unwrap();
    assert_eq!(w.instances[0].transform.translation, [2.0, 3.0, 4.0]);
    let p = mungefs::coll::Primitives::parse(chunks.iter().find(|c| &c.tag == b"prim").unwrap())
        .unwrap();
    assert!(matches!(
        p.prims[0].kind,
        mungefs::coll::PrimKind::Box {
            half: [2.0, 3.0, 4.0]
        }
    ));
    let sky = mungefs::sky::Sky::parse(chunks.iter().find(|c| &c.tag == b"sky_").unwrap()).unwrap();
    assert_eq!(sky.info.fog_range, Some([10.0, 50.0]));
    let mut ground =
        mungefs::terrain::Terrain::parse(chunks.iter().find(|c| &c.tag == b"tern").unwrap())
            .unwrap();
    assert_eq!(ground.cutters[0].planes, vec![[0.0, 1.0, 0.0, -1.0]]);
    terrain::apply(&file.terrain[0], &mut ground).unwrap();
    assert_eq!(ground.raw[8 * 16 + 8], 2);
    assert_eq!(ground.raw.iter().filter(|&&v| v != 0).count(), 1);
}

#[test]
fn height_rejection_is_atomic_and_bounds_include_edges() {
    let (_, mut file, _) = fixture();
    let raw = flat_terrain();
    let mut ground = mungefs::terrain::Terrain::parse(&first(&raw)).unwrap();
    // The fixture's boundary ends at 2 with cells of 1: a sample at 3 is a corner of a
    // cell the boundary reaches, a sample at 4 is not.
    file.terrain[0].height_samples.push([4.0, 1.0, 1.0]);
    assert!(terrain::apply(&file.terrain[0], &mut ground)
        .unwrap_err()
        .to_string()
        .contains("outside playable bounds"));
    assert!(ground.raw.iter().all(|&v| v == 0));
    file.terrain[0].height_samples = vec![[2.0, 2.0, 1.0]];
    terrain::apply(&file.terrain[0], &mut ground).unwrap();
    assert_eq!(ground.raw[9 * 16 + 10], 2);
    file.terrain[0].height_samples = vec![[3.0, 1.0, 1.0]];
    terrain::apply(&file.terrain[0], &mut ground).unwrap();
    assert_eq!(ground.raw[8 * 16 + 11], 2);
}

#[test]
fn terrain_query_window_rejects_noncentered_sizes_without_mutating_heights() {
    let (_, mut file, _) = fixture();
    let raw = flat_terrain();
    let mut ground = mungefs::terrain::Terrain::parse(&first(&raw)).unwrap();
    for size in [0, 1, 15, 17] {
        file.terrain[0].grid_size = size;
        assert!(terrain::apply(&file.terrain[0], &mut ground).is_err());
        assert!(ground.raw.iter().all(|&v| v == 0));
    }
    file.terrain[0].grid_size = 8;
    file.terrain[0].height_samples.clear();
    file.terrain[0].boundaries.clear();
    terrain::apply(&file.terrain[0], &mut ground).unwrap();
    assert_eq!(ground.info.grid_size, 16);
    assert!(ground.raw.iter().all(|&v| v == 0));
}

#[test]
fn sparse_world_patch_keeps_donor_records_and_canonical_order() {
    let (table, mut file, bytes) = fixture();
    let donor = rebuild::apply(&bytes, &table, &file).unwrap();
    file.world[0].patch_records = true;
    file.world[0].record_order = vec![("target".into(), 0), ("donor".into(), 0)];
    file.world[0].records[0].name = "new_post".into();
    file.world[0].records[0].transform[9] = 99.0;
    let result = rebuild::apply(&donor, &table, &file).unwrap();
    let chunks = mungefs::content_chunks(mungefs::open(&result).unwrap()).unwrap();
    let world =
        mungefs::world::World::parse(chunks.iter().find(|c| &c.tag == b"wrld").unwrap()).unwrap();
    assert_eq!(
        world
            .instances
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>(),
        ["new_post", "post"]
    );
    assert_eq!(world.instances[0].transform.translation[0], 99.0);
    assert_eq!(world.instances[1].transform.translation[0], 2.0);
    file.world[0].record_order = vec![("donor".into(), 1)];
    assert!(rebuild::apply(&donor, &table, &file)
        .unwrap_err()
        .to_string()
        .contains("out of bounds"));
}

#[test]
fn unknown_donor_fingerprint_fails_closed_but_ps2_uses_no_table() {
    let pc = wrap(&chunk(b"tex_", &chunk(b"FMT_", &[])));
    let error = correction(Path::new("fixture.lvl"), &pc)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unknown pc gameplay source fingerprint"),
        "{error}"
    );
    let ps2 = wrap(&chunk(b"tex_", &chunk(b"INFO", &[0; 24])));
    assert!(correction(Path::new("fixture.lvl"), &ps2)
        .unwrap()
        .is_none());
    let path = std::env::temp_dir().join(format!(
        "openbf-ps2-pass-through-{}.lvl",
        std::process::id()
    ));
    std::fs::write(&path, &ps2).unwrap();
    assert_eq!(read(&path).unwrap(), ps2);
    let (unchanged, donor) = read_with_original(&path).unwrap();
    assert_eq!(unchanged, ps2);
    assert!(donor.is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn only_the_named_hero_pack_skips_the_source_gate() {
    let pc = wrap(&chunk(b"tex_", &chunk(b"FMT_", &[])));
    let pack = Path::new("named-hero-pack.lvl");
    assert!(correction(pack, &pc).is_err());
    exempt_addon(pack);
    assert!(correction(pack, &pc).unwrap().is_none());
    assert!(correction(Path::new("another-pack.lvl"), &pc).is_err());
}

#[test]
fn one_file_under_two_names_takes_the_row_of_its_own_path() {
    let (table, _, _) = fixture();
    let row = |file: &str| -> File {
        serde_json::from_value(json!({"file": file, "sha256": "same"})).unwrap()
    };
    let (eng, uk) = (row("common/eng/common.lvl"), row("common/uk_/common.lvl"));
    let both = || vec![(&table, &eng), (&table, &uk)];
    let picked = same_bytes_by_path(Path::new("/d/_LVL_XBOX/COMMON/UK_/COMMON.LVL"), both());
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0].1.file, "common/uk_/common.lvl");
    // A name that says nothing stays ambiguous and is refused by the caller.
    assert_eq!(same_bytes_by_path(Path::new("/d/x.lvl"), both()).len(), 2);
    // A longer name is not this name.
    assert_eq!(
        same_bytes_by_path(Path::new("/d/mycommon/eng/common.lvl"), both()).len(),
        2
    );
}

#[test]
fn generated_correction_schemas_load() {
    let loaded = tables().unwrap();
    assert_eq!(loaded.len(), 4);
    for table in loaded {
        assert!(!table.files.is_empty());
    }
}

/// Every instruction list in all four tables, over the real files where present: each rebuilds its table's SHA-256.
#[test]
fn every_program_rebuilds_its_reference_from_the_real_donor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let sources = [
        ("bf1", "xbox", root.join("extracted/xbox_bf1")),
        ("bf2", "xbox", root.join("extracted/xbox_bf2")),
        (
            "bf1",
            "pc",
            root.join("PC_swbf1_exe/Star Wars Battlefront (Classic 2004)/GameData/Data/_LVL_PC"),
        ),
        (
            "bf2",
            "pc",
            root.join("PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc"),
        ),
    ];
    let mut rebuilt = 0;
    for (game, platform, source) in sources {
        if !source.is_dir() {
            eprintln!("SKIP: no {game} {platform} source");
            continue;
        }
        let table = tables()
            .unwrap()
            .iter()
            .find(|t| t.game == game && t.platform == platform)
            .unwrap();
        for file in &table.files {
            if file.collision.is_empty() && file.resources.is_empty() {
                continue;
            }
            let path = crate::join_ci(&source, &file.file);
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(digest(&bytes), file.sha256, "{}", path.display());
            rebuild::apply(&bytes, table, file)
                .unwrap_or_else(|e| panic!("{game} {platform} {}: {e:#}", file.file));
            rebuilt += 1;
        }
    }
    eprintln!("{rebuilt} source files rebuilt");
}

/// Written outright, so nothing can fail before the fingerprint does.
#[test]
fn only_the_table_fingerprint_lets_a_collision_program_through() {
    let programs: program::Programs = serde_json::from_value(json!({
        "schema":2,"reference":"ps2-v1","game":"bf1","platform":"pc","skeleton":[],"route":[],"bank":[],
        "collision":[{"n":"node","b":[0.0,0.0,0.0,1.0,1.0,1.0],"v":[4,1.5,2.5,-0.0],"p":[3,0,0,0]}]
    }))
    .unwrap();
    let target = |sha256: &str| -> Collision {
        serde_json::from_value(
            json!({"scope":[],"name":"mesh","ordinal":0,"program":0,"sha256":sha256}),
        )
        .unwrap()
    };
    let error = program::collision(&programs, &target(&"0".repeat(64)), None).unwrap_err();
    assert!(error.to_string().contains("did not rebuild"), "{error}");
    let leaf = [&[3u8][..], &[0; 6], &[0; 6]].concat();
    let mut info = [1u32, 0, 1, 3]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<u8>>();
    info.extend(
        [0f32, 0.0, 0.0, 1.0, 1.0, 1.0]
            .iter()
            .flat_map(|v| v.to_le_bytes()),
    );
    let position = [1.5f32, 2.5, -0.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<u8>>();
    let expected = chunk(
        b"coll",
        &[
            chunk(b"NAME", b"mesh\0"),
            chunk(b"NODE", b"node\0"),
            chunk(b"INFO", &info),
            chunk(b"POSI", &position),
            chunk(b"TREE", &chunk(b"LEAF", &leaf)),
        ]
        .concat(),
    );
    assert_eq!(
        program::collision(&programs, &target(&digest(&expected)), None).unwrap(),
        expected
    );
}

#[test]
fn a_program_that_rebuilds_something_else_is_refused() {
    let table = tables()
        .unwrap()
        .iter()
        .find(|t| t.game == "bf1" && t.platform == "xbox")
        .unwrap();
    let file = table
        .files
        .iter()
        .find(|f| !f.collision.is_empty())
        .unwrap();
    let mut target = file.collision[0].clone();
    target.sha256 = "0".repeat(64);
    let programs = table.programs.as_ref().unwrap();
    let error = program::collision(programs, &target, None)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("outside the donor") || error.contains("did not rebuild"),
        "{error}"
    );
}

#[test]
fn model_bounds_keep_vertex_dequantization_and_optional_sphere_semantics() {
    let (table, mut file, _) = fixture();
    file.classes.clear();
    file.world.clear();
    file.primitives.clear();
    file.terrain.clear();
    file.sky.clear();
    file.models = vec![
        serde_json::from_value(json!({"scope":[],"name":"station","ordinal":0,
        "sphere":null,"vis_box":[[-1,-2,-3],[1,2,3]]}))
        .unwrap(),
    ];
    let mut info = vec![0u8; 68];
    let original_vertex_box = [10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    info[12..36].copy_from_slice(&original_vertex_box);
    let original = wrap(&chunk(
        b"modl",
        &[
            text(b"NAME", "station"),
            chunk(b"INFO", &info),
            chunk(b"SPHR", &[0; 16]),
        ]
        .concat(),
    ));
    let result = rebuild::apply(&original, &table, &file).unwrap();
    let chunks = mungefs::content_chunks(mungefs::open(&result).unwrap()).unwrap();
    let model = mungefs::model::Model::parse(&chunks[0]).unwrap();
    assert_eq!(model.vertex_box.min, [10.0, 20.0, 30.0]);
    assert_eq!(model.vertex_box.max, [40.0, 50.0, 60.0]);
    assert_eq!(model.vis_box.min, [-1.0, -2.0, -3.0]);
    assert_eq!(model.vis_box.max, [1.0, 2.0, 3.0]);
    assert_eq!(model.sphere, None);
    assert_eq!(
        crate::glb::model_sphere(&model),
        [0.0, 0.0, 0.0, 56.0f32.sqrt() * 0.5]
    );
    file.models[0].sphere = Some([1.0, 2.0, 3.0, 9.0]);
    let result = rebuild::apply(&original, &table, &file).unwrap();
    let chunks = mungefs::content_chunks(mungefs::open(&result).unwrap()).unwrap();
    let model = mungefs::model::Model::parse(&chunks[0]).unwrap();
    assert_eq!(model.sphere, Some([1.0, 2.0, 3.0, 9.0]));
}

#[test]
fn public_kaadu_program_restores_exact_reference_root_motion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let reference = root.join("extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/GUN.LVL");
    let donor = root.join(
        "PC_swbf1_exe/Star Wars Battlefront (Classic 2004)/GameData/Data/_LVL_PC/SIDE/gun.lvl",
    );
    if !reference.is_file() || !donor.is_file() {
        return;
    }
    fn root_motion(bytes: &[u8]) -> Vec<f32> {
        let inner = mungefs::open(bytes).unwrap();
        let banks = mungefs::anim::find_banks(inner).unwrap();
        let bank = banks
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case("rep_walk_kaadu"))
            .unwrap();
        let clips = crate::walker::clips_of(bank, &[("walk_rightfoot_leftfoot", true)]);
        clips[0].as_ref().unwrap().root_z.clone()
    }
    let target = root_motion(&std::fs::read(&reference).unwrap());
    let original = root_motion(&std::fs::read(&donor).unwrap());
    assert_ne!(
        original, target,
        "the supplied donor must exercise the shifted root-motion defect"
    );
    assert_eq!(root_motion(&read(&donor).unwrap()), target);
}
