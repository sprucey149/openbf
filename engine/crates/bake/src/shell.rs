//! The BF2 PS2 shell's own assets (card BSHLA, `analysis/re-bf2-shell.md`
//! §1, §4): SHELL.LVL's sheets ([`SHELL_TEXTURES`]: the `logo`, the Instant
//! Action backdrop and mode icons), the background movie's menu loops and
//! the shell music, into `<out>/shell/` (schema: `manifest::shell`).
//!
//! - **Movie**: `MOVIES/SHELL.MVS` is a `ucfb` whose index (from 0x20) is a
//!   run of (key hash, value) word pairs; a `name` pair opens a segment's
//!   record and its `length` pair gives the segment's bytes; segment k
//!   starts at 0x800 + the earlier lengths, each rounded up to 2048 (§4
//!   [V]). SHELL.LVL's `mcfg` names the properties the menus play by hash
//!   and lists their segments by name hash ([`MENU_MOVIES`]: `shell_main` =
//!   segments 0-1, `shell_sub_left` = 2-9 in the order 2,3,4,5,8,6,7,9).
//!   Each segment is transcoded by ffmpeg to MJPEG at [`JPEG_QUALITY`]
//!   and written as a frame pack (`manifest::shell::FramePack`). The menu
//!   segments carry no sound (§4 [V][M]), so none is kept. `[ours]`: the
//!   transcode (the PS2 decodes the MPEG-2 on its IPU) - ffmpeg is a bake
//!   dependency already (the OGG encodes); the game needs no decoder but a
//!   JPEG one.
//! - **Music**: SOUND/SHELL.LVL baked as a map's bank is (`crate::sounds`):
//!   its `shell_music` stream stays streamed, indexed under `sounds/streams`;
//!   the property the shell plays is [`MUSIC`].

use anyhow::{anyhow, bail, Context, Result};
use manifest::shell::{FramePack, ShellIndex, ShellMovie};
use mungefs::fnv1a_lowercase as fnv;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[path = "shell_pc.rs"]
mod pc;

/// The movie properties the menus play (§4): the title and main menu, the
/// sub-menus.
pub const MENU_MOVIES: [&str; 2] = ["shell_main", "shell_sub_left"];
/// Campaign planet fly-in / transition loops stored in `SHELL.MVS` (segments 10..27).
pub const CAMPAIGN_TRANSITIONS: [&str; 18] = [
    "geo1tran1",
    "myg1tran1",
    "sb2tran1",
    "fel1tran1",
    "sb3tran1",
    "kas2tran1",
    "uta1tran1",
    "cor1tran1",
    "nab1tran1",
    "sb4tran1",
    "mus1tran1",
    "kam1tran1",
    "dea1tran1",
    "pol1tran1",
    "tan1tran1",
    "sb1tran1",
    "yav1tran1",
    "hot1tran1",
];
/// In-game campaign movies stored in `INGAME.MVS` and configured in `INSHELL.LVL`.
pub const INGAME_MOVIES: [&str; 40] = [
    "crawl1", "crawl2", "hot1cam1", "hot1cam2", "geomon01", "geomon02", "mygmon01", "mygmon02",
    "sb2mon01", "sb2mon02", "felmon01", "felmon02", "sb3mon01", "sb3mon02", "kasmon01", "kasmon02",
    "utamon01", "utamon02", "cormon01", "cormon02", "nabmon01", "nabmon02", "sb4mon01", "sb4mon02",
    "musmon01", "musmon02", "kammon01", "kammon02", "deamon01", "deamon02", "polmon01", "polmon02",
    "tanmon01", "tanmon02", "sb1mon01", "sb1mon02", "yavmon01", "yavmon02", "hotmon01", "hotmon02",
];
/// The shell's sheets: the title and main menu's logo (§3.4), the Instant
/// Action backdrop and the five mode icons its lists draw
/// (`analysis/re-bf2-menus.md` §1.1, §6, §8; `mode_icon_eli` is unused and
/// `mode_icon_tdm` is not on the disc), and the Rise of the Empire save
/// list's type icon (`ifs_campaign_load` main.f1 pc 22-33).
pub const SHELL_TEXTURES: [&str; 8] = [
    "logo",
    "iface_instant_action",
    "mode_icon_con",
    "mode_icon_2ctf",
    "mode_icon_ctf",
    "mode_icon_hunt",
    "mode_icon_ass",
    "rote_icon",
];
/// The music the start screen sets (`ifs_start` main.f2, §2).
pub const MUSIC: &str = "shell_soundtrack";
/// ffmpeg's `-q:v` for the frames (`[ours]` PROVISIONAL: 2-5 is visually
/// lossless over the disc's MPEG-2; 3 = about 19 KB a frame).
pub const JPEG_QUALITY: u32 = 3;
/// The movie's rate: NTSC 29.97 (every segment's stream header, §4 [M]).
pub const MOVIE_FPS: (u32, u32) = (30000, 1001);
/// The movie's frame (§4 [V]).
pub const MOVIE_SIZE: [u32; 2] = [512, 448];

/// One MVS segment: its name hash and byte range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub name: u32,
    pub start: usize,
    pub len: usize,
}

