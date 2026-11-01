//! The GLOBAL/CW/GCW VO bake — the maintainer's unified sound plan (i) and (ii)
//! (PLAN Log 2026-09-08 (b); RE: analysis/re-sound-global.md, lead-verified
//! 2026-09-07).
//!
//! BF2 keeps almost none of its speech in the per-map sample banks: 29 of
//! GLOBAL.LVL's 31 streams are mono VO (196.6 MB of SPU2 ADPCM) which the PS2
//! never loads — a mission Lua opens the file and the IOP streams 16 KiB
//! blocks out of it by segment offset — and the per-map `snd_` scripts name
//! those segments through `soundstreamproperties` (`bake sounds` now carries
//! the `(stream, segments)` pair). BF1 does the same from SOUND/CW.LVL and
//! GCW.LVL, which is where its nine-command tactical chatter lives.
//!
//! So the bake walks the streams' `info`, decodes the segments some `snd_`
//! script actually references, and writes one OGG per SEGMENT ID — the id is
//! the only name the data has (0 of 6,457 ids are preimageable, §2). The two
//! stereo MUSIC streams (630 MB) are NOT decoded: the maintainer's decision is that
//! music stays streamed, so they are only indexed — name, block size, and
//! each segment's absolute offset in the LVL, which is what a runtime
//! streamer needs to seek.
//!
//! Output (`bake global-sound <lvl[,lvl]> <outdir> [ref.lvl,...]`):
//!   <outdir>/<stream hash>/<segment id>.ogg
//!   <outdir>/global.ron   the index below
//!
//! What is OURS, not the disc's: the OGG container and [`VO_QUALITY`]
//! (PROVISIONAL — the PS2 plays the ADPCM directly), and the choice to skip
//! segments no shipped script names.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// ffmpeg `-qscale:a` for the VO encode. PROVISIONAL (ours): speech at
/// 22 kHz mono, chosen for size against a source that is already lossy
/// 4-bit ADPCM; nothing on the disc implies a bitrate.
pub const VO_QUALITY: u32 = 3;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SegmentRon {
    pub id: u32,
    pub frequency: u32,
    /// Offset of the segment's ADPCM inside the stream's `data` payload.
    pub offset: usize,
    /// ADPCM bytes of audio, excluding the block padding.
    pub size: usize,
    /// Decoded PCM frames of those bytes (`size/16*28`). The disc's own frame
    /// field counts the padded run instead; see `mungefs::stream`.
    pub frames: usize,
}

fn is_default_one(v: &u32) -> bool {
    *v <= 1
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StreamRon {
    pub name_hash: u32,
    /// File the stream lives in, e.g. "global.lvl" — with `data_offset`, the
    /// two coordinates a runtime streamer needs.
    pub source: String,
    pub channels: u32,
    pub block_size: u32,
    /// Byte offset of the `data` payload within that file.
    pub data_offset: usize,
    pub bytes: usize,
    /// False for the music streams: indexed, never decoded.
    pub baked: bool,
    pub segments: Vec<SegmentRon>,
    #[serde(default, skip_serializing_if = "is_default_one")]
    pub format: u32,
    #[serde(default, skip_serializing_if = "is_default_one")]
    pub substreams: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub interleave: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalSoundRon {
    pub sources: Vec<String>,
    /// ffmpeg -qscale:a the OGGs were written with.
    pub quality: u32,
    pub streams: Vec<StreamRon>,
    /// segment id hash → relative OGG path under this directory. Sorted by
    /// id; an id carried by several streams is written once.
    pub files: Vec<(u32, String)>,
    /// Distinct segment ids the reference LVLs' `snd_` scripts name — the
    /// whole set, including ids that live in streams this bake never saw
    /// (per-map VO). 0 when no reference set was given, which bakes every
    /// VO segment.
    pub referenced: usize,
    /// Distinct VO segment ids no reference script names — left unbaked
    /// (§2: 21.8 MB of GLOBAL is referenced by nothing on the disc, most
    /// likely cut lines). `files.len() + unreferenced` is every distinct VO
    /// id in `streams`.
    pub unreferenced: usize,
}

pub struct Options {
    pub quality: u32,
    /// Encoder threads; 0 = `available_parallelism`.
    pub jobs: usize,
    /// Stop after this many OGGs (0 = no limit). Tests only.
    pub limit: usize,
    /// Write the index but no audio.
    pub index_only: bool,
    /// File name of the index written under the output directory. The
    /// per-map bake writes [`MAP_INDEX`] instead, so a map's own streams and
    /// the game-wide ones are never mistaken for each other.
    pub index: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            quality: VO_QUALITY,
            jobs: 0,
            limit: 0,
            index_only: false,
            index: "global.ron".to_string(),
        }
    }
}

