# OPUS RE (loop lane 2, 2026-09-26, card HVRE) - [O] UNAUDITED: no Fable pass
# RE: the hover integrator, transcribed — BF2's tick, spring point, inputs, drive, contact and position step

> **AUDIT 2026-09-28 (loop lane 1, audit of the HOVR landing, PARTLY — `engine/artifacts/loop/audits.md`).** This doc
> held under a re-derivation from the B2 decomp, the ELF and PS2 RAM, done before reading it. The port missed two of
> its terms, and the port's world contacts have two defects (card HCON). Scripts, RAM data and verdicts are in
> `engine/artifacts/lead_tmp/audit-hovr/`. One Opus verifier: 20 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED.
> - **Held:**
>   - §4, §6.2-§6.16 and §7 statement by statement, and §2's order (keys 0 → 15).
>   - Every name → offset the port reads (setter FUN_002223a0).
>   - RAM (the audit's own `ramscan.py`): 138 hovers in 30 states; g = 1 − dt on 134; 3-6 springs; the yaw target
>     matches on 129 / 138. The port's class numbers equal the live K bit for bit on 138 / 138. Rest heights match on
>     45 of 47 quiet parked hovers.
> - **Corrections:**
>   - **K9.** FUN_001699e0 moves the body by n·depth in both branches. FUN_001701a8 returns n·depth and never reads
>     the list, so there is no "constrained form".
>   - **The contact loop re-queries.** FUN_00160da8 (B2:711-800) passes each handled response's displacement to
>     FUN_0016beb8. That moves the query origin DAT_0066abf0 before FUN_0016bf10 finds the next contact, so
>     push-outs never stack.
>   - **§5.2.**
>     - The ±16 look clamp also needs the controlled entity to be an EntitySoldier (B2:234695, 234793); a hover's
>       look inputs stay ±1.
>     - The timer tests the ±1-clamped stick BEFORE FUN_002b4598's aim shaping (0x2b0a9c).
>   - **§3, +0x228 bit 3.** FUN_001e3c38 is the destructables' revive, not a hover path. FUN_00232f60 sets the bit
>     at full health (B2:148160).
>   - **§6.15.** The ELF computes (x − tilt)·(BankFilter·dt) (0x21ede4, 0x21ee24).
> - **New from RAM:**
>   - E+0x1cf0 = 0 on 509 / 509 springs and E+0x1cf4 > 0 on 492, so the contacts that feed a spring tick run earlier
>     in the same frame.
>   - E+0x500 = E+0x130 bit for bit on 138 / 138.
>   - Whether the pass runs before the drive or between the drive and the springs stays [H].
>   - §6.10's BoostSpeed scale is live on every shipped BF2 hover (BoostSpeed 17-40 on all 138), boosting or not.
>     §6.19 fits RAM: the one airborne SetAltitude-0 hover holds g = 0.

**The question** (card HVRE). The statement-level physics of BF2's hover. It runs 21 of the discs' 53 vehicle classes
(every hover and treaded class; BF1's two commandhovers reuse BF1's copy). PLAN locks BF2 ground handling, so this is
what HOVR builds. `re-vehicles.md` §3 located the functions and read their equations. It left five questions open:
where pos += v·dt runs, the strafe carve term, entity[0xf0] / +0x360, the B2 contact twin, and the 30·dt aim factor.
**All five are answered here (§0).**

**Method.** The lead read every function below in the B2 decomp, line by line. The disassembly settled what the decomp
hides (`scripts/tools/eedis2.py`): VU0 broadcast components, cross-product operand order, call arguments,
the slew branch. Float literals were read from the code bytes (`lead_tmp/hvre/floatlits.py`); .data constants and
vtable words with `re_check.py word`. Hops, in order:
- the thunks FUN_00224de0 / FUN_002245d0 → tick FUN_0021ca80 → spring FUN_0021f5d0 → FUN_005613c8;
- input FUN_0021b470 → drive FUN_0021d8b8, with its helpers FUN_0053b030 / FUN_0053ad40 / FUN_0053af38 /
  FUN_0053ae90 / FUN_0053a158 / FUN_0053a6a0 / FUN_0053a070 / FUN_001bc7c0 / FUN_00161c60 / FUN_00164ff0/8;
- SetTransform FUN_002249c0;
- contact thunk 0x224b00 → FUN_0021fd98 → FUN_0021f508 / FUN_0021f978 / FUN_00169668 / FUN_001699e0;
- the player controller FUN_002b0548 and FUN_003eef38;
- the task scheduler FUN_001af7e8 / FUN_001af508 / FUN_001af640;
- the ctors FUN_00219918 → FUN_00267bf0 → FUN_00212b38 → FUN_00168e18;
- the class ctor FUN_00221a20, derive copy FUN_00221e08, setter FUN_002223a0 and entry init FUN_00224368.

B1 twins were opened only for the differences in §8. Scratch notes: `engine/artifacts/lead_tmp/hvre/`.

**Conventions.** B2 = `analysis/SWBFRNT2.ELF-decomp.c` line; B1 likewise for SWBFRONT. `E` = the hover entity (drive
*(decompiler detail omitted)*, tick *(decompiler detail omitted)*, input *(decompiler detail omitted)*). `K` = its class (`E+0x4f4`). Offsets are bytes. Matrices are the PS2's
row-vector 4×4: rows right, up, fwd, pos; a local point p maps to p.x·right + p.y·up + p.z·fwd + pos. dt ≈ 1/30
(`a5-tickrate.md`; the port's fixed tick). Tags: [V] read at the cited lines, [D] derived, [H] guess, [O] = load-bearing
and verifier-SUPPORTED (§12).

## 0. The answers [O]

| VEH0 open question | answer |
|---|---|
| where pos += v·dt and the orientation step run | **In the drive itself** (§6.13–§6.16). p' = p + v·dt; q' = quat(E) ⊗ dq(ω·dt). E' = T(pitch, roll) · mat(q', p') is written to E+0x120 by the SetTransform virtual FUN_002249c0. No shared integration step. The world collision pass is separate: it calls the contact virtual (§7). Its place in the frame is [H]. |
| the strafe carve term | Only when StrafeSpeed ≥ 1e-4 and m > 1e-4: Δv = ((up·ω_yaw) × v)·dt, its length capped at \|tractionStep − accelStep\|, then scaled by m (§6.8). |
| entity[0xf0] / E+0x360 in the human yaw rate | E+0x360 = the controllable's +0xf0 (the controllable sits at E+0x270). **A full-stick timer:** the player controller adds dt while turn² + pitch² ≥ 0.9 (cap 1), else sets it to 0 (§5.2). A pad driver turns at PitchRate at first and blends to the class rate over 1 s of full right stick. |
| the B2 contact twin | **FUN_0021fd98**, reached through the thunk at 0x224b00 (`jal 0x21fd98; addiu a0,a0,-16`). The thunk is slot +0x20 of the physics-object vtable 0x61cc10 (§7). Damage is FUN_0021f978 (B1 FUN_001ee3e0's formula); the spring hook is FUN_0021f508. |
| BF2's 30·dt aim factor | aim += input · rate · 30·dt · dt (÷ zoom) (B2:132578-132579). At dt = 1/30 this is rate·dt, the same as BF1. |

Found on the way, all load-bearing for HOVR:
1. **The visual tilt feeds back into the body.** The drive untilts E for its frames (§6.2). It then builds q from the
   *tilted* E (B2:134413) and re-applies the new tilt on top (B2:134566-134601). Each tick's tilt therefore stays in
   the body's orientation. The springs and level spring must fight it. BF1 does the same (B1:98720-98750, 99299).
2. **BF2's TurnFilter is a slew rate**, not a filter. ω.yaw steps toward its target by TurnFilter·dt and stops at it
   (§6.10; disassembly 0x21e9e8-0x21ea2c). BF1 uses ω += (target − ω)·min(TurnFilter·dt, 1) (B1:99294-99298).
3. **A spring point measures the ground only through collision contacts.** A sphere pressed d into terrain or a
   building along normal n gives c = min(d·(n·up), BodySpringLength)/BodySpringLength, when n·up > 0.5 and n.y > 0.5
   (FUN_0021f508). The spring point's own query is the **water surface** (FUN_005613c8 = LAYR height + bob): hovers
   float on water. It is gated by class flag +0xe76, default on; its prop is `FloatsOnWater`, fnv 0xefb6b10a.
4. **Every shipped hover uses spring points.** All 22 baked hover-base class defs author 3-6 AddSpringBody (§6.4), so
   the no-spring branch (SetAltitude ray, lift) never runs on disc data. In PCSX2 RAM, all 138 live hovers (30 BF2
   states, 14 classes, 8 maps) have 3-6 springs.
5. **Impact damage is dealt for only two kinds of contact:** another class-8 (vehicle) body, and a spring sphere pressed
   deeper than its BodySpringLength. A hull primitive against terrain or buildings gets friction 0.25, restitution
   0.125 and a push-out, but **no damage** (§7).
6. **BF2 speeder bikes jump.** The class fields +0xf88..+0xf98 are authored only by `imp_hover_speederbike` and
   `rep_hover_barcspeeder` (§6.9). RAM: +0xf90 = 50 on the END1 and KAS2 bikes.

## 1. Fields

**Entity E (B2 hover; ctor FUN_00219918)** [V]:

| offset | what | written by |
|---|---|---|
| +0x10 | physics object; its vtable is at +0xcc = 0x61cc10; collision class at +0x34 = 8 (§7.0) | ctor chain |
| +0x20 | bounding sphere (centre xyz, radius w), world space | SetTransform |
| +0x120..+0x15c | E: rows right, up, fwd, pos. **Includes the visual tilt.** | FUN_002249c0 (B2:138524-138548) |
| +0x270 | controllable (vtable +0x284 = 0x61ca80). Its inputs are at E+0x2dc throttle, +0x2e0 strafe, +0x2e4 turn, +0x2e8 pitch; E+0x360 is the full-stick timer | player controller |
| +0x228 bit 3 | alive / active: set by FUN_001e3c38, cleared by the kill FUN_0021a840 (B2:131280) | — |
| +0x330 | driver's player slot; < 0 = AI or none | — |
| +0x4c0 | velocity v (world) | drive, spring, contact |
| +0x4d0 / +0x4d4 / +0x4d8 | ω x (pitch) / yaw / z (roll), **body frame** | drive, spring |
| +0x4e0 | ground factor g | spring (= 1), tick (−dt), drive (no-spring probe), contact (SA ≤ 0) |
| +0x500 | ground normal / up target | tick (= E.up), drive probe, contact |
| +0x544 / +0x548 | tilt pitch / roll (rad) | drive |
| +0x554 / +0x558 | aim yaw / pitch | input |
| +0x1cf0 + 32i | spring point i's compression c this tick (max) | contact hook, water query; cleared by the spring |
| +0x1dc0 | energy (boost, jump) | drive |
| +0x1dd4 | jump timer | drive |
| +0x1dd8 bit 41 | jumping | drive |
| +0x1ddc | boost flag (byte) | input |

**Class K (B2 offsets; ctor FUN_00221a20 B2:136606-136746; derive FUN_00221e08).** The §3.1 props of re-vehicles
keep their offsets. Also used here [V]:
- **Spring points** — count +0xf54. Each point is a 32-byte entry at +0xe94 + 32i: [0] sphere id, [1] length, [2] VSF,
  [3] VDF, [4] OXSF, [5] OXDF, [6] OZSF, [7] OZDF.
- **Water float +0xe76** (`FloatsOnWater`) — ctor 1 (B2:136724); the derive copies the parent's (B2:136936); prop hash
  0xefb6b10a sets it to strtol(value) != 0 (B2:138034-138036). Only `cis_tread_snailtank` authors it (= 0). RAM: 0 on
  the FEL1/KAS2 snail tanks, 1 on every other hover class.
- **Spring jitter +0xe90** — ctor 1; NoRandomSpring clears it.
- **Energy** — EnergyAutoRestore +0xf64 (0); drain per second while boosting +0xf68 (0; every BF2 hover authors 20).
- **Boost** — BoostSpeed +0xf6c (0), BoostAcceleration +0xf70 (0); turn scale at full boost +0xf78 (ctor 0.5;
  speeder bikes 0.3).
- **Jump** — +0xf88 (0), +0xf8c (0), +0xf90 (0), +0xf94 (0.3), +0xf98 (0). The two BF2 speeder bikes author 0.1,
  0.35, 50, 0.4, 100.
- **Tread UV scroll** — count +0xf9c (hailfire 2, snailtank 1); scales +0xfb0/+0xfc0/+0xfd0/+0xfe0.
- **Landing effect** — g threshold +0xcd4 = 0.9.
- **Aim** — +0x684 = 1.0 (aim yaw rate); aim-yaw clamp +0x690/+0x694 = 0/0. Neither setter writes these two (§12).

## 2. The frame order [V][O] except the contact pass [H]

The hover's two updates are tasks in the same task list (kind 2):
- The controllable (E+0x270) registers through its vtable slot +0x10. That slot IS FUN_001af7e8 (ELF word 0x61ca90 =
  0x1af7e8), called with key 0 (B2:136420).
- The physics task (E+0x1cd0, vtable 0x61cd98, slot +0xc = FUN_00224de0) registers with key 15 (B2:136421).
- FUN_001af508 keeps the buckets in ascending key order (B2:55969-55977). FUN_001af640 runs them in list order
  (disassembly 0x1af658-0x1af680).

Per frame, therefore: **controller → FUN_0021b470 (drive: forces, yaw, pos, orientation) → FUN_0021ca80 (springs, g
decay)**. The contact pass (FUN_00160da8 / FUN_00169328 call the body's contact virtual, B2:770 / 7902) is reached
only through function pointers. Its place in the frame is [H] here — **settled 2026-10-07 (card VREST,
`re-vehicle-rest.md` §5): the collision world's task runs at key 10, BETWEEN the drive and the spring tick, in both
games.** It fills each spring's c; the next spring tick (the same frame's) uses it
and clears it.

**RAM check** (`lead_tmp/hvre/hover_ram.py`: entities found by vtable 0x61cc70 with 0x61cc10 at +0xcc, in all BF2
PCSX2 states). The states were captured between frames:
- g = 0.9667 = 1 − 1/30 on 134 of 138 live hovers — every touching hover. So the spring tick ran after the last drive.
- The other four: 0.9333, 0.8, 0.5667, 0 — that is, 2, 6 and 13 ticks since the last contact, and one long airborne.

## 3. The tick — FUN_0021ca80(dt, E) (via FUN_00224de0, B2:138694) [V][O]

```
T1  if FUN_003d7a20(E, &dt) != 0: return          B2:132891-132894  (0 offline: returns 0 when DAT_00600548 == 0, B2:433482-433483)
T2  if E.driver >= 0: DAT_006d85c4 += 1           B2:132896-132898  (a counter; not physics)
T3  for i in 0 .. K+0xf54: spring_point(dt, E, i) B2:132899-132906  (§4)
T4  E+0x500 = E.up (E+0x130)                      B2:132908-132911
T5  g = max(g - dt, 0)                            B2:132907, 132912-132915
T6  aim ray, effect slots, engine sound, rider matrices   B2:132916-133467  (no store to v, ω or E — §12 check 2)
```

## 4. A spring point — FUN_0021f5d0(dt, E, i) (B2:134917-135024) [V][O]

```
S1  sph = FUN_00168cd0(E+0x70, K[+0xe94+32i])    B2:134938-134940   (the point's collision sphere; null → return)
S2  wsph = FUN_00160258(sph, E+0x120)            B2:134941          (world centre at wsph+0x30, uses the TILTED E)
S3  if K+0xe76:                                   B2:134942          (water float, default on)
      r = radius(sph)                             B2:134943
      if FUN_005613c8(centre, &h):               B2:134944          (h = water LAYR height + bob, only over a wet cell)
        d = h - (centre.y - r)                    B2:134945-134946
        if d > 0: c_i = max(c_i, min(d / len_i, 1))              B2:134947-134952
                  if d/len_i > 1 and v.y < 0: v.y *= 0.5; v.x *= 0.9; v.z *= 0.9; ω *= 0.9   B2:134953-134963
S4  if c_i > 0:                                   B2:134968
      if K+0xe90: c_i *= 0.9 + 0.1·u, u ∈ [0,1) from the LCG x' = x·0x19660d + 0x3c6ef35f, run twice   B2:134969-134975
      g = 1                                       B2:134978
      v += up · (c·VSF_i·VelocitySpring − (1−c)·VDF_i·VelocityDamp·min(v·up, 0)) · dt     B2:134979-135000  (up = E+0x130, tilted)
      ω.x += (c·OXSF_i·OmegaXSpring − (1−c)·ω.x·OXDF_i·OmegaXDamp) · dt                     B2:135004-135008
      ω.z += (c·OZSF_i·OmegaZSpring − (1−c)·ω.z·OZDF_i·OmegaZDamp) · dt                     B2:135009-135013
S5  E+0x1cf4+32i = c_i;  c_i = 0;  E+0x1d00..0x1d08+32i = 0              B2:135015-135021
```
FUN_005613c8 (B2:696593-696621) runs only when a water grid exists (DAT_006615d4 != 0). It clamps (x, z) into the grid;
if FUN_00561ef0 says the cell is wet, it returns 1 and h = DAT_00661604 + DAT_0060bef0: the first LAYR height plus the
6 cm bob (`re-water.md` §2, `re-water-flags.md` §7). Otherwise it returns 0. The jitter's LCG state DAT_0073d868 is one
engine global; the port cannot know it (PROVISIONAL: a per-vehicle LCG).

## 5. Inputs

### 5.1 FUN_0021b470(dt, E) (via FUN_002245d0 = controllable slot +0xc, B2:138269) [V][O]
```
I1  if FUN_003d7a20(E, &dt) != 0: return          B2:131964-131968   (same gate as T1)
I2  (net / death / effect bookkeeping, seat cycling, sounds)          B2:131969-132509
I3  mouse = E.driver >= 0 ? FUN_003eef38() : 0    B2:132510-132513   (a profile flag, [H] mouse/keyboard; the pad → 0)
I4  thr, str, trn, pit = E+0x2dc, +0x2e0, +0x2e4, +0x2e8                 B2:132515-132518
I5  boost = (E.driver >= 0 and thr > 0.9 and (E+0x2bc bit0 or boost))    B2:132519-132526
          or (AI with E+0x324 != 0 and FUN_002b6ad0()+0x4d0 == 3)         B2:132540-132548;  E+0x1ddc = boost  B2:132550
I6  z = dt, or dt / seatweapon[+200] while zoomed                         B2:132551-132554
I7  if !mouse: AI  → yawRate = K+0x684, pitchRate = PitchRate
               human → pitchRate = PitchRate; yawRate = PitchRate + (K+0x684 − PitchRate)·E[+0x360]   B2:132555-132564
    else yawRate = pitchRate = K+0x684                                   B2:132566-132568
I8  aimPitch += pit·pitchRate·30·dt·z;  aimYaw −= trn·yawRate·30·dt·z     B2:132578-132579
I9  FUN_0021d8b8(dt, thr, str, trn, E, &boost)                           B2:132580  (§6)
I10 aimYaw −= oldRight·E'.fwd; aimPitch −= oldUp·E'.fwd  (the vehicle's own turn this tick)   B2:132570-132577, 132581-132606
I11 aimYaw = clamp(aimYaw, K+0x690, K+0x694); aimPitch = clamp(aimPitch, PitchLimits)            B2:132607-132620
I12 water wake effect (FUN_005613c8, +0.02 m)                            B2:132628-132700  (not physics)
```

### 5.2 The player controller FUN_002b0548 (B2:234571) — what fills E+0x2dc.. and E+0x360 [V][O]
- mouse = FUN_003eef38(player) (B2:234777).
- pad[0], pad[1], pad[2], pad[3] (the controller record `a local[0x2c]`) become strafe, throttle, turn, pitch.
  Each is clamped to ±1 (±16 with the mouse flag) (B2:234778-234811). Which physical stick is pad[0..3] is [H].
- **Pad only** (B2:234817-234829): if turn² + pitch² ≥ 0.9, then +0xf0 = min(+0xf0 + dt, 1) and +0xf4 likewise;
  else both = 0.
- The move pair: any nonzero axis under 0.3 in magnitude becomes ±0.3. The pair is then scaled into the unit circle,
  and so is the look pair (B2:234831-234863).

## 6. The drive — FUN_0021d8b8(dt, thr, str, trn, E, boost*) (B2:133566-134804) [V][O]

```
6.1  if StrafeSpeed < 1e-4: str = 0                                     B2:133653-133655
6.2  M = Rz(−roll)·Rx(−pitch); U = M·E  (untilted rows right, up, fwd, pos)  B2:133657-133694
       Rz(a) rows (c,s,0),(−s,c,0),(0,0,1) [FUN_0053b030 builds]; M ← M·Rx(a), Rx rows (1,0,0),(0,c,s),(0,−s,c) [FUN_0053ad40]
     m = max(|thr|, |str|)                                             B2:133659-133666 (max.s 0x21d988)
6.3  energy: if !boost:
       if !jumping: FUN_001e1fc0(f·EnergyAutoRestore·dt, E+0x1dc0) (regen; f = 1, or 1.5 [DAT_005feda0] when the
                    driver's +0x1a0 passes FUN_00360018(·,6))           B2:133696-133708
     else if BoostSpeed > 0: if !FUN_001e2010(K+0xf68·dt, E+0x1dc0): boost = 0, then regen   B2:133712-133723
     else boost = 0, then regen                                          B2:133724-133728
```
**6.4 No spring points (K+0xf54 < 1)** (B2:133730-134114). No shipped hover takes this branch (§0.4). It is
transcribed for completeness; HOVR may leave it PROVISIONAL.
```
 a  if SetAltitude > 0:                                                 B2:133731
      o = bounding-sphere centre (E+0x20, or the same sphere's copy in the array at *(E+0x10) when that is set;
          FUN_002249c0 writes both), L = its radius + 10·SA             B2:133732-133759
      t = ray(L, o, (0,−1,0)) → n (preset (0,1,0), DAT_0073d8a0); t = 1.0 when nothing is hit
          (FUN_00161c60 → FUN_00163078 returns hitlength / length, with hitlength starting at length)   B2:133760-133762
      E+0x500 = n;  h = L·t + U.pos.y − o.y                             B2:133766-133770
      if v.x² + v.z² > 25:                                              B2:133763-133771 (VADDz.x 0x21deb4)
        d = v + (0, −L, 0); t2 = ray(|d|, o, d/|d|) → n2                B2:133772-133795
        if t2 < 1: h = min(h, −(t2·d)·n2)                               B2:133796-133810
                   if n2.y > 0: E+0x500 = normalize(n + (n2 − n)·(1 − t2)·n2.y)   B2:133811-133831
      g = clamp(1 − (h − SA)/(9·SA), 0, 1) → E+0x4e0                    B2:133837-133844
      target up = normalize(n.x·g, n.y + 0.1, n.z·g)                     B2:133845-133860
    else if g == 1: ray(32, U.pos + (0, r, 0), (0,−1,0)) → E+0x500; target up = E+0x500   B2:133864-133886
    else target up = (0, 1, 0)                                           B2:133888-133892
 b  n = normalize(E+0x500); R' = normalize(n × U.fwd); F' = R' × n;
    ground frame G = rows [R', n, F', 0]                                 B2:133894-133932 (VOPMULA/VOPMSUB 0x21e1f4, 0x21e220; FUN_001bc7c0 = rows)
 c  v.y += ((GravityScale − 1)·g + 1)·(−9.8)·dt                         B2:133933-133936
 d  if SA > 0: f = 9.8·g + LiftSpring·(SA − h) − LiftDamp·(v·n); if f > 0: v += n·f·dt   B2:133938-133963
 e  ω.x *= 1 − min(LevelDamp·dt, 1); ω.z likewise                      B2:133964-133971
    ω.x += (U.fwd·t)·min(LevelSpring·dt, 1); ω.z −= (U.right·t)·min(LevelSpring·dt, 1)   B2:133972-133994
 f  thrust (6.7) with F = F' (G row 2), S = R' (G row 0), carve axis n (G row 1)          B2:133995-134114
```
**6.5 Spring points (else)** (B2:134115-134250).
```
 a  v.y += (GravityScale + (1 − GravityScale)·g)·(−9.8)·dt              B2:134116-134119
 b  ω.x += U.fwd.y·|U.fwd.y|·min(LevelSpring·dt, 1)                     B2:134120-134124
    ω.z −= U.right.y·|U.right.y|·min(LevelSpring·dt, 1)                 B2:134125-134129   (no LevelDamp here)
 c  thrust (6.7) with F = U.fwd, S = U.right, carve axis U.up           B2:134130-134250
```
**6.6** (the ground factor) g enters only through s in 6.7 and the gravity weights above.

**6.7 Thrust — velocity matching** (B2:133995-134114; 134130-134250).
```
 s  = g · max(U.up.y, 0) · dt
 aS = (boost ? BoostAcceleration : Deceleration + (Acceleration − Deceleration)·m) · s
 tS = Traction · s
 target = thr < 0 ? ReverseSpeed : (boost ? BoostSpeed : ForwardSpeed)
 v += F · clamp(thr·target − v·F, −aS, aS)                             (then stored)
 if StrafeSpeed < 1e-4:  v += S · clamp(−str·StrafeSpeed − v·S, −tS, tS)       (traction; str = 0 here)
 else:                   v += S · clamp(−str·StrafeSpeed − v·S, −aS, aS)
                         6.8 carve, if m > 1e-4
```
**6.8 Carve** (B2:134074-134110; 134209-134245; disassembly 0x21dcfc-0x21dd60).
```
 c = ((axis · ω.yaw) × v) · dt;  lim = tS − aS
 if lim² < |c|²: c *= lim / |c|        (sqrtf FUN_0052bc10; a negative lim flips c)
 v += c · m
```
**6.9 Jump** (B2:134251-134333), only when K+0xf90 > 0. Call the five jump fields f88, f8c, f90, f94, f98.
```
 not jumping: if button (E+0x2b4 bit0) and g > 0.9 and E.fwd·v > FS·f94: start (bit 41 on); timer = 0
 jumping: if timer ≥ f88 and !button: stop
          else if !drain(f98·dt): stop
          else timer += dt; f = clamp((E.fwd·v − FS·f94)/(FS − FS·f94), 0, 1)
               if timer < (1 − f)·f88 + f·f8c: v += E.up · f90 · dt
               else stop
```
**6.10 Yaw** (B2:134334-134412; disassembly 0x21e9cc-0x21ea2c).
```
 speed = |v|; vF = E.fwd·v   (E: the tilted row)                              B2:134336-134352
 f = clamp(vF / ForwardSpeed, 0, 1); base = SpinRate + (TurnRate − SpinRate)·f
 rate = (AI or mouse) ? base : PitchRate + (base − PitchRate)·E[+0x360]       B2:134353-134381
 target = −rate · trn;  if BoostSpeed > 0 and speed > FS:
     fb = clamp((speed − FS)/(BoostSpeed − FS), 0, 1); target *= 1 − fb·(1 − K+0xf78)   B2:134382-134398
 if ω.yaw < target: ω.yaw = min(ω.yaw + TurnFilter·dt, target)
 else:              ω.yaw = max(ω.yaw − TurnFilter·dt, target)                 B2:134399-134412
```
**6.11 Orientation** (B2:134413-134446).
```
 q = quat(E+0x120)   (FUN_0053a158(out, matrix): trace method, then x, y, z negated — the row-convention quaternion)
 if |ω| > 1e-4: a = |ω|·dt; dq = (ω/|ω| · sin(a/2), cos(a/2))   (FUN_0053a070: 2048-entry table sincos)
                q' = q ⊗ dq   (FUN_0053a6a0 = Hamilton product a·b: ω is in the BODY frame)
```
**6.12 Trample** [H] (B2:134447-134506). If speed > 0.1·FS: FUN_001c9460(4r², r, centre − (v/|v|)·2r, v·dt/2), where
r = the bounding radius. It is a sphere query ahead of the body; what it does to what it finds was not read.

**6.13 Position** (B2:134424, 134507-134531; disassembly 0x21ecfc-0x21ed58).
```
 p' = E.pos + v·dt
 v_local = (U.right·v, U.up·v, U.fwd·v)
```
**6.14 Bank input** (B2:134532-134556; VOPMULA/VOPMSUB 0x21ed6c): k = (ω × v_local) / ForwardSpeed.

**6.15 Tilt** (B2:134557-134565).
```
 pitch += ((thr·ThrustPitchAngle + k.z·BankAngle) − pitch) · BankFilter · dt
 roll  += ((str·StrafeRollAngle − k.x·BankAngle) − roll) · BankFilter · dt
```
**6.16 Write-back** (B2:134566-134601).
```
 T = Rx(pitch)·Rz(roll)   (FUN_0053af38 builds Rx; FUN_0053ae90 T ← T·Rz)
 E' = T · mat(q', p')     (FUN_0053ac20 builds mat)
 E+0x120.. = E'           (slot +0x14 of 0x61cc10 = FUN_002249c0, B2:138524-138548; also moves the bounding sphere)
```
Next tick, §6.2's U = mat(q'), but §6.11's q = quat(T·mat(q')). So each tick's tilt stays in the body (§0.1).

