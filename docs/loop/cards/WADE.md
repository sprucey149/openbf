---
id: WADE
title: A wading soldier shows through the water: draw the skinned soldiers BEFORE the near water, as both PS2s do (opaque model segments bucket 5, the near flat water bucket 8 / 7, blended with Z write over them) - the maintainer's BF1 KAS2 stormtrooper ends at the belt, BF2 FEL1's shins are cut off; and the walkabout's soldier stands on the floor, not 0.59 m over BF2 YAV1's bazaar
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why (rewritten 2026-10-06 after the capture + RE; the original card text is kept below)
The maintainer, playtest 2026-10-04: "Water still isn't transparent ... you can't see its body in the water"
(`playtest/playtest_20261004/2_kas2_stormtrooper_in_water.png`), and his walkabout findings of 2026-09-28: BF2 FEL1's
river cuts a soldier's shins off, BF2 YAV1's bazaar soldier stands ON a water sheet.
- **The water is not too opaque: the soldier is drawn after it.** The port's water writes depth and the skinned
  passes come after the water runs (`render/src/frame.rs`, "They stay after the water runs ... where the disc draws
  it against the model buckets is not in `analysis/`"), so everything under the surface fails the depth test.
- **Both PS2s draw him first** (`analysis/re-water.md` §9.1 [O]): opaque model segments are bucket 5, the near flat
  water bucket 8 (BF1) / 7 (BF2), blended `Cs·As + Cd·(1−As)` with Z test and Z write; buckets are kicked in
  ascending order. His legs show through the water with the same tint as the bed.
- **The stand is right in the sim, wrong in the walkabout**: a free PS2 soldier stands on the bed / floor at every
  depth (22 spots, the port's `Sim::ground_under` within 2.2 cm at 21). BF2 YAV1's bazaar floor is −72.818, 0.32 m
  under the plane; `gallery::ground_stand` puts the walkabout's soldier at max(terrain, the post's y) = −72.225.

## Evidence
- Pairs, PS2 | port (main 2560be8) | port without the water layer, a stormtrooper planted at the PS2 soldier's own
  feet: `engine/artifacts/pcsx2/ref/bf1_kas2/wade/pairs/s094.png` (0.96 m: the maintainer's picture reproduced) and `s148.png`
  (1.50 m), `bf2_fel1/wade/pairs/s061.png` (0.61 m) and `s118.png` (1.18 m), `bf2_yav1/wade/pairs/baz_n.png` (0.32 m);
  the PS2 with its water switched off beside it: `pairs/<spot>__ps2_on_off.png`. FINDINGS F65, F66.
- RE: `analysis/re-water.md` §9 (22 of 24 claims SUPPORTED by the verifier, 2 PARTLY folded in).
- Tools (scratch, `engine/artifacts/lead_tmp/wade/`): `ps2/wade_probe.py` (the PS2 stands), `tools/wade_pose.rs`
  (pose_render + one planted soldier), `tools/wade_check.py` (the referee), `tools/wade_ground.rs` (the port's floor
  at a point), `out/spots.json` (the five PS2 stands and poses the referee replays).

## Original card text (2026-09-28)
The maintainer (INBOX 2026-09-28, walkabout findings (a) and (b)): two soldier-in-water defects on BF2 maps.
- (a) BF2 FEL1 `engine/artifacts/walkabout/bf2_fel1/cp1_cp6_1.png`: the soldier stands on the river bed and the water
  surface cuts his shins off — nothing below the waterline shows through. (Planner viewed it: all four bearings.)
- (b) BF2 YAV1 `engine/artifacts/walkabout/bf2_yav1/cp4_bazaar.png`: the bazaar floor is under a water sheet and the
  soldier's feet sit ON it.

### (original) Evidence / questions
- PCSX2 pairs first: a soldier wading at the FEL1 spot, and the YAV1 bazaar post (water height vs floor vs soldier).
- RE (a): how translucent the PS2's water is over a submerged unit — the water pass is ALPHA 0x44 (`analysis/re-water.md`
  §1), so why does the port's Water layer read opaque here? And where the PS2 stands a soldier in water: on the bed and
  wading, held at some depth, or depth-limited / killed (see DTHR).
- (b): a thin film he stands in, or the port's water too high, or his stand too high? The fix follows the PS2.
- Check whether the gallery's stand (walkabout: the terrain as floor) differs from the sim's stand in water.

### (original) Acceptance
- PCSX2 pairs for both spots viewed; an [O] RE note (re-water.md addendum) answering both questions; build card(s) — the
  render half here (M1), a sim half as its own card if the stand is wrong (M3).

## Notes
- Created 2026-09-28 by the planner. Related: HWAT (the hover float's WMAP wet grid — the same water data), DTHR, WCOL.
- 2026-09-29 (PXWB, lane 2): the PS2 side of the YAV1 bazaar is captured — the PS2 soldier at the bazaar post, four bearings, the port beside it: `pcsx2/ref/bf2_yav1/walk/pairs/cp4_bazaar__sheet.png` (+ the state `pcsx2/ref/states/bf2_yav1_walk_cp4_bazaar_b000.p2s`, the soldier held at his stand in a 0-AI battle). FEL1 cp1_cp6_1 comes with PXWB's FEL1 run.
- 2026-09-29 (PXWB, FINDINGS F54): FEL1 cp1_cp6_1 captured too — the PS2 soldier WADES, his shins visible through a bright turquoise river whose bed shows through; the port's river is darker and flatter: `pcsx2/ref/bf2_fel1/walk/pairs/cp1_cp6_1__sheet.png` (all four bearings; (0,300,640,470) b000 PS2 (54,85,72) vs port (49,65,46)); state `pcsx2/ref/states/bf2_fel1_walk_cp1_cp6_1_b000.p2s`.
