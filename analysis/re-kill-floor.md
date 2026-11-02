# OPUS RE (2026-10-01, Fable chat session) - [O] UNAUDITED: no verifier pass; Fable re-read §0 rows 2a (B1:116586-116603, B2:159373-159398) and 1e/1f (B2:368956) in the decomp and they say what the table says

2026-10-01, Opus, analysis only (no tracked file touched). Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`,
B2 = `analysis/SWBFRNT2.ELF-decomp.c`; disassembly with capstone where Ghidra drops register arguments.
Tags: [V] read in the decomp or the disassembly, [D] deduced from [V] pieces, [H] not confirmed.

## 0. Answer table

| # | Question | BF1 (B1) | BF2 (B2) | Conf. |
|---|---|---|---|---|
| 1a | What `AddDeathRegion` stores | Region manager `DAT_00518398`: 32 slots `{region*, type}` at +0x18; death = type 0 (`FUN_002af2c0` → `… type 1 = `water_sink_region`, type 2 = `AddLandingRegion` | [V] |
| 1b | Who reads it, and how often | `FUN_002af1c0`, slot +0xc of the manager's update vtable (word at 0x53ef7c); returns 1, so it runs every frame | `FUN_00371148`, slot +0xc of vtable 0x62d120 (word 0x62d12c). It is registered on the per-frame update list (`FUN_001af7e8(obj, -10, 2)`) and dispatched by `FUN_001af598` | [V] (per-frame: [D]) |
| 1c | Which entities are tested | Broadphase mask **6** = layers 2 and 4 (soldiers + vehicles), so AI and humans alike | Broadphase mask **0xb** = layers 1, 2 and 8: soldiers (2), droids/droidekas (2), hovers, walkers and the other layer-8 vehicles. **Not** the layer-4 class (ctor `FUN_001d0eb0`) and not buildings (0x10). AI and humans alike | [V] masks; [D] class→layer |
| 1d | Which point is tested | One **point**: the entity's matrix position, i.e. the soldier's **feet** (`s+0x230`; soldier vtable +0x40 = `s+0x200`, and +0x30 of that is the position row), tested point-in-shape | A **sphere**: the entity's collision bounding sphere (centre at `E+0x20`, radius in w). A box region kills when the distance from the sphere centre to the box is less than the radius, so it is a touch test | [V] |
| 1e | How it kills | `FUN_002af0d0` → `FUN_001bbc98(E+8, {0,0,0,0}, 0)`: instant kill, health = 0, no killer | `FUN_00370fd8` → health vtable +0x20 = **`FUN_001df640`** (desc from `FUN_001df050(1.0, d, 0,0,0,0,0)`, no killer), third argument **0** | [V] |
| 1f | Invincibility on that path | *(decompiler detail omitted)* skips BF1's developer `/invincible` test (`DAT_00519060`): the region kills an "invincible" human | *(decompiler detail omitted)* skips the `sos` test (`DAT_00600b1c`): **the death region bypasses the cheat** | [V] |
| 2a | Hard-coded out-of-world rule (soldier tick) | `FUN_00205cc8`: **y < −500**, \|x\| > 1000, \|z\| > 1000 or y > 500 → health vtable +0xc (`FUN_00211200` → `FUN_001ffec0`, soldier die) | `FUN_00240bf8`: **v.y < −60**, **y < −1000**, \|x\| > 2500, \|z\| > 2500 or y > 1000 → health vtable +0xc (`FUN_00255d00` → `FUN_002379f0`, soldier die) | [V] |
| 2a' | Water sink height (BF1 soldier) | y < manager+0x118 (the y of the plane region `water_sink_region`; default −FLT_MAX) → `FUN_001bbc98(.., 0)` kill | The soldier tick does **not** read +0x118. Only two other classes do (`FUN_001ee668`, `FUN_001f8548`) | [V] |
| 2b | Fall timer kill | None. The airborne timer only changes stance (BF1 → stance 6, BF2 → stance 8) | None | [V] (the full call list in §2.3) |
| 2c | Landing damage | (approach − T)² × S through TakeDamage; defaults T = (10, 3, 0), S = (1, 0, 0) | the same law and the same defaults | [V] |
| 2d | Map boundary | 2-D (x, z only) | 2-D (x, z only) | [V] |
| 2 | **bes1** (no region, no terrain, no water sink) | A soldier who walks off a platform falls until **y < −500**, then the tick kills him. Landing damage never fires, because there is nothing to land on | — | [D] |
| 3 | Hidden decks at KAM1 (BF2 trooper, 300 HP) | — | Landing damage = (v_n − 13)² for a flat floor (n.y = 1). Drops of 8 / 15 / 25 m cost about **16 / 80 / 181 HP** (with the Fall-state drag). A drop of about **40 m** is lethal (≈ 25.5 m if there were no drag). Nothing else on the PS2 kills a soldier who is standing there | [V] law and constants; [D] numbers |