**6.17 Effects and sound** (B2:134602-134741). A landing effect (FUN_0043ceb0) fires when g rises past its previous
value while g ≤ K+0xcd4 = 0.9. Engine sound: FUN_0044c650 / FUN_00441350. Pilot anim blend: E+0x54c / E+0x550.

**6.18 Tread UV scroll** (B2:134742-134797): for i < K+0xf9c, uv += dt·(vF·scale + ω.yaw·scale), wrapped to [0, 1).

**6.19** (B2:134798-134800): if SetAltitude ≤ 0: g = 0.

## 7. Contact — FUN_0021fd98(E+0x10 → E, contact, other, list) [V][O]

**7.0 Wiring.**
- Collision class 8: FUN_00219918 loads a3 = 8 (0x21991c) → FUN_00267bf0 leaves a3 alone → FUN_00212b38 (s1 = a3,
  then a1 = s1) → FUN_00168e18 stores it at [9] = E+0x34 (B2:7530).
- The contact record: normal at +0x10, depth at +0x20, our primitive index at +0x24 (byte). FUN_00160da8 fills it
  and calls the virtual (B2:770).
- The thunk word 0x224b00 also sits in a second vtable at 0x62b8c8 (0x62b8a8 slot +0x20). Subclass entity ctors
  FUN_00353888 / FUN_00353928 install it (B2:348863, 348878; the commandhover family, which no BF2 map ships).
