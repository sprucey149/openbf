---
id: VLOCK
title: In-vehicle aim assist: BF2's lock-on for every seat weapon (the reversible default VTUR took from PLAN §Vehicles & space) — RE BF2's lock button and acquisition loop, then build it for riders
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
VTUR recorded the call (FEEL_NOTES 2026-09-29): vehicles use BF2's lock-on, not BF1's magnet. Nothing is built: a seat weapon has no assist.

## Evidence
Historical readings: re-vehicles §9 names BF2's weapon setter (FUN_00328fb0, B2:319976-320750) and six Controllable copies (B2:109086, 116820, 132164, 159266, 179562, 188918). Its later UORD correction (analysis/re-ordnance-kinds.md §5) says those ticks copy TargetLockMaxDistance / ...Lose and the Target* mask, rather than LockOnRange / Angle / OffAngle. UORD names FUN_0017a908 as the human HUD lock acquisition and FUN_0033a658 as the launcher's candidate reader; §12 still leaves the acquisition pick rule and button [H]. These are continuation leads, not fresh primary verification. BF1 alternative: re-vehicles §9 describes the magnet in any seat whose weapon has LockOnRange > 0.

## Acceptance
RE (discipline, verifier): BF2's lock acquisition, hold and loss, its button; then a build card or this one re-phased with the numbers; FEEL_NOTES updated; STATUS 'Calls you may want to reverse' keeps the line.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).