## 1. `AddDeathRegion` and its consumer

### 1.1 The binding and the store [V]

B2 `AddDeathRegion` 0x18d628 (B2:33182-33192) → `FUN_00371338(DAT_005feea0, name)` → `FUN_00371480(mgr, name, 0)`
(B2:369113-369137). `FUN_00371480` (B2:369208-369246):
*(decompiler excerpt omitted from the public copy)*
Types: 0 = death (`FUN_00371338`), 1 = `FUN_00371410`, 2 = landing (`FUN_003713a8`, `AddLandingRegion`). The only
caller of type 1 is `FUN_00461868` B2:524155 `FUN_00371410(DAT_005feea0, 0x63a6a0)`, where 0x63a6a0 = the string
`'water_sink_region'` (`re_check word b2 0x63a6a0 --str`). Manager ctor `FUN_00370df0` (B2:368859-368871):
vptr 0x62d120, `FUN_001af7e8(obj, -10, 2)`, slots zeroed, `+0x118 = 0xff7fffff` (−FLT_MAX).

The B1 twin is `AddDeathRegion` 0x1857a8 (B1:27539-27550) → `FUN_002af2c0(DAT_00518398, name)` → `FUN_002af380(.., 0)`
(B1:239630-239685). It has the same body, including the plane/type-1 → +0x118 branch. B1 type 1 = `FUN_002af320`; its only
caller is `FUN_0037cb08` B1:369362 `FUN_002af320(DAT_00518398, 0x5495b0)`, and 0x5495b0 = `'water_sink_region'`.
Default +0x118 = 0xff7fffff (B1:239313).

### 1.2 The per-frame consumer [V]

B2 `FUN_00371148` (B2:369033-369110) is not called directly (`callers` = 0). It is the word at 0x62d12c, which is
vtable 0x62d120 + 0xc (`word b2 0x62d100 24`). The update list calls slot +0xc: `FUN_001af598` B2 (*(decompiler detail omitted)*; return 0 = unlink). `FUN_001af640`/`FUN_001af988` walk those lists,
and `FUN_001af988` is called from the frame loop `FUN_00425840` (B2:482549/482569) [D: once per frame].
*(decompiler excerpt omitted from the public copy)*
B1 `FUN_002af1c0` (B1:239540-239578; vtable word at 0x53ef7c): the same loop, with `FUN_002aeee0` → `FUN_002af0d0` for
types 0 and 1. It returns 1.

### 1.3 Which entities and which point [V]

**BF2.** `FUN_00370fa0` (B2:368941) is `FUN_00161b50(region, out, max, a3, t0)`. Ghidra drops the register arguments,
so here is the disassembly:
```
0x370fac move $a3, $zero          ; ignore-entity = none
0x370fb0 jal 0x161b50
0x370fb4 addiu $t0, $zero, 0xb    ; layer mask 0xb
```
`FUN_00162df0` (B2) builds the AABB of the region's bounding sphere (region+0x30 centre, +0x4c radius). It then walks the
collision grid (`mgr+0xcc`, created unconditionally by the collision init `FUN_00161df8` B2:1802-1805) with callback
`FUN_00162cd0`:
*(decompiler excerpt omitted from the public copy)*
`FUN_00536f10` (B2:666401-666600) switches on the region shape. Sphere (1): `|c − p|² < (R + r)²`. Box (2): `p` goes into
the box frame, then `FUN_005381a8(r, half, local)` returns `dist(local, box)² < r²` (B2:667201-667240). The disassembly
shows that $f12 (the radius) is never written inside `FUN_00536f10` before the `jal 0x5381a8`. So **a box region kills
when the sphere touches it.** Per re-hover §1, E+0x20 is the bounding sphere written by SetTransform. For a soldier, the
centre's height above the feet and the radius were not read [H: about body centre, r ≈ 1.1 m, so the sphere bottom is
near the feet].

