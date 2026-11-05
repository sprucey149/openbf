//! Tier-1 (data-gated): every baked map's vehicle spawn pads, built for each
//! of its missions as a battle builds them (card VPAD, analysis/re-vehicles.md
//! §1 census, §7): the conquest layer merged, the mission's posts applied,
//! the side + map classes loaded (and a mission's extra side, BF1 NAB1's
//! gun). Each pad resolves its class per team; the only class name no loaded
//! class answers is the one the census found the PS2 would spawn nothing
//! for. Every class the pads name has a model the viewer can draw, textured
//! (card VDRAW).

use game::classdb::ClassDb;
use game::layout::MapLayout;
use game::mission::MissionDef;
use game::vehicle_pads::{build, PadBuild};
use manifest::Manifest;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn missions_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
}

/// The manifest with its conquest layer merged, as `Scene::apply_mode_layer`
/// merges it.
fn conquest_manifest(dir: &std::path::Path) -> Option<Manifest> {
    let mut m: Manifest =
        ron::from_str(&std::fs::read_to_string(dir.join("manifest.ron")).ok()?).ok()?;
    if let Some(i) = game::assets::mode_layer_index(&m, "con") {
        let l = m.layers.remove(i);
        m.regions.extend(l.regions);
        m.instances.extend(l.instances);
    }
    Some(m)
}

/// `(pad build, the mission's layout)` for one map + mission.
fn build_for(dir: &std::path::Path, m: &Manifest, mission: &MissionDef) -> (PadBuild, MapLayout) {
    let factions = mission.factions();
    let game_key = m.source_game.to_ascii_lowercase();
    let mut db = ClassDb::default();
    db.load_map(dir).expect("map classes");
    let f: Vec<&str> = factions.iter().map(String::as_str).collect();
    let classes = game::assets::baked_root().join("classes");
    let map = dir.file_name().unwrap().to_string_lossy().into_owned();
    let extra = game::vehicle_pads::mission_side_files(&game_key, &map)
        .iter()
        .copied();
    for file in game::roster::class_files(&game_key, &f)
        .into_iter()
        .chain(extra)
    {
        let p = classes.join(file);
        if p.exists() {
            db.load(&p).expect("side classes");
        }
    }
    let mut layout = MapLayout::from_manifest(m);
    layout.apply_mission(mission);
    let names: Vec<String> = layout.cps.iter().map(|c| c.name.clone()).collect();
    let b = build(m, &db, &factions, |zone| {
        names.iter().position(|n| n.eq_ignore_ascii_case(zone))
    });
    (b, layout)
}

#[test]
fn every_pad_resolves_a_class_per_team_or_is_a_known_absence() {
    let root = game::assets::baked_root();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        eprintln!("SKIP: no bake at {}", root.display());
        return;
    };
    let mut dirs: Vec<PathBuf> = dirs.flatten().map(|e| e.path()).collect();
    dirs.sort();
    let (mut maps, mut pads, mut runs, mut with_cp, mut in_control) = (0, 0, 0, 0, 0);
    let (mut spawning, mut both) = (0, 0);
    let mut absent = BTreeSet::new();
    for dir in dirs {
        let Some(m) = conquest_manifest(&dir) else {
            continue;
        };
        let key = dir.file_name().unwrap().to_string_lossy().into_owned();
        let n = m
            .instances
            .iter()
            .filter(|i| i.class == game::vehicle_pads::PAD_CLASS)
            .count();
        if n == 0 {
            continue;
        }
        maps += 1;
        pads += n;
        for suffix in ["_con.ron", "_con_cw.ron"] {
            let Ok(mission) = MissionDef::load(&missions_dir().join(format!("{key}{suffix}")))
            else {
                continue;
            };
            runs += 1;
            let (b, layout) = build_for(&dir, &m, &mission);
            assert_eq!(b.pads.len(), n);
            for p in &b.pads {
                let classes = p.classes.iter().flatten().count();
                spawning += usize::from(classes > 0);
                both += usize::from(classes == 2);
                for c in p.classes.iter().flatten() {
                    assert!(c.spec.max_health > 0.0, "{key} {}: MaxHealth", p.name);
                    assert!(core::vehicle::is_ring(&c.spec.seats), "{key} {}", p.name);
                    assert!((c.spec.class as usize) < b.classes.len());
                }
                if let Some(ci) = p.cp {
                    with_cp += 1;
                    let control = layout.cps[ci].control.as_ref();
                    in_control += usize::from(control.is_some_and(|r| r.contains(p.pos)));
                }
            }
            for (_, class) in &b.absent {
                absent.insert((key.clone(), class.clone()));
            }
        }
    }
    if maps == 0 {
        eprintln!("SKIP: no baked map has a pad");
        return;
    }
    eprintln!(
        "{maps} maps, {pads} pads, {runs} missions: {spawning} pad-missions spawn \
         ({both} for both teams); {with_cp} follow a post, {in_control} of them \
         stand inside its ControlRegion; absent {absent:?}"
    );
    // A bake from before card KAAD has no BF1 SIDE/GUN classes, and NAB1's
    // kaadu stays unresolved there.
    let gun = game::assets::baked_root().join("classes/bf1_gun.ron");
    let expect: BTreeSet<(String, String)> = [
        ("bf1_nab1", "gun_walk_kaadu"),
        ("bf2_yav1", "re_hover_barcspeeder"),
    ]
    .iter()
    .filter(|(map, _)| *map != "bf1_nab1" || !gun.exists())
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(absent, expect);
}

