# OPUS RE (2026-10-06) - [O] UNAUDITED

# RE: zoom (BF1 + BF2) — the button, the weapon's steps, the camera, the turn rate, the scope

Card ZOOM (loop lane 2, max effort). Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1), `analysis/SWBFRNT2.ELF-decomp.c`
(B2), both ELFs (data tables, vtables and tiny virtuals read as words), every LVL of both discs (class census), the
mission scripts, and a live PS2 (PCSX2, software renderer): scripted pad trials on BF1 NAB2 (a Rebel soldier and a
Rebel marksman) and BF2 COR1 (a Rebel rifleman), one row per game update holding the game's own zoom flag, press
word, weapon state / zoom / divisor and the camera's projection. Tags: [V] read (lines cited; each cited line lies
in the function named, `lead_tmp/zoom/cite_check.py`), [M] measured live on the PS2, [D] derived, [H] hypothesis;
`[O]` marks the load-bearing claims. Scratch, tools, logs: `engine/artifacts/lead_tmp/zoom/`.

Conventions: BF1 soldier O (the live tools hold its health component C = O + 8), its Controllable K = O + 0x2a0;
BF2 soldier E, its Controllable K2 = E + 0x270. "Update" = one game update (BF1 33.36 ms on the frame's own clock,
BF2 a fixed 1/30 s).

## 0. Headline

**R3 toggles the zoom of the weapon in hand. It is a press, not a hold, and only a human's soldier has it.**

| | BF1 | BF2 |
|---|---|---|
| button (default layout) | R3 | R3 |
| first press | zoom = the weapon's `ZoomMin` | the same |
| second press | `ZoomMax` when it differs from `ZoomMin` (and `ZoomRate` is 0), else off | the same |
| third press | off | the same |
| a weapon with `ZoomMax` <= 1 (grenades, tools; the default) | no zoom | the same; a lightsaber never |
| what the zoom is | the camera's projection scale S eases to the zoom value: per update S <- z + (S - z) exp(-25 dt) | the same |
| field of view under the zoom | the class's `FirstPersonFOV` (no BF1 soldier authors one: 48 deg, as unzoomed) | `FirstPersonFOV` 70 deg (unzoomed third person: 65), eased the same way |
| rifle (2.5 / 2.5), vertical FOV | 37.69 deg -> 15.55 deg | 52.06 deg -> 24.24 deg |
| sniper rifle (2 / 8) | 19.37 deg, then 4.89 deg | 30.05 deg, then 7.68 deg |
| camera, most weapons | third person over the right shoulder: the stance's ZOOM camera section, TrackOffset (0.4, 0.05 / 0.2 / 0.2, 2.8), tilt 3.5 deg, taken at once | the same |
| camera, `ZoomFirstPerson` weapons (sniper rifles; BF2 also bowcaster, award pistol, target pistol) | the scope: the eye at the fire point less 0.3 m along the aim, looking along the aim; the soldier's own body not drawn; the `weapon_scope` overlay | the same, 0.6 m |
| turn rate while zoomed | yaw and pitch rates divided by the weapon's turn divisor (= the zoom value on every disc weapon) | the same |
| cancelled by | a reload of the primary, a switch of the weapon in hand, the secondary's throw, a roll, being thrown, death, changing seat | the same, and a sprint |
| kept through | walking (same speed), a jump, a crouch, firing | the same |
| a bot | never zooms | never zooms |

## 1. The button [O]

- **BF1** [V]. The pad object keeps a table of 20 logical actions per physical control and mode
  (`FUN_0034c5f8` B1:336769-336787 walks controls 0..19 and hands each one's previous and current value to
  `FUN_0034a908` with the action from the table of the current mode). The physical order is the mask table at
  0x519130 (START, SELECT, LEFT, RIGHT, UP, DOWN, L3, **R3 = index 7 (0x0400)**, SQUARE, CIRCLE, TRIANGLE, CROSS, L1,
  R1, L2, R2, then the four stick axes). The default tables are at 0x519170 (mode 0), 0x5191c0, 0x519210 (the pad
  constructor `FUN_0034c508` copies them, B1:336671). Mode 0: R3 -> action **4**; SQUARE 6, CIRCLE 3, TRIANGLE 7,
  CROSS 2, L1 1, R1 0, L2 0xe, R2 0xc, LEFT / RIGHT / UP / DOWN 8 / 9 / 0xa / 0xb, L3 none. No table maps a control
  to action 5.
- `FUN_0034a908` (B1:335454-335565): for actions 0..0x11 it sets bit `1 << action` of the logical record's word
  +0x10 — actions 0, 1, 3, 4, 5 while the control is down (value >= 0.125), actions 2 and 6..0x11 only on the update
  the control goes down (B1:335470-335505).
- The PlayerController update `FUN_0025e098` turns each bit into a press counter in the Controllable: for bit 4 the
  byte K+0x54 becomes 1 on the first update the bit is seen set after it was clear, counts up while held, -1 and
  down after a release (B1:180097-180150); bit 5 feeds K+0x56 the same way (B1:180152-180153). K+0x54 is the zoom
  press (section 4), K+0x56 the first-person view toggle.
