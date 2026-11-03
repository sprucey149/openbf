---
id: SIMK
title: The sim constants still PROVISIONAL in `core` — sprint recovery, unit separation, spawn-slot bookkeeping, jump height, ammo/reload edge cases — and the A.1/A.2 constants re-read
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 5 and §Standing backlog 4 (briefed: `engine/artifacts/briefs/brief_a4_constants.md`, a five-sweep budget, never
run). `grep -rn PROVISIONAL engine/crates/core`.

## Acceptance
One constant family per session: its `analysis/` citation ([O]) and the constant changed with a declared sweep move.

## Notes
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.6): the per-stance ControlSpeed table is now its own card, CSPD (BF2 classes author
  sprint / jump / roll turn 0.35 via `com_inf_default`; the port's CONTROL_SPEED is the constructor's defaults).
  BF2's JumpHeight 1.78 stores as a jump velocity sqrt(36 h) = 8.005 (setter 170972-170976; RAM), the port's 8.0.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
