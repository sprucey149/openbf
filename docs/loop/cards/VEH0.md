---
id: VEH0
title: Vehicles (P7): what the PS2 does and the build plan — the first M2 card
milestone: M2
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
branch:
resume: done 2026-09-25: analysis/re-vehicles.md [O] (verifier 38/42 SUPPORTED, 4 PARTLY folded); cards HVRE VCLS HOVR VSEAT VPAD VBAKE VDMG VTUR VAI VWLK VFLY AIMF; VENT amended
created: 2026-09-25 (restructure)
---

## Why
M2 (P7 ground vehicles, then flyers) has never started; a port without vehicles is the biggest gap a player sees. PLAN §LOCKED FEATURE SET → Vehicles & space, §Phases P7 and P6.5 E (vehicle AI lands with P7).

## Notes (planner 2026-09-25)
- The RE must answer PLAN §DESIGN SPECS "P7 DESIGN — entity shape"'s seven questions in its order: (1) hover/walker/flyer
  class props + the three integrators' PS2 addresses; (2) seats, enter/exit (BF2 `EnterVehicle`/`ExitVehicle`; BF1 has no
  binding); (3) damage regions, `*_chunk1..4` / `*_exp`; (4) vehicle spawn pads; (5) turrets as seats on buildings;
  (6) BF1 aim assist vs vehicles; (7) BF1 vs BF2 handling. First drivable target: BF1 TAT1 `all_hover_skiff`.
- From HOLD 1 (archived HANDOFF): the 0x04 blend bit sits on 25 BF1 / 39 BF2 side models (flyers, gunships, heroes);
  `bake::soldier::segment_blends` REFUSES 0x02 and a 0x04 word with 0x08/0x40/0x80 — a skinned vehicle needing either
  needs that pass first (`rep_walk_atte` carries 0x02). Vehicle `zaa_` sound banks are unparsed (HOLD 4).
- Already split off: VPRE (behaviour-neutral prerequisites, eligible now) and VENT (the entity skeleton, deps VEH0).
  Your split adds the rest (hover integrator, seats/enter-exit, damage, spawn pads, render, AI, walker, flyer).

## Result (2026-09-25, lane 2)
`analysis/re-vehicles.md` [O] UNAUDITED answers the seven questions (§0) with the census (53 classes, 336 pads, 25 maps),
both games' class code (hover §3, walker/flyer §4, seats §5, damage §6, pads §7, turrets §8, aim §9) and the port survey
(§10); one foreground Opus verifier: 38 SUPPORTED, 4 PARTLY (folded in), 0 NOT SUPPORTED. The split (§11): HVRE, VCLS,
HOVR, VSEAT (first drivable: TAT1's skiff), VPAD, VBAKE, VDMG, VTUR, VAI, VWLK, VFLY; plus AIMF (M3) from the aim
correction; VENT amended. Report `engine/artifacts/loop/reports/20260925_2050_re_VEH0.md`.
