---
id: FXP3
title: Particle effects, part 3: the spawn hooks — bolt impacts by surface (ImpactEffect Soft/Rigid/Static/Terrain/Water/Shield), explosion Effect/WaterEffect, TrailEffect, vehicle Damage/Chunk/Thrust/Wake/Stomp effects (re-fx §7); sim events carry the ordnance class and struck surface; PCSX2 pairs
milestone: M1
value: 5
phase: build
status: done
deps: FXP2
files: engine/crates/core/src/sim/state.rs,engine/crates/core/src/sim/combat.rs,engine/crates/core/src/projectile.rs,engine/crates/game/src/fx.rs,engine/crates/game/src/walk.rs,engine/CRATES.md
declares:
needs:
branch: fxp3
resume: landed cdbf9438 2026-09-30
created: 2026-09-30
alpha: yes
---

## Why
With FXP2 the port can run any PS2 effect; the game still has to spawn the right one. `analysis/re-fx.md` §7 ([O],
setters and spawn sites read in both ELFs, a census of the six showcase maps) says which ODF key names which effect
and when the PS2 spawns it.

## Evidence
`analysis/re-fx.md` §7; the per-weapon chains `engine/artifacts/lead_tmp/fxp/AGENT_C_weapons.txt` (muzzle → ordnance
→ impacts → explosion, with where each effect lives); B1 impact selector `FUN_00276810` (197939–198038), explosion
`FUN_002701a0` (193756–193807).

## Note (2026-09-30, FXB RE, `analysis/re-bolt.md` §5.1)
BF1's impact explosions also carry a `LightFlash` (explosion class LightColor / LightRadius / LightDuration: an omni
light at the blast, colour fading linearly to black over the duration, lighting MODELS only — BF1's terrain light pass
is gated off): 17 BF1 bolt types name a lit explosion (vehicle / walker cannons, orbital salvo; e.g. AAT cannon
250 170 5 190, r 5 m, 0.5 s; AT-AT / AT-ST r 20 m, 1 s); infantry rifles and pistols have no impact explosion.
Spawn rule: the same 60 m visibility test as the effect. Model lighting from runtime lights is BPNT's path.

## Note from FXP2 (2026-09-30, lane 2)
`Fx::spawn(name, Frame)` is the hook (game/src/fx.rs); a struck `BoltExpired` already spawns the rifle impact at the hit
with world axes (PROVISIONAL for non-rifle ordnance) — replace that route. Check the hit POINT the PS2 passes
(*(decompiler detail omitted)* of B1 `FUN_00276810`, spawn at 198038): spawned exactly on NAB1's terrain the port's flash loses its lower half
to the ground (depth tested, `lead_tmp/fxp2/cap/sheet_port_nab1_impact.png`) while the PS2's flash reads whole
(`lead_tmp/fxp/ps2cap/crop_nab1_impact.png`) — an offset along the normal or the bolt is likely.
