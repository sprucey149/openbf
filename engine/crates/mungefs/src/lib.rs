//! Offline reader for PS2 `ucfb` (LVL) containers. Runtime never touches this;
//! only the `bake` tool does. Formats per audit A2 (analysis/a2-bake-formats.md).
//!
//! Container: chunks of [4-byte tag][u32 LE payload size][payload][pad to 4].
//! Tags are either 4 ASCII chars ("tex_", "NAME", ...) or the FNV-1a-32 hash
//! of a lowercased name (same scheme as the PC games).

pub mod anim;
pub mod cloth;
pub mod coll;
mod coll_fan;
pub mod config;
pub mod entity;
pub mod foley;
pub mod foliage;
pub mod font;
pub mod fx;
pub mod hud;
pub mod iso;
pub mod light;
pub mod lightbeam;
pub mod locl;
pub mod model;
pub mod particle;
pub mod path;
pub mod plan;
pub mod rle;
pub mod sanm;
pub mod skel;
pub mod sky;
pub mod sound_props;
pub mod soundbank;
pub mod stream;
pub mod terrain;
pub mod texture;
pub mod texture_pc;
pub mod texture_xbox;
pub mod water;
pub mod world;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum UcfbError {
    #[error("not a ucfb container (bad magic)")]
    BadMagic,
    #[error("truncated chunk at offset {0}")]
    Truncated(usize),
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Input that is structurally impossible or past a sanity limit — a
    /// parent cycle, a self-referential directory, a decompression bomb.
    /// Distinct from `Truncated` (data ran out) and `Unsupported` (a real
    /// format we do not decode).
    #[error("malformed: {0}")]
    Malformed(String),
}

/// Four-byte chunk tag, e.g. `ucfb`, `lvl_`, `scr_`, `NAME`.
pub type Tag = [u8; 4];

/// FNV-1a-32 with the game's byte fold (`b | 0x20`) — the hash used for
/// non-ASCII chunk tags, `lvl_` sub-level names, and PROP keys (A2 §1).
pub fn fnv1a_lowercase(name: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in name.bytes() {
        h ^= (b | 0x20) as u32;
        h = h.wrapping_mul(16_777_619);
    }
    h
}

/// A single chunk view into a buffer.
pub struct Chunk<'a> {
    pub tag: Tag,
    pub payload: &'a [u8],
    /// Byte offset of the chunk header within the file (for diagnostics).
    pub offset: usize,
}

impl<'a> Chunk<'a> {
    /// The tag as a u32 (LE), for comparing against `fnv1a_lowercase` hashes.
    pub fn tag_u32(&self) -> u32 {
        u32::from_le_bytes(self.tag)
    }

    /// True if the tag is 4 printable-ASCII bytes.
    pub fn tag_is_ascii(&self) -> bool {
        self.tag.iter().all(|&b| (0x20..0x7f).contains(&b))
    }

    /// Iterate this chunk's children (valid only for container chunks; on
    /// leaf data this yields garbage or Truncated errors — caller knows the
    /// schema).
    pub fn children(&self) -> impl Iterator<Item = Result<Chunk<'a>, UcfbError>> {
        chunks_at(self.payload, self.offset + 8)
    }

    /// First child with the given ASCII tag.
    pub fn child(&self, tag: &[u8; 4]) -> Option<Chunk<'a>> {
        self.children().flatten().find(|c| &c.tag == tag)
    }

    /// All children with the given ASCII tag.
    pub fn children_tagged(&self, tag: &'a [u8; 4]) -> impl Iterator<Item = Chunk<'a>> + 'a {
        self.children().flatten().filter(move |c| &c.tag == tag)
    }

    /// Payload interpreted as a NUL-terminated string (e.g. NAME chunks).
    pub fn as_str(&self) -> Option<&'a str> {
        let end = self.payload.iter().position(|&b| b == 0)?;
        std::str::from_utf8(&self.payload[..end]).ok()
    }

    /// First child with the given tag, read as a NUL-terminated string.
    /// `None` if the child is absent or its payload is not one — the two
    /// cases are indistinguishable to every caller in this crate, which
    /// either `ok_or_else`s a `Truncated` or defaults.
    pub fn child_str(&self, tag: &[u8; 4]) -> Option<&'a str> {
        self.child(tag).and_then(|c| c.as_str())
    }

    /// [`Chunk::child_str`] owned — what a parser keeping a NAME/TYPE wants.
    pub fn child_string(&self, tag: &[u8; 4]) -> Option<String> {
        self.child_str(tag).map(str::to_owned)
    }
}

