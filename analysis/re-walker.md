# OPUS RE (loop lane 2, 2026-09-29, card VWLK) - [O] UNAUDITED: no Fable pass
# RE: the walker, transcribed — BF2's input, locomotion (turn, animation clock, root motion), legs, jump, contact

> **AUDIT 2026-09-29 (loop lane 1, PARTLY — `engine/artifacts/loop/audits.md`).** Re-derived from the B1/B2 decomp,
> the ELF instructions, the disc (the audit's own entc and zaa_ decoders) and PS2 RAM (save states and two live PCSX2
> runs) before reading this doc. **Held: everything WALK builds for BF2's walkers.**
> - §4: the clip clock (0x25e1d0-0x25e2b4: dt' = dt·max(0.5, speed·|stick|/(W·RS)), 0.5 = 0x5fd240); the stick
>   classes; the boost latch and energy; the turn tree (0x25da34-0x25dd2c); the 2θ yaw (FUN_0025ed70 builds
>   (c²−s², 2cs) from cosf/sinf θ); the root motion (FUN_00532858 returns the root delta).
> - §5's transition table (all 77 words at 0x5fd100) and clip entries ((N−1)/30; the AT-ST walk decodes to 21 frames,
>   4.5376 m → 6.8064 m/s).
> - §6's legs with every constant; §7 (only FUN_0025b2a8 sets bit 0x80, and no walker on either disc authors
>   JumpHeight); §8's probes; §2's task keys; §9 C4/C5; §11's conversions and ctor defaults.
> - §14.6's AT-ST numbers: 5.0 / 10.0 m/s, 4.0 s + 2.2 s, ×0.5 below |stick| 0.681, 1.0 / 0.6 rad/s, 29.4 m/s².
> - **Live, BF2 HOT1, an AI-driven imp_walk_atst, 60 s:**
>   - per-tick yaw steps quantize at 0.0333 rad = 2·0.5/30 (165 ticks, none at 0.0167);
>   - sustained turns run 0.97-1.10 rad/s;
>   - full-stick walking covers 5.02-5.12 m/s over five 4.4-5.3 s segments, not the clip's 6.81;
>   - a stride finished with the stick released plays at ×1 (§4.8's "else dt' = dt").
>
> **Corrections** (one Opus verifier: V1-V10 SUPPORTED, V4 PARTLY on wording):
> 1. **§13's "BF1" column uses BF2's (N−1)/30 clip duration.**
>    - BF1's anim entry lasts N/30: B1 FUN_00230be0 at 0x230d40 computes frames × 1/30 with no −1. RAM agrees: the
>      tauntaun walk is 0.3667 s, the AT-ST 0.7000, the kaadu 0.3333.
>    - Its layer spreads the N−1 keyframe intervals over that time: FUN_0043d898 advances phase by dt·30/N, and
>      FUN_0043da00 samples frame ⌊(N−1)·phase⌋.
>    - So a BF1 walker covers z·30/N: AT-ST 6.48, tauntaun 19.77, spider 2.97, AT-AT 1.48, AT-TE 1.86 — not 6.81 /
>      21.75 / 3.08 / 1.50 / 1.91.
>    - Live on BF1 HOT1: the AT-ST covered 6.474 m/s over 66 s (2 walkers); the tauntaun 18.1 over 40 s (5 walkers;
>      the extra −8 % is [H]).
>    - The "under BF2's clock" column stands.
> 2. **The census misses gun_walk_kaadu** (§0's "15 classes", §11, §13). Its BASE is walker, in BF1's and BF2's
>    SIDE/GUN.LVL, and BF1 fields it:
>    - the three Naboo Plains missions load it (`SIDE\gun.lvl`);
>    - three NAB1 pads key it as ClassRepDef and ClassAllDef, i.e. for the defenders in either era;
>    - the IA states hold 3 live kaadus (class+0x14 = FNV "gun_walk_kaadu"), one of them ridden.
>    - Its BF1 speed of 19.54 m/s would fall to 10.86 under BF2's clock: its MaxSpeed 5 is under the ×0.5 floor.
> 3. **§4.6's flag line** ("every BF2 walker has bits 1 and 3 on, bit 2 off, bit 4 on") holds only for the fielded
>    BF2 classes.
>    - A class that authors none of the six flag hashes keeps the ctor's bits 1, 2 and 3, with bit 4 clear. That
>      covers BF2's unfielded kaadu and every BF1-disc walker run under BF2's code.
>    - Neither that case nor BF1's YawLimits is analysed here. BF1's AT-ST has ±35°; the tauntaun, kaadu and AT-TE
>      author none (0/0), so §4.5 A turns the body after the aim at once. → card WALKB1.
>
> Evidence: `engine/artifacts/lead_tmp/audit-vwlk/`, `engine/artifacts/loop/reports/20260929_0945_audit_VWLK.md`.
>
> **RE 2026-10-07 (loop lane 2, card WALKB1) — §13 rewritten, §16 new, both [O] UNAUDITED.** BF1's own walker is
> transcribed and replayed tick by tick on the PS2 (29,241 of 29,241 steps, six classes, one human driver).
> - The audit's correction 1 holds for the AT-ST, tauntaun, spider and kaadu. **The AT-AT and AT-TE walk at 2.22 and
>   2.80 m/s, not 1.48 and 1.86**: BF1 multiplies the clip rate by `TEMP_AnimationSpeed` (1.5 on both), a prop BF2's
>   ELF never reads (§13.2, §13.6). The tauntaun's "−8 %" is an undrawn walker's lost keyframe plus the slope loss.
> - Correction 3 is answered in §13.8 (BF2's turn tree with BF1's data) and the per-class call is §13.9.
> - §2's [H] is settled for the walker: the contacts run between the controllable and the body task (§13.1).
> - §16: a tauntaun's or kaadu's body sphere on the terrain, in both games — the kaadu's downhill creep and a
>   cos²(slope) loss on every climb. The port has neither (card WCON).

**The question** (card VWLK). How a BF2 walker moves, statement by statement. That covers the 15 walker-family classes on the
two discs: AT-ST ×3, the one-man AT-ST, the spider walker, the tauntaun, and the AT-AT / AT-TE (commandwalker). PLAN locks
BF2 handling for ground vehicles, so this is what the walker build copies. `re-vehicles.md` §4.1 located the functions. It
left open which of FUN_0025d5e0's two clock branches a player's walker takes, the legs, the jump and the contact. **All
four are answered here (§0).**

**Method.** The lead read every function below in the B2 decomp and settled the branch trees, operands and constants in the
disassembly (`scripts/tools/eedis.py`; dumps `lead_tmp/vwlk/dis_*.txt`). Float literals came from the code
bytes (`lead_tmp/hvre/floatlits.py`), .data words from `re_check.py word`, and stores from `widescan.py`. Three checks
against the discs and the PS2 itself:
- the walker clips' root motion decoded from the side LVLs with the repo's own `mungefs::anim` (`lead_tmp/vwlk/animspeed`);
- live walker entities in 15 BF2 PCSX2 save states (`lead_tmp/vwlk/walker_ram.py`);
- a 45 s live PCSX2 run sampling an AI one-man AT-ST's anim clock and position against the game clock
  (`lead_tmp/vwlk/wlk_run.sh`, `wlk_sample.py`, `wlk_analyse.py`).

Hops, in order:
- the vtables: entity 0x61f850, the controllable 0x61f660, the physics object 0x61f7f0, the task 0x61f9a0, and the
  commandwalker's 0x62bf64 block;
- the ctor FUN_00259970 and the activate FUN_00262e78;
- input FUN_0025f4a0 (via FUN_00266e80);
- locomotion FUN_0025d5e0 (slot +0x130) and its helpers FUN_0025ec40 / FUN_0025ea48 / FUN_0025eef8 / FUN_0025ed70 /
  FUN_0025ecd8 / FUN_00260918 / FUN_0025aaa0;
- the anim player FUN_002a0500 / FUN_002a0820 / FUN_002a0aa0 / FUN_002a0b30 → FUN_00532858 / FUN_00532598 /
  FUN_00532440, and the clip entry FUN_002a0c78;
- the task FUN_00267700 → FUN_00261208 (slot +0x124) → legs FUN_00260ba0 → FUN_002a2028;
- the jump FUN_0025b2a8 (via FUN_00267018, controllable slot +0xac);
- the contact thunk 0x2673d0 → FUN_0025c2b0;
- the pose FUN_0025f050 / FUN_0025f210;
- the class: ctor FUN_00263160, derive FUN_002636b0, setter FUN_00263f68, the turret-section setter FUN_00270770, and
  the anim table FUN_002663e0;
- the energy helpers FUN_001e1c88 / FUN_001e1fc0 / FUN_001e2010;
- the terrain ray FUN_0055fb38.

The B1 twins were opened only for §13. Scratch: `engine/artifacts/lead_tmp/vwlk/`.

**Conventions.** B2 = `analysis/SWBFRNT2.ELF-decomp.c` line; B1 likewise for SWBFRONT. `E` = the walker entity (locomotion
*(decompiler detail omitted)*, input *(decompiler detail omitted)*, *(decompiler detail omitted)*/*(decompiler detail omitted)* as int pointers: `piVar[n]` = E+4n). `K` = its class (`E+0x4b4`). Offsets
are bytes. Matrices are the PS2's row-vector 4×4 (rows right, up, fwd, pos), as in `re-hover.md`. The stick names follow the
hover's controllable (`re-hover.md` §5): E+0x2dc throttle, +0x2e0 strafe, +0x2e4 turn, +0x2e8 pitch. dt = the game's
measured frame time (≈ 1/30, `a5-tickrate.md`). Tags: [V] read at the cited lines, [D] derived, [H] guess, [O] =
load-bearing and verifier-SUPPORTED (§15).

## 0. The answers [O]

| VWLK question | answer |
|---|---|
| the two clock branches and which one a player takes | Branch A runs **only while jumping** (E+0x2190 bit 0x80). It plays the clip at the actual forward speed v·fwd when that is faster than the clip's own speed, and freezes the clip at its end. Branch B is **the walk**: whenever the walker walks (§4.1's `walking`) with a walk direction set (E+0x2200 ≠ 0), dt' = dt·max(0.5, speed·\|stick\| / (walk-clip speed · SkeletonRootScale)), with speed = MaxSpeed, or BoostSpeed while boosting (§4.8). **Live PS2: an AI one-man AT-ST at full throttle ran its clip at ×0.9000 against the predicted ×0.9021 over 32.5 game seconds (§12.3).** A player's walker takes B every walking tick. A is dead on the shipped data: no walker class on either disc authors JumpHeight (§7). |
| the turn | Each tick the body yaws by **2θ**, where θ = clamp(command, ±TurnSpeed·dt) is a HALF angle (FUN_0025ed70 builds the rotation from (c²−s², 2cs)). So the top yaw rate is **2·TurnSpeed rad/s**. TurnSpeed is StoppedTurnSpeed standing, ForwardTurnSpeed walking, BoostTurnSpeed boosting or jumping, and 0 in a custom-anim stance. The command comes from a class-flag tree (§4.5; on the shipped classes, §4.6). The AT-ST and every BF2 walker set flags 1 and 3: the body turns to keep the aim inside YawLimits, and, walking, to put the left stick's direction under the camera — **camera-relative walking**. |
| the legs | Each leg pair casts two vertical rays of 2·LegRayHitLength through its foot prims. One pair (AT-ST, one-man, tauntaun) lifts the body so the ground under the feet is ≤ 0.1 m above the origin, else it falls at 3 g. Two or more pairs (spider, AT-AT, AT-TE) also pitch the body from the front/back height difference (a half-angle integrator, ±0.2955) and, with the roll flag, roll toward atan2(left−right, WalkerWidth) (±0.3 half angle). Pitch and roll live on a named bone only; the entity matrix stays upright (§6, §10). |
| the jump | FUN_0025b2a8 fires on the jump button's press (E+0x2b4 & 3 == 3), grounded or within 0.3 s of leaving the ground, when v·fwd ≥ K+0x758 and EnergyCostJump can be paid: v.y += sqrt(19.6·JumpHeight), and v += fwd·v·fwd·(K+0x754 − 1) when K+0x754 > 1. **No shipped walker jumps** (§7). |
| the contact | FUN_0025c2b0: ignored prims (K+0xfb8 mask). A soldier under a foot with the normal up (n.y > 0.9) is crushed. Terrain supports the walker (v.y = 0, grounded) unless a leg pair claims the contact. A vehicle deals impact damage with the hover's CollisionThreshold/Scale law. A building under a foot supports it (n.y ≥ 0.2, no push-out); otherwise it pushes the walker out **horizontally**. The generic push-out follows everything else, the soldier, unclaimed-terrain and vehicle cases included; only a leg-claimed terrain contact and a building skip it (§9). |
| frame order | controllable (key 0): input FUN_0025f4a0 → locomotion FUN_0025d5e0 → aim yaw/pitch → fall (pos.y += v.y·dt when falling). Then the task (key 15): FUN_00261208 → the terrain wall probes → legs FUN_00260ba0 → pose. The world contacts run in the physics pass [H: its place in the frame, as for the hover]. |
| BF1 vs BF2 | BF1 plays the walk at the real dt, so a walker has one speed: z·TEMP_AnimationSpeed·30/N; MaxSpeed is only the AI's. Its body turns by 2·StoppedTurnSpeed·turn, standing or walking, where turn = the left stick's X blended with the aim's drag past YawLimits; it turns in place at 5:1 and never reads ForwardTurnSpeed. BF2 makes that 5 the class field K+0xe10 and adds the clock, the camera-relative turn, boost, energy and the jump (§13, replayed live). |

## 1. Fields [V][O]

**Entity E (B2 walker; ctor FUN_00259970, B2:175401-175764; 0x2290 bytes, the pool FUN_00267440 B2:184897)**:

| offset | what | written by |
|---|---|---|
| +0x10 | physics object; vtable at +0xcc = 0x61f7f0 (slot +0xc GetPosition 0x1d4248, +0x10 SetPosition FUN_00267160, +0x20 contact thunk 0x2673d0) | ctor B2:175465 |
| +0x120..+0x15c | E: rows right, up, fwd, pos. **Yaw only**: nothing in the walker code tilts it (§10) | FUN_0025ed70 (turn), SetPosition (moves) |
| +0x270 | controllable (vtable at +0x284 = 0x61f660; slot +0xc → FUN_0025f4a0, slot +0xac → the jump, slot +0xcc → the AI FUN_00262248) | ctor B2:175463 |
| +0x2b4 | jump button word (bits 0 and 1 both set = pressed this frame [H: which bit is the edge]) | controller |
| +0x2bc bit 0 | boost button | controller |
| +0x2dc / +0x2e0 / +0x2e4 / +0x2e8 | throttle / strafe / turn / pitch sticks | controller, AI FUN_00262248 |
| +0x324 | the AI brain (0 when no AI) | — |
| +0x330 | driver's player slot; < 0 = AI or empty | — |
| +0x360 | full-stick timer (the controller ramps it, `re-hover.md` §5.2) | controller |
| +0x4b4 | class K | ctor B2:175458 |
| +0x4c0 | velocity v (x, z from the root motion; y from gravity and the jump) | locomotion, legs, jump, contact |
| +0x50c | aim yaw, relative to the body (+ = left, see §4.6) | input, locomotion |
| +0x510 | aim pitch | input |
| +0x514 | body pitch, a HALF angle a (the bone pitches by 2a) | legs |
| +0x1560 / +0x1570 | the pose buffer; +0x1570 = bone 0's local rotation | anim player, pose |
| +0x1d70 | the anim player (0x4c bytes, 3 layers, FUN_002a0500) | ctor B2:175670 |
| +0x2188 | the turn-step clip's leg-offset target (−h) | FUN_0025eef8 |
| +0x2190 | flags (u64): 0x2 dead (FUN_0025ce68), 0x4 AI-near (FUN_00262248), 0x40 supported this tick, 0x80 jumping, bits 8-9 custom-stance request, bits 10-12 custom anim + 1 | §15 check 5 lists every writer |
| +0x2194 | leg pair count (= K+0xe3c) | ctor B2:175673 |
| +0x2198 + 4i | leg pair objects (0x130 bytes, FUN_002a1468) | ctor B2:175717-175718 |
| +0x21e4 | time since last supported | locomotion |
| +0x21e8 | the current anim index (0..10, ≥ 11 custom; table §5) | FUN_0025ea48 |
| +0x2200 | walk direction: 0 none, 1 forward, 2 backward | FUN_0025ec40, FUN_0025ea48, locomotion |
| +0x2204 | energy: [0] → K+0x794 {max, floor, recover threshold}, [1] energy, [2] bit 0 exhausted | ctor B2:175474, 175482-175483 |
| +0x2210 / +0x2214 | **the leg offset h** as a unit complex (cos h, sin h): the legs trail the body by 2h | locomotion, FUN_0025ed70, FUN_0025ecd8 |
| +0x2218 / +0x221c | (cos a, sin a) of the body pitch a = E+0x514 | legs |
| +0x2220 / +0x2224 | the body roll as a unit complex, \|angle\| ≤ 0.3 | legs |
| +0x226c | the water-damage countdown | input |
| +0x2250 | the physics task (its vtable at +0x14 = 0x61f9a0) | ctor B2:175495 |
| +0x2270 bit 32 | boost latched | locomotion |
| +0x2280 | the custom-stance timer | input, locomotion |

**Class K (0xfd0 bytes; ctor FUN_00263160 B2:182284-182463; setter FUN_00263f68 B2:182793-184022)**: the props and their
offsets are in §11.

## 2. The frame order [O] except the contact pass [H]

- The activate FUN_00262e78 (entity vtable slot +0xac) registers the controllable, E+0x270, with key 0 in list 2
  (B2:182142), and the task, E+0x2250, with key −15 (B2:182143).
- FUN_001af7e8 files a task in the bucket `−key` (B2:56112, *(decompiler detail omitted)*). FUN_001af508 is the
  sorted insert that keeps the buckets ascending (B2:55969). The per-frame walk (FUN_001af988 → FUN_001af640 →
  FUN_001af598) runs list 2 in that forward order. So bucket 0 (the controllable) runs before bucket 15 (the task), as
  for the hover (`re-hover.md` §2).
- The controllable's update is slot +0xc of 0x61f660, FUN_00266e80 = `FUN_0025f4a0(this − 0x270)` (B2:184549).
- The task's slot +0xc is FUN_00267700 = the entity's vtable slot +0x124, FUN_00261208 (B2:184991; ELF word 0x61f974 =
  0x261208).

Per frame, therefore: **input (§3) → locomotion (§4) → aim → fall → body task (§8) → legs (§6) → pose (§10)**. The world
contacts (§9) come through the physics object's vtable and belong to the collide pass. That pass is the collision
world's task, key 10 in both games (`re-vehicle-rest.md`), so it runs **between** the controllable (0) and the body
task (15): fall → contacts → legs. The kaadu's creep measures exactly that order (§16); the heading's [H] is settled
(card WALKB1, 2026-10-07).

The **commandwalker** (AT-AT, AT-TE) runs the same functions. Its vtable block at 0x62bf64 holds FUN_00261208,
FUN_0025cfe0, FUN_0025d1b0, **FUN_0025d5e0**, FUN_0025efd8, FUN_0025f050, FUN_0025f440, FUN_0025f210 and FUN_0025f3e0 in
the walker's slot order (`vt.py b2 0x62bf64`). A whole-ELF data-word scan finds FUN_0025d5e0 only at 0x61f980 and 0x62bf70.

## 3. The input — FUN_0025f4a0(dt, E) (B2:179422-180199) [O]

*(decompiler excerpt omitted from the public copy)*
No 30·dt factor here: the walker's aim rates are rad/s (compare the hover's I8).

## 4. The locomotion — FUN_0025d5e0(dt, fwd, strafe, turn, E, turnSpeed*) (B2:178022-178723) [O]

`fwd`, `strafe` and `turn` are the input's I10/I11 values. Below, `p2 = fwd`, `p3 = strafe` (the LEFT stick's X), and
`p4 = turn` (the RIGHT stick's X). Registers from the disassembly: s5 = `walking`, s6 = `stance`, s3 = `inPlace`,
$f28 = speed, $f29 = dt at entry, $f23 = the clip dt.

### 4.1 Classifying the stick (B2:178062-178128; 0x25d654-0x25d7a8)
```
stance  = custom-stance request (bits 8-9 of E+0x2190, counted only when E+0x2280 ≥ 0.1 [DAT_005fd238])
          or a custom anim playing (E+0x2190 & 0x1c00)                                     B2:178065-178077
inPlace = K+0xe10·|p2| ≤ |p3|  or  p2² + p3² ≤ 0.01                                        B2:178080-178081
walking = jumping (E+0x2190 & 0x80)  or  (!inPlace and !stance)                             B2:178086-178100
human bit-3 rewrite: if driver ≥ 0 and !walking and !(K+0xe18 bit 2) and (K+0xe18 bit 3) and |p3| > 0.1:
    p2 = 1/sqrt(K+0xe10² + 1)  (negated when E+0x2200 == 2);  inPlace = false;  walking = true      B2:178101-178128
```
K+0xe10 is tan(the in-place angle) (§11). The rewrite gives a sideways stick a forward part of cos(89°) on an AT-ST, so
the walker walks where the stick points instead of stepping in place (§4.6).

### 4.2 Speed and turn speed; boost and energy (B2:178131-178233; 0x25d7ac-0x25da30)
*(decompiler excerpt omitted from the public copy)*
The latch is E+0x2274 bit 0 = bit 32 of the u64 at E+0x2270; only the ctor and this function store it. So once a human's
boost button starts a boost, the boost keeps itself on, button released, for as long as the stick, walk direction and
energy conditions hold.

Energy (FUN_001e2010 / FUN_001e1fc0, B2:92597-92637): a drain fails only while the exhausted bit is set. The drain that
takes the energy below 0 still succeeds, sets the bit and floors the energy at K+0x798. Regen clears the bit once the
energy reaches K+0x79c and caps it at K+0x794.

### 4.3 Reverse steering (B2:178234-178238)
`if driver ≥ 0 and K+0xe18 bit 4 and !inPlace and p2 < 0: p3 = −p3` — a human backing up steers like a car.

### 4.4 The yaw window (B2:178239-178251; 0x25da80-0x25db10)
```
hh = 2·atan2(h.s, h.c)            (the legs' offset, full angle)
p4 ≤ 0: min' = YawMin + hh;                  max' = YawMax·(1 + 0.8·p4) + hh
p4 > 0: min' = YawMin·(1 − 0.8·p4) + hh;     max' = YawMax + hh
step = *turnSpeed·dt
```
Holding the look stick narrows the far limit to 20 %, so the body starts to follow sooner.

### 4.5 The turn command (B2:178252-178336; 0x25db14-0x25dd1c)
```
A  human and K+0xe18 bit 1:                                                        B2:178316-178335
       aimYaw < min'  → cmd = 2·(min' − aimYaw)
       aimYaw > max'  → cmd = 2·(max' − aimYaw)
B  (otherwise, or inside the window) human and K+0xe18 bit 3:                       B2:178280-178313
       walking:
           aimYaw ≤ 0 and −p3 < |p2|·K+0xe08,  or  aimYaw ≥ 0 and p3 < |p2|·K+0xe08:
               cmd = 0.95·(atan2(p3, |p2|) − aimYaw)                  (the stick angle, camera-relative)
           else: bit 2 → C;  else cmd = 0.95·(atan2(p3·K+0xe08, |p3|) − aimYaw)   (the angle clamped at the e08 angle)
       not walking: bit 2 → C;  |p3| > 0.1 → cmd = 0.95·(atan2(p3·K+0xe08, |p3|) − aimYaw);  else cmd = 0
           (D still runs, but walking is already false)
C  (AI, or bits clear) cmd = p3·step                                                B2:178254-178257
       walking and |p3| < |p2|·K+0xe0c: cmd = clamp(cmd, ±step)·(|p3|/|p2| − K+0xe08)/(K+0xe0c − K+0xe08)   B2:178258-178271
       K+0xe14·|p3| > 0: cmd −= K+0xe14·(1 − |p3|)·aimYaw                                                   B2:178275-178278
D  if !(bit 2) and p4 == 0 and (E+0x2200 == 0, or == 1 with p2 < 0.5, or == 2 with p2 > −0.5):
       cmd ≤ 0.5·YawMin or cmd ≥ 0.5·YawMax → walking = false                       B2:178338-178359
E  θ = cmd == 0 ? 0 : clamp(cmd, ±step)                                             B2:178360-178370
F  human and bit 3: aimYaw += 2θ                                                    B2:178373-178375
G  FUN_0025ed70(E, (cos θ, sin θ)): E's rows rotate by 2θ about up                 B2:178376-178380
       right' = (c²−s²)·right + 2cs·fwd;  fwd' = −2cs·right + (c²−s²)·fwd   (0x25ed88-0x25eea0)
       h ← h·(cos θ + i sin θ)                                              (0x25eeb4-0x25eeec)
```
`cmd` is a half angle. The body turns by 2θ and the aim by the same 2θ, so the aim keeps its world direction.
- **Signs [D].** θ > 0 moves fwd toward −right; aimYaw grows as the body turns that way. So aimYaw is positive to the
  left of the body when +right is the vehicle's left, the hover's row convention (`re-hover.md` §5, the strafe sign). In
  A, an aim past a limit pulls the body after it.
- **The B loop [D].** Unsaturated, it overshoots: e' = e − 1.9e = −0.9e per tick, where e = aimYaw − φ. It rings, and
  shrinks by 0.9 each tick; the ±step clamp saturates it far from the target.
- **C's factor.** On a class that authors K+0xe08/e0c (the tan of 87°/88° on BF2 walkers), C's factor is negative for
  a stick nearer than 87° to forward. It reaches only the AI (bit 3 routes humans to B) and bit-2 classes (none ship,
  §11). With the ctor's e08 = e0c = 0 it never applies.

### 4.6 What the turn tree does on the shipped classes [D]
Every BF2 walker has bits 1 and 3 on, bit 2 off, bit 4 on (§11). For a human driver:
- the right stick (p4) swings the aim; once the aim leaves the window (±YawLimits, narrowed to 20 % on the far side while
  the stick is held), the body follows at up to 2·TurnSpeed rad/s;
- with the aim inside the window and the left stick pushed, the body turns until the aim sits at the stick's angle φ, so
  the walker walks where the stick points relative to the camera (and a purely sideways stick walks nearly sideways, via
  §4.1's rewrite);
- with the left stick idle, the body holds (cmd from B is 0 with |p3| ≤ 0.1).

For BF2's AT-ST that is: StoppedTurnSpeed = ForwardTurnSpeed = 0.5 → 1.0 rad/s (57°/s); BoostTurnSpeed 0.3 → 0.6 rad/s;
YawLimits ±90°.

### 4.7 Picking the clip — only while idle (B2:178381-178439; 0x25de74-0x25dfd4)
```
if E+0x21e8 == 0 (idle):
    dead (E+0x2190 & 2): action 6 (death); E+0x2280 = 0; clear bits 8-12
    a custom anim playing: nothing
    stance: find the custom entry whose mask K+0x684+0x30j matches; E+0x2280 = −0.3 (DAT_005fd23c); bits 10-12 = j+1;
            start anim 11+j (B2:178400-178423)
    !walking: FUN_0025eef8(p2, p3, E)          (the turn step, below)
    walking: action 3 (walk back) if p2 < 0 and not jumping, else action 2 (walk forward)
    E+0x21d4 = 0
else: FUN_0025aaa0(E)   (B2:176065-176256; footstep effects only: sounds, stomp decals, rumble, water splashes — its only
      stores are E+0x21d4/0x21d8/0x21a4+4i/0x21bc+4i, none to the matrix, v or h)
FUN_0025eef8 (B2:179127-179171): thr = 0.1 if E+0x2200 == 0 and the stick is nonzero, else TurnThreshold (K+0x784, stored sin(x/2));
    h.s > thr → E+0x2188 = −atan2(h.s, h.c), action 5 (turnr);  h.s < −thr → the same, action 4 (turnl);
    else if E+0x21e8 ≠ 0: action 0
```
FUN_0025ec40(E, action) (B2:178954-178984) sets E+0x2200 = 1 for action 2 and 2 for action 3. Action 0 with E+0x2200 == 2
becomes action 1. It then starts anim `DAT_005fd100[7·min(E+0x21e8, 0 if > 10) + action]` (§5, B2:178982).
FUN_0025ea48 (B2:178882-178951) handles the anim number:
- anim 4, the empty entry that follows the death clip: it only records E+0x21e8 = 4 (B2:178895, 178949);
- any other anim outside 5..10: it resets E+0x2200 to 0 (B2:178896);
- every anim but 4: it starts the clip on the player and on every leg pair's player, reversed when E+0x2200 == 2
  (B2:178902); blends it in; and records E+0x21e8 (B2:178949).

### 4.8 The clip dt — the two branches (B2:178440-178560; 0x25dfd8-0x25e2b4)
```
if dt ≤ 0: skip to 4.10
RS = SkeletonRootScale (K+0x668)
A  jumping (E+0x2190 & 0x80):                                                        B2:178444-178487
       rem = the clip's time left;  rem < 1e-4 → dt' = 0
       else: s = clip speed (player+0x10)·RS;  0 < s < speed → dt' = dt·speed/s  (speed = v·fwd, §4.2)
             dt' = min(dt', rem − 1e-4)                  (a jump never ends the clip; it holds at the end)
B  walking and E+0x2200 ≠ 0:                                                         B2:178489-178537
       E+0x2200 == 1 and p2 < 0, or == 2 and p2 > 0: reverse the player and the leg pairs' players (FUN_002a0aa0);
           E+0x2200 = 2 or 1  (walking backward plays the same clips in reverse)
       W = walk-clip speed (anim table [7] +0x8)·RS
       dt' = dt·(speed·|stick|)/W   if 0.5·W < speed·|stick|,   else dt'= 0.5·dt      (DAT_005fd240 = 0.5)
C  a custom anim playing (bits 10-12 = j+1): an event at the entry's time K+0x6a0+0x30(j−1) → FUN_0025e708
                                                                                      B2:178547-178559
D  stance: dt' = 4·dt   (DAT_005fd244)                                                B2:178541-178546
   else dt' = dt
```
|stick| = sqrt(p2² + p3²), after §4.1's rewrite. So an AT-ST at full stick walks its clip at 5/6.806 = ×0.735 and
covers 5.0 m/s. Boosting it runs ×1.47 and covers 10 m/s. Below |stick| ≈ 0.68 the clip floors at ×0.5, 3.4 m/s.

### 4.9 Advance, root motion, clip changes (B2:178561-178715; 0x25e2b8-0x25e634)
*(decompiler excerpt omitted from the public copy)*
FUN_00260918(d, E) (B2:180214-180376; 0x260918-0x260b90) moves the body along the LEGS' forward, pitched by the body pitch:
```
p = 2·E+0x514 (0 when DAT_005fd234 ≠ 0 — never: .data byte 0, only two loads in the ELF)
legs' fwd F = sin 2h·right + cos 2h·fwd   (when h.c < 1; else F = fwd)
pos += (F·cos p − up·sin p)·d        (SetPosition)
```

### 4.10 Jumping and the tail (B2:178705-178721)
- **While jumping:** pos += v·dt (GetPosition, SetPosition). There is no root motion while airborne from a jump.
- **The tail:** vtable +0x13c (FUN_0025f440: the pose, §10), then E+0x2190 &= ~0x40. The supported bit therefore lasts
  one tick; the legs (§6) and the contacts (§9) set it again.

## 5. The clip set, the transition table, the clip data [O]

**The anim table** (FUN_002663e0, B2:184025-184163; K+0x670, K+0x66c = 11 + K+0x675 entries). Each entry is built by
FUN_002a0c78 from up to three space-separated clip names; when all three exist the extra two are the `_leftup` /
`_rightup` layers. The strings are ELF words 0x61f308..0x61f548:

| # | clips | loops |
|---|---|---|
| 0 | idle idle_leftup idle_rightup | 0 |
| 1 / 2 | turnl … / turnr … | 0 |
| 3 | death01 | — |
| 4 | (none) | |
| 5 / 6 | idle_to_leftfoot … / idle_to_rightfoot … | 0 |
| 7 / 8 | walk_leftfoot_rightfoot … / walk_rightfoot_leftfoot … | 1 |
| 9 / 10 | leftfoot_to_idle … / rightfoot_to_idle … | 0 |
| 11+ | the class's custom anims (K+0x680 + 0x30j, FUN_002a0e40) | |

- **An entry's speed and duration** (FUN_002a0c78, B2:223242-223315): duration = (frames − 1)·(1/30) (DAT_005fdf50 =
  0.0333…); speed = track 0's z at the last frame / duration.
- **The clips are looked up by the bare name** (the bank's MINA CRC-32 of "walk_leftfoot_rightfoot"). The bank is
  AnimationName.
- **The transition table** `DAT_005fd100[row = current anim][action]` (11 × 7 ints, read from the ELF):

| current \ action | 0 stop | 1 stop (back) | 2 fwd | 3 back | 4 turn L | 5 turn R | 6 die |
|---|---|---|---|---|---|---|---|
| 0 idle | idle | idle | idle>L | L>idle | turnl | turnr | death |
| 1 turnl | idle | idle | idle>L | L>idle | idle | idle | death |
| 2 turnr | idle | idle | idle>L | R>idle | idle | idle | death |
| 5 idle>L | L>idle | idle | walkLR | R>idle | L>idle | L>idle | death |
| 6 idle>R | R>idle | idle | walkRL | L>idle | R>idle | R>idle | death |
| 7 walkLR | R>idle | idle>L | walkRL | walkRL | R>idle | R>idle | death |
| 8 walkRL | L>idle | idle>R | walkLR | walkLR | L>idle | L>idle | death |
| 9 L>idle | idle | idle>L | idle>L | walkRL | turnl | turnr | death |
| 10 R>idle | idle | idle>R | idle>L | walkLR | turnl | turnr | death |

(rows 3 and 4 are all "none"). Walking backward (E+0x2200 = 2) runs these same clips in reverse.

**Sampling [O].** FUN_00532598 samples a layer at frame ⌊(frames − 1)·u⌋ and blends to the next frame only when the
layer's +0x1335 flag is set (B2:662053-662131).
- The walker clears that flag on every layer each tick: FUN_00261208 B2:181106-181113 for the player, the legs B2:180452-
  180456 for the leg pairs.
- Only the walker's draw sets it (FUN_0025b428 B2:176583-176606, reached from the renderable's slot +0x54 thunk
  FUN_00267118).
- So an undrawn walker's root advances in whole keyframes, and a drawn one interpolates. The average speed is the same
  either way. PS2 RAM shows exactly one keyframe's root delta per tick on the (distant, [H] undrawn) AI walkers (§12).

**The disc's clips** (`lead_tmp/vwlk/animspeed`, the repo's `mungefs::anim` decoder; BF2 side LVLs; walk = anim 7/8):

| bank | walk frames | walk z (m) | walk speed (m/s) | idle>L speed | L>idle speed | turn clip |
|---|---|---|---|---|---|---|
| imp_walk_atst | 21 (0.667 s) | 4.538 | **6.806** | 3.059 | 3.077 | 21 f |
| rep_walk_oneman_atst | 15 (0.467 s) | 2.601 | 5.574 (steps alternate 0.18552 / 0.18615 m) | 1.395 | L>idle z runs −2.143 → 0 | 21 f |
| all_walk_tauntaun | 11 (0.333 s) | 7.250 | 21.75 | 6.159 | 2.430 | 36 f |
| cis_walk_spider | 29 (0.933 s) | 2.870 | 3.075 | 1.370 | 1.372 | 31 f |
| rep_walk_atte | 46 (1.5 s) | 2.857 | 1.905 | 0.807 | 0.888 | 41 f |
| imp_walk_atat | 81 (2.667 s) | 4.000 | 1.500 | 0.709 | 0.401 | 161 f |

The PS2's own anim table agrees for the AT-ST: entry [7] = speed 6.8064, duration 0.6667 in RAM (§12).

## 6. The legs — FUN_00260ba0(dt, E) (B2:180379-180630) and FUN_002a2028 (B2:224163-224259) [O]

**The leg pair** (FUN_002a1468, B2:223590-223693; class entry K+0xe40 + 0x38i, one per `WalkerLegPair`):
- entry +0x0 / +0x4 = FootBoneLeft / FootBoneRight;
- +0x8 / +0xc = the left / right **foot collision prims** (hashes 0x21322adb / 0xb62deb24, e.g. `p_l_foot` / `p_r_foot`
  on the AT-ST);
- +0x10 / +0x14 = LegBoneLeft / LegBoneRight;
- +0x18.. = the bone masks for the leg layers.

The pair object keeps its own anim player (+0x18) for the `_leftup` / `_rightup` layers. P+0x0 = half the two foot prims'
summed z at setup (FUN_002a1750, B2:223802): the pair's forward offset.

**Per pair**, FUN_002a2028(a = E+0x514, P, M = E+0x120, feet = the two foot prims' model-space positions) (B2:224163-224259):
*(decompiler excerpt omitted from the public copy)*
**The body** (FUN_00260ba0):
```
maxS = max over pairs of S (start −1);  avgD = mean of P+0x128                           B2:180416-180458
g = −29.4 m/s², or −9.8 while jumping (E+0x2190 & 0x80)                                   B2:180461-180464
one pair:  maxS ≥ 0: if maxS − 0.1 > 0: pos.y += maxS − 0.1;  v.y = 0; supported           B2:180465-180485
           else v.y += g·dt                                                               B2:180486-180489
two+ pairs: maxS > 0:
           Δ = S(pair 1) − S(pair 0); if Δ ≠ 0: a = clamp(a + 0.01·Δ, ±0.29552022); (E+0x2218, E+0x221c) = (cos a, sin a)   B2:180492-180511
           lift = (max(S0, −0.1) + max(S1, −0.1))/2; if lift ≠ 0: pos.y += lift;  v.y = 0; supported                  B2:180512-180541
           else v.y += g·dt
           roll flag (K+0x78c) and |avgD| > 1e-4:  w = WalkerWidth (K+0x790), r = √(avgD² + w²)                            B2:180546-180594
               e = (avgD/r)·R.c − (w/r)·R.s;  R.s += R.c·e·0.05;  R.c −= R.s·e·0.05;  renormalise
               |R.s| > 0.29552022 → R = (0.9553365, ±0.29552022)        (R = E+0x2220/+0x2224)
E+0x2240.. = v with y zeroed (a copy)                                                        B2:180620-180629
```
- **What the numbers mean [D].** a is a half angle, stepped by 1 % of the pair height difference each tick. It stops
  when tilting the pairs' ray bases has levelled their supports, that is at the slope. The bone pitches 2a (±0.591 rad);
  the roll bone turns by twice R's angle (±0.6 rad).
- **The walker never falls at the legs.** v.y only grows there; I16 of the input moves it next frame. A one-pair
  walker's origin settles between the ground under its feet and 0.1 m below it (it drops at 3 g until a foot ray
  finds ground at or above the origin). Two pairs follow the ground down at most 0.1 m a tick.
- The −0.62 / 0.05 relaxation (B2:180596) sits behind DAT_005fd234, which is never written (§4.9): dead.

## 7. The jump — FUN_0025b2a8(E) (B2:176406-176479) [O]

```
if !jumping and K+0x750 > 0:                                   B2:176423-176424  (K+0x750 = sqrt(19.6·JumpHeight), setter B2:183645-183652)
    if !supported and E+0x21e4 > 0.3: return 0                 B2:176425-176428  (0.3 s after leaving the ground)
    vF = v·E.fwd
    if K+0x758 ≤ vF and FUN_001e2010(EnergyCostJump K+0x7a8, energy):             B2:176442-176443
        v.y += K+0x750                                                            B2:176448-176449
        if K+0x754 > 1 and vF > 0: v += E.fwd·vF·(K+0x754 − 1)                    B2:176450-176466
        E+0x21e4 = 0; supported off; jumping on; return 1                         B2:176472-176475
```
It is reached only from the locomotion's §4.2 call, through FUN_00267018 (controllable slot +0xac, B2:184649).
- **Defaults** (ctor): K+0x750 = 0, K+0x754 = 1.0. K+0x758 is NOT set by the ctor: RAM holds stray denormals there
  (§12).
- The setters: K+0x754 is hash 0x0761451c (B2:182958) and K+0x758 is 0xf0f20aba (B2:183895, "JumpMinSpeed" by fnv,
  unconfirmed); `re-vehicles.md`'s VEH0 notes.
- **Dead on the discs.** No walker class on either disc authors JumpHeight, 0x0761451c or 0xf0f20aba (§11's census).
  A raw byte scan of the PROP chunks in all 285 LVL files on both discs (bake-independent) confirms it. JumpHeight
  appears only on soldier classes: com_hero_default, com_jedi_default, com_inf_default, the heroes and jedi, and
  snw_inf_wampa. 0x0761451c appears only on cis_hero_grievous, and 0xf0f20aba nowhere.
  RAM shows K+0x750 = 0 on the AT-ST and the tauntaun. BF2's tauntaun authors EnergyCostJump 20 but no JumpHeight, so it
  cannot jump.

## 8. The body task — FUN_00261208(dt, E) (B2:180633-181353) [O]

```
B1  gate FUN_003d7a20; dead (E+0x2190 & 0x2) and past the death clip (E+0x21e8 == 4, the empty entry): return   B2:180697-180711
B2  terrain wall probes, when a terrain is loaded (DAT_0060bed4: 1 after the terrain's INFO chunk, FUN_0055db58 B2:694898;
    0 at unload B2:695014):                                                                     B2:180712-180831
        o = pos + (0, 0.85·L, 0);  F = horizontal unit fwd (reversed when E+0x2200 == 2);  S = (−F.z, 0, F.x)
        two or more pairs: reach ×1.3 and o += F·(the front pair's P[0])                      B2:180747-180760
        for d in (F·0.8·r + S·0.7·r, F·0.8·r − S·0.7·r), r = 0.85·L (×1.3):
            t = FUN_0055fb38(0.1, 3.0, o, o + d)   (B2:180788, 180819; a terrain-heightfield ray, B2:695628-695682)
            t < 1: pos −= d·(1 − t)
B3  not dead: legs FUN_00260ba0 (§6)                                                            B2:180833-180835
B4  pose: vtable +0x144, +0x138 (FUN_0025f050), skeleton update FUN_00533f20, +0x140 (FUN_0025f210)   B2:180839-180843
B5  eye / aim ray / cockpit / network (no movement)                                              B2:180844-181353
```
The probes keep a walker out of terrain that rises steeply ahead. Buildings are not tested here; the contacts handle them.

## 9. Contact — FUN_0025c2b0(E, contact, other, list) (B2:177174-177615; thunk 0x2673d0 = physics slot +0x20) [O]

The contact record, as in `re-hover.md` §7: normal n at +0x10, depth at +0x20, our primitive at +0x24.
```
C1  other IsA DAT_00688f88: return 1 (ignored)                                   B2:177220-177224
C2  our prim's bit in K+0xfb8: return 1                                          B2:177228-177230
C3  foot = our prim is one of the K+0xe1a foot prims (K+0xe1c + 4i), or on the same bone      B2:177232-177257
C4  other class 2 (soldier): n.y > 0.9 and foot → damage the soldier (the stomp), continue    B2:177263-177281
C5  other class 0x80 (terrain): a leg pair claims the prim (FUN_002a1aa0) → return;             B2:177284-177313
        else v.y = 0; E+0x2244 = 0; supported
C6  other class 8 (vehicle): impact damage to the WALKER's own health E+0x170 (hover law: t = CT0 + CT1·(n·up) + CT2·(n·fwd);
        dmg = (s + t)²·(CS0 + …) when s < −t), supported when n.y > 0.7071, then the generic push-out   B2:177324-177486 (health B2:177463)
C7  other class 0x10 (building): K+0xe18 bit 6 and the building IsA DAT_00689368 → damage it;   B2:177487-177512
        n.y ≥ 0.2 and foot: v.y = 0; supported when n.y > 0.7071; return 0 (no push-out)                 B2:177513-177519
        else push out horizontally along (n.x, 0, n.z) by min(2|d|, depth²/|d|), d = n·depth (FUN_001699e0)   B2:177520-177553
C8  otherwise the generic FUN_00169668 push-out                                  B2:177601-177614
```
The dispatch is not exclusive (verifier, disassembly 0x25c584-0x25cd58). After C4, an unclaimed C5 and C6, the code re-reads
the class, fails the 0x10 test and falls into C8's generic push-out as well. Only a leg-claimed terrain contact (C5's
return) and a building (C7) skip it. C4 also needs the other to IsA DAT_00689828.

The exact form of C7's horizontal push-out distance was not re-derived from the disassembly [H]. Neither was which
classes DAT_00688f88, DAT_00689368 and DAT_00689828 name [H].

## 10. Pose — FUN_0025f050 / FUN_0025f210 (B2:179197-179382) [O]

> 2026-10-16 (card USKN, `re-soldier-skeleton-skins.md` §1, §6): the SkeletonRootScale written here is bone record bit 1 /
> +0x68, which the pose builders apply to bone_root's TRANSLATION only (`VMULx.xyz`), not as a subtree scale; the port's
> `walker_pose.rs` scales the subtree. No walker in the port's bake authors a value other than 1.

- **Before the skeleton update** (FUN_0025f050, slot +0x138):
  - bone 0's local rotation (E+0x1570) is multiplied by the quaternion (0, h.s, 0, h.c): a yaw of 2h. So the whole
    model turns with the legs, not the body;
  - the bone whose name hashes to 0x16e27226 gets SkeletonRootScale, (sin a, 0, 0, cos a) when a ≠ 0, and
    (0, 0, R.s, R.c) when R.c < 1.
- **After the update** (FUN_0025f210, slot +0x140): every one of these is undone with its conjugate, for the next clip
  sample.
- **The visible result [D].** When the body turns in place, the model stays planted: the root is counter-rotated by the
  accumulated 2h. The turn-step clip, or the 10 %-a-tick decay while walking, then brings the legs round. The upper hull
  follows the aim through the weapon aimers (VTUR). Which bone 0x16e27226 names is [H].

## 11. The class — setter, defaults, the discs' values

**Setter conversions (FUN_00263f68) [O]**, checked in RAM (§12):
- **TurnThreshold** 0x46bba6d5 → K+0x784 = FUN_00539f88(x·0.5) = sin(x/2) (B2:183237). RAM: 0.6 → 0.29552, 1.0 → 0.47943,
  0.45 → 0.22311.
- **MaxTerrainAngle** 0x47a95951 → K+0x788 = FUN_00539ff8(deg·π/180) = cos (B2:183263-183266). RAM: 30 → 0.86602. Not
  read by any walker function (the walker module, B2 lines 175 000-186 000, holds only the ctor, derive and setter
  accesses).
- **Three angle props:** 0xea47dd5c → K+0xe08, 0xafb0caf4 → K+0xe0c, 0xbed8917f → K+0xe10. Each is
  tan(min(|deg|·π/180, 1.5550884)), and 0 stays 0 (B2:183688-183702, 183746-183760, 183853-183867). RAM: 87/88/89 →
  19.0812 / 28.6364 / 57.2904.
- **Flag bits of K+0xe18:**
  - bit 0 = 0x55bc9af9 (read by the AI, B2:181581);
  - bit 1 = 0xe57fbb1d (aim window);
  - bit 2 = 0xba804df3 (skip the camera-relative turn);
  - bit 3 = 0x8ebd30be (camera-relative turn);
  - bit 4 = 0x3f164e6b (reverse steering);
  - bit 6 = 0x7fdeedf1 (damage buildings).
  Each is strtol(value) ≠ 0 (B2:183224-183229, 183320-183325, 183452-183458, 183522-183527, 183737-183742,
  183870-183875).
- **K+0xe14** = 0xd128c668 (B2:183830). **K+0xe04** = 0x954a0a1f (B2:183562). **K+0x78c** (roll) = 0x65587105 as bool
  (B2:183398-183401). **K+0x790** = WalkerWidth 0x0b9bb9c9 (B2:183052 → 183259).
- **JumpHeight** → K+0x750 = sqrt(19.6·h) (B2:183645-183652). **SkeletonRootScale** → K+0x668 (B2:183270).
  **YawLimits / PitchLimits** degrees → radians.
- **Quirks (verifier).**
  - Hashes 0x4aee3e6b and 0xc33a9f04 set a K+0xfb8 bit and then fall into WalkerWidth's `%f` read
    (0x265fb8-0x265ff8). A numeric value would overwrite WalkerWidth.
  - K+0xe18 bit 7 is neither set by the ctor nor copied by the derive, and RAM shows it set on one state's AT-ST.
    Nothing in §4 reads bit 7.
- **Sections.** With a WalkerSection open, the turret setter FUN_00270770 gets first refusal (B2:182865-182869). What it
  does not claim (legs, speeds, turn rates) still lands on the walker. Its claims include YawLimits and PitchLimits,
  hence the per-section limits.

**Ctor defaults (FUN_00263160, B2:182284-182463) [O]:**
- MaxSpeed 20; BoostSpeed 0; JumpHeight 0 (K+0x750); K+0x754 = 1.
- MaxTurnSpeed 2; MaxYawSpeed 2; YawLimits 0/0; MaxPitchSpeed 0.5; PitchLimits −0.5/0.5.
- StoppedTurnSpeed 0.1; ForwardTurnSpeed 0.1; BoostTurnSpeed 0.01; TurnThreshold 0.3 (raw); MaxTerrainAngle 0.9 (raw).
- roll 0; WalkerWidth 1.
- Energy {100, floor from FUN_001e1c88, recover 20}; EnergyAutoRestore 0; EnergyBoostDrain 1; EnergyCostJump 0.
- CollisionThreshold 7.5/2.5/0; CollisionScale 1/0/0.
- K+0xe04 = e08 = e0c = e14 = 0; **K+0xe10 = 5.0 (BF1's in-place ratio)**; flags = bits 1, 2, 3.
- StompThreshold 0.15; LegRayHitLength 2.5; LegPairCount 0; SkeletonRootScale 1.
- WaterDamageInterval 1; WaterDamageAmount 0.

**The discs' walker classes** (`lead_tmp/vwlk/walkcensus.py`: the baked class RONs, base chain resolved, section routing
as the setter does it; raw ODF values):

| class (BF2) | MaxSpeed / Boost | Stop / Fwd / Boost turn | TurnThr | e08/e0c/e10 (deg) | flags b0-b4 | YawLimits | LegRay / pairs | energy bar / regen / drain |
|---|---|---|---|---|---|---|---|---|
| imp_walk_atst (+_jungle, _snow) | 5 / 10 | 0.5 / 0.5 / 0.3 | 0.6 | 87/88/89 | 1 1 0 1 1 | −90 90 | 5 / 1 | 80 / 9 / 20 |
| rep_walk_oneman_atst | 5 / 10 | 0.8 / 0.6 / 0.4 | 0.6 | 87/88/89 | 1 1 0 1 1 | −90 90 | 5 / 1 | 80 / 9 / 20 |
| all_walk_tauntaun | 20 / 30 | 0.5 / 0.8 / 0.3 | 1.0 | 87/88/89 | 1 1 0 1 1 | −90 90 | (2.5) / 1 | 100 / 20 / 10 |
| cis_walk_spider (+kas) | 3 / — | 0.3 / 0.2 / — | 0.45 | 87/88/89 | – 1 0 1 1 | −90 90 | 10 / 2, roll, width 22 | — |
| imp_walk_atat (cmd) | 2 / — | 0.05 / 0.03 / — | 0.35 | 50/67.5/78 | – 1 0 1 1, b6 | −35 35 | 16 / 2 | — |
| rep_walk_atte (cmd) | 2.5 / — | 0.13 / 0.13 / — | 0.35 | 87/88/89 | – 1 0 1 1 | −20 20 | 5 / 2, roll, width 10 | — |

BF1 classes author none of the BF2-only props: e08-e10, the flags, Boost*, the energy props and JumpHeight. See §13.
MaxSpeed and MaxTurnSpeed also scale the AI's steering: FUN_00262248 → FUN_002a54d8(dt, MaxSpeed, 0 or 0.4·MaxSpeed,
MaxTurnSpeed, …) (B2:181615, 181632). **Transcribed since (2026-10-01, card WAI): `re-walker-ai.md`** — the AI's
steering lands in E+0x2e0 (p3), flag bit 0 selects its strafe branch, and §4.5 C's negative factor does turn a bot's
spider / AT-TE / AT-AT away from its goal while it walks.

## 12. The PS2 itself [O]

**12.1 Save states** (`walker_ram.py` over 15 BF2 states on HOT1, FEL1, GEO1, END1, NAB2, UTA1; `walker_ram_out.txt`).
- 19 walker entities were found by the vtable pair (0x61f850 at +0, 0x61f7f0 at +0xcc).
- The class fields match §11's conversions for the AT-ST, one-man, tauntaun and spider. E+0x2204 points at K+0x794,
  energy = EnergyBar, E+0x2194 = LegPairCount, and |h| = 1.0000 on every walker.
- Entry [7] in RAM = 6.8064 / 0.6667 (AT-ST), 5.5737 / 0.4667 (one-man), 21.75 / 0.3333 (tauntaun), 3.0746 / 0.9333
  (spider), equal to the disc decode.
- The moving AI walkers show |v_xz| = exactly one keyframe's root delta ×30 (one-man 5.566 = 0.18552·30; AT-ST at t =
  0.454 s: 9.31 = the 12→13 delta ×30). Four full-throttle walkers show v = 0 (a tick that did not cross a keyframe),
  as §5's sampling predicts.

**12.2 Why a single state cannot show the clock.** The root steps in whole keyframes, so a one-tick |v| is 0 or 1
keyframe for any clock between ×0.5 and ×1.

**12.3 Live** (`wlk_run.sh bf2_nab2_cp3_cp4_post1_spawn.p2s 45 nab2`; `live_nab2.csv`, 1466 rows; game time = the u64 cycle
clock at 0x66ae90 / 294,912,000, `a5-tickrate.md`). The AI one-man AT-ST (E = 0x1d2d8f0) walked 645 sample pairs (32.47
game s) at mean |stick| 1.006:
- **anim clock ×0.9000**, predicted ×0.9021 = 5·1.006/5.574;
- **ground speed 4.85 m/s** over the whole walk. Clean straight 2 s windows gave 4.70-5.24 (mean 4.84), against a
  predicted 5.02 (−3.6 %, inside the ±5 % window scatter; the shortfall's cause is [H]);
- the walker also stopped, stepped (turnr) and restarted during the run. Its h reached 0.508 (2h = 61°) before a turn
  step fired.

## 13. BF1's own walker, and BF2's code with BF1 data [O] (card WALKB1, 2026-10-07)

BF1's walker is the same design: the clips, the transition table, the legs, the root motion along the legs' forward.
It differs in three things. **The clip runs at the real dt** (no clock, so one speed per class). **The body turn is one
blend of two sticks** (§13.3). **Nothing reads ForwardTurnSpeed.** This section transcribes BF1's law, checks it tick
by tick on the PS2, says what BF2's code makes of BF1's data, and records the per-class call for the port.
Scratch: `engine/artifacts/lead_tmp/walkb1/`. Conventions for BF1: E = the entity (vtable 0x533b18), K = E+0x604; rows
right E+0x200, fwd E+0x220, pos E+0x230; v E+0x5f0; sticks E+0x300 throttle, E+0x304 left X, E+0x308 right X, E+0x30c
pitch; aimYaw E+0x644, aimPitch E+0x648, body pitch E+0x64c; h = (E+0x2384, E+0x2388); anim E+0x2330; walk direction
E+0x2350; dead E+0x22d0.

### 13.1 Functions and frame order [V]
- The activate B1 FUN_0021c9a0 registers the object at E+0x540 with key 0 and the task at E+0x23b0 with key −15, in
  list 2 (B1:132531-132532). The key-0 object's update is `… the
  task runs FUN_0021b3a0, which calls **the legs** FUN_0021ae90 (B1:131552).
- The input calls **the locomotion** FUN_00218fa0 through entity vtable slot +0x110 (word 0x533c28).
- **The contact** FUN_00217d50 is the physics object's callback (`FUN_00217d50(obj − 0x100)`, B1:134649; vtable word
  0x533a9c). The collision world is the key-10 task in both games (`re-vehicle-rest.md`), so a frame is **input and
  locomotion (0) → contacts (10) → probes and legs (15)**. That settles §2's [H] for the walker; §16 rests on it.

### 13.2 The class [V], RAM [O]
Ctor B1 FUN_0021cca8 (B1:132673-132819), setter FUN_0021d980 (B1:133152-134088):

| prop | K+ | ctor default | note |
|---|---|---|---|
| MaxSpeed | 0x17b8 | 20 | read only by the AI (B1:132205); the locomotion never loads it |
| MaxTurnSpeed | 0x17bc | 2.0 | the aim drag's rate R (§13.3), and the AI's turn rate |
| MaxYawSpeed | 0x17c0 | 1.0 | raised to MaxTurnSpeed **at the moment it is set** (B1:133216-133222) |
| YawLimits | 0x17c4 / 0x17c8 | 0 / 0 | degrees → rad |
| MaxPitchSpeed | 0x17cc | 0.5 | |
| PitchLimits | 0x17d0 / 0x17d4 | −0.5 / +0.5 rad | degrees → rad |
| TEMP_AnimationSpeed (0x6b8d8679) | 0x261c | 1.0 | every anim entry: rate = it, duration = frames/30/it (FUN_00230e30, B1:146441) |
| StoppedTurnSpeed | 0x2620 | 0.1 | **the** body turn speed, standing and walking |
| ForwardTurnSpeed | 0x2624 | 0.03 | stored, **never read**: the only two instructions with that displacement are the ctor's store (0x21cd84) and the setter's address (0x21ea10) |
| TurnThreshold | 0x2628 | 0.3 | sin(x/2) |
| PCMaxTurnSpeed / PCMaxPitchSpeed, PCPitchRate / PCMaxYawSpeed | 0x17a4 / 0x17a8 / 0x17ac | 20 | the mouse branch's rates |

- **The MaxYawSpeed quirk.** The AT-STs author MaxYawSpeed 0.7 *before* MaxTurnSpeed 1.5; at that moment MaxTurnSpeed is
  still the ctor's 2.0, so MaxYawSpeed becomes 2.0 and stays. RAM: K+0x17c0 = 2.0, K+0x17bc = 1.5.
- **TEMP_AnimationSpeed** (the name is in the mod tools' `odfguide.md`; its FNV is the hash). The AT-AT and the AT-TE
  author 1.5 on both discs, the AT-STs and the spider 1.0. **BF2 never reads it**: the constant is nowhere in BF2's ELF
  (no raw word, no `lui`, no immediate; two scanners), and at two sites in BF1's (0x1d4238, 0x21de7c).
- **Sections.** `WalkerSection = TURRET1..TURRET8` (or `TURRET`) opens section 0..7; any other value, `BODY` included,
  closes it (B1:134029-134080). With a section open the turret setter FUN_00225f08 gets first refusal (B1:133210-133215),
  as in BF2.
- The four defaults at K+0x2620..0x262c are not copied to a derived class (verifier); every walker class derives from
  the root and authors the three that are read.

**The classes in PS2 RAM** (`ram_b1class.py` over six states; raw props `wprops_bf1.txt`):

| class | MaxSpeed | MaxTurn | MaxYaw | YawLimits | MaxPitch | PitchLimits | Stopped | Forward | TurnThr | AnimSpeed | body yaw rate |
|---|---|---|---|---|---|---|---|---|---|---|---|
| imp_walk_atst (+_snow, _jungle) | 5 | 1.5 | 2.0 (0.7 authored) | ±35° | 0.4 | −35 / 40° | 0.5 | 0.03 | 0.45 | 1 | 1.0 rad/s |
| all_walk_tauntaun | 1 | 2 | 1 | 0 / 0 | 0.5 | ±28.6° | 1 | 1 | 1.0 | 1 | 2.0 |
| gun_walk_kaadu | 5 | 1 | 1 | 0 / 0 | 0.5 | ±28.6° | 1 | 1 | 1.0 | 1 | 2.0 |
| cis_walk_spider | 3 | 0.6 | 1 | ±45° | 0.5 | −25 / 20° | 0.1 | 0.03 | 0.45 | 1 | 0.2 |
| imp_walk_atat | 2 | 0.4 | 1 | ±35° | 0.5 | −25 / 5° | 0.05 | 0.03 | 0.35 | 1.5 | 0.1 |
| rep_walk_atte | 2.5 | 0.6 | 1 | 0 / 0 | 0.5 | ±20° | 0.1 | 0.03 | 0.35 | 1.5 | 0.2 |

### 13.3 The input — B1 FUN_00219ed8(dt, E) (B1:130443-130989; 0x219ed8-0x21ac10) [O]
The decomp text drops the locomotion call's third float; W3 below is read from the instructions.
```
W1  mouse = FUN_00311250(driver)   (true only in a network session: DAT_00518be4 / be8 are 0 in the ELF, set by the
                                    host scripts; offline 0)                                     B1:130692
    z = dt, or dt / seatweapon[+0xdc] when zoomed (E+0x2c0 ≠ 0 and E+0x67c ≥ 0 and the weapon exists)   B1:130712-130715
    rates:  human (slot E+0x388 ≥ 0): pitchRate = MaxPitchSpeed;  yawRate = MaxPitchSpeed + (MaxYawSpeed − MaxPitchSpeed)·E[+0x3c0]
            no human:                 pitchRate = MaxPitchSpeed;  yawRate = MaxYawSpeed                 B1:130716-130733
            (mouse: sticks ×4 clamped to ±1; rates K+0x17a8, K+0x17a8 → K+0x17ac by the same ramp)
W2  aimPitch += pitch·pitchRate·z;   aimYaw −= rightX·yawRate·z                                  B1:130734-130735
W3  over = aimYaw − YawMin when aimYaw < YawMin;  aimYaw − YawMax when aimYaw > YawMax;  else 0     0x21a56c-0x21a594
    t    = clamp(−over / (MaxTurnSpeed·dt), −1, 1)                (K+0x17a4 for the mouse)        0x21a598-0x21a5dc
    w    = 1 − |leftX|  (1 when leftX == 0);   turn = t·w + leftX  (turn = t when leftX == 0)      0x21a5e0-0x21a608
W4  fwd = throttle, overridden by the body pitch a: a < −0.25 → −1; a < −0.18 → min(fwd, 0);
          a > 0.25 → +1; a > 0.18 → max(fwd, 0)                                                 B1:130826-130841
W5  savedRight = the right row;   LOCOMOTION(dt, fwd, turn, E)                                    0x21a60c, 0x21a850-0x21a864
W6  aimYaw −= w·(savedRight · fwd_new) = aimYaw + w·sin 2θ                                        B1:130918-130931
    aimYaw = clamp(aimYaw, YawMin, YawMax);  aimPitch = clamp(aimPitch, PitchMin, PitchMax)       B1:130932-130946
W7  fall: not dead and v.y < 0: pos.y += v.y·dt                                                  B1:130947-130958
```
There is no yaw-rate ease near the limits (BF2's I13) and the aim yaw IS clamped (BF2 clamps only the pitch,
B2:180176-180182).

### 13.4 The locomotion — B1 FUN_00218fa0(dt, fwd, turn, E) (B1:129667-129913) [O]
```
M1  inPlace = |fwd|·5 ≤ |turn|                                                                 B1:129696
M2  θ = turn·dt·StoppedTurnSpeed, no clamp;  FUN_00219860(E, (cos θ, sin θ)) every tick           B1:129699-129703
        right' = (c²−s²)·right + 2cs·fwd;  fwd' = −2cs·right + (c²−s²)·fwd;  h ← h·(c + i s)      0x219860-0x2199e4
M3  idle (anim 0): dead → action 6;  inPlace → the turn step FUN_002199e8;  else action 2 (3 when fwd < 0)   B1:129704-129722
        FUN_002199e8: h.s > TurnThreshold → E+0x2380 = −atan2(h.s, h.c), action 5;  h.s < −TurnThreshold → action 4;
                      else action 0 unless already idle      (one threshold: no 0.1 as BF2's)      B1:130168-130199
M4  not inPlace: walking against the walk direction reverses the players and flips E+0x2350       B1:129733-129776
M5  rem ≤ dt: advance by rem, root delta, next action (dead 6; inPlace 0; fwd > 0 → 2; else 3), then the rest of dt;
    the clip advances by dt itself — no class speed, no multiplier                                 B1:129778-129872
M6  v.x, v.z = (the tick's root z delta)/dt · (fwd.x, fwd.z);  the body moves along the legs' forward, pitched 2a   B1:129880-129891
M7  anim 1-2: h turned by E+0x2380·dt/duration;  other non-idle anims: h decays 10 % a tick (FUN_002197c8)   B1:129892-129911
```
The action table at 0x5178e8 is BF2's DAT_005fd100 word for word (77 of 77). BF1's action 0 also becomes 1 when the
walk direction is 2; BF2's clamp of an anim index above 10 is not in BF1 (verifier).

**Legs** B1 FUN_0021ae90 (B1:131156-131362) [V]: the same 3 g. One pair: maxS ≥ 0 → the body is lifted by the excess
over 0.1 (DAT_00517a4c) and v.y = 0, else v.y −= 29.4·dt; BF1 keeps no supported flag. Two pairs: the body pitch gains
0.01 (DAT_00517a50) of the pair difference and is clamped ±0.55 (BF2: ±0.2955); the roll is set to
atan2(avgD, WalkerWidth) at once (BF2 eases it by 0.05 a tick).

### 13.5 What that is to drive [D]
- **Left stick Y** walks, at one speed: the clip's own (§13.6). There is no slow walk; any throttle that beats the 5:1
  in-place ratio plays the walk at full rate.
- **Left stick X turns the body**, standing or walking, at 2·StoppedTurnSpeed·x. A stick more than 78.7° off forward
  (|x| ≥ 5·|y|) turns in place.
- **Right stick X swings the aim** inside YawLimits; past a limit the aim is clamped and the excess becomes body turn
  (the drag, weighted by 1 − |leftX|), a deadbeat command at MaxTurnSpeed. On a 0 / 0 class (tauntaun, kaadu, AT-TE)
  the aim never leaves the body, so **both sticks steer**: turn = leftX + (1 − |leftX|)·clamp(rightX·yawRate /
  MaxTurnSpeed, ±1).
- The aim keeps its world direction through a body turn only in the share 1 − |leftX|: at full left stick the aim goes
  round with the body.
- **The AI** (B1 FUN_0021c180, `re-walker-ai.md` §9) writes its path turn into the left stick's X and its aim turn into
  the right stick's X; on a 0 / 0 class the aim turn steers too. It turns in place to 5°, then walks.
- No boost, energy, jump, camera-relative turn or reverse steering.

### 13.6 The gait [O]
A BF1 walker covers **z·TEMP_AnimationSpeed·30/N**: the clip of N frames lasts N/30/rate (FUN_00230e30) and its N − 1
intervals are spread over that time (the audit's finding). Live, the mean time of a whole walk clip is 0.3666 s
(tauntaun), 0.7004 (AT-ST), 1.8003 (AT-AT), 0.9671 (spider), 1.0218 (AT-TE), 0.3337 (kaadu): N/30/rate to 1 ms.

| walker | N | z (m) | rate | BF1 gait (m/s) | live BF1, AI in the walk loop | BF1 data under BF2's clock (the port today) | BF2's class values |
|---|---|---|---|---|---|---|---|
| AT-ST | 21 | 4.538 | 1 | **6.48** | 6.46 (130 clips) | 5.00 | 5.0, boost 10.0 |
| tauntaun | 11 | 7.250 | 1 | **19.77** | 17.96 (82 clips) | 10.88 | 20, boost 30 |
| kaadu | 10 | 6.515 | 1 | **19.54** | 18.39 (92 clips); ridden by the player 19.54 root (28 clips) | 10.86 | not fielded (BF1's values) |
| spider | 29 | 2.870 | 1 | **2.97** | 2.92 (59 clips) | 3.00 | 3.0 |
| AT-AT | 81 | 4.000 | 1.5 | **2.22** | 2.20 (87 clips) | 2.00 | 2.0 |
| AT-TE | 46 | 2.857 | 1.5 | **2.80** | 2.76 (median run; one of the two was blocked) | 2.50 | 2.5 |

(The audit banner's 1.48 and 1.86 for the AT-AT and AT-TE left the rate out; the port's 8 s drive from a standstill
reads 4.74 / 10.23 / 10.47 / 2.79 / 1.70 / 2.32, `port_bf1_*.txt`.)

**Why an AI tauntaun runs 18, not 19.8.** The root sampler interpolates only when the layer's word +0x1340 is set (B1
FUN_0043da00, 0x43da7c); the walker sets it only in its draw (0x2175e8, 0x217640) and clears it every tick in the task
(0x21bbe8, 0x21b000). An undrawn walker therefore steps whole keyframes, and the clip's LAST interval is usually lost at
the wrap: 66 of 82 tauntaun clips and 37 of 92 AI-kaadu clips came one interval short, each with two motionless ticks
instead of one. The loss is the size of that one interval: a tenth of the tauntaun's even stride, 0.024 m of the
AT-ST's eased 4.54 m (62 of its 130 clips lost it; verifier). The player's own kaadu, always drawn, moved 0.652 m on
every tick and lost none in 28 clips. The rest of the shortfall is the slope loss of §16. **The port interpolates
always, so its walkers run the drawn speed; the stepping is a PS2 saving, not a rule to copy.**

### 13.7 The law on the PS2 [O]
`b1_live.py` samples every walker once per game tick (the last read of each clock value; software renderer) with its
sticks; `b1_check.py` replays W1-W3, W5-W6 and M2 for one frame (the sticks of the new row, the state of the old one,
dt = the clock step) and compares the next fwd row and aim yaw.

| sample | class | steps | body yaw and aim yaw within 2e-4 rad | steps with a turn |
|---|---|---|---|---|
| Hoth, AI (`live_hot1_ai.csv`) | tauntaun | 6,552 | 6,552 | 1,684 |
| | AT-AT | 4,794 | 4,794 | 86 |
| | AT-ST | 4,794 | 4,794 | 3,773 |
| Geonosis, AI (`live_geo1_ai.csv`) | spider | 2,766 | 2,766 | 2,218 |
| | AT-TE | 4,794 | 4,794 | 4,599 |
| Naboo Plains, AI (`live_nab1_ai.csv`) | kaadu | 4,634 | 4,634 | 1,613 |
| Naboo Plains, **a human** (`human_nab1_k.csv`) | kaadu | 907 | 907 | 420 |

- The largest error is 3e-6 rad; the verifier's own replay gives the same counts.
- **Each term is needed.** Dropping the drag (over = 0) leaves 75-80 % on the AT-ST and tauntaun; w = 1 leaves 71 %
  on the AT-ST; a single angle 71-76 %; ForwardTurnSpeed while walking 84 % on the AT-ST and 82-85 % on the spider and
  AT-TE; the AT-ST's authored yaw rate 0.7 leaves 75 %; the previous row's sticks 71-99.9 %. For the human, the timer of
  the previous row or the AI's unramped rate leaves 90 %.
- **The human run** (the player placed beside the flat pad's kaadu by RAM, Triangle, scripted sticks): left stick
  right 1.5 s turned the body 2.94 rad at up to 2.000 rad/s; right stick right 2.5 s turned it 4.46 rad (1.0 rising to
  2.0 rad/s with the stick timer); throttle + left stick turned 5.94 rad in 3 s while covering 16.6 m/s, the legs
  trailing the body by 34° and the walker moving along the legs.
- Not exercised live (instructions only): W4's body-pitch override (the largest |a| in any row is 0.106), the zoom
  divisor, the mouse branch, a human on a class with non-zero YawLimits.

### 13.8 BF2's code with BF1's data [D], measured on the port
A class that authors none of BF2's props keeps BF2's ctor: flag bits 1, 2, 3; e08 = e0c = e14 = 0; e10 = 5. That is
every BF1-disc walker under the port's integrator until this card, and BF2's own unfielded kaadu. Through §4:
- **§4.1.** inPlace = 5·|p2| ≤ |p3| (BF1's ratio). Bit 2 blocks the bit-3 rewrite: a sideways stick does not walk.
  **§4.3.** No reverse steering (bit 4 clear).
- **§4.5 A** (bit 1) still sends the body after the aim once the aim leaves the window, and its command replaces B and C.
- **§4.5 B.** With e08 = 0 its first test is "the stick's side is opposite the aim's" (aimYaw ≤ 0 and p3 > 0, or
  aimYaw ≥ 0 and p3 < 0; B2:178281-178295). One tick of it puts the aim on the stick's side, and bit 2 then sends
  every later tick to C (B2:178299-178305). Standing, bit 2 goes straight to C. D is skipped (B2:178338).
- **§4.5 C** is then the human's turn as well as the AI's: cmd = p3·step, with no factor (e0c = 0) and no aim term
  (e14 = 0). step = TurnSpeed·dt, and **walking, TurnSpeed is ForwardTurnSpeed** (B2:178206-178218), the prop BF1 never
  read. On the AT-ST, spider, AT-AT and AT-TE it holds BF1's ctor default 0.03: the body turns at most 0.06 rad/s (3.4°/s)
  while walking, for a human and for a bot, against BF1's own 1.0 / 0.2 / 0.1 / 0.2. A bot walks only inside 5° of its
  goal (`re-walker-ai.md` §3 B9), so it stops and turns in place at every bend.
- **YawLimits 0 / 0** (tauntaun, kaadu, AT-TE): the window is one value, the legs' offset hh. A body turn moves aimYaw
  and hh by the same 2θ, so only the legs coming round changes aimYaw − hh, and A turns the body back to the aim. The
  left stick cannot steer; the right stick does, through A. BF2 does not clamp the aim yaw, so on the slow AT-TE (0.2
  rad/s) the aim runs away from the hull.
- **Speeds.** The clock's ×0.5 floor halves the tauntaun and the kaadu (§13.6).

`walker_drive` on main ab64c9c, BF1 data (`port_bf1_*.txt`): AT-ST, 4 s of left stick: the body turned 1.11 rad and
stopped; walking it yawed at most 0.060 rad/s. Tauntaun and kaadu, 4 s of left stick: 0.20 and 0.34 rad; right stick:
the body follows at 2.0 rad/s. AT-TE, 4 s of left stick: 0.00 rad; 5 s of right stick: the aim at −8.28 rad. Spider:
0.2 rad/s standing, 0.06 walking. The planted foot slips 7.8 m/s (tauntaun) and 8.7 m/s (kaadu) at the half clock.

### 13.9 The call (reversible; FEEL_NOTES has the lines)
PLAN locks BF2 handling for ground vehicles. **BF2's code with BF1's data is neither game** (§13.8), so on a BF1 map:

| class | the walker body's values | full stick | BF1's own |
|---|---|---|---|
| imp_walk_atst, _snow, _jungle | BF2's class of the same name (§11) | 5.0 m/s, boost 10.0 for 4 s; camera-relative walk; 1.0 rad/s | 6.48, no boost; left stick turns at 1.0 |
| all_walk_tauntaun | BF2's | 20, boost 30; 1.0 / 1.6 rad/s | 19.77; 2.0 |
| cis_walk_spider | BF2's, with e0c = 0 | 3.0; 0.6 / 0.4 rad/s | 2.97; 0.2 |
| imp_walk_atat | BF2's, with e0c = 0 | 2.0; 0.1 / 0.06 rad/s | 2.22; 0.1 |
| rep_walk_atte | BF2's, with e0c = 0 | 2.5; 0.26 rad/s | 2.80; 0.2 |
| gun_walk_kaadu | its own, plus what BF2's tauntaun changed for handling, and MaxSpeed = its BF1 gait | 19.54, no boost | 19.54; 2.0 |

- **"The body's values"** are what the integrator reads: speeds, the three turn speeds, TurnThreshold, the six flags,
  the three angles, YawLimits and PitchLimits of the body, the energy props, MaxTurnSpeed / MaxYawSpeed / MaxPitchSpeed,
  LegRayHitLength, roll, WalkerWidth, StompThreshold. Health, weapons, seats, turret sections, sounds, collision
  prims, model and clips stay BF1's. The clips are the same on both discs (§13.6's N and z are §5's).
- **e0c = 0 on the three classes without flag bit 0.** e0c feeds only §4.5 C's factor, which a human never reaches on
  a bit-3 class with bit 2 clear. With BF2's 88° (67.5° on the AT-AT) a bot's spider, AT-TE or AT-AT turns away from its
  goal while it walks and crabs 5° off it (`re-walker-ai.md` §7); BF1's bots converged. On BF1 maps they keep doing so.
- **The kaadu** has no fielded BF2 form. It takes the tauntaun's handling changes between the discs (StoppedTurnSpeed
  0.5, ForwardTurnSpeed 0.8, YawLimits ±90°, PitchLimits −35 / 40°, flags 0, 1, 3, 4, the angles 87 / 88 / 89°,
  StompThreshold 0.5) and MaxSpeed = z·30/N = 19.54, which BF2's clock plays at ×(N − 1)/N = ×0.9. No boost: it never
  had one.
- **The other way,** if the maintainer wants BF1's pace under BF2's handling: MaxSpeed = z·TEMP_AnimationSpeed·30/N on every
  BF1-map walker (AT-ST 6.48, AT-AT 2.22, AT-TE 2.80) and no boost. One switch in the build.

## 14. For the build (AT-ST first)

1. **Data.**
   - The walker class props with §11's conversions and ctor defaults, the base chain and section routing (extend
     `game::vehicle_class`).
   - Per anim bank: the 11-entry clip table, each entry's frames, duration, looping flag and root-z curve (track 0) —
     a small bake beside VBAKE's vehicles (new; the soldier bake already decodes clips).
2. **Sim.** A `walker::step` in the §2 order: §3 I7-I16, §4 all of it, §6, §8, and §9 as the contacts the port's
   `CollisionWorld` reports. State:
   - h, a, R;
   - anim index, direction and time;
   - flags;
   - E+0x21e4, energy, aim yaw/pitch, the water timer.
   Root motion comes from the baked curves, interpolated (the drawn-walker case, §5), at §4.8's clip dt.
3. **Input.** The walker takes the hover's pad mapping (throttle, strafe, turn, pitch; the full-stick timer). §4.6's
   control scheme follows from the class flags; nothing is port-specific.
4. **Visual (a second card).** The skinned walker GLB (VBAKE left walkers rigid) and the clip playback: the 3 layers, the
   leftup/rightup blend from P+0x120, and §10's root yaw and pitch/roll bone.
5. **PROVISIONAL:**
   - ~~the contact pass's place in the frame~~ (settled 2026-10-07: between the controllable and the body task,
     §13.1; the port still runs it last — card WCON);
   - C7's push-out distance;
   - what the stomp damage is;
   - bone 0x16e27226;
   - the AI steering (VTAI / VAI).
6. **Test numbers (AT-ST, dt = 1/30) [D].**
   - Full stick: clip ×0.7346, 5.0 m/s; boosting ×1.469, 10.0 m/s; |stick| < 0.681: ×0.5, 3.40 m/s.
   - Boost drains 20/s from 80: 4.0 s of boost from full. Below 0 the energy is exhausted (floored at 0); regen runs at
     9/s and clears it at 20, 2.2 s later.
   - The body yaws at most 1.0 rad/s (0.6 boosting).
   - Idle turn step at |2h| > 0.6 rad (TurnThreshold), or 0.2 rad when the stick is nonzero from a standstill (thr 0.1).
   - Falls at 29.4 m/s² until a foot ray finds ground.

## 15. Checks run, the verifier, what stayed [H]

Outputs are in `engine/artifacts/lead_tmp/vwlk/`: `check1_lines.txt`, `checks.txt`, `verify/`, `walker_ram_out.txt`,
`live_nab2.csv`, `walkcensus_out.txt`, `rootcurves.txt`.

- **Check 1 (`re_check.py fn`).** Run on all 290 cited lines.
  - The first pass found 11 bad citations. §9's contact lines were estimates, and several range ends ran one line into
    the next function. All are fixed.
  - The second pass puts every line in the function its section names.
- **Check 2 (absence).**
  - DAT_005fd234: two byte loads, no store (widescan), .data byte 0. The branch it gates is dead.
  - The +0x1335 interpolate flag: 6 hits in 5 functions, every writer named in §5.
  - K+0x788: no reader in the walker module.
  - FUN_0025c2b0 is reached only by the jal at 0x2673d8. FUN_0025d5e0 and FUN_00261208 are reached only by vtable words.
  - The jump hashes are on no walker class (census plus the 285-LVL PROP scan).
- **Check 3 (twins).**
  - B1 FUN_00218fa0, FUN_00219860 and FUN_0021ae90 were opened for §13.
  - The callers (text, data words, jal) of every B2 function in the chain are listed in `checks.txt`.
  - The commandwalker's vtable block holds the same functions.
- **Check 4 (bytes).**
  - Float literals from the code bytes of FUN_0025d5e0, FUN_0025f4a0, FUN_00260ba0 and FUN_002a2028.
  - .data words: DAT_005fd100 (11 × 7), 0x5fd238-0x5fd290, 0x5fdf50/0x5fdf5c, 0x5feda0 and 0x60bed4.
  - The vtable words and the clip-name strings at 0x61f308-0x61f548.
- **Check 5 (defaults and writers).** Every store to the following is listed with its function in `checks.txt`:
  - E+0x2190, E+0x2200, the boost latch, E+0x514 and E+0x21e8;
  - K+0xe18, K+0x750, K+0x668, the three turn speeds and K+0xe10;
  - DAT_0060bed4 (1 on every map with a terrain) and DAT_005fd234 (never written).
  The mouse flag behind FUN_003eef38 is clear offline (verifier).
- **PS2.**
  - 19 walker entities in 15 BF2 save states: the class conversions, the anim table, and one keyframe of root motion
    per tick.
  - The 45 s live NAB2 run: clock ×0.9000 against ×0.9021.
- **Verifier.** One Opus agent, in the foreground, given `verify/claims.md` only (48 claims). Verdicts are in
  `verify/VERDICTS.md`: **44 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED.** Everything below is folded in:
  - the four PARTLYs: I2's mouse branch; L11's custom-anim branch and FUN_0025aaa0's range; L13's anim-4 case; C1's
    non-exclusive dispatch;
  - its refinements: the self-sustaining boost latch, K+0x758 left uninitialised, the WalkerWidth fall-through,
    K+0xe18 bit 7, the one-man step alternation, B1:129696, and the per-frame walk FUN_001af988.
  The lead reproduced its one new number, the 285-LVL scan (§7).
- **Stayed [H]:**
  - where the contact pass runs in the frame (settled by card WALKB1: key 10, §13.1 and §16);
  - C7's push-out distance; which classes DAT_00688f88, DAT_00689368 and DAT_00689828 name; the stomp's damage;
  - which bit of E+0x2b4 is the press edge;
  - bone 0x16e27226's name;
  - the AI's steering (FUN_002a54d8) and its boost mode 3 — settled in `re-walker-ai.md` (2026-10-01), bar which game
    situations set mode 3; that doc also names §9's three class ids (EntityCarrier, EntityFlyer, EntitySoldier);
  - the live run's 3.6 % ground-speed shortfall against clip × clock ([H] with a likely cause since 2026-10-07: an
    undrawn walker loses its clip's last keyframe interval at most wraps, §13.6 — one of the one-man AT-ST's 14 is
    7 %; not re-measured on BF2);
  - whether the distant AI walkers in the states were undrawn (their whole-keyframe steps suggest so; BF1's switch
    is read and measured in §13.6);
  - the custom anims (K+0x675): none on the classes seen in RAM; the AT-AT's bank holds 14 extra clips;
  - reading +right as the vehicle's left, inherited from `re-hover.md`.
- **Rebuilding the scratch decoder.** `animspeed` is `engine/artifacts/lead_tmp/vwlk/animspeed` (a path dependency on
  `engine/crates/mungefs`). Build it with `cargo build --release --manifest-path engine/artifacts/lead_tmp/vwlk/animspeed/Cargo.toml`
  and any `CARGO_TARGET_DIR` under `engine/artifacts/lead_tmp/`.

### 15.1 Card WALKB1 (2026-10-07): §13 and §16
Outputs are in `engine/artifacts/lead_tmp/walkb1/`.
- **Hops.** B1: the activate FUN_0021c9a0, the input FUN_00219ed8 (thunk 0x21f910), the locomotion FUN_00218fa0, the
  turn FUN_00219860, the turn step FUN_002199e8, FUN_00219738 / FUN_00219550, the move FUN_0021ac10, the legs
  FUN_0021ae90, the contact FUN_00217d50 (thunk FUN_0021fca8) → FUN_00231b00, FUN_00168c90 → FUN_00168ff0 /
  FUN_001701a0; the class ctor FUN_0021cca8, derive FUN_0021d0e8, setter FUN_0021d980, FUN_00230e30; the mouse test
  FUN_00311250. B2: FUN_0025d5e0's turn tree and FUN_0025c2b0's terrain branch, re-read for §13.8 and §16.
- **Check 1** (`re_check.py fn`): every cited B1 line is in the function its paragraph names. **Check 4**: the table
  at 0x5178e8 (77 words, equal to BF2's 0x5fd100), DAT_00517a4c = 0.1, DAT_00517a50 = 0.01, the vtable words
  0x533c28 and 0x533a9c. **Check 3** (twins): BF2's FUN_0025c2b0 against BF1's FUN_00217d50 for §16; BF2's
  FUN_0025d5e0 and FUN_0025f4a0 against BF1's pair for §13.
- **Check 2** (absences), each by a scan, with its limit:
  - ForwardTurnSpeed unread in BF1: `offscan.py b1 0x2624` finds the ctor's store and the setter's address only (a
    base other than K would not show; the live replay with ForwardTurnSpeed on walking ticks fails, §13.7);
  - TEMP_AnimationSpeed unread in BF2: `constscan.py b2 0x6b8d8679` and `widescan.py bf2` find nothing, and the
    verifier's own any-alignment scan agrees;
  - FUN_00219ed8 and FUN_00217d50 are reached only by their two thunks (`refs.py`);
  - no yaw-rate ease and no speed multiplier in BF1's input and locomotion (the functions read whole; the verifier
    listed every class load: 0x2620, 0x2760 and 0x17a0 in the locomotion).
- **Check 5** (flags): DAT_00518be4 / DAT_00518be8 are 0 in the ELF; be8 is set only by `ScriptCB_SetAmHost`'s helpers
  (0x322f30, 0x322f58) and copied to be4 at 0x2fd59c (verifier): the mouse branch is a network-session branch.
- **PS2.** RAM class fields in six BF1 states (`ram_b1class.py`). Four live software-renderer runs under PCSX2
  (`b1_live.py`: Hoth, Geonosis and Naboo Plains with AI walkers; `human_drive.py`: the player on a Naboo kaadu).
  Card WAI's BF2 Hoth sample re-read for §16 (`b2_slope.py`).
- **The port.** `walker_drive` (main ab64c9c's binary) on the eight BF1 classes and their five BF2 twins
  (`port_bf1_*.txt`, `port_bf2_*.txt`).
- **Verifier.** One Opus agent, in the foreground, given `verify/claims.md` only (47 claims):
  **43 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED** (`verify/VERDICTS.md`; its own replay, scans and RAM readers are beside
  it). The PARTLYs are folded in: the building push's horizontal form and its third case (§16); the replay's scope
  (it does not replay W4, and rows of a dead walker are skipped: 389 tauntaun, 3 spider, 106 kaadu pairs); the AT-ST
  also loses its last interval, a small one (§13.6); one creep window straddles a facet change (§16). Its extras
  are folded in too: the interpolation switch (§13.6), the unexercised branches (§13.7), the uninherited defaults
  (§13.2), the three conditions of the zoom divisor (§13.3 W1), the network flag's writers.
- **Stayed [H]:** the creep's constant 0.4 % deficit against 0.98·sin φ; what E+0x3c0 is beyond a 0..1 ramp (taken as
  the hover's full-stick timer); the conditions inside the draw function that set the interpolation word; BF1's
  body-pitch override, zoom divisor and mouse branch (read, never exercised live); whether BF2's undrawn walkers lose
  the last interval the same way; the loader handing props to the setter in file order (what the AT-ST's RAM shows).
  The baked kaadu `p_r_foot` has radius 0 (an undecoded prim kind; the PS2 holds 0.22) — a bake defect for card WCON.

## 16. A body primitive on the terrain: the slope loss and the creep [O] (card WALKB1, 2026-10-07)

The KAAD audit saw unridden kaadus slide downhill on the PS2. It is §9 C5's else branch with C8, in both games, and
it also slows a ridden tauntaun or kaadu on every climb. The port builds neither (card WCON).

**The rule [V].**
- B1 FUN_00217d50 (B1:128859-129262), other class 0x40 (terrain): each leg pair is asked FUN_00231b00, which claims
  the contact only when its primitive is one of the pair's two foot prims (B1:147006-147025). A claimed contact
  returns. An unclaimed one sets v.y = 0 (B1:128994) and reaches the generic FUN_00168c90 (B1:129222-129223), which
  for terrain and buildings moves the body by **normal × depth** (FUN_00168ff0, B1:7578-7617).
- B2 FUN_0025c2b0 is the same (class 0x80; FUN_002a1aa0; v.y = 0, E+0x2244 = 0, supported; then FUN_00169668,
  B2:177284-177313, 177601).
- BF1 differences in the same function: a foot on a building face with n.y ≥ 0.8 (BF2: 0.2) gets v.y = 0 and then the
  generic push-out too (BF2 returns without one); the horizontal building push is min(2·depth·|n_xz|, depth/|n_xz|)
  along n's horizontal unit, and falls to the generic call when |n_xz| = 0 (BF1's form, from the instructions
  0x218424-0x21844c; verifier).
- **Which classes have a body prim in TerrainCollision:** `all_walk_tauntaun` and `gun_walk_kaadu`, on both discs:
  `p_collision` beside the two feet. Every other walker and commandwalker class lists only its leg pairs' foot prims
  (the verifier checked every class in all 323 LVL files). The kaadu's is a sphere of radius 0.5 at (0, 0.5, 0), its
  bottom at the origin; the tauntaun's a sphere of radius 1.5 at (0, 1.515, 0), its bottom 1.5 cm above the origin.

**What it does [D].** The frame is input (the fall: pos.y += v.y·dt) → contacts → legs (§13.1). On a slope of angle
φ the sphere holds the origin above the ground under the lower foot, so the one pair's support test (maxS ≥ 0, §6)
fails and the legs add one tick of 3 g: v.y = −29.4·dt. Next frame the body falls 29.4·dt², the sphere meets the
slope, and the contact zeroes v.y and pushes it out **along the terrain's normal**, not straight up. Per tick the body
therefore moves by the fall with its normal part removed: **29.4·dt²·sin φ down the fall line, |v| = 0.98·sin φ m/s**.
On flat ground the push is vertical and nothing moves. A ridden walker gets the same contact after every step: a
horizontal step d into a climb is pushed back along the normal and becomes **d·cos²φ − 29.4·dt²·sin φ·cos φ** over the
ground. Descending, it falls onto the slope and loses nothing.

**Measured on the PS2 [O].**
- *The creep* (`creep_fit.py` over the KAAD audit's three samples of unridden kaadus, 6 s windows): |v| / (0.98·sin φ)
  = 0.9955-0.9974 in 21 of 24 moving windows, on slopes of 2.9° to 14.1°; two windows hold a 4 s ride and one
  straddles a change of terrain facet (0.9855; second by second it stays 0.996-0.999, verifier). The kaadu on the flat
  pad moves 0.000. Headings follow the facet: (+0.0866, −0.0156, −0.0864) m/s on one pad for 30 s, (−0.1901, −0.0385,
  0.000) on the other. In this card's own 30 s sample (`human_nab1_k.csv`) the two unridden kaadus on slopes read v.y =
  −0.98 on 908 of 908 ticks and crept at 0.166 and 0.107 m/s, and the map's two parked AT-STs read 0 and moved
  0.0000 m. The constant 0.4 % deficit against 0.98·sin φ is [H].
- *The slope loss* (`b1_slope.py`, `b2_slope.py`: ground covered over root motion applied, walking ticks, by the path's
  slope; the model beside it):

| walker (sample) | climb 2-8° | climb 8-16° | climb > 16° | level | descent |
|---|---|---|---|---|---|
| BF1 kaadu, AI | 0.9882 at 4.8° (model 0.9885) | 0.9524 at 11.1° (0.9531) | 0.8930 at 17.7° (0.8931) | 0.9995 | 1.000-1.003 |
| BF1 tauntaun, AI | 0.9890 at 4.5° (0.9893) | 0.9379 at 12.7° (0.9398) | — | 0.9998 | 1.000-1.003 |
| BF2 tauntaun, AI (`wai/live_hot1.csv`) | 0.9904 at 3.7° | 0.9572 at 10.4° (20 ticks) | — | 0.9986 | 0.999-1.002 |
| BF1 AT-ST (feet only) | 0.9995 at 4.0° | 0.9995 at 11.2° | 0.9979 at 21.9° | 0.9995 | 0.9996 |
| BF2 AT-ST (feet only) | — | — | 0.9981 at 19.3° | 0.9982 | 0.9987 |

  The player's own kaadu fits tick by tick: steps of 0.637, 0.600 and 0.576 m at 7.2°, 15.0° and 18.5°, the model
  0.638, 0.600, 0.577 (a root step of 0.652 m).
- So a tauntaun or a kaadu climbs 5 % slower at 12°, 11 % at 18°, and a parked one slides off a slope at 0.05-0.24 m/s.
  The two-pair walkers lose only the cosine of their own body pitch (§4.9; AT-AT 0.9885 at 11.7°).

**For the build (card WCON).** `core::vehicle::walker::contacts` gains the terrain contact of every TerrainCollision
prim that no leg pair claims (v.y = 0, push-out by normal × depth; BF2's also sets supported), and the pass moves
between the controllable and the body task (today it runs last, which would zero v.y before the fall and hide the
creep). Referees: the three parked NAB1 kaadus' velocity vectors above, and the slope table.

## 17. The collision bodies ride the live pose [O] (AUDIT of card RKHUL, 2026-10-22)

A walker's collision prims are placed on its ANIMATED bones every update, not at the bind. Static read only (no RAM
sample); two separate workflow readers, each SUPPORTED on all four claims, plus the auditor's own `re_check` runs.

- **The body matrix [V].** B2 `FUN_00160160` (B2:121-218; `re_check fn`): with a table and a PRNT hash (body+0x48,
  MIPS 0x160180 `lw a2,72(s0)`), each output row is the XFRM row times the table matrix (B2:154-188, VMULA.x / VMADDA.y
  / VMADDA.z / VMADD.w broadcasting the XFRM row over the table's rows; ELF words 0x1601b0-0x1601bc): body = XFRM ×
  table[PRNT], row vectors, XFRM first - the order the port's `bake::bind_pose::prims_at_bind` uses. No table, hash 0
  or a failed lookup: the XFRM alone. B1 `FUN_00160160` (B1:121-220) is the same (hash at body+0x44, lookup
  `FUN_00444998`). (`analysis/re-turret-collision.md` 1.1 for the armed building.)
- **The walker's table [V].** The BF2 walker ctor `FUN_00259970` writes the physics vtable 0x61f7f0 at E+0xcc
  (B2:175465); slot +0x4c is word 0x61f83c = 0x00267438 = `jr ra` with delay slot 0x26743c = 0x24821d6c (`addiu v0,
  a0, 0x1d6c`): the table is (E+0x10)+0x1d6c = E+0x1d7c (`re_check word`). Slot +0x24 is `FUN_001691a0`, the same ray
  getter as the armed building's, and the target-point queries `FUN_002133c8` / `FUN_00213488` call slot +0x4c on
  E+0x10 (re-turret-collision 1.2). The ctor's `FUN_0057e988(E+0x1d7c, E+0x550)` (B2:175724, in `FUN_00259970`)
  enters, per bone, a POINTER to the skeleton instance's matrix E+0x550+0x10+0x40·i. A commandwalker's vtable
  0x62bde0 keeps the same three slots.
- **Rewritten every update [V].** `FUN_00261208` (the walker task) calls `FUN_00533f20(E+0x550, 0x73d8c0)`
  (B2:180843; B2:180841, cited here first, is the line before - card VPOSW), which ends in `FUN_00533d38`, writing each bone's 64-byte matrix at +0x10+0x40·i. So the prims follow
  the clip, with `FUN_0025f050`'s root yaw / pitch edits (§10) in the matrices.
- **BF1 [V].** Ctor `FUN_00215c88` writes vtable 0x533a80 at E+0x1c0; slot +0x44 is word 0x533ac4 = 0x0021fd10 =
  `jr ra; addiu v0,a0,0x1dc0` (0x21fd14 = 0x24821dc0): table E+0x1ec0, filled by `FUN_00497f28` with pointers to the
  instance's bone matrices; the walker task `FUN_0021b3a0` calls `FUN_00440b38(E+0x690, 0x66ef20)` every update
  (B1:131561).

**The port (main a88147b onward).** RKHUL bakes each prim through its bone's BIND matrix and `Sim::sync_hull` places
it rigidly (PROVISIONAL there). The drawn walker is posed by its clips (`game::viewer::vehicles` `pose_meshes`). A
pad-spawned walker after 90 ticks (idle, clip_time 0.97 spider): the BF2 spider's leg cylinders sit 3.34-3.47 m, its
foot spheres 3.64-4.29 m and its body sphere 0.89-0.96 m from the drawn idle legs and body; drawn idle vertices inside
the hull 1208 against 4980 at the bind model; rockets at the prims' drawn points hit 35 / 42 (BF2 GEO1) and 39 / 46
(BF1 GEO1) - 10 of 16 spider legs, 1 joint box and 3 of 3 AT-TE main barrels pass through - while rockets at the bind
points hit 46 / 46. Card WHULL. [H]: the PS2's own spider idle pose was not captured; the gap above is the port's
drawn pose against the port's hull.
