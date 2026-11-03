---
id: VSHUD
title: The HUD while seated: BF2's reticle group (reticle, clip ring, heat ring, refire bits) at the seat gun's aim and the weapon panel with the seat's weapon and the vehicle's health - the PS2 shows them in a turret, the port hides both off foot
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
RE DONE 2026-10-22 (`analysis/re-seat-hud.md` [O], workflow-verified; live PS2 frames per seat kind
`engine/artifacts/lead_tmp/vshud/ps2/ps2_seat_sheet.png`): BF2 runs the SOLDIER's HUD code on the seat's controllable
(record +0x14c for a hover, a walker and a building turret alike). So in a seat the PS2 shows the seat's weapons in the
weapon rows (AT-ST: two rows), the heat ring / refire bits / "OVERHEATED!" exactly as on foot, and the reticle where the
SEAT's aim ray goes on screen: (0.5, y) of the point 1024 m along it through the drawn camera (Hoth tower turret: the
centre; speeder bike / AT-ST: 5 / 6 deg above it, the classes' TiltValue - the port's `vehicle_camera` already tilts).
The soldier's blue health bar is hidden on entry and the green vehicle bar (the vehicle's health; a building turret's
own health) shows at its place. The port's `hud_model` hides rows, reticle and target off foot and `reticle_y` returns
0.5 for a rider. The maintainer's ask (INBOX 2026-10-21 item 3) covers the seat guns.

(Original: Card OVHT's RE (analysis/re-heat-hud.md 6-7): on the PS2 BF2 a seated player keeps the reticle group and
the weapon panel (Mygeeto turret ...). The port's hud_model shows the weapon rows and the reticle only on foot.)

## Do (build, `analysis/re-seat-hud.md` 8)
1. `hud_model` (game/src/main/battle.rs) in a seat with a gun: row 1 = `GunSetup::weapons[0]` (FIRE, the PS2's
   channel 0), row 2 = `weapons[1]` if any, from the seat gun's class and its `GunState::weapons[i]` (counts, clip
   fraction, infinite for RoundsPerClip < 1, the class's HUD key / mesh); a seat with no gun: no rows
   (`weaponsDisable`). Boarding is a weapon change, so the name shows its 2.5 s (once the name text exists).
2. The reticle in a seat: `WalkSim::reticle_y` = the point 1024 m along `SeatLook::eye` + `SeatLook::view` through
   the drawn camera (x 0.5) - the on-foot rule with the seat's ray; gate it like the PS2: the vehicle / building alive
   (hover, flyer, building turret), a gun on the seat (walker).
3. The heat ring, refire bits and "OVERHEATED!": OVHT's events fed from the seat gun's `WeaponState` (deps OVHT; the
   seat half of OVHT is THIS card).
4. Health (ui/src/hud.rs `feed`): on mounting send `player1.healthDisable` and then `player1.vehicle.health` /
   `.healthFraction` from the vehicle's health (the port's building turret is itself the building); on dismounting
   `player1.vehicle.healthDisable` and re-send the soldier's health. Energy: the vehicle's own if modelled, else leave.
5. Not here: the seating chart (card VSEAC); the target readout in a seat ([H] in the doc).

## Acceptance
- PS2 | port picture pairs per seat kind (Hoth tower `turet12`, Endor speeder bike, Endor AT-ST - the PS2 halves and
  their save states are in `lead_tmp/vshud/ps2/`, `seat_hud.py` remakes them), the ring lit, with the sim's heat beside
  each; the port's reticle line within 2 lines of 480 of the PS2's (240.0 / 178.5 / 166.1) when the port's camera
  section matches (state the camera difference if not).
- A test: a seated player's HUD model carries the seat gun (rows, heat) and the reticle at the seat ray's projection,
  and the feed sends `healthDisable` + `vehicle.health*` on mounting and `vehicle.healthDisable` on dismounting;
  mutation-checked (gate the rows on foot again, project the soldier's ray, drop the disable).

## Cloud audit checkpoint (2026-10-24 07:29 UTC)
Claim AUDIT selected `analysis/re-seat-hud.md`; primary-evidence re-derivation is blocked, not completed. The RE note stays `[O] UNAUDITED`; no OK/PARTLY/WRONG verdict was earned. Both decompiled corpora and extracted ELFs, `lead_tmp/vshud/{tools,ps2,checks}`, `lead_tmp/re/bhud/dump_named.txt`, the cited VRIDE states, and PCSX2 tooling/references are absent here. Baked assets, missions and `/dev/dri` are absent too, so the build acceptance cannot currently run. Report: `reports/20261024_0729_audit_seat_hud.md`.

Restore these locally owned inputs, verify the PS2 caller/vtable/ControlsUnit paths, replay the 20 BF2 and 6 BF1 states and remeasure heat against each state's own Screenshot.png before issuing an audit verdict. Then complete OVHT and the original VSHUD acceptance; clear the cloud block only after verifying prerequisites. No acceptance threshold changed.

Previous build continuation (preserved): BUILD from analysis/re-seat-hud.md section 8 (Do 1-4). Rides on OVHT's heat events (deps OVHT). No golden shows a rider and the sim is untouched: declares is expected to stay empty - check with gates.py --auto. PS2 halves for the pairs: lead_tmp/vshud/ps2/b2{tower,bike,atst}_*.png + .p2s.