- Masses default 1.0 (+0x68 of the physics object, B2:7546).

```
K1  if class(other) < 8: → base                                  B2:135237
K2  vrel = v − v_other;  s = vrel·n                               B2:135238-135257
K3  if class(other) == 2: → base                                  B2:135259-135265
K4  if class(other) == 8: damage(s)  [FUN_0021f978], then K7      B2:135274-135275
K5  else i = springhook(depth, prim, n)  [FUN_0021f508]:          B2:135278
      the point whose sphere is prim: if n·E.up > 0.5 and n.y > 0.5:
          c_i = max(c_i, min(len_i, depth·(n·E.up)) / len_i)       B2:134865-134913
    if i >= 0:
      if depth <= len_i: return 0          (contact left to the spring)       B2:135281-135284
      damage(s)                                                                B2:135285
      if s < 0 and alive: v −= n·s; v −= (0.1·n·s with y zeroed)   (DAT_005fcfbc = 0.1)   B2:135286-135318
      push out by (depth − len_i) along n  [FUN_001699e0]                      B2:135321
      return 1
K7  if s < 0 and alive (E+0x228 bit 3):                           B2:135340-135341
      μ = 0.25, or (1 − max(E.up·n, 0)⁴)·0.25 when SetAltitude == 0   B2:135342-135353
      j = −1.125·s;  class(other) == 8: j *= m_o'/(m_s·(0.5·max(v_other·n, 0) + 1) + m_o'),
                                       m_o' = m_o·(0.5·max(−v·n, 0) + 1)   (DAT_005fcfc0 = 0.5)   B2:135354-135397
      vt = vrel − n·s;  vt −= vt·min(μ·j/|vt|, 1);  v = vt + n·(j + s) + v_other   B2:135400-135447
      (so the normal speed comes back at 0.125·|s|, and the tangential speed loses 0.28125·|s|)
      skid sound                                                  B2:135457-135492
K8  if SetAltitude ≤ 0 and n.y > 0.5: E+0x500 = n; g = 1          B2:135501-135509
K9  base: FUN_00169668 → FUN_001699e0(depth, body, n, list): pos += n·depth, or its constrained form with a
    contact list   B2:135511-135525, 7937-8054, 8086-8130
```
**Damage FUN_0021f978(s, E, n, other)** (B2:135029-135181).
```
 a = n·E.up;  b = n·E.fwd
 if s < 0:
   t = CT0 + CT1·a + CT2·b
   if s < −t: dmg = (s + t)² · (CS0 + CS1·a + CS2·b)
              dmg *= a factor from the other body's class (vtable +0x40) [H]
              if dmg > 0: FUN_001e0900(E+0x170, {…, dmg, 1.0 ×9}, −1)
```
Some other bodies return early (an IsA test against DAT_006894f8 / DAT_00689960, [H] which classes).

