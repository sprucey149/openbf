# OPUS RE (2026-10-02) - [O] UNAUDITED — Pickups: what a dead unit drops, the item on the ground, who takes it and what it gives (card PWUP)

Both games. Sources: the two decomps, the ELFs, the discs (`COMMON.LVL`, `INGAME.LVL`, every side LVL, the mission
scripts), the RAM of seven PCSX2 save states, and four live PS2 battles sampled 20 times a second (BF1 Naboo Plains
and Hoth, BF2 Mygeeto and Utapau; 1,532 game seconds, 638 deaths, 444 drops, 42 pickups with an identified taker).
Tags: [V] read in the code (lines cited), [M] measured on the PS2 (RAM or live), [D] derived, [H] hypothesis,
[O] load-bearing and unaudited. Line numbers are `analysis/SWBFRONT.ELF-decomp.c` (B1) and
`analysis/SWBFRNT2.ELF-decomp.c` (B2). Evidence, tools and logs: `engine/artifacts/lead_tmp/pwup/`.
The item's flight (gravity, the bounce, the landing) is `analysis/re-grenades.md` §5.2 and is not repeated; the
dispenser weapon that THROWS pickups is card MINE's (§7.1 here has what this pass learned about it).

## 0. What the PS2 does, in one page [O]

1. **Who drops.** Only a soldier-class unit: its class holds a table of up to 8 (item class, probability) pairs,
   written by `DropItemClass` / `DropItemProbability` / `NextDropItem` [V]. The droideka (class `walkerdroid`)
   authors the same keys and they are never read [V, absent]. Ewoks, Gungans and Tuskens author none.
2. **The roll.** One uniform number per death; the table is walked subtracting each probability, the first entry
   that takes it below zero is the drop, none if the table runs out [V]. One item at most. No drop when the
   PowerupItem pool is full (25 items; BF1 scripts set 25 to 60 per map) [V].
   BF1: rank and file of the Alliance, Republic and CIS drop on 80 % of deaths (ammo 0.40, health100 0.30, dual
   0.10); the Empire, the Alliance's basic marksman and its snow vanguard on 25 % (ammo 0.05, health25 0.10,
   health100 0.10). BF2: every regular class 70 % (ammo 0.25, health25 0.15, offense 0.02, defense 0.02, health100
   0.08, dual 0.08, energy 0.10); heroes 70 % with their own six entries [V, M]. Live: 0.84 / 0.91 against 0.80,
   0.23 against 0.25, 0.74 / 0.73 against 0.70 (§8).
3. **When.** In the soldier's kill function, which its update calls on the first update after its health reaches
   zero — but not while it is thrown, tumbling or getting up: then the kill, and the drop, wait until that ends
   (35 of 444 live drops came more than 1.5 s after the death, the latest 7.6 s) [V, M]. Riders killed with their
   vehicle drop too [M].
4. **Where.** At the centre of the dead soldier's bounding sphere, 1.0 m above its feet, upright, facing its
   heading [V, M]. BF1: with the soldier's velocity at that moment (so a body knocked back at 5 m/s sends its drop
   the same way). BF2: with the soldier's velocity when that is faster than 3 m/s, otherwise 3 m/s toward the
   soldier's LEFT (its matrix row 0) [V, M]. It falls (-9.8 m/s2), passes through soldiers and vehicles and lands
   within about 0.5 s [M].
5. **How long.** `LifeSpan` 30 s, and the entity is removed 1 s after that: 31.0 s on the field [V, M]. BF2 fades it
   out over that last second. BF1 stops DRAWING it when 1 s of life is left: invisible for its last 2 s, still
   there [V].
6. **Who takes it.** Any living soldier or droid of EITHER team that touches it, friend or foe of the unit that
   dropped it — there is no team test [V]. It is a collision contact between the taker's body and the item's box
   (the model's bounding box, 0.19-0.49 m across, 0.48-0.60 m tall); the taker is not blocked by it [V, M]. Live the
   item is gone when the taker's feet are 0.3-1.0 m from it (median 0.7) [M]. It can be taken at once if it has
   landed, and in flight only once it is 0.5 s old [V, M]. Vehicles take nothing [V].
7. **What it gives**, and it is taken ONLY if it gives something [V]:
   - health: `SoldierHealth` points, up to the maximum — BF1 health25 +50, health100 +75, dual +100; BF2 health25
     +100, health100 +200, dual +200 [V, M]. A unit at full health leaves a health item on the ground.
   - ammo: `SoldierAmmo` clips added to the reserve of EVERY weapon the unit carries, each up to its own cap —
     ammo +1, dual +2 [V, M]. A weapon that dispenses pickups is skipped.
   - BF2 energy +150 (sprint / jet energy, up to the bar's maximum) [V].
   - BF2 offense: damage dealt x1.5 for 12 s; defense: damage taken x0.5 for 20 s [V; the timers M].
   - A BF2 hero takes no health from an item; a buff item it touches is used up and gives it nothing [V].
8. **Feedback.** The item's `PowerupSound` at the item, and an effect on the taker: `HealthGainEffect` (default
   `Powerup_blue` / `com_sfx_powerup_blue`) when its health rises, `AmmoGainEffect` (`Powerup_red`) on ammo,
   BF2's `EnergyGainEffect` (`com_sfx_powerup_green`) on energy [V]. The item draws the `player_shadow` blob
   under it [V].
9. **Bots.** BF1's bots do not look for pickups: they take one only by walking over it (of 233 items that ran
   their course in two battles, 11 were taken) [V, absent; M]. BF2's bots, when a human player is near (AI level
   of detail 3 or more), look every 2 s for an item within 6 m that would give them health or ammo and walk to it
   [V].

## 1. Method

Hops: the entity label tables (which constructor a `ClassLabel` builds) -> the PowerupItem class constructor, its
property setter and its vtables -> the instance update and contact functions -> the give-health / give-ammo helpers
-> the soldier class setter (the drop table) -> the soldier's kill function and its callers -> the AI's references
to the item type -> the discs (every `powerupitem` class; every soldier class's table resolved the PS2's way) ->
RAM of save states (class objects, the pool, live items) -> four live battles. The twin in BF2 was found from the
same label and the same property hashes, then compared function by function.

Class label -> class constructor [V]: B1 `FUN_00172ab0` (15507): `powerupitem` (string 0x5272b8, 15634) ->
`FUN_002a8798`, size 0x1540; `powerupstation` (0x5272a8) -> `FUN_002aa5d8`. B2 `FUN_00173d10` (16308):
`powerupitem` (0x60fcc0, 16454) -> `FUN_003665c0`, size 0x450; `powerupstation` -> `FUN_00368898`.

