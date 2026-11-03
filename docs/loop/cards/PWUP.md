---
id: PWUP
title: Pickups, built: a dead soldier's drop (its class's table, one roll, the pool cap), the item on the ground (falls, lands, 31 s), the pickup by touch (health to the cap, +clips, either team, only when it gives something), drawn and heard; BF2's energy and offense / defense items
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
`analysis/re-powerups.md` (`[O]`, commit 772d174; verifier 60 / 3 / 0 of 63; four live PS2 battles). On the PS2 a dead
soldier drops a health or ammo pickup on 80 % of deaths (BF1 Alliance / Republic / CIS), 25 % (BF1 Empire) or 70 %
(BF2); it lies 31 s and anyone of either team who walks over it and needs it takes it. The port has none of it: no
drop, no item, nothing drawn (M-ALPHA lists pickups).

## Notes
- Created 2026-09-28 by the planner. M3 (plays right).
- UAUD1 (2026-09-29, analysis/unit-audit-bf1.md §3.4): every BF1 class authors `DropItemClass` / `DropItemProbability` / `NextDropItem`
  (read by B1 `FUN_0020f2e0`, 0x406d2b75, line 123060). Three profiles: ammo 0.40 / health100 0.30 /
  `com_item_powerup_dual` 0.10 (ALL, REP, CIS rank and file); ammo 0.05 / health25 0.10 / health100 0.10 (IMP, the ALL
  basic marksman, the ALL snow vanguard); ammo 0.20 / health25 0.05 / health100 0.0 (heroes, Wookiee locals); Jawas
  0.5 / 0.4 / 0.1. The pilots' `*_powerup_dispenser` (ch1) places `com_item_powerup_dual`.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.4): all 49 fielded BF2 classes inherit `com_inf_default`'s (INGAME.LVL) drop list:
  `com_item_powerup_ammo` 0.25, `_health25` 0.15, `_offense` 0.02, `_defense` 0.02, `_health100` 0.08, `_dual` 0.08,
  `_energy` 0.10 (7 entries, each with NextDropItem "-"). Heroes (`com_jedi_default`) author their own 6. The engineers'
  ch1 `*_powerup_dispenser` places health & ammo.

## Note from GREN (2026-09-30, analysis/re-grenades.md §5.1, §5.2)
The item entity's physics is read there (B1 class `FUN_001f4408`, update `FUN_001f3aa0`, contact `FUN_001f40a0`; B2
`FUN_002263c0` is its setter): `LifeSpan` default FLT_MAX, `Rebound` 0.3, `Friction` 0.5; gravity -9.8 x `Gravity`;
soldiers and vehicles are passed through; a contact slower than 1 m/s along the normal lands it (aligned to the
surface), a faster one bounces. The powerup DISPENSER is the same `dispenser` weapon as the mines' (per-unit `MaxItems`
4, drop velocity (0.3..MaxStrength) x 10 m/s + the owner's) — card MINE builds the weapon; the powerup item's own
pickup rule is this card's.

## Built and landed (2026-10-03, afb7926 + 6bc4290, docs 951e3e4; report `reports/20261003_0300_card_PWUP.md`)
All four steps: the class tables and item classes (`classdb.rs` `drop_table`, `roster.rs`), the sim
(`core/src/sim/pickups.rs`: drop, item, pickup, BF2 energy and buffs), the draw (`game/src/viewer/pickups.rs`, not
`pickup_vis.rs`: the upload helpers live in the viewer), the sounds and the gain effects. Acceptance run: table census,
11 sim tests (mutation-checked), 8-seed battle census (every table within 0.03), port frames and the SOUND_LOG line on
NAB1 and UTA1. NOT run: the PCSX2 frame beside the port's, and the pickup rate is 2-4x the PS2's on three maps — card
PWPX. Not ported: `analysis/re-powerups.md` 9b's last bullet.
