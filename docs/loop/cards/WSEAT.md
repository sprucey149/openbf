---
id: WSEAT
title: A walker's turret section keeps every seat key, as BF2's MountedTurret setter claims them through the seat setter FUN_002f6eb8 (B2:191723-191731): today PilotType, EyePointOffset, TrackCenter/TrackOffset, TiltValue and ThirdPersonFOV authored in a walker TURRET section fall onto the walker's own pilot seat (vehicle_class::scoped, walker::turret_claims); card VRIDE kept only the pose keys
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
Found building VRIDE (analysis/re-vehicle-riders.md 7): the port's walker scoping gives the turret setter first refusal with its OWN compare list only; on the PS2 the turret setter falls through to the Controllable seat setter and returns its claim, so every seat key of a walker turret section stays with that section. The port moves them to the walker's pilot seat: a walker gunner section's PilotType / EyePointOffset / camera keys override the pilot's (boarding, the gun's eye, the seat camera). VRIDE fixed the five pose keys (draw only, no sim effect).

## Evidence
- B2 FUN_00270770 tail B2:191723-191731: FUN_001d0328, FUN_00302f10, then FUN_002f6eb8(argument 1 + 0x60, ...) returned. FUN_002f6eb8 (B2:285090-285312) compares ~22 keys (list in this card's first session).
- vehicle_class.rs scoped(): by_walker && Scope::Turret && !TURRET_CLAIMS.contains(k) && !is_seat_pose_key(k) -> Scope::Body.

## Acceptance
- A census through the baked dbs: per walker class (AT-TE, AT-AT, AT-ST, spiders, one-man AT-ST, tauntaun, kaadu) every seat's PilotType / EyePointOffset / Track* before and after; a test that a walker turret section's seat key stays on its section (mutation: drop the claim -> fails).
- Sweep + arena: the move is declared (sim) with the classes whose pilot seat changes named; BF1's walker setter twin read first (does BF1's turret setter fall through the same way?).
