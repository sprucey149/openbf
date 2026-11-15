# RE: changing class at a friendly command post while alive (issue #28)

Status: [O] UNAUDITED. BF2 PS2 decomp read by grep / `re_check.py` only, 2026-10-28; no new PCSX2 run. Extends
`re-spawn-flow.md` §7 ("state 4, the class select while alive").

Question (issue #28): on BF2 PS2, where may a live soldier change class, is the change immediate or at a respawn,
and what happens to his health and ammo? Answer: inside a post his team can spawn at, facing it, the use button
opens the class screen; an accepted class replaces the soldier **at once, where he stands**; the new soldier takes
the old one's **health fraction** and, per weapon channel, the old weapons' **ammo fraction**. The port keeps the
first two and replaces the ammo rule with the maintainer's (§5).

## 1. Who opens the screen [V]

`python3 scripts/re_check.py callers b2 FUN_00397f00`:

*(decompiler excerpt omitted from the public copy)*

`FUN_00397f00` (B2:392728-392739) sets the spawn display's state `+0x18 = 4` and opens it (`FUN_00396a00`): the
class screen over a live unit (`re-spawn-flow.md` §1 table, state 4). In the soldier update `FUN_001f8548`
(B2:109291-109299) it runs only for a unit with a viewport (*(decompiler detail omitted)*, the index handed to
`FUN_00395878`, the viewport's spawn display: a human), whose input word `+0x2c0` has `& 3 == 3`, and whose flag bit
32 of `+0x1bc8` (*(decompiler detail omitted)*) was set this update. [H] `+0x2c0 & 3` is the use button's press: the PS2 frame
`ps2/out/bf2_heal_2m.png` (`re-powerup-station.md` §5.3) shows the prompt "△ CHANGE SOLDIER CLASS", BF2 hint 5
"{useButton} change soldier class", armed by the same flag's twin in `FUN_00248c38` (bit 17 of `+0x4e0`).

## 2. "Near" a post [V]

The flag is cleared each update (B2:109004) and set by one of two scans (B2:109161-109290):

1. **Posts** (B2:109166-109233): every post of the list `DAT_005fed34` whose mask `+0x74` holds the player's index
   (`member+0x130`) - BF2's counted players-inside mask of the capture region (`re-cp-capture.md` table, `+0x74`;
   `re-capture-display.md` 128) - with the soldier **facing it**: the unit vector from the soldier to the post
   dotted with the soldier's facing (*(decompiler detail omitted)*) `> 0.15` (B2:109226), and `FUN_00356880(post, team) != 0`:
   the post's enabled bit `+0xb40 & 1`, owned by the soldier's team, with spawn means (`+0x90 != 0` or a positive
   count at `+0xb3c` +0x124) (B2:`FUN_00356880`, the spawn test the AI shares). So an enemy or neutral post never
   qualifies, and neither does a soldier merely standing in a post he cannot spawn at.
2. **Other same-team objects** (B2:109236-109289): a 30 m gather (`FUN_00161a98(0x41f00000 = 30.0, ..., mask
   0x10)`), each object within its class's `+0x338` distance, facing it (`> 0.15`), same team (`+0x268` bits).
   The class field is the object class's `SwitchClassRadius` (checked against the original game; BF2's baked class
   data authors it only on post classes: `com_bldg_controlzone`, its `_CTF` and untakeable variants 5 m,
   `rep_bldg_forwardcenter` 15 m, Endor's `end_bldg_bunker_controlpanel` 8 m, Hoth's `hoth_bldg_shieldgenerator`
   25 m). Unlike scan 1 it needs only the same team, not spawn means. [H] which objects the gather's mask 0x10
   takes; posts are assumed. A class that authors no `SwitchClassRadius` keeps the class constructor's default
   -1.0 (B2:186493 in `FUN_00269d98`, `+0x338 = 0xbf800000`; a base copies it at B2:186590), which no distance
   is under: such an object gives no reach. Built for Hunt only (issue #161, §5).

"Near" is the post's own capture region (scan 1) or its class's `SwitchClassRadius` (scan 2). The facing cosine
0.15 is about 81 deg.

## 3. The accept: immediate, in place [V]

State 4's X runs the same code as state 3 (B2:393063-393132): the class refusal `FUN_00364290`, then
`FUN_0034dac0(member, class)` (B2:393075), state 5, and `FUN_0039b748` (the member's team / class / post record).
`FUN_0034dac0` (B2:344446-) with a live unit (`member+0x148 != 0`):

- copies the old unit's matrix (its vtable +0x30), B2:344510-344530;
- reads the old soldier's **health fraction** `+0x170 / +0x174` (*(decompiler detail omitted)*), B2:344555;
- for each weapon (passing a filter on the member's `+0x1a0` and the weapon class's `+0x1e2` / `+0x1e4`, [H] the
  usable ones) averages `(word +0xc + word +0x10) / word +8` (the rounds held over the weapon's total, [H] per
  `re-ammo-reload.md` §"Original rule") into one fraction per channel (`vtable +300` returns 0 or 1), B2:344560-344610;
- removes the old unit (its vtable +8 with 3: not a death, no kill event), then sets the member's class
  (`FUN_0034f1f8`) and spawns the new one **at the saved matrix** through the member spawn `FUN_0034e1f0`;
- sets the new soldier's health to fraction x its max (`FUN_001df3d8`), and each weapon's ammo to its channel's
  fraction x its total (`FUN_0052a790`, then `FUN_00303958` loads the clip), B2:344640-344690. A hero class
  (bit 33 of class `+0x330`) takes 1.0 for both.

So the change is immediate, not at the next respawn, and the new soldier stands where the old one stood.

## 4. BF1 [D]

BF1's spawn screen (`FUN_002df2b0`, `re-spawn-flow.md` §8) has states 0, 2, 3, 4 = team, spawn point, unit, spawn
asked: no screen over a live unit was found. The class change is BF2's feature; the port gives it to BF1's maps too
(maintainer, `analysis/plan/FEATURE_SET.md` 2026-10-27, issue #28).

## 5. The port and its deviations

Built in `engine/crates/core/src/sim/spawn/class_change.rs` (stage `SpawnStage::Alive` = the PS2's state 4):

| | PS2 (BF2) | port |
|---|---|---|
| where | in the capture region of a post his team can spawn at, facing it (cos > 0.15) | the same (`Sim::class_change_post`, `CLASS_CHANGE_FACING`) |
| Hunt | the same two scans; BF2's Hunt posts have no capture region, so scan 2's 5 m holds | **issue #161:** no capture count (Hunt's posts keep none): an authored region of a post his team can spawn at, or within the post class's `SwitchClassRadius` of a post his team owns (`Sim::hunt_class_change_near`); a BF2 post class without one has no reach, as in the original; **named deviation:** every BF1 post (BF1 has no class change and no radius in its data) takes 5 m. Hero Assault keeps none |
| button | use (△) | the use bit `ACTION` (pad △, keyboard E); E also carries △'s back |
| who | a human with a viewport | a `Controller::Player` slot; bots never |
| maps | BF2's | **every map of both games** (named deviation, maintainer 2026-10-27) |
| when | at once, at the old matrix | at once, the old position, yaw and pitch; a new generation, `Event::Respawn` |
| tickets | the unit is removed, not killed | no ticket, no death, the alive count unchanged |
| health | fraction carried | fraction carried (the maintainer's "health carries over", BF2's rule) |
| ammo | per-channel fraction carried to the new class | **named deviation (maintainer, issue #28):** a class not yet played this life is full; a class played this life comes back with exactly the rounds (clip, spare per weapon slot) it had when he left it; no refill by swapping back |
| death | - | **named deviation:** a death forgets every class's ammo (`Sim::class_ammo`) and closes a live class screen |
| back | △ returns to play (state 5) | the same; no swap |
| same class | rebuilds | closes the screen, no swap |
| vehicle in reach | not read | a vehicle the use press can enter takes it first (PROVISIONAL) |
| kills | the member's | carried; the career count (#19) credits the class in use when each kill lands |

## 6. Open

- The second scan (§2.2, `SwitchClassRadius`) is built for Hunt only; Conquest still reads scan 1 alone
  (its posts' regions are wider than 5 m, but a command walker's or a forward center's radius is not read).
- The PS2's prompt "△ CHANGE SOLDIER CLASS" (hint 5) is not drawn: the port has no tip system yet.
- Which input bits `+0x2c0 & 3` are, and the PS2's order between entering a vehicle and opening the screen, are
  not read: [H].
