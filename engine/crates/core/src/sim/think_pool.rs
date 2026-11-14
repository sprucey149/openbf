//! Issue #168 (`analysis/plan/LAN_MULTIPLAYER.md` §8b): the bots' thinks on
//! worker threads, with a result that does not depend on how many.
//!
//! Why it is safe: every think reads a sim no one moves until the write-back
//! ([`crate::ai::think`] takes `&Sim`; `Sim: Sync`, so nothing in it can be
//! written through a shared reference), and its only output is the
//! `(AiState, PadState, NavTick)` the write-back applies afterwards, in unit
//! index order, on the calling thread. A think therefore never reads what
//! another think of the same pass wrote, and the order the thinks ran in
//! cannot reach the sim.
//!
//! The per-thread caches the thinks touch, and why a worker's are its own:
//! - `ai::think_memo` (the lane and floor-walk answers): opened and cleared
//!   per worker per pass exactly as the serial pass opens the caller's. Its
//!   answers are pure functions of their key and the unmoved sim, so a hit
//!   and a fresh cast agree, and a partition that casts again gets the same
//!   answer another partition remembered.
//! - `collision::cost` (ray counters): a diagnostic; each worker's count is
//!   taken and added back to the caller's ([`super::Sim::tick_cost`]).
//! - `collision::world`'s broad-phase scratch, `walls::queries`' EPA buffers
//!   and start contacts, `ai::team`'s near lists: scratch buffers emptied or
//!   reset before every use; they hold no answer between calls.
//!
//! The partitions are fixed: thread `w` of `n` runs jobs `w, w + n, ...`
//! (interleaved, so a run of expensive bots spreads). Thread 0 is the
//! caller; the others are persistent pool threads woken per pass. Results
//! come back in job order whatever finished first.

use std::sync::OnceLock;

/// The default cap on think threads (the caller included).
const AUTO_CAP: usize = 4;
/// The most threads `OPENBF_SIM_THREADS` may ask for.
const MAX_THREADS: usize = 64;
/// Fewer thinks than this per thread are not worth the thread: a short list
/// runs on fewer threads (still the same result).
pub(crate) const MIN_THINKS_PER_THREAD: usize = 2;
/// A worker's stack: the main thread's usual 8 MiB, since a think runs on
/// either (the default spawned stack is 2 MiB).
const WORKER_STACK: usize = 8 << 20;

/// The think threads a new [`super::Sim`] uses (its `think_threads`):
/// `OPENBF_SIM_THREADS` when set to a number (1 = the serial pass), else the
/// machine's available parallelism minus one, between 1 and 4. Read once per
/// process. It changes how fast a tick runs, never what it computes.
pub fn default_think_threads() -> usize {
    static THREADS: OnceLock<usize> = OnceLock::new();
    *THREADS.get_or_init(|| {
        resolve(
            std::env::var("OPENBF_SIM_THREADS").ok().as_deref(),
            std::thread::available_parallelism().map_or(1, usize::from),
        )
    })
}

/// [`default_think_threads`] from its two inputs.
fn resolve(env: Option<&str>, available: usize) -> usize {
    match env.and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(n) => n.clamp(1, MAX_THREADS),
        None => available.saturating_sub(1).clamp(1, AUTO_CAP),
    }
}

/// How many threads `jobs` thinks run on, given the sim's `think_threads`.
pub(crate) fn threads_for(think_threads: usize, jobs: usize) -> usize {
    think_threads
        .clamp(1, MAX_THREADS)
        .min(jobs.div_ceil(MIN_THINKS_PER_THREAD))
        .max(1)
}

/// A pass's work for one pool thread: a lifetime-erased closure (see the
/// SAFETY note in [`run`]).
type Task = Box<dyn FnOnce() + Send + 'static>;

/// The persistent think threads, shared by every sim of the process and
/// grown on demand: a tick wakes them (a channel send) instead of spawning
/// threads, which cost more than the thinks they would run.
fn pool(threads: usize) -> Vec<std::sync::mpsc::Sender<Task>> {
    static POOL: OnceLock<std::sync::Mutex<Vec<std::sync::mpsc::Sender<Task>>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    while pool.len() < threads {
        let (tx, rx) = std::sync::mpsc::channel::<Task>();
        let spawned = std::thread::Builder::new()
            .name(format!("think-{}", pool.len() + 1))
            .stack_size(WORKER_STACK)
            .spawn(move || {
                // A task catches its own panic, so the thread outlives it.
                while let Ok(task) = rx.recv() {
                    task();
                }
            });
        if spawned.is_err() {
            break;
        }
        pool.push(tx);
    }
    pool.iter().take(threads).cloned().collect()
}

/// What a pool thread's partition sends back: its results and its rays, or
/// its panic.
type PartResult<R> = std::thread::Result<(Vec<R>, crate::collision::cost::RayCost)>;

