---
id: CWLK
title: Three state leaks in the bots' route and team plan, read in cleanse 15: a corner walk of exactly six corners (or whose re-search fails) aims its leg line from the leg's START again, not from the last corner (MOUTH's rule lost); a post's held count keeps holders who draw away in the same plan; a unit respawned into a slot reads the dead unit's defence spot until the next plan
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Cleanse 15 (2026-10-13, core diff 177140d..0487827) read these in the code; none was run. Each is a hypothesis until one case is traced on main (RULES top-10 no. 2). Lines are main 0487827.

1. **Corner walk continued (read by the reviewer AND the cleanse lead).** `ai/route.rs:1258-1266`: at the last corner of a full array `advance_route` calls `fill_detour`, which first zeroes `detour`, `detour_len` and `detour_at` (559-562). When the re-search returns an empty path (the walk had exactly `DETOUR_MAX` = 6 corners) or `None`, the route is left with `detour_at == 0`, so `seg_from` (695-703) returns `r.from` and `leg_point` projects the bot onto the leg's ORIGINAL line - the line the six corners went round. A walk of 1-5 corners keeps its last corner (`detour[at-1]`). Card MOUTH's rule is "past a corner the line starts at that corner". The test `a_corner_walk_over_the_legs_array_is_continued` covers only a refill that finds a next corner.
2. **Held count (reviewer's trace; the lead read the two sites only).** `commander.rs:526-538` fixes `held[ci] = min(holders, cap)` before the per-unit loop; a holder inside the cap that draws away in that plan (dwell, give-up, the 5 s rule, BF1's owner change) is not taken off, so a redraw onto that post is refused as `full` (676-686). Scenario: BF1, eight capturers of post c, cap 4 - on the capture plan `held[c]` is 4 and nobody may draw c. It corrects at the next plan (30 ticks).
3. **Respawned slot's spot (reviewer's trace, inferred).** `TeamPlan::spot(index)` (`commander.rs:229`) has no generation test and nothing clears `spots` at respawn: a bot respawned into slot i before the team's next plan reads the previous life's defence spot and stance mask (a Snipe hint picked for a scoped class, read by an unscoped one). The plan keeps `gens`; `of()` has the same gap from before DEFS.

Also read, NOT in this card's acceptance: `ai/avoid.rs:187-190` - the opening-door pick returns before the `floor_ahead` test every other open-way pick passes (matters only at a door within a metre of a drop; no such door was looked for).

## Evidence
The findings are summarised in `engine/artifacts/loop/reports/20261013_0805_cleanse_core.md`.

## Acceptance
- First, per item: a census on main that says how often it happens (1: legs whose corner walk had 6 or more corners, per map, and the stalls on them - `lead_tmp/obst2` legcen; 2 and 3: a count in a headless round). An item that never happens on the 32 maps is closed with that count and no code.
- For each item built: a unit test of the scenario above that fails on main (mutation-checked), and the 24-seed x 32-map sweep table against a null twin of main (stalls, recycles, deaths per map) with no map worse past noise; the behaviour gate PASS or sorted by rule 0.

## Notes
Value 3: none of the three is tied to something the maintainer saw. Item 1 touches the same code as the Mustafar pit fix (OBST2: 23 recycles in 8 rounds before it) - run bf2_mus1 first.
