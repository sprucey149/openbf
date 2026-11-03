---
id: FOLGB
title: BF2 grass may be lit, and the grass's GS state is now readable: the BF2 grass draw uploads the light set like the (lit) BF2 leaves; the port draws BF2 grass unlit with a PROVISIONAL source-over blend
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
FOLGG draws the grass patches unlit, source-over, no depth write ('GS blend PROVISIONAL'). LEAF's RE found (a) the BF2 grass draw `FUN_003c6e60` 422576–422577 uploads the current light set (`FUN_005804a0` -> `FUN_0057fd90`) exactly as the BF2 leaf draw does, and BF2 leaves ARE lit (lights off: -70 % on DAG1, re-leafpatch §6.6); (b) the B1 grass state struct `FUN_002ed830` 276723–276727 reads as GS registers: ALPHA 0x44 (blend), TEST 0x5160d = ATST GREATER 0x60 (alpha > 0.75 writes Z) with AFAIL FB_ONLY (the rest blended without Z), PRIM 0x58 — the leaf draw's reading proven on PCSX2 (re-leafpatch §6.5).

## Evidence
`analysis/re-leafpatch.md` §6.5–§6.6; the leaf method: `engine/artifacts/lead_tmp/re/leaf/ps2exp/` (mkstates.py 'nolight' + 'never' variants, measure.py) — reuse it on a BF2 grass view (DAG1 / FEL1 / END1 carpets) with the grass TEST struct patched to never-pass to get the grass mask.

## Acceptance
Measured: BF2 grass pixels with the lights zeroed vs control (lit or not); the grass struct patched (never / keep) to confirm TEST/FB_ONLY; BF1 as control (expected unlit). Then either close (unlit confirmed) or a build change: BF2 grass lit on the PS2's normals + the blend/depth rule, goldens declared.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
