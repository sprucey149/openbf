---
id: OBST2
title: The rest of the bot design's avoidance after OBST: barriers as a keep-out walked by their corners every step, parked vehicles' and turrets' footprints, soldiers and moving vehicles in velocity space (8 bots each way through a 3 m corridor), the probe only off checked ground (cost halved, B13), the black box's unit row
milestone: M3
value: 5
phase: build
status: done
deps: OBST,OBSF
files: engine/crates/core/src/ai/avoid.rs,engine/crates/core/src/ai/mover.rs,engine/crates/core/src/ai/steer.rs,engine/crates/core/src/ai/route.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/nav.rs,engine/crates/core/src/collision/shape.rs,engine/crates/core/src/collision/walls.rs,engine/crates/core/src/sim/state.rs
declares: sim; sweep; arena
needs:
effort: high
branch:
resume: CLOSED by the planner 2026-10-12 14:22 after its third landing (7dfb448 the corner search, 600fac7 + 996313a the black box's row, fe043b4 + 03422f5 the cost cuts and the closed-form cylinder ray). What was left went: item 2 (footprints of parked vehicles and turrets) and the shape.rs test cases -> card OBSV; B13's cost bar (0.187 ms a tick for 0.123) -> a note on PERF; item 3 (velocity space) not built - the corridor scene passes on main, a looks question; B10's residue and variant h1 measured and not taken. Undo: loopctl.py set OBST2 status=todo. Binaries and tools: lead_tmp/obst2/README.md
created: 2026-10-07
alpha: yes
---
## Why
Card OBST (landed e9ce6db, 2026-10-07) built the part of `analysis/bot-design.md` 4.7 that its census named: a bot goes round a body on ONE side (a commitment), turns an inside corner along the next wall, leaves the obstacle only when the free straight way ends nearer its aim point than the walk has been, and walks by a small body before it touches it (`core/src/ai/avoid.rs`). Stalled 10 s windows 2.44 % -> 1.13 %, a bot at a map body 2,832 -> 808, recycles 105 -> 64 over 256 rounds (report `loop/reports/20261007_*_card_OBST.md`). The design's other steps were NOT built, and three of its bars are not met.

## What is left (each behind a switch in one scratch binary, the null variant = main byte for byte, 24 seeds before believing a gain - cards MOVR and OBST both found 8 seeds too noisy for recycles: 49 - 73 between like variants)
1. **Barriers every step** (design 4.7 item 3; SAIR 2): the segment to the aim point against the soldier barriers grown by the body radius + 0.5 m on a grid, aim at the silhouette corner on the committed side, never walk into one, leave one by the nearest edge, refuse a goal in one. The port's bots stand inside a barrier 1.1-3.9 % of their time (PS2 0.3-1.2 %, `lead_tmp/botd/tools/inbarr.py` - not re-measured after OBST: do that first). `nav::detour` allocates: a fixed-array form is needed for a per-pick call.
2. **Parked vehicles' and turrets' footprints** as rectangles passed by their corners. Today a hull over 3 m wide is met and gone round by rule 1-3 (test: under 1 s of sliding at a 2.4 x 4.8 m box); `com_weap_gunturret` is still the small body with the most contact time.
3. **Soldiers and moving vehicles in velocity space** (15 candidates every 3rd tick, Detour's weights), and the scene test: 8 bots each way through a 3 m corridor pass without a stall (mutation-checked). OBST left soldiers to the separation pass (`mate` stalls 110 -> 73 of 259,164 windows).
4. **The probe's cost (B13)**: unchanged by OBST - 14.3 s against 14.3 s of CPU for 10 min of BF2 KAM1 + MYG1 + POL1 + BF1 KAS2 single-threaded. The early turn's 3 m sweep runs on every pick with nothing being gone round; the design's probe is every 5th tick and only off checked ground.
5. **The black box's unit row**: `Behaviour` (card MOVR asked) and the commitment (`MoverState::round`: live, the nearest, the allowance) are not in `game::unit_probe::Probe`; `Steer::Round` is.

## Measured by OBST and not landed (patches and tables in `lead_tmp/obst/`; base main 9cd9b52, e9ce6db = OBST)
- `fix2_cumulative.patch`: what a bot goes along counts as beside it when a step that way is shut by ANYTHING (a steep floor, a drop, a rim barrier), not only by a body's sweep. Cures a 4 s jitter along a steep face on Kamino (scene worst 17 -> 9 reversals); over 24 seeds rounds 767 / 768 (a 6 v 6 stand-off on BF1 GEO1), recycles 169 -> 180, no-progress at a round's end 41 -> 69. Not taken: no gain outside the scene.
- `fix1.patch`: the early turn only for a body between the bot and its aim point. Scene trails over 6 reversals 4 -> 1; recycles 169 -> 224 over 24 seeds. Not taken as written (card OBSF has what was).
- Leaning off a grazed wall alongside: no gain (1.15 % against 1.13 % of windows), removed.

## Residual list (OBST's final census, `lead_tmp/obst/ab/final/`: bodylegs.txt, spots.md)
- Legs with the most body stalls left: BF2 DAG1 2 -> 9 at `dag1_prop_grandtree_8` (the hub 16 m over the bot: LLEG), BF1 NAB1 2 -> 6 at `nab1_prop_ruins_column2` (53 m off the leg's line), BF1 BES1 122 -> 7 at `main building 02`, BF2 GEO1 9 -> 12 / 13 (290 -> 51).
- BF1 BES1 deaths under the floor 284 -> 333 over 8 seeds with its recycles 15 -> 0: bots that used to stall there walk on into the map's floor leak.
- A penned bot gives its post up 10 s later than under MOVR (the going-round allowance), and its six failed windows need no longer be in a row (`a_blocked_walk_fails_recovers_and_gives_its_post_up`).
- B2 still missed: KAM1 stay path 3.5 m (bar 3.0), KAS2 13.4 m / 2.42 reversals (bars 3.0 / 0.72).

## Session 2026-10-09 (lane 2) - item 5 landed; item 1 (barriers) built, measured, NOT landed
Everything is in `engine/artifacts/lead_tmp/obst2/` (README.md there: tools, patches, tables). Base = main eec32d1.
**Baselines re-taken on main (the card's numbers were OBST's):** 24 seeds x 32 maps: rounds 768 / 768, recycles 198
(OBSR's watch raised them from 169 by design - hold a variant to 198, not 169), deaths under the floor 881 (BES1 848),
region 188, lost 36. Stalled 10 s windows 2,524 of 245,945 (1.03 %). Bot time inside a soldier barrier (bare
rectangle; `tools/inbarr2.py`): KAM1 2.19, POL1 2.22, DEA1 2.12, KAS2 4.09 % (PS2 0.80 / 0.32 / 1.16 / 0.26).
The maintainer's Kamino scene (`tools/scene_seeds.sh`): 3 of 419 trails over 6 reversals (seeds 60-67), 5 of 869 (68-83).
**Built (patches `obst2_scratch_knobs_k5.patch`, `obst2_barrier_step_f7.patch`; apply on main 996313a):** the PS2's walk step
(`FUN_00250fc8`, RE_LOG "Barriers are the PS2's Obstacles") as `NavGraph::step_aim` - the step (1.5 s of top speed)
toward the point walked at tested against the barriers grown by the capsule radius, the first one crossed turned to
its silhouette corner (radius + 0.5 m) on a side kept per aim point, up to 5 times, no allocation - called from
`avoid::pick`; a leg the corner search could not resolve (`Route::unresolved`), a straight-line fallback leg, a leg
the stuck window re-planned, a lost bot's way back and a flight are exempt. Three more rules behind switches.
| variant (8 seeds unless said) | recycles | floor | region | kills | in a barrier, 4 maps | the maintainer's scene |
|---|---|---|---|---|---|---|
| main | 60 (24 seeds 198) | 283 (881) | 39 (188) | 58,786 | 2.72 % | 3 of 419 |
| step rule | 64 (196) | 97 (299) | 7 (40) | -2.1 % | 1.59 % | not run |
| + no avoidance step into a barrier | 70 (194) | 81 (211) | 9 (22) | -1.7 % | 1.57 % | - |
| + leave a barrier by its nearest edge | 80 (260) | 56 | 18 | -0.9 % | 1.35 % | - |
| step + no-step-into, barriers holding an end of the leg skipped (`fin`, cleaned) | 43 (190) | 55 (171) | 15 (42) | -1.9 % (t -2.7) | 1.18 % (KAM1 1.45, POL1 1.36, DEA1 1.38, KAS2 0.60) | **19 of 405** |
| ... + hub points drawn clear by 0.5 m | 69 | 77 | 22 | -2.7 % (t -5.2) | 1.17 % (older base) | - |
| rules also on a re-planned leg | 397 / 346 | - | - | - | - | - |
| step rule only, + barriers the LEG'S LINE crosses skipped, corners picked on the bare rectangle, give up after the tries | 83 | 238 | 59 | -1.9 % | 2.32 % | 7 of 375; 28 of 798 |
| ... one try | 79 | 297 | 65 | -3.1 % | 2.60 % | 5 of 383; 20 of 793 |
| ... a corner at most 60 deg off the wanted heading | 79 | 268 | 63 | -0.9 % | - | 9 of 829 (main 6 of 848) |
`fin`'s stalled windows: 2,860 of 247,621 (1.15 %; main 1.03 %, the "open" class 771 -> 1,289). All rounds finish in
every variant listed.
**Why it is not landed.** (1) the maintainer's scene gets worse with every form that keeps the gains (19-33 trails over 6
reversals for 3): BF2 Kamino's pods are ringed by four barriers with 1.6 m gaps at the corners, hub 46 (radius 15 m)
has its centre inside the ring, and a leg leaving it has its line through a barrier - the bot is turned at a corner,
then at the next barrier's, back and forth; the no-step-into rule also refuses the wall-follow along the pod (the
barriers lie on its walls). (2) Skipping the barriers a leg's own line crosses cures (1) to 5-9 trails and loses
the gains (floor 238-297, in a barrier 2.3-2.6 %). So [H, not isolated - the last three rows changed three things
at once] the gain comes from barriers ON the legs' own lines: `route::leg_point` (card MOVR2) pulls a bot onto the
line `Route::from` -> waypoint, while the corner walk (`fill_detour`) was planned from where the bot STOOD - the line
walked was never tested. Bespin's rim (floor deaths 848 -> 176), BF2 Geonosis's regions (101 -> 6) and BF1
Tatooine's gap (recycles 48 -> 3) are where it shows.
**Also found:** BF2 Utapau's landing (y 14.4): hub 61's table hop (arc 79 to hub 64, then the ONE_WAY arc 64 -> 60
down) is rejected in `route::plan_leg` (no floor on the level / `arc_unwalkable`), the bot gets a straight-line leg
at the post 199 m off and gets down only over the rim barrier or by leg 12 -> 62's step at (-159, 111) (66 of 194
descents in 3 rounds); a barrier rule without the leg-end skip cut that to 18 and Utapau's kills by 15-28 %. It is
card RETW's "21 under the y 14.4 landing" from the other side. `tools/drops.py`, `tools/cells.py`.
**Not done:** items 2 (footprints), 3 (velocity space, the corridor test), 4 (B13 - not measured: the machine was at
load 140 from another lane's sweeps all session, a per-tick CPU number would not be worth quoting).

## Session 2026-10-09 (lane 1, evening) - item 1's CAUSE found and landed 7dfb448: the corner search itself
Everything is in `engine/artifacts/lead_tmp/obst2/` (README.md, "Second session"). Report `loop/reports/20261009_1725_card_OBST2.md`.
**The census the last session asked for** (`tools/legcen.sh` + `legcen.py`: every tick a bot stands in a soldier barrier,
classed by its leg; every new leg tested on its own line and from where the bot stood; 32 maps x 2 seeds): 27 % of
the in-barrier ticks (34 % of the entries) were on a barrier the leg's own line crosses - and 22,268 of 68,186 legs
that cross a barrier from where the bot stood had NO corner walk: `nav::detour` took the first 24 barriers of the
band in index order, the discs hold 30-60 a band at the median (up to 421), the crossed one was not among them and
the search answered "the straight leg is clear". The step rule's whole gain was this bug.
**Landed:** `nav::detour` is the PS2's own lazy search (BF1 `FUN_002691a0`: the silhouette corners of the barriers a
segment really crosses, one level deeper per crossing; RE_LOG "The corner search, read again", one verifier, 5 of 5
claims); a corner walk over the leg's six corners is continued at its last one. Constants unchanged.
| 24 seeds x 32 maps, base 177140d | recycles | under the floor | death region | lost | kills |
|---|---|---|---|---|---|
| main | 202 | 891 | 164 | 31 | 174,640 |
| landed | 160 | 152 | 53 | 60 | 174,288 |
Rounds 768 / 768 both. Bespin's floor deaths 858 -> 124, BF2 Geonosis's region deaths 101 -> 16, Polis Massa's recycles
28 -> 0, Dagobah's 27 -> 9; worse: BF1 Tatooine's recycles 46 -> 62, BF2 Kamino's lost 6 -> 50 (below). In a barrier,
8 seeds: KAM1 2.34 -> 1.98, POL1 2.21 -> 1.87, DEA1 2.17 -> 1.86, KAS2 4.09 -> 0.32 % (the PS2 0.80 / 0.32 / 1.16 / 0.26)
- the four together 2.75 -> 1.46 %. Stalled 10 s windows 1.05 -> 0.70 % (2,569 of 244,368 -> 1,648 of 233,989; at a map
body 780 -> 378). The maintainer's Kamino scene, trails over 6 reversals: 2 of 387 and 3 of 396 (main 4 of 422 and 5 of 415);
on the base it landed on (a4df1b5, after card CLSE) 5 of 421 and 2 of 379 (main 6 of 408 and 4 of 417). CPU on the
four B13 maps 0.281 -> 0.274 ms a tick (a search is 15 segment tests at the mean, 507 at most).
**Measured and NOT taken** (8 seeds unless said; `ab/<name>`):
| variant | recycles | floor | region | lost | in a barrier | the maintainer's scene |
|---|---|---|---|---|---|---|
| main (`e_old`, `e_old_b`) | 66 (24: 202) | 309 | 45 | 10 | 2.75 % | 4 of 422 |
| corners asked of the BARE rectangle (`e_new`) | 81 | 146 | 25 | 31 | 2.28 % | 3 of 416 |
| the landed search without the continued walk (`f10`) | 62 | 55 | 29 | 30 | 1.36 % | 3 of 376 |
| ... the PS2's test radius, capsule alone (`f05`) | 47 | 56 | 42 | 32 | 1.68 % | **13 of 396** |
| **landed** (`g1`, `g1_b`) | 48 (160) | 54 | 17 | 23 | 1.46 % | 2 of 387 |
| + behind a line's start the start is walked at (`g17`, `g17_b`) | 54 (167) | 56 | 16 | 14 | 1.54 % | 2 of 374 |
| the PS2's own corner pick + no arrival plane while a corner is ahead (`h1`, `h1_b`) | 35 (**130**) | 61 | 24 | 29 | 1.34 % | **11 of 394, 8 of 373** |
| an unresolved search goes to the hub's centre first (4 maps x 24 seeds, `kam_u1`) | 43 -> 78 | | | KAM1 45 -> 15 | | |
`h1` is worth a second look: 30 fewer recycles over 24 seeds, and worse in the maintainer's scene (the pick alone 7 of 416).
**The Kamino casualty is card RETW's pad:** bots that spawn on cp3's west pod pad (-100.6 / -103.3, 61.3, 317.5) are
LOST from their first second (the pad is off every hub's floor), walk at hub 54's centre and leave only by the lost
bot's JUMP over the gap at x -93; one that meets the kerb at z 317.8 (a drop ahead: `steer ledge`) never jumps and
dies there after 45 s, one that meets it at z 318.3 (a wall) slides, jumps and is on its way. Where it meets the
kerb is set by its first ten ticks' heading, before it is confirmed lost: main's (bogus) first corner lay to the
north-east. 2 seeds: 4 of 13 pad spawns die (main 1 of 10); 24 seeds 50 lost deaths for 6. The search's "no way"
is right there: the pad is outside the ring of rail barriers, `barrier867` shuts the 9 m between `barrier672` / `673`.
**Left on this card:** items 2 (footprints), 3 (velocity space, the corridor scene), 4 (B13: halve the probe's cost).
B10's bar (at most the PS2's time in a barrier) is met on none of the four maps; what is left by class
(`legcen.py legcen/k1`): a leg the stuck window re-planned 15 %, off a leg (hold, fight) 15 %, the leg's own line
13 + 6 %, the barrier holds the waypoint 22 % (on BF2 Kamino 6,142 of 12,297 ticks - 2,195 of them one bot in
barrier 389, 1,776 one in barrier 595: a waypoint drawn in a barrier's margin and a bot standing there), a corner
walk under way 8 %, no relation 11 %.

## Session 2026-10-09 (lane 1, night) - item 4's first half built on branch `obst2` 45c40cf, PARKED ungated (STOP)
Everything is in `engine/artifacts/lead_tmp/obst2/` (README.md, "Third session"). Report `loop/reports/20261009_1840_card_OBST2.md`.
**Where the cost was** (`tools/sample.sh` + `prof_incl.py` / `prof_callers.py`, 621 stacks inside `Sim::step` on the
four maps, main 7dfb448): `think` 68 %, `avoid::pick` 35 % (KAM1 46, MYG1 35, POL1 30, KAS2 17), every floor ray
together 43 %. Inside the pick: the steep-floor probe's ground query 15 points (it asks the SOLDIER's foot from a
metre over the floor, so the ring of eight more rays ran on every pick), wall sweeps 13 (the 3 m look-ahead most
of it), the ledge probe 4. Across everything: parry's ray at a cylinder (GJK) 15 %; `level_at` in `think` 6-10 %.
**Built, each behind a switch first (`obst2_cost_knobs_p3.patch`, `OBST2_P=<bits>`; no switch = main's seed-42 rows
byte for byte; cost one run at a time, x main):**
| switch | x main (4 seeds) | 8 seeds x 32 maps: recycles (main 54) | taken |
|---|---|---|---|
| 1 the steep probe asks one ray first, the ring only of a steep hit | 0.84 | 70 | yes |
| 2 a ray at a cylinder in closed form | 0.84 | 51 | yes |
| 8 the look-ahead sweeps only bodies no wider than `SMALL_BODY` | 0.93 | 54 | yes |
| 1 + 2 + 8 (the branch) | 0.76 (6 seeds) | 56 (24 seeds: 157 for 160) | - |
| 4 the 3 m sweep's answer reused for the 1 m probe | 0.96 | - | no: not the same answer (a block at t 0 comes from the END pose) and no gain |
| 16 the level test on every third think | 0.90 | 46 | no: 2 s to notice a lost bot for 0.67, two tests fail; the exact form is an ask cache by position |
| 32 no penetration test for a body the cast never meets | 1.00 on top of 27 | - | no: 11 of 36 rows move (a capsule touching a wall and walking a hair into it is a miss to the cast) and nothing saved |
| 1 + 2 + 8 + 16 | 0.67-0.70 | 48 (24 seeds: 152) | - |
**Not noise-proof at 8 seeds:** BF1 Endor's stalled windows read 72 -> 125 on seeds 1-8 (every single switch
121-152) and 368 -> 171 on seeds 9-24; Bespin 186 -> 241 and 358 -> 293. Over 24 seeds the two maps 984 -> 830.
**Item 3's scene, measured on main:** eight friends each way through a 3 m corridor (walks to held points, two
abreast, head on): all 16 at their points, the last at tick 106 against 104 with the walls 30 m apart, 0 blocked
picks - the separation pass shoulders them by. At 2 m none arrives (shoved onto the walls, a holder gives its walk
up). So velocity space would change how the passing LOOKS, not whether it stalls; the test is on the branch.
**Left:** B13's second half (resume), items 2 and 3, B10's residue.

