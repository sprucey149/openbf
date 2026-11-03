---
id: BZONE1
title: Buildable turrets on BF1: com_weap_gunturret authors the same build-zone + hologram props as BF2's unbuilt-at-start turrets (VTUR2) — does BF1 start them unbuilt? Find the zone key's ELF reader in both eras and a PS2 frame/RAM read of a BF1 gun turret at battle start
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
VTUR2 found that BF2's buildable turrets start UNBUILT. The PS2 shows a flat `com_prop_buildzone` pad, not the
turret, at BF2 COR1 / TAT2 / YAV1. Their `built` twins stand as turrets. The marker is entc PROP 0x97C931C5 =
`com_prop_buildzone`, beside the hologram 0x48542041 = `com_holo_turret`. The port applies it on BF2 only
(`manifest::BUILD_ZONE_PROP`).
BF1's `com_weap_gunturret` (bf1_common) authors the SAME pair, as does `com_prop_barrier_02`. VTURB spawns every
BF1 gun turret as a boardable, built turret (NAB1 21, KAS2 16, …). If BF1 also starts them unbuilt, those are wrong.

## Evidence
- `engine/artifacts/lead_tmp/vtur2/pose/bf1_nab1_turrets.png` (PS2 | port on main):
  - NAB1 `walk/cp0_cp1_b000`: a gun turret instance 7.7 m ahead. The PS2 shows a small flat light shape at the
    soldier's feet there.
  - `walk/cp3_cp11_b270` / `b090`: one 12 m ahead; nothing standing is visible on the PS2.
  - The port's pose_render does not merge common, so it draws no BF1 gun turret at all there. Not a comparison.
- Both ELFs read the hologram key into the building: B1 FUN_001c0b10 B1:67354 → +0x1624; B2 FUN_001e4970
  B2:94407 → +0x624.
- `re_check.py absent` finds 0x97C931C5 in neither decomp, in any spelling tried (0x97c931c5, -0x6836ce3b,
  2546545093). Its reader is untraced: try widescan/immscan for the lui/ori pair.

## Acceptance
- A PCSX2 frame or RAM read of a BF1 gun turret at battle start (built or unbuilt: e.g. ps2live's entity walk, or a
  clear camera on NAB1's cp0 turret), and the zone key's ELF reader in both eras.
- Then either extend the BF2 rule to BF1 (`add_turrets` unbuilt_zones, `Scene::resolve_class`), with the sweep move
  declared, or record why BF1 differs.
- Also: how an engineer builds one (BF2 has engineers; the build action and its time).
## AUDIT VTUR2 (2026-09-30) — the RE this card asked for is done; phase → build, deps → TBLD
- The zone key's reader exists in both eras, and the "absent in neither decomp" line above is false. Ghidra prints the
  constant sign-extended, and `re_check.py absent` matches whole tokens.
  - B1 FUN_001c0b10 B1:67394 / B2 FUN_001e4970 B2:94473 (`0xffffffff97c931c5`).
  - The key is `UnbuiltGeometryName` (FNV). It names a MODEL (B1 table 0x5818c8 → class+0x156c; B2 0x684530 →
    class+0x4d4) plus its "UnbuiltCollisionMesh".
  - 0x48542041 is `UnbuiltHoloOdf` (class+0x1624 / +0x624).
- BF1 does what BF2 does. The building ctor B1 FUN_001bf400 sets entity+0x2a4 = 2 (unbuilt) when class+0x156c is set
  (B1:66399-66405). BF1 COMMON/ENG/COMMON.LVL carries the `com_prop_buildzone` model, so the lookup succeeds.
- PS2 RAM (`lead_tmp/audit-vtur2/bstate1.py`; ArmedBuilding vtable 0x52ef60 from FUN_001c4378): every BF1
  `com_weap_gunturret` starts unbuilt (state 2) in the 1-bot and walk states.
  - KAS2 16/16 in both eras.
  - NAB1 21/21.
  - In NAB1's normal-AI side-select state, 8/21 are already built (state 0).
- The verifier supported C3, C5 and C14 (`lead_tmp/audit-vtur2/verify/`).
- So extend the BF2 rule to BF1. Also cover `com_prop_barrier_02`: BF1's `resolve_class` must then draw its build zone.
- Do NOT land this before TBLD (bots build turrets): alone it turns NAB1's 21 and KAS2's 16 turrets into pads for the
  whole battle, where the PS2's bots build them within seconds.

## TBLD's RE (2026-10-03, `analysis/re-build-zones.md`) - deps now BLDR
- BF1 does build its gun turrets: live on Naboo Plains the two sides' pilots (UnitRepairAgent) add 30 a shot every
  0.5 s (10 shots, 4.4-4.5 s for the 300 HP turret), built count 7 of 21 at the sample's start -> 20 of 21 at 225 s.
- A fallen BF1 gun turret also rebuilds itself: no `RespawnTime` key -> the armed building's default 120 s ->
  regeneration 2.5 HP a second (measured, 8,366 samples). An initially unbuilt pad has regeneration 0.
- `com_weap_gunturret` instances: 129 on 12 BF1 maps (BES2 5, END1 3, KAM1 16, KAS2 16, NAB1 21, NAB2 9, RHN1 14,
  RHN2 10, TAT1 12, TAT2 11, YAV1 6, YAV2 6); showcase maps NAB1, TAT1 (the port's `tat1`), YAV2.
- `com_prop_barrier_02` has no instance on any BF1 map (disc census): nothing to do for it.
- This card needs TBLD (states), FCUT (the cutter) and BLDR (the bots) first; acceptance: the BF1 built count over
  time on `nab1` beside the PS2's curve, and no map's sweep round lost.

- 2026-10-22 06:50 (card TSLG, lane 2): **answered by RAM - BF1 starts every gun turret UNBUILT.** Each BF1 map's kept one-unit side-select state (`ref/states/<map>_units1_*_sideselect.p2s`), every ArmedBuilding (vtable 0x52ef60, state E+0x2a4): all 118 `com_weap_gunturret` are state 2 (BES2 5, END1 3, KAM1 16, KAS2 16, NAB1 21, NAB2 9, RHN1 14, RHN2 10, YAV1 6, YAV2 6, TAT1 12); every other BF1 turret class is state 0 (bes_bldg_aaa_turret 8, geo_bldg_geoturret 10, hoth_bldg_dish_turret 12, hoth_bldg_hoth_turret 12, yav_bldg_tower_turret 7). Positions and facings equal the port's instances (167 / 167, 0.000 m, 0.00 deg). The port spawns the 118 as built turrets. Script `lead_tmp/tslg/tools/census_bf1.py`, output `lead_tmp/tslg/census_bf1.txt`. Caveat: a units1 battle's level load runs before the bots are held, so a pad a bot built before side select would read 0 - none did. TBLD's NAB1 rows agree (21 pads at state 2, health 0 / 300, `lead_tmp/tbld/ps2/nab1_a_pads.csv` t 0.3).
