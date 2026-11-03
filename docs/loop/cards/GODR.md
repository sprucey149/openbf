---
id: GODR
title: Godrays: the PS2 draws faint light shafts (BASE godray) wherever a map's FX file enables them and a godray class seeds rays - BF1 / BF2 Endor's 135 / 134 placed clusters (1,080 rays live in RAM on BF1 Endor), Yavin 1's cluster and leaf trees, Geonosis and Felucia's attached rays; the port has no godray code (analysis/re-skipped-classes.md 4)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The PS2 draws light shafts in Endor's forest (and on Yavin 1, Geonosis, Felucia); the port draws none. Found by card SKIPC's RE
(2026-10-15): `analysis/re-skipped-classes.md` section 4 [O] has the class (B1 FUN_00370fc0 / FUN_00370d18, B2 FUN_00455760 /
FUN_004554f0), the manager (B1 DAT_00519498, B2 DAT_00601290), the map FX switch (`Effect("Godray") Enable`, the keys and their
fields, the defaults) and which maps enable and seed it (not YAV2 / RHN1 / RHN2 / DAG1: enabled, no godray class).

## Evidence
- Live RAM, BF1 END1: 1,080 active rays = 135 clusters x NumRays 8; MaxViewDistance 130, fade 70, length 40, 3-19 rays in the
  view list per bearing (`engine/artifacts/lead_tmp/re/skipc/ps2/run_end1.log`).
- Knock-out pairs (the active list emptied for one grab): `engine/artifacts/lead_tmp/re/skipc/ps2/end1_b090_godray_quad.png`,
  `end1_godray_knock_sheet.png` - tapering vertical shafts, +5 to +7 levels of mean luminance in the b090 bands; the stored
  PS2 poses and frames are `ps2/bf1_end1/cp2up_b*.json/png` (+ `_ko`), the port at the same poses `cp2up_b*_port.png`.
- The capture tool: `engine/artifacts/lead_tmp/re/skipc/ps2/skipc_walk1.py <map> --stands 'name:x,y,z@b1/b2^pitch' --knock`.

## Acceptance
- RE (this phase): the draw, [O] in both games - the ray's shape and direction (record word 8 = -1.0, the sun, OffsetAngle
  +0xa0), the segments (B1 FUN_00370228), the line-of-sight cut (FUN_0036fdc0 -> FUN_00161d68, MaxLength +0x4c), the view
  selection and fade (FUN_0036f678, +0x44 / +0x48), the lifetime / alpha cycle (Min/MaxLifetime, Min/MaxAlpha), the texture
  scroll and the blend; and why BF1 END1's 471 tree attachments put no ray in the active list (RAM: 1,080 = clusters only).
  Check the model against the stored knock-out pairs: the shafts' screen columns at the four cp2up bearings.
- Build: the port's frame at the four cp2up poses shows the shafts in the PS2's diff bands (band luminance lift within 2
  levels of the PS2's); the port seeds 1,080 rays on BF1 END1 and 4 on BF1 GEO1 (the RAM counts) and none where the map's
  FX does not enable them; goldens that move are declared.
