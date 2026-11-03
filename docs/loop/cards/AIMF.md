---
id: AIMF
title: The soldier's magnet acquisition is the PS2's filtered cone (FUN_0025e098: sphere query, Target team/kind bits, cone score, LOS), not the MountedTurret ray the port copied — it can magnet onto friendlies
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The port's soldier magnet (`core::sim::input` `Sim::acquire_magnet`, `engine/crates/core/src/sim/input.rs:118-145`) takes
the nearest capsule the aim ray hits, ANY team — copied from `re-aim-assist.md` §3, whose function turned out to be the
MountedTurret's. On the PS2 the soldier never magnets onto friendlies and picks by a cone, not a ray. Found by VEH0's aim
RE (not an audit job, but a WRONG finding → a card, value ≥ 4). Feel: M3.

## Evidence
`analysis/re-vehicles.md` §9 [O] (verifier claims A1, A2 SUPPORTED / PARTLY-folded): FUN_0025e098 (B1:180429-180892)
— sphere query radius weaponClass+0xd0 (≤ 300, mask 0x1e), team bit (+0xf4 >> (FUN_00221788 + 1)), kind bit
(+0xf8 >> kind), the FUN_00241b98 cone score, an LOS ray (mask 0x48), stored only within LockOnRange + radius, a sticky
current-target path (B1:180600-180642). The correction banner is at the top of `re-aim-assist.md` §3.

## Do / Acceptance
Transcribe FUN_0025e098's acquisition (B1:180429-180892, incl. FUN_00221788's relation encoding and the +0xd0 radius
prop 0x8474ea2c) into `re-aim-assist.md` §3 (replacing the turret text, which moves to a "MountedTurret" subsection),
checks + one verifier; then a build: `acquire_magnet` = the PS2's filtered cone; tests: a friendly in the crosshair is
never the magnet, an enemy just off the ray is; declared `sim` / `arena` moves.

## RE done (2026-09-28, lane 2): `analysis/re-aim-assist.md` §3, [O]
Verifier: 30 SUPPORTED, 2 PARTLY (folded), 0 NOT. 23 BF1 save states were checked in RAM. The spec is §3.3 (the
algorithm), §3.4 (the score), §3.6 (a soldier's bounds and aim point) and §9 (what the port needs).