/// Run `think` on every job, on `threads` threads in fixed interleaved
/// partitions, each inside its own open think memo; the results in `jobs`
/// order. One thread is the serial pass: the caller, one memo, in order.
/// A think's panic reaches the caller, after every partition has finished.
pub(crate) fn run<J: Sync, R: Send>(
    jobs: &[J],
    threads: usize,
    think: impl Fn(&J) -> R + Sync,
) -> Vec<R> {
    let n = threads.clamp(1, jobs.len().max(1));
    if n == 1 {
        return crate::ai::think_memo::during_thinks(|| jobs.iter().map(&think).collect());
    }
    let think = &think;
    let part = move |w: usize| -> Vec<R> {
        crate::ai::think_memo::during_thinks(|| jobs.iter().skip(w).step_by(n).map(think).collect())
    };
    let part = &part;
    #[cfg(test)]
    let style = crate::ai::tests::STYLE.with(std::cell::Cell::get);
    let (done_tx, done_rx) = std::sync::mpsc::channel::<(usize, PartResult<R>)>();
    let workers = pool(n - 1);
    let mut parts: Vec<Option<Vec<R>>> = (0..n).map(|_| None).collect();
    let mut panic = None;
    for w in 1..n {
        let tx = done_tx.clone();
        let task = move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // A test scene's shooting style is the caller's.
                #[cfg(test)]
                crate::ai::tests::STYLE.with(|c| c.set(style));
                crate::collision::cost::take();
                let out = part(w);
                #[cfg(test)]
                crate::ai::tests::STYLE.with(|c| c.set(None));
                (out, crate::collision::cost::take())
            }));
            let _ = tx.send((w, r));
        };
        let task: Box<dyn FnOnce() + Send + '_> = Box::new(task);
        // SAFETY: the task borrows `jobs`, `think` and `part` from this call.
        // It is only extended to `'static` so a pool thread can hold it; this
        // function does not return or unwind before every task has finished
        // or been dropped: each task owns a clone of `done_tx`, the caller's
        // own clone is dropped below, and the loop below receives until
        // every clone is gone (`recv` errs only then). The caller's own
        // partition runs under `catch_unwind`, so its panic waits too.
        let task: Task =
            unsafe { std::mem::transmute::<Box<dyn FnOnce() + Send + '_>, Task>(task) };
        if let Some(Err(std::sync::mpsc::SendError(task))) =
            workers.get(w - 1).map(|tx| tx.send(task))
        {
            // The pool thread is gone: run it here.
            task();
        }
    }
    drop(done_tx);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| part(0))) {
        Ok(out) => parts[0] = Some(out),
        Err(p) => panic = Some(p),
    }
    while let Ok((w, r)) = done_rx.recv() {
        match r {
            Ok((out, rays)) => {
                crate::collision::cost::add(rays);
                parts[w] = Some(out);
            }
            Err(p) => {
                panic.get_or_insert(p);
            }
        }
    }
    if let Some(p) = panic {
        std::panic::resume_unwind(p);
    }
    // Back into job order: job k is part k % n's (k / n)th. A task a dying
    // pool thread dropped unrun is run here (the same answer).
    let mut iters: Vec<_> = parts
        .into_iter()
        .enumerate()
        .map(|(w, p)| p.unwrap_or_else(|| part(w)).into_iter())
        .collect();
    let mut out = Vec::with_capacity(jobs.len());
    for k in 0..jobs.len() {
        out.extend(iters[k % n].next());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thread_count_comes_from_the_override_or_the_machine() {
        assert_eq!(resolve(None, 1), 1, "one core: the serial pass");
        assert_eq!(resolve(None, 2), 1);
        assert_eq!(resolve(None, 4), 3);
        assert_eq!(resolve(None, 32), AUTO_CAP, "capped");
        assert_eq!(resolve(Some("1"), 32), 1, "the override wins");
        assert_eq!(resolve(Some(" 8 "), 2), 8);
        assert_eq!(resolve(Some("0"), 8), 1);
        assert_eq!(resolve(Some("999"), 8), MAX_THREADS);
        assert_eq!(resolve(Some("many"), 8), AUTO_CAP, "not a number: auto");
        assert_eq!(threads_for(8, 1), 1, "one think stays on one thread");
        assert_eq!(threads_for(8, 5), 3, "two or more thinks a thread");
        assert_eq!(threads_for(8, 64), 8);
        assert_eq!(threads_for(4, 9), 4);
        assert_eq!(threads_for(0, 64), 1);
    }

    #[test]
    fn results_come_back_in_job_order_on_any_thread_count() {
        let jobs: Vec<u32> = (0..37).collect();
        let want: Vec<u64> = jobs.iter().map(|&j| u64::from(j) * 7 + 1).collect();
        for n in [1, 2, 3, 4, 8, 37, 50] {
            let got = run(&jobs, n, |&j| u64::from(j) * 7 + 1);
            assert_eq!(got, want, "{n} threads");
        }
        assert!(run(&[] as &[u32], 4, |&j| j).is_empty());
    }

    #[test]
    fn a_worker_panic_reaches_the_caller() {
        let jobs: Vec<u32> = (0..8).collect();
        let caught = std::panic::catch_unwind(|| {
            run(&jobs, 4, |&j| {
                assert!(j != 5, "think 5");
                j
            })
        });
        let msg = caught.expect_err("the panic");
        let text = msg
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| msg.downcast_ref::<&str>().copied());
        assert_eq!(text, Some("think 5"));
    }

    /// The scenes: the pinned all-proxy skirmish (four proxies, Elite), and
    /// 24 bots of both sides on the L graph under BF1 and BF2 rules, one
    /// with a human walking and firing among them.
    fn scenes() -> Vec<(&'static str, crate::sim::Sim, bool)> {
        use crate::sim::Sim;
        use crate::unit::{Controller, UnitSpec};
        use glam::Vec3;
        let bot = |sim: &mut Sim, team: u8, at: Vec3| {
            sim.spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(team, at)
            })
            .expect("bot")
        };
        let mut out = Vec::new();
        let (mut s, _) = crate::ai::tests::l_sim();
        s.units.clear();
        s.players.clear();
        s.difficulty = crate::skill::Difficulty::Elite;
        let p = crate::ai::proxy::Strength::Good.profile();
        for (team, x, z) in [
            (0, 0.0, 0.0),
            (0, 3.0, 0.0),
            (1, 0.0, -20.0),
            (1, 3.0, -20.0),
        ] {
            let h = bot(&mut s, team, Vec3::new(x, 0.0, z));
            assert!(s.set_proxy(h, Some(p)));
        }
        out.push(("proxy_skirmish", s, false));
        for (name, bf2, human) in [
            ("l_bots_bf1", false, false),
            ("l_bots_bf2_human", true, true),
        ] {
            let (mut s, _) = crate::ai::tests::l_sim();
            if bf2 {
                s.ground_rules = crate::soldier::GroundRules::Bf2;
            }
            if human {
                s.spawn(&UnitSpec::player(0, Vec3::new(1.0, 0.0, 3.0)))
                    .expect("player");
            }
            for k in 0..12 {
                let f = k as f32;
                bot(&mut s, 0, Vec3::new(-6.0 + f, 0.0, 2.0 + (k % 3) as f32));
                bot(&mut s, 1, Vec3::new(34.0 + f, 0.0, -44.0 + (k % 4) as f32));
            }
            out.push((name, s, human));
        }
        out
    }

    /// The human's pad: walk, turn and fire, from integer arithmetic.
    fn human_pad(t: u32) -> crate::sim::PadState {
        crate::sim::PadState {
            ly: if (t / 90) % 3 == 2 { -12000 } else { i16::MAX },
            rx: ((t % 80) as i16 - 40) * 300,
            buttons: if t % 20 < 6 {
                crate::sim::buttons::FIRE
            } else {
                0
            },
            ..Default::default()
        }
    }

    /// Issue #168 (§8b): the thinks on 1, 2, 3 and 8 threads give the same
    /// sim, hashed every tick; and a predicted step (no thinks) from the
    /// threaded sim is the serial one's.
    #[test]
    fn any_think_thread_count_steps_the_same_sim() {
        for (name, base, human) in scenes() {
            let mut sims: Vec<_> = [1usize, 2, 3, 8]
                .into_iter()
                .map(|n| {
                    let mut s = base.clone();
                    s.think_threads = n;
                    s
                })
                .collect();
            let bots = base
                .units
                .iter()
                .filter(|u| u.controller == crate::unit::Controller::Ai)
                .count();
            let mut peak = 0;
            for t in 1..=600u32 {
                let pads = if human {
                    vec![human_pad(t)]
                } else {
                    Vec::new()
                };
                for s in &mut sims {
                    s.step(&pads);
                }
                let thinking = sims[0]
                    .units
                    .iter()
                    .filter(|u| u.alive && u.controller == crate::unit::Controller::Ai)
                    .count();
                peak = peak.max(thinking);
                let want = sims[0].hash();
                for s in &sims[1..] {
                    assert_eq!(
                        format!("{:016x}", s.hash()),
                        format!("{want:016x}"),
                        "{name}: {} threads differ from 1 at tick {t} ({bots} bots)",
                        s.think_threads
                    );
                }
            }
            assert!(
                peak >= 4,
                "{name}: at least two think threads ran ({peak} thinking at most)"
            );
            let mut a = sims[0].clone();
            let mut b = sims[3].clone();
            for t in 601..=608u32 {
                let pads = if human {
                    vec![human_pad(t)]
                } else {
                    Vec::new()
                };
                a.step_predicted(&pads);
                b.step_predicted(&pads);
                assert_eq!(a.hash(), b.hash(), "{name}: predicted tick {t}");
            }
        }
    }
}
