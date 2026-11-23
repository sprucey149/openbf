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
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Number of steps [`run_setup`] reports through [`ProgressSink::on_step`].
pub const STEP_COUNT: u8 = 6;

/// An argument list for a child process, each item anything that is
/// `AsRef<OsStr>` (`&str`, `String`, `Path`, `PathBuf`), kept lossless.
macro_rules! args {
    ($($arg:expr),* $(,)?) => {
        [$(::std::ffi::OsString::from(::std::convert::AsRef::<::std::ffi::OsStr>::as_ref(&$arg))),*]
    };
}
pub(crate) use args;

/// Environment overrides for a child: `Some` sets the variable, `None` removes it.
pub type EnvOverrides<'a> = [(&'a str, Option<&'a str>)];

/// Callback hooks used by `BuildHandle` (GUI) or headless tests to receive
/// step transitions and streamed output lines.
pub trait ProgressSink: Send + Sync {
    fn on_step(&self, index: u8, total: u8, label: &str);
    fn on_line(&self, line: &str);
}

/// Report step `index` of [`STEP_COUNT`] and echo it as a console header.
fn announce_step(sink: &dyn ProgressSink, index: u8, label: &str) {
    sink.on_step(index, STEP_COUNT, label);
    sink.on_line(&format!("== [{index}/{STEP_COUNT}] {label} =="));
}

/// Keep a console child from flashing a window over the GUI launcher on Windows.
pub(crate) fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// True when `program args` runs and exits successfully, with all output discarded.
fn tool_responds(program: &str, args: &[&str]) -> bool {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut cmd).status().is_ok_and(|s| s.success())
}

/// A Python 3.10+ interpreter on `PATH` and the arguments that select it.
fn find_python() -> Result<(PathBuf, &'static [&'static str])> {
    #[cfg(target_os = "windows")]
    let candidates: &[(&str, &[&str])] = &[("py", &["-3"]), ("python", &[]), ("python3", &[])];
    #[cfg(not(target_os = "windows"))]
    let candidates: &[(&str, &[&str])] = &[("python3", &[]), ("python", &[])];

    const VERSION_CHECK: &str = "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)";
    candidates
        .iter()
        .find(|(bin, extra)| tool_responds(bin, &[*extra, &["-c", VERSION_CHECK]].concat()))
        .map(|(bin, extra)| (PathBuf::from(bin), *extra))
        .context("Python 3.10+ not found on PATH (required for mission generation during setup)")
}

fn check_ffmpeg() -> Result<()> {
    if tool_responds("ffmpeg", &["-version"]) {
        Ok(())
    } else {
        bail!("ffmpeg not found on PATH (required for audio/video conversion during setup)")
    }
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

    pub fn step(&self, index: u8, label: &str) {
        announce_step(self.sink.as_ref(), index, label);
    }

    pub fn log(&self, line: &str) {
        self.sink.on_line(line);
    }

    /// Run `bake <args>`, pointing its audio restoration at this launcher.
    pub fn run_bake(&self, args: &[impl AsRef<OsStr>], env: &EnvOverrides<'_>) -> Result<()> {
        let mut cmd = Command::new(&self.bake_bin);
        cmd.args(args)
            .env("OPENBF_AUDIO_RESTORE_BIN", &self.launcher_exe)
            .env_remove("OPENBF_AUDIO_RESTORE_SCRIPT");
        apply_env(&mut cmd, env);
        let desc = format!(
            "bake {}",
            args.first()
                .map(|a| a.as_ref().to_string_lossy())
                .unwrap_or_default()
        );
        self.run_command(cmd, &desc)
    }

    /// Run an isolated (`-I`) Python `script` with `args`.
    pub fn run_python(
        &self,
        script: &Path,
        args: &[impl AsRef<OsStr>],
        env: &EnvOverrides<'_>,
    ) -> Result<()> {
        let mut cmd = Command::new(&self.python_bin);
        cmd.args(self.python_extra)
            .arg("-I")
            .arg(script)
            .args(args)
            .env("PYTHONUNBUFFERED", "1");
        apply_env(&mut cmd, env);
        let name = script.file_name().unwrap_or(script.as_os_str());
        self.run_command(cmd, &format!("python {}", name.to_string_lossy()))
    }

    /// Run `cmd` to completion, streaming its output to the sink and killing
    /// it when setup is cancelled.
    pub fn run_command(&self, mut cmd: Command, label: &str) -> Result<()> {
        self.check_cancelled()?;
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console(&mut cmd);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to start {label}"))?;
        let streams = [
            child.stdout.take().map(|out| self.stream(out)),
            child.stderr.take().map(|err| self.stream(err)),
        ];
        *self.active_child.lock().unwrap() = Some(child);

        let status = loop {
            let mut slot = self.active_child.lock().unwrap();
            let Some(c) = slot.as_mut() else {
                break None;
            };
            if self.cancel.load(Ordering::SeqCst) {
                let _ = c.kill();
            }
            match c.try_wait() {
                Ok(None) => {
                    drop(slot);
                    std::thread::sleep(Duration::from_millis(60));
                }
                done => {
                    *slot = None;
                    break done.transpose();
                }
            }
        };

        for thread in streams.into_iter().flatten() {
            let _ = thread.join();
        }

        self.check_cancelled()?;
        match status {
            Some(Ok(st)) if st.success() => Ok(()),
            Some(Ok(st)) => bail!("{label} failed ({st})"),
            Some(Err(e)) => Err(e).with_context(|| format!("waiting for {label}")),
            None => bail!("{label} cancelled"),
        }
    }

    fn stream(&self, reader: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<()> {
        let sink = Arc::clone(&self.sink);
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                let line = line.trim();
                if !line.is_empty() {
                    sink.on_line(line);
                }
            }
        })
    }
}

