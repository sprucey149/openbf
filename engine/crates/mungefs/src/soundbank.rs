//! SampleBank parsing (RE: analysis/re-sound-banks.md; hash fn = ELF
//! FUN_00444b30 which is exactly `fnv1a_lowercase` — each byte OR 0x20).
//!
//! SoundBankList (0x5E262256) → SampleBank (0x9A644D15) →
//!   info (0x0FB40705): flat (u32 field-hash, u32 value) pair stream:
//!     bank header (name/format/numsamples/size), then per-sample records
//!     delimited by marker pairs whose VALUE is hash("sample") = 0x96E382A7;
//!     record fields: id, frequency, size, decoded-frame-count, padding.
//!     There is no offset field — offsets accumulate over (size + padding).
//!   data (0xD872E2A5): concatenated SPU2 ADPCM.

use crate::{chunks, fnv1a_lowercase, Chunk, UcfbError};

pub struct Sample {
    pub id_hash: u32,
    pub frequency: u32,
    /// Offset into the bank's data payload.
    pub offset: usize,
    /// ADPCM byte length.
    pub size: usize,
    /// Decoded PCM frame count from the info chunk (== size/16*28).
    pub frames: u32,
    /// Set when this record is an alias: it owns no data and `offset`/`size`
    /// point at the aliased sample (field hash FNV("alias") = 0x7d268157;
    /// 40–55 % of the records in every SOUND/<planet>.LVL bank are aliases —
    /// the bank header `size` counts only real samples).
    pub alias_of: Option<u32>,
}

pub struct SampleBank<'a> {
    pub name_hash: u32,
    pub format: u32,
    pub samples: Vec<Sample>,
    pub data: &'a [u8],
}

/// Find all SampleBanks under a ucfb payload (searches top level and one
/// level under SoundBankList).
///
/// A strict subset of [`find_banks_deep`], and DELIBERATELY not replaced by
/// it (audit item M-R7): the deep walk descends `lvl_`/`SoundBankList`
/// wrappers and can therefore return a DIFFERENT bank first. Its one caller,
/// `bake::beep`, picks a bank by position, and `bake/tests/suite/beep.rs` pins the
/// result to a traced reference (rate 3,605 / 1,260 frames / 440.6 Hz), so
/// swapping the walk is a behaviour change to be measured, not a cleanup.
pub fn find_banks<'a>(inner: &'a [u8]) -> Vec<SampleBank<'a>> {
    let list_tag = fnv1a_lowercase("SoundBankList");
    let bank_tag = fnv1a_lowercase("SampleBank");
    let mut banks = Vec::new();
    for c in chunks(inner).flatten() {
        collect_list(&c, list_tag, bank_tag, parse_bank, &mut banks);
    }
    banks
}

/// Consume a direct sound item or the immediate matching children of its list.
pub(crate) fn collect_list<'a, T>(
    c: &Chunk<'a>,
    list_tag: u32,
    item_tag: u32,
    parse: impl Fn(&Chunk<'a>) -> Result<T, UcfbError>,
    out: &mut Vec<T>,
) -> bool {
    let t = c.tag_u32();
    if t == item_tag {
        out.extend(parse(c));
    } else if t == list_tag {
        for child in c.children().flatten() {
            if child.tag_u32() == item_tag {
                out.extend(parse(&child));
            }
        }
    } else {
        return false;
    }
    true
}

/// True for chunks worth descending into when hunting sound content: `lvl_`
/// wrappers and hashed-name wrappers (bank-name chunks). Everything else —
/// ASCII content chunks, and the hashed `data`/`info`/StreamList/Stream
/// chunks whose payloads are raw bytes — is a leaf (walking a 20 MB stream
/// payload as a chunk sequence is how the first version hung).
///
/// `in_lvl`: the chunk is a `lvl_`'s child - its hashed name wrapper,
/// whatever the hash's bytes are. BF1 KAS.LVL's `kas1gcw` hashes to
/// 0x2A396768 = "hg9*", four printable bytes, and was taken for an ASCII
/// leaf: Kashyyyk Islands GCW baked no map sounds (card WSND; the one such
/// name among the 69 sub-levels of the two discs).
pub fn is_sound_wrapper(c: &Chunk<'_>, in_lvl: bool) -> bool {
    if &c.tag == b"lvl_" {
        return true;
    }
    if c.tag_is_ascii() && !in_lvl {
        return false;
    }
    let t = c.tag_u32();
    let leaf = [
        fnv1a_lowercase("data"),
        fnv1a_lowercase("info"),
        fnv1a_lowercase("StreamList"),
        fnv1a_lowercase("Stream"),
        fnv1a_lowercase("SampleBank"),
        fnv1a_lowercase("SoundBankList"),
    ];
    !leaf.contains(&t)
}