/// Little-endian scalar reads at a byte offset into a chunk payload — the one
/// implementation of the `from_le_bytes(b[o..o + N])` line every parser in
/// this crate used to spell for itself. Each panics on a short payload, which
/// is why every caller length-checks the payload first (`if p.len() < N`).
pub(crate) mod le {
    pub fn u16_at(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }
    pub fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }
    pub fn u64_at(b: &[u8], at: usize) -> u64 {
        u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
    }
    pub fn f32_at(b: &[u8], at: usize) -> f32 {
        f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }
}

/// `dim >> level`, clamped to 1 and safe for any `level` a corrupt INFO might
/// claim (a plain shift past 31 panics in debug builds). Mip dimensions in
/// both texture decoders come through here.
pub(crate) fn halve(dim: u32, level: usize) -> u32 {
    u32::try_from(level)
        .ok()
        .and_then(|l| dim.checked_shr(l))
        .unwrap_or(0)
        .max(1)
}

/// Iterate top-level chunks of a buffer slice.
pub fn chunks(buf: &[u8]) -> impl Iterator<Item = Result<Chunk<'_>, UcfbError>> {
    chunks_at(buf, 0)
}

fn chunks_at(mut buf: &[u8], base: usize) -> impl Iterator<Item = Result<Chunk<'_>, UcfbError>> {
    let mut offset = base;
    std::iter::from_fn(move || {
        if buf.len() < 8 {
            return None;
        }
        let tag: Tag = buf[0..4].try_into().unwrap();
        let size = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
        let end = 8usize.checked_add(size)?;
        if end > buf.len() {
            // Stop after the error: a caller that skips errors (`flatten`)
            // would otherwise get this same `Err` forever.
            buf = &[];
            return Some(Err(UcfbError::Truncated(offset)));
        }
        let payload = &buf[8..end];
        let chunk_offset = offset;
        let padded = end.div_ceil(4) * 4;
        let advance = padded.min(buf.len());
        buf = &buf[advance..];
        offset += advance;
        Some(Ok(Chunk {
            tag,
            payload,
            offset: chunk_offset,
        }))
    })
}

/// Content chunk tags that live directly under `lvl_` wrappers. Anything else
/// inside a `lvl_` is a hashed sub-level wrapper whose children are content
/// — a hash can happen to be printable ASCII (TAT2's hunt layer 0x2A635340 =
/// "@Sc*"), so membership here, not `tag_is_ascii`, decides.
pub const CONTENT_TAGS: &[&[u8; 4]] = &[
    b"entc", b"wpnc", b"ordc", b"expc", b"tex_", b"modl", b"skel", b"gmod", b"coll", b"wrld",
    b"tern", b"path", b"PATH", b"plan", b"lght", b"zaa_", b"zaf_", b"scr_", b"Locl", b"mcfg",
    b"prim", b"fx__", b"snd_", b"font", b"anim", b"bnd_", b"shdw", b"CLTH", b"comb",
];

