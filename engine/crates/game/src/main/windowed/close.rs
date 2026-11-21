//! Closing the game promptly, in every state (the third playtest's Windows
//! close freeze).
//!
//! Every way out of the windowed game (the window's close, the console's
//! close or Ctrl+C, a Quit, a render or load failure) goes through
//! [`App::close_now`]. It arms a watchdog first, then hides the window, so
//! whatever the teardown does after that happens off screen and is bounded:
//!
//! 1. arm the [`WATCHDOG`]: if the close is not finished by then, the
//!    process ends hard, with the stuck step's name in `close.log`;
//! 2. let the cursor go, hide the window, and leave an Exclusive display
//!    mode (Borderless is only hidden: leaving it resizes the window and
//!    rebuilds the swapchain in the middle of the teardown);
//! 3. say goodbye to the LAN peers (the session's own 200 ms bound);
//! 4. abandon a loading worker instead of joining it (it is CPU-only and
//!    may still be decoding the whole sound library);
//! 5. record the round, save the profile, write the black box;
//! 6. drop the GPU objects, then the window, while the loop still runs;
//! 7. write one `close.log` line with the step timings, and end the loop.
//!
//! After the loop, the player entry points end the process at once
//! ([`end_process`]) instead of dropping the rest of the App, whose drops
//! join threads (the controller poller, the shell movie) with no bound.

use super::{App, Screen};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// `[ours]` How long a close may take before the process is ended hard.
/// The black box and the profile are written well inside it.
/// measure: the `close:` line's total in close.log from the playtests.
pub(super) const WATCHDOG: Duration = Duration::from_secs(5);

/// `[ours]` How long the console's close handler waits for the event loop's
/// own close before ending the process itself (Windows ends a console
/// process 5 s after a console close).
#[cfg_attr(not(windows), allow(dead_code))]
const CONSOLE_WAIT: Duration = Duration::from_secs(4);

/// close.log is started again past this size.
const LOG_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseReason {
    /// The window's close: its X, Alt+F4, the taskbar's Close window.
    Window,
    /// The console window's close, Ctrl+C or Ctrl+Break (Windows), or a
    /// log-off or shutdown.
    Console,
    /// A Quit chosen in the game (the shell's, a direct launch's pause menu,
    /// `OPENBF_QUIT_AT`).
    Quit,
    /// A render, map or battle start failure.
    Error,
}

impl CloseReason {
    fn name(self) -> &'static str {
        match self {
            CloseReason::Window => "window",
            CloseReason::Console => "console",
            CloseReason::Quit => "quit",
            CloseReason::Error => "error",
        }
    }

    /// A close from outside the game records the round in play, as the
    /// window's close always has; a Quit and a failure keep their old
    /// behaviour (no round recorded).
    fn records_round(self) -> bool {
        matches!(self, CloseReason::Window | CloseReason::Console)
    }
}

/// The close step running now, for the watchdog's line.
static STEP: Mutex<&'static str> = Mutex::new("not closing");

fn set_step(step: &'static str) {
    *STEP.lock().unwrap_or_else(|e| e.into_inner()) = step;
}

fn current_step() -> &'static str {
    *STEP.lock().unwrap_or_else(|e| e.into_inner())
}

/// A thread that calls `fire` with the step running at the deadline,
/// unless [`Watchdog::disarm`] came first. The real close never disarms it
/// ([`std::mem::forget`]): it stays armed until the process ends.
pub(super) struct Watchdog {
    /// Both used by the tests' disarm only; the real close keeps the
    /// sender alive by forgetting the watchdog.
    #[cfg_attr(not(test), allow(dead_code))]
    cancel: mpsc::Sender<()>,
    #[cfg_attr(not(test), allow(dead_code))]
    thread: std::thread::JoinHandle<()>,
}