/// Every segment id named by a `soundstreamproperties` in these LVLs.
pub fn referenced_segments(refs: &[String]) -> Result<HashSet<u32>> {
    let mut out = HashSet::new();
    for r in refs {
        let buf = std::fs::read(r).with_context(|| r.clone())?;
        let inner = mungefs::open(&buf).map_err(|e| anyhow::anyhow!("{e}: {r}"))?;
        for p in mungefs::sound_props::find_all(inner) {
            out.extend(p.segments.iter().map(|(id, _)| *id));
        }
    }
    Ok(out)
}

pub fn bake_global_sound(
    sources: &[String],
    outdir: &str,
    refs: &[String],
    opts: &Options,
) -> Result<GlobalSoundRon> {
    let want = if refs.is_empty() {
        None
    } else {
        Some(referenced_segments(refs)?)
    };
    let out = Path::new(outdir);
    std::fs::create_dir_all(out)?;

    let bufs: Vec<Vec<u8>> = sources
        .iter()
        .map(|s| std::fs::read(s).with_context(|| s.clone()))
        .collect::<Result<_>>()?;

    // (source file name, stream) in disc order — the RON's order and the
    // "written once" winner both follow it, so the bake is reproducible.
    let mut streams = Vec::new();
    for (src, buf) in sources.iter().zip(&bufs) {
        let inner = mungefs::open(buf).map_err(|e| anyhow::anyhow!("{e}: {src}"))?;
        let name = Path::new(src)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        for s in mungefs::stream::find_streams(inner) {
            streams.push((name.clone(), s));
        }
    }
    if streams.is_empty() {
        bail!("no Stream chunks in {sources:?}");
    }

    let mut jobs: Vec<Job> = Vec::new();
    let mut claimed: HashSet<u32> = HashSet::new();
    let mut unreferenced = 0usize;
    let mut ron_streams = Vec::new();
    for (si, (src, s)) in streams.iter().enumerate() {
        let baked = s.channels == 1;
        let mut segs = Vec::new();
        for (gi, g) in s.segments.iter().enumerate() {
            let frames = if s.format == 5 {
                let blk = 36 * (s.channels.max(1) as usize) * (s.substreams.max(1) as usize);
                g.size / blk * 64
            } else {
                g.size / 16 * 28
            };
            segs.push(SegmentRon {
                id: g.id_hash,
                frequency: g.frequency,
                offset: g.offset,
                size: g.size,
                frames,
            });
            if !baked || g.alias_of.is_some() {
                continue;
            }
            // Dedupe first: an id carried by two or three streams is written
            // once. That is safe because a repeated id holds byte-identical
            // ADPCM at the same rate wherever it appears — measured, not
            // assumed: BF2 GLOBAL 58 of 58 repeated ids, BF1 CW+GCW 172 of
            // 172 (`bf1_repeated_segment_ids_are_the_same_audio` below).
            // Both counts below are per ID, not per record.
            if !claimed.insert(g.id_hash) {
                continue;
            }
            if want.as_ref().is_some_and(|w| !w.contains(&g.id_hash)) {
                unreferenced += 1;
                continue;
            }
            jobs.push(Job {
                stream: si,
                seg: gi,
                rel: format!("{:08x}/{:08x}.ogg", s.name_hash, g.id_hash),
                id: g.id_hash,
                rate: g.frequency,
            });
        }
        if !baked && !opts.index_only {
            write_music_body(out, src, s.data_file_offset as u64, s.data)?;
        }
        ron_streams.push(StreamRon {
            name_hash: s.name_hash,
            source: src.clone(),
            channels: s.channels,
            block_size: s.block_size,
            data_offset: s.data_file_offset,
            bytes: s.data.len(),
            baked,
            segments: segs,
            format: s.format,
            substreams: s.substreams,
            interleave: s.interleave,
        });
    }
    if opts.limit > 0 {
        jobs.truncate(opts.limit);
    }
    for j in &jobs {
        if let Some(d) = out.join(&j.rel).parent() {
            std::fs::create_dir_all(d)?;
        }
    }

    // ffmpeg is checked HERE, not at entry: the per-map bake ([`bake_map_streams`],
    // run for all 34 map dirs) indexes plenty of LVLs whose streams are music
    // only, and those have nothing to encode. Where there IS audio to write,
    // a missing encoder still fails loudly.
    let files: Vec<(u32, String)> = if opts.index_only || jobs.is_empty() {
        Vec::new()
    } else {
        if !crate::ogg::have_ffmpeg() {
            bail!("global-sound needs ffmpeg on PATH (the VO is encoded to OGG)");
        }
        encode_all(&jobs, &streams, out, opts)?
    };

    let ron = GlobalSoundRon {
        sources: sources
            .iter()
            .map(|s| {
                Path::new(s)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
            })
            .collect(),
        quality: opts.quality,
        streams: ron_streams,
        files,
        referenced: want.as_ref().map_or(0, |w| w.len()),
        unreferenced,
    };
    crate::write_ron(&out.join(&opts.index), &ron)?;
    Ok(ron)
}

