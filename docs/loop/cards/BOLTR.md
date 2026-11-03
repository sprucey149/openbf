---
id: BOLTR
title: Same-tick bolt remnants use the fired gun class
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
A vehicle gun that strikes within its first tick can draw and light its bolt
as the rider's foot weapon. It can also lose the remnant when that foot weapon
has no laser or is too slow, or draw a remnant for a gun with no laser.

## Evidence
Existing analysis/re-bolt.md sections1/3/4/5.2 assigns appearance, class velocity,
fade and BF2 light to the fired ordnance class. core::sim::guns emits that gun's
weapon_class in Event::Shot and stores it in Bolt; the tracked visual path
already reads Bolt.weapon_class. same_tick_hits instead reads u.weapon_class
for eligibility, first-step reach, age and Remnant.class. BOLTM repaired only
the separate Impact correlation. No new PS2 rule or constant is proposed.

Current66d6f9c, generated WalkSim public enter_vehicle ->40down-look ticks ->
actual FIRE: one Shot/struck BoltExpired/Terrain Impact, no live bolt. Distinct
rider0/gun1 data reproduces four assertion failures:
- Light radius4 instead of gun9, remnant class0 instead of1, flight age0.028125s
  instead of0.0225s (gun velocity1.25times rider velocity).
- Rider with laser=None before FIRE:0remnants instead of1 for a laser gun.
- Rider velocity0.1before FIRE:0remnants instead of1 for the unchanged gun.
- Gun with laser=None before FIRE:1remnant instead of0; rider still has laser.
Same-class actual mounted control passes. Temporary use of Shot.weapon_class
for this class lookup makes all five probes pass and22bolt tests pass when
the intentionally old-style pin is explicitly excluded; source restored exactly.
These are caller witnesses, not shipped-class prevalence or GPU acceptance.

## Scope
Use the Shot's weapon class for same-tick remnant eligibility, velocity/step,
age and stored class. Appearance/texture/width/length/color/light/fade then
follow that class through the existing readers. Preserve current unit lookup,
mounted/on-foot start policy, position/direction/class surface discrimination,
tracked path, hit-offset law, nonterrain PROVISIONAL normal and sim data.
Replace BOLTM's mounted_remnant_styling_unchanged bug-preservation assertion
with a permanent correct fired-class referee; retain its surface/decoy tests.
No new schema, core simulation, constants, shaders, geometry, bake or gates.
Do not broaden into event matching/duplicate-shot redesign or lifecycle policy.

## Acceptance
1. Permanent asset-free public seating and actual WalkSim::step_tick/FIRE
   witnesses reproduce all four failures above with mutations to authored
   fixture weapon data BEFORE firing; no injected Shot/Impact or private
   remnant seeding for principals. Prove legal seating, actual gun-class
   Shot/Impact, struck expiry, empty live-bolt list and nonzero executed tests.
   Exact parent fails four intended assertions; same-class and on-foot controls
   pass. Require gun light radius/color and age, class/texture selection and
   differing width/core/glow lengths/colors to come from the fired class.
2. Use different gun/rider fade data and prove first-update midpoint, next
   terrain/world offset, lifecycle/fade duration and BF1 no-light behavior.
   Keep BOLTM/BOLTSURF terrain/Building/class/position/direction decoys, tracked
   multi-tick and ordinary on-foot controls; assert sim hash/events unchanged.
   Show rider-no-laser/slow cases draw1and gun-no-laser draws0 without changing
   the surface law. Add the opposite speed boundary so the rider's longer
   step cannot admit an unrelated struck expiry for a slower gun.
3. Independent compiled mutants must fail distinct assertions: restore rider
   lookup entirely; use rider class only for stored Remnant.class; use rider
   velocity only for reach/age; use rider laser eligibility. Preserve existing
   class/position/direction rejection mutants. Restore every mutant; no setup,
   compile failure, test skip or zero matches counts. Replace the old pin with
   a test that fails on parent and passes only on the intended fired-class rule.
4. Under shared CI lock run focused bolt_vis, full game library, fmt, workspace
   all-target Clippy and map/protect. Keep CRATES rows true. Full CI and all
   ordinary applicable gates remain required; no golden/baseline/pin/threshold/
   approval changes. A synthetic fixture never earns content acceptance.
5. Before acceptance, on exact parent/candidate with real class banks and
   identical HOT1/GEO1/FEL1 inputs, census same-tick mounted shots separately
   from tracked shots: correct gun eligibility, class, age, dimensions/texture,
   BF2 light color/radius/offset and fade for every supported correlation, zero
   mismatches. Repeat eight BF2 maps hot1/kas2/yav1/end1/geo1/tan1/nab2/fel1
   seed42/18000ticks, plus BF1 no-light controls; sim events/hash/outcomes stay
   equal. View paired GPU frames of mounted immediate hits and control shots,
   then full CI/map/protect/GPU/golden/parity/sim/sweep/arena/leak/bake/behavior
   gates. Preserve complete BOLTM/BOLTSURF/GCL16 Acceptance and pending holds.