/// Depth-capped descent through the wrappers a SOUND LVL nests its content
/// in (`lvl_` -> hashed bank name -> content), calling `visit` on every chunk
/// on the way down. `visit` returns true when it has CONSUMED the chunk,
/// which stops the descent there — a bank or a `snd_` payload is raw data,
/// not a wrapper, and walking into it yields garbage children.
pub fn walk_sound_wrappers<'a>(
    inner: &'a [u8],
    depth_cap: usize,
    visit: &mut dyn FnMut(&Chunk<'a>) -> bool,
) {
    for c in chunks(inner).flatten() {
        walk_sound_wrappers_of(&c, depth_cap, visit);
    }
}

/// [`walk_sound_wrappers`] from one top-level chunk down.
pub fn walk_sound_wrappers_of<'a>(
    top: &Chunk<'a>,
    depth_cap: usize,
    visit: &mut dyn FnMut(&Chunk<'a>) -> bool,
) {
    fn walk<'a>(
        c: &Chunk<'a>,
        in_lvl: bool,
        depth: usize,
        cap: usize,
        visit: &mut dyn FnMut(&Chunk<'a>) -> bool,
    ) {
        if visit(c) || depth >= cap || !is_sound_wrapper(c, in_lvl) {
            return;
        }
        let lvl = &c.tag == b"lvl_";
        for (n, k) in c.children().flatten().enumerate() {
            walk(&k, lvl && n == 0, depth + 1, cap, visit);
        }
    }
    walk(top, false, 0, depth_cap, visit);
}

/// Banks anywhere in the payload: SOUND/*.LVL keep their SoundBankLists
/// inside `lvl_` wrappers under hashed bank-name chunks.
pub fn find_banks_deep<'a>(inner: &'a [u8]) -> Vec<SampleBank<'a>> {
    let list_tag = fnv1a_lowercase("SoundBankList");
    let bank_tag = fnv1a_lowercase("SampleBank");
    let mut banks = Vec::new();
    walk_sound_wrappers(inner, 3, &mut |c: &Chunk<'a>| {
        collect_list(c, list_tag, bank_tag, parse_bank, &mut banks)
    });
    banks
}

pub(crate) fn info_pairs(info: &[u8]) -> Vec<(u32, u32)> {
    info.as_chunks::<8>()
        .0
        .iter()
        .map(|p| {
            (
                u32::from_le_bytes(p[0..4].try_into().unwrap()),
                u32::from_le_bytes(p[4..8].try_into().unwrap()),
            )
        })
        .collect()
}

pub(crate) struct SampleRecord {
    pub id: u32,
    pub frequency: u32,
    pub size: u32,
    pub frames: Option<u32>,
    pub padding: u32,
    pub alias_of: Option<u32>,
}

/// INFO records end at the next sample marker (including invalid records),
/// and contain at most seven field pairs. Markers match the value, not the key.
pub(crate) fn sample_records(pairs: &[(u32, u32)]) -> impl Iterator<Item = SampleRecord> + '_ {
    let h_sample = fnv1a_lowercase("sample");
    let h_id = fnv1a_lowercase("id");
    let h_freq = fnv1a_lowercase("frequency");
    let h_size = fnv1a_lowercase("size");
    let h_pad = fnv1a_lowercase("padding");
    let h_alias = fnv1a_lowercase("alias");
    let mut markers = pairs
        .iter()
        .enumerate()
        .filter(move |(_, (_, v))| *v == h_sample)
        .peekable();
    std::iter::from_fn(move || {
        let (start, _) = markers.next()?;
        let end = markers
            .peek()
            .map(|(i, _)| *i)
            .unwrap_or(pairs.len())
            .min(start + 8);
        Some(&pairs[start + 1..end])
    })
    .filter_map(move |rec| {
        let get = |h: u32| rec.iter().find(|(k, _)| *k == h).map(|(_, v)| *v);
        Some(SampleRecord {
            id: get(h_id)?,
            frequency: get(h_freq)?,
            size: get(h_size)?,
            frames: get(0x1D48_FEEF), // field name unknown; decoded frame count
            padding: get(h_pad).unwrap_or(0),
            alias_of: get(h_alias),
        })
    })
}

