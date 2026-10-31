//! The game's read-only data: a directory on disk, or the archive appended to the game's own executable (card PACK).
//!
//! A dev build reads `assets_baked/` and `missions/` as it always has. A packed build (`game --write-pack`, run by
//! `scripts/pack.py`) carries both inside its one file. Every runtime read of baked data goes through this crate, so the
//! rest of the game does not know which of the two it is reading.
//!
//! [`mount`] says which archive name serves which directory path. It picks ONE source per mount, at mount time: the
//! directory if it exists, else the archive when the executable carries that name. Nothing mixes the two, so a stale
//! partial directory can never shadow single files of the archive. A path under no archive-backed mount is a plain disk
//! path.
//!
//! Layout appended to the executable (all integers little-endian):
//! ```text
//! [exe bytes][file bodies ...][index][footer]
//! index:  u32 count, then per entry: u16 path length, path (UTF-8, '/'-separated, "<name>/<rel>"), u64 offset, u64 length
//! footer: b"OPENBFPK", u64 pack start, u64 index offset, u64 index length      (32 bytes, the file's last)
//! ```
//! Offsets are from the start of the file. The pack start is where the executable proper ends, so re-packing a packed
//! binary copies only the executable.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAGIC: &[u8; 8] = b"OPENBFPK";
const FOOTER: u64 = 32;
const COMPRESSED_FLAG: u64 = 1 << 63;

/// An archive: the open file and its index (path -> offset, length).
pub struct Archive {
    file: File,
    path: PathBuf,
    start: u64,
    entries: BTreeMap<String, (u64, u64)>,
}

impl Archive {
    /// The archive inside `path`, None when the file carries none.
    pub fn open(path: &Path) -> io::Result<Option<Archive>> {
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        if size < FOOTER {
            return Ok(None);
        }
        let mut f = [0u8; FOOTER as usize];
        file.seek(SeekFrom::Start(size - FOOTER))?;
        file.read_exact(&mut f)?;
        if &f[..8] != MAGIC {
            return Ok(None);
        }
        let start = u64::from_le_bytes(f[8..16].try_into().unwrap());
        let at = u64::from_le_bytes(f[16..24].try_into().unwrap());
        let len = u64::from_le_bytes(f[24..32].try_into().unwrap());
        if start > at || at.checked_add(len).and_then(|end| end.checked_add(FOOTER)) != Some(size) {
            return Err(bad("footer does not match the file size"));
        }
        let len = usize::try_from(len).map_err(|_| bad("index too large for this platform"))?;
        let mut index = vec![0u8; len];
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut index)?;
        let mut r = &index[..];
        let n = take_u32(&mut r)?;
        let mut entries = BTreeMap::new();
        for _ in 0..n {
            let l = take_u16(&mut r)? as usize;
            if r.len() < l {
                return Err(bad("index truncated"));
            }
            let name =
                String::from_utf8(r[..l].to_vec()).map_err(|_| bad("index path not UTF-8"))?;
            r = &r[l..];
            let off = take_u64(&mut r)?;
            let raw_n = take_u64(&mut r)?;
            let compressed = (raw_n & COMPRESSED_FLAG) != 0;
            let n = raw_n & !COMPRESSED_FLAG;
            if (compressed && n < 5) || off < start || off.checked_add(n).is_none_or(|end| end > at)
            {
                return Err(bad("entry outside the pack"));
            }
            entries.insert(name, (off, raw_n));
        }
        Ok(Some(Archive {
            file,
            path: path.to_path_buf(),
            start,
            entries,
        }))
    }

    /// Bytes of the file before the pack: the executable proper.
    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry's name, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    fn get(&self, key: &str) -> Option<(u64, u64)> {
        self.entries.get(key).copied()
    }

    /// Any entry under `key/`.
    fn has_dir(&self, key: &str) -> bool {
        let p = format!("{key}/");
        self.entries
            .range(p.clone()..)
            .next()
            .is_some_and(|(k, _)| k.starts_with(&p))
    }

    /// The names directly under `key/` (files and directories), sorted, once each.
    fn children(&self, key: &str) -> Vec<String> {
        let p = format!("{key}/");
        let mut out: Vec<String> = Vec::new();
        for k in self
            .entries
            .range(p.clone()..)
            .map(|(k, _)| k)
            .take_while(|k| k.starts_with(&p))
        {
            let child = k[p.len()..].split('/').next().unwrap_or_default();
            if out.last().map(String::as_str) != Some(child) {
                out.push(child.to_string());
            }
        }
        out
    }

    fn read(&self, off: u64, raw_len: u64) -> io::Result<Vec<u8>> {
        let compressed = (raw_len & COMPRESSED_FLAG) != 0;
        let stored = raw_len & !COMPRESSED_FLAG;
        let len = usize::try_from(stored).map_err(|_| bad("entry too large for this platform"))?;
        let mut buf = vec![0u8; len];
        read_exact_at(&self.file, &mut buf, off)?;
        if !compressed {
            return Ok(buf);
        }
        if buf.len() < 5 {
            return Err(bad("compressed entry truncated"));
        }
        let expected = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
        let mut out = Vec::with_capacity(expected);
        flate2::read::DeflateDecoder::new(&buf[4..]).read_to_end(&mut out)?;
        if out.len() != expected {
            return Err(bad("decompressed length mismatch"));
        }
        Ok(out)
    }
}

