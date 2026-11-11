//! Card WHY: a reason code on every behaviour change and on every failed
//! walk of a bot (`analysis/bot-design.md` 4.4, 4.5: "every walk has an
//! outcome - Failed(why)"). The codes are the ones the code already knows at
//! those points: the link of `think`'s chain that ran ([`super::brain`]),
//! and what the mover's books say when a progress window fails (the
//! steering's pick, the going-round commitment, the leg's source). L4D's
//! action system carries the same "reason" on each transition
//! (`engine/artifacts/lead_tmp/botre/modern.md` A6).
//!
//! DIAGNOSTIC ONLY: the log ([`WhyLog`], a small fixed ring per bot) is
//! never hashed and never read by a rule. The
//! black box shows it per unit; the sweep's census counts the failed-walk
//! codes per map (`NavStats::fails`).

use super::state::{AiState, Behaviour, NavTick, Route, RouteSource, SteerPick};

/// Why a bot changed what it does, or why its walk failed. The failed-walk
/// codes are the block from [`Why::Wall`] on ([`Why::is_fail`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Why {
    #[default]
    None,
    // ── A behaviour change: what the new behaviour was entered for.
    /// Off the playable level: the walk back to it (card BNDS).
    OffLevel,
    /// A grenade on the ground near by: run from it.
    Grenade,
    /// An item to pick up.
    Pickup,
    /// On its sniper's spot (card SNPR).
    SnipeSpot,
    /// On the Cover role's position (card TEAM).
    GuardSpot,
    /// A target seen in reach.
    Seen,
    /// Hit in the last second.
    Hit,
    /// A remembered target keeps it on its position.
    Memory,
    /// A vehicle claimed: the walk to board it.
    Claim,
    /// A squad order.
    Order,
    /// At its post (or its defence spot).
    Arrived,
    /// The commander gave it another post.
    GoalChanged,
    /// The fight is over (no target in reach for its last 2 s).
    TargetLost,
    /// The run from the grenade is over.
    Safe,
    /// The walk got nearer its waypoint again: out of Recover.
    Recovered,
    /// The claim on a vehicle ended.
    ClaimEnded,
    /// In a seat.
    Seated,
    /// Nowhere to be and nobody to fight.
    Idle,
    /// Back on its way (anything else that ends in Travel).
    SetOut,
    // ── A failed walk: what the mover's books said when it failed.
    /// The steering slid along a wall or fanned off it.
    Wall,
    /// What shut the way last was a rim barrier: an edge of the level.
    Rim,
    /// Going round a body (a soldier, a parked hull, a prop).
    Body,
    /// Going round, and the allowance for it spent (card OBST).
    RoundSpent,
    /// No floor ahead: the bot stands at a ledge (card STFAL).
    Ledge,
    /// Every way was shut and the steering backed off.
    Boxed,
    /// The leg had no hop on the graph: a straight walk at the goal.
    OffGraph,
    /// The way ahead was open and the bot got no nearer (pushed, jammed).
    NoGain,
    /// Back and forth at its waypoint (card ARCM's detector).
    Oscillation,
    /// A driver's progress window (hover / walker).
    Driving,
    /// A lost bot's walk back to the level got no nearer its point.
    ReturnFailed,
    /// The Recover ladder gave the goal up (`GIVE_UP_AFTER` failed windows).
    GaveUp,
    /// Failed windows at one place: sent off along the floor (card RLOOP).
    Pinned,
    /// The last resort: recycled (cards RCYC, BNDS).
    Recycled,
    /// Moved on to a hub past the failing link where no human saw it, in
    /// place of the recycle (card UNSN).
    Relocated,
}

