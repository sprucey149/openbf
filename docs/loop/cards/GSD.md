---
id: GSD
title: A GS-register dump from PCSX2, for the questions only the GS can answer: the D-pad menu's second material layer, the wreck's LOD at ~475 m, the far water fog pass, the 0x04 crate sheet
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Several `[D]`/`[H]` rendering answers wait on what the GS actually receives, not on the CPU's reading.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 7 (a)(b)(c) and 10(g) (ALPHA RULE v2 stands until a GS dump; the "0x04 sheet past the test" crate question).
PCSX2's GS dump facility; `engine/artifacts/pcsx2/README.md`.

## Acceptance
A repeatable dump step in the PCSX2 README; each of the four questions answered from a dump (or left [H] with why), findings
in `engine/artifacts/pcsx2/ref/FINDINGS.md`, follow-on cards.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
