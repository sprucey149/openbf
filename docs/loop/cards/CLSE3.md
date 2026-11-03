---
id: CLSE3
title: Polis Massa after CLSE: inside 10 m the port's bots still deal 0.36 of the PS2's damage per aimer-second (29 hp against 81; Kamino 53 against 71) - they fire for 23 % of that time against 34 %, hold a target they do not see for 11-31 % of it, and hit with 0.3 of their rifle shots against 0.6-0.9: trace it and name the cause
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card CLSE's acceptance asked for the port's damage per aimer-second inside 10 m at 0.67-1.5 x the PS2's on both
maps. After its landing (77f4c70) Kamino is at 0.75 x (53 hp against 71) and **Polis Massa at 0.36 x (29 against
81)**. Polis Massa is corridors: this is the doorway fight the maintainer sees.

## Evidence (`engine/artifacts/lead_tmp/clse/`; `analysis/re-soldier-ai.md` 17.1, 17.4)
- `out/table_ps2.txt` against `out/table_port_v11.txt` (v11 = the landed rule on BF2 maps, byte for byte), inside
  10 m on Polis Massa, PS2 | port: firing 34 % of aimer time | 23 %; damage while firing 203 | 94 hp/s; a rifle's
  falls of the target's health per shot 0.58-0.88 | 0.21-0.31 - while the port's rifle HITS somebody with 0.57 of
  its shots by its own hit rows, so half of its hits are not on the unit it aims at (or are the killing hit, which
  no method here counts on either side); rifle in hand 52 % | 29 % (pistol 22 | 32, shotgun 9 | 15).
- The port's aimers there hold a target they do not see now for 18 % of the time inside 5 m and 40 % at 5-10 m
  (Kamino 2 % and 15 %; `out/split_port_v11.txt`); "other" (off the tracker's 2 deg, a friend in the line, the
  tap gate) 22-28 %.
- Not yet looked at: who stands between (the PS2's bots shoot through friends, the port's wait - FFLN, 3,952
  held ticks a round on Hoth alone), the bolt's start at the muzzle at arm's length, the aim tracker's lag (the
  aim sits a median 14-16 deg off inside 5 m).

## Result (2026-10-22, measure; main 9193a4c; `lead_tmp/clse3/`, report `loop/reports/20261022_0720_card_CLSE3.md`)
Polis Massa inside 10 m is now 22.0 hp per aimer-second against the PS2's 80.8 (0.27 x; Kamino 32.6 / 70.6 = 0.46 x). The cause
tables are in the report. The largest term is not gunnery: the metric splits every fall over the victim's aimers and the port's
victim has 3.38 of them against the PS2's 1.40 (0.41 x); the victim's own hurt rate is 63.6 against 103.3 hp/s (0.62 x).
Knock-ins of two PS2 rules (sight line from the eye to the target point; 22.5 deg trigger cone) left the figure at 19-23 hp.
The 0.67 x per aimer-second bar is not reachable by the gunner alone; card CLSE6 carries the crowd and the hurt rate.
