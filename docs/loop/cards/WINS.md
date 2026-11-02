---
id: WINS
title: One side wins every round on 10 of 32 maps (main a88147b, 16 seeds each: bf2_kam1 16-0 with team 1 at 0-1 tickets and 0 alive, bf2_dag1 16-0, bf2_uta1 0-16, bf1_rhn2 16-0, bf1_geo1 / bf1_hot1 0-16, bf1_end1 15-1, bf1_kas1 / bf1_yav1 1-15, bf2_geo1 1-14): per map, sort the share into the mission's own asymmetry (tickets, unit counts, classes, the PS2's bots-only result) and a port fault on one side, name the fault on the worst showcase maps
milestone: M3
value: 5
phase: measure
status: done
deps:
files:
declares:
needs:
effort:
branch:
resume:
created: 2026-10-22
alpha: yes
---

## Why
KA137's report (2026-10-21 23:36): all 24 BF2 Kamino rounds on main end "team 0 wins" with team 1 at 0-2 tickets. The
planner tallied the fidelity baseline's 16 seeds per map (main a88147b, `lead_tmp/fidelity_rebase_20261021b/fidelity*/
sweep_s*.txt`, team 0 - team 1 wins): bf2_kam1 16-0, bf2_dag1 16-0, bf1_rhn2 16-0, bf1_end1 15-1, bf1_geo1 0-16, bf1_hot1
0-16, bf2_uta1 0-16, bf1_kas1 1-15, bf1_yav1 1-15, bf2_geo1 1-14; showcase maps: uta1 0-16, yav2 2-14, myg1 3-13, nab1
5-11, tat1 6-10, bf2 hot1 7-9. Kamino's losers end with 0 alive (a wipe, not a bleed). Some of this is the mission's own
design (Hoth, Geonosis); a port fault that starves one side (a route, a spawn pen, a class, a vehicle) would make every
battle the maintainer plays one-sided. No card measured win share before; BOTM / BOTV do not.

## Notes
- planner 2026-10-22 00:10: KA137 also saw 2 of 24 KAM1 rounds recycle a bot at (-185, 68, 31), cp1, leg 100 -> 103, steer `slide`, floor_dy -128 (`lead_tmp/ka137/main24r/s2.err`, `s16.err`) - check whether it is on the losing side.

## Result (2026-10-22, lane 1, main 3ae13d3) - DONE, no code
Port columns: the fidelity baseline's 16 seeds (main a88147b, `lead_tmp/fidelity_rebase_20261021b/fidelity*/`); posts,
captures and env deaths from WINS's 8-seed re-run on main 3ae13d3 with a read-only probe (once-a-second post owners +
tickets and `EnvDeath` rows in the BOT_SAMPLES ev file; patch `lead_tmp/wins/probe.patch` on main, binary
`lead_tmp/wins/rto_probe`, outputs `lead_tmp/wins/run1/`). "Loser lost" = tickets lost by kills + by the rest (bleed and
unattributed deaths). Every mission gives both sides equal tickets; the sweep fields 17 (team 0) v 16 on every map (the
slot-0 filler), yet over all 511 rounds team 0 wins 243 and team 1 268 - no aggregate tilt.

| map | port t0-t1 (16) | winner's tickets (median) | loser's end | posts held t0 / t1 (8 seeds) | PS2 rounds | sort |
|---|---|---|---|---|---|---|
| bf2_uta1 (showcase) | 0-16 | 72 | 0 tickets: kills 77 + bleed 73 | 1.4 / 3.9 | Rebels (t0) won 48-2 and 28-0, held 3.8 / 2.1 | PORT: Rebels trapped at their base (card UTAX) |
| bf2_kam1 | 16-0 | 86 | 0 tickets: kills 90 + bleed 60 | 3.6 / 2.3 | Imperials (t1) led 79-45 (STALL, 415 s), 33-3, won 34-1; held 2.5 / 3.3 | PORT: Imperial route ping-pong at hubs 46/44 (card KAMPP) |
| bf1_geo1 | 0-16 | 189 | domination (all 16 rounds), some at 77 s on 197/235 | 1.1 / 2.7 | - (rule [O]) | PORT-RULE: walker post ignored (card B1WP, needs the maintainer) |
| bf1_hot1 | 0-16 | 188 | domination (all 16) | 1.0 / 3.9 | - (rule [O]) | PORT-RULE: same (B1WP) |
| bf2_dag1 | 16-0 | 42 | 0 tickets: kills 125 + 25 | 3.4 / 2.3 | - | UNSORTED: close rounds, kills even, no one-team trap in the census |
| bf1_rhn2 | 16-0 | 93 | kills 127 + 73 | 3.6 / 2.6 | - | UNSORTED (no trap; BF1's own bleed is not played) |
| bf1_end1 | 15-1 | 101 | kills 98 + 102 | 3.0 / 1.8 | - | UNSORTED |
| bf1_kas1 | 1-15 | 37 | kills 147 + 53 | 2.4 / 3.4 | - | UNSORTED (mission is 32 Imp v 25 Reb; the sweep is 16 v 16) |
| bf1_yav1 | 1-15 | 90 | kills 99 + 101 | 2.3 / 4.4 | - | UNSORTED |
| bf2_geo1 | 1-14 (+1 none) | 67 | kills 86 + 60 | 2.7 / 3.9 | - | PORT candidate: the (-84, 264) lap is one-sided, Rep 121 / CIS 40 unit-s a round (note on LAPS) |
| bf1_yav2 (showcase) | 2-14 | 70 | kills 118 + 82 | 2.1 / 2.8 | - | UNSORTED (37 Imperial lives never leave spawn v 19 - small) |
| bf2_myg1 (showcase) | 3-13 | 61 | kills 85 + 62 | 1.7 / 2.8 | - | UNSORTED |

Common shape: kills are even on every lopsided BF2 map (within 15 %); the round is decided by posts (bleed). So a share
this one-sided is a POST problem - a side that cannot reach or hold posts - not a fighting one. The census that finds it
(per team: slide share, recover share, time to 50 m from spawn, unit-s per 10 m cell) is `lead_tmp/wins/census_slide.txt`
and the scripts in `lead_tmp/wins/tools/` (wins_cp.py: posts held / captures / majority; wins_leave.py: time to 50 m;
wins_b.py: tickets lost by kills vs the rest).
- KA137's two recycles at (-185, 68, 31) (s2, s16) were team 0 - the winning side; not a factor.
- PS2 tool: `lead_tmp/wins/ps2/wins_ps2.py <map> --tag rN` (STALL's sampler, stops 30 s after a team's tickets hit 0;
  `PCSX2_FORCE_SW=1` while the RTX is lost; ~10-15 min a round; two maps at once are fine).
