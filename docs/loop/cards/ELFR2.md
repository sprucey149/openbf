---
id: ELFR2
title: BF2 0x04 segments blend by texture alpha and BF1 Normal-class 0x04 words blend (re-material-flags 0.3 / 0.4 [V]) but the alpha census bakes them OPAQUE or MASK: POL1 / COR1 windows draw solid panes, YAV1's and BES1 / HOT1's holo screens solid green (merged: ELFR3)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BF2's per-segment draw (`FUN_0058ed70`) composites every segment whose MTRL word has bit 0x04 (or c.a != 0xff, or draw
flag 0x800000): ALPHA 0x44, Z test with no Z write, As = texture alpha x vertex alpha (`analysis/re-material-flags.md`
§0.3 / §0.5 [V 723411-723480]). The port's ALPHA RULE v2 census (`bake/src/alpha.rs::verdict`, PROVISIONAL) overrides
the bit in two branches and bakes the material OPAQUE (texture alpha forced to 255):
- rule 3's last branch (0x04, no modulation bit, amax >= 193 and a smooth ramp, mid-band > 0.30 — an all-255 alpha
  would have come out MASK): POL1's windows `Pol1_bldg_window` (MTRL 0x10004 on the eight `pol1_bldg_radar_window_*`
  models and `pol_bldg_hanger_control_window`, 0x10205 on the three `radar_center_window_*`) and COR1's temple windows
  `cor1_bldg_youngling_room01` (0x10205 / 0x204 on ten `*window*` models); words read by `lead_tmp/pxwb/mtrl2.py`;
- rule 2 (a modulation word with no scroll bit is OPAQUE): POL1's landing / health-room windows (`pol1_bldg_window`,
  0x10085 / 0x10285 — the 0x80 specular bit) and YAV1's `scanlines` holo screens (0x1000005, RTYP 3).
The ELFR card moves BF1 to its ELF rule and leaves BF2 "unchanged unless its own §0.3 rule is ported with a PCSX2
check" — this card is that check and that port.

## Evidence (PXWB, 2026-09-29; `engine/artifacts/pcsx2/ref/<map>/walk/pairs/<stem>__sheet.png`)
- POL1 `cp4_cp1con` (all four bearings) and `cp3_cp4con` (b000, b180, b270): the PS2 looks through the radar room's
  windows onto the base and the misty asteroid surface; the port draws an opaque teal pane. Structure (mean luminance
  gradient, `lead_tmp/pxwb/boxm.py`): cp4_cp1con b090 (60,80,590,330) PS2 1.53 vs port 0.98; b270 (130,30,560,330)
  1.75 vs 0.84; cp3_cp4con b270 (20,60,300,300) 1.32 vs 0.89 (the PS2's frames are the blurrier ones). Crop:
  `pairs/cp3_cp4con_b270.png` (a hill silhouette and a mist bank through the PS2's glass).
- COR1 `cp0_cp5` (all four) and `cp2_cp3` (b090-b270): the courtyard, a lit doorway and the city lights through the
  PS2's windows, an opaque blue-grey pane in the port — cp0_cp5 b000 (10,20,230,250) gradient 0.88 vs 0.42, b090 1.00
  vs 0.42; cp2_cp3 b180 (320,110,630,230) 1.51 vs 0.75.
- YAV1 `cp1_temple` b180 (FINDINGS F47): the holo screens see-through on the PS2, solid green in the port.
- Baked: every full-resolution model's material above is OPAQUE and its texture's alpha is 255 everywhere
  (`pol1_bldg_window.ktx2`, `cor1_bldg_youngling_room01.ktx2` — forced by `make_opaque`, [D] from the verdict); the
  radar windows' LOWD models (`Pol1_bldg_window_lowrez2`, 0x10004) bake MASK.

## Acceptance
- Behind ELFR's flag (or its BF2 twin), BF2 world segments take the §0.3 rule where the census disagrees: 0x04 (and
  c.a != 0xff) composites with the texture's alpha, sorted like the port's other blends.
- A census first: every BF2 material whose verdict changes, per map (the rule-2 and rule-3 branches above, and any
  other); view the goldens they touch and declare exactly those + `rebake=maps`.
- Re-render the pairs above (`port.py <map>/walk <spot>`, `pairs.py`, `walk_sheet.py`) and VIEW them: the outside
  shows through the POL1 and COR1 windows, the YAV1 holo screens are see-through; no new holes (`walk_index.py`).

## Notes
- Created 2026-09-29 by PXWB (lane 2), FINDINGS F62. MUSW (MUS1's control-room windows) may be the same family: check
  `mus1_prop_window`'s word (baked OPAQUE, flags 0x10201 — no 0x04, so probably not).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

## Merged from ELFR3 (planner 2026-09-30 21:45; the maintainer's batch rule: value <= 3, shared files — one session, one gates run)
**ELFR3's title:** BF1 Normal-class 0x04 words blend on the PS2 (re-material-flags §0.4 [V]) but the census bakes them OPAQUE or MASK: YAV1's free-standing temple holo panels (yav_bldg_controlroom_maps 0x4) and BES1/HOT1's command-centre holo maps (HOTH_bldg_command_center_maps 0x5) draw solid green where the PS2 sees through them

## Why
ELFR (landed the Normal-without-bits OPAQUE and Scroll|0x04 BLEND overrides) found that the PXWA "temple holo panels" are NOT
the Scroll `scanlines` material: the free-standing panels in `pcsx2/ref/bf1_yav1/walk/cp1_temple_b180` are
`yav_bldg_controlroom_maps` (flags 0x4, Normal class, a0 0.336 mid 0.325 → census rule 3 OPAQUE); `PXC_DROP_LAYERS=Opaque`
removes them (ELFR report). The ELF blends every Normal 0x04 word without 0x02 (ALPHA 0x44, Z-tested, no Z write).
BES1's control-room panels (`bes1_bldg_console_b`, `HOTH_bldg_command_center_maps` 0x5) are the same case.
## Scope — the census (ELFR scratch bake, `lead_tmp/elfr/census/`): 44 BF1 materials, Normal class, 0x04 without 0x02, not BLEND:
most are bimodal MASK cutouts (bamboo LOWD, fences, gates, moss, sky mountains/planets) that would draw the same shape
blended; the risky ones: `kam_crate` 0x05 (§0.6(a), grime alpha mid 0.55 — the shredded-crate case ALPHA RULE v2 was made for),
`kam_bldg_chamber_floor`/`kam_bldg_interior_floors` 0x10045 (alpha 224–229 everywhere → a 0.9-opaque floor), `Rhn1_prop_floor_01`
0x45 (mid 0.99), `nab2_bldg_rot_floor` 0x10005 (mid 1.0), the RHN2/BES1 sky clouds (sky path: D7).
## Acceptance
- Grab PCSX2 frames of the KAM1 crate and a KAM1 interior floor (and RHN1's tunnel floor) first: do they show see-through?
  If yes, take the full Normal 0x04 → BLEND rule in `bf1_elf_verdict`; if not, find what the PS2 does to their alpha
  (vertex alpha? the CLUT scale makes 229 → 115/128) before splitting the rule.
- Re-render `bf1_yav1/walk cp1_temple` and `bf1_bes1/walk cp0_cp9, cp1_cp12` (ELFR's `lead_tmp/elfr/render.sh` pattern:
  before/after from the same pxc_pose): the panels see-through. Declare the goldens the census names.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

Both halves have their own acceptance above; land them as two commits on one branch, one `land.py`.
