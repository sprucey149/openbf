---
id: DUSTE
title: The dusteffect class — volumes of big drifting alpha sprites (mist, dust, blowing snow) — is drawn by the PS2 and has no code in the port: BF2 POL1's 8 pol1_prop_mist volumes haze the whole asteroid surface on the PS2 (far ridges and the nebula behind a pale drifting mist), crisp in the port; placed on 10 port maps (BF1 GEO1 TAT1 HOT1 KAM1 YAV1 YAV2, BF2 HOT1 KAM1 POL1 YAV1) (PXWB walkabout pairs)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
PXWB (2026-09-29) at POL1's three outdoor posts: the PS2 draws a pale, drifting mist over the asteroid's surface and
across the sky; the port's surface and nebula are crisp (`engine/artifacts/pcsx2/ref/bf2_pol1/walk/pairs/
cp1_cp6con__sheet.png`, `cp2_cp5con__sheet.png`, `cp5_cp3con__sheet.png`, b000/b180/b270). POL1's fog is off
(`analysis/re-fog.md`: 0..0), so the haze is not the fog pass: the map places eight `pol1_prop_mist` instances, class
base `dusteffect`, texture `greymist`. The port has no dusteffect code (0 hits in `engine/crates`; `game/tests/
load_scene.rs` counts them as "dust effect emitters (no geometry)").

## Evidence
- Boxes (`lead_tmp/pxwb/boxm.py`): cp2_cp5con b180 the mist bank over the far base (0,200,300,320) PS2 (77,79,86) vs
  port (51,50,51); b270 the hills (0,150,640,300) (65,65,75) vs (55,50,51); the sky away from the sun is lifted from
  black — cp1_cp6con b180 (0,0,640,130) (13,14,17), 2.8 % exact black vs (3,3,3), 85.4 %; toward the sun the lit mist
  and the glare together — cp1_cp6con b090 (330,300,640,460) (109,110,115) vs (26,26,29) (the glare half is SUNG's).
- The class, `engine/assets_baked/bf2_pol1/classes.ron` `pol1_prop_mist`, its 16 ODF properties named by FNV match
  ([D], `lead_tmp/veh0/fnv.py`): MinPos "-120 0 -60", MaxPos "120 10 60" (the spawn box), MinVel "-1 0 1", MaxVel
  "-1 2 1", MinSize 30, MaxSize 50 (m), MinLifeTime 7, MaxLifeTime 10, Alpha 0.08, NumParticles 100, MaxDistance 500,
  Texture greymist, HeightScale 0.4, RadiusFadeMin 250, RadiusFadeMax 300, CameraDistance 40.
- Placed instances (the bakes' manifests): BF1 GEO1 `geo_prop_dust` x23 (browndust), HOT1 `hoth_prop_dust` x52
  (whitedust) + `hoth_prop_mist` x15, KAM1 `kam1_prop_mist` x22, YAV1 `yav_prop_mist` x23, YAV2 x9, TAT1
  `tat_prop_dust` x66 (browndust); BF2 HOT1 x52 + x31, KAM1 x11, POL1 x8, YAV1 x23. BES1 `bes1_prop_cloud` and RHN2
  `Rhn2_prop_cloud` are defined with 0 placements.
- The PS2 code: BF2 registers the class label "dusteffect" (string 0x60fcd8 -> B2:16461; its ctor candidate
  `FUN_001bd570` B2:65976, `analysis/re-ai-sniper.md` 207); BF1 carries the RTTI names `10DustEffect`,
  `15DustEffectClass`, `20DustEffectRenderData` (`analysis/bf1-strings.txt` 1652-1653, 3705).

## Acceptance (RE first, the discipline)
- How the PS2 draws a DustEffect: particle spawn in the box (relative to the instance or around the camera —
  CameraDistance, RadiusFade*), motion, size, life and fade, the sprite's blend and colour (lit or not, fogged or not),
  the draw distance and the per-frame particle count; which of the 16 properties are read and where (setters by hash).
- Then a build split: the volumes drawn in the port (a batched additive/alpha sprite pass), POL1's pairs re-rendered
  (`port.py bf2_pol1/walk`) and VIEWED against the PS2 frames above, then the other nine maps' posts.

## Notes
- Created 2026-09-29 by PXWB (lane 2), FINDINGS F63. KFIRE's title names YAV1's mist (`yav_prop_mist`): it is this
  class. HOT1's "falling snow" (F56 -> WTHR) may be `hoth_prop_dust` (whitedust) rather than weather: check here.

- 2026-10-15 (SKIPC's census, `analysis/re-skipped-classes.md` 1; `lead_tmp/re/skipc/census_main_efa2d56.txt`, main
  efa2d56, the map + its conquest layer as a battle merges them): 8 dusteffect classes, 324 instances on 11 maps. Two
  corrections to the list above: BF1 RHN2 places 13 `rhn2_prop_cloud` (not 0); BF2 HOT1's conquest battle has 7
  `hoth_prop_mist` (31 across all of the manifest's layers). On the showcase maps: TAT1 66, YAV2 9, BF2 HOT1 59.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — blowing sand / snow on TAT1 and HOT1. Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Hoth maintainer trial (2026-11-02)

map's existing classes.ron, active instances and source transparency. Hoth
conquest's 52 snow and seven mist placements are covered by a regression.
The follow-up traces world-axis wind, compressed camera-facing cards and bounds
against original RAM/vertex expansion. Premultiplied native blending retains
the original unscaled HDR source color without increasing opacity or counts.
See `analysis/re-hoth-atmosphere.md` for source reads, provisional details and
local evidence. Do not duplicate that implementation. This does not close the
independent verification, BF1 work or the original all-map acceptance above.
