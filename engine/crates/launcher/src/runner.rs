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
    pub lines: VecDeque<String>,
    pub outcome: Option<BuildOutcome>,
}

impl BuildProgress {
    /// Append a console line, dropping the oldest past [`MAX_CONSOLE_LINES`].
    fn push_line(&mut self, line: String) {
        if self.lines.len() >= MAX_CONSOLE_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }
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
            p.push_line(trimmed.to_string());
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
    ) -> Self {
        let progress = Arc::new(Mutex::new(BuildProgress {
            running: true,
            step_index: 0,
            step_total: pipeline::STEP_COUNT,
            step_label: "Starting setup...".to_string(),
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
                        p.push_line(format!("Error: {err_str}"));
                        let detail = pick_failure_line(&p.lines).unwrap_or(err_str);
                        p.outcome = Some(BuildOutcome::Failed(detail));
                    }
                }
            }
            drop(p);
            wait_wake();
        });

        Self {
            progress,
            cancel,
            child: child_slot,
        }
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

/// Build the game launch command with launcher context and display backend env vars.
pub(crate) fn build_launch_command(
    paths: &LauncherPaths,
    config: &LauncherConfig,
    force_desktop: bool,
    ext_profile: &extensions::profile::ExtensionProfile,
) -> Command {
    let mut cmd = Command::new(&paths.game_out);
    if let Some(dir) = paths.game_out.parent() {
        cmd.current_dir(dir);
    }
    cmd.env(extensions::profile::LAUNCHED_BY_LAUNCHER_ENV, "1");
    cmd.env(extensions::profile::DATA_DIR_ENV, &paths.data_dir);
    if config.group_maps_by_source {
        cmd.env(extensions::profile::GROUP_MAPS_ENV, "1");
    } else {
        cmd.env_remove(extensions::profile::GROUP_MAPS_ENV);
    }
    if force_desktop {
        cmd.arg("--desktop");
    }
    let vr_active = !force_desktop && ext_profile.is_enabled(extensions::OPENBF_VR_ID);
    if vr_active {
        // OpenXR graphics bindings require Vulkan; ignore a desktop Dx12 choice while VR is active.
        cmd.env_remove("WGPU_BACKEND");
    } else if let Some(backend) = config.graphics_api.wgpu_backend() {
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
    cmd
}

/// Launch the packed `openbf` game executable as a fully detached process.
pub fn launch_game(
    paths: &LauncherPaths,
    config: &LauncherConfig,
    force_desktop: bool,
    ext_profile: &extensions::profile::ExtensionProfile,
) -> Result<()> {
    if !paths.is_game_ready() {
        bail!("openbf is not built yet — run Setup first");
    }
    let mut cmd = build_launch_command(paths, config, force_desktop, ext_profile);
    cmd.spawn()
        .with_context(|| format!("launching {}", paths.game_out.display()))?;
    Ok(())
}

/// True for a console line that reports a failure (shown red, and preferred
/// as the failure summary).
pub(crate) fn is_error_line(line: &str) -> bool {
    ["pack:", "rebake:", "ERROR", "Traceback"]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        || line.contains("Error:")
}

fn pick_failure_line(lines: &VecDeque<String>) -> Option<String> {
    let is_noise = |line: &str| line.starts_with("step times:");
    lines
        .iter()
        .rev()
        .find(|line| !is_noise(line) && is_error_line(line))
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

    #[test]
    fn launch_command_passes_desktop_override_and_clears_dx12_for_vr() {
        let dir = std::env::temp_dir().join(format!("openbf_runner_test_{}", std::process::id()));
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let mut config = LauncherConfig::default();
        config.graphics_api = crate::config::GraphicsApi::Dx12;
        let mut profile = extensions::profile::ExtensionProfile::default();
        profile.set_enabled(extensions::OPENBF_VR_ID, true);

        let vr_cmd = build_launch_command(&paths, &config, false, &profile);
        let vr_args: Vec<_> = vr_cmd.get_args().collect();
        assert!(vr_args.is_empty());
        let vr_envs: std::collections::HashMap<_, _> = vr_cmd
            .get_envs()
            .map(|(k, v)| (k.to_os_string(), v.map(|val| val.to_os_string())))
            .collect();
        assert_eq!(
            vr_envs.get(std::ffi::OsStr::new("WGPU_BACKEND")),
            Some(&None)
        );
        assert_eq!(
            vr_envs
                .get(std::ffi::OsStr::new(extensions::profile::LAUNCHED_BY_LAUNCHER_ENV))
                .and_then(|v| v.as_deref()),
            Some(std::ffi::OsStr::new("1"))
        );

        let desktop_cmd = build_launch_command(&paths, &config, true, &profile);
        let desktop_args: Vec<_> = desktop_cmd.get_args().collect();
        assert_eq!(desktop_args, [std::ffi::OsStr::new("--desktop")]);
        let desktop_envs: std::collections::HashMap<_, _> = desktop_cmd
            .get_envs()
            .map(|(k, v)| (k.to_os_string(), v.map(|val| val.to_os_string())))
            .collect();
        assert_eq!(
            desktop_envs
                .get(std::ffi::OsStr::new("WGPU_BACKEND"))
                .and_then(|v| v.as_deref()),
            Some(std::ffi::OsStr::new("dx12"))
        );
    }
}
