//! Maintainer-selected Instant Action defaults (2026-10-29; issue #86 retune 2026-11-03).
//! Combatants include humans. Mission files retain the original disc values.
//! Tickets: Normal + Good proxy, seeds 1–3 on both sides; 6–9 minute medians.
//!
//! Issue #171 side compensation (OURS, a measured last resort): on most maps the
//! side that holds the post majority early bleeds the other for the whole round
//! (bot kills stay near even), so the map side, not the players, decided it. The
//! entries marked `OURS #171` move tickets from the favoured side to the other,
//! keeping the pair's total, toward team 0's bot-only Hard win share of 50 %
//! (band 40-60 %). Where the cause is in the map (one side reaches the deciding
//! post first, or respawns far from the front), `layout::CONQUEST_PARITY` fixes
//! the map first and the pair here is refitted from the authored one. No side
//! starts with more than 60 % of the pair (about 1.5 : 1; maintainer decision
//! 2026-11-19, held by a test), so a few maps stay lopsided at the cap; every
//! death costs exactly one ticket on every map. Each names the cause found in
//! the sweep's `sides` census. The p3 pairs are fitted by the margin shift (a
//! one-ticket move changes the end margin by about 1.1 tickets) on 96 seeds and
//! checked on 96 fresh ones. Entries whose note cites seeds 3001+ or 4001+
//! missed on an earlier fresh block and were refitted, so that block is the
//! one they passed on, not an independent confirmation. measure: the bot-only Hard side sweep
//! (`round_to_outcome` with `BOT_CENSUS=1`, every Conquest entry: 32 seeds to
//! screen, 96 fresh ones to confirm) after any sim, AI or map change; replace:
//! fix the cause and return the entry to its authored pair.
pub use ui::population::Defaults;

