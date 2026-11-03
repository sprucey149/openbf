---
id: MINE
title: Mines, time bombs and detpacks: the dispenser weapon (per-unit MaxItems), the placed mine entity (lands, arms, fires at an enemy's closest approach inside TriggerRadius; BF2 sticks and fires on contact), the time bomb (LifeSpan) and BF2's detonator (the next press sets the pack's trigger flag)
status: archived
issue: 43
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #43](https://github.com/sprucey149/openbf/issues/43).

## Why
`analysis/re-grenades.md` §5 (`[O]`, f0da5ad). The rocketeers' mines (both games), the engineers' detpacks and the
Bothan's / Wookiee's time bombs are class equipment the PS2's class-select screen lists; they were missing at this card's original baseline. All of it is
one weapon class (`dispenser`, + BF2's `detonator`) and one entity class (`mine`).

## Original build scope
1. **The dispenser weapon** (§5.1): the grenade's cycle (GREN); at `InitialSalvoDelay` it drops the entity named by
   `OrdnanceName` at the fire point with velocity = aim x (MinStrength + (MaxStrength - MinStrength) x strength) x
   Velocity (default 10) + the owner's velocity; per-unit `MaxItems` (the oldest is destroyed).
2. **The item physics** (§5.2): gravity -9.8 x `Gravity`; soldiers and vehicles are passed through (BF1); a contact
   slower than 1 m/s along the normal LANDS it (aligned to the surface, still); a faster one bounces with the class's
   `Rebound` / `Friction` in the grenade's formula. BF2: passes through its owner for 0.5 s, sticks to the world and to
   vehicles / buildings, bounces off people.
3. **The mine** (§5.2): shootable (`MaxHealth`); each tick — life out -> `ExplosionExpire`; dead -> the destroyed
   explosion (0xa2723442); landed and an ENEMY body within `TriggerRadius` + its radius with dot(its velocity, centre -
   mine) >= 0 -> `ExplosionTrigger`, the owner credited; a friend inside the second radius suppresses it (0 on the
   landmine). BF2's landmine also fires on an enemy's touch (key 0xbfb22e35) and lives 60 s.
4. **Time bomb**: a mine with `LifeSpan` (BF1 10 s, BF2 5 s) and no trigger radius. **Detpack** (BF2 `detonator`): one
   pack out (`MaxItems` 1, a 0.5 s charged throw at 2-10 m/s); with a pack out the next fresh press sets its trigger
   flag instead of throwing.
Bots do not lay mines in this card (the AI's use of a dispenser was not RE'd).

## Acceptance (run and paste into the report)
- Unit tests: (a) a mine dropped by a standing unit lands within 2 m ahead and is armed from the tick it lands; (b) an
  enemy walking a straight line that passes 1.2 m from it sets it off on the first tick where its velocity no longer
  points at the mine, a friend on the same line does not; a standing enemy placed on it sets it off at once; (c) the
  5th mine of a unit removes its 1st (`MaxItems` 4); (d) BF2 detpack — place, second press: the trigger explosion
  (Damage 3000, radii 0.5 / 3.5) on the next tick; shot dead: no damage; (e) the time bomb explodes at 10.0 s (BF1) /
  5.0 s (BF2) +- one tick; (f) the BF1 landmine's blast kills a soldier at 4 m (Damage 1750, inner radius 4.5).
- Census: every dispenser / detonator weapon of the fielded classes resolves its entity class and its three explosion
  classes from the baked data (paste the count per game; the disc table is re-grenades §9).
- Gates: `sim` declared; sweep / arena unmoved (bots do not use it).
## Note from PWUP (2026-10-02, analysis/re-powerups.md §6, §7.1)
What the pickup RE learned about the THROWN pickups (this card's dispenser weapon):
- The BF1 pilots' `*_weap_inf_powerup_dispenser` places `com_item_powerup_dual` (`MaxItems` 4, strength 0.3-0.4 x 10
  m/s). Live (NAB1): a CIS pilot droid stopped, the dual left it at 4.0 m/s forward, flew under -9.8 m/s2 and was
  taken in flight right after its first 0.5 s (an item in flight cannot be taken younger than that).
- The AI that uses it is BF1's supply behaviour `FUN_00256c78` (B2 `FUN_002d72d8`): an ally "needs" health or ammo
  only when no item giving it already lies within 15 m of that ally (`FUN_00256b30` / `FUN_002d71a0`).
- An ammo pickup never refills a weapon that itself dispenses pickups (the give-ammo skip rule, §5.4).
- Thrown items share the PowerupItem pool with the drops (25; BF1 scripts 25-60): a full pool also stops a drop.
- The item entity (physics, life, pickup) is PWUP's build; this card adds the weapon and the supply AI.

- GREN landed 2026-10-02 (d4886ba): the slot and channel machinery is in (`Unit::held`, `Sim::held_weapons`,
  `game::roster::held_slots`). A channel-1 slot is built only when it is a `grenade` weapon with a `sticky` ordnance
  (`thrown_weapon_from_db`); a dispenser / detonator slot is skipped there — that filter is where this card's weapons
  come in, and the HUD's second row and the R1 switch already follow whatever channel 1 holds.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Implementation status (2026-10-29)

See [the implementation and comparison-status report](https://github.com/sprucey149/openbf/blob/main/docs/loop/reports/20261028_MINE.md). All 53 carrying classes retain their explosive equipment in the source and disc census. The report supersedes the old missing-input blocker, but explicitly leaves a fresh live PS2 mine/detpack comparison outstanding. Source presence and executable/data checks do not close that acceptance requirement.
