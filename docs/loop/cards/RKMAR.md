---
id: RKMAR
title: Rocket safety referees enforce the existing one-metre blast clearance
milestone: M3
value: 4
phase: cleanup
status: done
deps:
files: engine/crates/core/src/ai/gunner_foot/tests.rs, engine/crates/core/src/sim/step/tests.rs, engine/CRATES.md
declares:
needs:
effort:
branch:
resume: Source0fc7493 on main; four both-rule referees, core873/game367/fmt/Clippy pass;14compiled mutants caught and production restored exactly. Full CI/cleanup gates pending; original Acceptance unchanged. reports/20261025_1029_card_RKMAR.md
created: 2026-10-25
alpha: yes
tries: 1
---
## Why
RKSLA requires the existing one-metre clearance around a rocket blast to be
refereed at the aim gate and at the shot release. The guards currently work,
but deleting that clearance still passes every existing core-library test.
A bounded regression slice can protect this safety policy independently of
the remaining impact-normal repair and long-charge measurements.

## Evidence
RKSLA Acceptance explicitly names a friendly soldier 4.5m from a blast of
outer radius4m for both release and friend_in_blast. This is openbf's existing
safety policy (RKSLF/RKEX; gunner_foot::BLAST_CLEAR), not a new PS2 rule.
Retain the parent policy and every original RKSLA requirement.

## Acceptance (run and paste)
- All new cases execute on current source with exact existing sim/trace pins.
  At radius4m, a live friendly on foot at4.5m holds in each policy;5.5m and
  enemy controls do not. Compute fixture capsule positions independently of
  the private margin constant. State whether each distance is from the
  capsule centre, blast point or muzzle; verify the predicted wall hit.
- Independent mutations of each policy's extra margin, in the query and in
  the final distance comparison, compile and fail their own assertion. Catch
  the shared BLAST_CLEAR=0 mutant too. Restore source exactly after each run.
  A universal hold or widened-radius mutant fails an outside control; mutations that admit an enemy, dead or mounted friendly fail the appropriate
  control. Prove admission through the effective query/caller; a redundant local
  predicate omission need not fail if another existing filter enforces it.
- At least one real-step release case per rule holds only the margin friend,
  with no Shot/ammo spent and no health lost; otherwise equal outside/enemy
  cases release their salvo with exact Shot ownership/ammo/hold observations.
- Focused tests, full core/game libraries, fmt and workspace all-target Clippy
  pass. Production code is byte-identical to the parent. No gate move declared;
  full CI and all unchanged applicable cleanup gates must pass for acceptance.