/// Every class a pad can spawn has a `GeometryName` whose LOD0 the viewer
/// finds (the map's bake, then the sides — `game::viewer::find_model`), and
/// every texture that model's segments name is found too, across every side
/// of the game where the model's own side lacks it (re-vehicles §10 item 3).
#[test]
fn every_pad_class_has_a_drawable_model() {
    use game::viewer::{find_model, find_texture, ModelSource, Untextured, VehicleSides};
    let root = game::assets::baked_root();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        eprintln!("SKIP: no bake at {}", root.display());
        return;
    };
    let mut dirs: Vec<PathBuf> = dirs.flatten().map(|e| e.path()).collect();
    dirs.sort();
    let mut checked = BTreeSet::new();
    let (mut from_map, mut cross_side) = (0, BTreeSet::new());
    for dir in dirs {
        let Some(m) = conquest_manifest(&dir) else {
            continue;
        };
        let key = dir.file_name().unwrap().to_string_lossy().into_owned();
        let sides = VehicleSides::load(&root, &m.source_game.to_ascii_lowercase());
        for suffix in ["_con.ron", "_con_cw.ron"] {
            let Ok(mission) = MissionDef::load(&missions_dir().join(format!("{key}{suffix}")))
            else {
                continue;
            };
            let (b, _) = build_for(&dir, &m, &mission);
            assert_eq!(b.geometry.len(), b.classes.len());
            // A building turret's class (card VTURB) has no viewer geometry:
            // the world draws the building.
            let turret = |i: usize| b.turrets.iter().any(|t| t.spec.class as usize == i);
            for (i, (class, g)) in b.classes.iter().zip(&b.geometry).enumerate() {
                if turret(i) {
                    assert!(g.is_none(), "{key} {class}: a turret drawn twice");
                    continue;
                }
                let g = g
                    .as_deref()
                    .unwrap_or_else(|| panic!("{key} {class}: no GeometryName"));
                let src = find_model(&m, &sides, g)
                    .unwrap_or_else(|| panic!("{key} {class}: no model for {g}"));
                if !checked.insert((key.clone(), g.to_ascii_lowercase())) {
                    continue;
                }
                let (path, home) = match &src {
                    ModelSource::Map(k) => {
                        from_map += 1;
                        (dir.join("models").join(format!("{k}.glb")), None)
                    }
                    ModelSource::Side(i, stem) => (
                        sides.dir(*i).join("models").join(format!("{stem}.glb")),
                        Some(*i),
                    ),
                };
                let cpu = game::assets::load_glb(&path).expect("vehicle glb");
                for (_, _, mat, _, _) in &cpu.submeshes {
                    if game::viewer::route_untextured(Some(mat)) != Untextured::LookupFailed {
                        continue;
                    }
                    let t = find_texture(&m, &sides, home, mat)
                        .unwrap_or_else(|| panic!("{key} {class} ({g}): texture {mat}"));
                    if matches!(t, game::viewer::TexSource::Side(i) if Some(i) != home) {
                        cross_side.insert(mat.to_ascii_lowercase());
                    }
                }
            }
        }
    }
    if checked.is_empty() {
        eprintln!("SKIP: no baked map has a pad class");
        return;
    }
    eprintln!(
        "{} (map, geometry) pairs drawable, {from_map} from the map's own bake; \
         textures from another side: {cross_side:?}",
        checked.len()
    );
}

