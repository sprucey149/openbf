---
id: FOLGG
title: Grass patches: the PS2's grasspatch sprite clusters — the ferns and grass carpets FOLG places (BF1 END1 KAS1 KAS2 NAB1 YAV1 YAV2; BF2 DAG1 END1 FEL1 KAS2 YAV1) and the grasspatch classes placed as world objects (SKIPC's nab_prop_flowers, yav_prop_grass(_tall))
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The ground carpets the PS2 draws on most jungle/forest maps are `grasspatch` sprite clusters, not models: DAG1's ferns
(`dag1_prop_grass`), END1's `end_fern` / `end_prop_grass`, KAS1's grass and reeds, KAS2's tall grass, NAB1's grass,
YAV1/YAV2's grass, FEL1's `fel1_prop_thick_bush` (FINDINGS F17, F22, F23, F29). FOLG bakes where they go; this card
draws them. The same class is placed directly as world objects on BF1/BF2 NAB2 (`nab_prop_flowers` x53, the Theed
planters) and BF1 YAV1 (`yav_prop_grass_tall` x10, `yav_prop_grass` x4) — SKIPC's grass items, which the port skips today.

## Evidence
`analysis/re-foliage.md` §7 ([O], verifier-checked; RAM-checked templates): the class properties and defaults
(NumParticles / MinSize / MaxSize / RadiusFadeMin / RadiusFadeMax / DarknessMin / DarknessMax / NumParts / Alpha /
MaxDistance / Texture + the unnamed hashes), the template (N sprites uniform in a disc, falloff sink, size, grey,
atlas part per NumParts, tall blades), the atlas tables (ELF .data 0x518948–0x518a57, read at bake time), the
camera-facing quad and the tall-blade quad, the sine sway (phase += dt·0.8·(1 + 0.2·(i mod 5))), the 20 m fade before
the distance (the `prp_` GrassPatch distance for a FOLG instance; the class MaxDistance, white, for a world object),
the instance tint and terrain tilt (§6; a patch is never yawed or scaled).

## Notes
- Blend: the fade multiplies the sprite alpha, so draw blended (sorted) with the texture's alpha; the exact GS state is
  [H] in the RE — match the pairs. The corner ↔ UV order is [H]; check one atlas sheet (end_fern, NumParts 3) by eye.
- Split-screen: keep full density (FOLG's decision); the PS2 divides NumParticles by the camera count.
- LEAF's leafpatch draw is a sibling (B1 `FUN_002ee970`); share code if LEAF's RE shows the same shape.
- 2026-10-05 (card LSUM landed, bf263f8): the light sum is now made in display units and the ambient floor is gone (report `reports/20261005_1505_card_LSUM.md`). The grass patches' own shader (`render/src/grass.wgsl`) still adds ambient + sun + Light2 in LINEAR light; unmeasured - score it with parity on a grass map (bf1_nab1, bf2_fel1) before changing it.
