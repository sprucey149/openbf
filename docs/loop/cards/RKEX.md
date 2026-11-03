---
id: RKEX
title: A landed rocket explodes: the rocket launchers' ordnance (the jet trooper's, the torpedo launchers') fires like a plain bolt today - no ExplosionName blast, no splash damage, no explosion effect - and the maintainer saw rockets pass through the ground, vehicles and units; reproduce that, then build the explosion on impact
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
(the maintainer, 2026-10-20, playing the jet trooper) Rocket launchers 'don't have the explosion'; the rockets (drawn as the placeholder prism) 'travel through things like the ground, vehicles and units and don't explode'.
Read in the code: the rocket's ordnance class (BF2 `rep_weap_inf_rocket_launcher_ord`: model com_weap_missile, a rocket-trail effect, ExplosionName `rep_weap_inf_rocket_launcher_exp`) is built as a plain bolt (card UORD: 'every primary's ordnance as a straight bolt'). `Sim::resolve_bolt` (core/src/sim/combat.rs) applies the bolt's damage and a puff impact on a unit / vehicle / world hit, but nothing reads ExplosionName: no splash damage and no explosion effect, so a rocket lands as a bullet. The spawn ray DOES test terrain, world bodies, vehicle hulls and units, so 'passes through' is not explained by the code - it may be a drawing / timing artefact (the placeholder prism outliving the bolt), the ORDNANCE-layer subset (bodies without an ordnance layer let a bolt through, as on the PS2), or a real miss; hence reproduce first.

## Evidence
`core/src/projectile.rs` (`Bolt::spawn`, one ray at launch), `core/src/sim/combat.rs` `resolve_bolt`, `core/src/sim/blast.rs` (radius damage that exists for vehicle wrecks and thrown grenades), card UORD (the other ordnance kinds; RE for flight), card GREN (`analysis/re-grenades.md`: the explosion rule for thrown ordnance), card FXP (explosion effects).

## Notes
