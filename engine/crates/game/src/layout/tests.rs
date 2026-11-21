use super::*;
use glam::Vec2;

/// Five posts (`cp1`..`cp5`), each with its own capture sphere, plus one
/// `sarlac01` region for the death-region path.
fn manifest() -> Manifest {
    let mut instances = String::new();
    for i in 1..=5 {
        instances.push_str(&format!(
            r#"(class: "cp_c", name: "CP{i}",
                rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: ({i},0,0),
                props: [(2734521612, "1"), (3334562297, "cp{i}capture")]),"#
        ));
    }
    let regions: String = (1..=5)
        .map(|i| {
            format!(
                r#"(shape: "sphere", name: "cp{i}capture",
                    rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: ({i},0,0),
                    size: (0.1,4,0.1), props: []),"#
            )
        })
        .collect();
    ron::from_str(&format!(
        r#"(
            map: "t", source_game: "bf2",
            terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: [],
            classes: [(name: "cp_c", base: "commandpost", geometry: None)],
            geometries: [],
            regions: [{regions}
                (shape: "sphere", name: "sarlac01",
                 rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,-5,0),
                 size: (0.1,9,0.1), props: []),
            ],
            instances: [{instances}],
        )"#
    ))
    .expect("manifest parses")
}

fn mission(src: &str) -> MissionDef {
    ron::from_str(src).expect("mission parses")
}

/// PROP 0xA2FD7D0C = Team, 0xC6C161F9 = CaptureRegion — the two literals
/// the synthetic manifest above spells in decimal.
#[test]
fn synthetic_manifest_uses_the_real_prop_hashes() {
    assert_eq!(PROP_TEAM, 2_734_521_612);
    assert_eq!(PROP_CAPTURE_REGION, 3_334_562_297);
}

#[test]
fn post_class_names_apply_visibility_and_capture_rules_only_in_campaign() {
    let mut m = manifest();
    m.instances[0].class = "com_bldg_inv_untakeable_controlzone".into();
    let instant_action = MapLayout::from_battle_manifest(&m, false);
    assert!(instant_action.cps[0].visible);
    assert!(instant_action.cps[0].capturable);

    let campaign = MapLayout::from_mission_manifest(&m, false, true);
    assert!(!campaign.cps[0].visible);
    assert!(!campaign.cps[0].capturable);
}

#[test]
fn ally_spawn_data_is_separate_from_the_post_spawn_path() {
    let mut m = manifest();
    m.instances[0].props.extend([
        (PROP_SPAWN_PATH, "owned".into()),
        (PROP_ALLY_PATH, "allies".into()),
        (PROP_ALLY_COUNT, "65536".into()),
    ]);
    let layout = MapLayout::from_manifest(&m);
    let post = &layout.cps[0];
    assert_eq!(post.spawn_path.as_deref(), Some("owned"));
    assert_eq!(post.ally_path.as_deref(), Some("allies"));
    assert_eq!(post.ally_count, Some(65536));
    assert_eq!(layout.cps[1].ally_path, None);
    assert_eq!(layout.cps[1].ally_count, None);
}

