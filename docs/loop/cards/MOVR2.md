---
id: MOVR2
title: The PS2's stuck clock, arrival radii and leg aim, measured again on OBST's steering: each cost pinball windows on Kamino or Polis Massa with today's wall slide and was left out of MOVR (bot design 4.5, 4.7)
milestone: M3
value: 5
phase: build
status: done
deps: OBST,MOVR,OBSF
files: engine/crates/core/src/ai/mover.rs,engine/crates/core/src/ai/route.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai.rs
declares: sim; sweep; arena
needs:
effort: high
branch: movr2
resume: landed 6f6160b2 2026-10-07
created: 2026-10-06
alpha: yes
---
## Why
Card MOVR (2026-10-06) built six rules behind switches and landed the three that measured as gains (the arrive latch
and deadband, the heading held three ticks, the think's phase by unit index) plus the give-up rung on the old 2 s
window. Three rules of `analysis/bot-design.md` 4.5 / 4.7 (the PS2's, `analysis/re-soldier-ai.md` 2) LOST on today's
port and were left out - by measurement, not by choice of design:

| rule (alone, on top of the held heading; 9 maps x 8 seeds) | KAM1 pinball / stay path / stay rev | POL1 | recycles |
|---|---|---|---|
| held heading only | 1.47 % / 4.7 m / 0.92 | 2.29 % / 6.3 m / 1.05 | 27 |
| + the PS2's stuck clock (2 m in 3 s, no node progress in 5 s) for the 2 s window, with the Recover ladder | 1.39 / 5.8 / 0.84 | 5.02 / 10.5 / 1.56 | 40 |
| + arrive 1.0 m at a node (today 1.5) | 2.03 / 6.5 / 1.25 | 2.30 / 7.1 / 1.16 | 42 |
| + arrive 0.5 m at the last node (today 1.0) | 1.83 / 7.2 / 1.99 | 3.08 / 9.7 / 1.89 | 28 |
| + aim at the projection on the leg advanced by speed + 0.2 m (today: 5 m ahead when over 1 m off the line) | 1.87 / 4.8 / 1.02 | 2.10 / 6.5 / 1.14 | 45 |

The reading (mine, [H]): each presupposes a mover that does not snag - the PS2 has barriers and contact only, the
port's wall slide runs along a wall for seconds, and a 3 - 5 s clock or a 0.5 m radius leaves it there longer.
Card OBST replaces that steering; these rules are to be measured again on top of it.

## Notes
- MOVR's report: `engine/artifacts/loop/reports/` (card MOVR, 2026-10-06) has every variant's table.
- 2026-10-07 planner: **value 4 -> 5.** BOTV waits on this card, and BOTV is the gate the maintainer set in front of PERF and UCNT; at value 4 the picker reaches it only after every value-4 M2 and M1 card (dry run: not in the next 34 picks). Every other card BOTV waits on is 5. OBST landed (e9ce6db) and its follow-up OBSF is being built: measure on main after OBSF; if a card OBST2 (the barrier keep-out, vehicle footprints, soldiers in velocity space - named in e9ce6db's message) has landed by then, on that. MOVR's open point is yours too: give up after 3 or after 6 failed windows (9 maps x 8 seeds favour 6, 24 seeds x 4 maps favour 3). Undo: `loopctl.py set MOVR2 value=4`.
- 2026-10-07 (card FGHT's session, a note): the maintainer's Kamino scene still has one trail over 6 reversals in 40
  snapshots on main 69f8c84, and it is a LEG walk, not a fight: leg 46 -> 44 from (-20, 60.3..61.4, 105) slides
  between `kam_bldg_pod2`'s wall and a Steep stop, 20 reversals in 10 s, 28 m walked for 1.8 m net
  (`lead_tmp/fght/scene_c7/t652400.json` unit 30; the same spot in `scene_c6/t623600.json` unit 27). Runner:
  `lead_tmp/fght/tools/scene_seeds.sh <binary> <out dir>`.
