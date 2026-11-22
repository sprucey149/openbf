//! `[ours]` The game's console text, also kept in a file a player can send.
//!
//! A packed build started from a file manager (a macOS app, a desktop
//! shortcut) has no terminal, so everything the game prints (the loading
//! timings and heartbeat, a failed load's reason, a panic) used to go
//! nowhere. The player entry points ([`start`]: the shell and a LAN
//! launch) copy stdout and stderr into `openbf_data/logs/openbf-<UTC date
//! and time>.log` and still print them to the terminal. The newest [`KEEP`]
//! logs are kept; older ones are deleted when a session starts.
//!
//! How: each stream's descriptor is pointed at a pipe, and a thread per
//! stream copies what arrives to the terminal and to the file, so every
//! `println!`, `eprintln!` and C library message is caught without touching
//! the callers. A panic is also written straight to the file by the panic
//! hook, with a backtrace, because the process may end before the copying
//! thread has caught up. [`finish`] puts the streams back and waits a moment
//! for the copy to drain; the player exits call it.
//!
//! Unix (Linux, macOS) only for now. Windows builds keep their console, and
//! `start` returns `None` there. follow-up (measure: a Windows report that
//! needs it): the same tee through `SetStdHandle`.
//!
//! `OPENBF_LOG=0` turns it off. Nothing here reaches the simulation.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::Duration;

/// `[ours]` Session logs kept (the new one included).
/// measure: how many sessions back a player's report usually needs.
pub const KEEP: usize = 5;
/// `[ours]` How long [`finish`] waits for the copy to drain.
const DRAIN: Duration = Duration::from_millis(500);
const PREFIX: &str = "openbf-";
const SUFFIX: &str = ".log";

type Shared = Arc<Mutex<std::fs::File>>;

/// The open log, for the panic hook and [`finish`].
static FILE: Mutex<Option<Shared>> = Mutex::new(None);
/// The open log's path, for the on-screen notices.
static PATH: OnceLock<PathBuf> = OnceLock::new();
static HOOK: Once = Once::new();
#[cfg(unix)]
static TEE: Mutex<Option<imp::Tee>> = Mutex::new(None);

/// Where the session logs go: `openbf_data/logs` beside the game.
pub fn default_dir() -> PathBuf {
    crate::profile::data_dir().join("logs")
}

/// Start the session log in [`default_dir`] unless `OPENBF_LOG=0`. Returns
/// its path; `None` (with a line on stderr saying why) when it could not be
/// started, and the game runs on with the terminal only.
pub fn start() -> Option<PathBuf> {
    if std::env::var_os("OPENBF_LOG").is_some_and(|v| v == "0") {
        return None;
    }
    match start_in(&default_dir()) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("log: not written: {e}");
            None
        }
    }
}

/// The session log's path once [`start`] opened one.
pub fn path() -> Option<&'static Path> {
    PATH.get().map(PathBuf::as_path)
}

/// [`start`] in `dir`: prune the old logs, open the new one, tee both
/// streams into it and install the panic hook.
pub fn start_in(dir: &Path) -> std::io::Result<PathBuf> {
    #[cfg(not(unix))]
    {
        let _ = dir;
        Err(std::io::Error::other("this platform keeps its console"))
    }
    #[cfg(unix)]
    {
        let mut tee = TEE.lock().unwrap_or_else(|e| e.into_inner());
        if tee.is_some() {
            return Err(std::io::Error::other("already started"));
        }
        std::fs::create_dir_all(dir)?;
        prune(dir, KEEP.saturating_sub(1))?;
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = dir.join(file_name(secs));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(
            file,
            "openbf session log: {} (build {}{}), {} {}",
            utc_stamp(secs),
            env!("OPENBF_BUILD_COMMIT"),
            if env!("OPENBF_BUILD_DIRTY") == "1" {
                " dirty"
            } else {
                ""
            },
            std::env::consts::OS,
            std::env::consts::ARCH,
        )?;
        let file: Shared = Arc::new(Mutex::new(file));
        *tee = Some(imp::Tee::start(&file)?);
        *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
        let _ = PATH.set(path.clone());
        install_panic_hook();
        println!("log: {}", path.display());
        Ok(path)
    }
}

