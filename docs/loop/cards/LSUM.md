---
id: LSUM
title: The light sum in display units, and no ambient floor: the PS2 adds ambient + sun + points in GS units and multiplies by the texel once; the port sums linearised colours in linear light (Kamino's lit decks 1.4 x dark) and floors 8 maps' ambient at 60/255 - the prototype takes all 954 pairs 10.62 -> 9.74 with no map up by more than 0.5
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The PS2's VU1 computes a model's vertex colour as `amb·mat + Σ Cᵢ·mat·termᵢ` in GS units, and the GS multiplies it into
the gamma-space texel once. The port linearises each colour (`bake::lighting`'s transfer rule) and sums in LINEAR space.
That is exact for a single product but not for a sum or a weight. It reshapes every mid-tone on every lit model, in both
games.

## Evidence
`analysis/re-sun-direction.md` §4.3. The SUND scratch knob `SUND_GSUM=1` sums in GS units and linearises once
(`openbf-wt/sund-scratch`, shader.wgsl). The census is `lead_tmp/re/sund/census_pair.py`:
- **BF1**, with SUND's direction, d⁴ and prelit rule (`ps2d4s → ps2d4s_g`): 67,927 blocks, 14.76 → 10.06, 71.9 % closer.
  - d⁴ against Lambert: 65 % closer under the GS sum, 51 % under the linear sum.
  - All of SUND plus the GS sum: 82 % closer (22.6 → 9.3).
- **BF2** (`ps2 → ps2_g`): 48.5 % closer overall.
  - Better: KAS2 85 %, POL1 85 %, TAT2 80 %, KAM1 62 %.
  - Worse: COR1 3 % (5.5 → 21.3: the floor turns too bright, `lead_tmp/re/sund/sheets/lsum_bf2.png`), MYG1 11 %, MUS1
    4 %, NAB2 31 %.
- LITE's own COR1 measurement matched the PS2 WITH the linear sum (`re-model-lighting.md` §8).

## Notes
- The scratch knob's WGSL is the starting point: sum `linear_to_srgb` of each baked colour × its weight, clamp at
  255/128, then `srgb_to_linear` once (`openbf-wt/sund-scratch` diff; it is removed at SUND's cleanup — the diff is saved
  at `lead_tmp/re/sund/scratch_knobs.patch`).
- **FOLGL data point (2026-09-28, BF2 FEL1, `analysis/re-foliage.md` §16).** At the four PCSX2 poses, with the masks from
  `lead_tmp/re/folgl/measure.py`:
  - Sun-lit caladium leaves (`fel1_prop_foliage_d`): the linear sum is +1.4 −0.2 −1.1 against the PS2; the Lit sum in
    GS units is +6.9 +5.3 +2.1 (worse).
  - `fel1_prop_foliage_b`: −3.7 −2.9 −6.3 linear, −3.1 −2.1 −6.1 in GS units.
  - The same shader edit is saved at `lead_tmp/re/folgl/gsum_knob.patch`. One more BF2 map where GS units lose.
- 2026-09-29 (PXWB, lane 2; FINDINGS F49): BF2 TAT2's sun and shade — the PS2's shaded walls are neutral/cool grey, its sunlit walls bright pink-orange; the port paints both a flat warm beige: `pcsx2/ref/tat2/walk/pairs/cp1_cp2__sheet.png` b180 (20,300,200,440) (85,90,86) vs (90,80,60); `cp5_cp8__sheet.png` b180 (300,80,620,300) (76,78,77) vs (80,69,52); `cp3_cp1__sheet.png` b000 (300,20,620,200) (157,179,183) vs (158,144,115); `cp2_cp6__sheet.png` b000 the sunlit tower (60,40,200,300) (186,139,112) vs (123,112,89). The HUE of the shade (blue channel +25..+68 on the PS2) says the ambient's colour, not only the sum's colour space.
