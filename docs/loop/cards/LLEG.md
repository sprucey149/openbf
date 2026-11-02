---
id: LLEG
title: A last leg finds the ramp or the doorway: a local floor search when the straight line at a post or a quarry sticks (BF2 GEO1's raised cp6 and walled cp7, BF1 GEO1 hub 43, TAT1's one-hub dune sea)
milestone: M3
value: 5
phase: build
status: done
deps: STARC,PARR,BOTD,OBST
files: engine/crates/core/src/sim/spawn.rs,engine/crates/core/src/sim/state.rs,engine/crates/core/src/sim/round.rs,engine/crates/game/examples/round_to_outcome.rs
declares: sweep
needs:
effort:
branch: lleg
resume: landed c0794ebf 2026-10-07
created: 2026-10-02
alpha: yes
---
## Why
A LAST leg is a straight line at the post (or at the quarry) from wherever the bot stands in the post's hub. Where
the point is on a platform with one ramp, or inside a building with one doorway, the straight line meets a wall and
the slide never finds the way in. Found by STLVL's census (2026-10-02, `lead_tmp/stlvl/`): after STLVL this is the
largest recycle source left, and the only map that still loses a round in 8 seeds.

## Evidence (STLVL branch, 8 seeds x 120,000 ticks; `lead_tmp/stlvl/c1_s1_8`, `c1_s9_16`, tool `tools/lv.py`)
- **BF2 GEO1 hub 18 / cp6** (post at (49.5, -58.7, 299.3) on a 9 x 6 m platform 4.3 m over the ground; ONE ramp, on
  its south side, x 48..51, z 287..297; hub 18 has r 28.7 and no finer graph): 18-40 recycles per 8 seeds,
  `last->18 dest 18`, steer `slide` from the ground and `fwd` from the platform (hunters going down). Seed 15: a
  1 v 2 endgame hangs there to tick 120,000 (93 recycles).
- **BF2 GEO1 hub 9 / cp7** (post at (-84.3, -62.7, 269.4) in the open court of a ring building, roof at -49.8 over
  x -94..-72, z 260..278; spawn nodes inside): 40-70 recycles per 8 seeds, `last->9 dest 9`, `slide`, 11-16 m from
  the post, all outside the wall. Hub 16 / cp1 the same shape (6-8).
- BF2 GEO1 finishes 14 of 16 seeds before and after STLVL; its recycles are 184 (main) and 285 (STLVL) over 16 seeds,
  set by which rounds hang.
- BF1 GEO1 hub 43 (66, -63, 105) `last->43`, 6-10 per 8 seeds; TAT1 hub 104 (r 525 m: the whole dune sea is one hub,
  so a route there is one straight last leg) 20-45 per 8 seeds at (-114, 15, 0) and (-228, 12, 24).
- Picture a spot: `FLOOR_GRID=50,295,13,1,-50 STUCK_CENSUS=1 round_to_outcome 1 1 bf2_geo1` (grep `^fg`).

## Design (the port's own; check first what the PS2 does - `analysis/re-round-stalls.md`, RE_LOG "Arrival and locate")
- When a last leg's stuck window fires, search a local path: a breadth-first flood over a 0.75 m floor grid from the
  feet (one floor ray per cell, a chest-high ray between cells: `lead_tmp/lvlf/tools/lvlf_obs.rs` `flood` is this
  search, read-only), bounded (the hub's radius, a cell cap), to the cell nearest the leg's point.
- Pull the path into corners and write them into the route's corner walk (`Route::detour`, `DETOUR_MAX`), which the
  follower already walks.
- STARC's second half ("a last leg that starts where the capture centre is in reach": NAB1 ruins column, MYG1 spiral
  ruin) is this card's case: do it here and leave STARC the arc-line half.

## Note from STARC (2026-10-02, landed c9d2776) - before building, dump the walls at each spot
- STARC's POL1 radar door was a COLLISION defect (a doorway the bake filled: `lead_tmp/starc/README.md`), not routing;
  it is fixed in the bake and POL1's recycles went 41 -> 5 over 9 seeds. So for each spot this card owns, check first
  that the way is open: trace one unit, dump the walls round it, and count the PS2's soldiers crossing there
  (`lead_tmp/starc/tools/ps2_cross.py` on `lead_tmp/stall/ps2/<map>_soldiers.csv`). The disc-side census
  (`tools/collcensus.py`) finds no other signed-fan hole on a baked map, so the other spots are not THIS defect.