/// A music stream's ADPCM to `out/`[`manifest::music_body`], as on the disc
/// (card PACK: the game streams it from its own data). An identical file
/// already there is left alone: a rebake writes 1.7 GB of music otherwise.
fn write_music_body(out: &Path, source: &str, data_offset: u64, data: &[u8]) -> Result<()> {
    let path = out.join(manifest::music_body(source, data_offset));
    if std::fs::metadata(&path).is_ok_and(|m| m.len() == data.len() as u64)
        && std::fs::read(&path).is_ok_and(|old| old == data)
    {
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, data).with_context(|| path.display().to_string())
}

/// Index file the per-map stream bake writes under `<map>/sounds/streams/`.
pub const MAP_INDEX: &str = "streams.ron";

/// The per-map half of the stream bake (queue item VO). A planet
/// SOUND/<PLANET>.LVL carries the map's OWN streams next to its sample banks —
/// the map and era MUSIC (stereo `<map>`, `<planet>{cw,gcw}_music`) and the
/// mono `<map>_emt` stream (measured: 44.1 kHz one-shots of 0.6-8.5 s, one
/// per map at most; what they sound like is not established here) — and the
/// same file's `snd_` names
/// segments of both (re-sound-global.md §1.1 "per-map SOUND/<planet>.LVL = 5
/// StreamLists + `lvl_` wrappers holding SampleBank + `snd_`", §4 "per-map
/// SOUND/<planet>.LVL streams ... are likewise unbaked"). Those ids are in no
/// GLOBAL/CW/GCW stream, so until now the properties naming them resolved to
/// nothing: 8-12 per BF2 map, 30 on tat1 (report
/// `engine/artifacts/loop/reports/20260909_0008_global-sound-runtime.md`).
///
/// Same walk and the same rules as [`bake_global_sound`] — the LVL is its own
/// reference set (only segments its `snd_` names are encoded), mono streams
/// are decoded to OGG, stereo MUSIC streams are indexed and never decoded
/// (the maintainer's decision, PLAN Log 2026-09-08 (b)) — writing [`MAP_INDEX`].
pub fn bake_map_streams(sound_lvl: &str, outdir: &Path) -> Result<GlobalSoundRon> {
    let src = sound_lvl.to_string();
    let opts = Options {
        index: MAP_INDEX.to_string(),
        ..Default::default()
    };
    bake_global_sound(
        std::slice::from_ref(&src),
        &outdir.to_string_lossy(),
        std::slice::from_ref(&src),
        &opts,
    )
}

