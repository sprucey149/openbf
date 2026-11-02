---
id: AIBST
title: Trace the PS2's per-weapon burst bounds and inter-burst pause for bots (BURST_MIN / BURST_MAX / PAUSE_BASE_S are placeholders)
milestone: M3
value: 2
phase: re
status: done
deps:
files:
declares:
needs:
effort:
branch:
resume: Answered by card CLSE4's measure (2026-10-12, re-soldier-ai 20.3, docs 5fbbbd4): the keys are BarrageMin / BarrageMax / BarrageDelay, class defaults 3 / 6 / 2.0 s in both games, only flyer weapons and the defence turret write them; the build is CLSE4's
created: 2026-10-02
---
## Why
Split off card STTAP (2026-10-02; cleanse CLNC's note there). `core/src/ai.rs`'s fire discipline for automatic weapons - `BURST_MIN` 2, `BURST_MAX` 6, `PAUSE_BASE_S` 0.5 - is PROVISIONAL with no RE: the PS2 reads the burst bounds per weapon class (`vtbl+0x34/+0x38`) and the pause constant from `weaponclass+0x21c` (`analysis/re-ai-difficulty.md` 4.1 / 4.4 gives only the shapes; BF2 `FUN_002bac98`, BF1 `FUN_0023c9c8`). Single-shot weapons no longer use them (STTAP).

## Evidence
- `engine/crates/core/src/ai.rs` 404-419 (the constants and their comment).
- BF2 `FUN_002bb1b0` sends a non-single weapon to `FUN_002bac98` (B2:242171); BF1 `FUN_0023cd40` to `FUN_0023c9c8`.
