---
id: WRST
title: The super battle droid's wrist rocket: no soldier of the port holds it - a channel-1 weapon that is not thrown is dropped by roster::held_slots, so neither the player nor a bot can fire it (found by card HVYW, whose choice rule for it runs on a fixture only); build the secondary-channel missile for the player and the bots (the maintainer's INBOX 2026-10-12 16:20 names it)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer's note of 2026-10-12 16:20 asks that a super battle droid fire its wrist rockets at vehicles and at groups. Card HVYW built the
choice (a heavy weapon: at a vehicle, or at a crowd of 3 within 9 m, never at a lone man) and found the weapon is in nobody's hands.

## Evidence (card HVYW, 2026-10-13, branch hvyw 548ef04)
- The baked class db, read by a scratch test: BF1 `cis_inf_battledroid` and BF2 `cis_inf_default_rifleman` hold
  `cis_weap_inf_wrist_rocket` in slot 2 on CHANNEL 1; `game::roster::held_slots` builds a channel-1 slot only when it is a thrown
  weapon (its doc: dispensers, mines and tools wait for their cards), so the droid's held slots are the blaster and slot 1 only.
- The census over 8 seeds x 32 maps (`lead_tmp/hvyw/sw/clean9/hv`): no row names a wrist rocket - never in hand, never fired; the
  droid's rows are `cis_weap_inf_wrist_blaster` and `wrist_shrapnel` / `wrist_trishot`.
- `ai::arsenal::armed` answers `Armed::Hand` for a channel-1 pick that is not thrown: the gunner would press FIRE with the blaster.
- The class: one round, a 2 s reload, AI range 0 / 16 / 48 m, a missile of 100 (BF2), `AITargetVehicle 0` (`engine/crates/game/tests/suite/roster.rs`
  `the_baked_launchers_are_missiles_for_vehicles`).
  Current path confirmed by the 2026-10-24 checkpoint. That test checks class-db
  slots and skips without real classes; held-slot admission still needs this
  card's baked-droid scene and census. A skipped test is not acceptance.

## What to build
1. Read `analysis/re-grenades.md` 2.3 / 2.4 first (channel 1 has the soldier while its fire state runs; the switch per channel): what
   the PS2 does with a channel-1 weapon that is a cannon, not a grenade - trigger on press or release, the aim (no lob), the HUD slot.
2. `roster::held_slots`: build a channel-1 slot whose class is a missile / cannon weapon; `Sim::held_weapons`: fire it as a bolt from
   channel 1 (the grenade's path is the throw); the player's SECONDARY_FIRE fires it.
3. Bots: `arsenal::armed` gets the case (a channel-1 weapon fired straight: aim as a rifle, press SECONDARY_FIRE on target inside its
   band, the friend-in-line rule with the missile's wind-up); HVYW's choice already treats it as heavy.

## Acceptance
- A scene test, mutation-checked: a droid with the baked class's slots fires its wrist rocket at a hover tank in range and at a 4-man
  group, and its blaster at a lone soldier; the player's secondary trigger fires one rocket and the 2 s reload runs.
- The census (`lead_tmp/hvyw/tools/hvcen.py`, HEAVY_CENSUS on `round_to_outcome`), main beside the branch, 8 seeds x 32 maps: wrist
  rocket shots by target kind - at a single soldier under 10 %; team kills not above main's per map; 256 / 256 rounds.

## Notes
- 2026-10-13 lane 1 (HVYW): filed. A rocket has no blast in the port yet (card UORD): one hits one man.
