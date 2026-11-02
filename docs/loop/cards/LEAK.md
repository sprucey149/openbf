---
id: LEAK
title: Collision leak census on all 32 maps (the maintainer 2026-09-29): a headless run fires soldier capsules at every reachable wall face and edge at walk, sprint and jump speeds and flags every end state inside a closed volume, under a deck, off the walkable surface or off the nav graph; per-map counts like the round sweep, and a gate that fails on a new leak
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/game/examples/leak_census.rs,engine/crates/game/examples/leak_census_accepted.txt,engine/CRATES.md,scripts/gates.py,scripts/land.py
declares: none (a new example and the leak gate; nothing existing may move — the leak baseline is seeded into baseline_dir after landing)
needs:
branch: leak
resume: landed 13c7843; leak baseline seeded into 20260929_0836_VTAI/leak_main_1.txt; next: JWAL, WTUN, VCOL, LEAK2
created: 2026-09-29
---

## Why
The maintainer (INBOX 2026-09-29), after a soldier jumped through a KAS2 platform wall (card JWAL): audit every map for the same
class of hole, not just that one: all maps, all invisible walls, all collision hulls. Nothing today would catch a
leak. The round sweep proves that rounds end, not that units stay out of closed volumes.

## Evidence
- The known case: JWAL (`lead_tmp/playtest_0929/kas2_platform_*.png`).
- The collision code: `engine/crates/core/src/collision.rs`, `collision/walls.rs` (`wall_sweep`), the soldier
  integrator in `core/src/soldier.rs` / `sim/`; the nav hub graph in `core/src/nav.rs`.

## Notes
- Build job, medium effort. The census is ours (engineering), so it needs no RE. Its verdicts about the PS2 go through
  JWAL or new cards.
- files/declares: fill them at `loopctl start` (a new example + gates.py/gates.toml hooks; declares probably
  `ci=+N` and the new gate's baseline). Remember `land-runs-mains-gates-script`: a gates.py change may need to land as
  its own card first.
