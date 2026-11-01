//! Audio STREAM parsing (RE: analysis/re-sound-global.md §1.1, lead-verified
//! 2026-09-07). The music/VO half of a sound LVL, which the SampleBank reader
//! in `soundbank` never touched:
//!
//! StreamList (0x5BB97F21) -> Stream (0x5F6F6D65, bytes `emo_`) ->
//!   info (0x0FB40705): the same flat (u32 field-hash, u32 value) pair stream
//!     a SampleBank `info` uses — header (name/format/numchannels/block/
//!     numsegments/size), then per-segment records delimited by marker pairs
//!     whose VALUE is hash("sample") = 0x96E382A7 (the first marker's KEY is
//!     0xB969BE96, later ones 0x2E789FB4); record fields: id, frequency,
//!     size, frame count, padding, optional alias.
//!   data (0xD872E2A5): concatenated SPU2 ADPCM, one 16 KiB-aligned run per
//!     segment. There is no offset field — offsets accumulate over
//!     (size + padding), and `padding` is real ADPCM: silent blocks that
//!     round the segment up to the stream's 16 KiB block size, which is why
//!     the info frame count is `(size + padding)/16*28` and not `size/16*28`
//!     (measured on BF2 GLOBAL.LVL: every mono record, 0 exceptions).
//!
//! The PS2 never loads a stream file: `FUN_005547b0` reads `info` only and
//! adds the `data` chunk's file position to every segment offset, then the
//! IOP streams 16 KiB blocks from the open handle. The bake instead decodes
//! the segments it wants (`bake::global_sound`).

use crate::soundbank::{info_pairs, sample_records};
use crate::{fnv1a_lowercase, Chunk, UcfbError};

pub struct Segment {
    pub id_hash: u32,
    pub frequency: u32,
    /// Offset into the stream's data payload.
    pub offset: usize,
    /// ADPCM byte length of the audio proper (excludes `padding`).
    pub size: usize,
    /// Silent ADPCM that pads the segment to the block size. Counted by the
    /// info frame count and by the next segment's offset; not decoded.
    pub padding: usize,
    /// Frame count from the info chunk (== (size + padding)/16*28).
    pub frames: u32,
    /// Set when this record aliases another segment's data (GLOBAL.LVL has
    /// none; kept because the record format is the bank's, which does).
    pub alias_of: Option<u32>,
}

pub struct Stream<'a> {
    pub name_hash: u32,
    pub format: u32,
    pub channels: u32,
    pub substreams: u32,
    pub interleave: u32,
    /// Streaming block size (0x4000 = 16 KiB on every stream measured).
    pub block_size: u32,
    /// The `size` header field: the data payload length the header declares.
    pub declared_size: u32,
    pub segments: Vec<Segment>,
    pub data: &'a [u8],
    /// Byte offset of the `data` PAYLOAD within the FILE (not within the
    /// ucfb payload) — what the PS2 adds to each segment offset, and what a
    /// runtime streamer seeks to. `buf[data_file_offset..]` starts with the
    /// first segment's ADPCM.
    pub data_file_offset: usize,
}

/// Every Stream under a ucfb payload, including the ones nested in `lvl_`
/// wrappers (the per-map SOUND/<planet>.LVL shape). `soundbank`'s wrapper
/// walk already treats StreamList/Stream as leaves, so this consumes them.
pub fn find_streams(inner: &[u8]) -> Vec<Stream<'_>> {
    let list_tag = fnv1a_lowercase("StreamList");
    let stream_tag = fnv1a_lowercase("Stream");
    let mut out = Vec::new();
    crate::soundbank::walk_sound_wrappers(inner, 3, &mut |c: &Chunk<'_>| {
        crate::soundbank::collect_list(c, list_tag, stream_tag, parse_stream, &mut out)
    });
    out
}

