use super::*;
use crate::build_chunk as chunk;

fn make_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openbf_addon_test_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_minimal_core_lvl(id: &str, name: &str, desc: &str) -> Vec<u8> {
    let mut body = Vec::new();
    for (k, v) in [
        (format!("mapname.name.{id}"), name),
        (format!("mapname.description.{id}"), desc),
    ] {
        let units: Vec<u16> = v.encode_utf16().chain(std::iter::once(0)).collect();
        body.extend(locl::key(&k).to_le_bytes());
        body.extend(((6 + units.len() * 2) as u16).to_le_bytes());
        for u in units {
            body.extend(u.to_le_bytes());
        }
    }
    body.extend([0u8; 8]);
    let mut locl_payload = chunk(b"NAME", b"english\0");
    locl_payload.extend(chunk(b"BODY", &body));
    let locl_chunk = chunk(b"Locl", &locl_payload);
    let mut out = b"ucfb".to_vec();
    out.extend((locl_chunk.len() as u32).to_le_bytes());
    out.extend(locl_chunk);
    out
}

#[test]
fn detects_invalid_and_dll_only_folders() {
    let empty = make_temp_dir("empty");
    let res = inspect_addon_dir(&empty);
    assert_eq!(res.kind, AddonKind::Invalid);
    assert!(matches!(res.compatibility, AddonCompatibility::Invalid(_)));
    let _ = fs::remove_dir_all(&empty);

    let dll = make_temp_dir("dll");
    fs::write(dll.join("custom_hook.dll"), b"MZ").unwrap();
    let res = inspect_addon_dir(&dll);
    assert_eq!(res.kind, AddonKind::Unsupported);
    assert!(matches!(
        res.compatibility,
        AddonCompatibility::Unsupported(ref msg) if msg.contains("DLL")
    ));
    let _ = fs::remove_dir_all(&dll);
}

#[test]
fn detects_non_pc_addon_as_unsupported() {
    let ps2 = make_temp_dir("ps2");
    fs::create_dir_all(ps2.join("data/_LVL_PS2")).unwrap();
    let res = inspect_addon_dir(&ps2);
    assert_eq!(res.kind, AddonKind::Unsupported);
    assert!(matches!(
        res.compatibility,
        AddonCompatibility::Unsupported(ref m) if m.contains("non-PC")
    ));
    let _ = fs::remove_dir_all(&ps2);
}

