---
id: VWLK
title: The walker, transcribed: BF2 FUN_0025d5e0 root-motion locomotion (animation clock scaled to MaxSpeed/BoostSpeed), the legs, the jump — then a build card (AT-ST first)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
15 walker-family classes (AT-ST ×3 variants, spider walker, tauntaun, one-man AT-ST, AT-AT / AT-TE as mobile command
posts) move by the walk animation's root motion — a different integrator from the hover.

## Evidence
`analysis/re-vehicles.md` §4.1: BF1 FUN_00218fa0 (the animation at the real dt; MaxSpeed read only by the AI), BF2
FUN_0025d5e0 (the animation clock scaled toward MaxSpeed / BoostSpeed in two branches, turn rate Stopped / Forward /
Boost), legs FUN_0021ae90 (foot placement, body pitch ±0.55 rad, 3 g when unsupported), commandwalker = walker + a
command-post component. PLAN locks BF2 handling for ground vehicles.

## Do / Acceptance
Transcribe BF2 FUN_0025d5e0 (both clock branches and their conditions), the legs, the jump (JumpHeight → launch speed)
and the walker contact into `analysis/re-walker.md` with the §RE checks and one verifier; then write the build card
(AT-ST first; needs VBAKE's skinned models).

**Note (VBAKE, 2026-09-26):** VBAKE landed the walkers RIGID in their skel bind pose (`side_<game>/vehicles/<side>/models/`),
not skinned. VWLK's build owns the skinned walker GLB: each walker model has a same-named `skel`; `rep_walk_atte` carries
MTRL 0x07 / 0x10003 (the 0x02 alpha test) that `bake::soldier::segment_blends` refuses — decide that pass there
(`bake/src/vehicle.rs` module doc).

**Planner note (2026-09-26):** raised 3 → 4 — M2 is the active milestone and, after HVRE, this integrator is next (the AT-ST stands on TAT1 and most GCW pads, VPAD spawns it).
