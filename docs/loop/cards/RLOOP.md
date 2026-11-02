---
id: RLOOP
title: A bot that re-plans again and again without moving is ended, not left: on BF2 Dagobah two bots alternate recover (10 s) and travel (2 s) against one barrier for 150 s each (main 8ad30f4, seed 7) with `lost 0`, `level 0`, a `stuck` event every 6-12 s and nothing that stops it - after N failed recoveries with under 2 m of net movement the bot is lost (RETW's floor path back) or recycled
milestone: M3
value: 5
phase: build
status: done
deps: RETW
files: engine/crates/core/src/ai.rs,engine/crates/core/src/ai/stall.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/mover.rs,engine/crates/core/src/sim/bounds.rs,engine/CRATES.md,scripts/loop_spells.py
declares: sweep; arena; sim
needs:
effort:
branch: rloop
resume: LANDED 70b9c47 (+70bfa91, a059815) 2026-10-15 18:40: floor-walk give-up; report reports/20261015_1840_card_RLOOP.md. Open for the planner: wire scripts/loop_spells.py into fidelity; BF2 Kashyyyk pen (46,28,-109) is the one spot left over 30 s.
created: 2026-10-11
alpha: yes
first: 3
---
## Why
The maintainer, 2026-10-11 (chat), of the Dagobah stand: "when bots die and respawn, they just stand there as is for a few minutes?" The
cause of THAT spot is card DAGS (a first leg aimed at a hub 8 m below the spawn). This card is the safety net under every case
like it: whatever puts a bot where its route cannot be reached, it must not stay 150 s. Today nothing ends it - the lost rule
wants the bot off the playable level, the recycle wants no progress on ONE plan, and each recover hands it a new plan.

## Evidence
- `engine/artifacts/lead_tmp/dags/main_8ad30f4_bf2_dag1_s7_t3000.json`, units 6 and 3 (`blackbox_read --unit 6`): from tick 1470
  `recover` for 10 s then `travel` for 2 s, over and over; the plan flips `-1 -> 12` / `12 -> 16`; `stop wall 0.00` on
  `dag1_prop_barrier_23` each time; position inside a 2 m box at (2.4..4.0, 4.5, -47.9) for 150 s; `stuck` events at ticks 1650,
  1830, 2025, 2205, 2415, 2445, 2805.
- Main's own fidelity run, seeds 1-16: ten such spells of 76-172 s on Dagobah alone
  (`lead_tmp/fidelity_rebase_20261011/gates2/fidelity*/botq/bf2_dag1_s*.csv`). Card DAGS has the trace.
- Card MOUTH's stalls (`lead_tmp/walk/`) are the short form of the same thing; its longest are 8-11.5 s.

## Notes
- `files:` is empty on purpose (the AI files are held by OBST2's parked branch): the session lists what it touches.
- Not this card: the first leg from a spawn (DAGS), a leg's mouth (MOUTH), the lost bot's path back (RETW).
- 2026-10-12 planner: **a second spot for the census, BF2 Kashyyyk (48, 28, -108).** Today's fidelity baseline run on main 19708fa (`lead_tmp/fidelity_rebase_20261012b/gates/fidelity/seeds.md`, the bf2_kas2 rows): 6 recycles there on 4 of seeds 1-8 (1, 2, 5, 7), leg `48->47`, both teams (t0 358 and t1 336 standing foot-seconds, 647 of them in `recover`, 447 in `board`); `stand20_pct@bf2_kas2` 1.51 on seeds 1-8 and 0.59 on 9-16 (main 2d1fb60: 1.03; GRAIA's landing report flagged it). Not traced. If it is the same recover / travel cycle the rule here ends it; if its cause is a first leg or a boarding order, say so on DAGS / SPNC.
- 2026-10-12 card MOUTH (measure): **a second standing case, BF1 Bespin Platforms, every round.** Unit 8 spawns at
  (-21.1, 98.7, -187.6) touching `bes1_bldg_main02`, fails leg `6->8` at the round building's door 5.9 m short of hub 8,
  is put on legs `8->122` / `122->7` (through the INSIDE) and walks the wall OUTSIDE between (-13, -172) and (-2.5, -157)
  for the whole round: `recover` count 14 in its first minute, `round_t` over 1,800 ticks, never lost, never recycled
  inside two minutes. It is 351 of the map's 566 stalls in 8 seeds (`lead_tmp/mouth/out/bes1_port_P.txt`;
  `tools/trace.py out/bes1_port_p.csv snap_bes1 '122>7' 3`). The PS2's bots and the port's others use that door alike
  (`out/bes1_sites.md`: 2 PS2 stalls in 95 passes of the building). Under MOUTH's prototype BF1 Kamino shows the
  same shape: unit 26 at (150, 52, 155), legs `-1->13` / `13->16`, 34 stalls in three of eight seeds.

