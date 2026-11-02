//! Tier-1: the classic-plus texture tier resolves against the shipped bake
//! (data-gated on `engine/assets_baked`).
//!
//! Item 11's bake writes the PS2 disc's pixels as `<name>.ktx2` and the PC
//! install's override as `<name>.tier2.ktx2` beside it; this file pins what
//! `game::assets::load_texture` picks, and that the FILESYSTEM rule it uses
//! ("a `.tier2.ktx2` next to the base") agrees with the bake's own record in
//! `Manifest::texture_tier2`. If those two ever disagree the loader silently
//! renders the wrong tier, which no golden would catch — the goldens force
//! `Ps2`.

use game::assets::{
    baked_root, load_texture, set_texture_tier, CommonManifest, Manifest, TextureTier,
};
use std::path::Path;

/// `set_texture_tier` is process-wide, so every test that flips it runs under
/// this lock and puts the default back.
static TIER: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn manifest_of(dir: &Path) -> Option<Manifest> {
    ron::from_str(&std::fs::read_to_string(dir.join("manifest.ron")).ok()?).ok()
}

/// One baked directory that carries textures and its tier-2 record.
struct TexSet {
    key: String,
    dir: std::path::PathBuf,
    tier2: Vec<(String, String)>,
}

/// The pixel dimensions in a KTX2 header, without decoding a level.
fn ktx2_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let r = ktx2::Reader::new(&bytes[..]).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    let h = r.header();
    (h.pixel_width, h.pixel_height)
}

/// Every `.tier2.ktx2` in the bake is listed by its map's manifest, and every
/// listed one is on disk under the name the manifest gives. The loader probes
/// the filesystem; this is what says the probe and the record cannot drift.
#[test]
fn the_tier2_files_and_the_manifest_lists_agree() {
    let assets = baked_root();
    // Absent or an empty data link. Anything in it is a bake, held to the count below.
    if !std::fs::read_dir(&assets).is_ok_and(|mut d| d.next().is_some()) {
        eprintln!("SKIP: no assets_baked");
        return;
    }
    let mut checked = 0usize;
    let mut dirs = 0usize;
    // Every baked directory that carries textures: the maps, and the two
    // `common_bf*` bakes, whose 12 overrides no map manifest lists.
    let mut sets: Vec<TexSet> = Vec::new();
    for (key, _game, dir) in game::assets::discover_maps(&assets) {
        let Some(m) = manifest_of(&dir) else { continue };
        let tier2 = m.texture_tier2;
        sets.push(TexSet { key, dir, tier2 });
    }
    for key in ["common_bf1", "common_bf2"] {
        let dir = assets.join(key);
        let Some(c): Option<CommonManifest> = std::fs::read_to_string(dir.join("common.ron"))
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
        else {
            continue;
        };
        sets.push(TexSet {
            key: key.to_owned(),
            dir,
            tier2: c.texture_tier2,
        });
    }
    for TexSet { key, dir, tier2 } in &sets {
        let tex_dir = dir.join("textures");
        let listed: std::collections::BTreeMap<&str, &str> = tier2
            .iter()
            .map(|(n, f)| (n.as_str(), f.as_str()))
            .collect();
        for (name, file) in &listed {
            assert_eq!(
                *file,
                format!("{name}.tier2.ktx2"),
                "{key}: manifest names {file} for {name}"
            );
            assert!(
                tex_dir.join(file).is_file(),
                "{key}: manifest lists {file}, which is not baked"
            );
            assert!(
                tex_dir.join(format!("{name}.ktx2")).is_file(),
                "{key}: {name} has a tier 2 but no PS2 base — the loader would \
                 fall back to a file that is not there"
            );
            checked += 1;
        }
        let mut on_disk = 0usize;
        for e in std::fs::read_dir(&tex_dir).into_iter().flatten().flatten() {
            let f = e.file_name();
            let Some(f) = f.to_str() else { continue };
            let Some(stem) = f.strip_suffix(".tier2.ktx2") else {
                continue;
            };
            assert!(
                listed.contains_key(stem),
                "{key}: {f} is baked but the manifest does not list it"
            );
            on_disk += 1;
        }
        assert_eq!(on_disk, listed.len(), "{key}: tier 2 count");
        dirs += 1;
    }
    assert!(dirs > 0, "no baked directory had a manifest");
    eprintln!("{checked} tier-2 overrides over {dirs} baked directories");
}