fn parse_bank<'a>(bank: &Chunk<'a>) -> Result<SampleBank<'a>, UcfbError> {
    let bad = || UcfbError::Truncated(bank.offset);
    let info_tag = fnv1a_lowercase("info");
    let data_tag = fnv1a_lowercase("data");
    let mut info = None;
    let mut data = None;
    for c in bank.children().flatten() {
        if c.tag_u32() == info_tag {
            info = Some(c.payload);
        } else if c.tag_u32() == data_tag {
            data = Some(c.payload);
        }
    }
    let (info, data) = (info.ok_or_else(bad)?, data.unwrap_or(&[]));

    let pairs = info_pairs(info);
    let h_name = fnv1a_lowercase("name");
    let h_format = fnv1a_lowercase("format");

    let name_hash = pairs
        .iter()
        .find(|(k, _)| *k == h_name)
        .map(|(_, v)| *v)
        .ok_or_else(bad)?;

    let format = pairs
        .iter()
        .find(|(k, _)| *k == h_format)
        .map(|(_, v)| *v)
        .unwrap_or(1);

    // Records are delimited by marker pairs whose VALUE is hash("sample").
    // Alias records (an `alias` field naming another sample id) own no data:
    // offsets accumulate over real records only; aliases resolve afterwards.
    let mut samples = Vec::new();
    let mut offset = 0usize;
    for rec in sample_records(&pairs) {
        let size = rec.size;
        let padding = rec.padding as usize;
        let frames = rec.frames.unwrap_or_else(|| {
            if format == 2 {
                size / 2
            } else {
                (size / 16).saturating_mul(28)
            }
        });
        samples.push(Sample {
            id_hash: rec.id,
            frequency: rec.frequency,
            offset,
            size: size as usize,
            frames,
            alias_of: rec.alias_of,
        });
        if rec.alias_of.is_none() {
            offset += size as usize + padding;
        }
    }
    // Resolve aliases to their target's data window (no chains observed;
    // an unresolvable alias is dropped).
    let real: std::collections::HashMap<u32, (usize, usize, u32)> = samples
        .iter()
        .filter(|s| s.alias_of.is_none())
        .map(|s| (s.id_hash, (s.offset, s.size, s.frames)))
        .collect();
    samples.retain_mut(|s| match s.alias_of {
        None => true,
        Some(target) => match real.get(&target) {
            Some(&(off, size, frames)) => {
                s.offset = off;
                s.size = size;
                s.frames = frames;
                true
            }
            None => false,
        },
    });
    Ok(SampleBank {
        name_hash,
        format,
        samples,
        data,
    })
}

#[cfg(test)]
mod alias_tests {
    use super::*;

    #[test]
    fn tat_sound_lvl_aliases_resolve_and_fit() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/TAT.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: SOUND/TAT.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let banks = find_banks_deep(inner);
        assert_eq!(banks.len(), 4);
        let b = banks.iter().find(|b| b.name_hash == 0x6426_7af8).unwrap();
        assert_eq!(b.samples.len(), 402);
        assert_eq!(
            b.samples.iter().filter(|s| s.alias_of.is_some()).count(),
            172
        );
        for bank in &banks {
            for s in &bank.samples {
                assert!(s.offset + s.size <= bank.data.len(), "{:08x}", s.id_hash);
            }
            // Real samples tile the bank data exactly (the header `size`).
            let used: usize = bank
                .samples
                .iter()
                .filter(|s| s.alias_of.is_none())
                .map(|s| s.size)
                .sum();
            assert!(used <= bank.data.len() && bank.data.len() - used < 4096);
        }
    }
}