/// Decode + encode every job, `jobs` threads wide. Each worker writes its own
/// file, so the OUTPUT does not depend on the schedule; the returned list is
/// sorted by segment id.
fn encode_all(
    jobs: &[Job],
    streams: &[(String, mungefs::stream::Stream<'_>)],
    out: &Path,
    opts: &Options,
) -> Result<Vec<(u32, String)>> {
    let n = if opts.jobs > 0 {
        opts.jobs
    } else {
        std::thread::available_parallelism().map_or(4, |p| p.get())
    };
    let cursor = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<(u32, String)>> = Mutex::new(Vec::with_capacity(jobs.len()));
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..n.min(jobs.len().max(1)) {
            scope.spawn(|| loop {
                let i = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(j) = jobs.get(i) else { return };
                let (_, stream) = &streams[j.stream];
                let seg = &stream.segments[j.seg];
                let r = (|| -> Result<()> {
                    let adpcm = stream
                        .segment_adpcm(seg)
                        .with_context(|| format!("segment {:08x} outside its data", j.id))?;
                    let pcm = if stream.format == 5 {
                        crate::adpcm::decode_ima(adpcm)
                    } else {
                        crate::adpcm::decode(adpcm)
                    };
                    // The serial is the segment id: reproducible, and an OGG
                    // taken out of the tree still says which segment it is.
                    let ogg = crate::ogg::encode_mono(&pcm, j.rate, opts.quality, j.id)?;
                    std::fs::write(out.join(&j.rel), ogg).with_context(|| j.rel.clone())?;
                    Ok(())
                })();
                match r {
                    Ok(()) => {
                        results.lock().unwrap().push((j.id, j.rel.clone()));
                        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if d.is_multiple_of(500) {
                            eprintln!("global-sound: {d}/{} segments encoded", jobs.len());
                        }
                    }
                    Err(e) => errors.lock().unwrap().push(format!("{:08x}: {e:#}", j.id)),
                }
            });
        }
    });
    let errs = errors.into_inner().unwrap();
    if !errs.is_empty() {
        bail!("{} segment(s) failed, first: {}", errs.len(), errs[0]);
    }
    let mut files = results.into_inner().unwrap();
    files.sort_unstable();
    Ok(files)
}

/// One segment to decode and encode: an index pair into the stream table,
/// plus what the encoder needs without touching it again.
struct Job {
    stream: usize,
    seg: usize,
    rel: String,
    id: u32,
    rate: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("openbf-globalsound-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    /// Data-gated Tier-1: BF1 SOUND/CW.LVL indexes to the RE's two streams
    /// and bakes a handful of segments to OGG (re-sound-global.md §0.3).
    #[test]
    fn bf1_cw_lvl_indexes_and_bakes() {
        let src = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/CW.LVL"
        )
        .to_string();
        if !Path::new(&src).exists() {
            eprintln!("SKIP: SOUND/CW.LVL not present");
            return;
        }
        let out = scratch("cw");
        let opts = Options {
            limit: 4,
            jobs: 2,
            ..Default::default()
        };
        let r = bake_global_sound(&[src], out.to_str().unwrap(), &[], &opts).expect("bake");
        assert_eq!(r.streams.len(), 2);
        assert_eq!(r.referenced, 0, "no reference set: everything is wanted");
        assert_eq!(r.unreferenced, 0);
        let tac = r
            .streams
            .iter()
            .find(|s| s.name_hash == mungefs::fnv1a_lowercase("cw_tac_vo"))
            .expect("cw_tac_vo");
        assert_eq!(tac.segments.len(), 63);
        assert_eq!(tac.bytes, 1_130_496);
        assert!(tac.baked && tac.channels == 1 && tac.source == "cw.lvl");
        // Segment offsets tile the payload with the block padding.
        let tail = tac.segments.last().unwrap();
        assert!(tail.offset + tail.size <= tac.bytes);
        assert_eq!(r.files.len(), 4);
        for (id, rel) in &r.files {
            let b = std::fs::read(out.join(rel)).expect("ogg written");
            assert_eq!(&b[0..4], b"OggS");
            assert_eq!(u32::from_le_bytes(b[14..18].try_into().unwrap()), *id);
        }
        let index: GlobalSoundRon =
            ron::from_str(&std::fs::read_to_string(out.join("global.ron")).unwrap()).unwrap();
        assert_eq!(index.files, r.files);
        let _ = std::fs::remove_dir_all(&out);
    }