/// The behaviour-change codes, in order (`Why as u8` indexes it; card
/// HYST's census of what the early thinks changed).
pub const CHANGES: [Why; 20] = [
    Why::None,
    Why::OffLevel,
    Why::Grenade,
    Why::Pickup,
    Why::SnipeSpot,
    Why::GuardSpot,
    Why::Seen,
    Why::Hit,
    Why::Memory,
    Why::Claim,
    Why::Order,
    Why::Arrived,
    Why::GoalChanged,
    Why::TargetLost,
    Why::Safe,
    Why::Recovered,
    Why::ClaimEnded,
    Why::Seated,
    Why::Idle,
    Why::SetOut,
];

/// The failed-walk codes, in order (the census's columns).
pub const FAILS: [Why; 15] = [
    Why::Wall,
    Why::Rim,
    Why::Body,
    Why::RoundSpent,
    Why::Ledge,
    Why::Boxed,
    Why::OffGraph,
    Why::NoGain,
    Why::Oscillation,
    Why::Driving,
    Why::ReturnFailed,
    Why::GaveUp,
    Why::Pinned,
    Why::Recycled,
    Why::Relocated,
];

impl Why {
    /// A failed walk's code (not a behaviour change's).
    pub fn is_fail(self) -> bool {
        self as u8 >= Why::Wall as u8
    }

    /// Its column in [`FAILS`] (and `NavStats::fails`).
    pub fn fail_index(self) -> Option<usize> {
        self.is_fail()
            .then(|| usize::from(self as u8 - Why::Wall as u8))
    }

    /// The short name the census and the black box print.
    pub fn tag(self) -> &'static str {
        match self {
            Why::None => "none",
            Why::OffLevel => "off_level",
            Why::Grenade => "grenade",
            Why::Pickup => "pickup",
            Why::SnipeSpot => "snipe_spot",
            Why::GuardSpot => "guard_spot",
            Why::Seen => "seen",
            Why::Hit => "hit",
            Why::Memory => "memory",
            Why::Claim => "claim",
            Why::Order => "order",
            Why::Arrived => "arrived",
            Why::GoalChanged => "goal_changed",
            Why::TargetLost => "target_lost",
            Why::Safe => "safe",
            Why::Recovered => "recovered",
            Why::ClaimEnded => "claim_ended",
            Why::Seated => "seated",
            Why::Idle => "idle",
            Why::SetOut => "set_out",
            Why::Wall => "wall",
            Why::Rim => "rim",
            Why::Body => "body",
            Why::RoundSpent => "round_spent",
            Why::Ledge => "ledge",
            Why::Boxed => "boxed",
            Why::OffGraph => "off_graph",
            Why::NoGain => "no_gain",
            Why::Oscillation => "oscillation",
            Why::Driving => "driving",
            Why::ReturnFailed => "return_failed",
            Why::GaveUp => "gave_up",
            Why::Pinned => "pinned",
            Why::Recycled => "recycled",
            Why::Relocated => "relocated",
        }
    }
}

/// One line of the log: at `tick`, the bot entered `doing` for `why`, or
/// (`failed`) its walk failed for `why` while it was `doing`, on nav arc
/// `arc` (`u16::MAX`: none - the last leg, a straight walk, no route).
/// Recover is entered for a failed-walk code, so `failed` tells the two
/// apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WhyRow {
    pub tick: u64,
    pub doing: Behaviour,
    pub why: Why,
    pub arc: u16,
    pub failed: bool,
}

/// Rows a bot keeps.
pub const WHY_ROWS: usize = 8;

/// The last [`WHY_ROWS`] reasons of one bot's life, a ring. DIAGNOSTIC:
/// never hashed, never read by a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WhyLog {
    rows: [WhyRow; WHY_ROWS],
    /// Rows ever written (the next goes to `n % WHY_ROWS`).
    n: u32,
}

impl WhyLog {
    pub fn push(&mut self, row: WhyRow) {
        self.rows[self.n as usize % WHY_ROWS] = row;
        self.n = self.n.wrapping_add(1);
    }

    /// The rows kept, oldest first.
    pub fn rows(&self) -> impl Iterator<Item = &WhyRow> {
        let n = self.n as usize;
        let kept = n.min(WHY_ROWS);
        (n - kept..n).map(move |i| &self.rows[i % WHY_ROWS])
    }

