# RE: jet packs, jump packs and the Jedi double jump - both games (card JETP)

The PS2's jet state from code and ELF words, for a later build card: the jump-jet (dark trooper), the hover jet (jet
troopers, Geonosians, Jango / Boba, Vader / the Emperor), BF2's Jedi double jump (the same code path), fuel, the HUD
bar, effects and sounds, the bots' use. Read with `re-stance-roll.md` (stance machines, 4.3a-4.3d the jump and air
movers), `re-soldier-ai.md` 4.7 (bots) and `unit-audit-bf1.md` / `unit-audit-bf2.md` 3.6 (class values).

> **Status: UNAUDITED; section 16 holds the live PS2 runs, everything before it is code reading.** Every [V] is a decomp line checked with `re_check.py fn` and, where the
> number matters, the ELF instruction words; [D] is arithmetic or a reading of a field's meaning; [H] is a guess. No
> PCSX2 trial was taken for this note and no independent verifier has re-read it yet (RULES "run the verifier on RE
> claims BEFORE writing docs" - owed: build plan step 9). The feel numbers in 6 are [D] from the formulas;
> section 11 lists the captures that would turn them into [V-PS2].

Tags: B1 = `analysis/SWBFRONT.ELF-decomp.c` line, B2 = `analysis/SWBFRNT2.ELF-decomp.c` line; ELF VAs are
`extracted/bf1/SWBFRONT.ELF` / `extracted/bf2/SWBFRNT2.ELF`. Soldier object = s, its class = c (BF2 c = s+0x4a4,
BF1 c = s+0x958).

## 1. Summary

| | BF1 | BF2 |
|---|---|---|
| Trigger | jump button pressed while in the jump (4) or fall (6) stance, not thrown (B1:117619) | jump button press-edge while in the jump (4) or fall (8) stance (B2:160615-160618); bots call the jet slot directly |
| Stances | one jet stance, 7; hover or not by c+0x190c (`JetType = hover`) | 6 = jump-jet, 7 = hover (c+0x778 bit 3 = `JetType hover`) |
| Kick on start | v += normalize(right x (-0.1 strafe) + fwd x max(stick_fwd, -0.1) + (0, 1.1, 0)) x JetJump - identical in both games | same |
| Fuel cost to start | fuel >= JetFuelMinBorder, then fuel -= JetFuelInitialCost (floor 0) | same, plus JetEnergyDrain from the energy bar |
| Fuel recharge | every update, in EVERY stance, jet included: += JetFuelRechargeRate x dt, cap 1 | every update OUTSIDE stances 6 / 7 |
| Jump-jet (not hover) | no drain; stick x 0.7 to the ground mover's target, accel x 1.2 x 0.3; gravity 18; vy += JetPush x dt; cannot be cancelled | drains JetFuelCost / s; vy += JetPush x dt; the air mover with the stick x JetAcceleration (drag, gravity 18); a second tap ends it |
| Hover | vertical speed pulled to 0 (gravity cancelled), climb = sin(aim pitch) x JetPush x 4 x stick; horizontal targets 2 x stick x speeds; drains JetFuelCost / s; a tap ends it | own mover: no gravity, all velocity x exp(-2 dt), velocity along the AIM (pitch doubled + lift) and the side approach the ControlSpeed jet row's targets at JetAcceleration; drains JetFuelCost / s; a tap ends it |
| End | fuel empty or tap (hover) -> fall (6); landing -> stand | fuel empty or tap -> fall (8); landing -> stand, 0.3 s light lock |
| Jedi | - | a lightsaber class's "jet" (not hover) is a second JUMP: v set to the stick's run speeds, vy = 1.5 x jump speed |

## 2. The class fields (setters, defaults, every class that authors them)

### 2.1 Keys and fields [V]

| key (hash) | BF2 field, setter line | BF1 field, setter line | engine default (both) |
|---|---|---|---|
| JetJump (0xc4b2cb20) | c+0x870, B2:170967 | c+0x18d8, B1:123562 | 7.0 |
| JetPush (0xf0b75e90) | c+0x874, B2:170830 | c+0x18dc, B1:123503 | 14.0 |
| JetAcceleration (0x8aac2908) | c+0x878, B2:171315 | **none** | 20.0 (BF2) |
| JetFuelRechargeRate (0x0825c1bf) | c+0x87c, B2:171491 | c+0x18e0, B1:122886 | 0 |
| JetFuelCost (0x36d9de4d) | c+0x880, B2:171877 | c+0x18e4, B1:123053 | 0.2 |
| JetFuelInitialCost (0x1ebb5841) | c+0x884, B2:171542 | c+0x18e8, B1:122911 | 0 |
| JetFuelMinBorder (0xf8a37c8a) | c+0x888, B2:170913 | c+0x18ec, B1:123533 | 0.1 |
| JetEnergyDrain (0x0c2425ea) | c+0x88c, B2:171527 | **none** | 0 |
| JetEffect (0x69c4d0dd) | c+0x890 (an effect class looked up by name), B2:171855 | c+0x18f0, B1:123317 | none |
| JetType (0x91fabb5c) = `hover` (0x70ceab2d) | c+0x778 \|= 8, B2:171303-171311 (any other value: ignored) | c+0x190c = 1, B1:123384-123388 | not hover |
| ControlSpeed `jet` row (0x47bf39c1, stance 0xd7e330f4 -> 6) | fwd c+0x794, strafe c+0x7b4, turn c+0x7d4 (`re-stance-roll.md` 4.3) | **none** | 0.3 / 0.3 / 1.0 |
| EngineSound / TurnOnSound / TurnOffSound | the class's engine-sound block at c+0x5b0 (reader `FUN_0044da60`, B2:510570 / 510584) | block at c+0x16f0 (`FUN_00366ea0`) | none |
| JetDuration (0x10e3903c) | **read by nothing** | **read by nothing** | - |

BF2 defaults: `FUN_00250488` stores them with FPU registers the decomp drops; read from the instructions
(`eedis.py bf2 0x250488 0x250dc8`): 0x250c14 `swc1 $f0 2160(s2)` with $f0 = 0x40e00000 (7.0, set at 0x250bf0),
0x250c3c `$f1 -> 2164` = 0x41600000 (14.0, 0x250bdc), 0x250c44 `$f25 -> 2168` = 0x41a00000 (20.0, 0x25089c),
0x250c7c `$f27 -> 2176` = 0x3e4ccccd (0.2), 0x250c34 `$f31 -> 2184` = 0x3dcccccd (0.1), zero to 2172 / 2180 / 2188 /
2192 (recharge, initial, drain, effect). c+0x778 is set to `(x & ...0056) | 0x46` (B2:169380): bits 1, 2 and 6 on
by default, bit 3 (hover) off. BF1 `FUN_0020d238`: B1:121980 (JetJump = 0x40e00000, 7.0 via B1:121806), B1:121984-121995.

Absence receipts:

```
$ re_check.py absent b2 0x10e3903c JetDuration --elf --src extracted/bf1/SWBFRONT.ELF --src extracted/bf2/SWBFRNT2.ELF --card JETP
'0x10e3903c' in B1: 0 hit(s)   '0x10e3903c' in B2: 0 hit(s)   'JetDuration' in B1: 0   in B2: 0
raw extracted/bf1/SWBFRONT.ELF: 0 hit(s)   raw extracted/bf2/SWBFRNT2.ELF: 0 hit(s) (incl. lui/ori builds)
[receipt:4a2701ba]
$ re_check.py absent b2 0x36d9de4d --elf --card JETP        # control: JetFuelCost, a key the code does read
0x36d9de4d lui/ori: SWBFRONT.ELF VA 0x20f53c; SWBFRNT2.ELF VA 0x252f18     [receipt:4dabc45f]
$ re_check.py absent b1 0x8aac2908 0xc2425ea 0x47bf39c1 --elf --card JETP   # JetAcceleration, JetEnergyDrain, ControlSpeed
'0x8aac2908' in B1: 0   in B2: 1 (FUN_00252bb0 171315)
'0xc2425ea'  in B1: 0   in B2: 1 (FUN_00252bb0 171527)
'0x47bf39c1' in B1: 0   in B2: 1 (FUN_00252bb0 172005)
raw SWBFRONT.ELF: 0 hit(s); raw SWBFRNT2.ELF: 3 hit(s) - lui/ori at 0x25321c, 0x252d04, 0x252fec   [receipt:4fad90ae]
```

So **JetDuration does nothing in either game** (the Geonosians' 5.0 / 30.0 / 6000.0 are inert), and **BF1 has no
JetAcceleration, JetEnergyDrain or ControlSpeed** - its jet speeds are hard-coded (4.2).

### 2.2 Every class that authors a jet key (the disc's own class chunks) [V disc]

Census: `python3 scripts/tools/jet_census.py bf1` / `bf2` (every `entc` chunk of every LVL under
`extracted/<game>/lvl`, keys named by FNV; the chunk walk is `lead_tmp/veh0/entc.py`'s): 4 BF1 classes, 31 BF2.
The baked `engine/assets_baked/classes/*.ron` agree for every class they hold; the Geonosians are local classes the
bake does not carry (`bf1_geo1/classes.ron`, `bf2_geo1/classes.ron` hold no `geo_inf_geonosian`).

BF1 (`SIDE/*.LVL`, BASE soldier, no inheritance):

| class | JetJump | JetPush | hover | cost / initial / recharge / border | MaxSpeed / strafe / Acceleraton | JetEffect | EngineSound / TurnOn / TurnOff |
|---|---|---|---|---|---|---|---|
| imp_inf_dark_trooper | 21 | 0 | no | 0 / 0.5 / 0.025 / 0.49 | 6.0 / 4.5 / 70 | darktrooper_jetpack | rep_inf_Jetpack_engine_parameterized / rep_weap_jetpack_turnon / rep_weap_jetpack_turnoff |
| imp_inf_dark_troopersnow | 21 | 0 | no | 0 / 0.4 / 0.025 / 0.4 | 6.0 / 4.5 / 70 | darktrooper_jetpack | same |
| rep_inf_jet_trooper | 5 | 8 | yes | 0.2 / 0.25 / 0.1 / 0.24 | 6.5 / 4.5 / 70 | jetpack | same |
| geo_inf_geonosian (GEO1 locals) | 8 | 8 | yes | 0.1 / 0.2 / 0.03 / 0.3 | 6.0 / 5.0 / 30 | geo_wingsfly | geo_inf_fly_parameterized / - / - |

BF2 (BASE chains resolved; `com_inf_default`'s jet row is 0.30 / 0.30 / 1.00, `com_hero_default`'s 2.0 / 1.25 /
1.25, `com_jedi_default`'s 1.50 / 1.25 / 1.25):

| class (chain) | JetJump / Push / Accel | hover | cost / initial / recharge / border | energy drain | jet row | JetEffect | sounds |
|---|---|---|---|---|---|---|---|
| imp_inf_dark_trooper (`imp_inf_default_dark_trooper`) | 21 / 0 / 10 | no | 0 / 0.5 / 0.075 / 0.49 | 0 | 0.3 / 0.3 / 1.0 | imp_sfx_jetpack_darktrooper | rep_inf_Jetpack_engine_parameterized, turnon, turnoff |
| rep_inf_ep2/ep3_jettrooper (`rep_inf_default_jettrooper`) | 5 / 8 / 30 | yes | 0.16 / 0.25 / 0.12 / 0.24 | 0 | 1.5 / 1.25 / 1.25 | rep_sfx_jetpack | same |
| rep_inf_ep2_jettrooper_sniper | 0 / 0 / 0 | yes | 1.0 / 1.0 / 0 / **1.04** | 0 | 1.5 / 1.25 / 1.25 | rep_sfx_jetpack | same (never jets: border > 1) |
| cis_hero_jangofett / imp_hero_bobafett (`com_hero_default`) | 7 / 8 / 30 | yes | 0.15 / 0.25 / 0.12 / 0.24 | 0 | 2.0 / 1.25 / 1.25 | cis_sfx_jetpack / imp_sfx_jetpack | same |
| geo_inf_geonosian (GEO1 locals; BASE com_inf_default) | 8 / 8 / (20) | yes | 0.1 / 0.2 / 0.1 / 0.3 | 0 | 2.0 / 1.25 / 1.25 | geo_wingsfly | geo_inf_fly_parameterized |
| geo_inf_agro_geonosian (BASE soldier) | 8 / 8 / (20) | yes | 0.1 / 0.2 / 0.03 / 0.3 | 0 | 1.5 / 1.25 / 1.25 | geo_wingsfly | geo_inf_fly_parameterized |
| imp_hero_darthvader, imp_hero_emperor (`com_jedi_default`) | 7 / 7 / 20 | yes | 0.3 / 0.25 / 0.9 / **1.0** | 30 | 1.5 / 1.25 / 1.25 | - | - |
| every other Jedi hero, jed_* (`com_jedi_default`) | 10 / 0 / 10 | no | 0 / 0 / 0 / 0 | 40 (Yoda 30) | 1.5 / 1.25 / 1.25 | - | - |
| snw_inf_wampa (`com_jedi_default`) | 0 / 0 / 0 | no | defaults (recharge 0, border 0.1) | 0 | 0.3 / 0.3 / 1.0 | - | - |

(20) = not authored, the engine default. Geonosian MaxSpeed 6.0 / 5.0, agro 8.0 / 6.0; Vader / Emperor 6.5 / 6.5.

## 3. Starting a jet [V]

### 3.1 BF2 - `FUN_002476a0` (controllable vtable slot +0xbc; thunk 0x255c08 -> `jal 0x2476a0`)

```
$ re_check.py word b2 0x61ea54 2 --u32       # soldier controllable vtable (0x61e998) + 0xbc / + 0xc0
0x61ea54: u32 0x00255c08    0x61ea58: u32 0x00255c28     (0x255c10: jal 0x2476a0 / 0x255c30: jal 0x247a80)
```

1. Refused unless the stance is 4 (jump) or 8 (fall) (B2:162471).
2. Refused while fuel s+0xb90 < JetFuelMinBorder (B2:162475; `c.lt.s` at 0x2476e4: equal passes).
3. Refused when s+0x328 is set and its +0x18c is non-zero (B2:162478-162485) [H: a seat / attached object].
4. `FUN_001e2010(JetEnergyDrain, energy bar s+0xab0)` (B2:162487): refused only if the bar is already depleted; a
   short bar is overdrawn to its floor and marked depleted (`re-stance-roll.md` 4.3d; the floor is minus the largest
   of the jump / sprint-jump / roll costs and JetEnergyDrain).
5. fuel -= JetFuelInitialCost, floored at 0 (B2:162492-162496).
6. The kick (B2:162497-162549, instructions 0x247750-0x247874):
   `dir = normalize( R x (-strafe x 0.1) + F x max(fwd, -0.1) + (0, 1.1, 0) )`, R = body right row s+0x120, F =
   body forward row s+0x140 (level), fwd / strafe = the update's stick s+0x2dc / s+0x2e0. For a soldier whose
   s+0x330 < 0 (a bot) the AI controller's vector (+0x4f0) REPLACES dir when its squared length > 0 - not normalised
   [V: no rsqrt on that path, 0x247814-0x247848]. Then `v += dir x JetJump` (an `add`, 0x24786c).
   ```
   $ re_check.py word b2 0x24775c 2 --u32    # lui/ori 0x3dcccccd = 0.1  (strafe scale)
   $ re_check.py word b2 0x247784 2 --u32    # lui/ori 0xbdcccccd = -0.1 (the fwd floor, max.s at 0x2477a8)
   $ re_check.py word b2 0x247794 2 --u32    # lui/ori 0x3f8ccccd = 1.1  (the lift, stored as y at 0x2477c4)
   ```
7. Which state:
   - s+0x4e0 bit 0x10000 set and c+0x778 bit 3 clear -> **the Jedi double jump** (3.3);
   - otherwise stance **7** when c+0x778 bit 3 (hover), else **6** (B2:162605-162609).
8. The jet engine sound starts if not already sounding: `FUN_0044cc28(health ratio s+0x170 / s+0x174, s+0x420,
   c+0x5b0, pos, vel)` (B2:162611-162623; the block's start sound at +4 [D: TurnOnSound], then the engine loop).
9. `FUN_002ae728(s+0x324, 3)` [H: tells the controller the jet started]. Returns 1.

Where the press comes from (`FUN_00240bf8`, cases 4 / 8, B2:160563-160650): after the air mover and the fly-height
term (5.3), while `stance timer <= 0.3 or the floor bit (s+0xa74 & 2) is clear` (otherwise it lands, B2:160614), and
only when bit 3 of s+0x228 is set and the stance is not 9 (thrown): the jump word's press edge `(s+0x2b4 & 3) == 3`
calls slot +0xbc (B2:160615-160618). Not inside the player-only block: the button word is the pad's for a human.
Then a hint counter `FUN_00394428(DAT_00661358, n, player)` (the tutorial hint object - `re-powerup-station.md`,
`re-capture-display.md`): n = 0x14 when the held weapon's slot +0x5c is true, else n = 1 when JetFuelRechargeRate > 0
(B2:160619-160630) [D: 0x14 = the "double jump" hint, 1 = the "jet pack" hint].

**Bots** do not press: the AI mover `FUN_00249f80` jumps (slot +0xac, B2:164644, sets its controller flag 0x20), and
on a later update in stance 4 with the lock s+0x4e4 <= 0 calls slot +0xbc itself (B2:164630-164633); controller bit 6
makes it call slot +0xc0, jet off (B2:164623-164626). While a bot hovers (stance 7) the same function turns its body
pitch toward the controller's wanted height (+0x4d4): rate = 0.35 x (wanted - current), capped at 30 x dt
(B2:164661-164675) [D: wanted height; the hover flies along the aim, 4.2].

### 3.2 BF1 - `FUN_00209540` (called directly from the update, B1:117620)

Same kick, same constants (B1:118755-118807), with s+0x200 / s+0x220 the right / forward rows, s+0x300 / s+0x304 the
stick, the bot override from s+0x3f0 when s+0x388 < 0 (B1:118787-118797), JetJump c+0x18d8. Gates: only fuel s+0xc6c
>= JetFuelMinBorder (B1:118746); NO stance test inside (the caller's), no energy. fuel -= JetFuelInitialCost floored
at 0 (B1:118750-118754). Always stance **7** (B1:118808). Engine sound starts as in BF2 (`FUN_00366550` /
`FUN_003660b8`, class block c+0x16f0, B1:118809-118823).

The caller: the jump / thrown / fall case (4, 5, 6) of `FUN_00205cc8`, after its mover and the fly-height term, when
not landing this update (landing = stance timer > 0.3 and the floor bit s+0xbfd & 2), not thrown (stance 5), and the
jump byte s+0x2ee == 1 (pressed; the same byte the stand case's jump reads, B1:117433) (B1:117586-117621).

### 3.3 BF2's Jedi double jump (the not-hover branch with s+0x4e0 bit 0x10000) [V code; D meaning]

s+0x4e0 bit 0x10000 is set at spawn when a weapon on channel 0 answers true to its slot +0x5c (B2:149641-149645) and
cleared before the loop (B2:149594) [H: the slot is "melee / lightsaber"; every class that fields one is a Jedi or
Sith]. In `FUN_002476a0` (B2:162553-162600), after the kick:

- stance 4 (jump) again;
- v = F x (fwd x (fwd > 0 ? MaxSpeed : MaxStrafeSpeed)) + R x (-strafe x MaxStrafeSpeed): the kick's horizontal part
  is thrown away;
- vy = (vy + kick) < J / 2 ? (vy + kick) + J : 1.5 x J, J = the class's jump speed c+0x764 = sqrt(36 x JumpHeight);
- if s+0x7f0 (the soldier's current combo / flip [H]) is set, `FUN_0027b9e0(fwd, -strafe, s+0x7f0, stance==4 && timer>=0.5)`
  must accept it or the jump stays but the timer reset is skipped; stance timer = 0.

Fuel is irrelevant for these classes (border 0, initial 0, recharge 0, spawn fuel 0); the energy bar pays 40 (Yoda
30) and a depleted bar refuses. A third press in the air takes the same path again while the bar is not depleted
[D]. Vader and the Emperor are `JetType hover`, so they skip this and hover (stance 7).

## 4. The jet states, per update [V]

### 4.1 BF2 stance 6 - the jump-jet (`FUN_00240bf8` case 6/7, B2:160685-160732)

1. Posture bits cleared; fuel -= JetFuelCost x dt. Below 0: fuel = 0 and the jet ends.
2. Else the jump word's press edge ends it (a TAP, not a release: B2:160694).
3. Ending = slot +0xc0 (`FUN_00247a80`: stance 6 / 7 -> 8, fall). If s+0x4e0 bit 0x10000, slot +0xbc again at once
   (a saber class re-jumps; unreachable for jet troopers).
4. Not ending: turn rate x ControlSpeed jet turn (c+0x7d4) for a human only (`-1 < s+0x330`, B2:160704-160706), then
   **vy += JetPush x dt** (B2:160721-160722) and the air mover `FUN_00248560(dt, turn, fwd x JetAcceleration,
   -strafe x JetAcceleration, s)` (B2:160723-160724): `v *= exp(-0.012 |v| dt)`, `v += F x fwd x JetAcceleration x dt
   + R x (-strafe) x JetAcceleration x dt`, `vy -= 18 dt` unless on the floor, then the move (`re-stance-roll.md`
   4.3a, measured there for the jump).
5. The fly-height term (5.3), then landing if the floor bit is set (B2:160731 -> `FUN_00246828` -> stand-up
   `FUN_00246850`). No 0.3 s guard here, unlike the jump.

### 4.2 BF2 stance 7 - the hover (same case; mover `FUN_00248898`, B2:163384-163597)

Steps 1-3 and the turn as 4.1. Then `FUN_00248898(dt, fwdT, sideT, turn, JetAcceleration, s)` with
`fwdT = fwd x (fwd > 0 ? MaxSpeed : MaxStrafeSpeed) x ControlSpeed.jet.fwd (c+0x794)` and
`sideT = -strafe x MaxStrafeSpeed x ControlSpeed.jet.strafe (c+0x7b4)` (B2:160709-160718). Inside (instructions
0x2488d4-0x248a8c):

1. On the floor: take off the velocity's part into the floor normal s+0x4b0 when negative.
2. `v *= exp(-2 dt)` on all three axes (`FUN_0052b1b8` = expf, the same call AIRC replayed; `lui 0xc000` at 0x24893c).
3. The flight axis `D = normalize(a.x, 2 x (a.y + 0.08), a.z)`, a = the aim direction s+0x350 (pitch + kick, written by
   the aim update `FUN_0023ecb8` AFTER the movement - so last update's aim; `re-aim-assist.md` 10.2). 0x3da3d70a =
   0.08 at 0x248958, `add.s` then `add.s f1, f1, f1` (0x248970, 0x2489a0).
4. Per axis, clamp toward the target: `v += D x clamp(fwdT - v.D, +-JetAcceleration x dt)` then
   `v += R x clamp(sideT - v.R, +-JetAcceleration x dt)` (R = s+0x120, level).
5. **No gravity** and no JetPush in this mover. pos += v dt; the body's move (vtable +0x10) and turn (+0x14).
6. The fly-height term, landing as 4.1.

```
$ re_check.py word b2 0x24893c 1 --u32    0x24893c: u32 0x3c01c000   (lui at, 0xc000: -2.0)
$ re_check.py word b2 0x248958 2 --u32    0x3c013da3 0x3421d70a     (0.08)
```

### 4.3 BF1 stance 7 (`FUN_00205cc8` case 7, B1:117623-117691)

Not hover (c+0x190c == 0, the dark trooper): stick fwd and strafe x 0.7, vy += JetPush x dt (B1:117627-117631), no
fuel drain, no cancel. Hover (c+0x190c == 1, jet trooper, Geonosian), B1:117632-117670, instructions 0x207bf4-0x207d04:

- strafe x 2;
- vy += ((2.0 - vy) x 2.5 + 13.0) x dt, then vy += (a . (0,1,0)) x JetPush x fwd x 4 x dt, a = aim direction s+0x3b0
  (BF1 K+0x110, `re-aim-assist.md` 10.2);
- fwd = fwd x (a . F) x 2, F = body forward row s+0x220 (so x cos(pitch));
- fuel -= JetFuelCost x dt; below 0 -> 0 and the jet ends; else the jump byte == 1 ends it.

Then (not ending) the shared mover `FUN_002098b8(dt, fwd, strafe, turn, brake, s)` with brake 0.3 for hover, 0 for the
jump-jet (B1:117676). In stance 7 the mover (B1:119197-119298): first brakes each horizontal axis toward 0 by
Acceleraton x dt x 0.6 x brake (0x5178a4 = 0.6; in stance 7 even off the floor, B1:119217), then raises it toward
`stick x MaxSpeed` / `stick x MaxStrafeSpeed` by **Acceleraton x 1.2 x 0.3** x dt (B1:119224-119225; never lowers),
then gravity -18 x dt off the floor (not -13.5: that is stances 5 / 9). Ending -> stance 6 (fall). Fly-height term,
then landing on the floor bit with no timer guard (B1:117687-117689 -> `FUN_00209018`).

So BF1's hover holds height: per update dvy = (13 + 2.5 x (2 - vy) - 18) dt = -2.5 vy dt, plus the aim climb.

```
$ re_check.py word b1 0x207bfc 1 --u32    0x3c014000 (2.0)
$ re_check.py word b1 0x207c04 1 --u32    0x3c014020 (2.5)
$ re_check.py word b1 0x207c10 1 --u32    0x3c014150 (13.0)
$ re_check.py word b1 0x207c34 1 --u32    0x3c014080 (4.0)
$ re_check.py word b1 0x207d14 2 --u32    0x3c013f33 0x34213333 (0.7, the jump-jet's stick)
$ re_check.py word b1 0x5178a4 3 --f32    0.6, 0.4, 0.75 (the mover's brake share)
```

### 4.4 Landing out of a jet [V]

BF2 `FUN_00246850` from stance 6 / 7: always the light lock, s+0x4e4 = 0.3 with flag 0x800 (B2:161869-161872: the
`a local - 6U < 2` test skips the air-time rule a jump uses), landing sound bank +0x50, anim event 5, s+0xa76 |= 0x20.
BF1 `FUN_00209018` from stance 7: the lock s+0xc0c is set to 0.5 only when the stance timer > 2 s, else left as it was
(10.6 is stance 4's only, B1:118505-118517).

## 5. Fuel, energy, ceiling [V]

### 5.1 Fuel

| | BF1 (s+0xc6c) | BF2 (s+0xb90) |
|---|---|---|
| range | 0..1 | 0..1 |
| spawn | 0.5 if JetFuelRechargeRate > 0, else 0 (B1:111797-111803; the temp s+0xc20 zeroed at B1:111748) | same rule (B2:149862-149867; ELF 0x235628 `sw zero 2708`, 0x2356fc `c.lt.s`, 0x23570c `lui 0x3f00`) |
| recharge | += rate x dt every update, any stance, cap 1 (B1:117346-117350, unconditional: 0x207280-0x2072a8) | += rate x dt unless stance 6 / 7, cap 1 (B2:160108-160116); also in `FUN_0024de60` when the controllable's slot +0x18 is 0 (B2:167436-167441) [H: a soldier not driven - seated / dead] |
| drain in the jet | hover only: JetFuelCost / s | both jet stances: JetFuelCost / s |

Field census (who touches fuel at all):

```
$ re_check.py absent b2 0xb90 "[0x2e4]" --card JETP     [receipt:e6af8df0]
'0xb90' in B2: FUN_002476a0 (jet on), FUN_00250488 (class ctor - another object), FUN_002c6970, FUN_002d9890,
  FUN_002d99c0, FUN_002dc448 (bots), FUN_003b4e70 (HUD), FUN_001f0578/0cf8/0f98/12c0, 0040df10, 00418ee0, 004194e8
  (other objects' +0xb90)
'[0x2e4]' in B2: FUN_00234a68 (spawn), FUN_00240bf8 (tick), FUN_0024de60 (idle recharge), FUN_002dc0b8 (bot)
$ re_check.py absent b1 0xc6c "[0x31b]" 0x18dc 0x18e0 0x18e4 0x190c --card JETP     [receipt:2e28a90f]
'0xc6c' in B1: FUN_00209540, FUN_00250450 / FUN_00258f30 / FUN_00259058 (bots), FUN_002c3690 (HUD) (+2 unrelated)
'[0x31b]' in B1: FUN_001ff1e8 (spawn), FUN_00205cc8 (tick), FUN_0020c880 (idle recharge), FUN_0032e7e0
'0x190c' in B1: FUN_00205cc8 117627, FUN_0020d238 / FUN_0020d940 (ctor, copy), FUN_0020f2e0 (setter), FUN_00259058 (bot)
```

### 5.2 Energy (BF2 only)

JetEnergyDrain is paid from the stamina bar at the start (3.1 step 4); in stances 4 and up the bar's rate is 0
(`re-stance-roll.md` 4.3d), so a hovering Vader neither regains nor spends energy until he lands.

### 5.3 The fly-height ceiling

Both jet stances (and BF2 4 / 8 / 9, BF1 4 / 5 / 6) after their mover: if `ceiling - feet.y < 5`:
`vy += ((ceiling - feet.y) - 5) x dt x 10` (BF2 B2:160726-160730, BF1 B1:117678-117681; `re-stance-roll.md` 4.3b).
A spring toward 5 m under the ceiling, applied after the update's move (so it bends the next one). Ceiling: BF2
DAT_00697e54 (Lua `SetMaxPlayerFlyHeight`, B2:32250; applies to bots too here), BF1 DAT_005873b0 (`SetMaxFlyHeight`,
B1:26984); default 200.0. BF2 conquest values (`re-boundary.md` T3b [D]): COR1 25, DAG1 20, DEA1 72, END1 43, FEL1 53,
GEO1 50, HOT1 70, KAM1 102, MUS1 90 / 84.16, MYG1 20, NAB2 25, POL1 55, TAT3 90, UTA1 29.5, YAV1 14; KAS2, TAT2 200.
BF1 per-mission values: not read (11.7).

## 6. What each class does, in numbers [D from 3-5, dt = 1/30]

Kick directions: no stick = straight up x JetJump; full forward = (up 0.740, forward 0.673) x JetJump; full back =
(0.996, -0.090); full side = 0.09 sideways only.

- **BF2 dark trooper.** Tap jump, tap again in the air: +21 m/s straight up, or +15.5 up / +14.1 forward with the stick.
  From a jump's second update vy ~ 8.0 - 0.6 + 15.5 = 22.9 m/s: apex about 14.6 m over that point before drag
  (exp(-0.012 |v| dt) takes about 1.2 % a second per m/s of speed), stick accel 10 m/s^2 in the air, gravity on.
  Tap again: falls (stance 8, the jump's 2.1 m/s^2 stick). Fuel 1.0 -> 0.5 -> 0.0: two leaps from full, one from
  spawn (0.5 >= 0.49), then 0.49 / 0.075 = 6.5 s to the next.
- **BF2 jet trooper.** Kick +5 m/s up (3.7 up / 3.4 forward with the stick). Hover: no gravity; with the stick
  released all speed dies by e^-2t (86 % gone in 1 s). Full stick: 10.5 m/s along the aim (7 x 1.5), backing 7.875 m/s
  (5.25 x 1.5; corrected from the caller B2:160710–160717), strafe 6.6 m/s; the speed along D moves at most 30 x dt = 1.0 m/s per update. Level aim climbs:
  D.y = 0.16 / 1.0127 -> +1.66 m/s at full forward; aim 4.6 deg down flies level. Turn x 1.25 (players). Fuel:
  start costs 0.25, 0.16 / s -> 4.7 s from full, 1.6 s from the spawn's 0.5; empty to the border 0.24 in 2.0 s,
  to full 8.3 s. (The port flies every jet trooper on BF1's flight and BF2's refill instead since 2026-11-10:
  7.5 s / 2.5 s, refill 0.12 / s, 13.)
- **Jango / Boba**: as the jet trooper with kick 7, forward 2.0 x MaxSpeed, cost 0.15 / s (5.0 s from full). (The
  port flies them 8.5 s from full at 0.0882 / s since 2026-11-10, refill 0.12 / s, 13.)
- **BF2 Geonosian**: kick 8, hover at 2.0 x 6.0 = 12 m/s forward, strafe 6.25, JetAcceleration 20 (default), cost
  0.1 / s, start 0.2, border 0.3 -> 8 s from full.
- **Vader / Emperor**: need a FULL bar (border 1.0); spawn 0.5 -> wait 0.56 s; each hover costs 30 energy and 0.25 fuel,
  lasts 0.75 / 0.3 = 2.5 s, kick 7, forward 1.5 x 6.5 = 9.75 m/s, accel 20; refills in 1.1 s.
- **Other Jedi**: the double jump - vy = 1.5 x sqrt(36 x 3.5) = 16.8 m/s (Luke, Obi-Wan, ...: JumpHeight 3.5) and the
  run speed in the stick's direction; 40 energy each.
- **BF1 dark trooper**: the same 21 m/s kick, then a ballistic arc with gravity 18 and the stick at 0.7 x 6.0 = 4.2 m/s
  (accel 70 x 1.2 x 0.3 = 25.2 m/s^2, raise-only), no drag, no cancel, lands. Fuel recharge 0.025 / s even in the air:
  two leaps from full, one from spawn, then 19.6 s (snow variant: border 0.4 -> 8 s after the second).
- **BF1 jet trooper**: kick 5; hover holds height (vy relaxes to 0 at 2.5 / s); steady climb 12.8 x sin(pitch) x stick
  m/s; forward target 2 x cos(pitch) x 6.5 = 13 m/s, strafe 9 m/s, reached at 25.2 - 12.6 m/s^2 (the brake works
  against the raise every update). Net fuel 0.2 - 0.1 = 0.1 / s -> 7.5 s from full, 2.5 s from spawn.
- **BF1 Geonosian**: as the BF1 jet trooper with kick 8, Acceleraton 30 (accel 10.8, brake 5.4), 12 / 10 m/s, net fuel
  0.07 / s -> 11.4 s.

**BF1 vs BF2, the differences that a player feels:** BF1's jet trooper hovers longer (recharge during flight), climbs by
pitch with the stick and is faster (13 m/s against 10.5); BF2's hover flies along the view with a gentle built-in
climb, and every move eases (exp drag). BF1's dark trooper cannot cancel and waits 20 s; BF2's can tap out and waits
6.5 s. BF2 adds energy costs and the Jedi double jump.

## 7. The per-update order, PS2 against the port

PS2 BF2 `FUN_00240bf8` (one soldier update): stance timer += dt (B2:160119); fuel recharge outside 6 / 7
(B2:160108-160116); the engine sound's per-update drive (`FUN_0044c650`, speed and acceleration, B2:160122-160198);
the energy rate (B2:160209); the stance switch (4 / 8: air mover -> ceiling -> jet press -> `FUN_002476a0`; 6 / 7:
drain -> cancel or mover -> ceiling -> landing); ... the aim update `FUN_0023ecb8` (B2:161062). The effects and the
engine sound's turn-off run in `FUN_00239bb0` (B2:153393-153449; 8). BF1 `FUN_00205cc8`: recharge (B1:117346) before
the switch, the same switch shape, aim update B1:117775.

The port (`engine/crates/core/src/soldier.rs` `Soldier::step_looking`, called from `sim/step.rs` 523; then the wall
pass and `Soldier::resolve_ground` in `sim/step.rs` 545-640):

| PS2 | port today | where the jet goes |
|---|---|---|
| stance timer, fuel recharge (before the switch) | `time_in_stance += dt` (soldier.rs ~688), energy block (~846) | fuel recharge beside the energy block; BF2: skip in the jet stances |
| turn (per stance factor; jet x c+0x7d4 for humans) | `self.yaw -= turn x ctl[2]` (~703) | `Stance::control()` gets a Jet row (index 6) from the class |
| case 4 / 8: air mover, ceiling, then the press -> jet on | `Stance::Jump \| Fall` returns after `air_steer` + gravity + `pos += vel dt` (~915-931) | after that move and before `return`: the ceiling term, then `jump_edge && jet_on()` (kick added to vel for the NEXT move, like `take_off`) |
| case 6 / 7 | - | new `Stance::Jet` / `Stance::Hover` branch: drain, tap-off -> Fall, else the mover, `pos += vel dt`, ceiling |
| landing in 6 / 7 (floor bit) | Jump / Fall land at the top of the next update (`grounded && !first_air`, ~741) | same place, lock 0.3 (`c.soft_lock`), no first-air guard |
| wall pass / floor contact (after the move) | `sim/step.rs`: `in_air` for Jump / Fall / Thrown | jet stances count as `in_air` (AIR_STEP); `rising` should include them |
| aim update after the move | the port's aim is `yaw` / `pitch` already turned this update | the hover axis wants the aim BEFORE this update's turn (4.2 step 3) [D: one update of lag] |

## 8. Effects, sounds, animation

- **Effect [V]:** BF2 `FUN_00239bb0` (B2:153393-153421): in stance 6 / 7 the class's JetEffect is spawned once (if its
  handle s+0xaec is dead) and attached to the soldier at the hard point `DAT_005fdd48 + 0x11ab0` [H: the jet bone];
  outside 6 / 7 `FUN_00239ad8` kills it (B2:152995-153004) and keeps a second class effect c+0x894 alive (s+0xaf4)
  [H: Geonosian folded wings - the BF1 class's 0x94272331 `geonosianwings_folded`]. BF1 `FUN_002013a0`
  (B1:113213-113236), same shape, effect c+0x18f0, `FUN_002012c8` outside stance 7. Baked: BF2 `rep_sfx_jetpack`,
  `imp_sfx_jetpack`, `imp_sfx_jetpack_darktrooper`, `cis_sfx_jetpack`, `geo_wingsfly`
  (`assets_baked/side_bf2/effects/*/effects.ron`); BF1 `darktrooper_jetpack`, `jetpack`, `geo_wingsfly`
  (`assets_baked/common_bf1/effects.ron`).
- **Engine sound [V code] (card JETP2, 2026-10-28):** the soldier feeds the same engine component as the vehicles
  (`re-vehicle-sounds.md` §3-§4, block BF1 c+0x16f0 / BF2 c+0x5b0, instance s+0x568). Key offsets by the BF2 setter
  `FUN_0044da60`: `EngineSound` 0x84bbef6a +0, `TurnOnSound` 0xea603e52 +4, `TurningOffSound` 0x7d2c2fa2 +0x1c,
  `TurnOffSound` 0x042dabae +0x24, `TurnOffTime` 0xc8a9e3c6 +0x2c (ctor `FUN_0044d7e8` 3.0, `0x40400000`).
  - On: at the jet's start (3.1 step 8; BF1 B1:118809-118823). `FUN_0044cc28` plays block +4 = **TurnOnSound**
    (`FUN_0044cfb0(.., block + 4, .., 2)`, B2:510025-510047) and asks the record to play.
  - Update (BF1 B1:117352-117376, before the stance switch; BF2 B2:160192-160196): while on, `speed` = |v| in m/s
    (s+0xa20 / s+0x550, `vsqrt` of the dot) and `acceleration` = `previous - speed / dt` (the walker's precedence slip:
    `FUN_00365bc0(dt, v, prev - v / dt, health, 0...)`), health = health / max. The baked record
    `rep_inf_Jetpack_engine_parameterized` (one layer, both games' maps): speed -> Pitch (0, 0.9)-(1, 1.0),
    acceleration -> Gain (-0.1, 0.5) (0, 0.4) (0.1, 0.5) ... (1, 1.0). So hovering still: pitch 0.9, gain 0.4; any
    motion: pitch 1.0 and the gain at its left end, 0.5 (acceleration about -30 x |v|).
  - Off: **on every update outside the jet stance** while the engine is on (BF1 B1:117778-117783: `stance != 7` ->
    `FUN_00366318`; BF2 `FUN_00239ad8` -> `FUN_0044ce98`, the same shape), plus the cleanup `FUN_002012c8`
    (B1:113045-113059). The turn-off first plays `TurningOffSound` and starts the run-down timer at 0, then in the
    SAME call, when `TurnOffTime <= timer`, plays **TurnOffSound** and stops the loop (B1:354146-354160). Every jet
    class on both discs authors `TurnOffTime 0.0` (and no `TurningOffSound`), so the turn-off sounds and the loop stops
    on the first update after the jet ends - **whatever ended it: a tap, empty fuel, a landing, a death**. This closes
    open question 6 [V code; no live capture].
  - Names: 2.2; `rep_inf_Jetpack_engine_parameterized`, `rep_weap_jetpack_turnon/off` are in every map's
    `sounds/names.ron` and bank, the record in `sounds.ron`'s `engines`; `geo_inf_fly_parameterized` is in no baked
    `names.ron` (11.10).
- **Animation [V] (card JETP2):** the special-anim tables hold `jetpack_hover` (BF2 0x19 at 0x5fde48, BF1 0x15
  at 0x517af0; `re-stance-roll.md` 3.1 / 4.1); BF1's ELF also names `humanlz_rifle_jetpack_hover`. The selection:
  BF1 `FUN_0022a0a8` (the animator's state is the soldier's stance s+0xbe4) case 7 -> special 0x15 (B1:142135-142138),
  so the dark trooper's jump-jet plays it too (BF1 has one jet stance); BF2 `FUN_0027bae0` (fed the stance s+0x7e4,
  B2:153824-153827) `case 6: case 7:` -> simple id 0x19 (B2:200463-200466): **both jet stances, the jump-jet included,
  play `jetpack_hover`**; no transition clip in or out. It is the one special a weapon action is drawn over (BF1's
  `jal 0x22c7f0` at 0x22b214 with a2 = 1; BF2 from the stand's action row, `re-soldier-anim-layers.md` §7). Landing
  from it, BF1 takes `landsoft` always (old special 0x15, `re-soldier-anim-layers.md` §10). Baked: every human
  set has `human_rifle_jetpack_hover` (61 frames, loop, blend 0.15 BF1; BF2 the `jetpack_hover` pair: loop, 0.15 /
  0.25, Torso); BF2's resolver fills a set without its own from that set's `jump` (`re-soldier-anim-layers.md` §6.2),
  so a BF2 soldier with a launcher in hand plays `human_bazooka_jump` in a jet, as the PS2's tables hold.
- **Effect, where it rides [D]:** BF1 attaches with no hard point (`FUN_0036ced0(effect, soldier, 0, ..)`: the
  soldier's matrix from vtable +0xfc); BF2 at `DAT_005fdd48 + 0x11ab0` (not named). The disc's jet effects emit along
  the frame's -x from x -0.24..-0.6, y -0.21, z +-0.15: two nozzles under and behind a frame whose x runs up the
  spine - `bone_ribcage`'s axes; 0.24 m under it is the bottom of the dark trooper's pack part (1.22-1.72 m in the
  bind pose). The port rides the drawn `bone_ribcage` [H, PROVISIONAL].

## 9. HUD

- **BF2 [V layout, V code]:** group `player1jetpackbar` in `assets_baked/common_bf2/hud/1playerhud.ron` (line ~3780):
  position (0.063165, 0.47552) viewport, rotation 270 deg (a vertical bar), scale 0.146068, fade in 0.1 / hold 2.01 /
  out 1.0, shown on `player1.jetFuelFraction`, hidden on `player1.jetDisable`. Children: frame meshes
  `hud_jetpackbar_frame_front` (rgb 40,40,38) and `_back` (alpha 0.5, rgb 205,236,234); fill barbitmap
  `player1jetpack_fill` (value `jetFuelFraction`, bitmap `hud_jetpack_and_energybar_blankfill`, rect 1.18923 x
  0.181946, uv (0, 0.518)-(1, 1), z 38, colour event `jetFuelColor`); threshold barbitmap `player1jetpack_threshold`
  (value `jetFuelThreshold`, uv width 0.49, z 252, rgb 132,0,0, pulse rate event `jetFuelWarning`).
  The publisher `FUN_003b4e70` (B2:410790-410849; event table `&DAT_006a91f8 + player x 0x31c`, registered in
  `FUN_003b0aa8` B2:407923-407930: +0x44 jetDisable, +0x48 jetFuelFraction, +0x4c jetFuelWarning, +0x50
  jetFuelThreshold): the bar is ON for a class with JetFuelRechargeRate > 0 and c+0x778 bit 2 (`dsll 30; dsra32`,
  0x3b4ed4-0x3b4ee0; bit 2 is on by the ctor default) - so jet troopers, dark trooper, Jango, Boba, Geonosians,
  Vader, Emperor; never the other Jedi. On: posts MinBorder to jetFuelThreshold. Each change of fuel: jetFuelFraction =
  fuel, jetFuelWarning = fuel <= MinBorder ? 1 - fuel / MinBorder : 0. Off: jetDisable. `jetFuelColor` has no
  publisher in this function [H: left at the layout's colour].
  The port's `ui/src/hud.rs` `feed` publishes health and energy only.
- **BF1 [V code]:** no layout file; a code-drawn icon `hud_jetpack_icon` (descriptor 0x5186e8: the string, (-35, 20),
  24 x 24), chosen by `FUN_002c2530` for a class with JetFuelRechargeRate > 0 (B1:252096-252106) and drawn by
  `FUN_002c3690`: scaled / filled by the fuel (B1:252419-252421), and in stance 7 at or under MinBorder its colour
  flashes between two table colours (DAT_005b0620 / 0x5b0628) at a period that shortens with the fuel
  (`fuel^2 x 4 + 0.25` s, B1:252363-252417).

## 10. Bots (SAIR 4.7, checked here) [V code]

- Eligibility for any jet behaviour: JetFuelRechargeRate > 0 and fuel > MinBorder (BF2 `FUN_002d9890` B2:263470,
  `FUN_002d99c0` B2:263513; BF1 `FUN_00258f30` B1:175803-175808, `FUN_00259058` B1:175849-175855). **So the Jedi
  (recharge 0) never use the jet behaviours**; the port's roster flag (`JetJump > 0`, `game/src/roster.rs`) gives
  them jet routes [D: a mismatch to settle, 11.11].
- The hop: every randint(6, 12) s, mode 0xc (BF1 mode 10) (B2:263475-263478, B1:175813-175816).
- The jump-jet hint: non-hover classes only (c+0x778 bit 3 clear / c+0x190c == 0), every randint(3, 8) s, a JetJump
  hint, mode 0xd (BF1 0xb) (B2:263515-263523, B1:175855-175870).
- The hover attack (BF2 `FUN_002dc0b8`, B2:265233-265240): target 20 m or more away, and
  `distance / speed <= (fuel - JetFuelInitialCost) / JetFuelCost` (the fuel lasts the trip).
- BF1 `FUN_00250450` (B1:169560-169567): every 5 s a coin flip (LCG bit 16) [D: SAIR's "jets straight up at random"].
- How a bot jets: 3.1 "Bots" (jump, then slot +0xbc; the AI vector replaces the kick's direction; pitch steering in
  the hover). Routes: the follower sets jump / jet from the connection's flags (SAIR).

## 11. Open questions / PCSX2 captures needed

1. **Live per-update trials** (`lead_tmp/airc/ps2/air_jump.py` style, both games, one row per game update): the dark
   trooper's leap (stick none / forward, from a stand jump's 2nd update), the jet trooper's hover (released stick,
   full forward at level aim, aim up 30 deg, strafe), fuel per update through a hover. Replay the formulas of 4.1-4.3
   against the rows before building. Expected [D]: BF2 jet trooper level-aim climb 1.66 m/s; BF1 steady climb 12.8 x
   sin(pitch).
2. BF1's recharge during the hover (net drain 0.1 / s) - the biggest BF1/BF2 difference; read s+0xc6c per update.
3. The bot vector at controller +0x4f0 (BF1 +0x3f0): its length (the kick = that x JetJump, not normalised).
4. s+0x4e0 bit 0x10000: confirm it is the lightsaber (slot +0x5c of which weapon class); and the Jedi double jump's
   vy (16.8 m/s for JumpHeight 3.5) live.
5. c+0x778 bit 2 (the HUD gate) and bit 0 (stance 0xc after 0.5 s in the air, B2:160632-160634): which keys set them.
6. ~~The engine sound's stop~~ - answered from code (8, card JETP2): TurnOffSound plays on the first update outside the
   jet stance whatever ended it, landing included (`TurnOffTime 0.0` on every jet class). A live capture (the sound
   voice table in RAM) is still owed to make it [V-PS2].
7. BF1 per-mission `SetMaxFlyHeight` values (lift MISSION.LVL's Lua 4.0 constants).
8. Fall damage after a jet: does the PS2 kill a dark trooper landing at 20+ m/s? (the port's `LETHAL_LANDING_SPEED`
   applies to any landing today).
9. Independent verifier pass on this note (RULES) - not done.
10. Data gaps in the bake: `geo_inf_fly_parameterized`. The Geonosian classes are baked since issue #82 (§17).
11. Do PS2 Jedi bots take `jet_jump` connections? The behaviours exclude them (10); the route follower was not read.

## 12. Build plan (small testable steps)

PLAN's locked feature set says "Jet/jump-pack: BF2 feel; infinite fuel via cheat", and the maintainer's rule is one
rule on all maps from the better game, as a named deviation. **Decision owed before step 3 (recommendation in
bold; taken by the maintainer 2026-10-28, steps 1-8 built: 13):** BF1 classes run **BF2's rules (4.1 / 4.2) with their own disc numbers where BF1 authors the key, and BF2's
same-role class where it does not** (BF1 jet trooper: JetAcceleration 30 and jet row 1.5 / 1.25 / 1.25 from
`rep_inf_default_jettrooper`; BF1 dark trooper: JetAcceleration 10, jet turn 1.0; BF1 Geonosian: BF2 geo_inf_geonosian's
row 2.0 / 1.25 / 1.25 and accel 20), recharge outside the jet stances (BF2). Alternative: BF1's own rules on BF1 maps
(4.3). Each step lands with tests; none needs a gate threshold change.

1. **Class data** (`core::soldier::SoldierConstants::from_props`): JetJump, JetPush, JetAcceleration, JetType hover,
   the four fuel numbers, JetEnergyDrain, the ControlSpeed jet row, JetEffect name, the three sound names; engine
   defaults of 2.1; last-wins for repeated keys. Test: the 2.2 tables for the fielded classes (both games), the
   sniper jet trooper's border 1.04, the BF1 mapping of the decision.
2. **Fuel** on `Soldier`: spawn 0.5 if recharge > 0 else 0; recharge (cap 1) outside the jet stances; the
   `infinite_jet_fuel` cheat pins the player's at 1. Tests: spawn values, 8.3 s empty-to-full for the BF2 jet trooper.
3. **Jet on** in the Jump / Fall branch of `step_looking`, after the air move: press edge, stance gate, fuel >= border,
   energy (`pay` rule of 4.3d with JetEnergyDrain), fuel -= initial, the kick of 3.1 step 6 (exact constants), the
   new stance. Tests: kick vectors for no stick / forward / back / side; refused under the border; refused on a
   depleted bar.
4. **Jump-jet stance** (`Stance::Jet`): drain, tap -> Fall, vy += JetPush dt, `air_steer` with the stick x
   JetAcceleration as an acceleration, gravity, ceiling, landing -> Stand with the 0.3 s lock. Tests: a BF2 dark
   trooper leap's apex and travel against the formula (and against the capture of 11.1 when taken).
5. **Hover stance** (`Stance::Hover`): the `FUN_00248898` mover (floor cut, exp(-2 dt), the doubled-pitch axis + 0.08,
   per-axis clamp at JetAcceleration dt, no gravity), the jet row's turn for humans, drain, tap -> Fall. Tests: speed
   decay with the stick released (e^-2t), 10.5 m/s cap, 1.66 m/s climb at level aim, 4.7 s flight.
6. **Jedi double jump** (BF2 heroes with a saber): the 3.3 branch. Test: vy 16.8 and the stick's run speed.
7. **Ceiling** term in Jump / Fall / Thrown / jet stances, with the mission's MaxPlayerFlyHeight (needs the value in
   the mission data; default 200). Test: a hover under a 20 m ceiling settles 5 m below it.
8. **Sim wiring**: jet stances count as `in_air` for the wall pass and `rising` for the floor probe while vy > 0; no
   fire lock. Test: a hover against a wall slides; a hover over a deck does not snap onto it while rising.
9. **Verifier + captures** (11.1-11.4) before step 4 lands its numbers as [V-PS2].
10. **Bots**: the AI mover's jet request (jump, then jet-on next update when the lock is 0; the bot direction override),
    the hover pitch steering (0.35 gain, 30 dt cap), the hop / hint / hover-attack schedulers with their fuel tests,
    and the roster's jet flag limited to recharge > 0 (Jedi out). Tests: scheduler intervals over a seeded run; the
    MUS1 connections 58 (26-25) and 65 (27-24) traversed both ways with motion and fuel (the card's private note).
11. **Effect + sounds**: JetEffect attached while in a jet stance and killed outside; engine loop with turn-on at start
    and turn-off at the end; black-box SOUNDS rows show them.
12. **HUD**: publish jetFuelFraction / jetFuelWarning / jetFuelThreshold / jetDisable by 9's rule. Test: the bar shows
    for the jet trooper and not for Luke.
13. **Animation**: `jetpack_hover` in the jet stances.

## 13. Port (card JETP, 2026-10-28, branch `jetpack`): steps 1-8 and the cheat

**The maintainer's decision (2026-10-28), the one taken in 12:** BF2's jet rules (4.1 / 4.2, 5.1's BF2 column) on
every map, BF1 maps included - a named deviation `[ours]` under the standing rule "one rule everywhere from the better
game". Each class keeps its own disc numbers; a BF1 class borrows from its BF2 twin only the keys BF1 never authors
(`game::roster::BF1_JET_BORROW`: the jet trooper JetAcceleration 30 and the jet row 1.5 / 1.25 / 1.25 from
`rep_inf_ep2_jettrooper`; both dark troopers JetAcceleration 10 from `imp_inf_dark_trooper`, jet row the ctor's
0.3 / 0.3 / 1.0). The Geonosians are skipped: their class data is not baked (11.10). BF2's fuel rule everywhere: no
recharge in a jet stance.

**Amended 2026-11-10 (playtest 2026-11-05 B4, `[ours]`):** BF1's JetFuelCost was set for a tank that refills in the
hover (5.1), so on BF2's no-refill rule a BF1 hover class flew half as long (the jet trooper 3.75 s from full, 1.25 s
from spawn). A BF1 hover class now pays the net cost JetFuelCost - JetFuelRechargeRate in flight
(`game::roster::bf1_net_jet_fuel_cost`): the jet trooper 0.2 - 0.1 = 0.1 / s (7.5 s from full, 2.5 s from spawn, BF1's
own times, section 16's net drain), the BF1 Geonosian 0.1 - 0.03 = 0.07 / s (11.4 s). The disc's JetFuelInitialCost,
border and spawn fuel are unchanged (they are the data in both games). Leap classes (cost 0) and every BF2 class keep
their disc numbers (except the jet troopers since 2026-11-10, below). Do not revert this as "not the disc number": it is the translation of BF1's rule onto BF2's.

**Amended 2026-11-10 (the maintainer, "one game", `[ours]`):** every jet trooper class on both discs flies on one fuel
profile, BF1's as the port flies it: in-flight cost 0.1 / s, lift-off cost 0.25, border 0.24, refill 0.1 / s outside
the jet, spawn 0.5. That is 7.5 s from full, 2.5 s from spawn, 2.4 s from empty back to the border and 10 s from empty
to full (`game::roster::{JET_TROOPER_ROOTS, JET_TROOPER_FUEL_COST, JET_TROOPER_FUEL_RECHARGE, is_jet_trooper}`). The
jet troopers are the classes whose chain reaches BF1's `rep_inf_jet_trooper` or BF2's `rep_inf_default_jettrooper`
(ep2, ep3, training, rifleman) and that can start a hover (JetJump > 0, border <= 1); the ep2 sniper (JetJump 0,
border 1.04) still never jets. It replaces BF2's disc drain 0.16 / s and refill 0.12 / s (4.7 s from full, 1.6 s from
spawn, 8.3 s empty to full): a BF2 trooper now flies 1.6 times as long. The lift-off cost and border were already the
same on both discs, so only the drain and the refill change; BF1's trooper keeps exactly its numbers (0.2 - 0.1 and
0.1). Jango and Boba (5.0 s), Vader and the Emperor, the dark troopers' leap, the Jedi double jump and the
Geonosians (BF2 8.0 s, BF1 net 0.07 / s, 11.4 s) keep their own; `bf1_net_jet_fuel_cost` now serves only a BF1 hover
class that is not a jet trooper. The mover fixtures in `core/src/soldier/tests/jet.rs` keep the disc's 0.16 / 0.12
because they replay the PS2 capture (section 16). Do not revert this as "not the disc number". Tests:
`roster::every_jet_trooper_flies_as_long_as_bf1s`, `roster::the_jet_classes_carry_their_disc_numbers`.

**Amended 2026-11-10, later (the maintainer: "use the bf1 fly time and the bf2 refill time for both games ... Increase
jango and boba to allow to fly a little longer than standard troops, say 8.5s vs the standard 7.5s, and have their
jetpacks refill the same speed as everyone elses", `[ours]`):** the jet pack's refill outside the jet is BF2's disc
rate 0.12 / s for every jet trooper on both discs (`game::roster::JET_PACK_FUEL_RECHARGE`, `JET_TROOPER_FUEL_RECHARGE`):
2.0 s from empty back to the border 0.24, 8.3 s from empty to full (BF1's 0.1 gave 2.4 s / 10 s). The flight stays
BF1's (0.1 / s, 7.5 s from full, 2.5 s from spawn). Jango and Boba Fett (`cis_hero_jangofett`, `imp_hero_bobafett`;
BF2 only, BF1 has neither class; `game::roster::{JET_HERO_ROOTS, JET_HERO_FUEL_COST, is_jet_hero}`) keep their disc's
lift-off cost 0.25 and border 0.24 and pay (1 - 0.25) / 8.5 = 0.0882 / s in flight: 8.5 s from full, 2.8 s from the
spawn's half tank (their disc's 0.15 gave 5.0 s / 1.7 s); they refill at the same 0.12 / s (already their disc rate).
The dark troopers' leap, the Geonosians, Vader and the Emperor and the Jedi keep their own. BF1 maps with a jet trooper
change (the refill), so their round hashes move. Tests: `roster::every_jet_trooper_flies_as_long_as_bf1s` (flight from
full and spawn, refill to the border and to full), `roster::jango_and_boba_fly_8_5_s_and_refill_as_a_trooper`,
`roster::the_jet_classes_carry_their_disc_numbers`.

Bot rounds (`round_to_outcome 9000 42`, release, `JET_CENSUS=1`, main 39c6f1eb against the refill):

| map | hash main | hash refill | jets main: units / hops / attacks | jets refill: units / hops / attacks |
|---|---|---|---|---|
| bf2_geo1 | b17ea4a4507421cf | e1952b11617abe44 | 13 / 11 / 17 | 12 / 12 / 20 |
| bf1_geo1 | c9d94cde45449c1b | 6c11d2e95500591e | 8 / 26 / 13 | 9 / 32 / 18 |
| bf2_mus1 (GCW, dark trooper) | c772c41b5300a1d7 | c772c41b5300a1d7 | 8 / 18 / 0 | 8 / 18 / 0 |

The faster refill puts jet-trooper bots back in the air sooner (more hops and attacks); the dark-trooper map keeps
its hash. These rounds field no Fett.

Bot rounds (`round_to_outcome 9000 42`, release, `JET_CENSUS=1`, main a99f64be against the profile):

| map | jet classes | hash main | hash profile | jets main: hops / attacks | jets profile: hops / attacks |
|---|---|---|---|---|---|
| bf2_geo1 | ep2 jet trooper, Geonosian | 3abede4cc7914e00 | b17ea4a4507421cf | 35 / 10 | 11 / 17 |
| bf2_mus1 (GCW) | dark trooper | c772c41b5300a1d7 | c772c41b5300a1d7 | 18 / 0 | 18 / 0 |
| bf1_geo1 | BF1 jet trooper, Geonosian | caa3e80d39f4dcb9 | caa3e80d39f4dcb9 | 29 / 22 | 29 / 22 |

The BF2 bot's hover-attack gate (`core::ai::jet`, the trip's time against the fuel left over the cost) opens about 1.6
times farther away, and an attack takes the place of a plain hop, so BF2 jet-trooper bots attack from the air more
and hop less. BF1 maps and maps without a jet trooper keep their hashes.

**Amended 2026-11-10, the dark troopers' refill (the maintainer: "make it the faster refill stat, again we're trying
to normalize things across both games", `[ours]`):** BF1's two dark troopers (`imp_inf_dark_trooper`,
`imp_inf_dark_troopersnow`) refill their leap at BF2's dark trooper's `JetFuelRechargeRate`, 0.075 / s in place of
BF1's 0.025 (`game::roster::BF1_LEAP_REFILL`, a one-game row beside `BF1_JET_BORROW`). Back to the lift-off border
that is 6.5 s, not 19.6 s (the snow variant's border 0.4: 5.3 s, not 16 s). The rest stays each disc's: BF1's 21 m/s
kick, lift-off cost 0.5 (snow 0.4), border 0.49 (snow 0.4) and no cost in flight; BF2's numbers on BF2 maps. A dark
trooper bot on a BF1 GCW map can leap again in a third of the wait, so those rounds move: the round goldens' `bf1_yav2`, `tat1`
and `bf1_nab2` rounds end at other ticks, some with the other winner (`award-weapons.md` section 11). Test:
`roster::bf1_dark_troopers_refill_their_leap_as_bf2s`; `roster::the_jet_classes_carry_their_disc_numbers` and
`roster::every_jet_trooper_flies_as_long_as_bf1s` hold the new value.

Built for human players and the sim; bots, effects, sounds, the HUD bar and the animation are card JETP2 (12 steps
9-13).

| step | where | what |
|---|---|---|
| 1 class data | `core::soldier::SoldierConstants` (`jet_*`, `saber`, `control_row`, `has_jet`), `game::roster::soldier_constants` / `jet_class_data` / `BF1_JET_BORROW` | the 2.1 keys with their engine defaults; the jet row from the chain child-first, row by row; the lightsaber flag = a channel-0 `melee` weapon |
| 2 fuel | `Soldier::fuel`, `Soldier::spawn_fuel`, `core::unit::UnitSpec::build`; the cheat in `Sim::step` (`sim/step.rs`) | spawn 0.5 when the class recharges, else 0; recharge before the stance switch outside the jet stances, cap 1; `Cheats::infinite_jet_fuel` pins a player's tank at 1 for a class with a jet only (it gives a rifleman no jet) |
| 3 start | `Soldier::jet_on`, called in the Jump / Fall branch of `step_looking` after the air move and the ceiling | a jump press edge in the air (the second tap); fuel >= border (equal passes); JetEnergyDrain by the jump's energy rule (`Soldier::pay_energy`: 0 passes, depleted refuses, short overdraws); fuel -= initial, floor 0; the kick of 3.1 step 6 added for the next move |
| 4 jump-jet | `Stance::Jet`, `Soldier::step_jet` | fuel cost a second (empty or a tap -> Fall, no move that update); vy += JetPush dt, then the air drag, the stick x JetAcceleration, gravity off the floor |
| 5 hover | `Stance::Hover`, `Soldier::step_jet` | the floor cut, v x exp(-2 dt), the flight axis from the aim BEFORE this update's turn (pitch lifted 0.08 and doubled), per-axis approach at JetAcceleration dt to the jet row's targets; no gravity |
| - both | `step_looking` | a human's turn x the jet row's turn factor; the energy rate 0 in the jet stances; landing (a floor contact) -> Stand with the 0.3 s lock, no first-air guard |
| 6 Jedi | `Soldier::jet_on` | a sabre class without hover: the double jump of 3.3 (stance Jump, the stick's run speeds, vy + J under J / 2 else 1.5 J); a sabre class's jet ending tries a start again at once (Vader, the Emperor: refused under their full-tank border) |
| 7 ceiling | `Soldier::fly_ceiling` (Jump, Fall, Thrown, both jets, after the move), `Sim::fly_ceiling`, `MissionDef::max_player_fly_height`, `MapLayout::apply_mission` -> `MissionApplied::fly_ceiling`, `scripts/gen_missions.py` | 5.3's spring; the mission's last `SetMaxPlayerFlyHeight`, else 200 |
| 8 sim | `sim/step.rs` | `Stance::airborne` (the jets in the air for the wall pass's air step), `rising` for a jet going up (the floor probe), the taps a human's only (`AirEnv::human`), fuel in the hash when non-zero (the jet state is the stance) |

Controls: jump (pad and keyboard, the existing binding) on the ground jumps; a second press in the air starts the
jet (the dark trooper's leap, the jet trooper's / Jango's / Boba's / Vader's hover, a Jedi's double jump); a third
press ends a jet into a fall (a Jedi's third press jumps again while the energy bar holds). In the hover the left
stick flies along the view (aim 4.6 deg down for level flight; level aim climbs gently), back and sideways; the
right stick turns at 1.25 x (the jet row). Releasing the stick lets the hover coast to a stop. Landing ends it. The
third-person camera keeps the stand's section and trails his speed, as measured in section 16 (it took the sprint's
section until 2026-11-01).

### The feel as built (dt 1/30; tests `core/src/soldier/tests/jet.rs`, `core/src/sim/step/tests/jet.rs`)

| class | number | RE (6) | built |
|---|---|---|---|
| BF2 dark trooper | vy after a forward kick on the jump's 2nd air update | 22.9 m/s | 22.9 |
| | rise over the kick point | 14.6 m "before drag" | 11.6 m with the drag (an independent f64 replay of 4.1 agrees to 1 cm) |
| | leaps from full / from spawn | 2 / 1 | 2 / 1 |
| | refill to the border | 6.5 s | 6.53 s (196 updates) |
| BF2 jet trooper | forward / strafe at full stick | 10.5 / 6.6 m/s | 10.5 / 6.56 |
| | backing | 6.6 m/s | **7.9** (4.2's formula: MaxStrafeSpeed x the row's FORWARD factor 1.5; 6's 6.6 takes the strafe factor - the port follows the formula and its instructions; JETP2's captures settle it) |
| | level flight | aim 4.6 deg down | 4.59 deg |
| | climb at level aim | 1.66 m/s | 1.66 |
| | flight from full / refill empty to full | 4.7 s / 8.3 s | 4.7 / 8.33 |
| | released stick | e^-2t | e^-2t off the flight and right axes; along them the approach also brakes 1 m/s an update |
| BF1 jet trooper on BF2 rules | forward / strafe | (BF1: 13 / 9) | 9.75 / 5.63 (6.5 / 4.5 x 1.5 / 1.25) |
| | flight from full / from spawn | (BF1: 7.5 s / 2.5 s, recharging in flight) | 7.5 s / 2.5 s (net cost 0.1, no recharge in flight; 3.75 s / 1.25 s before 2026-11-10) |
| BF2 jet trooper as shipped | flight from full / from spawn / refill empty to full | (disc: 4.7 s / 1.6 s / 8.3 s) | 7.5 s / 2.5 s / 10 s (the one jet trooper profile, 2026-11-10) |
| BF1 dark trooper on BF2 rules | | (BF1: no cancel, no drag, 19.6 s refill) | BF2's leap: tap-off, drag, stick at 10 m/s^2; refill to 0.49 at BF2's 0.075 / s = 6.5 s (0.025 / s, 19.6 s, before the 2026-11-10 refill row) |
| Vader / Emperor | start / hover / energy | full tank, 2.5 s, 30 | full tank (0.56 s from spawn), 2.5 s, 30; the bar holds while he hovers |
| Jedi (JumpHeight 3.5) | double jump | 16.8 m/s, 40 energy | 16.84, 40; the stick's run speed |
| any | ceiling | spring toward 5 m under it | a climbing hover settles 4.7 m under it (15.3 m under a 20 m ceiling) |

### Judgment calls (OURS or PROVISIONAL, beyond the decision)
- The jets' taps are a human's (`AirEnv::human`): the PS2's bots call the jet slot themselves, so a bot's pad never
  jets until JETP2 builds that; bot rounds change only by the hashed fuel.
- The jet's ending update holds the position (4.1 step 2-3: "not ending" runs the mover); the fall moves from the next.
- The hover's flight axis takes the aim before this update's turn (7's [D] one update of lag).
- The energy rate is 0 in the jet stances only; Jump / Fall keep the port's existing restore (4.3d says 0 there too;
  not changed here).
- The infinite-fuel cheat applies to a class with a jet only (`has_jet`); the BF2 sniper jet trooper's border 1.04
  still refuses (as on the PS2).
- The ceiling on a BF1 map is the script's `SetMaxPlayerFlyHeight`, as BF2 reads it; BF1's own soldier ceiling was
  `SetMaxFlyHeight` (the AI flyers' in BF2). Missions with only `SetMaxFlyHeight` (BF1 KAM1, NAB2, BES2, TAT2, KAS2 CW;
  BF2 KAS2, TAT2) keep 200. The values (`gen_missions.py`): BF1 BES1 220, END1 43, GEO1 50, HOT1 70, KAS1 40, KAS2
  50 (GCW), NAB1 -40, RHN1 30, RHN2 30, TAT1 90, YAV1 14, YAV2 -22 (the negative ones sit over their terrain: NAB1's
  is -140..-80, YAV2's -63..-50); BF2 as 5.3, with MUS1 GCW 84.16. **The local `engine/missions/*.ron` predate the field:
  regenerate them (`python3 scripts/gen_missions.py`) to carry the ceilings; until then every map has 200.**
- Not built from 3.1: the seat gate (s+0x328), the combo / flip call (s+0x7f0) of 3.3, the tutorial hint counter.
- Fall damage after a jet: the port's `LETHAL_LANDING_SPEED` (25 m/s) applies (11.8 open); a dark trooper's leap
  lands at about 20 m/s on level ground.

### Bot rounds (`round_to_outcome 9000 42`, release, main 5e6efa36 against `jetpack`)

| map | outcome, tickets (both) | hash main | hash jetpack | jetpack without the fuel in the hash |
|---|---|---|---|---|
| bf1_kas2 (dark trooper) | none in 9000, 166 / 117 | d9edf5738dc1ba04 | 919f71cec7b4bcaa | d9edf5738dc1ba04 |
| bf2_kas2 | none in 9000, 3 / 93 | 63ade9d4ca18deae | f776aa66d2a2052c | 63ade9d4ca18deae |
| bf2_yav1 (dark trooper) | none in 9000, 68 / 15 | 7d9f20349e846f67 | 3952e620d42dbe5b | 7d9f20349e846f67 |
| bf2_cor1 | team 0 at 8818, 83 / 0 | 6eeee158fcc6bd97 | c51ff0c1ca3285d1 | - |

Bots do not jet, so every round plays exactly as before: the hashes move only because a jet class's fuel is hashed
(a probe build that leaves it out reproduces main's hashes). With the regenerated missions' ceilings (YAV1 14, BF1
KAS2 50, COR1 25; BF2 KAS2 none) the four hashes are the jetpack column's: no bot's jump came within 5 m of a ceiling.

## 14. Port (card JETP2, 2026-10-28, branch `jetfx`): animation, sounds, effect (12 steps 11 and 13)

Presentation only: nothing here is sim state or hashed, so bot rounds and their hashes do not move.

| what | where | as built |
|---|---|---|
| class keys | `game::roster::JetLook` / `jet_look` (on `ClassRoster` and `walk::ClassVisual` as `jet_look`) | `JetEffect`, `EngineSound`, `TurnOnSound`, `TurningOffSound`, `TurnOffSound`, `TurnOffTime` (3.0 unset) through the chain, for a class with a jet (`has_jet`) or a hover (Vader, the Emperor); none for any other class |
| animation | `game::animator::Special::JetHover`, `stance_of` | `Stance::Jet` and `Stance::Hover` -> `jetpack_hover` (8: both games, both stances); a shot / throw / reload plays over it from the stand's action row, the legs keep it; BF1 lands soft out of it (`animator/jump.rs`, its air clock restarts every jet update - PROVISIONAL) |
| engine | `game::engine_sound::EngineSounds::tick` -> `tick_jets` | one `Instance` per soldier (Drive::Walker: |v| m/s, `previous - speed / dt`), on while alive in a jet stance; its layers are `Voice`s with `soldier: true`, played by `SoundLib::loop_voices` as `LoopSource::Jet` (3D at the soldier, every unit); `TurnOnSound` at a turn-on (from off, or back on while running down), `TurningOffSound` at a run-down's start, `TurnOffSound` when it ends - at once with every disc jet class's `TurnOffTime 0.0`. The one-shots are `JetShot`s: the window plays them with `SoundLib::play` (so the black box's SOUNDS rows list them), `SOUND_LOG` writes a `jet` row each |
| effect | `game::fx` `jet.rs` (`Fx::follow_jets` from `Fx::ingest`, `Fx::set_jet_frames`), `SoldierVisuals::jet_frames` | the class's `JetEffect` spawned once per jet stance, riding the drawn `bone_ribcage` (8 "where it rides"; before his first pose, its bind pose turned to his yaw), stopped (particles live out) when the jet ends, he dies or he is gone |

Not built here: the PS2's turn-on / turn-off share one sound handle (a turn-off cuts a turn-on still playing); the port
starts each as its own one-shot. BF2's second class effect (c+0x894, outside the jet: the Geonosian's folded wings)
and the Geonosians themselves (not baked). The HUD fuel bar (12 step 12) and the bots (step 10) are JETP2's rest.

Tests: `animator/tests/jet.rs` (the stance table; five baked jet models of both games play `human_rifle_jetpack_hover`
on both layers in both jet stances, a shot over it, the landing back to idle), `engine_sound/tests.rs` (turn-on, loop,
turn-off at every end over three flights; a run-down with a `TurnOffTime`; a new life; the baked classes' keys and the
KAS banks' records and sounds, both games), `fx/tests/jet.rs` (both games' four effects baked, spawned on the jet,
riding him, stopped at the end and at a death; the drawn bone moves it), `soldier_vis/tests.rs`
(`a_jet_rides_the_drawn_ribcage`). Pictures: `engine/artifacts/lead_tmp/jetfx/` (BF2 GEO1 ep2 jet trooper, BF2 KAS2
dark trooper, BF1 GEO1 jet trooper; from behind and the side; the scratch tool `jet_still.rs.txt` beside them).

## 15. Bot commands and fuel HUD (issue #72, 2026-11-01)

`core::ai::jet` now schedules rechargeable classes, and hands the soldier a
separate bot command through `AirEnv`; player jump taps are unchanged. A
request first jumps, then starts the jet on a later unlocked Jump update.
The controller kick replaces the ordinary kick without normalization, as at
BF2 `0x247814–0x247848`. The scheduler takes integer 6–12 s hop and 3–8 s
non-hover hint intervals from `FUN_002d9890` / `FUN_002d99c0`. Hover attacks
require the current retained contact at least 20 m away and enough fuel for distance /
MaxSpeed (`FUN_002dc0b8`, which fetches the current target without a fresh visibility
test); the attack's wanted height is target +10 m
(`FUN_002dc448`, B2:265350–265381). The hover's flight axis uses the 0.35
height correction capped at 30 × dt from `FUN_00249f80`. These functions were
re-read in the decomp for this implementation; this is not a new live capture.

The integration with the port's own AI is [ours]: deterministic per-unit
hashed draws, an initially delayed first hop, a committed flight destination,
no ground ledge veto while in that flight, a 3 m attack end radius (route
arrival needs the ordinary arrival radius and feet above the endpoint), and cancellation
for boarding, tool use and off-map recovery. An attack whose target has
gone also ends, as in BF2 FUN_002dc448 (B2:265345–265379), which returns to
its previous behavior and runs the flight exit command. The shared bot
policy drops a launched hover attack and discards a pending one. JetJump
hints activate inside
their authored radius (ordinary route arrival slack when absent); their
endpoint/direction semantics remain a fidelity follow-up. The height
correction changes the hover's flight axis without redirecting its weapon's
aim. The flight axis follows requested travel, including sideways/backward movement.
Jet links use a surface projection below the higher authored hub (Mustafar has
y=0 endpoint hubs); arrival tests the destination floor height. The isolated
Mustafar nav/collision test reaches all four endpoints, but the reverse trips
finish by walking after fuel expires. Full mission lava/hazard traversal is
still unvalidated; the isolated fixture does not establish that acceptance. The ground collision and
fly-height ceiling continue to run. The new scheduler and flight fields are
hashed; classes without rechargeable fuel carry no additional hash bytes.
The roster's route flag now uses `SoldierConstants::has_jet`, excluding a
fuel-less Jedi double jump.

The BF2 HUD publishes `jetFuelFraction`, `jetFuelThreshold`, `jetFuelWarning`
and `jetDisable` through its authored `player1jetpackbar` tree. A living
on-foot class with rechargeable fuel sees it; a death, a seat, a remote view
or a fuel-less class disables it. Warning is `1 - fuel / border` at or below
a positive border and zero above it. Disable is sent on the falling edge and
forgets fuel/threshold events so a class with identical fuel values restores the bar.
The offscreen check exposed an old HUD default-depth hypothesis: PS2
`FUN_005a30b0` / `FUN_005a3178` initializes render +0x48 to 128, not zero.
Using that default keeps the opaque jet frame behind its fill at ZOrder 38.

The backing-speed discrepancy in §6 was a numerical summary error:
`FUN_00240bf8` B2:160710–160717 selects MaxStrafeSpeed for negative forward
input, then multiplies it by the **forward** jet control factor, not the
strafe factor. That is 5.25 × 1.5 = 7.875 m/s for the BF2 jet trooper, agreeing
with §4.2 and the existing simulation. No physics constant changed here.
A per-update PS2 capture is still required to call this live-verified.

Remaining original-game acceptance is explicit: per-update motion/fuel
captures, sound voice-table capture, the effect attach bone, the jet camera,
Geonosian asset/engine coverage and independent RE verification. This slice
does not turn those open checks into passes.

## 16. Live PS2 runs and Mustafar's hazards (issue #72, 2026-11-01) [M]

**How.** A software PCSX2 session on a no-AI battle's side-select state, one row per game update (`dt` 1/30): feet,
velocity, fuel, stance, camera. BF2 `bf2_geo1_ai0_one_sideselect`, BF1 `bf1_geo1_units1_one_sideselect`. The menu keeps
BF2's jet trooper behind points in this state, so the player spawned as the first class and his class pointer was
written to the loaded jet trooper's before the trials; the fuel was written to 1.0 before each. Jump at t = 0.3, second
tap at 0.7, stick from 1.2, a tap at 4.6. The class's values read from RAM are section 2.2's: BF2 5 / 8 / 30, recharge
0.12, cost 0.16, initial 0.25, border 0.24; BF1 5 / 8, recharge 0.1, cost 0.2, initial 0.25, border 0.24.

**BF2 hover against `Soldier::step_looking`** (`the_hover_replays_the_ps2_capture`). Updates since the jet started:

| update | vy | rise m | fuel |
|---|---|---|---|
| 0 | 6.257 | 0 | 0.750 |
| 5 | 4.48 | 0.86 | 0.723 |
| 10 | 3.21 | 1.47 | 0.697 |
| 20 | 1.65 | 2.23 | 0.643 |
| 45 | 0.31 | 2.87 | 0.510 |
| 90 | 0.02 | 3.01 | 0.270 |

The port gives the same to 0.02 in each, and the settled speeds: forward 10.5, back 7.88 (section 4.2's formula; this
settles the 7.9 against 6.6 of section 6), strafe 6.56, and with the aim raised about 28 deg (camera -11.5 to +17.1;
the port's test takes +24.6 from spawn's -4) climb 7.8 with 7.03 forward. Fuel falls
0.16 a second in the hover and rises 0.12 a second on the ground. The empty tank ends the hover at the border's end
(fuel 0.019 at 5.34 s, on the ground by 6.67 s).

**BF1 hover** (section 11 item 2 answered). Net drain 0.1 a second through the hover, so recharge runs in it. Settled:
forward and back 12.97, strafe 9.0 (2 x MaxSpeed and 2 x MaxStrafeSpeed, section 4.1); at this aim forward sinks 0.9
m/s and back climbs 0.9, and he holds height at rest. The port flies BF1 maps on BF2's rules (9.75 forward on a BF1 jet trooper), its named deviation of
section 13: this measures the size of that choice and does not change it.

**The default class's jump-jet** (BF2, JetJump 7, JetPush 14, JetAcceleration 20, stance 6, given fuel by the same write)
climbs at 8.26, tops out near 2.6 s and reaches 35.9 m/s forward by 4 s. No shipped class flies with those values; it is
the camera's fast case below.

**The camera (issue #72's named deviation, settled).** There is no jet section. At rest in the hover the camera sits
exactly where it stood (BF2 clone trooper 3.2 m back at -14 deg, jet trooper and BF1 3.5 m at -11.5 deg). Moving, it
trails on all three axes by 0.0514 x velocity: 0.54 m at 10.5 m/s, -0.41 at 7.88 back, 0.34 at 6.56 strafe, 1.83 at
35.9. That is an ease of 15 a second run once per 30 Hz update (e^-0.5 / (1 - e^-0.5) / 30). The port's jets now
keep the stand section and take that ease (`core::unit::jet_camera_lag`, `WalkSim::camera_track`), which also dies away after a jet ends.
- **Not ported:** the same trail shows on foot (a landing at 25 m/s, stance 0: 4.42 m back, easing to 3.105 by the
  time he stops). The port's on-foot camera has no trail. Changing that moves every soldier's camera and is left for
  a playtest.
- **Not ported:** the jet trooper's class-specific 3.5 m stand camera. The port has one default camera for every class.

**Mustafar's hazards.** The mission adds no death region. Its hazards are the map's nine `damage` regions, which nothing
in the port read: a bot could stand in lava. BF2 `FUN_0035c8e0` makes one from each region named `damage...` (the
key table at 0x5fed50, keys matched by prefix, missing scales 0); `FUN_0035c578` runs every update, hitting each living
object the region gathers for `rate x dt`, scaled by health type, with no owner. The gather is the bounding-sphere
touch of `re-kill-floor.md`. Measured at 400 health, teleported above each and released:

| where | region | PS2 | port |
|---|---|---|---|
| steam, (-54.24, 111.41) | rate 1, person 50 | -1.667 per update | same (`mustafars_regions_hurt_as_on_the_ps2`) |
| pit, (-58.78, 152.53) | rate 900, person 100 | -3000 in one update | dead at once |
| lava, (100, 80) | rate 1000, person 200 | -6667 in one update (health -6267) | dead at once |
| first hit, falling | box top 75.51, and 67.47 | feet 0.43-0.67 m over the top | sphere 1.0 up, radius 1.46: 0.46 |

Only BF2 FEL1 (20, 30 a second), MUS1 and POL1 (60 a second) author any, and no BF1 map. A building scale is authored
nowhere, so buildings are not tested. `core::sim::DamageRegion`.

**The jet links.** Hubs 24 and 25 are leaves, each joined only by its jet arc to the deck (26 and 27). The route tables give
those arcs weight only for a bot bound for that leaf, so a conquest round has no reason to fly them: `JET_CENSUS` over
five seeds on BF2 MUS1 shows no bot starting one. Sent there by hand, in the real conquest scene with the lava on and
the mission's 90 m ceiling, a jet trooper crosses all four directions alive
(`mustafar_jet_connections_arrive_alive_through_the_mission_hazards`). The PS2 stands at those hubs at 58.46, 55.51,
58.46 and 55.45; the port at 58.48, 55.54, 58.48 and 55.49. Whether the PS2's bots ever pick a leaf as a goal is not read.

**Rounds** (`JET_CENSUS=1`, seeds 1-4 and 42, 18,000 ticks; jump-jet bots do not hover-attack, so MUS1's ep3 jet trooper has
none). BF2 GEO1: 13-16 jet units, 37-115 hops, 7-19 hover attacks. BF1 GEO1: 11-16 units, 36-88 hops, 1-8 attacks. MUS1:
7-10 units, 34-49 hops. Minimum gap between a bot's flights 6.0 s, as drawn; means run 9-31 s because a hop needs an idle
grounded moment with fuel, and the count includes the gaps between lives.

**Still open.** The jet sound voices and the effect's attach bone (no capture). The Geonosians: both GEO1 missions field
them as locals, nothing in the game spawns a mission's locals team yet, and their classes are not baked. (Both resolved
by issue #82: §17.)

## 17. Where a hop goes, and the BF1 Geonosian (issue #82, 2026-11-04)

**Report.** Geonosians flew up, stalled and came down nearly where they rose. In the PS2 a hop takes the bot far away.

**The hop's spot [V code].** The mode 0xc entry is BF2 `FUN_002dbb68` (B2:264972) and BF1 `FUN_0025aa08` (B1:177019). It
makes up to 10 tries:

1. Draw x and z, each `(u - 0.5) * 2 * 40`. Reject the draw if x^2 + z^2 < 400, so the spot is 20 to 40*sqrt(2) m away.
2. The spot is the bot's position plus (x, 0, z).
3. Clear test `FUN_002eea68`, radius `FUN_002a76b8`. The radius is class+0x370, half the aim size.
4. A sky ray from the spot to spot + (0, 40, 0), masks 0x9a (BF2) and 0x4e (BF1).
5. Only when blind jumps are off: a line test `FUN_002ef6b8`. `DAT_005fdfe2` is 1 by default; only Kamino clears it.
6. The first spot that passes becomes the move destination, with jet request bit 4 and kick vector 0.

The state's update `FUN_002dbfa8` ends it 1 s after controller bit 0x20 clears. Every caller is an attack sub-state
(2-9), so **a bot hops only in a fight**. Jet off (bit 0x40) is set only by `FUN_002dc5c0`, the exit of the hover
attack (mode 0xe).

**The port** (`core/src/ai/jet.rs` `hop_spot`):

- A hop needs a target and the 6-12 s schedule. It flies to the first clear spot of 10 hashed draws, kick zero, no
  wanted height.
- [ours] The point and, on Kamino, intervening line test use enabled nav soldier barriers expanded by half the
  aim size. BF2 `FUN_002eea68` (B2:278740–278790) and `FUN_002ef6b8` (B2:279349–279475) instead query the original
  obstacle index with the class filter. trace: its complete obstacle membership and shape/height rules against
  the port's rectangles. The line condition is BF2 B2:265077–265083 / BF1 B1:177127–177134. The default is on
  (BF2 `FUN_002a4030`, B2:225626); `SetAllowBlindJetJumps` (B2:32255–32264) changes it. The canonical stock
  mission correction disables it for `bf1_kam1` and `bf2_kam1`, including retained mission files, across sources.
- The tactical sky query is separate from ordinary sight. BF2 B2:265074 / BF1 B1:177125 cast 40 m upward and
  exclude the requesting soldier. Their 0x9a / 0x4e masks select soldier, vehicle, building and terrain objects
  (re-collision-layers §5); these object masks are **not** per-body subset masks. The port reads terrain,
  both current world-body subsets and current vehicle hulls, plus other living unseated soldiers' bolt prims.
  It skips the querying soldier. Ordinary `perception::los` remains ordnance-only.
- **Deliberate conservative port correction:** a soldier-solid overhead body rejects a tactical hop even if
  absent from its ordnance subset. This fixes the paired roof control; it is not a claim that 0x9a means
  `Layers::ALL`. Retail BF2's ray callback `FUN_00162f60` (B2:2542–2588) reaches `FUN_00168318`
  (B2:6852–6950), whose filtered branch reads the ordnance subset at B2:6945. The port collapses object
  membership and authored body subsets. trace: exact retail object ownership/subset selection and moving
  soldier pose queries before replacing the conservative union and the existing posture-based bolt prims.
- [ours] The flight ends within 3 m of the spot. trace: `FUN_002dbfa8`'s arrival, and when it clears bit 0x20.


**BF1 Geonosian on BF2 jet rules.** This applies the named deviation of §2.2 and §12, `BF1_JET_BORROW` in
`game/src/roster.rs`. The baked BF2 `geo_inf_geonosian` has JetAcceleration 20 (default) and the jet row
[2.0, 1.25, 1.25]. Without the row, the BF1 class kept the ctor row [0.3, 0.3, 1.0] and hovered at about 1.5 m/s.
Tests: `bf1_jet_borrowing_matches_the_bf2_twins`, `bf1_geonosians_fly_on_their_bf2_twins_jet_rules`.

**Rounds** (sim trace, 5,400 ticks, flights of the locals team):

| Map | Before | After |
|---|---|---|
| BF2 GEO1 | 7.8 m mean displacement, 33% of samples near still | 25.6 m, 3% |
| BF1 GEO1 | 4.9 m, 31% | 18.5 m, 0% |

### Bespin Platforms tactical landing check (2026-11-12, #135)

The Dark Trooper could select a sky-clear spot over the void and fly
past the deck. The native BF1 tactical spot query uses the authored
barrier and sky checks (FUN_0025aa08, B1 177019–177134). The port's
Bespin deck zone also requires a reachable, walkable-height landing
surface through the existing tactical spot check. Authored jet links
and hover-attack goals follow their own queries. measure: matched
original Dark Trooper landings and deck-edge falls.
