---
id: VTEAM
title: The vehicle's live team as the PS2 keeps it: team 0 (neutral) when empty for ordinary classes, the last entrant's in ANY seat while occupied, command vehicles never neutral, the pad's team kept apart as a home team; plus VENT's audit gaps (per-class HealthType, despawn paths unseat)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
AUDIT 2026-09-25 of VENT (ceb27c6) → PARTLY (`engine/artifacts/loop/audits.md`, report
`reports/20260925_2210_audit_VENT.md`). `Sim::vehicle_team` returns the driver seat's team, else the pad's. The PS2's
live vehicle team — the field CanEnter, the team relation and targeting read — follows different rules in both games.
VSEAT / VPAD / VAI built on VENT's rule would let no enemy take an empty vehicle, and an abandoned vehicle would stay on
its pad's side. Two small VENT gaps ride along. Nothing spawns a vehicle yet, so this is latent — fix it before VSEAT / VPAD.

## Evidence
`analysis/re-vehicles.md` §7.1 [O] (audit, one Opus verifier: 10/10 SUPPORTED, qualifications folded in):
- **Any seat sets the team.** A new occupant of ANY seat sets the vehicle's team to the occupant's: B1 FUN_0023c488
  B1:154825-154829, B2 FUN_002f5478 B2:283845-283846. The own seat, every MountedTurret and every PassengerSlot resolve
  to the vehicle.
- **Empty means neutral.** When the last occupant leaves (every seat empty), the team becomes 0 = neutral if the class
  flag is set:
  - B1: class+0x80, default 1 in FUN_00221b28 (B1:135870). 0 for commandhover / commandwalker (B1:224878, 230522).
  - B2: class+0x330 bit 34, default set in FUN_00269d98 (B2:186494). Cleared for commandflyer / commandhover /
    commandwalker (B2:348465, 348966, 354410) and 'mine'.
- **Pads leave empty vehicles neutral.** A pad gives an EMPTY ordinary vehicle no team: B1 FUN_002b0238
  B1:240605-240629, B2 FUN_003725f0 B2:370478-370520.
  - Command vehicles, and vehicles spawned with a pilot or a self-piloted seat, do take the pad's team.
  - The pad's team is also recorded apart, and AI code reads it:
    - B1: once per CLASS at class+0x88, unless the ODF bool 0x0ffe3c06 is set at +0x84. Read by FUN_00241f18 B1:159173.
    - B2: in the vehicle's bits 40-43, unless class bit 35. Read by FUN_002d6828 and FUN_002e2e38.
- **Who may enter.** CanEnter (B1 FUN_0020b1b0, B1:120029-120039): a team < 1 lets anyone in. Otherwise an AI needs the
  same team and a player needs a friendly relation.
- **HealthType is per class.** The ODF overrides the vehicle default (B1 FUN_00221e48, B1:136172-136195). BF1's
  `all_walk_tauntaun` is "animal" (`engine/assets_baked/classes/bf1_all.ron:6138`). VENT's `VehicleSpec::build`
  hardcodes `HealthType::Vehicle`.
- **Despawns keep the seat.** `Sim::shed_ai` (spawn.rs:99) and `Sim::vacate_player_slot` (spawn.rs:207) take a unit off
  the field without `unseat_unit`. Reproduced on ceb27c6 with `engine/artifacts/lead_tmp/audit-vent/scratch_despawn_tests.patch`:
  - The seat keeps the dead handle `(0, gen 1)` and `is_empty()` stays false.
  - After the slot is reused (gen 2), the new unit cannot take that seat.

## Notes
- **B1/B2 differences** (no BF1 vehicle is affected):
  - B1 counts a PilotType-1 ("self") seat as occupied. Its seat ctor sets pilot = self.
  - B2's seat setter treats PilotType 1 (and 4 piloting itself) as empty, but the B2 pad treats them as a SetTeam trigger.
  - The only BF1 class with a "self" PilotType is `imp_droid_probe`, a droid.
- **Home-team pad record.** B1 keeps the pad team per CLASS, B2 per vehicle. The port keeps it per vehicle (`home_team`),
  which covers both. Say so in the module header.
