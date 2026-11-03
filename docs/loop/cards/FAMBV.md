---
id: FAMBV
title: BF1 Naboo's fambaa shield dome as the PS2 draws it: RE the ShieldEffect draw of the fambaa_shield config (Radius 60, Height 30, texture fambaa_shield2 scrolled 0.1, colours 255x4 / 240 160 255, three unnamed keys) and draw it in the port, matched to the PS2 frames
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer's DECISION 1 (2026-09-28): the fambaa's shield is Naboo Plains' unique feature. FAMB brings the fambaas and the
shield's rule into the port; this card is how it LOOKS - the lavender translucent dome over both fambaas.

## Evidence
- `analysis/re-fambaa-shield.md` 6 [O]: the fx__ `fambaa_shield` is a Shield config parsed by B1 FUN_0037bb18
  (B1:368442-368640): Radius 60 (+0x28), Height 30 (+0x2c), Texture fambaa_shield2 (+0x1c), Color 255 255 255 255
  (+0x20), key 0x01e4aaf7 240 160 255 (+0x24), ScrollRate 0.1 (+0x3c), 0xb28af6f4 0.8 (+0x30), 0x08aab3aa 10 (+0x34),
  0xc8a9e3c6 2 (+0x38), 0x5127f14d "fambaa" -> +0x18 = 1 (a shape flag). The shield weapon makes the effect object
  in FUN_00293920 (B1:219208-219216, wc+0x2c8) and places it at the owner's matrix.
- DSHN audit note on DEKA: the ShieldEffect module draws through its own blend records (FUN_00389338: Cs x As + Cd,
  Cs x 0.5 + Cd, Cs + Cd).
- PS2 frames: `lead_tmp/famb/ps2/dome_out110.png` (110 m out, the dome's edge), `dome_out75.png`, `dome_in30.png`,
  with their save states (`dome_out75.p2s`, `dome_in30.p2s`; `pcsx2/pose.py` gives the camera).

## Acceptance
- RE [O]: the dome's mesh (shape flag, segments, how Radius / Height shape it), its blend, the two colours' roles,
  the scroll, what the three unnamed keys do; verifier workflow as the prompt says.
- Build: a port frame from the PS2 camera of `dome_out75.p2s` and of `dome_out110.p2s`-equivalent poses beside the
  PS2 frame: the dome's edge within 2 % of the frame width of the PS2's, the dome-covered region's mean colour within
  10 / 255 per channel of the PS2's (HUD masked). Seen from inside: the shell visible as in `dome_in30.png`.

## Notes
- Created 2026-10-16 by FAMB's RE (lane 2). deps FAMB (the fambaas must exist in the port first).
