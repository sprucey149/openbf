---
id: LSUM
title: The light sum in display units, and no ambient floor: the PS2 adds ambient + sun + points in GS units and multiplies by the texel once; the port sums linearised colours in linear light (Kamino's lit decks 1.4 x dark) and floors 8 maps' ambient at 60/255 - the prototype takes all 954 pairs 10.62 -> 9.74 with no map up by more than 0.5
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/render/src/shader.wgsl,engine/crates/bake/src/lighting.rs,engine/crates/render/src/lib.rs,engine/crates/render/src/lighting.rs,engine/crates/render/tests/light_path.rs,engine/CRATES.md
declares: goldens=maps/bf1_bes1.png,maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf2_cor1.png,maps/bf2_dag1.png,maps/bf2_dea1.png,maps/bf2_end1.png,maps/bf2_fel1.png,maps/bf2_geo1.png,maps/bf2_hot1.png,maps/bf2_kam1.png,maps/bf2_kas2.png,maps/bf2_mus1.png,maps/bf2_myg1.png,maps/bf2_nab2.png,maps/bf2_pol1.png,maps/bf2_tan1.png,maps/bf2_uta1.png,maps/bf2_yav1.png,maps/tat1.png,maps/tat2.png,tat1_hud.png,tat1_overview.png,tat1_spawn.png,units/bf1_all_inf_marksperson.png,units/bf1_all_inf_marksperson_snow.png,units/bf1_all_inf_pilot.png,units/bf1_all_inf_snowtrooper.png,units/bf1_all_inf_soldier.png,units/bf1_all_inf_trooperfleet.png,units/bf1_all_inf_wookie.png,units/bf1_cis_inf_bdroid.png,units/bf1_cis_inf_sbdroid.png,units/bf1_imp_inf_atatpilot.png,units/bf1_imp_inf_atstpilot.png,units/bf1_imp_inf_darktrooper.png,units/bf1_imp_inf_pilot.png,units/bf1_imp_inf_scout.png,units/bf1_imp_inf_shocktrooper.png,units/bf1_imp_inf_snowtrooper.png,units/bf1_imp_inf_stormtrooper.png,units/bf1_rep_inf_arctrooper.png,units/bf1_rep_inf_jettrooper.png,units/bf1_rep_inf_pilot.png,units/bf1_rep_inf_trooper.png,units/bf2_all_inf_bothanspy.png,units/bf2_all_inf_engineer.png,units/bf2_all_inf_marksperson.png,units/bf2_all_inf_markspersonsnow.png,units/bf2_all_inf_snowtrooper.png,units/bf2_all_inf_soldier.png,units/bf2_all_inf_tantive4trooper.png,units/bf2_all_inf_vanguard.png,units/bf2_all_inf_wookie.png,units/bf2_cis_inf_bdroid.png,units/bf2_cis_inf_magnaguard.png,units/bf2_cis_inf_sbdroid.png,units/bf2_imp_inf_atatcommander.png,units/bf2_imp_inf_atatpilot.png,units/bf2_imp_inf_darktrooper.png,units/bf2_imp_inf_gunner.png,units/bf2_imp_inf_scout.png,units/bf2_imp_inf_shocktrooper.png,units/bf2_imp_inf_snowtrooper.png,units/bf2_imp_inf_stormtrooper.png,units/bf2_rep_inf_arctrooper.png,units/bf2_rep_inf_clonecommander.png,units/bf2_rep_inf_ep3armoredpilot.png,units/bf2_rep_inf_ep3heavytrooper.png,units/bf2_rep_inf_ep3jettrooper.png,units/bf2_rep_inf_ep3sniper.png,units/bf2_rep_inf_ep3trooper.png,units/bf2_rep_inf_feluciatrooper.png,units/bf2_rep_inf_jettrooper.png,units/bf2_rep_inf_pilot.png,units/bf2_rep_inf_trooper.png,walkabout_stats.ron; rebake=maps; parity=bf2_cor1,bf2_mus1,bf2_nab2,bf2_yav1
needs:
effort:
branch: lsum
resume: landed bf263f81 2026-10-05
created: 2026-09-26
defer: 
value_pre_alpha: 4
alpha: yes
---

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