/// Map/era pairs preserve the mission's side order. Both sides field the same
/// number of combatants on every map (issue #171: the disc's uneven BF1
/// Geonosis, Kashyyyk and Naboo Theed populations decided those rounds for the
/// bigger side; the totals are kept). BF1 Kashyyyk Docks fielded 24 to the
/// deck side's 32 in the first p3 fit; with the native Wookiees and PR #183's
/// bot changes that overshot (team 0 6 %), so the p3 refit put it back on
/// 32/32 and its tickets even the rest.
const fn entry(
    map: &'static str,
    cw: bool,
    combatants: [u32; 2],
    tickets: [u32; 2],
) -> (&'static str, bool, Defaults) {
    (
        map,
        cw,
        Defaults {
            combatants,
            tickets,
        },
    )
}
pub const TABLE: &[(&str, bool, Defaults)] = &[
    // OURS: 13.12 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [107, 107] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 23/32 at the previous [86, 128] (seeds 1-32),
    //   48/96 here on fresh seeds 1001-1096 (50 %, 95 % 40-60).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [98, 116].
    //   Bot-only Hard: team 0 won 10/32 at [98, 116] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (95 rounds, predicted 53 % here); 20/31 on fresh seeds
    //   36001-36032 (65 %, 95 % 47-79).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_bes1", false, [20, 20], [108, 106]),
    // OURS: 13.03 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [95, 95] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 19/32 at the previous [76, 114] (seeds 1-32),
    //   50/96 here on fresh seeds 3001-3096 (52 %, 95 % 42-62).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    entry("bf1_bes1", true, [20, 20], [82, 108]),
    // OURS #171: side compensation, was [124, 124] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 24/32 at the previous [124, 124] (seeds 1-32),
    //   44/96 here on fresh seeds 1001-1096 (46 %, 95 % 36-56).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [115, 133].
    //   Bot-only Hard: team 0 won 16/32 at [115, 133] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 49 % here); 14/32 on fresh seeds
    //   36001-36032 (44 %, 95 % 28-61).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_bes2", false, [16, 16], [120, 128]),
    // OURS #171: side compensation, was [142, 142] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 14/32 at the previous [162, 122] (seeds 1-32),
    //   46/96 here on fresh seeds 3001-3096 (48 %, 95 % 38-58).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [164, 120].
    //   Bot-only Hard: team 0 won 14/32 at [164, 120] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (95 rounds, predicted 55 % here); 20/31 on fresh seeds
    //   36001-36032 (65 %, 95 % 47-79).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_bes2", true, [16, 16], [166, 118]),
    // OURS #171: side compensation, was [258, 258] (team 0 starts on three posts, team 1 on two).
    //   Bot-only Hard on this batch's tree: team 0 won 20/32 at the previous [213, 303] (seeds 1-32),
    //   48/96 here on fresh seeds 1001-1096 (50 %, 95 % 40-60).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [210, 306];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 30/32 at [210, 306] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 47 % here); 14/32 on fresh seeds
    //   36001-36032 (44 %, 95 % 28-61).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_end1", false, [24, 24], [221, 295]),
    // OURS #171: side compensation, was [237, 237] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 8/32 at the previous [284, 190] (seeds 1-32),
    //   39/95 here on fresh seeds 1001-1096 (41 %, 95 % 32-51).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [261, 213].
    //   Bot-only Hard: team 0 won 3/32 at [261, 213] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 9/32 on fresh seeds
    //   36001-36032 (28 %, 95 % 16-45). At the 60 % cap.
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_geo1", true, [24, 24], [284, 190]),
    // OURS #171: side compensation, was [232, 232] (the Empire starts on one post of five).
    //   Bot-only Hard, team 0 won 0/16 at the authored pair (seeds 1-16), 7/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [272, 192];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 9/32 at [272, 192] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (63 rounds, predicted 57 % here); 20/32 on fresh seeds
    //   36001-36032 (62 %, 95 % 45-77).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_hot1", false, [32, 32], [270, 194]),
    // OURS: pace after the blast change. measure: repeat both proxy sides after sim changes.
    // OURS #171: side compensation, was [164, 181] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 20/32 at the previous [139, 206] (seeds 1-32),
    //   41/96 here on fresh seeds 3001-3096 (43 %, 95 % 33-53).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [197, 148].
    //   Bot-only Hard: team 0 won 13/32 at [197, 148] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 55 % here); 21/32 on fresh seeds
    //   36001-36032 (66 %, 95 % 48-80). At the 60 % cap.
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_kam1", true, [16, 16], [207, 138]),
    // OURS: 10.15 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [169, 169] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 4/16 at the authored pair (seeds 1-16), 7/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [184, 154];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 0/32 at [184, 154] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 48 % here); 13/32 on fresh seeds
    //   36001-36032 (41 %, 95 % 26-58).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_kas1", false, [24, 24], [196, 142]),
    // OURS #171: side compensation, was [230, 230] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 17/32 at the previous [230, 230] (seeds 1-32),
    //   50/96 here on fresh seeds 1001-1096 (52 %, 95 % 42-62).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [223, 237];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 0/32 at [223, 237] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 45 % here); 12/32 on fresh seeds
    //   36001-36032 (38 %, 95 % 23-55).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_kas1", true, [24, 24], [253, 207]),
    // OURS #171: side compensation, was [250, 250] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 28/32 at the previous [200, 300] (seeds 1-32),
    //   54/96 here on fresh seeds 3001-3096 (56 %, 95 % 46-66).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [247, 253];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 2/32 at [247, 253] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 41 % here); 11/32 on fresh seeds
    //   36001-36032 (34 %, 95 % 20-52).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_kas2", false, [32, 32], [233, 267]),
    // OURS #171: side compensation, was [250, 250] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 29/32 at the previous [200, 300] (seeds 1-32),
    //   56/96 here on fresh seeds 1001-1096 (58 %, 95 % 48-68).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [218, 282];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 1/32 at [218, 282] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 52 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_kas2", true, [32, 32], [229, 271]),
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [283, 283].
    //   Bot-only Hard: team 0 won 4/32 at [283, 283] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 47 % here); 13/32 on fresh seeds
    //   36001-36032 (41 %, 95 % 26-58).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_nab1", false, [32, 32], [310, 256]),
    // OURS #171: side compensation, was [281, 281] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 11/16 at the authored pair (seeds 1-16), 12/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [266, 296].
    //   Bot-only Hard: team 0 won 8/32 at [266, 296] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 50 % here); 16/32 on fresh seeds
    //   36001-36032 (50 %, 95 % 34-66).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_nab1", true, [32, 32], [277, 285]),
    // OURS: 9.05 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [168, 168] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 4/16 at the authored pair (seeds 1-16), 9/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [178, 158].
    //   Bot-only Hard: team 0 won 12/32 at [178, 158] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 48 % here); 14/32 on fresh seeds
    //   36001-36032 (44 %, 95 % 28-61).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_nab2", false, [20, 20], [189, 147]),
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [200, 200].
    //   Bot-only Hard: team 0 won 11/32 at [200, 200] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 51 % here); 16/32 on fresh seeds
    //   36001-36032 (50 %, 95 % 34-66).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_nab2", true, [20, 20], [215, 185]),
    // OURS #171: side compensation, was [213, 213] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 14/32 at the previous [255, 171] (seeds 1-32),
    //   55/96 here on fresh seeds 1001-1096 (57 %, 95 % 47-67).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [180, 246].
    //   Bot-only Hard: team 0 won 20/32 at [180, 246] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 54 % here); 20/32 on fresh seeds
    //   36001-36032 (62 %, 95 % 45-77).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_rhn1", false, [20, 20], [171, 255]),
    // OURS: 9.04 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [184, 184] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 10/32 at the previous [220, 148] (seeds 1-32),
    //   55/96 here on fresh seeds 1001-1096 (57 %, 95 % 47-67).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [184, 184].
    //   Bot-only Hard: team 0 won 20/32 at [184, 184] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 50 % here); 16/32 on fresh seeds
    //   36001-36032 (50 %, 95 % 34-66).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_rhn1", true, [20, 20], [158, 210]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 12/16 at the authored pair (seeds 1-16), 8/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [165, 235].
    //   Bot-only Hard: team 0 won 21/32 at [165, 235] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 55 % here); 16/32 on fresh seeds
    //   36001-36032 (50 %, 95 % 34-66). At the 60 % cap.
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_rhn2", false, [16, 16], [160, 240]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 20/32 at the previous [185, 215] (seeds 1-32),
    //   41/96 here on fresh seeds 1001-1096 (43 %, 95 % 33-53).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [163, 237].
    //   Bot-only Hard: team 0 won 11/32 at [163, 237] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 19/32 on fresh seeds
    //   36001-36032 (59 %, 95 % 42-74).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_rhn2", true, [16, 16], [185, 215]),
    // OURS #171: side compensation, was [128, 128] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 23/32 at the previous [128, 128] (seeds 1-32),
    //   43/96 at [112, 144] on fresh seeds 1001-1096 (45 %, 95 % 35-55), before the Jawas fought.
    // OURS (p3, the Jawa change): refitted with the Jawas fighting both armies and
    //   the Jawa ally-path fix in `layout::CONQUEST_PARITY`. Bot-only Hard, natives
    //   on: team 0 won 8/24 at [112, 144] without the fix; with it 12/32 at
    //   [118, 138] (seeds 701-732) and here 17/32, 20/32 and 16/32 (seeds 501-532,
    //   601-632, 801-832; 53/96, 55 %, 95 % 45-65).
    //   measure: bot-only Hard side sweep, natives on; replace: fix the cause.
    entry("bf1_tat2", false, [16, 16], [120, 136]),
    // OURS #171: side compensation, was [135, 135] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 18/32 at the previous [135, 135] (seeds 1-32),
    //   52/96 at [140, 130] on fresh seeds 3001-3096 (54 %, 95 % 44-64), before the Jawas fought.
    // OURS (p3, the Jawa change): refitted with the Jawas fighting both armies and
    //   the Jawa ally-path fix in `layout::CONQUEST_PARITY`. Bot-only Hard, natives
    //   on: team 0 won 0/24 at [140, 130] without the fix, 13/32 at [146, 124]
    //   (seeds 501-532) and 19/32 at [152, 118] (seeds 601-632) with it, and 16/32
    //   here on fresh seeds 701-732 (50 %, 95 % 34-66).
    //   measure: bot-only Hard side sweep, natives on; replace: fix the cause.
    entry("bf1_tat2", true, [16, 16], [149, 121]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 1/16 at the authored pair (seeds 1-16), 8/16 here (seeds 49-64).
    //   At the 60 % cap.
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [240, 160].
    //   Bot-only Hard: team 0 won 18/32 at [240, 160] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 45 % here); 12/32 on fresh seeds
    //   36001-36032 (38 %, 95 % 23-55).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_yav1", false, [20, 20], [239, 161]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 14/32 at the previous [220, 180] (seeds 1-32),
    //   45/96 here on fresh seeds 1001-1096 (47 %, 95 % 37-57).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [228, 172].
    //   Bot-only Hard: team 0 won 11/32 at [228, 172] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 48 % here); 15/32 on fresh seeds
    //   36001-36032 (47 %, 95 % 31-64).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_yav1", true, [20, 20], [234, 166]),
    // OURS #171: side compensation, was [200, 200] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 17/32 at the previous [235, 165] (seeds 1-32),
    //   42/96 here on fresh seeds 1001-1096 (44 %, 95 % 34-54).
    //   At the 60 % cap.
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [240, 160];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 8/32 at [240, 160] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 55 % here); 18/32 on fresh seeds
    //   36001-36032 (56 %, 95 % 39-72).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_yav2", false, [16, 16], [239, 161]),
    // OURS #171: side compensation, was [222, 222] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 3/16 at the authored pair (seeds 1-16), 3/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [257, 187].
    //   Bot-only Hard: team 0 won 14/32 at [257, 187] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 51 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf1_yav2", true, [16, 16], [261, 183]),
    // OURS #171: side compensation, was [207, 207] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 12/16 at the authored pair (seeds 1-16), 5/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [197, 217].
    //   Bot-only Hard: team 0 won 11/32 at [197, 217] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 51 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_cor1", false, [16, 16], [210, 204]),
    // OURS #171: side compensation, was [170, 170] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 12/32 at the previous [155, 185] (seeds 1-32),
    //   57/96 here on fresh seeds 1001-1096 (59 %, 95 % 49-69).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [166, 174].
    //   Bot-only Hard: team 0 won 13/32 at [166, 174] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 51 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_cor1", true, [16, 16], [167, 173]),
    // OURS #171: side compensation, was [185, 185] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 11/16 at the authored pair (seeds 1-16), 5/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [170, 200].
    //   Bot-only Hard: team 0 won 14/32 at [170, 200] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 53 % here); 19/32 on fresh seeds
    //   36001-36032 (59 %, 95 % 42-74).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_dag1", false, [16, 16], [172, 198]),
    // OURS #171: side compensation, was [201, 201] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 13/32 at the previous [206, 196] (seeds 1-32),
    //   50/96 here on fresh seeds 1001-1096 (52 %, 95 % 42-62).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [211, 191].
    //   Bot-only Hard: team 0 won 16/32 at [211, 191] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 51 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_dag1", true, [16, 16], [212, 190]),
    // OURS #171: side compensation, was [251, 251] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 3/32 at the previous [301, 201] (seeds 1-32),
    //   57/96 here on fresh seeds 3001-3096 (59 %, 95 % 49-69).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [262, 240].
    //   Bot-only Hard: team 0 won 18/32 at [262, 240] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 18/32 on fresh seeds
    //   36001-36032 (56 %, 95 % 39-72).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_dea1", false, [16, 16], [242, 260]),
    // OURS #171: side compensation, was [259, 259] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 11/32 at the previous [310, 208] (seeds 1-32),
    //   48/96 here on fresh seeds 1001-1096 (50 %, 95 % 40-60).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [246, 272].
    //   Bot-only Hard: team 0 won 20/32 at [246, 272] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 45 % here); 11/32 on fresh seeds
    //   36001-36032 (34 %, 95 % 20-52).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_dea1", true, [16, 16], [233, 285]),
    // OURS #171: side compensation, was [171, 171] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 22/32 at the previous [171, 171] (seeds 1-32),
    //   40/96 here on fresh seeds 1001-1096 (42 %, 95 % 32-52).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [162, 180].
    //   Bot-only Hard: team 0 won 10/32 at [162, 180] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 57 % here); 23/32 on fresh seeds
    //   36001-36032 (72 %, 95 % 55-84).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_end1", false, [24, 24], [176, 166]),
    // OURS #171: side compensation, was [245, 245] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 19/32 at the previous [245, 245] (seeds 1-32),
    //   52/96 here on fresh seeds 1001-1096 (54 %, 95 % 44-64).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [234, 256].
    //   Bot-only Hard: team 0 won 19/32 at [234, 256] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 51 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_fel1", false, [16, 16], [216, 274]),
    // OURS #171: side compensation, was [217, 217] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 11/16 at the authored pair (seeds 1-16), 13/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [197, 237].
    //   Bot-only Hard: team 0 won 17/32 at [197, 237] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 18/32 on fresh seeds
    //   36001-36032 (56 %, 95 % 39-72).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_fel1", true, [16, 16], [192, 242]),
    // OURS #171: side compensation, was [250, 250] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 25/32 at the previous [250, 250] (seeds 1-32),
    //   44/96 here on fresh seeds 1001-1096 (46 %, 95 % 36-56).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [224, 276].
    //   Bot-only Hard: team 0 won 12/32 at [224, 276] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 44 % here); 10/32 on fresh seeds
    //   36001-36032 (31 %, 95 % 18-49).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_geo1", true, [28, 28], [232, 268]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 17/32 at the previous [145, 155] (seeds 1-32),
    //   53/96 here on fresh seeds 4001-4096 (55 %, 95 % 45-65).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [144, 156].
    //   Bot-only Hard: team 0 won 26/32 at [144, 156] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (94 rounds, predicted 48 % here); 14/32 on fresh seeds
    //   36001-36032 (44 %, 95 % 28-61).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_hot1", false, [32, 32], [137, 163]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 16/16 at the authored pair (seeds 1-16), 8/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [125, 175].
    //   Bot-only Hard: team 0 won 7/32 at [125, 175] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 18/32 on fresh seeds
    //   36001-36032 (56 %, 95 % 39-72).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_kam1", false, [24, 24], [137, 163]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 15/16 at the authored pair (seeds 1-16), 9/16 here (seeds 49-64).
    //   At the 60 % cap; team 0 still wins most rounds (72 % team 0, seeds 33-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [120, 180].
    //   Bot-only Hard: team 0 won 7/32 at [120, 180] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 58 % here); 24/32 on fresh seeds
    //   36001-36032 (75 %, 95 % 58-87).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_kam1", true, [24, 24], [142, 158]),
    // OURS #171: side compensation, was [198, 198] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 1/32 at the previous [198, 198] (seeds 1-32),
    //   50/96 here on fresh seeds 1001-1096 (52 %, 95 % 42-62).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [229, 167].
    //   Bot-only Hard: team 0 won 28/32 at [229, 167] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 48 % here); 15/32 on fresh seeds
    //   36001-36032 (47 %, 95 % 31-64).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_kas2", false, [24, 24], [203, 193]),
    // OURS #171: side compensation, was [170, 170] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 9/32 at the previous [160, 180] (seeds 1-32),
    //   49/96 here on fresh seeds 3001-3096 (51 %, 95 % 41-61); the pair was kept at [160, 180].
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [160, 180].
    //   Bot-only Hard: team 0 won 32/32 at [160, 180] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_kas2", true, [24, 24], [149, 191]),
    // OURS #171: side compensation, was [184, 184] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 16/32 at the previous [209, 159] (seeds 1-32),
    //   55/96 here on fresh seeds 1001-1096 (57 %, 95 % 47-67).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [216, 152].
    //   Bot-only Hard: team 0 won 17/32 at [216, 152] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 50 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_mus1", false, [16, 16], [214, 154]),
    // OURS #171: side compensation, was [227, 227] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 18/32 at the previous [257, 197] (seeds 1-32),
    //   46/96 here on fresh seeds 1001-1096 (48 %, 95 % 38-58).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [264, 190].
    //   Bot-only Hard: team 0 won 16/32 at [264, 190] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 53 % here); 19/32 on fresh seeds
    //   36001-36032 (59 %, 95 % 42-74).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_mus1", true, [16, 16], [270, 184]),
    // OURS #171: side compensation, was [231, 231] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 15/32 at the previous [276, 186] (seeds 1-32),
    //   54/96 here on fresh seeds 1001-1096 (56 %, 95 % 46-66).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [248, 214].
    //   Bot-only Hard: team 0 won 21/32 at [248, 214] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 47 % here); 13/32 on fresh seeds
    //   36001-36032 (41 %, 95 % 26-58).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_myg1", false, [20, 20], [245, 217]),
    // OURS #171: side compensation, was [246, 246] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 12/32 at the previous [295, 197] (seeds 1-32),
    //   43/96 here on fresh seeds 1001-1096 (45 %, 95 % 35-55).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [226, 266].
    //   Bot-only Hard: team 0 won 13/32 at [226, 266] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 46 % here); 12/32 on fresh seeds
    //   36001-36032 (38 %, 95 % 23-55).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_myg1", true, [20, 20], [236, 256]),
    // OURS #171: side compensation, was [203, 203] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 23/32 at the previous [203, 203] (seeds 1-32),
    //   41/96 here on fresh seeds 1001-1096 (43 %, 95 % 33-53).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [198, 208].
    //   Bot-only Hard: team 0 won 6/32 at [198, 208] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 54 % here); 20/32 on fresh seeds
    //   36001-36032 (62 %, 95 % 45-77).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_nab2", false, [20, 20], [226, 180]),
    // OURS #171: side compensation, was [217, 217] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 8/32 at the previous [227, 207] (seeds 1-32),
    //   47/96 here on fresh seeds 1001-1096 (49 %, 95 % 39-59).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [243, 191].
    //   Bot-only Hard: team 0 won 7/32 at [243, 191] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 45 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69). At the 60 % cap.
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_nab2", true, [20, 20], [260, 174]),
    // OURS: pace after the blast change. measure: repeat both proxy sides after sim changes.
    // OURS #171: side compensation, was [187, 187] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 20/32 at the previous [187, 187] (seeds 1-32),
    //   54/96 here on fresh seeds 1001-1096 (56 %, 95 % 46-66).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [154, 220];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 22/32 at [154, 220] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 48 % here); 13/32 on fresh seeds
    //   36001-36032 (41 %, 95 % 26-58). At the 60 % cap.
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_pol1", false, [16, 16], [150, 224]),
    // OURS #171: side compensation, was [150, 150] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 17/32 at the previous [125, 175] (seeds 1-32),
    //   52/96 here on fresh seeds 1001-1096 (54 %, 95 % 44-64).
    //   At the 60 % cap.
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [120, 180];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 22/32 at [120, 180] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 45 % here); 12/32 on fresh seeds
    //   36001-36032 (38 %, 95 % 23-55).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_pol1", true, [16, 16], [121, 179]),
    // OURS #171: side compensation, was [223, 223] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 2/16 at the authored pair (seeds 1-16), 3/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [243, 203].
    //   Bot-only Hard: team 0 won 22/32 at [243, 203] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (95 rounds, predicted 46 % here); 11/31 on fresh seeds
    //   36001-36032 (35 %, 95 % 21-53).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_tan1", false, [16, 16], [226, 220]),
    // OURS #171: side compensation, was [227, 227] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 12/32 at the previous [242, 212] (seeds 1-32),
    //   54/96 here on fresh seeds 1001-1096 (56 %, 95 % 46-66).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [251, 203].
    //   Bot-only Hard: team 0 won 19/32 at [251, 203] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 44 % here); 10/32 on fresh seeds
    //   36001-36032 (31 %, 95 % 18-49).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_tan1", true, [16, 16], [235, 219]),
    // OURS #171: side compensation, was [226, 226] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 18/32 at the previous [206, 246] (seeds 1-32),
    //   56/96 here on fresh seeds 1001-1096 (58 %, 95 % 48-68).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [187, 265].
    //   Bot-only Hard: team 0 won 4/32 at [187, 265] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 46 % here); 13/32 on fresh seeds
    //   36001-36032 (41 %, 95 % 26-58).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_tat3", false, [16, 16], [242, 210]),
    // OURS #171: side compensation, was [275, 275] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 14/16 at the authored pair (seeds 1-16), 6/16 here (seeds 49-64).
    //   At the 60 % cap.
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [220, 330].
    //   Bot-only Hard: team 0 won 1/32 at [220, 330] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 50 % here); 17/32 on fresh seeds
    //   36001-36032 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_tat3", true, [16, 16], [283, 267]),
    // OURS #171: side compensation, was [150, 150] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 7/32 at the previous [180, 120] (seeds 1-32),
    //   43/96 here on fresh seeds 1001-1096 (45 %, 95 % 35-55).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [144, 156].
    //   Bot-only Hard: team 0 won 23/32 at [144, 156] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 44 % here); 11/32 on fresh seeds
    //   36001-36032 (34 %, 95 % 20-52).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_uta1", false, [16, 16], [128, 172]),
    // OURS #171: side compensation, was [166, 166] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 6/32 at the previous [199, 133] (seeds 1-32),
    //   48/96 here on fresh seeds 3001-3096 (50 %, 95 % 40-60).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [166, 166].
    //   Bot-only Hard: team 0 won 12/32 at [166, 166] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 52 % here); 19/32 on fresh seeds
    //   36001-36032 (59 %, 95 % 42-74).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_uta1", true, [16, 16], [170, 162]),
    // OURS #171: side compensation, was [182, 182] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 12/16 at the authored pair (seeds 1-16), 7/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [157, 207];
    //   now with a map fix in `layout::CONQUEST_PARITY`.
    //   Bot-only Hard: team 0 won 28/32 at [157, 207] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (64 rounds, predicted 56 % here); 20/32 on fresh seeds
    //   36001-36032 (62 %, 95 % 45-77).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_yav1", false, [20, 20], [147, 217]),
    // OURS #171: side compensation, was [189, 189] (post layout and routes; no single cause found).
    //   Bot-only Hard on this batch's tree: team 0 won 18/32 at the previous [174, 204] (seeds 1-32),
    //   48/96 here on fresh seeds 3001-3096 (50 %, 95 % 40-60).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [188, 190].
    //   Bot-only Hard: team 0 won 24/32 at [188, 190] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (96 rounds, predicted 45 % here); 11/32 on fresh seeds
    //   36001-36032 (34 %, 95 % 20-52).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("bf2_yav1", true, [20, 20], [158, 220]),
    // OURS: 9.96 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [138, 138] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 5/16 at the authored pair (seeds 1-16), 4/16 here (seeds 49-64).
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [148, 128].
    //   Bot-only Hard: team 0 won 31/32 at [148, 128] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (95 rounds, predicted 55 % here); 20/31 on fresh seeds
    //   36001-36032 (65 %, 95 % 47-79).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("tat1", false, [24, 24], [125, 151]),
    // OURS #171: side compensation, was [208, 208] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 3/16 at the authored pair (seeds 1-16), 8/16 here (seeds 49-64).
    //   At the 60 % cap.
    //   measure: bot-only Hard side sweep, 16 seeds a side; replace: fix the cause.
    // OURS #171 (p3 refit, natives on, PR #183's bot changes kept): was [249, 167].
    //   Bot-only Hard: team 0 won 30/31 at [249, 167] (seeds 31001-31032); margin-shift fit on
    //   every round at this setup (95 rounds, predicted 53 % here); 19/32 on fresh seeds
    //   36001-36032 (59 %, 95 % 42-74).
    //   measure: bot-only Hard side sweep with natives; replace: fix the cause.
    entry("tat1", true, [24, 24], [216, 200]),
    // OURS #171: side compensation, was [150, 150] (the map fix in `layout::CONQUEST_PARITY`).
    //   Bot-only Hard on this batch's tree: team 0 won 19/32 at the previous [120, 180] (seeds 1-32),
    //   55/96 here on fresh seeds 3001-3096 (57 %, 95 % 47-67).
    //   measure: bot-only Hard side sweep, 96 fresh seeds an entry; replace: fix the cause.
    entry("tat2", false, [16, 16], [140, 160]),
    // OURS: 5.96 min before; measure: repeat seeds 1–3, both Good-proxy sides after sim changes.
    // OURS #171: side compensation, was [182, 182] (post layout and routes; no single cause found).
    //   Bot-only Hard, team 0 won 14/16 at the authored pair (seeds 1-16), 9/16 at [147, 217]
    //   (seeds 49-64), before the Jawas fought.
    // OURS (p3, the Jawa change): refitted with the Jawas fighting both armies and
    //   the CW spawn weight in `layout::CONQUEST_PARITY`. Bot-only Hard, natives on:
    //   team 0 won 17/24 at [147, 217] without it, 33/64 at [182, 182] with it
    //   (seeds 401-432, 501-532), 17/32 here on fresh seeds 601-632 (53 %, 95 % 36-69).
    //   measure: bot-only Hard side sweep, natives on; replace: fix the cause.
    entry("tat2", true, [16, 16], [176, 188]),
];