Layer ids (re-collision-layers §4/§5, re-hover §7). The base-entity ctor `FUN_00267bf0` takes the layer in a3, by
disassembly: soldier `FUN_00234a68` a3 = 2; droideka `FUN_001f2618` 2; `FUN_001ecba0` 2; hover `FUN_00219918` 8;
`FUN_001fff10` 8; `FUN_00232930` 8; walker `FUN_00259970` 8; `FUN_001d0eb0` **4**; the `FUN_001e2d40` family 0x10;
EntityLight `FUN_00224fb8` 1. So mask 0xb tests every soldier-like and every layer-8 vehicle, and skips the class built
by `FUN_001d0eb0` [D: which vehicle type that class is was not identified].

**BF1.** `FUN_002aeee0` (B1:239372-239463). The broadphase is `FUN_00161ba0(min, max, out, max, 0, 6)`: mask 6 = layers
2 | 4 = soldier | vehicle in BF1. A plane region (kind 4) gets a ±10000 box (0x461c4000). The callback `FUN_00162538`
returns the owner entity. Each candidate is then tested by `FUN_002af080`:
*(decompiler excerpt omitted from the public copy)*
`FUN_004c2b08` (B1:584689-) box case: `|row_i · (p − c)| < half_i` for i = 0, 1, 2. It is a pure point test. For the
soldier, the main vptr is 0x532d50 (stored by soldier ctor `FUN_001fee80` B1:111412), and its +0x40 = 0x532d90 =
`FUN_00211c00`:
```
0x211c00 jr $ra
0x211c04 addiu $v0, $a0, 0x200      ; matrix at s+0x200 -> +0x30 = s+0x230 = feet (re-soldier-ground table)
```
So **BF1 tests the soldier's feet point**. For vehicles the point is their matrix position [D].

### 1.4 The kill and the cheat [V]

B2 `FUN_00370fd8` (B2:368956-368980):
*(decompiler excerpt omitted from the public copy)*
Slot +0x20 of every health vtable is `FUN_001df640`. Ctors store `entity[0x8b] = vptr`, and vptr+0x20 = 0x1df640 holds
for all 28 classes listed by `grep "\[0x8b\] = &"` (B2:79977 … 477798). Example: the soldier's vptr is 0x61ea80 (ctor
`FUN_00234a68` B2:149406) and `word b2 0x61eaa0` = 0x001df640. The one exception is vptr 0x62c1d8 (ctor
`FUN_0035cb68`), whose +0x20 = `FUN_0035f3b8`.

`FUN_001df640` (B2:91078-91140):
*(decompiler excerpt omitted from the public copy)*
With `argument 3 = 0` the invincibility block is skipped. **The death region kills an invincible human.** The absence
check:
```
$ re_check.py absent b2 DAT_00600b1c
'DAT_00600b1c' in B2: 2 hit(s) in 2 function(s)
   FUN_001df640: lines 91114
   FUN_001dfb38: lines 91288
```
B1 `FUN_002af0d0` (B1:239484-239500): `FUN_001bbc98(argument 2 + 8, {0,0,0,0}, 0)`. `FUN_001bbc98` (B1:64127-64180) runs
the `DAT_00519060` (`/invincible`, developer-only) test only when `argument 3 != 0`, then sets health = 0 and calls vtable
+0x24.

Two more paths reach the same kill. Both are one-shots, not the per-frame test:
- B2 `FUN_00371090(mgr, region)` (B2:368985-369030) kills everything in a region once. Its only caller is
  `FUN_00354e10` B2:350151 (region from object+0x8c, after a spawn loop) [H: a CP/spawn clear].
- B1 `FUN_002af108`, caller `FUN_0029cc00` B1:226465.

## 2. Maps with no death region: the hard-coded out-of-world rule

### 2.1 BF2 soldier tick `FUN_00240bf8` (caller `FUN_00255838` B2:172360, thunk −0x270) [V]

B2:159373-159398. It runs only when this machine is not a network client:
*(decompiler excerpt omitted from the public copy)*
Soldier health vtable 0x61ea80 + 0xc = 0x255d00 → `FUN_002379f0(s)` (B2:151519-). That function clears the alive bit
(`a local[0x8a] &= ~8`), rolls the powerup drop and sets the death stances 0xf / 9 / 0x10. It never reads
`DAT_00600b1c` (absence above), so this path also bypasses the cheat. It does not go through `FUN_001df640`, so no damage
descriptor and no killer [D].

The v.y < −60 test is effectively unreachable in a plain fall [D]. The Fall-state integrator `FUN_00248560`
(B2:163191-) applies `v *= exp(−0.012·|v|·dt)` to the whole vector and `v.y −= 18·dt`, which gives a terminal speed of
√(18/0.012) = 38.7 m/s. It can matter only for launched soldiers (explosions, jet packs).

