---
id: CLN9
title: KAM1's ocean: the per-frame foam routing has no test, and the FFT runs on a wrapping clock
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why / Evidence / Acceptance
Why: the base sea must keep its opaque mesh while foam uses the alpha-bearing copy; ocean phase must continue when the material clock wraps.

Evidence: on main 4f2580f, WaterMotion::step routes meshes inline; windowed and headless callers feed clock(r.time), which wraps at 2000 s. KSEA already removed the still plane, so its old alpha-zero rule is obsolete.

Acceptance: extract the cell routing into a CPU function and kill a swapped-mesh mutation; keep an unwrapped f64 render-side water clock through FFT and foam, with continuity checks across TIME_WRAP and a long session. Full engine/ci.sh (+2 tests), approved GPU/time-zero goldens and unchanged replay/simhash/sweep gates remain required. Declare no moves; missing local checks keep this card incomplete.

## Original QUEUE.md entry (verbatim, 2026-09-25)
CLN9. [TODO — ADDED 2026-09-24 (lane 1) by the cleanse's bug hunt over `0cfdadb..bdbacd9` (report `engine/artifacts/loop/reports/20260924_0545_cleanse_render.md`); FREE — `game/src/water_anim.rs` and `game/tests/water_anim.rs` (W3 may touch the first — take this only while W3 is not IN PROGRESS or PARKED); LOW-to-MEDIUM value: nothing drawn is wrong at time 0, but OCF's two visible rules have no referee and the sea snaps once per clock wrap in play] **KAM1's ocean: the per-frame foam routing has no test, and the FFT runs on a wrapping clock** — (a) `water_anim`'s per-frame routing — which cell draws take the foam copy (`mesh: if foam { foam_mesh } else { mesh }`) and `0b203e7`'s zeroed foam alpha on the still plane past the far range (`d.color[3] = 0.0`) — runs only inside `step`, which needs a live `Renderer`, and no test names it (`grep -rlE 'WaterMotion|animate_water|is_foam|foam_mesh|color\[3\] = 0' engine/crates/*/tests` finds none; the file has no in-file tests): swapping the two meshes makes the base sea translucent, deleting the zeroing brings back the 640 m light ring, and ci stays green either way because the goldens never step the water. Build: move the routing into a pure function over the cell list and unit-test both rules on the CPU, each test shown to FAIL under the break it guards. (b) the FFT sea reads the renderer's WRAPPED clock (`water_anim::clock(r.time)`; `render::TIME_WRAP` is 2000 s since the 2026-09-24 cleanse, 1000 s before), and ω = √(g·k) has no 2000 s period, so every wave height snaps once per wrap in a long windowed session (the flat-water bob is within millimetres). Build: feed the ocean its own unwrapped clock (an f64 accumulator on the render side — the sim never reads it) — or, if `analysis/re-water.md` §3a shows the disc's ocean phase wraps, match that and say so. GATE: `./ci.sh` 0 at the GATES count + the new tests; `scripts/gpu_tests.sh` 0 with the goldens clean (they render the water at time 0); no sim value can move (the water is render-side) — say so with the replay gates. Files: `game/src/water_anim.rs`, `game/tests/water_anim.rs`, and for (b) `game/src/main/{windowed,headless}.rs`.

- **Acceptance (planner 2026-09-30 08:30, from the entry's GATE):** two CPU unit tests of the foam routing (foam cells take the foam mesh; the still plane past the far range gets alpha 0), each shown FAILING under the break it guards (paste); the ocean fed an unwrapped clock (or re-water §3a cited if the disc wraps); `./ci.sh` 0 at +N tests; goldens clean; sim/simhash/sweep identical (render-side only); nothing declared.

- Note (KAMR, 2026-09-30): card KSEA removes the still plane past the far range, so rule (a)'s "alpha 0 on the still
  plane" test is moot once KSEA lands; the foam-mesh routing test and (b) stand.

- Note (KSEA build, 2026-09-30): the still plane is gone from a stepped ocean (`WaterMotion::step` replaces the range
  with the patches), so rule (a)'s "alpha 0 on the still plane" has nothing left to test — drop it from the acceptance.
  The cell pick is now the pure `water_anim::cells_in_range` with a CPU test
  (`the_ocean_draws_whole_cells_that_reach_into_the_far_range`); the foam-mesh routing test and (b) stand.