/// TAT1's GCW battle at its first tick: the posts owned as the mission owns
/// them, every pad with a class for its post's team spawns at once (armed at
/// 0, §7) — the hover classes, and the walkers and flyers that stand idle
/// until their integrators land.
#[test]
fn tat1_spawns_its_vehicles_on_the_first_tick() {
    let dir = game::assets::baked_root().join("tat1");
    let (Some(m), Ok(mission)) = (
        conquest_manifest(&dir),
        MissionDef::load(&missions_dir().join("tat1_con.ron")),
    ) else {
        eprintln!("SKIP: no TAT1 bake");
        return;
    };
    let (b, layout) = build_for(&dir, &m, &mission);
    let mut sim = core::sim::Sim::empty(1);
    for cp in &layout.cps {
        let (capture_time, neutralize_time) = core::cp::CpDef::class_times(cp.major);
        sim.add_cp(
            core::cp::CpDef {
                name: cp.name.clone(),
                pos: cp.pos,
                capture: cp
                    .capture
                    .unwrap_or_else(|| core::region::Region::sphere(cp.pos, 8.0)),
                control: cp.control,
                value_bleed: 0,
                value_atk: [0, 0],
                value_def: [0, 0],
                spawn_nodes: Vec::new(),
                capture_time,
                neutralize_time,
            },
            match cp.team {
                1 => Some(0),
                2 => Some(1),
                _ => None,
            },
        );
    }
    let expect: usize = b
        .pads
        .iter()
        .filter(|p| {
            let team = p.cp.map_or(p.team, |ci| sim.cps[ci].owner);
            p.class_for(team).is_some()
        })
        .count();
    for p in &b.pads {
        sim.add_vehicle_pad(p.clone());
    }
    sim.step(&[]);
    let mut kinds: Vec<String> = sim
        .alive_vehicles()
        .map(|(_, v)| format!("{} {:?}", b.classes[v.class as usize], v.kind))
        .collect();
    kinds.sort();
    eprintln!(
        "{} of {} pads spawned: {kinds:#?}",
        kinds.len(),
        b.pads.len()
    );
    assert!(expect > 0);
    assert_eq!(kinds.len(), expect);
    assert!(
        sim.alive_vehicles().all(|(_, v)| v.team.is_none()),
        "empty = neutral"
    );
    assert!(kinds.iter().any(|k| k.ends_with("Hover")));
}

