---
id: LOC3B
title: Units located as the PS2 locates them (3-D, no level rule), a route from an arc started by the PS2's rule and a goal arc kept as an arc: measured by LOC3 to cost the port 1,315 recycles against 838 and 12 maps worse over 32 seeds - run again once the mover cards have landed, and take Dagobah's gain (recycles per round 2.50 -> 0.25) if it holds
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
LOC3 (2026-10-06) made `NavGraph::locate` the PS2's and uses it for posts. Where a UNIT stands (a route's start, a
hunt's quarry, a boarding, an order's goal) it kept the port's XZ locate and level rule as `locate_unit_on` - named
deviation D17 in `analysis/bot-design.md` - because the measurement went against the PS2's rule on today's port:
32 seeds x 32 maps, main 9f2aa09 (A) against the PS2's locate everywhere + the PS2's start on an arc, no level rule (C):
finishes 1,024 -> 1,022 (BF1 RHN1 seeds 13, 21), recycles 838 -> 1,315, 12 maps worse by |t| >= 3 (BF2 UTA1 kills / min
27.8 -> 16.9 and round 416 -> 566 s; BF1 BES1 kills / min 30.0 -> 19.7; recycles / round BF1 BES2 0.00 -> 1.09, BF2
POL1 0.72 -> 5.72, BF2 DEA1 0.00 -> 0.44, BF2 MUS1 0.09 -> 0.78, TAT1 0.91 -> 2.16, BF1 HOT1 1.44 -> 4.25), one better:
BF2 DAG1 2.50 -> 0.25 recycles / round, kills / min 33.3 -> 37.3. Table: `lead_tmp/loc3/ab/perseed_A_C.txt`.
What the split runs say (8 seeds x 9 maps, recycles; `lead_tmp/loc3/q/`, `tools/qtab.py`; noisy, +-3 of 8):
- the 3-D unit locate alone (no start rule, no level rule): BF1 HOT1 6 -> 33 (bots at the trench and the bunker roof
  locate on arc 3 / hub 4 and walk straight at a hub 55-75 m off through the trench props), BF2 POL1 12 -> 26;
- the level rule on top of it: POL1 back to 9-14, HOT1 no better (38-46);
- the PS2's start on an arc (the end whose draw runs along the arc, else the FARTHER end): BF2 DAG1 25 -> 2 - only with
  the farther end, a nearer-end fallback gives 18 - and TAT1 4 -> 19, BF2 HOT1 8 -> 20;
- PS2 locate for posts, hunts, boards and orders with the old start (variant H, full 32 seeds): no map beyond noise,
  recycles 872, finishes 1,021 of 1,024 (three endgame stand-offs, noted on ENDH) - so LOC3 landed posts only.

## Evidence
`engine/artifacts/lead_tmp/loc3/`: `all_variants_scratch.patch` (every variant behind env knobs: `start_for`, its
unit test, the level switch, the nearer-end fallback, the XZ start locate), `locate_ps2_with_aqd.rs` (the fixture
test + the YAV1 aqueduct re-plan test for card AQD: with the PS2's start rule the re-plan at (57.2, -61.5, 52.8)
starts at hub 4 and stays on arc 110 - it passed), `ab/{A,C,H,J}` (the sweeps), `tools/ab.sh`, `tools/mini.py`.
Not carried by any variant: at the farther end the PS2 also counts the unit as arrived on a random other arc of that
end (`FUN_002659e8`), and a goal on an arc keeps the ARC with the end the route's draw reaches (B1 186477-186512).

## Acceptance
1. After MOVR, OBST and LLEG have landed: variants A (main), C (the PS2's unit locate + start, no level rule) and the
   start rule alone, 32 seeds x 32 maps, `perseed.py`. Take what makes no map worse by |t| >= 3 on recycles per round,
   round length or kills / min and keeps finishes >= main's; what stays of D17 keeps its numbers in bot-design.md.
2. If the start rule lands: the AQD test (`locate_ps2_with_aqd.rs`) goes in with it.
3. BF2 DAG1: say what the farther end cures there (recycles / round 2.50 -> 0.25) - it may be a spot OBST or LLEG owns.

## Notes
- 2026-10-06 planner: stays value 3 and out of BOTV's deps (see BOTV's note of today). It carries the AQD test (`lead_tmp/loc3/locate_ps2_with_aqd.rs`): **card AQD's defect (the YAV1 aqueduct re-plan) is NOT fixed on main** - AQD was merged into LOC3 on 2026-10-03 and LOC3 landed the post half only. If a YAV1 aqueduct complaint comes in before this card runs, raise it.
- 2026-10-07 planner: MOVR (6061eec) and OBST (e9ce6db) are on main; this card now waits for LLEG only. `analysis/RE_LOG.md` has no LOC3 line (the file was dirty when LOC3 landed): add the pointer (bot-design.md 4.5 + D17, `game/tests/locate_ps2.rs`) with this card's docs.

## Note from card DAGS (lane 2, 2026-10-12 12:50): the PS2's start on an arc, one live case and one variant
- **Live PS2, BF2 Dagobah** (`lead_tmp/dags/ps2/`, 13 min of RAM): the 7 bots spawned at post cp4's upper node (-20.6, 3.9, -45.1) -
  nearest arc 16-12, at its hub-16 end, hub 16 eight metres under them - all start at hub 12, the arc's FAR end, by a corner walk
  ((-18.4, -59.8) -> (-18.2, -69.1)), and go down the ramp. The port (after DAGS) starts them at hub 10 by `level_hub`'s score.
- **Variant measured** (`DAGS=5` in the scratch binary `lead_tmp/dags-proto_target`, patch `lead_tmp/dags/proto_switch.patch`): in
  `start_hub`, a unit located on an arc whose nearer end is off its storey takes the other end when the floor walked from its feet
  reaches it. Dagobah's node then walks the PS2's way (108 of 141 lives south, median life 26.5 s against 14.5 with DAGS alone), but
  **BF2 Utapau's recycles go 2 -> 18 in 40 seeds** (a pen at (-186, 9.4, 135) under hubs 60 / 0; snapshot `lead_tmp/dags/trace/uta_v5/`)
  and 16 seeds x 32 maps read recycles 110 -> 107, world deaths 1,259 -> 1,289 (`lead_tmp/dags/ab/v5/seeds.md`). Trace Utapau's pen
  before building the arc-end start.