    /// The dedupe's premise: where two streams carry the same segment id,
    /// they carry the same audio. BF1's era pair is where it matters — 723
    /// records over four streams are 519 distinct ids.
    #[test]
    fn bf1_repeated_segment_ids_are_the_same_audio() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/"
        );
        let (Ok(cw), Ok(gcw)) = (
            std::fs::read(format!("{dir}CW.LVL")),
            std::fs::read(format!("{dir}GCW.LVL")),
        ) else {
            eprintln!("SKIP: BF1 CW/GCW.LVL not present");
            return;
        };
        let mut seen: std::collections::HashMap<u32, (&[u8], u32)> =
            std::collections::HashMap::new();
        let (mut records, mut repeated) = (0usize, 0usize);
        for buf in [&cw, &gcw] {
            let inner = mungefs::open(buf).unwrap();
            for st in mungefs::stream::find_streams(inner) {
                for g in &st.segments {
                    records += 1;
                    let audio = st.segment_adpcm(g).expect("in range");
                    if let Some((prev, freq)) = seen.insert(g.id_hash, (audio, g.frequency)) {
                        repeated += 1;
                        assert_eq!(prev, audio, "segment {:08x} differs", g.id_hash);
                        assert_eq!(freq, g.frequency, "segment {:08x} rate", g.id_hash);
                    }
                }
            }
        }
        assert_eq!((records, seen.len(), repeated), (723, 519, 204));
    }

    /// The per-map bake (item VO): BF1 SOUND/TAT.LVL carries the planet's
    /// music (stereo, indexed only) and `tat1_emt` (mono, 9 segments,
    /// 380,384 B of ADPCM), and its own `snd_` names every one of those 9.
    #[test]
    fn tat_lvl_bakes_its_own_emt_stream() {
        let src = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/TAT.LVL"
        );
        if !Path::new(src).exists() {
            eprintln!("SKIP: BF1 SOUND/TAT.LVL not present");
            return;
        }
        let out = scratch("tat-own");
        let r = bake_map_streams(src, &out).expect("bake");
        assert_eq!(r.streams.len(), 5);
        let emt = r
            .streams
            .iter()
            .find(|s| s.name_hash == mungefs::fnv1a_lowercase("tat1_emt"))
            .expect("tat1_emt");
        assert!(emt.baked && emt.channels == 1);
        assert_eq!(emt.segments.len(), 9);
        // The payload carries the 16 KiB block padding; the audio proper is
        // the sum of the segment sizes.
        assert_eq!(emt.bytes, 425_984);
        let adpcm: usize = emt.segments.iter().map(|g| g.size).sum();
        assert_eq!(adpcm, 380_384);
        // Every other stream is the map/era MUSIC: stereo, indexed, never
        // decoded — so the whole encode is those 9 segments.
        assert!(r
            .streams
            .iter()
            .filter(|s| !s.baked)
            .all(|s| s.channels == 2));
        assert_eq!(r.files.len(), 9);
        assert!(r.files.iter().all(|(_, rel)| rel.starts_with("33d74640/")));
        assert_eq!(r.unreferenced, 0);
        let index: GlobalSoundRon =
            ron::from_str(&std::fs::read_to_string(out.join(MAP_INDEX)).unwrap()).unwrap();
        assert_eq!(index.files, r.files);
        let b = std::fs::read(out.join(&r.files[0].1)).expect("ogg written");
        assert_eq!(&b[0..4], b"OggS");
        let _ = std::fs::remove_dir_all(&out);
    }

    /// The reference join: BF1 TAT.LVL's `snd_` names segments of the GCW
    /// streams, so baking GCW.LVL against it wants a strict subset.
    #[test]
    fn tat_references_a_subset_of_gcw() {
        let gcw = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/GCW.LVL"
        )
        .to_string();
        let tat = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/TAT.LVL"
        )
        .to_string();
        if !Path::new(&gcw).exists() || !Path::new(&tat).exists() {
            eprintln!("SKIP: BF1 SOUND LVLs not present");
            return;
        }
        let out = scratch("gcw");
        let opts = Options {
            index_only: true,
            ..Default::default()
        };
        let r = bake_global_sound(&[gcw], out.to_str().unwrap(), &[tat], &opts).expect("bake");
        let total: usize = r.streams.iter().map(|s| s.segments.len()).sum();
        assert!(r.referenced > 0 && r.unreferenced > 0);
        assert!(r.unreferenced < total, "{} of {total}", r.unreferenced);
        // Every tac segment TAT.LVL names is in the tac stream (§3: all 62).
        let tac = r
            .streams
            .iter()
            .find(|s| s.name_hash == mungefs::fnv1a_lowercase("gcw_tac_vo"))
            .expect("gcw_tac_vo");
        assert_eq!(tac.segments.len(), 62);
        let _ = std::fs::remove_dir_all(&out);
    }
}
