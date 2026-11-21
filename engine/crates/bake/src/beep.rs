//! P1: extract the BF1 loading-screen beep to WAV/OGG.
//!
//! The beep wave is `ui_load_lp` (hash 0xB62DC1E9) in CORE.LVL's `global`
//! SampleBank; on the loading screen it plays as two looping pitched
//! soundproperties: load_xtracking (pitch 2.1) and load_ytracking (pitch 1.8)
//! (RE: analysis/re-sound-banks.md). We emit the native sample plus both
//! as-played renders (same PCM, pitched playback rate), each as WAV always
//! and OGG when ffmpeg is available. ffmpeg draws the Ogg stream serial at
//! random, so every OGG is rewritten to carry a serial derived from its own
//! file name before it is written out (bug M-B4) — the page walk and the
//! checksum are [`crate::ogg::pin_ogg_serial`], shared with the VO bake.

use anyhow::{anyhow, Context, Result};
use std::path::Path;

pub const UI_LOAD_LP_HASH: u32 = 0xB62D_C1E9;

pub struct BeepBake {
    /// (file stem, sample rate, pcm)
    pub outputs: Vec<(String, u32, Vec<i16>)>,
    pub ogg_written: bool,
}

pub fn bake_beep(core_lvl: &[u8], outdir: &str) -> Result<BeepBake> {
    bake_beep_with_path(None, core_lvl, outdir)
}

pub fn bake_beep_with_path(
    core_path: Option<&Path>,
    core_lvl: &[u8],
    outdir: &str,
) -> Result<BeepBake> {
    let inner = mungefs::open(core_lvl).map_err(|e| anyhow!("{e}"))?;
    let global_hash = mungefs::fnv1a_lowercase("global");
    let shallow_banks = mungefs::soundbank::find_banks(inner);
    let deep_banks;
    let bank = match shallow_banks.iter().find(|b| b.name_hash == global_hash) {
        Some(b) => b,
        None => {
            deep_banks = mungefs::soundbank::find_banks_deep(inner);
            deep_banks
                .iter()
                .find(|b| b.name_hash == global_hash)
                .ok_or_else(|| anyhow!("no 'global' SampleBank in CORE.LVL"))?
        }
    };
    let s = bank
        .samples
        .iter()
        .find(|s| s.id_hash == UI_LOAD_LP_HASH)
        .ok_or_else(|| anyhow!("ui_load_lp not found in global bank"))?;
    let bnk_buf = if bank.data.is_empty() {
        core_path
            .and_then(crate::sounds::find_common_bnk)
            .and_then(|p| std::fs::read(p).ok())
    } else {
        None
    };
    let (format, native, expected_frames, raw_bytes): (u32, u32, usize, &[u8]) =
        if !bank.data.is_empty() {
            (
                bank.format,
                s.frequency,
                s.frames as usize,
                &bank.data[s.offset..s.offset + s.size],
            )
        } else if let Some(ref bbuf) = bnk_buf {
            let common = crate::sounds::common_bnk_samples(bbuf);
            let &(fmt, freq, frames, slice) = common
                .get(&UI_LOAD_LP_HASH)
                .ok_or_else(|| anyhow!("ui_load_lp not found in common.bnk"))?;
            (fmt, freq, frames as usize, slice)
        } else {
            return Err(anyhow!("global bank has no local data and no common.bnk"));
        };
    let mut pcm = crate::adpcm::decode_bank_sample(format, raw_bytes);
    if pcm.len() != expected_frames {
        return Err(anyhow!(
            "decoded {} frames, info says {}",
            pcm.len(),
            expected_frames
        ));
    }
    let mut restoration = crate::audio_restore::AudioRestore::from_env("effects");
    restoration.apply(native, &mut pcm)?;
    restoration.finish()?;
    let pitched = |p: f64| (native as f64 * p).round() as u32;
    let outputs = vec![
        ("ui_load_lp".to_string(), native, pcm.clone()),
        ("load_xtracking".to_string(), pitched(2.1), pcm.clone()),
        ("load_ytracking".to_string(), pitched(1.8), pcm),
    ];

    std::fs::create_dir_all(outdir)?;
    let mut ogg_written = false;
    for (stem, rate, pcm) in &outputs {
        let wav = crate::adpcm::wav_mono_s16(*rate, pcm);
        let wav_path = Path::new(outdir).join(format!("{stem}.wav"));
        std::fs::write(&wav_path, wav).with_context(|| stem.clone())?;
        // OGG via ffmpeg when available (kira plays both wav and ogg).
        let ogg_path = Path::new(outdir).join(format!("{stem}.ogg"));
        if encode_ogg("ffmpeg", &wav_path, &ogg_path)? {
            let mut buf =
                std::fs::read(&ogg_path).with_context(|| ogg_path.display().to_string())?;
            crate::ogg::pin_ogg_serial(&mut buf, ogg_serial(stem))
                .with_context(|| ogg_path.display().to_string())?;
            std::fs::write(&ogg_path, &buf).with_context(|| ogg_path.display().to_string())?;
            ogg_written = true;
        }
    }
    Ok(BeepBake {
        outputs,
        ogg_written,
    })
}