/// The ten hashes are transcribed as LITERALS (the disc numbers the Phase
/// B addendum lists) — and, MEASURED here, all ten are also exactly
/// `classdb::fnv1a_lowercase` of their key names. The addendum says they
/// are "NOT fnv1a of the key names"; that is a correction, not a change:
/// the port's own lowercasing is a byte-wise `| 0x20`, under which
/// `Value_ATK_Alliance` hashes to 0xA340D6B5. (A `to_lowercase()`-style
/// hash — `_` is 0x5F, `_ | 0x20` is 0x7F — does NOT, which is the
/// "naive lookup finds nothing" the addendum ran into.) The literals stay
/// because they are what the disc data was read with; this test pins BOTH
/// facts so neither can drift.
#[test]
fn the_value_prop_hashes_are_the_disc_literals_and_the_name_hashes() {
    for (i, f) in ["Alliance", "Empire", "Republic", "CIS", "Locals"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            PROP_VALUE_ATK[i],
            crate::classdb::fnv1a_lowercase(&format!("Value_ATK_{f}")),
            "ATK {f}"
        );
        assert_eq!(
            PROP_VALUE_DEF[i],
            crate::classdb::fnv1a_lowercase(&format!("Value_DEF_{f}")),
            "DEF {f}"
        );
    }
    assert_eq!(PROP_VALUE_ATK[0], 0xA340_D6B5);
    assert_eq!(PROP_VALUE_DEF[4], 0xE689_EF93);
    // No hash collides with any other prop key we read.
    let mut all = vec![
        PROP_TEAM,
        PROP_LABEL,
        PROP_CAPTURE_REGION,
        PROP_CONTROL_REGION,
        PROP_SPAWN_PATH,
        PROP_ALLY_PATH,
        PROP_ALLY_COUNT,
        PROP_VALUE_BLEED,
    ];
    all.extend(PROP_VALUE_ATK);
    all.extend(PROP_VALUE_DEF);
    let n = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), n, "prop key hashes are distinct");
}

/// A hand-built instance carrying the raw hashes: the five ATK and five
/// DEF values land in faction order, and a faction the post says nothing
/// about takes the class ctor's 100.
#[test]
fn the_raw_value_props_reach_command_post_def() {
    let props: String = PROP_VALUE_ATK
        .iter()
        .enumerate()
        // Locals (row 4) is left out on purpose: it must default.
        .take(4)
        .map(|(i, h)| format!(r#"({h}, "{}"),"#, (i as i32 + 1) * 10))
        .chain(
            PROP_VALUE_DEF
                .iter()
                .enumerate()
                .map(|(i, h)| format!(r#"({h}, "{}"),"#, (i as i32 + 1) * 5)),
        )
        .collect();
    let m: Manifest = ron::from_str(&format!(
        r#"(
            map: "t", source_game: "bf2",
            terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: [],
            classes: [], geometries: [],
            regions: [(shape: "sphere", name: "cpxcapture",
                rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,0,0),
                size: (0.1,4,0.1), props: [])],
            instances: [(class: "cp_c", name: "CPX",
                rotation: ((1,0,0),(0,1,0),(0,0,1)), translation: (0,0,0),
                props: [(3334562297, "cpxcapture"), {props}]),],
        )"#
    ))
    .expect("manifest parses");
    let l = MapLayout::from_manifest(&m);
    assert_eq!(l.cps.len(), 1);
    assert_eq!(l.cps[0].value_atk, [10, 20, 30, 40, VALUE_DEFAULT]);
    assert_eq!(l.cps[0].value_def, [5, 10, 15, 20, 25]);
    // A post that authors none of them is all defaults.
    let plain = MapLayout::from_manifest(&manifest());
    assert!(plain
        .cps
        .iter()
        .all(|c| c.value_atk == [VALUE_DEFAULT; 5] && c.value_def == [VALUE_DEFAULT; 5]));
}

#[test]
fn apply_mission_restricts_to_roster_and_kills_and_reassigns() {
    let mut l = MapLayout::from_manifest(&manifest());
    assert_eq!(l.cps.len(), 5);
    let m = mission(
        r#"(
            map: "t", mode: "con", source_game: "bf2",
            teams: [(faction: "imp", units: 16, tickets: 150),
                    (faction: "all", units: 16, tickets: 150)],
            ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
            era: Some("gcw"),
            posts: ["cp1", "CP2", "cp3", "cp4"],
            killed: ["CP4"],
            cps: [("cp1", 1), ("cp3", 2)],
            death_regions: ["Sarlac01", "nosuchregion"],
        )"#,
    );
    let a = l.apply_mission(&m);
    // cp5 is off the roster; cp4 is on it but killed.
    assert_eq!(
        (a.kept, a.dropped_by_roster, a.killed, a.reassigned),
        (3, 1, 1, 2)
    );
    // LAYOUT order is preserved — the sim indexes posts by position.
    let names: Vec<&str> = l.cps.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["CP1", "CP2", "CP3"]);
    assert_eq!(l.cps[0].team, 1);
    assert_eq!(l.cps[1].team, 1, "untouched posts keep the world's team");
    assert_eq!(l.cps[2].team, 2);
    // One death region resolves, one does not.
    assert_eq!(a.death_regions.len(), 1);
    assert_eq!(a.missing_death_regions, ["nosuchregion"]);
    assert!(a.death_regions[0].contains(Vec3::new(0.0, -5.0, 0.0)));
}

