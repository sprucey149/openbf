//! Minimal XDVDFS (Xbox DVD filesystem) reader, the Xbox discs' twin of
//! [`crate::iso`]: enough to enumerate and read files out of a user's own disc
//! image. Nothing is written back.
//!
//! Layout, **[V]** verified on both local images (a trimmed `.xiso.iso` of each
//! Battlefront):
//!
//! ```text
//! base + 0x10000   20 B "MICROSOFT*XBOX*MEDIA"
//!                  u32 rootSector, u32 rootSize, u64 FILETIME, ...
//!                  20 B magic again at +0x7EC
//! directory = a binary tree of 4-byte aligned entries, in a table of rootSize:
//!   u16 left  (dword offset of the left child;  0 = none)
//!   u16 right (dword offset of the right child; 0 = none)
//!   u32 sector, u32 size, u8 attributes (0x10 = directory), u8 nameLen, name
//! ```
//!
//! `base` is 0 on a trimmed image and non-zero on a full disc dump: the game
//! partition sits behind the video partition at one of [`PARTITION_BASES`].
//! Every number here is file data on an untrusted image, so directory tables
//! are capped and each node and directory is visited once.

use crate::UcfbError;
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};

const SECTOR: u64 = 2048;
const VOLUME_OFFSET: u64 = 0x10000;
const MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";

/// Where the game partition starts: a trimmed image (0), a full Redump dump
/// (`0x18300000`) and the two older disc layouts the same tools recognize.
pub const PARTITION_BASES: [u64; 4] = [0, 0x1830_0000, 0x0FD9_0000, 0x0208_0000];

/// Sanity caps on the tree: the real discs hold under 300 files in a few dozen
/// directories, with tables of a few KiB.
const MAX_DIR_TABLE: u32 = 4 << 20;
const MAX_DIRS: usize = 4096;
const MAX_FILES: usize = 1 << 20;
const MAX_PATH: usize = 4096;
const ENTRY_HEADER: usize = 14;
const ATTR_DIRECTORY: u8 = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdvdFile {
    /// Path with '/' separators, in the disc's own case, e.g.
    /// "Data/_LVL_XBOX/core.lvl".
    pub path: String,
    pub sector: u32,
    pub size: u32,
}

pub struct Xdvdfs<R> {
    reader: R,
    base: u64,
    pub files: Vec<XdvdFile>,
}

impl<R: Read + Seek> Xdvdfs<R> {
    /// [`UcfbError::BadMagic`] on an image with no XDVDFS volume at any known
    /// partition offset, so a caller can use it as the content test.
    pub fn open(mut reader: R) -> Result<Xdvdfs<R>, UcfbError> {
        let mut found = None;
        for base in PARTITION_BASES {
            let mut magic = [0u8; 20];
            if read_at(&mut reader, base + VOLUME_OFFSET, &mut magic).is_ok() && &magic == MAGIC {
                found = Some(base);
                break;
            }
        }
        let base = found.ok_or(UcfbError::BadMagic)?;
        let mut head = [0u8; 8];
        read_at(&mut reader, base + VOLUME_OFFSET + 20, &mut head)?;
        let root_sector = u32::from_le_bytes(head[0..4].try_into().unwrap());
        let root_size = u32::from_le_bytes(head[4..8].try_into().unwrap());

        let mut files = Vec::new();
        let mut pending = vec![(String::new(), root_sector, root_size)];
        let mut visited = HashSet::new();
        while let Some((prefix, sector, size)) = pending.pop() {
            // A directory pointing at itself or an ancestor would never end.
            if size == 0 || !visited.insert(sector) {
                continue;
            }
            if visited.len() > MAX_DIRS {
                return Err(UcfbError::Malformed(format!(
                    "xdvdfs: more than {MAX_DIRS} directories"
                )));
            }
            if size > MAX_DIR_TABLE {
                return Err(UcfbError::Malformed(format!(
                    "xdvdfs: directory table of {size} bytes at sector {sector}"
                )));
            }
            let mut table = vec![0u8; size as usize];
            read_at(&mut reader, base + sector as u64 * SECTOR, &mut table)?;
            walk_table(&table, &prefix, &mut pending, &mut files)?;
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Xdvdfs {
            reader,
            base,
            files,
        })
    }

    /// Byte offset of the game partition within the image.
    pub fn base(&self) -> u64 {
        self.base
    }

    /// Read a whole file by its listing entry.
    pub fn read_file(&mut self, f: &XdvdFile) -> Result<Vec<u8>, UcfbError> {
        let mut buf = Vec::new();
        self.reader
            .seek(SeekFrom::Start(self.base + f.sector as u64 * SECTOR))
            .and_then(|_| (&mut self.reader).take(f.size as u64).read_to_end(&mut buf))
            .map_err(|_| UcfbError::Truncated(f.sector as usize))?;
        if buf.len() != f.size as usize {
            return Err(UcfbError::Truncated(f.sector as usize));
        }
        Ok(buf)
    }

