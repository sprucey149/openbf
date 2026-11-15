//! The LAN host's self-check (issue #168, LAN §4.7): when a guest's hash
//! differs from the host's, the host first finds out whether its own game
//! parted, by replaying its round from the inputs everyone shares.
//!
//! Rules:
//! - The inputs are honest on every peer: the round's [`RoundSetup`] and
//!   the bundles the host closed (its [`super::desync::BundleLog`]). The
//!   round stepped through them is the round as it should be (the first
//!   playtest's reports re-simulated so).
//! - A replay starts from the host's latest [`Checkpoint`] when it has one
//!   before the mismatch: a copy of its own game ([`WalkSim::checkpoint`],
//!   [`EventFeed::checkpoint`]) at a check tick on which every live guest's
//!   hash equalled the host's (`net::NetEvent::HashesAgreed`), so that
//!   tick's game is proven by `Sim::hash`, and only by it: what the copy
//!   holds outside the hash (the feed's round points, the per-slot round
//!   state) is the host's own, unproven. Then a check steps the few
//!   seconds since, not the round; the checkpoint after tick C replays
//!   from tick C + 1. Without one (early in a round, or a lone guest that
//!   parted before any agreement), and for the check that confirms an
//!   agreeing one ([`super::peer`]), it builds the round from its setup
//!   with the caller's [`Rebuild`] and steps the log from tick 0.
//! - A [`Check`] steps the log to the mismatch tick T and compares
//!   `Sim::hash` there with the host's live hash (the `Desync` event's
//!   `host_hash`). Equal ([`Verdict::Agrees`]): the guest's game parted,
//!   and the check stops at once. Different ([`Verdict::Parted`]): the
//!   host's game parted, and the replay steps on to the host's current
//!   tick, ready to take the live game's place ([`Replayed`]).
//! - [`Rebuild::Thread`] runs the replay on a worker thread, so the host's
//!   frames go on: the build (a map load in the window), the log's decoding
//!   and the steps are all the worker's. The builder is handed the check's
//!   [`Stop`] to look at between its load stages. The host forwards every bundle it
//!   steps meanwhile ([`Check::forward`]), and [`Check::catch_up`] steps
//!   whatever the worker had not reached. [`Rebuild::Inline`] runs it at
//!   once on the caller's thread (the in-process virtual network, whose
//!   runs must repeat).
//! - One check at a time; dropping one stops its worker at the next tick.

use super::desync::{self, HashRing, LogSince};
use super::RoundSetup;
use crate::event_feed::EventFeed;
use crate::walk::WalkSim;
use net::TickBundle;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

/// A round built for slot `local` from its setup, with its event feed,
/// exactly as the peer built its own.
pub type Built = anyhow::Result<(WalkSim, Option<EventFeed>)>;

/// A builder a worker thread can run; it looks at the [`Stop`] between its
/// load stages.
pub type ThreadBuild = dyn Fn(&RoundSetup, u8, &Stop) -> Built + Send + Sync;

/// A worker's check was dropped (module rules): its builder stops at the
/// next load stage instead of finishing a map load nobody waits for.
pub struct Stop<'a>(&'a AtomicBool);

impl Stop<'_> {
    /// `Err` once the check was dropped.
    pub fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.0.load(Ordering::Relaxed), "stopped");
        Ok(())
    }
}

/// A builder run on the caller's thread.
pub type InlineBuild = dyn FnMut(&RoundSetup, u8) -> Built;

/// How the host rebuilds its round for a check (module rules).
pub enum Rebuild {
    Thread(Arc<ThreadBuild>),
    Inline(Box<InlineBuild>),
}

/// The host's own game after stepping check tick `tick`, every live guest
/// having agreed on it (module rules).
pub struct Checkpoint {
    pub tick: u32,
    pub walk: WalkSim,
    pub feed: Option<EventFeed>,
    /// The host's rows through `tick`.
    pub ring: HashRing,
}

impl Checkpoint {
    /// A copy of the host's game as it stands after stepping `tick`.
    pub fn of(tick: u32, walk: &WalkSim, feed: Option<&EventFeed>, ring: &HashRing) -> Self {
        Self {
            tick,
            walk: walk.checkpoint(),
            feed: feed.map(EventFeed::checkpoint),
            ring: ring.clone(),
        }
    }

