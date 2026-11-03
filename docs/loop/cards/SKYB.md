---
id: SKYB
title: The port's sky reads 3-11 levels brighter than the PS2's before the frame filter on 10 of 11 maps measured (BF1 NAB1 +6.3, TAT1 +9.0, NAB2 +11.2, BES2 +8.8, BES1 +6.7, RHN1 +4.8, KAS2 +3.3, YAV1 +3.6; BF2 MYG1 +10.5, HOT1 +4.7; only KAM1 is under, -4.0) - find what the PS2 does to a dome shell's colour that the port does not
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The sky is a third to a half of a frame on the open maps. On NAB1 (showcase, the worst of the six on parity) it carries +4.0 of the map's +10.6 dluma, on TAT1 +2.4: the DomeInfo filter multiplies whatever excess the surface under it has by about 1.3.

## Evidence
- `lead_tmp/brgt/prefilter.txt` and `split_all.txt` (card BRGT, main 6bc2437, 954 pairs): the sky class (pixels the port draws no terrain, water or model on), PS2 with the filter undone against the port's filter-off frame.
- Per channel on NAB1 the port / PS2 ratio is 1.04 / 1.07 / 1.09 (bluer as well as brighter): `lead_tmp/brgt/tools/chan.py bf1_nab1 sky`.
- The sky class also holds whatever the PS2 draws against the sky that the pose render does not (soldiers, effects, far objects), so a few levels may be the instrument's: check on poses that look up first.

## Acceptance
- The cause named from the ELF / a PCSX2 frame (shell vertex colour, a tint, the fog on a dome, the texture's own decode), or the card closed with the instrument's share shown.
- If built: the sky class's pre-filter dluma within +-3 on NAB1 and TAT1 by `lead_tmp/brgt/tools/prefilter.py`, no showcase map's block error rising > 0.5.

## Planner 2026-10-03 07:15 - `alpha: yes`
BRGT measured the sky over the PS2's on four of the six showcase maps (BF1 NAB1 +6.3 at 0.49 of the frame, TAT1
+9.0; BF2 MYG1 +10.5, HOT1 +4.7), and the alpha is measured by the parity score on those maps. Value 3 kept.
Also from BRGT's report, not carded (BES2 is not a showcase map; the backlog cap): BF1 BES2's models read +5.9 and
warmer (R 1.08, G 1.05, B 1.01) - the PS2's shaded walls are blue-grey, the port's brown on 12 of 28 poses
(`lead_tmp/brgt/sheet_bes2.png`); cause not named. If this card's RE finds a dome or ambient colour rule, check
BES2's walls against it before closing.