/// The segment table of an MVS file.
pub fn mvs_segments(mvs: &[u8]) -> Result<Vec<Segment>> {
    if mvs.get(..4) != Some(b"ucfb") {
        bail!("not a ucfb movie file");
    }
    let word = |at: usize| {
        mvs.get(at..at + 4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    };
    let (name, length) = (fnv("name"), fnv("length"));
    let mut segs = Vec::new();
    let mut at = 0x20;
    // The first record is the file's own (its name, a count): every later
    // `name` opens a segment.
    let mut first = true;
    let mut next_start = 0x800usize;
    while at + 8 <= 0x800 {
        let (Some(k), Some(v)) = (word(at), word(at + 4)) else {
            break;
        };
        if k == name {
            if !first {
                segs.push(Segment {
                    name: v,
                    start: 0,
                    len: 0,
                });
            }
            first = false;
        } else if k == length {
            if let Some(s) = segs.last_mut().filter(|s| s.len == 0) {
                s.start = next_start;
                s.len = v as usize;
                next_start += s.len.div_ceil(2048) * 2048;
            }
        }
        at += 8;
    }
    segs.retain(|s| s.len > 0);
    if segs.is_empty() {
        bail!("no segment records in the movie's index");
    }
    for s in &segs {
        let ok_hdr = mvs.get(s.start..s.start + 4) == Some(&[0, 0, 1, 0xba])
            || mvs.get(s.start..s.start + 3) == Some(b"BIK")
            || is_xmv(mvs.get(s.start..).unwrap_or_default());
        if s.start + s.len > mvs.len() || !ok_hdr {
            bail!(
                "segment {:08x} at {}: no MPEG, BIK or XMV header",
                s.name,
                s.start
            );
        }
    }
    Ok(segs)
}

/// The segments' name hashes of the movie property `property` in the
/// `mcfg` chunks of `lvl` (the LVL's inner content), in play order.
pub fn movie_segments(lvl: &[u8], property: &str) -> Result<Vec<u32>> {
    let content = mungefs::content_chunks(lvl).map_err(|e| anyhow!("{e}"))?;
    let (name, segment) = (fnv("name"), fnv("segment"));
    fn segments(n: &mungefs::config::ConfigNode, segment: u32, out: &mut Vec<u32>) {
        for c in &n.children {
            if c.hash == segment {
                out.extend(c.values.first());
            }
            segments(c, segment, out);
        }
    }
    for c in content.iter().filter(|c| &c.tag == b"mcfg") {
        let nodes = mungefs::config::parse_tree(c).map_err(|e| anyhow!("mcfg: {e}"))?;
        for n in &nodes {
            let named = n
                .find(name)
                .and_then(|x| x.values.first())
                .is_some_and(|h| *h == fnv(property));
            if named {
                let mut out = Vec::new();
                segments(n, segment, &mut out);
                return Ok(out);
            }
        }
    }
    bail!("no movie property {property:?} in the LVL's mcfg")
}

/// Split an MJPEG byte stream (ffmpeg `-f image2pipe`) into its images:
/// a JPEG's scan escapes every 0xFF, so an SOI marker only starts an image.
pub fn split_jpegs(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut starts: Vec<usize> = stream
        .windows(3)
        .enumerate()
        .filter(|(_, w)| w == &[0xff, 0xd8, 0xff])
        .map(|(i, _)| i)
        .collect();
    starts.push(stream.len());
    starts
        .windows(2)
        .map(|w| stream[w[0]..w[1]].to_vec())
        .collect()
}

/// An Xbox movie segment: an XMV, whose 4-byte magic sits 12 bytes in.
fn is_xmv(segment: &[u8]) -> bool {
    segment.get(12..16) == Some(b"xobX")
}

/// The ffmpeg demuxer for a movie segment: PS2 MPEG-2, PC Bink or Xbox XMV.
fn segment_format(segment: &[u8]) -> &'static str {
    if segment.get(..3) == Some(b"BIK") {
        "bink"
    } else if is_xmv(segment) {
        "xmv"
    } else {
        "mpeg"
    }
}