pub fn for_map(map: &str, cw: bool) -> Option<Defaults> {
    TABLE
        .iter()
        .find(|(m, era, _)| *m == map && *era == cw)
        .or_else(|| TABLE.iter().find(|(m, _, _)| *m == map))
        .map(|(_, _, d)| *d)
}

/// Issue #171 (OURS, p3 refit with natives): the human-side ticket shift per
/// (map, era, tier). [`TABLE`] holds each map's bots-only Hard pair, fitted
/// for even sides with no human. A human then wins at very different rates
/// map to map and tier to tier, so each entry here moves tickets between the
/// sides when the humans are on one of them: `[tier][human side]` tickets
/// given TO the humans' side from the bots' side (negative: taken from it),
/// at 100 % reinforcements. Tiers in [`core::skill::Difficulty::ALL`] order;
/// a missing entry or 0 leaves the pair as it is. The result stays inside
/// the 60 % cap (maintainer 2026-11-19); kills, health and team sizes are
/// never touched, and every death still costs one ticket.
/// measure: the stand-in sweep (`round_to_outcome` with `SWEEP_PROXY=<side>:good`,
/// 32 seeds a side at each tier) toward Hard 55-70 % and Elite 40-55 % for
/// the stand-in's side; replace: fix the map or AI cause and set it to 0.
pub const HUMAN_SHIFT: &[(&str, bool, [[i32; 2]; 4])] = &[
    ("bf1_bes1", false, [[0, 0], [0, 0], [6, 5], [6, -14]]),
    ("bf1_bes1", true, [[0, 0], [0, 0], [16, -11], [18, -27]]),
    ("bf1_bes2", false, [[0, 0], [0, 0], [9, -1], [22, -22]]),
    ("bf1_bes2", true, [[0, 0], [0, 0], [-11, 3], [4, -2]]),
    ("bf1_end1", false, [[0, 0], [0, 0], [8, 8], [-4, 12]]),
    ("bf1_geo1", true, [[0, 0], [0, 0], [0, 1], [0, 0]]),
    ("bf1_hot1", false, [[0, 0], [0, 0], [8, 22], [7, 22]]),
    ("bf1_kam1", true, [[0, 0], [0, 0], [-10, 0], [0, 0]]),
    ("bf1_kas1", false, [[0, 0], [0, 0], [6, -2], [0, -6]]),
    ("bf1_kas1", true, [[0, 0], [0, 0], [3, 3], [20, -23]]),
    ("bf1_kas2", false, [[0, 0], [0, 0], [5, 15], [24, -5]]),
    ("bf1_kas2", true, [[0, 0], [0, 0], [8, 8], [9, 9]]),
    ("bf1_nab1", false, [[0, 0], [0, 0], [22, 43], [26, 0]]),
    ("bf1_nab1", true, [[0, 0], [0, 0], [29, 29], [46, -11]]),
    ("bf1_nab2", false, [[0, 0], [0, 0], [-6, -6], [-26, 34]]),
    ("bf1_nab2", true, [[0, 0], [0, 0], [-13, 5], [-38, 38]]),
    ("bf1_rhn1", false, [[0, 0], [0, 0], [11, 0], [55, -43]]),
    ("bf1_rhn1", true, [[0, 0], [0, 0], [36, -14], [36, -30]]),
    ("bf1_rhn2", true, [[0, 0], [0, 0], [-10, 20], [-4, 0]]),
    ("bf1_yav1", false, [[0, 0], [0, 0], [1, 1], [0, 3]]),
    ("bf1_yav1", true, [[0, 0], [0, 0], [5, -5], [0, 42]]),
    ("bf1_yav2", false, [[0, 0], [0, 0], [-32, 40], [-34, 23]]),
    ("bf1_yav2", true, [[0, 0], [0, 0], [0, 59], [-24, 17]]),
    ("bf2_cor1", false, [[0, 0], [0, 0], [-25, -16], [-44, 14]]),
    ("bf2_cor1", true, [[0, 0], [0, 0], [-14, -14], [-30, 14]]),
    ("bf2_dag1", false, [[0, 0], [0, 0], [-5, -20], [-2, -27]]),
    ("bf2_dag1", true, [[0, 0], [0, 0], [-19, -19], [-5, -18]]),
    ("bf2_dea1", false, [[0, 0], [0, 0], [-13, -13], [-41, -41]]),
    ("bf2_dea1", true, [[0, 0], [0, 0], [-12, -12], [16, -47]]),
    ("bf2_end1", false, [[0, 0], [0, 0], [-10, 0], [-23, 18]]),
    ("bf2_fel1", false, [[0, 0], [0, 0], [-10, -30], [-19, -19]]),
    ("bf2_fel1", true, [[0, 0], [0, 0], [-8, -8], [-16, 0]]),
    ("bf2_geo1", true, [[0, 0], [0, 0], [31, -41], [9, 9]]),
    ("bf2_hot1", false, [[0, 0], [0, 0], [6, 6], [-8, 11]]),
    ("bf2_kam1", false, [[0, 0], [0, 0], [4, 4], [0, 0]]),
    ("bf2_kam1", true, [[0, 0], [0, 0], [0, 13], [0, 0]]),
    ("bf2_kas2", false, [[0, 0], [0, 0], [-5, -5], [29, -33]]),
    ("bf2_kas2", true, [[0, 0], [0, 0], [-1, -1], [-4, -4]]),
    ("bf2_mus1", false, [[0, 0], [0, 0], [-17, -4], [-5, -6]]),
    ("bf2_mus1", true, [[0, 0], [0, 0], [-24, 0], [-5, 0]]),
    ("bf2_myg1", false, [[0, 0], [0, 0], [-2, -2], [13, -22]]),
    ("bf2_myg1", true, [[0, 0], [0, 0], [5, 5], [16, -38]]),
    ("bf2_nab2", false, [[0, 0], [0, 0], [-10, -10], [-20, -1]]),
    ("bf2_nab2", true, [[0, 0], [0, 0], [-6, 0], [-6, 0]]),
    ("bf2_pol1", false, [[0, 0], [0, 0], [0, -13], [0, -34]]),
    ("bf2_pol1", true, [[0, 0], [0, 0], [0, -17], [0, -25]]),
    ("bf2_tan1", false, [[0, 0], [0, 0], [-17, -17], [-21, -2]]),
    ("bf2_tan1", true, [[0, 0], [0, 0], [-3, -3], [-1, -1]]),
    ("bf2_tat3", false, [[0, 0], [0, 0], [1, 1], [-42, 40]]),
    ("bf2_tat3", true, [[0, 0], [0, 0], [-3, -3], [-21, 15]]),
    ("bf2_uta1", false, [[0, 0], [0, 0], [-7, -24], [10, -30]]),
    ("bf2_uta1", true, [[0, 0], [0, 0], [-14, -14], [4, -24]]),
    ("bf2_yav1", false, [[0, 0], [0, 0], [0, -8], [45, -67]]),
    ("bf2_yav1", true, [[0, 0], [0, 0], [-5, -5], [26, -35]]),
    ("tat1", false, [[0, 0], [0, 0], [1, 7], [-13, 14]]),
    ("tat1", true, [[0, 0], [0, 0], [-1, 15], [-28, 31]]),
];

