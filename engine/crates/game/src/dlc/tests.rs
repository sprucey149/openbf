use super::*;
use std::path::PathBuf;

fn mission() -> MissionDef {
    ron::from_str(
        r#"(map:"tat2",mode:"eli",source_game:"bf2",teams:[],
            heroes:[],ai_alive_cap:16,spawn_delay:10.0,spawn_interval:0.25,era:Some("cw"))"#,
    )
    .unwrap()
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openbf_dlc_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("classes")).unwrap();
        std::fs::create_dir_all(path.join("side_bf2")).unwrap();
        Self(path)
    }

    fn manifest(&self, heroes: &str) {
        std::fs::write(
            self.0.join("side_bf2/dlc_heroes.ron"),
            format!("(heroes:[{heroes}],soldiers:[],weapons:[],textures:[])"),
        )
        .unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn dlc_heroes_default_to_on_only_with_the_bake_marker() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let options =
        || crate::shell_ia::instant_action(root, root, core::skill::Difficulty::Normal).options;
    let absent = options();
    assert!(!absent.dlc_heroes_installed);
    assert!(!absent.dlc_heroes);
    scratch.manifest(BOTH);
    let mut present = options();
    assert!(present.dlc_heroes_installed);
    assert!(present.dlc_heroes);
    present.adjust(ui::shell::Opt::DlcHeroes, 1, 1);
    assert!(!present.dlc_heroes);
}

#[test]
fn a_readers_own_class_db_gains_the_heroes_only_with_the_marker() {
    let scratch = Scratch::new();
    let classes = scratch.0.join("classes");
    std::fs::write(
        classes.join(CLASSES),
        r#"[(name:"dlc_hero_fisto",kind:"entc",base:"soldier",props:[])]"#,
    )
    .unwrap();
    let mut db = crate::classdb::ClassDb::default();
    load_classes(&classes, &mut db).unwrap();
    assert!(
        db.resolve("dlc_hero_fisto").is_none(),
        "no marker, no class"
    );
    scratch.manifest(BOTH);
    load_classes(&classes, &mut db).unwrap();
    assert!(db.resolve("dlc_hero_fisto").is_some());
}

const BOTH: &str = r#"(class:"dlc_hero_fisto",geometry:"rep_inf_kitfisto",side:"rep"),
    (class:"dlc_hero_ventress",geometry:"cis_inf_ventress",side:"cis")"#;

#[test]
fn absent_addon_and_other_sides_keep_authored_assignments() {
    let root = Scratch::new();
    let classes = root.0.join("classes");
    let mut m = mission();
    assert_eq!(hero(&classes, &m, "rep").unwrap(), None);
    assert_eq!(hero(&classes, &m, "cis").unwrap(), None);
    // Invalid optional bytes must not affect another game or faction.
    std::fs::write(root.0.join("side_bf2/dlc_heroes.ron"), "bad").unwrap();
    for faction in ["all", "imp"] {
        assert_eq!(hero(&classes, &m, faction).unwrap(), None);
    }
    m.source_game = "bf1".into();
    assert_eq!(hero(&classes, &m, "rep").unwrap(), None);
}

#[test]
fn the_dlc_manifest_resolves_heroes_only_for_bf2_hero_assault() {
    let root = Scratch::new();
    let classes = root.0.join("classes");
    root.manifest(BOTH);
    std::fs::write(classes.join(CLASSES), "[]").unwrap();
    let mut m = mission();
    for mode in ["eli", "con", "ass", "ctf", "hun"] {
        m.mode = mode.into();
        for (side, class) in HEROES {
            assert_eq!(
                hero(&classes, &m, side).unwrap(),
                (mode == "eli").then_some(class)
            );
        }
    }
}