## Note from MOUTH's build (2026-10-12 13:31, lane 1)
- A case for this card, BF1 Kamino: a bot in the corner between the ramp's east side and the walkway's south rail at
  (149.55, 52.0, 155.12), 5.9 m from hub 13's centre, alternates `-1>13` / `13>16` / `51>13` in `recover` for the rest
  of the round (wall `kam_bldg_ramp1` / `kam_bldg_mushplatform1` at 0.0 m, slides 8 m east and comes back). Seen under
  MOUTH's corner rule ALONE (unit 26, seeds 61 / 64 / 67: `lead_tmp/mouth/proto_fe_all/v1/bf1_kam1/`,
  `python3 lead_tmp/mouth/build/unitrows.py <snap> 26 20 60`); with both landed rules no bot entered it in 8 seeds.
- MOUTH also fixed a source of false failed windows that this card's counts include on older mains: the progress
  window's tick count was carried over a hub hop (23-40 of 38-112 failed windows in 3 minutes of a round were 0-3
  ticks after a hop). Re-take this card's Dagobah numbers on main 273a441 or later before building on them.
- 2026-10-12 14:01 card RLOOP (build): **built on branch `rloop` (2d426f3 + 3aa13e2), not landed - the fidelity gate refuses `recycled`, and that is the rule's own cost.** Everything is in `engine/artifacts/lead_tmp/rloop/` (`sw/` the sweeps, `tools/loops.py` = `scripts/loop_spells.py` on the branch, `tools/snap.sh`, `bin/` main's and the branch's sweep binaries).
  - **Census, main 4ca302f (after DAGS), 16 seeds x 32 maps** (`sw/main16_loops.txt`): 117 spells of 20 s+ inside a 2 m box in recover / travel, 81 over 30 s, 5,392 s. Dagobah 0. BF2 Kamino 36 (33 of them ONE spawn node, see below), BF1 Endor 16, BF1 Yavin Temple 15, BF2 Geonosis 13, BF2 Kashyyyk 9; 17 maps in all.
  - **Why the ladder never ended it** (`snap/main_4ca302f/bf2_end1_s1_t7000_u20.txt`): unit 20 at (98.5, -0.6, -50.0), plans `-1 -> 12` / `12 -> 11` in turn every 2 s, `recover` 1..5 and then from 1 again - a new plan whose waypoint is nearer reads as progress, so the count never reaches the post's give-up (6) and the bot stands until the 120 s recycle. BF2 Kashyyyk (46, 28, -109), the planner's spot, is the same loop (`snap/.../bf2_kas2_s4_t5000_u9.txt`: legs `48 -> 47` / `-1 -> 48`, between `kas2_bldg_tree` and `kas2_bldg_platform_roof`; there the post IS given up, attack2 -> defend0, and the next one leads into the same wall).
  - **The rule** (`ai/stall.rs`): failed progress windows counted while the bot has not been 2 m (`PIN_ROAM`) from the first failure's place, through every plan; at 10 (`PIN_FAILS`) it is recycled; a bot off the level dies as lost. RETW is not on main, so it is the recycle. What the PS2 does with its stuck flag is unread: ours, PROVISIONAL.
  - **N from the census:** no value separates - 27 bots in 256 rounds walked away after 10 or more failures at one place (most 27). 8 seeds: bound 10 leaves 0 spells over 30 s, 12 leaves 5, 15 leaves 33; recycles 80 / 80 / 95 against main's 61. An earlier give-up of the post (at 5 failures) on top: 79 recycles, 4 spells over 30 s - not taken.
  - **After, 16 seeds** (`sw/f16_loops.txt`): 70 spells, 1 over 30 s (30.3 s, BF2 Kashyyyk), 1,652 s; rounds 512 / 512; deaths under the floor 115 -> 115; lost 39 -> 46; kills -0.3 %. **Recycles 109 -> 152** (BF2 Kashyyyk 12 -> 22, BF1 Yavin Temple 2 -> 10, BF1 Endor 5 -> 12; 14 maps up by 1 or more, 0 down). The acceptance line "recycles not up on any map" is NOT met and cannot be with this rule: a bot that stood two minutes and was recycled once is now recycled after 22 s, respawns, and some go back.
  - Gate: `lead_tmp/rloop/gates_fid/fidelity/fidelity.md` - WORSE `recycled` (all maps) 49 -> 80; BETTER `stand20_pct` (all maps) 0.19 -> 0.05, `stand20_pct@bf2_kas2` 1.51 -> 0.58. Sorted CAUSED (by design).
  - Main moved to d096ae7 (MOUTH) while this was measured: the numbers above are on 4ca302f; re-run `tools/sw.sh` on the rebased branch before landing.
  - Not done: the spell count in the fidelity numbers (a builder cannot edit gates.toml; the instrument is `scripts/loop_spells.py`).

