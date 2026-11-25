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
    let abr = Path::new(r"E:\Projects\bf1+bf2\star_wars_battlefront2 modded\GameData\addon\ABR");
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