impl Watchdog {
    pub(super) fn arm(
        timeout: Duration,
        fire: impl FnOnce(&'static str) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (cancel, wait) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("exit-watchdog".into())
            .spawn(move || {
                if let Err(mpsc::RecvTimeoutError::Timeout) = wait.recv_timeout(timeout) {
                    fire(current_step());
                }
            })?;
        Ok(Self { cancel, thread })
    }

    #[cfg(test)]
    pub(super) fn disarm(self) {
        let _ = self.cancel.send(());
        let _ = self.thread.join();
    }
}

/// End the process now: no atexit work, no DLL detach (Windows), no drops.
/// Only for a close that already wrote what it had to.
pub(super) fn hard_exit(code: i32) -> ! {
    #[cfg(windows)]
    {
        // SAFETY: plain kernel32 calls; the pseudo-handle needs no closing.
        unsafe {
            win::TerminateProcess(win::GetCurrentProcess(), code as u32);
        }
        std::process::abort()
    }
    #[cfg(unix)]
    {
        extern "C" {
            fn _exit(status: i32) -> !;
        }
        // SAFETY: async-signal-safe libc exit.
        unsafe { _exit(code) }
    }
    #[cfg(not(any(windows, unix)))]
    std::process::exit(code)
}

/// After a finished close: flush the console text and end the process,
/// skipping the App's remaining drops. Windows ends it with
/// TerminateProcess rather than ExitProcess: ExitProcess stops the watchdog
/// thread first and then runs every DLL's detach (the GPU driver's among
/// them), which is the part that can hang.
pub(super) fn end_process() -> ! {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    #[cfg(windows)]
    hard_exit(0);
    #[cfg(not(windows))]
    std::process::exit(0)
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    pub(super) type CtrlHandler = unsafe extern "system" fn(u32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        pub(super) fn GetCurrentProcess() -> *mut c_void;
        pub(super) fn TerminateProcess(process: *mut c_void, code: u32) -> i32;
        pub(super) fn SetConsoleCtrlHandler(handler: Option<CtrlHandler>, add: i32) -> i32;
    }

    pub(super) static REQUESTED: AtomicBool = AtomicBool::new(false);

    /// CTRL_C, CTRL_BREAK, CTRL_CLOSE, CTRL_LOGOFF, CTRL_SHUTDOWN.
    const EVENTS: [u32; 5] = [0, 1, 2, 5, 6];

    /// Runs on a thread of its own. Asks the event loop for a close and
    /// waits for it to end the process; past the wait it ends the process
    /// itself, so Windows' own end (ExitProcess on this thread while the
    /// event thread may be in the GPU driver) never runs.
    pub(super) unsafe extern "system" fn on_console(kind: u32) -> i32 {
        if !EVENTS.contains(&kind) {
            return 0;
        }
        REQUESTED.store(true, Ordering::SeqCst);
        std::thread::sleep(super::CONSOLE_WAIT);
        super::hard_exit(0)
    }

    pub(super) fn requested() -> bool {
        REQUESTED.load(Ordering::SeqCst)
    }
}

/// Windows: the console window's close and Ctrl+C become a close the event
/// loop runs ([`App::close_now`]). The console itself is kept as it is.
pub(super) fn install_console_handler() {
    #[cfg(windows)]
    // SAFETY: registers a handler with the signature kernel32 calls.
    unsafe {
        if win::SetConsoleCtrlHandler(Some(win::on_console), 1) == 0 {
            eprintln!("close: console handler not installed");
        }
    }
}

/// The console asked for a close (Windows; never elsewhere).
pub(super) fn console_close_requested() -> bool {
    #[cfg(windows)]
    return win::requested();
    #[cfg(not(windows))]
    false
}

/// Where close.log goes: beside the black box's snapshots
/// (`openbf_data/playtest` in a packed build), whether or not it is on.
fn log_dir() -> PathBuf {
    game::blackbox::default_dir()
}

/// One line appended to `dir/close.log`, started again past [`LOG_LIMIT`].
fn append_log(dir: &Path, line: &str) {
    let path = dir.join("close.log");
    let fresh = std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_LIMIT);
    let result = std::fs::create_dir_all(dir).and_then(|_| {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(!fresh)
            .write(true)
            .truncate(fresh)
            .open(&path)?;
        writeln!(f, "{line}")
    });
    if let Err(e) = result {
        eprintln!("close: {} not written: {e}", path.display());
    }
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The close's steps and their milliseconds; the step running is also kept
/// for the watchdog.
pub(super) struct Steps {
    started: Instant,
    at: Instant,
    open: Option<&'static str>,
    pub(super) done: Vec<(&'static str, f64)>,
}

impl Steps {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            at: now,
            open: None,
            done: Vec::new(),
        }
    }

