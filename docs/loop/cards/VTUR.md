---
id: VTUR
title: Gunner seats and vehicle weapons: MountedTurret sections, aimers (rate-limited, clamped, aimed at the camera ray's hit), fire points; building turrets as seat-only vehicles; the in-vehicle aim/lock-on call
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Vehicle weapons and gunner seats (the skiff's three guns, the AT-ST's chin and head guns), and every building turret
on the maps (`com_weap_gunturret` and BF2's `tur_*`), use one seat/aimer machinery.

## Evidence
`analysis/re-vehicles.md` §5 (TURRET1..8 sections = MountedTurret seats with up to 4 weapons each, props routed turret →
camera → aimer → seat), §8 (armedbuilding = the same sections, no pilot seat → a seat-only vehicle), §9 (aimers: defaults
pitch ±45°, yaw ±30°, PitchRate 1.0, YawRate 2.0 rad/s; rate-limited toward the 512 m camera ray's hit, clamped to the
limits; BF1 magnetism runs in any seat whose weapon has LockOnRange > 0; BF2's lock-on is one mechanism on foot and in
every vehicle kind — its button and acquisition loop not yet located).
