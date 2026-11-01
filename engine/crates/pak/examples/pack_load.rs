//! The pack layouts and load cost, on the production `pak` code (card PACK). No game data is written outside `<scratch>`.
//!
//! ```text
//! pack_load write <out> [--pak <file>] [--split-at <bytes>] <name>=<dir>...   pack with this executable as the game
//! pack_load read <name>/<rel>                                                 print one entry the way the game reads it
//! pack_load bench <dir> <scratch> [rounds]                                    time reading every file of <dir>
//! ```
//!
//! `bench` packs `<dir>` three ways (raw, the legacy raw Deflate and the current zstd, whose pack is the one
//! `write_pack` makes), then rotates between them reading all the entries, so a warm cache favours none. Run it in
//! release (`cargo run --release -p pak --example pack_load -- bench engine/assets_baked/tat1 <scratch>`). It times
//! reading and decompressing, not building the scene or uploading to the GPU.
//!
//! Prints `min / median / max` milliseconds per round for each arm, and the stored size.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("write") if args.len() >= 2 => write(&args[1], &args[2..]),
        Some("read") if args.len() == 2 => read(&args[1]),
        Some("bench") if args.len() >= 3 => {
            let rounds = args.get(3).map_or(11, |r| r.parse().expect("rounds"));
            bench(Path::new(&args[1]), Path::new(&args[2]), rounds)
        }
        _ => {
            eprintln!("usage: pack_load write|read|bench (see the head of this file)");
            std::process::exit(2);
        }
    }
}

fn write(out: &str, rest: &[String]) -> std::io::Result<()> {
    let out = PathBuf::from(out);
    let mut pak_file = out.with_file_name(pak::PAK_NAME);
    let mut split_at = pak::SINGLE_FILE_LIMIT;
    let mut entries = Vec::new();
    let mut it = rest.iter();
    while let Some(spec) = it.next() {
        match spec.as_str() {
            "--pak" => pak_file = PathBuf::from(it.next().expect("--pak <file>")),
            "--split-at" => split_at = it.next().expect("--split-at <n>").parse().expect("bytes"),
            _ => {
                let (name, dir) = spec.split_once('=').expect("<name>=<dir>");
                entries.extend(pak::collect(Path::new(dir), name)?);
            }
        }
    }
    let exe = std::env::current_exe()?;
    println!(
        "{:?}",
        pak::write_pack_at(&exe, &out, &pak_file, &entries, split_at)?
    );
    Ok(())
}

/// What the game does: mount a directory that does not exist, then read through the mount.
fn read(key: &str) -> std::io::Result<()> {
    let (name, rel) = key.split_once('/').expect("<name>/<rel>");
    let mount = Path::new("pack_load_no_such_directory");
    if !pak::mount(mount, name) {
        eprintln!("no archive carries {name}");
        std::process::exit(3);
    }
    std::io::stdout().write_all(&pak::read(mount.join(rel))?)
}

/// A pack of `files` with `body` choosing each stored body: the on-disk format without the production writer.
fn fixed_pack(
    out: &Path,
    files: &[(String, Vec<u8>)],
    body: impl Fn(&[u8]) -> Option<(Vec<u8>, u64)>,
) -> std::io::Result<u64> {
    let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
    let mut pos = 0u64;
    let mut index = (files.len() as u32).to_le_bytes().to_vec();
    for (name, raw) in files {
        let eligible = raw.len() >= 128
            && [".ktx2", ".glb", ".ron", ".wav"]
                .iter()
                .any(|ext| name.ends_with(ext));
        let packed = body(raw).filter(|(b, _)| eligible && b.len() + 64 < raw.len());
        let (stored, flags): (&[u8], u64) = match &packed {
            Some((b, flags)) => (b, *flags),
            None => (raw, 0),
        };
        w.write_all(stored)?;
        index.extend((name.len() as u16).to_le_bytes());
        index.extend(name.as_bytes());
        index.extend(pos.to_le_bytes());
        index.extend((stored.len() as u64 | flags).to_le_bytes());
        pos += stored.len() as u64;
    }
    w.write_all(&index)?;
    w.write_all(b"OPENBFPK")?;
    w.write_all(&0u64.to_le_bytes())?;
    w.write_all(&pos.to_le_bytes())?;
    w.write_all(&(index.len() as u64).to_le_bytes())?;
    w.flush()?;
    Ok(pos + index.len() as u64 + 32)
}

fn deflate(raw: &[u8]) -> Option<(Vec<u8>, u64)> {
    let mut out = (raw.len() as u32).to_le_bytes().to_vec();
    let mut enc = flate2::write::DeflateEncoder::new(&mut out, flate2::Compression::fast());
    enc.write_all(raw).ok()?;
    enc.finish().ok()?;
    Some((out, 1 << 63))
}

fn bench(dir: &Path, scratch: &Path, rounds: usize) -> std::io::Result<()> {
    std::fs::create_dir_all(scratch)?;
    let entries = pak::collect(dir, "d")?;
    let mut files = Vec::with_capacity(entries.len());
    for (name, path) in &entries {
        files.push((name.clone(), std::fs::read(path)?));
    }
    let total: usize = files.iter().map(|(_, b)| b.len()).sum();
    let (raw, legacy) = (scratch.join("raw.pack"), scratch.join("deflate.pack"));
    let (stub, zstd) = (scratch.join("stub"), scratch.join("zstd.pack"));
    fixed_pack(&raw, &files, |_| None)?;
    fixed_pack(&legacy, &files, deflate)?;
    std::fs::write(&stub, b"")?;
    let side = scratch.join("zstd.unused");
    let layout = pak::write_pack_at(&stub, &zstd, &side, &entries, pak::SINGLE_FILE_LIMIT)?;
    assert_eq!(layout, pak::PackLayout::SingleFile);
    let mut arms: Vec<_> = [("raw", &raw), ("deflate", &legacy), ("zstd", &zstd)]
        .into_iter()
        .map(|(label, path)| {
            let archive = pak::Archive::open(path)?.expect("a pack");
            for (name, body) in &files {
                assert_eq!(&archive.read_entry(name)?, body, "{label} {name}");
            }
            let stored = std::fs::metadata(path)?.len();
            std::io::Result::Ok((label, stored, archive, Vec::<f64>::new()))
        })
        .collect::<Result<_, _>>()?;
    for round in 0..rounds {
        for step in 0..arms.len() {
            let n = arms.len();
            let (_, _, archive, samples) = &mut arms[(round + step) % n];
            let start = Instant::now();
            let mut read = 0;
            for (name, _) in &files {
                read += archive.read_entry(name)?.len();
            }
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(read, total);
        }
    }
    println!(
        "{} files, {total} bytes, {rounds} rotating warm rounds",
        files.len()
    );
    for (label, stored, _, samples) in &mut arms {
        samples.sort_by(f64::total_cmp);
        println!(
            "{label:8} stored {stored:>12}  min {:9.3} ms  median {:9.3} ms  max {:9.3} ms",
            samples[0],
            samples[samples.len() / 2],
            samples[samples.len() - 1]
        );
    }
    for path in [raw, legacy, stub, zstd] {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