    /// Close the running step and start `step`.
    fn enter(&mut self, step: &'static str) {
        self.end();
        self.open = Some(step);
        set_step(step);
    }

    fn end(&mut self) {
        let now = Instant::now();
        if let Some(step) = self.open.take() {
            self.done.push((
                step,
                now.saturating_duration_since(self.at).as_secs_f64() * 1000.0,
            ));
        }
        self.at = now;
    }

    pub(super) fn total_ms(&self) -> f64 {
        self.at
            .saturating_duration_since(self.started)
            .as_secs_f64()
            * 1000.0
    }

    fn line(&self, reason: CloseReason, screen: Screen, notes: &[String]) -> String {
        let steps: Vec<String> = self
            .done
            .iter()
            .map(|(s, ms)| format!("{s} {ms:.0}"))
            .collect();
        let mut line = format!(
            "close: unix={} reason={} screen={screen:?} total={:.0}ms steps(ms): {}",
            unix_seconds(),
            reason.name(),
            self.total_ms(),
            steps.join(", ")
        );
        for n in notes {
            line.push_str("; ");
            line.push_str(n);
        }
        line
    }
}

/// `OPENBF_EXIT_STALL_TEST=1` (dev mode): the close hangs in its GPU step,
/// so a run shows the watchdog ending the process.
fn stall_test() -> bool {
    game::dev::on() && std::env::var("OPENBF_EXIT_STALL_TEST").is_ok_and(|v| v == "1")
}

impl App {
    /// Ask for a close from inside a frame (a Quit, a render failure): the
    /// loop runs it once the frame's events are handled
    /// (`ApplicationHandler::about_to_wait`).
    pub(super) fn request_close(&mut self, reason: CloseReason) {
        self.close_pending.get_or_insert(reason);
    }

    /// The close (module docs). Runs once; later calls do nothing.
    pub(super) fn close_now(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        reason: CloseReason,
    ) {
        if self.closing {
            return;
        }
        let dir = log_dir();
        set_step("arm");
        let dog_dir = dir.clone();
        match Watchdog::arm(WATCHDOG, move |step| {
            let line = format!(
                "exit watchdog: unix={} teardown stuck at {step} after {} s, ending the process",
                unix_seconds(),
                WATCHDOG.as_secs()
            );
            eprintln!("{line}");
            append_log(&dog_dir, &line);
            hard_exit(0)
        }) {
            // Armed until the process ends.
            Ok(dog) => std::mem::forget(dog),
            Err(e) => eprintln!("close: exit watchdog not started: {e}"),
        }
        let screen = self.screen;
        let (steps, notes) = self.close_steps(reason);
        let line = steps.line(reason, screen, &notes);
        println!("{line}");
        append_log(&dir, &line);
        set_step("loop exit");
        event_loop.exit();
    }