- STARC built no routing change: `route::walk_point` / `approach_leg` are as KSTK left them. The RCYC table on STARC's
  card (END1, KAS1, NAB1 (-108, -108, 60..66), GEO1 `last->43`, DEA1, KAS2 (90, 18, -114), TAT1) was not looked at.

## Note from STEND (2026-10-02, `lead_tmp/stend/README.md`) - BF1 BES2's last leg beside the stairs
- BF1 BES2's whole `stuck_max` tail is one place: the lower plaza (y -56.3) under cp1's terrace (y -50.0, post at (-180.3, -50.0, 170.9), hub 52). A bot spawned at cp1's lower spawn nodes walks a last leg straight at hub 52 or 53 into the terrace wall at z 159.4 and slides away from the stairs ((-198, 159.4) and (-163, 159.4); the stairways are x -192..-186 and -174..-168, z 154..166, arcs 53 (2-52) and 52 (1-51)). 6 of 9 seeds' longest runs on main c9d2776, one recycle a seed at most; after STEND seed 42 still reaches 3,540 there.
- `FLOOR_GRID=-180,160,24,3,-45` (and `,-53`) on `bf1_bes2` shows both floors and the stairs.

## Note from OBST's measurement (2026-10-02, `lead_tmp/obst/README.md`, main 7b64fcd) - the last-leg class, sized
32 maps x 8 seeds x 10 min, 10 s windows of a walking bot with under 3 m net: 16,224 stalled; a last leg at a point
that is NOT a post's centre (a defence spot, a hunted enemy's point, an approach leg to a hub's centre) is 4,232 (26 %),
a last leg at a post's centre 3,426 (21 %). The 20 top spots per map with the body's name: `lead_tmp/obst/all/spots.md`
(classes `spot` and `post`); TAT1 alone has 1,256 `spot` windows (hunts with the aim point 13-18 m BELOW the bot,
around (-245, 14, 7) by cp1: the one-hub dune sea). New card PARR takes the post's-centre part the PS2's way (a drawn
point of the capture region); build PARR first and re-list - what is left of `post` and all of `spot` is this card.

- Planner 2026-10-02 14:45: `deps: STARC,PARR`. PARR changes what a last leg at a post aims at (a standable point of the capture region) and both cards edit `ai/brain.rs` and `ai/route.rs`; OBST's note above already says to build PARR first and re-list. Undo: `loopctl.py set LLEG deps=STARC`.
- PARR 2026-10-02 (landed 6305dd9): a last leg now aims at a drawn standable point of the capture region (`route::post_point`), not the centre; when no point stands it still aims at the centre. Re-list the spots from `lead_tmp/parr/census_v2/windows.csv` and `python3 lead_tmp/parr/pushatk.py lead_tmp/parr/census_v2`: BF2 GEO1 cp1 (bunker, 144 push samples + 11), TAT1 cp3 (moisture farm, the point 2.8 m under the bot, 102), BF1 YAV1 cp5 (bazaar, 74), BF2 MUS1 cp3 (the point 7.4 m under a bot on `mus1_bldg_collection_top`, 21). `walk_floor` accepts a drop of up to 4 m per 0.5 m step: a tighter drop is one thing to try.

## Planner 2026-10-02 17:45 - waits for BOTD
This card changes the soldier brain (`core/src/ai`) that the bot programme is about to redesign (the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot algorithm"; BOTD: no rewrite before he has read the design). Held so no lane builds on the old brain meanwhile; BOTD adopts, rewrites or closes it when it cuts its build cards. Undo: remove BOTD from `deps:`.

- Audit CONT (2026-10-02): **"TAT1's one-hub dune sea" is not a last-leg problem - it is card HUBM's.** Hub 104
  (r 525 m) has NO arcs; the PS2's locate skips a hub whose arc-mask OR has no bit for the mover (B1 `FUN_00266678`
  B1:185936, mask built by `FUN_002663a8`; BF2 `FUN_002eb918` / `FUN_002ebf70`), the port's `locate_on` does not, so
  cp7 and cp2 and every unit in the open sit "in" hub 104. With the filter prototyped TAT1's recycles go 111 -> 26
  over 8 seeds (hunting 52.2 -> 1.2 % of on-foot unit-seconds). The same prototype sends more bots to BF2 GEO1's
  walled cp7 spot (-84, 264): recycled 61 -> 85 there - that one stays this card's. `lead_tmp/audit-cont/`.

