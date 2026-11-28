//! OGG/Vorbis encoding with a REPRODUCIBLE Ogg bitstream serial.
//!
//! Encoding is ffmpeg's (`-c:a libvorbis`), as `bake::beep` has always done —
//! but ffmpeg picks the Ogg stream serial from `av_get_random_seed()`, so two
//! encodes of the same PCM differ in the serial and in every page CRC after
//! it (bug M-B4: 24 bytes per `audio/*.ogg`, and the bake-byte gate has to
//! whitelist them). The VO bake writes thousands of files, so it cannot ship
//! that: [`encode_mono`] rewrites the serial of every page to a caller-chosen
//! value and recomputes the page CRCs, which makes the bytes a pure function
//! of (pcm, rate, serial, ffmpeg build).
//!
//! Ogg page header (RFC 3533 §6): `OggS`, version, header type, granule
//! position (8), serial (4) at +14, page sequence (4), CRC (4) at +22,
//! segment count (1) at +26, then that many lacing bytes and their sum of
//! payload bytes. The page CRC is a 32-bit CRC (poly 0x04C11DB7, init 0, no
//! reflection, no final xor) over the whole page with the CRC field zeroed.

use anyhow::{anyhow, bail, Context, Result};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut r = (i as u32) << 24;
        let mut b = 0;
        while b < 8 {
            r = if r & 0x8000_0000 != 0 {
                (r << 1) ^ 0x04c1_1db7
            } else {
                r << 1
            };
            b += 1;
        }
        t[i] = r;
        i += 1;
    }
    t
}
static CRC_TABLE: [u32; 256] = crc_table();

/// The Ogg page checksum (RFC 3533 §6), table-driven: CRC-32 with polynomial
/// 0x04c11db7, initial value 0, neither input nor output reflected and no
/// final xor, over the whole page with the checksum field zeroed. Pinned to
/// known answers from an independent implementation in the tests below.
fn ogg_crc(page: &[u8]) -> u32 {
    let mut c: u32 = 0;
    for &x in page {
        c = (c << 8) ^ CRC_TABLE[(((c >> 24) as u8) ^ x) as usize];
    }
    c
}

/// Set every page's serial to `serial` and recompute its CRC, in place.
/// Rejects malformed page sequences. Earlier pages may already be modified on
/// error; callers must discard unsuccessful results. Returns the page count.
///
/// This is the crate's ONE Ogg page walk: `bake::global_sound` reaches it
/// through [`encode_mono`], `bake::beep` calls it on the file ffmpeg wrote.
pub fn pin_ogg_serial(buf: &mut [u8], serial: u32) -> Result<usize> {
    let mut at = 0usize;
    let mut pages = 0usize;
    while at < buf.len() {
        if buf.len() - at < 27 || &buf[at..at + 4] != b"OggS" {
            bail!("not an Ogg page at {at}");
        }
        let nseg = buf[at + 26] as usize;
        if buf.len() - at < 27 + nseg {
            bail!("truncated lacing table at {at}");
        }
        let body: usize = buf[at + 27..at + 27 + nseg]
            .iter()
            .map(|&b| b as usize)
            .sum();
        let len = 27 + nseg + body;
        if buf.len() - at < len {
            bail!("truncated page at {at}");
        }
        buf[at + 14..at + 18].copy_from_slice(&serial.to_le_bytes());
        buf[at + 22..at + 26].copy_from_slice(&[0, 0, 0, 0]);
        let crc = ogg_crc(&buf[at..at + len]);
        buf[at + 22..at + 26].copy_from_slice(&crc.to_le_bytes());
        at += len;
        pages += 1;
    }
    Ok(pages)
}

/// Resolve the `ffmpeg` executable to invoke: `OPENBF_FFMPEG` when set,
/// else a bundled copy beside this binary (`<exe_dir>/ffmpeg(.exe)`,
/// `<exe_dir>/tools/ffmpeg(.exe)`, `<exe_dir>/openbf_data/tools/ffmpeg(.exe)`),
/// else `"ffmpeg"` on `PATH`.
pub fn ffmpeg_bin() -> std::path::PathBuf {
    let env = std::env::var_os("OPENBF_FFMPEG");
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    resolve_ffmpeg_bin(env.as_deref(), exe_dir.as_deref())
}

fn resolve_ffmpeg_bin(
    env_override: Option<&std::ffi::OsStr>,
    exe_dir: Option<&std::path::Path>,
) -> std::path::PathBuf {
    if let Some(ov) = env_override.filter(|s| !s.is_empty()) {
        return std::path::PathBuf::from(ov);
    }
    let name = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);
    if let Some(dir) = exe_dir {
        for cand in [
            dir.join(&name),
            dir.join("tools").join(&name),
            dir.join("openbf_data").join("tools").join(&name),
        ] {
            if cand.is_file() {
                return cand;
            }
        }
    }
    std::path::PathBuf::from("ffmpeg")
}