fn apply_env(cmd: &mut Command, env: &EnvOverrides<'_>) {
    for (key, value) in env {
        match value {
            Some(value) => cmd.env(key, value),
            None => cmd.env_remove(key),
        };
    }
}

/// RAII guard that always removes the staging temp directory when setup ends.
struct TempWorkspace {
    dir: PathBuf,
}

impl TempWorkspace {
    fn create(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join(format!(".setup_tmp_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
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
    announce_step(sink.as_ref(), 1, "verifying tools and game sources");

    let plan = BuildPlan::from_config(config)?;
    let bake_bin = paths
        .bake_bin()
        .ok_or_else(|| anyhow!("bake tool not found (expected bake{EXE_SUFFIX} in openbf_data/tools/ or beside the launcher)"))?;
    let base_game_bin = paths.base_game_bin().ok_or_else(|| {
        anyhow!("base game binary not found (expected openbf{EXE_SUFFIX} or game{EXE_SUFFIX})")
    })?;
    let launcher_exe = std::env::current_exe().context("locating launcher executable")?;

    check_ffmpeg()?;
    let (python_bin, python_extra) = find_python()?;

    let ctx = ExecCtx {
        bake_bin,
        launcher_exe,
        python_bin,
        python_extra,
        cancel,
        active_child,
        sink,
    };

    match &plan.bf1 {
        Some(bf1) => ctx.log(&format!(
            "Sources: Battlefront I ({}) + Battlefront II ({})",
            bf1.platform().label(),
            plan.bf2.platform().label()
        )),
        None => ctx.log(&format!(
            "Sources: Battlefront II only ({}; prone disabled, BF1 maps hidden)",
            plan.bf2.platform().label()
        )),
    }
    if let Some(dlc) = &plan.dlc_dir {
        ctx.log(&format!("DLC Heroes folder: {}", dlc.display()));
    }

    std::fs::create_dir_all(&paths.data_dir)
        .with_context(|| format!("creating {}", paths.data_dir.display()))?;
    // Also clean up any stale `.setup_tmp_*` left by a hard kill from a previous run.
    cleanup_stale_temps(&paths.data_dir);
    let temp = TempWorkspace::create(&paths.data_dir)?;

    // Step 2: extract discs (PS2/Xbox ISOs -> temp; PC uses _LVL_PC in place).
    ctx.step(2, "extracting disc data");
    let bf1_tree = match &plan.bf1 {
        Some(src) => Some(prepare_source_tree(&ctx, GameId::Bf1, src, &temp.dir)?),
        None => None,
    };
    let bf2_tree = prepare_source_tree(&ctx, GameId::Bf2, &plan.bf2, &temp.dir)?;

    // Step 3: extract Lua bytecode via embedded scripts.
    ctx.step(3, "extracting Lua scripts");
    let embedded = MaterializedMissions::materialize(&temp.dir)?;
    let trees = BakeTrees {
        bf1: bf1_tree,
        bf2: bf2_tree,
        bf2_iso: plan.bf2.disc_image().map(Path::to_path_buf),
        lua_root: temp.dir.join("lua"),
        out_dir: temp.dir.join("assets_baked"),
        temp_root: temp.dir.clone(),
    };
    embedded.extract_lua(&ctx, &trees)?;

    // Step 4: generate missions/*.ron via embedded gen_missions.py.
    ctx.step(4, "generating missions");
    let missions_dir = temp.dir.join("missions");
    embedded.generate_missions(&ctx, &plan, &trees, &missions_dir)?;

    // Step 5: bake all asset groups natively (zero Git Bash, Rust audio restore).
    ctx.step(5, "baking game assets");
    rebake::run_all_bake_groups(&ctx, &plan, &trees, &embedded)?;

    // Step 6: pack into split openbf.exe (~20 MB) + openbf.pak.
    ctx.step(6, "packing game archive");
    pack_split_game(
        &ctx,
        &base_game_bin,
        &paths.game_out,
        &trees.out_dir,
        &missions_dir,
        &temp.dir,
    )?;

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

/// The LVL directory the bake reads for one game: a PC install's `_LVL_PC`
/// in place, or a console disc extracted into the setup temp directory.
fn prepare_source_tree(
    ctx: &ExecCtx,
    game: GameId,
    source: &GameSource,
    temp_root: &Path,
) -> Result<PathBuf> {
    let (iso, subcommand, folder, lvl_dir) = match source {
        GameSource::PcLvlDir(dir) => {
            ctx.log(&format!(
                "{}: using PC data at {}",
                game.title(),
                dir.display()
            ));
            return Ok(dir.clone());
        }
        GameSource::Ps2Iso(iso) => (
            iso,
            "extract-iso",
            game.as_str().to_owned(),
            "DATA/_LVL_PS2",
        ),
        GameSource::XboxIso(iso) => (
            iso,
            "extract-xiso",
            format!("xbox_{}", game.as_str()),
            "Data/_LVL_XBOX",
        ),
    };
    let dest = temp_root.join("extracted").join(folder);
    std::fs::create_dir_all(&dest)?;
    ctx.run_bake(&args![subcommand, iso, dest], &[])?;
    Ok(ci_path(&dest, lvl_dir))
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

    let pak_out = out_exe.with_file_name(pak::PAK_NAME);
    let new_exe = out_exe.with_file_name(format!("openbf.new{EXE_SUFFIX}"));
    let new_pak = out_exe.with_file_name(format!("{}.new", pak::PAK_NAME));
    let remove_staged = || {
        let _ = std::fs::remove_file(&new_exe);
        let _ = std::fs::remove_file(&new_pak);
    };
    remove_staged();

    let mut cmd = Command::new(&packer_copy);
    cmd.args(args![
        "--write-pack",
        new_exe,
        "--pak",
        new_pak,
        "--split-at",
        "0",
        mount_arg("assets_baked", baked_dir),
        mount_arg("missions", missions_dir),
    ]);
    if let Err(e) = ctx.run_command(cmd, "game --write-pack") {
        remove_staged();
        return Err(e);
    }

    // Swap pak first, then exe, so a split build never pairs a new exe with an old pak.
    if new_pak.exists() {
        replace_file(&new_pak, &pak_out)?;
    }
    replace_file(&new_exe, out_exe)
}

/// `<name>=<dir>` for `--write-pack`.
fn mount_arg(name: &str, dir: &Path) -> OsString {
    let mut arg = OsString::from(name);
    arg.push("=");
    arg.push(dir.as_os_str());
    arg
}

fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
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
                .and_then(OsStr::to_str)
                .is_some_and(|n| n.starts_with(".setup_tmp_"))
        {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}
