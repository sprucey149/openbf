---
id: BOLTM
title: Mounted same-tick bolt lights match the fired weapon
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
A first-tick vehicle-gun bolt can put its terrain light on the reverse-flight
side. BOLTSURF matches the Impact against the rider's current foot weapon
instead of the class in the Shot. Keep this repair separate from the older
remnant-style/class-selection issue and from the parent's full visual checks.

## Evidence
Existing analysis/re-bolt.md section5.2 and BOLTSURF define the two light-offset
branches. Event::Shot and Event::Impact already carry the actual gun class;
core::sim::guns emits the seat gun's class. No new original-game rule is needed.

Current main5343477, generated WalkSim turret scene: public enter_vehicle,
40down-look ticks and real FIRE produce Shot/BoltExpired/Impact(Terrain)
in one tick, no live bolt, and one remnant. Rider class0/gun class1 (otherwise
identical): impact(0.002997943,-0.18805528,0.524314), expected next-update light
(0.002997943,0.31194472,0.524314), actual
(0.0023317335,0.29817924,0.40779978), error0.11732644m. Same-class mounted
control error0. Temporary shot-class correlation makes both errors0 and all
12bolt tests pass; all runtime edits were restored exactly. This is an inline
caller witness, not a real-class or GPU acceptance receipt.

## Scope
Only match the terrain Impact against the Shot's weapon_class, retaining exact
end-position and flight-direction discrimination. Keep every other class use,
ray length, drawn remnant, lifetime, fade, BF1 no-light behavior, tracked bolt,
offset law and simulation event/hash unchanged. The older wrong remnant-style
bug (rider class used for laser eligibility/size/color/speed) stays on the
parent as a separate decision; do not silently fix or endorse that behavior.
No schema, core sim, geometry, constants, shaders, bake or gate changes.

## Acceptance
1. Permanent asset-free real WalkSim::step_tick -> BoltVis::update/omni_lights
   scene: a mounted player fires a different-class gun at terrain, with actual
   Shot and Impact both naming the gun and an empty live-bolt list that tick.
   Prove public seating, one remnant, reaching midpoint and next-update
   hit+0.5*terrain_normal within1e-4m. No injected events or private remnant
   state in the principal witness. Exact parent must fail that assertion,
   with otherwise equal same-class mounted and on-foot controls passing.
2. Keep BOLTSURF's Building/terrain slope, tracked, fade, lifetime and BF1
   no-light controls. Add mounted Building and decoy controls that separately
   disagree in class, end position and direction; rejecting a valid gun-class
   terrain Impact and accepting an unrelated rider-class terrain decoy must
   each fail distinct assertions. Include differing rider/gun data so the
   test cannot pass by matching only an identical clone. Assert no sim/hash/
   event mutation by visual updates. Prove unrelated remnant styling unchanged.
3. Reverting the correlation fix must compile and fail the actual mounted
   principal; removing class/position/direction checks individually must
   compile and fail the relevant decoy assertion. No setup/compile error,
   skipped/zero test count or missing data counts. Restore every mutant.
4. Under shared CI lock: focused bolt_vis tests, full game library, fmt,
   workspace all-target Clippy and available map/protect checks. Keep CRATES
   rows accurate. Full CI and ordinary applicable gates remain required;
   no changed baseline, pin, golden, threshold, approval or exemption.
5. Before earned acceptance, repeat the parent's real HOT1/GEO1/FEL1
   first-tick mounted terrain/world-surface witness with identical inputs
   on exact parent/candidate; require zero wrong branch among supported
   surface correlations. Keep the earlier eight-map census and no-mismatch/
   BF1 controls, view GPU light positions and run the parent's full CI/map/
   protect/GPU/golden/parity/sim/sweep/arena/leak/bake/behavior gates through
   the normal isolated-worktree workflow. Preserve complete BOLTSURF and
   GCL16 Acceptance; fixture success never completes those cards.