fn parse_stream<'a>(stream: &Chunk<'a>) -> Result<Stream<'a>, UcfbError> {
    let bad = || UcfbError::Truncated(stream.offset);
    let info_tag = fnv1a_lowercase("info");
    let data_tag = fnv1a_lowercase("data");
    let (mut info, mut data, mut data_file_offset) = (None, None, 0usize);
    for c in stream.children().flatten() {
        if c.tag_u32() == info_tag {
            info = Some(c.payload);
        } else if c.tag_u32() == data_tag {
            data = Some(c.payload);
            // `Chunk::offset` is relative to the ucfb PAYLOAD, so the file
            // offset of this chunk's header is +8 (the ucfb header) and of
            // its payload another +8 (the chunk header). That payload
            // position is what the PS2 adds to every segment offset.
            data_file_offset = c.offset + 16;
        }
    }
    let (info, data) = (info.ok_or_else(bad)?, data.ok_or_else(bad)?);

    let pairs = info_pairs(info);
    let h_name = fnv1a_lowercase("name");
    let h_size = fnv1a_lowercase("size");
    let h_sample = fnv1a_lowercase("sample");
    let h_channels = fnv1a_lowercase("numchannels");
    let h_block = 0x182F_D58D; // field name unknown; value == the block size

    // Header = the pairs before the first record marker.
    let first_marker = pairs.iter().position(|(_, v)| *v == h_sample);
    let head = &pairs[..first_marker.unwrap_or(pairs.len())];
    let hget = |h: u32| head.iter().find(|(k, _)| *k == h).map(|(_, v)| *v);
    let name_hash = hget(h_name).ok_or_else(bad)?;
    let format = hget(fnv1a_lowercase("format")).unwrap_or(1);
    let channels = hget(h_channels).unwrap_or(1);
    let substreams = hget(fnv1a_lowercase("numsubstreams")).unwrap_or(1);
    let interleave = hget(0x740F_DB0C).unwrap_or(0);

    let mut segments = Vec::new();
    let mut offset = 0usize;
    for rec in sample_records(&pairs) {
        let size = rec.size;
        let padding = rec.padding as usize;
        // Include padding before converting to frames. Hostile u32 sizes
        // can exceed both the frame-count and (on 32-bit) offset ranges.
        let padded_size = u64::from(size) + padding as u64;
        let frames = rec.frames.unwrap_or_else(|| {
            let computed = if format == 5 {
                let blk = 36 * u64::from(channels.max(1));
                padded_size / blk * 64
            } else {
                padded_size / 16 * 28
            };
            u32::try_from(computed).unwrap_or(u32::MAX)
        });
        segments.push(Segment {
            id_hash: rec.id,
            frequency: rec.frequency,
            offset,
            size: size as usize,
            padding,
            frames,
            alias_of: rec.alias_of,
        });
        if rec.alias_of.is_none() {
            offset = usize::try_from(padded_size)
                .ok()
                .and_then(|size| offset.checked_add(size))
                .ok_or_else(|| UcfbError::Malformed("stream segment offset overflow".into()))?;
        }
    }
    Ok(Stream {
        name_hash,
        format,
        channels,
        substreams,
        interleave,
        block_size: hget(h_block).unwrap_or(0),
        declared_size: hget(h_size).unwrap_or(0),
        segments,
        data,
        data_file_offset,
    })
}