/// Encode `wav` to `ogg` with `ffmpeg` (the program to start). Ok(false) when
/// there is no such program, so a bake without ffmpeg keeps its WAVs as
/// before. An ffmpeg that runs and fails is an error naming the file: before,
/// it was skipped without a word (an ffmpeg without libvorbis, such as plain
/// Homebrew ffmpeg, left the beep with no OGG and the bake went on).
/// `-nostdin` and a null stdin keep ffmpeg off the terminal, so a bake run in
/// the background is never stopped waiting for it (SIGTTIN). Neither changes
/// the bytes it writes.
fn encode_ogg(ffmpeg: &str, wav: &Path, ogg: &Path) -> Result<bool> {
    let status = match std::process::Command::new(ffmpeg)
        .args(["-nostdin", "-y", "-loglevel", "error", "-i"])
        .arg(wav)
        .args(["-c:a", "libvorbis", "-qscale:a", "6"])
        .arg(ogg)
        .stdin(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("starting {ffmpeg}")),
    };
    if !status.success() {
        let _ = std::fs::remove_file(ogg);
        return Err(anyhow!(
            "{ffmpeg} could not encode {} ({status}); the bake needs an ffmpeg with the \
             libvorbis encoder (`ffmpeg -hide_banner -encoders | grep libvorbis`)",
            ogg.display()
        ));
    }
    Ok(true)
}

/// The serial the beep's `<stem>.ogg` carries: a hash of the file name and
/// nothing else, so two bakes of the same input produce the same bytes (bug
/// M-B4 — ffmpeg draws the serial at random, and the 24 bytes it moved per
/// file, the serial in each of the three pages and the checksum that covers
/// it, were the whole of the difference between two runs).
fn ogg_serial(stem: &str) -> u32 {
    mungefs::fnv1a_lowercase(&format!("{stem}.ogg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No ffmpeg at all keeps the old WAV-only result; an ffmpeg that runs
    /// and fails (`false` stands in for one without libvorbis) is reported,
    /// and its partial OGG is not left behind.
    #[cfg(unix)]
    #[test]
    fn beep_ogg_encode_reports_a_failed_ffmpeg() {
        let dir = std::env::temp_dir().join(format!("beep-encode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (wav, ogg) = (dir.join("t.wav"), dir.join("t.ogg"));
        std::fs::write(&wav, crate::adpcm::wav_mono_s16(22050, &[0; 64])).unwrap();
        let missing = dir.join("no-such-ffmpeg");
        assert!(!encode_ogg(missing.to_str().unwrap(), &wav, &ogg).unwrap());
        std::fs::write(&ogg, b"partial").unwrap();
        let error = encode_ogg("false", &wav, &ogg).unwrap_err().to_string();
        assert!(error.contains("t.ogg") && error.contains("libvorbis"), "{error}");
        assert!(!ogg.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The serials the three shipped `engine/assets_baked/audio/*.ogg` carry
    /// in all three of their pages (read from the bake of main `f4e4e76` with
    /// `struct.unpack_from('<I', b, 14)`). The bytes must not move when this
    /// rule is refactored, so the rule is pinned to its own output rather than
    /// to a second copy of the hash. The page walk itself is tested in
    /// `crate::ogg`.
    #[test]
    fn the_ogg_serial_is_the_file_name_hash() {
        assert_eq!(ogg_serial("ui_load_lp"), 0x5b79_dc14);
        assert_eq!(ogg_serial("load_xtracking"), 0x0c3d_850e);
        assert_eq!(ogg_serial("load_ytracking"), 0x98b0_9eed);
    }
}