/// Run ffmpeg over one segment with `args` between the input and the output,
/// and return its exit status and what it wrote to stdout. The segment goes
/// through a pipe, save an XMV: that demuxer seeks, and from a pipe it stops
/// after the first few frames, so it reads a temporary file instead.
fn run_ffmpeg(segment: &[u8], args: &[&str]) -> Result<(std::process::ExitStatus, Vec<u8>)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let fmt = segment_format(segment);
    let file = (fmt == "xmv").then(|| {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("openbf-movie-{}-{n}.xmv", std::process::id()))
    });
    struct Remove<'a>(Option<&'a Path>);
    impl Drop for Remove<'_> {
        fn drop(&mut self) {
            if let Some(path) = self.0 {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let _remove = Remove(file.as_deref());
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-f", fmt, "-i"]);
    if let Some(path) = &file {
        std::fs::write(path, segment).with_context(|| path.display().to_string())?;
        cmd.arg(path).stdin(Stdio::null());
    } else {
        cmd.arg("pipe:0").stdin(Stdio::piped());
    }
    let mut child = cmd
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .context("ffmpeg (the shell movie's transcode needs it on PATH)")?;
    let feeder = child.stdin.take().map(|mut stdin| {
        let data = segment.to_vec();
        std::thread::spawn(move || stdin.write_all(&data))
    });
    let out = child.wait_with_output()?;
    if let Some(feeder) = feeder {
        let fed = feeder
            .join()
            .map_err(|_| anyhow!("ffmpeg feeder panicked"))?;
        // ffmpeg may stop reading before the end of a segment (trailing bytes its
        // demuxer rejects) yet exit cleanly with every frame: a closed pipe is then fine.
        if out.status.success() {
            match fed {
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
                other => other.context("ffmpeg stdin")?,
            }
        }
    }
    Ok((out.status, out.stdout))
}

/// One movie segment → its frames as JPEGs.
fn transcode(segment: &[u8]) -> Result<Vec<Vec<u8>>> {
    let q = JPEG_QUALITY.to_string();
    let scale = format!("scale={}:{}", MOVIE_SIZE[0], MOVIE_SIZE[1]);
    let mut args = vec!["-an"];
    if segment_format(segment) != "mpeg" {
        args.extend(["-vf", &scale]);
    }
    args.extend(["-c:v", "mjpeg", "-q:v", &q, "-f", "image2pipe", "pipe:1"]);
    let (status, stdout) = run_ffmpeg(segment, &args)?;
    if !status.success() {
        bail!("ffmpeg failed ({status})");
    }
    let frames = split_jpegs(&stdout);
    if frames.is_empty() {
        bail!("ffmpeg wrote no frames");
    }
    Ok(frames)
}

/// One Bink (PC) or XMV (Xbox) segment's sound as 48 kHz stereo samples, or
/// `None` when it has none. These containers carry the sound beside the
/// picture, where a PS2 segment's comes out of [`demux_pss_audio`]. `[ours]`:
/// ffmpeg decodes it, as it does the picture; the rate and the downmix of
/// Xbox's six-channel monologues are a bake choice.
fn transcode_audio(segment: &[u8]) -> Result<Option<(u16, u32, Vec<i16>)>> {
    if segment_format(segment) == "mpeg" {
        return Ok(None);
    }
    let rate = MOVIE_AUDIO_RATE.to_string();
    let (status, stdout) = run_ffmpeg(
        segment,
        &["-vn", "-map", "0:a:0?", "-ac", "2", "-ar", &rate, "-f", "s16le", "pipe:1"],
    )?;
    // The picture's transcode already vouches for the segment: a sound that
    // will not decode leaves the movie silent rather than failing it.
    if !status.success() || stdout.len() < 4 {
        return Ok(None);
    }
    let pcm = stdout
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    Ok(Some((2, MOVIE_AUDIO_RATE, pcm)))
}

/// The rate [`transcode_audio`] writes a PC or Xbox movie's sound at.
const MOVIE_AUDIO_RATE: u32 = 48_000;

/// Where the shell and campaign movies sit on the PS2 disc.
pub const MVS_ON_DISC: &str = "DATA/_LVL_PS2/MOVIES/SHELL.MVS";
pub const INGAME_MVS_ON_DISC: &str = "DATA/_LVL_PS2/MOVIES/INGAME.MVS";

/// Read a movie file at `path`, or extract `on_disc` from the disc image when
/// `path` is an `.iso`.
pub fn read_movie_entry(path: &str, on_disc: &str) -> Result<Vec<u8>> {
    if !path.to_ascii_lowercase().ends_with(".iso") {
        return Ok(std::fs::read(path)?);
    }
    let f = std::fs::File::open(path)?;
    let mut disc = mungefs::iso::Iso::open(std::io::BufReader::new(f))
        .map_err(|e| anyhow!("not an ISO9660 image ({e})"))?;
    let i = disc
        .find(on_disc)
        .ok_or_else(|| anyhow!("no {on_disc} on the disc"))?;
    let entry = mungefs::iso::IsoFile {
        path: disc.files[i].path.clone(),
        lba: disc.files[i].lba,
        size: disc.files[i].size,
    };
    disc.read_file(&entry)
        .map_err(|e| anyhow!("{on_disc}: {e}"))
}

/// The shell movie file at `path`, or read out of the disc image when `path` is
/// an `.iso` (a dev tree's extracted discs leave `MOVIES/` out; `bake
/// extract-iso` keeps this one file).
pub fn read_movie(path: &str) -> Result<Vec<u8>> {
    read_movie_entry(path, MVS_ON_DISC)
}

/// Say which of `wanted` the copy did not supply: the campaign plays on
/// without them (the front end skips a movie with no bake).
fn report_absent(what: &str, wanted: &[&str], baked: &[ShellMovie]) {
    let absent: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|w| !baked.iter().any(|m| m.name == *w))
        .collect();
    if !absent.is_empty() {
        println!("  {what} not in this copy, left out: {}", absent.join(" "));
    }
}

/// One movie file the campaign cutscenes are read from.
enum MovieFile {
    /// An MVS on disk.
    Path(String),
    /// `INGAME.MVS` inside a PS2 disc image.
    Ps2Disc(String),
    /// A movie inside an Xbox disc image: (image, path on the disc).
    XboxDisc(String, String),
}

impl MovieFile {
    fn read(&self) -> Result<Vec<u8>> {
        match self {
            MovieFile::Path(path) => std::fs::read(path).with_context(|| path.clone()),
            MovieFile::Ps2Disc(path) => {
                read_movie_entry(path, INGAME_MVS_ON_DISC).with_context(|| path.clone())
            }
            MovieFile::XboxDisc(image, entry) => {
                let f = std::fs::File::open(image).with_context(|| image.clone())?;
                let mut disc = mungefs::xdvdfs::Xdvdfs::open(std::io::BufReader::new(f))
                    .map_err(|e| anyhow!("{image}: {e}"))?;
                let i = disc
                    .find(entry)
                    .ok_or_else(|| anyhow!("{image}: no {entry}"))?;
                let file = disc.files[i].clone();
                disc.read_file(&file)
                    .map_err(|e| anyhow!("{image}: {entry}: {e}"))
            }
        }
    }
}

impl std::fmt::Display for MovieFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MovieFile::Path(path) | MovieFile::Ps2Disc(path) => f.write_str(path),
            MovieFile::XboxDisc(image, entry) => write!(f, "{image}: {entry}"),
        }
    }
}

/// The movie files the campaign movie sources name. Each source is an MVS (PS2 and PC `INGAME.MVS`, PC and Xbox `CRAWL.MVS`, an Xbox
/// planet's `<MAP>INGAME.MVS`) or a disc image read in place: a PS2 disc's
/// `INGAME.MVS`, an Xbox disc's crawl and planet movies. Every copy names its
/// segments by the movie's own name hash, so the files' order does not matter.
fn campaign_movie_files(sources: &[String]) -> Result<Vec<MovieFile>> {
    let mut out = Vec::new();
    for source in sources {
        let source = source.as_str();
        let mut head = [0u8; 4];
        {
            use std::io::Read;
            std::fs::File::open(source)
                .and_then(|mut f| f.read_exact(&mut head))
                .with_context(|| source.to_owned())?;
        }
        if &head == b"ucfb" {
            out.push(MovieFile::Path(source.to_owned()));
            continue;
        }
        let f = std::fs::File::open(source).with_context(|| source.to_owned())?;
        if let Ok(disc) = mungefs::xdvdfs::Xdvdfs::open(std::io::BufReader::new(f)) {
            let before = out.len();
            let movies = format!("{}/movies/", crate::xbox::DATA_ROOT.to_ascii_lowercase());
            for file in &disc.files {
                let lower = file.path.to_ascii_lowercase();
                let Some(name) = lower.strip_prefix(&movies) else {
                    continue;
                };
                if name == "crawl.mvs" || name.ends_with("ingame.mvs") {
                    out.push(MovieFile::XboxDisc(source.to_owned(), file.path.clone()));
                }
            }
            if out.len() == before {
                bail!("{source}: no campaign movies on the Xbox disc");
            }
            continue;
        }
        out.push(MovieFile::Ps2Disc(source.to_owned()));
    }
    if out.is_empty() {
        bail!("no campaign movie source");
    }
    Ok(out)
}

