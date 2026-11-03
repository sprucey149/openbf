---
id: GRNC
title: The floor census as a kept tool, and the holes GRND left: commit examples/floor_census.rs, run it on all 32 maps, and close or name the shallow 0.3-0.6 m rim and seam steps (bf2_kam1 139 points, bf1_kam1 304) and the three leads on other maps (MUS1 conference floor, END1 heightfield outside the drawn terrain, MYG1 platform cores)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer's INBOX 2026-10-01 10:30: GRND (3c7b00c) is closed, and 'what is left (the census as an example, the other 30 maps, the shallow holes) wants a small new card'. This is it. A hole in the census is a place where a soldier stands on nothing drawn, or sinks through a drawn floor - the maintainer's 'invisible floors' class of defect, on maps nobody has looked at.

## Evidence
- GRND's close note (card GRND, `lead_tmp/grnd-fable/README.md`): bf2_kam1 holes 544 -> 139, none of the 25 largest clusters deeper than 0.47 m; bf1_kam1 313 -> 304 (shallow). GRND's acceptance `holes = 0` was NOT met. The census example is not committed (tools: `lead_tmp/kamr/tools/` floor_census / kamr_walk / kamr_pose; memory note 'Floor census and ground probes' lists three traps that made it lie).
- Leads GRND did not follow (its Notes): BF2 MUS1 `mus1_bldg_conference`'s collision floor 0.5-0.8 m over the drawn surface on 15,160 m2; BF1 / BF2 END1 ~709,000 m2 of reachable heightfield outside the drawn terrain; BF2 MYG1 platform cores; BF2 UTA1 `p_floor` boxes.

## What to do
1. Commit the census as `game/examples/floor_census.rs` (read-only; rustfmt / clippy clean; a memory cap when run).
2. Run it on all 32 maps. Per map: holes (collision floor with no drawn surface within 5 cm under the feet), ghosts (a drawn floor with no collision), by cluster, with the deepest step.
3. For each cluster deeper than 0.3 m on the six showcase maps and both Kaminos: a port render at the spot and, where the cause is not obvious, the PS2 drop test (`ps2-camera-anywhere` memory). Fix what is a port defect in this card if it is one rule; card the rest (at most 2 new cards).

## Acceptance
- `cargo run --release --example floor_census -- <map>` exists and prints the per-map table; the 32-map table is in the report.
- The six showcase maps (BF1 NAB1, TAT1, YAV2; BF2 UTA1, MYG1, HOT1) and both KAM1: 0 clusters deeper than 0.3 m without a named cause (the disc's own / a port defect with its card).
- If a fix is made: declared moves only, and the leak census `new 0`.
