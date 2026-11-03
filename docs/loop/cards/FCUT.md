---
id: FCUT
title: The fusion cutter works: the repair weapon class in a soldier's hands - an amount per target type once per ShotDelay on the acquired target (BF2: pad +50, destroyed +50, damaged building +100, vehicle +200 every 0.5 s; BF1 +30 / +20 / +60 / +60) - and the green build hologram over an unbuilt pad, drawn only for a living player who carries a cutter
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
A pad becomes a turret only by a repair to full health (card TBLD's RE, `analysis/re-build-zones.md`). The tool is the
fusion cutter, weapon class `repair`. The port has no such class: `game/src/roster.rs` `held_slots` leaves the tool out
of every engineer's and pilot's hands, so neither the player nor a bot can build a pad, rebuild a fallen turret or
repair a vehicle. This card puts the cutter in the hands (player and bot alike; the bot's use of it is card BLDR) and
draws the build hologram, which the PS2 shows only to a player who carries a cutter.

## Evidence (`analysis/re-build-zones.md` 2 and 4, [O]; verifier 33 / 4 / 0)
- One shot = the class's amount for the target's type, added to its health (capped at max), once per `ShotDelay`
  while the trigger is held; the first shot lands at once. Types: 3 vehicle, 4 building, 5 destroyed building, 6
  unbuilt building. BF2 (all sides): 200 / 100 / 50 / 50 every 0.5 s. BF1 all / cis / imp / rep: 60 / 60 / 20 / 30
  every 0.5 s; gar / gun / wok / jawa: 30 / 30 / 10 / 15 every 0.25 s. Keys `VehicleHealth`, `BuildingHealth`,
  `BuildingRebuild`, `BuildingBuild`, `ShotDelay`.
- The player's target is the acquisition's (re-aim-assist 3): `LockOnRange` + the target's radius (BF2 4.0 m, BF1
  3.0 / 6.0 m), cone `LockOnAngle` (BF2 90, BF1 30 deg), the class's team and type masks. No target = nothing happens.
- Measured on the PS2: steps of exactly 50.0, 0.5 s apart; a BF2 turret (800) from 0 in 16 steps, 7.5 s from the
  first to the last; a BF1 gun turret (300) in 10 steps, 4.40-4.51 s.
- The hologram (`com_holo_turret`: green 0 225 98, image `com_icon_turret`, visible inside 35 m with a fade from
  31.5 m, 0.75 m over the pad, turning 0.2 turns a second, flicker 0.4-0.8, fade-in 2 s): on while the building is
  unbuilt, hidden on a screen unless its soldier is alive and carries a repair weapon in any slot.
  PS2 frame: `engine/artifacts/lead_tmp/tbld/ps2/yav1_holo_1b8b5c0_1.png` (state `.p2s` beside it; crop
  `yav1_holo_crop.png`).

## Acceptance
- A sim test, mutation-checked: a soldier holding the trigger with an unbuilt BF2 `tur_bldg_laser` acquired gives it
  50 at once and +50 every 0.5 s, and the 16th shot (7.5 s after the first) builds it; a BF1 `com_weap_gunturret` takes
  10 shots of 30 (4.5 s); a damaged BF2 hover gains 200 a shot; a pad 0.5 m beyond `LockOnRange` + radius gains nothing.
- The weapon census of a sweep map with engineers: every class that carries a cutter on the disc holds it in the port
  (BF2 slot 2 on channel 0, BF1 slot 3 on channel 1); no bot fires it at an enemy (count over a sweep: 0).
- The hologram: a PCSX2 pair at the pose of `lead_tmp/tbld/ps2/yav1_holo_1b8b5c0_1.p2s` with an engineer (the green
  icon over the pad) and the same pose with a rifleman (none); a test that the draw list holds the hologram only when
  the viewed unit is alive, on foot, carries a repair weapon and stands within 35 m.
- Declare the sweep / arena moves (a new held slot changes the sim hash of every class with a cutter).

## Notes
- Out of scope: BF2's slicing of enemy vehicles (`TargetEnemy 1`), mines (`MineHealth -1000`), the ammo table, the
  cutter's sound and HUD icon beyond what the weapon slots already draw (file a note if they are missing).
- The energy bar (`HeatPerShot` 0.06 against a bar of 1): one bar builds one BF2 turret (16 x 0.06 = 0.96). Whether it
  recovers while firing was not traced; the port's heat rule for other weapons applies.
- The hologram's model and class may need a bake step (`com_icon_turret`, `com_holo_turret` in BF2 SIDE/TUR.LVL and BF1
  COMMON.LVL): if so add the bake file to `files:` and `rebake=` to `declares:` before starting.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
