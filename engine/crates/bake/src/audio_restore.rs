//! Optional pack-selected PCM restoration, before WAV writing or the single
//! Vorbis encode. Direct bakes without a script keep their decoded PCM verbatim.
//! Each bank task / voice encoder thread owns and reuses its own Python worker.

use anyhow::{ensure, Context, Result};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub(crate) struct AudioRestore {
    config: Option<Config>,
    worker: Option<Worker>,
    failed: bool,
}

struct Config {
    binary: Option<PathBuf>,
    python: OsString,
    script: PathBuf,
    kind: &'static str,
}

impl AudioRestore {
    pub(crate) fn from_env(kind: &'static str) -> Self {
        let config = if let Some(bin) = std::env::var_os("OPENBF_AUDIO_RESTORE_BIN") {
            Some(Config {
                binary: Some(bin.into()),
                python: OsString::new(),
                script: PathBuf::new(),
                kind,
            })
        } else {
            std::env::var_os("OPENBF_AUDIO_RESTORE_SCRIPT").map(|script| Config {
                binary: None,
                python: std::env::var_os("OPENBF_PYTHON").unwrap_or_else(|| "python3".into()),
                script: script.into(),
                kind,
            })
        };
        Self {
            config,
            worker: None,
            failed: false,
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.config.is_some()
    }

    pub(crate) fn apply(&mut self, rate: u32, pcm: &mut [i16]) -> Result<()> {
        let Some(config) = &self.config else {
            return Ok(());
        };
        ensure!(!self.failed, "audio restoration worker previously failed");
        let result = (|| {
            if self.worker.is_none() {
                self.worker = Some(Worker::spawn(config)?);
            }
            self.worker.as_mut().unwrap().apply(rate, pcm)
        })();
        if result.is_err() {
            // Never reuse a partial response or launch a replacement per sample.
            // Dropping the guard also reaps a child that failed mid-request.
            self.worker.take();
            self.failed = true;
        }
        result
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        if let Some(mut worker) = self.worker.take() {
            worker.finish()?;
        }
        Ok(())
    }
}

struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
}

impl Worker {
    fn spawn(config: &Config) -> Result<Self> {
        let mut cmd = if let Some(bin) = &config.binary {
            ensure!(
                bin.is_absolute(),
                "audio restoration binary must be absolute"
            );
            let mut c = Command::new(bin);
            c.args(["--audio-restore-worker", config.kind]);
            c
        } else {
            ensure!(
                config.script.is_absolute(),
                "audio restoration script must be absolute"
            );
            let mut c = Command::new(&config.python);
            c.arg("-I")
                .arg(&config.script)
                .args(["--worker", config.kind]);
            c
        };
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Inherit stderr so diagnostics cannot fill an undrained pipe or
            // contaminate the binary response on stdout.
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("start {} audio restoration worker", config.kind))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn apply(&mut self, rate: u32, pcm: &mut [i16]) -> Result<()> {
        let size = pcm.len().checked_mul(2).context("PCM size overflow")?;
        let size = u32::try_from(size).context("audio restoration payload exceeds u32")?;
        let raw: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        let input = self
            .stdin
            .as_mut()
            .context("audio restoration stdin closed")?;
        input
            .write_all(&rate.to_le_bytes())
            .and_then(|_| input.write_all(&size.to_le_bytes()))
            .and_then(|_| input.write_all(&raw))
            .and_then(|_| input.flush())
            .context("write PCM to audio restoration worker")?;
        let mut restored = vec![0; raw.len()];
        self.stdout
            .as_mut()
            .context("audio restoration stdout closed")?
            .read_exact(&mut restored)
            .context("read PCM from audio restoration worker")?;
        for (sample, bytes) in pcm.iter_mut().zip(restored.chunks_exact(2)) {
            *sample = i16::from_le_bytes([bytes[0], bytes[1]]);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        // Clean header EOF tells the worker that every sample is complete.
        self.stdin.take();
        self.stdout.take();
        let status = self
            .child
            .wait()
            .context("wait for audio restoration worker")?;
        ensure!(
            status.success(),
            "audio restoration worker failed ({status})"
        );
        Ok(())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stdin.take();
        self.stdout.take();
        // Rust's Child does not reap on drop. Cover failed requests, encoding /
        // file errors and unwinding as well as explicit successful finish.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests;
