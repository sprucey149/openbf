---
id: ELFR3
title: BF1 Normal-class 0x04 words blend on the PS2 (re-material-flags §0.4 [V]) but the census bakes them OPAQUE or MASK: YAV1's free-standing temple holo panels (yav_bldg_controlroom_maps 0x4) and BES1/HOT1's command-centre holo maps (HOTH_bldg_command_center_maps 0x5) draw solid green where the PS2 sees through them
status: archived
---

Historical technical record. Recorded state before archival was `done`.

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