/// `Pc` takes the override where one exists and the PS2 base where none does;
/// `Ps2` always takes the base. Measured against the KTX2 headers, so a
/// wrong pick cannot hide behind equal-looking pixels.
#[test]
fn the_tier_decides_which_file_load_texture_reads() {
    let _lock = TIER.lock().unwrap_or_else(|e| e.into_inner());
    let assets = baked_root();
    // bf2_cor1 is the port's densest tier-2 map (52 overrides).
    let dir = assets.join("bf2_cor1");
    let Some(m) = manifest_of(&dir) else {
        eprintln!("SKIP: no baked bf2_cor1");
        return;
    };
    let tex = dir.join("textures");
    let Some((name, file)) = m.texture_tier2.first() else {
        eprintln!("SKIP: bf2_cor1 baked no tier 2 (no PC install at bake time)");
        return;
    };
    let base_size = ktx2_size(&tex.join(format!("{name}.ktx2")));
    let over_size = ktx2_size(&tex.join(file));

    set_texture_tier(TextureTier::Ps2);
    let ps2 = load_texture(&tex, name).expect("ps2 tier loads");
    assert_eq!((ps2.width, ps2.height), base_size, "{name} at Ps2");

    set_texture_tier(TextureTier::Pc);
    let pc = load_texture(&tex, name).expect("pc tier loads");
    assert_eq!((pc.width, pc.height), over_size, "{name} at Pc");

    // A texture with no override reads the same file at either tier. Taking
    // it from the SAME directory keeps the case honest: the fallback is what
    // 90 % of a tier-2 map's textures take.
    let plain = m
        .textures
        .iter()
        .find(|n| !tex.join(format!("{n}.tier2.ktx2")).exists())
        .expect("bf2_cor1 has a texture with no override");
    set_texture_tier(TextureTier::Ps2);
    let a = load_texture(&tex, plain).expect("no-override at Ps2");
    set_texture_tier(TextureTier::Pc);
    let b = load_texture(&tex, plain).expect("no-override at Pc");
    assert_eq!((a.width, a.height), (b.width, b.height), "{plain} size");
    assert_eq!(a.rgba, b.rgba, "{plain} pixels");

    set_texture_tier(TextureTier::default());
}

/// The detail companion is the PS2's own at BOTH tiers — it is a modulated x2
/// overlay the disc authored, not a resolution tier, and the bake writes no
/// `.tier2` for it.
#[test]
fn the_detail_companion_does_not_follow_the_tier() {
    let _lock = TIER.lock().unwrap_or_else(|e| e.into_inner());
    let assets = baked_root();
    let dir = assets.join("tat1").join("textures");
    if !dir.join("tat_skiff01.ktx2").is_file() {
        eprintln!("SKIP: no baked tat1");
        return;
    }
    for tier in [TextureTier::Ps2, TextureTier::Pc] {
        set_texture_tier(tier);
        let t = load_texture(&dir, "tat_skiff01").expect("skiff loads");
        let d = t
            .detail
            .as_ref()
            .unwrap_or_else(|| panic!("{tier:?}: detail"));
        assert_eq!(
            (d.width, d.height),
            ktx2_size(&dir.join("tat_skiff01.dtl.ktx2")),
            "{tier:?}"
        );
    }
    set_texture_tier(TextureTier::default());
}

// ---------------------------------------------------------------------------
// The SIDE bake (soldiers and weapons) — item T2C.
//
// T2S's report §2 answered "does a soldier pick up tier 2?" by READING the
// call chain; these two tests answer it by running it. The side group is the
// one texture set the agreement test above cannot see: the side bake writes no
// `manifest.ron` and no `common.ron` (T2S report — the only RON it writes is
// `soldiers/*.anims.ron`, clip metadata), so `discover_maps` never returns it
// and there is no recorded tier-2 list to check the files against. What stands
// in for the manifest here is the runtime's own question: the names the
// soldier/weapon path asks `load_texture` for come from the GLB materials
// (the two `upload_texture_file` calls in `soldier_vis.rs`, `gallery.rs:307`),
// lowercased.

/// Every `<stem>.tier2.ktx2` under `dir`, sorted.
fn tier2_stems(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let f = e.file_name();
            let f = f.to_str()?;
            Some(f.strip_suffix(".tier2.ktx2")?.to_owned())
        })
        .collect();
    out.sort();
    out
}