fn find_sibling_ci(base: &str, target: &str) -> Option<String> {
    let dir = Path::new(base).parent()?;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(target))
        {
            return Some(entry.path().to_string_lossy().into_owned());
        }
    }
    None
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn decode_pcm_or_vag(audio_type: u32, block: &[u8]) -> Result<Vec<i16>> {
    match audio_type {
        // PS2 SShd type 1 is uncompressed 16-bit signed little-endian PCM.
        1 => Ok(block
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()),
        // PS2 SShd type 0x10 is SPU2 VAG ADPCM.
        0x10 => Ok(crate::adpcm::decode(block)),
        other => bail!("unsupported PSS SShd audio type {other}"),
    }
}

/// Demux the Sony PS2 `SShd` + `SSbd` audio stream carried inside MPEG-2 PS
/// private stream 1 (`0xbd`) packets of a PSS movie segment.
/// Returns `Ok(None)` when the segment carries no `0xbd` audio stream.
pub fn demux_pss_audio(segment: &[u8]) -> Result<Option<(u16, u32, Vec<i16>)>> {
    let mut raw = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= segment.len() {
        let Some(rel) = segment[pos..].windows(3).position(|w| w == [0, 0, 1]) else {
            break;
        };
        let i = pos + rel;
        if i + 4 > segment.len() {
            break;
        }
        let sc = segment[i + 3];
        if sc == 0xba {
            let stuff = segment.get(i + 13).map_or(0, |b| (b & 7) as usize);
            pos = i + 14 + stuff;
        } else if sc == 0xb9 {
            break;
        } else if sc >= 0xbb {
            if i + 6 > segment.len() {
                break;
            }
            let plen = u16::from_be_bytes([segment[i + 4], segment[i + 5]]) as usize;
            let end = (i + 6 + plen).min(segment.len());
            if sc == 0xbd && i + 9 <= end {
                let hl = segment[i + 8] as usize;
                let sub_start = i + 9 + hl;
                if sub_start + 4 <= end {
                    let sub = &segment[sub_start..end];
                    // PS2 PSS audio private stream packets open with 0xff 0xa0 + 2 pad bytes.
                    if sub.starts_with(&[0xff, 0xa0]) {
                        raw.extend_from_slice(&sub[4..]);
                    }
                }
            }
            pos = end.max(i + 4);
        } else {
            pos = i + 4;
        }
    }
    if raw.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(
        raw.len() >= 32 && &raw[..4] == b"SShd",
        "PSS audio missing SShd header (len {})",
        raw.len()
    );
    let hdr_size = le_u32(&raw[4..8]) as usize;
    anyhow::ensure!(
        hdr_size >= 16 && 8 + hdr_size + 8 <= raw.len(),
        "truncated SShd"
    );
    let audio_type = le_u32(&raw[8..12]);
    let sample_rate = le_u32(&raw[12..16]);
    let channels = le_u32(&raw[16..20]);
    let interleave = le_u32(&raw[20..24]) as usize;
    anyhow::ensure!(
        channels == 1 || channels == 2,
        "unsupported PSS channel count {channels}"
    );
    let bd_off = 8 + hdr_size;
    anyhow::ensure!(
        &raw[bd_off..bd_off + 4] == b"SSbd",
        "PSS audio missing SSbd header"
    );
    let bd_size = le_u32(&raw[bd_off + 4..bd_off + 8]) as usize;
    // Clamp rather than asserting equality: INGAME.MVS sb4mon01 is 21 bytes short at EOF.
    let payload_end = (bd_off + 8 + bd_size).min(raw.len());
    let payload = &raw[bd_off + 8..payload_end];

    let pcm = if channels == 1 {
        decode_pcm_or_vag(audio_type, payload)?
    } else {
        anyhow::ensure!(
            interleave >= 2,
            "invalid PSS stereo interleave {interleave}"
        );
        let frame_bytes = interleave * 2;
        let est_samples = (payload.len() / frame_bytes) * interleave;
        let mut out = Vec::with_capacity(est_samples);
        for chunk in payload.chunks_exact(frame_bytes) {
            let l = decode_pcm_or_vag(audio_type, &chunk[..interleave])?;
            let r = decode_pcm_or_vag(audio_type, &chunk[interleave..])?;
            for (sl, sr) in l.into_iter().zip(r) {
                out.push(sl);
                out.push(sr);
            }
        }
        out
    };
    Ok(Some((channels as u16, sample_rate, pcm)))
}

/// A frame pack's file name: the segment's index in the movie and its hash.
pub fn pack_name(index: usize, name: u32) -> String {
    format!("{index:02}_{name:08x}.objm")
}

struct MovieSpec<'a> {
    prop: &'a str,
    prefix: &'a str,
    demux_audio: bool,
    required: bool,
}

