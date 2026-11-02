---
id: BACKL
title: The `SunInfo` BACK light: every BF1 map authors `BackAngle`/`BackColor`/`BackDegree`, the port parses and never renders it
milestone: M1
value: 3
phase: re
status: done
deps:
files:
declares:
needs:
branch:
resume: closed 2026-09-26 by SUND's RE: neither ELF reads the SunInfo back light (analysis/re-sun-direction.md §5)
created: 2026-09-25
---

## Why
A second, opposing light the port drops changes the shaded side of every lit model on BF1 — part of LITE's "too bright /
wrong shade" family and SUND's sun work.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 10(b); `mungefs/src/sky.rs:121` ("Parsed, not yet …"). Read LITE's and SUND's answers first if they have run.

## Notes
- 2026-09-25 (LITE RE, `analysis/re-model-lighting.md` §6): at BF1 NAB2's Rotunda the port matched the PS2 (columns 59 vs 57) with the sun direction corrected and NO back light [D, one map]. BF1's 0x40 segments get a light set whose third 8-byte word is black (DAT_0069c440): read that set's layout before deciding whether the back light lives there.
- 2026-09-26 (SUND RE, `analysis/re-sun-direction.md` §5 and §1, [O], verifier claim D1 SUPPORTED): **CLOSED AS UNUSED.**
  - **Neither ELF reads the back light.** The hashes of `BackAngle` 0xccf25af1, `BackColor` 0x9744e135 and
    `BackDegree` 0xfad2260a appear nowhere: no decomp hit, no `lui` of their high halves (hi or hi+1) with an
    ori/addiu of the low half, no data word, and no name string to hash at runtime. `Angle` and `SunInfo` were the
    positive controls.
  - **The model light set has no room for it.** BF1's model upload sends ONE directional light (the sun, 0x69c590)
    plus three black null lights (0x55e3c8), FUN_004ac0d8 569139.
  - **The 0x40 set's third word** is {black ambient, sun factor 0}, i.e. the prelit rule (SUND §1.10).
  - Nothing to port; the parsed fields stay recorded-only. Reverse: `loopctl.py set BACKL status=todo`.
