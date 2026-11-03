---
id: PWSTD
title: Broken health / ammo droids: a station shot to 0 (MaxHealth 80, BF2 health droid 200) explodes (com_inf_rechargedroid_exp), shows its _dst model and stops serving until rebuilt (BF1 pilot / BF2 engineer fusion cutter); BF2's *_unbuilt stations start unbuilt - RE who damages and rebuilds a station, then build it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
card PWST's RE (analysis/re-powerup-station.md §7, §8, §12): a station is an EntityBuilding with MaxHealth 80 (BF2 health droid 200); its building state (+0x2a4) goes to 1 (destroyed: DestroyedGeometryName, ExplosionName com_inf_rechargedroid_exp) or 2 (unbuilt) and then it gives, sounds and animates nothing until FUN_001c00c8 sets it back to 0. The port's stations (after PWST) cannot break.

## Evidence
re-powerup-station.md §7 (FUN_001bf400 / FUN_001bfb88 / FUN_001c00c8), §5.2 (BF1 tooltips 7 / 8 / 9 for a broken station under the reticle), §8 [H] BF2's engineer repair agent FUN_002d6828 may pick a damaged station; BF2 GEO1 places one com_item_healthrecharge_unbuilt.

## Acceptance
1. RE ([O], verifier): what damages a station (bolts, blasts, team rules), its death (explosion, the _dst model), how it is rebuilt / repaired (BF1 pilot, BF2 engineer: rate, who), whether bots shoot or repair stations; one PCSX2 measurement (shots to kill a health droid, BF1 and BF2).
2. Build: a station shot to 0 stops serving (test) and shows its _dst model; a rebuild brings it back; sweep / fidelity declared.