/// Put the streams back and let the copy drain (bounded by [`DRAIN`]). The
/// game's exits call it; later output goes to the terminal only.
pub fn finish() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    #[cfg(unix)]
    {
        let tee = TEE.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(tee) = tee {
            tee.finish(DRAIN);
        }
    }
    let file = FILE.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(f) = file {
        let _ = f.lock().unwrap_or_else(|e| e.into_inner()).flush();
    }
}

/// The panic, straight into the file (the copying thread may not get to it
/// before the process ends). The earlier hook still prints it as before.
fn install_panic_hook() {
    HOOK.call_once(|| {
        let before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            before(info);
            let Some(file) = FILE.try_lock().ok().and_then(|f| f.clone()) else {
                return;
            };
            let thread = std::thread::current();
            let text = format!(
                "\npanic in thread '{}': {info}\n{}\n",
                thread.name().unwrap_or("unnamed"),
                std::backtrace::Backtrace::force_capture()
            );
            // A held lock is the copying thread's, mid-write: wait a little.
            for _ in 0..50 {
                match file.try_lock() {
                    Ok(mut f) => {
                        let _ = f.write_all(text.as_bytes());
                        let _ = f.flush();
                        return;
                    }
                    Err(std::sync::TryLockError::Poisoned(e)) => {
                        let _ = e.into_inner().write_all(text.as_bytes());
                        return;
                    }
                    Err(std::sync::TryLockError::WouldBlock) => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
            }
        }));
    });
}

/// A session's file name: `openbf-YYYY-MM-DD_HHMMSS.log` (UTC), so the
/// names sort by time.
pub fn file_name(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(unix_secs);
    format!("{PREFIX}{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}{s:02}{SUFFIX}")
}

/// `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc_stamp(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(unix_secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// Delete the oldest session logs in `dir` until at most `keep` are left.
/// Only files named as [`file_name`] names them are touched.
pub fn prune(dir: &Path, keep: usize) -> std::io::Result<()> {
    let mut logs: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(SUFFIX))
        })
        .collect();
    logs.sort();
    let extra = logs.len().saturating_sub(keep);
    for old in &logs[..extra] {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// Unix seconds to (year, month, day, hour, minute, second), UTC; the
/// days-to-civil conversion of the proleptic Gregorian calendar.
fn civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        (rem / 60 % 60) as u32,
        (rem % 60) as u32,
    )
}

#[cfg(unix)]
mod imp {
    use super::Shared;
    use std::io::{Read, Write};
    use std::os::fd::{FromRawFd, RawFd};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    extern "C" {
        fn pipe(fds: *mut i32) -> i32;
        fn dup(fd: i32) -> i32;
        fn dup2(from: i32, to: i32) -> i32;
        fn close(fd: i32) -> i32;
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    /// The same values on Linux and macOS.
    const F_SETFD: i32 = 2;
    const FD_CLOEXEC: i32 = 1;

    fn check(r: i32) -> std::io::Result<i32> {
        if r < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(r)
        }
    }

    /// One stream's redirect: the descriptor's original, kept to put back.
    struct Stream {
        fd: RawFd,
        saved: RawFd,
        done: mpsc::Receiver<()>,
    }

    pub(super) struct Tee(Vec<Stream>);

    impl Tee {
        pub(super) fn start(file: &Shared) -> std::io::Result<Self> {
            let mut streams = Vec::new();
            for fd in [1, 2] {
                match Stream::start(fd, file.clone()) {
                    Ok(s) => streams.push(s),
                    Err(e) => {
                        Self(streams).finish(Duration::ZERO);
                        return Err(e);
                    }
                }
            }
            Ok(Self(streams))
        }

        /// Put each descriptor back: the pipe's last writer goes, its thread
        /// reads to the end and says so.
        pub(super) fn finish(self, wait: Duration) {
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
            for s in &self.0 {
                // SAFETY: plain descriptor calls on descriptors this owns.
                unsafe {
                    dup2(s.saved, s.fd);
                    close(s.saved);
                }
            }
            let until = Instant::now() + wait;
            for s in self.0 {
                let left = until.saturating_duration_since(Instant::now());
                let _ = s.done.recv_timeout(left);
            }
        }
    }

