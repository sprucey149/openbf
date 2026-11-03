---
id: TGAP
title: BF1 Tatooine's footpath is shut between hubs 78 and 77 on the PS2 too (measured 2026-10-09: a PS2 soldier passes 0 of 4 times west, 4 of 4 east; the port's slab is shut both ways) - 44 of the map's 47 recycles in 32 rounds: a load-time wall test beside the floor walk marks the arc unwalkable so the route table takes its second way
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
From card OBSR (2026-10-07, landed d9db73c). BF1 Tatooine 1's "rock gap" (AUDIT OBST item 1) is not a steering
defect: the footpath's connection from hub 78 to hub 77 is SHUT for the port's soldier, on the steering before card
OBST too. The map's worst stuck spot stays until it is walkable or routed round: 44 of the map's 47 recycles in 32
rounds (seeds 9-40) are there, 1,579 unit-seconds in 8 rounds (seeds 9-16), every life ending in the 120 s recycle.

## Evidence (`engine/artifacts/lead_tmp/obsr/`, main e54841a unless said)
- The path: a soldier-only chain of small hubs (mask 0x1, radius 0.5 - 1.2 m) 86 -> 85 -> .. -> 79 -> 78 -> 77 -> 76
  -> 75 -> 74 -> 2, arcs 81 .. 93; vehicles go 2-100-101-102 / 2-54. Hub 78 (-424.2, 74.6), hub 77 (-430.0, 80.3).
- 0 lives cross from the corridor's end (x over -427.8, z under 78.4) to hub 77's side (x under -429, z 79 - 83) in
  8 rounds x 4 Hz samples, on the parent 9cd9b52 (`lead_tmp/audit-obst/tat1/pre`) AND on main; no sample of either
  build lies south of z 78.5 in the 24 m square round (-427, 77). The parent's "173 unit-seconds" was its bots being
  given up sooner (957 samples there against 14,344), not a passage.
- The wall grid (`trace/tat1_gap_grid.txt` 0.5 m, `tat1_gap_fine.txt` 0.2 m; scratch hook `WALL_GRID=<radius>` beside
  `FLOOR_GRID`, in `obsr_scratch_knobs.patch`): `tat1_bldg_rock_rubble_pillar` (body 71, origin (-440.2, 17.8, 85.5))
  and `tat1_bldg_rock_rubble` (body 77, origin (-401.2, 13.2, 97.0)) form a V that hub 78 stands in; its apex at
  (-427, 78.4) is closed, and stays closed for a capsule of radius 0.4, 0.3, 0.2 and 0.1 m. A capsule swept from
  (-427.0, 78.1) to (-429.6, 79.3) is stopped at every height from the floor to 4 m over it (`PINCH=` hook): a leaning
  slab, not a kerb. At terrain level under the rocks' tops (`FLOOR_GRID` from y 19.2) the same band is shut.
- Both classes have no collision property (`collision: []` in the manifest): every body of the model is soldier
  collision, the layer rule has nothing to drop.
- The floor walk calls arc 89 walkable both ways (`fg arc 89 78-77 .. unwalkable a>b false b>a false`): it tests
  floors, not walls. Arc 88 is unwalkable 78 -> 79, so a bot at hub 78 with arc 89 refused has no hop left.
- The hubs' authored heights (79: 15.2, 78: 17.2, 77: 19.5) follow the terrain ramp (16.9, 17.8, 19.4); the port
  resolves hub 78's floor to the rock's top (20.56), 3.4 m over its authored height, with the terrain 0.6 m over it.
- Trace of one life: `bb/main/tat1_s10.json` unit 19 (480 s; 3 s laps between (-427.9, 77.9) and (-422.5, 73.2)).

