---
id: FOLG
title: The terrain's foliage: the PS2 carpets the ground with ferns and grass (BF2 Dagobah and Felucia; BF1 KAS1, KAS2's high ground, YAV1), the port draws bare terrai
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/foliage.rs,engine/crates/mungefs/src/lib.rs,engine/crates/bake/src/foliage.rs,engine/crates/bake/src/map.rs,engine/crates/manifest/src/lib.rs,engine/crates/manifest/src/tests.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/render/src/draw.rs,engine/crates/render/src/shader.wgsl,engine/CRATES.md
declares: rebake=tat1,tat2,maps; goldens=maps/bf1_end1.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf1_geo1.png,maps/bf1_rhn2.png,maps/bf1_kas2.png,maps/bf1_nab2.png,maps/bf2_end1.png,maps/bf2_fel1.png,maps/bf2_geo1.png,maps/bf2_dag1.png,maps/bf2_nab2.png,maps/bf2_yav1.png
needs:
branch: folg
resume: landed 9b8d040a 2026-09-26
created: 2026-09-25 (from QUEUE.md)
---

## Why / Evidence / Acceptance
- **Why:** the PS2 plants ferns, grass, bushes, rocks and tree-billboard lines on BF1 END1/KAS1/KAS2/NAB1/NAB2/YAV1/
  YAV2/GEO1/RHN2 and BF2 DAG1/END1/FEL1/KAS2/NAB2/YAV1/GEO1; the port draws bare ground. RE done:
  `analysis/re-foliage.md` ([O], 2026-09-26, one Opus verifier: 30 SUPPORTED, 3 PARTLY folded, 0 NOT SUPPORTED).
- **Evidence:** FOLG = 4 layer bits per cell; `prp_` = the layers' meshes (a model, a `grasspatch`, a `leafpatch`)
  and a path block (props every N m along named paths). Placement is deterministic: `lead_tmp/re/folg/place.py`
  reproduces 2,610 of 2,610 live PS2 instances in 9 save states (x, z < 1 mm, yaw < 1e-4). All layer and path
  models are already baked. 0 hits for FOLG / prp_ / grasspatch in `engine/crates`.
- **This build (FOLG):** parse FOLG + `prp_` (mungefs), run re-foliage §4 exactly in the bake (the 256-float table
  read from the extracted ELF at bake time — no ELF bytes in git), attach y / up / yaw / scale / tint from the baked
  terrain (§6; yaw and scale for models only), write the instance list
  into the map manifest; place the path-block props (§9) as extra world instances of their class; draw the
  `File`-model instances with the existing model pipeline plus the §6 fade (d → 1.45·d, horizontal distance) and
  near-fade. Grasspatch and leafpatch instances are baked but drawn by FOLGG / LEAF.
- **Acceptance:** a test pins a slice of `lead_tmp/re/folg/live_instances.json` as literals (x, z < 1 mm, yaw < 1e-4;
  the tint exact when fed the RAM x, z; y within a measured tolerance). The pairs `bf2_fel1/pairs/cp3_cp3_1_postA_spawn`
  and `bf1_end1/pairs/cp1_cp2_postA_spawn` re-rendered show the foliage models, and `bf1_kas2` shows the treebill line.
  Only the declared goldens move; sims unmoved.