/// Every texture name the soldier/weapon path would ask for under `side`:
/// the skinned materials of `soldiers/*.glb` and the static materials of
/// `weapons/*.glb`, lowercased exactly as the two call sites lowercase them,
/// and with `overrides` the class skins `soldiers/*.skins.ron` name (card USKN).
fn side_material_names(side: &Path, overrides: bool) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    for e in std::fs::read_dir(side.join("soldiers"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let p = e.path();
        // A class's override texture (card USKN) is asked for by name.
        if p.to_string_lossy().ends_with(".skins.ron") {
            if overrides {
                let skins: manifest::SoldierSkins =
                    ron::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                names.extend(skins.overrides);
            }
            continue;
        }
        if p.extension().is_none_or(|x| x != "glb") {
            continue;
        }
        let a =
            game::assets::load_skinned_glb(&p).unwrap_or_else(|e| panic!("{}: {e:#}", p.display()));
        names.extend(
            a.mesh
                .submeshes
                .iter()
                .map(|(_, _, m, _)| m.to_ascii_lowercase()),
        );
    }
    for e in std::fs::read_dir(side.join("weapons"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "glb") {
            continue;
        }
        let m = game::assets::load_glb(&p).unwrap_or_else(|e| panic!("{}: {e:#}", p.display()));
        names.extend(
            m.submeshes
                .iter()
                .map(|(_, _, n, _, _)| n.to_ascii_lowercase()),
        );
    }
    names
}

/// The side group's stand-in for the manifest agreement test above: every
/// tier-2 file the side bake wrote is a texture some soldier or weapon GLB
/// names, and every one of them has the PS2 base that `upload_texture_file`
/// gates on (`viewer::upload_texture_file` returns `None` — an untextured soldier — when
/// `<name>.ktx2` is missing, whatever the tier).
///
/// How many overrides there are depends on which discs went in and on how old
/// the bake is, so the counts below are floors: a bake with more donors has
/// more, and the older bake's two sides without donors have none. The donors
/// themselves are wired up in `scripts/rebake.sh`'s `side_donors`, which
/// `bake/tests/suite/side_steps.rs` holds to its list.
#[test]
fn the_side_tier2_files_are_textures_the_soldier_path_asks_for() {
    let assets = baked_root();
    let side = assets.join("side_bf2");
    if !side.join("textures").is_dir() {
        eprintln!("SKIP: no baked side_bf2");
        return;
    }

    // The agreement test above cannot reach this directory: it walks
    // `discover_maps` (a `manifest.ron`) plus the two `common_bf*`
    // (a `common.ron`), and the side bake writes neither.
    assert!(
        !side.join("manifest.ron").exists() && !side.join("common.ron").exists(),
        "side_bf2 grew a manifest — fold it into the agreement test above \
         instead of keeping this stand-in"
    );
    assert!(
        !game::assets::discover_maps(&assets)
            .iter()
            .any(|(k, _, _)| k == "side_bf2"),
        "discover_maps returned side_bf2"
    );

    let stems = tier2_stems(&side.join("textures"));
    if stems.is_empty() {
        eprintln!("SKIP: side_bf2 baked no tier 2 (no PC install at bake time)");
        return;
    }
    // Pinned: the PC install's `side/rep.lvl` + `side/cis.lvl` gave 21 of the
    // 26 wanted textures (T2S report, measured on the shipped tree); card
    // HUDW's REP and CIS pistol sheets added two. Card USKN's class skins
    // add six (the battle droid's blue / camo / green / orange / red, the
    // sharpshooter's red) once the side is baked with them.
    let skins: Vec<String> = std::fs::read_dir(side.join("soldiers"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".skins.ron"))
        .flat_map(|p| {
            let s: manifest::SoldierSkins =
                ron::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            s.overrides
        })
        .collect();
    // An override that is also a model's own sheet (the ARC trooper's
    // `rep_inf_arc_blue`, a pair that changes nothing) is not a new stem.
    let own = side_material_names(&side, false);
    let skin_stems = stems
        .iter()
        .filter(|s| skins.contains(s) && !own.contains(*s))
        .count();
    // PR #3 adds REP/CIS fusion cutters with their two PC first-person sheets.
    for cutter in [
        "rep_1st_weap_inf_fusioncutter",
        "cis_1st_weap_inf_fusioncutter",
    ] {
        assert!(
            stems.iter().any(|stem| stem == cutter),
            "missing cutter sheet {cutter}"
        );
    }
    // REP and CIS's PC donors alone gave 25 and 6 (measured on the bake of
    // 2026-10-30). ALL and IMP's bring those to 53 and 11 (largest copy wins,
    // measured on a whole `rebake.sh side` with a PC install and no Xbox disc).
    // An ALL or IMP override marks the newer bake, which is then held to its
    // own floor, so it cannot quietly lose the donors of another side.
    let (own_floor, skin_floor) = if stems
        .iter()
        .any(|s| s.starts_with("all_") || s.starts_with("imp_"))
    {
        (53, 11)
    } else {
        (25, 6)
    };
    assert!(
        stems.len() - skin_stems >= own_floor,
        "side_bf2 has {} tier-2 stems, under {own_floor}: {stems:?}",
        stems.len() - skin_stems
    );
    if !skins.is_empty() {
        assert!(
            skin_stems >= skin_floor,
            "side_bf2 has {skin_stems} class-skin tier-2 stems, under {skin_floor}: {stems:?}"
        );
    }

    let bf1 = assets.join("side_bf1");
    for (key, dir) in [("side_bf2", &side), ("side_bf1", &bf1)] {
        let stems = tier2_stems(&dir.join("textures"));
        let names = side_material_names(dir, true);
        for stem in &stems {
            assert!(
                names.contains(stem),
                "{key}: {stem}.tier2.ktx2 is baked but no soldier or weapon GLB names it \
                 — nothing would ever load it"
            );
            assert!(
                dir.join("textures").join(format!("{stem}.ktx2")).is_file(),
                "{key}: {stem} has a tier 2 but no PS2 base — upload_texture_file would \
                 return None and the mesh would draw untextured"
            );
        }
        eprintln!(
            "{key}: {} tier-2 overrides, all named by one of {} GLB materials",
            stems.len(),
            names.len()
        );
    }
}

