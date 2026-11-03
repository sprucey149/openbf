---
id: KFIRE
title: Ambient effects attached to BF1 props that the PS2 draws and the port does not: KAS1's torch and bonfire flames and their orange light (a night map: 33 kas1_prop_torch, kas1_bldg_bonfire, 31 kas1_firelight), Theed's fountain sprays (nab2_prop_fountain x5), YAV1's mist (yav_prop_mist), KAM1's lamp glows (PXWA walkabout pairs)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BF1 Kashyyyk: Islands is a night map lit by fire on the PS2; the port draws cold torch poles and an empty fire pit.
The PXWA walkabout (PCSX2 at the port's own post stands, 1-unit battle) shows at every post:
- the torch flames on the `kas1_prop_torch` poles (33 on the map) — `pcsx2/ref/bf1_kas1/walk/pairs/cp0_cp3__sheet.png`
  b090, `cp1_cp4__sheet.png` b000, `cp3_cp5__sheet.png` b000/b180/b270;
- the bonfire `kas1_bldg_bonfire` burning under the cp6 pavilion, its orange light on the roof's underside —
  `cp5_cp6__sheet.png` b090 (the port: the pit empty, the roof dark);
- the huts' walls lit warm orange near the torches (`cp3_cp5` b000) where the port only warms a patch of ground.
The map also places 31 `kas1_firelight` and one `kas1_fireflies` instance (`engine/assets_baked/bf1_kas1/manifest.ron`).

## Acceptance (RE first)
- What draws the flame: the ODF chain of `kas1_prop_torch` / `kas1_bldg_bonfire` (an attached `fx__` effect or emitter
  class, its texture, size, animation, additive blend) and what `kas1_firelight` is (a light with colour / range /
  flicker, or an effect) — cite the disc and both decomps per the RE discipline; FXC's census of unparsed `fx__`
  chunks is the parser side.
- A build split (or `phase=build` with `files:`/`declares:`): the flames drawn, the fire lights on nearby models and
  terrain if the PS2 does that, BF2's twins noted. Re-render the pairs above with `port.py bf1_kas1/walk` and VIEW them.

## Notes
- Created 2026-09-29 by PXWA (lane 2). KAM1's lamp glows (`bf1_kam1/walk/pairs/cp2_cp5__sheet.png` b270,
  `cp6_cp13__sheet.png` b000) may be the same family (a light with a halo sprite): check while here.- **More (PXWA, the rest of BF1):** TAT1's Tusken campfires (`tat_prop_tuskcampfire`) burn with smoke and throw a strong
  orange pool of light on the sand — `engine/artifacts/pcsx2/ref/tat1/walk/pairs/cp5_cp1__sheet.png` b000 (ground band
  (420,195,640,225) PS2 (144,124,96) vs port (117,103,87)), `cp4_cp3` / `cp5_cp1` b270 (a fire in the distance);
  Theed's fountain sprays (`bf1_nab2/walk/pairs/cp1_cp3__sheet.png` b180); Endor's platform torches and ground mist
  (`bf1_end1/walk/pairs/cp2_cp5__sheet.png` b090, `cp4_cp10__sheet.png` b000). The light pool says these fires carry a
  light, not only a sprite.
- 2026-09-29 (PXWB, lane 2; FINDINGS F56): the BF2 twin — small flames on END1's Ewok platforms and trunks on the PS2 only (`pcsx2/ref/bf2_end1/walk/pairs/cp5_cp10__sheet.png` b000, `cp1_cp6__sheet.png` b090, `cp3_cp5__sheet.png` b090).
- 2026-09-29 (PXWB part 4, FINDINGS F63): `yav_prop_mist` (this card's YAV1 mist) is a `dusteffect` class — the family (10 port maps, big drifting alpha sprites in a box; POL1's hazes its whole surface on the PS2) now has its own card **DUSTE**; leave the mists to it.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).