## Planner 2026-10-12 14:22 - rebased, re-measured on today's main, and what it waits for
- The branch is rebased onto main d096ae7 (MOUTH on): `rloop` = 2dffd9d (the rule) + cbb6763 (the script). One conflict, the
  `core/src/ai.rs` / `ai/state.rs` counts in `engine/CRATES.md` (now 908 / 1372, as `size_report.sh --check-map` prints).
- `gates.py --groups fidelity --fidelity-both` on it, beside the baseline re-take of main d096ae7 made the same quarter
  hour (`lead_tmp/rloop/gates_fid_rebased/`, `lead_tmp/fidelity_rebase_20261012c/gates/`;
  `python3 engine/artifacts/lead_tmp/planner_1012c_fid.py main=<dir>/fidelity rloop=<dir>/fidelity`):
  | all maps | main 1-8 | rloop 1-8 | main 9-16 | rloop 9-16 |
  |---|---|---|---|---|
  | recycled | 30 | 46 | 44 | 58 |
  | stand20_pct | 0.10 | 0.04 | 0.11 | 0.06 |
  | noprog | 15 | 16 | 8 | 7 |
  | finished_pct | 100 | 100 | 100 | 100 |
  Per map, seeds 1-8: `stand20_pct` BF2 Kashyyyk 1.33 -> 0.54, BF1 Endor 0.68 -> 0.23, BF2 Kamino 0.39 -> 0.05; `recycled`
  BF2 Kashyyyk 5 -> 11, BF1 Endor 3 -> 8, BF1 Yavin Temple 0 -> 3, BF2 Yavin 0 -> 2. Hoth's rows equal main's (the rule
  never fires there). The gate's tolerance on `recycled` (all maps) is 12: both blocks are over it, by 4 and by 2.
- So MOUTH's hop fix made the trade smaller (16 + 14 more recycles in 512 rounds; it was 43) but did not remove it. The
  refusal stays CAUSED by design. STATUS question 36 puts it to the maintainer.
- **The default, taken without waiting:** `deps: RETW`. The card's own rule says a given-up bot "takes RETW's floor
  path back when RETW is on main, else is recycled" - with RETW on main the session re-works the give-up that way and
  recycles only a bot no floor path leads out for. The maintainer's yes before that lands the branch as it is.
- The measured binary: `lead_tmp/rloop/bin/rebased_cbb6763/round_to_outcome` (needs the worktree `openbf-wt/rloop`).
- Undo of the status change: `loopctl.py set RLOOP status=blocked deps=`.
- The census on the same four runs (`lead_tmp/rloop/tools/loops.py <dir>/botq`, 16 seeds x 32 maps): main d096ae7 86
  spells, 69 over 30 s, 4,071 s; the rebased branch 54 spells, **5 over 30 s** (4 on seeds 1-8, 1 on 9-16), 1,329 s.
  Before the rebase it was 1 over 30 s: with MOUTH's hop fix fewer windows fail, so ten failures take longer. The bar
  "no spell over 30 s" is no longer met at `PIN_FAILS` 10 - re-read it when the card is re-worked.
- 2026-10-12 17:05 planner, a number to watch, not traced: after CLSE2 (main 5fbbbd4) BF2 Kashyyyk's `stand20_pct`
  reads 1.95 / 1.80 on seeds 1-8 / 9-16 against 1.33 / 1.24 on main d096ae7 (inside the gate's 0.7; up on both
  blocks); all maps 0.10 / 0.11 -> 0.14 / 0.15. This card's rule took Kashyyyk to 0.54. `lead_tmp/fidelity_rebase_20261012d/`.

