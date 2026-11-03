---
id: HCRAD
title: A hover's detour corners at its own radius (PS2: corners r + 0.5 m out, r = half the X/Z diagonal of the class model's box - BF1 bike 2.44, BF2 bike 1.95, hailfire 6.39; the port plans every unit at the soldier's 1.0 m) - and why, with it, BF1 Endor's bikes keep the PS2's distance from barriers but crash half as often (0.13 a ride against the PS2's 0.28)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Card HCORN's RE (`analysis/re-hover-corners.md` 1, 2, 5): the PS2 plans a hover's detour corners at the hover's OWN
radius - corners r + 0.5 m outside both barrier edges, the leg tested at r, r = half the X/Z diagonal of the class
model's box (B1 `FUN_00235ad0` / B2 `FUN_002a76b8`, kind 6 x 0.5). Live: BF1 bike 2.94 (130 of 137 BF1 Endor corners),
BF1 tat1's other classes 5.42 / 6.08, BF2 bike 2.45, BF2 Kashyyyk 4.39 / 4.83, hailfire 6.89. The port plans every
unit's corners at the soldier's 1.0 / 1.5 m (`ai::DETOUR_RADIUS`), so its bikes skim barriers: fast bike ticks within
3 m of a hover barrier on BF1 Endor 13.6 % against the PS2's 10.65 %.

## Evidence (engine/artifacts/lead_tmp/hcorn/)
- BF1's box is the model's `dummyroot` INFO min / max: RAM *(*(E+0x240)+0x1c) +0x80 / +0x90 (`tools/bounds_ram.py`,
  bike 2.439, K 0x15806f0 4.920 = the live offsets less 0.5), the same words in `SIDE/IMP.LVL` @0x931ac,
  `SIDE/REP.LVL` @0x138738, `SIDE/ALL.LVL` @0x108a78 / @0x113eb8 (`tools/bbox_disc.py`). The port's baked GLB boxes
  give the BF1 bike 1.95 / 2.01 (whether the bake keeps the INFO box anywhere was not searched). BF2: the collision model's box fits all four live classes
  (1.950 / 3.889 / 4.328 / 6.393; `bf2_collision_boxes.txt`) - whether BF2 reads the same INFO box is open.
- Prototype (HCX=3 = HCORN's rule + r 2.44 for every hover, `hcorn_x_prototype.patch`, 24 + 24 seeds): Endor's fast
  barrier shares become the PS2's (2.83 / 10.84 % within 1 / 3 m; PS2 2.09 / 10.65) but its crashes per ride fall to
  0.136 / 0.129 against the PS2's 0.28 (15 / 54 rides); tat1 bikes 0.03 a ride (PS2 0.08, 4 / 49).
- So the PS2's bikes crash about twice as often at the same barrier distances. Its corner hand-overs inside R kill 9 %
  within 2 s (6 / 67), the prototype's 1.6 % (18 / 1126); 2 of its 15 crash kills came inside the unstick manoeuvre
  (`re-hover-ai.md` 11); the probe's ray set differs (port: ORDNANCE bodies; BF1 mask 0x21f). Card VSUB (hover
  contacts against the soldier subset) bears on it too.
- Also open from HCORN's prototype: the PS2 draws a hover's waypoint in the hub (live: hub 91's waypoints 13.2 / 6.3 m
  from its centre), the port gives a driver the hub's centre (`route.rs` 959-966, OURS). Bit 4 (drivers drawn like
  soldiers): tat1 rock deaths 1, Endor 0.280 a ride, but a NEW tat1 spot (-216, 24) with 27 deaths - trace it first.

## Acceptance
- The corner radius per hover class from the class's own box, baked; a test that a BF1 bike's corner lands 2.94 m out
  (r 2.439) and a BF2 bike's 2.45 m.
- BF1 Endor: fast ticks within 3 m of a hover barrier within 1.5 points of the PS2's 10.65 % (`tools/contact.py` /
  the HJC f3 census); crash per ride reported against the PS2's 0.28 with the cause of the gap named (a trace of where
  the PS2's extra crashes come from, or a measured knock-in that closes it) - landing at 0.13 needs that answer first.
- tat1 rock deaths stay <= HCORN's; the 14 hover maps within noise of main otherwise.
