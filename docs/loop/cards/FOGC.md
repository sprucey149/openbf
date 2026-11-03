---
id: FOGC
title: The PS2's fog curve beyond the fogspread diagonal: in the fogged bins BF1 KAS2's sheet takes 0.5-0.65x fogspread(f,f), and patched ramps give different amounts at the same f - read the VU1 UV-matrix fog path (u64 bit 0x100) or patch the GS CLUT to an index ramp in a kept state and read the sampled texel back
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
REDB builds the PS2's fog amount as the disc's `fogspread` table sampled on its diagonal at planar f (`analysis/re-fog-amount.md`
§8, PROVISIONAL). The blend space and the depth are measured right. The curve is not:
- in the fogged bins BF1 KAS2's sheet takes 0.5–0.65× the diagonal (vlevel, f 0.05–0.15);
- with patched short ramps the SAME pixels take different amounts at the same f (0.36 / 0.55 / 0.53 at f ≈ 0.475 on
  900..1700 / 650..1450 / 0..1500 m), fog appears before `near`, and 650..1450 saturates near 0.72.
So the VU program's fog coordinate is not a plain per-pixel (f, f). This matters most on short-ramp maps: BF1 END1/YAV1
90..350, BES1 80..430, BF2 MYG1 0.5..600.

## Evidence
- `analysis/re-fog-amount.md` §4 (the fog matrix in RAM: columns 0 = 1 = planar f), §5.3–§5.4, §10.
- Frames and tables: `engine/artifacts/lead_tmp/re/redb/` (`fogcurve.py`, `fogcurve_{vantage,vcurve,vlevel}.txt`, `natcheck.py`,
  `ps2exp/frames/*`, the kept states `ps2exp/states/redb_{vantage,vcurve,vlevel}.p2s`).
- Pass-2 u64 0x800944 carries 0x100 = the VU1 "UV-matrix program" (`re-vu1-cull.md`, agent-level).

## Acceptance
- The PS2's fog amount as a function of what it really depends on, from one of:
  - (a) the VU1 path that turns the fog matrix into the GS's (s, t, q) — `scripts/tools/vu1dis.py`, VU instr
    i < 0x100 at `.vutext+0x10+8i`;
  - (b) a kept state with the fogspread CLUT (GS VRAM block 0x3950) patched to an index ramp, reading the sampled
    texel back per pixel;
  - (c) a fit that predicts all five patched ramps and the natural one within noise.
- Then a build (or a recorded no-change) replacing REDB's diagonal.

## Notes
Created 2026-09-29 by REDB (lane 2).

## REDB build's census (2026-09-29, lane 1; `lead_tmp/redb/pxc/census.txt`, renders in `pxc/renders/`)
The diagonal rule (planar f, sRGB lerp) against the old linear-light fog, at every PCSX2 pose of the 15 fogged maps,
over 8x8 blocks the change moved: 18,245 blocks, mean |port - PS2| 18.3 -> 11.2; 22 of 35 moved poses closer.
Per map: MYG1 18.0 -> 8.5, KAM1 9.1 -> 7.4, KAS2 17.2 -> 14.9, BF2 HOT1 27.2 -> 26.1, BES1 even, END1 19.1 -> 19.8,
YAV1 21.6 -> 21.8, but **TAT1 22.7 -> 26.1** (cp0 v3: the PS2 hazes the far dunes almost to the fog colour; the new rule
leaves them darker), **GEO1 9.2 -> 10.9** (partly geometry that differs), BF1 HOT1 13.9 -> 15.6 (15 blocks). So on
TAT1 the PS2's amount at large f is ABOVE the diagonal, while KAS2's fogged bins sit below it: another sign the curve is
not a function of f alone. Use these poses as the FOGC test set.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).
