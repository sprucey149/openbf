---
id: RIDK
title: A rider of a destroyed vehicle: who killed him? The port kills every rider as a WORLD death with no killer (Sim::destroy_vehicle, PROVISIONAL since card VDTH: 'killed or ejected [H]') - 78 % of all world deaths on main are these riders, and every card that makes bots better at destroying vehicles raises the fidelity gate's world_death_pct (HVYW: 1.08 -> 1.48). RE what the PS2 does with the occupants and the kill credit, then build it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
When a vehicle's hull pool runs out the port kills its riders through `kill_environment(u, DeathCause::VehicleDestroyed)`: a death with
no killer, no kill on the scoreboard, counted with the falls and the death regions. The code says so itself
(`core/src/sim/vehicles.rs` `destroy_vehicle`: 'at death B1 FUN_001ea190 calls each seat object's vtable +0xc (B1:96789), killed or
ejected [H]. PROVISIONAL (ours): every rider dies'). The player who shoots a tank down gets no kill for its crew.

## Evidence (card HVYW, 2026-10-13; `lead_tmp/hvyw/tools/wcause.py` over the sweeps in `lead_tmp/hvyw/sw`)
- Main fb4a2f1, three blocks of 8 seeds x 32 maps (`main`, `main_b2`, `main_b3`): world deaths 537 / 556 / 588, of them riders of a
  destroyed vehicle 421 / 445 / 473 (78 - 80 %). The gate's world_death_pct reads 1.08 / 1.14 / 1.13; with the riders taken as kills
  it would read 0.22 / 0.21 / 0.20.
- Branch hvyw 548ef04 (rockets at vehicles; `clean9`, `br9_b2`, `br9_b3`): riders 574 / 596 / 622, the other world deaths 143 / 103 /
  112 against main's 116 / 111 / 115; world_death_pct 1.48 / 1.41 / 1.47 - the whole rise is riders.
- The blast of the wreck already carries the PILOT as its attacker (re-vehicle-death 4 [O]); who is credited with the riders is unread.

## Notes
- 2026-10-13 lane 1 (HVYW): filed from the fidelity gate's reading of HVYW. HVYW's own change is not in this path (it fires the
  rockets; `destroy_vehicle` is untouched).
- 2026-10-16 lane 2 (RE, report `reports/20261016_1530_card_RIDK.md`): RE done, `analysis/re-vehicle-death.md` §10 (f7f19c6).
  Both games KILL every rider with the vehicle's last-damage record (= the port's `killer`, the killing hit's attacker): one kill per
  rider for the destroyer (a team kill if friendly), a ticket each, past the invulnerability switch; a crash into the world (no
  attacker) = deaths with no killer. Order on the PS2: the destroyer's vehicle-kill stat, the blast (driver as attacker, riders
  immune), the chunks, then turrets, passengers, the driver. Live BF1 (RHN1 + NAB1, `lead_tmp/ridk/tools/ridk_probe2.py`):
  6 hovers, 11 riders, every rider's record = the vehicle's, each destroyer's kills + riders within 0.1 s. Presentation only:
  exposed riders leave a corpse, riders inside the hull are deleted (§10.5). The fidelity gate's `world_death_pct` should fall
  (HVYW's reading: 1.08 -> about 0.2 with the riders as kills) - a move the planner re-takes the baseline for, not a refusal.
