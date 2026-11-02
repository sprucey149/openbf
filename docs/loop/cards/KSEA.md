---
id: KSEA
title: Kamino's sea edge: the port draws its own flat plane past the ocean's far pass; the PS2 draws whole 160 m tiles reaching 0.8*FarSceneRange (640 m), black behind, and fogs the far part with the world fog colour in BF2 too; pose_render must step the ocean
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/water_anim.rs,engine/crates/game/src/viewer/look.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/game/examples/pose_render.rs
declares: goldens=any; parity=bf1_kam1,bf2_kam1
needs:
effort:
branch: ksea
resume: landed 6643ba04 2026-09-30
created: 2026-09-30
alpha: yes
---

## Why
The maintainer's first Kamino complaint: "the sea nearby is wavy but then there's a hard cutoff and it looks flat and a different
colour". The flat part is the port's own still plane (`water_anim.rs` header says so); the PS2 draws no water there.
`analysis/re-kamino.md` §2 ([O]).

## Evidence
- PS2 frames, lightning frozen (`lead_tmp/kamr/ps2exp/out/s2_*.png`, pose in `s2_ctl0.json`): the sea darkens into
  black at its far edge and the frame is black from there to the horizon — also at a flash's peak. `FarSceneRange`
  800 → 400 pulls the edge from ~680 m to ~360 m (`sea_far400.png`): whole 160 m tiles reaching 0.8·FarSceneRange.
- The far part takes the world fog colour: pokes of green / red / white add 47 / 43 / 39–46 levels at the edge, 0 by
  ~430 m (table in §2.1). BF2 `FUN_00575670` 708400–708451 reads it; the port's `water_look` gives a BF2 ocean no fog.
- `views/sea_compare.png`: PS2 | port | port with `OPENBF_FAR_CULL=1` — the flag already gives the PS2's structure.
- `examples/pose_render.rs` never steps the ocean: every parity render of KAM1 shows the flat plane.