/// The runtime answer to T2S §2: the tier switch lives inside `load_texture`
/// itself (`assets::load_texture`), which is the one decode
/// `viewer::upload_texture_file` performs, so the soldier and weapon path gets
/// tier 2 with no call-site change. Measured on the KTX2 headers of all 21
/// overrides — every one is strictly larger than its base in both dimensions,
/// so the header alone says which file was read.
#[test]
fn the_tier_decides_which_file_the_soldier_path_reads() {
    let _lock = TIER.lock().unwrap_or_else(|e| e.into_inner());
    let tex = baked_root().join("side_bf2").join("textures");
    if !tex.is_dir() {
        eprintln!("SKIP: no baked side_bf2");
        return;
    }
    let stems = tier2_stems(&tex);
    if stems.is_empty() {
        eprintln!("SKIP: side_bf2 baked no tier 2 (no PC install at bake time)");
        return;
    }
    for stem in &stems {
        let base = ktx2_size(&tex.join(format!("{stem}.ktx2")));
        let over = ktx2_size(&tex.join(format!("{stem}.tier2.ktx2")));
        assert!(
            over.0 > base.0 && over.1 > base.1,
            "{stem}: tier 2 {over:?} is not larger than the base {base:?} — \
             the header can no longer tell the two files apart"
        );

        set_texture_tier(TextureTier::Ps2);
        let ps2 = load_texture(&tex, stem).expect("side texture at Ps2");
        assert_eq!((ps2.width, ps2.height), base, "{stem} at Ps2");

        set_texture_tier(TextureTier::Pc);
        let pc = load_texture(&tex, stem).expect("side texture at Pc");
        assert_eq!((pc.width, pc.height), over, "{stem} at Pc");
    }
    // A side texture with NO override is the disc's at either tier — the
    // fallback 35 of the 56 shipped `side_bf2` textures take (56 bases, 21
    // of them overridden; the directory's other 21 files are the overrides).
    let plain = std::fs::read_dir(&tex)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let f = e.file_name();
            let f = f.to_str()?;
            let s = f.strip_suffix(".ktx2")?;
            (!s.ends_with(".tier2") && !stems.iter().any(|t| t == s)).then(|| s.to_owned())
        })
        .next()
        .expect("side_bf2 has a texture with no override");
    set_texture_tier(TextureTier::Ps2);
    let a = load_texture(&tex, &plain).expect("no-override at Ps2");
    set_texture_tier(TextureTier::Pc);
    let b = load_texture(&tex, &plain).expect("no-override at Pc");
    assert_eq!((a.width, a.height), (b.width, b.height), "{plain} size");
    assert_eq!(a.rgba, b.rgba, "{plain} pixels");

    set_texture_tier(TextureTier::default());
    eprintln!(
        "side_bf2: {} overrides read at Pc, bases at Ps2",
        stems.len()
    );
}
