use super::*;

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pak_test_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A fake "executable" plus three files, packed; returns (dir, packed file).
fn packed(tag: &str) -> (PathBuf, PathBuf) {
    let d = scratch(tag);
    std::fs::write(d.join("exe"), b"\x7fELF not really an executable").unwrap();
    std::fs::write(d.join("a"), b"alpha").unwrap();
    std::fs::write(d.join("b"), b"").unwrap();
    std::fs::write(d.join("c"), (0..=255u8).collect::<Vec<_>>()).unwrap();
    let entries = vec![
        ("assets_baked/m1/manifest.ron".to_string(), d.join("a")),
        ("assets_baked/m1/sounds/empty.ogg".to_string(), d.join("b")),
        ("assets_baked/audio/x.bin".to_string(), d.join("c")),
    ];
    write_pack(&d.join("exe"), &d.join("packed"), &entries).unwrap();
    let p = d.join("packed");
    (d, p)
}

#[test]
fn round_trip() {
    let (d, p) = packed("rt");
    let a = Archive::open(&p).unwrap().expect("a pack");
    assert_eq!(a.len(), 3);
    assert_eq!(a.start(), std::fs::metadata(d.join("exe")).unwrap().len());
    let (off, len) = a.get("assets_baked/m1/manifest.ron").unwrap();
    assert_eq!(a.read(off, len).unwrap(), b"alpha");
    assert_eq!(a.get("assets_baked/m1/sounds/empty.ogg").unwrap().1, 0);
    assert!(
        a.has_dir("assets_baked/m1") && a.has_dir("assets_baked") && !a.has_dir("assets_baked/m")
    );
    assert_eq!(a.children("assets_baked"), ["audio", "m1"]);
    assert_eq!(a.children("assets_baked/m1"), ["manifest.ron", "sounds"]);
    // The executable proper comes back byte for byte.
    let bytes = std::fs::read(&p).unwrap();
    assert_eq!(
        &bytes[..a.start() as usize],
        &std::fs::read(d.join("exe")).unwrap()[..]
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn repack_keeps_only_the_executable() {
    let (d, p) = packed("repack");
    std::fs::write(d.join("z"), b"zulu").unwrap();
    write_pack(
        &p,
        &d.join("again"),
        &[("missions/z.ron".to_string(), d.join("z"))],
    )
    .unwrap();
    let a = Archive::open(&d.join("again")).unwrap().unwrap();
    assert_eq!(a.names().collect::<Vec<_>>(), ["missions/z.ron"]);
    assert_eq!(a.start(), std::fs::metadata(d.join("exe")).unwrap().len());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_plain_file_has_no_pack_and_a_bad_footer_is_an_error() {
    let (d, p) = packed("plain");
    assert!(Archive::open(&d.join("exe")).unwrap().is_none());
    let mut bytes = std::fs::read(&p).unwrap();
    bytes.insert(10, 0); // every offset is now one off
    std::fs::write(d.join("broken"), bytes).unwrap();
    assert!(Archive::open(&d.join("broken")).is_err());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn reader_seeks_inside_one_entry() {
    let (d, p) = packed("reader");
    let a = Archive::open(&p).unwrap().unwrap();
    let (start, len) = a.get("assets_baked/audio/x.bin").unwrap();
    let mut r = Reader::Pak {
        file: File::open(&p).unwrap(),
        start,
        len,
        pos: 0,
    };
    let mut b = [0u8; 4];
    r.seek(SeekFrom::Start(100)).unwrap();
    r.read_exact(&mut b).unwrap();
    assert_eq!(b, [100, 101, 102, 103]);
    r.seek(SeekFrom::End(-2)).unwrap();
    let mut rest = Vec::new();
    r.read_to_end(&mut rest).unwrap();
    assert_eq!(
        rest,
        [254, 255],
        "a read stops at the entry's end, not the file's"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn normalize_is_lexical() {
    assert_eq!(
        normalize(Path::new("assets_baked/bf1_kam1/../missions/./x")),
        Path::new("assets_baked/missions/x")
    );
    assert_eq!(
        normalize(Path::new("assets_baked/../missions")),
        Path::new("missions")
    );
    assert_eq!(normalize(Path::new("../a")), Path::new("../a"));
}

#[test]
fn unmounted_paths_read_the_disk() {
    let d = scratch("disk");
    std::fs::create_dir_all(d.join("q/r")).unwrap();
    std::fs::write(d.join("q/b.txt"), "b").unwrap();
    std::fs::write(d.join("q/a.txt"), "a").unwrap();
    assert_eq!(read_to_string(d.join("q/a.txt")).unwrap(), "a");
    assert!(is_dir(d.join("q")) && is_file(d.join("q/a.txt")) && !exists(d.join("q/none")));
    assert_eq!(
        read_dir(d.join("q")).unwrap(),
        [d.join("q/a.txt"), d.join("q/b.txt"), d.join("q/r")]
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn identical_files_are_stored_once() {
    let d = scratch("dedupe");
    std::fs::write(d.join("exe"), b"exe").unwrap();
    std::fs::write(d.join("m1"), vec![7u8; 5000]).unwrap();
    std::fs::write(d.join("m2"), vec![7u8; 5000]).unwrap();
    let mut other = vec![7u8; 5000];
    other[4999] = 8;
    std::fs::write(d.join("m3"), other).unwrap();
    let entries = vec![
        ("a/kas1/music.bin".to_string(), d.join("m1")),
        ("a/kas2/music.bin".to_string(), d.join("m2")),
        ("a/x/music.bin".to_string(), d.join("m3")),
    ];
    write_pack(&d.join("exe"), &d.join("p"), &entries).unwrap();
    let a = Archive::open(&d.join("p")).unwrap().unwrap();
    assert_eq!(a.get("a/kas1/music.bin"), a.get("a/kas2/music.bin"));
    assert_ne!(
        a.get("a/kas1/music.bin").unwrap().0,
        a.get("a/x/music.bin").unwrap().0
    );
    let (off, len) = a.get("a/x/music.bin").unwrap();
    assert_eq!(a.read(off, len).unwrap()[4999], 8);
    // exe + two bodies + index + footer: the copy was not written.
    let index = 4 + 3 * (2 + 8 + 8) + entries.iter().map(|e| e.0.len()).sum::<usize>();
    assert_eq!(
        std::fs::metadata(d.join("p")).unwrap().len() as usize,
        3 + 2 * 5000 + index + 32
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn hostile_ranges_are_invalid_data() {
    let d = scratch("hostile_ranges");
    let path = d.join("pack");
    let footer = |start: u64, at: u64, len: u64| {
        let mut bytes = MAGIC.to_vec();
        bytes.extend(start.to_le_bytes());
        bytes.extend(at.to_le_bytes());
        bytes.extend(len.to_le_bytes());
        bytes
    };
    for (at, len) in [(u64::MAX, 1), (0, u64::MAX)] {
        std::fs::write(&path, footer(0, at, len)).unwrap();
        assert_eq!(
            Archive::open(&path).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
    }
    for (off, len) in [(u64::MAX, 1u64), (0, 1)] {
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend(1u16.to_le_bytes());
        bytes.push(b'a');
        bytes.extend(off.to_le_bytes());
        bytes.extend(len.to_le_bytes());
        bytes.extend(footer(0, 0, 23));
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            Archive::open(&path).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
    }
    // Empty entries may sit exactly at the index boundary.
    let mut bytes = 1u32.to_le_bytes().to_vec();
    bytes.extend(1u16.to_le_bytes());
    bytes.push(b'a');
    bytes.extend(0u64.to_le_bytes());
    bytes.extend(0u64.to_le_bytes());
    bytes.extend(footer(0, 0, 23));
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        Archive::open(&path).unwrap().unwrap().get("a"),
        Some((0, 0))
    );
    bytes.remove(22);
    bytes.truncate(22);
    bytes.extend(footer(0, 0, 22));
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        Archive::open(&path).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn reader_seek_overflow_retains_position() {
    let (d, p) = packed("seek_overflow");
    let mut r = Reader::Pak {
        file: File::open(p).unwrap(),
        start: 0,
        len: 1,
        pos: 0,
    };
    assert_eq!(r.seek(SeekFrom::Start(u64::MAX)).unwrap(), u64::MAX);
    assert_eq!(
        r.seek(SeekFrom::Current(1)).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(r.stream_position().unwrap(), u64::MAX);
    assert_eq!(r.seek(SeekFrom::Current(-1)).unwrap(), u64::MAX - 1);
    assert_eq!(r.read(&mut [0; 1]).unwrap(), 0);
    r.seek(SeekFrom::Start(0)).unwrap();
    assert_eq!(
        r.seek(SeekFrom::Current(-1)).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(r.stream_position().unwrap(), 0);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn compressible_entries_shrink_and_round_trip_identically() {
    let d = scratch("compress");
    std::fs::write(d.join("exe"), b"exe").unwrap();
    let raw: Vec<u8> = (0..8192u32).flat_map(|i| (i % 7).to_le_bytes()).collect();
    std::fs::write(d.join("tex.ktx2"), &raw).unwrap();
    let entries = vec![(
        "assets_baked/m/textures/t.ktx2".to_string(),
        d.join("tex.ktx2"),
    )];
    write_pack(&d.join("exe"), &d.join("p"), &entries).unwrap();
    let packed_len = std::fs::metadata(d.join("p")).unwrap().len() as usize;
    assert!(packed_len < raw.len() / 2);
    let a = Archive::open(&d.join("p")).unwrap().unwrap();
    let (off, len) = a.get("assets_baked/m/textures/t.ktx2").unwrap();
    assert_ne!(len & COMPRESSED_FLAG, 0);
    assert_ne!(len & ZSTD_FLAG, 0);
    assert_eq!(a.read(off, len).unwrap(), raw);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn incompressible_entries_stay_raw() {
    let d = scratch("incompressible");
    let mut state = 0x1234_5678u32;
    let raw: Vec<u8> = (0..4096)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    std::fs::write(d.join("random.ktx2"), &raw).unwrap();
    assert!(
        maybe_compress("random.ktx2", &d.join("random.ktx2"), raw.len() as u64)
            .unwrap()
            .is_none()
    );
    std::fs::write(d.join("exe"), b"exe").unwrap();
    write_pack(
        &d.join("exe"),
        &d.join("p"),
        &[("random.ktx2".to_string(), d.join("random.ktx2"))],
    )
    .unwrap();
    let archive = Archive::open(&d.join("p")).unwrap().unwrap();
    let (off, len) = archive.get("random.ktx2").unwrap();
    assert_eq!(len, raw.len() as u64);
    assert_eq!(archive.read(off, len).unwrap(), raw);
    let _ = std::fs::remove_dir_all(d);
}

fn fixture_archive(d: &Path, body: &[u8], flags: u64) -> Archive {
    let mut bytes = body.to_vec();
    let at = bytes.len() as u64;
    let mut index = 1u32.to_le_bytes().to_vec();
    index.extend(1u16.to_le_bytes());
    index.push(b'a');
    index.extend(0u64.to_le_bytes());
    index.extend((at | flags).to_le_bytes());
    bytes.extend(&index);
    bytes.extend(MAGIC);
    bytes.extend(0u64.to_le_bytes());
    bytes.extend(at.to_le_bytes());
    bytes.extend((index.len() as u64).to_le_bytes());
    let path = d.join("fixture");
    std::fs::write(&path, bytes).unwrap();
    Archive::open(&path).unwrap().unwrap()
}

#[test]
fn uncompressed_and_legacy_deflate_archives_remain_readable() {
    let d = scratch("legacy");
    let raw = vec![b'a'; 200_000];
    let old = fixture_archive(&d, &raw, 0);
    let (off, len) = old.get("a").unwrap();
    assert_eq!(old.read(off, len).unwrap(), raw);
    let mut body = (raw.len() as u32).to_le_bytes().to_vec();
    let mut encoder = flate2::write::DeflateEncoder::new(&mut body, flate2::Compression::fast());
    encoder.write_all(&raw).unwrap();
    encoder.finish().unwrap();
    let old = fixture_archive(&d, &body, COMPRESSED_FLAG);
    let (off, len) = old.get("a").unwrap();
    assert_eq!(old.read(off, len).unwrap(), raw);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn malformed_compressed_bodies_are_bounded_and_rejected() {
    let d = scratch("compressed_bad");
    for zstd in [false, true] {
        let raw = vec![42; 200_000];
        let mut body = (raw.len() as u32).to_le_bytes().to_vec();
        if zstd {
            zstd::stream::copy_encode(raw.as_slice(), &mut body, 3).unwrap();
        } else {
            let mut encoder =
                flate2::write::DeflateEncoder::new(&mut body, flate2::Compression::fast());
            encoder.write_all(&raw).unwrap();
            encoder.finish().unwrap();
        }
        let flags = COMPRESSED_FLAG | if zstd { ZSTD_FLAG } else { 0 };
        for end in 5..body.len() {
            let archive = fixture_archive(&d, &body[..end], flags);
            let (off, len) = archive.get("a").unwrap();
            assert!(
                archive.read(off, len).is_err(),
                "accepted truncated body {end}, zstd={zstd}"
            );
        }
        let mut trailing = body.clone();
        trailing.push(0);
        let archive = fixture_archive(&d, &trailing, flags);
        let (off, len) = archive.get("a").unwrap();
        assert!(
            archive.read(off, len).is_err(),
            "accepted trailing byte, zstd={zstd}"
        );
        // Declared-size bombs do not reserve the advertised 4 GiB before reading a tiny stream.
        let tiny = vec![7];
        let mut huge = u32::MAX.to_le_bytes().to_vec();
        if zstd {
            zstd::stream::copy_encode(tiny.as_slice(), &mut huge, 3).unwrap();
        } else {
            let mut encoder =
                flate2::write::DeflateEncoder::new(&mut huge, flate2::Compression::fast());
            encoder.write_all(&tiny).unwrap();
            encoder.finish().unwrap();
        }
        let archive = fixture_archive(&d, &huge, flags);
        let (off, len) = archive.get("a").unwrap();
        assert!(archive.read(off, len).is_err());
        body[..4].copy_from_slice(&0u32.to_le_bytes());
        let archive = fixture_archive(&d, &body, flags);
        let (off, len) = archive.get("a").unwrap();
        assert!(
            archive.read(off, len).is_err(),
            "expanded beyond advertised size, zstd={zstd}"
        );
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn windows_executable_size_includes_index_and_footer() {
    assert!(check_pack_size(WINDOWS_EXE_LIMIT - 37, 4, Some(WINDOWS_EXE_LIMIT)).is_ok());
    assert!(
        check_pack_size(WINDOWS_EXE_LIMIT - 36, 4, Some(WINDOWS_EXE_LIMIT))
            .unwrap_err()
            .to_string()
            .contains("4 GiB")
    );
    assert!(check_pack_size(u64::MAX, 4, None).is_err());
    let d = scratch("exe_limit");
    // Sparse source: the guard must refuse before copying even one byte or replacing an existing output.
    let exe = d.join("large.exe");
    File::create(&exe)
        .unwrap()
        .set_len(WINDOWS_EXE_LIMIT - 36)
        .unwrap();
    let out = d.join("output");
    std::fs::write(&out, b"keep previous output").unwrap();
    assert!(write_pack(&exe, &out, &[])
        .unwrap_err()
        .to_string()
        .contains("4 GiB"));
    assert_eq!(std::fs::read(&out).unwrap(), b"keep previous output");
    let _ = std::fs::remove_dir_all(d);
}
