---
id: TMSK
title: The PS2 draws no terrain on mask-0 patches (BF1 KAS1 951/1024, KAS2's lake bed, END1, RHN2, NAB2, BES2; BF2 END1) nor where the layers cover < 1; the port draws LTEX[0] there — F16's grey lake bed where the PS2 shows the mirrored sky
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The PS2 issues no terrain layer pass for a patch whose PTCH mask is 0 (`analysis/re-terrain-layers.md` §2.5 [O]: B2 FUN_0056e7d8 zeroes the slot's chains, FUN_0056e898 builds none; B1 FUN_004851b0 / FUN_004852e0). Where a vertex's layers cover < 1 it shows what is behind (§5.5 holes). Those areas are each map's floor: BF1 KAS1 -52.2 m (951 of 1024 patches), KAS2's lake bed -5.9 m (403), END1 (872), RHN2 (858), NAB2 (855), BES2 (389), BF2 END1 (872). The port draws them with LTEX[0], which is F16's grey lake bed; the PS2 shows no bed, so its water shows REFL's mirrored sky shells.

## Evidence
`lead_tmp/re/talph/holes_*.png`, `topdown_*.png` (navy = mask-0, red = holes); `engine/artifacts/pcsx2/ref/FINDINGS.md` F16; census `lead_tmp/re/talph/census.txt`.

## Note (REFL RE, 2026-09-29, lane 2)
REFL is RE-done: `analysis/re-sky-reflect.md` §3–§5 [O], build pending.
- On the PS2 the near lake shows the MAIN-target mirrored copy (the 0x1000031 draw, bucket 3) through the Z-written
  near water, because nothing is drawn under the water on mask-0 patches. Removing that one draw on the PS2 turns the
  near lake from (37.6,60.1,92.7) to (28.0,36.8,43.0).
- REFL's scratch prototype with ALL terrain dropped (a stand-in for this card), at the PS2's KAS1 East Harbor lake
  pose, pitch −19.3° (`lead_tmp/re/refl/port/v3dn_refl_noterr.png`): near-lake box (100–140, 150–450) = (44.2,66.0,95.3),
  against the PS2 natural frame's (37.0,59.6,92.6). The port today shows (68.1,74.7,80.4), the bed.
- That pose, its states and frames: `lead_tmp/re/refl/ps2exp/` (`states/refl_v3dn.p2s`, `frames/refl_v3dn__ctl.png`).

## Note (PXWA, 2026-09-29, lane 2): KAS1's lake at the posts
The PCSX2 walkabout shows KAS1's lake as the port's worst BF1 mismatch at eye height: a flat pale slate band where the
PS2 has dark mirrored water — `engine/artifacts/pcsx2/ref/bf1_kas1/walk/pairs/cp0_cp3__sheet.png` b270 band
(0,255,640,290): PS2 (22,32,34) sd 30, port (75,89,101) sd 8; `cp2_cp2__sheet.png` b090 (a pale lake floor),
`cp3_cp5__sheet.png` b090. Re-render these after the build (`port.py bf1_kas1/walk`).
- **BF1 RHN2 at every cliff-edge post (PXWA):** the port shows a flat grey checkered floor running to the horizon where the
  PS2 shows the cloud sea and drifting mist below the Citadel's edge — `engine/artifacts/pcsx2/ref/bf1_rhn2/walk/pairs/
  cp5_cp6__sheet.png` b090/b180/b270 (below-horizon box (0,245,640,310) at b180: PS2 (71,83,96) sd 29, port
  (108,116,137) sd 13), `cp6_cp5__sheet.png` b180/b270, `cp4_cp2__sheet.png` b180. RHN1's harbour plane
  (`bf1_rhn1/walk/pairs/cp2_cp7__sheet.png` b090/b180, `cp0_cp2__sheet.png` b180) looks the same — check whether RHN1 has
  mask-0 patches too. The most visible TMSK case on BF1 after KAS1's lake; the planner may want value 5.
