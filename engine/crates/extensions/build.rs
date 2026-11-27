//! `MOD_CONVERTER_ID`: a hash of every source file `bake mod-convert` is
//! built from, so a mod cache written by any other converter reads as stale
//! with no number to bump by hand. `MOD_ASSET_ID`: the same without the
//! mission and menu logic ([`LOGIC`]), so a change there keeps the baked
//! assets of the stale cache (`bake::mods::assets`).

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The crates `bake` links (its path dependencies, transitively) and the
/// folders of each that reach the binary.
const CONVERTER: &[(&str, &[&str])] = &[
    ("bake", &["src"]),
    ("audio", &["src"]),
    ("extensions", &["src"]),
    ("mungefs", &["src"]),
    ("manifest", &["src", "data"]),
    ("pak", &["src"]),
];

/// The converter's mission and menu logic, below `crates/`: it chooses what
/// to bake and writes the missions and cache metadata, but makes no byte of a
/// baked asset (`bake/tests/asset_code.rs` checks that no asset code calls
/// it). What it chooses is part of each asset step's key.
const LOGIC: &[&str] = &[
    "bake/src/mods/convert.rs",
    "bake/src/mods/mission.rs",
    "extensions/src/mods.rs",
    "mungefs/src/addon.rs",
    "mungefs/src/addon/",
    "mungefs/src/lua50.rs",
    "mungefs/src/lua50/",
    "mungefs/src/modscan.rs",
];

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if p.is_dir() {
            if name != "tests" {
                files(&p, out);
            }
        } else if name != "tests.rs" {
            out.push(p);
        }
    }
}

fn main() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .to_path_buf();
    let mut h = Sha256::new();
    let mut assets = Sha256::new();
    for (krate, dirs) in CONVERTER {
        let root = crates.join(krate);
        let mut list = vec![root.join("Cargo.toml")];
        for d in *dirs {
            let dir = root.join(d);
            println!("cargo:rerun-if-changed={}", dir.display());
            files(&dir, &mut list);
        }
        println!("cargo:rerun-if-changed={}", root.join("Cargo.toml").display());
        let mut rel: Vec<(String, PathBuf)> = list
            .into_iter()
            .map(|p| {
                let r = p
                    .strip_prefix(&crates)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                (r, p)
            })
            .collect();
        rel.sort();
        for (r, p) in rel {
            let Ok(bytes) = std::fs::read(&p) else {
                continue;
            };
            let text: Vec<u8> = bytes.into_iter().filter(|&b| b != b'\r').collect();
            let logic = LOGIC.iter().any(|l| {
                if l.ends_with('/') {
                    r.starts_with(l)
                } else {
                    r == *l
                }
            });
            for h in std::iter::once(&mut h).chain((!logic).then_some(&mut assets)) {
                h.update(r.as_bytes());
                h.update([0]);
                h.update((text.len() as u64).to_le_bytes());
                h.update(&text);
            }
        }
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    for (name, h) in [("converter_id", h), ("asset_id", assets)] {
        let id: String = h.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect();
        std::fs::write(out.join(name), id).expect("write the id");
    }
}