    impl Stream {
        fn start(fd: RawFd, file: Shared) -> std::io::Result<Self> {
            let mut p = [0i32; 2];
            // SAFETY: plain descriptor calls; every new descriptor is either
            // handed to a `File` that owns it or closed on the error paths.
            unsafe {
                check(pipe(p.as_mut_ptr()))?;
                let echo = dup(fd);
                let saved = dup(fd);
                if echo < 0 || saved < 0 || dup2(p[1], fd) < 0 {
                    let e = std::io::Error::last_os_error();
                    for d in [p[0], p[1], echo, saved] {
                        if d >= 0 {
                            close(d);
                        }
                    }
                    return Err(e);
                }
                close(p[1]);
                for d in [p[0], echo, saved] {
                    fcntl(d, F_SETFD, FD_CLOEXEC);
                }
                let reader = std::fs::File::from_raw_fd(p[0]);
                let echo = std::fs::File::from_raw_fd(echo);
                let (tx, done) = mpsc::channel();
                std::thread::Builder::new()
                    .name(format!("log-tee-{fd}"))
                    .spawn(move || copy(reader, echo, &file, tx))
                    .map_err(|e| {
                        dup2(saved, fd);
                        close(saved);
                        e
                    })?;
                Ok(Self { fd, saved, done })
            }
        }
    }

    /// Everything the pipe gets, to the terminal and the file, until the
    /// game puts the stream back. A write that fails is dropped, never
    /// retried, so the pipe keeps draining.
    fn copy(
        mut reader: std::fs::File,
        mut echo: std::fs::File,
        file: &Shared,
        done: mpsc::Sender<()>,
    ) {
        let mut buf = [0u8; 8192];
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            let _ = echo.write_all(&buf[..n]);
            let mut f = file.lock().unwrap_or_else(|e| e.into_inner());
            let _ = f.write_all(&buf[..n]);
        }
        let _ = file.lock().unwrap_or_else(|e| e.into_inner()).flush();
        let _ = done.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sort_by_time_and_read_as_utc() {
        // 2026-11-22 10:53:07 UTC.
        assert_eq!(file_name(1_795_344_787), "openbf-2026-11-22_105307.log");
        assert_eq!(utc_stamp(0), "1970-01-01 00:00:00 UTC");
        // A leap day and the end of a year.
        assert_eq!(utc_stamp(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc_stamp(1_798_761_599), "2026-12-31 23:59:59 UTC");
        assert!(file_name(1_000) < file_name(1_000_000));
    }

    #[test]
    fn prune_keeps_the_newest_logs_and_nothing_else_is_touched() {
        let dir = std::env::temp_dir().join(format!("openbf-log-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for s in [10u64, 20, 30, 40, 50, 60] {
            std::fs::write(dir.join(file_name(s * 86_400)), "x").unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "keep").unwrap();
        prune(&dir, 4).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "notes.txt".to_owned(),
                file_name(30 * 86_400),
                file_name(40 * 86_400),
                file_name(50 * 86_400),
                file_name(60 * 86_400),
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The tee catches a raw write to the stream (as a C library's would
    /// be), keeps the terminal copy, and a panic's hook text, then lets go.
    #[cfg(unix)]
    #[test]
    fn the_tee_writes_both_streams_and_a_panic_to_the_file() {
        let dir = std::env::temp_dir().join(format!("openbf-log-tee-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = start_in(&dir).unwrap();
        assert!(start_in(&dir).is_err(), "one session log per process");
        std::io::stdout()
            .write_all(b"tee-test stdout marker\n")
            .unwrap();
        std::io::stderr()
            .write_all(b"tee-test stderr marker\n")
            .unwrap();
        let panicked = std::thread::Builder::new()
            .name("tee-test".into())
            .spawn(|| panic!("tee-test panic marker"))
            .unwrap()
            .join();
        assert!(panicked.is_err());
        finish();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("openbf session log: "), "{text}");
        assert!(text.contains("tee-test stdout marker"), "{text}");
        assert!(text.contains("tee-test stderr marker"), "{text}");
        assert!(
            text.contains("panic in thread 'tee-test': ") && text.contains("tee-test panic marker"),
            "{text}"
        );
        // Put back: later writes no longer reach the file.
        std::io::stdout().write_all(b"after finish\n").unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert!(!std::fs::read_to_string(&path)
            .unwrap()
            .contains("after finish"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