/// The [`HUMAN_SHIFT`] row for one map/era and tier, `[human on team 0,
/// human on team 1]`.
pub fn human_shift(map: &str, cw: bool, tier: core::skill::Difficulty) -> [i32; 2] {
    let t = core::skill::Difficulty::ALL
        .iter()
        .position(|d| *d == tier)
        .unwrap_or(1);
    HUMAN_SHIFT
        .iter()
        .find(|(m, era, _)| *m == map && *era == cw)
        .map_or([0; 2], |(_, _, rows)| rows[t])
}

/// The round's starting pair at 100 % reinforcements: `base` with the
/// [`HUMAN_SHIFT`] of the one human's side. Only a round with exactly one
/// human is shifted: the table is fitted to one player, and two or more
/// humans (on one side or split) keep `base` until a group is measured. Integer arithmetic, set once before any ticket is
/// lost; every input is in the round setup, so LAN peers agree.
pub fn human_tickets(
    map: &str,
    cw: bool,
    tier: core::skill::Difficulty,
    base: [u32; 2],
    humans: [u8; 2],
) -> [u32; 2] {
    let side = match humans {
        [1, 0] => 0,
        [0, 1] => 1,
        _ => return base,
    };
    let shift = i64::from(human_shift(map, cw, tier)[side]);
    if shift == 0 {
        return base;
    }
    let total = i64::from(base[0]) + i64::from(base[1]);
    let cap = total * 3 / 5;
    let ours = (i64::from(base[side]) + shift).clamp(total - cap, cap);
    let mut out = base;
    out[side] = ours as u32;
    out[1 - side] = (total - ours) as u32;
    out
}

