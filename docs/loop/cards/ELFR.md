---
id: ELFR
title: BF1 materials by the ELF's segment rule, not the alpha census: KAS2 City Hall's lower walls are missing (flag 0x1 thatch MASKed away), the platform huts show clear-black between the bamboo, YAV1's temple holo panels (Scroll|0x04) draw opaque — the PS2 draws the first two solid and blends the third (PXWA walkabout pairs)
milestone: M1
value: 5
phase: build
status: done
deps:
files: bake/src/alpha.rs,bake/src/map.rs,bake/src/glb.rs
declares: goldens=maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,walkabout_stats.ron; rebake=maps
needs:
branch: elfr
resume: landed a3d2d65f 2026-09-29
created: 2026-09-29
---
## Why
The PXWA walkabout (PCSX2 at the port's own post stands) shows holes and wrong translucency where the port's ALPHA RULE v2
(the texture-alpha census, `bake/src/alpha.rs`, PROVISIONAL) disagrees with the BF1 ELF's own segment rule
(`analysis/re-material-flags.md` §0.4 [V 568105–568160]: Normal class — 0x02 alpha TEST, 0x04 BLEND, neither = OPAQUE
with no test; Scroll class (0x400) — 0x02 ignored, 0x04 BLEND; §0.6 (b) and §7 asked for exactly this PCSX2 settlement).
- **BF1 KAS2 City Hall's lower walls are missing** (`kas1_bldg_hut_grand`, material `kas2_bldg_hut_walls`, flag word 0x1):
  the texture's bottom quarter is alpha 0 but carries thatch RGB (138,103,77); the PS2 draws it solid to the floor, the
  port MASKs it away and the lake shows through. `pcsx2/ref/bf1_kas2/walk/pairs/cp3_cp3__sheet.png` (b090/b180/b270).
- **KAS2's platform huts show clear-black between the bamboo** (`kas2_bldg_platform_roof` / `_platform_1`, material
  `kas2_bldg_platform_small`, flags 0x1, MASK a0 0.26): port exact (0,0,0) over 1.5–2.8 % of the lower frame, the PS2
  dark brown (43,34,30). `…/bf1_kas2/walk/pairs/cp1_cp5__sheet.png`, `cp2_cp4__sheet.png`, crop
  `lead_tmp/pxwa/frames/kas2_cp5_bamboo_x2.png`.
- **BF1 YAV1's temple holo panels are opaque green in the port, see-through on the PS2** (`yav_bldg_greattemple_contents`
  / `yav_bldg_controlroom`, material `scanlines`, flag word 0x415 = Scroll | 0x10 | 0x04 | lit — the ELF BLENDS it; the
  port bakes OPAQUE, rule 2): left panel PS2 (53,72,69) vs port (51,131,89). `…/bf1_yav1/walk/pairs/cp1_temple__sheet.png`
  b180, crop `lead_tmp/pxwa/frames/yav1_temple_b180_crop.png`. `blinking lights` (0x417) is the same class.

- **More cases (PXWA, the rest of BF1):** BF1 BES1's control-room holo panels (`pcsx2/ref/bf1_bes1/walk/pairs/
  cp0_cp9__sheet.png` b090/b270, `cp1_cp12__sheet.png` b000/b180: see-through with a moving yellow scan line on the PS2,
  opaque green in the port); BF1 GEO1's Techno Union wreck (`geo_bldg_technounion_hull`, a `geo_bldg_wrecked_02` segment
  with flags 0x1 MASKed by the port): the PS2's lattice is solid dark grating, the port's full of see-through holes
  (`bf1_geo1/walk/pairs/cp3_cp10__sheet.png` b000/b090/b180).
## Notes
- Created 2026-09-29 by PXWA (lane 2). The RE is done ([V] in re-material-flags.md); this card is build.
- YAV1's bazaar fountain `yav_waterfall_02` (0x414, BLEND in the port) still reads as a bright near-opaque sheet where the
  PS2 shows a faint stream (`…/bf1_yav1/walk/pairs/cp5_bazaar__sheet.png`, MEDIUM — the soldier hides the lower half):
  check its blend factor (As = texture alpha × vertex alpha 0x80·c.a/0xff) while here.
- 2026-09-29 (PXWB, lane 2; FINDINGS F47): the BF2 twin. YAV1's temple holo screens are see-through on the PS2 and solid green in the port (`pcsx2/ref/bf2_yav1/walk/pairs/cp1_temple__sheet.png` b180 (100,120,200,300) (48,62,49) vs (49,127,84)); YAV1's bazaar obelisk has a bright white waterfall sheet + two gold strips in the port, plain stone with a faint mist on the PS2 (`cp4_bazaar__sheet.png` b090 (290,0,370,280) (71,84,83) vs (108,107,101)). [H] the same segment-flag rule, read on BF2's ELF.
- 2026-09-29 (PXWB part 2, FINDINGS F56, LOW-MEDIUM): BF2 COR1's Jedi Temple windows show the courtyard beyond on the PS2 and an opaque blue-grey pane in the port (`pcsx2/ref/bf2_cor1/walk/pairs/cp0_cp5__sheet.png` b000/b090).
- 2026-09-29 (PXWB part 4, FINDINGS F62): the BF2 cases above (YAV1's holo screens, COR1's temple windows) and POL1's radar-room windows now live on **ELFR2** (the BF2 §0.3 rule, deps ELFR): the census overrides BF2's 0x04 blend in rule 2 (scanlines 0x1000005) and rule 3's last branch (the windows, 0x10004 / 0x10205 / 0x204).
- 2026-09-29 (lane 1, LANDED a3d2d65 + goldens c63deb8; report `reports/20260929_2225_card_ELFR.md`): Normal-without-bits OPAQUE and Scroll|0x04 BLEND overrides; City Hall walls, YAV1 bazaar obelisk, the console screens and END1's Ewok ramps now match the PS2. NOT fixed here, carded: the free-standing holo panels are `yav_bldg_controlroom_maps` 0x4 Normal → **ELFR3**; the KAS2 bamboo black and GEO1 lattice holes are 0x07 words (BF1 AFAIL FB_ONLY) → **ELFB**.
