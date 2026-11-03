---
id: KAMPP
title: BF2 Kamino: Imperial attackers ping-pong between hubs 46 and 44 at kam_bldg_pod2 (table 46->44, BFS 44->46 back on arc 36) and walk home - they spend ~10 unit-s a round at the Rebel posts, the Rebels hold them 95-100 % and win 16 of 16; the PS2's Imperials led 79-45 and 33-3 and won 34-1
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card WINS (2026-10-22): on main 3ae13d3 the Rebels win BF2 Kamino 16 of 16 seeds (fidelity baseline) and 8 of 8 (WINS
re-run); the Imperials end on 0-1 tickets. On the PS2 the Imperials win: WINS's PS2 rounds (bots only, 16 a side) have
the Imperials ahead 79-45 at 415 s (card STALL's sample, `lead_tmp/stall/ps2/bf2_kam1_cps.jsonl`) 33-3 after 895 s of
sampling (`lead_tmp/wins/ps2/bf2_kam1_r1_cps.jsonl`) and won round 2 outright (1-34, every post; `..._r2_cps.jsonl`),
holding 3.2-3.4 posts on average against the Rebels' 2.4-2.6 (the port: Rebels 3.6, Imperials 2.3). In the port the Imperials attack the Rebel
posts (cp1, cp6, cp5) and almost never arrive: 9-10 unit-s a round within 20 m of cp1 / cp6 and 0 of cp5, against
~70-90 unit-s a round per target spent in one corridor x -20..0, z 80..200 (8 seeds, `lead_tmp/wins/run1/bq/`); the
Rebels hold their three posts 95-100 % of every round; Imperials die 728 lives to the Rebels' 612.

## Evidence (one traced case, main 3ae13d3 + a read-only probe)
Black box `engine/artifacts/lead_tmp/wins/bb/k9000/bf2_kam1_s1.json` (BLACKBOX_AT=9000, seed 1), `--unit 30`: Imperial
rifleman #30, attack1 (cp6 at (-31, 27), dest hub 47), walks 33->43->46, then `leg 46->44 arc 36 t` (table), slides on
`kam_bldg_pod2` @ (-14.6, 60.3, 102.3) (STOP Wall 0.13 m), reaches hub 44 and gets `leg 44->46 arc 36 b` (the BFS
fallback back along the same arc), then 42->40, 42->46, 46->44 again, 46->43, 43->33, 33->2 - it walks back north to
(-7, 185), 60 s after it was at (-20, 100). The table's next hop from 46 is 44, and from 44 the route goes back to 46:
a ping-pong. The same unit's previous life (bb/k7000, tick 7500) walked the same 33->43->46 approach.
Related: KA137's arc 137 (121 -> 123, the sea crossing) is cut for soldiers in the port (BNDS arc_unwalkable) - check
whether the table's route from 44 to 47 still assumes it.

## Do
Find why hub 44 has no consistent next hop toward hub 47 (and the other Rebel posts) - a table built with an arc the
soldier filter or `arc_unwalkable` later drops, a level-rejected leg, a barrier - and fix it at the root.

## Acceptance
- A test on the KAM1 graph: from hubs 46 and 44 the routes to hub 47 agree (no hop back along the arc just walked), mutation-checked.
- 8-seed census on the branch: Imperial unit-s within 20 m of cp1 / cp6 per round >= 40 (main 9-10); report the win share
  (main 16-0 Rebels; the PS2 had the Imperials ahead in both rounds).

## The maintainer's 2026-10-22 priority and referee
Promoted from post-alpha by explicit instruction. After the existing 8-seed spatial bar,
run 16 seeds on current main and candidate: no side above 12 wins unless bots-only PS2
rounds demonstrate the same lean. Name the PS2 evidence and all unfinished rounds;
an unexplained one-sided result remains a port fault. Do not change the baseline.