| role | BF1 | BF2 |
|---|---|---|
| item class ctor (base item class) | `FUN_002a8798` 234878 (`FUN_001f4408` 104222) | `FUN_003665c0` 361759 (`FUN_00226150` 139698) |
| item class setter (base) | `FUN_002a8910` 234932 (`FUN_001f4608` 104289) | `FUN_003667b8` 361831 (`FUN_002263c0` 139773) |
| instance update (base) | `FUN_002a84f0` 234778 (`FUN_001f3aa0` 103771) | `FUN_00366258` 361649 (`FUN_00225578` 139084) |
| instance contact (base) | `FUN_002a85e0` 234815 (`FUN_001f40a0` 104017) | `FUN_00366348` 361686 (`FUN_00225a30` 139298) |
| draw (base) | `FUN_001f3cb0` 103858 | `FUN_00225860` 139208 |
| give health / ammo | `FUN_002ab428` 237073 / `FUN_002ab4d8` 237117 | `FUN_003696e8` 364167 / `FUN_003697d0` 364216 |
| give energy / buffs | - | `FUN_00369a98` 364332 / `FUN_00369c38` 364394 |
| soldier class setter | `FUN_0020f2e0` 122812 | `FUN_00252bb0` 170689 |
| soldier kill | `FUN_001ffec0` 112062 | `FUN_002379f0` 151519 |
| soldier update (calls the kill) | `FUN_00205cc8` 116426 | `FUN_00240bf8` 158414 |

Instance vtables (ELF words): B1 item 0x53e850, its collision body's 0x53e7b8 (slot +0x1c = 0x2a8f00, the thunk to
the contact function), its update interface's 0x53e768 (slot +0xc = 0x2a8cd8 -> the update), class vtable 0x53e720.
B2 item 0x62c8d0, class vtable 0x62c740. Soldier: B1 damage interface 0x532d20 (slot +0xc = 0x211200 -> the kill;
slot +0x24 = 0x211270 -> the health-changed handler), class vtable 0x532b20; B2 instance 0x61eb88, class 0x61e8d8.

## 2. The item classes

### 2.1 The discs and RAM [V, M]

BASE `powerupitem`; BF1 in `COMMON.LVL`, BF2 in `INGAME.LVL`. Every class: `LifeSpan` 30, `Gravity` 1.0, `Rebound`
0.0, `Friction` 1.0, `Velocity` 0.0 (never read, §2.2), `PowerupSound`. The RAM of a battle holds exactly these
numbers in the class objects (`tools/ram_items.py`, seven states).

| class | BF1 | BF2 |
|---|---|---|
| `com_item_powerup_ammo` | `SoldierAmmo` 1 | `SoldierAmmo` 1 |
| `com_item_powerup_health25` | `SoldierHealth` 50 | `SoldierHealth` 100 |
| `com_item_powerup_health100` | `SoldierHealth` 75 | `SoldierHealth` 200 |
| `com_item_powerup_dual` | `SoldierHealth` 100, `SoldierAmmo` 2 | `SoldierHealth` 200, `SoldierAmmo` 2 |
| `com_item_powerup_energy` | - | `SoldierEnergy` 150 |
| `com_item_powerup_offense` | - | `BuffOffenseTimer` 12, `BuffOffenseMult` 1.5 |
| `com_item_powerup_defense` | - | `BuffDefenseTimer` 20, `BuffDefenseMult` 0.5 |

`PowerupSound`: `com_weap_ammo_pickup` (ammo), `com_weap_powerup_pickup` (the others). BF1 adds
`CollisionOtherSound` `com_weap_mine_land` (played on a bounce or landing contact).
The names are what the items are called, not what they give: a BF1 "health100" restores 75 of a trooper's 300.

### 2.2 The setter [V]

The class holds one amount per HEALTH TYPE of the taker — the low 3 bits of the taker's entity flags, set by its
class's `HealthType`: 0 person, 1 animal, 2 droid, 3 vehicle, 4 building (BF1's arrays hold 7 entries, BF2's 8).

| key (hash) | BF1 `FUN_002a8910` | BF2 `FUN_003667b8` |
|---|---|---|
| `PersonHealth` f51ad206, `AnimalHealth` 60d746d7, `DroidHealth` 9622c73b, `VehicleHealth` 14e381c9, `BuildingHealth` a6c59e9f | class+0x14f0 + 4 x type (235016, 234947, 234974, 234956, 234988) | class+0x3c0 + 4 x type (361981, 361847, 361842, 361852, 361946) |
| `PersonAmmo` 738f412e, `AnimalAmmo` bfe8e397, `DroidAmmo` 0fda4dbb, `VehicleAmmo` e728d955, `BuildingAmmo` a25f865f | class+0x150c + 4 x type (234962, 234998, 234952, 235011, 234942) | class+0x3e0 + 4 x type (361886, 361919, 361863, 361958, 361935) |
| **`SoldierHealth` f33b41df** | person AND droid (235004-235007) | person AND droid (361986) |
| **`SoldierAmmo` c3da529f** | person AND droid (234981-234984) | person AND droid (361968) |
| `SoldierEnergy` 0adc5145; `PersonEnergy` 84622134, `AnimalEnergy` fb0507ad, `DroidEnergy` 3fe43479, `VehicleEnergy` 56238a9b, `BuildingEnergy` da2c3905 | - | class+0x400 + 4 x type; Soldier = person and droid (361857, 361911, 361993, 361874, 361879, 361975) |
| `BuffOffenseTimer` 70b204af / `BuffOffenseMult` ce532e64 | - | class+0x420 / +0x424 (361895, 361963) |
| `BuffDefenseTimer` 7705fcc1 / `BuffDefenseMult` b25fb62e | - | class+0x428 / +0x42c (361906, 361941) |
| `BuffHealthTimer` 460b143b / `BuffHealthRate` b6639618 | - | class+0x430 / +0x434 (361869, 361951); no disc item authors them |
| `DebuffDamageTimer` 805b51e7 / `DebuffDamageRate` 9834a7b4 | - | class+0x438 / +0x43c (361901, 361931): a timed damage (§5.5); no disc item authors them |
| `PowerupSound` a2a034ea / `AmbientSound` 65a920ba | sounds class+0x1528 / +0x1530 (234993, 234967) | class+0x440 / +0x448 (361924, 361891) |

Everything else falls to the base item setter: `LifeSpan` (7c7c544b) -> B1 class+0x14b0 / B2 +0x380, default
FLT_MAX; `Gravity` (d209ff45) -> +0x14cc / +0x3a4; `Rebound` (678de338) -> +0x14d0 / +0x3a8, default 0.3;
`Friction` (a51be2bb) -> +0x14d4 / +0x3ac, default 0.5; `CollisionSound` / `CollisionOtherSound` /
`CollisionWaterSound`; `LightColor` / `LightRadius` (104222-104235, 104289-104453; 139698-139720, 139773-).
`Velocity` (32741c32) is compared in neither item setter nor in the entity base: the only readers are ordnance,
the dispenser weapon and particle code (`check_absent.txt`), so the items' `Velocity = 0.0` does nothing [V, absent].
RAM (BF1 NAB1 / YAV2 / TAT1 / HOT1, BF2 UTA1 / MYG1 / HOT1): ammo `[1, 0, 1, 0, 0]`, BF1 dual health
`[100, 0, 100, 0, 0]`, BF2 offense buffs `[12, 1.5, 0, 0, ...]`, defense `[0, 0, 20, 0.5, ...]` — person and
droid filled, animal / vehicle / building zero [M].

### 2.3 The body [V, M]