#[test]
fn a_bare_side_and_fpm_folder_is_a_replacement_candidate_set() {
    let dir = make_temp_dir("bare_side");
    let inner = dir.join("Some Sides 1.0");
    fs::create_dir_all(inner.join("side")).unwrap();
    fs::create_dir_all(inner.join("fpm/imp")).unwrap();
    fs::write(inner.join("Readme.txt"), "copy side and fpm into _lvl_pc").unwrap();
    fs::write(inner.join("side/imp.lvl"), [b'u', b'c', b'f', b'b', 0, 0, 0, 0]).unwrap();
    fs::write(inner.join("fpm/imp/impoff.lvl"), [b'u', b'c', b'f', b'b', 0, 0, 0, 0]).unwrap();
    let res = inspect_addon_dir(&dir);
    assert!(res.content_root.is_none());
    assert_eq!(res.kind, AddonKind::LegacyAssetPack);
    assert!(res.compatibility != AddonCompatibility::Invalid(String::new()));
    let mut rels: Vec<&str> = res.stock_override_lvls.iter().map(|o| o.rel_path.as_str()).collect();
    rels.sort();
    assert_eq!(
        rels,
        ["Some Sides 1.0/fpm/imp/impoff.lvl", "Some Sides 1.0/side/imp.lvl"]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn detects_single_wrapper_layout_and_core_locl() {
    let dir = make_temp_dir("wrapped");
    let inner = dir.join("MyAddon_v1");
    fs::create_dir_all(inner.join("data/_LVL_PC/TST")).unwrap();
    fs::write(
        inner.join("Readme.txt"),
        "--Test Map - By Modder--\nA test city",
    )
    .unwrap();
    fs::write(
        inner.join("data/_LVL_PC/core.lvl"),
        make_minimal_core_lvl("tst", "Test City", "Test City: Harbor\nHarbor battle"),
    )
    .unwrap();
    fs::write(
        inner.join("data/_LVL_PC/TST/tst.lvl"),
        b"ucfb\x00\x00\x00\x00",
    )
    .unwrap();

    let res = inspect_addon_dir(&dir);
    assert!(res.content_root.is_some());
    assert_eq!(res.display_name, "Test City: Harbor");
    assert_eq!(res.author.as_deref(), Some("Modder"));
    // Without addon missions an unreferenced LVL is a replacement candidate;
    // the stock catalog then finds no stock `tst.lvl` and keeps it inactive.
    assert!(res.unused_lvls.is_empty());
    assert!(res
        .stock_override_lvls
        .iter()
        .any(|o| o.rel_path.ends_with("TST/tst.lvl")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn real_abr_inspection_when_present() {
    // `OPENBF_ABR_ADDON` names a local copy of the ABR addon (`GameData/addon/ABR`).
    let Some(abr) = std::env::var_os("OPENBF_ABR_ADDON") else {
        return;
    };
    let abr = Path::new(&abr);
    if !abr.is_dir() {
        return;
    }
    let res = inspect_addon_dir(abr);
    assert_eq!(res.kind, AddonKind::LegacyMapAddon);
    assert_eq!(res.legacy_addon_id.as_deref(), Some("ABR"));
    assert_eq!(res.display_name, "Abregado-Rae: Venezia");
    assert_eq!(res.author.as_deref(), Some("Bobfinkl"));
    let names: Vec<&str> = res
        .missions
        .iter()
        .map(|m| m.script_name.as_str())
        .collect();
    assert_eq!(names, ["abrg_con", "abrc_con", "abrg_ctf", "abrc_ctf"]);
    assert!(res.missions.iter().all(|m| m.supported));
    assert!(res.stock_override_lvls.is_empty());
    assert!(res.unused_lvls.iter().any(|u| u.ends_with("kas2.lvl")));
    assert!(res.unused_lvls.iter().any(|u| u.ends_with("tat2.lvl")));
}

#[test]
fn a_mission_is_named_by_the_longest_start_of_its_script_name() {
    let table = |key: &str| match key {
        "mapname.name.rxc" => Some("Raxus".to_owned()),
        "mapname.name.cor1" => Some("Coruscant".to_owned()),
        "mapname.name.co" => Some("shorter".to_owned()),
        _ => None,
    };
    let name = |script| mission_text(script, "mapname.name.", table);
    assert_eq!(name("rxc1_con").as_deref(), Some("Raxus"));
    assert_eq!(name("cor1c_ctf").as_deref(), Some("Coruscant"));
    assert_eq!(name("xyz1_con"), None);
}

#[test]
fn only_a_world_named_for_its_stock_folder_is_a_stock_world() {
    for stock in ["cor/cor1.lvl", "kas/kas2.lvl", "spa/spa1.lvl"] {
        assert!(mission_fold::is_stock_world_path(stock), "{stock}");
    }
    for other in [
        "spa/spa_sky.lvl",
        "side/rep.lvl",
        "ingame.lvl",
        "load/common.lvl",
    ] {
        assert!(!mission_fold::is_stock_world_path(other), "{other}");
    }
}

#[test]
fn a_download_laid_out_as_the_game_folder_is_read_addon_by_addon() {
    let dir = make_temp_dir("game_folder");
    let game = dir.join("GameData");
    for addon in ["AAA", "BDT"] {
        fs::create_dir_all(game.join("addon").join(addon)).unwrap();
        fs::write(game.join("addon").join(addon).join("addme.script"), b"ucfb").unwrap();
    }
    fs::create_dir_all(game.join("DATA/_LVL_PC")).unwrap();
    let canon = fs::canonicalize(&dir).unwrap();
    let found = detect_game_folder(&dir, &canon).expect("game folder");
    assert_eq!(found.dir, game);
    assert_eq!(found.addons.len(), 2);

    // One addon on its own is not a game folder.
    let alone = game.join("addon/BDT");
    let canon = fs::canonicalize(&alone).unwrap();
    assert!(detect_game_folder(&alone, &canon).is_none());
    let _ = fs::remove_dir_all(&dir);
}

/// Minimal Lua 5.0 bytecode chunk builder for synthetic addon/mission tests.
fn lua50_chunk(consts: &[(&str, Option<f64>)], insns: &[u32]) -> Vec<u8> {
    let mut b = Vec::new();
    // Header: \x1bLua, version 0x50, little-endian, int=4, size_t=4, inst=4, op=6, a=8, b=9, c=9, num=8, test=31415926.5
    b.extend_from_slice(b"\x1bLua");
    b.extend_from_slice(&[0x50, 1, 4, 4, 4, 6, 8, 9, 9, 8]);
    b.extend_from_slice(&31415926.5f64.to_le_bytes());
    // source string (empty)
    b.extend_from_slice(&0u32.to_le_bytes());
    // lineDefined, nups, numparams, is_vararg, maxstacksize
    b.extend_from_slice(&0i32.to_le_bytes());
    b.extend_from_slice(&[0, 0, 0, 16]);
    // lines, locals, upvalues (all 0)
    b.extend_from_slice(&0i32.to_le_bytes());
    b.extend_from_slice(&0i32.to_le_bytes());
    b.extend_from_slice(&0i32.to_le_bytes());
    // constants
    b.extend_from_slice(&(consts.len() as i32).to_le_bytes());
    for (s, num) in consts {
        if let Some(n) = num {
            b.push(3);
            b.extend_from_slice(&n.to_le_bytes());
        } else {
            b.push(4);
            b.extend_from_slice(&((s.len() + 1) as u32).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
            b.push(0);
        }
    }
    // protos (0)
    b.extend_from_slice(&0i32.to_le_bytes());
    // instructions
    b.extend_from_slice(&(insns.len() as i32).to_le_bytes());
    for ins in insns {
        b.extend_from_slice(&ins.to_le_bytes());
    }
    b
}

fn lua_ucfb(scripts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut payload = Vec::new();
    for (name, body) in scripts {
        let mut n = name.as_bytes().to_vec();
        n.push(0);
        let mut sc = chunk(b"NAME", &n);
        sc.extend(chunk(b"BODY", body));
        payload.extend(chunk(b"scr_", &sc));
    }
    let mut out = b"ucfb".to_vec();
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

fn op_abx(op: u32, a: u32, bx: u32) -> u32 {
    (op & 0x3F) | ((bx & 0x3FFFF) << 6) | ((a & 0xFF) << 24)
}

fn op_abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
    (op & 0x3F) | ((c & 0x1FF) << 6) | ((b & 0x1FF) << 15) | ((a & 0xFF) << 24)
}

#[test]
fn discovers_custom_eras_overrides_and_custom_team_names() {
    // Lua 5.0 opcodes: LOADK=1, LOADBOOL=2, GETGLOBAL=5, SETGLOBAL=7, SETTABLE=9, NEWTABLE=10, CALL=25, RETURN=27
    // RK constant encoding uses MAXSTACK=250 in mungefs::lua50::eval.
    const K: u32 = 250;

    // Build addme.script:
    // sp_missionselect_listbox_contents[1] = {
    //   mapluafile = "TST%s_%s",
    //   era_g = 1, mode_con_g = 1,
    //   era_y = 1, mode_con_y = 1,
    //   era_k = 1, mode_con_k = 1,
    //   change = { era_y = { name = "Vong War", icon2 = "custom_vong_icon" } }
    // }
    let addme_consts: Vec<(&str, Option<f64>)> = vec![
        ("sp_missionselect_listbox_contents", None), // 0
        ("mapluafile", None),                        // 1
        ("TST%s_%s", None),                          // 2
        ("era_g", None),                             // 3
        ("", Some(1.0)),                             // 4
        ("mode_con_g", None),                        // 5
        ("era_y", None),                             // 6
        ("mode_con_y", None),                        // 7
        ("era_k", None),                             // 8
        ("mode_con_k", None),                        // 9
        ("change", None),                            // 10
        ("name", None),                              // 11
        ("Vong War", None),                          // 12
        ("icon2", None),                             // 13
        ("custom_vong_icon", None),                  // 14
    ];
    let addme_insns = vec![
        op_abx(5, 0, 0),            // R0 = sp_missionselect_listbox_contents
        op_abc(10, 1, 0, 0),        // R1 = {} (entry)
        op_abc(9, 1, K + 1, K + 2), // R1.mapluafile = "TST%s_%s"
        op_abc(9, 1, K + 3, K + 4), // R1.era_g = 1
        op_abc(9, 1, K + 5, K + 4), // R1.mode_con_g = 1
        op_abc(9, 1, K + 6, K + 4), // R1.era_y = 1
        op_abc(9, 1, K + 7, K + 4), // R1.mode_con_y = 1
        op_abc(9, 1, K + 8, K + 4), // R1.era_k = 1
        op_abc(9, 1, K + 9, K + 4), // R1.mode_con_k = 1
        op_abc(10, 2, 0, 0),        // R2 = {} (change)
        op_abc(10, 3, 0, 0),        // R3 = {} (change.era_y)
        op_abc(9, 3, K + 11, K + 12), // R3.name = "Vong War"
        op_abc(9, 3, K + 13, K + 14), // R3.icon2 = "custom_vong_icon"
        op_abc(9, 2, K + 6, 3),     // R2.era_y = R3
        op_abc(9, 1, K + 10, 2),    // R1.change = R2
        op_abc(9, 0, K + 4, 1),     // R0[1] = R1
        op_abc(27, 0, 1, 0),        // RETURN
    ];
    let addme_bytes = lua_ucfb(&[("addme", lua50_chunk(&addme_consts, &addme_insns))]);

    // Build mission script:
    // SupportsCustomEraTeams = true (optional)
    // CustomEraTeam1 = "New Republic", CustomEraTeam2 = "Yuuzhan Vong" (optional)
    // ReadDataFile("dc:TST\\tst.lvl")
    // SetTeamName(1, "all"); SetTeamName(2, "imp")
    let make_mission = |custom_teams: bool| {
        let mut consts: Vec<(&str, Option<f64>)> = vec![
            ("ReadDataFile", None),           // 0
            ("dc:TST\\tst.lvl", None),        // 1
            ("SetTeamName", None),            // 2
            ("", Some(1.0)),                  // 3
            ("all", None),                    // 4
            ("", Some(2.0)),                  // 5
            ("imp", None),                    // 6
            ("SupportsCustomEraTeams", None), // 7
            ("CustomEraTeam1", None),         // 8
            ("New Republic", None),           // 9
            ("CustomEraTeam2", None),         // 10
            ("Yuuzhan Vong", None),           // 11
        ];
        let mut insns = Vec::new();
        if custom_teams {
            insns.push(op_abc(2, 0, 1, 0)); // R0 = true
            insns.push(op_abx(7, 0, 7));    // SupportsCustomEraTeams = true
            insns.push(op_abx(1, 0, 9));    // R0 = "New Republic"
            insns.push(op_abx(7, 0, 8));    // CustomEraTeam1 = R0
            insns.push(op_abx(1, 0, 11));   // R0 = "Yuuzhan Vong"
            insns.push(op_abx(7, 0, 10));   // CustomEraTeam2 = R0
        }
        // ReadDataFile("dc:TST\\tst.lvl")
        insns.push(op_abx(5, 0, 0));
        insns.push(op_abx(1, 1, 1));
        insns.push(op_abc(25, 0, 2, 1));
        // SetTeamName(1, "all")
        insns.push(op_abx(5, 0, 2));
        insns.push(op_abx(1, 1, 3));
        insns.push(op_abx(1, 2, 4));
        insns.push(op_abc(25, 0, 3, 1));
        // SetTeamName(2, "imp")
        insns.push(op_abx(5, 0, 2));
        insns.push(op_abx(1, 1, 5));
        insns.push(op_abx(1, 2, 6));
        insns.push(op_abc(25, 0, 3, 1));
        insns.push(op_abc(27, 0, 1, 0));
        consts.truncate(12);
        lua50_chunk(&consts, &insns)
    };
    let mission_lvl = lua_ucfb(&[
        ("tstg_con", make_mission(false)),
        ("tsty_con", make_mission(true)),
        ("tstk_con", make_mission(false)),
    ]);

    let dir = make_temp_dir("custom_eras");
    fs::create_dir_all(dir.join("data/_LVL_PC/TST")).unwrap();
    fs::write(dir.join("addme.script"), &addme_bytes).unwrap();
    fs::write(dir.join("data/_LVL_PC/mission.lvl"), &mission_lvl).unwrap();
    fs::write(dir.join("data/_LVL_PC/TST/tst.lvl"), b"ucfb\x00\x00\x00\x00").unwrap();

    let insp = inspect_addon_dir(&dir);
    assert_eq!(insp.missions.len(), 3, "errors={:?} warnings={:?}", insp.errors, insp.warnings);
    let by_name = |n: &str| insp.missions.iter().find(|m| m.script_name == n).unwrap();
    assert_eq!(by_name("tstg_con").era, "gcw");
    assert_eq!(by_name("tsty_con").era, "y");
    assert_eq!(
        by_name("tsty_con").custom_team_names,
        Some(["New Republic".to_owned(), "Yuuzhan Vong".to_owned()])
    );
    assert_eq!(by_name("tstk_con").era, "k");
    assert_eq!(by_name("tstk_con").custom_team_names, None);

    let groups = insp.map_groups("mod_tst");
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.modes_gcw, vec!["con".to_owned()]);
    assert_eq!(g.modes_custom_eras.get("y"), Some(&vec!["con".to_owned()]));
    assert_eq!(g.modes_custom_eras.get("k"), Some(&vec!["con".to_owned()]));
    let ov_y = g.era_overrides.get("y").expect("era_y override");
    assert_eq!(ov_y.name.as_deref(), Some("Vong War"));
    assert_eq!(ov_y.icon.as_deref(), Some("custom_vong_icon"));

    let ron_names: BTreeMap<&str, &str> = g
        .missions
        .iter()
        .map(|pm| (pm.mission.script_name.as_str(), pm.mission_ron.as_str()))
        .collect();
    assert_eq!(ron_names["tstg_con"], "mod_tst__tst_con.ron");
    assert_eq!(ron_names["tsty_con"], "mod_tst__tst_con_era_y.ron");
    assert_eq!(ron_names["tstk_con"], "mod_tst__tst_con_era_k.ron");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn game_folder_resolves_non_dc_custom_side_lvls_for_addon_missions() {
    const K: u32 = 250;
    let addme_consts: Vec<(&str, Option<f64>)> = vec![
        ("sp_missionselect_listbox_contents", None), // 0
        ("mapluafile", None),                        // 1
        ("TST%s_%s", None),                          // 2
        ("era_k", None),                             // 3
        ("", Some(1.0)),                             // 4
        ("mode_con_k", None),                        // 5
    ];
    let addme_insns = vec![
        op_abx(5, 0, 0),
        op_abc(10, 1, 0, 0),
        op_abc(9, 1, K + 1, K + 2),
        op_abc(9, 1, K + 3, K + 4),
        op_abc(9, 1, K + 5, K + 4),
        op_abc(9, 0, K + 4, 1),
        op_abc(27, 0, 1, 0),
    ];
    let addme_bytes = lua_ucfb(&[("addme", lua50_chunk(&addme_consts, &addme_insns))]);

    // Mission script calls ReadDataFile("SIDE\\rvs.lvl", "rvs_inf_rbasic") without "dc:"
    let mission_consts: Vec<(&str, Option<f64>)> = vec![
        ("ReadDataFile", None),    // 0
        ("dc:TST\\tst.lvl", None), // 1
        ("SIDE\\rvs.lvl", None),   // 2
        ("rvs_inf_rbasic", None),  // 3
        ("SetTeamName", None),     // 4
        ("", Some(1.0)),           // 5
        ("rep", None),             // 6
        ("", Some(2.0)),           // 7
        ("imp", None),             // 8
    ];
    let mission_insns = vec![
        op_abx(5, 0, 0),
        op_abx(1, 1, 1),
        op_abc(25, 0, 2, 1),
        op_abx(5, 0, 0),
        op_abx(1, 1, 2),
        op_abx(1, 2, 3),
        op_abc(25, 0, 3, 1),
        op_abx(5, 0, 4),
        op_abx(1, 1, 5),
        op_abx(1, 2, 6),
        op_abc(25, 0, 3, 1),
        op_abx(5, 0, 4),
        op_abx(1, 1, 7),
        op_abx(1, 2, 8),
        op_abc(25, 0, 3, 1),
        op_abc(27, 0, 1, 0),
    ];
    let mission_lvl = lua_ucfb(&[("tstk_con", lua50_chunk(&mission_consts, &mission_insns))]);

    let dir = make_temp_dir("game_folder_custom_side");
    let addon = dir.join("GameData/addon/TST");
    fs::create_dir_all(addon.join("data/_LVL_PC/TST")).unwrap();
    fs::create_dir_all(dir.join("GameData/DATA/_LVL_PC/SIDE")).unwrap();
    fs::write(addon.join("addme.script"), &addme_bytes).unwrap();
    fs::write(addon.join("data/_LVL_PC/mission.lvl"), &mission_lvl).unwrap();
    fs::write(addon.join("data/_LVL_PC/TST/tst.lvl"), b"ucfb\x00\x00\x00\x00").unwrap();
    fs::write(
        dir.join("GameData/DATA/_LVL_PC/SIDE/rvs.lvl"),
        b"ucfb\x00\x00\x00\x00",
    )
    .unwrap();

    let insp = inspect_addon_dir(&dir);
    assert_eq!(insp.missions.len(), 1);
    let m = &insp.missions[0];
    assert_eq!(m.era, "k");
    assert!(
        m.custom_side_lvls
            .iter()
            .any(|(s, p)| s == "rvs" && p.ends_with("rvs.lvl")),
        "expected rvs.lvl in custom_side_lvls, got {:?}",
        m.custom_side_lvls
    );
    assert!(
        !insp
            .stock_override_lvls
            .iter()
            .any(|o| o.rel_path.to_ascii_lowercase().ends_with("rvs.lvl")),
        "custom side rvs.lvl should not remain in stock_override_lvls"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A two-sided mission script reading each of `reads` as
/// `ReadDataFile(path, layer)`.
fn reading_script(reads: &[(&str, &str)]) -> Vec<u8> {
    // LOADK=1, GETGLOBAL=5, CALL=25, RETURN=27.
    let mut consts: Vec<(&str, Option<f64>)> = vec![("ReadDataFile", None)];
    let mut insns = Vec::new();
    for (path, layer) in reads {
        let at = consts.len() as u32;
        consts.extend([(*path, None), (*layer, None)]);
        insns.push(op_abx(5, 0, 0));
        insns.push(op_abx(1, 1, at));
        insns.push(op_abx(1, 2, at + 1));
        insns.push(op_abc(25, 0, 3, 1));
    }
    for (number, side) in [(1.0, "all"), (2.0, "imp")] {
        let at = consts.len() as u32;
        consts.extend([("SetTeamName", None), ("", Some(number)), (side, None)]);
        insns.push(op_abx(5, 0, at));
        insns.push(op_abx(1, 1, at + 1));
        insns.push(op_abx(1, 2, at + 2));
        insns.push(op_abc(25, 0, 3, 1));
    }
    insns.push(op_abc(27, 0, 1, 0));
    lua50_chunk(&consts, &insns)
}

fn inspect_stock_map_mod(name: &str, scripts: &[(&str, Vec<u8>)]) -> AddonInspection {
    let dir = make_temp_dir(name);
    fs::create_dir_all(dir.join("data/_LVL_PC/TST")).unwrap();
    fs::write(dir.join("addme.script"), lua_ucfb(&[])).unwrap();
    fs::write(dir.join("data/_LVL_PC/mission.lvl"), lua_ucfb(scripts)).unwrap();
    for lvl in ["sky.lvl", "obj.lvl"] {
        fs::write(dir.join("data/_LVL_PC/TST").join(lvl), b"ucfb\x00\x00\x00\x00").unwrap();
    }
    let inspection = inspect_addon_dir(&dir);
    let _ = fs::remove_dir_all(&dir);
    inspection
}

#[test]
fn a_mission_on_the_games_own_map_takes_that_map_with_the_mods_layers_on_it() {
    let sky = ("dc:TST\\sky.lvl", "cor1");
    let insp = inspect_stock_map_mod(
        "stock_map",
        &[
            (
                "cor1a_con",
                reading_script(&[("cor\\cor1.lvl", "cor1_Conquest"), sky]),
            ),
            (
                "cor1a_ass",
                reading_script(&[
                    ("cor\\cor1.lvl", "cor1_assault"),
                    ("dc:TST\\obj.lvl", "cor1_obj"),
                ]),
            ),
        ],
    );
    assert_eq!(insp.missions.len(), 2, "warnings={:?}", insp.warnings);
    for mission in &insp.missions {
        assert!(mission.supported, "{:?}", mission.unsupported_reason);
        assert_eq!(mission.map_lvl_rel.as_deref(), Some("cor/cor1.lvl"));
        let path = mission.map_lvl_path.as_deref().unwrap();
        assert_eq!(stock_world_rel(path), Some("cor/cor1.lvl"));
    }

    // Each mode layers the map differently, and both share the map's menu row.
    let groups = insp.map_groups("mod_tst");
    assert_eq!(groups.len(), 2);
    assert!(groups[0].variant_title.is_some());
    assert_eq!(groups[0].variant_title, groups[1].variant_title);
}

#[test]
fn two_scripts_offering_one_mode_keep_their_own_menu_rows() {
    let stock = ("cor\\cor1.lvl", "cor1_Conquest");
    let insp = inspect_stock_map_mod(
        "stock_map_rows",
        &[
            (
                "cor1a_con",
                reading_script(&[stock, ("dc:TST\\sky.lvl", "cor1")]),
            ),
            (
                "cor1b_con",
                reading_script(&[stock, ("dc:TST\\obj.lvl", "cor1_obj")]),
            ),
        ],
    );
    let groups = insp.map_groups("mod_tst");
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|g| g.variant_title.is_none()));
}

#[test]
fn modes_added_to_a_stock_entry_join_that_maps_menu_row() {
    // LOADK=1, GETGLOBAL=5, GETTABLE=6, SETTABLE=9, RETURN=27; RK constants
    // start at 250.
    const K: u32 = 250;
    let addme = lua50_chunk(
        &[
            ("sp_missionselect_listbox_contents", None), // 0
            ("", Some(1.0)),                             // 1
            ("era_x", None),                             // 2
            ("mode_con_x", None),                        // 3
        ],
        &[
            op_abx(5, 0, 0),            // R0 = the list
            op_abc(6, 1, 0, K + 1),     // R1 = R0[1], the stock Coruscant entry
            op_abc(9, 1, K + 2, K + 1), // R1.era_x = 1
            op_abc(9, 1, K + 3, K + 1), // R1.mode_con_x = 1
            op_abc(27, 0, 1, 0),
        ],
    );
    let parsed = parse_addme_script(&lua_ucfb(&[("addme", addme)])).unwrap();
    assert_eq!(
        parsed.script_eras.get("cor1x_con").map(String::as_str),
        Some("x")
    );
    assert_eq!(
        parsed.stock_row_scripts.get("cor1x_con").map(String::as_str),
        Some("cor1")
    );
    // The stock entries it left alone add nothing.
    assert_eq!(parsed.registered_scripts.len(), 1);
    assert!(parsed.legacy_ids.is_empty());
}