## 8. BF1 differences that matter to the port

- **Yaw.** BF1 filters: ω.yaw += (−rate·turn − ω.yaw)·min(TurnFilter·dt, 1) (B1:99294-99298). BF2 slews (§6.10).
  Same TurnFilter number: BF1 reaches 63 % of the target in 1/TurnFilter seconds; BF2 reaches the whole target in
  |target|/TurnFilter seconds. PLAN's lock takes BF2's (FEEL_NOTES line when HOVR lands).
- **Turn blend and fold.** BF1 blends by stick and folds strafe into turn (re-vehicles §3.3). BF2 blends by speed,
  with no fold.
- **Tilt feedback.** The same in BF1. The drive untilts the matrix at +0x200 (rotations B1:98720-98721, pointer
  B1:98723, product B1:98733-98750) and builds q from that same pointer (B1:99299).
- **Contact.** B1 FUN_001ee708 has the same shape: its hook is FUN_001edf78, its damage FUN_001ee3e0, and its vehicle
  class is 4 (B1:100075, 100112-100123).

## 9. The TAT1 skiff by hand (HOVR's test numbers)

`tat_hover_skiff` (TAT1 `classes.ron`, decoded) over the BF2 defaults:
- SetAltitude 1, GravityScale 4, Acceleration 5, Deceleration 8, Traction 30, ForwardSpeed 16, ReverseSpeed 12,
  StrafeSpeed 4.
- SpinRate 1, TurnRate 1, PitchRate 1.5, TurnFilter 2.
- VelocitySpring 5, VelocityDamp 2.5, OmegaX 4 / 2.2, OmegaZ 6 / 2.2, LevelSpring 7, LevelDamp 1.
- TPA 0.01, SRA 0.01, BankAngle −0.01, BankFilter 3.
- Three AddSpringBody points: (−1.8, −0.7, 1.7, r 2), (1.8, −0.7, 1.7, r 2), (0, 0.3, −2, r 3), each
  BodySpringLength 1. BodyOmegaXSpringFactor 2 is on the third (the last-added point).