### 2.2 BF1 soldier tick `FUN_00205cc8` (caller `FUN_00210bf0` B1:123817) [V]

`a local = argument 2 + 0x8c` (B1:116558) = s+0x230 = feet. B1:116586-116613:
*(decompiler excerpt omitted from the public copy)*
The soldier's health vptr is 0x532d20 (`a local[0x2c]`, B1:111404), and +0xc = 0x211200 → `FUN_001ffec0(s)` (B1:112062).
That function clears the alive bit (`a local[0x2b] &= ~8`), the twin of B2 `FUN_002379f0`. BF1 has no velocity test.

`water_sink_region` exists in the BF1 LVLs MISSION, END1, KAM1, BES2, YAV2, YAV2_H and in the BF2 LVLs MISSION, END1,
DEA1, GEO1 (byte grep of `extracted/*/lvl`). **BES1 has none.** BF2's soldier tick never reads the sink height (readers
of `DAT_005feea0 + 0x118`: only B2:101538 `FUN_001ee668` and B2:109366 `FUN_001f8548`).

Other classes have their own floors [V, classes not all named]:
- B1 y < −500: `FUN_001ea9f8`, `FUN_00219ed8`, `FUN_001f8410` (B1:107122).
- B2 y < −500: `FUN_001eac88`, `FUN_0021b470` (hover), `FUN_001ee668`, `FUN_001f8548`, plus `FUN_00225578`,
  `FUN_0022b2f8`, `FUN_0022d1e0`, `FUN_0025f4a0` (walker), `FUN_00366258` (all hits of `< -500.0` / `0x154) < -`).

### 2.3 No fall-timer kill [V]

Every health or damage call inside the two soldier ticks:
- B1 `FUN_00205cc8` (116426-118145): 116589-116603 (the four world-bounds kills), 116612 (water sink), 116911-116965
  (corpse/death handling of an already-dead soldier, `0x0020688c`), and 117240 `FUN_001bc8e0` (water damage, below).
- B2 `FUN_00240bf8` (158414-161738): 159377-159395 (the five bounds kills), 159614-159666 (the corpse path), 160040
  `FUN_001e0900` (water damage), and 161055.

None of them is keyed on the stance timer. The timers s+0xbec (BF1) and s+0x7ec (BF2) only drive `SetStance`
(re-soldier-ground §1.6a).

Not asked, but it is environmental. With the water surface above the soldier by more than a class depth (B2 `FUN_005613c8`
water query, B2:159994-160040; B1 `FUN_004768f0`, B1:117186-117240), a timer (class+0x1218 / +0x1e54) deals periodic
damage through **TakeDamage** (`FUN_001e0900` → `FUN_001dfb38`, so the cheat is honoured). A desc field is set to 30.0
[H: 30 damage per tick of that timer; drowning].

### 2.4 Landing damage (both games) [V]

B2 `FUN_0023cfd0` B2:155715-155866. The approach is `v_rel · n`. Against a layer-0x10 (building) contact it is multiplied
by contact+0x14 = n.y.
*(decompiler excerpt omitted from the public copy)*
Defaults are set by the soldier class ctor `FUN_00250488` (its +0x1c = class vptr 0x61e8d8), B2:169419-169425:
`+0x7dc = 10.0, +0x7e0 = 3.0, +0x7e4 = 0; +0x7e8 = 1.0, +0x7ec = 0, +0x7f0 = 0`. ODF keys (soldier class property setter
`FUN_00252bb0`): `CollisionThreshold` (0x2cdb588a) → +0x7dc/+0x7e0/+0x7e4 (B2:171651-171652); `CollisionScale`
(0x1a87bacf) → +0x7e8/+0x7ec/+0x7f0 (B2:171547-171548).

BF1 `FUN_00202f38` B1:114478-114540 has the same law: class+0x1850/54/58 and +0x185c/60/64, and `FUN_001bc8e0`. The
defaults are in B1:121824-121839 (`+0x1850 = 10, +0x1854 = 3, +0x185c = 1`).

LVL scan of every BF1/BF2 `entc` that sets either key (`scratchpad/props.txt`): the ordinary troopers do not set them,
so they take the class defaults. Heroes/Jedi (`com_jedi_default`, `com_hero_default`), jet troopers, dark troopers and
the geonosians set `CollisionScale = 0 0 0`, so they take no landing damage. Vehicles set their own values.

