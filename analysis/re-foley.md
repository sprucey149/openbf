# OPUS RE (2026-10-02) - [O] UNAUDITED - soldier foley: the `ffx_` table, the surface, what starts a step

Card FOLY. What the PS2 does when a soldier steps, jumps, lands, rolls, crouches, lies down or gets up: where the
sound table is on the disc, which entry a soldier uses, how the surface is chosen, what starts a step and which of
walk / run it is, and why the weapons' `*_mvt_*` keys are silent. Both games.

Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1), `analysis/SWBFRNT2.ELF-decomp.c` (B2), the two ELFs, the SOUND and
world LVLs of both discs, two PCSX2 save states (RAM), and the two live runs of card SNDC
(`engine/artifacts/lead_tmp/sndc/bf1_nab1_run2.csv`, `bf2_uta1_run2.csv`). Tags: **[V]** read in the decomp or on
the disc (lines cited, function checked with `re_check.py fn`), **[L]** measured on PCSX2 (a live run or a save
state's RAM), **[D]** derived, **[H]** hypothesis. Load-bearing claims carry **[O]**. Tools and raw outputs:
`engine/artifacts/lead_tmp/foly/` (section 10).

## 0. Headline

1. **The table is a config chunk `ffx_` in the map's SOUND LVL, one set per sub-level `<map><era>`** (`nab1cw`,
   `uta1gcw`, ...). Two record kinds exist on the discs: `FoleyFXSoldier` (a name, a `Class`, 13 sound slots) and
   `FoleyFXGroup` (a name such as `terrain_foley`, `stone_foley`, `metal_foley`, and a list of `FoleyFXSoldier`
   names). BF1 ships 386 soldier records in 97 groups, BF2 592 in 140. [V][O]
2. **A soldier class names its row with the ODF key `FoleyFXClass`** (`cis_inf_droid`, `rep_inf_trooper`,
   `all_inf_soldier`, ... 9 values); **a prop or building class names its surface with `FoleyFXGroup`**
   (`metal_foley`, `stone_foley`, `wood_foley`, `snow_foley`, `dirt_foley`, `grass_foley`). [V][O]
3. **The surface is the last thing the soldier's body touched**: the terrain gives the group `terrain_foley`, the
   water object or any contact below the water surface gives `water_foley`, any other object gives its class's
   `FoleyFXGroup`. Inside the group the record whose `Class` equals the soldier's `FoleyFXClass` is used; what
   `terrain_foley` means is map data (grass on Naboo Plains, stone on Utapau, snow on Hoth, dirt on BF1 Tatooine,
   water on Yavin Arena). No terrain texture and no collision material is read. [V][O]
4. **A step is started by the drawn animation, not by a timer**: when the soldier is posed for drawing, each foot
   bone (`bone_l_foot`, `bone_r_foot`) that came down and then rises again from below 0.17 m starts a footstep,
   left or right by the bone. A second, distance-based rule (a step every `FootstepStride` = 1.75 m) runs only
   for a human player's soldier, and only when he has no animator or is in first person. **An AI soldier that is
   not being drawn makes no footstep at all.** [V][L][O]
5. **Walk or run is the animation in use**, not the speed: BF1 plays the run sound when the movement clip is one
   of the four run clips - forward above 5.5 m/s (5.0 to stay), backward always, strafe left always, strafe right
   above 5.0 / 5.5 m/s; BF2 when the soldier's speed mode is "run" (above 0.78 x MaxSpeed, 0.71 to stay). Live:
   the BF1 player strafing left at 4.4 m/s plays run steps, strafing right at 4.5 m/s walk steps. [V][L][O]
6. **Jump, Land, Roll, Squat (crouch), Prone, Stand (get up), BodyFall** are slots of the same record, started by
   the stance functions. The two `Footstep*Stop` slots are filled on the disc and read by nothing. BF2's records
   carry no `Prone` and no `Stand` value, so getting up from a crouch is silent there. [V][O]
7. **The weapons' `JumpSound` / `LandSound` / `RollSound` / `SquatSound` / `ProneSound` / `StandSound` and four
   step keys are started by the code on every one of these events and are silent**: none of the 1482 (BF1) and
   494 (BF2) `*_mvt_*` properties has a sample in any sample bank on its disc. [V][L][O]
8. Dead on both discs: `FoleyFXWalker` and `FoleyFXOrdnance` records (none shipped), `foleyfx` regions (no map
   has one and the lookup has no caller), `ClothingRustleSound` (no class sets it), `RunSpeed` (no class sets it;
   the field has no default). [V]

## 1. The table on the disc

### 1.1 Where and what [V]

- Chunk tag `ffx_` (0x5f786666). B1 `FUN_00175d00` 17458-17459 hands it to `FUN_00367558`; B2 `FUN_00177058`
  18422-18423 to `FUN_0044e188`. It is the engine's compiled-config format (`analysis/a2-bake-formats.md`): a
  `NAME`, then `DATA` [u32 hash][u8 argc][args] each followed by a `SCOP` of child `DATA`.