    /// [`App::close_now`]'s steps after the watchdog, without the event
    /// loop (the tests run them).
    pub(super) fn close_steps(&mut self, reason: CloseReason) -> (Steps, Vec<String>) {
        self.closing = true;
        self.close_pending = None;
        let mut notes = Vec::new();
        let mut steps = Steps::new();

        steps.enter("mouse");
        self.release_mouse();
        // The idle inhibit borrows the display connection: before the window.
        self.idle = None;

        steps.enter("hide");
        if let Some(w) = &self.window {
            w.set_visible(false);
            if matches!(
                w.fullscreen(),
                Some(winit::window::Fullscreen::Exclusive(_))
            ) {
                // winit panics when the display mode's restore fails; the
                // system restores it when the process ends anyway.
                let left = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    w.set_fullscreen(None);
                    w.set_visible(false);
                }));
                notes.push(match left {
                    Ok(()) => "exclusive left".to_string(),
                    Err(_) => "exclusive restore failed (left to the system)".to_string(),
                });
            }
        }

        steps.enter("net");
        if let Some(n) = self.netplay.take() {
            n.shutdown();
            notes.push("lan goodbye sent".into());
        }

        steps.enter("loading");
        if let Some(mut l) = self.finish_loading() {
            l.worker.abandon();
            notes.push("loading worker detached".into());
        }

        steps.enter("record");
        if reason.records_round() {
            if let Some((walk, _)) = self.walk.as_mut() {
                if !self.round_recorded {
                    self.round_recorded = true;
                    super::record_round(&mut self.profile, walk, self.feed.battle_stats());
                }
            }
        }

        steps.enter("blackbox");
        self.blackbox_exit();

        steps.enter("gpu");
        if stall_test() {
            eprintln!("close: OPENBF_EXIT_STALL_TEST=1, stalling in the gpu step");
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }
        self.hud_view = None;
        self.gpu = None;
        self.walk = None;
        self.overlay = None;
        self.renderer = None;

        steps.enter("window");
        self.window = None;
        steps.end();
        (steps, notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn exit_watchdog_fires_after_the_timeout_with_the_step() {
        let (tx, rx) = mpsc::channel();
        let armed = Instant::now();
        let dog = Watchdog::arm(Duration::from_millis(50), move |step| {
            let _ = tx.send((step, armed.elapsed()));
        })
        .unwrap();
        let (_, after) = rx.recv_timeout(Duration::from_secs(5)).expect("fired");
        assert!(after >= Duration::from_millis(50), "{after:?}");
        dog.disarm();
    }

    #[test]
    fn exit_watchdog_does_not_fire_once_disarmed() {
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        let dog = Watchdog::arm(Duration::from_millis(100), move |_| {
            f.store(true, Ordering::SeqCst);
        })
        .unwrap();
        dog.disarm();
        std::thread::sleep(Duration::from_millis(200));
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn close_log_appends_and_starts_again_past_its_limit() {
        let dir = std::env::temp_dir().join(format!("openbf-close-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        append_log(&dir, "one");
        append_log(&dir, "two");
        let text = std::fs::read_to_string(dir.join("close.log")).unwrap();
        assert_eq!(text, "one\ntwo\n");
        std::fs::write(dir.join("close.log"), vec![b'x'; LOG_LIMIT as usize + 1]).unwrap();
        append_log(&dir, "three");
        let text = std::fs::read_to_string(dir.join("close.log")).unwrap();
        assert_eq!(text, "three\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn app() -> App {
        let setup = super::super::tests::setup(None);
        App::new(
            Default::default(),
            Default::default(),
            setup,
            None,
            None,
            None,
            Default::default(),
        )
    }

    /// A close during a loading screen whose worker is still busy returns
    /// at once (the worker is left to the process's end), and the close's
    /// GPU objects and window are gone afterwards.
    #[test]
    fn close_now_detaches_a_busy_loading_and_clears_the_window_side() {
        let mut a = app();
        let (release, wait) = mpsc::channel::<()>();
        let worker = super::super::loading::Worker::spawn(move || {
            let _ = wait.recv();
            anyhow::bail!("released")
        })
        .unwrap();
        a.loading = Some(super::super::Loading {
            map_name: "test".into(),
            started: Instant::now(),
            playback: Default::default(),
            display: Default::default(),
            worker,
            ready: None,
            shown: false,
            cues: None,
            held: None,
        });
        a.screen = Screen::Loading;
        let started = Instant::now();
        let (steps, notes) = a.close_steps(CloseReason::Window);
        let took = started.elapsed();
        assert!(took < Duration::from_millis(500), "{took:?}");
        assert!(a.closing);
        assert!(a.loading.is_none());
        assert!(a.renderer.is_none() && a.window.is_none() && a.overlay.is_none());
        assert!(a.gpu.is_none() && a.walk.is_none());
        assert!(
            notes.iter().any(|n| n == "loading worker detached"),
            "{notes:?}"
        );
        let names: Vec<_> = steps.done.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            names,
            ["mouse", "hide", "net", "loading", "record", "blackbox", "gpu", "window"]
        );
        let _ = release.send(());
    }

    #[test]
    fn close_pending_keeps_the_first_reason() {
        let mut a = app();
        a.request_close(CloseReason::Error);
        a.request_close(CloseReason::Quit);
        assert_eq!(a.close_pending, Some(CloseReason::Error));
    }
}
