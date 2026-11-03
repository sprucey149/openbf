---
id: VCHNK
title: A destroyed vehicle throws its CHUNKn bodies: the baked chunk models launched by re-vehicle-death §6.1, 40-kg cosmetic rigid bodies (gravity, ChunkBounciness/Stickiness, ChunkTerrainCollisions, settle), gone 3 s (BF1) / 8 s (BF2) after resting; the _exp class's NumChunks debris (5 s)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Split from VDTH (2026-09-29): VDTH landed the blast's area damage and a placeholder fireball; the chunks the PS2 throws at a vehicle's death (and the _exp class's debris) are still missing — a destroyed hull just flashes and is gone.

## Evidence
analysis/re-vehicle-death.md §6 [O]: launch law (B1 FUN_001e2630, B1:91931-92126: vel × ChunkVelocityFactor + dir × ChunkSpeed, or U(15,30)/w + the |v|<5 floor; w = the class's blast sphere, now `Sim::blast_setup`), the body (mass 40, restitution ChunkBounciness 0.6, friction ChunkStickiness 0.7, damping 0.001/0.004, B1 g −9.8 / B2 ChunkGravity), settle rules, lifetimes (BF1 3 s / BF2 8 s at rest; explosion debris 5 s), the 8×10 pool. VBAKE bakes the chunk models (ManifestVehicle::chunks). The per-chunk prim positions (the centroid c and d) need the CHUNKSECTION's ChunkNodeName / prim — check what the bake keeps.

## Acceptance
game-only presentation (chunks never touch the sim — the PS2's chunk-vs-vehicle response is [H]): on Event::VehicleDestroyed spawn the class's chunks with §6.1's launch, integrate §6.2 against the heightfield (and world collision if cheap), draw them with the chunk GLBs, remove on the lifetime. Unit tests for the launch law and the lifetime. A capture or scripted frame that shows chunks flying after a scripted destroy (a scratch example, never a golden unless declared).

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — explosions: a destroyed vehicle's chunks. Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Cleanse note (2026-10-29)
Most of the launch/draw/lifetime is now in `game::fx::chunks` (night debris work, issue #49): the §6.1 launch law,
floor contacts that settle after three bounces, BF2's 8 s rest on every map, a 30 s airborne bound, droid and
vehicle death events. Its provisional deviations are in FEEL_NOTES (sphere contacts not rigid bodies, mesh-bound
centres instead of ChunkNodeName, cosmetic spin). Still open for acceptance: the body model of §6.2, ChunkNodeName
origins, ChunkTrail effects, the side rebake (`scripts/rebake.sh side`) and a capture after a scripted destroy.
