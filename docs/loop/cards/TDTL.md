---
id: TDTL
title: Two terrain draw rules on the same files: the DTLX detail pass (both PS2s multiply the frame by 2 x the detail alpha over the 3x3 patches round the eye; the port draws none) and no terrain on mask-0 patches or where the layers cover < 1 (F16's grey lake bed where the PS2 shows the mirrored sky) (merged: TMSK)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Every terrain names a detail texture in `tern -> DTLX` (hoth_detail, yav_detail_1, fel1_det02, nab2_detail_1, …). Both
PS2s draw it over the ground near the camera and the port does not, so the near ground lacks the PS2's grain. Most maps
average about neutral; BF2 FEL1's detail darkens its ground by about 0.79.

## Evidence
`analysis/re-terrain-lighting.md` §6 [O] (verifier F1).
- **BF2.** FUN_00571bd0 sets texture DAT_006615c0 (706678) and calls FUN_00570f68 (706690), gated by DAT_0060bed6 (ELF 1).
  It draws the 3×3 patches round the eye with descriptor word 0x89 (ALPHA Cd·As) and colour 0xff808080. So the frame is
  multiplied by 2 × the detail alpha.
- **BF1.** FUN_00486e30 does the same (548780), gated by DAT_005229b0 (ELF 1; its stores are orphans).
- **The disc** (`lead_tmp/re/terl/texdump`): the textures' alpha means are 125–128 (FEL1 100.7). The RGB is ~0. The bake
  writes these textures opaque, because no material flags their alpha, so the signal is lost in `assets_baked`.

## Acceptance
- The bake keeps each DTLX texture's alpha.
- The terrain shader multiplies by 2 × the detail alpha (the port's `_dtl` form), tiled by the PS2's own detail UV scale.
  The RE step is to read the UV scale from FUN_005700a8 / FUN_00497868 (0x73d8c0).
- Reach: the PS2 draws only the 3×3 patches round the eye. A modern port may fade the detail out with distance instead of
  cutting it off. Decide, and log the choice in FEEL_NOTES-style in the report.
- Declared moves: the map goldens with near terrain.

## Note (TALPH RE, 2026-09-28, lane 2)
The detail pass does NOT test the patch's layer mask: BF2 FUN_005700a8 reads only entry +10 / +14; BF1 FUN_00485f20
reads +0x14. So near the eye it also multiplies the frame over mask-0 patches that no layer pass drew
(`analysis/re-terrain-layers.md` §2.5; verifier B2). Its descriptor (FUN_00570f68 706151) is ALPHA 0x89 and carries the
same 0x50000 word as the layer descriptor 0x76c080. Reading that word as the GS TEST (Z test on, GEQUAL) is [H].

## Value 3 → 4 (AUDIT TALPH, 2026-09-28, lane 2)
TALPH made this gap visible. With the layers drawn, BF1/BF2 HOT1's near ground is hoth_main_4 (64×64 texels at 64 m per
repeat, often nibble 0xe at the spawns), so the port's close-up snow is smooth where the PS2 frame shows hoth_detail's
fine ripples. In HOT1's `cp2_cp6_postA_spawn` snowR box, high-pass std (the 1.5-12 px band) is PS2 3.4, port with
layers 1.1, and the pre-TALPH port 12.4 (its own per-texture `_dtl` on hoth_main_1). The large-scale structure matches
(σ15 std PS2 7.4, port 7.6), so the layers are right and the grain is this card's. Sheets:
`lead_tmp/audit-talph/render/sbs_bf1_hot1__*.png`, `hot1_zoom.png`; report `reports/20260928_1345_audit_TALPH.md`.

## Note (2026-09-28, KOC's RE, lane 2; verifier side finding, reproduced)
Both gates are settled (`analysis/re-terrain-enable.md` §2.5):
- **The detail pass is live on retail.** BF1 `FUN_00486e30`'s gate `DAT_005229b0` has ELF word 1. Its only stores are
  two stubs missing from the decomp: 0x472690 toggles it, 0x4726a8 sets it. Nothing jumps to them, and no immediate or
  data word holds their address.
- **`FUN_00487908` (→ `FUN_00487020`) is dead on retail.** Its gate `DAT_005229b4` has ELF word 0, and neither the
  decomp nor an ELF scan finds a store.
- **Both run only inside the terrain draw,** which BF1 KAM1 and BES1 switch off (`TerrainEnable 0`). The detail pass
  must follow KOC's skip.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F53) [H]: BF2 END1's forest floor is 20-30 levels darker and dappled on the PS2 near the eye — (300,260,640,470) `pcsx2/ref/bf2_end1/walk/pairs/cp4_cp2__sheet.png` b000 PS2 (37,35,28) vs port (65,60,47); `cp1_cp6__sheet.png` b180 (0,300,280,470) (31,33,24) vs (60,59,41). A candidate for the detail pass's multiply (or the lightmap); measure with the pass on.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

## Merged from TMSK (planner 2026-09-30 21:45; the maintainer's batch rule: value <= 3, shared files — one session, one gates run)
**TMSK's title:** The PS2 draws no terrain on mask-0 patches (BF1 KAS1 951/1024, KAS2's lake bed, END1, RHN2, NAB2, BES2; BF2 END1) nor where the layers cover < 1; the port draws LTEX[0] there — F16's grey lake bed where the PS2 shows the mirrored sky