## PS2 measured (2026-10-09, `engine/artifacts/lead_tmp/tgap/`, report `reports/20261009_1100_card_TGAP.md`)
The answer to step 1: **the PS2's soldier does not walk 78 -> 77 either. The port's collision is not the defect;
the authored connection is shut in the original, one way.** BF1 TAT1, a pad-driven soldier (`tgap_probe.py`: dropped,
released, W held, the camera kept on the next hub; game-clock timed; `*_run.log`, `out/*.csv`, frames `out/*.png`):
- 78 -> 77 (west): **0 of 4 walks pass.** From hub 79 on the terrain (under the rock) and from hub 79 on the rock's
  top (the way the port's bots come) he stops at (-428.2, 19.75, 77.4) and stays for 26 s and 21 s; two more walks
  with the aim swung +-70 deg round hub 77's bearing and 20 jumps (48 s and 32 s) get no nearer than 2.86 m to hub 77
  and never west of x -428.95 (a crossing is x under -429, z 79-83).
- 77 -> 78 (east): **4 of 4 pass**, at 6.5 m/s without a stop, and go on over hubs 79, 80, 81 on the rock's top
  (y 20.4, 21.6, 22.6, 22.7). At the slab he is moved 0.85 m in one update ((-428.39, 19.62, 78.67) ->
  (-427.89, 19.78, 77.98); 0.21 m is a running step): the PS2's slab stops him from one side only.
- Hub 78 has two floors on the PS2 as in the port: dropped from y 26 he stands at 20.41 (the rock's top; the port
  20.56), from y 18.4 at 17.62 (the terrain under it; the hub is authored at 17.18). Hub 77: 19.42.
- The PS2's own bots: one AI battle (`battle_sample.py`, state `tat1_cp0_cp10_postA_spawn`, 510 game s to its end,
  38 soldiers on foot on average, 5 Hz, 97,318 rows): **0 rows within 6 m of hubs 75-79 and 81-86**; the 32 unit-s
  within 14 m of (-427, 77) are nine Tusken lives running from (-419, 15.8, 66) to (-439, 23.4, 71), south of the
  rock, in 3 s each. One battle, the player idle at his spawn: at the port's rate (12 long lives in 8 rounds) "none
  in one battle" is no proof the PS2 never routes a bot there.
- The port, the same two walks (`port/east`, `port/west`, `game --headless --walk-replay`, main 82fb7db+): east
  from (-432.1, 82.4) at hub 78 the soldier stops at (-428.4, 19.57, 79.0) and creeps to (-427.80, 19.52, 79.37):
  **the port's slab is shut BOTH ways** (the PS2's one). No port bot comes from that side: 0 samples on hub 77's
  side in 8 rounds (`obsr/spots/final/tat1`); every port bot at the apex stands on the rock's top (y 19.8-21.6).

## Built 2026-10-15 (lane 1, branch tgap 70f9d71; report `reports/20261015_2316_card_TGAP.md`)
Traced on main d896ca4 first: 22 of 23 long lives at the slab spawned at ONE node, cp1 node 4 (-377.3, 20.7, 83.0),
17 m off the footpath, whose first leg toward EVERY post was hub 85 -> 84 (spawn_node_census). The footpath 79 .. 86
is a dead end (86 is a leaf; 78 -> 79 already dead), so marking arc 89 alone would leave the bot at 78 with no hop.
Built: (1) `Sim::arc_walled` / `Sim::floor_path` - a best-first soldier walk round the walls, both ways required;
(2) `Sim::hub_trapped` - no walkable way back to the largest mutually-walkable hub set; `plan_leg` sends no soldier
into one, `level_hub` swaps a trapped start he does not stand in. Census `lead_tmp/tgap/census/` (passes_br3.txt):
57 -> 72 dead directions, all 15 new ones walked 0 times by main's bots (32 maps x 3 rounds); 17 trapped hubs.
Bars: seeds 9-16 at the gap 2,519 -> 0 unit-s, long lives 20 -> 0; seeds 9-40 recycles 90 -> 3. Fidelity: tat1
recycled 17 -> 0, nothing worse. Test `game/tests/nav_walls.rs` (3 mutations each fail it).
