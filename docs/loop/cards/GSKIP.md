---
id: GSKIP
title: The selective gates skip the golden step for any diff under engine/crates/core/, but core decides golden pixels: the gallery / walkabout vantage on the six stub-terrain maps is floored by core::collision ray_down, grass / foliage / leaves sit on core::heightfield, doors pose by core::door — GRND's whole file set would be skipped while it changes ray_down
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX (2026-09-30, speed-up round): "the first landings after this are the real test — the planner should read
their GATES_FOR_REPORT.md for anything odd (a wrong skip, a reused row that should have re-run) and card it value 5."
The planner read BPNT's, STHOP's and CLNC's. The reused rows and the single sweep/arena/leak runs are right. The skip
rule is not: `engine/goldens/deps.toml` lists `engine/crates/core/` under `skip` ("the goldens are static frames and none
of these draws"), but `core` code decides golden pixels in at least four places.

## Evidence (read 2026-09-30 21:45, main f0da5ad; none of it run — step 1 below runs it)
- `engine/crates/game/src/gallery.rs:232-240`: on a map with a stub `tern` (COR1, DEA1, MYG1, TAN1, UTA1, BF2 KAM1 — two
  are showcase maps) the gallery / walkabout vantage is floored by `scene.collision_world().ray_down(…)` =
  `core/src/collision/rays.rs:57`. A changed ray answer moves the camera, so the whole sheet.
- `game/src/viewer/grass.rs:217`, `foliage.rs:59`, `leaf.rs:265`: placement and tilt from
  `core::heightfield::Heightfield` (`height_at`, `normal_at`).
- `game/src/viewer/scene.rs:140`: door leaves posed by `core::door::sample`.
- `game/src/viewer/vehicles.rs:273-294`, `walker_pose.rs:28`: `core::vehicle::walker` (only if a golden draws a walker).
- CLNC (de2541b) was the first landing skipped under the rule: `gpu | goldens skipped (deps.toml) | no golden can move:
  16 changed file(s)`, with 11 of them under `core/src`. Its core changes were comments and one unused function, so nothing
  was missed THAT time.
- The next case is real: **GRND**'s `files:` are `core/src/collision/rays.rs`, `core/src/sim/state.rs` and an example —
  every one under a skipped prefix — and it changes `ray_down_with`. SRAD, VCOL, VSUB and LEAK3 edit `core/src/collision` too.
- The safety net (every 10th crate landing regenerates everything) would show the move up to nine landings late, on
  another card's landing, which then has to explain a golden it did not move.

## Notes
- memory `land-runs-mains-gates-script`: land.py runs main's `gates.py`, so this lands as its own card before the
  cards that rely on it; if `gates.py` itself needs no edit (deps.toml + selftest only), say so.
- Until this lands: GRND, SRAD, VCOL, VSUB, LEAK3 run `gates.py … --golden-groups all` (notes on those cards).
## Outcome (2026-10-01, landed bc6a047; report `reports/20261001_2035_card_GSKIP.md`)
- Mutations of main ce66fe2: `ray_down` + 1.0 m moved maps/bf2_cor1.png, maps/bf2_uta1.png, walkabout_stats.ron;
  `normal_at` = +Y moved bf1_end1, bf1_rhn2, bf1_yav2, bf2_end1, bf2_fel1 and the stats; a panic in `Sim::empty` /
  `Sim::new` moved nothing.
- Rule: only core's sim*, ai*, aim_assist, perception, skill, squad, tests and examples are skipped. `soldier.rs` RUNS.
- `gates.py` WAS edited: `core_link_check` fails the gpu gate when deps.toml skips a core module a golden test links.
- History (`scripts/gates_history.py`): 47 golden-moving landings, 0 skipped; 57 % of the other commits skip (65 % before).