/// Card VHULL: TAT1's pad vehicles are solid. Each class's hull comes from
/// its bake (`vehicle_pads::hull_parts`: the skiff from the map's own, the
/// rest from the side bakes) and joins the collision world where the pad
/// spawns it. A soldier walking broadside at the skiff from 8 m stops at
/// it; with no hull he walks through, as before; a bolt's ray from where he
/// started meets the skiff's hull.
#[test]
fn tat1_pad_vehicles_are_solid() {
    let dir = game::assets::baked_root().join("tat1");
    let (Some(m), Ok(mission)) = (
        conquest_manifest(&dir),
        MissionDef::load(&missions_dir().join("tat1_con.ron")),
    ) else {
        eprintln!("SKIP: no TAT1 bake");
        return;
    };
    let (b, _) = build_for(&dir, &m, &mission);
    let game_key = m.source_game.to_ascii_lowercase();
    let parts = game::vehicle_pads::hull_parts(&m, &dir, &game_key, &b.classes, |_| false);
    let report: Vec<String> = b
        .classes
        .iter()
        .zip(&parts)
        .map(|(c, p)| format!("{c}: {}", p.as_ref().map_or(0, |p| p.len())))
        .collect();
    eprintln!("hull bodies per class: {report:?}");
    let skiff = b
        .classes
        .iter()
        .position(|c| c.contains("skiff"))
        .expect("TAT1 has the skiff");
    assert!(parts[skiff].as_ref().is_some_and(|p| !p.is_empty()));

    let run = |solid: bool| {
        let mut sim = core::sim::Sim::empty(1);
        sim.heightfield = Some(core::heightfield::Heightfield {
            size: 2,
            unit: 8000.0,
            offset_x: 4000.0,
            offset_z: 4000.0,
            heights: vec![0.0; 4],
        });
        if solid {
            sim.hull_parts = parts.clone();
        }
        let pad = b
            .pads
            .iter()
            .find(|p| {
                p.classes
                    .iter()
                    .flatten()
                    .any(|c| c.spec.class as usize == skiff)
            })
            .expect("a skiff pad")
            .clone();
        let mut pad = pad;
        pad.cp = None;
        pad.team = pad
            .classes
            .iter()
            .position(|c| c.as_ref().is_some_and(|c| c.spec.class as usize == skiff))
            .map(|t| t as u8);
        sim.add_vehicle_pad(pad);
        sim.step(&[]);
        let (vh, v) = sim.alive_vehicles().next().expect("the skiff spawned");
        // The pad stands the ORIGIN on the ground (VPAD, PROVISIONAL) and the
        // skiff's hull hangs 1.4 m under it; rest its lowest body on the
        // ground instead, as a settled hover floats (card VREST's question).
        let lowest = sim
            .collision
            .hull(vh)
            .iter()
            .filter(|b| b.layers.contains(core::collision::Layers::ALL))
            .map(|b| b.aabb.min.y)
            .fold(0.0f32, f32::min);
        let mid = sim
            .collision
            .hull(vh)
            .first()
            .map_or(v.pos, |b| (b.aabb.min + b.aabb.max) * 0.5);
        sim.vehicle_mut(vh).unwrap().pos.y -= lowest;
        sim.sync_hulls();
        let at = sim.vehicle(vh).unwrap().pos;
        for body in sim.collision.hull(vh) {
            eprintln!(
                "  body {:?} {:?}..{:?}",
                body.layers, body.aabb.min, body.aabb.max
            );
        }
        // Broadside: from 8 m out on +X, across the hull's middle, facing −X.
        let start = glam::Vec3::new(at.x + 8.0, 0.0, mid.z);
        let p = sim
            .spawn(&core::unit::UnitSpec {
                yaw: std::f32::consts::FRAC_PI_2,
                ..core::unit::UnitSpec::player(0, start)
            })
            .expect("player");
        let hit = sim
            .collision
            .ray_with(
                start + glam::Vec3::Y,
                glam::Vec3::NEG_X,
                20.0,
                core::collision::Layers::ORDNANCE,
                core::collision::HullFilter::All,
            )
            .and_then(|h| h.owner);
        let fwd = core::sim::PadState {
            ly: 32767,
            ..Default::default()
        };
        for _ in 0..150 {
            sim.step(&[fwd]);
        }
        let end = sim.unit(p).unwrap().soldier.pos;
        eprintln!("solid {solid}: skiff at {at}, soldier {start} -> {end}");
        (at, end, hit == Some(vh))
    };
    let (at, end, hit) = run(true);
    assert!(hit, "the bolt ray meets the skiff");
    assert!(
        end.x > at.x + 1.0,
        "stopped short of its middle: {end} vs {at}"
    );
    let (at, end, hit) = run(false);
    assert!(!hit);
    assert!(end.x < at.x - 5.0, "no hull: walked through: {end} vs {at}");
}

