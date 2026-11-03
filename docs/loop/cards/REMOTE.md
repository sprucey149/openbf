---
id: REMOTE
title: The remote droid: the `remote` weapon throws a hovering `droid` the soldier flies (lift spring over the ground, stick to MaxSpeed), fires its blaster, holds 3 s for the destruct (BF1 orbital salvo, BF2 incinerator burst), runs down on its AddHealth / energy drain and hands control back
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
`analysis/re-remote-droid.md` (`[O]`), issue #44. BF1's marksmen, sharpshooters, scout troopers and sniper / assassin
droids and BF2's officers and Wookiee warriors carry a remote droid in channel 1; the class kit dropped the slot (8 BF1
/ 13 BF2 classes counting variants).

## What to build
1. **The weapon** (§2): a dispenser throw of the `OrdnanceName` droid; one droid per weapon, no throw while it lives;
   0xc43b77dd hands the soldier's controls to it at launch; a fresh press of the trigger retakes it.
2. **The droid** (§3, §4): the class keys with the constructor defaults (BF1 reads only "Acceleraton"); the flight step
   of `FUN_001c9fd8` (gravity, then lift over a 10a ground ray, stick toward MaxSpeed / MaxStrafeSpeed under the
   acceleration cap); yaw and pitch within `PitchLimits`; `AddHealth` and BF2 `EnergyAutoRestore` drain it out.
3. **Its weapons** (§3, §5): the channel-0 blaster from its WEAPONSECTION; the channel-1 destruct 3 s after one press (a countdown that cannot be cancelled) —
   BF1's salvo of `FUN_00272078`, BF2's incinerator burst; the droid's own `ExplosionName` when it dies.
4. **Damage**: bolts and blasts hit it by `DroidScale`.
5. **Game**: the camera at `TrackOffset` / `EyePointOffset`, the `com_weap_probedroid` model, the HUD's health and
   energy, the charge and destruct sounds and effects.
6. **Bots** (OURS): launch at a target within 40 m, fly at it and fire.

## Acceptance (run and paste into the report)
- Unit tests: hover just over `SetAltitude` by the gravity-first offset; the stick reaches MaxSpeed along the heading;
  out of the ground ray's reach it falls and cannot steer; the drain ends it at the lifetime; the salvo frame and its
  rounds converge on the target from the authored angle; launch -> steer -> ACTION -> retake; run-down returns control
  without a second throw; blaster kills credit the owner; blasts and bolts by DroidScale; one press counts down 3 s to
  the salvo, the launch and retake presses do not start it; the reticle follows the flown droid's aim; the render blend draws a droid between ticks and previews its mouse look; a bot launches, flies and fires; droids are hashed and replay exactly.
- Census: every carrying class of both discs builds the slot (channel 1, 1 clip), reads the disc's droid / blaster /
  destruct values, flies forward, hands back and expires at its lifetime ±0.2 s (8 BF1, 13 BF2).
- Gates: `sim` declared.