## Session 2026-10-12 (lane 1, land) - REFUSED at the branch gate; fidelity = NOISE, leak and a stats golden now declared; BLOCKED for the planner's baseline
Report `loop/reports/20261012_1105_land_OBST2.md`. Everything is in `lead_tmp/obst2/land_1012_refused/`. land.py rebased the
branch onto main 3d45fe3 (now `obst2` d9bf17a, no conflict) and refused three moves; main was not touched.
**1. Fidelity `stand20_pct@bf2_dag1` 1.08 -> 2.28: NOISE.** The baseline is stale twice over (seeds 1-8 = main 2d1fb60, the
confirm block = main 8ad30f4; GRAIA has landed since). On the gate's own measure, main e931532's tree against the branch
(`dag/blocks_40seeds.txt`; main's binary null-checked against GRAIA's seed-1 rows):
| 8-seed block | 1-8 | 9-16 | 17-24 | 25-32 | 33-40 | 40 seeds |
|---|---|---|---|---|---|---|
| main | 1.19 | 0.88 | 0.36 | 1.70 | 1.31 | 1.09 (sd 1.19 a seed) |
| branch | 2.28 | 1.23 | 1.41 | 0.14 | 0.46 | 1.10 (paired t 0.05) |
BF1 Endor (worse on 9-16 only, 0.23 -> 1.55): 40 seeds 0.45 -> 0.67, t 1.03, main's own blocks 0.10-0.99. All 32 maps,
16 seeds (`agg_vs_main.txt`): rounds 512 / 512 both, recycled 109 -> 99, stand20 0.20 / 0.15 -> 0.21 / 0.17. With a
baseline re-taken on today's main the gate drops it (9-16 worse by 0.35 for the tolerance 0.7).
**2. The leak census moves on 27 of 32 maps - declared (`leak`), it is the closed-form ray.** Totals: starts 122,972 ->
122,788, closed 29,227 -> 28,668, ceiling 44,971 -> 44,714, tunnel 1,229 -> 1,217, float 129 -> 124, push 532 -> 531,
floor 4,699 -> 4,714, sites 14,365 -> 13,973. 21 map x class rises, 156 runs (`rises.py`): closed +107 on 12 maps (BES1
+31, YAV1 +23, BF2 YAV1 +17), ceiling +29 (BF1 KAM1 +12, BF2 KAS2 +9, MYG1 +5, UTA1 +3), floor +18 (BF1 KAM1 +9, BF2
MYG1 +7, BES2 +1, GEO1 +1), tunnel +2 (BF2 DEA1). Why it is not a new leak: (a) main's ray at a cylinder (parry's
support-map cast) is wrong and the branch's is not - 160,000 rays of four kinds against an oracle that knows only the
inside test (`raychk-scratch/out.txt`): parry 5 misses (a ray down from inside a cylinder finds no bottom), 20 hits on
rays that pass 2 mm - 2 cm outside, 218 hits over 2 mm off (worst 2.3 cm); the closed form 0 / 0 / 0, worst 0.15 mm.
The build report's "under 1e-4 m" was not measured on such rays. (b) The census counts `closed` and filters its starts
with that same ray (`crossings`: each hit + 1 mm; a hit a few mm early is counted twice). Traced on BF1 Kamino at body
178 (`leak/near_{main,branch}.err`): the same start, heading and end position to the centimetre is `closed` on main and
`ok` on the branch; Kamino's 9 new `floor` runs come from 3 starts main skipped, at sites main already has (floor
sites 176 both; Mygeeto swaps 2 for 2).
**3. `goldens/walkabout_stats.ron` - declared.** 7 panels (BF1 / BF2 Kamino 3, Mygeeto 3, Tantive IV 1) move by 0.01-0.02
of 255 in a mean or deviation (`walkabout_stats.diff`; the test's tolerance is 3.0). No picture golden moved.
**The build's cost number reproduced** on today's main and the rebased branch (`perf/land_full_j1`, `land_3k_j1`; 6
seeds, one run at a time): whole rounds 0.239 -> 0.173 ms a tick (x 0.73, per-seed sd 0.075; main's twin x 0.997);
first 3,000 ticks 0.247 -> 0.184 (x 0.75; the twin read x 1.09 there, so that run's null is poor). Binaries
`bin/main_1012`, `bin/land_1012` (the branch's target was removed at blocking).

## Planner 2026-10-07 08:55 - value 4 -> 5
BOTV waits on this card, and at 4 the dry run (`lead_tmp/planner_1007b_dry.py`) did not reach it in 44 picks (the M3
build fours come after every M2, M0 and M1 four), which held BOTV, PERF and UCNT behind them. It is also the half of
The maintainer's 2026-10-02 ask that OBST left out, in his words: "know about ... vehicles, turrets and other bots on the way and
go around obstacles". Undo: `loopctl.py set OBST2 value=4`.

## Session 2026-10-12 (lane 2, land, 11:30-12:15) - LANDED fe043b4 + 03422f5 (item 4's first half, the closed-form ray, the corridor test); card RE-OPENED
Report `loop/reports/20261012_1215_land_OBST2.md`. Outputs `lead_tmp/obst2/land_fe043b4/` (gates), `land_1012b/` (my checks).
The rebase onto main 19708fa (REACT) conflicted in ONE line, `engine/CRATES.md`'s `core` total (both sides had changed
54,360); resolved by hand in the worktree with the count `size_report.sh` prints for the rebased tree (55,971), no source
line touched. Every gate then showed only the declared moves; fidelity PASS against the planner's 11:25 baseline (0 worse,
1 better: `noprog@bf2_geo1` 10 -> 1; Dagobah's `stand20_pct` did not refuse).
**Checked before `--commit`** (today's main 19708fa against the rebased branch, both binaries null-checked on two maps):
- 8 seeds x 32 maps, the gate's own sweeps (`land_1012b/agg_vs_main.txt`): rounds 256 / 256 both, recycled 49 -> 52,
  noprog 16 -> 7, stand20 0.19 -> 0.18 %, deaths in the world 1.24 -> 1.21 %, rev6 1.69 -> 1.68 %.
- Cost, B13's four maps, one run at a time, 6 seeds, TWICE (the other lane's sweeps ran beside both: main's twin read
  x 1.00 sd 0.25 and x 1.07 sd 0.20, so neither run alone is quotable): branch x 0.67 and x 0.71 of main; best reading
  per seed over both runs 0.258 -> 0.187 ms a tick, x 0.72 (per-seed sd 0.07; `land_1012b/perf_best_of.txt`). The clean
  run of this morning against main e931532 was 0.239 -> 0.173, x 0.73 with a twin at 0.997. B13's bar 0.123: NOT met.
- The leak census on main is byte-equal to the branch's of the 10-12 refusal, whose 21 map x class rises (156 runs) that
  session traced to the exact ray (`land_1012b/leak_rises.txt`); `walkabout_stats.ron`'s diff is line for line the one it
  read (0.01-0.02 of 255, 7 panels); no picture golden moved.
**After the commit:** the maintainer's Kamino scene, trails over 6 reversals, seeds 60-67 and 68-75 (`land_1012b/scene_*.txt`,
`tools/scene_seeds_main.sh`): main 19708fa 8 of 428 and 4 of 427, the landed code 6 of 416 and 5 of 396 - 12 of 855 against
11 of 812, no change. The build session's "0 of 409 and 1 of 392 for 5 of 421 and 2 of 379" (base 7dfb448) is NOT seen as
a gain on today's main; the scene is not worse either.
NOT re-run by me: the 24-seed recycle table on today's main (8 seeds above), the mutations.
A trap for the next session: a sweep binary kept from a worktree's target has the worktree's path compiled in and finds
no baked maps once land.py has removed the worktree - rebuild `round_to_outcome` in the main checkout (18 s) and null-check it.