    /// The newest row.
    pub fn last(&self) -> Option<&WhyRow> {
        self.rows().last()
    }
}

/// What the mover's books say about a progress window that failed on leg
/// `r` (read before the window's own cure changes them): the steering's
/// pick and the going-round commitment, else the leg's source.
pub(super) fn walk_failed(st: &AiState, r: &Route, driving: bool) -> Why {
    let round = st.mv.round;
    match st.steer {
        _ if driving => Why::Driving,
        _ if r.source == RouteSource::Straight => Why::OffGraph,
        _ if round.rim => Why::Rim,
        _ if round.ticks >= super::avoid::ROUND_MAX => Why::RoundSpent,
        SteerPick::Ledge => Why::Ledge,
        SteerPick::Back => Why::Boxed,
        SteerPick::Slide | SteerPick::Fan(_) => Why::Wall,
        SteerPick::Round => Why::Body,
        _ if round.live() || round.foot != 0 => Why::Body,
        _ => Why::NoGain,
    }
}

/// The failed-walk codes a tick reported, in order: the window's, what came
/// of it, and a recycle (`NavTick::stalled` / `recycle`) or its unseen move
/// (`NavTick::relocate`, card UNSN) last.
pub fn fail_codes(nt: &NavTick) -> impl Iterator<Item = Why> + '_ {
    let recycled = match () {
        _ if nt.stalled != 0 || nt.recycle != 0 => Some(Why::Recycled),
        _ if nt.relocate.is_some() => Some(Why::Relocated),
        _ => None,
    };
    nt.fails
        .iter()
        .copied()
        .take_while(|w| *w != Why::None)
        .chain(recycled)
}