/// Copy `exe` (without any pack it already carries) to `out` and append `entries` ((archive path, file on disk)).
/// Archive paths are "<name>/<rel>" with '/' separators. The output is made executable on Unix.
pub fn write_pack(exe: &Path, out: &Path, entries: &[(String, PathBuf)]) -> io::Result<()> {
    let keep = match Archive::open(exe)? {
        Some(a) => a.start(),
        None => std::fs::metadata(exe)?.len(),
    };
    let mut w = io::BufWriter::new(File::create(out)?);
    io::copy(&mut File::open(exe)?.take(keep), &mut w)?;
    let mut pos = keep;
    let mut index = Vec::new();
    index.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    // Identical files are stored once (a planet's music sits under each of its maps): (length, hash) -> the first
    // such file and its offset, confirmed byte for byte before it is shared.
    let mut seen: std::collections::HashMap<(u64, u64), Vec<(&Path, usize)>> =
        std::collections::HashMap::new();
    let mut uniques: Vec<(&str, &Path, u64)> = Vec::new();
    let mut entry_unique: Vec<usize> = Vec::with_capacity(entries.len());
    for (name, src) in entries {
        if name.len() > u16::MAX as usize || name.contains('\\') || name.starts_with('/') {
            return Err(bad(&format!("bad archive path {name:?}")));
        }
        let key = (std::fs::metadata(src)?.len(), hash_file(src)?);
        let mut found = None;
        for &(p, uidx) in seen.get(&key).into_iter().flatten() {
            if same_bytes(p, src)? {
                found = Some(uidx);
                break;
            }
        }
        let uidx = match found {
            Some(uidx) => uidx,
            None => {
                let uidx = uniques.len();
                uniques.push((name.as_str(), src.as_path(), key.0));
                seen.entry(key).or_default().push((src.as_path(), uidx));
                uidx
            }
        };
        entry_unique.push(uidx);
    }
    let workers = std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 16))
        .unwrap_or(4);
    let mut stored_spans: Vec<(u64, u64)> = Vec::with_capacity(uniques.len());
    for batch in uniques.chunks(64) {
        let next = std::sync::atomic::AtomicUsize::new(0);
        type CompressedSlot = Mutex<Option<io::Result<Option<Vec<u8>>>>>;
        let results: Vec<CompressedSlot> = (0..batch.len()).map(|_| Mutex::new(None)).collect();
        std::thread::scope(|s| {
            for _ in 0..workers.min(batch.len()) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= batch.len() {
                        break;
                    }
                    let (uname, usrc, ulen) = batch[i];
                    let res = maybe_compress(uname, usrc, ulen);
                    *results[i].lock().unwrap() = Some(res);
                });
            }
        });
        for (i, &(_, usrc, ulen)) in batch.iter().enumerate() {
            let compressed = results[i].lock().unwrap().take().unwrap()?;
            let off = pos;
            let raw_n = match compressed {
                Some(buf) => {
                    w.write_all(&buf)?;
                    let clen = buf.len() as u64;
                    pos += clen;
                    clen | COMPRESSED_FLAG
                }
                None => {
                    let n = io::copy(&mut File::open(usrc)?, &mut w)?;
                    if n != ulen {
                        return Err(bad(&format!("{} changed while packing", usrc.display())));
                    }
                    pos += n;
                    n
                }
            };
            stored_spans.push((off, raw_n));
        }
    }
    for ((name, _), &uidx) in entries.iter().zip(&entry_unique) {
        let (off, raw_n) = stored_spans[uidx];
        index.extend_from_slice(&(name.len() as u16).to_le_bytes());
        index.extend_from_slice(name.as_bytes());
        index.extend_from_slice(&off.to_le_bytes());
        index.extend_from_slice(&raw_n.to_le_bytes());
    }
    w.write_all(&index)?;
    w.write_all(MAGIC)?;
    w.write_all(&keep.to_le_bytes())?;
    w.write_all(&pos.to_le_bytes())?;
    w.write_all(&(index.len() as u64).to_le_bytes())?;
    w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Every file under `dir` (followed through links), as ("<name>/<rel>", path on disk), sorted by archive path.