    /// Find a file by path (case-insensitive).
    pub fn find(&self, path: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|f| f.path.eq_ignore_ascii_case(path))
    }
}

/// Walk one directory's binary tree, depth first from the node at offset 0.
fn walk_table(
    table: &[u8],
    prefix: &str,
    pending: &mut Vec<(String, u32, u32)>,
    files: &mut Vec<XdvdFile>,
) -> Result<(), UcfbError> {
    let mut stack = vec![0usize];
    let mut seen = HashSet::new();
    while let Some(off) = stack.pop() {
        if !seen.insert(off) {
            continue; // a cycle in the tree, or two parents for one node
        }
        // An all-0xFF node is what a directory with no entries holds, and the
        // padding between nodes is 0xFF: neither is an entry.
        let Some(head) = table.get(off..off + ENTRY_HEADER) else {
            return Err(UcfbError::Truncated(off));
        };
        let left = u16::from_le_bytes([head[0], head[1]]);
        let right = u16::from_le_bytes([head[2], head[3]]);
        if left == 0xFFFF && right == 0xFFFF {
            continue;
        }
        let sector = u32::from_le_bytes(head[4..8].try_into().unwrap());
        let size = u32::from_le_bytes(head[8..12].try_into().unwrap());
        let attributes = head[12];
        let name_len = head[13] as usize;
        let name = table
            .get(off + ENTRY_HEADER..off + ENTRY_HEADER + name_len)
            .ok_or(UcfbError::Truncated(off))?;
        let name = String::from_utf8_lossy(name);
        let path = if prefix.is_empty() {
            name.into_owned()
        } else {
            format!("{prefix}/{name}")
        };
        if path.len() > MAX_PATH {
            return Err(UcfbError::Malformed(format!(
                "xdvdfs: a path of {} bytes",
                path.len()
            )));
        }
        if attributes & ATTR_DIRECTORY != 0 {
            pending.push((path, sector, size));
        } else {
            if files.len() >= MAX_FILES {
                return Err(UcfbError::Malformed(format!(
                    "xdvdfs: more than {MAX_FILES} files"
                )));
            }
            files.push(XdvdFile { path, sector, size });
        }
        for child in [left, right] {
            if child != 0 && child != 0xFFFF {
                stack.push(child as usize * 4);
            }
        }
    }
    Ok(())
}