- Entry init (§10): OXSF −1, −1, 2 (2 set); OZSF −1, +1, 0.
- The skiff has springs, so §6.5 runs. At dt = 1/30 [D]:

**Rest height ≈ 1.94 m** (the body origin above flat ground; 1.98 m with NoRandomSpring).
- g in the drive = 1 − dt, because the spring tick sets 1 and then decays it (§2, T5). RAM shows 0.9667 on every
  touching hover (§2).
- Gravity is 9.8·(4 − 3·(29/30)) = 10.78 m/s².
- Balance: 5·Σc_jittered = 10.78, so Σ = 2.156.
- The pitch torques −c₁ − c₂ + 2c₃ = 0 and the roll torques c₂ − c₁ = 0 give equal c: jittered c = 0.719.
- Stored c = 0.719 / E[0.9 + 0.1u] = 0.757. Depth = 0.757 m.
- Both sphere bottoms sit 2.7 m below the origin (−0.7 − 2 = 0.3 − 3). Height = 2.7 − 0.757.
- The skiff's springs are lightly damped: damping acts only on the down-stroke, scaled by (1 − c). Expect a bob.

**Top speed 16.0 m/s** (ForwardSpeed; velocity matching has no drag).
- aS = Acceleration·g·dt = 5·(29/30)/30 = 0.161 m/s per tick, i.e. 4.83 m/s². 0 → 16 takes 3.31 s.
- Releasing the stick brakes at Deceleration·g = 7.73 m/s².
- BoostSpeed is 0, so there is no boost. The jump fields are 0, so there is no jump.

**Turn rate at speed 1.0 rad/s (57.3°/s).** SpinRate = TurnRate = 1, so the speed blend is flat.
- AI driver: the target is ±1.0. The slew of 2 rad/s² reaches it 0.5 s after full stick.
- Pad driver: target = 1.5 − 0.5·timer. ω meets it at 0.6 s (1.2 rad/s), then follows it down to 1.0 at 1.0 s.
- The carve keeps the velocity on the heading: |Δv| = 16·1/30 = 0.53 m/s per tick < lim 0.81. So the turning
  circle's radius is 16 m at 16 m/s.

**Numeric cross-check** (`lead_tmp/hvre/skiff_sim.py`: §4, §6.5, §6.7-6.16 and §7's hook on a plane, not the port):
- rest: mean 1.948 m over the last 5 s of 20 (1.886-2.020); NoRandomSpring 1.989 m;
- 16.0 m/s at 3.4 s;
- AI ω.yaw −1.0 at 0.5 s; heading rate 1.003 rad/s;
- pad ω peak 1.18 at 0.6 s, 1.0 from 1.0 s.

## 10. The VCLS addendum (re-vehicles §3.1), re-read — code in `game::vehicle_class` [V][O]

- The derive copy keeps the drive scalars. It sets VelocitySpring 15, VelocityDamp 5, OmegaX 3/1, OmegaZ 3/1 and the
  spring count 0. B1 FUN_001f00e0: B1:101409 copies +0x1788; B1:101527-101534 sets the rest. B2 FUN_00221e08:
  B2:136938-136946. It also copies +0xe76, +0xe90, the energy, jump and tread fields (B2:136935-136966).
- The entry init: VSF, VDF, OXDF, OZDF = 1; OXSF = −sign(z); OZSF = sign(x) (0 within ±0.1); length = radius × f.
  - B1 FUN_001f2030 (B1:102605-102645), f = DAT_00517828.
  - **B2 twin FUN_00224368** (B2:138140-138183), the same, f = DAT_005fcfdc.
  - This settles `vehicle_class.rs`'s "not opened line by line [H]".
  - **Correction (verifier):** f is 0.5 only by default. The GeometryName handler resets it to 0.5 (B1:101892;
    B2:137345, store 0x222d24). An unnamed prop, hash 0x77d6d149, reads it with `%f` (B1:102181, B2:137757; BF2's
    lui+addiu at 0x223414). No baked hover class authors 0x77d6d149, so `radius × 0.5` holds on the discs.
- **ODF-order quirk** [V]. The Body* modifiers write the LAST entry: base +0xe94 + 32·(count − 1). With no
  AddSpringBody yet, that base is +0xe74, so they land on the class globals:
  - BodySpringLength → VelocitySpring (+0xe78), BodyVelocitySpringFactor → +0xe7c, … BodyOmegaZDampFactor → the
    jitter flag byte +0xe90 (B2:137278).
  - No baked hover class puts a Body* line before its first AddSpringBody (bake scan, `lead_tmp/hvre/`), so
    `vehicle_class`'s "ignore when no point" matches the shipped data.
- B1 converts only PitchLimits from degrees (×0.017453294, B1:101859-101860). **B2 converts PitchLimits
  (B2:137310-137311) and BoostFOV (B2:137474).**
- CollisionThreshold / CollisionScale are three-%f reads (B1:101776, 101816).
- PilotType's value is hashed: none 0 (0xada7afdb), self 1 (0x645ba277), vehicle 2 (0x71cd882f), remote 3
  (0xc78d7953). Any other value returns without a write (B1:156006-156034).

## 11. For HOVR (what to build from this)

1. **Order and dt.** Per 1/30 tick: controller → drive (§6) → springs (§4) → contacts (§7). g reaches the drive as
   1 − dt when springs touched. Build E from q and the tilt exactly as §6.11-6.16 do, including the feedback. Every
   authored angle (TPA, SRA, BankAngle 0.01-ish) is tuned for it.
2. **Contacts** are the port's `CollisionWorld` sphere-vs-terrain/building queries on the spring spheres and the hull.
   - Depth along the normal feeds §7 K5.
   - Spring spheres: K5 (the push-out is depth − len).
   - The hull: K7-K9.
   - Impact damage (VDMG's `Sim::impact_vehicle`, no production caller yet) is dealt only for other vehicles and
     bottomed spring spheres (§0.5). A hull prim against terrain or a building deals none.
3. **Water float** (§4 S3): LAYR height + bob from the map's water. PROVISIONAL until built; the skiff's maps have no
   water.
4. BF2-only paths, driven by the class data: boost + energy (§6.3, §6.7, §6.10), the speeder-bike jump (§6.9), the
   tread scroll (§6.18).
5. The pad timer E+0x360 (§5.2) belongs to the driver's input state. FUN_003eef38's two bits are set only by the
   network deserializers, so offline every human takes the pad branch (§5.1 I7, §6.10). The move stick's 0.3 floor is
   the generic controller's (soldiers too, [H] whether the port has it).
6. **Spring branch first.** Every shipped hover has 3-6 spring points (§0.4). The no-spring branch (§6.4) can wait.
7. PROVISIONAL:
   - the contact pass's place in the frame;
   - the jitter's RNG;
   - the trample (§6.12);
   - the other body's damage factor;
   - masses (1.0).

## 12. Checks run, the verifier, what stayed [H]

Outputs are in `engine/artifacts/lead_tmp/hvre/`: `check1_lines.txt`, `hover_ram_out.txt`, `verify/`.

- **Check 1 (`re_check.py fn`).** Run on every B1/B2 line this doc cites (253 lines, `check1_lines.txt`). Seven cites
  were in the wrong function or were off by a line; they are fixed: FUN_00168e18's stores B2:7530/7546, FUN_001af508
  B2:55969-55977, the tick's end B2:133467, FUN_00169668 B2:7937-8054, the input gate B2:131964-131968, B1 PilotType
  B1:156006, and the B1 contact lines B1:100075/100112-100123. The verifier's five citation PARTLYs (S2, I5, D3, D15,
  D20) are folded into §4, §6.4 and §8.
- **Check 2 (absence).**
  - T3: no store to E's matrix, v or ω in the tick. Checked in the decomp and in the disassembly (any base register;
    only sp-relative hits).
  - K1/K9: a whole-ELF j/jal/data-word scan. FUN_0021fd98 is reached only by `jal` at 0x224b08. FUN_0021f978 is
    reached only at 0x21ff50 and 0x220050, both inside FUN_0021fd98.
  - C2: K+0x690/0x694 have no writer in the hover setter (text, and the ELF store scan by the verifier).
  - V3: one deg→rad pair in B1's setter; B2 has PitchLimits and BoostFOV.
  - Constants, by `widescan`: DAT_005fcfbc and DAT_005fcfc0 are read-only; DAT_005feda0 is read-only (0x62b1a8 is a
    data word). DAT_005fcfdc has two writers (§10).
  - Bake scan: no hover class authors 0x77d6d149 or spring-by-prim 0x577b739e, and none has a Body* line before its
    first AddSpringBody.