impl crate::walk::WalkSim {
    /// Apply the same percentage to the opening roster and later replenishment.
    /// Grow only missing roster slots; existing dead bots keep their spawn waves.
    pub fn set_team_size_percent(&mut self, pct: u32) {
        let Some(defaults) = self.conquest_defaults else {
            return;
        };
        self.team_size_pct = pct.clamp(10, ui::population::MAX_TEAM_PCT);
        if self.sim.outcome.is_some() {
            return;
        }
        let counts = defaults
            .scaled(self.team_size_pct, self.reinforcement_pct)
            .combatants;
        self.sim.set_combatants(counts);
        let additions = self.roster_gaps();
        self.grow_roster(additions);
    }

    /// Bot slots each team is short of its limit (alive or waiting to respawn).
    fn roster_gaps(&self) -> [u32; 2] {
        [0, 1].map(|team| {
            let roster = self
                .sim
                .units
                .iter()
                .filter(|u| !u.is_player && u.team == team && (u.alive || u.respawn_at.is_some()))
                .count() as u32;
            self.sim.ai_limit(team).saturating_sub(roster)
        })
    }

    /// A human who changes side frees a slot on his old team: fill it with a
    /// bot (only on a side change, so spent reinforcements are never refilled).
    pub(crate) fn refill_after_side_change(&mut self) {
        let humans = [0u8, 1].map(|team| {
            self.sim
                .players
                .iter()
                .flatten()
                .filter(|h| {
                    self.sim
                        .units
                        .get(h.index as usize)
                        .is_some_and(|u| u.gen == h.gen && u.team == team)
                })
                .count() as u32
        });
        let before = self.human_teams.replace(humans);
        let Some(before) = before else { return };
        if self.conquest_defaults.is_none() || self.sim.outcome.is_some() {
            return;
        }
        let gaps = self.roster_gaps();
        let additions = [0, 1].map(|t| gaps[t].min(before[t].saturating_sub(humans[t])));
        if additions != [0, 0] {
            self.grow_roster(additions);
        }
    }
}

