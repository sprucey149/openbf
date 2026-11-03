---
id: VDTH
title: A vehicle's death spectacle: the ExplosionName class's area damage (falloff, who it hits) and the CHUNKn bodies (launch, bounce, lifetime) — on VDMG's Event::VehicleDestroyed
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
VDMG (e7ed805) removes a dead vehicle and emits Event::VehicleDestroyed, but nothing detonates the _exp class or throws the chunks: a vehicle vanishes. The blast also damages what stands near it.

## Evidence
analysis/re-vehicles.md §6: the hover tick spawns ExplosionName at 0 health (B1:97306) and throws the chunks (FUN_001e2630, B1:97314; the launch law B1:91931-92121, bounciness 0.6 / stickiness 0.7 defaults B1:93002-93007). The explosion's damage vs distance is NOT RE'd: re-blaster-damage.md §6 only guesses the TakeDamage multiplier is a radius fraction [H]. Disc: imp_walk_atst_exp 50 damage radius 20; imp_hover_speederbike_exp 20 / 10 (expc classes). Chunk lifetime [H]; ChunkTerrainCollisions, ChunkOmega, ChunkTrailEffect are authored.

## Notes (RE done 2026-09-28, lane 2 — analysis/re-vehicle-death.md, [O], verifier 23 SUPPORTED / 4 PARTLY folded / 0 NOT)
- Build from §8 there. The blast is one function for every explosion: gather ≤ 20 in a box of half-side
  max(DamageRadiusOuter, PushRadiusOuter); LOS blocked only by buildings/terrain; f = 1 inside Inner, linear to 0 at
  Outer on d' = d − w/2 (DamageRadius = Outer, Inner default 0); the bolt's TakeDamage (HealthType scales, FF scale,
  AI never hurts a human teammate). Riders of a vehicle skip. The vehicle's own blast: its driver is the attacker, its
  team the team.
- w per victim: vehicles = max(modl SPHR r, the farthest AddSpringBody / ODF collision primitive) about the SPHR
  centre — the §3 RAM table (16 BF1 classes) is the test oracle; soldiers per stance 1.4626 / 1.1023 / 1.6221 (centre
  feet + 1.0 / 0.75 / ≈0.35).
- Push is card KNOCK. BF1 knocks back the living and corpses (states 5/8/9/12/13 excepted); BF2 pushes only the dead
  by default. AUDIT 2026-09-29: the soldier splash also gives anyone dead after it a +5 m/s kick along the blast
  (FUN_00200f90) — also KNOCK. The splash code in this build should leave a hook for it. Chunks are presentation in the
  port (a reversible call: the PS2's chunk-vs-vehicle contact response was not traced): §6 launch, 40-kg body,
  bounce/stick, settle, 3 s (BF1) / 8 s (BF2) at rest, explosion debris 5 s.
