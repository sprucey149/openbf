---
id: RCYC
title: The last-resort recycle: a bot that makes no NET progress for minutes is respawned, and every use is counted and listed as a defect to close (the maintainer, 2026-10-01: it should exist, and never be needed)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-01: "ideally the recycle one is not used, but it should be available in case a bot gets somehow stuck
somewhere (which should never happen because we close all those gaps where it could get stuck)."

Stock conquest has no exit once the losing side is clamped at alive = tickets (re-round-stalls C7): one bot that cannot
move and cannot be reached holds the round open for ever. BNDS (9440823) recycles a bot that is OFF the level after 45 s
(`LostState`, `DeathCause::Lost`). A bot that is on the level but going nowhere is not covered.

## Evidence (`lead_tmp/endings/README.md`, main 4603e9e, 8 seeds x 32 maps x 120,000 ticks)
- BF1 YAV1 seeds 2 and 8 never end (3 v 1, 1 v 1). Seed 8: unit 6 gen 10 slides at (107.4, -62.7, -136.1) for 3,424 s,
  3.3-3.9 m under hub 17 (the wrong floor, STLVL), 34 m from the last enemy, line of sight blocked. Five earlier lives sat
  at the same spot 86-175 s each. `level` reads 0.0, so `LostState` never opens.
- BF2 DEA1: both teams jam at the blast doors (STDOR). STUCK_CENSUS reports 0 units stuck over 5,000 ticks there, because the
  door pushes them back and forth and resets the stuck window. `lead_tmp/endings/walk_in_place.py` (speed > 1 m/s, under
  1 m net in 10 s) reads 15 % of unit-seconds.

## Notes
- STLVL's design had this as its own last resort ("a bot with no route and no progress for 3 min"); it lives here now.
- The PS2 sets a stuck flag (2 m in 3 s, re-round-stalls section 3); what its agents do with it is unread. This rule is
  ours and PROVISIONAL.
- Run scratch binaries under a memory cap (see OFFG's note).

## Note from the AUDIT of STFAL (2026-10-01) - a bot standing at a ledge is one of these
`SteerPick::Ledge` (STFAL: every candidate is a wall or a drop, the unit stands) keeps the stuck window running, and
`brain.rs`'s comment says "so it re-plans instead of standing there for good". Measured, the re-plan hands back the same
leg and the unit stays. Per unit, within 3 m of where it first stood at a ledge for 600 ticks or more (8 seeds x 32
maps): main 58fa0e3, 31 stays - BF2 KAM1 (52, 53, 139), unit 29 for 53,745 and 51,770 ticks in seeds 7 and 8 (of
60,000); FEL1 (140-146, 30-37, 45) 16 stays up to 3,730 ticks. At the landing, 86 stays - three whole-round stands on
KAM1 (-5, 50, 138) and 65 stays at the lip of a real pit on MYG1 (-137, 0, -102), one of 50,997 ticks. Treat a long
stand as a no-net-progress case and list it in the census; fix that comment when the code is touched. Instrument:
`lead_tmp/audit-stfal/tools/add_v2.py` (`AUDIT_FALLS` prints `LEDGESTAY`), README there.

## Result (2026-10-01, lane 2, branch `rcyc`: ff758c3 + 7e20064, ci green, gated, NOT landed - STOP went up at 20:47)
- **Built:** `core/src/ai/stall.rs` (`watch`, beside `LostState`), `AiState::stall` (hashed), `DeathCause::Stalled`; the
  census in `round_to_outcome` (`recycle` lines, `stalled` in the deaths line, a `shares` line per map) and
  `sweep_seeds.py` (`recycled` column, `## Recycles` spot list, the shares table).
- **The rule as built differs from the card's in one way:** "5 m net" from a point misses a bot PACING a wall (BF2 UTA1
  seed 42, 1 v 9 for 30,000 ticks). So once a team is at <= 10 reinforcements, progress is 5 m of NEW ground (outside the
  box of everything covered since the watch opened). All round, that rule was 31 % of Kamino's deaths, so mid-round it
  stays the card's. A recycle costs its ticket (FEEL_NOTES).
- **Acceptance, run (`lead_tmp/rcyc/README.md`):**
  - tests: `cargo test -p core --lib stall` 5 passed (penned bot recycled once at RECYCLE_AFTER, with its ticket,
    respawns at its own post, not again; a bot in a firefight, a holder, a defender on its spot are not); the hash test
    covers all 10 `StallState` fields.
  - 8 seeds x 32 maps x 120,000 ticks vs main ce66fe2's binary: 253 -> **255 / 256**; BF1 YAV1 6 -> **8/8**; no map lower.
  - **NOT met to the letter:** "no round ends NO OUTCOME with the loser at 3 alive or fewer" - BF2 KAM1 seed 3 is cut
    at 2 v 14 (tickets 2 / 33) with every bot moving: a slow round (OFFG), reaching 2 alive at the last tick, not a stall.
    Other variants left a 1 v 2 firefight that kills nobody (STTAP) and a 3 v 3 with all six defending (STEND) open:
    neither is a no-progress bot, and both cards carry the note.
  - **recycles per 8-seed sweep at 120,000 ticks: 496** at 212 spots, 79 on two or more seeds = the number the stall cards
    drive to 0. Owners (heuristic, `lead_tmp/rcyc/owners_d.md`): STARC 41 spots / 186, STLVL 21 / 96, OFFG 17 / 68; each
    card has its table. New card KA137 (Kamino's sea arc).
- **Gates** (`lead_tmp/rcyc/gates2/SUMMARY.md`): ci 1069 / 0 / 42, sim, simhash, parity, arena_flat, leak, map PASS;
  sweep and arena_tat2 MOVED (declared); gpu FAIL = main's new GSKIP check against this branch's pre-GSKIP
  `goldens/deps.toml` (goldens unchanged) - gone after the rebase land.py does.

## Landed (2026-10-01 23:01, lane 1: main 63fdcba = 9548ff7 + 63fdcba after the rebase; report `reports/20261001_2305_land_RCYC.md`)
- Acceptance re-run on the landed tree (`lead_tmp/rcyc/sweep120k_land`, 8 seeds x 32 maps x 120,000 ticks): 255 / 256
  (main 253), BF1 YAV1 8/8, no map lower, 496 recycles at 212 spots (79 on 2+ seeds) - byte-identical to the build
  session's `sweep120k_d`. `cargo test -p core --lib stall`: 5 passed.
- Still open, by the letter: BF2 KAM1 seed 3 cut at 2 v 14 - 0 units stuck or on one leg in the census, a slow round (OFFG).
- Gates: 0 goldens; sweep (seed 42: 30 -> 31 / 32) and arena_tat2 re-baselined (`loop_baselines/20261001_2301_RCYC`).
- The later stall cards measure against `lead_tmp/rcyc/sweep120k_land`.