## Why
The PS2 issues no terrain layer pass for a patch whose PTCH mask is 0 (`analysis/re-terrain-layers.md` §2.5 [O]: B2 FUN_0056e7d8 zeroes the slot's chains, FUN_0056e898 builds none; B1 FUN_004851b0 / FUN_004852e0). Where a vertex's layers cover < 1 it shows what is behind (§5.5 holes). Those areas are each map's floor: BF1 KAS1 -52.2 m (951 of 1024 patches), KAS2's lake bed -5.9 m (403), END1 (872), RHN2 (858), NAB2 (855), BES2 (389), BF2 END1 (872). The port draws them with LTEX[0], which is F16's grey lake bed; the PS2 shows no bed, so its water shows REFL's mirrored sky shells.

## Evidence
`lead_tmp/re/talph/holes_*.png`, `topdown_*.png` (navy = mask-0, red = holes); `engine/artifacts/pcsx2/ref/FINDINGS.md` F16; census `lead_tmp/re/talph/census.txt`.

## Acceptance
- Drop the mask-0 patches and TALPH's LTEX[0] backstop from the terrain draw; the heightfield (sim, collision) is untouched.
- Not before REFL draws the mirrored shells, or the lakes show the black clear colour (CLRB landed 2026-09-28).
- The DTLX detail pass still covers mask-0 patches near the eye on the PS2 (TDTL's; §2.5).
- BF1 KAS1 `cp2_cp2_eastharbor_v2/_v3` lake rows (100,150,600,235): PS2 (24,39,58) — the port today (57,54,51) shows the bed.
## Note (REFL RE, 2026-09-29, lane 2)
REFL is RE-done: `analysis/re-sky-reflect.md` §3–§5 [O], build pending.
- On the PS2 the near lake shows the MAIN-target mirrored copy (the 0x1000031 draw, bucket 3) through the Z-written
  near water, because nothing is drawn under the water on mask-0 patches. Removing that one draw on the PS2 turns the
  near lake from (37.6,60.1,92.7) to (28.0,36.8,43.0).
- REFL's scratch prototype with ALL terrain dropped (a stand-in for this card), at the PS2's KAS1 East Harbor lake
  pose, pitch −19.3° (`lead_tmp/re/refl/port/v3dn_refl_noterr.png`): near-lake box (100–140, 150–450) = (44.2,66.0,95.3),
  against the PS2 natural frame's (37.0,59.6,92.6). The port today shows (68.1,74.7,80.4), the bed.
- That pose, its states and frames: `lead_tmp/re/refl/ps2exp/` (`states/refl_v3dn.p2s`, `frames/refl_v3dn__ctl.png`).

## Note (PXWA, 2026-09-29, lane 2): KAS1's lake at the posts
The PCSX2 walkabout shows KAS1's lake as the port's worst BF1 mismatch at eye height: a flat pale slate band where the
PS2 has dark mirrored water — `engine/artifacts/pcsx2/ref/bf1_kas1/walk/pairs/cp0_cp3__sheet.png` b270 band
(0,255,640,290): PS2 (22,32,34) sd 30, port (75,89,101) sd 8; `cp2_cp2__sheet.png` b090 (a pale lake floor),
`cp3_cp5__sheet.png` b090. Re-render these after the build (`port.py bf1_kas1/walk`).
- **BF1 RHN2 at every cliff-edge post (PXWA):** the port shows a flat grey checkered floor running to the horizon where the
  PS2 shows the cloud sea and drifting mist below the Citadel's edge — `engine/artifacts/pcsx2/ref/bf1_rhn2/walk/pairs/
  cp5_cp6__sheet.png` b090/b180/b270 (below-horizon box (0,245,640,310) at b180: PS2 (71,83,96) sd 29, port
  (108,116,137) sd 13), `cp6_cp5__sheet.png` b180/b270, `cp4_cp2__sheet.png` b180. RHN1's harbour plane
  (`bf1_rhn1/walk/pairs/cp2_cp7__sheet.png` b090/b180, `cp0_cp2__sheet.png` b180) looks the same — check whether RHN1 has
  mask-0 patches too. The most visible TMSK case on BF1 after KAS1's lake; the planner may want value 5.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

Both halves have their own acceptance above; land them as two commits on one branch, one `land.py`.
- Planner 2026-09-30 21:45: `declares:` holds TMSK's eight goldens; the detail pass's own golden moves are added by the build after its first gates run.

## Note from BRGT (2026-10-03): the detail pass is what makes BF1 NAB1 the worst showcase map on parity
Value 3 -> 4, `alpha: yes`, defer dropped (NAB1 is a showcase map; report `reports/20261003_0425_card_BRGT.md`).
- Disc alpha means of the terrain DTLX textures (`lead_tmp/brgt/tools/tex_mean.rs`, factor = 2 x alpha / 255): BF1
  **nab1_detail 94.8 -> x0.743**, **nab2_detail_1 171.4 -> x1.344** (BF2's nab2_detail_1 is 128.0), kas_detail_1 112.1 ->
  x0.879, kas2_detail 139.4 -> x1.094, tat1_detail_1 138.9 -> x1.089, rhn2_main_detail 138.9 -> x1.089; neutral (0.98-1.006):
  bes2, end, geo, hoth, rhn1, yav. So "most maps average neutral" does not hold on BF1 NAB1 / NAB2 / KAS1.
- Measured, PS2 with the DomeInfo filter undone vs the port's filter-off frame, terrain pixels only (`prefilter.py`,
  `chan.py`, `whatif.py`): NAB1 port / PS2 = 1.22 on every channel (bottom screen rows 1.31 = 1 / 0.743 would be 1.35; top rows
  1.16-1.18); NAB2 bottom rows 0.76 (1 / 1.344 = 0.744); KAS1 bottom rows 1.25 (predicted 1.14).
- What-if, an UPPER BOUND (every terrain pixel x the mean factor, then the filter): NAB1 block 16.40 -> 12.85, dluma
  +10.6 -> +0.1; NAB2 10.22 -> 8.78; KAS1 11.11 -> 10.42.
- **Not confirmed on the x1.09 maps:** KAS2, TAT1 and RHN2's bottom rows read 0.98-1.05, not 0.92, and the same what-if
  WORSENS TAT1 (10.64 -> 11.91) and KAS2 (9.97 -> 10.29). Read the reach and the UV scale before trusting a mean (a mip of
  the detail texture far from the eye, or the 3x3 reach); acceptance per map by `parity.py`, TAT1 must not rise > 0.5.
