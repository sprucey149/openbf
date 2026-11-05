# OPUS RE (2026-10-07, loop lane 2, card VREST) - [O] UNAUDITED
# RE: where a parked vehicle rests — the pad's spawn point, the flyer's LandedHeight, and the frame order a hover settles by

> **AUDIT 2026-10-16 (loop lane 1, audit of the HSTEP landing, OK — `engine/artifacts/loop/audits.md`).** §5's frame
> order re-derived from both ELFs and held by two verifiers working apart (BF2 vtables 0x61ca80 / 0x61cd98 / 0x60f048,
> BF1 0x531448 / 0x5316e0 / 0x526690; BF1 names its key-15 task `Q211EntityHover13PostCollision`); neither collide pass
> has a `jal` in either ELF [receipt:bc92be9b] [receipt:ecd77d84]. First BF1 RAM check: 434 BF1 hover rows (283
> distinct hovers, 90 save states, both vtable words) hold every pending compression E+0x2170+32i at 0 (1302 springs,
> 1296 used > 0). One [M] number was wrong and is corrected in §5 (the HOVR referee's 1.32 → 0.71 cm). Still [H]: four
> other callers of a body's contact slot +0x20 (B2 FUN_0023cfd0, FUN_00256a18, FUN_00346100, FUN_00348e58) were not
> followed, and the spring point copies and clears its compression only when it finds its spring body (B2 FUN_0021f5d0's
> `FUN_00168cd0` test). Evidence: `engine/artifacts/lead_tmp/audit-hstep/` (wf/C2, wf/V1, wf/V2).

**The question** (card VREST). The port stands every pad vehicle's ORIGIN on the ground; flyers draw sunk (BF1
Geonosis's gunship half under the sand) and the card asked for "the rest height of a parked flyer and a treaded
vehicle per the PS2".
**Method.** The lead read the pad spawn step, the flyer class setter / constructor / integrator and the task lists in
both ELFs, read every pad's vehicle out of all 352 kept save states (`ps2veh.py`), read the flyer models' boxes off
both discs (`lhmodel.py`), and ran the port's own battle set-up for every mission in a scratch worktree at main
11e7d61 (`examples/vrest_census.rs`). One Opus verifier got the claims and citations only: 32 of 34 SUPPORTED, none
contradicted, two qualified (§7). Tools and outputs:
`engine/artifacts/lead_tmp/vrest/`. B1 = `SWBFRONT.ELF-decomp.c`, B2 = `SWBFRNT2.ELF-decomp.c` (lines); [V] read,
[D] derived, [R] read from PS2 RAM, [M] measured on the port, [H] guess; [O] = load-bearing.

## 0. The answers

| question | answer |
|---|---|
| where a pad puts its vehicle | **Both games:** a ray from 1 m over the pad to 1 m under it against terrain and the building layer; the vehicle is created at the hit with the pad's rotation, or **at the pad's own position when the ray meets nothing** (§1). 14 of 215 stock pads stand over 1 m above their ground; none is a flyer pad. |
| the parked flyer's height | Its constructor adds the class's **LandedHeight** to the spawn position (BF2 names the function: `EntityFlyerInitAsLanded` calls it). `LandedHeight` is the authored value, else **minus the min y of the model's `modl INFO` vis box**, set when `GeometryName` is parsed (§2). 18 of 18 live classes in RAM equal that to the float. |
| does it settle, drop or follow the ground | No. Landed (state 0) its velocity is overwritten with zero every tick and its body is a building-layer object (§3). 54 parked PS2 flyers: spawn point + LandedHeight to within 0.5 mm on 53; the 54th stands on a foliage rock the port's collision world lacks (§4). |
| the port today | Every parked flyer is low by exactly its LandedHeight: gunship 4.15 m, X-wing 1.78, Y-wing 1.33, jedi fighter 1.10, snowspeeder 0.50, geofighter 0.22; TIEs, droid fighter and MAF within 1.2 cm (§4). |
| the treaded classes (hailfire, snail tank) | They are hovers and rest **on their springs**: the hailfire's origin 0.148 m under flat ground, still to 3 mm over 8 states. The port's never comes to rest (a 14 cm swing, up to 4.8° of tilt, undamped), and every port hover bobs about 3× the PS2's band, because **the port runs the spring tick before the contacts; both PS2 games run drive → collide pass → spring tick** (task keys 0 / 10 / 15). With that order the port's hovers land within 2 cm of the PS2 on 92 of 114 pads (31 before) (§5). |

## 1. The pad's spawn point [V][O]

**BF1 `FUN_002b0238`** (the spawn step; `re-vehicles.md` §7 has the countdown around it):
```
pos = pad+0x60;  V = *(0x66ef00)                              B1:240466, 240469
start = pos + V;  dir = (0,0,0,1) - V                          B1:240474, 240477
f = FUN_00161d68(2.0, start, dir, &hit, 0, 0, 0, 0x48, 0)     B1:240490   (mask 0x48, flag 0: every body, not the ordnance subset)
if f < 1.0: matrix.pos = start + dir·(f + f), w = 1.0          B1:240492-240503
else:       matrix.pos = pos                                   (stored at B1:240487, not rewritten)
entity = (*(class+0x1c)+0x10)(class, &matrix)                  B1:240520-240521   (no carrier: *(pad+0xa8+4·team) == 0, B1:240505-240507)
```
- [R] `0x66ef00` = (0, 1, 0, 0) in live battles (BF1 GEO1 states), so the ray runs **from pad + 1 m to pad − 1 m**.
- Mask 0x48 = building layer 8 | terrain 0x40 (`re-collision-layers.md` §4); `FUN_00161d68` returns 1.0 with no world
  or a zero length and replaces only a zero mask (B1:1736-1743).
- A landed flyer is a layer-8 object (§3), so this ray meets one; a hover or walker it does not. [R, verifier] BF1
  GEO1's building list (world+0x54, 840 objects) holds the landed gunship's own body.
- **The window is read from the code only.** In all 54 rows of §4 the surface lies 0.18–0.68 m under the pad, so
  neither the +1 m start nor "no hit → the pad's own point" is exercised by a PS2 flyer.

**BF2's box against the pad's own vehicles** is ported as `Sim::pad_box_taken` (`re-space-assault.md` §10). It runs before
arming, and is a separate test from the `FUN_00161b50` query below.

**BF2 `FUN_003725f0`**: the same — `pad+0x60` (B2:370291), `*(0x73d8a0)` (B2:370318; [R] (0, 1, 0, 0)), start /
direction (B2:370319-370320), `FUN_00161c60(2.0, start, dir, &hit, 0, 0, 0, 0x90, 0)` (B2:370326; 0x90 = BF2's
building 0x10 | terrain 0x80), f < 1.0 → the hit (B2:370328-370339), the class's create virtual on `&a local`
(B2:370359). BF2 only: between the ray and the create a box query `FUN_00161b50(pad+0x110, …, 1, 0, 8)` that finds
something sends the step to `0x00372b9c` — countdown 1.0, no vehicle (B2:370349-370350). [R, verifier] BF2's layer
8 is the hovers and walkers (HOT1's AT-AT and tauntaun bodies are on list +0x40, id 8; its four parked snowspeeders on
list +0x68, id 0x10): **a BF2 pad does not spawn while a hover or walker stands in its box.** A second gate before the
create needs a session flag (`DAT_00600548` / `DAT_00600549`) that is 0 in the ELF and in RAM (`re-vehicle-pads-load.md`
§1 has the step's other gates).

**The port** (`core::sim::vehicles::tick_vehicle_pads`) asks the soldier's ground probe from pad + one step down to any
depth. [M] On the port's world the two rules give the same point on 201 of 215 pads; the other 14 stand 1.17–6.13 m
over their ground (BF2 GEO1 ×6, BF2 KAS2 ×2, BF2 UTA1 ×2, BF2 HOT1, BF2 FEL1, BF1 HOT1, TAT1): the PS2 creates the
vehicle in the air there and it falls. Every one that spawns a vehicle is a hover / walker pad, and the PS2's
vehicles at them stand at the port's settled height within 7 cm (the hailfires 13–17 cm, §5), so only a flyer would
show the difference. No pad has its ground more than 0.5 m ABOVE it.

## 2. The flyer's LandedHeight [V][O]

| | BF1 | BF2 |
|---|---|---|
| class field | class+0x17cc | class+0x89c |
| constructor default 0; class copy takes the parent's | `FUN_001dead8` B1:89274; `FUN_001dee10` B1:89430 | `FUN_0020f098` B2:124344; `FUN_0020f610` B2:124558 |
| `LandedHeight` (0x7642977a) parses into it | setter `FUN_001df3f0` B1:90030-90031 | setter `FUN_0020fd58` B2:124871-124872 |
| `GeometryName` (0x47c86b4a) ends with `if (model) field = −*(*(model+0x1c)+0x84)` | B1:89813, 89911-89916 (model = class+0x13dc) | B2:125205-125208 (model = class+0x160) |
| instance copy | entity+0x628 (B1:83966) | entity+0x66c (B2:114528) |
| **InitAsLanded**: zero the motion fields, position += (0, field, 0) | `FUN_001d7eb8` B1:84155-84178, called by the instance constructor `FUN_001d76a0` B1:83962 | `FUN_00200a98` B2:114817-114831, called by `FUN_001fff10` B2:114490 and by the Lua callback **`EntityFlyerInitAsLanded`** B2:35522 |

- The props are applied in file order, so **the later of `GeometryName` and `LandedHeight` wins**. Every class that
  authors `LandedHeight` authors it after `GeometryName` ([R]: its RAM value is the authored one, below).
- [R][D] `*(model+0x1c)` is the geometry object; its rows at +0x80 / +0x90 are the model's **`modl INFO` vis box**
  (`mungefs::model`: `vis_box`). Live BF1 gunship class: rows (−10.462, −4.150, −8.839) / (10.462, 10.462, 11.907);
  the disc's `rep_la_at_gunship` INFO: vertex box y −2.659..4.373, vis box exactly those rows. The visible mesh's
  floor is the card's −2.66 m, so [D] **the PS2's parked gunship stands with its belly about 1.5 m over the ground**
  (no kept PS2 frame shows one, §6.4).
- InitAsLanded writes the position through the body's set-position virtual (BF1 `FUN_001c6cc8`: the matrix row
  entity+0x230 and the bounding sphere, no ground test, B1:71903-71950). It runs once per constructed flyer in BF1
  (one `jal`, 0x1d7a38). In BF2 the script callback runs it again and adds the height a second time [D]; four campaign
  scripts call it (`dea1g_c`, `myg1c_c`, `kam1c_c`, `uta1c_c` — verifier), no conquest script.
- Nothing re-places the new entity afterwards, the pad's later calls on it included (verifier). Its body is on the
  moving layer (BF1 id 2) from the create until its first integrator tick moves it to the building layer (§3).
- A pad with a carrier class instead starts the flyer flying (`FUN_001d7f58`, B1:240535-240536); no stock map places
  a `vehiclepad` (`re-vehicles.md` §7).

**[R] 18 of 18 live classes** (`lhmodel.py`: the setter replayed over the disc's `entc` props and `modl` INFO, against
the class field in RAM): BF1 snowspeeder 0.5 (authored; vis box would give 0.146), xwing 1.778591, ywing 1.328592,
cloudcar 0.823675, droidfighter 0.004416, geofighter 0.217042, maf −0.0, tiebomber **−0.011526**, tiefighter 0.006017,
gunship 4.150177, jedifighter 1.1 (authored; 0.757); BF2 snowspeeder 0.5, tridroidfighter 3.083473, greviousfighter
2.191925, anakinstarfighter_sc 0.7, droidfighter_sc 1.056455, arc170fighter_sc 1.1, vwing 0.9 (the last six from a
space battle's hangar). The model is the one `GeometryName` names (LOD0): the Y-wing's LOWD box would give 1.3296.

**Every flyer-family class the discs define** (`lhmodel.py --all`; * = authored):
BF1 kwing 2.1885, snowspeeder 0.5*, xwing 1.7786, ywing 1.3286, cloudcar 0.8237, droidfighter 0.0044, geofighter
0.2170, maf 0.0, tiebomber −0.0115, tiefighter 0.0060, trooptrans 2.2516, gunship 4.1502, jedifighter 1.1*,
fedlander_dome −0.8003, vtrans (carrier) −2.2358.
BF2 snowspeeder 0.5*, rep / kam / ride gunships 3.3434, uta ride gunships 2.6592, imp_fly_ai_trooptrans 2.2516,
maf 0.0, and the space classes (awing 1.85, xwing_sc 1.7842, ywing_sc 1.3286, tiefighter_sc 3.24, tiebomber_sc 1.9501,
tieinterceptor 1.7143, vwing 0.9*, arc170 1.1*, jedifighter_sc 1.1*, anakin 0.7*, droidfighter_sc 1.0565, tridroid
3.0835, grevious 2.1919, the command gunships 3.34–3.54, imp_fly_trooptrans 1.6527).

## 3. Landed, it does not move [V][O]

BF1 integrator `FUN_001d9180` (state entity+0x618, `re-vehicles.md` §4.2):
- State 0, first switch: lift fraction = 0; nothing else until the input byte entity+0x2ee is 1 (B1:85627-85630).
- State 0, second switch: velocity (+0x5f0) and angular velocity (+0x600) are overwritten with the vector at 0x66eee0
  ([R] zero), +0x620 / +0x624 zeroed (B1:85842-85854). The position step that follows adds velocity · dt.
- While landed the body is kept on **layer 8, the building layer**: if its layer is not 8 it is removed from the
  collision world and re-inserted through `FUN_00161898` (list +0x54, id 8) (B1:85193-85206, 1344-1391). Takeoff
  re-inserts it through `FUN_001616f8` (list +0x2c, id 2) (B1:85633-85634, 1228-1295).
- BF2 `FUN_00202d08` is the same: state entity+0x60c, fraction = 0 (B2:117846-117848), both velocities = the zero
  vector at 0x73d880, four more fields zeroed (B2:118104-118119), layer 0x10 while landed (B2:116895-116909).
- [R] Every flyer at its pad in state 0 — 63 (map, pad, class) groups over 352 state rows, §4's 54 and nine the
  port spawns no vehicle for — has velocity (0, 0, 0) and one y in every state that holds it (spread 0.000000 m).

What takeoff and landing read (for card VFLY; BF1 lines, the lead's reading, not in the verifier's claims):
- Takeoff (state 1): a 1024 m ray down from the origin (mask 0xc8); the lift fraction advances by dt / TakeoffTime only
  while the height is over LandedHeight + TakeoffHeight (B1:85668-85684) — BF1 gates it as BF2 does (§4.2 said BF2 only).
- Landing (state 3): the ray starts at the body's sphere centre; height = the origin's height over the hit. Under
  LandedHeight + the sphere's radius the dust effect fires once; under LandedHeight, with body up · world up > 0.9375
  and the hit not a layer-0x80 or flagged object, state = 0 — else state 1 again (B1:85703-85793). No snap: it rests
  where that tick left it, at most one tick's descent under LandedHeight.

## 4. The PS2's own parked flyers, and the port's [R][M][O]

`ps2veh.py`: every pad of every kept state (BF1 ring 0x589a68, BF2 0x69c050; `re-vehicle-pads-load.md`), its vehicle's
matrix, and for a flyer its state, velocity, LandedHeight and the class model's box. `padrule.py`: §1's rule on the
PORT's world (terrain, static bodies) + the RAM LandedHeight, against the PS2's y.

- **54 (map, pad, class) rows, |error| < 0.5 mm on 53**: BF1 BES1 31, GEO1 4, HOT1 4, KAS1 5, NAB1 1, TAT1 5, BF2 HOT1 4
  (X-wing 12, TIE fighter 10, snowspeeder 8, droid fighter 7, gunship 5, TIE bomber 4, Y-wing 3, jedi fighter 3,
  geofighter 2).
- **The 54th, BF1 GEO1 `laat3`, is 0.249 m higher on the PS2.** [R] A `PropCollisionObject` (vtable 0x545880) lies
  under the pad: box (58.887, −63.005, 273.348)..(66.151, −62.390, 280.100) around the pad's (62.346, 275.471); the
  terrain there is flat at −62.760. It is one of GEO1's solid foliage rocks (`geo_prop_flatrocks`, `collision: true`),
  which the port draws but keeps out of its collision world (card FOLGC). So the pad ray's "building" hits include
  foliage collision objects.
- **The port (main 11e7d61, 10 s idle, `join.py`): PS2 y − port y** = snowspeeder +0.500 (8 pads), xwing +1.779 (12),
  ywing +1.328 (3), jedifighter +1.100 (3), geofighter +0.217 (2), gunship +4.150 (4) and +4.399 (laat3),
  droidfighter +0.005 (7), tiefighter +0.006 (10), tiebomber −0.011 (4). Walkers: mean |Δ| 2.1 cm over 47 pads
  (30 within 2 cm) — not this card's.
- The port spawns 70 pad flyers over its 40 missions; 42 of them (X-wing 12, jedi fighter 12, snowspeeder 8,
  gunship 5, Y-wing 3, geofighter 2) are classes with a LandedHeight over 0.2 m. BF1 BES1's
  `bes_fly_cloudcar` stands on the PS2 (0.824) and the port spawns none at that pad (not chased).

## 5. Hovers and the treaded classes: the frame order [V][R][M][O]

`treaded` is the hover class (`re-vehicles.md` §2). [R] A parked BF2 hailfire (GravityScale 3, VelocitySpring 4, three
0.4 m springs): used compressions 0.878 / 0.871 / 0.861, g 0.9667, velocity y +0.340 after the spring tick. One tick
of gravity, (GravityScale + (1 − GravityScale)·g)·9.8·dt = 0.3484 m/s (`re-hover.md` §6.5a), equals the three springs'
Σc·VelocitySpring·dt = 0.3479: **it rests on its springs.** Its spring spheres (`AddSpringBody` 0 3.8 2.0 r 4.0 and
±2.5 3.3 −0.8 r 3.5) all reach 0.2 m under the origin, so c = 0.871 puts the origin 0.148 m under flat ground: BF1
GEO1's two stand 0.145–0.148 m under the port's terrain height, BF2's 0.151, the same in every state (spread 3 mm).

**The order.** Both games keep their per-frame tasks sorted by key = −prio:
| | BF1 | BF2 |
|---|---|---|
| registration; ascending insert; runner | `FUN_001971c8` B1:38707-38721 (one sorted list) | `FUN_001af7e8` → `FUN_001af508(list, −prio)` B2:56112, 55969-55975; `FUN_001af640` B2:56008-56012 |
| hover controllable (the drive), prio 0 | vtable 0x531448: +0x10 = 0x1971c8 called with (0, 2) B1:101063; +0xc = 0x1f2268 | vtable 0x61ca80: +0x10 = 0x1af7e8 with (0, 2) B2:136420; +0xc = 0x2245d0 |
| **collision world**, prio −10 → key 10 | ctor `FUN_00161f00`: vtable 0x526690 B1:1844, registered B1:1920; +0xc = 0x163008, the collide pass | ctor `FUN_00161df8`: vtable 0x60f048 B2:1704, registered B2:1806; +0xc = 0x163450, the collide pass |
| hover physics task (the spring tick), prio −15 → key 15 | entity+0x214c, B1:101064 | entity+0x1cd0, B2:136421; vtable 0x61cd98 +0xc = 0x224de0 |

So a frame runs **drive → collide pass (it fills each spring's compression, `re-hover.md` §7 K5) → spring tick (it uses
and clears them)**. This settles the "[H]" of `re-hover.md` §2.
- [R] 134 of 134 touching BF2 hovers in the 30 states of `audit-hovr/hovers.jsonl` hold every pending compression
  (E+0x1cf0+32i) at 0.0 and a used one (E+0x1cf4+32i) > 0: the spring tick ran after the collide pass.
- [R, verifier] The live task rings agree in both games: a hover's controllable in the key 0 bucket, the collision
  world at key 10, the hover's physics task at key 15 (BF2 ring keys −10000, 0, 1, 10, 15, 20, 23, 24, 30, 60). The
  collide pass has no other caller (its address occurs only as the vtable word). The BF2 chain collide pass →
  `FUN_00163d08` → the body's contact virtual → `FUN_0021fd98` → `FUN_0021f508` was walked; BF1's by structure only.
- Other tasks share keys 1 and 10 (not opened). The collide pass skips the neighbour gathering on alternate frames
  for slow bodies over 60 m from every viewpoint; the terrain test is never skipped (verifier).

**The port** (`core::vehicle::hover::step`) runs drive → springs → contacts: each spring tick pushes with the
compressions of the pose one tick back. [D] A force that lags its pose by dt feeds a bounce (growth about ω²·dt/2 a
second) where the springs' own damping is only (1 − c)-weighted. [M] On main BF1 GEO1's parked hailfire (pad
`cp5hailfire1`, PS2 y −62.725..−62.728) swings over 14.5 cm in seconds 10–20 and 14.1 cm in seconds 20–40 (y −62.696..
−62.555), its up vector up to 0.084 off vertical — it does not decay (`trace_hailfire_old.txt`).

**[M] The port with `step` reordered** (scratch worktree, one switch; the old order reproduces main's census byte for
byte), against every parked PS2 hover — 114 (map, pad, class) rows, |PS2 y − port y|:
| | main, 10 s | reordered, 10 s | main, 40 s | reordered, 40 s |
|---|---|---|---|---|
| within 2 cm | 25 | 87 | 31 | **92** |
| within 5 cm | 86 | 112 | 71 | **112** |
| mean | 4.19 cm | 1.64 cm | 4.78 cm | **1.58 cm** |
| hailfire (4 pads), PS2 − port | −0.141 | −0.020 | −0.110 | **−0.002** (−0.005..+0.001) |
| snail tank (5) | −0.050..+0.056 | −0.017..+0.002 | −0.040..+0.014 | −0.015..0.000 |
| AAT (13) | −0.082..+0.018 | −0.021..+0.014 | −0.141..+0.052 | −0.027..+0.016 |

Reordered, the same hailfire swings over 4.3 cm in seconds 10–20 and 2.1 cm in seconds 20–40 (y −62.738..−62.717), its
tilt under 0.003 (`trace_hailfire_new.txt`). The two rows over 5 cm after: BF2 FEL1's pad `snail31` (a combat
speeder, +0.35) and one BF1 NAB1 fighter tank (+0.09), not chased. The PS2's own hovers differ by up to 5.5 cm between
states (the spring jitter), so 2 cm is a tight bin for the classes that have it. The HOVR referee
(`audit_hover_ram`: 72 SETTLE rows = 47 distinct parked BF2 hovers, one per class + rounded x, z, settled at their RAM
pose): mean |port mean y − PS2 y| 2.99 → 2.54 cm (1.57 → 1.07 without POL1's drifting hangar AAT, 68 cm off in both
orders), median 1.30 → 0.50 cm; the port's bob band 15.0 → 4.7 cm wide on average (the PS2's own spread over states:
3–5.5 cm). "PS2 y inside the port's band" goes 45 → 39 (within 1 cm of it 46 → 41): the band shrank, and three AATs
settle 2.5–3.9 cm off the PS2 in both orders. *(Corrected by AUDIT HSTEP 2026-10-16: this paragraph first read "47
quiet hovers, 1.32 → 0.71 cm, band 16.0 → 4.5 cm, inside 47 → 45"; the referee's outputs are byte-identical to the ones
stored here, and no selection of their rows gives those numbers - 90 tried, `lead_tmp/audit-hstep/wf/N3/`.)*

