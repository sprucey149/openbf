---
id: JUNI
title: One jump for the merged game - BF1 maps jump with BF2's take-off and air steering (the maintainer, 2026-10-25: "jump physics the same across all bf1 and bf2 maps ... match the better performing game"); the vertical take-off already landed with pull request #2
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
The maintainer, 2026-10-25: "we should have jump physics the same across all bf1 and bf2 maps for our version. Remember that
since we are merging both games together, some behavior must match the better performing game in that regard for
better consistency and overall better game experience." He likes BF2's uphill jumping. The port still jumps per
`GroundRules`, so a soldier's jump changes from map to map.

Landed already (public pull request #2, integrated 2026-10-25): the vertical take-off. BF1 maps ADD the jump velocity
to the vertical velocity, as BF2's PS2 does (B2 FUN_00246ed0, add.s at 0x247034); BF1's PS2 stores it
(FUN_00208b78, swc1 at 0x208bb4). FEEL_NOTES 2026-10-25 JUNI.

Before this build, still per game in `core::soldier` (re-stance-roll.md §4.3a/§4.3b, cards JARC and AIRC):
- the planar take-off: BF2 reshapes the planar velocity with JumpForwardFactor / JumpStrafeFactor; BF1 adds the facing
  x 0.25 x jump velocity x the forward stick (`BF1_JUMP_PUSH`);
- the air steering: BF2 drags (exp(-AIR_DRAG |v| dt)) and adds stick x ControlSpeed[jump] x Acceleration; BF1 raises
  each axis toward stick x BF1_AIR_SPEED x MaxSpeed and never lowers it;
- the late-press window: BF2 takes a press within 0.3 s of losing the floor; BF1 only for a stance 5 s old or one
  that touched down from an earlier hop.

## Acceptance
1. Decide per item above which game's rule is the merged game's (default BF2's, the maintainer's call on the uphill jump); write
   each as a FEEL_NOTES named deviation with both games' PS2 citations. Keep the PS2 RE notes as they are.
2. One code path for the chosen rules on every map; the class constants (JumpHeight, JumpForwardFactor, ...) stay
   each class's own.
3. The tests that pin BF1's PS2 arcs (JARC, AIRC, the late-press window) are re-scoped to the merged rule, each with
   a mutation that brings back the old per-game rule and fails; the PS2 numbers stay in the notes.
4. Paired 32 maps x 8 seeds sweep and fidelity, arena and sim gates; the moves declared; bots still finish every
   round. A by-hand feel check on a BF1 map (the maintainer).

## Build choices and boundaries (planner, 2026-10-26)
The maintainer's existing one-jump request selects BF2's remaining rules on every map:

| Item | Chosen rule | Existing PS2 evidence |
|---|---|---|
| Planar take-off | Facing-forward component times the class's JumpForwardSpeedFactor when positive; backward and side components times JumpStrafeSpeedFactor | re-stance-roll.md 4.3b; JARC / JGATE |
| Jump / Fall air steering | Existing BF2 exponential drag on all velocity components, then the existing air ControlSpeed times class Acceleration; retain first-air gravity order | re-stance-roll.md 4.3a; AIRC |
| Upright jump eligibility | Existing BF2 0.3 s floor-loss window regardless of stand age, without BF1's extra 0.5 s stand delay | re-stance-roll.md 4.3c; JGATE |

The eligibility choice includes a new press after rising from a crouch and a
post-landing press after the existing shared soft lock. Keep the crouch's own
0.5 s rise gate, rising instead of jumping on that press, prone/roll/knock/dead
exclusions, held-button edges, energy refusal/overdraw and shared lock behavior.
Write these as named BF1 deviations in FEEL_NOTES when implemented; the PS2
research stays intact. This is the approved merged-game behavior, not a new
claim that the two original games match.

The airborne helper is also used by Thrown. Retain original BF1/BF2 thrown
steering, gravity, landing/tumble and fire locks; a global GroundRules override
is outside scope. Preserve ordinary ground/slope/contact/stand-fall rules and
their timers. Class constants keep each class's own values. Equal jump laws
under matched constants and input states do not imply identical travel from
different class speeds or different slope run-ups. Preserve the already merged
vertical addition and first-air/gravity order. No schema, RNG, bot decision,
weapon-fire policy, camera or baseline-pin edits are authorized by this slice.

### Source referee required before implementation checkpoint
- Reproduce the planner's temporary matched-class take-off/air/late-press/
  crouch-rise and actual Sim::step probes on the exact parent. Keep numeric
  BF2 controls; no skipped-data success. Replace them with permanent tests.
- Test both rules, forward/back/side/diagonal/zero input, released and opposing
  air stick, varying yaw, inherited vertical speed and at least two different
  class parameter sets including nondefault forward/strafe factors. Check each
  first-air update and a complete flight with independent expected rows.
- Exercise the real Sim::step caller and a bot's existing jump caller; preserve
  the ramp phase suite, low-wall jump and over-height ledge refusal. Cover
  young/old Stand/Sprint, floor-loss offsets 0..12, actual crouch-rise/repress,
  post-landing lock boundaries, held presses and depleted/short energy.
- Re-scope old BF1 arc/air/JGATE expectations to the documented merged law;
  retain the PS2 numbers in research. Keep BF2 numeric witnesses and explicit
  original-rule controls for Thrown, ordinary ground/slope/contact and locks.
- Independently compiled old-planar, old-air, young-stance late-window,
  BF1-stand-delay, global-rules/Thrown contamination and vertical-set/first-air
  regressions each fail a named assertion; rebuild the restored control.
- Focused soldier, sim::step, bot-jump and knock tests; full core/game libs,
  fmt, workspace all-target Clippy, map/protect, and all applicable gates.
  Update module headers/CRATES rows and respect file-size limits.

Acceptance4 remains mandatory: separate exact parent/candidate binaries,
paired32maps x8seeds with finish/boarding/stuck/behavior rows, arena/sim/leak/
GPU/golden/parity/full CI, unchanged refusal classification and approved
baseline process, plus the maintainer's BF1-map feel check. A declared sim/sweep/arena/
leak move never approves a behavior threshold or permits a pin/golden edit.
