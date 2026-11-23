//! Native Stage 2 setup pipeline.
//!
//! Orchestrates the entire 6-step game build directly in Rust:
//! 1. Verify inputs & tools (`bake`, base `game`/`openbf` binary, `ffmpeg`, Python for mission gen)
//! 2. Extract PS2 / Xbox ISOs via `bake extract-iso` / `bake extract-xiso`
//! 3. Extract compiled Lua via embedded `extract_lua.py`
//! 4. Generate `missions/*.ron` via embedded `gen_missions.py`
//! 5. Bake every active asset group directly via `bake` (zero Git Bash) using
//!    `openbf-launcher --audio-restore-worker` for native Rust audio restoration
//! 6. Pack into always-split `openbf.exe` (~20 MB) + `openbf.pak` via `--write-pack --split-at 0`
//!
//! All intermediate extraction, Lua, mission, and bake staging lives in
//! `openbf_data/.setup_tmp` and is removed automatically when setup finishes,
//! fails, or is cancelled so the game folder stays minimal and clean.

pub mod audio_restore;
pub mod missions;
pub mod rebake;
pub mod scan;

use crate::config::LauncherConfig;
use crate::paths::{LauncherPaths, EXE_SUFFIX};
use anyhow::{anyhow, bail, Context, Result};
use missions::MaterializedMissions;
use rebake::BakeTrees;
use scan::{ci_path, BuildPlan, GameId, GameSource};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Callback hooks used by `BuildHandle` (GUI) or headless tests to receive
/// step transitions and streamed output lines.
pub trait ProgressSink: Send + Sync {
    fn on_step(&self, index: u8, total: u8, label: &str);
    fn on_line(&self, line: &str);
}

/// Shared execution context passed through every stage of the pipeline.
pub struct ExecCtx {
    pub bake_bin: PathBuf,
    pub launcher_exe: PathBuf,
    pub python_bin: PathBuf,
    pub python_extra: &'static [&'static str],
    pub cancel: Arc<AtomicBool>,
    pub active_child: Arc<Mutex<Option<Child>>>,
    pub sink: Arc<dyn ProgressSink>,
}

impl ExecCtx {
    pub fn check_cancelled(&self) -> Result<()> {
        if self.cancel.load(Ordering::SeqCst) {
            bail!("setup cancelled");
        }
        Ok(())
    }

    pub fn step(&self, index: u8, total: u8, label: &str) {
        self.sink.on_step(index, total, label);
        self.sink
            .on_line(&format!("== [{index}/{total}] {label} =="));
    }

    pub fn log(&self, line: &str) {
        self.sink.on_line(line);
    }

    pub fn run_bake(&self, args: &[String], extra_env: &[(&str, Option<&str>)]) -> Result<()> {
        self.check_cancelled()?;
        let mut cmd = Command::new(&self.bake_bin);
        cmd.args(args);
        cmd.env("OPENBF_AUDIO_RESTORE_BIN", &self.launcher_exe);
        cmd.env_remove("OPENBF_AUDIO_RESTORE_SCRIPT");
        for (k, v) in extra_env {
            match v {
                Some(val) => {
                    cmd.env(k, val);
                }
                None => {
                    cmd.env_remove(k);
                }
            }
        }
        let desc = format!("bake {}", args.first().map(String::as_str).unwrap_or(""));
        self.run_command(cmd, &desc)
    }

    pub fn run_python(
        &self,
        args: &[String],
        extra_env: &[(&str, Option<&str>)],
    ) -> Result<()> {
        self.check_cancelled()?;
        let mut cmd = Command::new(&self.python_bin);
        cmd.args(self.python_extra);
        cmd.args(args.iter());
        cmd.env("PYTHONUNBUFFERED", "1");
        for (k, v) in extra_env {
            match v {
                Some(val) => {
                    cmd.env(k, val);
                }
                None => {
                    cmd.env_remove(k);
                }
            }
        }
        let desc = format!(
            "python {}",
            args.get(1).map(String::as_str).unwrap_or("script")
        );
        self.run_command(cmd, &desc)
    }