/// Card VTUR2: every BF2 `tur_*` instance a conquest world places (classes
/// from SIDE/TUR.LVL) spawns as a turret, and its class row in the map's
/// manifest names geometry the map bake wrote — except a buildable one
/// (`manifest::BUILD_ZONE_PROP`), which starts as a registered unbuilt pad.
/// Counts per map are printed. A bake from
/// before VTUR2 (no `classes/bf2_tur.ron`) skips.
#[test]
fn every_placed_tur_class_spawns_a_turret_with_its_model() {
    let root = game::assets::baked_root();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        eprintln!("SKIP: no bake at {}", root.display());
        return;
    };
    if !root.join("classes/bf2_tur.ron").exists() {
        eprintln!("SKIP: no classes/bf2_tur.ron (a bake from before VTUR2)");
        return;
    }
    let mut dirs: Vec<PathBuf> = dirs.flatten().map(|e| e.path()).collect();
    dirs.sort();
    let mut census = Vec::new();
    for dir in dirs {
        let Some(m) = conquest_manifest(&dir) else {
            continue;
        };
        let key = dir.file_name().unwrap().to_string_lossy().into_owned();
        let tur = |c: &str| c.to_ascii_lowercase().starts_with("tur_");
        let zone = |c: &str| {
            m.classes
                .iter()
                .find(|k| k.name.eq_ignore_ascii_case(c))
                .and_then(|k| k.build_zone.as_deref())
        };
        let unbuilt = m
            .instances
            .iter()
            .filter(|i| tur(&i.class) && zone(&i.class).is_some())
            .inspect(|i| assert_eq!(zone(&i.class), Some("com_prop_buildzone"), "{key}"))
            .count();
        let placed = m.instances.iter().filter(|i| tur(&i.class)).count();
        let Ok(mission) = MissionDef::load(&missions_dir().join(format!("{key}_con.ron"))) else {
            continue;
        };
        let (b, _) = build_for(&dir, &m, &mission);
        let spawned: Vec<&str> = b
            .turrets
            .iter()
            .map(|t| b.classes[t.spec.class as usize].as_str())
            .filter(|c| tur(c))
            .collect();
        assert_eq!(
            spawned.len(),
            placed,
            "{key}: tur_ instances vs turret definitions"
        );
        assert_eq!(
            b.turrets
                .iter()
                .filter(|t| t.unbuilt && tur(&b.classes[t.spec.class as usize]))
                .count(),
            unbuilt,
            "{key}: initial pad definitions"
        );
        for c in spawned.iter().collect::<BTreeSet<_>>() {
            let g = m
                .classes
                .iter()
                .find(|k| k.name.eq_ignore_ascii_case(c))
                .and_then(|k| k.geometry.clone())
                .unwrap_or_else(|| panic!("{key} {c}: no manifest row with geometry"));
            assert!(
                dir.join("models").join(format!("{g}.glb")).is_file(),
                "{key} {c}: no model {g}"
            );
        }
        if placed > 0 {
            census.push(format!("{key} {placed} total ({unbuilt} unbuilt)"));
        }
    }
    eprintln!("tur_ turrets per conquest map: {}", census.join(", "));
    assert!(!census.is_empty(), "no BF2 map places a TUR turret");
}

