---
id: DAGS
title: BF2 Dagobah: team 0's bots stand in 'recover' for 20-140 s at (2.4..4.1, 4.5, -47.9) within seconds of spawning (already on main, 2 of 8 seeds; the PS2's spawn draw of card BSPN sends more bots there and its landing is refused on stand20_pct@bf2_dag1) - trace one case, name the spawn node / post and the cause, fix it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card BSPN (the maintainer's ask, approved by him 2026-10-11 for Bespin's falls) cannot land: the fidelity gate refuses stand20_pct@bf2_dag1 0.36 -> 1.08 on seeds 1-8 and 1.32 on the confirm seeds 9-16 (tolerance 0.7). The standing is not new - main has it at this same spot - the PS2's post-then-node draw only sends more bots to it.

## Evidence (land session 2026-10-11, report reports/20261011_1643_land_BSPN.md)
- Gate rows, Dagobah, seeds with a standing spell: baseline (main 4c238b1, engine identical to 8ad30f4) 1 of 8 (202 foot-s); branch bspn be6d9ac seeds 1-8 4 of 8 (143, 106, 242, 103 foot-s), seeds 9-16 5 of 8 (112, 103, 210, 192, 115). Nearly all of it is behaviour 'recover'.
- From the 4 Hz samples (botq/bf2_dag1_s<seed>.csv; still within 1 m for 20 s+ in 'recover'): team 0 bots at (2.2..4.1, 4.4..4.6, -47.8..-48.0), 6-40 s after their spawn, for 23-140 s - baseline s6 (units 9, 8) and s7 (units 6, 12); branch s1 (units 6, 5), s2 (unit 11), s9 (unit 7), s15 (unit 10). Second spot, below it at y -3.9: (10.6..19.8, -3.9, -43.8..-52.3), 20-31 s spells, both builds. One team 1 bot at (35.7, 3.7, 149.1) for 86 s on the branch's seed 3.
- Files: engine/artifacts/lead_tmp/bspn/land/branch/fidelity{,_confirm}/ (branch), engine/artifacts/lead_tmp/fidelity_rebase_20261009/gates/fidelity/ (baseline).

## Trace (Claude in chat, 2026-10-11, main 8ad30f4; snapshot `lead_tmp/dags/main_8ad30f4_bf2_dag1_s7_t3000.json`, `blackbox_read --unit 6` / `--unit 3`)
Seed 7, BF2 Dagobah, units 6 and 3 (team 0). On main's own fidelity run 10 such spells of 76-172 s show on seeds 3, 6, 7, 10, 11
(`lead_tmp/fidelity_rebase_20261011/gates2/fidelity*/botq/bf2_dag1_s*.csv`).
- Both SPAWN at one node, **(-20.6, 4.0, -45.1)**, feet y 4.0 (ticks 1230 and 1399).
- Both are put straight on leg **16 -> 15** (`locus N`, off the graph). Hub 16 (Hub22) is at (-6.6, -54.4) and hub 15 (Hub21) at
  (1.0, -47.0), radius 2.6 - and the leg's aim point is at **y -3.5**: `wp [1.0, -3.5, -47.0]`. Other bots cross hub 15 with their
  feet at y -3.3 .. -3.7 and pass in 1-8 samples. So the route runs about 8 m BELOW the level these two spawn on.
- They walk the upper level to hub 15's XZ (wp_d 0.39 at tick 1350, feet y 6.3), never count as at the hub, and from tick 1365 are
  against `dag1_prop_barrier_1 dag1_prop_barrier_23 @ (1.9, 4.0, -42.4)` (`stop wall 0.00`) at (2.4..4.0, 4.5, -47.9).
- From tick 1470 it alternates `recover` (10 s) and `travel` (2 s): recover re-plans `-1 -> 12` (hub 12's point (-10.0, 2.7, -66.2),
  the only one near its own height) then `12 -> 16` (y -3.4), each time with the barrier still shut in front; `stuck` events every
  6-12 s; `level 0.0`, `lost 0` throughout, so neither the lost rule nor a recycle ends it. 150 s for both.
- Why BSPN multiplies it: the flat node draw gave this node one share among all of team 0's nodes; the PS2's post-first draw gives
  its post a whole post's share (main 1 round in 8, BSPN 4-5 in 8).
NOT traced: which post owns the node; whether a floor walk from the node reaches hub 12 (the ramp down); what the PS2's bot does
from this node (a capture there is the bar); why the hub's "top floor" reads 7.0 in `floor_dy`.

Candidate fixes, to measure: (1) a spawned bot's first leg starts at a hub the floor walked from its feet reaches (`walk_floor`),
not the nearest hub in XZ - here hub 12, not 16; (2) a bot that has cycled recover / travel N times without 2 m of net movement
is lost (RETW's floor path back) or recycled, not left 150 s; (3) census every spawn node on the 32 maps: the node's floor against
the level of the hub its first leg takes. Same family as card MOUTH (a leg started from where its floor does not lead).
