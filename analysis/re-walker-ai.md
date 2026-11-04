# OPUS RE (loop lane 3, 2026-10-01, card WAI) - [O] UNAUDITED: no Fable pass
# RE: how a bot drives a walker — BF2's walker AI, the shared steering FUN_002a54d8, and the sticks it writes

**The question** (card WAI). `re-walker.md` transcribed the walker for a human driver and left the AI's side [H]: what
`FUN_00262248` and the steering `FUN_002a54d8` write into the sticks, why §4.5 C's factor is negative on BF2 classes, and
the boost mode 3. All three are answered here, and the answer is checked tick by tick against PS2 RAM (§10).

**Method.** Every function below was read in the B2 decomp and settled in the disassembly
(`scripts/tools/eedis.py`; dumps `lead_tmp/wai/dis_b2_*.txt`). Class ids were named from the vtables' own
getters (§1). Two live PCSX2 runs (software renderer) sampled every AI walker each game tick with the AI's move order
beside the sticks (`lead_tmp/wai/wai_sample.py`), and `wai_check.py` replays the transcription against them.

Hops: the controllable's slot +0xcc thunk FUN_00267038 → FUN_00262248 → FUN_002a54d8 (FUN_002a63e0, FUN_002a6138,
FUN_002ba8d8) → FUN_002a45a0 / FUN_002a50c0 (FUN_002a4400, FUN_002a4498, FUN_002a4cc8); the goal: FUN_002f1d80 →
FUN_002a5fa8 → FUN_002ba7e0; arrival FUN_002f1140 / FUN_002f1320 / FUN_002a6318; the yield FUN_0025c2b0 → FUN_002a8c30;
the bail FUN_002a95a8; the speed mode FUN_002ba970 ← FUN_002cf458 / FUN_002bf5d8; BF1's FUN_0021c180 → FUN_00233500.