fn bake_movie_group(
    specs: &[MovieSpec<'_>],
    lvl: &[u8],
    mvs: &[u8],
    segs: &[Segment],
    pc_shell: bool,
    movie_dir: &Path,
) -> Result<Vec<ShellMovie>> {
    struct Resolved<'a> {
        prop: &'a str,
        demux_audio: bool,
        required: bool,
        segs: Vec<(usize, u32, String)>,
    }

    let mut resolved = Vec::new();
    for spec in specs {
        let order = match movie_segments(lvl, spec.prop) {
            Ok(order) => order,
            Err(error) => {
                if pc_shell {
                    if let Some(order) = pc::movie_order(mvs, spec.prop) {
                        println!(
                            "  PC shell: {} uses the fingerprinted PC galaxy loops",
                            spec.prop
                        );
                        order
                    } else if !spec.required {
                        continue;
                    } else {
                        return Err(error);
                    }
                } else if !spec.required {
                    continue;
                } else {
                    return Err(error);
                }
            }
        };
        let mut items = Vec::with_capacity(order.len());
        let mut missing = false;
        for h in order {
            let Some((k, _)) = segs.iter().enumerate().find(|(_, s)| s.name == h) else {
                if !spec.required {
                    missing = true;
                    break;
                }
                bail!("{}: segment {h:08x} is not in the movie", spec.prop);
            };
            let name = format!("{}{}", spec.prefix, pack_name(k, h));
            items.push((k, h, name));
        }
        if missing || items.is_empty() {
            continue;
        }
        resolved.push(Resolved {
            prop: spec.prop,
            demux_audio: spec.demux_audio,
            required: spec.required,
            segs: items,
        });
    }

    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get().min(8))
        .max(1);
    let next_idx = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<Result<ShellMovie>>>> = (0..resolved.len())
        .map(|_| std::sync::Mutex::new(None))
        .collect();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let idx = next_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(job) = resolved.get(idx) else {
                    break;
                };
                let outcome: Result<ShellMovie> = (|| {
                    let mut files = Vec::with_capacity(job.segs.len());
                    let mut pcm_all: Vec<i16> = Vec::new();
                    let mut audio_fmt: Option<(u16, u32)> = None;
                    for &(k, h, ref name) in &job.segs {
                        let s = &segs[k];
                        let bytes = &mvs[s.start..s.start + s.len];
                        let frames = transcode(bytes)
                            .with_context(|| format!("{}: segment {k}", job.prop))?;
                        let pack =
                            FramePack::write(MOVIE_SIZE[0], MOVIE_SIZE[1], MOVIE_FPS, &frames);
                        std::fs::write(movie_dir.join(name), pack)?;
                        println!(
                            "  {}: segment {k} ({h:08x}): {} frames",
                            job.prop,
                            frames.len()
                        );
                        files.push(name.clone());
                        if job.demux_audio {
                            let sound = if bytes.get(..3) == Some(b"BIK") || is_xmv(bytes) {
                                transcode_audio(bytes)
                            } else {
                                demux_pss_audio(bytes)
                            }
                            .with_context(|| format!("{}: segment {k} audio", job.prop))?;
                            if let Some((ch, rate, mut samples)) = sound {
                                audio_fmt = Some((ch, rate));
                                pcm_all.append(&mut samples);
                            }
                        }
                    }
                    let audio = match (audio_fmt, pcm_all.is_empty()) {
                        (Some((ch, rate)), false) => {
                            let wav = if ch == 2 {
                                crate::adpcm::wav_stereo_s16(rate, &pcm_all)
                            } else {
                                crate::adpcm::wav_mono_s16(rate, &pcm_all)
                            };
                            let wav_name = format!("{}.wav", job.prop);
                            std::fs::write(movie_dir.join(&wav_name), wav)?;
                            Some(wav_name)
                        }
                        _ => None,
                    };
                    Ok(ShellMovie {
                        name: job.prop.to_owned(),
                        segments: files,
                        audio,
                    })
                })();
                *results[idx].lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
            });
        }
    });

    let mut out = Vec::with_capacity(resolved.len());
    for (job, slot) in resolved.iter().zip(results) {
        match slot.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap() {
            Ok(movie) => out.push(movie),
            // An optional movie (a campaign transition) that will not transcode is
            // left out, as if the disc lacked it; the game plays on without it.
            Err(e) if !job.required => {
                eprintln!("warning: {} skipped: {e:#}", job.prop);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Bake the shell into `<out>/shell/`, including menu movies, campaign
/// transitions and (when present) campaign cutscenes from `INSHELL.LVL` + `INGAME.MVS`.
pub fn bake_shell(
    shell_lvl: &str,
    mvs_path: &str,
    sound_lvl: &str,
    core_lvl: &str,
    out: &str,
) -> Result<ShellIndex> {
    bake_shell_with_campaign(shell_lvl, mvs_path, sound_lvl, core_lvl, out, None, None)
}

/// Bake the shell into `<out>/shell/` with an explicit optional `INSHELL.LVL` and
/// the movie files or disc images that hold the Rise of the Empire campaign
/// cutscenes ([`campaign_movie_files`]).
pub fn bake_shell_with_campaign(
    shell_lvl: &str,
    mvs_path: &str,
    sound_lvl: &str,
    core_lvl: &str,
    out: &str,
    inshell_lvl: Option<&str>,
    ingame_mvs: Option<&[String]>,
) -> Result<ShellIndex> {
    let dir = Path::new(out).join("shell");
    let lvl_bytes = std::fs::read(shell_lvl).with_context(|| shell_lvl.to_string())?;
    let lvl = mungefs::open(&lvl_bytes).map_err(|e| anyhow!("{shell_lvl}: {e}"))?;
    let pc_shell = pc::known_shell(&lvl_bytes);

    // The sheets.
    let tex_dir = dir.join("textures");
    std::fs::create_dir_all(&tex_dir)?;
    let mut textures = Vec::new();
    for c in mungefs::content_chunks(lvl)
        .map_err(|e| anyhow!("{e}"))?
        .iter()
        .filter(|c| &c.tag == b"tex_")
    {
        let Some(n) = c.child_str(b"NAME") else {
            continue;
        };
        let n = n.to_ascii_lowercase();
        if SHELL_TEXTURES.contains(&n.as_str()) && !textures.contains(&n) {
            let mut t = mungefs::texture::Texture::parse(c)
                .map_err(|e| anyhow!("{shell_lvl} tex_ 0x{:X}: {e}", c.offset))?;
            register_xbox_backdrop(c, &mut t);
            crate::detail::write_ktx2(&tex_dir, &format!("{n}.ktx2"), &t)?;
            textures.push(n);
        }
    }
    for want in SHELL_TEXTURES {
        if !textures.iter().any(|t| t == want) {
            if pc_shell && want == "iface_instant_action" {
                std::fs::write(
                    tex_dir.join("iface_instant_action.ktx2"),
                    crate::ktx2w::write_rgba8(512, 512, &[pc::backdrop()]),
                )?;
                textures.push(want.to_owned());
                println!("  PC shell: original Instant Action backdrop; console screen layout");
                continue;
            }
            bail!("{shell_lvl}: no tex_ {want}");
        }
    }

    // The movie's menu loops and campaign planet transitions.
    let mvs = read_movie(mvs_path).with_context(|| mvs_path.to_string())?;
    let segs = mvs_segments(&mvs).with_context(|| mvs_path.to_string())?;
    let movie_dir = dir.join("movies");
    std::fs::create_dir_all(&movie_dir)?;

    let mut shell_specs: Vec<MovieSpec<'_>> = MENU_MOVIES
        .iter()
        .map(|&prop| MovieSpec {
            prop,
            prefix: "",
            demux_audio: false,
            required: true,
        })
        .collect();
    if !pc_shell {
        shell_specs.extend(CAMPAIGN_TRANSITIONS.iter().map(|&prop| MovieSpec {
            prop,
            prefix: "",
            demux_audio: true,
            required: false,
        }));
    }
    let mut movies = bake_movie_group(&shell_specs, lvl, &mvs, &segs, pc_shell, &movie_dir)?;
    if pc_shell {
        println!("  campaign transitions: the PC copy has none; the campaign plays without them");
    } else {
        report_absent("campaign transitions", &CAMPAIGN_TRANSITIONS, &movies);
    }

    // Campaign crawls & monologues (`INSHELL.LVL` + the movies that hold them), if available.
    let auto_inshell = inshell_lvl
        .map(str::to_owned)
        .or_else(|| find_sibling_ci(shell_lvl, "INSHELL.LVL"));
    let auto_ingame = ingame_mvs.map(<[String]>::to_vec).or_else(|| {
        let beside: Vec<String> = ["INGAME.MVS", "CRAWL.MVS"]
            .iter()
            .filter_map(|name| find_sibling_ci(mvs_path, name))
            .collect();
        if !beside.is_empty() {
            Some(beside)
        } else {
            mvs_path
                .to_ascii_lowercase()
                .ends_with(".iso")
                .then(|| vec![mvs_path.to_owned()])
        }
    });
    if let (Some(in_lvl_path), Some(in_mvs_paths)) = (auto_inshell, auto_ingame) {
        let explicit = inshell_lvl.is_some() && ingame_mvs.is_some();
        let found = std::fs::read(&in_lvl_path)
            .with_context(|| in_lvl_path.clone())
            .and_then(|bytes| Ok((bytes, campaign_movie_files(&in_mvs_paths)?)));
        match found {
            Ok((in_lvl_bytes, files)) => {
                let in_lvl =
                    mungefs::open(&in_lvl_bytes).map_err(|e| anyhow!("{in_lvl_path}: {e}"))?;
                let before = movies.len();
                for file in &files {
                    let in_mvs = file.read()?;
                    let in_segs = mvs_segments(&in_mvs).with_context(|| file.to_string())?;
                    let in_specs: Vec<MovieSpec<'_>> = INGAME_MOVIES
                        .iter()
                        .filter(|prop| !movies[before..].iter().any(|m| m.name == **prop))
                        .map(|&prop| MovieSpec {
                            prop,
                            prefix: "ig_",
                            demux_audio: true,
                            required: false,
                        })
                        .collect();
                    movies.extend(bake_movie_group(
                        &in_specs, in_lvl, &in_mvs, &in_segs, false, &movie_dir,
                    )?);
                }
                report_absent("campaign cutscenes", &INGAME_MOVIES, &movies[before..]);
            }
            Err(e) if explicit => return Err(e),
            Err(_) => {
                println!("  campaign cutscenes skipped (no INSHELL.LVL / INGAME.MVS)");
            }
        }
    }

    // The music's bank and stream index.
    crate::sounds::bake_sounds(sound_lvl, &dir.to_string_lossy(), Some(core_lvl))?;

    let index = ShellIndex {
        movies,
        textures,
        music: MUSIC.to_owned(),
    };
    crate::write_ron(&dir.join("shell.ron"), &index)?;
    Ok(index)
}

/// [M][O] Native disc-art landmarks in the shared 512x448 logical frame:
/// Xbox body rows [75,438), PS2 [60,386); bright center housing top
/// 160..175 vs 132..147, bottom 280..291 vs 248..261. Xbox dark panels
/// x~28/477 vs PS2 ~15/483; the red sign is 215..290,121..143 vs
/// 212..286,94..116. These are artwork measurements, not runtime evidence.
/// OURS: register only this native Xbox 512-square sheet to the common
/// overlay. Crop x=[13,503); scale between the body/housing landmarks so
/// both housing edges fit, retaining opaque black outside the body.
/// measure: matched original Xbox 4:3/16:9 menu captures for acceptance.
fn register_xbox_backdrop(chunk: &mungefs::Chunk<'_>, texture: &mut mungefs::texture::Texture) {
    if !texture.name.eq_ignore_ascii_case("iface_instant_action")
        || !mungefs::texture_xbox::is_xbox(chunk)
        || [texture.info.width, texture.info.height] != [512, 512]
    {
        return;
    }
    let source = texture.rgba8(0);
    let mut rgba = [0, 0, 0, 255].repeat(512 * 512);
    for y in 60..386 {
        let (dst0, dst1, src0, src1) = match y {
            60..132 => (60, 132, 75, 160),
            132..148 => (132, 148, 160, 176),
            148..248 => (148, 248, 176, 280),
            248..262 => (248, 262, 280, 292),
            _ => (262, 386, 292, 438),
        };
        // Pixel centers, with taps clamped to the measured content crop.
        let sy = (src0 as f32
            + (y as f32 + 0.5 - dst0 as f32) * (src1 - src0) as f32 / (dst1 - dst0) as f32
            - 0.5)
            .clamp(75.0, 437.0);
        for x in 0..512 {
            let sx = (13.0 + (x as f32 + 0.5) * 490.0 / 512.0 - 0.5).clamp(13.0, 502.0);
            let (ix, iy) = (sx as usize, sy as usize);
            let (fx, fy) = (sx - ix as f32, sy - iy as f32);
            for c in 0..3 {
                let at = |xx, yy| source[(yy * 512 + xx) * 4 + c] as f32;
                let top = at(ix, iy) * (1.0 - fx) + at(ix + 1, iy) * fx;
                let bot = at(ix, iy + 1) * (1.0 - fx) + at(ix + 1, iy + 1) * fx;
                rgba[(y * 512 + x) * 4 + c] = (top * (1.0 - fy) + bot * fy).round() as u8;
            }
        }
    }
    // Discard unregistered source mips; the KTX2 writer derives the chain
    // from the corrected top mip, so minification keeps the same placement.
    texture.info.bpp = 32;
    texture.info.mip_count = 1;
    texture.palette.clear();
    texture.mips = vec![rgba];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbox_backdrop_registers_body_and_housing_only_for_native_sheet() {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = tag.to_vec();
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
            out.resize(out.len().next_multiple_of(4), 0);
            out
        }
        let mut info = Vec::new();
        for v in [512_u16, 512, 1, 1] {
            info.extend(v.to_le_bytes());
        }
        for v in [1_u32, 0x07, 512 * 512 * 4] {
            info.extend(v.to_le_bytes());
        }
        let body = vec![0; 512 * 512 * 4];
        let payload = [chunk(b"INFO", &info), chunk(b"BODY", &body)].concat();
        let native = mungefs::Chunk {
            tag: *b"tex_",
            payload: &payload,
            offset: 0,
        };
        assert!(mungefs::texture_xbox::is_xbox(&native));
        let source: Vec<u8> = (0..512)
            .flat_map(|y| {
                (0..512).flat_map(move |x| {
                    [
                        if (13..503).contains(&x) { 80 } else { 240 },
                        if (160..176).contains(&y) || (280..292).contains(&y) {
                            200
                        } else {
                            0
                        },
                        if (75..438).contains(&y) { 120 } else { 250 },
                        255,
                    ]
                })
            })
            .collect();
        let mut texture = mungefs::texture::Texture {
            name: "IFACE_INSTANT_ACTION".into(),
            info: mungefs::texture::TextureInfo {
                width: 512,
                height: 512,
                bpp: 32,
                detail_compressed: false,
                mip_distance: 0.0,
                mip_count: 2,
            },
            palette: Vec::new(),
            mips: vec![source.clone(), vec![9; 256 * 256 * 4]],
        };
        let pc_payload = [payload.clone(), chunk(b"FMT_", &[])].concat();
        let ps2_payload = [chunk(b"INFO", &info[..14]), chunk(b"BODY", &body)].concat();
        for payload in [&pc_payload, &ps2_payload] {
            let other = mungefs::Chunk {
                tag: *b"tex_",
                payload,
                offset: 0,
            };
            register_xbox_backdrop(&other, &mut texture);
            assert_eq!(texture.mips[0], source);
            assert_eq!(texture.mips.len(), 2);
        }
        texture.name = "logo".into();
        register_xbox_backdrop(&native, &mut texture);
        assert_eq!(texture.mips[0], source);
        texture.name = "iface_instant_action".into();
        register_xbox_backdrop(&native, &mut texture);
        assert_eq!(texture.mips.len(), 1);
        assert_eq!(texture.info.mip_count, 1);
        for y in 0..512 {
            for x in 0..512 {
                let pixel = &texture.mips[0][(y * 512 + x) * 4..][..4];
                if (60..386).contains(&y) {
                    assert_eq!([pixel[0], pixel[2], pixel[3]], [80, 120, 255]);
                } else {
                    assert_eq!(pixel, [0, 0, 0, 255]);
                }
            }
        }
        for y in [132, 146, 248, 249, 251, 260] {
            assert!(
                texture.mips[0][(y * 512 + 256) * 4 + 1] >= 180,
                "housing row {y}"
            );
        }
    }

    /// The campaign cutscenes come from whichever copy the player owns: a
    /// loose MVS (PC `INGAME.MVS` + `CRAWL.MVS`) is read as is, anything else
    /// as a disc image, and an empty list is refused rather than baking none.
    #[test]
    fn campaign_movie_sources_are_read_from_any_copy() {
        let dir = std::env::temp_dir().join(format!("openbf-campaign-movies-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mvs = dir.join("ingame.mvs");
        let crawl = dir.join("crawl, en.mvs");
        let disc = dir.join("disc.iso");
        std::fs::write(&mvs, b"ucfb....").unwrap();
        std::fs::write(&crawl, b"ucfb....").unwrap();
        std::fs::write(&disc, vec![0u8; 4096]).unwrap();
        let names = |files: Vec<MovieFile>| -> Vec<String> {
            files
                .into_iter()
                .map(|f| match f {
                    MovieFile::Path(p) => format!("mvs {p}"),
                    MovieFile::Ps2Disc(p) => format!("ps2 {p}"),
                    MovieFile::XboxDisc(p, e) => format!("xbox {p} {e}"),
                })
                .collect()
        };
        let path = |p: &Path| p.to_string_lossy().into_owned();
        assert_eq!(
            names(campaign_movie_files(&[path(&mvs), path(&crawl)]).unwrap()),
            [format!("mvs {}", path(&mvs)), format!("mvs {}", path(&crawl))]
        );
        assert_eq!(
            names(campaign_movie_files(&[path(&disc)]).unwrap()),
            [format!("ps2 {}", path(&disc))]
        );
        assert!(campaign_movie_files(&[]).is_err());
        assert!(campaign_movie_files(&[path(&dir.join("absent.mvs"))]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn segment_formats_follow_their_headers() {
        assert_eq!(segment_format(b"BIKi...."), "bink");
        let mut xmv = vec![0u8; 16];
        xmv[12..16].copy_from_slice(b"xobX");
        assert_eq!(segment_format(&xmv), "xmv");
        assert_eq!(segment_format(&[0, 0, 1, 0xba]), "mpeg");
    }

    #[test]
    fn jpegs_split_at_their_start_markers() {
        let a = [0xff, 0xd8, 0xff, 0xe0, 1, 2, 0xff, 0x00, 0xff, 0xd9];
        let b = [0xff, 0xd8, 0xff, 0xdb, 7, 0xff, 0xd9];
        let s: Vec<u8> = a.iter().chain(&b).copied().collect();
        assert_eq!(split_jpegs(&s), vec![a.to_vec(), b.to_vec()]);
        assert!(split_jpegs(&[]).is_empty());
    }

    #[test]
    fn demux_pss_audio_decodes_stereo_sshd_and_clamps_short_eof() {
        // Construct a minimal PSS segment: 0xba pack + 0xbd audio PES carrying SShd+SSbd (ilv=4).
        let mut sshd = Vec::new();
        sshd.extend_from_slice(&[0xff, 0xa0, 0x00, 0x00]);
        sshd.extend_from_slice(b"SShd");
        sshd.extend_from_slice(&24u32.to_le_bytes());
        sshd.extend_from_slice(&1u32.to_le_bytes()); // PCM s16 LE
        sshd.extend_from_slice(&48000u32.to_le_bytes());
        sshd.extend_from_slice(&2u32.to_le_bytes()); // stereo
        sshd.extend_from_slice(&4u32.to_le_bytes()); // 4 bytes = 2 samples per channel per block
        sshd.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
        sshd.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
        sshd.extend_from_slice(b"SSbd");
        // Advertise 12 bytes while providing 9 (1 full stereo block of 8 bytes + 1 truncated byte).
        sshd.extend_from_slice(&12u32.to_le_bytes());
        for s in [10i16, 20, -10, -20] {
            sshd.extend_from_slice(&s.to_le_bytes());
        }
        sshd.push(0x99); // short EOF tail like sb4mon01

        let mut seg = Vec::new();
        seg.extend_from_slice(&[0x00, 0x00, 0x01, 0xba]);
        seg.resize(18, 0); // 14 + 0 stuffing (plus 00 at offset 13)
        seg[13] = 0;
        let plen = (3 + sshd.len()) as u16;
        seg.extend_from_slice(&[0x00, 0x00, 0x01, 0xbd]);
        seg.extend_from_slice(&plen.to_be_bytes());
        seg.extend_from_slice(&[0x80, 0x00, 0x00]); // hl = 0
        seg.extend_from_slice(&sshd);

        let (ch, rate, pcm) = demux_pss_audio(&seg).unwrap().expect("audio decoded");
        assert_eq!((ch, rate), (2, 48000));
        // Left [10, 20], Right [-10, -20] -> interleaved [10, -10, 20, -20]
        assert_eq!(pcm, vec![10, -10, 20, -20]);
    }

    /// The baked shell (acceptance 3 of card BSHLA): both menu loops, in
    /// the mcfg's order, 512 x 448, and the shell music's stream indexed.
    /// Skips, saying so, on a bake from before the card.
    #[test]
    fn the_baked_shell_holds_the_menu_loops_and_the_music() {
        let dir = Path::new(crate::BAKED_ROOT).join("common_bf2/shell");
        let Ok(text) = std::fs::read_to_string(dir.join("shell.ron")) else {
            eprintln!("skip: no {} (bake common)", dir.display());
            return;
        };
        let index: ShellIndex = ron::from_str(&text).unwrap();
        let order = |name: &str| -> Vec<usize> {
            let m = index.movies.iter().find(|m| m.name == name).unwrap();
            m.segments.iter().map(|f| f[..2].parse().unwrap()).collect()
        };
        assert_eq!(order("shell_main"), [0, 1]);
        assert_eq!(order("shell_sub_left"), [2, 3, 4, 5, 8, 6, 7, 9]);
        if let Some(m) = index.movies.iter().find(|m| m.name == "geo1tran1") {
            let wav = m.audio.as_ref().expect("geo1tran1 wav");
            assert!(dir.join("movies").join(wav).exists(), "{wav}");
        }
        let first = &index.movies[0].segments[0];
        let pack =
            FramePack::parse(std::fs::read(dir.join("movies").join(first)).unwrap()).unwrap();
        assert_eq!([pack.width, pack.height], MOVIE_SIZE);
        // Segment 0 plays 34.97 s (§4 [M]).
        assert!((pack.seconds() - 34.97).abs() < 0.1, "{}", pack.seconds());
        for k in [0, 300] {
            let f = pack.frame(k).unwrap();
            assert_eq!(&f[..2], &[0xff, 0xd8], "frame {k} is a JPEG");
        }
        for t in SHELL_TEXTURES {
            assert!(dir.join(format!("textures/{t}.ktx2")).exists(), "{t}");
        }
        let streams = std::fs::read_to_string(dir.join("sounds/streams/streams.ron")).unwrap();
        assert!(
            streams.contains(&format!("name_hash: {}", fnv("shell_music"))),
            "the shell_music stream is indexed"
        );
        let bank = std::fs::read_to_string(dir.join("sounds/sounds.ron")).unwrap();
        assert!(bank.contains(&format!("name_hash: {}", fnv(MUSIC))));
    }
}
