---
id: KAAD
title: BF1 Naboo Plains fields kaadus: three NAB1 pads key gun_walk_kaadu for the defenders (ClassRepDef / ClassAllDef) and the PS2 spawns them (3 live in the IA states, one ridden) — the port calls the class absent (VPAD) because the missions load it from SIDE\gun.lvl
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Every BF1 Naboo Plains battle on the PS2 has kaadus on the defenders' pads. The port spawns none: VPAD's module doc
(`game/src/vehicle_pads.rs:12-14`) and census test (`game/tests/vehicle_pads.rs:121-128`) assert that the class is
null on the PS2. The VWLK audit (2026-09-29) found this contradicted by the disc and by RAM.

## Evidence (VWLK audit; the Opus verifier supported V8-V10)
- `lua/mission/nab1c.luac`, `nab1i.luac` and `nab1c_h.luac` each read `SIDE\gun.lvl` with `gun_inf_basic` and
  `gun_walk_kaadu`. The class (BASE walker) is defined in BF1 SIDE/GUN.LVL.
- NAB1.LVL: 7 pad instances name the kaadu. 3 key it as both ClassRepDef and ClassAllDef (the defender in either era);
  4 as ClassHisDef only.
  - B1 FUN_002af778 matches team i against column 0 and, for i < 3, column i. Teams 1/2 switch to the Historical row
    only when SetHistorical has set *(DAT_00518388 + 0xa4).
- RAM, `bf1_nab1_sideselect.p2s` / `_spawnmap` / `_cp1_cp2_postA_spawn`:
  - 3 walkers (vtables 0x533b18 / 0x533ae8) whose class+0x14 = 0xc4d0d528 = FNV "gun_walk_kaadu";
  - historical flag 0; team side rows [0, 4 CIS, 3 Republic, 5 Locals];
  - kaadu 0x1da37a0 sits at pad perma_spawn13's XFRM;
  - the moving kaadu carries a team-2 soldier 2.0 m above its origin.
- VPAD's other "absent" class, BF2 YAV1's `re_hover_barcspeeder`, is named only in YAV1.LVL (no side or mission loads
  it). That one stays absent.