/// Collect CONTENT chunks from an opened LVL payload, flattening `lvl_`
/// sublevel wrappers: side/mission LVLs nest their content one or two levels
/// down (`lvl_` -> hashed-name container -> content), map LVLs put content at
/// the top level. A child of `lvl_` whose tag is in [`CONTENT_TAGS`] is
/// content; any other is a container and is descended into.
pub fn content_chunks(buf: &[u8]) -> Result<Vec<Chunk<'_>>, UcfbError> {
    let mut out = Vec::new();
    for c in chunks(buf) {
        let c = c?;
        if &c.tag == b"lvl_" {
            for sub in c.children() {
                let sub = sub?;
                if CONTENT_TAGS.contains(&&sub.tag) {
                    out.push(sub);
                } else {
                    for inner in sub.children() {
                        out.push(inner?);
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

/// Parse the outer `ucfb` wrapper and return its payload.
pub fn open(buf: &[u8]) -> Result<&[u8], UcfbError> {
    if buf.len() < 8 || &buf[0..4] != b"ucfb" {
        return Err(UcfbError::BadMagic);
    }
    let size = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if 8 + size > buf.len() {
        return Err(UcfbError::Truncated(0));
    }
    Ok(&buf[8..8 + size])
}

/// Test-only chunk builder: `[tag][u32 payload len][payload][pad to 4]`.
#[cfg(test)]
pub(crate) fn build_chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + payload.len());
    v.extend(tag);
    v.extend((payload.len() as u32).to_le_bytes());
    v.extend(payload);
    while v.len() % 4 != 0 {
        v.push(0);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_synthetic_container() {
        // ucfb { NAME "abc\0" ; INFO u32 }
        let mut f = Vec::new();
        f.extend(b"ucfb");
        f.extend(24u32.to_le_bytes());
        f.extend(b"NAME");
        f.extend(4u32.to_le_bytes());
        f.extend(b"abc\0");
        f.extend(b"INFO");
        f.extend(4u32.to_le_bytes());
        f.extend(7u32.to_le_bytes());
        let inner = open(&f).unwrap();
        let cs: Vec<_> = chunks(inner).map(|c| c.unwrap()).collect();
        assert_eq!(cs.len(), 2);
        assert_eq!(&cs[0].tag, b"NAME");
        assert_eq!(cs[0].as_str(), Some("abc"));
        assert_eq!(&cs[1].tag, b"INFO");
        assert!(cs[0].tag_is_ascii());
    }

    /// A chunk that overruns its parent ends the walk with ONE error: callers
    /// that skip errors (`child`, `children().flatten()`) used to spin on it.
    #[test]
    fn truncated_child_ends_the_walk() {
        let mut buf = build_chunk(b"INFO", &[1, 2, 3, 4]);
        buf.extend(b"XXXX");
        buf.extend(0x1000u32.to_le_bytes());
        buf.extend([0u8; 4]);
        let items: Vec<_> = chunks(&buf).collect();
        assert_eq!(items.len(), 2);
        assert!(matches!(items[1], Err(UcfbError::Truncated(12))));
        let parent = Chunk {
            tag: *b"tern",
            payload: &buf,
            offset: 0,
        };
        assert!(parent.child(b"ABSN").is_none());
        assert_eq!(parent.children_tagged(b"INFO").count(), 1);
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(matches!(open(b"nope0000"), Err(UcfbError::BadMagic)));
    }

    #[test]
    fn fnv1a_reference_values() {
        // Verified hashes from A2 §1 (analysis/a2-bake-formats.md).
        assert_eq!(fnv1a_lowercase("INFO"), 0x0FB4_0705);
        assert_eq!(fnv1a_lowercase("name"), 0x8D39_BDE6);
        assert_eq!(fnv1a_lowercase("layer"), 0x07A6_40F6);
        assert_eq!(fnv1a_lowercase("team"), 0xA2FD_7D0C);
        assert_eq!(fnv1a_lowercase("captureregion"), 0xC6C1_61F9);
        // Case folding: hash is over the lowercased name.
        assert_eq!(fnv1a_lowercase("NAME"), fnv1a_lowercase("name"));
    }
}
