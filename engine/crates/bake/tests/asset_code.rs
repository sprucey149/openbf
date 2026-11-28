//! A mod cache's asset folders outlive a converter whose `MOD_ASSET_ID` is
//! unchanged (`bake::mods::assets`). That id leaves out the mission and menu
//! logic (`extensions/build.rs`, `LOGIC`), so no code inside it may call that
//! logic: a change there would not rebuild what it made.

use std::path::Path;

/// How the logic left out of the id is named from outside it.
const LOGIC_PATHS: &[&str] = &[
    "lua50",
    "modscan",
    "mission_fold",
    "mungefs::addon",
    "super::convert",
    "mods::convert",
    "super::mission::",
    "mods::mission::",
];

/// The uses that make no asset: the file, and what its line holds.
const ALLOWED: &[(&str, &str)] = &[
    // The command line, starting a conversion.
    ("bake/src/main.rs", "bake::mods::convert_mod_to_cache("),
    ("bake/src/main.rs", "bake::mods::ConvertOptions"),
    // The converter's input, wrapped; and the two logic modules declared.
    ("bake/src/mods.rs", "AddonInspection"),
    ("bake/src/mods.rs", "modscan"),
    ("mungefs/src/lib.rs", "pub mod "),
    // Plain data the logic hands the world bake, and the size it reads up to.
    ("bake/src/mods/world.rs", "mungefs::addon::{AddonWorldLoad, MAX_LVL_BYTES}"),
    ("bake/src/mods/assets.rs", "mungefs::addon::AddonWorldLoad"),
];

/// The files the id leaves out, as `extensions/build.rs` lists them.
fn logic(crates: &Path) -> Vec<String> {
    let script = std::fs::read_to_string(crates.join("extensions/build.rs")).unwrap();
    let list = script.split("const LOGIC: &[&str] = &[").nth(1).unwrap();
    let list = list.split("];").next().unwrap();
    list.split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "tests") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|x| x == "rs")
            && path.file_name().is_some_and(|n| n != "tests.rs")
        {
            out.push(path);
        }
    }
}

#[test]
fn no_asset_code_calls_the_mission_logic() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let logic = logic(crates);
    assert!(logic.iter().any(|l| l == "bake/src/mods/convert.rs"));
    let mut files = Vec::new();
    for krate in ["bake", "audio", "mungefs", "manifest", "pak"] {
        sources(&crates.join(krate).join("src"), &mut files);
    }
    let mut found = Vec::new();
    for file in files {
        let rel = file.strip_prefix(crates).unwrap();
        let rel = rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
        let left_out = |l: &String| rel == *l || (l.ends_with('/') && rel.starts_with(l));
        if logic.iter().any(left_out) {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        // A file's own tests make no asset.
        let text = text.split("#[cfg(test)]\nmod tests").next().unwrap();
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap();
            if LOGIC_PATHS.iter().any(|p| code.contains(p))
                && !ALLOWED.iter().any(|(f, l)| rel == *f && code.contains(l))
            {
                found.push(format!("{rel}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "asset code names the mission logic; move what it uses out of it, \
         or take the file out of `LOGIC`:\n{}",
        found.join("\n")
    );
}