    /// Another copy, for a check to step (this one stays).
    pub fn copy(&self) -> Self {
        Self::of(self.tick, &self.walk, self.feed.as_ref(), &self.ring)
    }
}

/// Where a check's replay starts (module rules).
pub enum Start {
    /// Build the round from its setup; replay from tick 0.
    Setup(RoundSetup),
    /// From this checkpoint; replay from the tick after it.
    Checkpoint(Box<Checkpoint>),
}

impl Start {
    /// The first tick the replay steps.
    pub fn first_tick(&self) -> u32 {
        match self {
            Start::Setup(_) => 0,
            Start::Checkpoint(c) => c.tick + 1,
        }
    }
}

/// The round replayed from its log, stepped through `next_tick - 1`.
pub struct Replayed {
    pub walk: WalkSim,
    pub feed: Option<EventFeed>,
    /// Its rows of the last [`desync::RING_TICKS`] ticks.
    pub ring: HashRing,
    pub next_tick: u32,
}

/// What a check found (module rules). `hash`: the replay's at the tick.
pub enum Verdict {
    Agrees { hash: u64 },
    Parted { hash: u64, replayed: Box<Replayed> },
}

/// Steps `b` on the replay (no sounds, no hooks: nobody watches it).
fn step(r: &mut Replayed, b: &TickBundle) -> anyhow::Result<()> {
    anyhow::ensure!(
        b.tick == r.next_tick,
        "bundle {} where tick {} was due",
        b.tick,
        r.next_tick
    );
    super::step(&mut r.walk, b, r.feed.as_mut(), |_, _| {});
    r.ring.push(b.tick, desync::row_of(&r.walk.sim));
    r.next_tick = b.tick + 1;
    Ok(())
}

/// The replay's first state: the round built (`build`), or a checkpoint.
fn begin(start: Start, build: impl FnOnce(&RoundSetup) -> Built) -> anyhow::Result<Replayed> {
    Ok(match start {
        Start::Setup(setup) => {
            let (walk, feed) = build(&setup)?;
            Replayed {
                walk,
                feed,
                ring: HashRing::default(),
                next_tick: 0,
            }
        }
        Start::Checkpoint(c) => {
            let c = *c;
            Replayed {
                walk: c.walk,
                feed: c.feed,
                ring: c.ring,
                next_tick: c.tick + 1,
            }
        }
    })
}

/// The replay itself (module rules): `log` from the start's tick, then
/// whatever `more` holds when it is done, if the host's game parted at
/// `tick`.
fn replay(
    begun: anyhow::Result<Replayed>,
    log: LogSince,
    more: Option<&mpsc::Receiver<TickBundle>>,
    tick: u32,
    live: u64,
    cancel: &AtomicBool,
) -> anyhow::Result<Verdict> {
    let mut r = begun?;
    let bundles = log.decode()?;
    anyhow::ensure!(
        bundles.last().is_some_and(|b| b.tick >= tick),
        "the log ends before the check at tick {tick}"
    );
    let n = bundles.len();
    let mut parted = None;
    for (i, b) in bundles.iter().enumerate() {
        anyhow::ensure!(!cancel.load(Ordering::Relaxed), "stopped");
        if i + desync::RING_TICKS >= n {
            step(&mut r, b)?;
        } else {
            // Rows only where the ring would keep them.
            anyhow::ensure!(b.tick == r.next_tick, "bundle {i} says tick {}", b.tick);
            super::step(&mut r.walk, b, r.feed.as_mut(), |_, _| {});
            r.next_tick = b.tick + 1;
        }
        if b.tick == tick {
            let hash = r.walk.sim.hash();
            if hash == live {
                return Ok(Verdict::Agrees { hash });
            }
            parted = Some(hash);
        }
    }
    if let Some(more) = more {
        while let Ok(b) = more.try_recv() {
            anyhow::ensure!(!cancel.load(Ordering::Relaxed), "stopped");
            step(&mut r, &b)?;
        }
    }
    let hash = parted.ok_or_else(|| anyhow::anyhow!("the replay never stepped tick {tick}"))?;
    Ok(Verdict::Parted {
        hash,
        replayed: Box::new(r),
    })
}