When a `GeometryName` is set and the class has no collision of its own, the base setter builds ONE box from the
model's bounding box: half extents (max - min) / 2, at the box centre (B1 `FUN_001f4608` 104362-104412, shape
`FUN_001696a8`; B2 `FUN_002263c0` 139879-139903). RAM (class -> body, and the model's own min / max;
`tools/ram_prims.py`):

| model | half extents x, y, z (m) | centre | classes |
|---|---|---|---|
| `item_powerup_ammo` | 0.1255, 0.2390, 0.1202 | (0.013, 0.239, 0) | ammo, both games |
| `com_item_powerup_dual` | 0.1565, 0.2504, 0.2451 | (-0.058, 0.240, 0.022) | dual, both games |
| `item_powerup_health100` (BF1) | 0.1206, 0.2976, 0.1206 | (0, 0.296, 0) | BF1 health100 |
| `item_powerup_health25` and BF2's other five | 0.0965, 0.2381, 0.0965 | (0, 0.237, 0) | BF1 health25; BF2 health25, health100, energy, offense, defense |

The item's own health is FLT_MAX / FLT_MAX: nothing shoots it away [M].

### 2.4 The pool [V, M]

Items live in the `PowerupItem` pool (B1 0x5899f0, element 0x300; B2 0x69bcc8, element 0x2d0). Default capacity
25 in both games (B1 `FUN_0033e590` 327907-327908; B2 `FUN_004241b0` 481806). BF1 mission scripts raise it with
`SetMemoryPoolSize("PowerupItem", n)`: bes1 40, geo1 25, hot1 60, kas1 40, kas2 30, nab1 40, rhn1 60, tat1 60,
yav2 40; the other BF1 maps keep 25. No BF2 mission script names the pool (only `test1c`), so BF2 is 25
everywhere. RAM: capacity 40 (NAB1, YAV2), 60 (TAT1, HOT1), 25 (BF2 UTA1, MYG1, HOT1) [M]. The pool is shared
with thrown pickups (§7.1). Live occupancy peaked at 21 of 40 (NAB1), 9 of 60 (HOT1), 16 and 21 of 25 (MYG1,
UTA1): with 16 bots a side the BF2 cap is close but was not hit.

## 3. The drop

### 3.1 The table [V, M]

Soldier class fields: count B1 class+0x1e10 / B2 +0x11d4; 8 class pointers +0x1e14 / +0x11d8; 8 probabilities
+0x1e34 / +0x11f8. A global write index (B1 `DAT_005178c8`, B2 `DAT_005fd0cc`):
- `DropItemClass` (406d2b75): looks the class up by name hash and stores it at slot[index]; count = max(count,
  index + 1) (B1 123060-123074; B2 171927-171942).
- `DropItemProbability` (26996dae): prob[index]; the same count rule (B1 122960-122966; B2 171669-171675).
- `NextDropItem` (5ea04066): index += 1, whatever its value (B1 123148-123149; B2 171756-171757).
- The index is reset to 0 only by the class COPY constructor, which also copies the parent's count, slots and
  probabilities (B1 `FUN_0020d940` 122092, 122126-122128; B2 `FUN_00250dc8` 169802, 169939-169941). Every disc
  class is built through it — it is slot +0xc of the soldier class vtable (B1 word 0x532b2c -> 0x2109a0; B2
  0x61e8e4 -> 0x255510), the slot the class loader calls on the BASE class (the verifier's read of the loaders
  `FUN_001bef00` / `FUN_001e28a8`) — so each class starts at index 0 on a copy of its parent's table: a BF2 child
  overrides its parent's entries by position and keeps the rest. Writers of the index: those two per game
  (`check_absent.txt`, check 5; the verifier's ELF store scan agrees).
- The three hashes occur once each in each ELF, in the soldier class setter (raw scan, both halves); the setter is
  referenced only as slot +0x20 of the soldier class vtable (B1 word 0x532b40, B2 0x61e8f8), never called. No
  other class reads the keys [V, absent]. `cis_inf_droideka` is BASE `walkerdroid` (B1 class ctor `FUN_001d31d8`,
  whose base is the plain entity class `FUN_001e30c8`): its `DropItemClass` lines are dead data, as are the probe
  droid's (`imp_droid_probe`, BASE `droid`), in both games.

Resolved tables (`tools/drops.py`: parent first, the index rule above; identical to the tables read from RAM in
seven states, `tools/ram_items.py`):

| BF1 | table | classes |
|---|---|---|
| A, total 0.80 | ammo 0.40, health100 0.30, dual 0.10 | 21: every ALL, REP and CIS team class except those below |
| B, total 0.25 | ammo 0.05, health25 0.10, health100 0.10 | 13: the 11 IMP classes, `all_inf_marksman`, `all_inf_vanguardsnow` |
| C, total 0.25 | ammo 0.20, health25 0.05, health100 0.0 | 9: the 5 heroes, the 3 Wookiees, `gar_inf_vanguard` |
| D, total 0.15 | ammo 0.07, health25 0.08, health100 0.0 | 5: `cis_inf_battledroid_av / _inf / _pilot / _sniper`, `gar_inf_soldier` |
| other | Jawa 0.5 / 0.4 / 0.1 (always drops); Geonosian 0.05 / 0 / 0.20; `gar_inf_pilot` 0.15 / 0 / 0.10 | |
| none | - | Ewoks, Gungans, Tuskens |

| BF2 | table | classes |
|---|---|---|
| regular, total 0.70 | ammo 0.25, health25 0.15, offense 0.02, defense 0.02, health100 0.08, dual 0.08, energy 0.10 | 121: everything under `com_inf_default` (Gungans and Tuskens too) |
| hero, total 0.70 | ammo 0.05, health100 0.05, dual 0.20, energy 0.10, offense 0.15, defense 0.15 | 33: `com_jedi_default`, `com_hero_default` and their children (the wampa too) |
| other | Geonosian 0.15 / 0.15 / 0.08 / 0.08 / 0.10 / 0.02 / 0.02 (total 0.60); Jawa ammo 0.25, health25 0.05, health100 0.80 (the walk ends there: 0.70 effective); Gamorrean 0.20 / 0.05 / 0; the acklay all zeros | |
| none | - | Ewoks |

The ORDER matters only for which item a given random number picks; the per-item chances are the table's.

### 3.2 The roll [V]

B1 `FUN_001ffec0`, B2 `FUN_002379f0`, the same code:
```
flags &= ~8                                   # not alive (B1 112138; B2 151594)
BF2 only: if flags & 0x10: no drop            # removed without a death, §3.5 (151611)
if class.drop_count > 0:                      # B1 112150; B2 151611
    u    = seed * 0x19660d + 0x3c6ef35f
    seed = u    * 0x19660d + 0x3c6ef35f       # the game's shared LCG: B1 DAT_0066eed0, B2 DAT_0073d870
    r    = float(((u >> 16) | (seed & 0xffff0000)) >> 9 | 0x3f800000) - 1.0     # [0, 1)   (112152-112155; 151613-151616)
    for i in 0 .. drop_count-1:
        r -= prob[i]                          # 112159; 151620
        if r < 0:
            if pool.count < pool.capacity:    # 112161; 151622
                spawn slot[i] (§3.4); stop
            # pool full: the walk goes on, r stays negative, nothing is ever spawned
```
One roll, at most one item; the roll is spent whether or not the pool has room. A network client does not roll
(the flag tests at 112139-112149; single player takes the branch — every drop of §8 came through it).
The soldier update spends one more roll (two steps of the same generator) just before it calls the kill: the
death-chunk chance against class+0x1e58 (B1 116903-116906) / class+0x121c (B2 159608-159612).
**BF2 only:** the item class's create function `FUN_00366d70` (362018-362040) makes nothing while
`FUN_003e4a30()` is non-zero — the byte `DAT_006d9330`, set by `FUN_003e4a40(1)` from `FUN_003d4920` and cleared
from three other callers (and written by a network read). 31 functions test it (captures, tickets, AI). [V] the
gate; [H] what the state is (a game-wide hold; it was clear all through the four battles of §8). BF1's create
function `FUN_002a8c20` has only the pool test.

### 3.3 When [V, M]

The kill function is slot +0xc of the soldier's damage interface (B1 vtable 0x532d20 -> thunk `FUN_00211200` 124217;
also slot +0xc of the interface at soldier+0x2a0, vtable 0x532be0 -> `FUN_00210c30`). Taking damage only clears the
alive bit (the health-changed handler ends in `FUN_001bcb10`, 64610: health <= 0 -> flags &= ~8). The KILL is called
from the soldier's update, on the first update that finds the bit clear — unless the soldier's state is one of:
- BF1 (`FUN_00205cc8` 116889-116902, call 116968): 3 (dead already), 5 (thrown), 8 (tumbling on the ground), 9
  (getting up);
