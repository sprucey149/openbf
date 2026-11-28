# PERF-3 result: smaller sun shadow map for split cells

Date 2026-11-27. Branch `claude/perf-3`, based on main 34106bee7.

## What changed

- `render/src/sun_shadow.rs`: a second Depth16 map, 2048x2048 (`CELL_SIZE`), made on the
  first split view that uses it. `cell_map(cell, window)` picks it for a split cell at most
  half the window in each dimension: every cell of 3 and 4 views (the 2x2 grid). One view and
  2 views (full-width or full-height halves) keep the 4096 map.
- `light_clip` snaps the light anchor with the chosen map's `texels_per_metre`;
  `draw_casters` draws into the chosen map; `composite` binds it in the bind group it already
  makes per call. No shader change, no viewport inside the 4096 map, no UV scale. `HALF_SPAN`
  (±32 m), the fades, bias and caster set are unchanged.
- `render/src/frame/submit.rs`: `render_scene` works out `shadow_cell` from the view slot and
  the window size and passes it to the three calls.
- Visible change, split only: a shadow texel grows from 1.6 cm to 3.1 cm. That is the same
  texels per screen pixel as one full-window 4K view at 4096.

## Probe (step 1)

A bench-only size override (not committed) on littlemallard, 4 views at 1.0, 120 waited
frames after 900 ticks, two runs each. Pass mark set before: at least 1 ms saved over 4 views
on both maps.

| map | depth @4096 | depth @2048 | casters per view (draws / instances / triangles) |
|---|---|---|---|
| mus1 | 2.91, 3.27 ms | 0.98, 0.96 ms | 54 / 69 / 12k |
| kas2 XL | 4.00, 3.98 ms | 1.20, 1.18 ms | 23 / 26 / 22k |

Passed (2.1-2.8 ms saved). Cost scales with map area, not caster count. Only draw and
triangle counts were logged: clear cost and caster fill were not separated, so this does not
rule out caster culling for a later batch.

## Tests

- `scripts/one_player_gate.py`: every row matches (release build of this branch).
- `cargo test -p render shadow -- --include-ignored`: 3 lib + 4 suite passed, including the
  new `sun_shadow::tests::only_quarter_cells_draw_the_cell_map`.
- `cargo test -p render --test suite view -- --include-ignored`: 6 passed (split views, grid
  bleed, acquire).
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 3 passed; the
  4-view capture was looked at.
- clippy on render: no new warnings; rustfmt clean on the touched files.

## Workstation captures (landing check)

Bench `--capture-frame`, 3840x2160, scale 1.0, seed 7, Team Size 200% (kas2 XL), before
(main) and after, 1-4 views on cor1, mus1, kas2. Files in the session scratchpad
`p3perf/caps/` (`*_base.png`, `*_new.png`, `*_diff.png`, `*_crop.png`).

| map | 1 view | 2 views | 3 views | 4 views |
|---|---|---|---|---|
| cor1 | identical | identical | 0.43% px differ, max 36 | 0.42%, max 38 |
| mus1 | identical | identical | 0.22%, max 30 | 0.23%, max 30 |
| kas2 XL | identical | identical | 0.66%, max 28 | 0.66%, max 9 |

The differences are shadow edges moving by a pixel or two (crops checked by eye); edges stay
smooth with the linear compare sampler. Sim hashes match before and after.

Moving camera: 4 views, scripted seats, captures at frames 2-16 (8 per map, mus1 and kas2 XL).
Edge pixels that change frame to frame inside the shadow-edge mask: mus1 38,993 -> 40,905
(+5%), kas2 957,479 -> 973,450 (+2%). Strips (`seq_*_strip.png`) show no visible crawl in
seat 0's cell. Only seat 0 walked, so crawl under motion was checked in one cell per map, 4
views only, mus1 and kas2 XL only (no 3-view run, no cor1); the edge-change counts are from that
cell too. The other cells were near-static.

## littlemallard measurements

Headless, ES-DE idle, nothing on the TV, /tmp/openbf-bench only (deleted afterwards), installed
openbf.pak read through a symlink, before and after binaries alternated. 3840x2160, elite,
seed 7, Team Size 200%. Waited rows: 900 ticks then 120 frames, mean GPU ms per frame, all
views summed.

| map | views | scale | sun shadow depth before -> after | GPU total before -> after |
|---|---|---|---|---|
| mus1 | 2 | 1.0 | 1.44 -> 1.44 | 10.30 -> 10.33 |
| mus1 | 2 | 0.5 | 1.66 -> 1.65 | 5.63 -> 5.63 |
| mus1 | 3 | 1.0 | 2.29 -> 0.62 | 12.59 -> 10.68 |
| mus1 | 3 | 0.5 | 2.28 -> 0.65 | 6.53 -> 4.89 |
| mus1 | 4 | 1.0 | 3.32 -> 0.98 | 16.26 -> 14.03 |
| mus1 | 4 | 0.5 | 3.35 -> 0.94 | 8.61 -> 6.17 |
| kas2 XL | 2 | 1.0 | 1.93 -> 1.93 | 22.93 -> 22.89 |
| kas2 XL | 2 | 0.5 | 1.92 -> 1.94 | 10.29 -> 10.31 |
| kas2 XL | 3 | 1.0 | 3.57 -> 1.04 | 21.45 -> 18.90 |
| kas2 XL | 3 | 0.5 | 3.55 -> 1.05 | 12.54 -> 10.04 |
| kas2 XL | 4 | 1.0 | 4.01 -> 1.20 | 29.72 -> 26.95 |
| kas2 XL | 4 | 0.5 | 3.93 -> 1.20 | 16.81 -> 14.07 |

Pipelined, 4 views, split UI, AI seats, scale 1.0, 60 s (mus1 after 900 ticks, kas2 XL after
1500). Two runs each, order base/new then new/base.

| row | before | after |
|---|---|---|
| mus1 frame p50 / p95 / p99 | 18.7 / 22.4 / 24.3; 18.6 / 22.6 / 24.1 | 16.3 / 20.2 / 21.8; 16.2 / 20.1 / 21.8 |
| mus1 GPU p50 / p95 | 17.8 / 20.3; 17.8 / 20.4 | 15.5 / 17.9; 15.4 / 18.0 |
| kas2 XL frame p50 / p95 / p99 | 30.6 / 34.3 / 35.6; 30.4 / 34.1 / 35.6 | 29.3 / 33.0 / 34.2; 29.4 / 33.1 / 34.6 |
| kas2 XL main CPU p50 / p95 | 29.6 / 33.4; 29.4 / 33.1 | 28.5 / 32.1; 28.5 / 32.2 |

Mustafar is still GPU-bound at 99% busy; its 4-view p95 is about 20 ms, not yet 16.7 ms.
kas2 XL p95 is now just under the 33.3 ms step. Its main thread also fell about 1 ms; the
cause was not traced (probably less driver time on the smaller clear and pass).

## Deviations

- The probe ran with a temporary env override in a separate build, not a bench flag in code;
  nothing of it is committed.
- Captures are the bench's `--capture-frame` stills and a scripted-seat frame sequence, not a
  free-flying camera. Only seat 0 walked during the sequence.

## What is left

- The TV look check of 3- and 4-view shadows on the playtest build (playtest note): with
  several players moving and turning at once, watch every 2x2 cell for shadow-edge crawl, since
  the capture check covered seat 0's cell only.
- Mustafar 4-view still needs about 3.5 ms at p95: PERF-4 (bloom) and the governor.
