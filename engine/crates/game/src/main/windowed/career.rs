use game::profile::{BattleStats, Profile};
use game::walk::WalkSim;

pub(super) fn record_round(profile: &mut Profile, walk: &mut WalkSim, mut stats: BattleStats) {
    // Split: a first player whose controller dropped this round has their points from the
    // class gate, as the further players' (`WalkSim::record_local_battles`; issue #194).
    if walk.local_count() > 1 {
        if let Some(points) = walk.carried_round_points(0) {
            stats.points = points;
        }
    }
    walk.record_battle(profile, stats);
    if let Err(e) = profile.save() {
        eprintln!("profile autosave failed: {e:#}");
    } else {
        println!(
            "career: {} rounds, {} wins, {} kills / {} deaths",
            profile.career.rounds, profile.career.wins, profile.career.kills, profile.career.deaths
        );
    }
}

/// Issue #194: the profiles of the seats a split battle's further players play on, by
/// Players-screen seat number (`App.split_profiles`; seat 0 is `App.profile`, never here).
pub(super) type SeatProfiles = [Option<Profile>; 4];

/// Local player `k`'s seat number from a snapshot of the seats (`split::seat_numbers`).
fn seat(seats: &[usize], k: usize) -> usize {
    seats.get(k).copied().unwrap_or(k)
}

fn save_seats(profiles: &SeatProfiles, seats: &[usize]) {
    for &s in seats {
        if let Some(p) = profiles.get(s).and_then(Option::as_ref) {
            if let Err(e) = p.save() {
                eprintln!("profile autosave failed ({}): {e:#}", p.name);
            }
        }
    }
}

/// Each further local player's class kills and medals into their own seat's profile, saved when
/// it changed (the first player's is `WalkSim::flush_career_progress` on `App.profile`).
pub(super) fn flush_split_careers(
    profiles: &mut SeatProfiles,
    walk: &mut WalkSim,
    seats: &[usize],
) {
    let changed = walk.flush_local_careers(profiles, &|k| seat(seats, k));
    save_seats(profiles, &changed);
}

/// Each further local player's round into their own seat's profile, with their own feed's
/// battle stats (`feeds[k - 1]`, `App.split_feeds`), saved: the split half of every
/// [`record_round`] call site.
pub(super) fn record_split_rounds(
    profiles: &mut SeatProfiles,
    walk: &mut WalkSim,
    feeds: &[game::event_feed::EventFeed],
    seats: &[usize],
) {
    let recorded = walk.record_local_battles(profiles, &|k| seat(seats, k), &|k| {
        feeds
            .get(k.wrapping_sub(1))
            .map(|f| f.battle_stats())
            .unwrap_or_default()
    });
    save_seats(profiles, &recorded);
    for s in recorded {
        if let Some(p) = profiles[s].as_ref() {
            println!(
                "career (P{}, {}): {} rounds, {} wins, {} kills / {} deaths",
                s + 1,
                p.name,
                p.career.rounds,
                p.career.wins,
                p.career.kills,
                p.career.deaths
            );
        }
    }
}
