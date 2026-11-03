---
id: CENS2
title: The collision census depends on the runs before it: one run of 52,845 on BF2 KAM1 flips (ok -> ceiling) when the map is dealt to 4 chunks instead of 1 - find the state a run leaves in the sim (not the doors, the tick, lcg or rng) so a run starts from the same world and the census can use every core on its slowest map (192 s -> about 80 s)
status: archived
issue: 93
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #93](https://github.com/sprucey149/openbf/issues/93).

## Why
The maintainer, 2026-10-07: speed the tests up. The census (`leak_census`, the `leak` gate, 3 min 20 s) runs one thread per MAP, so its wall time is the slowest map alone: BF2 FEL1 192 s against a 71 s average over 32 threads (the per-map times are in `loop_baselines/.../leak_1.err`). Card GSPD built `LEAK_CHUNKS=<k>`: a map's samples are dealt to k tasks and merged back in the one-thread order (`run_part` / `merge_map`). At k = 4 it would take the census to about 80 s - but the output is not the k = 1 output.

## Evidence
- BF2 KAM1, k = 4 vs k = 1: `ok 44317 / ceiling 747 / sites 382` becomes `ok 44316 / ceiling 748 / sites 383`: ONE run of 52,845, `spam` mode from start (-189.28, 75.30, 129.40), yaw -2.2269, body 234: k = 1 `ok` (tick 124, end (-184.99, 69.29, 133.94)); k = 4 `ceiling` (tick 43, end (-185.05, 75.50, 133.93)). k = 2, 4, 6 give ceiling and k = 1, 3, 5 give ok in `LEAK_NEAR=-189.28,129.4,15`: which earlier run shares the chunk decides it.
- The unit starts from `template.clone()` each run, so the leak is in the `Sim` (or a cache in `collision` / `heightfield`). Reset before every run, and the split still differs: the doors (`Door::open`, bodies re-posed), `sim.tick`, `sim.lcg`, `sim.rng`. With the resets k = 1 still equals the gate baseline, so they change nothing at k = 1.
- BF2 KAM1 (and the other 31 maps) at k = 1 are the gate baseline byte for byte; HOT1, COR1 and BF1 KAM1 at k = 1, 4 and 7 are identical to each other (the leak is rare).
- The gate runs k = 1 (the default), so nothing in a landing changed. The census is also skipped now when a diff cannot move it (`engine/goldens/leak_deps.toml`, `scripts/gates.py` `plan_leak`).

## Acceptance
- The state is named (bisect: dump the sim's hashable parts before a run in the k = 1 and k = 2 histories of the run above; the candidates left: `bolts`, `stickies`, `pickups`, `battle`, `cheats`, `assist` state, a cache in `CollisionWorld` / `Heightfield`, the unit's `AssistState`) and reset per run, so a run is a function of its start alone.
- k = 1 and k = 4 give the same census on all 32 maps; if resetting moves some k = 1 runs, the baseline is re-taken ONCE with the moved runs listed (a declared `leak` move).
- The default becomes 4; the census gate takes about 80 s; the wall time is printed beside it.

## Notes
Filed 2026-10-07 by Claude in chat (card GSPD, the maintainer's request to speed the tests). A per-map RESULT CACHE was considered and not built: after the skip rule a census only runs when soldier / collision / sim code changed, which changes every map's key, so a cache would almost never hit and could hide a leak behind a stale entry.
