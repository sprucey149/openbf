---
id: BESL
title: BF1 Cloud City's covered walls and halls: the port lights them with the orange sunset sun (warm tan, e.g. (124,104,72)) where the PS2 keeps them neutral-navy (89,86,87) — the sun-facing towers agree, the shaded ones do not (PXWA walkabout pairs)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

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

## Notes
- Created 2026-09-29 by PXWA (lane 2). The states `ref/states/bf1_bes2_walk_cp*_b000.p2s` are kept (a 1-unit battle,
  every pad parked, the player held — reload one and it stands at the post).