/// BF1 inlines conquest and hands no roster to an objective, so its
/// missions must leave the world layer's posts exactly as they are.
#[test]
fn bf1_mission_with_empty_roster_keeps_world_posts() {
    let mut l = MapLayout::from_manifest(&manifest());
    let m = mission(
        r#"(
            map: "t", mode: "con", source_game: "bf1",
            teams: [(faction: "imp", units: 25, tickets: 250),
                    (faction: "all", units: 25, tickets: 250)],
            ai_alive_cap: 25, spawn_delay: 10.0, spawn_interval: 0.25,
            era: Some("gcw"),
        )"#,
    );
    let a = l.apply_mission(&m);
    assert_eq!(
        (a.kept, a.dropped_by_roster, a.killed, a.reassigned),
        (5, 0, 0, 0)
    );
    assert_eq!(l.cps.len(), 5);
    assert!(a.death_regions.is_empty());
    // No SetMaxPlayerFlyHeight: the engine's ceiling (card JETP).
    assert_eq!(a.fly_ceiling, 200.0);
    let m = mission(
        r#"(
            map: "t", mode: "con", source_game: "bf2",
            teams: [(faction: "imp", units: 20, tickets: 150),
                    (faction: "all", units: 20, tickets: 150)],
            ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
            max_player_fly_height: Some(14.0),
        )"#,
    );
    assert_eq!(l.apply_mission(&m).fly_ceiling, 14.0);
}