impl<'a> Stream<'a> {
    /// The segment's ADPCM, excluding the silent block padding. `None` when
    /// the record points outside the data payload (never on the discs here).
    /// Borrowed from the FILE buffer, not from `self`, so a caller can keep
    /// slices from several streams at once.
    pub fn segment_adpcm(&self, seg: &Segment) -> Option<&'a [u8]> {
        self.data.get(seg.offset..seg.offset.checked_add(seg.size)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks;

    #[test]
    fn hostile_size_fallback_saturates_and_explicit_frames_win() {
        for (size, padding, authored, expected) in [
            (0xffff_fff0u32, 0u32, None, u32::MAX),
            (u32::MAX, u32::MAX, Some(7u32), 7u32),
            (u32::MAX, u32::MAX, None, u32::MAX),
            (0xffff_fff0, 0, Some(7), 7),
            (16, 16, None, 56),
        ] {
            let mut pairs = vec![
                (fnv1a_lowercase("name"), 1),
                (0xB969_BE96, fnv1a_lowercase("sample")),
                (fnv1a_lowercase("id"), 2),
                (fnv1a_lowercase("frequency"), 22050),
                (fnv1a_lowercase("size"), size),
                (fnv1a_lowercase("padding"), padding),
            ];
            if let Some(frames) = authored {
                pairs.push((0x1D48_FEEF, frames));
            }
            let info: Vec<u8> = pairs
                .into_iter()
                .flat_map(|(k, v)| k.to_le_bytes().into_iter().chain(v.to_le_bytes()))
                .collect();
            let mut body = crate::build_chunk(&fnv1a_lowercase("info").to_le_bytes(), &info);
            body.extend(crate::build_chunk(
                &fnv1a_lowercase("data").to_le_bytes(),
                &[],
            ));
            let outer = crate::build_chunk(b"test", &body);
            let c = chunks(&outer).next().unwrap().unwrap();
            let parsed = parse_stream(&c);
            if usize::BITS == 32 && u64::from(size) + u64::from(padding) > u64::from(u32::MAX) {
                assert!(matches!(parsed, Err(UcfbError::Malformed(_))));
            } else {
                let stream = parsed.unwrap();
                assert_eq!(stream.segments[0].frames, expected);
                assert!(stream.segment_adpcm(&stream.segments[0]).is_none());
            }
        }
    }

    /// Data-gated Tier-1: BF1 SOUND/CW.LVL — the clone-wars VO streams, the
    /// smallest real file in this format. Numbers from re-sound-global.md
    /// §0.3/§3 (`cw_vo` 314 segments, `cw_tac_vo` 63 segments / 1,130,496 B).
    #[test]
    fn bf1_cw_lvl_streams() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/CW.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: SOUND/CW.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let streams = find_streams(inner);
        assert_eq!(streams.len(), 2, "CW.LVL ships cw_vo + cw_tac_vo");
        let tac = streams
            .iter()
            .find(|s| s.name_hash == fnv1a_lowercase("cw_tac_vo"))
            .expect("cw_tac_vo");
        assert_eq!(tac.segments.len(), 63);
        assert_eq!(tac.data.len(), 1_130_496);
        assert_eq!(tac.channels, 1);
        assert_eq!(tac.block_size, 0x4000);
        let vo = streams
            .iter()
            .find(|s| s.name_hash == fnv1a_lowercase("cw_vo"))
            .expect("cw_vo");
        assert_eq!(vo.segments.len(), 314);
        for s in &streams {
            // The file offset really addresses the audio: reading the file
            // there gives the payload, not the `data` chunk header (which is
            // what an off-by-8 gives, and what the first version shipped).
            assert_eq!(
                &buf[s.data_file_offset..s.data_file_offset + 16],
                &s.data[..16],
                "stream {:08x} data_file_offset",
                s.name_hash
            );
            // Segments tile the payload exactly, and the header agrees.
            let used: usize = s
                .segments
                .iter()
                .filter(|g| g.alias_of.is_none())
                .map(|g| g.size + g.padding)
                .sum();
            assert_eq!(used, s.data.len(), "stream {:08x}", s.name_hash);
            assert_eq!(used as u32, s.declared_size);
            for g in &s.segments {
                assert!(s.segment_adpcm(g).is_some(), "{:08x}", g.id_hash);
                // The unnamed frame count is the PADDED length's frames.
                assert_eq!(g.frames as usize, (g.size + g.padding) / 16 * 28);
                assert!((4000..48100).contains(&g.frequency), "{}", g.frequency);
            }
        }
    }

    /// A synthetic Stream: two records, the second aliasing the first.
    #[test]
    fn parses_synthetic_stream() {
        let h = fnv1a_lowercase;
        let mut info: Vec<u8> = Vec::new();
        let mut pair = |k: u32, v: u32| {
            info.extend(k.to_le_bytes());
            info.extend(v.to_le_bytes());
        };
        pair(h("name"), 0xAABB_CCDD);
        pair(h("numchannels"), 1);
        pair(0x182F_D58D, 0x4000);
        pair(h("numsegments"), 2);
        pair(h("size"), 32);
        pair(0xB969_BE96, h("sample"));
        pair(h("id"), 0x1111_1111);
        pair(h("frequency"), 22050);
        pair(h("size"), 16);
        pair(0x1D48_FEEF, 56);
        pair(h("padding"), 16);
        pair(0x2E78_9FB4, h("sample"));
        pair(h("id"), 0x2222_2222);
        pair(h("frequency"), 22050);
        pair(h("size"), 16);
        pair(h("alias"), 0x1111_1111);
        let mut stream_payload = crate::build_chunk(b"\x05\x07\xb4\x0f", &info); // info
        stream_payload.extend(crate::build_chunk(b"\xa5\xe2\x72\xd8", &[0u8; 32])); // data
        let outer = crate::build_chunk(b"\x65\x6d\x6f\x5f", &stream_payload); // Stream
        let c = chunks(&outer).next().unwrap().unwrap();
        let s = parse_stream(&c).unwrap();
        assert_eq!(s.name_hash, 0xAABB_CCDD);
        assert_eq!(s.segments.len(), 2);
        assert_eq!(s.segments[0].offset, 0);
        assert_eq!(s.segments[0].size, 16);
        assert_eq!(s.segments[0].padding, 16);
        assert_eq!(s.segments[1].alias_of, Some(0x1111_1111));
        // An alias consumes no data: the next offset would still be 32.
        assert_eq!(s.segments[1].offset, 32);
    }
}
