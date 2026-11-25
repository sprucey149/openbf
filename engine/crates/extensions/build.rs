//! `MOD_CONVERTER_ID`: a hash of every source file `bake mod-convert` is
//! built from, so a mod cache written by any other converter reads as stale
//! with no number to bump by hand.

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
            h.update(r.as_bytes());
            h.update([0]);
            h.update((text.len() as u64).to_le_bytes());
            h.update(&text);
        }
    }
    let id: String = h.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect();
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("converter_id"), id).expect("write converter_id");
}
