---
id: VFLY
title: The flyer, transcribed: the 5-state machine (landed/takeoff/flying/landing/4) and the Min/Mid/Max speed law, BF1 FUN_001d9180 and BF2 FUN_00202d08 — then a build card (BF1 flyers, faithful per map)
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why
15 flyer classes; PLAN keeps flyers faithful per source map (BF1's flyable craft on BF1 maps).

## Evidence
`analysis/re-vehicles.md` §4.2: B1 FUN_001d9180 / B2 FUN_00202d08 — 5 states (landed / takeoff with a 1024 m ray and
fraction += dt/TakeoffTime / flying / landing, touchdown normal·up > 0.9375 / 4), target speed Mid ± t·(Max|Min − Mid)
clamped to ±Acceleraton·dt of the current forward velocity, first-order rate filters, the Lua fly-height band
(±200 m default); BF2 caps and boost; carrier = flyer + cargo.

## Do / Acceptance
Transcribe the BF1 integrator (and the BF2 differences) into `analysis/re-flyer.md` with the §RE checks and one
verifier — resolve Ghidra's case fall-through with the jump table, what state 4 is, and the terrain contact; then write
the build card.

**Planner note (2026-09-26):** raised 3 → 4 — VPAD now spawns X-wings / TIEs on TAT1 and every flyer pad; after HVRE this integrator is next.

**Note (VDRAW, 2026-09-28):** with parked vehicles now drawn, the BF1 GEO1 LAAT sits with its skids in the ground
(`lead_tmp/vdraw/geo1_v2.png`): VPAD's spawn height is `ground_under` (PROVISIONAL) at the model origin. The flyer's
landed rest height (landing gear / the landed state's ground ray) belongs in this card's transcription.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — flyers come right after the alpha (ground vehicles meet 'drivable vehicles'); first M2 card to raise then. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

## Note (VREST RE, 2026-10-07): the landed state, LandedHeight, and two lines for the transcription
`analysis/re-vehicle-rest.md` §2-§3 [O]: a flyer's constructor (BF2's `EntityFlyerInitAsLanded` body) adds LandedHeight
(authored, else minus the model's vis-box min y; table of every class in §2) to its spawn point; landed, its velocity
is zeroed every tick and its body is a BUILDING-layer object (takeoff moves it to the moving layer). The instance keeps
its own copy (B1 entity+0x628, B2 +0x66c), which takeoff and landing read: BF1's takeoff too holds the lift fraction
until the height is over LandedHeight + TakeoffHeight (B1:85668-85684; re-vehicles §4.2 said BF2 only), and the landing
ends where the origin's height first drops under LandedHeight with the body upright (B1:85703-85793) - no snap. Those
two readings are the lead's, not verifier-checked. Card VREST builds the parked height; this card starts from it.

## Note (card BNDY, 2026-10-19): BF2's fly-height band and the boundary - `analysis/re-boundary.md` 4 (T3b, [O])
FUN_00202d08 handles the fly-height band in each update (B2:116581, 118385-118395, 119056-119066):
- It clears bit 7 of the flyer's +0x660, then sets it again when the height (+0x154) is below DAT_00697e50 or above
  DAT_00697e54. The gate is `(+0x330 >= 0) || (+0x324 == 0)` [H: a human pilot, or no pilot].
- It clamps the height to [min - 0.2, max + 0.2]: the PLAYER limits when +0x330 >= 0 (Lua Set{Min,Max}PlayerFlyHeight,
  DAT_00697e50 / 54), else the AI's (Set{Min,Max}FlyHeight, DAT_00697e48 / 4c). Every default is -200 / +200
  (FUN_002a4030).

The boundary timer (FUN_00350660) counts a flyer with bit 7 set as OUTSIDE even inside the 2-D boundary. So a human at
the ceiling gets "Leaving Battlefield" and dies after 6.5 s, on maps without a 2-D boundary too.

Per conquest script, MaxPlayerFlyHeight is: COR1 25, DAG1 20, DEA1 72, END1 43, FEL1 53, GEO1 50, HOT1 70, KAM1 102,
MUS1 90 / 84.16, MYG1 20, NAB2 25, POL1 55, TAT3 90, UTA1 29.5, YAV1 14 (`scripts/gen_missions.py`). KAS2 and TAT2 set
only the AI's MaxFlyHeight (70, 40).

Also in the flyer code: a class with key 0xc6e9c80b = 1 (the three steered guided-rocket ordnance classes) gets 2.17 s
instead of 6.5 s. And FUN_002faf90 (from FUN_00202880) refuses a point 300 m ahead that is outside the boundary.

Settle the [H] gate with a 2-minute live sample before building: a human flyer held above MaxPlayerFlyHeight, its
record+0x184 read.