/// Encode mono s16 PCM to an OGG/Vorbis stream with the given serial.
/// `quality` is ffmpeg's `-qscale:a` (0..10).
pub fn encode_mono(pcm: &[i16], rate: u32, quality: u32, serial: u32) -> Result<Vec<u8>> {
    let ffmpeg = ffmpeg_bin();
    let mut child = Command::new(&ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-f", "s16le", "-ar"])
        .arg(rate.to_string())
        .args(["-ac", "1", "-i", "-", "-c:a", "libvorbis", "-qscale:a"])
        .arg(quality.to_string())
        .args(["-f", "ogg", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {} (is ffmpeg installed or bundled?)", ffmpeg.display()))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    // s16 LE is the sample's own layout on every target this builds for, but
    // spell it out rather than transmute: the encode must not depend on it.
    let raw: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    // ffmpeg blocks on a full stdout pipe long before a whole VO line fits in
    // stdin's, so the write has to run while this thread drains stdout.
    let writer = std::thread::spawn(move || stdin.write_all(&raw).and_then(|_| stdin.flush()));
    let mut out = Vec::new();
    let mut so = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    so.read_to_end(&mut out)?;
    let w = writer.join().map_err(|_| anyhow!("writer panicked"))?;
    let st = child.wait()?;
    let mut err = String::new();
    if let Some(mut se) = child.stderr.take() {
        let _ = se.read_to_string(&mut err);
    }
    w.context("write pcm to ffmpeg")?;
    if !st.success() {
        bail!("ffmpeg failed ({st}): {}", err.trim());
    }
    if out.is_empty() {
        bail!("ffmpeg produced no output: {}", err.trim());
    }
    pin_ogg_serial(&mut out, serial)?;
    Ok(out)
}

/// True when a usable `ffmpeg` is available via [`ffmpeg_bin`] (`OPENBF_FFMPEG`,
/// bundled beside `bake`, or on `PATH`) — the bake says so rather than writing
/// half a tree (`bake::beep` silently skips its OGGs instead, which is why the
/// beep group's output depends on the machine).
pub fn have_ffmpeg() -> bool {
    Command::new(ffmpeg_bin())
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, rate: u32, hz: f64) -> Vec<i16> {
        (0..n)
            .map(|i| {
                let t = i as f64 / rate as f64;
                (8000.0 * (t * hz * std::f64::consts::TAU).sin()) as i16
            })
            .collect()
    }

    /// The same PCM encodes to the same bytes twice, and the tone survives
    /// (Goertzel on the decode, as `bake/tests/suite/beep.rs` does for the beep).
    #[test]
    fn encode_is_reproducible() {
        if !have_ffmpeg() {
            eprintln!("SKIP: no ffmpeg");
            return;
        }
        let pcm = tone(22050, 22050, 440.0);
        let a = encode_mono(&pcm, 22050, 3, 0x1234_5678).expect("encode");
        let b = encode_mono(&pcm, 22050, 3, 0x1234_5678).expect("encode");
        assert_eq!(a, b, "two encodes of the same PCM differ");
        assert_eq!(&a[0..4], b"OggS");
        assert_eq!(
            u32::from_le_bytes(a[14..18].try_into().unwrap()),
            0x1234_5678
        );
        // A different serial changes the bytes but not the length (the serial
        // and the CRCs are fixed-width).
        let c = encode_mono(&pcm, 22050, 3, 1).expect("encode");
        assert_ne!(a, c);
        assert_eq!(a.len(), c.len());
        assert!(
            a.len() > 1000 && a.len() < pcm.len() * 2,
            "{} bytes",
            a.len()
        );
    }

    /// The rewriter is self-consistent: rewriting an already-rewritten stream
    /// to the same serial is a no-op, and every page is walked.
    #[test]
    fn pin_ogg_serial_is_idempotent_and_walks_every_page() {
        if !have_ffmpeg() {
            eprintln!("SKIP: no ffmpeg");
            return;
        }
        let pcm = tone(44100, 22050, 300.0);
        let mut a = encode_mono(&pcm, 22050, 3, 7).expect("encode");
        let before = a.clone();
        let pages = pin_ogg_serial(&mut a, 7).expect("rewrite");
        assert_eq!(a, before);
        assert!(pages >= 3, "{pages} pages");
    }

    /// A two-page synthetic stream with a junk serial and junk checksums.
    /// Needs no ffmpeg, so the CRC is pinned on every machine.
    fn synthetic() -> Vec<u8> {
        let mut v = Vec::new();
        for (htype, seq, segs, body) in [
            (2u8, 0u32, &[4u8, 3][..], &b"beepogg"[..]),
            (4, 1, &[1][..], &b"x"[..]),
        ] {
            v.extend_from_slice(b"OggS");
            v.extend_from_slice(&[0, htype]);
            v.extend_from_slice(&[0; 8]); // granule
            v.extend_from_slice(&0x1234_5678u32.to_le_bytes()); // serial, to be replaced
            v.extend_from_slice(&seq.to_le_bytes());
            v.extend_from_slice(&[0xaa; 4]); // checksum, to be replaced
            v.push(segs.len() as u8);
            v.extend_from_slice(segs);
            v.extend_from_slice(body);
        }
        v
    }

    /// Known answers from an INDEPENDENT implementation of the Ogg CRC
    /// (`engine/artifacts/lead_tmp/bake-determinism/ogg_pages.py`, which
    /// reproduces ffmpeg's own checksums on the three pages of every baked
    /// beep OGG). A change to the polynomial, the seed or the zeroed-field
    /// rule fails here rather than shipping streams no decoder accepts.
    /// (Item 16's test, moved here with the implementation it covers.)
    #[test]
    fn ogg_pages_get_the_serial_and_the_right_crc() {
        let mut v = synthetic();
        assert_eq!(pin_ogg_serial(&mut v, 0xdead_beef).unwrap(), 2);
        assert_eq!(&v[14..18], &0xdead_beefu32.to_le_bytes());
        assert_eq!(&v[36 + 14..36 + 18], &0xdead_beefu32.to_le_bytes());
        assert_eq!(
            u32::from_le_bytes(v[22..26].try_into().unwrap()),
            0x883f_50e7
        );
        assert_eq!(
            u32::from_le_bytes(v[36 + 22..36 + 26].try_into().unwrap()),
            0x5c0f_50d5
        );
        // Only the eight bytes of each page's serial and checksum moved.
        let orig = synthetic();
        let moved: Vec<usize> = (0..v.len()).filter(|&i| v[i] != orig[i]).collect();
        assert!(
            moved.iter().all(|&i| {
                let o = if i < 36 { i } else { i - 36 };
                (14..18).contains(&o) || (22..26).contains(&o)
            }),
            "moved {moved:?}"
        );
    }

    /// Pinning is a pure function of the serial: same serial, same bytes;
    /// different serial, different bytes. This is what makes the bake
    /// reproducible (M-B4), and it needs no ffmpeg.
    #[test]
    fn pinning_depends_only_on_the_serial() {
        let mut one = synthetic();
        let mut two = synthetic();
        pin_ogg_serial(&mut one, 0x87a6_a094).unwrap();
        pin_ogg_serial(&mut two, 0x87a6_a094).unwrap();
        assert_eq!(one, two);
        let mut three = synthetic();
        pin_ogg_serial(&mut three, 0xed7a_186e).unwrap();
        assert_ne!(one, three);
    }

    /// A stream that is not a page sequence is refused.
    #[test]
    fn pin_ogg_serial_rejects_non_ogg() {
        let mut junk = *b"NOTOggS junk payload____";
        assert!(pin_ogg_serial(&mut junk, 1).is_err());
        let mut riff = b"RIFF....WAVEfmt ".to_vec();
        assert!(pin_ogg_serial(&mut riff, 1).is_err());
        let mut short = synthetic();
        short.truncate(30);
        assert!(pin_ogg_serial(&mut short, 1).is_err());
    }

    #[test]
    fn resolve_ffmpeg_prefers_env_then_bundled_then_path() {
        let dir = std::env::temp_dir().join(format!("openbf_ffmpeg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("tools")).unwrap();
        let name = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);

        // Fallback to PATH when nothing is bundled and no env override is set.
        assert_eq!(
            resolve_ffmpeg_bin(None, Some(&dir)),
            std::path::PathBuf::from("ffmpeg")
        );

        // Bundled in tools/ is found.
        let in_tools = dir.join("tools").join(&name);
        std::fs::write(&in_tools, b"stub").unwrap();
        assert_eq!(resolve_ffmpeg_bin(None, Some(&dir)), in_tools);

        // Sibling next to the binary takes precedence over tools/.
        let sibling = dir.join(&name);
        std::fs::write(&sibling, b"stub").unwrap();
        assert_eq!(resolve_ffmpeg_bin(None, Some(&dir)), sibling);

        // Explicit OPENBF_FFMPEG override wins over everything.
        let explicit = std::ffi::OsStr::new("/custom/ffmpeg");
        assert_eq!(
            resolve_ffmpeg_bin(Some(explicit), Some(&dir)),
            std::path::PathBuf::from("/custom/ffmpeg")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