- Planner 2026-10-12 19:30: the Kashyyyk number the 17:05 note asked to watch is back down - `stand20_pct@bf2_kas2`
  0.47 / 0.87 on main bc10b82 (1.95 / 1.80 on 5fbbbd4, 1.33 / 1.24 before CLSE2); not traced, DEFS and CLSE4 landed
  between. All maps: stand20 0.08 / 0.10 % (0.14 / 0.15), last-resort respawns 48 / 46 (40 / 48).

- 2026-10-12 20:27 card DOORS (measure): a second site for this card, BF2 Polis Massa at (5..15, 22.4, 45.4) under
  the hall by `pol1_prop_door1`: a bot on the floor BELOW its hub (feet y 22.4, the door's floor 25.0) walks the
  hall's wall east and west on legs `2 > 25` / `-1 > 2`, `recover` 17 -> 23 in 20 s, `lost 0`. Three unit-lives hold 19
  of the door's 20 stalls in 8 seeds x 3 min (seeds 60, 62, 65: units 26, 22, 26;
  `lead_tmp/doors/snap/bf2_pol1_main/bf2_pol1_s62_t5400.json` unit 22 from t 130).

- 2026-10-15 17:15 (card RETW, lane 1): **BF2 Utapau's walled pit at (-187, 9.4, 128-140)**, between `uta1_bldg_lizardpen`
  (body 262) and `uta1_bldg_turtleshell` (body 290), 21 m under hub 60 (`locus N60 floor_dy -21`): a bot there walks its
  leg `-1->60` at (-163.8, 10.4, 129.0) through the wall, the level test flips it lost / not lost every few seconds,
  `recover` counts 18-24, and the last resort recycles it. MAIN does it (5751bc5 seed 42 unit 3,
  `lead_tmp/retw/bbm/bf2_uta1_s42.json`; recycles there 2 in 72 seeds); RETW's branch more often (2 -> 11 over 88
  seeds, `lead_tmp/retw/runs/m72` vs `v572`, `bb4/bf2_uta1_s14.json` unit 10): its floor walk ends at a cell 1.6 m
  away that is on the level and the leg drags the bot back in. Lives in the pit 20 s+ over seeds 1-40: 18 main, 21
  branch (`lead_tmp/retw/tools/pit.py bq/m bq/v`). This card's give-up is what ends it; the leg through the wall is a
  locate / route fault (hub 60 is 21 m overhead) worth naming when this card traces it.

## Build 2026-10-15 18:30 (lane 1) - re-worked on RETW, branch `rloop` = 1c338f6 + b089b53 + 805ce11 on main d5ae05f
- At `PIN_FAILS` (10) failed windows at one place the bot is no longer recycled outright: `stall::send_off` asks RETW's floor
  search for a walk from its feet to its leg's waypoint (`Sim::floor_walk_to`) and the bot walks it as a `LostState { pinned }`
  spell; recycled only with no walk, a walk window with no progress, or a second pin within `PIN_WALKED` (4 m) of the last walk.
- 16 seeds x 32 maps (`lead_tmp/rloop/sw/m16` main d5ae05f, `sw/v16` this; `tools/loops.py`): spells 30 -> 27, **over 30 s
  12 -> 2** (longest 76 -> 34 s, both BF2 Kashyyyk's pen (46, 28, -109), 11.5 m under hub 48), seconds 1,391 -> 671; recycles
  71 -> 82 (was +43 with the plain recycle), Kashyyyk 3 -> 7, Mygeeto 0 -> 3 (at (-96, 9, 174), main's 52 s stand spot),
  rounds 512 / 512, kills -0.02 %, world deaths 1,449 -> 1,452. PIN_FAILS 8 (`sw/v16n8`): over 30 s 1, recycles 94 - not taken.
- Gates `lead_tmp/rloop/gates_v2/SUMMARY.md`: fidelity PASS (0 worse, 0 better; recycled 33 on seeds 1-8), moved = ci count,
  sweep, arena_tat2 (declared).
- Acceptance left open: "no spell over 30 s" - 2 left at 34 s (Kashyyyk pen); "recycles not up on any map" - not met (6 maps
  up by 1-4: the trade). The spell count in the fidelity numbers is the planner's (`scripts/loop_spells.py`).