pub fn collect(dir: &Path, name: &str) -> io::Result<Vec<(String, PathBuf)>> {
    fn walk(d: &Path, key: &str, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
        for e in std::fs::read_dir(d)? {
            let p = e?.path();
            let n = p
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| bad(&format!("{}: name not UTF-8", p.display())))?;
            let k = format!("{key}/{n}");
            if p.is_dir() {
                walk(&p, &k, out)?;
            } else {
                out.push((k, p));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, name, &mut out)?;
    out.sort();
    Ok(out)
}

// ---------- the mounts ----------

struct Mount {
    /// The directory path as the game spells it, lexically normalised, and the same joined to the working directory.
    rel: PathBuf,
    abs: PathBuf,
    name: String,
}

static EXE: OnceLock<Option<Archive>> = OnceLock::new();
static MOUNTS: Mutex<Vec<Mount>> = Mutex::new(Vec::new());

/// The archive appended to the running executable, if any (opened once).
pub fn exe_archive() -> Option<&'static Archive> {
    EXE.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        match Archive::open(&exe) {
            Ok(a) => a,
            Err(e) => {
                eprintln!(
                    "pak: {} carries a damaged pack ({e}); reading from disk only",
                    exe.display()
                );
                None
            }
        }
    })
    .as_ref()
}

/// Serve the path `dir` from the archive's `name/` when `dir` is not a directory on disk and the executable carries
/// `name`. Returns true when the archive serves it. Mounting the same directory again replaces the earlier mount.
pub fn mount(dir: &Path, name: &str) -> bool {
    let rel = normalize(dir);
    let mut ms = MOUNTS.lock().unwrap();
    ms.retain(|m| m.rel != rel);
    if dir.is_dir() || !exe_archive().is_some_and(|a| a.has_dir(name)) {
        return false;
    }
    let abs = std::env::current_dir()
        .map(|c| normalize(&c.join(&rel)))
        .unwrap_or_else(|_| rel.clone());
    ms.push(Mount {
        rel,
        abs,
        name: name.to_string(),
    });
    true
}

/// True when any mount is served by the archive (a packed build running packed).
pub fn packed() -> bool {
    !MOUNTS.lock().unwrap().is_empty()
}

/// Where a path lives: on disk, or under this archive key.
enum Place {
    Disk(PathBuf),
    Pak(String),
}