enum Job {
    Done(Option<anyhow::Result<Verdict>>),
    Thread {
        rx: mpsc::Receiver<anyhow::Result<Verdict>>,
        tx: mpsc::Sender<TickBundle>,
        cancel: Arc<AtomicBool>,
    },
}

/// A guest whose hash differed from the host's, as the host saw it then.
#[derive(Clone, Debug)]
pub struct Differ {
    /// Its first differing check tick, and both hashes there.
    pub tick: u32,
    pub host_hash: u64,
    pub peer_hash: u64,
    /// The host's rows of the report at `tick`, taken then (a slow check
    /// must not find them gone from the ring).
    pub host_rows: Vec<String>,
}

/// One self-check in flight (module rules).
pub struct Check {
    /// The check tick whose hashes differed, and the host's live hash there.
    pub tick: u32,
    pub live: u64,
    /// The replay's first tick (0 without a checkpoint).
    pub from: u32,
    /// The host's tick when it started, and the tick it gives up at.
    pub started_at: u32,
    pub deadline: u32,
    /// The guests whose hash differed from the host's, the first one's at
    /// `tick`, the others' while it ran: each one's first.
    pub differs: BTreeMap<u8, Differ>,
    /// Bundles the host stepped after the check took its copy of the log.
    forwarded: VecDeque<TickBundle>,
    job: Job,
}

impl Check {
    /// Starts a check of guest `slot`'s difference `first` from `start`
    /// and the round's log from the start's tick on, at host tick `now`,
    /// giving up at `deadline`.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        rebuild: &mut Rebuild,
        start: Start,
        local: u8,
        log: LogSince,
        slot: u8,
        first: Differ,
        now: u32,
        deadline: u32,
    ) -> Self {
        let (tick, live) = (first.tick, first.host_hash);
        let from = start.first_tick();
        let job = match rebuild {
            Rebuild::Inline(build) => {
                let none = AtomicBool::new(false);
                let begun = begin(start, |s| build(s, local));
                Job::Done(Some(replay(begun, log, None, tick, live, &none)))
            }
            Rebuild::Thread(build) => {
                let (rtx, rx) = mpsc::channel();
                let (tx, brx) = mpsc::channel();
                let cancel = Arc::new(AtomicBool::new(false));
                let (build, stop) = (Arc::clone(build), Arc::clone(&cancel));
                let spawned = std::thread::Builder::new()
                    .name("lan-self-check".into())
                    .spawn(move || {
                        let halt = Stop(&stop);
                        let begun = halt
                            .check()
                            .and_then(|()| begin(start, |s| build(s, local, &halt)));
                        let _ = rtx.send(replay(begun, log, Some(&brx), tick, live, &stop));
                    });
                match spawned {
                    Ok(_) => Job::Thread { rx, tx, cancel },
                    Err(e) => Job::Done(Some(Err(anyhow::anyhow!("no worker thread: {e}")))),
                }
            }
        };
        Self {
            tick,
            live,
            from,
            started_at: now,
            deadline,
            differs: BTreeMap::from([(slot, first)]),
            forwarded: VecDeque::new(),
            job,
        }
    }

    /// A bundle the host stepped while the check runs.
    pub fn forward(&mut self, b: &TickBundle) {
        if let Job::Thread { tx, .. } = &self.job {
            let _ = tx.send(b.clone());
        }
        self.forwarded.push_back(b.clone());
    }

    /// The verdict, once there is one (never blocks).
    pub fn poll(&mut self) -> Option<anyhow::Result<Verdict>> {
        match &mut self.job {
            Job::Done(v) => v.take(),
            Job::Thread { rx, .. } => match rx.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("the check's worker stopped")))
                }
            },
        }
    }

    /// Steps the forwarded bundles `r` has not stepped, through `next_tick
    /// - 1` (the host's own).
    pub fn catch_up(&mut self, r: &mut Replayed, next_tick: u32) -> anyhow::Result<()> {
        while let Some(b) = self.forwarded.pop_front() {
            if b.tick >= r.next_tick {
                step(r, &b)?;
            }
        }
        anyhow::ensure!(
            r.next_tick == next_tick,
            "the replay reached tick {}, the host {next_tick}",
            r.next_tick
        );
        Ok(())
    }
}

impl Drop for Check {
    fn drop(&mut self) {
        if let Job::Thread { cancel, .. } = &self.job {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
