---
id: TPKT
title: TAT1: bots are recycled in a pocket between roofs at (-138, 20, 102) - 14 recycles on 4 of 8 seeds after BOWL (main 1); TAT1's recycles 36 -> 72
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
After BOWL (2026-10-02) the 8-seed sweep recycles 14 bots at (-138, 18..21, 102) on TAT1 (seeds 1 4 6 8; main: 1 on seed 5), all team 0, Hunt, off the graph near hub 43 (floor_dy +1.5..2.0), sliding. A recycle is a bot the player sees give up and vanish. The level test reads the same there before and after BOWL (profiles in lead_tmp/bowl/README.md), so the pocket is an old trap that more bots now reach; every RIM_DROP <= 0.75 variant shows it, RIM_DROP 1.0 does not.

## Evidence
- lead_tmp/bowl/sw2_base/seeds.md against sw2_v1t/seeds.md, the Recycles tables; recycle lines in sw2_v1t/sweep_s4.err (ticks 8075, 12908, 16807).
- TAT1 over 8 seeds: recycled 36 -> 72, kills 2,107 -> 2,002, median finish tick 19,404 -> 21,398, 8/8 both.

## What to do
1. TRACE_UNIT one of them (seed 4, unit 10, from tick 7000): which barrier turned it and where it was going.
2. Say whether the pocket has a way out a bot can walk (LEVEL_PROFILE across it), then fix the steering or the route there; do not raise RIM_DROP for it.

## Acceptance
- 8 seeds x 60,000 ticks on TAT1: no recycle spot at (-138, *, 102) on 2 or more seeds; TAT1's recycles back to 40 or under; no map's finish count drops.

## Planner note (2026-10-02 04:30)
`alpha: yes`: TAT1 is a showcase map. On main 22eddb0 (`lead_tmp/lvlf/rb_fix/seeds.md`, 8 seeds) the pocket
(-138, 21, 102) recycles 4 bots on 4 seeds (1 3 5 7) and TAT1 recycles 54 in all. Most of the others are at hub 104
(`last->104 dest 104`, floor dy +2.5..+7.2), which STLVL's and STARC's tables already own. So read this card's bar as:
the pocket gone on every seed, and TAT1's recycles not above the same run's baseline; "back to 40 or under" depends on
those two cards.

- Planner 2026-10-02 09:30: STLVL landed (9f5582e) with the tools for this: `recycle_lv` lines, `FLOOR_GRID=x,z,half,
  step,y_top` and `NAV_FLOORS=1` in `round_to_outcome`, `lead_tmp/stlvl/tools/lv.py`. It fixed the same class on TAT2
  (a soldier under rooftop hubs 25 / 26 routed from them). Re-count the pocket on main first
  (`lead_tmp/stlvl/c1_s1_8/seeds.md` has main's 8 seeds); close the card if it is gone on every seed.

## Audit note (AUDIT BOWL, 2026-10-02; main 69ba267 against the same tree with `RIM_DROP = 2.0`, 32 seeds, `lead_tmp/audit-bowl/`)
- The pocket at (-138, 20, 102) is an old trap, as this card says: recycles in its 12 m cell 26 on 16 seeds WITHOUT
  BOWL's rule, 35 on 13 seeds with it; stays in the box x -142..-132, z 98..108: 81 (3,736 unit-s, median 3 s, p90
  121 s) without the rule, 57 (5,178 unit-s, median 120 s) with it.
- A SECOND spot is the rule's own: (-69, 19, 51), a 35 degree slope (n.y 0.818) beside barriers 199 and 200, 5 m over
  hubs 96 / 97. Without the rule bots cross it in 1-5 s (511 unit-s in the box x -72..-60, z 46..60; median stay 3 s,
  p90 5 s; 0 recycles in the cell); with it 6,393 unit-s, median 10 s, p90 122 s, 21 recycles on 12 of 32 seeds. The
  level test reads `Unknown` there (5,799 of the 6,393 samples), not off: the units slide (`steer slide`) — the way down
  crosses a barrier the tighter rule now makes a wall.
- TAT1's recycles over 32 seeds: 131 without the rule, 180 with it. Acceptance here should name both cells; card LVLB
  owns the rule, this card the two spots. `tools/episodes.py <UNIT_SAMPLES dir> tat1 -72 -60 46 60` and
  `… -142 -132 98 108` print the stays; `out/tat1_stays.txt` has main's.

## Note (planner 2026-10-03 07:15, from audit JARC) - a second spot for the same tools
BF2 Kamino hub 98 at (90.8, 57.58, 193.4): the stored hub height is the interior corridor's ceiling. A PS2 soldier
released there lands on the floor at 51.48 (8 of 8); the port stands him on the ceiling's collision face at 57.575
(`lead_tmp/audit-jarc/floor_h98.txt`). With this card's instruments (`recycle_lv`, `FLOOR_GRID`, `NAV_FLOORS`) print
what `hub_floor` resolves for hub 98 and whether a bot is ever sent to stand at 57.58. Two outcomes: the hub's floor
resolves to 51.48 and nothing is wrong (say so here); or bots are placed on the ceiling face - then it is a
one-sided-face question (card COLN: the PS2's ray-against-leaf test is one-sided) and goes there as a note with the
numbers. Not a new card until measured.
