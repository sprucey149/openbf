---
id: INTL
title: BF1 interiors are 2-3x darker and uncoloured in the port: GEO1's cave glows red on the PS2 (rock (54,16,8) vs (18,9,3)), RHN2's blue-floored hall casts blue on its walls ((48,66,74) vs (31,27,25)) — the PS2 adds a light indoors the port lacks (PXWA walkabout pairs)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The PXWA walkabout (PCSX2 at the port's own post stands, 1-unit battle) shows two BF1 interiors lit 2-3x brighter and
COLOURED on the PS2 where the port draws them dim and neutral-brown:
- **GEO1's cave** by the arena (post cp6): rock (200,100,640,300) at b270 PS2 (54,16,8) vs port (18,9,3); b000
  (46,22,17) vs (19,10,3) — a red glow — `pcsx2/ref/bf1_geo1/walk/pairs/cp0_cp6__sheet.png`.
- **RHN2's blue-floored hall** (post cp7): walls b090 (260,40,560,250) PS2 (48,66,74) vs port (31,27,25); the crate
  b000 (68,60,54) vs (33,28,24); pillar b270 (31,41,45) vs (20,17,16) — a blue cast from the lit floor —
  `pcsx2/ref/bf1_rhn2/walk/pairs/cp1_cp7__sheet.png`.
Outdoors on the same maps the port and the PS2 agree, so it is something the PS2 adds indoors: a light region (LREG:
"dim the sun and blend the ambient ... inside region volumes" — for non-static models, so check whether static world
models take it too), local omni lights on world models, the prelit/COL0 path (LITEB), or LSUM's sum rule on a dim
scene. BES2's covered walls (BESL) go the other way (the port brighter and warmer) and may share the cause.

## Acceptance (RE first)
- For the two named rooms: the draws' light set on the PS2 — read the region / light lists live in the kept states
  (`ref/states/bf1_geo1_walk_cp0_cp6_b000.p2s`, `bf1_rhn2_walk_cp1_cp7_b000.p2s`) and prove the term with a patched
  state (zero it, the wall goes port-dark). Then a build card, the two pairs re-rendered (`port.py <map>/walk`) and viewed.

## Notes
- Created 2026-09-29 by PXWA (lane 2). Related: LREG (GEO1 note), BESL, LSUM, LITEB.- **TAT1's huts too (PXWA):** inside the cp2 hut the walls are brighter and pinker on the PS2 — b090 upper wall
  (300,40,640,200) PS2 (121,87,99) vs port (69,60,70), b270 (86,66,74) vs (68,59,65) — while the sunlit floor agrees
  ((122,100,88) vs (118,100,72)): `engine/artifacts/pcsx2/ref/tat1/walk/pairs/cp3_cp2__sheet.png`.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

## Merged from BESL (planner 2026-09-30): BF1 Cloud City's covered walls
The same question in the other direction (the port brighter and warmer than the PS2, under overhangs). BESL's text follows verbatim; answer both rooms' light sets in one RE.

## Why
The PXWA walkabout (PCSX2 at the port's own post stands) shows BF1 Cloud City's covered walls and halls in cool
neutral-navy shade on the PS2 and warm tan in the port. BES2's baked sun is a strong orange,
`sun_color (1.0, 0.578, 0.127)` linear = (255,200,100) (`engine/assets_baked/bf1_bes2/manifest.ron`). The PS2 lights
the sun-facing OUTER towers golden (`pcsx2/ref/bf1_bes2/walk/pairs/cp5_cp1__sheet.png` b000/b270, both agree), but not
the walls under the overhangs, and not the halls:
- `cp4_cp5__sheet.png` b270 upper band (300,20,640,90): PS2 (89,86,87) vs port (124,104,72); b000 upper wall PS2
  (56,55,62) vs port (79,66,49); b180 overhang PS2 (76,68,62) vs port (104,86,57).
- `cp2_cp2__sheet.png` b000 the hall past the door: PS2 (50,46,53) vs port (62,53,47); `cp3_cp7__sheet.png` b090.
- `cp5_cp1__sheet.png` b180: a teal-green door on the PS2, neutral brown in the port (MEDIUM).
LITEB (COL0, prelit) and SUND (the BF1 sun term, prelit draws without the sun) are done, so the port already has the
PS2's model light terms — something the PS2 applies per wall is still missing: a shadow in the walls' COL0 the port
does not multiply, a light region (LREG), the SunInfo back light (BACKL), or the material flags.

## Acceptance (RE first)
- For two named walls (the cp4 overhang and the cp4 b270 upper band): the model, its segments' flags and COL0, and the
  PS2's light set for that draw — which term makes the PS2 wall neutral where the port's is sun-orange. Settle it on
  PCSX2 with a patched state if the reading is not decisive (`lead_tmp/re/lite/ps2exp/`: zero the BF1 sun 0x69c588
  on `ref/states/bf1_bes2_walk_cp4_cp5_b000.p2s` and see whether the upper wall changes).
- Then a build card or a fix, and the three pairs re-rendered (`port.py bf1_bes2/walk`) and viewed.

## Notes
- Created 2026-09-29 by PXWA (lane 2). The states `ref/states/bf1_bes2_walk_cp*_b000.p2s` are kept (a 1-unit battle,
  every pad parked, the player held — reload one and it stands at the post).