/// Card VREST (`analysis/re-vehicle-rest.md` §2): every flyer class the side
/// bakes hold reads the LandedHeight the PS2's class setter leaves — the
/// authored number, else minus the model's vis-box min y. The values marked
/// RAM are the PS2's own class field in a live battle; the rest are the
/// setter replayed over the disc (`lead_tmp/vrest/lhmodel.py --all`).
/// `OPENBF_ASSETS` names another bake; a bake from before VREST (no
/// `vis_min_y`) is skipped.
#[test]
fn every_flyer_class_reads_the_ps2s_landed_height() {
    #[rustfmt::skip]
    const TABLE: [(&str, &str, f32); 39] = [
        ("bf1", "all_fly_kwing", 2.1885),
        ("bf1", "all_fly_snowspeeder", 0.5),            // RAM, authored
        ("bf1", "all_fly_xwing", 1.778_591),            // RAM
        ("bf1", "all_fly_ywing", 1.328_592),            // RAM
        ("bf1", "cis_fly_droidfighter", 0.004_416),     // RAM
        ("bf1", "cis_fly_fedlander_dome", -0.8003),
        ("bf1", "cis_fly_geofighter", 0.217_042),       // RAM
        ("bf1", "cis_fly_maf", 0.0),                    // RAM
        ("bf1", "imp_fly_tiebomber", -0.011_526),       // RAM
        ("bf1", "imp_fly_tiefighter", 0.006_017),       // RAM
        ("bf1", "imp_fly_trooptrans", 2.2516),
        ("bf1", "rep_fly_gunship", 4.150_177),          // RAM
        ("bf1", "rep_fly_jedifighter", 1.1),            // RAM, authored
        ("bf1", "rep_fly_vtrans", -2.2358),
        ("bf2", "all_fly_awing", 1.85),
        ("bf2", "all_fly_gunship_sc", 3.3434),
        ("bf2", "all_fly_snowspeeder", 0.5),            // RAM, authored
        ("bf2", "all_fly_xwing_sc", 1.7842),
        ("bf2", "all_fly_ywing_sc", 1.3286),
        ("bf2", "cis_fly_droidfighter_sc", 1.056_455),  // RAM
        ("bf2", "cis_fly_droidgunship", 3.538),
        ("bf2", "cis_fly_greviousfighter", 2.191_925),  // RAM
        ("bf2", "cis_fly_maf", 0.0),
        ("bf2", "cis_fly_tridroidfighter", 3.083_473),  // RAM
        ("bf2", "imp_fly_ai_trooptrans", 2.2516),
        ("bf2", "imp_fly_tiebomber_sc", 1.9501),
        ("bf2", "imp_fly_tiefighter_sc", 3.24),
        ("bf2", "imp_fly_tieinterceptor", 1.7143),
        ("bf2", "imp_fly_trooptrans", 1.6527),
        ("bf2", "rep_fly_anakinstarfighter_sc", 0.7),   // RAM, authored
        ("bf2", "rep_fly_arc170fighter_sc", 1.1),       // RAM, authored
        ("bf2", "kam_fly_ride_gunship", 3.3434),
        ("bf2", "rep_fly_gunship", 3.3434),
        ("bf2", "rep_fly_gunship_sc", 3.3434),
        ("bf2", "rep_fly_ride_gunship", 3.3434),
        ("bf2", "rep_fly_jedifighter_sc", 1.1),
        ("bf2", "rep_fly_vwing", 0.9),                  // RAM, authored
        ("bf2", "uta_fly_ride_gunship", 2.6592),
        ("bf2", "uta_fly_ride_gunshipmyg", 2.6592),
    ];
    let root =
        std::env::var_os("OPENBF_ASSETS").map_or_else(game::assets::baked_root, PathBuf::from);
    let mut checked = 0;
    for game_key in ["bf1", "bf2"] {
        // `vis_floors` reads the sides beside a map's directory.
        let floors = game::vehicle_pads::vis_floors(&root.join("map"), game_key);
        if floors.is_empty() {
            eprintln!("SKIP {game_key}: no vis_min_y in the side bakes (a bake from before VREST)");
            continue;
        }
        let mut db = ClassDb::default();
        let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("classes"))
            .expect("classes")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(game_key))
            })
            .collect();
        files.sort();
        for f in &files {
            db.load(f).expect("classes");
        }
        // Every flyer class the sides bake is in the table.
        let mut baked: Vec<&String> = floors
            .keys()
            .filter(|c| {
                let name = db.resolve(c).unwrap_or(c);
                game::vehicle_class::landed_height(&db, name, |k| floors.get(k).copied()).is_some()
            })
            .collect();
        baked.sort();
        for c in &baked {
            assert!(
                TABLE.iter().any(|(g, t, _)| *g == game_key && t == c),
                "{game_key} {c}: a baked flyer class the table lacks"
            );
        }
        for (_, class, want) in TABLE.iter().filter(|(g, ..)| *g == game_key) {
            if !floors.contains_key(*class) {
                continue;
            }
            // The disc spells one class in mixed case (`cis_fly_fedlander_DOME`).
            let name = db.resolve(class).unwrap_or(class);
            let got = game::vehicle_class::landed_height(&db, name, |k| floors.get(k).copied())
                .unwrap_or_else(|| panic!("{game_key} {class}: not a flyer"));
            assert!(
                (got - want).abs() < 6e-5,
                "{game_key} {class}: LandedHeight {got}, the PS2's {want}"
            );
            checked += 1;
        }
        // A hover has none.
        assert_eq!(
            game::vehicle_class::landed_height(&db, "imp_hover_speederbike", |k| floors
                .get(k)
                .copied()),
            None
        );
        println!("{game_key}: {} baked flyer classes", baked.len());
    }
    println!("LandedHeight: {checked} classes equal the table");
}