**Conventions.** As `re-walker.md`: E = the walker, K = its class (E+0x4b4), ctl = E+0x270, sticks E+0x2dc throttle (p2),
E+0x2e0 strafe (p3), E+0x2e4 turn (p4), E+0x2e8 pitch. M = the body matrix, rows **X** (+0x00), up (+0x10), fwd (+0x20),
pos (+0x30); in RAM X = up × fwd on every walker read, so X is the vehicle's LEFT in the port's terms
(`ai::ride::steer_to`'s `right` = fwd × up = −X). `ai` = the UnitController at E+0x324 (= ctl+0xb4), `mv` = ai+0x430, its
mover. dt = the frame time. Tags as in `re-walker.md`; [O] = load-bearing and verifier-SUPPORTED (§12).

## 0. The answers [O]

| question | answer |
|---|---|
| which stick steers a bot's walker | **The left stick's X, E+0x2e0 (p3)** — the stick §4.5 C reads. The AI's own "turn" output lands there (§2 A9). E+0x2e4 / E+0x2e8 carry only the aim. |
| the law | Outside 4 m of its goal a walker **turns in place until the goal is within 5°** (throttle 0), then walks with throttle = cos(error) and p3 = the same turn command (§3 B9). A class with flag bit 0 (BF2's AT-STs, one-man AT-ST, tauntaun) instead takes a **strafe branch inside 30°**: throttle 1 and p3 = −tan(error)/0.4, saturating at ±1 past 21.8° with the throttle falling to 0.4/tan(error) (§2 A8, §3 B8). |
| why C's factor is negative | It is, and the PS2 lives with it. In the strafe branch the AI negates its strafe (E+0x2e0 = −b), so the negative factor turns the walker TOWARD the goal. In the plain branch nothing compensates: a **BF2 spider, AT-TE or AT-AT turns AWAY from its goal while it walks**, crosses 5°, stops, turns back, and so crabs along ~5° off its goal with the throttle toggling (§7; live: median error 5.3°, 2996 of 2996 ticks replayed). |
| the sticks' range | Written raw, not shaped like a pad: (0.997, 1.0) on a walking spider, so \|stick\| reaches 1.41 and the clip clock runs ×1.38 (§7). |
| boost mode 3 | `mv+0xa0` (= ai+0x4d0), the unit's speed mode. 3 is set by the combat helper's state 6 for hover / walker units (a 5 s dash 20 m ahead) and by the follow navigator when over 1 m from its trail point (§8). Never seen in 220 s of AI walkers; the entries a walker can take are traced only as far as §8 says [H]. |
| other rules | A bot stops for **7.5 s** after its walker touches a hover / walker it should yield to (§8). Under **20 % health** the AI tells every AI occupant to get out — for 26 of 101 controllers; the rest ride it down (§8). |

## 1. Objects and fields [V][O]

- **Class ids** are globals filled at static init with a name hash (`FUN_005393c0(0x689960, "EntityWalker")`, B2:184898),
  so `re_check.py writers` shows none. Each entity vtable holds its id getter at slot +0xc and a name getter at +0x10:
  … "EntityWalker" at 0x61f1c8), DAT_006894f8 = EntityHover,
  DAT_00689300 = EntityDroideka, DAT_00689368 = EntityFlyer, DAT_0068c8f8 = MountedTurret, DAT_00689828 = EntitySoldier,
  DAT_00688f88 = EntityCarrier, DAT_00698110 = UnitController. (This also names `re-walker.md` §9's three [H] ids: C1
  ignores an EntityCarrier, C4 wants an EntitySoldier, C7's bit-6 damage wants an EntityFlyer.)
- **FUN_002a6138(entity)** = the unit kind: soldier 0, droid 1, **walker 2**, droideka 3, flyer 5, hover 6, turret /
  passenger slot 7, armed building 8, building 9, item 10, remote terminal 11 (0x2a6138-0x2a62d8). The commandwalker's
  IsA chains through EntityWalker's, so AT-AT / AT-TE are kind 2 as well (verifier).
- **FUN_002b6ad0(x) / FUN_002b6b20(x)**: a checked cast to UnitController — returns x itself (0x2b6ad0-0x2b6b18).
- **The mover mv = ai+0x430** (ctor FUN_002ba3c8, B2:241439):

| field | what |
|---|---|
| mv+0x1c | the path holder; `*(holder+0x10)` = the active path follower `m`, 0 = none (FUN_002ba8d8, 0x2ba8d8) |
| mv+0x20 | **the move order ("goal")**, 0x50 bytes: byte +0 valid; +0x10 the point; +0x20 the path node; +0x30 the segment direction; +0x40 a word; +0x44 = −1.0; +0x48 = 1.0. Set only by FUN_002ba7e0 (B2:241591); cleared (byte only) by FUN_002ba510 / FUN_002ba720 |
| mv+0x70 | the aim order: type (0 a target entity, 1 a point, 2 none), +0x10 the point, +0x20 a miss factor, +0x24 / +0x28 the target handle (FUN_002ba9a8 / FUN_002ba9c8 / FUN_002ba9e0) |
| mv+0xa0 | the speed mode (= ai+0x4d0): 0 normal, 3 boost, 4 quarters the arrival radius (FUN_002ba970, B2:241719) |
| mv+0xa8 | flags (u64 at ai+0x4d8): bit 2 / 3 leave / change seat requests (FUN_002bbf00), bit 5 "stand" (FUN_002ba810), bit 11 doubles the turn clamp (FUN_002bbde0) |
| ai+0x5a0 | a per-controller random integer 0..100, drawn once in the ctor (FUN_002b6c20 B2:238826-238827) |

## 2. The walker AI — FUN_00262248(dt, E) (B2:181441-181648; 0x262248-0x262668) [O]

Reached only through the thunk FUN_00267038 (B2:184659), slot +0xcc of the walker controllable vtable 0x61f660 and of
CommandWalker's 0x62bc50. A per-entity task (vtable 0x61fa00, update FUN_001eab08) calls that slot; where it is filed is
not found [H], but the order within a frame is measured (§10): **the mover sets the goal, the AI writes the sticks from
the body matrix of the tick before, then the locomotion turns the body.**
*(decompiler excerpt omitted from the public copy)*
K+0x748 MaxSpeed, K+0x75c MaxTurnSpeed (setter hash 0x5433f589, B2:183316), K+0x760 MaxYawSpeed, K+0x76c MaxPitchSpeed.
- **K+0xe18 bit 0** (hash 0x55bc9af9, name not cracked) is tested nowhere else in the ELF (0x262420; the derive copies
  it at 0x263b6c). Default 0. On the discs it is authored by five BF2 classes only: imp_walk_atst (+_jungle, _snow),
  rep_walk_oneman_atst, all_walk_tauntaun. RAM: flags …9b on the AT-ST and tauntaun, …9a on the spider.
- **A8 with an invalid goal [D]** (verifier): the 30° test uses the stale point, and the steering then writes nothing, so
  the branch stores E+0x2dc = max(old, 0) and E+0x2e0 = −old, a sign flip each tick. Seen only on the first tick after
  an AI attaches.

## 3. The steering — FUN_002a54d8 (B2:226596-226844; 0x2a54d8-0x2a5b50) [O]

`(dt, fwdSpeed, strafeSpeed, turnRate, pitchRate, ctl, goal, aim, &throttle, &strafe, &turn, &pitch)`. Seven callers:
the walker's two, the hover's FUN_00220528, and FUN_001f02e0, FUN_001fa4b0, FUN_0020b840, FUN_00249f80.
*(decompiler excerpt omitted from the public copy)*
cross.y = dir·X (X = up × fwd), so b > 0 when the goal lies toward −X.

## 4. A stick toward a point — FUN_002a45a0 (B2:225893-226210; 0x2a45a0-0x2a4cc8) [O]

`(dt, yawRate, pitchRate, ctl, &target, &turn, &pitch, flag)`.
- **Yaw, walkers** (every entity that is not a flyer, a hover, or a turret with yawRate > 0.5; 0x2a4834-0x2a4954):
  ```
  d = target − M.pos   (flag: − ctl+0xd0, the aim origin);   n = normalize(d − up·(up·d))
  f = M.fwd            (flag: ctl+0xe0, the aim direction)
  turn = clamp(−up·(f × n) / (yawRate·dt), ±lim)     lim = 2 with mv+0xa8 bit 11, else 1; no division when yawRate·dt ≤ 0
  ```
  up·(f × n) = n·X, so **turn = −(n·X)/(yawRate·dt)**: +1 when the goal is more than asin(yawRate·dt) toward −X. It is a
  deadbeat command — it asks for exactly the remaining angle if the body turned at yawRate.
- **Yaw, hover / flyer / fast turret** (0x2a46d4-0x2a4830): t = −l.x of the target's direction in the body frame (±1 by
  its sign when l.z < 0); u = clamp(t·(4.5 − clamp(yawRate, 1, 3)), ±1); turn = u·|u|. (The hover bot's stick — not the
  port's `steer_to`.)
- **Pitch, walkers** (0x2a4954-0x2a4c98): in the aim frame (aim direction ctl+0xe0, origin ctl+0xd0, world up), with da =
  the target offset with its sideways part removed: 0 when da·aim ≤ 0, else
  clamp(−right·(aim × normalize(da)) / (pitchRate·dt), ±1).

## 5. The aim point — FUN_002a50c0 (B2:226418-226595; 0x2a50c0-0x2a54d8) [O] except the error vector [H]

```
type 0 (target entity T, alive handle): P = FUN_002a4498(dt, ctl, T) (a lead) when the unit has a weapon; else M.pos + M.fwd·30
type 1 (point): P = the order's point;  miss = the order's float
type 2 (none):  goal.valid and |goal.pos − M.pos|_xz > R:  P = M.pos + normalize(goal.pos − M.pos)·30
                else:                                       P = M.pos + M.fwd·30
P += off·k   (off = FUN_002a4cc8's aim-error vector when a target object exists, or the controller's own with miss > 0;
              k from the distance; not transcribed [H])
FUN_002a45a0(dt, yawRate, pitchRate, ctl, &P, &turn, &pitch, flag)
```
So with no target a walker's turret looks along its travel, and a STOPPED walker's body stick (flag 0, §3's tail) is 0
with no target and the turn toward the target with one.

## 6. The goal — FUN_002f1d80 → FUN_002a5fa8; arrival [O]

- The move order is set by the mover's steering FUN_002f1d80 (B2:281390, BF1's FUN_0026aba8 twin) and by "stand"
  FUN_002ba810 (the unit's own position; bit 5 of mv+0xa8).
- **Look-ahead.** For an EntityWalker the point is … a
  hover passes |velocity| (B2:281227-281243). FUN_002a5fa8(d, a, b, pos) (0x2a5fa8-0x2a60e4): with A, B = the segment's
  ends at the unit's height and u its direction, L = u·(pos − A) + d + 0.2; the point is A + u·L, or B once L passes the
  segment's end. RAM: both tauntauns' goals sat 19.476 m ahead = 20.2 less one tick's 0.724 m.
- **Arrival** (FUN_002f1140, B2:280564): |unit − node|_xz < r, r = FUN_002a63e0 at the path's last node (**4.0 m** for a
  walker) and FUN_002a6318 at the others (**1.0 m**; 0x2a6318), × 0.25 in speed mode 4; or the node's plane passed
  (FUN_002f1320, B2:280653).

## 7. What the law does on the shipped classes [D], checked live (§10)

Port terms: s = n·right (right = fwd × up; s > 0 = the goal is to the right), c = n·fwd. Then n·X = −s.
- **Plain branch** (bit 0 clear, or the goal outside 30°): p3 = clamp(s/(MaxTurnSpeed·dt), ±1); p2 = 0 until c ≥ cos 5°,
  then p2 = c. In place §4.5 C gives cmd = p3·step: the body turns toward the goal at 2·StoppedTurnSpeed.
- **Strafe branch** (bit 0, c > cos 30°): |s| < 0.4·c → p2 = 1, p3 = −s/(0.4·c); else p2 = 0.4·c/|s|, p3 = −sign(s).
  Walking, C multiplies by (|p3|/|p2| − e08)/(e0c − e08) ≈ −2 (e08 / e0c = tan 87° / tan 88°), so cmd ≈ +2·(s/0.4c)·step,
  clamped to ±step: toward the goal, at the full rate once the error passes ~11°.
- **Plain branch, walking, on a class that authors e08 / e0c** (BF2 spider, AT-TE: 87 / 88; AT-AT: 50 / 67.5): the same
  negative factor meets an un-negated p3, so the body turns away from the goal at 2·ForwardTurnSpeed until c < cos 5°;
  then p2 = 0 and it turns back at 2·StoppedTurnSpeed. A two-state cycle around 5°, p3 pinned at ±1.
- **A class that authors neither** (every BF1-disc walker, BF2's kaadu; ctor e08 = e0c = 0, bits 1-3, bit 0 clear): no
  factor; plain branch; the walk converges on the goal.
- **Raw sticks.** (p2, p3) = (0.997, 1.0) gives |stick| = 1.41 in §4.8's clock: a walking spider's clip runs
  3·1.41/3.075 = ×1.38 on its walking ticks and ×1 on its turning ticks.

## 8. Boost, yield, bail-out

- **Speed mode 3 → boost** (`re-walker.md` §4.2: an AI boosts when ai+0x4d0 == 3). FUN_002ba970(mv, x) stores mv+0xa0
  [O]. The literal 3 is passed at two sites (verifier): FUN_002cf458 (B2:256978), the combat helper's state 6, only for
  hover and walker units — its enter code sets a destination 20 m ahead and a 5 s deadline; and FUN_002bf5d8
  (B2:245300-245303), the follow navigator, for any unit over 1 m from its trail point. Four more sites pass a variable.
  State 6 is entered by order type 2 (FUN_002ce648, soldier-only as far as traced) and by two messages in FUN_002cff60,
  one of them only for hover / walker with a clear ray 10 m ahead. **Which game situations send those is not traced [H].**
- **Yield** [O]. FUN_0025c2b0 stamps E+0x51c = the mission time when the walker touches a class-8 body and
  … only the ctor (−100.0) and that site store it. FUN_002a8c30
  (0x2a8c30-0x2a8e88): the other is a walker or hover; false if self's radius > 3× the other's, true if the other's >
  3× self's; else both must be moving, and it is true iff (−n)·ds < n·do (n from self to other; ds, do the unit
  velocities) — the one running into the other's back yields. For 7.5 s the AI's goal is its own position: it stops and
  turns to its aim. Not exercised in either capture (E+0x51c = −100 throughout).
- **Bail-out** [O]. Under 20 % health, when (ai+0x5a0 & 3) == 0 — 26 of the 101 values — FUN_002a95a8(E) sends every AI
  controller on the vehicle's list (E+0x240) a type-0x14 order (get out) and the AI returns without steering. The hover's
  AI FUN_00220528 opens with the same test (B2:135604-135610). Live: an AT-ST with ai+0x5a0 = 83 kept driving from
  17.7 % health to its death.

## 9. BF1 [V]

B1 FUN_0021c180 (B1:132081-132217) is the plain branch only: the same 7.5 s yield (B1:132130-132137), one
FUN_00233500(dt, MaxSpeed, 0, MaxTurnSpeed, 0, …) with its turn output in the left stick's X (E+0x304), one aim call. No
class flag, no health test. B1 FUN_00233500 has the same 5° rule (B1:148340-148354). BF1's locomotion has no e08 factor
(`re-walker.md` §13), so a BF1 walker on its own disc turns in place to 5° and then converges.

## 10. The PS2 itself [O]

`lead_tmp/wai/wai_run.sh <state> <s> <tag>` (SW renderer), `wai_sample.py` (per tick: the last read before the game
clock moves, so every update of the tick has run), `wai_check.py` (the replay). Each tick's sticks are predicted from
the matrix of the tick before and the move order of the tick, then the yaw step from those sticks by §4.5 C.

| run | walker | ticks replayed | both sticks equal (2e-3) | yaw step equal (1e-3 rad) | branches |
|---|---|---|---|---|---|
| BF2 GEO1, 100 s | cis_walk_spider ×2 | 5992 | 5992 | 5992 | turn 2383, walk 3609 |
| BF2 HOT1, 120 s | imp_walk_atst_snow | 1053 | 1053 | 1053 | strafe 1025, turn 14, stop 14 |
| | all_walk_tauntaun ×3 | 459 | 459 | 459 | strafe 347, stop 100, turn 12 |

Not replayed: 1040 AT-ST ticks stopped with an aim order of type 0 / 1 (the body stick then follows §5's error vector).
- **Spider** (plain branch, e08 authored): heading error to a goal over 4 m away: median 5.31°, p10 4.15°; p3 = ±1 on
  every tick; throttle 0.996-0.998 or 0; per tick the body turns 0.01333 rad away from the goal while walking
  (2·0.2/30) and 0.02 rad toward it while standing (2·0.3/30). Root speed (E+0x4c0) in the walk clip: mean 3.63 m/s
  against MaxSpeed 3.
- **AT-ST**: error median 0.05°, p90 3.4°; 5.09 m/s in the walk clip. **Tauntaun**: error median 0.01-0.26°;
  19.0-19.6 m/s. A rider holds "stand" (mv+0xa8 bit 5) for 30 ticks after boarding.
- The 2026-09-29 audit's 60 s AT-ST capture agrees: 1000 ticks (1, |p3| < 1), 18 ticks (0.694-0.986, ±1) — the strafe
  branch's two cases, 0.693 = 0.4/tan 30° — and none between.
- Speed mode 0 and boost latch 0 on every row; energy never moved.

## 11. For the build

1. **Seat.** `ai::ride::claim_seat` admits a walker class's pilot seat.
2. **The driver's sticks** (§7's port terms; R = 4.0 m; dt = the tick): the plain and strafe branches, chosen per tick
   by the class's bit 0 and the 30° test; stopped (no goal, or inside R on the final approach with no route left):
   p2 = 0 and p3 = the plain turn toward the bot's target, 0 without one. They go to the walker **unshaped and
   unquantized** (`HoverInput { human: false }`; not through `sim::seats::driver_input`'s floor and unit circle, and not
   through the pad's i8) — the spider needs (0.997, 1.0).
3. **Route.** Arrival 1.0 m at a node (or its plane passed), 4.0 m at the last. The PS2 aims 20.2 m ahead along the leg;
   the port's straight-at-the-node goal is OURS, as for the hover.
4. **Aim and fire** stay the port's (`ai::gunner`). PS2: deadbeat sticks toward §5's point at MaxYawSpeed / MaxPitchSpeed.
5. **Not built by this card:** boost (mode 3's senders [H]); the 7.5 s yield (the port has no walker-vehicle contact,
   `re-walker.md` §9 C6); the 20 % bail-out (the hover has the same rule — one card for both); A8's sign flip with no goal.
6. **Checks.**
   - `lead_tmp/wai/ps2_rows.csv`: 145 PS2 rows (inputs and the sticks written), every branch; the port's stick function
     must reproduce each to 2e-3.
   - Flat-ground runs: a BF2 AT-ST and tauntaun converge under 1° and walk 5.0 / ~20 m/s; a BF2 spider holds 4-6.5° with
     the throttle toggling and a root speed of 3.3-3.9 m/s; a BF1-disc walker converges.
   - The sweep counts walker boards and metres driven on BF2 HOT1 / GEO1 and BF1 NAB1 / HOT1.

## 12. Checks run, the verifier, what stayed [H]

Outputs in `engine/artifacts/lead_tmp/wai/`: `check1_lines.txt`, `checks.txt`, `check_out.txt`, `verify/`.
- **Check 1** (`re_check.py fn`): 106 B2 and 9 B1 lines; each lies in the function its section names, bar B1:148422
  (the next function), which is not cited.
- **Check 2 / 5** (absence, writers): K+0xe18 bit 0 — every load of +0xe18 in .text decoded, one unshifted test
  (0x262420) and the derive's copy; E+0x51c — two stores in 0x259970-0x2678c8 (0x25a360, 0x25c5ec); ai+0x5a0 — one
  non-stack store in 0x2a0000-0x300000 (0x2b7e54); the hashes 0x55bc9af9 and 0xd128c668 (K+0xe14) as PROP chunks in every
  LVL of both discs: five BF2 classes, and none (so C's aim term is dead everywhere).
- **Check 3** (twins): B1 FUN_0021c180 and FUN_00233500 opened (§9); FUN_002a54d8's seven callers listed; the
  commandwalker shares the thunk.
- **Check 4** (bytes): the constants are immediates, read in the disassembly: 0.2 (0x3e4ccccd), 7.5 (0x40f00000),
  0.8660254 (0x3f5db3d7), 0.4 (0x3ecccccd), 0.0001, 0.04, 0.0872665, 4.0, 0.5, 20.0, 30.0.
- **PS2**: §10; class fields read from the states' RAM (MaxTurnSpeed 1.5 / 2.0 / 0.6, e08 / e0c 19.0812 / 28.6364,
  K+0xe14 = 0, flag words).
- **To re-run:**
  ```
  python3 scripts/re_check.py callers b2 FUN_002a54d8        # 7 calls, two in FUN_00262248
  python3 scripts/re_check.py callers b2 FUN_00262248        # 1: B2:184659 in FUN_00267038
  python3 scripts/tools/refs.py b2 0x267038   # data ['0x61f72c', '0x62bd1c']
  python3 scripts/re_check.py callers b2 FUN_002ba7e0        # 2: FUN_002ba810, FUN_002f1d80
  python3 scripts/re_check.py callers b2 FUN_002b7e50        # 1: B2:238827 in FUN_002b6c20
  python3 scripts/tools/eedis.py bf2 0x262248 0x262668
  cd engine/artifacts/lead_tmp/wai && S=../../pcsx2/ref/states
  python3 wai_check.py live_geo1.csv $S/bf2_geo1_cp1_cp7_postA_spawn.p2s
      ticks replayed 2996: throttle equal 2996 (100.0 %), strafe equal 2996 (100.0 %), both 2996 (100.0 %)   (×2)
      yaw step from the sticks (4.5 C): equal within 1e-3 rad on 2996 / 2996
  python3 wai_check.py live_hot1.csv $S/bf2_hot1_cp1_cp6_postA_spawn.p2s
      ticks replayed 1053 … both 1053 (100.0 %);  69 … 69;  184 … 184;  206 … 206
  ```
- **Verifier**: one Opus agent, foreground, `verify/claims.md` only (39 claims) → `verify/VERDICTS.md`:
  **37 SUPPORTED, 2 PARTLY, 0 NOT SUPPORTED.** The PARTLYs are both in §8's boost (a second literal-3 site; state 6's
  other entries) and are folded in, as are its refinements: A8 is not gated on goal.valid, the table cosine, mode 4's
  quarter radius, CommandWalker's vtable, the 26-of-101 draw, the stopped walker's body stick.
- **Stayed [H]:** who sends the messages that start state 6, and whether a follow navigator ever drives a walker (so:
  when a PS2 bot boosts a walker); §5's aim-error vector and lead; where the AI task is filed (the order is measured);
  the yield and the bail-out in RAM (read, never observed); the name of hash 0x55bc9af9; FUN_002f1140's node-flag 0x100
  side effect; the hover's own AI FUN_00220528 beyond its call of this steering.

## 13. Who pilots what, and how they get in — live, 2026-10-02 (card WAI2) [O]

Measured, not read. Tools and raw rows: `engine/artifacts/lead_tmp/wai2/ps2/` (`load_run.sh` / `load_run1.sh` boot from the
Instant Action menu states on the SW renderer with `load_sample.py` / `load_sample1.py` attached, so every walker and
soldier is sampled at ~8 Hz from the level's first tick; `load_an.py`, `load_an1.py`, `long_an.py` print the tables below;
`st2.py`, `st2b.py`, `st1.py` read the same from a save state). Mission time is the AI's (B2 FUN_004240f8, B1
FUN_0033e4f0). One run per row unless said. No verifier pass: one decomp citation (`re_check.py fn b2 354305 175464` →
FUN_0035baa0, FUN_00259970), the rest is RAM.

**The command walker is in the walker list.** BF2's AT-AT and AT-TE are `FUN_0035baa0`'s class (B2:354285-354310): it
calls EntityWalker's constructor `FUN_00259970`, then stores vtable 0x62be40 at +0 and 0x62bde0 at +0xcc. WAI's sampler
(§10) kept only +0xcc = 0x61f7f0, so it never saw them. Class names by the hash at K+0x14: 0x1ab5640 `imp_walk_atat`,
0x1a30170 `rep_walk_atte`, 0x1ae1490 `imp_walk_atst_snow`, 0x1a128f0 `all_walk_tauntaun`, 0x1ae8950 `cis_walk_spider`.
BF1: a walker's matrix header word is 0x533a50, a command walker's 0x53e1e8; the pilot is the character whose +0xb8 is
matrix + 0xa0, a gunner one whose seat object's +0xcc is the entity (matrix − 0x144).

**A bot pilots every AT-AT and AT-TE, from the first seconds, and never gets out.**

| game, map | walker | pilot seated at | pilot first seen | then |
|---|---|---|---|---|
| BF2 HOT1 | `imp_walk_atat` ×2 | 21.1 s, 24.0 s (run 2: 17.0 s, 24.0 s) | 8.0 m, 8.5 m from it at 0.9 s, 3.8 s | one AI each to the end of the 240 s run; 452 m and 414 m, (229,51) → (−198,90) and (206,120) → (−200,55), standing 11-13 % of polls; health 60000 → 39549 / 29151 |
| BF2 GEO1 | `rep_walk_atte` | 18.5 s (run 2: 18.8 s) | 70.8 m from it at 2.7 s | one AI to 240 s; 131 m, z 326 → 197 by 140 s, then stands (67 % of polls) |
| BF1 HOT1 | AT-AT ×2 | 12.3 s, 14.2 s (units spawn at 10.1-12.0 s) | 9 m, 8 m | one AI each to the end of the 65 s run, a gunner from 13.1 s / 14.2 s; 103 m and 102 m at 2.0 m/s |
| BF1 GEO1 | AT-TE ×2 | (save states) piloted with two gunners each at 39.2 s and at 60.2 s | – | – |

**The command walker is where its team spawns.** BF2 HOT1: 6 of the 16 Imperials of the opening wave are first seen
8-9 m from an AT-AT (the other ten near (14,0,−23) and (−212,−5,192)), and a respawn at 34.9 s appears 8 m from one. BF1
HOT1: 14 of the 24 Imperials are first seen 8-9 m from an AT-AT (9 and 5). BF2 GEO1: 2 clones of the opening wave and
the respawns at 30.9 s and 42.1 s appear 7-20 m from the AT-TE. (`re-vehicles.md` §4: commandwalker = walker + a
command-post component; its spawn rule is `analysis/re-command-walker-post.md` — card CWSP, 2026-10-02.)

**How far a bot comes for a walker** (XZ distance from where it was first seen to the walker it then boards, and when):
BF1 HOT1 tauntauns 8, 13, 20, 28, 34, 38, 41 m, seated 14.8-25.2 s (4-15 s after spawning); its snow AT-STs 40 m
(25.7 s) and 146 m (37.8 s). BF2 HOT1 tauntauns 14.6 m and 54.9 m (21.1 s, 19.9 s); the first rider walked 48 m away
and turned back at 13 s, as the AT-ATs' pilots did (to 43 m and 50 m, back at 13-17 s). The BF2 snow AT-ST spawns at
45.1 s and was boarded at 49.9 s and 64.6 s (two runs). BF2 GEO1 spiders 60.0 m (9.2 s) and 12.5 m (43.7 s); its AT-TE
70.8 m. So the reach is at least 71 m.

**Piloted / alive, per class** (BF1 HOT1 live; BF2 HOT1 and GEO1, the 240 s runs):

| | 20 s | 30 s | 40 s | 60 s | 120 s | 180 s |
|---|---|---|---|---|---|---|
| BF1 HOT1 tauntaun | 5/8 | 7/8 | 7/8 | 1/3 | | |
| BF1 HOT1 snow AT-ST | 0/2 | 1/2 | 2/2 | 2/2 | | |
| BF1 HOT1 AT-AT | 2/2 | 2/2 | 2/2 | 2/2 | | |
| BF2 HOT1 tauntaun | | 2/2 | | 1/2 | 0/1 | 0/2 |
| BF2 HOT1 snow AT-ST | | – | | 0/1 | 1/1 | 1/1 |
| BF2 HOT1 AT-AT | | 2/2 | | 2/2 | 2/2 | 2/2 |
| BF2 GEO1 spider | | 1/2 | | 2/2 | 1/1 | 1/1 |
| BF2 GEO1 AT-TE | | 1/1 | | 1/1 | 1/1 | 1/1 |

The BF1 save states of §10 read 7/7, 5/5, 3/3 tauntauns at 39.6 / 49.2 / 60.6 s with this seat test, both snow AT-STs
piloted (a pilot and a gunner each — §10's rider test could not see them) and both AT-ATs. The 60 s tauntaun count is
not a stable number: the tauntauns alive fall from 8 to 3 as the rides end (next paragraph).

**A tauntaun ride goes all the way.** BF1 HOT1, seven rides: 282-711 m in 25-46 s (10.8-15.3 m/s). Six ended inside
the run, 4.5-41 m from the Imperial post cp6 (16.3, −15.8): the rider leaves the seat, the tauntaun is gone 1.5-2.2 s
later (six of six), and the rider is alive on foot afterwards (five of five that the run still covered). A dismount
followed by a despawn, or the mount's death throwing its rider, are not told apart [H] (BF1 walker health was not
sampled). BF2 HOT1, where health was sampled: rides of 837 m (48.8 s, from the rebel base to the AT-ATs' start), 451 m,
410 m, 468 m, 270 m and one of 38 m; a rider that gets out leaves a live tauntaun, which stands 10-114 s until another
bot takes it (three riders each on two tauntauns in 240 s); one ride ended with the tauntaun's death. The BF2 snow AT-ST:
one pilot for 137 s, 368 m, standing 42 % of polls. BF2 GEO1 spiders: spells of 88 s (198 m), 72 s (115 m), 145 s (288 m).

**The port against this** (main 22eddb0, `lead_tmp/wai2/census2/`, 4 seeds): no AT-AT or AT-TE is ever boarded — no bot
comes within 168 m of an AT-AT (nobody spawns at it), the AT-TE fails the 40 m seek range (BF1) or the route gain
(BF2) — the snow AT-ST is claimed and never reached (the claimer walks straight into a wall 17.7 m short), and BF1
HOT1's tauntauns read 3/8 at 20-30 s with rides of 158-203 m. Cards WAI2 (build) and CWSP.

**After card WAI2's build** (2026-10-02, `lead_tmp/wai2/v7/`, 4 seeds, the sweep line `walkers: ai_piloted at 30 / 60 /
120 / 180 s`): BF2 HOT1 AT-ATs 2/2 at 120 s and 180 s on 4 of 4 seeds (1/2 at 30 s: the second pilot is seated after
30 s, the PS2's by 24 s), its snow AT-ST 1/1 at 120 s; BF2 GEO1 AT-TE 1/1 at 30-180 s; BF1 HOT1 tauntauns 6/8 at
30 s, snow AT-STs 2/2 and AT-ATs 2/2 from 30 s; BF1 GEO1 AT-TEs 2/2. What was wrong on BF2: a vehicle was located on
the smallest hub disc it stood in whatever its type, and the AT-AT stands in a soldier hub (radius 66.4 m) laid over
the first hub of its own HUGE path (hubs 60-63, three arcs of mask 16), so its route never left that hub.

## 14. Segment look-ahead correction (issue #108, 2026-11-03)

The walker branch in BF2 `FUN_002f1d80:281244–281250` supplies a fixed 20 m
lead to `FUN_002a5fa8:227076–227155`. BF1 `FUN_0026aba8:189721–189732`
uses the same walker branch and distance, calling `FUN_00234830:149022–149135`.
Both helpers flatten the segment ends to the vehicle's height, project its
position on that segment and advance by the lead plus 0.2 m, stopping at the
segment endpoint. This is a segment target; it does not look through a corner
onto the following leg. The PC reference lead, Gametoast `PathFollower`
(`0x006137c0`), corroborates this ordering. The PS2 functions are the basis.

The port already hashes `Route::from`. For walkers it now retains the current
segment's start there as corners are passed, including when a full corner array
is refilled, and carries the reached waypoint into a following final leg.
Walker corner arrival also admits the passed endpoint plane within three
corner radii laterally of the incoming segment. **OURS, PROVISIONAL; measure:**
matched PS2 corner passes after lateral displacement. The bound retains the
spider's crab/turn corridor while preventing a sideways knock from skipping a
detour corner. A pending corner prevents arrival at the leg's final hub/post from skipping that corner. Other
movers retain their existing arrival and leg-start rules. Walker steering uses
the existing segment projection helper with a 20 m lead; its progress window
continues measuring distance to the real node, rather than a moving target.

The port's 1.5 m hub threshold, 1 m corner threshold, drawn hub centres,
post/drop-hub exits and stuck recovery remain named port choices from the
preceding sections. The BF2 spider's measured five-degree crab is retained,
so a 20.2 m look-ahead does not imply zero cross-track error. Leg yaw also trails
body yaw through a sharp turn. Tight-turn fixtures check segment starts, every
corner transition, bounded completion and the final endpoint, rather than
assuming instantaneous leg alignment. Straight-leg fixtures check the actual
steering command. Both run the driving/locomotion consumers beside the unchanged 145
PS2 stick samples. Fresh matched PS2 tight-turn/straight-leg route traces remain
unchecked; source reconstruction and port fixtures are not that acceptance.

## 15. Walker/hover contact priority and hold (issue #108, 2026-11-03)

Fresh reads of BF2 `FUN_0025c2b0:177324–177337` and BF1
`FUN_00217d00:129003–129021` establish the collision writers. Each asks its
priority predicate before recording mission time. Those predicates are BF2
`FUN_002a8c30:229192–229313` and BF1 `FUN_00236e58:150955–151069`.
Both admit another walker or hover. A vehicle over three times the other's
collision radius has priority; the smaller yields, even at rest. At lesser
ratios both horizontal speed squares must exceed 0.0001. The comparison then
uses normalized travel directions and the direction between their centres,
with a strict inequality. Equal head-on travel, and equal-size stationary
blockage, do not qualify. A rear vehicle following another does qualify,
independently of their relative speeds. Crossing priority depends on those
same directional projections, rather than team or entity index.

BF2 `FUN_00262248:181517–181543` and BF1 `FUN_0021c180:132130–132145`
replace the walker's movement destination with its own position for 7.5 s
after a qualifying collision. Aim continues. The PC lead Gametoast
`EntityWalker` (`0x00506ae0`) corroborates that hold. It is a walker hold when
meeting a hover/walker, not evidence for adding the same hold to the hover AI.
The existing RAM runs in §10 did not exercise it; no fresh yield capture is
claimed here.

The port records a hashed fixed-tick deadline, 225 ticks at 30 Hz, only after
a swept hull contact and the priority predicate. Refresh uses the same writer.
The driver pauses route advancement and both progress clocks while held,
continues aiming, and resumes at expiry. Priority is evaluated at the swept
contact position, before clamping velocity; using an attempted position beyond
the obstacle reverses the centre direction on a fast step. The installed model
collision sphere supplies radius; old fixtures without it use the existing
seat-radius approximation.

**Named port response, OURS, PROVISIONAL; measure:** matched original-game
opposing, crossing, following and stationary traffic, including rotating hulls,
source vehicle-subset geometry and original pair impulses. The port now clamps
AI-driven translation at the first contact for ground pairs involving a
walker. Only currently AI-piloted walkers receive the hold; player-driven and
unpiloted vehicles retain the prior locomotion/contact response. The collision
writer facts above do not establish a player hard-stop law. It uses
current SOLDIER-subset hull primitives; a mesh mover uses its conservative world
box against the other hull. This translation sweep does not establish swept
angular collision or the original contact impulse/damage law. Stable vehicle
and part order makes the response deterministic. Separating travel is admitted;
a pre-existing overlap is not claimed to be repaired by this pass.

The port also steers around an occupied route on the nearer side (right on an
exact collinear tie), leaving
its authored route intact. Equal-priority/stationary blockage does not manufacture
a hold. The bypass target sits one own-radius beyond the sum of radii: targeting
the inflated boundary itself made the chord from an existing contact run through
the obstruction, stalling a priority winner beside the held vehicle. Crossing
traffic must also keep its existing side: forcing a left-side approach across
to the right directs that driver through the obstruction. The traffic scan and
temporary steering point use the same effective 20.2 m reach as walker segment projection:
20 m supplied by the walker plus the shared helper's 0.2 m padding. The point is
extended to at least that reach from the driver. It is not an
arrival node: inside the ordinary 4 m arrival branch a sideways-facing plain
walker can walk with the authored reversed turn factor instead of turning in
place. Keeping this directional target outside that branch preserves the stick
law and allows the driver to turn before moving. This bypass
and its clearance are port recovery choices, not reconstructed PS2 trajectories.
Hover/hover and flyer traffic retain their previous paths. Headless port fixtures
cover both ground pair kinds, strict priority, contact-only start, refresh,
expiry, hashed lifetime, route/progress pause and continuous separation during
opposing/crossing/following/stationary passage. Original-game contact/feel
acceptance remains open.

## 16. Bound endgame command-walker reboarding (issue #108 / #86)

The §13 original-game samples keep command crews seated. The port's last-crew
escape is already a deliberate difference (§14 and the `COMMAND_ENDGAME`
measurement plan). It needs a matching autonomous boarding policy: leaving a
terminal command walker, waiting the ordinary 30 s exit cooldown, then boarding
it again can repeat indefinitely when the foot approach remains blocked nearby.
An armed terminal-drop fixture exercises the real claim, drive and step-1c
entry/exit consumers for 120 s and caps each of three crew members at one boarding.
Normal-population retention and recovery to normal boarding are separate controls.
The fixture supplies a retained post order and keeps the foot crew in boarding
reach; it does not claim an original-game crew comparison or a full physics round.

**OURS; measure:** PS2 command-walker last-crew/post-order boarding and unboarding.
An AI crew member's endgame command departure now closes autonomous command
boarding for that member while the same endgame context holds. The departure
latch is hashed, comes from the actual unseat writer, and is carried through
step-1c's retained ride-state write-back. The existing six-unit/ticket predicate
is shared with the pilot's escape decision. Once the team recovers, ordinary
search clears the latch. Explicit boarding orders/calls retain their precedence.
This adds no new gameplay cooldown and does not change human boarding.