- BF2 (`FUN_00240bf8` 159605-159607, call 159669): 9, 10, 11 (the same three) and 0xf, 0x10.
(State names: `analysis/re-stance-roll.md` §2.) So a unit killed by a blast that throws it drops its item where the
body ends its tumble. Live, the item's first sample came within 0.10 s of the alive bit clearing in 330 of 444
drops, within 0.5 s in 368, later than 1.5 s in 35 (BF1 max 7.61 s, BF2 2.20 s) [M].
The update also calls the kill when the soldier leaves the world (y < -500, |x| or |z| > 1000; 116591).
Soldiers killed inside a vehicle drop too: NAB1 showed 8 items appearing 1.0 m above the feet of soldiers that
were alive at full health in the sample before (for six, the soldier's slot was empty or reused in the next: no
corpse), two items at one spot twice (two riders) [M]; [D] that it is the vehicle's death.

### 3.4 Where and how fast [V, M]

- Position: the centre of the soldier's collision object's bounding sphere (`FUN_001645a8` on soldier+0x110 or the
  object's sphere list; B1 112181-112185). Live: first item sample minus that centre: |d| median 0.00 / 0.17 m
  (BF1 NAB1 / HOT1), 0.20 / 0.31 m (BF2, whose item already moves); the item starts 0.93-1.00 m above the feet
  (medians) [M].
- Orientation: up = world up, forward = the soldier's matrix row 2 (112182-112215): upright, facing its heading.
- Velocity, BF1: `item.SetVelocity(soldier.GetVelocity())` — item vtable 0x53e850 slot +0x24 returns the item, its
  slot +0x4c (0x1f5158) stores a vector at item+0x2c0; soldier vtable 0x532d50 slot +0x48 (0x211c08) returns
  soldier+0xa20 (112249-112258). Live: 173 of 215 fresh drops have the corpse's sampled velocity to within 0.3
  m/s (the rest: the corpse was sampled in another update); item speeds of exactly 5.00 m/s (the kill's
  knock-back) are common, and unmatched items left at 30-72 m/s [M].
- Velocity, BF2 (151711-151733; instructions 0x237d70-0x237e0c): `v = soldier.GetVelocity()` (vtable 0x61eb88 slot
  +0x4c -> soldier+0x550); if `v.v > 9.0` the item gets `v`; otherwise it gets `3.0 x row 0` of the soldier's world
  matrix (body vtable 0x61eb28 slot +0x48 returns soldier+0x120; item slot +0x50 = 0x226bf0 stores at item+0x290).
  Row 0 = up x forward: the soldier's LEFT (in three spawn states the player's row 0 . camera right = -1.000).
  Live: of 228 fresh BF2 drops 80 left at 3.00 m/s along +row 0, 138 with the corpse's velocity, 10 neither (the
  corpse sampled in another update); none along -row 0 [M].
- Then the base item: gravity -9.8 x `Gravity` m/s2 (live: vy falls 0.33 m/s per 1/30 s update), soldiers and
  vehicles passed through, a contact slower than 1 m/s along the normal lands it, a faster one bounces with
  `Rebound` 0 / `Friction` 1 (`re-grenades.md` §5.2). Live: landed 0.47-0.50 s after the drop (median), 90 % by
  0.57-0.80 s, at most 1.63 s [M].

### 3.5 Who never drops [V]

- Not a soldier class (droideka, probe droid, vehicles): §3.1.
- No table (Ewoks, Gungans, Tuskens in BF1; Ewoks in BF2). Live: 28 Gungan deaths, 0 drops [M].
- BF2 only: a soldier whose entity flag bit 4 is set at the kill (151611). The bit is set, with the alive bit
  cleared, at five code sites that remove a unit without a death (`AcceptHero` 0x18d0bc — the player's soldier
  when he becomes the hero — and 0x2b0dd8, 0x34d854, 0x3e4e70, 0x3ef118; `tools/bitset.py`). The same bit skips the
  death's score bookkeeping (151771). [V] the gate and the sites; [H] what the other four are.
- BF1 has no such test (the text of `FUN_001ffec0` between 112138 and 112161).

## 4. The item's life [V, M]

- Life starts at `LifeSpan` (30) and runs down by dt each update; the entity is removed when life <= -1.0 or when
  it is no longer alive (B1 103789-103791, `lui 0xbf80` at 0x1f3aa4; B2 139104-139106). Live: 385 items seen from
  spawn to expiry lasted 30.90-31.03 s; the last life value read was -0.997 / -1.000 [M].
- BF2 draws it while life > -1.0 with alpha = min(life + 1, 1): a 1 s fade at the end (139240-139241, `lui 0xbf80`
  at 0x225864). BF1 draws it only while life > +1.0 (`lui 0x3f80` at 0x1f3cb4, `c.le.s` / `bc1t` at 0x1f3cfc:
  skip when life <= 1.0) and the same alpha expression is then always 1: BF1's item disappears from view 29 s after
  the drop and can still be taken for 2 s more (one BF1 pickup at life -0.80, one BF2 at -0.17, live) [V, M].
- It is also removed below y -500 (B1 234799, BF2 361670; the base: B1 -500, BF2 -1000).
- The draw adds the `player_shadow` blob under it (B1 `FUN_001f3cb0`: a 5 m ray down, string 0x5318d8; B2
  `FUN_00225860`, string 0x61ce30). `AmbientSound` would loop at the item; no disc item authors it.

## 5. The pickup

### 5.1 The path [V]