    pub fn run_command(&self, mut cmd: Command, label: &str) -> Result<()> {
        self.check_cancelled()?;
        cmd.stdin(Stdio::null())
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
            .with_context(|| format!("failed to start {label}"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        {
            let mut slot = self.active_child.lock().unwrap();
            *slot = Some(child);
        }

        let out_thread = stdout.map(|out| {
            let sink = Arc::clone(&self.sink);
            std::thread::spawn(move || stream_lines(out, sink))
        });
        let err_thread = stderr.map(|err| {
            let sink = Arc::clone(&self.sink);
            std::thread::spawn(move || stream_lines(err, sink))
        });

        let status = loop {
            if self.cancel.load(Ordering::SeqCst) {
                if let Ok(mut slot) = self.active_child.lock() {
                    if let Some(c) = slot.as_mut() {
                        let _ = c.kill();
                    }
                }
            }
            let mut slot = self.active_child.lock().unwrap();
            let Some(c) = slot.as_mut() else {
                break None;
            };
            match c.try_wait() {
                Ok(Some(st)) => {
                    *slot = None;
                    break Some(Ok(st));
                }
                Ok(None) => {
                    drop(slot);
                    std::thread::sleep(Duration::from_millis(60));
                }
                Err(e) => {
                    *slot = None;
                    break Some(Err(e));
                }
            }
        };

        if let Some(t) = out_thread {
            let _ = t.join();
        }
        if let Some(t) = err_thread {
            let _ = t.join();
        }

        self.check_cancelled()?;
        match status {
            Some(Ok(st)) if st.success() => Ok(()),
            Some(Ok(st)) => bail!("{label} failed ({st})"),
            Some(Err(e)) => Err(e).with_context(|| format!("waiting for {label}")),
            None => bail!("{label} cancelled"),
        }
    }
}

fn stream_lines(reader: impl std::io::Read, sink: Arc<dyn ProgressSink>) {
    let mut br = BufReader::new(reader);
    let mut buf = String::new();
    loop {
        buf.clear();
        match br.read_line(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = buf.trim();
                if !trimmed.is_empty() {
                    sink.on_line(trimmed);
                }
            }
        }
    }
}

/// RAII guard that always removes the staging temp directory when setup ends.
struct TempWorkspace {
    dir: PathBuf,
}

impl TempWorkspace {
    fn create(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("creating {}", data_dir.display()))?;
        let dir = data_dir.join(format!(".setup_tmp_{}", std::process::id()));
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Self { dir })
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Execute the full 6-step native setup pipeline and clean up all temporary files.
pub fn run_setup(
    paths: &LauncherPaths,
    config: &LauncherConfig,
    cancel: Arc<AtomicBool>,
    active_child: Arc<Mutex<Option<Child>>>,
    sink: Arc<dyn ProgressSink>,
) -> Result<()> {
    let started = Instant::now();
    sink.on_step(1, 6, "verifying tools and game sources");
    sink.on_line("== [1/6] verifying tools and game sources ==");

    let plan = BuildPlan::from_config(config)?;
    let bake_bin = paths
        .bake_bin()
        .ok_or_else(|| anyhow!("bake tool not found (expected bake{EXE_SUFFIX} in openbf_data/tools/ or beside the launcher)"))?;
    let base_game_bin = paths
        .base_game_bin()
        .ok_or_else(|| anyhow!("base game binary not found (expected openbf{EXE_SUFFIX} or game{EXE_SUFFIX})"))?;
    let launcher_exe = std::env::current_exe().context("locating launcher executable")?;

    check_ffmpeg()?;
    let (python_bin, python_extra) = crate::runner::find_python()?;

    let ctx = ExecCtx {
        bake_bin,
        launcher_exe,
        python_bin,
        python_extra,
        cancel,
        active_child,
        sink,
    };

    if let Some(bf1) = &plan.bf1 {
        ctx.log(&format!(
            "Sources: Battlefront I ({}) + Battlefront II ({})",
            bf1.display_label(),
            plan.bf2.display_label()
        ));
    } else {
        ctx.log(&format!(
            "Sources: Battlefront II only ({}; prone disabled, BF1 maps hidden)",
            plan.bf2.display_label()
        ));
    }
    if let Some(dlc) = &plan.dlc_dir {
        ctx.log(&format!("DLC Heroes folder: {}", dlc.display()));
    }

    // Also clean up any stale `.setup_tmp_*` left by a hard kill from a previous run.
    cleanup_stale_temps(&paths.data_dir);
    let temp = TempWorkspace::create(&paths.data_dir)?;

    // Step 2: extract discs (PS2/Xbox ISOs -> temp; PC uses _LVL_PC in place).
    ctx.step(2, 6, "extracting disc data");
    let bf1_tree = match &plan.bf1 {
        Some(src) => Some(prepare_source_tree(&ctx, GameId::Bf1, src, &temp.dir)?),
        None => None,
    };
    let bf2_tree = prepare_source_tree(&ctx, GameId::Bf2, &plan.bf2, &temp.dir)?;
    let bf2_iso = match &plan.bf2 {
        GameSource::Ps2Iso(p) | GameSource::XboxIso(p) => Some(p.clone()),
        GameSource::PcLvlDir(_) => None,
    };

    // Step 3: extract Lua bytecode via embedded scripts.
    ctx.step(3, 6, "extracting Lua scripts");
    let embedded = MaterializedMissions::materialize(&temp.dir)?;
    let lua_root = temp.dir.join("lua");
    embedded.extract_lua(&ctx, bf1_tree.as_deref(), &bf2_tree, &lua_root)?;

    // Step 4: generate missions/*.ron via embedded gen_missions.py.
    ctx.step(4, 6, "generating missions");
    let missions_dir = temp.dir.join("missions");
    embedded.generate_missions(
        &ctx,
        &plan,
        bf1_tree.as_deref(),
        &bf2_tree,
        &lua_root,
        &missions_dir,
    )?;

    // Step 5: bake all asset groups natively (zero Git Bash, Rust audio restore).
    ctx.step(5, 6, "baking game assets");
    let baked_dir = temp.dir.join("assets_baked");
    let trees = BakeTrees {
        bf1: bf1_tree,
        bf2: bf2_tree,
        bf2_iso,
        lua_root,
        out_dir: baked_dir.clone(),
        temp_root: temp.dir.clone(),
    };
    rebake::run_all_bake_groups(&ctx, &plan, &trees, &embedded)?;

    // Step 6: pack into split openbf.exe (~20 MB) + openbf.pak.
    ctx.step(6, 6, "packing game archive");
    pack_split_game(&ctx, &base_game_bin, &paths.game_out, &baked_dir, &missions_dir, &temp.dir)?;

    // Drop temp workspace explicitly so cleanup errors (if any) happen before "Setup complete".
    drop(temp);

    let elapsed = started.elapsed().as_secs();
    ctx.log(&format!(
        "Setup complete in {:02}:{:02} -> {}",
        elapsed / 60,
        elapsed % 60,
        paths.game_out.display()
    ));
    Ok(())
}

fn prepare_source_tree(
    ctx: &ExecCtx,
    game: GameId,
    source: &GameSource,
    temp_root: &Path,
) -> Result<PathBuf> {
    match source {
        GameSource::PcLvlDir(dir) => {
            ctx.log(&format!("{}: using PC data at {}", game.title(), dir.display()));
            Ok(dir.clone())
        }
        GameSource::Ps2Iso(iso) => {
            let dest = temp_root.join("extracted").join(game.as_str());
            std::fs::create_dir_all(&dest)?;
            ctx.run_bake(
                &[
                    "extract-iso".into(),
                    iso.to_string_lossy().into_owned(),
                    dest.to_string_lossy().into_owned(),
                ],
                &[],
            )?;
            Ok(ci_path(&dest, "DATA/_LVL_PS2"))
        }
        GameSource::XboxIso(iso) => {
            let dest = temp_root.join("extracted").join(format!("xbox_{}", game.as_str()));
            std::fs::create_dir_all(&dest)?;
            ctx.run_bake(
                &[
                    "extract-xiso".into(),
                    iso.to_string_lossy().into_owned(),
                    dest.to_string_lossy().into_owned(),
                ],
                &[],
            )?;
            Ok(ci_path(&dest, "Data/_LVL_XBOX"))
        }
    }
}

fn pack_split_game(
    ctx: &ExecCtx,
    base_game_bin: &Path,
    out_exe: &Path,
    baked_dir: &Path,
    missions_dir: &Path,
    temp_root: &Path,
) -> Result<()> {
    ctx.check_cancelled()?;
    if let Some(parent) = out_exe.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let packer_copy = temp_root.join(format!("pack-tmp{EXE_SUFFIX}"));
    std::fs::copy(base_game_bin, &packer_copy).with_context(|| {
        format!(
            "copying {} to {}",
            base_game_bin.display(),
            packer_copy.display()
        )
    })?;

    let pak_out = out_exe.with_file_name("openbf.pak");
    let new_exe = out_exe.with_file_name(format!("openbf.new{EXE_SUFFIX}"));
    let new_pak = out_exe.with_file_name("openbf.pak.new");
    let _ = std::fs::remove_file(&new_exe);
    let _ = std::fs::remove_file(&new_pak);

    let mut cmd = Command::new(&packer_copy);
    cmd.args([
        "--write-pack",
        &new_exe.to_string_lossy(),
        "--pak",
        &new_pak.to_string_lossy(),
        "--split-at",
        "0",
        &format!("assets_baked={}", baked_dir.display()),
        &format!("missions={}", missions_dir.display()),
    ]);
    let res = ctx.run_command(cmd, "game --write-pack");
    if let Err(e) = res {
        let _ = std::fs::remove_file(&new_exe);
        let _ = std::fs::remove_file(&new_pak);
        return Err(e);
    }

    // Swap pak first, then exe, so a split build never pairs a new exe with an old pak.
    if new_pak.exists() {
        replace_file(&new_pak, &pak_out)?;
    }
    replace_file(&new_exe, out_exe)?;
    Ok(())
}

fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        if dst.exists() {
            let backup = dst.with_extension("old");
            let _ = std::fs::remove_file(&backup);
            std::fs::rename(dst, &backup)
                .with_context(|| format!("moving previous {} aside", dst.display()))?;
            if let Err(e) = std::fs::rename(src, dst) {
                let _ = std::fs::rename(&backup, dst);
                return Err(e).with_context(|| format!("installing {}", dst.display()));
            }
            let _ = std::fs::remove_file(&backup);
            return Ok(());
        }
    }
    std::fs::rename(src, dst)
        .with_context(|| format!("installing {} -> {}", src.display(), dst.display()))
}

fn cleanup_stale_temps(data_dir: &Path) {
    let Ok(rd) = std::fs::read_dir(data_dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir()
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(".setup_tmp_"))
        {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

fn check_ffmpeg() -> Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    if cmd.status().is_ok_and(|s| s.success()) {
        Ok(())
    } else {
        bail!("ffmpeg not found on PATH (required for audio/video conversion during setup)")
    }
}