- The chunks are in `DATA/_LVL_PS2/SOUND/<PLANET>.LVL`, inside the `lvl_` sub-level named `<map>` + `cw` | `gcw`
  (`nab1cw`, `uta1gcw`; three BF2 ones end in `cross`: `kam1cross`, `mus1cross`, `spa4cross`): all 66 sub-levels
  that carry foley hash to such a name (`lead_tmp/foly/table_out.txt`). No other LVL of either disc has an `ffx_`
  chunk (binary search of every LVL; the verifier's raw scan: 138 chunks in 10 BF1 files, 215 in 18 BF2 files).
- Top-level tags the loader accepts (B1 354904-354937, B2 510911-510934): `FoleyFXGroup` 0xA1C9CBC5,
  `FoleyFXSoldier` 0xD098618C, `FoleyFXWalker` 0x5E9B9948, `FoleyFXOrdnance` 0xFD545F86 (it builds a
  `FoleyFXImpact`). **On the discs only the first two occur**: BF1 97 groups + 386 soldier records, BF2 140 + 592.
- `FoleyFXSoldier` children (B1 setter `FUN_00369028` 356176-356246, base `FUN_003674c8` 354861-354870, B2
  `FUN_0044f610`; the object's slots are 8-byte sound handles, B1 ctor `FUN_00368e60` 356114-356130):

  | key (hash) | slot | disc value, e.g. nab1cw / `cis_inf_droid` |
  |---|---|---|
  | `name` 0x8D39BDE6 | +0x00 | 0xC6D421EA (referenced by the groups) |
  | `Class` 0xAB3E0BFF | +0x04 | `cis_inf_droid` |
  | `FootstepLeftWalk` 0x2A9F6673 | +0x18 | `cis_inf_droid_walk_grass_left` |
  | `FootstepRightWalk` 0xE79EB7C2 | +0x20 | `cis_inf_droid_walk_grass_right` |
  | `FootstepLeftRun` 0x640F6EFB | +0x28 | `cis_inf_droid_run_grass_left` |
  | `FootstepRightRun` 0xA4DF1B00 | +0x30 | `cis_inf_droid_run_grass_right` |
  | `FootstepLeftStop` 0xB59A8702 | +0x38 | `cis_inf_droid_stop_grass` (never read, 4.5) |
  | `FootstepRightStop` 0x54E8AF1F | +0x40 | `cis_inf_droid_stop_grass` (never read) |
  | `Jump` 0xA73F5C0D | +0x48 | `cis_inf_droid_jump_grass` |
  | `Land` 0x6DEA369C | +0x50 | `cis_inf_droid_land_grass` |
  | `Roll` 0x16FF3D1E | +0x58 | `cis_inf_droid_roll_grass` |
  | `Prone` 0x6DD99A25 | +0x60 | `cis_inf_droid_lie_grass` |
  | `Squat` 0x65BFA4E1 | +0x68 | `cis_inf_droid_squat_grass` |
  | `Stand` 0x75493323 | +0x70 | `cis_inf_droid_getup_grass` |
  | `BodyFall` 0x70DC49D0 | +0x78 | 0x55A0603C (a property of the LVL; its name is not recovered) |
  | `Impact` 0x1E71B297 | +0x10 | absent from every soldier record |

  The key names are hash matches of candidate strings [D]; the six `Footstep...` names came from a 3-word brute
  force (`lead_tmp/foly/names2.py`), the rest from single words. The VALUES are what matters to a port and they
  are read directly. **BF1's 386 records carry all 13 slots; BF2's 592 carry 11 - no `Prone`, no `Stand`.** A
  record is read until the first key its setter does not know (`FUN_00367408` 354842-354845).
- `FoleyFXGroup` children (B1 `FUN_00368090` 355487-355520, B2 `FUN_0044eb50`): `name`, then one `FoleyFX`
  (0xE64C1C32) per member, a `FoleyFXSoldier` name. A member whose record is not in the loaded data is skipped
  (B1 355500-355503: the lookup `FUN_00367798` returned 0). nab1cw lists 5 members per group and 2 of them are not
  defined in that sub-level; RAM holds 3 per group (section 8.3).
- The record `Class` values are `all_inf_soldier`, `imp_inf_trooper`, `imp_inf_soldier`, `cis_inf_droid`,
  `rep_inf_trooper`, `rep_inf_soldier`, `cis_inf_soldier`, `wok_inf_soldier`, `npc_inf_small` (BF2), and five
  that no ODF names: `cis_inf_sbdroid`, `all_inf_jedi`, `imp_inf_jedi`, `rep_inf_jedi`, `cis_inf_jedi`.
- Sound names are `<side>_inf_<unit>_<action>_<surface>[_left|_right]`; `imp_inf_trooper_land_grass` and
  `gungan_inf_heavy_turn_l` have the SAME hash 0x321177DA, so a name dictionary must prefer the family name here.
  Every foley property the family dictionary names has its samples in a bank of its disc on BF1 (2886 of 2886);
  on BF2 3827 of 3934 (`mvt_census.py`).

### 1.2 What the groups mean on the showcase maps [V] (`table_out.txt`)

| sub-level | groups (member -> the surface word of its sounds) |
|---|---|
| bf1 nab1cw | terrain_foley, grass_foley -> grass; stone_foley -> stone; metal_foley -> metal (wookiee: grass in all four) |
| bf1 nab1gcw | terrain_foley -> grass; grass / stone / metal_foley list only names the sub-level does not define (empty at run time) |
| bf1 tat1gcw (the port's `tat1`) | terrain_foley, dirt_foley -> dirt; stone_foley -> stone; metal_foley empty |
| bf1 yav2cw / yav2gcw | terrain_foley -> water; stone_foley -> stone; metal_foley -> metal (gcw) |
| bf2 uta1cw / uta1gcw | terrain_foley, stone_foley -> stone; metal_foley -> metal |
| bf2 myg1cw / myg1gcw | terrain_foley, stone_foley -> stone; metal_foley -> metal |
| bf2 hot1gcw | terrain_foley, snow_foley, dirt_foley -> snow; metal_foley -> metal |
| bf2 kam1cw | metal_foley only - no terrain group |
| bf1 nab2cw | terrain / stone / wood_foley -> stone; water_foley -> water; metal_foley -> metal |

### 1.3 The class keys [V]

- `FoleyFXClass` (0x99D55922): B1 `FUN_00367ee0` 355385-355389 stores the hash of the value; called from the
  entity class parsers `FUN_001e3678` 93123, `FUN_001fb6e8` 109390, `FUN_002738a8` 195905. B2 `FUN_0044e9c0` 511350.
  Baked side classes of both games together (`engine/assets_baked/classes/*.ron`, prop 2580896034):
  `all_inf_soldier` 18 classes, `imp_inf_trooper` 12, `cis_inf_droid` 11, `rep_inf_soldier` 8, `wok_inf_soldier` 7,
  `rep_inf_trooper` 7, `imp_inf_soldier` 6, `cis_inf_soldier` 4, `npc_inf_small` 1. A child class inherits it from
  its base.
- `FoleyFXGroup` (0xA1C9CBC5): B1 `FUN_00367b78` 355188-355193 looks the group up by name at parse time and stores
  the pointer; same three parsers (93122, 109386, 195907). B2 `FUN_0044e620` 511134-511137. Baked classes:
  `metal_foley` 125 side classes; in the per-map `classes.ron` files `metal_foley` 416, `stone_foley` 383, `wood_foley`
  58, `snow_foley` 18, `dirt_foley` 4, `grass_foley` 1.
- Soldier class keys read by the soldier class parser (B1 `FUN_0020f2e0`, B2 `FUN_00252bb0`):

  | key | B1 field / default | B2 field / default | set by a class on the discs |
  |---|---|---|---|
  | `FootstepStride` 0xD641F0D6 | class+0x1da0 (123585) / 1.75 (ctor `FUN_0020d238` 121987) | class+0x115c (170958) / 1.75 (ctor `FUN_00250488` 169623) | none |
  | `RunSpeed` 0x44C8744D | class+0x1da4 (123004) / **no default** | class+0x1160 (172001) / **no default** | none |
  | `ClothingRustleSound` 0x5625FC0B | class+0x1dfc (123119) | class+0x11b8 (171792) | none |
  | `MinFootHeight` 0x91415FA0 | - (literal 0.17, 116225 / 116332) | class+0x122c (171352) / 0.17 (169701) | one: BF2 `cis_inf_officer` 0.3 |
  | `WakeWaterSplashEffect` 0xF44FEE52 | class+0x1908 (123540) | class+0x8cc | - (an effect, not a sound) |

  `FootstepStride`, `RunSpeed` and `MinFootHeight` are brute-forced names [D]. `RunSpeed`'s field is written only
  by the parser (B1: `re_check.py absent b1 0x1da4` = the read at 115859 and the parse at 123004; B2: the read,
  the parse and the base-class copy 169922), so with no ODF setting it the value is whatever the allocation held.
  RAM: BF1 NAB1's classes hold eight unrelated bit patterns (0x7F343434, 0x2C2C2C30, 0x8000497A, ...); BF2
  UTA1's all hold 0 (8.3).

## 2. The run-time model [V]

- Every collision object carries a **collider** {class pointer, current FoleyFX, current group} at +0x30 and a
  **collidee** {class pointer} at +0x40 (B1 ctor `FUN_001685d0` 7101-7106: `[9]` = the object's type bit,
  `FUN_00367bc8` builds the collider, `FUN_00367af8` the collidee). A soldier's collision object is at soldier+0x100
  in BF1 (`FUN_001e1e18` 91383), entity+0x10 in BF2, so its collider is soldier+0x130 (B1) / entity+0x40 (B2).
- The collider class is the object's `FoleyFXClass` hash; the collidee class is its `FoleyFXGroup` pointer (1.3).
- A new collider starts on the terrain group (B1 `FUN_00367bc8` 355209-355210 with `FUN_00368430` = `DAT_00519484`;
  B2 `FUN_0044e670` 511155-511156). The two special groups are remembered when their records load: B1
  `FUN_00368090` 355493 / 355518-355519 (`terrain_foley` 0x26C04C0E -> `DAT_00519484`, `water_foley` 0x425AA124 ->
  `DAT_00519480`); B2 `FUN_0044eb50` 511448-511449 / 511465 (`DAT_0060125c`, `DAT_00601258`). Their only other
  writers clear them at unload (`re_check.py writers`, section 10).
- Changing group (B1 `FUN_00367c90` 355264-355271, B2 `FUN_0044e738` 511210-511217): only when the group is not
  null, differs from the current one and the collider has a non-zero class hash; then the group's member whose
  `Class` equals that hash is looked up (B1 `FUN_00368238` 355545-355575, B2 `FUN_0044eca8` 511525-511535), falling
  back to the first member whose `Class` is 0. The current group is replaced; **the current FoleyFX is replaced
  only when a member was found**, so on a group with no row for the class the previous surface's sounds stay.
- The soldier's sound source is "the current FoleyFX if it is a FoleyFXSoldier" (B1 `FUN_00204fd8` 115840 through
  `FUN_00367c30` 355234-355243, B2 `FUN_0023f808` 157485 / `FUN_0044e6d8` 511180-511189; the getter compares the
  FoleyFX's type hash, vtable slot +0xc, with the one asked - an asked type of 0 would skip the test, no caller
  passes 0). A null result plays nothing - a class with no `FoleyFXClass`, or a map with no matching row.

## 3. The surface [V][O]

`FUN_00367d08(collider, type, collidee, position)` (B1 355286-355343) / `FUN_0044e7b0` (B2 511232-511298) is called
from every collision callback with the OTHER object's type bit (15 call sites in B1, 16 in B2,
`checks/refs_b*.txt`). The soldier's own callback is B1 `FUN_00202f38` 114461-114474 - it makes the call for every
other body whose type is not 1 (114417); B2 `FUN_0023cfd0` 155761 is its twin by position and signature [D]. The
other sites are other entity classes' callbacks.

| the other object | B1 type | B2 type | group taken |
|---|---|---|---|
| contact point below the water surface at its XZ (B1 `FUN_004768f0`, B2 `FUN_005613c8`) | any | any | water (355286-355289; 511232-511235) |
| the terrain object (B1 `FUN_002123c8` 124844) | 0x40 | 0x80 | `terrain_foley` (355290-355293; 511273-511278) |
| the water object (B1 `FUN_00220cb8` 135216) | 0x80 | 0x100 | `water_foley` (355318-355320; 511263-511264) |
| an entity or prop | 1, 2, 4, 8, 0x10, 0x100, 0x200, 0x400, 0x800, 0x1000 | 1, 2, 4, 8, 0x10, 0x20, 0x200, 0x400, 0x800, 0x1000, 0x2000 | its class's `FoleyFXGroup`, if it has one (355337-355341; 511293-511297) |
| anything else (BF2 type 0x40 too, 511236-511238) | | | unchanged |

- A soldier standing in water also sets water directly when the ripple timer of the soldier update fires (B1
  `FUN_00205cc8` 117186-117256, the timer test at 117252, type 0x80; B2 `FUN_00240bf8` 160053-160057, type 0x100).
  The timer is one global (`DAT_0051789c`), period 1 / (0.3 v^2 + 4) s, not one per soldier.
- "Last contact wins": a wall the soldier brushes switches the surface until the next floor contact. [D]
- `foleyfx` regions: the region factory exists (B1 `FUN_00368b90`, string 0x547f30), but the two lookups
  `FUN_00368a78` / `FUN_00368b00` have no `jal`, no `j` and no data-word reference in the ELF, and no world LVL of
  either disc contains the string (`soundspace`, `deathregion` do). Dead. [V]

## 4. What starts a sound

### 4.1 The step from the animation [V][O]

B1 `FUN_00205538(soldier, pose, matrix)` 116118-116421, called once from the soldier's draw `FUN_002013a0` 113938
(itself the render entry `FUN_00210fb0` 124087) right after the animator has posed the skeleton; B2 `FUN_0023fe00`
157775-158050 from `FUN_00239bb0` 154311.

- Runs only while the soldier is on the ground (bit 1 of B1 soldier+0xbfd 116119, B2 entity+0xa74 157776). BF2
  also returns when the soldier is the first-person view's own body (157779-157815).
- Reads the model-space height of `bone_l_foot` (CRC 0x1AF77461) and `bone_r_foot` (0x2F06FA86), keeps last
  call's two heights (B1 soldier+0xc20 / +0xc24, 116419-116421).
- Per foot, with flag mask 0x1 (left) / 0x2 (right) of the u16 at B1 soldier+0xbfe (B2 entity+0xa76):
  - the foot DROPPED by more than 0.01 m since the last call: set the flag (and start `ClothingRustleSound`) -
    B1 116214-116217, 116316-116319;
  - else, if the flag is set and the foot ROSE by more than 0.01 m: clear the flag, and **if the previous height was
    below 0.17 m (BF2: the class's `MinFootHeight`) start the footstep** - B1 116224-116226 (left,
    `… B2 157922-157925, 158029-158032.
  So the sound comes when a foot that came down lifts off again, once per foot per stride, and the position is the
  foot bone's. The thresholds are per CALL (the PS2 draws at 30 Hz: 0.01 m per call = 0.3 m/s).
- Also at that moment, not sounds: a `decal_soldier_footprint` decal for either foot (B1 116281-116282 and
  116388-116389, string 0x532958) unless a session gate skips it (116227-116243; its flags are 0 in the ELF and in
  an offline battle's RAM), and the class's `WakeWaterSplashEffect` when the foot is under water and the soldier's
  squared speed is above 4.5 (116194-116204, 116309).

### 4.2 The step from distance [V]

B1 `… B2
`FUN_0023f9e0` 157584-157726 (`FUN_00240bf8` 159533).

- Gates, in order (B1): on the ground (115940); **not (has an animator and is not first person)** (115975-115977;
  the animator is soldier+0xbf0, taken in the draw at 113774-113777 and dropped at 116789-116799 when another
  soldier has taken it; first person is the view object at soldier+0x558); dt > 0; **soldier+0x388 >= 0**
  (115981). B2: the same four (157585, 157647-157648 with entity+0x7f0, entity+0x2a0, entity+0x330) plus "not
  rolling" (state 5) and a session gate (157588-157603) that passes when the selected one of `DAT_00600548` /
  `DAT_00600549` is 0 (both are in an offline battle, section 10).
- soldier+0x388 (B2 entity+0x330) is the local player's index: RAM has 0 on the player's soldier and -1 on every
  AI soldier, 43 of 43 (BF1) and 34 of 34 (BF2) [L]. **So this rule never runs for an AI soldier.**
- The rule: an accumulator (B1 soldier+0xc48, B2 entity+0xae8) grows by |velocity| x dt, or while standing
  (|v| < 0.0001) by |turn input| x dt x 0.0005 (115996-116001). At half of `FootstepStride` with no foot flag set:
  `ClothingRustleSound` and a foot flag, alternating by mask 0x10 of soldier+0xbfd (116053-116064). At the full
  stride, or at once when the soldier has stopped with anything accumulated: the footstep, left when flag 0x1 was
  set else right (116018-116043). BF1 then zeroes the accumulator (116042); BF2 keeps the remainder unless
  stopped (157685-157699) and steps only when a foot flag is already set (157668, 157682).
- Its walk / run test is speed < `RunSpeed` (B1 115858-115859, B2 157503-157504) - an uninitialised field (1.3).

### 4.3 Which soldiers step [V][L][O]

| soldier | drawn (holds an animator) | not drawn |
|---|---|---|
| AI | 4.1 | **nothing** |
| the player, third person | 4.1 | 4.2 (he has lost his animator) |
| the player, first person | BF2: 4.2 only. BF1: 4.2, and 4.1 too if the body is still posed [H] | 4.2 |

RAM (8.3): on BF1 NAB1 4 of 44 soldiers hold an animator (the player and three near him), on BF2 UTA1 1 of 35
(the player); every AI soldier's distance accumulator is 0.000, including 5 (BF1) and 17 (BF2) that are on the
ground, hold no animator and move at 5 m/s or more.

### 4.4 Walk or run [V][L][O]

`FUN_00205000(speed, soldier, right, pos, vel)` (B1 115856-115912) / `FUN_0023f830` (B2 157501-157553) picks the
slot: run left +0x28 / run right +0x30, walk left +0x18 / walk right +0x20. With an animator the test is its
current movement clip id (B1 animator+0xa0: 7, 8, 0xb, 0xc = run, 115862-115872; B2 animator+0x20c0: 6, 8, 10,
0xc, 157507-157513); without one, speed < `RunSpeed`.

- **BF1 ids** (`FUN_0022e990` 144969-145199, called at 142626 and stored at 142698 in the pose function
  `FUN_0022aa10`; pairs in `FUN_0022f328` 145314-145342): 0 idle (|fwd| and |side| under 0.3 m/s, or faster than
  8 m/s), 3 / 4 turning on the spot, 5 / 7 walk / run forward, 6 / 8 backward, 9 / 0xb left, 10 / 0xc right.
  Except in the 8 m/s test the speeds are first divided by the animator's scale (animator+0x20c4, the class's
  skeleton scale class+0x1e5c; 144992-144995). A 0 result keeps an old id of 0-4 (142631-142634). Where a pair's
  walk clip is missing the run id is returned at any speed.
  - forward: run when speed > 5.5 m/s, or > 5.0 when already running (145127-145148);
  - right (side > 0): run when the side speed > 5.0, or 5.5 when already walking right (145034-145052);
  - (2026-10-05, `re-soldier-anim-layers.md` 3.5: the two strafe bullets below hold only for a stance that HAS
    strafe clips - BF1 prone. Stand and crouch ship none, so a standing soldier strafes on the forward or backward
    clip with twisted legs; the live "left runs, right walks" is the alert window -1.8064 .. +1.1781 rad sending
    a left strafe to `runbackward`. Ids 9 / 0xb are walkright / runright by the name table, +x being his left.)
  - left: the test is `side < threshold` with side negative - always true - so **left is always the run clip**
    (145062-145080);
  - backward: the speed is negated before `speed < threshold` - always true - so **backward is always the run
    clip** (145154, 145174-145193).
  The clip's playback rate is x (0.95 v / v_clip + 0.05) (145143, 145197), v_clip the clip's own root speed, so a
  stride covers the same ground at any speed.
- **BF2 ids** (`FUN_0027ed18` 202142-202330): 5 / 7 / 9 / 0xb by direction, +1 (run) when the owner's speed mode
  (bits 44-45 of the u64 at entity+0xa70) is 2, or when the walk clip is missing (202311-202322). The mode is set
  each update in `FUN_0024f350` 168676-168808 from the HORIZONTAL distance moved (dx^2 + dz^2: `VMUL.xz` at
  0x24f730): 0 below dt x `SkeletonRootScale` (class+0x1220) x 0.3 (0.2 when already moving), else 2 above
  dt x MaxSpeed (class+0x74c) x 0.78 (0.71 when already 2), else 1. In stance states 3 and up it is written 0.
- Live (8.2): BF1 forward 6.5 m/s run, backward 6.3 m/s run, **left 4.4 m/s run, right 4.5 m/s walk**.

### 4.5 The other slots [V][O]

Every site is `FUN_00204fd8` / `FUN_0023f808` + one slot; the ELF has 9 `jal`s to the BF1 getter and 8 to BF2's,
no jump and no data word (`lead_tmp/foly/checks/refs_b*.txt`), and they are these:

| slot | BF1 site | BF2 site | when |
|---|---|---|---|
| Footstep walk / run x left / right | `FUN_00205000` | `FUN_0023f830` | 4.1, 4.2 |
| `Jump` +0x48 | `FUN_00208b78` 118198-118201 | `FUN_00246ed0` 162172-162175 | the jump action; started in the soldier's own handle, so it follows him |
| `Land` +0x50 | `FUN_00209018` 118505-118521 (from states 4, 6, 7) | `FUN_00246850` 161881-161884 (from 4, 6, 7, 8, 0xc, 0xe) | the stand-up action when it ends an air state |
| `Roll` +0x58 | `FUN_002093d0` 118659-118663, `FUN_00209488` 118699-118703 | `FUN_002471b0` 162271-162274 | the roll action; in the soldier's handle |
| `Prone` +0x60 | `FUN_00208c70` 118250-118254 | none | going prone (BF1) |
| `Squat` +0x68 | `FUN_00208b00` 118153-118159 | `FUN_00246e38` 162022-162025 | the crouch action |
| `Stand` +0x70 | `FUN_00209018` 118470-118474 (from states 1, 2) | `FUN_00246850` 161847-161850 (cases 1, 2) | standing up from crouch or prone; BF2's records leave the slot empty |
| `BodyFall` +0x78 | `FUN_00205cc8` 117586-117610 | `FUN_00240bf8` 160654-160658, 160777 | a thrown soldier (BF1 state 5) touching the ground after 0.3 s |
| `FootstepLeftStop` / `RightStop` +0x38 / +0x40 | none | none | never |

Each action also sends the current weapon an event number (0-3 the four steps, 4 jump, 5 land, 6 roll, 7 prone,
8 crouch, 9 stand): section 5.

## 5. The weapon `*_mvt_*` keys [V][L][O]

- `FUN_00284e50(weapon, n)` (B1 208840-208850; B2 `FUN_00326e50` 318633-318643) copies the sound handle at weapon
  class + 0x140 + 8n (B2 + 0x174 + 8n) and starts it at the owner, with no condition but "the weapon has an owner"
  (B1 weapon+0x68, B2 weapon+0x6c non-zero).
  The class parser `FUN_002864f0` fills those slots from `JumpSound` 0x580DC4EC (+0x160, 210021), `LandSound`
  0xAD3E89E3 (+0x168, 210300), `RollSound` 0xD211149D (+0x170, 210466), `SquatSound` 0xFC9B0358 (+0x180, 210558),
  `StandSound` 0x0E8B2D62 (+0x188, 209904), `ProneSound` 0xCADB9D74, and four step keys 0xB77F3981, 0xB989FC18,
  0x1064C0A7, 0x10A991E4 (+0x140 ... +0x158; names not recovered).
- The ODFs set them to `<side>_weap_inf_<weapon>_mvt_<jump|land|roll|squat|lie|getup>` (and `SquatSound` to the
  `_lie` sound, `ProneSound` to the `_squat` one). Those properties exist in the SOUND LVLs, each with one sample
  id - and **no sample bank on either disc contains any of those ids**: 0 of 1482 properties (BF1), 0 of 494 (BF2),
  against 2886 of 2886 BF1 foley properties by the same test (`lead_tmp/foly/mvt_census.py`; the verifier's own
  scan: 18 / 13 distinct mvt sample ids, none after an `id` field anywhere on its disc). "mvt property" means one
  the name dictionary names.
- Live: no `mvt` sound in either 105 s run (SNDC). A port plays nothing for these keys.

## 6. Things that are not footsteps but sit beside them [V]

- `ClothingRustleSound`: started when a foot starts down (4.1) and at half a stride (4.2); no class sets it.
- BF1 soldier update, after the foley call: a class loop sound (class+0x1de0) and `LowHealthSound` (class+0x1e04)
  below `LowHealthThreshold` x max health (116840-116861) - card USND's.
- `FoleyFXImpact` (`Impact` slot, `FUN_00169118` 7654 asks the collider for a FoleyFX of type `FoleyFXImpact`): a
  soldier's FoleyFX is a `FoleyFXSoldier`, and no `FoleyFXOrdnance` record ships, so nothing plays.

## 7. Sound properties

Foley properties inherit template 0xFE30AE3A: gain 0.7, 10-50 m, rolloff 3, four takes per step property
(`analysis/audio-census.md`, card SNDT). The farthest foley start in the two runs is 49.8 m / 50.1 m (a roll
each time; the farthest step 47.8 m / 23.7 m) [L]: a start with zero gain is refused, so the 50 m shows in the data.

## 8. Live evidence [L]

### 8.1 Counts (SNDC's runs, one-shot voices)

| run | steps within 6 m of the eye | steps farther | other foley |
|---|---|---|---|
| BF1 NAB1, 105 s | 110 (104 the player's `cis_inf_droid`, 6 a trooper beside him) | 107 (6-48 m, median 19 m) | near: jump 3, land 3; far: squat 5, getup 5, roll 1 |
| BF2 UTA1, 105 s (49 s of battle) | 45 (all the player's) | 15 | near: jump 2, land 2, squat 1; far: jump 1, roll 1 |

"Steps" are the run / walk rows (217 on BF1 = SNDC's count; all 234 BF1 foley rows split 116 / 118).

107 steps in 105 s from about 43 AI soldiers is one a second: what "only a drawn soldier steps" gives, and a
tenth of what every soldier within 50 m would. [D]

### 8.2 The player's steps (`lead_tmp/foly/player_steps.py <run.csv>`)

| | interval | spacing | speed | sounds |
|---|---|---|---|---|
| BF1 forward (W) | 0.267-0.367 s, mean 0.311 (12 steps) | 1.9-2.2 m | 6.5 m/s | run, right / left alternating |
| BF1 strafe left (A) | 0.334-0.367 s | 1.4-1.7 m | 4.4 m/s | **run** |
| BF1 strafe right (D) | 0.233 / 0.367 s alternating | 1.0 / 1.6 m | 4.5 m/s | **walk** |
| BF1 backward (S) | 0.233-0.267 s | 1.5-1.8 m | 6.3 m/s | run |
| BF2 forward | 0.267 / 0.300 s, mean 0.280 (13 steps) | 1.8-2.2 m | 7.0 m/s | run |

- The spacing is 2.0 m, not `FootstepStride`'s 1.75 m: the steps are the run clip's (19 frames, root travel
  4.014 m, `analysis/re-anim-keys.md` section 5), two per cycle. [D]
- Start of a run: one or two `walk` steps 0.03-0.13 s after the stick, the first `run` step at +0.33 s (3 of 3
  starts). The feet that were down lift (4.1) while the clip is still idle or walk. [D]
- End of a run: 0-2 `walk` steps within 0.25 s of the release (4 stops).
- Jump from a stand: `walk_right`, `jump`, `walk_left` on the same update (BF1), `jump` + `walk_right` (BF2);
  jump while running: `jump` alone. `land` 0.83-1.03 s after `jump` (5 jumps).
- The run's clock is quantised at 1/30 s (the game's update), which is the 0.267 / 0.300 / 0.333 pattern.

### 8.3 RAM (`lead_tmp/foly/ram_foley.py`, `ram_foley2.py` on the two SNDC states)

- BF1 `bf1_nab1_cp1_cp2_postA_spawn.p2s`: 16 FoleyFX objects, all `FoleyFXSoldier` (vtable 0x547f80); 4 groups
  `terrain_foley`, `grass_foley`, `stone_foley`, `metal_foley`, 3 members each (`cis_inf_droid`, `rep_inf_trooper`,
  `wok_inf_soldier`); water group pointer 0, terrain group pointer set. 44 soldiers: `FootstepStride` 1.750 in
  every class; collider class `cis_inf_droid` / `rep_inf_trooper`, group `terrain_foley`, FoleyFX of the same class.
- BF2 `bf2_uta1_cp3_con_cp1_postA_spawn.p2s`: 3 groups x 5 members (`cis_inf_droid`, `cis_inf_sbdroid`,
  `rep_inf_trooper`, `cis_inf_soldier`, `rep_inf_soldier`) = the disc's uta1cw; 35 soldiers, stride 1.750,
  `RunSpeed` 0, `MinFootHeight` 0.170 (0.300 on the three `cis_inf_officer`); 33 on `terrain_foley`, **2 on
  `metal_foley`** (standing on a metal prop); speed mode 1 on the three walkers (2.3, 5.2, 5.2 m/s, MaxSpeed 7),
  mode 2 on every soldier at 6.0-7.0 m/s. Session gate bytes 0x600548 = 0, 0x600549 = 0.

## 9. For the port

The port has no first-person view; each unit has an animator that is "visual only" (`game/src/animator.rs`:
PROVISIONAL clip choice, a 4.5 m/s walk / run switch, constant playback rate) and lives with the GPU assets, so a
headless run has none. Sounds start from `game::event_sounds`, once per stepped tick.

1. **Bake**: `SOUND/<PLANET>.LVL` -> per sub-level `<map><era>` its groups and their rows (class hash -> the slot
   property hashes), dropping members the sub-level does not define (1.1). `FoleyFXClass` per soldier class and
   `FoleyFXGroup` per prop class are already in the baked class props (2580896034, 2714356677), inherited through
   `base`. The sub-level for a battle is the map's id + `cw` / `gcw` by the era.
2. **Surface** (section 3): what the soldier stands on - terrain -> `terrain_foley`; below the water surface ->
   `water_foley`; a prop -> its class's `FoleyFXGroup`; a group that is missing, or has no row for the soldier's
   `FoleyFXClass`, keeps the last row. Start on `terrain_foley`.
3. **Steps** (4.1, 4.4, 8.2): the sound is the drawn clip's foot coming off the ground. A sim-state model that gives
   the same sounds without the GPU-side animator: per unit, the PS2's movement clip id by the chooser's rules, a
   phase advanced by distance moved / the clip's root travel (run forward 4.014 m per cycle: 2.0 m per step; walk
   forward 2.581 m), a step at each foot's lift-off phase of that clip (run 4.1's rule over the clip once, at
   30 Hz). Left and right alternate; the first step of a start comes at once (a walk step) and a stop ends with
   one (8.2). `FootstepStride` 1.75 m is the first-person rule and is not what a third-person player hears.
4. **Gait**: BF1 by clip id - forward above 5.5 m/s (5.0 to stay), backward and strafe left always run, strafe
   right above 5.0 / 5.5; BF2 above 0.78 x MaxSpeed (0.71 to stay) in any direction.
5. **Who**: on the PS2 only a drawn soldier steps (4.3). Default to "inside the camera's view"; then the count
   compares with 8.1 (104 own + 107 others in 105 s on NAB1). "Every soldier within 50 m" is about ten times
   denser - a reversible feel choice, one constant.
6. **Events** (4.5): Jump, Land, Roll, Squat, BodyFall; Stand and Prone where the row has the slot (BF1 maps).
   Jump and Roll follow the soldier.
7. **Nothing** for the weapon `*_mvt_*` keys, the two Stop slots, `ClothingRustleSound`.

For card ANIM: the movement clip chooser, its thresholds and the playback rate x (0.95 v / v_clip + 0.05) of 4.4
are the PS2's answer to the animator's provisional 4.5 m/s switch and constant rate.

## 10. Checks run

Scratch `engine/artifacts/lead_tmp/foly/`: `h.py` / `crc.py` (the two name hashes), `ffx_parse.py` + `dump/` (every
`ffx_` chunk), `table.py` -> `table_out.txt`, `names*.py`, `keyname.py`, `bank_ids.py`, `mvt_census.py`,
`lvl_tops.py`, `player_steps.py`, `ram_foley.py`, `ram_foley2.py`, `checks/`.

1. **fn** - `re_check.py fn` over every cited line: 127 BF1 lines (`checks/fn_b1.txt`), 104 BF2 lines
   (`checks/fn_b2.txt`); each is in the function named beside it in this file (the verifier re-checked each).
2. **absent** - `re_check.py absent b1 0x1da4` -> 2 hits (`FUN_00205000` 115859, `FUN_0020f2e0` 123004);
   `absent b2 0x1160` -> the read 157504, the copy 169922, the parse 172001 and three unrelated functions;
   `absent b1 FUN_00368a78 FUN_00368b00` -> only their own headers; `scripts/tools/refs.py` (every `jal` / `j` /
   data word of an address in the ELF) for the getters, the step functions, the collider update and the loader
   (`checks/refs_b1.txt`, `refs_b2.txt`); `/usr/bin/grep -a -i foleyfx` over every world LVL -> 0 files, with
   `soundspace` 17 / 25 and `deathregion` 6 / 16 files as the sanity hits; record kinds and sample presence by
   `table.py` and `mvt_census.py`.
3. **twin** - every BF1 function above has its BF2 twin opened and cited: loader, group reader, lookup, collider
   update, getter, step chooser, the two step rules, the stance sites, the weapon event, the class parser and ctor.
   BF2's gait rule is NOT BF1's (4.4).
4. **word** - strings 0x547fa0 `FoleyFXSoldier`, 0x547f30 `foleyfx`, 0x547f10 `FoleyFXImpact`, 0x548008
   `FoleyFXWalker`, 0x532958 `decal_soldier_footprint`, B2 0x639440 / 0x6393e8; vtables 0x547f80 / 0x547ef0 /
   0x547fe8 / 0x547e58 (slot +0xc of each returns its type hash global: `eedis.py` 0x369400, 0x3688c8, 0x367a98);
   terrain / water object vtables 0x5330f0 / 0x533e30; B2 0x600548 = 0 and 0x60054c = 0x01010100 (`checks/words.txt`).
5. **writers** - `DAT_00519484` / `DAT_00519480`: set in `FUN_00368090` 355518-355519, cleared in `FUN_00368388`
   355631-355632; B2 `DAT_0060125c` / `DAT_00601258`: `FUN_0044eb50` 511448-511449, `FUN_0044ee10` 511595-511596;
   B2 `DAT_00600548`: 0 at 17265, 432254, 432636, 482975 and `= DAT_00600549` at 432251; `DAT_00600549` set to 1
   in `FUN_00400030`, `FUN_00400058`, `FUN_004066c0` and - found by the verifier's ELF store scan, not in the
   Ghidra export - a virtual at 0x40bb20 (store at 0x40bc24), all session code; RAM of an offline battle has
   both 0. A text `writers` list needs the ELF store scan behind it.

What stays **[H]** or **[D]**: the key NAMES found by brute force (the hashes and values are [V]); whether BF1's
first-person body is still posed; "last contact wins" as heard (not measured against a wall); the 2.0 m stride
as the run clip's root travel (the BF1 playback divisor F vs F-1 moves the predicted interval between 0.325 and
0.308 s against 0.311 measured); the five `*_jedi` / `sbdroid` Class names; the unrecovered `BodyFall` and
weapon step key names; why BF1 nab1gcw's non-terrain groups name rows it does not ship.

**Verifier** (one Opus agent, foreground, claims and citations only - `lead_tmp/foly/claims.md`, verdicts
`lead_tmp/foly/verdicts.md`, its scripts `lead_tmp/foly/verify/`): 64 claims, **48 SUPPORTED, 16 PARTLY, 0 NOT
SUPPORTED**. Five PARTLY were substantive and are folded in above, each reproduced first: a fourth writer of
`DAT_00600549` outside the Ghidra export; BF2's speed mode uses the horizontal distance and is written 0 in states
3 and up; BF1's chooser divides the speeds by the animator's scale and a 0 result keeps an old id of 0-4; the foley
sample counts (2886 / 2886, 3827 / 3934, not the stale 1967 / 3176) and a mistyped sample id; the live counts (217
step rows = 110 + 107, 234 foley rows). Eleven were wording (the three `cross` sub-levels, flag masks 0x1 / 0x2
named as bits, the type-1 skip in the soldier's callback, the ripple timer, the getter's type 0, the decal's gate
and its right-foot twin, the missing-walk-clip corners, BF2's weapon+0x6c). Its extra finds: BF2's records have no
`Prone` / `Stand` slot; BF1's `RunSpeed` word holds eight unrelated bit patterns over the 44 soldiers.
