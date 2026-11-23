//! Native Rust PCM restoration worker for the pack bake.
//!
//! Matches `scripts/restore_audio.py` identically so `bake` can invoke
//! `openbf-launcher --audio-restore-worker <dialogue|effects|music>` without
//! spawning Python per sound bank or voice encoder thread.
//!
//! Wire protocol (identical to `scripts/restore_audio.py`):
//! Each request is `<u32 le sample_rate><u32 le byte_len><s16le PCM>`.
//! Each reply is `<byte_len bytes of s16le PCM>`. Clean EOF ends the worker.

use anyhow::{bail, Context, Result};
use std::f64::consts::TAU;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::process::{Command, Stdio};

const FULL_SCALE: f64 = 32768.0;

#[derive(Clone, Copy, Debug)]
struct Profile {
    channels: usize,
    rumble_hz: f64,
    body_hz: f64,
    body_db: f64,
    presence_db: f64,
}

fn profile_for(kind: &str) -> Result<Profile> {
    // Deliberate presentation choices (mirrors scripts/restore_audio.py PROFILES).
    match kind {
        "dialogue" => Ok(Profile {
            channels: 1,
            rumble_hz: 65.0,
            body_hz: 210.0,
            body_db: 1.6,
            presence_db: 1.4,
        }),
        "effects" => Ok(Profile {
            channels: 1,
            rumble_hz: 28.0,
            body_hz: 160.0,
            body_db: 0.9,
            presence_db: 0.9,
        }),
        "music" => Ok(Profile {
            channels: 2,
            rumble_hz: 16.0,
            body_hz: 145.0,
            body_db: 0.35,
            presence_db: 0.25,
        }),
        other => bail!("unknown audio restoration profile: {other}"),
    }
}

fn decode_s16le(bytes: &[u8]) -> Vec<f64> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64)
        .collect()
}

fn decode_f32le(bytes: &[u8]) -> Vec<f64> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64)
        .collect()
}

fn round_s16(v: f64) -> i16 {
    v.round_ties_even().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

fn window_powers(samples: &[f64], size: usize) -> Vec<f64> {
    samples
        .chunks(size)
        .map(|w| w.iter().map(|&s| s * s).sum::<f64>() / (w.len() as f64))
        .collect()
}

fn noise_floor(samples: &[f64], powers: &[f64], window: usize) -> Option<f64> {
    let peak = powers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let fs2 = FULL_SCALE * FULL_SCALE;
    if peak < fs2 * 10f64.powf(-3.0) {
        return None;
    }
    let floor_min = fs2 * 10f64.powf(-6.8);
    let floor_max = (peak * 10f64.powf(-2.2)).min(fs2 * 10f64.powf(-3.7));
    if powers.len() < 6 {
        return None;
    }
    for start in 2..(powers.len() - 3) {
        let candidate = &powers[start..start + 3];
        let lowest = candidate.iter().copied().fold(f64::INFINITY, f64::min);
        let highest = candidate.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if !(floor_min <= lowest && highest <= floor_max) || highest > 1.65 * lowest {
            continue;
        }
        let s0 = start * window;
        let s1 = ((start + 3) * window).min(samples.len());
        let chunk = &samples[s0..s1];
        let diff: f64 = chunk
            .windows(2)
            .map(|w| {
                let d = w[1] - w[0];
                d * d
            })
            .sum();
        if diff >= 0.16 * (chunk.len() as f64) * highest {
            let avg = candidate.iter().sum::<f64>() / (candidate.len() as f64 * fs2);
            return Some(10.0 * avg.log10());
        }
    }
    None
}

fn ffmpeg_filter(
    pcm: &[u8],
    rate: u32,
    channels: usize,
    filters: &str,
    ffmpeg: &str,
    output_format: &str,
) -> Result<Vec<u8>> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-filter_threads",
        "1",
        "-f",
        "s16le",
        "-ar",
        &rate.to_string(),
        "-ac",
        &channels.to_string(),
        "-i",
        "pipe:0",
        "-af",
        filters,
        "-f",
        output_format,
        "pipe:1",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning {ffmpeg} for audio restoration"))?;
    let mut child_in = child.stdin.take().context("ffmpeg stdin unavailable")?;
    let mut child_out = child.stdout.take().context("ffmpeg stdout unavailable")?;
    let mut child_err = child.stderr.take().context("ffmpeg stderr unavailable")?;

    let input = pcm.to_vec();
    let writer = std::thread::spawn(move || -> io::Result<()> {
        child_in.write_all(&input)?;
        child_in.flush()?;
        drop(child_in);
        Ok(())
    });
    let err_reader = std::thread::spawn(move || -> Vec<u8> {
        let mut buf = Vec::new();
        let _ = child_err.read_to_end(&mut buf);
        buf
    });

    let mut stdout = Vec::new();
    child_out
        .read_to_end(&mut stdout)
        .context("reading ffmpeg audio filter stdout")?;
    let write_res = writer.join().map_err(|_| anyhow::anyhow!("ffmpeg writer panicked"))?;
    let stderr = err_reader.join().unwrap_or_default();
    let status = child.wait().context("waiting for ffmpeg audio filter")?;

    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr).trim().to_string();
        if detail.is_empty() {
            bail!("FFmpeg audio filter failed: {status}");
        } else {
            bail!("FFmpeg audio filter failed: {detail}");
        }
    }
    if let Err(e) = write_res {
        if e.kind() != io::ErrorKind::BrokenPipe {
            return Err(e).context("writing PCM to ffmpeg");
        }
    }

    let expected = pcm.len() * if output_format == "f32le" { 2 } else { 1 };
    if stdout.len() != expected {
        bail!(
            "FFmpeg audio filter changed the sample count (expected {expected}, got {})",
            stdout.len()
        );
    }
    Ok(stdout)
}

