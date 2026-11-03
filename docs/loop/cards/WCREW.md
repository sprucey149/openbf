---
id: WCREW
title: A command walker wedged with its crew holds a round open: BF1 Geonosis seed 3 never ends (7 Republic alive, all riders of three walkers that do not move, one inside cp2's capture region so the post stays contested; COMMAND_ENDGAME is 6) - on main a crew wedges at the same spot (50, 124) in seed 281, 16 of 480 rounds have a wedged crew
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Found by HJAM's land job (2026-10-15): the fidelity gate refused HJAM on finished_pct over all maps 100.00 -> 99.61 - BF1
Geonosis seed 3 gives NO OUTCOME in 60000 ticks. All 7 Republic survivors ride three walkers that stopped moving (noprog
31,000-41,000 ticks, probe WALL toi 0): vehicle 15 with rider #4 stands INSIDE cp2's capture region, so the droids
capturing it for 37,000 ticks never take it, nobody bleeds and team 0's 7 tickets never fall. walker_drive.rs (card WAI2):
a command walker's pilot stays where it stuck unless his team is down to COMMAND_ENDGAME (6) alive. HJAM does not touch
walkers (its hover only reshuffles the round), and main has the fault: a crew is wedged at the same spot on main's seed 281.

## Evidence (engine/artifacts/lead_tmp/hjam/)
- geostall.md (the sort, every number); geo3/bf1_geo1_s3.json + geo3/digest.txt (black box, branch, seed 3, tick 58000).
- ab/{main3,fix3}_geo{120,360,960}: BF1 Geonosis seeds 1-1440. Rounds without an outcome main 0 / HJAM 2 (seeds 3, 184);
  rounds with a Republic walker crew (2+ riders at one spot) wedged 6000+ ticks, seeds 1-480: main 16, HJAM 18
  (main seeds 11, 29, 112, 126, 179, 219, 222, 280, 281, 326, 333, 422, 463, 465, 478; spots (158, 44), (-51, 18), (50, 124)).
- Binaries: bin/main3 (main e85c91d), bin/fix3 (HJAM on e85c91d): reproduce with bin/fix3 60000 3 bf1_geo1.

## Outcome (2026-10-16 02:20, lane 1, LANDED 5e25eac)
- Cause traced: the walkers were not wedged. The round was held by the CAPTURE rule: the port counted a seated rider in a
  post's region (vehicle 15's pilot in cp2 kept it contested); the PS2 does not (BF1 RAM, 73 / 73 post rows with a seated
  soldier inside count only those on foot - lead_tmp/wcrew_cpcount.py / .txt; BF2 not sampled [H]).
- Acceptance 1 met: seeds 3 / 184 on hjam + fix end (team 1 at 21,574 / 18,997; hjam alone NO OUTCOME) - lead_tmp/wcrew/hj_s*.txt, hjonly_s*.txt.
- Acceptance 2 re-scoped, not built: rounds without an outcome over Geonosis 1-480: hjam+fix 0, main+fix 0 (hjam 2).
  "Wedged crews" by my end-of-round census (lead_tmp/wcrew_crews.py; the HJAM session's own script was not kept): main
  2493141 7 in 7 rounds, main+fix 5, hjam+fix 6. 5-6 of 7 are command walkers on their route's final hop 2-5 m from
  the hub, 50-56 m from the post: WAI2's designed stand (the PS2's pilot never gets out, re-walker-ai 13), not a
  wedge. Halving them needs a deviation from the PS2 (a command walker pilot moving on from his drop hub) - a planner call.
- Acceptance 3: 24 seeds x 32 maps (lead_tmp/wcrew/{main,fix}_all24): 768 / 768 both, kills +8 a seed (t 0.29),
  recycles 82 -> 90 (paired t 1.45). Test `sim::round::tests::a_rider_in_a_post_neither_counts_nor_contests` (fails with the rule removed).