## 6. For the build

1. **VREST (build): flyers.** Bake the top LOD's vis-box min y per vehicle class beside `ManifestVehicle::sphere`
   (`bake::vehicle`, rebake `side`); the class reader replays §2 in PROP order (parent first; `GeometryName` → −min y;
   `LandedHeight` → the value); the pad spawn puts a flyer at §1's point + (0, LandedHeight, 0). §2's table is the
   test; §4's 54 rows are the referee (53 within 1 cm; laat3 −0.249 until FOLGC). Hovers and walkers can keep the
   port's ground: they end there (§1).
2. **HSTEP (new card): `hover::step` = drive → contacts → springs.** One reorder; the pinned hover tests move. Referee:
   §5's table (`vrest_census.rs` + `join.py`) and `audit_hover_ram`.
3. **FOLGC**: when the solid foliage rocks join the collision world the pad ray must see them (laat3, +0.249 m).
4. **A PS2 picture is owed**: no kept frame shows a parked gunship; the build should take one (the pose by RAM hold)
   before the 1.5 m gap under it is called right or wrong by eye.
   **Taken 2026-10-09 (card VREST's build, landed 82fb7db):** `engine/artifacts/lead_tmp/vrest/build/pair_geo1_gunship.png`
   and `pair_nab1_xwing.png` — PS2 | the port after the card | the port before, one pose each (PS2 poses and save
   states in `build/ps2/`, taken by `parked.py`). By eye the PS2's gunship and X-wing stand where the port's now do
   (the X-wing on its gear); the soldier hides the middle of the gunship's belly in the PS2 frame. With the card in,
   §4's referee reads 53 of 54 flyers within 2 cm (laat3 +0.249, FOLGC), on main's rebaked bake.

## 7. Checks run, the verifier, what stayed [H]

**Verifier** (one Opus agent, 26 min, 140 tool uses; claims and citations only; `lead_tmp/vrest/CLAIMS.md`,
`verify/VERDICTS.md`): 34 claims, **32 SUPPORTED, 0 NOT SUPPORTED**. It re-read the spawn ray, the setter's store, the
InitAsLanded body and the registrations in the disassembly, scanned both ELFs for every store at the class field's
displacement (flyer: exactly the four of §2 in each game), read the task rings and the collision lists from RAM, and
re-ran the tools. Qualified:
- the hover table's time series was in no stored file — now `trace_hailfire_{old,new}.txt` (§5);
- §4's 0.5 mm is at the census's three decimals, and its rows do not test the ray's window (§1);
- that the rock was what the `laat3` ray hit is an inference (the implied hit y −62.511 lies inside the prop's box).
Everything else it added is in §1, §2, §3 and §5 marked "verifier". It did not run the port.

**Stayed [H] / not done:** the ±1 m window against a live PS2 vehicle; BF1's contact chain to the spring hook (by
structure); which tasks sit at keys 1 and 10 beside the collision world; walkers (the same task keys, not examined);
the two hover rows over 5 cm; BES1's cloud car the port does not spawn; a PS2 picture of a parked gunship.

Commands (outputs in `lead_tmp/vrest/checks.txt`):
- `re_check.py fn b1 240466 240469 240490 240492 240502 240521` → all in `FUN_002b0238`; `fn b1 83962 83966` →
  `FUN_001d76a0`; `84168 84178` → `FUN_001d7eb8`; `85200 85629 85842` → `FUN_001d9180`; `89274` → `FUN_001dead8`;
  `89430` → `FUN_001dee10`; `89813 89912 90031` → `FUN_001df3f0`; `1844 1920` → `FUN_00161f00`; `2692` →
  `FUN_00163008`; `38715` → `FUN_001971c8`; `101063 101064` → `FUN_001efa78`.
- `re_check.py fn b2 370318 370328` → `FUN_003725f0`; `114490 114528` → `FUN_001fff10`; `114819 114831` →
  `FUN_00200a98`; `35522` → `EntityFlyerInitAsLanded`; `124872 125206` → `FUN_0020fd58`; `116895 117847 118105` →
  `FUN_00202d08`; `1704 1806` → `FUN_00161df8`; `2785` → `FUN_00163450`; `55969` → `FUN_001af508`; `56112` →
  `FUN_001af7e8`; `136420 136421` → `FUN_002216b8`.
- `re_check.py word b1 0x526690 6` → +0xc 0x00163008, +0x10 0x001971c8; `word b1 0x531448 6` → +0xc 0x001f2268, +0x10
  0x001971c8; `word b2 0x60f048 5` → +0xc 0x00163450, +0x10 0x001af7e8; `word b2 0x61cd98 5` → +0xc 0x00224de0;
  `word b2 0x61ca80 6` → +0xc 0x002245d0, +0x10 0x001af7e8.
- `re_check.py callers b1 FUN_001d7eb8` → 1 (B1:83962); `scripts/tools/refs.py b1 0x1d7eb8` → jal [0x1d7a38], no data word.
  `callers b2 FUN_00200a98` → 2 (B2:35522, 114490); `refs.py b2 0x200a98` → jal [0x1906ec, 0x20031c].
  `refs.py b1 0x163008` → data [0x52669c] only; `refs.py b2 0x163450` → data [0x60f054] only.
- `re_check.py writers b1 DAT_0066ef00` → one store, the static initialiser `FUN_00445fa8` (B1:505277); RAM gives the
  value. Every `+ 0x17cc` line of B1 was listed: the flyer's are 89274, 89430, 89912, 90031 (stores) and 71464, 83966,
  84168, 240379 (reads).
