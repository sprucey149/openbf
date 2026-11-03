---
id: LEAF
title: Leaf patches: the PS2 hangs sprite clusters of leaves on trees' branch hardpoints (BF2 YAV1, DAG1's canopy, KAS2, COR1's planter tree, NAB2's ivy walls; BF1 NAB
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance (rewritten 2026-09-30 by the RE session, lane 2)
- **Why:** the PS2's leafy canopies are `leafpatch` sprite clusters — on props' skeleton hardpoints (nearly every
  leafy tree), as world instances (BF1/BF2 NAB2's 40 ivy vines, BF2 COR1's 37 planter patches), on DAG1's path
  hellbushes and as FOLG `LeafPatch` layers (YAV bushes). The port draws none, so BF2 YAV1/DAG1/NAB2/KAS2, COR1's
  planters and BF1 NAB1's near grove, NAB2 and YAV1/YAV2 read bare. (NAB1's FAR round crowns are the LOWD model →
  card LOD, not this card.)
- **Evidence:** `analysis/re-leafpatch.md` [O] (0597dc9): template transcribed and reproduced for all 23 live classes,
  the attach rule for all 3,078 live objects in 9 PCSX2 states; the GS state and BF1-unlit / BF2-lit proven on
  patched states — the PS2's own with/without-leaves frames are `engine/artifacts/lead_tmp/re/leaf/ps2exp/frames/
  sheet_{bf1_nab1g,bf2_dag1c,bf2_yav1l}.png` (`__never` = no leaves). Pairs: F24/F32/F46/F53/F61/F64 below.
- **Acceptance:** (1) a unit test reproduces RAM templates (the `template.py` classes: positions ≤ 1e-4 m, sizes and
  greys ≤ 1e-5, parts exact) and the per-map attach counts of re-leafpatch §2.3; (2) at the three patched-state views
  (port renders at their poses) the port's leaf-opaque pixel means (mask = `leafmask.py` ∧ PS2 |never − ctl| > 20)
  within ~15 levels of the PS2's `__ctl`, and the F24 canopy box within ~20 (re-measure the port's "before" at the
  build's parent); (3) blended, not alpha-tested — BF2 YAV1's fading canopy must survive; BF1 unlit, BF2 lit on ±Y
  normals (PROVISIONAL); (4) only the declared goldens move, manifests DIFF only for the schema + the leaf maps, sim
  hashes unmoved.

## Original QUEUE.md entry (verbatim, 2026-09-25)
LEAF. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 3 (FINDINGS F24, F32, part of F29); **[RE] FIRST** (job D, at max); FREE; the build is a new draw (bake: the class's properties + the attach; render: a sprite cluster) with its own [REBAKE]/[GOLDENS]] **Leaf patches: the PS2 hangs sprite clusters of leaves on trees' branch hardpoints (BF2 YAV1, DAG1's canopy, KAS2, COR1's planter tree, NAB2's ivy walls; BF1 NAB1, NAB2, YAV1, YAV2); the port draws bare branches** [RE] — evidence `engine/artifacts/pcsx2/ref/bf2_yav1/pairs/cp3_landingzone_post1_v3_up.png` (box (250,20,640,200): PS2 (24,31,8) dense canopy vs port (136,141,143) sky through twigs — reproduced by this session), `…/bf2_nab2/pairs/cp3_cp4_post1_v1_canal.png`. What batch 3 read (its own [V], NOT yet through a §5 verifier): `yav_prop_leaftree` carries `AttachOdf = yav_prop_leafpatch` at `AttachToHardpoint hp_branch_*`; `yav_prop_leafpatch` is `BASE leafpatch` with `NumParticles 30`, `MinSize 7.0`, `MaxSize 8.0`, `Texture yav_leaf`, `Radius 12`, `Height 1`, `Alpha 1.0`, `MaxDistance 1000`; both ELFs register a `leafpatch` class factory — BF2 `FUN_00173d10` decomp 16469–16470 (string 0x60fd08 = "leafpatch", bytes re-read this session at off = va − 0x160000 + 0x1000) → `FUN_001ca9d8`; BF1 `FUN_00172ab0` 15649–15650 (0x527300) → `FUN_001b21e0`; the port has 0 hits for `leafpatch` / `AttachOdf` under `engine/crates` (re-counted) and bakes the classes `geometry: None`. Disc census in FINDINGS F24. RE (§5): how LeafPatch renders (placement of the NumParticles sprites inside Radius/Height, size range, facing, lighting, alpha test/blend, distance fade) from `FUN_001ca9d8` / `FUN_001b21e0` and their renderable; how `AttachOdf` + `AttachToHardpoint` place a child on a hardpoint (does the port attach ANY odf today? — an absence claim, search it); whether the sprite set is seeded (deterministic) per instance. Build: bake the leafpatch class properties + the attach list into the manifest; render the clusters (instanced billboards) at the hardpoints. GATE: the pair above reads leafy (canopy box mean within ~20 levels of the PS2's); bake DIFF only in the declared manifests; goldens only where a leafpatch is in view (declared after the RE); sim hashes unmoved. Files: `bake/src/` (the class/odf reader + manifest), `render/src/` (a new leafpatch draw), `game/src/` (the upload) — disjoint from FOLG's terrain foliage except the manifest schema.

## Note from FOLG's RE (2026-09-26, lane 2, `analysis/re-foliage.md` §8–§9)
- **The Dagobah canopy is this card's** [D]: every DAG1 grand tree / tree group / barrier / vine / hellbush class carries
  `AttachOdf = dag1_prop_leafpatch` (entc census); not terrain foliage and not a MASK-cutoff loss.
- **FOLG places leafpatches too:** BF1 YAV1/YAV2 `prp_` layers 2–3 are `LeafPatch("yav_prop_bush2"/"yav_prop_bush")`
  (256 instances map-wide; FOLG's bake places them, this card draws them). And the `prp_` path block puts props with
  leafpatch attachments along paths: DAG1 `dag1_prop_hellbush` (~32 along `hell_bush`), BF1 YAV2 `yav2_prop_hellbush`.
- The leafpatch draw is B1 `FUN_002ee970` (called from the foliage instance draw `FUN_003465c0` 332731) and the class
  resolve hook `FUN_001b38f8` (333083) — the grasspatch's sibling (`FUN_002ed080`, re-foliage §7); FOLGG builds the
  grasspatch renderer, so read how much the two share.

## Note (PXWA, 2026-09-29, lane 2): BF1 YAV1 and NAB2 at every post
The PCSX2 walkabout shows the leaf canopy missing at every BF1 YAV1 forest bearing (`engine/artifacts/pcsx2/ref/bf1_yav1/
walk/pairs/cp0_landingzone__sheet.png`, `cp2_viaduct__sheet.png`, `cp3_cp1__sheet.png`: dense leaves on the PS2, bare twig
trees in the port) and the leafy planter trees bare on NAB2 (`bf1_nab2/walk/pairs/cp2_cp1__sheet.png` b090,
`cp3_cp6__sheet.png` b180).
- **BF1 NAB1 is the worst case (PXWA):** at every post the PS2 shows round leafy trees and a lush grove where the port
  shows dead bare trunks (106 `nab_prop_trees2`): `engine/artifacts/pcsx2/ref/bf1_nab1/walk/pairs/cp4_cp4__sheet.png`
  (all four bearings), `cp0_cp1__sheet.png` b000/b270, `cp1_cp2__sheet.png` b000, `cp3_cp11__sheet.png` b270. Naboo
  Plains reads as a dead forest in the port — the planner may want this at value 5.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F46): BF2 YAV1 at EVERY post — the PS2's trees carry full leafy crowns closing over the sky, the port's are bare trunks and twigs; the upper band (0,0,640,200) is 50-70 levels darker on the PS2: `pcsx2/ref/bf2_yav1/walk/pairs/cp0_reflectingpool__sheet.png` b090 (58,60,56) vs (114,118,118), `cp6_cp1__sheet.png` b090 (35,36,27) vs (86,88,85), `cp2_viaduct__sheet.png` b090 (30,31,25) vs (96,99,97). The biggest visual gap PXWB has found on BF2 so far (the planner may want value 5).
- 2026-09-29 (PXWB, FINDINGS F53/F61): BF2 DAG1 (dense dark leaf masses on the PS2, bare twisted branches in the port; upper band (0,0,640,160) `pcsx2/ref/bf2_dag1/walk/pairs/cp3_cp4__sheet.png` b180 PS2 (30,37,41) vs port (53,65,77)) and BF2 NAB2 (Theed's trees and ivy leafy on the PS2, bare in the port: `bf2_nab2/walk/pairs/cp0_cp1__sheet.png` b090, `cp4_cp6__sheet.png` b180, `cp5_cp2__sheet.png` b180) — with YAV1 (F46) three BF2 maps.
- 2026-09-29 (PXWB part 4, FINDINGS F64): BF2 COR1's terrace planter tree (`Cor1_prop_leafpatch_tall` on `Cor1_bldg_walk_young`, batch 3's F24 case) at the walkabout stand — a tall dark leafy column on the PS2, empty sky in the port: `pcsx2/ref/bf2_cor1/walk/pairs/cp1_cp2__sheet.png` b000 (250,0,310,200) PS2 (22,24,30), gradient 2.87 vs port (32,31,56), 0.02 (`lead_tmp/pxwb/boxm.py`); b090 and b270 too.
- Planner 2026-09-29 19:05: value 4 → 5. Both walkabouts rank it the biggest visible gap: PXWA F38 (BF1 NAB1 reads as a dead forest) and PXWB F46/F53/F61/F64 (BF2 YAV1, DAG1, NAB2 full-leaved on the PS2, bare twigs in the port; COR1's planter tree). The maintainer set 4 on 09-25 before these pairs existed. Reverse: `loopctl.py set LEAF value=4`.
