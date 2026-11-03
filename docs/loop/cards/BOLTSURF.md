---
id: BOLTSURF
title: Same-tick bolt lights use the struck surface rather than terrain proximity
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A bolt that hits within its first tick can put its BF2 impact light on the wrong
side of a surface. The same-tick remnant guesses terrain from a height gap;
tracked bolts instead retain the simulation's actual Hit::Terrain identity.

## Evidence
Split GCL16 finding10. Existing analysis/re-bolt.md section5.2 specifies the
impact offset; bolt_vis already uses terrain normal for terrain and reverse
flight for other surfaces (the latter remains PROVISIONAL). This card changes
only which existing branch is selected, without claiming a new PS2 normal rule.
Sim::resolve_bolt emits Event::Impact with Surface::Terrain/Building/Vehicle,
position, direction and weapon class; no new event or simulation data is needed.

Temporary caller probes on main3828393 use WalkSim::new(seed42,spawnZERO), a
33x33 Heightfield with unit8/offsets128 and normal PadState input (ry=-32767
for18..22 ticks, then FIRE). The built-in rifle and actual events are used:
- Slope heights[i] = -2 * ((i/33)*8 -128), h(x,z)=-2z: five first-tick
  terrain impacts are labeled nonterrain. At pitch18, head is approximately
  (0.05830845,1.0059658,-0.72590727), height gap0.446m; next-update light is
  0.098503m from head+0.5*terrain_normal. Pitch22 error0.162996m.
- Flat h=0 plus CollisionWorld box translated(0,-0.4,0), half(128,0.5,128):
  four first-tick Building impacts (pitch19..22) at y0.100000 are labeled
  terrain. Next-update light errors against the existing reverse-flight
  branch are0.424343/0.409178/0.393898/0.380052m.
- Flat terrain without the box passes:23 first-tick hits over pitch1..40.
  Original bolt_vis tests pass3/3 after all instrumentation is removed.
These are synthetic caller results, not shipped-map prevalence or parity.

## Scope
Use existing per-impact surface identity for same-tick remnant terrain choice.
Keep correlation tied to the matching shot/impact position, direction and
weapon class; do not select an unrelated first Impact. Preserve tracked hits,
remnant lifetime/fade/reaching midpoint, BF1 no-light behavior, existing
nonterrain PROVISIONAL offset and sim state. No event/schema/hash/geometry,
ray-march/threshold, light strength/radius, render shader or bake changes.
Do not solve the mismatch by widening the0.2m heuristic: near-ground buildings
prove that height proximity cannot determine the struck surface.

## Acceptance
1. Permanent asset-free caller tests reproduce BOTH fixtures above through
   real WalkSim::step_tick -> BoltVis::update/omni_lights, with no injected
   Shot/Impact or seeded private remnant state in the principal witnesses.
   Require actual Impact surface and empty live-bolt list on the firing tick,
   one remnant and reaching midpoint, then the correct existing branch on the
   next update (position tolerance1e-4); expiration/fade stay unchanged.
2. Keep a passing ordinary flat-terrain control, slope-terrain control, BF1
   no-light control and a tracked multi-tick hit. Add decoy/multiple-impact
   cases with differing position, direction and weapon class so unrelated
   events cannot select terrain. Use direct event controls only in addition
   to the real-step witnesses. Test nonterrain impact near terrain and with
   no heightfield; preserve the existing reversed-flight fallback.
3. Exact-parent principal fixtures must fail by assertion after proving legal
   setup/actual impact. Restore only the old height heuristic and require
   both corresponding assertions to fail; force either terrain choice and
   require the opposite-surface control to fail. A mismatch of correlation
   inputs must fail its decoy assertion. Restore and rerun clean; no compile
   errors, zero matches, skips or missing inputs earn this bar.
4. Run focused bolt_vis tests, full game library, fmt and workspace all-target
   Clippy under the shared CI lock, then full CI when prerequisites exist.
   Keep CRATES counts and all ordinary gates unchanged. Demonstrate visual
   update leaves simulation hash/events unchanged in the caller fixture.
5. Before earned acceptance, restore actual BF2 HOT1/terrain and world-surface
   scenes, real classes/common light textures, paired inputs/references and
   approved GPU/goldens. Exercise first-tick and tracked impacts, view the
   resulting light positions, compare unchanged no-mismatch/BF1 controls and
   run all applicable CI/map/protect/GPU/golden/parity/sim/sweep/arena/leak/
   bake/behavior gates through the normal isolated-worktree workflow. Declare
   actual measured golden moves before any approved update. Synthetic tests
   are implementation evidence only; preserve GCL16 and earlier full debts.