/// After the think: log the failed walk the tick reported, then the change
/// of behaviour (or of post) with the reason the chain gave.
pub(super) fn log(st: &mut AiState, tick: u64, was: Behaviour, cp: Option<u32>, nt: &NavTick) {
    let doing = st.behaviour;
    for (i, why) in fail_codes(nt).enumerate() {
        st.why.push(WhyRow {
            tick,
            doing,
            why,
            arc: if i == 0 { nt.fail_arc } else { None }.unwrap_or(u16::MAX),
            failed: true,
        });
    }
    let new_post = st.target_cp != cp && st.target_cp.is_some();
    if doing == was && !new_post {
        return;
    }
    let why = match nt.why {
        // Recover: the failure that put it there.
        Why::None | Why::SetOut if doing == Behaviour::Recover && nt.fail() != Why::None => {
            nt.fail()
        }
        Why::None | Why::SetOut if new_post => Why::GoalChanged,
        Why::None => match doing {
            Behaviour::Ride | Behaviour::Man => Why::Seated,
            Behaviour::None => Why::Idle,
            _ => Why::SetOut,
        },
        w => w,
    };
    st.why.push(WhyRow {
        tick,
        doing,
        why,
        arc: u16::MAX,
        failed: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_keeps_the_newest_rows_oldest_first() {
        let mut l = WhyLog::default();
        assert_eq!(l.rows().count(), 0);
        for t in 0..(WHY_ROWS as u64 + 3) {
            l.push(WhyRow {
                tick: t,
                ..WhyRow::default()
            });
        }
        let ticks: Vec<u64> = l.rows().map(|r| r.tick).collect();
        assert_eq!(ticks, (3..WHY_ROWS as u64 + 3).collect::<Vec<_>>());
        assert_eq!(l.last().map(|r| r.tick), Some(WHY_ROWS as u64 + 2));
    }

    #[test]
    fn the_change_codes_are_indexed_by_their_value() {
        for (i, w) in CHANGES.iter().enumerate() {
            assert_eq!(*w as usize, i, "{w:?}");
            assert!(!w.is_fail());
        }
        assert_eq!(CHANGES.len(), Why::Wall as usize);
    }

    #[test]
    fn the_fail_block_is_contiguous_and_in_order() {
        for (i, w) in FAILS.iter().enumerate() {
            assert_eq!(w.fail_index(), Some(i), "{w:?}");
        }
        assert!(!Why::SetOut.is_fail());
        assert_eq!(
            Why::Relocated as usize - Why::Wall as usize + 1,
            FAILS.len()
        );
    }

    /// Four 12 m walls round the origin, 5 m out: no way out.
    fn closed_room() -> crate::collision::CollisionWorld {
        use crate::collision::{Body, CollisionWorld, Shape};
        use glam::Vec3;
        let slab =
            |c: Vec3, half: Vec3| Body::new(glam::Mat4::from_translation(c), Shape::Box { half });
        CollisionWorld::new(
            vec![
                slab(Vec3::new(0.0, 0.0, -5.5), Vec3::new(6.0, 6.0, 0.5)),
                slab(Vec3::new(0.0, 0.0, 5.5), Vec3::new(6.0, 6.0, 0.5)),
                slab(Vec3::new(-5.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 6.0)),
                slab(Vec3::new(5.5, 0.0, 0.0), Vec3::new(0.5, 6.0, 6.0)),
            ],
            Vec::new(),
        )
    }

    /// A bot penned in a room: its first behaviour is logged with a reason,
    /// each failed window is a row with the mover's code and the leg's arc,
    /// Recover is entered for that code, and the round's census counts the
    /// same failures as the log holds.
    #[test]
    fn a_penned_bot_logs_its_failed_walks_and_why_it_recovers() {
        use crate::sim::PadState;
        let (mut sim, h) = crate::ai::tests::l_sim();
        sim.collision = closed_room();
        for _ in 0..900 {
            sim.step(&[PadState::default()]);
            if sim.nav_stats.fails.iter().sum::<u64>() >= 2 {
                break;
            }
        }
        let rows: Vec<WhyRow> = sim.unit(h).expect("alive").ai.why.rows().copied().collect();
        // The first row: why it set out (its first post).
        assert_eq!(rows[0].why, Why::GoalChanged, "{rows:?}");
        // The first failure: the mover's code and the leg's arc, then
        // Recover entered for it on the same tick.
        let i = rows.iter().position(|r| r.failed).expect("a failure");
        assert!(rows[i].why.is_fail(), "{rows:?}");
        let (fail, next) = (rows[i], rows[i + 1]);
        assert_ne!(fail.why, Why::OffGraph, "{rows:?}");
        assert_ne!(fail.arc, u16::MAX, "the leg's arc: {rows:?}");
        assert_eq!(
            (next.tick, next.doing, next.why, next.failed),
            (fail.tick, Behaviour::Recover, fail.why, false),
            "{rows:?}"
        );
        // The census counts each failed window once.
        let windows = rows.iter().filter(|r| r.failed).count() as u64;
        assert_eq!(sim.nav_stats.fails.iter().sum::<u64>(), windows, "{rows:?}");
    }

    /// A failed window is classified by the steering's pick and the
    /// commitment, the leg's source first.
    #[test]
    fn a_failed_window_names_what_the_books_say() {
        let mut st = AiState::default();
        let r = Route::default();
        st.steer = SteerPick::Slide;
        assert_eq!(walk_failed(&st, &r, false), Why::Wall);
        st.steer = SteerPick::Ledge;
        assert_eq!(walk_failed(&st, &r, false), Why::Ledge);
        st.steer = SteerPick::Fwd;
        assert_eq!(walk_failed(&st, &r, false), Why::NoGain);
        st.mv.round.meet = 2.0;
        assert_eq!(walk_failed(&st, &r, false), Why::Body);
        st.mv.round.rim = true;
        assert_eq!(walk_failed(&st, &r, false), Why::Rim);
        let straight = Route {
            source: RouteSource::Straight,
            ..Route::default()
        };
        assert_eq!(walk_failed(&st, &straight, false), Why::OffGraph);
        assert_eq!(walk_failed(&st, &straight, true), Why::Driving);
    }
}