- **Check 3 (twins).**
  - B2 contact FUN_0021fd98 ↔ B1 FUN_001ee708; hook FUN_0021f508 ↔ FUN_001edf78; damage FUN_0021f978 ↔ FUN_001ee3e0;
    entry init FUN_00224368 ↔ FUN_001f2030; derive FUN_00221e08 ↔ FUN_001f00e0. Each B2 function's callers were
    listed.
  - B1's drive and filter differences are in §8.
- **Check 4 (bytes).** Every float literal the transcription uses was read from the code bytes
  (`lead_tmp/hvre/floatlits.py`). The .data words and vtable words were read with `re_check.py word`. The class-8
  argument was traced in the disassembly (§7.0).
- **Check 5 (defaults and writers).** Each flag's default, writers, and whether it is live by default:
  - DAT_00600548: .data 0, network writers only → offline 0.
  - K+0xe76: ctor 1, derive, FloatsOnWater. Live.
  - K+0xe90: ctor 1, derive, NoRandomSpring, and the §10 quirk. Live.
  - K+0xf90: ctor 0, setter. Live on the bikes only.
  - E+0x228 bit 3: the alive bit. Live while alive.
  - FUN_003eef38: 0 offline.
  - E+0x360: zeroed by the input reset FUN_002f4a50 (B2:283266). The hover ctor calls it (B2:130915), and so does the
    controller when the player is not driving (B2:234772). FUN_002b0548 ramps it.
  - PCSX2 RAM agrees for all of these in 30 BF2 states (§2).
- **Verifier.** One Opus agent, in the foreground, given `verify/claims.md` only (53 claims). Verdicts are in
  `verify/VERDICTS.md`: **46 SUPPORTED, 7 PARTLY, 0 NOT SUPPORTED.**
  - Five PARTLYs were citations (fixed).
  - Two were substance: the second writer of the spring-length factor (C4, V2), folded into §10.
  - Its refinements, also folded in: a second vtable holds the contact thunk (§7.0); the E+0x228 bit 3 writer list
    is not exhaustive; the jump ends at its blended limit even with the button held (as §6.9 reads).
  - It also read PCSX2 RAM. The lead reproduced its hover census and g values with its own script (`hover_ram.py`).
- **Stayed [H]:**
  - where the world contact pass runs in the frame;
  - the trample FUN_001c9460's effect;
  - the other body's damage factor (vtable +0x40) and the IsA early-outs in FUN_0021f978;
  - the terrain's collision class. It is [D] > 8: otherwise the springs could never compress on terrain;
  - which physical stick is pad[0..3];
  - FUN_003eef38's meaning (a profile field, [H] the input device);
  - that FUN_0053ac20 inverts FUN_0053a158 (the verifier read row 0 of both and found them consistent);
  - where FUN_002f74c0 sends E+0x4c0 (a vtable of *(E+0x32c), [H] sound);
  - the name of hash 0x77d6d149 (0x577b739e is `SetSpringBody`, §13.2).

## 13. Which of a hover's bodies meet the terrain and buildings — OPUS RE (2026-09-30, loop lane 3, card HHUL) - [O] UNAUDITED

**The answer [O].** On every shipped hover, in both eras:
- Only the spring spheres meet the terrain.
- Only the spring spheres meet buildings. The exception is the two BF2 fighter tanks, whose `p_cube1` box also meets
  buildings.
- The coll mesh and the geometry's other primitives meet neither.

There are two independent reasons:
1. **The spring lines replace both subsets.** Every `AddSpringBody` line names its new sphere in a `BuildingCollision`
   line and a `TerrainCollision` line. Those handlers overwrite the layer's subset with the bodies named since
   `GeometryName`. So after the first spring, the geometry's default membership is gone (§13.2).
2. **A mesh body is inert on the moving side.** It never collides as the moving object, against the terrain or anything
   else (§13.1).

**RAM check.** 18 of 18 live hover classes hold exactly these subsets: 9 BF1 (the TAT1 skiff included) and 9 BF2, 343
class sightings in 347 PCSX2 states (§13.4).

**The world side.** In a building, a hover's bodies meet that building's **vehicle** subset, not its soldier subset
(§13.3).

**For the port (§13.6):**
- `HoverParams::hull` is `p_cube1` on two classes, buildings only.
- The springs already meet the world, as they should.
- Two differences elsewhere become work: the world's vehicle subset (card VSUB), and the PS2's sphere-vs-terrain rule
  (the HHUL build).

Hops, in order:
- the per-tick passes: B1 FUN_00163008 / FUN_00163658; B2 FUN_00163450 / FUN_00163d08;
- the terrain collides: B1 FUN_00212530, B2 FUN_00256a18;
- the body narrowphases: FUN_00167910 / FUN_00168088; FUN_001677b8 / FUN_00167aa8;
- the pair tables: FUN_00167b40 / FUN_00168288;
- the base collides: FUN_00168a60 / FUN_00169328;
- the body vtables and the prim-vs-terrain tests: FUN_0016b7d8 / FUN_0016c6c0;
- the class parse: the hover setters FUN_001f0648 / FUN_002223a0, the generic parsers FUN_001e3678 / FUN_00214600, and
  the layer handlers FUN_001e4ee0, FUN_001e5070 / FUN_00215fb8, FUN_00216198;
- the entity ctors FUN_001e1e18 / FUN_00212b38.

Scripts and outputs are in `engine/artifacts/lead_tmp/hhul/`:
- `census.py`: the class parse replayed over the baked class DB;
- `ram_subsets.py`: the RAM check;
- `world_vehicle_subset.py`: the world census;
- `check1_b{1,2}.txt`;
- `verify/`: the claims and VERDICTS.

### 13.1 The chain [V][O]

