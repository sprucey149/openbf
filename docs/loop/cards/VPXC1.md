---
id: VPXC1
title: BF1 parked-vehicle PCSX2 pair: a BF1 frame at a pad with its vehicle still parked (none of the 16 BF1 reference sets shows one — bots board them before the first frame), the port's VDRAW frame at the same pose
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
VDRAW's acceptance asked for one PCSX2 pair per game. BF2 is done exactly: the YAV1 AAT at the HOVR audit's RAM matrix lines up with the PS2 frame (lead_tmp/vdraw/pair_yav1_aat.png). BF1's pad frames (END1, RHN1, TAT1 v3, GEO1 spawn/v2, NAB1 v3, HOT1) all show the pad EMPTY in the PS2 — its vehicle already taken — while the port (no vehicle AI yet, VAI) keeps them parked.

## Evidence
lead_tmp/vdraw/padview.py (pads in each reference frustum), lead_tmp/vdraw/{end1,rhn1,tat1_v3,geo1_spawn,geo1_v2}.png (the port's frames). The scratch renderer lead_tmp/vdraw/vdraw_pose.rs (copy into game/examples of a scratch worktree; VEH_AT places one vehicle at a RAM matrix).

## Done 2026-09-29 by PXWA (lane 2)
- A BF1 battle with ONE unit per team (the IA menu state's `SetUnitCount` store patched to 1; 0 crashes the load), the
  bots held 300 m up from the side-select screen: every pad stays parked (`engine/artifacts/pcsx2/walk_bf1.py`).
- Pairs, viewed: KAS2 GCW Beach Head b270, 3 AT-STs — `lead_tmp/pxwa/veh/pair_kas2_cp0_b270_atst.png`; YAV1 GCW landing
  zone b090, 2 AT-STs — `lead_tmp/pxwa/veh/pair_yav1_lz_b090_atat.png` (the port's rigid bind-pose legs vs the PS2's
  idle stance: WSKN).
- Position and yaw from RAM (`lead_tmp/pxwa/veh_table.py`: the PS2 entity matrices vs vdraw_pose's VPAD list, tables
  `lead_tmp/pxwa/veh/<map>_table.md`): KAS2 3 AT-STs xz <= 6 mm, heading 0.00, y -0.14/+0.003/-0.05 m; YAV1 2 AT-STs
  xz <= 4 mm, heading <= 0.04, y -0.13/0.00; 4 speeder bikes xz 1-4 cm, heading <= 0.06, y +0.15..0.17 m (the PS2
  higher); KAS1 TIE fighter/bomber y +-1 cm, X-wing +1.776 m and Y-wing +1.330 m higher on the PS2 (-> VREST); every
  `com_weap_gunturret` (KAS2 16, KAM1 16) xz <= 6 mm, heading <= 0.05, y +0.25 m on the PS2.