- **BF2** [V]. The same mask table at 0x600e00 is followed by five mode tables (0x600e40, +0x50 each). Table 0: R3
  -> action **5**; L3 2, SQUARE 0xc, CIRCLE 4, TRIANGLE 8, CROSS 3, L1 1, R1 0, L2 0xf, R2 0xd, LEFT / RIGHT / UP /
  DOWN 0xa / 0xb / 9 / 7, SELECT 0x1c. The pad routine `FUN_002b0548` updates the Controllable's word K2+0x54 from
  bit 5 (B2:235537-235538) and K2+0x58 from bit 6 (B2:235607-235608); in a word `(w & 3) == 3` is the update of the
  press. No table-0 control maps to action 6. Table 2 gives action 5 to L3 and 2 to R3 with the stick pairs swapped;
  table 4 gives 5 to both.
- [M] BF1: R3 down seen on update 15; the next row has K+0x54 = 1 and the zoom flag 1. Held for 1.5 s: one toggle,
  nothing at the release. BF2: the row after the press has the word E+0x2c4 = 3 and the flag 1.
- Side finds [M]: in BF2 **D-pad Down reloads** (weapon state 4 one update after the press) and Square does not
  (Square is action 0xc, the word K2+0x5c); in BF1 Square reloads (state 5). `pcsx2/README.md` said "R3 = crouch
  toggle" for BF1: it is the zoom (the shoulder camera of section 5 sits lower).

## 2. The weapon class keys, and what the discs author [O]

Weapon class reader B1 `FUN_002864f0`, B2 `FUN_00328fb0` (the key is fnv1a-32 of the lower-cased name):

| key | hash | BF1 class | BF2 class | rule |
|---|---|---|---|---|
| `ZoomMin` | 0x1881b1a6 | +0x78 | +0xb0 | 0 -> 1.0; **also copied to the min turn divisor** (B1:209888-209899, B2:320430-320439) |
| `ZoomMax` | 0x2a96e8b4 | +0x7c | +0xb4 | 0 -> 1.0; also copied to the max turn divisor (B1:209974-209983, B2:320352-320361) |
| `ZoomRate` | 0x9573fb20 | +0x80 | +0xb8 | raw (B1:210277-210280, B2:320582-320583) |
| `ZoomTurnDivisorMin` | 0xb7226d43 | +0x84 | +0xbc | 0 -> 1.0 (B1:210291-210296, B2:320682-320687); name found by hash |
| `ZoomTurnDivisorMax` | 0xad37783d | +0x88 | +0xc0 | 0 -> 1.0 (B1:210309-210316, B2:320656-320661); name found by hash |
| `ZoomFirstPerson` | 0x14ec7e1d | +0x8c (word) | bit 0x33 of the u64 at +0x1e0 | bool (B1:209883, B2:320422-320426) |
| `SniperScope` | 0xf15aaeb2 | +0xe0 (word) | bit 0x34 of +0x1e0 | bool (B1:210512-210516, B2:320959-320963); no reader found (section 8) |
| `ScopeTexture` (BF2 only) | 0x32e2a37a | — | +0x84 (a name hash) | (B2:320365-320367); no reader found (section 8) |

- Constructor defaults [V]: BF1 `FUN_00285a98` sets +0x78, +0x7c, +0x84, +0x88 to 1.0 (B1:209460-209463); a class
  that authors no zoom key has `ZoomMax` 1.0 and cannot zoom (section 4).
- A divisor key written BEFORE `ZoomMin` / `ZoomMax` in an ODF is overwritten by the copy [D]; no disc class
  authors either divisor key.
- The cannon class's third spread column is named here by hash: **`StandAimSpread`** 0x5bc0137f, **`CrouchAimSpread`**
  0x67bfc1d7, **`ProneAimSpread`** 0x7d9da8bd (B1 `FUN_00289b48` 212024 / 212060 / 212069 -> class +0x1f8 / +0x204 /
  +0x210, degrees to radians; B2 `FUN_00330810` 324890 / 324900 / 324950). The constructor zeroes the nine spreads
  (B1 `FUN_002899c0` 211886, 0x24 bytes at +0x1f0; B2 `FUN_003304e0` 324648, at +0x254).
