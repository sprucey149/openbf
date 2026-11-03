---
id: WINS2
title: One-sided rounds on two showcase maps, after WINS: BF1 YAV2 ends 2-14 and BF2 MYG1 3-13 over 16 seeds on main with kills near even - take the PS2's own column (bots-only rounds, posts held, first capture, time out of spawn) and sort each map PORT / DESIGN; a PORT map gets its traced cause as a card
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card WINS (2026-10-22, `reports/20261022_0051_card_WINS.md`, table in `cards/WINS.md` ## Result) sorted 14 lopsided maps but left two
SHOWCASE maps UNSORTED for want of a PS2 column: bf1_yav2 2-14 (margin 70, kills 118 + 82, posts held 2.1 / 2.8; 37 Imperial
lives never leave spawn against 19) and bf2_myg1 3-13 (margin 61, kills 85 + 62, posts 1.7 / 2.8). A showcase map that one
side always wins would be seen in the maintainer's first battles; it may also be the mission's design.

## Evidence
- Port: `lead_tmp/fidelity_rebase_20261021b/fidelity*/sweep_s*.txt` (16 seeds a map) and WINS's probe re-run
  (`lead_tmp/wins/probe.patch`: post owners + tickets once a second).
- PS2 sampler: `lead_tmp/wins/ps2/wins_ps2.py` (bots-only rounds; BF1's no-AI patch is not wanted here - BF1 rounds need
  the player parked, memory 'PS2 samples without the player think slowly': note it beside the numbers).

## Notes
- No GPU needed (PCSX2 SW renderer; the sweep binary is CPU only). Filed by the planner 2026-10-22 03:20 from WINS's
  UNSORTED rows.
## Result (2026-10-22, lane 1, main f44cdcf) - DONE, no code
PS2: bots-only conquest rounds on PCSX2 (SW), the player left on the spawn map (never spawned; memory 'PS2 samples
without the player think slowly' - far-from-player bots). BF2 MYG1 GCW (16 AI a team, the IA menu's setting) by
`lead_tmp/wins/ps2/wins_ps2.py`; BF1 YAV2 GCW (`YAV2i`, the mission's own 25 v 25, no unit patch) by the new
`wins_ps2_bf1.py`; columns by `wins_sum.py` (it reproduces WINS's UTA1 / KAM1 PS2 rows: 48-2, 28-0, 34-1). Teams are the
Lua numbers (YAV2 1 = Empire, 2 = Alliance; MYG1 1 = Alliance, 2 = Empire). Port: the fidelity baseline's 16 seeds;
posts / leave-spawn from WINS's 8-seed probe run (`lead_tmp/wins/run1`, `wins_cp.py`, `wins_leave.py`).

| map | side | PS2 rounds: winner, tickets left (start ~185 / ~147) | PS2 posts held Emp / All | port (16 seeds) | port posts held Emp / All | sort |
|---|---|---|---|---|---|---|
| bf1_yav2 | player parked Empire | All 86-0 (485 s), All 74-0 (390 s) | 1.89 / 3.10, 2.16 / 2.70 | All 14 - Emp 2, winner's median 70 of 200 | 2.1 / 2.8 | DESIGN |
| bf1_yav2 | player parked Alliance (control) | All 73-0 (510 s) | 1.87 / 3.11 | | | (same) |
| bf2_myg1 | player parked Alliance | Emp by domination, tickets 2-2 (1280 s); All 14-0 (665 s) | 3.07 / 1.55, 1.66 / 2.19 | Emp 13 - All 3, winner's median 61 of 150 | 2.8 / 1.7 | PORT -> card MYGC |

- **YAV2 = DESIGN.** The PS2's Alliance wins 3 of 3, with either side's slot taken by the parked player, by 73-86 tickets
  (the port's 70 is not over twice the PS2's). The PS2's Empire holds fewer posts (1.9-2.2 v 2.7-3.1, as the port's 2.1 v
  2.8), has fewer alive (mean 16.5 v 21.6 in r1) and is slower out of spawn (median 27-33 s to 50 m v 24-25 s; 9-11 lives
  over 60 s that never did, v 11-18 for the Alliance, which fields more lives) - the same lean as the port's 37 v 19.
  Both start with the Alliance on 3 posts (PS2 owners [1, 2, 2, 1, 2] at the first sample).
- **MYG1 = PORT** (the PS2's margin, 0 and 14, is under half of the port's 61; its winner splits). Kills do not decide the
  port's rounds (Alliance 96.5 v Empire 83.8 a round); posts do: the port's Alliance never reaches the Empire's forward
  cp5_con, the PS2's flips it inside the first minute. Trace, numbers and spot on card MYGC (value 5, alpha).
- The opening wave is not the cause: a scratch prototype of OPNW (posts drawn per opening bot) still ends 3-13
  (`lead_tmp/wins2/ab/`, note on OPNW).
- Time to 50 m from spawn on the PS2 is 2-3x the port's on both maps (YAV2 median 24-33 s v 12 s; MYG1 17.5-20 s v 8-10 s):
  the no-player think rate, not compared further.
