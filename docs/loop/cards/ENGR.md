---
id: ENGR
title: Engineer and pilot bots serve: repair vehicles, turrets and buildings and give health / ammo to the HUMAN within 75 m (never to an AI), lay mines at Mine hints, and the three one-line special-weapon rules (cloak with no threat, buff with 3 friends in range, auto-turret against a cluster) - the port's bots do none
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
On the PS2 a friendly engineer or pilot bot walks up to the player and drops health / ammo, repairs the vehicle or
turret he sits in, and mine-layers seed their own posts. The port's bots carry these weapons and never use them: a
player-visible gap that is plain PS2 behaviour (card SAIR, `analysis/re-soldier-ai.md` 4.8, table row "engineers,
pilots"). Held behind BOTD (it decides where a "serve" behaviour sits in the layers) and GRAI (weapon selection).

## Evidence (all [V] in re-soldier-ai 4.8 / AGENT_H Q3-Q4, `engine/artifacts/lead_tmp/sair/AGENT_H.md`)
- Serve (B2 `FUN_002d6828` search, `FUN_002d76a8` state): `UnitType` repair / support / special / pilot only; every
  5 s; friendly or neutral entities within 75 m that lack health or ammo the bot's WeaponRepair or powerup dispenser
  gives for their type, with no powerup already within 15 m; a HUMAN soldier within 30 m who stands or faces the bot
  is taken at once; otherwise the nearest non-soldier slower than 0.5 m/s; **never an AI soldier, never itself**.
  Walk into weapon range (or 5 m), hold the repair tool / drop one item; give up after 20 s out of position or 8 s
  with no health gain. BF1's twin is UnitRepairAgent (75 m, 30 m, 5 s; off under 26 tickets) - its think is not read.
- Mines (B2 `FUN_002d4810` / `FUN_002d5120`): a unit with a mine dispenser, every 5 s, up to 3 Mine hints (TYPE "5")
  within 100 m at posts its team can spawn at, no enemy mine within 6.1 m and no own mine within 2.25 m: walk, lay,
  3 s, next hint.
- Three rules in the think (B2 `FUN_002b8160` / `FUN_002b8258` / `FUN_002b8400`): cloak (WeaponInvisibility) when
  there is no threat; a friendly-target emitter (buff) when threatened with 3 friendly units, itself included, in
  its range; an auto-turret dispenser when the threat is a soldier in a cluster (over 3 of his side, none of his
  enemies, within 9 m).

## Acceptance
- A headless scene test per rule: (1) a human at half health standing still within 30 m of a friendly engineer bot
  gains health within 15 s and the bot's dispenser count drops; (2) a damaged, parked friendly hover within 75 m is
  repaired to full by a pilot bot; (3) no bot ever serves an AI soldier (census over a sweep: 0); (4) a mine-layer
  on a map with Mine hints lays at least one mine at an own post in a 300 s round. Each test mutation-checked.
- Sweep: rounds finished unchanged; kills / min within the seed noise (8 seeds).

## Notes
- Depends on MINE (the mine entity) for rule (4) and on the powerup dispenser being a held weapon (PWUP landed).
- The PS2's "never an AI" is a fact, not a wish: BOTD may widen it (the maintainer wants more team sense) as a named deviation.

## BOTD (2026-10-03) - adopted as the Serve behaviour (bot-design.md 4.4); after GRAI and TEAM
Not in BOTV's deps (value 3, not an alpha card): PERF does not wait for it.

## TBLD's RE (2026-10-03, `analysis/re-build-zones.md`) - the building half moved out
- Serving BUILDINGS (unbuilt pads, destroyed and damaged turrets) is card BLDR now (value 4, M2, after TBLD and
  FCUT); this card keeps vehicles, the human, dispensers, mines and the three special-weapon rules. BLDR builds
  `core/src/ai/serve.rs`; add target kinds there.
- The cutter itself (weapon class `repair`, amounts per target type, one shot per ShotDelay) is card FCUT: a vehicle
  gains 200 a shot in BF2 and 60 in BF1 (30 for four BF1 sides at 0.25 s).
- Two BF2 quirks measured live, not to copy (BLDR's named deviations): a dead zone under 1 m from the target's centre
  (the bot stands 20 s), and the AI's weapon target has no range test (a held trigger repairs from up to 59 m).
- [L, not yet read in the code] BF1 Naboo: built gun turrets with regeneration 1.5 HP a second (1,390 samples) -
  probably a pilot seated in the turret repairing it. One to read when this card is taken.
- BF2 GEO1 has one `com_item_healthrecharge_unbuilt` (a buildable health droid, hologram `com_holo_health`): the same
  build rule would apply; not on a showcase map.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
