---
id: WPSH
title: The wall pass pushes soldiers THROUGH walls (LEAK census class push): M2 — a tick's XZ step longer than the capsule radius crosses a zero-thickness coll mesh sheet and the push completes the crossing (BF2 sprint-jump 17.55 m/s = 0.585 m a tick passes a 4 m sheet in 46/51 timings); M3 — a mesh body's deepest contact points INTO the building (the census's own KAS2 control, the cp5 hut: 0.84 m push toward its axis at tick 20); a swept or sub-stepped horizontal move and no push toward the far side of an uncrossed face
milestone: M3
value: 5
phase: build
status: done
deps: JWAL
files: engine/crates/core/src/collision/walls.rs,engine/crates/core/src/sim/step.rs,engine/crates/game/examples/leak_census.rs
declares: leak; sweep; arena; sim
needs:
branch: wpsh
resume: landed 2c026546 2026-09-29
created: 2026-09-29
---

## Why
JWAL's RE (`analysis/re-soldier-walls.md` §4, §4.1) found two ways the wall pass itself PUSHES a soldier through a
wall. The LEAK census counts both as its `push` class. Neither is the maintainer's KAS2 case (that is JWAL's M1), and JWAL's
fix does not touch them: the push in both is wall-like and is applied.
- **M2, tunnelling.** A `coll` mesh wall is a zero-thickness sheet, and `push_out` takes the capsule's deepest
  contact. Once the capsule's axis is past the sheet, the normal points to the far side and the push completes the
  crossing. From a pose touching the wall, that happens when a tick's step exceeds the 0.5 m radius (above 15 m/s):
  - BF2 MaxSpeed 9: a sprint-jump moves 17.55 m/s, 0.585 m a tick;
  - a roll moves 15.3 m/s, 0.51 m a tick [D];
  - the BF1 stormtrooper's fastest is 12.7 m/s, which does not cross.

  The port also keeps the velocity while pressed against a wall; JWAL's build removes the into-wall part, as the
  PS2 does.
- **M3, an inward push.** The census's own KAS2 control is the `kas2_bldg_platform_roof` hut on the cp5 deck at
  (4.886, 16.518, −7.062). A jump at its flared, double-skinned wall gets a 0.838 m push at tick 20, pointing 1° off
  the hut's axis: into the hut.

## Evidence
- M2: `lead_tmp/jwal/band_before_bf2speed.txt` (`JWAL_MAXSPEED=9 jwal_probe band`, the probe
  `lead_tmp/jwal/tools/jwal_probe.rs`). Sprint-jumps pass mesh sheets 2.75–4.0 m tall in 46 of 51 timings.
- M3: `lead_tmp/jwal/leak_hut_trace.txt` (`LEAK_NEAR=4.9,-7.06,14 LEAK_TRACE=1.28,4.66,jump leak_census bf1_kas2`).
  Tick 20: push (0.254, −0.799) from (2.657, 17.385, 0.343).
- [H] The PS2 tests the soldier SWEPT from its previous position C+0x198 to C+0x228. A RAM teleport that leaves
  C+0x198 behind free-falls through the world (`lead_tmp/jwal/ps2exp/out/tele_diag.txt`).

## Notes
- Do this after JWAL: it changes the same `push_out`, and JWAL's velocity slide shrinks M2's pressed-jump case.