The item's collision object is on layer 1 (RAM: the layer word of every live item's object is 1, a soldier's 2;
both games) [M]. The soldier's contact handler, on a contact with a layer-1 body, calls
that body's contact callback and applies no push (B1 `FUN_00202f38` 114417-114421); the generic pair test also
calls both bodies' callbacks (B1 `FUN_00168a60` 7412, 7422). The item's callback is `FUN_002a85e0` (B1) /
`FUN_00366348` (B2). The pair test takes the item's bodies for the taker's layer and the taker's bodies for layer 1
— `FUN_00168350` (6931) maps layer 1 to the model's default subset.

### 5.2 The rule [V]

B1 234835-234874 (instructions 0x2a85e0-0x2a8794), B2 361709-361754:
```
if item alive:                                            # flags bit 3
    if not landed and LifeSpan - life <= 0.5: -> base contact (bounce / land)    # 234836-234838; 361710-361711
    taker = the other body's entity                       # 234844; 361718
    if taker and taker alive:
        t = taker.flags & 7                               # its HealthType, §2.2
        a = give_health(class.health[t], taker)           # 234850; 361726
        b = give_ammo  (class.ammo[t],   taker, skip pickup dispensers)          # 234851; 361727
        BF2: c = give_energy(class.energy[t], taker); d = give_buffs(the 8 buff numbers, taker)   # 361728-361729
        if any of them is true:
            play PowerupSound at the item                 # 234860; 361741 (one exception, below)
            kill the item                                 # 234866; 361745-361747
            return
-> base contact
```
The sound is skipped only when the taker's vtable slot +0xd4 (B2 +0xe0) returns 1 (a soldier) AND B1 soldier+0x954
(B2: bit 20 of the word at entity+0x4e0) is set — a flag set where an animated prop seizes a soldier (B1
`FUN_001fd650` 110531, B2 `FUN_002312c0`) and by a network read; in ordinary play the sound always plays
(234853-234861; 361735-361741) [V; the flag's meaning D].
No team is read anywhere in it, and the item carries none from a drop. A taker that would gain nothing leaves the
item where it is. A vehicle's type is 3: every disc item gives 0 to it, and the base contact ignores it.

### 5.3 Health [V, M]

B1 `FUN_002ab428` (instructions 0x2ab428-0x2ab494): amount != 0 and (health < max or amount < 0) ->
health = min(health + amount, max), true. B2 `FUN_003696e8` (0x3696e8-0x36976c) adds one condition: the unit's
health regeneration rate (entity+0x178) must be >= 0. B2's wrapper `FUN_00369770` (0x369770-0x3697a8) first
refuses any class with bit 33 of class+0x330 set — the bit `SetHeroClass` puts on a team's hero class
(`SetHeroClass` 32701 -> `FUN_0036e800` -> `FUN_0036e770` 367470): **a BF2 hero takes no health from an item**.
Live [M]: BF1 80 -> 155 and 10.1 -> 85.1 (health100, +75), 50 -> 100 (health25, +50), 240 -> 300 (dual, capped);
BF2 156 -> 256, 200 -> 300, 100 -> 200 (health25, +100), 9.2 -> 209.2 (dual, +200), capped cases 485 -> 550,
268.9 -> 400.

### 5.4 Ammo [V, M]

B1 `FUN_002ab4d8` (0x2ab4d8-0x2ab69c), for every weapon of every weapon holder of the unit:
```
skip if the weapon's class is a dispenser whose placed class is a PowerupItem class      # 237148-237150
new = reserve + amount                       # reserve clips, weapon+0x90
cap = max_clips(weapon+0x94) - (1 if the clip in the gun holds a round else 0)           # 237156
reserve = 0 if new < 0 else min(new, cap)
true if any weapon's reserve changed by more than 1e-4                                   # 237167
```
A weapon with an unlimited reserve (FLT_MAX) is unchanged. BF2 `FUN_003697d0`: the same walk; a weapon wants ammo
when its reserve is below max - 1, or when it is a one-clip weapon whose clip is not full (then the clip is folded
into the reserve first, `FUN_00303928`); `FUN_003038c8` adds the clips with the same cap (364250-364283).
Live [M]: BF1 reserve +1 on slots 0 and 3 of a clone trooper; BF2 +1 on the slots below their cap, +2 from a dual.
After an ammo gain a soldier plays its `AmmoGainEffect` (B1 `FUN_001ffcc0` 111984, called from `FUN_002ab6a0` 237191).

### 5.5 BF2: energy and buffs [V; timers M]

- Energy `FUN_00369a98`: for each weapon holder of the unit with an energy bar: if energy < max, add
  min(max - energy, amount) (364354; `FUN_001e1fc0` clears the exhausted flag once past its threshold).
- Buffs `FUN_00369c38`: only a soldier (or the second unit class, RTTI ids `DAT_00689828` / `DAT_00689300`); true
  whenever one of the four timers is positive — a buff item is always taken (364422-364443). The setters
  (`FUN_0024eb18`, `FUN_0024eb70`, `FUN_0024ebc8`, `FUN_0024ec20`; 168000-168060): on a hero class (bit 33) or a
  class with bit 5 of class+0x778 the timer is ZEROED; otherwise the timer and its number are replaced only when
  the new timer is longer than what is left. So a hero that touches a buff item uses it up and gets nothing
  (`FUN_00369c38` has already answered true). Fields: offense timer / multiplier soldier+0x1020 / +0x1024, defense
  +0x1028 / +0x102c, health-regeneration +0x1030 / +0x1034, the timed damage (`DebuffDamage*`) +0x1038 / +0x103c.
- The tick `FUN_0024ece0` (168091-): timers of 300 s or less run down by dt; while the health timer runs, health +=
  rate x dt; while the fourth runs, the unit takes rate x dt damage from the stored attacker. Two award bonuses set
  permanent buffs through the same fields (340 s timers that do not run down: x1.5 offense, x0.5 defense;
  168133-168144).
- Use: `FUN_0024f048` (168229) returns the offense multiplier while its timer is non-zero, else 1.0 — the soldier's
  outgoing-damage factor (thunk `FUN_00255c48`); `FUN_0024ec88` (168073) multiplies the unit's incoming-damage
  factor by the defense multiplier while its timer is positive (thunk `FUN_00255d58`). [V] the getters; [D] that
  every weapon and every damage goes through them.
- Live [M]: five offense pickups set soldier+0x1020 to 12.000, then 11.967, 11.900 ... 0 after 12 s; one defense
  pickup +20.0.

### 5.6 Feedback [V]

- `PowerupSound` at the item (§5.2).
- On the taker, soldier class fields: `HealthGainEffect` (68c9c2d9; B1 class+0x18f8, default `Powerup_blue`,
  121993; B2 class+0x898, default hash 0x9d26c2f2 = `com_sfx_powerup_blue`, 169631), played by the health-changed
  handler when health ROSE (B1 `FUN_001ffda0` 112032-112045: any heal, a health droid's too); `AmmoGainEffect`
  (8d8743f9; B1 +0x18fc `Powerup_red`, 121996; B2 +0x89c `com_sfx_powerup_red`, 169632; B2 `FUN_002363d8`);
  BF2 `EnergyGainEffect` (80a20ae3; +0x8a0 `com_sfx_powerup_green`, 169645). One at a time per unit (a handle is
  kept). No disc class overrides the three keys.

### 5.7 The reach [M, D]

The taker's side is the soldier's collision spheres (`analysis/re-soldier-walls.md`: radius 0.57, centres 0.15 m
ahead of the feet); the item's side is §2.3's box. Geometry gives a head-on reach of about 0.57 + 0.15 + 0.10..0.25
= 0.8-1.0 m feet to item centre, and 0.5-0.6 m for an item beside the path [D].
Live, 42 pickups with an identified taker (`live/pw_reach.py`; feet to item, horizontal):

| | at the sample showing the gain | one sample earlier (not yet taken) |
|---|---|---|
| BF1 (10) | min 0.29, median 0.72, max 0.97 | median 0.95, max 1.38 |
| BF2 (32) | min 0.32, median 0.65, max 0.97 | median 0.98, max 1.29 |

A sample is 1 or 2 updates (0.2-0.4 m of walking), so the contact comes at 0.7-1.0 m head-on. Two takers stood
0.06 m and 0.36 m from a resting item for at least one sample WITHOUT taking it and took it when they moved: the
contact needs one of the two bodies to move [M; the cause H]. A landed item is taken at once (an energy item landed
0.62 m from a standing sniper and was taken 0.27 s after the drop); an item in flight was taken at age 0.53 s [M].

## 6. The AI

- **BF1: no item seek** [V, absent]. The PowerupItem type id (`DAT_00589a38` = hash of "PowerupItem", written by
  `FUN_002a8fc0` 235247) is read by the item's own type tests (`FUN_002a9168`, and a getter with no decomp header
  at 0x2a9188, the item vtable's slot +0xc) and by ONE other function, `FUN_00256b30` (174088-174097): it gathers
  up to 10 layer-1 bodies within 15 m of a unit and ORs, per health type, which kinds of item lie there. Its
  caller `FUN_00256c78` (174138-174142) is the SUPPLY behaviour of a unit carrying a health / ammo giver: an ally
  needs health only if no health item already lies
  within 15 m of it. It is also the only gather call in BF1 whose mask includes layer 1. Live: 11 of 233 items
  were taken (10 by a soldier walking over them, 1 thrown item in flight).
- **BF2: a detour** [V]. `FUN_002bd628` (243867; called from the AI think `FUN_002b7790` 239304 -> `FUN_002bc1a8`
  242797): returns unless the bot's level of detail (ai+0x560) is 3 or more, the unit is not in a vehicle
  (`FUN_002a6138`) and unit+0x4ac is 0 (243882-243891); then every 2.0 s `FUN_002be050(6.0)` (243893-243895), and
  4.0 s more after a hit. `FUN_002be050` (244352): up to 6 layer-1 bodies within 6 m; for a PowerupItem
  (`DAT_0069bd18`) whose class gives this unit's health type health it lacks (`FUN_003697b0`) or ammo it lacks
  (`FUN_00369a78`), and that `FUN_002ef6b8` passes (a path / line test [H]), it queues three points: here, 1 m
  past the item, here again (244388-244457). Energy and buffs are not looked for.
  The type id `DAT_0069bd18` is read by this function, by the supply mask `FUN_002d71a0` (261763-261767) and by
  the item's own type test only. (Two other BF2 gathers include layer 1 in their mask — `FUN_002b0548` 236419,
  mask 0x3b, and the blast gather `FUN_0030c8f8` 300525, mask 0x103b — and never ask for the item type.)
  The level of detail (`FUN_002b7b00` 239590-239659): 4 within 7 m of a human player, 2 beyond 20 m, 3 between,
  less 1 for each of two visibility tests failed [V the numbers; H the tests]. With the player on the spawn map
  (every live run here) no bot detours; UTA1's bots still took 27 items by walking over them.

## 7. Other sources of items (not this card)

### 7.1 Thrown pickups (card MINE's weapon) [M, V]
The BF1 pilots' `*_weap_inf_powerup_dispenser` places `com_item_powerup_dual`, `MaxItems` 4, strength 0.3-0.4 x 10
m/s. Live: a CIS pilot droid stopped, a dual appeared at it at 4.0 m/s forward, flew under -9.8 m/s2 and was
taken in flight right after its 0.5 s (no soldier was within 2 m: [H] a droideka, health type droid, which the
soldier sampler does not see). The supply AI is §6's `FUN_00256c78`.

### 7.2 BF1's team bonus [V]
`FUN_002a4650` (231816; an update reached through vtable word 0x53e584): while a team has bonus bit 2, every 10 s
it spawns, at each command post the team owns, one of `com_item_powerup_ammo` / `_health25` / `_health100` chosen
uniformly (231994-232032; its own generator `DAT_0066eec8`; pool room tested). Galactic Conquest's bonus; not
active in Instant Action.

### 7.3 Stations
`powerupstation` (the health and ammo droids) uses the same give functions (callers in `check_absent.txt`).
Its reach, rate, team rule, tips and animation: `analysis/re-powerup-station.md` (card PWST, 2026-10-20).

## 8. PCSX2 live (2026-10-02, software renderer, RAM read through /dev/shm, nothing written)

`live/pw_live.py <game> <side-select state>`: a full-AI battle, the player left on the spawn map, the whole EE RAM
read 20 times a second (every PowerupItem instance, every soldier), stamped with the EE cycle clock. Analysis:
`live/pw_analyse.py`, `pw_summary.py`, `pw_reach.py`, `pw_taken.py`; data `live/out/*.npz`.

| battle | game s | corpses (+ soldiers gone while alive) | drops matched to a corpse (+ unmatched fresh items) | pickups / expired |
|---|---|---|---|---|
| BF1 Naboo Plains, CW | 356 | 219 (+15) | 160 (+10) | 8 / 162 |
| BF1 Hoth | 439 | 108 (+21) | 55 (+9) | 3 / 60 |
| BF2 Mygeeto, CW | 335 | 120 (+10) | 89 (+3) | 6 / 83 |
| BF2 Utapau, CW | 402 (the round ended at 344 and a new one began) | 191 (+16) | 140 (+3) | 28 / 98 |

Pickups by a soldier per minute of battle, nobody near the player: NAB1 1.2 (7), HOT1 0.4 (3), MYG1 1.1 (6), UTA1
4.9 (28 in the first round's 344 s). On UTA1 the 13 items alive at the round's end were removed at once (life
-0.5 to 30.0 s), and 8 of the 140 drops belong to the second round.

By the victim's table (a "corpse" is a soldier whose alive bit was seen to clear; a soldier gone while alive is a
rider killed in a vehicle or a removal — they bracket the true ratio from both sides):

| battle | table (total) | corpses | drops | drops / corpses | drops / (corpses + gone) | items got | table x corpses |
|---|---|---|---|---|---|---|---|
| BF1 NAB1 | A (0.80) | 191 | 160 | 0.838 | 0.777 | ammo 82, health100 63, dual 15 | 76.4, 57.3, 19.1 |
| BF1 NAB1 | none (Gungans) | 28 | 0 | 0 | 0 | - | - |
| BF1 HOT1 | B (0.25): the Empire, the snow vanguard | 64 | 15 | 0.234 | 0.190 | ammo 5, health25 7, health100 3 | 3.2, 6.4, 6.4 |
| BF1 HOT1 | A (0.80): the Alliance | 44 | 40 | 0.909 | 0.800 | ammo 20, health100 19, dual 1 | 17.6, 13.2, 4.4 |
| BF2 MYG1 | regular (0.70) | 120 | 89 | 0.742 | 0.685 | ammo 31, health25 15, energy 15, dual 12, health100 11, offense 3, defense 2 | 30, 18, 12, 9.6, 9.6, 2.4, 2.4 |
| BF2 UTA1 | regular (0.70) | 191 | 140 | 0.733 | 0.676 | ammo 47, health25 36, energy 20, dual 15, health100 12, offense 7, defense 3 | 47.8, 28.6, 19.1, 15.3, 15.3, 3.8, 3.8 |

Every bracket holds its table's total. The Empire's low table against the Alliance's is plain on Hoth: 15 drops
from 64 Imperial deaths, 40 from 44 Rebel ones.

## 9. The port and the build at the research baseline

Current pickup code is `core/src/sim/pickups.rs` and `game/src/viewer/pickups.rs`.
The following inventory describes the 2026-10-02 baseline; it is not a claim that
those features remain unimplemented or that current original-game acceptance is complete.

At that research baseline, the port had no pickup: no class reads the drop keys, no item entity, nothing drawn (search of `engine/crates` for
`powerup`, `DropItem`, `pickup`: no runtime hit). The build (card PWUP, phase build), in commits:
1. **Data.** Per soldier class its resolved drop table (§3.1: parent first, the index rule); per item class its
   numbers (§2.1) and box (§2.3); per map the pool size (§2.4). A droideka gets none.
2. **Sim: the drop.** At a unit's death one draw from the sim's RNG, §3.2's walk, the pool cap. Spawn at the unit's
   centre (feet + 1.0 m), §3.4's velocity (BF1 the unit's; BF2 the unit's above 3 m/s, else 3 m/s to its left).
   Not for a unit removed without a death, and (BF2) not while the game is held (§3.2: e.g. once the round is over).
3. **Sim: the item.** Gravity -9.8, lands on the first slow contact (the grenade card's item physics where it
   exists; a ground ray otherwise), life 30 -> -1, removed.
4. **Sim: the pickup.** §5.2 for every living soldier-type unit of either team: landed or older than 0.5 s; health
   to the cap, +clips to every weapon to its cap; taken only when something was given; BF2 hero rule. Reach: the
   unit's body against the item's box, 0.7-1.0 m head-on (§5.7); testing every living unit each tick is the port's
   choice (the PS2 needs one of the two to move).
5. **Draw and sound.** The item models (COMMON.LVL / INGAME.LVL), upright at the item; BF1 hidden for the last 2 s,
   BF2 fading over the last second; the shadow blob; `PowerupSound`; the gain effects.
6. **BF2 extras** (own commits, or a follow-up): energy; the offense / defense timers and where they scale damage.
The bots' 6 m detour (§6, BF2) is its own card (PWAI).

Referee numbers for the build's tests: §3.1's tables (a 10,000-death census per table must land within 1 % of
each entry); the pool cap; life 31.0 s; health25 / health100 / dual amounts per game; +1 / +2 clips; a full-health
unit leaves a health item; an enemy takes what a unit dropped; §8's battle ratios.

### 9b. What the port does after PWUP (afb7926 + 6bc4290, 2026-10-03)

- **Data** — `game::classdb::ClassDb::drop_table` resolves a class's table the PS2's way (§3.1); `game::roster`
  builds each item class (`item_class_from_db`: §2.1's numbers, §2.3's boxes by model name, BF2's health100 on the
  small box) and the pool size (`item_pool`: §2.4's script values by map). Census of the baked four sides + the
  shared file (`game/tests/roster.rs` `drop_tables_are_the_discs`): BF1 A 21, B 13, heroes 5, battle droids 4;
  BF2 regular 107, hero 23 (the rest of §3.1's 121 / 33 / 9 / 5 are the locals' side LVLs, which are not baked).
- **Sim** — `core/src/sim/pickups.rs`: the roll at the death (one draw of `Sim::lcg`, spent with a full pool too;
  nothing for a class with no table, nor for the port's own bot recycles), the item (1.0 m over the feet, §3.4's
  velocity per game, -9.8 m/s2, lands on a floor ray, life 30 -> -1 s), the pickup (§5.2: landed or older than
  0.5 s, any living person or droid of either team not in a seat, health / clips / BF2 energy and buffs, only
  when it gave something) and the buffs' factor on bolt and blast damage (`buff_scale`).
- **Drawn and heard** — `game/src/viewer/pickups.rs` (the model, BF1 hidden for the last 2 s),
  `game/src/event_sounds.rs` (`PowerupSound`, BF1's landing sound), `game/src/fx.rs` (the gain effects).
- **Measured** (8 seeds, `lead_tmp/pwup/census/`): drops / deaths by table — NAB1 0.807 (0.80) and 0.249 (0.25),
  HOT1 0.805 and 0.274, MYG1 0.698 (0.70), UTA1 0.726; live items peak 20 / 12 / 21 / 23 (pools 40 / 60 / 25 /
  25). Pickups per minute 3.2 / 1.8 / 3.0 / 3.2 against the PS2's 1.2 / 0.4 / 1.1 / 4.9 (§8): the port's bots take
  more on three maps — open (the port tests the touch every tick for every unit, §5.7's moving-body rule is not
  built, and its bots walk other lines).
- **Not ported / PROVISIONAL**: the drop waits for no tumble (the port's death is immediate, §3.3); no wall
  contact in the item's flight; BF2's 1 s fade and the `player_shadow` blob; the hero rules (§5.3, §5.5 — the sim
  has no hero); the dispenser's skip (§5.4, card MINE); a weapon's reserve cap is the reserve it spawned with;
  BF2's create gate is taken as "the round is decided" (§3.2 [H]); the bots' 6 m detour (§6, card PWAI). No PCSX2
  frame of an item has been set beside the port's (card PWPX).

## 10. Checks run (output in `lead_tmp/pwup/`)

1. **fn** — `check_fn_b1.txt` (101 lines), `check_fn_b2.txt` (134 lines): every cited line is in the function
   named beside it. The first cite of a few functions in my notes was 2-5 lines off the header (a print with its
   leading blank lines dropped); the table of §1 uses `re_check.py`'s header lines.
2. **absent** — `check_absent.sh` -> `check_absent.txt`: the three drop hashes by raw ELF scan (one lui / ori pair
   each, in the soldier setter); the setters' and kill functions' references (jal + data words); the PowerupItem
   type ids (B1 `DAT_00589a38` 2 functions, `DAT_00589a40` 4; B2 `DAT_0069bd18` 3, `DAT_0069bd20` 4 — a symbol
   search: the verifier added a writer and a header-less getter it cannot see, §6); every gather call with its
   layer mask (mask exactly 1: B1 one call, B2 two; BF2 has two more whose mask includes layer 1); every
   pool-room test (the item creators: B1 the
   kill, the bonus, the create function; B2 the kill and the create function); `Velocity`'s readers; the scripts
   naming the pool (binary-safe grep; the same search finds `SetMemoryPoolSize` in 107 BF2 scripts).
3. **twin** — BF2's functions were found from the label `powerupitem` and from the same property hashes, not by
   pattern; each row of §1's table was read on both sides. Differences found and stated: the velocity rule (§3.4),
   the removed-without-death bit (§3.5), the regeneration and hero tests (§5.3), energy and buffs (§5.5), the fade
   (§4), the AI detour (§6).
4. **word** — `check_words.txt` (instruction listings): 0.5 (`lui 0x3f00` 0x2a863c, 0x3663bc), -1.0 / +1.0 (§4),
   9.0 and 3.0 (0x237d84, 0x237de4), 6.0 / 2.0 / 4.0 and the `slti 3` (0x2bd67c-0x2bd6f4), 15.0 (0x256b74), 1e-4
   (0x38d1b717 at 0x2ab558), the bit-33 test (0x369780-0x36978c); strings `Powerup_blue` 0x5329d8, `Powerup_red`
   0x5329e8, `player_shadow` 0x5318d8 / 0x61ce30, `PowerupItem` 0x53e958 / 0x62c9f8; vtable words as cited.
5. **writers** — `check_absent.txt` (end): the drop index (2 writers a game), the BF1 pool capacity (default 25 +
   two state restores), the network flags that gate the roll (set only by session functions; single player takes
   the drop branch, as 444 live drops show), BF2's flag bit 4 (5 sites, `tools/bitset.py`), class bit 33 (one
   setter, `FUN_0036e770`, plus the class copy).
6. **Verifier** — one foreground Opus agent, given `verify_claims.md` only (63 claims): 60 SUPPORTED, 3 PARTLY, 0 NOT
   SUPPORTED (`verify/VERDICTS.md`, 40 min, 200 tool calls). The three PARTLY were omissions, folded in: the
   pickup sound's one exception (§5.2, twice) and two more references to BF1's item type id (§6). It also found,
   and I reproduced: BF2's create gate (§3.2); a hero using up a buff item for nothing (§5.5); two other BF2
   gathers whose mask includes layer 1 (§6); the names `DebuffDamageTimer` / `DebuffDamageRate`; the probe droid's
   dead drop keys; the extra roll in the soldier update; why the per-class index reset holds (§3.1); and that
   `tools/bitset.py` stopped short of BF2's last code (fixed — the whole file is scanned now; the same five sites).

## 11. What stayed [H] or unread

- The pickup's exact contact test: which of the soldier's bodies are in its default subset, and why a standing
  taker does not take a resting item (§5.7). The build gets the bracket, not the routine.
- BF2 states 0xf and 0x10 (the kill is not called in them) and the four unnamed removed-without-death sites (§3.5).
- BF2's hold flag `DAT_006d9330` (§3.2): what sets the state in play, if anything in single player.
- `FUN_002ef6b8`, the test a BF2 bot's detour must pass, and the two visibility tests in the level of detail (§6).
- The taker of the thrown dual in §7.1.
- No measurement with a human player: the BF2 detour and the gain effects were read, not watched.

## 12. BF2 bot detours in the port (2026-10-26, B2ITEM)

`core/src/ai/items.rs` implements the section 6 health/ammo item detour. A BF2 foot bot scans up to six nearby
items every two seconds, tests the actual primary and held-weapon reserve caps, and accepts a useful
health/ammo item within six metres. A successful search adds four seconds to the next search. The local move
goes one metre past the item and returns to its starting point, then resumes the existing objective.
Disappearance or loss of need starts the return early. Scan and detour state are hashed; diagnostics name the
behavior `pickup` and walk kind 6. BF1's path leaves this state untouched. Service stations are outside this
search.

Port mechanisms, not newly verified PS2 findings: the human-proximity LOD gate uses 20 m, with LOS required
beyond 7 m; the two original visibility predicates remain unidentified in section 6. The gate reads the human
player slots. Reachability uses the existing LOS and standing-body floor-line checks, including the point past
the item; a blocked candidate does not hide a later reachable candidate. Recovery, boarding, player squad
orders and grenade escape outrank the detour. A six-second limit and a failed mover window abandon it. These
use the port's existing movement architecture and are not a claim of bit-identical original AI.

The item search and ammo grants share reserve-cap eligibility in `sim/pickups.rs`. Regression coverage
includes fixed-step health/ammo collection and return, deterministic repeated inputs, search timing,
primary/held/infinite reserves, BF1 exclusion, full units, energy/buff-only items, range, mounted units,
blocked paths, alternative candidates, disappearing items, droid exclusion and all retained hash fields.
Shared validation results are in `re-build-zones.md` section 10. No new live PCSX2 comparison or full-round
sweep is claimed.

## 13. Local bot supply deployment (#165, 2026-11-13)

[O] Static source trace, not a fresh original-game play comparison.
Re-read BF2 `FUN_002d6828` (B2:261417–261436): a repair/pilot-class
recipient search runs at five-second intervals over friendly/neutral
objects within 75 m, excluding the giver. The soldier exclusion at
B2:261518–261531 rejects AI soldiers. `FUN_002d71a0`
(B2:261739–261786) gathers up to ten pickups within 15 m and records health
and ammo coverage separately. `FUN_002d72d8` (B2:261791–261846) combines
those flags with the target's missing health and weapon reserve need.
`FUN_002d7510` (B2:261876–261925) requests a dispenser release once upon
lining up; its repair-tool branch can keep firing. The update's service
range includes a five-metre test (B2:262009), not a five-metre search.

BF1 has the same nearby-item suppression in `FUN_00256b30`
(B1:174064–174116) and separate need flags in `FUN_00256c78`
(B1:174117–174170); `FUN_00256fe8` (B1:174249–174361) walks into service
range and invokes the release helper `FUN_00256ea0`. Section 7.1 retains
an earlier live observation of an original BF1 pilot dropping a dual pack.
The original game was also checked at PC addresses 0x0062e300,
0x0062ec20, 0x0062edd0, 0x0062ef50 and 0x0062f080; these confirm the
recipient exclusion, radius, separate flags and one-release behavior.

The port deliberately extends support to **self and AI allies** as
requested by the maintainer. It uses a local search within 5 m, followed
by an ordinary secondary-channel tap; it does not divert engineers from
their objectives for a 75 m service walk. It waits for no known enemy,
three seconds after a hit, a grounded standing posture and no urgent
movement or other active weapon action. Selection/aim and subsequent release/collection each get two seconds,
and a press starts a five-second retry interval. Those scheduling choices
are OURS and PROVISIONAL: measure matched original supply interruptions,
release distances and live ally pickup success before tuning them.

Health and ammo use the shared pickup need flags, including hero-health
and dispenser-ammo exclusions. Existing accessible pickups within 15 m
suppress each supplied resource independently; wall/floor accessibility is
an explicit port extension of the original radius-only mask. Same-tick
recipient claims yield by unit index before a press. Self drops aim at the authored downward pitch limit and take a short
clear approach to collect the released pack; ally drops use the dispenser's authored launch speed and the existing low
arc solver. Finite ammunition, release delay, pickup flight/contact and
pool/refund rules remain with the shared weapon/pickup pipeline. No new
weapons or supplies are added to any arsenal. Fresh original-game supply
captures and live port acceptance remain unchecked.