## BOTD (2026-10-03) - adopted: build step 5, after OBST
`analysis/bot-design.md` 4.5: a last leg's goal lies outside every soldier barrier (the PS2's planner refuses a goal that is not clear). Today 39 % of the bots stalled on a last leg at a post stand inside a barrier and 62 % inside one grown 0.5 m (bot-design.md M3). Start there; the local floor search is for what is left. Acceptance unchanged. In BOTV's deps.

## Planner 2026-10-03 03:35 - numbers after HUBM (main af03b6c and later)
HUBM moved this card's spots (its report, 24 seeds 9-32): BF2 GEO1 recycles 179 -> 149, the cp7 spot (-84, 264)
55 -> 34. Re-take the Evidence table on main before building: part of the walled-cp7 case was a post resolved to
the wrong hub (7 more posts changed hub at HUBM, BF1 GEO1 cp6 among them), and what is left is this card's.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**

## Session 2026-10-07 05:40 (lane 1) - re-listed on main 79f1890; what was built
- **The card's Evidence is gone on main** (16 seeds x 120,000 ticks, `lead_tmp/lleg/base16`): BF2 GEO1 16 / 16, 8 recycles;
  BF1 GEO1 3; TAT1 18; 40 last-leg recycles in 512 rounds, no spot with more than 3. HUBM / PARR / LOC3 / OBST took them.
- **What is left of the last-leg class** (OBST's census, 32 maps x 8 seeds, `lead_tmp/lleg/cen/base`): 861 of 3,325
  stalled windows (spot 643 + post 218). **384 of them, and 12 % of ALL stalled windows, are one place: BF1 Naboo
  Plains' cp3 spawn node (-107.2, -108.5, 63.2), which lies INSIDE the fallen column `nab1_prop_ruins_column2`** (a closed
  mesh 3 m thick; wall grid in the report). A bot spawned there stood on the ground inside the mesh and walked its walls
  (STARC's and OBSR's "ruins column"). The PS2 (live RAM, `lead_tmp/lleg/ps2/nab1_soldiers.csv`, 464 s, one spawn at the
  node): the soldier appears ON the column at y -104.24, stands 1.5 s, walks off its end, drops and walks on.
- **Built (f5431cd):** a post's spawn node whose floor is a pen (`Sim::floor_reach`, a flood over a 0.75 m grid, under
  200 cells) stands on the lowest floor over it within 6 m that is none (`Sim::resolve_spawn_seats`, when a post is
  added). Moves two nodes of the 32 maps: BF1 NAB1's (to -104.2) and BF2 MUS1 cp6's (-66.2, 66.7, 23.1), inside a crate
  (to 67.9; not measured on the PS2). The other 30 maps' rounds are byte for byte main's.
- **The local floor search (the card's Design) was built and measured and is NOT landed:** an A* over a 0.75 m floor
  grid into the leg's corner walk, on a failed stuck window (v1) and with a 12 m look along the line every window (v3),
  only for a bot standing in the leg's hub (without that guard a straight fallback 300 m long kept its corners: BF2
  MYG1 0 -> 116 windows). 32 seeds, last-leg windows without the Naboo pen: base 2,258, v1 2,127 (-6 %), v3 2,047 (-9 %);
  per 8-seed block base 513-632, v3 496-535, the four block differences -136 / -31 / +22 / -66: not resolved. All stalled:
  12,999 -> 12,638 / 12,608 (-3 %). Patch and module: `lead_tmp/lleg/local_search_scratch.patch`, `local.rs.txt`
  (binaries `bin_k3`, switch `LLEG=1|3`); tools `census.sh`, `spots.py`, `tot.py`. Spots it did cure: BF2 DAG1's grand
  tree (96, -16) 90 -> 41, BF1 END1 (-112, 240) 22 -> 0. Left for the planner: land it only with a per-spot case.
