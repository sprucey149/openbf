---
id: SKIPC
title: Instances the port skips as "no drawable model" that the PS2 draws: the command-post console (`com_bldg_controlzone`, every map), BF1 Theed's planter shrubs and
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/assets.rs,engine/crates/game/src/main/battle.rs,engine/crates/game/examples/pose_render.rs,engine/crates/game/src/gallery.rs
declares: goldens=maps/bf1_bes1.png,maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf2_cor1.png,maps/bf2_dag1.png,maps/bf2_dea1.png,maps/bf2_end1.png,maps/bf2_fel1.png,maps/bf2_geo1.png,maps/bf2_hot1.png,maps/bf2_kam1.png,maps/bf2_kas2.png,maps/bf2_mus1.png,maps/bf2_myg1.png,maps/bf2_nab2.png,maps/bf2_pol1.png,maps/bf2_tan1.png,maps/bf2_uta1.png,maps/bf2_yav1.png,maps/tat1.png,maps/tat2.png,walkabout_stats.ron; parity
needs:
effort:
branch: skipc
resume: landed 9b71b68d 2026-10-15
created: 2026-09-25 (from QUEUE.md)
alpha: yes
---

## Why / Evidence / Acceptance
(Rewritten by the planner 2026-09-30; the original entry below keeps the detail.)
- **Why:** the port skips, as "no drawable model", classes the PS2 draws at nearly every post of both games: the
  command-post console (`com_bldg_controlzone`) and its holo, and the recharge / GNK ammo droids
  (`com_item_healthrecharge`, `com_item_weaponrecharge`), plus BF1 Theed's vines (`nab2_prop_vine*`). The grass classes
  went to FOLGG.
- **Evidence:** FINDINGS F12, F50, F55, F56, F60, F61, F64; the PXWA/PXWB sheets named in the notes below;
  `game/src/viewer/scene.rs` `upload_scene` skips an instance whose `resolve_model` is None.
- **Acceptance (measurable):**
  - RE: a table of class → where its model lives → drawn on the PS2 (frame) → why the port skips it
    (`analysis/re-skipped-classes.md`).
  - Build: `PXC_SKIPPED=1` on the six showcase maps lists none of the console / recharge-droid / vine classes.
  - Pixel test: the named walkabout pairs, re-rendered with `port.py <map>/walk`, show the console and the droids in the
    boxes the notes give. At least BF1 KAS2 cp1_cp5, BF1 YAV1 cp6_reflectingpool and BF2 COR1 cp4_cp4.
  - The goldens that move are declared, and PARITY does not regress on the affected maps once it exists.

## Original QUEUE.md entry (verbatim, 2026-09-25)
SKIPC. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 2 (FINDINGS F12); **[RE] FIRST**; FREE] **Instances the port skips as "no drawable model" that the PS2 draws: the command-post console (`com_bldg_controlzone`, every map), BF1 Theed's planter shrubs and vines (`nab_prop_flowers` ×53, `nab2_prop_vine1/2` ×40), BF1 Yavin's grass (`yav_prop_grass_tall` ×10, `yav_prop_grass` ×4)** [RE] — evidence `engine/artifacts/pcsx2/ref/bf1_yav1/pairs/cp5_bazaar_fountain_spawn.png` (the console and its green holo on the PS2, the bare octagonal base in the port), `…/bf1_nab2/pairs/cp4_cp4_rotunda_{v3,v4_up,v5_up}.png` (hedges and trees on the PS2, empty planters and bare facades in the port). The port: `game/src/viewer/scene.rs` 593–595 (`upload_scene` skips an instance whose `resolve_model` is None); the lists via `PXC_SKIPPED=1` are in FINDINGS F12 (NAB2 226 instances, YAV1 102, KAM1 70). The 2026-09-01 census's "all no drawable model — emitters, spawns, sound markers, turrets" (`analysis/PLAN_LOG_2026.md` 2435–2438) does not hold for these. RE (§5): per skipped class, why `resolve_model` finds nothing (the ODF's geometry key, a model in a LVL the map bake does not pull — common / side / ingame — or a class label the manifest drops), and which of the rest (mist, god rays, turrets, recharge droids, `com_inv_col_*`) the PS2 draws; the console's holo is an effect (`HoloOdf com_holo_controlzone`, `analysis/re-cp-capture.md` 189). GATE: a table class → where its model lives → drawn on the PS2 (frame) → why the port skips it, in a dated `analysis/` §, + a build entry ([REBAKE], [GOLDENS]). Files: `analysis/re-skipped-classes.md` (new), `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/skipc/`.

## Note from FOLG's RE (2026-09-26, lane 2)
- `nab_prop_flowers` (BF1/BF2 NAB2 x53), `yav_prop_grass_tall` (BF1 YAV1 x10) and `yav_prop_grass` (x4) are
  `grasspatch` classes (BASE grasspatch; `nab_prop_flowers` uses the rectangle property 0x35a4393b = 2.8 x 2.8 — a
  planter). A world-object grasspatch draws with its class MaxDistance and a white tint (B1 `FUN_001af3e0` 55299,
  `analysis/re-foliage.md` §7). **Card FOLGG draws them**; this card keeps the other skipped classes.