| step | BF1 | BF2 |
|---|---|---|
| **pass: the vehicle list** (B1 layer 4, B2 layer 8) vs buildings + terrain | FUN_00163008: list +0x40 → FUN_00163240(mask 5), FUN_00163658(mask 8) (B1:2692-2693) | FUN_00163450: if DAT_005f87a0 == 0 (.data 0, no store in the ELF) → FUN_00163810(0xd), FUN_00163d08(+0x40, 0x10), FUN_00163d08(+0x54, 0x10) (B2:2785-2788) |
| each sub-step: the terrain, then each candidate | terrain slot +0x18 (B1:3327), candidate via the mover's slot +0x18 (B1:3352) | terrain slot +0x58 (B2:3764), candidate via the mover's slot +0x2c (B2:3796) |
| terrain collide | FUN_00212530 (word 0x533108) | FUN_00256a18 (word 0x61edd8) |
| its gate | … 0 only on KAM1 / BES1, re-terrain-enable) (B1:124902) | DAT_0060bed4 (.data 0; set to 1 by the terrain loader FUN_0055db58 on its INFO chunk, B2:694898) (B2:173277) |
| the mover's **terrain subset** | layer 0x40 (B1:124905-124907) | layer 0x80 → +0x120 (B2:173280-173282) |
| pair each body with terrain body 0 | FUN_00167b40 (B1:124909) | FUN_00168288 (B2:173285) |
| loop: per-body narrowphase → the mover's response → drop the pair | FUN_00167910 → slot +0x1c → FUN_00167ed0 (B1:124919-124925) | FUN_00168088 → slot +0x20 → FUN_001686a8 (B2:173295-173301) |
| per body: the body's own vtable slot +0x1c | B1:6353 (vtable at body+0x48) | B2:6743 (vtable at body+0x4c) |
| the hit body's index → contact byte +0x24 | B1:6357 | B2:6747 |
| building collide (the mover's slot) | FUN_00168a60 (hover vtable 0x531540, slot +0x18 = word 0x531558) | FUN_00169328 (hover vtable 0x61cc10, slot +0x2c = word 0x61cc3c) |
| subsets: self = the mover's subset for the OTHER's layer; other = the other's subset for the MOVER's layer | B1:7397-7399 | B2:7872-7874 |
| model narrowphase: the SELF body's vtable | model vtable 0x5267f0 slot +0xc = FUN_001677b8 → self body slot +0xc (B1:6316) | model vtable 0x60f248 slot +0xc = FUN_00167aa8 (B2:6412), used unless self is a soldier with model byte +2 == 0 |

**The coll mesh body is inert as the mover [V, ELF words].** The mesh body is `CollisionMesh`, vtable BF1 0x526528
(FUN_00160da0, B1:744) / BF2 0x60ef28 (FUN_00160cd8, B2:672).
- Its terrain slot +0x1c is BF1 0x161388 / BF2 0x1611a0. Both are `jr ra; daddu v0, zero, zero`.
- BF1: its pair slot +0xc is 0x1610a0, also a return-0.
- BF2: FUN_00167aa8 skips any self body whose byte +0x45 != 0 (B2:6516). The mesh ctor sets that byte to 1 (B2:674);
  the prim ctors set 0 (B2:294).
- A mesh does collide as the OTHER body: a mover's prim tests it through the mesh's slot +0x10 (B2 FUN_00160da8).
  That is how soldiers meet a hover's hull mesh (VHULL).

**Primitives.** The prim vtable is BF1 0x526558 (B1:309) / BF2 0x60ef58 (B2:314).
- Terrain slot +0x1c: BF1 → FUN_0016b7d8, BF2 → FUN_0016c6c0, on the shape at +0x50.
- BF2's prim-vs-mesh (FUN_00160da8 → FUN_0016b908 / FUN_0016bf10, B2:711-800) accepts kinds 3, 0, 4 and 2 (B2:9929).
  It has a separate box branch (B2:10266).
- Kinds: 0 sphere, 2 box, 3 `main_body`'s capsule-like shape, 4 cylinder. The file's DATA kinds 0/1/3 load as spheres,
  2 as a cylinder, 4 as a box (re-collision-layers §1).

**The per-body terrain callback** is the mover's slot +0x28 (B1) / +0x30 (B2). It is called for every body tested
(B1:6356/6360, B2:6746/6751). On the hover it is a no-op: BF1 0x531568 = 0x169218 and BF2 0x61cc40 = 0x169b98, both
`jr ra; nop`.

**The spring hook matches by that index.** FUN_0021fd98 passes the contact's byte +0x24 to FUN_0021f508 (B2:135278).
FUN_0021f508 compares it with each spring entry's body index K+0xe94+32i (B2:134883). BF1 does the same:
FUN_001ee708 → FUN_001edf78 (B1:100116, compare B1:99757).

**The hover's physics object uses the CLASS's model, not a copy.**
- BF2 FUN_00212b38 → FUN_00168e18(obj, layer, class+0x168, …) (B2:126388), stored at obj+0x60 (B2:7544).
- BF1 FUN_001e1e18 → FUN_001685d0(obj, layer, class+0x13e4, …) (B1:91383, stored B1:7107).
- The promotion fields that could change a collide's layers are never written by a hover setter, and are 0 in RAM on
  all 18 classes (§13.4):
  - BF1 model words +8/+0xc: FUN_00168a60, B1:7377-7378;
  - BF2 model bytes +2/+3: FUN_00169328.
  - Their writers are the soldier, flyer and other setters: B2 FUN_0020fd58, FUN_0022f6e8, FUN_00233ca8, FUN_00387060,
    FUN_001d1f58; B1 FUN_001df3f0.

**BF2 sub-steps** (FUN_00163d08):
- d = |position − previous position| (B2:3390-3402).
- If d > 100 m, the object is snapped with no collision (B2:3405-3412).
- Else it runs n = (int)(d/s + 1) steps, with s = min(½·slot +0x1c's value, the bounding radius) (B2:3418-3433).
- A hover moves well under s a tick, so n = 1 [D]. The port's one contact pass per tick is equivalent.

### 13.2 Which bodies are in the terrain and building subsets [V][O]

The class parse, in PROP order:
1. **`GeometryName`.** The hover setter first calls the generic parser: BF1 FUN_001f0648 → FUN_001e3678 (B1:101890);
   BF2 FUN_002223a0 → FUN_0026a340 → FUN_00214600 (B2:137343).
   - The generic parser adds the geometry's `%s%d` primitives with the add-body default (all five subsets; BF2 by
     MASK, 0 = all five).
   - Then it sets the six per-layer accumulators to 0:
     - BF1 B1:92633-92651: DAT_005177d4 terrain, DAT_005177d8 building, …
     - BF2 B2:127747-127766: DAT_005fcf6c terrain, DAT_005fcf70 building, ….
   - The hover setter then adds a `CollisionMesh` mesh body when the geometry has a coll mesh (B1:101897-101903;
     B2:137355-137357).
   - It adds a kind-3 `main_body` from the model's bounding box when the geometry had no primitives (B1:101920-101966;
     B2:137377-137422).
   - It puts the mesh in the soldier and ordnance subsets: BF1 **sets** them to {mesh} (B1:101983-101986); BF2 **ORs**
     it in (B2:137465-137468).
2. **`BuildingCollision` / `TerrainCollision` = X.**
   - A named body: accumulator |= bit(X), and that layer's subset := the accumulator.
     - BF1 FUN_001e4ee0, layer 8: B1:93382-93385.
     - BF1 FUN_001e5070, layer 0x40: B1:93450-93453.
     - BF2 FUN_00215fb8, 0x10: B2:128583-128590.
     - BF2 FUN_00216198, 0x80: B2:128669-128676.
   - `none` / `clear`: the accumulator and the subset are set to 0.
   - A name no body has writes nothing.
   - The only writers of the four accumulators are these handlers and step 1's reset (`re_check writers`).
3. **`AddSpringBody` = x y z r.**
   - BF2 B2:137597-137651: `%f %f %f %f` → a sphere (FUN_00169df8) named `snprintf("SpringBody%d", count+1)` (string
     0x61c9a0, B2:137620). It is a primitive body with no parent and MASK 0 (FUN_00160380, B2:137624), added to the
     class model (FUN_00167048, B2:137625).
   - Then 0x002233dc (B2:137561-137564) runs FUN_00224368 (the spring entry), **FUN_00215fb8(K, name)** and
     **FUN_00216198(K, name)**. Disassembly 0x2232bc-0x2233fc: s1 = the name buffer is a1 of both calls.
   - BF1 is the same: B1:102072-102130, sprintf at 0x531388 into s0; `jal 0x1e4ee0` at 0x1f14bc and `jal 0x1e5070` at
     0x1f14c8 with a1 = s0 (0x001f14c8, B1:102036-102037).
   - **`SetSpringBody` = X** (hash 0x577b739e = fnv("SetSpringBody"); a spring on an existing primitive) makes the same
     two calls with X (B2:137550-137564; B1:102024-102037).

**So [D, checked by §13.4].** After the first spring, both subsets are exactly the bodies named since `GeometryName`:
- terrain = the SpringBody spheres + the `SetSpringBody` / `TerrainCollision` names;
- building = the same with `BuildingCollision`.

Qualifiers:
- A `none` line empties its layer again.
- The `*CollisionPrim` keys bypass the accumulators and rewrite all six subsets directly (B1:93262-93327, B2:128420-128513).
  - Keys: `TerrainCollisionPrim` 0x7cdccf2a, `BuildingCollisionPrim` 0x8cf29543, and the Vehicle / Soldier / … variants.
  - They are reachable from the hover setters.
  - The only class on either disc with one is BF1 `com_item_vehiclerecharge`, a prop (verifier; reproduced).

A hover's `TerrainCollision` line also appends the body to a per-class list: BF2 K+0xcb0, count K+0xcc8
(B2:137889-137897); BF1 K+0x26c0 / +0x26d8 (B1:102300-102308).
- The list's only consumer is the tick FUN_0021ca80 (B2:133179-133282). It plays an effect at each listed body while
  moving.
- The class copy FUN_00221e08 copies it (B2:136923-136924). Not physics.

**Census** (`census.py` over the baked class DB: 22 hover-integrator classes, 12 BF1 with the TAT1 skiff, 10 BF2):
- No class authors `TerrainCollision`.
- `BuildingCollision` is authored only by BF2 `imp_hover_fightertank` / `rep_hover_fightertank`: `= p_cube1`, after
  their four springs.
- `p_cube1`:
  - a box, half-extents (1.511, 0.491, 0.956), at (0, 0.511, 1.559) in the vehicle frame;
  - identity rotation; parent `bone_root`, which sits at the origin;
  - it spans z 0.60–2.52 and y 0.02–1.00;
  - the front springs (±1.8, 1.25, 1.7, r 2.3) reach further forward, so the box meets a post or edge between them
    first.
- Every other class: terrain = building = its 3–6 springs.

### 13.3 What a hover meets in a building: the building's VEHICLE subset [V][O]

The building collide (§13.1) pairs the mover's subset for the building's layer with the building's subset for the
mover's layer:
- BF1: hover layer 4 vs building 8 → the hover's +0x9c (building) × the building's +0x98 (**vehicle**) subset.
- BF2: hover 8 vs building 0x10 → the hover's +0x118 (FUN_00168af0: 0x10 → +0x118) × the building's +0x110
  (4/8 → +0x110, **vehicle**) subset. Layer ids are from the collision-world lists in RAM
  (memory `ps2-collision-layers-in-ram`).
- The port's hover (and walker) contacts query the world's **soldier** subset (`sim/vehicles.rs` `WorldContacts`,
  "the port has no vehicle subset yet").

