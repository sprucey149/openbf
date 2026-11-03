---
id: CHTAM
title: The infinite-ammo cheat should still empty the clip and reload: the port's cheat never takes a round from the clip (weapon.rs `if !infinite_ammo { self.clip -= ... }`), so the rocket launcher (1-round clip) and the SBD wrist rocket fire at their raw refire rate with no reload; RE the PS2 cheat and match it
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer 2026-10-28: "you can just spam click rockets. In the originals there is a delay and the guy does a reloading animation ... the original infinite ammo cheat still counted through a magazine so reloading was still counted."

## Evidence
- Black box `prev1.json` 09:24 (BF2 GEO1, the player a CIS rocket trooper): 55 shots, `cis_weap_inf_rocket_launcher` clip 1 spare 6 unchanged.
- `core/src/weapon.rs` 1238 (no auto-reload), 1343 (no empty-clip stop), 1373 (no round spent) all skip under `infinite_ammo`; the flag is `cheats.infinite_ammo && is_player` (sim/step.rs 751, guns.rs 396, ordnance.rs 37).
- No RE of the PS2 cheat yet (grep analysis/ 2026-10-28).

## Acceptance
- RE note: what the PS2's infinite-ammo cheat changes (expected: the reserve is endless, the clip empties and reloads).
- Built: with the cheat on, the clip counts down and reloads with the reload time; a test per weapon kind (rifle, rocket launcher, wrist rocket, heat weapon).
