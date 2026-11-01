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
    let pcm = crate::adpcm::decode_bank_sample(format, raw_bytes);
    if pcm.len() != expected_frames {
        return Err(anyhow!(
            "decoded {} frames, info says {}",
            pcm.len(),
            expected_frames
        ));
    }
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
        if let Ok(st) = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&wav_path)
            .args(["-c:a", "libvorbis", "-qscale:a", "6"])
            .arg(&ogg_path)
            .status()
        {
            if st.success() {
                let mut buf =
                    std::fs::read(&ogg_path).with_context(|| ogg_path.display().to_string())?;
                crate::ogg::pin_ogg_serial(&mut buf, ogg_serial(stem))
                    .with_context(|| ogg_path.display().to_string())?;
                std::fs::write(&ogg_path, &buf).with_context(|| ogg_path.display().to_string())?;
                ogg_written = true;
            }
        }
    }
    Ok(BeepBake {
        outputs,
        ogg_written,
    })
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
