---
id: VPOSF
title: A flyer's posed nodes are drawn: the snowspeeder's FinAnimation flaps (turn axis + speed setpoint law) and tow-cable gun, the gunships' wing and ball turrets, the Y-wing's turret, the MAF's and K-wing's guns - by the PS2's node table (analysis/re-vehicle-pose.md 6)
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why
A flyer's posed nodes on the PS2 (`analysis/re-vehicle-pose.md` 6, card VPOSE's RE): its aimers and MountedTurrets
write into the flyer's node table (BF1 E+0x1ce0, BF2 E+0x1940) and its draw reads it; the snowspeeder (BF2 HOT1) also
poses its flaps by its `FinAnimation` clip `all_snowspeeder_9pose` (bottom flaps 55 deg, top flaps 52) from the raw
turn axis and the speed setpoint (a = turn + 4, b = turn + (s > 0 ? 1 : 7), c = |s|, s placed on MinSpeed / MidSpeed /
MaxSpeed in [-1, 1]; B1 FUN_001d8620 B1:84525-84557, B2 FUN_00201300 B2:115336-115384), and its rear gunner's
tow-cable gun turns (`towcablegun`, 382 vertices, PitchTurnFactor 1). The gunships' wing turrets and ball turrets
(BF2 MYG1, 58 vertices each), the Y-wing's turret, the MAF's guns, the K-wing's pods do likewise. The port draws
flyers rigid.

## Evidence
- re-vehicle-pose 6: [L] the aimer formula fits the flyers' aimer nodes on every RAM row (X-wing, gunship wing guns,
  MAF, TIE fighters, `lead_tmp/vpose/replay_aimer_veh.txt`) and the snowspeeder's tow gun turret node 36/36
  (`replay_turret.txt`); [H] BF1's gunship ball turrets fit only at yaw 0 (10/24) and the MAF's rear cannon 0/7.
- [L] the snowspeeder's bank holds one clip (no 'takeoff', which would pose instead of the fins).

- **Note from card VPOSW (2026-10-22, `analysis/re-walker-nodes.md` 0.1):** the [H] misfits above were VPOSE's
  reader, not the PS2: `vpose/ps2table.py` read every vehicle's table at the HOVER's offset (BF1 E+0x1d20, BF2
  E+0x17e0), 16 hash slots off a flyer's (BF1 E+0x1ce0, BF2 E+0x1940). Re-read at the flyer's own table
  (`lead_tmp/vposw/ftable.py` -> `ps2/*_ftable.json`, replay `lead_tmp/vposw/freplay.txt`): every flyer node row fits
  the aimer / turret formulas with the parent's own entry, 1,501 / 1,501 - the MAF's rear cannon 7/7 (was 0/7), BF1's
  gunship ball turrets 24/24 (was 10/24); the MAF's parents point into the flyer's fin pose (E+0x740 / 0x780 / 0x880).
  So this card's first RE question is answered; BF2 flyers other than the snowspeeder were not in VPOSE's dumps
  (re-read the BF2 states with ftable.py's offsets before building). A world-based aimer's steering frame is
  (class bind) x (parent's entry) x (world) (re-walker-nodes 5) - the same on a flyer.

## Acceptance
- RE first (bounded): the ball-turret / rear-cannon misfit (which parent or factor), the flyer's speed setpoint in
  the port's flyer model.
- Then: the snowspeeder's flaps posed from its numbers equal the PS2's table rows (a BF2 HOT1 state with a flown
  snowspeeder) within 0.01 deg; a gunship's turret nodes from the gunner's look; pictures PS2 beside the port.
