//! Background setup runner and game launcher.
//!
//! Runs the native Stage 2 setup pipeline ([`crate::pipeline::run_setup`]) on
//! a background thread, streaming progress and log lines back to the UI thread
//! with 20 Hz repaint throttling. Launching `openbf` spawns a detached child
//! process with any configured `WGPU_BACKEND` override.

use crate::config::LauncherConfig;
use crate::paths::LauncherPaths;
use crate::pipeline::{self, ProgressSink};
use anyhow::{bail, Context, Result};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

/// Minimum interval between routine console-line UI repaints (step headers
/// and completion always wake immediately).
const WAKE_THROTTLE: Duration = Duration::from_millis(50);
/// Maximum log lines retained for the embedded setup console.
const MAX_CONSOLE_LINES: usize = 500;

/// Live progress snapshot read by the UI each frame.
#[derive(Clone, Debug, Default)]
pub struct BuildProgress {
    pub running: bool,
    pub step_index: u8,
    pub step_total: u8,
    pub step_label: String,
    pub last_line: String,
    pub lines: VecDeque<String>,
    pub outcome: Option<BuildOutcome>,
}

/// Terminal state of a completed build run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildOutcome {
    Succeeded,
    Cancelled,
    Failed(String),
}

/// Handle to an active or finished background build.
pub struct BuildHandle {
    progress: Arc<Mutex<BuildProgress>>,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

struct UiProgressSink {
    progress: Arc<Mutex<BuildProgress>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    last_wake: Mutex<Instant>,
}

impl ProgressSink for UiProgressSink {
    fn on_step(&self, index: u8, total: u8, label: &str) {
        if let Ok(mut p) = self.progress.lock() {
            p.step_index = index;
            p.step_total = total;
            p.step_label = label.to_string();
        }
        if let Ok(mut lw) = self.last_wake.lock() {
            *lw = Instant::now();
        }
        (self.wake)();
    }

    fn on_line(&self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Ok(mut p) = self.progress.lock() {
            p.last_line.clear();
            p.last_line.push_str(trimmed);
            if p.lines.len() >= MAX_CONSOLE_LINES {
                p.lines.pop_front();
            }
            p.lines.push_back(trimmed.to_string());
        }
        let should_wake = if let Ok(mut lw) = self.last_wake.lock() {
            let now = Instant::now();
            if now.duration_since(*lw) >= WAKE_THROTTLE {
                *lw = now;
                true
            } else {
                false
            }
        } else {
            true
        };
        if should_wake {
            (self.wake)();
        }
    }
}

impl BuildHandle {
    /// Start the native Stage 2 setup pipeline on a background thread.
    pub fn start(
        paths: &LauncherPaths,
        config: &LauncherConfig,
        on_update: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let progress = Arc::new(Mutex::new(BuildProgress {
            running: true,
            step_index: 0,
            step_total: 6,
            step_label: "Starting setup...".to_string(),
            last_line: String::new(),
            lines: VecDeque::with_capacity(64),
            outcome: None,
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(on_update);

        let sink: Arc<dyn ProgressSink> = Arc::new(UiProgressSink {
            progress: Arc::clone(&progress),
            wake: Arc::clone(&notify),
            last_wake: Mutex::new(Instant::now() - WAKE_THROTTLE),
        });

        let paths_owned = paths.clone();
        let config_owned = config.clone();
        let wait_prog = Arc::clone(&progress);
        let wait_cancel = Arc::clone(&cancel);
        let wait_child = Arc::clone(&child_slot);
        let wait_wake = Arc::clone(&notify);

        std::thread::spawn(move || {
            let result = pipeline::run_setup(
                &paths_owned,
                &config_owned,
                Arc::clone(&wait_cancel),
                wait_child,
                sink,
            );

            let mut p = wait_prog.lock().unwrap();
            p.running = false;
            if wait_cancel.load(Ordering::SeqCst) {
                p.outcome = Some(BuildOutcome::Cancelled);
                p.step_label = "Cancelled".to_string();
            } else {
                match result {
                    Ok(()) => {
                        p.step_index = p.step_total;
                        p.step_label = "Setup complete".to_string();
                        p.outcome = Some(BuildOutcome::Succeeded);
                    }
                    Err(e) => {
                        let err_str = format!("{e:#}");
                        if p.lines.len() >= MAX_CONSOLE_LINES {
                            p.lines.pop_front();
                        }
                        p.lines.push_back(format!("Error: {err_str}"));
                        let detail = pick_failure_line(&p.lines).unwrap_or(err_str);
                        p.outcome = Some(BuildOutcome::Failed(detail));
                    }
                }
            }
            drop(p);
            wait_wake();
        });

        Ok(Self {
            progress,
            cancel,
            child: child_slot,
        })
    }

    pub fn snapshot(&self) -> BuildProgress {
        self.progress.lock().unwrap().clone()
    }

    /// Terminate the active setup step and request pipeline cancellation.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
        }
    }
}

pub(crate) fn find_python() -> Result<(PathBuf, &'static [&'static str])> {
    #[cfg(target_os = "windows")]
    let candidates: &[(&str, &[&str])] = &[("py", &["-3"]), ("python", &[]), ("python3", &[])];
    #[cfg(not(target_os = "windows"))]
    let candidates: &[(&str, &[&str])] = &[("python3", &[]), ("python", &[])];

    for (bin, args) in candidates {
        let mut check = Command::new(bin);
        check
            .args(*args)
            .args([
                "-c",
                "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            check.creation_flags(CREATE_NO_WINDOW);
        }
        if check.status().is_ok_and(|s| s.success()) {
            return Ok((PathBuf::from(bin), args));
        }
    }
    bail!("Python 3.10+ not found on PATH (required for mission generation during setup)")
}

/// Launch the packed `openbf` game executable as a fully detached process.
pub fn launch_game(paths: &LauncherPaths, config: &LauncherConfig) -> Result<()> {
    if !paths.is_game_ready() {
        bail!("openbf is not built yet — run Setup first");
    }
    let mut cmd = Command::new(&paths.game_out);
    if let Some(dir) = paths.game_out.parent() {
        cmd.current_dir(dir);
    }
    if let Some(backend) = config.graphics_api.wgpu_backend() {
        cmd.env("WGPU_BACKEND", backend);
    } else {
        cmd.env_remove("WGPU_BACKEND");
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()
        .with_context(|| format!("launching {}", paths.game_out.display()))?;
    Ok(())
}

fn pick_failure_line(lines: &VecDeque<String>) -> Option<String> {
    let is_noise = |line: &str| line.starts_with("step times:");
    let is_error = |line: &str| {
        line.starts_with("pack:")
            || line.starts_with("Error:")
            || line.starts_with("ERROR")
            || line.contains("Error:")
            || line.starts_with("rebake:")
    };
    lines
        .iter()
        .rev()
        .find(|line| !is_noise(line) && is_error(line))
        .or_else(|| lines.iter().rev().find(|line| !is_noise(line)))
        .or_else(|| lines.back())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_error_line_over_step_times_summary() {
        let mut lines = VecDeque::new();
        lines.push_back("== [5/6] baking game assets ==".to_string());
        lines.push_back("Error: bake common failed (exit code: 1)".to_string());
        lines.push_back("step times: 1/6 00:00, 5/6 03:37; total 03:39".to_string());
        assert_eq!(
            pick_failure_line(&lines).as_deref(),
            Some("Error: bake common failed (exit code: 1)")
        );
    }
}
