---
id: VDMGA
title: Audit vehicle and turret health: player damage does reach a vehicle (an AT-TE at 10589 of 40000 after the player's rockets), but every vehicle a bot destroyed reads "destroyed by #-1" (no killer credited) and the spiders' health reads 17336 / 19952 / 20000 - check damage in, kill credit and the max-health numbers, vehicles and building turrets, both games
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer 2026-10-28: "turrets and vehicles can't lose health yet?", then "my health [rockets] was taking down clone vehicles, but vehicle and turret health should be investigated nonetheless". An outside contributor reports the fusion cutter (#35) cannot heal vehicles or turrets because their health never drops.

## Evidence
- Black boxes of 2026-10-28 (build a43f94e5+dirty, BF2 GEO1): F9 at 09:24 shows vehicle 12 `rep_walk_atte` hp 10589 (max 40000) beside the player's rocket trooper; the 09:14 run's events list five vehicles "destroyed by #-1"; spiders at 17336 and 19952.
- VDMG (done) built the hull pool; RIDK (done) the riders' killer; TCOL (blocked) building turrets as bodies.

## Acceptance
- Measured: a soldier's bolt, a rocket and a seat gun each lower a hover, a walker and a building turret's health by the disc's numbers (tests per kind).
- The killer of a destroyed vehicle is credited when a unit destroyed it (or the PS2's own rule, read).
- Building turrets lose health and can be repaired (the fusion cutter's precondition).