## Note (PXWA, 2026-09-29, lane 2): the recharge droids are drawn on the PS2 at nearly every BF1 post
The PCSX2 walkabout (1-unit battles, so nothing hides them) shows `com_item_healthrecharge` / `com_item_weaponrecharge`
(GNK-style boxes and the tall cylinder droid) beside most posts on every BF1 map it covered, all absent in the port —
e.g. `engine/artifacts/pcsx2/ref/bf1_kas2/walk/pairs/cp1_cp5__sheet.png` (both, b000/b180), `bf1_yav1/walk/pairs/
cp6_reflectingpool__sheet.png` b000/b180, `bf1_bes2/walk/pairs/cp3_cp7__sheet.png` b000, `bf1_end1/walk/pairs/cp2_cp5__sheet.png`
b180, `bf1_kas1/walk/pairs/cp1_cp4__sheet.png` b180. Theed's planter vines/ivy (`nab2_prop_vine*`) likewise on every
NAB2 post (`bf1_nab2/walk/pairs/cp5_cp5__sheet.png` b090). Of this card's classes the droids are the most seen.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F50): BF2 too — the GNK ammo droid ("stand near this droid to refresh ammo") and the recharge droids beside the posts on the PS2 only: `pcsx2/ref/tat2/walk/pairs/cp3_cp1__sheet.png` b270, `cp4_cp3__sheet.png` b000 (the cantina: recharge + power droid), `bf2_geo1/walk/pairs/cp6_cp3__sheet.png` b180, `bf2_yav1/walk/pairs/cp2_viaduct__sheet.png` b180.
- 2026-09-29 (PXWB part 2, FINDINGS F55/F56): more BF2 recharge / GNK droids at DAG1 cp3, END1 cp5 and cp6, FEL1 cp2-1, HOT1 cp6; DEA1 cp5's command-post console holo (a bright blue-white beam, `pcsx2/ref/bf2_dea1/walk/pairs/cp2_cp5__sheet.png` b090).
- 2026-09-29 (PXWB, FINDINGS F60/F61): KAS2 cp3con (b270), MUS1 cp4 (b180), NAB2 cp5 (`pcsx2/ref/bf2_nab2/walk/pairs/cp2_cp5__sheet.png` b000, the ammo droid) — the droids stand beside nearly every BF2 post too.
- 2026-09-29 (PXWB part 4, FINDINGS F64): BF2 POL1 cp2con (`pcsx2/ref/bf2_pol1/walk/pairs/cp0_cp2con__sheet.png` b000 the recharge droid, b180 a power droid) and BF2 COR1 cp4 (`bf2_cor1/walk/pairs/cp4_cp4__sheet.png` b000, the health droid — "THIS DROID WILL HEAL YOU WHEN YOU STAND NEAR IT") and cp6 (`cp5_cp6__sheet.png` b000 the recharge droid, b180 a GNK power droid): the droids stand beside every BF2 map's posts on the PS2.

## RE result (2026-10-15, lane 2) - `analysis/re-skipped-classes.md` [O] (204c929); phase -> build, card STLC folded in
- **A battle on main efa2d56 draws the console, the three recharge droids and the gun turrets** (the COMMON bake is
  merged: `main/battle.rs` 29-37); the vines and grass are LEAF's / FOLGG's passes. The gap the card's pictures show is the
  **STILL path**: `examples/pose_render.rs` without `POSE_COMMON=1` (parity frames, `pcsx2/port.py`, every PXWA / PXWB / PXC
  port column) and `game/src/gallery.rs` (432 the gallery views, 629 the walkabout) load the map with no `merge_common`.
  Census (`lead_tmp/re/skipc/census_main_efa2d56.txt`): still path skips consoles 191, droids 584, turrets / built-turret
  pads 166 instances; battle path 0 of them. Control picture `lead_tmp/re/skipc/kas2_cp5_battle_pair.png` (the battle path
  draws both droids at PXWA's KAS2 cp1_cp5 stand).
- PS2-only draws split off: godrays -> **GODR** (re), BF1 Geonosis cp2's forward center -> **FWDC** (build); dust / mist ->
  DUSTE. Not gaps: VBIND's two names (the gian speeder is campaign-only, the autoturret is a weapon class), Hoth's buried
  hologram, `com_inv_col_*`.
- **Build (this card):** one shared "load a map as a battle does" (map + mode layer + `merge_common`) used by
  `main/battle.rs`, `pose_render` (POSE_COMMON becomes the default / goes away) and `gallery.rs` (both loads). Predict the
  golden moves first from the data (views with a post, a droid or a gun turret in sight), then run the branch gates.
- **Acceptance (run and paste):** `PXC_SKIPPED=1 pose_render` on the six showcase maps lists none of the console / droid /
  turret classes (the census example's still column = its battle column); `port.py bf1_kas2/walk`, `bf1_yav1/walk`,
  `bf2_cor1/walk` re-rendered into a scratch copy show the droids and the console in the PS2's boxes at cp1_cp5,
  cp6_reflectingpool and cp4_cp4; `scripts/parity.py` per-map table before / after (expected down on post frames, no map up
  by more than 0.5); the goldens that move declared.
