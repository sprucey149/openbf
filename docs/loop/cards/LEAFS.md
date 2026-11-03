---
id: LEAFS
title: Leaf canopies as the PS2 lays them out: an unseeded leafpatch class's seed and each leaf object's tint are fixed draws of the global LCG (reseeded to 0 at level load), not per-session random — bake BF1's per map and BF2's per era (LEAF audit)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT of LEAF (2026-09-30, PARTLY): the leaf patches' placement, template, draw and colour hold. One premise is wrong:
`analysis/re-leafpatch.md` §0/§4.2 and `game::viewer::leaf`'s module doc say an unseeded `leafpatch` class (every YAV,
DAG1 and KAS2 class) "takes a random seed per session, so any seed is as faithful". The port therefore seeds them with the
FNV-1a of the class name. It also draws each object's tint from an LCG seeded by the map name. The PS2 does neither at
random. It reseeds its global LCG to 0 at level load (B1 `FUN_00173a98` 16151, B2 `FUN_00174b28` 16762/16876), so each
class seed and each object tint is a fixed draw of that sequence. The PS2's canopies can be reproduced exactly; the port's
are a different random arrangement of the same sprites. The gap is most visible up close: BF2 DAG1's leaf-pixel mask
overlaps the PS2's with IoU 0.38, and 0.58 with the PS2's seed.

## Evidence
Report `engine/artifacts/loop/reports/20260930_0330_audit_LEAF.md`; scratch `engine/artifacts/lead_tmp/audit-leaf/`
(`census.py` RAM reader, `lcgfind.py`, `tintfind.py`, `seeds_*.json`, `port/`, `verify/VERDICTS.md`). One Opus verifier:
13 SUPPORTED, 3 PARTLY (wording folded in below), 0 NOT.
1. **Class seeds.** A class without `Seed` keeps the seed its ctor draws: B1 `FUN_001b21e0` 57316–57319, B2 `FUN_001ca9d8`
   +0xb0. In all 96 PCSX2 states of the 9 leaf maps, each such seed equals the LCG sequence from 0 after n steps. The
   seed formula is (a >> 16) | (b & 0xffff0000), with a = LCG(state), b = LCG(a), LCG = x·0x19660d + 0x3c6ef35f.
   - **BF1 is fixed per map.** The same n holds in every state, across two capture sessions (2026-09-25, 2026-09-29) and
     both eras. YAV1: leafpatch 1648, leafpatch2 1650, bush2 13112, bush 13114. YAV2: bush2 1626, bush 1628, leafpatch
     1630, leafpatch2 1632.
   - **BF2 differs between the CW load and the GCW/AI-0 load:**

     | map | class | CW n | GCW n |
     |---|---|---|---|
     | DAG1 | dag1_prop_leafpatch | 3284 | 3154 |
     | KAS2 | kas_prop_leafpatch | 4002 | 3720 |
     | YAV1 | leafpatch, leafpatch2 | 6930, 6932 | 6862, 6864 |
     | YAV1 | bush2, bush | 15690, 15692 | 15622, 15624 |

   - COR1's two GCW loads (33 soldiers vs 0) have identical tints, so the era, not the AI count, is the likely driver
     [D: one map].
2. **Object tints.** B1 `FUN_001b1448` 56917–56931 draws R, G, B = 0.7 + 0.2u + 0.1u′ with 8 steps of the same
   sequence. All 530 BF1 NAB1 tints fall at steps 1654..8360 (the verifier's matcher). Every BF1 map's tints are
   identical in all its states; BF2 has one tint set per era.
3. **Visible effect.** With the RAM seeds the port's leaf pixels overlap the PS2's better on BF2 DAG1 (IoU 0.376 →
   0.577, at every threshold) and a little on KAS2 (0.445 → 0.481). BF2 YAV1 moves only marginally (0.700 → 0.711;
   reversed at threshold 8). BF1 YAV1 gets slightly worse (0.325 → 0.314), where the port's tints, sway phase and FOLG
   bushes also differ from the PS2 frame.

## Acceptance
- **Seeds.** Every unseeded class takes the PS2's seed: BF1 per map, BF2 per (map, era) for the eras measured. A
  data-gated test reproduces the RAM seed of every unseeded class in the 96 states (`lead_tmp/audit-leaf/out/*.json`).
  Pick the source and write it down: a table of step counts n (RAM-measured, cite this card), or a replay of the load's
  LCG consumers if one is found. An era without a measured n keeps today's fallback, flagged PROVISIONAL.
- **Tints.** Either reproduce the PS2's per-object tints the same way (BF1 exact; BF2 per era), or keep the PROVISIONAL
  LCG and record why (the tint only scales each object's brightness, 0.7–1.0).
- **Pictures.** At the audit's DAG1 and KAS2 poses (`lead_tmp/audit-leaf/port_frames/*_pose.json`, PS2 frames in
  `frames/`), the leaf-mask IoU reaches the RAM-seed values (DAG1 ≥ 0.55). Goldens move only on the leaf maps whose
  seeds or tints change.
- **Docs.** Fix `analysis/re-leafpatch.md` §0/§4.2 and the `leaf.rs` module doc.
## Planner note (2026-09-30 05:00)
- Folded in from the audit's "no card" call: the port fogs leaves (`grass.wgsl` `fs_grass`) and the PS2's leaf draw binds
  no fog pass (audit banner in `analysis/re-leafpatch.md`). Only BF1/BF2 YAV1 are touched (≤ ~8 % on leaves already
  fading). Take it in this build if the pass split is cheap; otherwise leave a note here. Kept at value 4, untagged:
  only BF1 YAV2 among the showcase maps has unseeded classes, and its gain is unmeasured.