- **Decisions (reversible):** no PS2 streaming or pool cap (draw everything inside its distance); full foliage in
  split-screen (the PS2's BF2 `sky_` split-screen block turns it off or down); the per-instance tint may differ from
  the PS2's by seed (same spread) unless the placement is made bit-exact.

## Original QUEUE.md entry (verbatim, 2026-09-25)
FOLG. [TODO — **NOTE 2026-09-25 (PXC batch 3, FINDINGS F29): more bare ground — BF1 END1 and BF2 END1 (a fern carpet on the PS2, flat turf in the port; neither Endor authors a leafpatch, so this is FOLG's half), BF1 NAB1 tall grass (`engine/artifacts/pcsx2/ref/bf1_nab1/pairs/cp1_cp2_postA_spawn.png`). The leafy tree crowns (BF2 YAV1, BF1 YAV2, NAB1, DAG1's canopy) are NOT terrain foliage — they are the `leafpatch` class, queued as LEAF.** ADDED 2026-09-25 (lane 1) by PXC batch 2 (FINDINGS F22, with F14, F17, F23); **[RE] FIRST**; FREE; the build is a new system (bake + render) with its own [REBAKE]/[GOLDENS]] **The terrain's foliage: the PS2 carpets the ground with ferns and grass (BF2 Dagobah and Felucia; BF1 KAS1, KAS2's high ground, YAV1), the port draws bare terrain — it parses no `FOLG`** [RE] — evidence `engine/artifacts/pcsx2/ref/bf2_fel1/pairs/cp3_cp3_1_postA_spawn.png` (box (0,250,640,480): PS2 (45,50,32) spread 32.5 vs port (91,98,49) 18.7), `…/bf2_dag1/pairs/cp4_cp3_postA_{v3,v3_up}.png` (ground spread 22.6 vs 7.9; the pitched-up top half (40,50,60) vs (75,90,106) — a canopy), `…/bf1_kas1/pairs/cp2_cp2_eastharbor_*.png`, `…/bf1_kas2/pairs/cp3_cp3_cityhall_out*.png`, `…/bf1_yav1/pairs/cp2_viaduct_viaduct_spawn.png`. The port: 0 hits for `FOLG` under `engine/crates` (`mungefs/src/terrain.rs` reads 20 other tags); DAG1 skips no foliage class (`PXC_SKIPPED`). The disc: `analysis/re-terrain-heights.md` §8 — BF1's `FOLG` is 512×512 at 4 bits a cell, read by `FUN_00344d98` (decomp 331498, a nibble lookup); BF2's seven FOLG chunks are 2–8 KB (the batch-2 verifier's census), so BF2 may ride another path. RE (§5): the callers of `FUN_00344d98` and what a nibble selects (a foliage type → which model / texture, the map's foliage definitions), density, draw distance, fade, wind; BF2's twin OPENED (check 3); the Dagobah canopy — foliage, or tree-leaf cutouts lost to the MASK cutoff (queue AT)? GATE: `analysis/re-foliage.md` (§5 header, [O], verifier) + a build entry with its file and golden lists. Files: `analysis/re-foliage.md` (new), a pointer in `analysis/re-terrain-heights.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/folg/`.

## RE result (2026-09-26, lane 2) — the planner's [H] answered
- **Confirmed, with a split:** KAS2's grass is planted by the FOLG layers (`prp_` layer 0 = GrassPatch
  `kas_prop_grass_tall`; KAS1 = `kas_prop_grass` + `kas_prop_reeds`), but the **tree billboards come from the `prp_`
  path block** — `kas_prop_treebill` every 28 m along the path `jungle1` (986 m, ~35 props), not from a nibble
  (re-foliage §9). The same block places the treebill lines on BF1 END1/NAB2/YAV1, YAV2's hellbushes, and BF2
  END1/FEL1/NAB2/YAV1 plus DAG1's `hell_bush` hedge. Both kinds are in this build (models + path props); the grass
  sprites are **FOLGG**.
- **Grasspatch vs leafpatch:** separate draws in the ELF (B1 `FUN_002ed080` vs `FUN_002ee970`, the same module);
  LEAF's RE decides how much FOLGG's renderer can share.
- **Collision:** only meshes with the `0x26edc40c` flag (BF2 FEL1 `fel1_bldg_pitcher_bush`, GEO1 `geo_prop_flatrocks`)
  get a collision object on the PS2 (re-foliage §11); everything else is walk-through. Path props are ordinary props
  (their ODF decides).

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F53): BF2 DAG1's ground plants are pale yellow-beige in the port and dark blue-green on the PS2 — lower right (320,300,640,470) `pcsx2/ref/bf2_dag1/walk/pairs/cp5_cp6__sheet.png` b000 PS2 (34,45,45) vs port (60,65,58); `cp1_cp5__sheet.png` b000 (35,46,46) vs (53,61,56). END1's undergrowth is thicker on the PS2 at every post (`bf2_end1/walk/pairs/*__sheet.png`).
- 2026-10-05 (card LSUM landed, bf263f8): the light sum is now made in display units and the ambient floor is gone (report `reports/20261005_1505_card_LSUM.md`). bf2_fel1 parity 10.25 -> 10.56 (+0.31, passes the 0.5 gate): lit foliage overshoots a little under this sum, as FOLGL measured on the caladium leaves; not special-cased.
