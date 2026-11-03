---
id: FOGN
title: BF2 fogs far-scene draws only (beyond NearSceneRange[2], blended over b..c); the port fogs every draw from near (MYG1 mid-distance +20 levels)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BF2 applies its fog pass only to far-scene draws: beyond `NearSceneRange[2]`, with the near scene composited over the
far scene between b and c (`analysis/re-fog.md` §2.2 / §2.5 / §6 [V]). `bake::lighting::fog_from` lists this as
"NOT reproduced". The port fogs every draw from `near`, which lifts BF2's mid-distance toward the fog colour.

## Evidence
Queue LITE's instrument at BF2 MYG1 (`analysis/re-model-lighting.md` §8; `lead_tmp/re/lite/means.txt`):
- the mid-distance band (0,60,640,200) reads 82 with the port's fog, 46 without, PS2 61 (spawn);
- on `_v2` it reads 68 / 33 against the PS2's 50.

## Acceptance
- A rendering decision, taken PROVISIONAL and written in `look.rs`: BF2 fog only past `NearSceneRange[2]` with the
  near/far blend over b..c, or an approximation of it.
- The declared moves are every BF2 fogged map golden (END1, FEL1, HOT1, KAM1, MYG1, TAT3 and YAV1 fog-on; those with no
  far layer already fade instead).
- The maintainer's standing fog-density call (`FOG_DENSITY_SCALE` 0.92) stays.
- Land after LITEB: the band then depends on this term alone.

**Planner (2026-09-28):** KASF's FOGB asks the same question for BF1 (near models unfogged on the PS2) and adds planar
vs radial distance. Kept as separate cards (different game, different phase); if FOGB finds a near-scene rule in BF1 too,
build both in one branch — the files are the same.

## Note (REDB, 2026-09-29, lane 2)
REDB builds the fog AMOUNT (planar depth, the `fogspread` table, a lerp on the stored bytes) for every fogged draw
(`analysis/re-fog-amount.md` §8). This card keeps BF2's far-scene-only rule; build it after REDB lands, in the same files.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F50): BF2 GEO1's far view is a thick brown dust haze with drifting dust puffs on the PS2, crisp in the port (`pcsx2/ref/bf2_geo1/walk/pairs/cp0_cp2__sheet.png` b090/b180, `cp3_cp6__sheet.png` b000/b090) — a data point for the far-scene fog amount (the puffs are ambient FX).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).