/// The per-game half of the `PrimaryStance` bitmask rule lives HERE, in
/// the one place that holds both the authored value and the manifest's
/// `source_game` (RE lead-verified — `core::commander::stance_mask`): the
/// SAME hint is prone-only on a BF1 map and stand on a BF2 one.
#[test]
fn a_hints_stance_mask_is_read_per_game() {
    let dir = std::env::temp_dir().join(format!("openbf-hint-stance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    // PrimaryStance 4, SecondaryStance 0x1a = stance bits 2 (prone) + both
    // peek sides (8 | 16).
    let src = |game: &str| {
        format!(
            r#"(
        map: "t", source_game: "{game}",
        terrain_name: None, sky_name: None, terrain_glb: None,
        models: [], textures: [], classes: [], geometries: [],
        regions: [], instances: [],
        nav: Some((
            plan: Some((
                hubs: [(name: "Hub0", pos: (0, 0, 0), radius: 2.0, arcs: [])],
                arcs: [], routes: "nav/none.bin",
            )),
            hints: [(name: "hint0", kind: 2,
                     xform: ((1,0,0),(0,1,0),(0,0,1),(5,0,6)),
                     primary_stance: Some(4), secondary_stance: Some(26))],
        )),
    )"#
        )
    };
    let mask = |game: &str| {
        let m: Manifest = ron::from_str(&src(game)).expect("manifest parses");
        let g = nav_graph(&m, &dir).expect("graph");
        let h = &g.hints[0];
        (h.stance_mask, h.secondary_stance_mask, h.peek_sides)
    };
    use core::commander::{MASK_CROUCH, MASK_PRONE, MASK_STAND};
    // BF1 keeps the raw byte: 4 is prone alone, and 26 & 7 = 2 is crouch.
    assert_eq!(mask("bf1"), (MASK_PRONE, MASK_CROUCH, 3));
    // BF2 masks `& 3`: 4 becomes 0 -> the ctor default, stand. Its AI never
    // prones, and the peek sides are read the same way on both.
    assert_eq!(mask("bf2"), (MASK_STAND, MASK_CROUCH, 3));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A manifest with a graph: the sidecar is opened through the path the
/// MANIFEST names (layer keys are de-duplicated, so it cannot be rebuilt
/// from a layer name), and a `BARR` becomes an XZ rectangle.
#[test]
fn nav_graph_reads_the_sidecar_the_manifest_names() {
    use core::nav::Mover;
    let dir = std::env::temp_dir().join(format!("openbf-nav-graph-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("nav")).expect("temp dir");
    // routes_h.bin: two hubs, five types; SOLDIER hub 0 has one row whose
    // entry for hub 1 is weight 31 slot 0.
    let mut bin = vec![2u8, 0, 5];
    for t in 0..5 {
        for h in 0..2 {
            if t == 0 && h == 0 {
                bin.extend_from_slice(&[1, 0, 31 << 3]);
            } else {
                bin.push(0);
            }
        }
    }
    std::fs::write(dir.join("nav/routes_h.bin"), &bin).expect("sidecar");

    let src = r#"(
        map: "t", source_game: "bf2",
        terrain_name: None, sky_name: None, terrain_glb: None,
        models: [], textures: [], classes: [], geometries: [],
        regions: [], instances: [],
        nav: Some((
            plan: Some((
                hubs: [
                    (name: "Hub0", pos: (0, 3, 0), radius: 2.0, arcs: [0]),
                    (name: "Hub1", pos: (20, 3, 0), radius: 2.0, arcs: [0]),
                ],
                arcs: [(name: "Connection0", a: 0, b: 1, type_mask: 1, flags: 0)],
                routes: "nav/routes_h.bin",
            )),
            hints: [(name: "hint0", kind: 1,
                     xform: ((1,0,0),(0,1,0),(0,0,1),(5,0,6)))],
            barriers: [(name: "barrier0", pos: (10, 0, 5),
                        rows: ((1,0,0),(0,1,0),(0,0,1)),
                        size: (4, 2, 1), flags: 1)],
        )),
    )"#;
    let m: Manifest = ron::from_str(src).expect("manifest parses");
    let g = nav_graph(&m, &dir).expect("graph");
    assert_eq!((g.hubs.len(), g.arcs.len()), (2, 1));
    assert_eq!(g.routes.hub_count, 2);
    assert_eq!(
        g.next_hop(0, 1, Mover::SOLDIER, None, None, 0),
        Some((0, 1))
    );
    // The hint comes across too (`core::commander` picks from them).
    assert_eq!(g.hints.len(), 1);
    assert_eq!(g.hints[0].pos, Vec3::new(5.0, 0.0, 6.0));
    // It authors no `PrimaryStance`, and a hint that authors none still
    // asks for STAND (both hint ctors default to it) — the mask is never 0.
    assert_eq!(g.hints[0].stance_mask, core::commander::MASK_STAND);
    assert_eq!(
        g.hints[0].secondary_stance_mask,
        core::commander::MASK_STAND
    );
    assert_eq!(g.hints[0].peek_sides, 0);
    // The barrier is an XZ rectangle at (pos.x, pos.z) with HALF sizes.
    assert_eq!(g.barriers.len(), 1);
    let b = g.barriers[0];
    assert_eq!(
        (b.centre, b.half, b.mask),
        (Vec2::new(10.0, 5.0), Vec2::new(4.0, 1.0), 1)
    );
    assert!(b.contains(Vec2::new(13.0, 5.5)));
    assert!(!b.contains(Vec2::new(13.0, 7.0)));

    // No sidecar: the graph still builds, and the BFS fallback routes on
    // it (the table answers nothing).
    std::fs::remove_file(dir.join("nav/routes_h.bin")).expect("rm");
    let g = nav_graph(&m, &dir).expect("graph without a table");
    assert_eq!(g.next_hop(0, 1, Mover::SOLDIER, None, None, 0), None);
    assert_eq!(g.route_bfs(0, 1, Mover::SOLDIER, &|_| true), Some((0, 1)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A map with no `nav` at all has no graph — the AI walks straight at its
/// post, as it did before Phase A.
#[test]
fn a_manifest_without_nav_has_no_graph() {
    assert!(nav_graph(&manifest(), std::path::Path::new(".")).is_none());
}

#[test]
fn region_lookup_is_case_insensitive() {
    let l = MapLayout::from_manifest(&manifest());
    assert!(l.region("SARLAC01").is_some());
    assert!(l.region("sarlac01").is_some());
    assert!(l.region("nope").is_none());
}

/// Issue #171 parity rows: the sweep's extra rows apply over the table's on
/// their own map and era only, an empty list is the game's own path, and
/// `StartTeam` / `SpawnWeight` set what they name. A map fixed in both eras
/// carries the same rows in each (Rhen Var Harbor's weights differ), and
/// every table row's post index is one its map keeps after the mission
/// roster (checked against the baked layout and mission when present).
#[test]
fn conquest_parity_rows() {
    let con = |map: &str, era: &str| {
        mission(&format!(
            r#"(
                map: "{map}", mode: "con", source_game: "bf2",
                teams: [(faction: "rep", units: 16, tickets: 150),
                        (faction: "cis", units: 16, tickets: 150)],
                ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
                era: Some("{era}"),
            )"#
        ))
    };
    let state = |l: &MapLayout| -> Vec<(u8, [i32; 5], [i32; 5], f32)> {
        l.cps
            .iter()
            .map(|c| (c.team, c.value_atk, c.value_def, c.ai_spawn_weight))
            .collect()
    };
    let m = con("t", "cw");
    let (mut a, mut b) = (
        MapLayout::from_manifest(&manifest()),
        MapLayout::from_manifest(&manifest()),
    );
    assert_eq!(a.apply_mission(&m), b.apply_mission_with(&m, &[]));
    assert_eq!(state(&a), state(&b));

    let extra = [
        ("t", true, ParityFix::StartTeam { post: 0, team: 2 }),
        ("t", true, ParityFix::NeutralStart(1)),
        (
            "t",
            true,
            ParityFix::SpawnWeight {
                post: 2,
                weight: 30.0,
            },
        ),
        ("t", true, goals(3, 1, 4, 9)),
        // Another era and another map: untouched.
        ("t", false, ParityFix::StartTeam { post: 4, team: 2 }),
        ("u", true, ParityFix::StartTeam { post: 4, team: 2 }),
    ];
    let mut l = MapLayout::from_manifest(&manifest());
    l.apply_mission_with(&m, &extra);
    let s = state(&l);
    assert_eq!(s[0].0, 2);
    assert_eq!(s[1].0, 0);
    assert_eq!(s[2].3, 30.0);
    // Team 1 is the CIS: the CIS row of the values (crate::walk::faction_row).
    let cis = crate::walk::faction_row("cis");
    assert_eq!((s[3].1[cis], s[3].2[cis]), (4, 9));
    let base = state(&a);
    assert_eq!(s[4], base[4]);
    for i in [0, 1, 2] {
        assert_eq!((s[i].1, s[i].2), (base[i].1, base[i].2));
    }

    // The table's rows: a map fixed in both eras has the same rows in each
    // (Rhen Var Harbor's CW spawn weight is milder; BF2 Kamino's are GCW
    // only: its CW entry measured even without them),
    // and every row's post index is inside its map's roster when the bake
    // is here to say so.
    for map in ["bf2_dea1", "bf1_kas2", "bf1_bes1", "bf2_myg1", "bf2_uta1"] {
        let rows = |cw: bool| {
            CONQUEST_PARITY
                .iter()
                .filter(|(m, e, _)| *m == map && *e == cw)
                .map(|(_, _, f)| format!("{f:?}"))
                .collect::<Vec<_>>()
        };
        assert!(!rows(false).is_empty(), "{map}");
        assert_eq!(rows(false), rows(true), "{map}");
    }
    assert!(!CONQUEST_PARITY
        .iter()
        .any(|(map, cw, _)| *map == "bf2_kam1" && *cw));
    let rhn1 = |cw: bool| {
        CONQUEST_PARITY
            .iter()
            .filter(|(map, e, _)| *map == "bf1_rhn1" && *e == cw)
            .count()
    };
    assert_eq!((rhn1(false), rhn1(true)), (2, 2));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for (map, cw, fix) in CONQUEST_PARITY {
        let post = match *fix {
            ParityFix::NeutralStart(i) => i,
            ParityFix::StartTeam { post, .. }
            | ParityFix::SpawnWeight { post, .. }
            | ParityFix::Goals { post, .. }
            | ParityFix::AllyNodes { post, .. } => post,
        };
        // A CW-only map (BF1 Geonosis, Kamino) has one `{map}_con.ron`
        // carrying `era: Some("cw")`, and no `_con_cw` file.
        let read = |suffix: &str| {
            std::fs::read_to_string(root.join(format!("missions/{map}{suffix}.ron")))
        };
        let msrc = if *cw {
            read("_con_cw").or_else(|_| read("_con"))
        } else {
            read("_con")
        };
        let lpath = root.join(format!("assets_baked/{map}/manifest.ron"));
        let (Ok(msrc), Ok(lsrc)) = (msrc, pak::read_to_string(&lpath)) else {
            continue;
        };
        let mission: MissionDef = ron::from_str(&msrc).expect("mission parses");
        assert_eq!(
            mission.era.as_deref() == Some("cw"),
            *cw,
            "{map}: the row's era is not the mission's ({:?})",
            mission.era
        );
        let mut manifest: Manifest = ron::from_str(&lsrc).expect("manifest parses");
        // The mode sublevel, merged as `Scene::apply_mode_layer` does.
        if let Some(i) = crate::assets::mode_layer_index(&manifest, "con") {
            let layer = manifest.layers.remove(i);
            manifest.regions.extend(layer.regions);
            manifest.instances.extend(layer.instances);
        }
        let mut l = MapLayout::from_manifest(&manifest);
        l.apply_mission(&mission);
        assert!(
            post < l.cps.len(),
            "{map} (cw {cw}): post {post} of {}",
            l.cps.len()
        );
        checked += 1;
    }
    // With the bake here, every row is checked: none is skipped silently.
    if root.join("assets_baked/bf2_dea1/manifest.ron").exists() {
        assert_eq!(checked, CONQUEST_PARITY.len(), "rows checked");
    }
}

#[test]
fn an_ally_nodes_row_drops_only_its_posts_ally_path_nodes() {
    let con = |era: &str| {
        mission(&format!(
            r#"(
                map: "t", mode: "con", source_game: "bf2",
                teams: [(faction: "rep", units: 16, tickets: 150),
                        (faction: "cis", units: 16, tickets: 150)],
                ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
                era: Some("{era}"),
            )"#
        ))
    };
    let layout = || {
        let mut m = manifest();
        m.instances[0].props.extend([
            (PROP_ALLY_PATH, "allies".into()),
            (PROP_ALLY_COUNT, "4".into()),
        ]);
        let mut l = MapLayout::from_manifest(&m);
        l.paths.push(SpawnPath {
            name: "allies".into(),
            nodes: (0..5)
                .map(|i| SpawnNode {
                    pos: Vec3::new(i as f32, 0.0, 0.0),
                    yaw: 0.0,
                })
                .collect(),
        });
        l
    };
    let xs = |l: &MapLayout| -> Vec<f32> {
        l.path("allies")
            .unwrap()
            .nodes
            .iter()
            .map(|n| n.pos.x)
            .collect()
    };
    let extra = [(
        "t",
        true,
        ParityFix::AllyNodes {
            post: 0,
            drop: 0b10110,
        },
    )];
    let mut cw = layout();
    cw.apply_mission_with(&con("cw"), &extra);
    assert_eq!(xs(&cw), [0.0, 3.0]);
    // Another era keeps every node; a post without an ally path is a no-op.
    let mut gcw = layout();
    gcw.apply_mission_with(&con("gcw"), &extra);
    assert_eq!(xs(&gcw), [0.0, 1.0, 2.0, 3.0, 4.0]);
    let mut other = layout();
    other.apply_mission_with(
        &con("cw"),
        &[("t", true, ParityFix::AllyNodes { post: 1, drop: 1 })],
    );
    assert_eq!(xs(&other).len(), 5);
}
