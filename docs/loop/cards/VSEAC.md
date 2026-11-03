---
id: VSEAC
title: The vehicle seating chart on the HUD: BF2's player1vehicleseatinggroup (4 s on boarding - each seat's icon empty / self / player / AI and its name, e.g. CO-PILOT over Player 1) - the PS2 shows it in every vehicle and turret, the port's HUD tree has no vehicleseating element
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
analysis/re-seat-hud.md 6 (card VSHUD): on entry FUN_003b4bb8 sends player1.vehicle.seatingMesh (B2:410756-410768; again when bit 32 of *(FUN_003af9d8(p)+0x3940) is set, [H] a seat change), which enables Group("player1vehicleseatinggroup") at (0.129043, 0.596705), fade in 0.25 / hold 4 / out 0.25 (DUMP:2001-2057): a vehicleseating element with per-seat icons hud_vehicle_seatingchart_{empty,player1,ai}_icon and texts. PS2 frames: lead_tmp/vshud/ps2/b2atst_cold.png (CO-PILOT empty over Player 1), b2tower_cold.png (Player 1). The port's HUD tree has no vehicleseating element [receipt:6d2d5a7e].

## Do
1. RE: the vehicleseating element's draw (its constructor and per-frame layout in BF2's HUD code: row order, which name each seat shows - the class's seat label? - and the icon per occupant kind), from the layout records and one more PCSX2 frame of a vehicle with an AI in another seat.
2. Build: the element in hud_tree / hud_draw, fed per seat from the sim's seat ring on boarding.

## Acceptance
- PS2 | port pairs of the AT-ST and the tower turret frames above (the chart's rows, icons and texts in place to 2 px of 640 x 480), and one with an AI co-rider.
- A test: the chart shows for 4 s after mounting and lists every seat of the ring with the right occupant kind; mutation-checked.