fn read_at<R: Read + Seek>(r: &mut R, pos: u64, buf: &mut [u8]) -> Result<(), UcfbError> {
    r.seek(SeekFrom::Start(pos))
        .and_then(|_| r.read_exact(buf))
        .map_err(|_| UcfbError::Truncated(pos as usize))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn entry(left: u16, right: u16, sector: u32, size: u32, attr: u8, name: &str) -> Vec<u8> {
        let mut e = Vec::new();
        e.extend(left.to_le_bytes());
        e.extend(right.to_le_bytes());
        e.extend(sector.to_le_bytes());
        e.extend(size.to_le_bytes());
        e.push(attr);
        e.push(name.len() as u8);
        e.extend(name.as_bytes());
        while e.len() % 4 != 0 {
            e.push(0xFF);
        }
        e
    }

    /// A volume with its root table at sector 40 and any `extra` sectors.
    fn image(base: u64, root: &[u8], extra: &[(u32, &[u8])]) -> Cursor<Vec<u8>> {
        let mut img = vec![0u8; (base + 80 * SECTOR) as usize];
        let v = (base + VOLUME_OFFSET) as usize;
        img[v..v + 20].copy_from_slice(MAGIC);
        img[v + 20..v + 24].copy_from_slice(&40u32.to_le_bytes());
        img[v + 24..v + 28].copy_from_slice(&(root.len() as u32).to_le_bytes());
        let at = (base + 40 * SECTOR) as usize;
        img[at..at + root.len()].copy_from_slice(root);
        for (s, bytes) in extra {
            let at = (base + *s as u64 * SECTOR) as usize;
            img[at..at + bytes.len()].copy_from_slice(bytes);
        }
        Cursor::new(img)
    }

    fn err_of<R: Read + Seek>(r: Result<Xdvdfs<R>, UcfbError>) -> UcfbError {
        match r {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        }
    }

    #[test]
    fn walks_the_tree_and_reads_a_file() {
        // Root: "b" with left child "a" (a directory holding "c.lvl").
        let b = entry(5, 0, 50, 3, 0x20, "b.lvl"); // 20 B -> a at dword 5
        let a = entry(0, 0, 60, 16 + 4, ATTR_DIRECTORY, "a");
        let mut root = b.clone();
        root.extend(&a);
        let sub = entry(0, 0, 70, 4, 0x20, "c.lvl");
        let mut disc =
            Xdvdfs::open(image(0, &root, &[(50, b"abc"), (60, &sub), (70, b"wxyz")])).unwrap();
        let paths: Vec<&str> = disc.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a/c.lvl", "b.lvl"]);
        let at = disc.find("A/C.LVL").unwrap();
        let f = disc.files[at].clone();
        assert_eq!(disc.read_file(&f).unwrap(), b"wxyz");
    }

    #[test]
    fn finds_a_volume_behind_the_video_partition() {
        let root = entry(0, 0, 50, 2, 0x20, "x");
        let disc = Xdvdfs::open(image(0x0208_0000, &root, &[(50, b"hi")])).unwrap();
        assert_eq!(disc.base(), 0x0208_0000);
        assert_eq!(disc.files[0].path, "x");
    }

    #[test]
    fn a_ps2_image_is_not_a_volume() {
        let e = err_of(Xdvdfs::open(Cursor::new(vec![0u8; 0x20000])));
        assert!(matches!(e, UcfbError::BadMagic), "{e}");
    }

    #[test]
    fn a_directory_pointing_at_itself_terminates() {
        let root = entry(0, 0, 40, 20, ATTR_DIRECTORY, "loop");
        let disc = Xdvdfs::open(image(0, &root, &[])).unwrap();
        assert!(disc.files.is_empty());
    }

    #[test]
    fn a_tree_node_pointing_at_itself_terminates() {
        // Two 16-byte nodes; the second's left child is the second again.
        let mut root = entry(4, 0, 50, 1, 0x20, "x");
        root.extend(entry(4, 0, 50, 1, 0x20, "y"));
        let disc = Xdvdfs::open(image(0, &root, &[])).unwrap();
        assert_eq!(disc.files.len(), 2);
    }

    #[test]
    fn an_empty_directory_table_has_no_entries() {
        let disc = Xdvdfs::open(image(0, &[0xFF; 2048], &[])).unwrap();
        assert!(disc.files.is_empty());
    }

    #[test]
    fn a_node_past_the_table_errors() {
        let root = entry(0x4000, 0, 50, 1, 0x20, "x");
        let e = err_of(Xdvdfs::open(image(0, &root, &[])));
        assert!(matches!(e, UcfbError::Truncated(_)), "{e}");
    }

    #[test]
    fn an_absurd_directory_table_errors() {
        let mut img = image(0, &[], &[]).into_inner();
        let v = VOLUME_OFFSET as usize;
        img[v + 24..v + 28].copy_from_slice(&u32::MAX.to_le_bytes());
        let e = err_of(Xdvdfs::open(Cursor::new(img)));
        assert!(matches!(e, UcfbError::Malformed(_)), "{e}");
    }

    #[test]
    fn a_file_past_the_image_errors() {
        let root = entry(0, 0, 70_000_000, 8, 0x20, "x");
        let mut disc = Xdvdfs::open(image(0, &root, &[])).unwrap();
        let f = disc.files[0].clone();
        assert!(disc.read_file(&f).is_err());
    }

    /// Real data: both local Xbox images list every file and read back the
    /// LVLs extracted to `extracted/xbox_bf*` by an independent tool, byte for
    /// byte.
    #[test]
    fn real_discs_match_their_extracted_lvls() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
        for (iso, tree, want_files) in [
            (
                "Star Wars - Battlefront (USA, Europe).xiso.iso",
                "xbox_bf1",
                169,
            ),
            (
                "Star Wars - Battlefront II (USA) (En,Es,It) (Rev 1).xiso.iso",
                "xbox_bf2",
                219,
            ),
        ] {
            let Ok(file) = std::fs::File::open(format!("{root}/iso/{iso}")) else {
                eprintln!("SKIP: {iso} not present");
                return;
            };
            let mut disc = Xdvdfs::open(std::io::BufReader::new(file)).unwrap();
            assert_eq!(disc.files.len(), want_files, "{iso}");
            for rel in ["core.lvl", "mission.lvl", "geo/geo1.lvl"] {
                let on_disc = format!("Data/_LVL_XBOX/{rel}");
                let at = disc.find(&on_disc).unwrap_or_else(|| panic!("{on_disc}"));
                let f = disc.files[at].clone();
                let got = disc.read_file(&f).unwrap();
                let want = std::fs::read(format!("{root}/extracted/{tree}/{rel}")).unwrap();
                assert_eq!(got, want, "{on_disc}");
            }
        }
    }
}
