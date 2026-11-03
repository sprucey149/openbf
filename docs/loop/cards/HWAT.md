---
id: HWAT
title: Hovers float on water (re-hover §4 S3): the sim gets the map's WMAP wet grid + LAYR height + the 6 cm bob, and each spring point's query compresses on it (115 of 138 RAM hovers have FloatsOnWater on)
status: archived
issue: 70
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #70](https://github.com/sprucey149/openbf/issues/70).

## Why
Split from HOVX (2026-09-28). A hover driven onto water sinks to the bottom in the port; on the PS2 its spring spheres compress on the water surface (FUN_005613c8) and it skims.
## Evidence
re-hover §4 S3 (B2:134942-134963): if K+0xe76 (FloatsOnWater, hash 0xefb6b10a, ctor 1; only cis_tread_snailtank authors 0): d = h - (centre.y - r); if d > 0: c_i = max(c_i, min(d/len, 1)); if d/len > 1 and v.y < 0: v.y *= 0.5, v.xz *= 0.9, omega *= 0.9. FUN_005613c8 (B2:696593-696621): only with a water grid; clamps (x, z) into it; a wet cell (FUN_00561ef0, WMAP nibble != 0 — re-water §4) returns h = the first LAYR height + DAT_0060bef0 (the bob, 0.06·sin(1.8849558·t), zero when OscillationEnable = 0, BF2 FEL1 — re-water-flags §7). mungefs::water already parses WMAP (is_wet); the manifest carries only the height and the GLB, so the wet grid (grid, cell size, origin, nibbles) must be baked into ManifestWater.
## Acceptance
Unit test pins S3 against a hand number (a skiff sphere 0.5 m under a flat water plane: c = 0.5/len; the bottomed damping); the wet grid baked for the 12 water maps (rebake=maps, the manifest RON grows); hover_drive on a water map's pad (BF2 KAS2 beach bike, NAB2) rides the surface; sweep outcomes unchanged (only hashes). The bob needs the sim time (t = ticks·dt) — PROVISIONAL phase.

## Notes
- 2026-09-29 (PXWB, lane 2): a parked hover on water, the PS2 vs the port — FEL1's combat speeder on the river at cp6-1 rides 0.874 m higher on the PS2 (RAM matrix y 10.46 vs the port's 9.59 after 2 idle ticks; xz 4.3 cm, heading 0.02°), clear of the water where the port's sits in it: `pcsx2/ref/bf2_fel1/walk/pairs_veh/cp1_cp6_1_b000.png`, table `lead_tmp/pxwb/veh/bf2_fel1_table.md` (the other FEL1 speeder, on land: +0.20 m).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

- 2026-10-06 (card WADE, lane 2): card WDRN (the soldier's drowning) depends on this card's wet grid + LAYR + bob in the sim. Measured on the PS2 for the same query: the bob word is 0 on BF2 FEL1 (`OscillationEnable(0)`), and a soldier in a DRY cell 4.8 m under FEL1's plane gets no water at all (`analysis/re-water.md` section 9.2).

## Note from AUDIT HSTEP (2026-10-16)
BF1's spring point has the same float but NO class gate: B1 FUN_001ee040 (B1:99812-99830) runs the water probe
FUN_004768f0 (a water grid present DAT_005648dc, the cell clamp, the wet test FUN_00477458 - BF2's FUN_005613c8 shape)
on every spring every tick and raises E+0x2170+32i by max, with the same v.y *= 0.5 / v.xz, omega *= 0.9 overshoot rule;
BF2 gates it on K+0xe76 (FloatsOnWater). Read by me and by both HSTEP-audit verifiers (`lead_tmp/audit-hstep/wf/V1`,
`V2`). Build the BF1 rule ungated (by the class's game), not BF2's flag for both.
