---
id: STILLCP
title: Still minimaps use the conquest mission's surviving posts and owners
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A still minimap can show a command post the conquest mission removes, with
world ownership colors that disagree with the same capture's holograms and
lights. Its fallback bounds can also include the removed post. This affects
still HUD/spawn captures and their checks; the live battle reads sim posts.

## Evidence
Split only GCL16 finding4. Existing analysis/re-vertex-colour-interp.md §6–7
records mission post removal and ownership as the rule already implemented by
point_light::still_mission/still_posts and holo::still. No new original-game
claim or parity measurement is made here. MapLayout::apply_mission already
handles roster, killed posts and ownership, retaining layout order.
On main886745a, asset-free public still/spawn_still caller probes load a valid
adjacent conquest mission, then fail three assertions: three posts instead of
two, world friend color instead of mission enemy, and a removed post retained
in spawn output. Bounds print centre[500,0]/extent600 instead of[50,0]/150.
A no-mission control passes with three world posts and original colors/bounds.
The probes were removed afterward; existing minimap tests pass3/3.

## Scope
Use the same existing conquest-mission selection/post resolution as the still
lights/holograms for minimap::still and spawn_still. Preserve surviving layout
order, post positions, number-to-name identity, selection index meaning and
the existing team1 viewer convention. Derive fallback bounds from surviving
posts with the existing100m padding; retain authored mapbounds unchanged.
Keep world-only behavior when the existing resolver has no valid mission.
No changes to mission resolution policy, battle minimaps, sim, HUD geometry,
point-light/hologram behavior, constants, schemas or rendering gates.

## Acceptance
1. Add permanent asset-free caller fixtures that invoke still and spawn_still
   with a valid mission loaded through the existing resolver (two legal teams,
   positive reinforcements). Use a temporary bake/adjacent missions directory;
   do not rely on installed mission data or silently skip. Exercise roster
   removal, KillObject and reownership independently, with case-blind names.
2. Reproduce the measured three-post fixture: CP1(x0/team1/HUDIndex1),
   CP2(x100/team2/HUDIndex2), CP6(x1000/team2/HUDIndex6), all z0. Mission roster
   CP1/CP2/CP6, killed CP6, owners CP1=2/CP2=1. Still output must have exactly
   two posts, colors enemy/friend, fallback centre[50,0]/extent150. Spawn output
   must retain positions/numbers1,2, selection1 and matching filtered bounds.
3. Also remove a MIDDLE post: surviving spawn numbers must stay attached to
   their names/positions rather than zip with the unfiltered layout. Keep a
   no-mission control, neutral owner, authored mapbounds, and all posts killed
   with the existing centre[0,0]/extent100 fallback. Preserve existing
   BF1/world fallback, live MapStatic::model/spawn_model and minimap tests.
4. The mission-sensitive caller assertions must fail on the exact parent;
   require passing legal-fixture setup first. Omit only mission application
   and require those assertions to fail again. A restored original-layout
   numbering zip must fail the middle-removal check. No compiler error,
   zero-test run, missing-input failure or skipped fixture earns this bar.
5. Run cargo test -p game --lib minimap::tests -- --nocapture, the full game
   library, cargo fmt --all -- --check, workspace all-target Clippy and full CI.
   Keep CRATES counts, size cap and all existing gates unchanged. On real
   BF2 DEA1/HOT1 missions compare still roster/owners/numbers with mission-
   applied layout and actual still HUD/spawn pictures; verify removed posts
   are absent and existing no-removal controls unchanged. Run all applicable
   CI/map/protect/GPU/golden/parity/sim/sweep/arena/leak/behavior gates before
   earned landing. View and declare actual golden moves through normal gates.
   Preserve GCL16's original Acceptance and all earlier validation obligations.