fn denoise(pcm: &[u8], rate: u32, floor: f64, ffmpeg: &str) -> Result<Vec<f64>> {
    let delay = 2 * (rate / 80);
    let filters = format!(
        "apad=pad_len={delay},afftdn=nr=6:nf={floor:.2}:gs=5,atrim=start_sample={delay}"
    );
    let out = ffmpeg_filter(pcm, rate, 1, &filters, ffmpeg, "f32le")?;
    Ok(decode_f32le(&out))
}

fn shape_tone(samples: &[f64], rate: u32, profile: Profile) -> Vec<f64> {
    let rate_f = rate as f64;
    let coeff = |hz: f64| -((-TAU * hz / rate_f).exp_m1());
    let ac = coeff(profile.rumble_hz);
    let ab = coeff(profile.body_hz);
    let al = coeff(850.0_f64.min(rate_f * 0.16));
    let ah = coeff(2850.0_f64.min(rate_f * 0.38));
    let body_gain = 10f64.powf(profile.body_db / 20.0) - 1.0;
    let presence_gain = 10f64.powf(profile.presence_db / 20.0) - 1.0;

    let mut output = vec![0.0_f64; samples.len()];
    for ch in 0..profile.channels {
        let mut rumble = samples[ch];
        let mut body = samples[ch];
        let mut low = samples[ch];
        let mut high = samples[ch];
        let mut i = ch;
        while i < samples.len() {
            let current = samples[i];
            rumble += ac * (current - rumble);
            body += ab * (current - body);
            low += al * (current - low);
            high += ah * (current - high);
            output[i] =
                current - rumble + body_gain * (body - rumble) + presence_gain * (high - low);
            i += profile.channels;
        }
    }
    output
}

fn music_edge(
    original_pcm: &[u8],
    restored_pcm: &[u8],
    channels: usize,
    frames: usize,
    reverse: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(original_pcm.len());
    for (i, (oc, rc)) in original_pcm
        .chunks_exact(2)
        .zip(restored_pcm.chunks_exact(2))
        .enumerate()
    {
        let old = i16::from_le_bytes([oc[0], oc[1]]) as f64;
        let new = i16::from_le_bytes([rc[0], rc[1]]) as f64;
        let frame = i / channels;
        let num = if reverse { frames - 1 - frame } else { frame };
        let blend = (num as f64) / (frames as f64);
        out.extend_from_slice(&round_s16(old + blend * (new - old)).to_le_bytes());
    }
    out
}

