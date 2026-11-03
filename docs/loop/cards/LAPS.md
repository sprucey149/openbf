---
id: LAPS
title: The places the wider recycle watch now shows: bots lapping at BF1 Bespin (0, -160) (24 in 32 rounds, 0 seen before), BF1 Geonosis (108, -12), BF2 Geonosis (-84, 264) and Polis Massa's endgame bursts - find each cause from one snapshot and a flood, cure or file it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
From card OBSR (2026-10-07, landed d9db73c). The last resort now gives up a bot that walks two minutes without
getting 15 m from where its count began (`RECYCLE_ROAM`); until then its 5 m net could not see a bot LAPPING a few
metres of ground, and the recycle count hid those places. Over 32 seeds x 32 maps recycles went 206 -> 284 with the
rule (197 on main before the card): the 78 more are lives that used to lap for the rest of a round. Each repeated
cell is a defect to find and close - a recycle is a bot dying alone in front of the player.

## Evidence (`engine/artifacts/lead_tmp/obsr/ab/t_r15`, seeds 9-40, the landed rule; `tools/permap.py`)
Top cells (12 m), recycles in 32 rounds, beside main e54841a (`ab/s_base`):
- BF2 Dagobah (96, -24): 32 (main 30) - seen before, not new.
- BF1 Tatooine 1 (-432, 72) + (-420, 72): 44 (main 22) - card TGAP (a shut connection).
- BF1 Bespin Platforms (0, -156) + (0, -168): 24 of the map's 27 (main 0; 6 with the rim rule alone) - bots on leg 122 -> 7 or last -> 122 at
  about (-3 .. -7, 98.9, -159 .. -168), `steer slide`, from the round's first spawn wave (seeds 10 and 12: unit 8,
  tick 4,695 = the first 120 s after spawning). A 40 m box walked at 4 m/s (`tools/recbox.py`).
- BF2 Kashyyyk (48, -108): 10 (main 10).
- BF2 Polis Massa: 43 over the map (main 12; 39 with OBSR's rim rule alone, `ab/s_b`), 15 of them at (84, 12 .. 24).
  Bursts of 3 - 6 bots within a second late in a round (seeds 11, 14, 18, 23, 27, 37, 39 of `ab/s_b`) read as the
  ENDGAME's new-ground rule, which OBSR did not change [derived: the mid-round rule cannot give up a bot that was
  15 m from where its count began]: of 54 recycles sampled on the landed build (`ab/f_final`, seeds 1-8, the first
  540 s) two had walked 750 - 774 m through a 104 - 117 m box in their last two minutes, both Polis Massa seed 3
  (`tools/recbox.py`). Why the rim rule brings more of those endgames is not known.
- BF1 Geonosis (108, -12): 7 (main 0; card MOVR2's note: by hub 36, team 1, leg last -> 36 or 2 -> 14).
- BF2 Geonosis (-84, 264): 7 (main 4).

## What to do
For each cell: one snapshot (`BLACKBOX_ON=recycle` on `round_to_outcome`, then `blackbox_read --unit <id>`), the
spawn it came from, a flood from that spawn (`Sim::floor_reach`: a pen?), the wall grid (`WALL_GRID`, see card
TGAP), and whether a life crosses the place at all (4 Hz samples, `--botq`). Then the cure by its cause - a seat, a
shut connection, a rim - as its own card when it is not a few lines. Bespin's first: its cell is new and the largest.
Also decide the endgame's rule on Polis Massa: new ground against the box of the whole life recycles walkers there
(2 of 54 sampled recycles had a box over 100 m).

## Acceptance
- A table in the report: each cell above, its cause in one line with the snapshot's path, and its recycles in 32
  rounds (seeds 9-40) before and after.
- All 32 maps, seeds 9-40: recycles at most 230 (main d9db73c: 284) with rounds finished at least 1,021 of 1,024.

- 2026-10-09 12:45 planner. `alpha: yes` -> `defer: post-alpha`: all four places are on maps outside the six showcase maps (BF1 Bespin, BF1 Geonosis, BF2 Geonosis, Polis Massa), value 4, and it blocks no alpha card (the maintainer's cap rule, 2026-09-30). Undo: `loopctl.py set LAPS alpha=yes defer=`.

- WINS 2026-10-22 (main 3ae13d3, 8 seeds, `lead_tmp/wins/census_slide.txt`): two of these laps are ONE-SIDED. BF1 BES1
  (0, -160): team 0 slides there 403 unit-s a round, team 1 ~0 (team 0 slide 13.4 % of rows, recover 14.4 %; team 1 0.3 /
  0.1) - team 0 still wins 6-2, but it is the largest one-team trap of the 14 maps measured. BF2 GEO1 (-80, 270): the
  Republic 121 unit-s a round, the CIS 40, and the CIS win 14 of 16 (1-14 + 1 no outcome). Value this card with that.

## The maintainer's 2026-10-22 override
No longer deferred: BF2 Geonosis (-84,264) first, then the other lap sites. Keep the
existing 32-seed per-site and all-map bars; add 16-seed winner/margin/post tables on each
affected map. No side over 12/16 unless PS2 bots-only trials show the same lean. Trace
all no-outcome rounds. Historical aggregate thresholds do not replace a fresh baseline.
