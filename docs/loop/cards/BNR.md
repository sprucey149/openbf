---
id: BNR
title: Where a door's bone-bound geometry sits at REST: model space, or its bone's rest frame?
milestone: M3
value: 3
phase: re
status: done
deps:
files:
declares: marks: GOLDENS REBAKE SWEEP
needs:
effort:
branch:
resume:
created: 2026-09-25 (from QUEUE.md)
defer: post-alpha
acceptance: needed
---

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
BNR. [TODO — **NOTE 2026-09-25 (DWS's measurement): this is now DEA1's no-outcome knot — 13 units wall-slide against blast doors 0 and 2, which their own presence holds at open = 1.00 all round; lifting those two doors' collision (an upper bound on the fix: a correct door is shut until someone reaches its trigger) drops the no-progress census 13 → 1 and moves bf2_dea1's row to `tickets 1/3 alive 1/3 … hash 5213f4f287f29ff2` (`lead_tmp/dws/nodoor02.*`); PREDICT that the build moves bf2_dea1's sweep row and nothing on the maps without a blast door.** ADDED 2026-09-25 (lane 1) by DRV's landing; **[RE] first**, FREE (disc + `analysis/` only); the fix after it is [REBAKE] maps and probably [SWEEP] (it moves door collision) and maybe [GOLDENS]] **Where a door's bone-bound geometry sits at REST: model space, or its bone's rest frame?** The port reads both a door model's `BNAM`-bound segments and its `PRNT`-bound collision prims as MODEL space at rest and moves them by `D(f) = B(f)·B(0)⁻¹` (`bake::door`, PROVISIONAL [D]). Every door whose bones rest at the origin (KAM1, POL1, TAN1, MUS1's garage) reads the same either way; two disagree. **DEA1 `dea1_prop_door_blast`** (5 instances): `bone_leftdoors` rests at x = −5 and its `open` clip takes it to −10, `bone_rightdoors` +5 → +10 (151 frames), yet the segments on `bone_rightdoors` span model x −7.4..−0.7 and those on `bone_leftdoors` 0.7..7.4, and the prims agree (`p_leftdoors1..3` at x +2.4..+4.1 under `bone_leftdoors`). Read as model space, the shut door has a 1.4 m slit and opening slides each leaf 5 m ACROSS the doorway through the other (the port draws AND collides it that way since DR2/DRV — `lead_tmp/door-visual/cap/sheet_blast_kam.png` top row); read in the bone's rest frame, the leaves meet at the centre on a diagonal seam (the prims are boxes rotated 44°) and slide apart. **MUS1 `mus1_prop_door_tri`**: `bone_door` rests ROTATED 20.8° about x; the model-space panel fits its frame on the untilted instance `door_cont_3` (`cap/sheet_cont3.png`), which a bone-frame reading would tilt 20.8° off — so if the rule is the bone frame, it is translation-only, or the tri door's rest rotation is special. RE (§5 discipline, at max): BF2's hard-skin draw of a non-`bone_space` model (`mungefs::model` header, `re-softskin.md`) — which matrix a `BNAM` segment's vertices get at rest — and the collision side, how a `prim`'s `XFRM` composes with its `PRNT` node (`mungefs::coll`'s PROVISIONAL); a PCSX2 capture of DEA1's blast doors shut would settle the look. Build after: one rule for both in `bake::door` + `Scene::collision_and_doors` + the static draw. GATE: the RE lifted `[O]`; after the build, the DEA1 blast door shut with no slit and opening outward in a `drv_door` capture (`lead_tmp/door-visual/tools/drv_door.rs`), the MUS1 tri doors unchanged, the sweep's move DECLARED. Files: `analysis/`, then `engine/crates/bake/src/door.rs`, `engine/crates/game/src/assets.rs`, `engine/crates/game/src/viewer/scene.rs`, `engine/crates/mungefs/src/coll.rs`.

## Note (TBIND, 2026-09-30)
- TBIND's `bake::bind_pose` draws every non-door model's `BNAM` segment in its bone's bind frame. The evidence is the
  PS2 frame of HOT1's mortar (`lead_tmp/audit-vtur2/pose/bf2_hot1__cp1_cp6_postA_spawn__pair.png`, head on the pillar).
  It leaves `door` classes' models as stored, for this card.
- The per-segment BBOX census (`lead_tmp/tbind/bnscan2.py`): every non-door case fits the bone frame (UTA1's
  barrel1 under its own `hp_fire3`, the SPA ship-turret barrels, treesmash, BES2's forklift claws). DEA1's
  `dea1_prop_trashcompactor` (not a door class) is coherent only in the bone frame: walls at z ±5.8..9.8 with the
  debris between them; as stored, the walls sit back to back through the debris. Its `prim`s share the segments'
  frame (e.g. `p_cube4` under `bone_wall1a` at z 1.28), so the port's collision (prims as model space) no longer
  matches its drawn compactor.
- INFO word 1 (the +0xad flag) is 1 on the blast door and the compactor, but on the whole of DEA1 too (it is a
  per-map material switch, `flag2.py`), so it does not separate the two readings.

## Closed by STDOR (2026-10-01)
Answer: the bone's frame, rotation included, for the drawn segments and the prims (`analysis/re-bf2-doors.md` §10;
PS2 pairs `lead_tmp/stdor/pics/`). The earlier reading "MUS1's tri door only fits model space" came from a port
capture, not a PS2 frame: the PS2 draws MUS1's garage and square doors inside their slanted frames. Built in
`bake::bind_pose` on branch `stdor`. Non-door prims: card PRIMB.
