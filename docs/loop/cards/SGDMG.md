---
id: SGDMG
title: A bolt that lands in its first update strikes at age 0 - BF2's shotgun pellet does its full 65 (head 195) up close, as PCSX2 shows, not the falloff's 48.3
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
A playtest found the BF2 shotgun far weaker than on the PS2. The port billed every pellet as one update (1/30 s)
old when it first struck, so the age falloff (65 -> 15 over 0.1 s) cut it to 48.3: a point-blank body shot did
386 and left a 400-health soldier alive.

## Evidence
PCSX2, Dagobah, health reset before each shot, target 3 m ahead: every reading a multiple of 65 (520 = 8 body,
1560 = 8 head, mixes such as 1430 = 1 body + 7 head). `analysis/re-ordnance-kinds.md` section 7 has the table.

## Change
`Bolt::advance` adds a tick to the age only when the flight goes on, so the update in which a bolt lands uses the
age it had when that update began (the PS2 runs every ordnance of the bolt family, missile included, through
the same core and sweep). A falloff that drops (the shotgun) does one step more at impact; one that rises (the
SBD wrist rocket, 100 -> 250 over 0.5 s) one step less: its 3 m chest hit is 100 + 250 blast = 350 of a clone's
400, not 360. The moved pins: `v5_bolt_flight_time_to_20m_target` (hit age 7 ticks, not 8) and
`sbd::a_wrist_rocket_to_the_chest_on_coruscant_deals_its_hit_and_its_blast`, `sbd::sbd_fires_bolts_and_rockets_with_identical_gameplay_on_both_map_sets` (130 -> 120).

## Open
The 10 m and 25 m PS2 readings (the steps past the first update) are not measured.

## 2026-10-30: the wrist rocket now flies

Card UORD made `com_weap_inf_wrist_rocket_ord` a real missile (`OrdKind::Missile`), so the round
spawns at `MinSpeed` 20 m/s and accelerates at 80 m/s² up to `Velocity` 80 instead of resolving its
hit from the firing press's whole-flight ray. The PS2 does the same: BF2's missile instance starts at
`MinSpeed`, and its update runs `speed = min(speed + Acceleration dt, Velocity)`
(`analysis/re-ordnance-kinds.md` §4.1 lines 159-160, §4.2 line 172, §4.3 line 192), so these are the
faithful numbers, not a regression.

The hit is still billed by the age falloff (`Falloff { start 0, duration 0.5, end_damage 250 }` on
`MaxDamage` 100) at the age the landing update began with - the rule this card landed. What changed is
that the round now takes several updates to arrive, so the arming curve has run:

- `sbd::a_wrist_rocket_to_the_chest_on_coruscant_deals_its_hit_and_its_blast`: direct 100 -> 130
  (0.1 s of flight at 3 m), blast 250 unchanged; the clone is left with 20 of 400, not 50.
- `sbd::sbd_fires_bolts_and_rockets_with_identical_gameplay_on_both_map_sets`: direct 120 -> 170
  (0.2333 s at 8 m), blast 250; 9180 of 10 000 left.

The old 100 and 120 were the bolt approximation, not the original's damage: with the whole flight
collapsed into one press ray the curve was read at age 0 (one tick, after this card's change), so the
round's 20 -> 80 m/s climb never happened. Both map sets still read the same 170 because BF1's
`cis_inf_battledroid` flies BF2's `cis_inf_rifleman` weapon set through `roster::WEAPON_DONORS`, and
the ramp flag is `MissileClass::ramps = db.is_bf2()` on that donor db; card SBD's "BF2 SBD weapon
mechanics ... shared by both map sets" therefore holds for this shot, and no per-disc pin is needed.
(A BF1-authored wrist rocket - `bf1_cis.ron`'s own `cis_weap_inf_wrist_rocket_ord`, Velocity 50, no
`MinSpeed`/`Acceleration` - would fly flat; no test fires one.)