- **Disc census** (`lead_tmp/zoom/census.py`, every LVL, BASE chains resolved; `census_bf1.txt`, `census_bf2.txt`):
  0 PROP chunks of `ZoomTurnDivisorMin`, `ZoomTurnDivisorMax`, `StandAimSpread`, `CrouchAimSpread`, `ProneAimSpread`
  on either disc; every `*StillSpread` is 0.0 (79 chunks BF1, 249 BF2). So on the discs **the turn divisor is the
  zoom value** and **zooming does not change the spread** (section 7).

  | hand weapon | BF1 Min / Max | BF2 Min / Max | `ZoomFirstPerson` |
  |---|---|---|---|
  | rifle (all sides), pistol, bowcaster (BF1), commando pistol | 2.5 / 2.5 | 2.5 / 2.5 (rifle, pistol, chaingun, sonic blaster, bulldog, wrist rocket) | no |
  | shotgun, rocket / torpedo launcher, mortar launcher, arc caster | 2.0 / 2.0 | 2.0 / 2.0 | no |
  | CIS repeater, blunderbuss (BF1) | 3.0 / 3.0 | repeater 3.0 / 3.0 | no |
  | sniper rifle, Rebel and CIS (BF1); every side's (BF2) | **2.0 / 8.0** | **2.0 / 8.0** | yes (+ `SniperScope`) |
  | sniper rifle, Empire, Republic, Tusken (BF1) | 6.0 / 6.0 | award sniper rifle 6.0 / 6.0; Tusken 5.0 / 5.0 | yes |
  | bowcaster, target pistol (BF2) | — | 2.0 / 5.0 | yes |
  | award pistol and its kin (BF2) | — | 3.0 / 3.0 | yes |
  | grenades, detonators, fusion cutter, dispensers, lightsabers | none (1.0) | none (1.0) | — |

  `ZoomRate` is 0 on every hand weapon of both discs; it is non-zero only on turret, vehicle and deployed-turret
  weapons (BF1 3 classes, BF2 31 class variants: e.g. `tur_weap_laser` 2 / 8 at 1.0, and the engineer's deployed
  `*_weap_inf_autoturret`, which is the turret's gun, not a weapon in hand).

## 3. The weapon's zoom state and the toggle [O]

- A weapon instance holds its zoom value and its turn divisor: BF1 +0xd8 / +0xdc, BF2 +0xc4 / +0xc8; set at
  creation to `ZoomMin` and the min divisor (B1 `FUN_002838d8` 207932-207933; B2 `FUN_00325448` 317561-317564) [V].
- **The toggle** B1 `FUN_00283c80(weapon, flag)` (208034-208067), B2 `FUN_003259a0` (317717-317750), the same code:
  - flag 0: zoom = `ZoomMin`, divisor = min divisor, return 1;
  - flag set, zoom == `ZoomMin`, `ZoomMin` != `ZoomMax` and `ZoomRate` == 0: zoom = `ZoomMax`, divisor = max divisor,
    return 1;
  - otherwise: zoom = `ZoomMin`, divisor = min divisor, return 0.
- **The continuous zoom** B1 `FUN_00283c18(dt, axis, weapon)` (208007-208031): zoom += axis x zoom x `ZoomRate` x dt,
  clamped to [`ZoomMin`, `ZoomMax`]; divisor = min + max x t, t = (zoom - min) / (max - min) (not a blend: at the top
  it is min + max). B2 `FUN_003258f8` (317686-317714): divisor = (1 - t) min + t max when max - min > 0.0001, else
  max. Reached only with `ZoomRate` != 0: no hand weapon. BF1's has no guard on max - min [verifier]: its one
  class with `ZoomRate` != 0 and `ZoomMin` == `ZoomMax` (`hoth_weap_bldg_dishturret_cannon`, 5 / 5 at 1.0) divides 0
  by 0 for its divisor [D; a turret's matter, not read further].
- [M] BF1 marksman, `all_weap_inf_sniperrifle` (RAM: 2.0 / 8.0, divisors 2.0 / 8.0, `ZoomFirstPerson` 1): four presses
  give flag / zoom / divisor 1 / 2.0 / 2.0, 1 / 8.0 / 8.0, 0 / 2.0 / 2.0, 1 / 2.0 / 2.0. BF1 and BF2 rifle (2.5 / 2.5):
  on, off, on, off.

## 4. The soldier's zoom routine, and what cancels the zoom [O]

- **The flag**: BF1 the word O+0x2c0 (= K+0x20), BF2 the byte E+0x3e0 (= K2+0x170).
- **BF1 `FUN_00204578(dt, O)`** (B1:115287-115379) [V]:
  1. returns at once when O+0x388 (the player index) < 0 (115301-115302): **a bot's flag is never set**;
  2. (when `DAT_00519098` is set, a press of K+0x56 toggles the first-person view, 115305-115339;)
  3. the weapon is O+0xbb0[O+0xbdc], the weapon in hand of the active channel; none -> flag 0 (115345);
  4. class `ZoomMax` <= 1.0 -> flag 0 (115352, 115375);
  5. the active channel is the primary's (O+0xbd8 == 0) and the weapon's state is 5, reloading -> flag 0, return
     (115353-115356);
  6. K+0x54 == 1 (the press) -> flag = toggle(weapon, flag), O+0xbf0 = 0 (115364-115367);
  7. class `ZoomRate` != 0 -> continuous zoom with the axis K+0x60 (115370-115371).
- **BF2 `… the weapon is E+0x7c0[the
  signed 4-bit field at bits 16-19 of E+0x7e0]; **the weapon's virtual at vtable +0x5c true -> flag 0** (156856-156861);
  (the view toggle on K2+0x58, 156864-156867;) `ZoomMax` <= 1.0 -> 0 (156880, 156908); channel bits 20-21 of E+0x7e0
  zero and weapon state 4, reloading -> 0, return (156881-156884); `(E+0x2c4 & 3) == 3` -> toggle (156891-156894);
  `ZoomRate` != 0 -> continuous zoom with E+0x2dc (156900-156904).
  - The virtual returns 0 (`jr ra; move v0, zero` at 0x32b598) in 19 of the 20 weapon vtables the weapon constructors
    assign, and 1 (`li v0, 1` at 0x344f70) in vtable 0x629838, the `WeaponMelee` one [V, ELF words]: **a lightsaber
    does not zoom**.
- **Callers** [V]: BF1 `FUN_00204578` is called from the soldier update `FUN_00205cc8` (117026) and the spawn
  `FUN_0020c880`; BF2 `FUN_0023ead8` from `FUN_00240bf8` (159692) and `FUN_0024de60`. In the update the call is
  replaced by "flag = 0" when the soldier's state is
  - BF1 (O+0xbe4): 3 dead, 5 thrown, 10 or 11 rolling (B1:116889-116891, 117025-117026);
  - BF2 (E+0x7e4): 0xf or 0x10 dead, 9 thrown, 5 rolling, **3 sprinting** (B2:159686-159689).
  Every other state runs the routine: standing, crouched, prone, a jump, a fall, a jet.
- **A weapon switch** sets the flag 0 when the switched channel is the active one (B1:117834-117835 in
  `FUN_00205cc8`; B2:161183 in `FUN_00240bf8`) [V]. **The secondary's throw** makes the grenade the weapon in hand,
  and step 4 clears the flag on the next update [D][M]. **A seat change** clears it (B1 `FUN_0020b5e8` 120263; B2
  `FUN_0024af28` 165203, `FUN_0024b4d0` 165423) [V].
- Nothing restores the flag after a cancel: the next press starts again at `ZoomMin` [D][M].
- With the flag set and `ZoomRate` != 0 the two move inputs are zeroed (B1:117030-117053; B2:159702-159718): the
  stick zooms instead of walking. No hand weapon [V].
- [M] (rows in section 9): a reload cancels on the update after the weapon enters its reload state (BF1 and BF2);
  the switch on the update of the switch; the throw one update after the grenade is in hand; a roll and BF2's
  sprint on the update after the state changes. A jump, a crouch, walking and firing keep the zoom; the walking
  speed is unchanged (BF1 6.50 against 6.51 m/s, BF2 6.99 against 7.00).

## 5. The camera [O]

- **The player camera** B1 `FUN_001a9f20`, B2 `FUN_001bb510` asks the controlled entity's camera interface for a
  zoom and a field of view and eases toward both [V]:
  - zoom target = 1.0, then the interface's zoom virtual (B1:52038-52039; B2:64555-64556);
  - FOV target = the default 0x3f567751 = 0.837758 rad = 48 deg (B1 `FUN_0019e538` 43717; B2 `FUN_001b92c0` 62845),
    then the interface's FOV virtual (B1:52040-52041; B2:64558-64559);
  - **S <- target + (S - target) x expf(-25 dt)** (B1:52347-52350; B2:64893-64896) and the FOV the same way
    (B1:52353; B2:64899). `FUN_00435370` is `expf` (its error record names the string "expf" at 0x5564f8). With no
    entity S = 1.0 (B1:52029; B2:64545).
  - S divides the projection's tan(fov / 2) (`re-sky-pass.md` section 1): **vertical tan = tan(FOV / 2) x 0.7666 / S**
    [M: the camera's two tangents keep the ratio 0.76660 in every row of both games, and 2 atan of the horizontal
    one reads 48.000 deg in BF1 zoomed or not, 65.000 and 70.000 deg in BF2].
- **The soldier's virtuals** (interface vtable B1 0x532b80: +0xc / +0x10 / +0x14 are thunks 0x210ac8 / 0x210ae8 /
  0x210b08 to `FUN_0020c110` / `FUN_0020c560` / `FUN_0020c450`; B2 0x61e948: +0x10 / +0x14 / +0x18 are thunks
  0x255678 / 0x255698 / 0x2556b8 to `FUN_0024cd00` / `FUN_0024d9a0` / `FUN_0024d858`) [V, ELF words]:
  - **zoom** (B1 `FUN_0020c560` 121097-121103; B2 `FUN_0024d9a0` 167226-167233): the weapon's zoom value when the
    flag is set, there is a weapon in hand and the soldier is alive; else 1.0;
  - **FOV** (B1 `FUN_0020c450` 121046-121080; B2 `FUN_0024d858` 167157-167205): dead -> the default; flag clear ->
    the Controllable class's +0x1c0 in third person, +0x1bc in first person; **flag set -> +0x1bc**.
  - The class keys (B1 `FUN_0023da58`, B2 `FUN_002f6eb8`): `FirstPersonFOV` 0x4785d6b7 -> +0x1bc, `ThirdPersonFOV`
    0x20728c12 -> +0x1c0, degrees x 0.017453294 (B1:156086-156088, 156112-156114; B2:285254-285256, 285279-285281);
    the constructor sets both to the default 48 deg (B1 `FUN_0023d778` 155819-155821; B2 `FUN_002f6ce0`).
  - Disc (`lead_tmp/zoom/census_fov.py`): no BF1 soldier class authors either key (59 of 59): **48 deg zoomed or
    not**. BF2: 156 of 162 soldier class variants resolve to `FirstPersonFOV` 70 / `ThirdPersonFOV` 65 (through
    `com_inf_default`); the other six (gamorrean guard, ewoks, jawa, an agro geonosian) author neither.
  - So the zoomed vertical FOV is 2 atan(tan(24 deg) x 0.7666 / z) in BF1 and 2 atan(tan(35 deg) x 0.7666 / z) in
    BF2: rifle 15.55 deg / 24.24 deg [M both], sniper 19.37 deg [M] then 4.89 deg (BF1), 30.05 deg then 7.68 deg
    (BF2) [D].
- **Where the camera is** (B1 `FUN_0020c110`, B2 `FUN_0024cd00`) [V]:
  - third person: the camera section is class + 0x1920 + posture x 0xc0 + **flag x 0x60** (B1:121015; the getter
    `FUN_0020b180` 120008), B2 class + 0x8e0 + index x 0xc0 + flag x 0x60 (B2:166889-166913): the stance's ZOOM section,
    whose values `re-aim-assist.md` section 10.1 read from RAM: TrackOffset (0.4, 0.05 stand / 0.2 crouch / 0.2
    prone, 2.8), tilt 3.5 deg, both tensions FLT_MAX (taken at once);
  - **the scope view**: flag set, a weapon in hand and B1 `FUN_00283d18(weapon)` / B2 `FUN_00325a30` true
    (B1:120927; B2:166775) -> the first-person frame: the eye = the soldier's aim point (K+0x100; BF2 the posture's
    point) less **0.3 m** along the aim (`DAT_005178b4`, B1:120984; ELF 0.3), in BF2 **0.6 m when the flag is set**
    and 0.3 in the plain first-person view (B2:166847-166850); the camera looks along the aim, no tilt. Two
    more conditions [verifier; reproduced]: the branch is not taken while "first person suspended" is up — BF1 the
    word O+0xc4c (B1:120931; set by `FUN_00209758` on the change to state 5, thrown, in first-person view,
    B1:118885-118889; cleared back in state 0 once the busy timer is out, B1:116574-116579), BF2 bit 0x8000 of
    E+0x4e0 (B2:166779; set on the change to state 9 or 10, B2:162819-162823) — and a set flag whose weapon fails
    the test still takes this frame when the player's view record says first person (the unzoomed rule);
  - `FUN_00283d18` (B1:208070-208100): the class's `ZoomFirstPerson` (208080), or an owner whose entity-type virtual
    (+0xd4) is neither 1 nor 4 (208089-208094). The type virtual returns 1 for `EntitySoldier` and 4 for
    `EntityDroideka` (ELF: `li v0, 1` at 0x211f40, `li v0, 4` at 0x1d6660; flyer 2, walker 3, droid 6, hover 7):
    **a soldier's zoom is the scope only with `ZoomFirstPerson`; a vehicle's weapon always zooms in first person.**
    B2 `FUN_00325a30` (317753-317784) is the same test (bit 0x33, type virtual +0xe0; the soldier's returns 1).
  - the body: B1 `FUN_0020cf48` (121537-121585) answers "drawn" first when O+0xc4c is set or the soldier is dead
    (121549-121551), then "not drawn" for the player's own viewport while the flag is set on a `FUN_00283d18`
    weapon (121552-121570), then the first-person rule of the view record.
- The aim follows the camera path already read (`re-aim-assist.md` section 10.2): in third person the aim origin
  and the reticle's place come from the ZOOM section [D: the reticle sits tan(3.5 deg) x S / (vertical tan) of the
  half height above the centre: 0.448 for BF1's rifle, 0.285 for BF2's]; in the scope view the aimer takes the fire
  point and the aim itself, so the shot goes where the centre of the scope looks [V there, step 7].
- [M] BF1 and BF2, pitch -4.01 deg: the eye relative to the feet in the camera's yaw frame (right, up, back) is
  (0.000, 2.348, 3.457), tilt 5.00 deg (BF1) and (0.000, 2.575, 3.105), tilt 10.00 deg (BF2) unzoomed, and
  **(0.400, 2.216, 2.769), tilt 3.50 deg in both games on the first update of the zoom** — the STANDZOOM section to
  the millimetre. Zooming out the eye eases by 0.606 per update (the stance section's MoveTension 15: exp(-15 dt))
  and BF1's tilt by 0.367 (AimTension 30); BF2's tilt returns at once. BF1 marksman in the scope: (0.060, 1.604,
  0.057), tilt 0.00 — the fire point of `re-aim-assist.md` section 10.2 (1.7 m up, 0.25 m along the aim, 0.10 m
  below it, 0.06 m right) less 0.3 m along the aim: (0.060, 1.604, 0.057) by the arithmetic.
- [M] S per update after a press: BF1 1.0000, 1.8486, 2.2171, 2.3772, 2.4467 ... (factor 0.4342 = exp(-25 x
  0.03336)); BF2 1.0000, 1.8481, 2.2167 ... (factor 0.4346 = exp(-25 / 30)); the marksman's second step 2.0000,
  5.3947, 6.8687 ... toward 8. BF2's vertical tan eases 0.48838 -> 0.51540 -> 0.52742 ... -> 0.53678 = tan(35 deg) x
  0.7666 (the FOV itself is what is blended: 70 + (65 - 70) x 0.4346 = 67.83 deg -> 0.5153).

## 6. The turn rate and the aim assist [O]

- **Yaw** [V]: B1 `FUN_002098b8` 119371-119375, B2 `FUN_00247fa8` 162921-162926: with the flag set, a weapon in
  hand and a player index >= 0 the turn rate is **divided by the weapon's turn divisor**.
- **Pitch** [V]: B1 `FUN_00205cc8` 117164-117180 (the pitch factor, class +0x1844 or 1.0, divided by the divisor),
  117190 (stick x factor), 117321 (pitch += that x dt); B2 `FUN_00240bf8` 159984-159991.
- **Aim assist** [V]: B1 `FUN_0025fea8` 181019-181020: with K+0x20 set the assist's turn rate R is divided by the
  same divisor (`re-aim-assist.md` section 5 had this as "weapon+0xdc, the zoom factor").
- [M] BF1 rifle (divisor 2.5): yaw, right stick held 1 s, 47.7 deg/s rising 9.03 deg/s per update to 309.4 deg/s
  unzoomed; 19.1 rising 3.6 to 123.8 zoomed — 1 / 2.497 to 1 / 2.499 on every update. Pitch 38.7 deg/s against
  15.5. Marksman at zoom 2: yaw 23.9 .. 154.7 (1 / 2.00), pitch 19.3. BF2 rifle: yaw 64.9 .. 286.2 against 25.9 ..
  114.4 (1 / 2.50); pitch 57.2 against 22.9.
- Not this card's, seen in the same rows [M]: a human's yaw rate is not flat. It starts at the pitch rate's base
  and rises linearly to the class's turn speed over the first second of a held stick (B1:119344-119345: class
  +0x1844 + (class +0x1840 - class +0x1844) x a ramp in the Controllable; B2 `FUN_00247fa8` 162907-162920), and BF1
  scales it by the sensitivity setting (0.19 x n + 0.4, `FUN_0034caf0`). The port turns at `max_turn_speed` flat.
  Read through by card LOOKR (2026-10-22): `re-look-ramp.md` (the ramp climbs only at a FULL look stick).

## 7. Spread [V]

The cannon's posture spread takes its base from the `*AimSpread` column instead of `*StillSpread` while the zoom
flag is passed (`re-blaster-damage.md` section 4.2; the names are section 2's). Both columns are 0.0 on every disc
weapon: **zooming changes no spread**. The AI's sniper table (`re-ai-sniper.md`) reads `ZoomMax` as a class test
only; a bot has no zoom flag.

## 8. The scope display, its sounds, `ScopeBlur` [O]

- **BF1 `ScopeDisplay`** (HUD element, vtable 0x540880; update `FUN_002d66a8` B1:262837-262969) [V]: shown while the
  viewport is live, the unit alive, the Controllable's zoom flag set, the weapon a `FUN_00283d18` one or the view
  in first person (262930-262939) **and the camera's S > 1.0** (262940-262942). It is four quads,
  each half the screen, with the texture rectangle mirrored left / right and top / bottom (`FUN_002d6138`
  262694-262704), textured `weapon_scope` (`FUN_002d6440` 262753; the string at 0x540870): **one quarter picture
  mirrored into the four quadrants.** `lead_tmp/zoom/weapon_scope_bf1_mirrored_on_green.png` shows it: an opaque
  dark surround, an octagonal window (alpha 57 of 255 at its clearest), range ladders and a centre ring with ticks.
- **Sounds** [V]: while the display is up, a rising S starts the zoom-in sound and a falling S the zoom-out sound
  (262958-262966 and the lines after). `SetSoundEffect` names them `ScopeDisplayZoomIn` 0x4d9f349b, `ScopeDisplayZoomOut` 0x414b5348,
  `ScopeDisplayAmbient` 0x651e07ad (B1:32284 / 32298 / 32313; B2:45322 / 45284 / 45334). The missions set
  `binocularzoomin` / `binocularzoomout` (43 of 44 BF1 mission scripts, 102 of 128 BF2) and never the ambient. They
  belong to the scope display: **the shoulder zoom of a rifle is silent** [D].
- **BF2** (vtable 0x62e9a0; update `FUN_0038f558` B2:388122-388293) [V]: the same test (388231-388245: the flag byte
  K2+0x170, `FUN_00325a30`, S > 1.0 at camera +0x140) and the same four mirrored quads (`FUN_0038f050`
  387970-387987), textured with the hash 0xf6bcfa8e = fnv("weapon_scope") (`FUN_0038f340` 388033). The sounds start
  once per zoom movement (a latch at +0x3a0, 388245-388278) which the soldier's zoom virtual clears when the zoom
  value changes (B2:167250). BF2 alone also hides the display while the flag `FUN_004241a0` returns is set
  (`DAT_00600b3c`, ELF 0; set in `FUN_00425840`) [verifier].
  - **`ScopeTexture` is not applied on the PS2** [D]: the class constructor defaults class +0x84 to
    fnv("weapon_scope") (B2 `FUN_00327e98` 319492), the quad's set-texture call on the display's first quad has
    one site in the ELF (0x38f358, the constant), every code site that loads the display table 0x661368 lies in a
    function read here, and the disc holds ONE scope texture chunk, `weapon_scope` (INGAME.LVL); `weapon_scope2` /
    `3` / `4` occur only as ODF values (54 / 24 / 1). BF1's is in COMMON.LVL.
  - While the display is up BF2's HUD update switches one HUD group (B2:411733 `FUN_003b61e0`); which elements was
    not read [H].
    **Read 2026-10-28 (scope playtest) [V]:** the byte is the display's +0x379 (*(decompiler detail omitted)*, B2:411737; `FUN_0038f340`
    clears it). A change of it calls `FUN_003b6b50` (B2:411738-411741). Then *(decompiler detail omitted)* (*(decompiler detail omitted)*:
    the first word of both channels' HUD cache, 0x6a9050 + ch x 0x28, is 0 after `FUN_003b3398`; its meaning was not
    read) picks the event sent on a change of *(decompiler detail omitted)*: +0x6a9338 = `player%d.weaponsDisable` (string 0x6314b8)
    when set, +0x6a9334 = `player%d.weaponsEnable` (0x6314a0) when clear (registration B2:408037-408040;
    B2:411756-411767). So **the weapon panel group `player1weaponinformation` (and the overheat text) goes off in the
    scope**. While scoped the health / energy senders `FUN_003b4bb8`, `FUN_003b5708`, the in-vehicle health
    `FUN_003b4668` and the jet fuel `FUN_003b4e70` are skipped (B2:411769-411784: those bars keep their last
    values); the target publisher `FUN_003b3398` (B2:411747-411755), the reticle placer and the map senders
    `FUN_003b5020` / `FUN_003b51b8` run either way, so **the target readout stays in the scope**. Whether the
    reticle is drawn over the overlay was not checked on a frame [H]. Ported: `ui::hud_scope`.
- **`Effect("ScopeBlur")`** [V for the hook]: BF1's `ScopeBlurEffect` (vtable 0x549170, constructor `FUN_0037a770`
  B1:367754) owns the per-viewport flag the display raises (`DAT_005194f0`, B1:262813). Blocks exist in BF1 END1,
  GEO1 and BF2 END1, FEL1, MUS1, GEO1, TAT3; what the effect draws was not read [H].

## 9. The live check [M]

`lead_tmp/zoom/ps2/zoom_probe.py bf1|bf2 [--class-right N] [--only ..] [--tag ..]` (setup and sampler from
`audit-jgate/ps2/gate_probe.py`; no shared lock; ~2.5 min a run), digests by `zoom_read.py` and `eye_read.py`, rows
in `lead_tmp/zoom/ps2/out/` (70 files). Six runs: BF1 soldier (`all_inf_soldierurban`, rifle 2.5), BF1 pilot
(shotgun 2.0), BF1 marksman (`all_inf_marksman`, sniper rifle 2 / 8), BF2 rifleman three times (rifle 2.5).

| trial | BF1 | BF2 |
|---|---|---|
| R3 tapped four times | flag 1, 0, 1, 0 (rifle); 1 / 2.0, 1 / 8.0, 0, 1 / 2.0 (marksman) | flag 1, 0, 1, 0 |
| R3 held 1.5 s | one toggle on the press | one toggle |
| reload while zoomed | Square: state 5 on update 79, flag 0 on 80 | D-pad Down: state 4 on 127, flag 0 on 128 |
| primary switch (R2) | flag 0 on update 40, slot 1 on 41 | flag 0 on 40, slot 1 on 41 |
| secondary (L1) | channel 1, grenade in hand on 40, flag 0 on 41 | the same |
| roll | state 10 on 46, flag 0 on 47 | state 5 on 49, flag 0 on 50 |
| sprint | — | state 3 on 46, flag 0 on 47 |
| jump, crouch, walk, fire | flag stays 1 | flag stays 1 |

Not obtained: a BF2 sniper (the class screen's cursor did not move with the keys tried), so BF2's second step and
its 0.6 m scope eye are [V] only; a frame of the scope overlay.

## 10. What the port has, and what a build needs

**Built 2026-10-07 (card ZOOM): items 1-4 below**, without the continuous zoom of a `ZoomRate` class (no hand
weapon), the eased return of the eye on the zoom out (the stance sections' MoveTension, card CAMS) and the "first
person suspended" gate (the port has no thrown state); item 5 is card ZSCP. Before the build:
`buttons::ZOOM` (R3) has no consumer; `WeaponClass` reads only `ZoomMax`; `aim_assist` takes a
`zoom_factor` nobody sets; `Weapon::set_posture` takes a `zoomed` flag nobody sets; `camera_section(rules, stance)`
has no zoom sections; the chase FOV is the measured unzoomed tan (BF1 0.34131, BF2 0.48838, card CFOVM's
"writers unread" — they are section 5's); `weapon_scope.ktx2` is baked for both games.

1. **Class data** (`core/src/weapon.rs`): `ZoomMin`, `ZoomRate`, the two divisors with the copy rule and the 0 ->
   1.0 rule, `ZoomFirstPerson`; the `*AimSpread` names for the third spread column.
2. **Sim** (`core/src/unit.rs`, `core/src/sim/step.rs`, `core/src/sim/input.rs`): per human unit a zoom flag and per
   weapon its zoom value and divisor; section 4's routine on the press edge of `ZOOM`, in its order, with its
   cancels (reload of the primary, switch of the weapon in hand, the secondary's throw, roll, thrown, dead; BF2
   sprint); bots never. The flag goes to `set_posture` and the divisor to the aim assist.
3. **Turn** (`core/src/soldier.rs`): yaw and pitch rates divided by the divisor while zoomed.
4. **Camera** (`core/src/unit.rs` `camera_section`, `game/src/walk.rs`): the ZOOM sections (taken at once); S eased
   by exp(-25 dt) toward the weapon's zoom and applied to the PS2 tan; BF2's base FOV eased 65 -> 70 deg; the
   scope view for `ZoomFirstPerson` weapons (the eye at the fire point less 0.3 m, BF2 0.6 m; no tilt; own body
   not drawn); the aim ray and the reticle through the same section.
5. **Scope display** (its own card, ZSCP): the mirrored `weapon_scope` overlay while zoomed on a scope weapon with
   S > 1, the zoom-in / zoom-out sounds, `ScopeBlur`.

Acceptance a build can run: the press sequence per weapon from the baked classes (rifle on / off; sniper 2, 8, off);
each cancel; yaw and pitch rates = plain / divisor; S after n ticks = z + (1 - z) exp(-25 n dt); the zoomed FOV
(BF1 rifle 15.55 deg, BF2 24.24 deg) and eye (0.400, 2.216, 2.769 at pitch -4.01 deg); the scope eye (0.060, 1.604,
0.057); a PCSX2 pair for the shoulder zoom and for the scope.

## 11. Checks run, what stays [H]

- **Check 1** (`re_check.py fn` by `lead_tmp/zoom/cite_check.py` over `cites.txt`): 242 cited lines, each in the
  function named (three function names corrected by the check before writing).
- **Check 2** (absence): `re_check.py absent` for `DAT_005194f0` (3 hits: the display's two setters and the blur
  effect's constructor) and `DAT_006613b8` (BF2 twin, 3 hits); `refs.py` for the callers of `FUN_0020c110`,
  `FUN_0020c450`, `FUN_0020c560`, `FUN_0020cf48` (one thunk each) and BF2's three; a raw ELF scan for every code
  site loading 0x661368 (17 `lui` / low-half pairs by this scan, 11 sites by the verifier's; all in functions
  read) and for the set-texture virtual on the display's quads (1);
  `propscan.py` for the five unauthored keys (0 chunks on both discs).
- **Check 3** (the twin): every BF1 function of sections 3-6 and 8 has its BF2 twin opened and cited beside it; the
  differences found are BF2's melee test, sprint in the cancel list, the 0.6 m scope eye, the blended divisor and
  the sound latch.
- **Check 4** (bytes): the pad mask and mode tables of both ELFs (`lead_tmp/zoom` session log; 0x519130, 0x519170,
  0x600e00, 0x600e40); 0x3f567751 = 48.0 deg; "expf" at 0x5564f8; "weapon_scope" at 0x540870; the vtable slots of
  section 5 and the type virtuals; the melee virtual.
- **Check 5** (writers): the zoom flag's writers are section 4's (B1: `FUN_00204578`, `FUN_00205cc8` x 2,
  `FUN_0020b5e8`, the seat function `FUN_0020b2c8`, and six state readers that store a received bit, B1:317457,
  318049, 318567, 319094, 319358, 319577 [verifier]; B2: `FUN_0023ead8`, `FUN_00240bf8` x 2, `FUN_0024af28`,
  `FUN_0024b4d0` x 3), by a text search of both offsets' forms, not an ELF store scan. The toggle has seven `jal`
  sites per game: the soldier's and six other controllables' (vehicles, turrets), not read here.
- **Verifier**: one Opus agent in the foreground, given `lead_tmp/zoom/verify/claims.md` only (52 claims A1-H8);
  verdicts in `lead_tmp/zoom/verify/VERDICTS.md`. **48 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED.** It re-derived every
  live number from the raw rows, re-ran both disc censuses with its own walker and read every cited ELF word. The
  PARTLY items are folded in and reproduced: F9 / F11 (the scope frame's "first person suspended" gate, and the
  view record's alternative), F12 (the body test's first gate), H6 (the tangent ratio is 0.76660, not 0.76659).
  Its extra finds are in sections 3, 5 and 8 marked [verifier].
- **Stays [H] / not read**: what `ScopeBlur` draws (what BF2 hides under the scope: read, §8); whether the reticle
  is drawn inside the scope; `SniperScope`'s and `ScopeTexture`'s readers (none found in the decompile text — a
  reader inside an undecompiled function is not excluded for `SniperScope`); the camera helper `FUN_0057ab20`'s
  16.0 / 4.0 above S 1.01 (no caller in the text); the listener rule that reads the scope flag in the camera
  manager (B2 `FUN_001ba080`); BF2's sniper live.
