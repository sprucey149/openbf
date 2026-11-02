---
id: BAKAU
title: The full-bake gate's ~5,840 audio/global_bf* .ogg DIFF lines on every full-bake run (TALPH, SUND, TERL) with no audio code touched: encoder non-determinism or a stale shared audio bake? Until answered the bake gate cannot see an audio move
milestone: M0
value: 2
phase: measure
status: done
deps:
files:
declares:
needs:
branch:
resume: closed by the planner 2026-09-28: answered by KOC's gates — the shared audio bake predates the 2026-09-20 ffmpeg upgrade (Lavc62.28.102 vs .103 vendor tag, 6 bytes/file); the fix is card AUDRB
created: 2026-09-28
---

## Why
The TALPH gate run (`lead_tmp/talph/gates2/SUMMARY.md`) showed ~5,840 DIFF lines, all `audio/global_bf*` .ogg, with no audio code touched; the SUND and TERL full-bake runs show the same. land.py compares only declared groups, so a real audio move would hide in the noise.

## Evidence
`reports/20260928_1235_resume_TALPH.md` (Gates); `lead_tmp/talph/gates2/`.

## Notes
Measure only; no engine change in this card.