**World census** (`world_vehicle_subset.py`: the layer rule of `game::collision_layers` over the 32 baked maps' base
instances, common merged). 22,258 instance-bodies:
- **7,967 are soldier-only.** PS2 vehicles pass through them. FEL1 3,135 (the whisker / pitcher clumps' `p_-so-` prims);
  KAM1 1,052; UTA1 707; POL1 640; MYG1 638; COR1 618; TAN1 296; BF1 KAS1 211; GEO1 126.
- **1,477 are vehicle-only.** PS2 vehicles stop at them. FEL1 485; BF1 KAS1 279; MYG1 217 (sky_mountains walls);
  UTA1 131; BF1 NAB1 106 (`nab_prop_trees2`'s tree cylinders); BF1 KAS2 52; BF1 GEO1 28.
- 12,814 are in both.

### 13.4 PCSX2 RAM agrees [V][O]

`ram_subsets.py` over the 347 states of `pcsx2/ref/states`:
- It finds each hover class by fnv(name) at K+0x14.
- It reads the class model:
  - B2: K+0x168, count +0x104, bodies +4, u64 subsets building +0x118 / terrain +0x120;
  - B1: K+0x13e4, count +0x90, bodies +0x10, u32 subsets building +0x9c / terrain +0xa0.
- It compares them with `census.py`'s replay.

**18 classes, 343 sightings, 18 agree, 0 differ:**
- BF1: all_hover_combatspeeder, cis_hover_aat, cis_hover_stap, cis_tread_hailfire, imp_ and rep_hover_fightertank,
  imp_ and rep_hover_speederbike, tat_hover_skiff.
- BF2: all_hover_combatspeeder, cis_hover_aat, cis_hover_stap, cis_tread_hailfire, cis_tread_snailtank, imp_ and
  rep_hover_fightertank, imp_hover_speederbike, rep_hover_barcspeeder.

On every one:
- terrain = the SpringBody bodies; building = the SpringBody bodies, plus `p_cube1` on the two BF2 fighter tanks.
- The live body lists equal the replay's, name by name and in order, on all 18: prims, then the mesh, then the
  springs.
- The promotion fields are (0, 0).

Also seen (for VHULL / VSUB), in the BF2 classes with no collision lines (`all_hover_combatspeeder`,
`cis_tread_snailtank` and `rep_hover_barcspeeder` in RAM; `all_hover_bos_combatspeeder` by the replay):
- Their MASK-default soldier and vehicle subsets hold the spring spheres. PS2 soldiers are stopped by those invisible
  spheres.
- The `p_-vbt-` prims carry MASK bits for building and terrain, but the springs' lines drop them from those subsets.
  RAM confirms it.

### 13.5 How a body meets the terrain [V][O]

**Sphere (every spring).** BF2 FUN_0016c6c0 kind 0 (disassembly 0x16c708-0x16cd0c; B2:10686-10697, 10896-10955) and
BF1 FUN_0016b7d8 (B1:9800-9811, 10010-10065) do the same:
```
p = centre − (0, r, 0)                            (the lowest point; the contact point)
h = height(p.x, p.z)                              (B2 FUN_0055f258 / B1 FUN_00473db0: −300.0 when the cutter test says
                                                   the SURFACE point (x, h, z) is cut)
d = h − centre.y + r
contact only if 0 ≤ d ≤ 2r                        (c.lt.s d<0, c.lt.s 2r<d)
n = the terrain triangle's normal at (p.x, p.z)   (B2 FUN_0055f2b0 / B1 FUN_00473e08 = heightfield::normal_at)
depth = n·(0, d, 0) = d·n.y
```
The port's PROVISIONAL rule (`sim/vehicles.rs` `WorldContacts`, HCON) differs:
- The port uses depth = r − (c.y − h)·n.y when |(c.y − h)·n.y| < r, and tests the cut at the centre.
- The PS2 gives depth = r·n.y − (c.y − h)·n.y: shallower by r(1 − n.y). That is 0.12 m on 20° and 0.27 m on 30° for an
  r 2 spring.
- The window is vertical: h within r of the centre's height.
- The cut is tested at the surface point under the centre.
- On flat ground the two agree.

**Box** (no shipped hover has one in its terrain subset). The lowest corner is sampled:
- Each half-extent is signed against its axis's world y; an axis with |y| ≤ 1e-4 adds 0.
- d = h − corner.y, window [0, 2·(centre.y − corner.y)], depth d·n.y.

**Cylinder** (none either). It samples the lowest point of the down-facing cap's rim, then the same point on the other
cap, then the opposite rim point.
- The first two are accepted within 4v, v = √((r·right.y)² + (½h·up.y)² + (r·fwd.y)²).
- The final window is [0, 2v]. With |up·n| ≈ 1 the point moves to the cap centre.
- B2:10698-10895 / B1:9812-10009 (read, not re-derived from the disassembly).

### 13.6 For the port

1. **HHUL build.**
   - `HoverParams::hull` becomes a list of hull parts. Each part has a shape (sphere / box / cylinder), a local pose, and
     flags for terrain and building.
   - Its content is the replay of §13.2 over the class's ordered PROP list: the named non-spring bodies, resolved
     against the geometry's baked prims at their XFRM. `p_cube1`'s parent `bone_root` sits at the origin; other PRNT
     bones stay identity, PROVISIONAL as in VHULL.
   - On the discs that is `p_cube1`, buildings only, on the two BF2 fighter tanks.
   - The hull meets buildings through K7-K9 (§7): no damage; the K9 push is full against 0x10 and 0x80 (FUN_00169668,
     B2:7935-8054).
   - The springs' terrain contact becomes §13.5's sphere rule. The box/cylinder rules are built only if a class needs
     them.
   - The box-vs-building narrowphase: FUN_0016bf10's box branch is not transcribed. parry's exact box contact stands in,
     PROVISIONAL.
2. **VSUB (new card).**
   - Build the VEHICLE subset into the runtime world. `Layer::Vehicle` already exists in `collision_layers`;
     `Layers::VEHICLE` is reserved.
   - Point `WorldContacts` and `WalkerWorld` at it. For walkers this is [D]: they reach buildings through the same
     collide.
   - Soldier queries are untouched.
   - Add the hull side: the spring spheres in the soldier / vehicle subsets of the four BF2 classes above (for VHULL and
     a later K4).
3. **Unchanged, now [V].** A hover nose-first into a wall is stopped by its spring spheres.
   - A spring sphere pressed less than BodySpringLength into a wall gets no response. The hook needs n·up > 0.5 to
     compress, and K5 returns 0.
   - Past that, it is pushed out by depth − len (§7 K5).
   - The port already does this.

### 13.7 Checks run, the verifier, what stayed [H]

- **Check 1 (`re_check.py fn`).** Every cited line, 97 B2 + 68 B1 (`check1_b{1,2}.txt`); all in the functions named.
  The first draft cited a vtable store one line off in each era (B1:742 → 744, B1:301 → 309) and the GeometryName
  branch at B2:127744 (→ 127747); these are fixed.
- **Check 2 (absence).**
  - The mesh slots: disassembly of every return-0 slot.
  - The promotion-field writers: `re_check absent` on `0x16a) =`, `0x16b) =`, `0x13ec) =`, `0x13f0) =`; no hover setter
    writes them.
  - The accumulators' writers: `re_check writers`.
  - `*CollisionPrim` / `TerrainCollision` on hover classes: a class-DB scan.
  - The runtime result: the RAM check.
- **Check 3 (twins).** Each B2 function above against its B1 twin. Callers:
  - FUN_00215fb8 / FUN_00216198 are called only by FUN_00214600 and FUN_002223a0.
  - FUN_001e4ee0 / FUN_001e5070 are called only by FUN_001e3678 and FUN_001f0648.
  - FUN_00168088 is called only by FUN_00256a18.
  - FUN_0021f508 is called only by FUN_0021fd98.
- **Check 4 (bytes).**
  - The vtable words: 0x526528/0x526558/0x5267f0/0x533108/0x531540 and 0x60ef28/0x60ef58/0x60f248/0x61edd8/0x61cc10.
  - The strings `SpringBody%d`, `%f %f %f %f`, `CollisionMesh`, `main_body` in both ELFs.
  - FUN_0016c6c0's 0.5 (0x16c708), 2r (0x16cc2c) and 1e-4 (0x16cc8c).
  - The −300 sentinel 0xc3960000.
- **Check 5 (flags).**
  - DAT_005229a8: .data 1, `TerrainEnable`.
  - DAT_0060bed4: .data 0, set 1 by the terrain loader, cleared by FUN_0055eb50.
  - DAT_005f87a0: .data 0, no store.
  - The promotion fields: ctor 0 (FUN_00166e28 B2:5824-5827), no hover writer, RAM 0.
- **Verifier.** One Opus agent in the foreground, given `verify/claims.md` only (17 claims). Result: **15 SUPPORTED,
  2 PARTLY, 0 NOT SUPPORTED** (`verify/VERDICTS.md`).
  - Folded in:
    - the `*CollisionPrim` qualifier and the per-layer wording (C8);
    - the class copy that also reads the TerrainCollision list (C16);
    - the RAM run now covers the TAT states and records the promotion fields itself (C9);
    - B1:99757 for the BF1 hook compare.
  - It also checked in RAM that every live hover collision object points at the class model with the vtable and layer
    above.
- **Stayed [H]:**
  - FUN_0016bf10's box-vs-triangle narrowphase (not transcribed; parry stands in);
  - the cylinder / box terrain rules were not checked instruction by instruction (no shipped hover uses them);
  - the walkers' building layer (VSUB confirms);
  - where the contact pass sits in the frame (unchanged, §2).
