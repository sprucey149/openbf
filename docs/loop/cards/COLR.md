---
id: COLR
title: Collision RESPONSE: the step/slope rule (`STEP_HEIGHT`, `WALKABLE_MIN_NY`, `TERRAIN_BURY` are ours), the layer-id constructor stores, the six `LEAF` flag bytes
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 13; RE_LOG "Collision LAYERS" §open; `analysis/re-collision-layers.md`. `push_out` is BPO's.

## Acceptance
One answer per session ([O]), each with a build card whose sweep move is declared.

- Planner 2026-09-29: JWAL (a jump carries a soldier through a KAS2 platform wall) and LEAK (the 32-map leak census)
  now own the "units get where they shouldn't" question; this card stays the step/slope constants.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — SRAD (the JWAL audit) now owns the wall/ground slope rule (n.y 0.6) and the push; this card keeps STEP_HEIGHT / TERRAIN_BURY. Value 2 (kept).
