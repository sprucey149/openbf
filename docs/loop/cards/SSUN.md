---
id: SSUN
title: BF1 soldiers take 2.6-7x less sun in the port than on the PS2 when lit from overhead or in front (KAS2 SBD, ambient off: T90 17.3 vs 6.7, real sun 12.7 vs 3.7; back-lit equal) - plant the PS2's own bone pose first, then name the term
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
After card SBLT's ambient fix the port's BF1 KAS2 super battle droid (no specular pass) should read ~43.8 against the
PS2's 51.2. The rest is the sun: with the ambient off, the PS2 droid takes 2.6-7x more sun than the port's when the
light comes from overhead or in front (and 3.4x under KAS2's real sun), while with the light behind it (camera side)
the two agree (29.1 vs 29.7). The form is the same on both (BF1's ((1+n.l)/2)^4, normals turned by the bone in VU1
0x5ac-0x5c3), the scale matches on the back plate, there is no left/right mirror, and per-vertex vs per-pixel evaluation
moves the port by under 0.5 level. Soldiers are on screen all the time; a soldier lit from above / in front reads too
dark in the port.

## Evidence
- `analysis/re-soldier-light.md` §3 (card SBLT), table §3.2; frames `lead_tmp/sblt/ps2/frames/kas2_nospec+nocamb+sun*`,
  port `lead_tmp/sblt/port/kas2_vtx_idle_noamb_sun*`; sheets `lead_tmp/sblt/sheet_T90_T135_x3.png`,
  `sheet_side_x3.png`.
- Confound: the port's pose is a gallery clip frame (the SBD's `human_sbdroid_stand_idle_emote` frame 0); the PS2's
  thighs read bulky and catch overhead light where the port's legs show thin struts.

## Acceptance
- Plant the port's SBD in the PS2's own pose: read the soldier's bone matrices from the save state (the VU upload's rows
  144+ per bone, or the animator's palette in RAM - find it by the matrices' orthonormality near the soldier's feet) and
  feed them to `lead_tmp/sblt/tools/sblt_soldier.rs` as the palette.
- Re-run the sun sweep (`lead_tmp/sblt/ps2/mk1.py` sunT / sunS against `tools/port_kas2.sh` SBLT_SUNT / SBLT_SUNS):
  either the gap closes (pose: close the card) or it stays - then name the term (a per-bone normal scale, a second
  light, a skinned-path rule) with a patched state, and fix it to within 3 levels of the PS2 at T90 / T120 / real sun.