#[test]
fn only_complete_two_hero_bake_enables_native_assignments() {
    let root = Scratch::new();
    let classes = root.0.join("classes");
    root.manifest(BOTH);
    assert!(hero(&classes, &mission(), "rep").is_err());
    std::fs::write(classes.join(CLASSES), "[]").unwrap();
    assert_eq!(
        hero(&classes, &mission(), "rep").unwrap(),
        Some("dlc_hero_fisto")
    );
    assert_eq!(
        hero(&classes, &mission(), "cis").unwrap(),
        Some("dlc_hero_ventress")
    );
    root.manifest(r#"(class:"dlc_hero_fisto",side:"rep")"#);
    assert!(hero(&classes, &mission(), "rep").is_err());
    root.manifest(&BOTH.replace("dlc_hero_ventress", "cis_hero_grievous"));
    assert!(hero(&classes, &mission(), "rep").is_err());
}

fn mygeeto() -> MissionDef {
    MissionDef::load(
        &Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"))
            .join("bf2_myg1_con_cw.ron"),
    )
    .unwrap()
}

fn rosters(root: &Path, mission: &MissionDef) -> [crate::roster::FactionRoster; 2] {
    let (rosters, warnings) = crate::roster::FactionRoster::pair(
        &root.join("classes"),
        "bf2",
        ["rep", "cis"],
        Some(mission),
    )
    .unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    rosters
}

fn register(
    rosters: &[crate::roster::FactionRoster; 2],
    heroes: [Option<u8>; 2],
) -> core::sim::Sim {
    use core::{cp::CpDef, region::Region};
    use glam::Vec3;
    let mut sim = core::sim::Sim::empty(42);
    sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let mut names = Vec::new();
    for (team, roster) in rosters.iter().enumerate() {
        sim.classes[team] = roster
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
        let pos = Vec3::X * team as f32 * 100.0;
        sim.add_cp(
            CpDef {
                name: format!("cp{team}"),
                pos,
                capture: Region::sphere(pos, 8.0),
                control: None,
                value_bleed: 1,
                value_atk: [1; 2],
                value_def: [1; 2],
                spawn_nodes: vec![(pos, 0.0)],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(team as u8),
        );
    }
    crate::heroes::install(&mut sim, heroes);
    sim
}

fn spawn_scored_players(sim: &mut core::sim::Sim) {
    for team in 0..2 {
        let spec = sim.classes[team][0].spec(
            team as u8,
            glam::Vec3::X * team as f32 * 100.0,
            0.0,
            core::unit::Controller::Player { slot: team as u8 },
            0,
            0,
        );
        let player = sim.spawn(&spec).unwrap();
        sim.heroes
            .as_mut()
            .unwrap()
            .points
            .resize(sim.units.len(), 0);
        sim.heroes.as_mut().unwrap().points[player.index as usize] = 10;
    }
}

fn battle_pad(tick: usize) -> core::sim::PadState {
    core::sim::PadState {
        buttons: if tick == 1 {
            core::sim::buttons::HERO_ACCEPT
        } else if tick > 2 && tick % 10 < 5 {
            core::sim::buttons::FIRE
        } else {
            0
        },
        ..Default::default()
    }
}

fn first_difference<T: std::fmt::Debug + PartialEq>(name: &str, a: &[T], b: &[T]) {
    if a.len() != b.len() {
        eprintln!("{name}: lengths {} / {}", a.len(), b.len());
    }
    if let Some((index, (a, b))) = a.iter().zip(b).enumerate().find(|(_, (a, b))| a != b) {
        eprintln!("{name}[{index}]: PS2 {a:?}; PC {b:?}");
    }
}

fn hero_assault() -> MissionDef {
    MissionDef::load(
        &Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions")).join("tat2_eli.ron"),
    )
    .unwrap()
}

fn assault_rosters(
    root: &Path,
    mission: &MissionDef,
    dlc: bool,
) -> [crate::roster::FactionRoster; 2] {
    let classes = root.join("classes");
    let (mut pair, warnings) =
        crate::roster::FactionRoster::pair(&classes, "bf2", ["all", "imp"], Some(mission)).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    for (team, roster) in pair.iter_mut().enumerate() {
        crate::heroes::prepare_assault(roster, &classes, mission, team, dlc).unwrap();
    }
    pair
}

#[test]
#[ignore = "requires OPENBF_DLC_TEST_ROOT selective DLC bake over retail assets"]
fn real_dlc_heroes_extend_both_hero_assault_rosters() {
    let root = PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_ROOT").expect("DLC test overlay"));
    let mission = hero_assault();
    let without = assault_rosters(&root, &mission, false);
    let with = assault_rosters(&root, &mission, true);
    for (team, (stock, extended)) in without.iter().zip(&with).enumerate() {
        assert_eq!(stock.classes.len(), mission.teams[team].classes.len());
        let (added, retained) = extended.classes.split_last().unwrap();
        assert!(stock
            .classes
            .iter()
            .map(|c| &c.class)
            .eq(retained.iter().map(|c| &c.class)));
        assert_eq!(added.class, HEROES[team].1);
        assert_eq!(Some(added.label.as_str()), label(&added.class));
        assert!(
            added.weapon.saber.is_some(),
            "{}: usable saber",
            added.class
        );
    }
}

/// The optional bake's effect layer, read last, and only with its marker.
#[test]
#[ignore = "requires OPENBF_DLC_TEST_ROOT and OPENBF_DLC_TEST_BASE_ROOT"]
fn real_dlc_effects_load_only_with_the_bake_and_after_retail() {
    let dlc = PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_ROOT").expect("DLC test overlay"));
    let base =
        PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_BASE_ROOT").expect("retail test assets"));
    let map = base.join("bf2_myg1");
    let names = ["com_sfx_starbladetrail", "com_sfx_weap_fistobubble_exp"];
    let library = crate::fx::Library::load(&dlc, &map, "bf2", &["rep", "cis"]);
    let retail = crate::fx::Library::load(&base, &map, "bf2", &["rep", "cis"]);
    let gcw = crate::fx::Library::load(&dlc, &map, "bf2", &["all", "imp"]);
    for name in names {
        assert!(library.get(name).is_some(), "{name}: baked");
        assert!(retail.get(name).is_none(), "{name}: no bake");
        assert!(gcw.get(name).is_none(), "{name}: not their battle");
    }
    assert!(library.get("com_sfx_dusttrail").is_some());
    assert_eq!(library.len(), retail.len() + 7, "only the DLC's seven join");
}

/// Kit Fisto's tentacles rest where the body bake placed them and only they
/// move: the body's joints, which the saber's tracks follow, do not.
#[test]
#[ignore = "requires OPENBF_DLC_TEST_ROOT selective DLC bake"]
fn real_fisto_tentacles_hang_from_his_head_and_sway_alone() {
    let root = PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_ROOT").expect("DLC test overlay"));
    let body =
        crate::assets::load_skinned_glb(&root.join("side_bf2/soldiers/rep_inf_kitfisto.glb"))
            .unwrap();
    let skel = &body.skeleton;
    let head = skel.index_of("bone_head").expect("head");
    let root_bone = skel.index_of("tentacle_DummyRoot").expect("tentacle root");
    assert_eq!(skel.parents[root_bone], Some(head));
    assert!(root_bone >= 26, "body joints keep their indices");
    let tip = skel.index_of("tentacle_bone_string_3").expect("a tip");
    let rest = crate::skin::Pose::bind(skel);
    let mut swayed = rest.clone();
    skel.sway(&mut swayed, 0.7);
    let rest_palette = skel.palette(&rest.locals());
    let sway_palette = skel.palette(&swayed.locals());
    assert!(rest_palette[tip].abs_diff_eq(glam::Mat4::IDENTITY, 1e-4));
    assert!(!sway_palette[tip].abs_diff_eq(glam::Mat4::IDENTITY, 1e-4));
    for (bone, name) in skel.names.iter().enumerate() {
        if !name.starts_with("tentacle_") {
            assert_eq!(sway_palette[bone], rest_palette[bone], "{name}");
        }
    }
    let at = sway_palette[tip]
        .transform_point3(skel.global_bind[tip].transform_point3(glam::Vec3::ZERO))
        - skel.global_bind[tip].transform_point3(glam::Vec3::ZERO);
    assert!(at.length() > 0.001 && at.length() < 0.25, "{at:?}");
}

/// Reconstruct the former append path independently, then compare stepped sim
/// hashes with this branch's absent-addon path on the same retail class data.
#[test]
#[ignore = "requires OPENBF_DLC_TEST_BASE_ROOT retail assets without DLC manifest"]
fn absent_dlc_matches_authored_hero_sim_hashes() {
    let root =
        PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_BASE_ROOT").expect("retail test assets"));
    assert!(!root.join("side_bf2/dlc_heroes.ron").exists());
    let m = mygeeto();
    let mut reference = rosters(&root, &m);
    let mut current = reference.clone();
    let mut selected = [None; 2];
    for team in 0..2 {
        let name = &m
            .heroes
            .iter()
            .find(|(t, _)| usize::from(*t) == team + 1)
            .unwrap()
            .1;
        let db =
            crate::roster::load_classdb(&root.join("classes"), "bf2", &[&reference[team].faction])
                .unwrap();
        let mut authored = crate::roster::ClassRoster::build(&db, name, 0, Some(1)).unwrap();
        authored.anim.custom =
            crate::animator::custom::for_hero(&root.join("classes"), &m, name).unwrap();
        reference[team].classes.push(authored);
        selected[team] =
            crate::heroes::append(&mut current[team], &root.join("classes"), Some(&m), team);
        assert_eq!(
            selected[team],
            Some((reference[team].classes.len() - 1) as u8)
        );
    }
    let mut reference = register(&reference, selected);
    let mut current = register(&current, selected);
    for sim in [&mut reference, &mut current] {
        spawn_scored_players(sim);
    }
    for tick in 0..120 {
        assert_eq!(reference.hash(), current.hash(), "absent DLC tick {tick}");
        let pad = battle_pad(tick);
        reference.step(&[pad, pad]);
        current.step(&[pad, pad]);
    }
    assert!(current
        .players
        .iter()
        .flatten()
        .all(|h| current.is_hero(*h)));
    println!(
        "absent DLC: 120 identical state hashes; final {:016x}",
        current.hash()
    );
}

/// Conquest keeps the authored heroes and their state with either bake.
#[test]
#[ignore = "requires OPENBF_DLC_TEST_ROOT and OPENBF_DLC_TEST_BASE_ROOT"]
fn conquest_heroes_match_with_or_without_the_dlc_bake() {
    let installed =
        PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_ROOT").expect("DLC test overlay"));
    let absent =
        PathBuf::from(std::env::var_os("OPENBF_DLC_TEST_BASE_ROOT").expect("retail test assets"));
    assert!(installed.join("side_bf2/dlc_heroes.ron").exists());
    assert!(!absent.join("side_bf2/dlc_heroes.ron").exists());
    let m = mygeeto();
    let [mut on_bake, mut plain] = [&installed, &absent].map(|root| {
        let mut rosters = rosters(root, &m);
        let mut selected = [None; 2];
        for (team, roster) in rosters.iter_mut().enumerate() {
            selected[team] = crate::heroes::append(roster, &root.join("classes"), Some(&m), team);
            let hero = &roster.classes[usize::from(selected[team].expect("authored hero"))];
            assert!(!hero.class.starts_with("dlc_"), "{}", hero.class);
        }
        let mut sim = register(&rosters, selected);
        spawn_scored_players(&mut sim);
        sim
    });
    for tick in 0..120 {
        assert_eq!(on_bake.hash(), plain.hash(), "Conquest tick {tick}");
        let pad = battle_pad(tick);
        on_bake.step(&[pad, pad]);
        plain.step(&[pad, pad]);
    }
}

#[test]
#[ignore = "requires OPENBF_DLC_TEST_ROOT and OPENBF_DLC_TEST_PC_ROOT baked source overlays"]
fn ps2_and_pc_dlc_sources_have_identical_hero_sim_hashes() {
    let roots = ["OPENBF_DLC_TEST_ROOT", "OPENBF_DLC_TEST_PC_ROOT"]
        .map(|key| PathBuf::from(std::env::var_os(key).expect(key)));
    let mission = hero_assault();
    let [mut ps2, mut pc] = roots.map(|root| {
        let mut rosters = assault_rosters(&root, &mission, true);
        for (roster, (_, name)) in rosters.iter_mut().zip(HEROES) {
            roster.classes.retain(|class| class.class == name);
            assert_eq!(roster.classes.len(), 1);
        }
        let mut sim = register(&rosters, [None; 2]);
        for team in 0..2 {
            let spec = sim.classes[team][0].spec(
                team as u8,
                glam::Vec3::X * team as f32 * 100.0,
                0.0,
                core::unit::Controller::Player { slot: team as u8 },
                0,
                0,
            );
            sim.spawn(&spec).unwrap();
        }
        sim.deathmatch = Some(core::sim::deathmatch::Deathmatch::new(50, [0, 0]));
        // Each hero has an ordinary enemy within saber range. The two
        // players' own bases are apart, so these targets isolate their hits.
        for team in 0..2 {
            sim.spawn(&core::unit::UnitSpec {
                controller: core::unit::Controller::Idle,
                ..core::unit::UnitSpec::trooper(
                    1 - team,
                    glam::Vec3::new(f32::from(team) * 100.0, 0.0, -1.0),
                )
            })
            .unwrap();
        }
        sim
    });
    assert_eq!(ps2.weapon_classes.len(), pc.weapon_classes.len());
    for (ps2, pc) in ps2.weapon_classes.iter().zip(&pc.weapon_classes) {
        assert_eq!(ps2.name, pc.name);
        if let (Some(ps2_saber), Some(pc_saber)) = (&ps2.saber, &pc.saber) {
            if ps2_saber.clips != pc_saber.clips {
                if let Some((a, b)) = ps2_saber
                    .clips
                    .iter()
                    .zip(&pc_saber.clips)
                    .find(|(a, b)| a != b)
                {
                    eprintln!(
                        "{}: first differing clip {} / {}; root speeds {:?} / {:?}",
                        ps2.name, a.name, b.name, a.root_speed, b.root_speed
                    );
                    first_difference("root frames", &a.root_frames, &b.root_frames);
                    first_difference("weapon frames", &a.weapon_frames, &b.weapon_frames);
                    first_difference("blade frames", &a.frames, &b.frames);
                }
            }
            assert!(
                ps2_saber.clips == pc_saber.clips,
                "{}: PS2/PC physical saber clips differ",
                ps2.name
            );
            assert!(
                ps2_saber.throw_edges == pc_saber.throw_edges,
                "{}: PS2/PC throw edges differ",
                ps2.name
            );
            assert!(
                ps2_saber.deflect_edges == pc_saber.deflect_edges,
                "{}: PS2/PC deflect edges differ",
                ps2.name
            );
            assert!(
                ps2_saber.weapon_bind == pc_saber.weapon_bind,
                "{}: PS2/PC weapon bind differs",
                ps2.name
            );
            assert!(
                ps2_saber.throw_bindings == pc_saber.throw_bindings,
                "{}: PS2/PC throw bindings differ",
                ps2.name
            );
            assert_eq!(
                ps2_saber.damage, pc_saber.damage,
                "{}: PS2/PC melee damage",
                ps2.name
            );
            assert_eq!(
                ps2_saber.state_seconds, pc_saber.state_seconds,
                "{}: PS2/PC combo timings",
                ps2.name
            );
            assert_eq!(ps2_saber.blades.len(), pc_saber.blades.len());
            for (a, b) in ps2_saber.blades.iter().zip(&pc_saber.blades) {
                assert_eq!(
                    (a.length, a.width),
                    (b.length, b.width),
                    "{}: PS2/PC blade shape",
                    ps2.name
                );
            }
        } else {
            assert_eq!(ps2.saber.is_some(), pc.saber.is_some());
        }
    }
    for tick in 0..120 {
        assert_eq!(
            ps2.hash(),
            pc.hash(),
            "PS2/PC DLC first divergent tick {tick}"
        );
        let pad = battle_pad(tick);
        ps2.step(&[pad, pad]);
        pc.step(&[pad, pad]);
    }
    for handle in ps2.players.iter().take(2) {
        assert!(ps2.hero_unit(ps2.unit(handle.unwrap()).unwrap()));
    }
    println!(
        "PS2/PC DLC: 120 identical hero sim hashes; final {:016x}; target HP {:?}",
        ps2.hash(),
        ps2.units[2..]
            .iter()
            .map(|u| u.health.cur)
            .collect::<Vec<_>>()
    );
}