fn restore_music(
    pcm: &[u8],
    rate: u32,
    frames: usize,
    profile: Profile,
    ffmpeg: &str,
) -> Result<Vec<u8>> {
    let presence = 2600_u32.min(rate / 3);
    let filters = format!(
        "highpass=f={},bass=f={}:g={},equalizer=f={}:t=q:w=1.1:g={},volume=-0.25dB,alimiter=limit=0.99996:level=0:latency=1",
        profile.rumble_hz, profile.body_hz, profile.body_db, presence, profile.presence_db
    );
    let result = ffmpeg_filter(pcm, rate, profile.channels, &filters, ffmpeg, "s16le")?;
    let edge = 1usize.max(((rate as usize) / 120).min(frames / 4));
    let size = edge * 2 * profile.channels;
    let prefix = music_edge(&pcm[..size], &result[..size], profile.channels, edge, false);
    let suffix = music_edge(
        &pcm[pcm.len() - size..],
        &result[result.len() - size..],
        profile.channels,
        edge,
        true,
    );
    let mut out = Vec::with_capacity(result.len());
    out.extend_from_slice(&prefix);
    out.extend_from_slice(&result[size..result.len() - size]);
    out.extend_from_slice(&suffix);
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn tone(
    pcm: &[u8],
    original: &[f64],
    rate: u32,
    kind: &str,
    profile: Profile,
    powers: &[f64],
    window: usize,
    ffmpeg: &str,
) -> Result<Vec<f64>> {
    let floor = if kind == "dialogue" && rate >= 8000 {
        noise_floor(original, powers, window)
    } else {
        None
    };
    let Some(floor) = floor else {
        return Ok(shape_tone(original, rate, profile));
    };

    let mut filtered = denoise(pcm, rate, floor, ffmpeg)?;
    let floor_power = FULL_SCALE * FULL_SCALE * 10f64.powf(floor / 10.0);
    let wet: Vec<f64> = powers
        .iter()
        .map(|&power| 1.0_f64.min(32.0 * floor_power / power.max(floor_power)))
        .collect();
    let mut at = 0usize;
    for i in 0..filtered.len() {
        if (at + 1) * window <= i && at + 1 < wet.len() {
            at += 1;
        }
        let left = wet[at.saturating_sub(1)];
        let amount = left + (wet[at] - left) * (((i - at * window) as f64) / (window as f64));
        let original_sample = original[i];
        filtered[i] = original_sample + amount * (filtered[i] * FULL_SCALE - original_sample);
    }
    Ok(shape_tone(&filtered, rate, profile))
}

/// Restore one interleaved s16le PCM take, preserving sample rate and length.
pub fn process_pcm(pcm: &[u8], rate: u32, kind: &str, ffmpeg: &str) -> Result<Vec<u8>> {
    let profile = profile_for(kind)?;
    let frame_bytes = 2 * profile.channels;
    if rate == 0 || !pcm.len().is_multiple_of(frame_bytes) {
        bail!("Invalid {kind} PCM: {rate} Hz, {} bytes", pcm.len());
    }
    let frames = pcm.len() / frame_bytes;
    if frames < (rate as usize) / 35 {
        return Ok(pcm.to_vec());
    }
    if kind == "music" {
        if frames < 4 {
            return Ok(pcm.to_vec());
        }
        return restore_music(pcm, rate, frames, profile, ffmpeg);
    }

    let original = decode_s16le(pcm);
    let window = 1usize.max((rate as usize) / 20) * profile.channels;
    let powers = window_powers(&original, window);
    let peak_power = powers.iter().copied().fold(0.0_f64, f64::max);
    if peak_power <= 1.0 {
        return Ok(pcm.to_vec());
    }
    let restored = tone(pcm, &original, rate, kind, profile, &powers, window, ffmpeg)?;

    let restored_power = window_powers(&restored, window);
    let mut input_power = 0.0_f64;
    let mut output_power = 0.0_f64;
    for (idx, &p) in powers.iter().enumerate() {
        if p >= peak_power * 0.01 {
            input_power += p;
            output_power += restored_power[idx];
        }
    }
    let peak = restored
        .iter()
        .fold(0.0_f64, |acc, &v| acc.max(v.abs()));
    if output_power <= 0.0 || !output_power.is_finite() || !peak.is_finite() || peak <= 0.0 {
        return Ok(pcm.to_vec());
    }
    let gain = (input_power / output_power)
        .sqrt()
        .min((FULL_SCALE - 1.0) / peak);

    let edge = 1usize.max(((rate as usize) / 120).min(frames / 4));
    let mut out = Vec::with_capacity(pcm.len());
    for (idx, (&old, &new)) in original.iter().zip(restored.iter()).enumerate() {
        let frame = idx / profile.channels;
        let dist = frame.min(frames - 1 - frame).min(edge);
        let blend = (dist as f64) / (edge as f64);
        out.extend_from_slice(&round_s16(old + blend * (gain * new - old)).to_le_bytes());
    }
    Ok(out)
}

/// Run the stdin/stdout binary worker loop for `bake`.
pub fn run_worker(kind: &str) -> Result<()> {
    let _ = profile_for(kind)?;
    let ffmpeg = std::env::var("OPENBF_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let mut header = [0u8; 8];
    loop {
        let mut got = 0usize;
        while got < 8 {
            let n = reader.read(&mut header[got..])?;
            if n == 0 {
                if got == 0 {
                    writer.flush()?;
                    return Ok(());
                }
                bail!("Truncated audio request header");
            }
            got += n;
        }
        let rate = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let mut pcm = vec![0u8; size];
        reader
            .read_exact(&mut pcm)
            .context("Truncated audio request payload")?;
        let restored = process_pcm(&pcm, rate, kind, &ffmpeg)?;
        writer.write_all(&restored)?;
        writer.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_silent_buffers_pass_through_unchanged() {
        let short = vec![0u8; 100];
        assert_eq!(process_pcm(&short, 22050, "effects", "ffmpeg").unwrap(), short);
        let silent = vec![0u8; 22050 * 2];
        assert_eq!(
            process_pcm(&silent, 22050, "dialogue", "ffmpeg").unwrap(),
            silent
        );
    }

    #[test]
    fn effects_tone_shaping_preserves_length_and_boundary_samples() {
        let rate = 22050u32;
        let frames = 4410usize;
        let mut pcm = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let s = (10000.0 * (TAU * 440.0 * (i as f64) / (rate as f64)).sin()) as i16;
            pcm.extend_from_slice(&s.to_le_bytes());
        }
        let out = process_pcm(&pcm, rate, "effects", "ffmpeg").unwrap();
        assert_eq!(out.len(), pcm.len());
        // Edge taper keeps first and last frames identical to original.
        assert_eq!(&out[..2], &pcm[..2]);
        assert_eq!(&out[out.len() - 2..], &pcm[pcm.len() - 2..]);
    }
}
