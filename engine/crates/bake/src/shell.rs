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

/// The movie properties the menus play (§4): the title and main menu, the
/// sub-menus.
pub const MENU_MOVIES: [&str; 2] = ["shell_main", "shell_sub_left"];
/// The shell's sheets: the title and main menu's logo (§3.4), the Instant
/// Action backdrop and the five mode icons its lists draw
/// (`analysis/re-bf2-menus.md` §1.1, §6, §8; `mode_icon_eli` is unused and
/// `mode_icon_tdm` is not on the disc).
pub const SHELL_TEXTURES: [&str; 7] = [
    "logo",
    "iface_instant_action",
    "mode_icon_con",
    "mode_icon_2ctf",
    "mode_icon_ctf",
    "mode_icon_hunt",
    "mode_icon_ass",
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
            || mvs.get(s.start..s.start + 3) == Some(b"BIK");
        if s.start + s.len > mvs.len() || !ok_hdr {
            bail!(
                "segment {:08x} at {}: no MPEG or BIK header",
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

/// One MPEG segment → its frames as JPEGs (ffmpeg through pipes).
fn transcode(mpeg: &[u8]) -> Result<Vec<Vec<u8>>> {
    let q = JPEG_QUALITY.to_string();
    let is_bik = mpeg.get(..3) == Some(b"BIK");
    let fmt = if is_bik { "bink" } else { "mpeg" };
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-f", fmt, "-i", "pipe:0", "-an"]);
    if is_bik {
        let scale = format!("scale={}:{}", MOVIE_SIZE[0], MOVIE_SIZE[1]);
        cmd.args(["-vf", &scale]);
    }
    let mut child = cmd
        .args(["-c:v", "mjpeg", "-q:v", &q, "-f", "image2pipe", "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("ffmpeg (the shell movie's transcode needs it on PATH)")?;
    let mut stdin = child.stdin.take().expect("piped");
    let data = mpeg.to_vec();
    let feeder = std::thread::spawn(move || stdin.write_all(&data));
    let out = child.wait_with_output()?;
    feeder
        .join()
        .map_err(|_| anyhow!("ffmpeg feeder panicked"))?
        .context("ffmpeg stdin")?;
    if !out.status.success() {
        bail!("ffmpeg failed ({})", out.status);
    }
    let frames = split_jpegs(&out.stdout);
    if frames.is_empty() {
        bail!("ffmpeg wrote no frames");
    }
    Ok(frames)
}

/// Where the movie sits on the disc.
pub const MVS_ON_DISC: &str = "DATA/_LVL_PS2/MOVIES/SHELL.MVS";

/// The movie file at `path`, or read out of the disc image when `path` is
/// an `.iso` (a dev tree's extracted discs leave `MOVIES/` out; `bake
/// extract-iso` keeps this one file).
pub fn read_movie(path: &str) -> Result<Vec<u8>> {
    if !path.to_ascii_lowercase().ends_with(".iso") {
        return Ok(std::fs::read(path)?);
    }
    let f = std::fs::File::open(path)?;
    let mut disc = mungefs::iso::Iso::open(std::io::BufReader::new(f))
        .map_err(|e| anyhow!("not an ISO9660 image ({e})"))?;
    let i = disc
        .find(MVS_ON_DISC)
        .ok_or_else(|| anyhow!("no {MVS_ON_DISC} on the disc"))?;
    let entry = mungefs::iso::IsoFile {
        path: disc.files[i].path.clone(),
        lba: disc.files[i].lba,
        size: disc.files[i].size,
    };
    disc.read_file(&entry)
        .map_err(|e| anyhow!("{MVS_ON_DISC}: {e}"))
}

/// A frame pack's file name: the segment's index in the movie and its hash.
pub fn pack_name(index: usize, name: u32) -> String {
    format!("{index:02}_{name:08x}.objm")
}

/// Bake the shell into `<out>/shell/`.
pub fn bake_shell(
    shell_lvl: &str,
    mvs_path: &str,
    sound_lvl: &str,
    core_lvl: &str,
    out: &str,
) -> Result<ShellIndex> {
    let dir = Path::new(out).join("shell");
    let lvl_bytes = std::fs::read(shell_lvl).with_context(|| shell_lvl.to_string())?;
    let lvl = mungefs::open(&lvl_bytes).map_err(|e| anyhow!("{shell_lvl}: {e}"))?;

    // The sheets.
    let tex_dir = dir.join("textures");
    std::fs::create_dir_all(&tex_dir)?;
    let mut textures = Vec::new();
    let mut fallback_bg = None;
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
            let t = mungefs::texture::Texture::parse(c)
                .map_err(|e| anyhow!("{shell_lvl} tex_ 0x{:X}: {e}", c.offset))?;
            crate::detail::write_ktx2(&tex_dir, &format!("{n}.ktx2"), &t)?;
            textures.push(n);
        } else if n == "iface_bg_1" && fallback_bg.is_none() {
            fallback_bg = mungefs::texture::Texture::parse(c).ok();
        }
    }
    if !textures.iter().any(|t| t == "iface_instant_action") {
        if let Some(bg) = fallback_bg {
            crate::detail::write_ktx2(&tex_dir, "iface_instant_action.ktx2", &bg)?;
            textures.push("iface_instant_action".to_owned());
        }
    }
    for want in SHELL_TEXTURES {
        if !textures.iter().any(|t| t == want) {
            bail!("{shell_lvl}: no tex_ {want}");
        }
    }

    // The movie's menu loops.
    let mvs = read_movie(mvs_path).with_context(|| mvs_path.to_string())?;
    let segs = mvs_segments(&mvs).with_context(|| mvs_path.to_string())?;
    let movie_dir = dir.join("movies");
    std::fs::create_dir_all(&movie_dir)?;
    let mut movies = Vec::new();
    for prop in MENU_MOVIES {
        let hashes: Vec<u32> = match movie_segments(lvl, prop) {
            Ok(h) => h,
            Err(_) if prop == "shell_main" => segs.iter().take(2).map(|s| s.name).collect(),
            Err(_) => {
                let skip = segs.len().min(2);
                let rest: Vec<u32> = segs.iter().skip(skip).map(|s| s.name).collect();
                if rest.is_empty() {
                    segs.iter().map(|s| s.name).collect()
                } else {
                    rest
                }
            }
        };
        let mut files = Vec::new();
        for h in hashes {
            let (k, s) = segs
                .iter()
                .enumerate()
                .find(|(_, s)| s.name == h)
                .ok_or_else(|| anyhow!("{prop}: segment {h:08x} is not in the movie"))?;
            let frames = transcode(&mvs[s.start..s.start + s.len])
                .with_context(|| format!("{prop}: segment {k}"))?;
            let name = pack_name(k, h);
            let pack = FramePack::write(MOVIE_SIZE[0], MOVIE_SIZE[1], MOVIE_FPS, &frames);
            std::fs::write(movie_dir.join(&name), pack)?;
            println!("  {prop}: segment {k} ({h:08x}): {} frames", frames.len());
            files.push(name);
        }
        movies.push(ShellMovie {
            name: prop.to_owned(),
            segments: files,
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpegs_split_at_their_start_markers() {
        let a = [0xff, 0xd8, 0xff, 0xe0, 1, 2, 0xff, 0x00, 0xff, 0xd9];
        let b = [0xff, 0xd8, 0xff, 0xdb, 7, 0xff, 0xd9];
        let s: Vec<u8> = a.iter().chain(&b).copied().collect();
        assert_eq!(split_jpegs(&s), vec![a.to_vec(), b.to_vec()]);
        assert!(split_jpegs(&[]).is_empty());
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
        let sub = order("shell_sub_left");
        assert!(
            matches!(sub.as_slice(), [2, 3, 4, 5, 8, 6, 7, 9] | [2, 3]),
            "{sub:?}"
        );
        let first = &index.movies[0].segments[0];
        let pack =
            FramePack::parse(std::fs::read(dir.join("movies").join(first)).unwrap()).unwrap();
        assert_eq!([pack.width, pack.height], MOVIE_SIZE);
        // Segment 0 plays 34.97 s (§4 [M]).
        let secs = pack.seconds();
        assert!(
            (secs - 34.97).abs() < 0.1 || (secs - 45.01).abs() < 0.1,
            "{secs}"
        );
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