/// The Team size and Reinforcements help: averages over every map/era entry
/// at these percentages (no single map: Instant Action plays them all).
pub fn menu_note(team_pct: u32, ticket_pct: u32) -> [String; 2] {
    ui::population::notes(ui::population::summary(TABLE, team_pct, ticket_pct))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_cover_all_maps_and_keep_ratios_and_human_slot() {
        assert_eq!(TABLE.len(), 61);
        for (i, (map, cw, d)) in TABLE.iter().enumerate() {
            assert!(!TABLE[..i].iter().any(|(m, e, _)| m == map && e == cw));
            assert!(d.combatants.iter().all(|n| (16..=32).contains(n)));
            assert_eq!(d.scaled(100, 100), *d);
            for team in 0..2 {
                let ai = d.initial_ai(team);
                assert_eq!(ai[team as usize] + 1, d.combatants[team as usize]);
            }
        }
        let d = for_map("bf1_kas2", false).unwrap();
        assert_eq!(d.combatants, [32, 32]);
        assert_eq!(d.scaled(50, 100).combatants, [16, 16]);
        // Every entry fields even sides (the p3 refit with natives put BF1
        // Kashyyyk Docks back on 32/32).
        for (map, _, d) in TABLE {
            assert!(d.combatants[0] == d.combatants[1], "{map}");
        }
        assert_eq!(for_map("bf1_nab2", false).unwrap().combatants, [20, 20]);
        assert_eq!(for_map("bf1_nab2", true).unwrap().combatants, [20, 20]);
        let at = |t, r| ui::population::summary(TABLE, t, r);
        // 1228 combatant slots over 61 entries (BF1 Kashyyyk Docks on 32/32).
        assert!((at(100, 100).team - 1228.0 / 61.0).abs() < 1e-4);
        // More team size means more units and shorter matches; more
        // reinforcements, more tickets and longer matches; 200% caps every map.
        assert!(at(50, 100).team < at(100, 100).team && at(100, 100).team < at(200, 100).team);
        assert_eq!(at(500, 100), at(200, 100));
        assert!(at(50, 100).minutes > at(100, 100).minutes);
        assert!(at(100, 100).minutes > at(200, 100).minutes);
        assert!(at(100, 50).tickets < at(100, 100).tickets);
        assert!(at(100, 50).minutes < at(100, 100).minutes);
        assert!(at(100, 200).minutes > at(100, 100).minutes);
        let [team, reinf] = menu_note(100, 100);
        assert!(team.starts_with("Average team size: 20 units\nAverage match length: "));
        assert!(reinf.starts_with("Average reinforcements: "));
        assert!(reinf.ends_with(team.lines().nth(1).unwrap()));
        assert_eq!(ui::population::tickets([25, 25], 10), [3, 3]);
    }

    /// Every human-side shift keeps both sides inside the 60 % cap, names a
    /// map/era of the table once, and leaves a round with no human, or with
    /// two or more, on the table's pair.
    #[test]
    fn human_shifts_stay_inside_the_cap() {
        use core::skill::Difficulty;
        for (i, (map, cw, _)) in HUMAN_SHIFT.iter().enumerate() {
            assert!(for_map(map, *cw).is_some(), "{map}");
            assert!(!HUMAN_SHIFT[..i].iter().any(|(m, e, _)| m == map && e == cw));
        }
        for (map, cw, d) in TABLE {
            for tier in Difficulty::ALL {
                assert_eq!(human_tickets(map, *cw, tier, d.tickets, [0, 0]), d.tickets);
                for humans in [[1, 1], [2, 0], [0, 2], [3, 0], [2, 1], [1, 2]] {
                    assert_eq!(
                        human_tickets(map, *cw, tier, d.tickets, humans),
                        d.tickets,
                        "{map} {tier:?} {humans:?}: only a lone human is shifted"
                    );
                }
                for humans in [[1, 0], [0, 1]] {
                    let [a, b] = human_tickets(map, *cw, tier, d.tickets, humans);
                    assert_eq!(a + b, d.tickets[0] + d.tickets[1], "{map}");
                    assert!(5 * a.max(b) <= 3 * (a + b), "{map} {tier:?}: {a}/{b}");
                }
            }
        }
        // A map without a row keeps its pair; a row's shift goes to the lone
        // human's side and stops at the cap.
        assert_eq!(
            human_tickets("none", false, Difficulty::Hard, [100, 100], [1, 0]),
            [100, 100]
        );
        let [e0, e1] = human_shift("bf1_nab2", false, Difficulty::Elite);
        assert_eq!(
            human_tickets("bf1_nab2", false, Difficulty::Elite, [168, 168], [1, 0]),
            [
                (168 + e0).clamp(135, 201) as u32,
                (168 - e0).clamp(135, 201) as u32
            ]
        );
        assert_eq!(
            human_tickets("bf1_nab2", false, Difficulty::Elite, [168, 168], [0, 1]),
            [
                (168 - e1).clamp(135, 201) as u32,
                (168 + e1).clamp(135, 201) as u32
            ]
        );
    }

    /// Uneven starts are allowed for side parity, never more than 60 % of the
    /// pair's total on one side (maintainer decision, issue #171).
    #[test]
    fn no_side_starts_with_more_than_sixty_percent_of_the_tickets() {
        for (map, cw, d) in TABLE {
            let [a, b] = d.tickets;
            assert!(
                5 * a.max(b) <= 3 * (a + b),
                "{map} (cw {cw}): {a}/{b} gives one side more than 60 %"
            );
        }
    }
}
