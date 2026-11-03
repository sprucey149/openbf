---
id: BRGT
title: BF1 bright areas overshoot the PS2 once the DomeInfo filter is on (BFLT): NAB1 median dluma +9.8, BES2 +6.4, RHN1 +3.9; KAM1 and RHN1 mean block error +0.7 — the port's bright surfaces (KAM1's storm sky, NAB1's lit stone) are already too bright before the filter; attribute by surface and card the cause
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
BFLT ports the PS2's frame filter faithfully (its own output within 2 levels of the measured model), which moved TAT1 −15.1 → −0.1 median dluma. On maps whose bright areas the port already over-draws, the lift overshoots.

## Evidence
- `lead_tmp/bflt/parity/{on,off}/scores.csv` (BFLT branch, scratch bake, filter on vs OPENBF_NO_FILTER=1; the off run = main's scores exactly): NAB1 dluma −4.8 → +9.8 (block 17.14 → 16.40), BES2 −2.3 → +6.4, RHN1 −0.7 → +3.9 (block +0.71), KAM1 −6.1 → +0.6 (block +0.66).
- `lead_tmp/bflt/parity/sheet_bf1_kam1.png` (PS2 | off | on): the port's KAM1 storm sky is far brighter than the PS2's before the filter.

## Result (2026-10-03, main 6bc2437; report `reports/20261003_0425_card_BRGT.md`; data `lead_tmp/brgt/`)
Instrument: a scratch `surf_render` (pose_render + `SURF_KEEP`) labels every port pixel sky / terrain / water / model; its
filtered frames score exactly as main's parity (954 pairs, max difference 0.0005). The PS2 frame with the filter undone
(v = (v' + gT) / (1 + g) above T) against the port's filter-off frame says whether a surface or the filter is over.
**The filter is not the cause anywhere: it multiplies an excess the surface already has by ~1.3.**

| map | surface | share | pre-filter dluma | with the filter | of the map's dluma |
|---|---|---|---|---|---|
| NAB1 +10.6 | terrain | 0.37 | +14.8 | +20.2 | +7.5 |
| | sky | 0.49 | +6.3 | +8.1 | +4.0 |
| | models | 0.14 | -5.7 | -6.8 | -0.9 |
| RHN1 +5.2 | terrain | 0.40 | +13.2 | +16.0 | +6.4 |
| | sky / models | 0.13 / 0.47 | +4.8 / -3.9 | +5.1 / -4.1 | +0.7 / -1.9 |
| BES2 +6.4 | models | 0.89 | +5.9 | +6.9 | +6.2 |
| KAM1 -11.5 | models | 0.61 | -13.6 | -17.2 | -10.5 |
| | sky | 0.36 | -4.0 | -4.2 | -1.5 |

- **NAB1 terrain** = the missing DTLX detail pass: nab1_detail's alpha is 94.8, x0.743 near the eye. Card TDTL raised to 4,
  alpha (its note has the numbers; upper-bound what-if NAB1 block 16.40 -> 12.85, dluma +10.6 -> +0.1).
- **Sky** over on 10 of 11 maps measured: new card SKYB (re, value 3).
- **RHN1 terrain**: far snow drawn white where the PS2 shows dark haze (5 poses 1.2-1.4 x) - note on FARC; the other
  20 poses are 1.03-1.08 x, cause not named.
- **BES2 models**: +5.9 before the filter on every pose group, and warmer (R 1.08, G 1.05, B 1.01; on 12 of 28 poses
  the PS2's shaded walls are blue-grey where the port's are brown, `lead_tmp/brgt/sheet_bes2.png`). Not a showcase map,
  cause not named, no card (card rule: 2 per pass) - planner: a `defer: post-alpha` card if wanted. Pose
  cp0_cp4_postA_v2 renders a flat grey frame in the port (camera inside a wall or a missing model).
- **KAM1** no longer overshoots (KSKY landed): it is under, on the decks (card B1DK).
- Nothing was changed in the port: no map's block error moved.