/// Card PWST (`analysis/re-powerup-station.md` §2.2, measured in RAM on NAB1
/// and HOT1): a conquest battle's `vehicle_pads::install` places every
/// health, ammo and vehicle droid of the six showcase maps' loaded layers,
/// each with its instance `Radius` (5.0; the vehicle droids 15.0), and the
/// classes' numbers are the discs'.
#[test]
fn the_showcase_maps_place_their_droids() {
    let want = [
        ("bf1_nab1", "bf1_nab1_con", [5, 5, 5]),
        ("tat1", "tat1_con", [4, 5, 2]),
        ("bf1_yav2", "bf1_yav2_con", [9, 9, 0]),
        ("bf2_uta1", "bf2_uta1_con", [7, 10, 0]),
        ("bf2_myg1", "bf2_myg1_con", [5, 7, 0]),
        ("bf2_hot1", "bf2_hot1_con", [9, 9, 0]),
    ];
    let classes = game::assets::baked_root().join("classes");
    let mut ran = 0;
    for (map, mission, [health, ammo, vehicle]) in want {
        let dir = game::assets::baked_root().join(map);
        let (Some(m), Ok(mission)) = (
            conquest_manifest(&dir),
            MissionDef::load(&missions_dir().join(format!("{mission}.ron"))),
        ) else {
            eprintln!("SKIP: no {map} bake");
            continue;
        };
        let mut sim = core::sim::Sim::empty(1);
        game::vehicle_pads::install(&mut sim, &m, &dir, &classes, &mission.factions())
            .expect("install");
        let count = |name: &str, r: f32| {
            sim.stations
                .iter()
                .filter(|s| {
                    let c = &sim.station_classes[usize::from(s.class)];
                    c.name == name && s.radius == r
                })
                .count()
        };
        let got = [
            count("com_item_healthrecharge", 5.0),
            count("com_item_weaponrecharge", 5.0),
            count("com_item_vehiclerecharge", 15.0),
        ];
        eprintln!(
            "{map}: {} stations, health / ammo / vehicle {got:?}",
            sim.stations.len()
        );
        assert_eq!(got, [health, ammo, vehicle], "{map}");
        assert_eq!(sim.stations.len(), health + ammo + vehicle, "{map}");
        let bf2 = m.source_game.eq_ignore_ascii_case("bf2");
        for c in &sim.station_classes {
            let (delay, hp, clips, veh) = match c.name.as_str() {
                "com_item_healthrecharge" => (1.0, if bf2 { 25.0 } else { 15.0 }, 0, 0.0),
                "com_item_weaponrecharge" => (2.0, 0.0, 1, 0.0),
                "com_item_vehiclerecharge" => (1.0, 0.0, 0, 400.0),
                other => panic!("{map}: unexpected station class {other}"),
            };
            assert_eq!(c.delay, delay, "{map} {}", c.name);
            assert_eq!(c.health, [hp, 0.0, hp, veh, 0.0], "{map} {}", c.name);
            let vclip = u32::from(veh > 0.0);
            assert_eq!(c.ammo, [clips, 0, clips, vclip, 0], "{map} {}", c.name);
        }
        ran += 1;
    }
    eprintln!("{ran} of 6 maps checked");
}

/// Issue #41: a vehicle's critical prims (its class's `HitLocation`, the
/// PS2's x4 on 15 BF2 classes) reach its hull with that factor and in the
/// ORDNANCE subset - a bolt can strike them. On BF2 Hoth (the AT-AT's neck,
/// the AT-ST's hips) and Geonosis (the AT-TE, the spider walker), every pad
/// class that authors one has a part carrying it.
#[test]
fn vehicle_critical_prims_are_on_the_hull() {
    use core::collision::Layers;
    let classes = game::assets::baked_root().join("classes");
    let mut checked = 0;
    for (map, mission) in [("bf2_hot1", "bf2_hot1_con"), ("bf2_geo1", "bf2_geo1_con")] {
        let dir = game::assets::baked_root().join(map);
        let (Some(m), Ok(mission)) = (
            conquest_manifest(&dir),
            MissionDef::load(&missions_dir().join(format!("{mission}.ron"))),
        ) else {
            eprintln!("SKIP: no {map} bake");
            continue;
        };
        let (b, _) = build_for(&dir, &m, &mission);
        let mut sim = core::sim::Sim::empty(1);
        game::vehicle_pads::install(&mut sim, &m, &dir, &classes, &mission.factions())
            .expect("install");
        let mut db = ClassDb::default();
        for f in game::roster::class_files(
            &m.source_game.to_ascii_lowercase(),
            &["all", "imp", "rep", "cis"],
        ) {
            let p = classes.join(f);
            if p.exists() {
                db.load(&p).unwrap();
            }
        }
        for (i, c) in b.classes.iter().enumerate() {
            let want = game::assets::hit_locations(&db, c);
            let Some((_, mult)) = want.first() else {
                continue;
            };
            let parts = sim.hull_parts[i].as_ref().expect("a hull");
            assert!(
                parts
                    .iter()
                    .any(|p| p.hit_mult == *mult && p.layers.contains(Layers::ORDNANCE)),
                "{map} {c}: no part at x{mult} in the ordnance subset ({want:?})"
            );
            checked += 1;
        }
    }
    eprintln!("classes with a critical prim checked: {checked}");
}