fn place(path: &Path) -> Place {
    let ms = MOUNTS.lock().unwrap();
    if !ms.is_empty() {
        let p = normalize(path);
        for m in ms.iter() {
            let root = if p.is_absolute() { &m.abs } else { &m.rel };
            if let Ok(rest) = p.strip_prefix(root) {
                let mut key = m.name.clone();
                for c in rest.components() {
                    key.push('/');
                    key.push_str(&c.as_os_str().to_string_lossy());
                }
                return Place::Pak(key);
            }
        }
    }
    Place::Disk(path.to_path_buf())
}

fn pak() -> &'static Archive {
    exe_archive().expect("a mount exists only when the archive does")
}

fn not_found(key: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{key}: not in the packed data"),
    )
}

// ---------- the std::fs look-alikes ----------

/// Files read or opened through this crate since the process started, and
/// the bytes [`read`] returned (card HITCH: the frame recorder counts the
/// ones a battle frame makes - a read in a frame waits on the disk).
static READS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static READ_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// ([`read`] and [`open`] calls, bytes read) so far.
pub fn reads() -> (u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (READS.load(Relaxed), READ_BYTES.load(Relaxed))
}

fn count(bytes: usize) {
    use std::sync::atomic::Ordering::Relaxed;
    READS.fetch_add(1, Relaxed);
    READ_BYTES.fetch_add(bytes as u64, Relaxed);
}

pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let out = match place(path.as_ref()) {
        Place::Disk(p) => std::fs::read(p),
        Place::Pak(k) => {
            let (off, len) = pak().get(&k).ok_or_else(|| not_found(&k))?;
            pak().read(off, len)
        }
    };
    count(out.as_ref().map_or(0, Vec::len));
    out
}

pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let path = path.as_ref();
    String::from_utf8(read(path)?).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: not UTF-8", path.display()),
        )
    })
}

pub fn is_file(path: impl AsRef<Path>) -> bool {
    match place(path.as_ref()) {
        Place::Disk(p) => p.is_file(),
        Place::Pak(k) => pak().get(&k).is_some(),
    }
}

pub fn is_dir(path: impl AsRef<Path>) -> bool {
    match place(path.as_ref()) {
        Place::Disk(p) => p.is_dir(),
        Place::Pak(k) => pak().has_dir(&k),
    }
}

pub fn exists(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    is_file(path) || is_dir(path)
}

/// The paths directly inside `path` (`path.join(name)` each), sorted by name. Unlike `std::fs::read_dir` the order is
/// fixed, on disk too.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let path = path.as_ref();
    let mut names: Vec<std::ffi::OsString> = match place(path) {
        Place::Disk(p) => std::fs::read_dir(p)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<io::Result<_>>()?,
        Place::Pak(k) => {
            if !pak().has_dir(&k) {
                return Err(not_found(&k));
            }
            pak().children(&k).into_iter().map(Into::into).collect()
        }
    };
    names.sort();
    Ok(names.into_iter().map(|n| path.join(n)).collect())
}

/// A file opened for seek + read: a disk file, or one entry's byte range inside the executable.
pub enum Reader {
    Disk(File),
    Pak {
        file: File,
        start: u64,
        len: u64,
        pos: u64,
    },
    Mem(io::Cursor<Vec<u8>>),
}

pub fn open(path: impl AsRef<Path>) -> io::Result<Reader> {
    count(0);
    match place(path.as_ref()) {
        Place::Disk(p) => Ok(Reader::Disk(File::open(p)?)),
        Place::Pak(k) => {
            let (start, len) = pak().get(&k).ok_or_else(|| not_found(&k))?;
            if (len & COMPRESSED_FLAG) != 0 {
                Ok(Reader::Mem(io::Cursor::new(pak().read(start, len)?)))
            } else {
                Ok(Reader::Pak {
                    file: File::open(&pak().path)?,
                    start,
                    len,
                    pos: 0,
                })
            }
        }
    }
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Reader::Disk(f) => f.read(buf),
            Reader::Mem(c) => c.read(buf),
            Reader::Pak {
                file,
                start,
                len,
                pos,
            } => {
                let n = buf.len().min(len.saturating_sub(*pos) as usize);
                if n == 0 {
                    return Ok(0);
                }
                read_exact_at(file, &mut buf[..n], *start + *pos)?;
                *pos += n as u64;
                Ok(n)
            }
        }
    }
}