Flat landing (up·n = 1): T = 13 m/s, damage = (v − 13)² × 1. In a free fall at g = 18 (BF2 Stand state: gravity only;
Fall state 8 from 1.0 s airborne adds the drag of §2.1), with v.x = 0:

| drop | v at impact (drag / no drag) | damage (drag / no drag) | vs 300 HP |
|---|---|---|---|
| 4.7 m | 13.0 | 0 | — |
| 8 m | 16.9 / 16.9 | **16** / 16 | survives |
| 15 m | 22.0 / 23.3 | **80** / 105 | survives |
| 25 m | 26.5 / 30.0 | **181** / 289 | survives |
| ≈ 40 m | 30.3 / 38 | 300 / 620 | lethal (drag); no-drag lethal from 25.5 m |

[D] for the numbers. dt → 0; walking speed adds to |v| and so to the drag. Shields and armour inside `FUN_001dfb38` are
not modelled.

### 2.5 The boundary is 2-D [V]

B2 `FUN_0034c550` (B2:343393-343418) and B1 `FUN_002956d8` (B1:220520-220545) pass only `argument 2[0]` (x) and
`argument 2[2]` (z) to the shape test (`FUN_0034c780` / `FUN_002958f8`):
*(decompiler excerpt omitted from the public copy)*
Height plays no part. A soldier below a platform but inside the boundary polygon gets no boundary timer.

### 2.6 bes1 verdict [D]

BF1 BES1 has no `AddDeathRegion`, no `water_sink_region` and no terrain. The boundary is 2-D. Landing damage needs a
contact. So a soldier who walks off a platform falls until his feet pass **y = −500** (or leaves |x|, |z| ≤ 1000), and
`FUN_00205cc8` then kills him with no killer and no cheat check. Time to the floor from platform height y0 without drag:
√(2·(y0 + 500)/18), about 7.5 s from y0 = 0 [H: BF1's airborne drag was not read; with BF2-like drag it is about 14 s].

## 3. KAM1 (BF2) hidden collision [D]

The `deathregion` box (centre y ≈ 10) is touched only by a soldier whose bounding sphere reaches it. Standing on the
hidden soldier-only discs and cores (y ≈ 42-62) is far above it. Nothing else applies there: y > −1000, |x|, |z| < 2500,
and the boundary is 2-D. Neither KAM1 BF2 LVL has a `water_sink_region`, and the soldier tick does not read it anyway.
So such a soldier **lives**, minus the landing damage of §2.4:
- an 8-18 m drop from a deck (y 70-80) onto a core at about 62 costs 16-110 HP;
- a 25-38 m drop onto a disc at about 42 costs 180-290 HP (drag model), which is survivable at full health and lethal
  if hurt.

A hero or jet trooper (`CollisionScale 0`) takes nothing.

## 4. What the port should do

1. Death regions: keep a per-tick test, but BF2 = **bounding-sphere touch** (BF1 = feet point) on soldiers and
   layer-8/2 vehicles, and the kill must **ignore** invincibility, with no killer (today `kill_environment` spares an
   invincible human; that is wrong for both games).
2. Add the hard world floor/box to every soldier: BF2 y < −1000 or > 1000, |x| or |z| > 2500, v.y < −60; BF1 y < −500
   or > 500, |x| or |z| > 1000. It is a cheat-bypassing kill. That alone covers bes1.
3. BF1 only: `water_sink_region` (plane y) under the feet = the same kill as a death region.
4. Replace `LETHAL_LANDING_SPEED = 25` with damage (v_n − T)² × S through normal damage (cheat honoured). Defaults are
   T = 10 + 3·n.y and S = 1, with the ODF overrides (heroes, jet and dark troopers = 0).
5. The boundary stays 2-D (card BNDY); it does not double as a kill floor.

## 5. Open / next reads
- The soldier's bounding-sphere centre offset and radius (who writes E+0x20 for a soldier: its SetTransform). A RAM
  read of `s+0x20..0x2c` on KAM1 would settle it.
- Which vehicle class is layer 4 (`FUN_001d0eb0`, health vptr 0x618fe8). It is not tested by BF2 death regions.
- BF1's airborne drag (bes1 fall time only).
- The water-damage amount and depth (§2.3): desc layout of `FUN_001df050`/`FUN_001dfb38`.
- The other-object factor (class vtable +0x40) for building classes. The soldier class and the base return 1.0
  (`FUN_0026bcb8`: `lui at,0x3f80; mtc1; jr ra`); buildings were not checked.
