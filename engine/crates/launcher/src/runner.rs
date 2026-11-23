//! Background `pack.py` runner and game launcher.
//!
//! Drives `scripts/pack.py` in a child process, streaming stdout/stderr lines
//! back to the UI thread and parsing `== [mm:ss] N/6 ...` step headers for a
//! deterministic progress bar. Launching `openbf` spawns a detached child
//! process with any configured `WGPU_BACKEND` override.

use crate::config::{LauncherConfig, Platform};
use crate::paths::LauncherPaths;
use anyhow::{anyhow, bail, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

/// Minimum interval between routine stdout/stderr UI repaints (step headers
/// and completion always wake immediately).
const WAKE_THROTTLE: Duration = Duration::from_millis(50);

/// Live progress snapshot read by the UI each frame.
#[derive(Clone, Debug, Default)]
pub struct BuildProgress {
    pub running: bool,
    pub step_index: u8,
    pub step_total: u8,
    pub step_label: String,
    pub last_line: String,
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

impl BuildHandle {
    /// Start `pack.py` on a background thread.
    pub fn start(
        paths: &LauncherPaths,
        config: &LauncherConfig,
        on_update: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let mut cmd = build_pack_command(paths, config)?;
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .context("starting scripts/pack.py (is Python 3.10+ installed?)")?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let progress = Arc::new(Mutex::new(BuildProgress {
            running: true,
            step_index: 0,
            step_total: 6,
            step_label: "Starting setup...".to_string(),
            last_line: String::new(),
            outcome: None,
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        let child_slot = Arc::new(Mutex::new(Some(child)));
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(on_update);

        if let Some(out) = stdout {
            let prog = Arc::clone(&progress);
            let wake = Arc::clone(&notify);
            std::thread::spawn(move || pump_lines(out, prog, wake));
        }
        if let Some(err) = stderr {
            let prog = Arc::clone(&progress);
            let wake = Arc::clone(&notify);
            std::thread::spawn(move || pump_lines(err, prog, wake));
        }

        let wait_prog = Arc::clone(&progress);
        let wait_cancel = Arc::clone(&cancel);
        let wait_child = Arc::clone(&child_slot);
        let wait_wake = Arc::clone(&notify);
        std::thread::spawn(move || {
            let status = loop {
                let mut guard = wait_child.lock().unwrap();
                let Some(proc) = guard.as_mut() else {
                    break None;
                };
                match proc.try_wait() {
                    Ok(Some(st)) => {
                        *guard = None;
                        break Some(Ok(st));
                    }
                    Ok(None) => {
                        drop(guard);
                        std::thread::sleep(std::time::Duration::from_millis(120));
                    }
                    Err(e) => {
                        *guard = None;
                        break Some(Err(e));
                    }
                }
            };

            let mut p = wait_prog.lock().unwrap();
            p.running = false;
            if wait_cancel.load(Ordering::SeqCst) {
                p.outcome = Some(BuildOutcome::Cancelled);
                p.step_label = "Cancelled".to_string();
            } else {
                match status {
                    Some(Ok(st)) if st.success() => {
                        p.step_index = p.step_total;
                        p.step_label = "Setup complete".to_string();
                        p.outcome = Some(BuildOutcome::Succeeded);
                    }
                    Some(Ok(st)) => {
                        let detail = if p.last_line.is_empty() {
                            format!("pack.py exited with status {st}")
                        } else {
                            p.last_line.clone()
                        };
                        p.outcome = Some(BuildOutcome::Failed(detail));
                    }
                    Some(Err(e)) => {
                        p.outcome = Some(BuildOutcome::Failed(e.to_string()));
                    }
                    None => {
                        p.outcome = Some(BuildOutcome::Cancelled);
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

    /// Terminate the running `pack.py` process.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
        }
    }
}

fn pump_lines(
    reader: impl std::io::Read,
    progress: Arc<Mutex<BuildProgress>>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let buf = BufReader::new(reader);
    let mut last_wake = Instant::now() - WAKE_THROTTLE;
    for line in buf.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut step_changed = false;
        if let Ok(mut p) = progress.lock() {
            if let Some((idx, total, label)) = parse_step_header(trimmed) {
                p.step_index = idx;
                p.step_total = total;
                p.step_label = label;
                step_changed = true;
            }
            p.last_line.clear();
            p.last_line.push_str(trimmed);
        }
        let now = Instant::now();
        if step_changed || now.duration_since(last_wake) >= WAKE_THROTTLE {
            last_wake = now;
            wake();
        }
    }
}

/// Parse `pack.py`'s `== [mm:ss] 2/6 the discs` step headers.
pub fn parse_step_header(line: &str) -> Option<(u8, u8, String)> {
    let rest = line.strip_prefix("== [")?;
    let (_, after_time) = rest.split_once("] ")?;
    let (frac, label) = after_time.split_once(' ')?;
    let (cur, total) = frac.split_once('/')?;
    let cur: u8 = cur.parse().ok()?;
    let total: u8 = total.parse().ok()?;
    if total == 0 || cur > total {
        return None;
    }
    Some((cur, total, label.trim().to_string()))
}

/// Build the `Command` that invokes `scripts/pack.py` for the user's selected
/// sources.
pub fn build_pack_command(paths: &LauncherPaths, config: &LauncherConfig) -> Result<Command> {
    let script = paths
        .pack_script()
        .ok_or_else(|| anyhow!("scripts/pack.py not found beside the launcher or repository"))?;
    let pack_root = paths
        .pack_root
        .as_ref()
        .ok_or_else(|| anyhow!("pack root not found"))?;

    let bf2 = config
        .bf2
        .path
        .as_ref()
        .ok_or_else(|| anyhow!("Battlefront II source is not selected"))?;

    let (python, extra_args) = find_python()?;
    let mut cmd = Command::new(python);
    cmd.args(extra_args);
    cmd.current_dir(pack_root);
    cmd.arg(&script);

    if let Some(bf1) = &config.bf1.path {
        push_source_arg(&mut cmd, 1, config.bf1.platform, bf1);
    } else {
        cmd.arg("--bf2-only");
    }
    push_source_arg(&mut cmd, 2, config.bf2.platform, bf2);

    // Passing `--iso-dir` keeps `pack.py` scoped strictly to the launcher's
    // chosen sources (and optional DLC folder) instead of picking up unrelated
    // files from `iso/`.
    let scan_dir = if let Some(dlc) = &config.dlc_dir {
        dlc.clone()
    } else {
        let empty = paths.data_dir.join("launcher_empty_scan");
        std::fs::create_dir_all(&empty).with_context(|| format!("creating {}", empty.display()))?;
        empty
    };
    cmd.arg("--iso-dir").arg(scan_dir);

    if let Some(prebuilt) = &paths.prebuilt_game_bin {
        cmd.arg("--game-bin").arg(prebuilt);
    }
    cmd.arg("--out").arg(&paths.game_out);
    cmd.env("PYTHONUNBUFFERED", "1");

    Ok(cmd)
}

fn push_source_arg(cmd: &mut Command, game_num: u8, platform: Platform, path: &Path) {
    let flag = match platform {
        Platform::Pc => format!("--pc{game_num}"),
        Platform::Ps2 | Platform::Xbox => format!("--iso{game_num}"),
    };
    cmd.arg(flag).arg(path);
}

fn find_python() -> Result<(PathBuf, &'static [&'static str])> {
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
    bail!("Python 3.10 or newer was not found on PATH")
}

/// Spawn the built `openbf` executable detached with the configured graphics
/// API override (if any).
pub fn launch_game(paths: &LauncherPaths, config: &LauncherConfig) -> Result<()> {
    if !paths.is_game_ready() {
        bail!("OpenBF is not built yet — run Setup first");
    }
    let mut cmd = Command::new(&paths.game_out);
    if let Some(dir) = paths.game_out.parent() {
        cmd.current_dir(dir);
    }
    if let Some(backend) = config.graphics_api.wgpu_backend() {
        cmd.env("WGPU_BACKEND", backend);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pack_step_headers() {
        assert_eq!(
            parse_step_header("== [00:12] 2/6 the discs"),
            Some((2, 6, "the discs".to_string()))
        );
        assert_eq!(
            parse_step_header("== [95:12] 6/6 pack"),
            Some((6, 6, "pack".to_string()))
        );
        assert_eq!(parse_step_header("   already there"), None);
        assert_eq!(parse_step_header("== [00:01] 7/6 bogus"), None);
    }

    #[test]
    fn pack_command_maps_platforms_dlc_and_prebuilt_game_bin() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_cmd_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("scripts").join("pack.py"), "# stub\n").unwrap();
        let openbf = dir.join(format!("openbf{}", crate::paths::EXE_SUFFIX));
        std::fs::write(&openbf, b"stub").unwrap();
        let paths = LauncherPaths::from_exe(
            &dir.join(format!("openbf-launcher{}", crate::paths::EXE_SUFFIX)),
            None,
        );

        let mut cfg = LauncherConfig::default();
        cfg.bf1.platform = Platform::Pc;
        cfg.bf1.path = Some(dir.join("BF1_PC"));
        cfg.bf2.platform = Platform::Ps2;
        cfg.bf2.path = Some(dir.join("BF2.iso"));
        cfg.dlc_dir = Some(dir.join("DLC_XBOX"));

        let cmd = build_pack_command(&paths, &cfg).expect("build pack command");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a == "--pc1"));
        assert!(args.iter().any(|a| a == "--iso2"));
        assert!(args.iter().any(|a| a == "--iso-dir"));
        assert!(args.iter().any(|a| a == "--game-bin"));
        assert!(args.iter().any(|a| a == "--out"));
        assert!(args.iter().any(|a| a.ends_with("DLC_XBOX")));

        // BF2-only omits --pc1/--iso1 and passes --bf2-only.
        cfg.bf1.path = None;
        let cmd_bf2 = build_pack_command(&paths, &cfg).expect("build bf2-only pack command");
        let args_bf2: Vec<String> = cmd_bf2
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(!args_bf2.iter().any(|a| a == "--pc1" || a == "--iso1"));
        assert!(args_bf2.iter().any(|a| a == "--bf2-only"));
        assert!(args_bf2.iter().any(|a| a == "--iso2"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
