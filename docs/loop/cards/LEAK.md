---
id: LEAK
title: Collision leak census on all 32 maps (the maintainer 2026-09-29): a headless run fires soldier capsules at every reachable wall face and edge at walk, sprint and jump speeds and flags every end state inside a closed volume, under a deck, off the walkable surface or off the nav graph; per-map counts like the round sweep, and a gate that fails on a new leak
status: archived
---

Historical technical record. Recorded state before archival was `done`.

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