impl Seek for Reader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Reader::Disk(f) => f.seek(to),
            Reader::Mem(c) => c.seek(to),
            Reader::Pak { len, pos, .. } => {
                let p = match to {
                    SeekFrom::Start(n) => n as i128,
                    SeekFrom::End(d) => *len as i128 + d as i128,
                    SeekFrom::Current(d) => *pos as i128 + d as i128,
                };
                if p < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek before the start",
                    ));
                }
                *pos = u64::try_from(p).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "seek beyond u64 range")
                })?;
                Ok(*pos)
            }
        }
    }
}

fn should_compress(name: &str, raw_len: u64) -> bool {
    if !(128..=(u32::MAX as u64)).contains(&raw_len) {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ktx2")
        || lower.ends_with(".glb")
        || lower.ends_with(".ron")
        || lower.ends_with(".wav")
}

fn maybe_compress(name: &str, src: &Path, raw_len: u64) -> io::Result<Option<Vec<u8>>> {
    if !should_compress(name, raw_len) {
        return Ok(None);
    }
    let raw = std::fs::read(src)?;
    if raw.len() as u64 != raw_len {
        return Err(bad(&format!("{} changed while packing", src.display())));
    }
    let mut out = Vec::with_capacity(raw.len());
    out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    let mut enc = flate2::write::DeflateEncoder::new(&mut out, flate2::Compression::fast());
    enc.write_all(&raw)?;
    enc.finish()?;
    if (out.len() as u64) + 64 < raw_len {
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

// ---------- helpers ----------

/// `a/./b/../c` -> `a/c`, without touching the disk. A leading `..` stays.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(unix)]
fn read_exact_at(f: &File, buf: &mut [u8], off: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(f, buf, off)
}

#[cfg(windows)]
fn read_exact_at(f: &File, mut buf: &mut [u8], mut off: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let n = f.seek_read(buf, off)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
        off += n as u64;
    }
    Ok(())
}

/// FNV-1a 64 over the file, streamed.
fn hash_file(p: &Path) -> io::Result<u64> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut r = io::BufReader::with_capacity(1 << 20, File::open(p)?);
    loop {
        let buf = r.fill_buf()?;
        if buf.is_empty() {
            return Ok(h);
        }
        for &b in buf {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        let n = buf.len();
        r.consume(n);
    }
}

fn same_bytes(a: &Path, b: &Path) -> io::Result<bool> {
    let (mut ra, mut rb) = (
        io::BufReader::new(File::open(a)?),
        io::BufReader::new(File::open(b)?),
    );
    let (mut x, mut y) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let n = ra.read(&mut x)?;
        if n == 0 {
            return Ok(rb.read(&mut y)? == 0);
        }
        rb.read_exact(&mut y[..n])?;
        if x[..n] != y[..n] {
            return Ok(false);
        }
    }
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("pak: {msg}"))
}

fn take_u16(r: &mut &[u8]) -> io::Result<u16> {
    let (h, t) = r
        .split_at_checked(2)
        .ok_or_else(|| bad("index truncated"))?;
    *r = t;
    Ok(u16::from_le_bytes(h.try_into().unwrap()))
}

fn take_u32(r: &mut &[u8]) -> io::Result<u32> {
    let (h, t) = r
        .split_at_checked(4)
        .ok_or_else(|| bad("index truncated"))?;
    *r = t;
    Ok(u32::from_le_bytes(h.try_into().unwrap()))
}

fn take_u64(r: &mut &[u8]) -> io::Result<u64> {
    let (h, t) = r
        .split_at_checked(8)
        .ok_or_else(|| bad("index truncated"))?;
    *r = t;
    Ok(u64::from_le_bytes(h.try_into().unwrap()))
}

#[cfg(test)]
mod tests;
