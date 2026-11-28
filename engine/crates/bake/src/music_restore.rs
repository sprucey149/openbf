//! Offline restoration of indexed music using the runtime's original decoder.
//! Both the frame count and the cue identity survive the lossless FLAC encode.

use crate::audio_restore::AudioRestore;
use crate::global_sound::StreamRon;
use anyhow::{ensure, Context, Result};
use audio::{StreamTake, TakeReader};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn stream(out: &Path, index: &mut StreamRon, worker: &mut AudioRestore) -> Result<()> {
    let body = out.join(manifest::music_body(
        &index.source,
        index.data_offset as u64,
    ));
    for cue in &mut index.segments {
        let take = StreamTake {
            path: body.clone(),
            offset: cue.offset as u64,
            size: cue.size,
            format: index.format,
            channels: index.channels,
            substreams: index.substreams,
            interleave: index.interleave as usize,
            frequency: cue.frequency,
            block_size: index.block_size as usize,
        };
        if take.frames() == 0 {
            continue;
        }
        let path = out.join(manifest::restored_music_take(
            &index.source,
            index.data_offset as u64,
            index.name_hash,
            cue.id,
            cue.offset as u64,
        ));
        let frames = take.frames();
        cue.restored_bytes = restore_take(take, path, worker)
            .with_context(|| format!("restore music {:08x}/{:08x}", index.name_hash, cue.id))?;
        cue.frames = frames;
    }
    Ok(())
}

fn restore_take(take: StreamTake, path: PathBuf, worker: &mut AudioRestore) -> Result<u64> {
    ensure!(take.channels == 2, "expected stereo music");
    let frames = take.frames();
    let rate = take.frequency;
    let mut pcm = Vec::with_capacity(frames * 2);
    let mut reader = TakeReader::open(take)?;
    loop {
        let block = reader.next_block()?;
        if block.is_empty() {
            break;
        }
        for sample in block.into_iter().flatten() {
            pcm.push((sample * 32768.0).round() as i16);
        }
    }
    ensure!(pcm.len() == frames * 2, "decoded music frame count changed");
    worker.apply(rate, &mut pcm)?;
    write_flac(path, rate, &pcm)
}

fn write_flac(path: PathBuf, rate: u32, pcm: &[i16]) -> Result<u64> {
    std::fs::create_dir_all(path.parent().context("music output path has no parent")?)?;
    let scratch = path.with_extension("flac.tmp");
    let mut encoder = Command::new(crate::ogg::ffmpeg_bin())
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "s16le",
            "-ar",
        ])
        .arg(rate.to_string())
        .args([
            "-ac",
            "2",
            "-i",
            "pipe:0",
            "-map_metadata",
            "-1",
            "-fflags",
            "+bitexact",
            "-c:a",
            "flac",
            "-compression_level",
            "3",
            "-f",
            "flac",
        ])
        .arg(&scratch)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;
    let mut input = encoder.stdin.take().context("music encoder has no input")?;
    // Feed bounded writes and close stdin before waiting, including on error.
    let result = (|| -> std::io::Result<()> {
        let mut bytes = Vec::with_capacity(32 * 1024);
        for block in pcm.chunks(16 * 1024) {
            bytes.clear();
            bytes.extend(block.iter().flat_map(|value| value.to_le_bytes()));
            input.write_all(&bytes)?;
        }
        Ok(())
    })();
    drop(input);
    let status = encoder.wait()?;
    if result.is_err() || !status.success() {
        crate::remove_optional_file(&scratch)?;
        result?;
        anyhow::bail!("FLAC encoder exited with {status}");
    }
    let bytes = std::fs::metadata(&scratch)?.len();
    std::fs::rename(scratch, path)?;
    Ok(bytes)
}
